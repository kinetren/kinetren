//! FxThumbService: миниатюры эффектов для пикера (Р.54, п.430).
//!
//! Форма — СЕРВИС по образцу `ThumbService` (`bg.rs` называет его эталоном
//! этой формы): свой поток, condvar-очередь, LIFO + dedup, RAM-LRU,
//! отмена поколением. Каркас задачи (`BgJob`) тут не подходит: задача
//! конечна и имеет прогресс, а миниатюры — бесконечная очередь мелких
//! заявок, живущая, пока открыт пикер.
//!
//! Чего в сервисе НЕТ и быть не должно:
//! * декодеров и sidecar-БД. Кадр-основу разрешает ВЫЗЫВАЮЩИЙ и передаёт
//!   готовой картинкой (или просит встроенный образец). Инвариант №8 в
//!   силе: рисование может только ЕНКЬЮИТЬ заявки, а декодирует чужой
//!   выделенный воркер — `ThumbService`;
//! * персиста. Дорогая половина превью (декод) уже лежит JPEG'ом в
//!   `thumbnails`; наложить цепочку на готовый кадр дешевле, чем прочитать
//!   результат из sqlite, поэтому кэш здесь только RAM и только на сессию.

use eframe::egui;
use kt_model::{FxInstance, MediaId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Опорный размер расчёта — размер тумбы источника (`thumbs::THUMB_W/H`).
/// Считать сразу в размере ячейки нельзя: часть параметров задана в
/// ПИКСЕЛЯХ (`Pixelate.size`, радиусы размытий), и в мелком кадре они
/// показали бы другую СИЛУ эффекта («превью вида, не силы», п.428).
pub const REF_W: u32 = crate::thumbs::THUMB_W;
pub const REF_H: u32 = crate::thumbs::THUMB_H;
/// Хранимый размер ячейки (вдвое больше рисуемого — hidpi).
pub const CELL_W: u32 = 160;
pub const CELL_H: u32 = 90;
/// LRU готовых миниатюр: 160×90 RGBA ≈ 57 КБ, то есть ~14 МБ на потолке.
pub const MAX_READY_FX: usize = 256;
/// Мисклик-гард (Р.54, требование владельца «а только через секунду
/// пытаться делать (вдруг мисклик)»). Ждёт НЕ рендер — он дешёвый и идёт
/// сразу, — а ЗАКАЗЫ декода: просить тумбу недостающего кадра до этого
/// срока значит платить открытием файла за случайный клик.
pub const DECODE_GUARD: std::time::Duration = std::time::Duration::from_millis(800);
/// Пауза между рендерами, когда система занята (игра, экспорт): очередь
/// LIFO, поэтому свежие ячейки всё равно первые.
const BUSY_PAUSE: std::time::Duration = std::time::Duration::from_millis(25);

/// На чём считать миниатюру.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FxThumbBase {
    /// Кадр источника: ключ `ThumbService` — ТОТ ЖЕ, что у филмстрипа
    /// (общий кэш, новых декодов не порождает).
    Thumb(crate::thumbs::ThumbKey),
    /// Встроенный образец (`kt_fx::video::test_pattern`): клип-вложение,
    /// морфер без источника, тумба ещё не приехала — пустых ячеек не
    /// бывает.
    Pattern,
    /// Он же с ПРОЗРАЧНЫМИ полями: `DropShadow` на непрозрачном кадре
    /// тождествен по построению (п.428б).
    PatternAlpha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FxThumbKey {
    pub base: FxThumbBase,
    /// Хэш рецепта: цепочка + момент + камера (см. `recipe_hash`).
    pub recipe: u64,
}

impl FxThumbKey {
    /// Считает ли ячейка кадр этого медиа. Нужно при перенастройке
    /// генератора: картинка изменилась, а ключ — нет (п.412).
    pub fn is_media(&self, id: MediaId) -> bool {
        match self.base {
            FxThumbBase::Thumb(t) => t.media == id,
            FxThumbBase::Pattern | FxThumbBase::PatternAlpha => false,
        }
    }
}

/// Рецепт миниатюры одной строкой. Хэшируется `Debug`-представление, а не
/// serde: у `FxInstance` половина полей под `skip_serializing_if`, и новое
/// поле с умолчанием из JSON выпало бы — из `Debug` не выпадет (тот же
/// приём, что у `fx_chain_hash` в kt-engine).
pub fn recipe_hash(
    chain: &[FxInstance],
    u: f64,
    burst_index: i64,
    cam3d: Option<kt_fx::geom::Cam3d>,
) -> u64 {
    let repr = format!("{chain:?}|{u:?}|{burst_index}|{cam3d:?}|{REF_W}x{REF_H}>{CELL_W}x{CELL_H}");
    xxhash_rust::xxh3::xxh3_64(repr.as_bytes())
}

/// Эффекты, видимые ТОЛЬКО на прозрачности: демо-образцу для них нужны
/// прозрачные поля, иначе ячейка покажет исходный кадр. `DropShadow` на
/// непрозрачном кадре тождествен ПО ПОСТРОЕНИЮ — тень падает из-под
/// альфы, а её нет (п.428б, сторож t544d).
///
/// На РЕАЛЬНОМ кадре события подмены не делаем: там passthrough честен —
/// у непрозрачного клипа тень и правда ничего не меняет, и врать об этом
/// в пикере нельзя.
fn needs_alpha(fx: &[FxInstance]) -> bool {
    fx.iter().any(|f| matches!(f, FxInstance::DropShadow { .. }))
}

/// Заявка на миниатюру. Кадр-основа приходит ГОТОВОЙ: воркер не ходит ни
/// в чужие кэши, ни в декодеры.
pub struct FxThumbJob {
    pub key: FxThumbKey,
    /// `None` — база из `FxThumbBase::Pattern*`.
    pub base_img: Option<Arc<egui::ColorImage>>,
    /// Цепочка с УЖЕ разрешёнными кривыми (значения на момент курсора).
    pub chain: Vec<FxInstance>,
    pub u: f64,
    pub burst_index: i64,
    pub cam3d: Option<kt_fx::geom::Cam3d>,
    pub generation: u64,
}

/// Очередь заявок: LIFO + dedup по ключу + отмена поколением. Отдельный
/// тип, а не пара полей — по образцу `SourceLru`: так она проверяется
/// юнит-тестом без потока и без egui (t546).
#[derive(Default)]
pub(crate) struct JobQueue {
    jobs: VecDeque<FxThumbJob>,
    seen: HashSet<FxThumbKey>,
}

impl JobQueue {
    pub(crate) fn seen_key(&self, key: &FxThumbKey) -> bool {
        self.seen.contains(key)
    }
}

impl JobQueue {
    /// true — заявка принята (новая). Дубликат отбрасывается: одна и та
    /// же ячейка просится на каждом кадре интерфейса, пока не готова.
    pub(crate) fn push(&mut self, job: FxThumbJob) -> bool {
        if !self.seen.insert(job.key) {
            return false;
        }
        self.jobs.push_back(job);
        true
    }

    /// Свежие первыми (`pop_back`); заявки прошлых поколений выбрасываются
    /// молча — пикер закрыли или курсор уехал, считать их незачем.
    pub(crate) fn pop(&mut self, generation: u64) -> Option<FxThumbJob> {
        while let Some(job) = self.jobs.pop_back() {
            self.seen.remove(&job.key);
            if job.generation >= generation {
                return Some(job);
            }
        }
        None
    }

    pub(crate) fn len(&self) -> usize {
        self.jobs.len()
    }
}

pub(crate) struct Shared {
    cond: std::sync::Condvar,
    queue: Mutex<JobQueue>,
    ready: Mutex<HashMap<FxThumbKey, Arc<egui::ColorImage>>>,
    order: Mutex<VecDeque<FxThumbKey>>,
    generation: AtomicU64,
    /// Система занята (игра/экспорт) — «сбавить обороты» (правило 12
    /// чек-листа: фоновая работа не мешает).
    busy: AtomicBool,
    /// Владелец умер — поток обязан выйти (доктрина `bg.rs`: задача и
    /// сервис гасятся вместе с владельцем).
    stop: AtomicBool,
}

impl Shared {
    fn new() -> Arc<Shared> {
        Arc::new(Shared {
            cond: std::sync::Condvar::new(),
            queue: Mutex::new(JobQueue::default()),
            ready: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            generation: AtomicU64::new(0),
            busy: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        })
    }

    fn publish(&self, key: FxThumbKey, img: Arc<egui::ColorImage>) {
        let mut ready = self.ready.lock().unwrap();
        let mut order = self.order.lock().unwrap();
        if ready.insert(key, img).is_none() {
            order.push_back(key);
        }
        while order.len() > MAX_READY_FX {
            if let Some(old) = order.pop_front() {
                ready.remove(&old);
            }
        }
    }
}

/// Кэш egui-текстур ячеек. Живёт В СЕРВИСЕ, а не в `KtApp`, по причине
/// заимствований: окна FX и морфера держат внутри ui-замыкания `&self.model`
/// (а морфер ещё и зовёт свои методы по `&self`), и `&mut` на поле
/// приложения туда уже не пролезает. `RefCell`, а не `Mutex`: текстуры
/// трогает ТОЛЬКО UI-поток — воркер о них не знает, он отдаёт `ColorImage`.
#[derive(Default)]
struct TexCache {
    map: HashMap<FxThumbKey, egui::TextureHandle>,
    lru: VecDeque<FxThumbKey>,
}

pub struct FxThumbService {
    shared: Arc<Shared>,
    tex: std::cell::RefCell<TexCache>,
}

impl FxThumbService {
    pub fn start() -> FxThumbService {
        let shared = Shared::new();
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("fxthumbs".into())
            .spawn(move || worker(sh))
            .expect("spawn fxthumbs");
        FxThumbService { shared, tex: std::cell::RefCell::new(TexCache::default()) }
    }

    /// Поставить заявку (dedup). Отрисовка никогда не блокируется.
    pub fn request(&self, job: FxThumbJob) {
        if self.shared.ready.lock().unwrap().contains_key(&job.key) {
            return;
        }
        let mut q = self.shared.queue.lock().unwrap();
        if q.push(job) {
            drop(q);
            self.shared.cond.notify_one();
        }
    }

    pub fn get(&self, key: &FxThumbKey) -> Option<Arc<egui::ColorImage>> {
        self.shared.ready.lock().unwrap().get(key).cloned()
    }

    /// Текущее поколение (его несут заявки).
    pub fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::Relaxed)
    }

    /// Пикер закрыли / курсор уехал: всё, что в очереди, считать больше
    /// незачем. ГОТОВЫЕ миниатюры остаются — их LRU и так ограничен, а
    /// повторное открытие пикера обычно просит те же ячейки.
    pub fn bump_generation(&self) {
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        self.shared.cond.notify_all();
    }

    /// Медиа осталось тем же, а КАРТИНКА изменилась (правка параметров
    /// генератора, Р.53 п.412): ключ ячейки помнит `ThumbKey`, который от
    /// параметров не зависит, поэтому готовые миниатюры этого медиа
    /// протухли. Зовётся рядом с `ThumbService::retune_media`.
    pub fn invalidate_media(&self, id: MediaId) {
        self.shared.ready.lock().unwrap().retain(|k, _| !k.is_media(id));
        self.shared.order.lock().unwrap().retain(|k| !k.is_media(id));
        let mut tex = self.tex.borrow_mut();
        tex.map.retain(|k, _| !k.is_media(id));
        tex.lru.retain(|k| !k.is_media(id));
    }

    /// Текстура готовой ячейки (заливается при первом показе) либо `None`.
    fn texture_for(&self, ctx: &egui::Context, key: FxThumbKey) -> Option<egui::TextureId> {
        if let Some(h) = self.tex.borrow().map.get(&key) {
            return Some(h.id());
        }
        let img = self.get(&key)?;
        let h =
            ctx.load_texture(format!("fxthumb{}", key.recipe), img, egui::TextureOptions::LINEAR);
        let id = h.id();
        let mut tex = self.tex.borrow_mut();
        tex.map.insert(key, h);
        tex.lru.push_back(key);
        while tex.lru.len() > MAX_READY_FX {
            if let Some(old) = tex.lru.pop_front() {
                tex.map.remove(&old);
            }
        }
        Some(id)
    }

    /// Занятость системы: играем или идёт экспорт.
    pub fn set_busy(&self, busy: bool) {
        self.shared.busy.store(busy, Ordering::Relaxed);
    }

    /// Есть ли незакрытые заявки. Гейт перерисовки вешается на ЭТО, а не
    /// на «пикер открыт»: иначе открытое меню держало бы 60 fps в простое
    /// (дефект t38).
    pub fn pending(&self) -> bool {
        self.shared.queue.lock().unwrap().len() > 0
    }

    /// Уже в очереди? Чтобы отрисовка не собирала заявку заново на каждом
    /// кадре, пока ячейка считается (dedup внутри очереди спасает от
    /// работы, но не от сборки задания).
    pub fn queued(&self, key: &FxThumbKey) -> bool {
        self.shared.queue.lock().unwrap().seen_key(key)
    }
}

/// Контекст ячеек пикера на ОДИН кадр: сервис, кэш текстур и всё, что
/// задаёт рецепт (кадр-основа, цепочка клипа, момент, камера).
///
/// Живёт снаружи `Window::show`, а внутрь отдаётся одной ссылкой: окно
/// держит `&self.model`, и разъехаться эти два заимствования не могут —
/// поля разные (тот же приём, что у буферов копипаста в `fx_editor`).
pub struct PickerCtx<'a> {
    pub svc: &'a FxThumbService,
    /// На чём считать (кадр источника / встроенный образец).
    pub base: FxThumbBase,
    /// Готовый кадр-основа; для `Pattern*` — `None`.
    pub base_img: Option<Arc<egui::ColorImage>>,
    /// Цепочка КЛИПА с уже разрешёнными кривыми — то, что стоит на
    /// событии сейчас. Кандидат ячейки дописывается в хвост, поэтому
    /// картинка показывает результат клика, а не эффект «в вакууме».
    pub chain: Vec<FxInstance>,
    pub u: f64,
    pub burst_index: i64,
    pub cam3d: Option<kt_fx::geom::Cam3d>,
    pub generation: u64,
    /// Переиспользуемый буфер «цепочка + кандидат»: ячеек в полоске
    /// десятки, и Vec на каждую был бы аллокацией в draw-пути на кадр
    /// (инвариант №12).
    full: Vec<FxInstance>,
}

impl<'a> PickerCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        svc: &'a FxThumbService,
        base: FxThumbBase,
        base_img: Option<Arc<egui::ColorImage>>,
        chain: Vec<FxInstance>,
        u: f64,
        burst_index: i64,
        cam3d: Option<kt_fx::geom::Cam3d>,
    ) -> PickerCtx<'a> {
        let generation = svc.generation();
        PickerCtx {
            svc,
            base,
            base_img,
            chain,
            u,
            burst_index,
            cam3d,
            generation,
            full: Vec::new(),
        }
    }

    /// Текстура ячейки для цепочки «клип + кандидат», либо `None` — тогда
    /// заявка поставлена и ячейка рисуется плашкой до готовности.
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        extra: &[FxInstance],
    ) -> Option<egui::TextureId> {
        self.full.clear();
        self.full.extend_from_slice(&self.chain);
        self.full.extend_from_slice(extra);
        let base = match self.base {
            FxThumbBase::Pattern if needs_alpha(&self.full) => FxThumbBase::PatternAlpha,
            b => b,
        };
        let key = FxThumbKey {
            base,
            recipe: recipe_hash(&self.full, self.u, self.burst_index, self.cam3d),
        };
        if let Some(id) = self.svc.texture_for(ctx, key) {
            return Some(id);
        }
        if !self.svc.queued(&key) {
            self.svc.request(FxThumbJob {
                key,
                base_img: self.base_img.clone(),
                chain: self.full.clone(),
                u: self.u,
                burst_index: self.burst_index,
                cam3d: self.cam3d,
                generation: self.generation,
            });
        }
        None
    }
}

impl Drop for FxThumbService {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.cond.notify_all();
    }
}

/// Разрешены ли ЗАКАЗЫ декода (тумба/постер) — мисклик-гард. Чистая
/// функция: часы приходят снаружи, поэтому тест не спит.
pub fn decode_orders_allowed(since_open: std::time::Duration) -> bool {
    since_open >= DECODE_GUARD
}

/// `ColorImage` → `Frame`. `Color32` хранит цвет ПРЕМУЛЬТИПЛИЦИРОВАННЫМ,
/// а цепочка работает с прямой альфой (правило альфы, пп.417-418): у
/// JPEG-тумбы альфа сплошь 255 и это тождество, но у кадра генератора-
/// титра — нет, и без разворота тёмная кромка приехала бы в цепочку.
fn frame_from(img: &egui::ColorImage) -> kt_fx::video::Frame {
    let mut rgba = img.as_raw().to_vec();
    let has_alpha = rgba.chunks_exact(4).any(|p| p[3] != 255);
    if has_alpha {
        kt_fx::video::unpremultiply(&mut rgba);
    }
    kt_fx::video::Frame { w: img.width() as u32, h: img.height() as u32, rgba, has_alpha }
}

/// Рендер одной заявки. Вынесен из воркера, чтобы тест мог позвать его
/// напрямую — без потока и без ожиданий.
pub(crate) fn render_job(
    job: &FxThumbJob,
    scratch: &mut kt_fx::video::FxScratch,
) -> egui::ColorImage {
    let base = match (&job.base_img, job.key.base) {
        (Some(img), _) => frame_from(img),
        (None, FxThumbBase::PatternAlpha) => kt_fx::video::test_pattern(REF_W, REF_H, true),
        // база не приехала, а ключ говорит «кадр источника» — считаем на
        // образце: пустых ячеек не бывает (ключ при этом чужой, и когда
        // кадр приедет, UI попросит его под своим ключом)
        (None, _) => kt_fx::video::test_pattern(REF_W, REF_H, false),
    };
    // расчёт на ОПОРНОМ размере, хранение — в размере ячейки (п.428)
    let full = kt_fx::video::render_fx_thumb(
        &base,
        &job.chain,
        REF_W,
        REF_H,
        job.u,
        job.burst_index,
        job.cam3d,
        scratch,
    );
    let mut cell = scratch.take((CELL_W as usize) * (CELL_H as usize) * 4);
    kt_fx::video::fit_into(&full, CELL_W, CELL_H, &mut cell);
    let img = egui::ColorImage::from_rgba_unmultiplied([CELL_W as usize, CELL_H as usize], &cell);
    scratch.put(cell);
    scratch.put(full.rgba);
    img
}

fn worker(shared: Arc<Shared>) {
    let mut scratch = kt_fx::video::FxScratch::default();
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        let job = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    return;
                }
                let cur = shared.generation.load(Ordering::Relaxed);
                if let Some(job) = q.pop(cur) {
                    break job;
                }
                let (guard, _t) =
                    shared.cond.wait_timeout(q, std::time::Duration::from_secs(1)).unwrap();
                q = guard;
            }
        };
        if shared.ready.lock().unwrap().contains_key(&job.key) {
            continue;
        }
        let img = render_job(&job, &mut scratch);
        // поколение могло сменится ПОКА считали: публиковать всё равно
        // можно (ключ адресует ровно эту картинку), а вот считать
        // следующие устаревшие заявки — нет, их выбросит pop()
        shared.publish(job.key, Arc::new(img));
        if shared.busy.load(Ordering::Relaxed) {
            std::thread::sleep(BUSY_PAUSE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(recipe: u64) -> FxThumbKey {
        FxThumbKey { base: FxThumbBase::Pattern, recipe }
    }

    fn job(recipe: u64, generation: u64) -> FxThumbJob {
        FxThumbJob {
            key: key(recipe),
            base_img: None,
            chain: vec![],
            u: 0.5,
            burst_index: 90_000,
            cam3d: None,
            generation,
        }
    }

    /// Тест 546 (п.430): очередь миниатюр — LIFO, dedup и отмена
    /// поколением. Дубликат обязан отбрасываться: одна и та же ячейка
    /// просится на КАЖДОМ кадре интерфейса, пока не готова, и без dedup
    /// очередь росла бы пропорционально fps.
    #[test]
    fn t546_queue_is_lifo_dedup_and_cancels_by_generation() {
        let mut q = JobQueue::default();
        assert!(q.push(job(1, 0)));
        assert!(q.push(job(2, 0)));
        assert!(!q.push(job(1, 0)), "duplicate key was queued twice");
        assert_eq!(q.len(), 2);
        // свежие первыми
        assert_eq!(q.pop(0).expect("job").key.recipe, 2);
        assert_eq!(q.pop(0).expect("job").key.recipe, 1);
        assert!(q.pop(0).is_none());

        // ключ, снятый с очереди, можно заказать снова (иначе повторное
        // открытие пикера не дозаказало бы то, что не успело посчитаться)
        assert!(q.push(job(1, 0)));

        // отмена поколением: заявки прошлых поколений выбрасываются
        assert!(q.push(job(3, 1)));
        assert_eq!(q.pop(1).expect("fresh job").key.recipe, 3);
        assert!(q.pop(1).is_none(), "a stale job survived the generation bump");
    }

    /// Публикация и LRU: потолок держится, а вытесняется САМОЕ СТАРОЕ.
    #[test]
    fn t546b_ready_lru_has_a_ceiling() {
        let shared = Shared::new();
        let img = || Arc::new(egui::ColorImage::new([2, 2], vec![egui::Color32::BLACK; 4]));
        for i in 0..(MAX_READY_FX as u64 + 10) {
            shared.publish(key(i), img());
        }
        assert_eq!(shared.ready.lock().unwrap().len(), MAX_READY_FX);
        assert!(shared.ready.lock().unwrap().get(&key(0)).is_none(), "oldest survived");
        assert!(shared.ready.lock().unwrap().get(&key(MAX_READY_FX as u64 + 9)).is_some());
        // повторная публикация того же ключа не растит очередь порядка
        let before = shared.order.lock().unwrap().len();
        shared.publish(key(MAX_READY_FX as u64 + 9), img());
        assert_eq!(shared.order.lock().unwrap().len(), before);
    }

    /// Рецепт различает ровно то, от чего зависит картинка.
    #[test]
    fn t546c_recipe_separates_what_changes_the_picture() {
        let a = vec![FxInstance::blur(8)];
        let b = vec![FxInstance::blur(9)];
        let base = recipe_hash(&a, 0.5, 90_000, None);
        assert_eq!(base, recipe_hash(&a, 0.5, 90_000, None), "recipe is not stable");
        assert_ne!(base, recipe_hash(&b, 0.5, 90_000, None), "chain change not seen");
        assert_ne!(base, recipe_hash(&a, 0.6, 90_000, None), "clip progress not seen");
        assert_ne!(base, recipe_hash(&a, 0.5, 91_000, None), "tick (noise seed) not seen");
        let cam = kt_fx::geom::Cam3d { cam: kt_model::CameraParams::default(), world_scale: 0.5 };
        assert_ne!(base, recipe_hash(&a, 0.5, 90_000, Some(cam)), "camera not seen");
        let mut cam2 = cam;
        cam2.world_scale = 0.25;
        assert_ne!(
            recipe_hash(&a, 0.5, 90_000, Some(cam)),
            recipe_hash(&a, 0.5, 90_000, Some(cam2)),
            "world scale not seen"
        );
        // пустая цепочка — тоже рецепт (чистый кадр в ячейке «шаблона»)
        assert_ne!(base, recipe_hash(&[], 0.5, 90_000, None));
    }

    /// Рендер заявки: ячейка нужного размера, цепочка ВИДНА, а тот же
    /// рецепт даёт тот же результат (иначе полоска мерцала бы).
    #[test]
    fn t546d_render_job_gives_a_cell_and_the_chain_is_visible() {
        let mut s = kt_fx::video::FxScratch::default();
        let plain = render_job(&job(0, 0), &mut s);
        assert_eq!(plain.size, [CELL_W as usize, CELL_H as usize]);
        let mut with_fx = job(1, 0);
        with_fx.chain = vec![FxInstance::Mirror { horizontal: true, vertical: false }];
        let mirrored = render_job(&with_fx, &mut s);
        assert_ne!(plain.as_raw(), mirrored.as_raw(), "the chain changed nothing");
        // второй прогон на ТОМ ЖЕ пуле — байт-в-байт (грязный пул, t68)
        let again = render_job(&with_fx, &mut s);
        assert_eq!(mirrored.as_raw(), again.as_raw(), "a reused pool changed the bytes");
        // альфа-образец отличается от непрозрачного
        let mut alpha = job(2, 0);
        alpha.key.base = FxThumbBase::PatternAlpha;
        assert_ne!(plain.as_raw(), render_job(&alpha, &mut s).as_raw());
    }

    /// Сервис целиком: заявка доезжает до `get()`, а `invalidate_media`
    /// выбрасывает ячейки ИМЕННО этого медиа (правка генератора, п.412).
    #[test]
    fn t546e_service_delivers_and_invalidates_by_media() {
        let svc = FxThumbService::start();
        let k = key(77);
        svc.request(job(77, svc.generation()));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let img = loop {
            if let Some(img) = svc.get(&k) {
                break img;
            }
            assert!(std::time::Instant::now() < deadline, "the worker never delivered");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(img.size, [CELL_W as usize, CELL_H as usize]);
        assert!(!svc.pending(), "queue is not empty after delivery");

        // ячейки на кадре медиа №1 и на образце
        let mine = FxThumbKey {
            base: FxThumbBase::Thumb(crate::thumbs::ThumbKey { media: MediaId(1), t: 0 }),
            recipe: 5,
        };
        let other = FxThumbKey {
            base: FxThumbBase::Thumb(crate::thumbs::ThumbKey { media: MediaId(2), t: 0 }),
            recipe: 5,
        };
        let px = Arc::new(egui::ColorImage::new([2, 2], vec![egui::Color32::BLACK; 4]));
        svc.shared.publish(mine, px.clone());
        svc.shared.publish(other, px.clone());
        svc.invalidate_media(MediaId(1));
        assert!(svc.get(&mine).is_none(), "retuned media kept its stale thumb");
        assert!(svc.get(&other).is_some(), "invalidation hit the wrong media");
        assert!(svc.get(&k).is_some(), "invalidation hit the built-in pattern");
    }

    /// Путь, которым ходит ОТРИСОВКА: `PickerCtx::texture` обязана
    /// сначала поставить заявку и вернуть `None`, а после работы воркера —
    /// отдать текстуру. Без этого теста «ячейки чёрные» видно только
    /// живым сценарием: харнесс-тест вёрстки находит ячейку по подписи и
    /// про картинку внутри не знает ничего.
    #[test]
    fn t546g_picker_ctx_requests_then_gets_a_texture() {
        let svc = FxThumbService::start();
        let ctx = egui::Context::default();
        let mut pv = PickerCtx::new(&svc, FxThumbBase::Pattern, None, vec![], 0.5, 90_000, None);
        let fx = [FxInstance::blur(8)];
        assert!(pv.texture(&ctx, &fx).is_none(), "текстура нашлась до всякого рендера");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if pv.texture(&ctx, &fx).is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "ячейка так и осталась без текстуры: заявка не доехала до воркера \
                 или ключ заявки не совпал с ключом поиска"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // повторный вызов берёт ГОТОВУЮ текстуру, а не заливает заново
        let first = pv.texture(&ctx, &fx);
        let second = pv.texture(&ctx, &fx);
        assert_eq!(first, second, "текстура ячейки пересоздаётся на каждом кадре");
    }

    /// Мисклик-гард сторожит ЗАКАЗЫ ДЕКОДА, а не рендер.
    #[test]
    fn t546f_decode_guard_waits_but_rendering_does_not() {
        use std::time::Duration;
        assert!(!decode_orders_allowed(Duration::from_millis(0)));
        assert!(!decode_orders_allowed(Duration::from_millis(799)));
        assert!(decode_orders_allowed(DECODE_GUARD));
        assert!(decode_orders_allowed(Duration::from_secs(3)));
    }
}
