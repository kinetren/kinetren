//! ThumbService: филмстрип-тумбнейлы для видео-клипов.
//!
//! НЕ трогает декодеры воспроизведения (поправка к инварианту №8: paint
//! может только ЕНКЬЮИТЬ запросы этому выделенному воркеру). Свой
//! короткоживущий FfmpegSource на медиа; jpeg персистится в sidecar
//! индекс-БД (таблица thumbnails) — повторное открытие проекта рисует
//! полосу без декода.

use crate::util::{open_source, resolve_media_path};
use eframe::egui;
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::Tick;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const THUMB_W: u32 = 320;
pub const THUMB_H: u32 = 180;
/// LRU готовых текстур (порт kMaxPreviewCacheEntries=384).
pub const MAX_READY: usize = 384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThumbKey {
    pub media: MediaId,
    /// Момент в тиках, выровненный вызывающим по сетке шага.
    pub t: i64,
}

/// Шаг источника для тумбнейлов (порт videoClipThumbnailSourceStepMs):
/// один тумб при клипе ≤5 c, иначе dur/slots, зажатый в 1..15 c.
/// Шаг тумбов КВАНТУЕТСЯ к фиксированной лестнице (п.2): непрерывная
/// зависимость от зума давала новый набор ключей на каждый щелчок колеса —
/// филмстрип перегружался целиком. Соседние ступени переиспользуют кэш
/// (ключи крупной ступени — подмножество мелкой).
pub const THUMB_STEP_LADDER: [i64; 7] =
    [45_000, 90_000, 180_000, 450_000, 1_350_000, 4_050_000, 10_800_000]; // 0.5/1/2/5/15/45 c/2 м

/// Ключ тумбы: момент источника, квантованный к сетке шага ОТ НУЛЯ ФАЙЛА
/// (п.11 р.6, тест 62). Раньше таймлайн квантовал от source_in — при
/// source_in, не кратном шагу, бин и таймлайн плодили РАЗНЫЕ ключи на один
/// момент файла (двойной декод + раздутая sidecar-БД).
pub fn thumb_key_t(source_in: i64, timeline_local: i64, step: i64) -> i64 {
    (((source_in + timeline_local).max(0)) / step.max(1)) * step.max(1)
}

pub fn thumb_source_step(clip_dur: Tick, inner_width_px: f32, thumb_w_px: f32) -> i64 {
    if clip_dur.0 <= 5 * 90_000 {
        // короткий клип: один тумб, ключ стабилен (кратен 0.5 c)
        return THUMB_STEP_LADDER[0].max(clip_dur.0 / 2).max(45_000);
    }
    let slots = (inner_width_px / thumb_w_px).max(1.0) as i64;
    let rough = clip_dur.0 / slots.max(1);
    // ближайшая ступень СВЕРХУ (реже — дешевле)
    *THUMB_STEP_LADDER
        .iter()
        .find(|&&s| s >= rough)
        .unwrap_or(&THUMB_STEP_LADDER[THUMB_STEP_LADDER.len() - 1])
}

/// Состояние тумбы для UI: Failed НЕ перезаказывается (п.1 р.7 — вечный
/// цикл «запрос → seek+reopen → ошибка» раскручивал счётчик FFmpeg-open).
#[derive(Debug, PartialEq)]
pub enum ThumbState {
    Ready(Arc<egui::ColorImage>),
    Failed,
    Missing,
}

/// Кэш открытых источников воркера: LRU с вытеснением СТАРЕЙШЕГО (п.1 р.7:
/// прежний `clear()` при >2 означал полное переоткрытие каждого файла при
/// ≥4 активных медиа — шторм FFmpeg-open). Generic для юнит-теста без FFmpeg.
pub(crate) struct SourceLru<V> {
    cap: usize,
    map: HashMap<MediaId, V>,
    order: VecDeque<MediaId>,
}

impl<V> SourceLru<V> {
    pub fn new(cap: usize) -> Self {
        SourceLru { cap: cap.max(1), map: HashMap::new(), order: VecDeque::new() }
    }
    pub fn contains(&self, id: MediaId) -> bool {
        self.map.contains_key(&id)
    }
    pub fn get_mut(&mut self, id: MediaId) -> Option<&mut V> {
        if self.map.contains_key(&id) {
            self.order.retain(|k| *k != id);
            self.order.push_back(id);
        }
        self.map.get_mut(&id)
    }
    pub fn insert(&mut self, id: MediaId, v: V) {
        self.map.insert(id, v);
        self.order.retain(|k| *k != id);
        self.order.push_back(id);
        while self.map.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

/// Очередь воркера: LIFO пар (ключ, near-допуск) + dedup-множество ключей.
type ThumbQueue = (VecDeque<(ThumbKey, i64)>, HashSet<ThumbKey>);

/// Лимит одновременно открытых источников тумб-воркера.
const MAX_OPEN_SOURCES: usize = 4;
/// Потолок негативного кэша тумб (переполнение — полный clear, дёшево).
const MAX_FAILED: usize = 4096;

struct Shared {
    /// Длительности медиа. `None` = «пробовали открыть, не вышло» —
    /// повторная заявка НЕ ставится (п.1 р.7: вечный probe-цикл открывал
    /// файл на каждом UI-кадре).
    durations: Mutex<HashMap<MediaId, Option<Tick>>>,
    /// Явные PENDING-заявки на длительность (снимает воркер, публикуя
    /// результат — успех или неудачу).
    probe: Mutex<std::collections::HashSet<MediaId>>,
    cond: std::sync::Condvar,
    /// LIFO + dedup по ключу; второй элемент пары — допуск near-поиска в БД
    /// (тики): 0 = только точный ключ (таймлайн), >0 — скраб бина может
    /// показать ближайшую готовую тумбу (п.14 р.7). ThumbKey не расширяем —
    /// общий кэш бин↔таймлайн (тест 62).
    queue: Mutex<ThumbQueue>,
    ready: Mutex<HashMap<ThumbKey, Arc<egui::ColorImage>>>,
    order: Mutex<VecDeque<ThumbKey>>, // для LRU-выселения ready
    /// Негативный кэш: ключи, по которым открытие/декод/энкод упали.
    failed: Mutex<HashSet<ThumbKey>>,
    /// Постеры: момент самого информативного кадра медиа (п.4 р.9).
    /// `None` внутри — «искали, информативного кадра нет» (аудио/битое).
    posters: Mutex<HashMap<MediaId, Option<i64>>>,
    /// Заявки на поиск постера (обслуживаются после тумб).
    poster_jobs: Mutex<std::collections::VecDeque<MediaId>>,
    /// Есть ли у медиа видео/аудио (публикует probe-ветка воркера).
    kinds: Mutex<HashMap<MediaId, (bool, bool)>>,
    /// ЖИВАЯ карта медиа: пополняется при добавлении файлов (п.3 — раньше
    /// карта замораживалась при старте и новые медиа не получали тумбы).
    media: Mutex<HashMap<MediaId, String>>,
}

pub struct ThumbService {
    shared: Arc<Shared>,
}

impl ThumbService {
    pub fn start(project: &Project, project_path: Option<PathBuf>) -> ThumbService {
        let shared = Arc::new(Shared {
            durations: Mutex::new(HashMap::new()),
            probe: Mutex::new(std::collections::HashSet::new()),
            cond: std::sync::Condvar::new(),
            queue: Mutex::new((VecDeque::new(), HashSet::new())),
            ready: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            failed: Mutex::new(HashSet::new()),
            posters: Mutex::new(HashMap::new()),
            poster_jobs: Mutex::new(std::collections::VecDeque::new()),
            kinds: Mutex::new(HashMap::new()),
            media: Mutex::new(HashMap::new()),
        });
        let media: HashMap<MediaId, String> = {
            let base = project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
            project
                .media
                .iter()
                .filter(|m| !m.path.starts_with("synth:"))
                .map(|m| (m.id, resolve_media_path(base.as_deref(), &m.path)))
                .collect()
        };
        *shared.media.lock().unwrap() = media;
        let db_path = project_path.map(|p| p.with_extension("ktindex.db"));
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("thumbs".into())
            .spawn(move || worker(sh, db_path))
            .expect("spawn thumbs");
        ThumbService { shared }
    }

    /// Поставить в очередь (dedup); отрисовка никогда не блокируется.
    /// Зарегистрировать новое медиа (добавление файла в проект) — тумбы
    /// начинают строиться сразу, без перезапуска сервиса.
    pub fn add_media(&self, id: MediaId, resolved_path: String) {
        if !resolved_path.starts_with("synth:") {
            self.shared.media.lock().unwrap().insert(id, resolved_path);
            // новое/переустановленное медиа: негативные кэши устарели
            self.invalidate_media(id);
        }
    }

    /// Сброс негативных кэшей медиа (файл снова онлайн / переустановлен).
    pub fn invalidate_media(&self, id: MediaId) {
        self.shared.durations.lock().unwrap().remove(&id);
        self.shared.failed.lock().unwrap().retain(|k| k.media != id);
    }

    pub fn request(&self, key: ThumbKey) {
        self.request_near(key, 0);
    }

    /// Заявка с допуском near-поиска в sidecar-БД: воркер может показать
    /// БЛИЖАЙШУЮ готовую тумбу в пределах tol тиков вместо декода (скраб
    /// бина, п.14 р.7 — тумбы филмстрипа с шагом 0.5-2 c переиспользуются).
    pub fn request_near(&self, key: ThumbKey, tol: i64) {
        if self.shared.failed.lock().unwrap().contains(&key) {
            return; // негативный кэш: не раскручивать декод заново
        }
        let mut q = self.shared.queue.lock().unwrap();
        if q.1.insert(key) {
            q.0.push_back((key, tol)); // pop_back = LIFO: свежие первыми
            drop(q);
            self.shared.cond.notify_one();
        }
    }

    pub fn get(&self, key: &ThumbKey) -> Option<Arc<egui::ColorImage>> {
        self.shared.ready.lock().unwrap().get(key).cloned()
    }

    /// Состояние ключа: Failed НЕ стоит перезаказывать (бин не должен
    /// держать вечный repaint-цикл по нему).
    pub fn state(&self, key: &ThumbKey) -> ThumbState {
        if let Some(img) = self.get(key) {
            return ThumbState::Ready(img);
        }
        if self.shared.failed.lock().unwrap().contains(key) {
            return ThumbState::Failed;
        }
        ThumbState::Missing
    }

    /// Момент постера (информативного кадра) либо ЗАЯВКА на его поиск —
    /// ровно одна на медиа за сессию (п.4 р.9).
    pub fn poster_t_or_request(&self, id: MediaId) -> Option<i64> {
        if let Some(p) = self.shared.posters.lock().unwrap().get(&id) {
            return *p;
        }
        let mut jobs = self.shared.poster_jobs.lock().unwrap();
        if !jobs.contains(&id) {
            jobs.push_back(id);
            self.shared.cond.notify_one();
        }
        None
    }

    /// Есть ли у медиа видео и аудио (для карточки бина: аудио рисуем
    /// волноформой, а не гоняем тумб-путь впустую).
    pub fn kinds(&self, id: MediaId) -> Option<(bool, bool)> {
        self.shared.kinds.lock().unwrap().get(&id).copied()
    }

    /// Длительность медиа, если воркер уже открывал источник (п.11 р.6).
    pub fn duration(&self, id: MediaId) -> Option<Tick> {
        self.shared.durations.lock().unwrap().get(&id).copied().flatten()
    }

    /// Длительность либо ОДНОРАЗОВАЯ заявка на неё: повторная заявка не
    /// ставится ни при pending (probe), ни после неудачи (durations=None) —
    /// п.1 р.7: прежний вариант открывал файл на каждом UI-кадре.
    pub fn duration_or_probe(&self, id: MediaId) -> Option<Tick> {
        if let Some(d) = self.shared.durations.lock().unwrap().get(&id) {
            return *d;
        }
        if self.shared.probe.lock().unwrap().insert(id) {
            self.shared.cond.notify_one();
        }
        None
    }
}

fn worker(shared: Arc<Shared>, db_path: Option<PathBuf>) {
    let mut db = db_path.and_then(|p| kt_media::index::MediaIndexDb::open(&p).ok());
    // LRU открытых источников (п.1 р.7: раньше clear() при >2)
    let mut sources: SourceLru<Box<dyn kt_media::MediaSource>> = SourceLru::new(MAX_OPEN_SOURCES);
    let mut db_ids: HashMap<MediaId, i64> = HashMap::new();
    loop {
        // condvar вместо 33 пробуждений/с на пустой очереди (п.1);
        // probe-заявки будят цикл и БЕЗ ключей в очереди (п.1 р.7 —
        // иначе они голодают, когда failed-фильтр опустошает очередь)
        let next = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if let Some(kt) = q.0.pop_back() {
                    q.1.remove(&kt.0);
                    break Some(kt);
                }
                if !shared.probe.lock().unwrap().is_empty()
                    || !shared.poster_jobs.lock().unwrap().is_empty()
                {
                    break None;
                }
                let (guard, _timeout) =
                    shared.cond.wait_timeout(q, std::time::Duration::from_secs(2)).unwrap();
                q = guard;
            }
        };
        // заявки на длительность (скраб бина): пакетный индекс делает
        // открытие дешёвым, декода нет. Результат публикуется ВСЕГДА —
        // и неудача тоже (None), чтобы UI не заказывал заново (п.1 р.7).
        let probes: Vec<MediaId> = shared.probe.lock().unwrap().drain().collect();
        for pid in probes {
            if shared.durations.lock().unwrap().contains_key(&pid) {
                continue;
            }
            let dur = shared.media.lock().unwrap().get(&pid).cloned().and_then(|path| {
                kt_media::ffmpeg::FfmpegSource::open_indexed(&path, db.as_mut()).ok().map(|src| {
                    let info = src.info();
                    // вид медиа достаётся бесплатно: источник уже открыт
                    shared
                        .kinds
                        .lock()
                        .unwrap()
                        .insert(pid, (info.has_video, info.has_audio));
                    info.duration
                })
            });
            shared.durations.lock().unwrap().insert(pid, dur);
        }
        // заявки на постер (п.4 р.9): ищем информативный кадр — сначала по
        // готовым тумбам в БД (без единого декода), потом декодом
        // нескольких кандидатов. Ровно одна попытка на медиа: результат
        // (в т.ч. «нет кадра») пишется в БД и в память.
        let poster_job = shared.poster_jobs.lock().unwrap().pop_front();
        if let Some(pid) = poster_job
            && !shared.posters.lock().unwrap().contains_key(&pid)
        {
            let found = find_poster(&shared, &mut db, &mut sources, pid);
            shared.posters.lock().unwrap().insert(pid, found);
        }
        let Some((key, near_tol)) = next else {
            continue;
        };
        let fail = |shared: &Shared, key: ThumbKey| {
            let mut f = shared.failed.lock().unwrap();
            if f.len() >= MAX_FAILED {
                f.clear();
            }
            f.insert(key);
        };
        if shared.ready.lock().unwrap().contains_key(&key) {
            continue;
        }
        let Some(path) = shared.media.lock().unwrap().get(&key.media).cloned() else {
            fail(&shared, key); // неизвестное медиа: не заказывать вечно
            continue;
        };
        let path = &path;

        // 1) пробуем БД: точный ключ, затем ближайший в допуске (п.14 р.7)
        let mut jpeg: Option<Vec<u8>> = None;
        if let Some(db) = &mut db {
            let db_id = *db_ids.entry(key.media).or_insert_with(|| {
                db.ensure_media(std::path::Path::new(path)).map(|(id, _)| id).unwrap_or(-1)
            });
            if db_id >= 0 {
                jpeg = db.thumbnail(db_id, key.t).ok().flatten();
                if jpeg.is_none() && near_tol > 0 {
                    // ближайшая готовая тумба вместо декода; публикуется под
                    // ЗАПРОШЕННЫМ ключом, в БД не пишется
                    jpeg = db
                        .thumbnail_near(db_id, key.t, near_tol)
                        .ok()
                        .flatten()
                        .map(|(_t, j)| j);
                }
            }
        }
        // 2) декод при промахе (+ запись в БД точного ключа)
        if jpeg.is_none() {
            if !sources.contains(key.media) {
                // синтетика — через open_source; файлы — через open_indexed
                // с уже открытой БД (переиспользуем пакетный индекс, тест 34)
                let opened: Option<Box<dyn kt_media::MediaSource>> =
                    if path.starts_with("synth:") || db.is_none() {
                        open_source(path).ok()
                    } else {
                        kt_media::ffmpeg::FfmpegSource::open_indexed(path, db.as_mut())
                            .ok()
                            .map(|s| Box::new(s) as _)
                    };
                match opened {
                    Some(src) => sources.insert(key.media, src),
                    None => {
                        // НЕ dummy-источник (п.1 р.7): честная неудача
                        shared.durations.lock().unwrap().entry(key.media).or_insert(None);
                        fail(&shared, key);
                        continue;
                    }
                }
            }
            let src = sources.get_mut(key.media).expect("source just inserted");
            shared.durations.lock().unwrap().insert(key.media, Some(src.info().duration));
            let mut ok = false;
            if let Ok(frame) =
                src.read_video_frame(Tick(key.t), Some(Quality { width: THUMB_W, height: THUMB_H }))
            {
                let mut buf = Vec::new();
                let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 80);
                let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.into())
                    .unwrap();
                if image::DynamicImage::ImageRgba8(img).write_with_encoder(enc).is_ok() {
                    if let (Some(db), Some(&db_id)) = (&db, db_ids.get(&key.media))
                        && db_id >= 0
                    {
                        let _ = db.put_thumbnail(db_id, key.t, &buf);
                    }
                    jpeg = Some(buf);
                    ok = true;
                }
            }
            if !ok {
                fail(&shared, key); // негативный кэш: без вечного seek+reopen
                continue;
            }
        }
        // 3) jpeg -> ColorImage в ready (+LRU)
        if let Some(j) = jpeg
            && let Ok(img) = image::load_from_memory_with_format(&j, image::ImageFormat::Jpeg)
        {
            let rgba = img.to_rgba8();
            let ci = egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as usize, rgba.height() as usize],
                rgba.as_raw(),
            );
            let mut ready = shared.ready.lock().unwrap();
            let mut order = shared.order.lock().unwrap();
            ready.insert(key, Arc::new(ci));
            order.push_back(key);
            while order.len() > MAX_READY {
                if let Some(old) = order.pop_front() {
                    ready.remove(&old);
                }
            }
        }
    }
}

/// «Информативность» кадра: разброс средней яркости по блокам 16×9.
/// Чёрный/одноцветный/пересвеченный кадр даёт ~0 — именно такие кадры
/// стоят в начале роликов и превращали карточку бина в чёрный квадрат.
fn frame_score(rgba: &[u8], w: u32, h: u32) -> i64 {
    if w == 0 || h == 0 || rgba.len() < (w as usize * h as usize * 4) {
        return 0;
    }
    const BX: u32 = 16;
    const BY: u32 = 9;
    let mut means = [0f32; (BX * BY) as usize];
    let mut global = 0f32;
    for by in 0..BY {
        for bx in 0..BX {
            let (x0, x1) = (bx * w / BX, ((bx + 1) * w / BX).max(bx * w / BX + 1));
            let (y0, y1) = (by * h / BY, ((by + 1) * h / BY).max(by * h / BY + 1));
            let mut sum = 0f32;
            let mut n = 0f32;
            // выборка каждого 2-го пикселя по обеим осям — хватает с запасом
            let mut y = y0;
            while y < y1.min(h) {
                let mut x = x0;
                while x < x1.min(w) {
                    let i = ((y * w + x) * 4) as usize;
                    let l = 0.299 * rgba[i] as f32
                        + 0.587 * rgba[i + 1] as f32
                        + 0.114 * rgba[i + 2] as f32;
                    sum += l;
                    n += 1.0;
                    x += 2;
                }
                y += 2;
            }
            let m = if n > 0.0 { sum / n } else { 0.0 };
            means[(by * BX + bx) as usize] = m;
            global += m;
        }
    }
    let cnt = (BX * BY) as f32;
    let mean = global / cnt;
    // сплошная чернота/пересвет — неинформативно независимо от разброса
    if !(8.0..=247.0).contains(&mean) {
        return 0;
    }
    let var = means.iter().map(|m| (m - mean) * (m - mean)).sum::<f32>() / cnt;
    var.sqrt() as i64
}

/// Кандидаты в постер: доли длительности (лениво, до 5 попыток —
/// решение пользователя). Квантуются тем же шагом, что и филмстрип, —
/// ключи попадают в общий кэш (тест 62).
fn poster_candidates(dur: Tick) -> Vec<i64> {
    let step = THUMB_STEP_LADDER[0];
    let mut v: Vec<i64> = [0.02, 0.10, 0.25, 0.50]
        .iter()
        .map(|f| thumb_key_t(0, (dur.0 as f64 * f) as i64, step))
        .collect();
    v.push(0);
    v.sort_unstable();
    v.dedup();
    v
}

/// Минимальный разброс яркости, при котором кадр считается информативным.
const POSTER_MIN_SCORE: i64 = 6;

fn find_poster(
    shared: &Shared,
    db: &mut Option<kt_media::index::MediaIndexDb>,
    sources: &mut SourceLru<Box<dyn kt_media::MediaSource>>,
    media: MediaId,
) -> Option<i64> {
    let path = shared.media.lock().unwrap().get(&media).cloned()?;
    let db_id = db.as_mut().and_then(|d| d.ensure_media(std::path::Path::new(&path)).ok())
        .map(|(id, _)| id);
    // уже искали в прошлой сессии — берём из БД
    if let (Some(d), Some(id)) = (db.as_ref(), db_id)
        && let Ok(Some(t)) = d.poster(id)
    {
        return (t >= 0).then_some(t);
    }
    let dur = shared.durations.lock().unwrap().get(&media).copied().flatten();
    let Some(dur) = dur.filter(|d| d.0 > 0) else {
        return None; // длительности ещё нет — заявка вернётся позже
    };
    let mut best: Option<(i64, i64)> = None; // (score, t)
    for t in poster_candidates(dur) {
        // 1) кадр уже мог построить филмстрип — оценим без декода
        let ready = shared.ready.lock().unwrap().get(&ThumbKey { media, t }).cloned();
        let score = if let Some(img) = ready {
            let px: Vec<u8> = img.as_raw().to_vec();
            frame_score(&px, img.width() as u32, img.height() as u32)
        } else {
            // 2) декод кандидата
            if !sources.contains(media) {
                let opened: Option<Box<dyn kt_media::MediaSource>> =
                    if path.starts_with("synth:") || db.is_none() {
                        open_source(&path).ok()
                    } else {
                        kt_media::ffmpeg::FfmpegSource::open_indexed(&path, db.as_mut())
                            .ok()
                            .map(|s| Box::new(s) as _)
                    };
                match opened {
                    Some(src) => sources.insert(media, src),
                    None => break,
                }
            }
            let src = sources.get_mut(media).expect("source just inserted");
            match src.read_video_frame(Tick(t), Some(Quality { width: THUMB_W, height: THUMB_H })) {
                Ok(f) => frame_score(&f.rgba, f.width, f.height),
                Err(_) => break,
            }
        };
        if best.as_ref().is_none_or(|(bs, _)| score > *bs) {
            best = Some((score, t));
        }
        // лениво: первый же достаточно «жирный» кадр — наш
        if score >= POSTER_MIN_SCORE {
            break;
        }
    }
    let chosen = best.filter(|(s, _)| *s > 0).map(|(_, t)| t);
    if let (Some(d), Some(id)) = (db.as_ref(), db_id) {
        let _ = d.put_poster(id, chosen.unwrap_or(-1), best.map(|(s, _)| s).unwrap_or(0));
    }
    chosen
}

#[cfg(test)]
mod key_tests {
    use super::*;

    /// Тест 87 (п.4 р.9): метрика информативности кадра — чёрный,
    /// белый и одноцветный кадры дают 0, картинка со структурой — больше
    /// порога. Из-за жёсткого t=0 карточки бина были чёрными квадратами.
    #[test]
    fn t87_frame_score_rejects_flat_frames() {
        use super::{POSTER_MIN_SCORE, frame_score, poster_candidates};
        let (w, h) = (64u32, 36u32);
        let flat = |v: u8| [v, v, v, 255].repeat((w * h) as usize);
        assert_eq!(frame_score(&flat(0), w, h), 0, "чёрный кадр");
        assert_eq!(frame_score(&flat(255), w, h), 0, "белый кадр");
        assert_eq!(frame_score(&flat(128), w, h), 0, "одноцветный кадр");
        // «шахматка» крупными блоками — заведомо информативна
        let mut img = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let v = if (x / 8 + y / 6) % 2 == 0 { 20 } else { 230 };
                img[i] = v;
                img[i + 1] = v;
                img[i + 2] = v;
                img[i + 3] = 255;
            }
        }
        assert!(
            frame_score(&img, w, h) >= POSTER_MIN_SCORE,
            "структурный кадр должен пройти порог: {}",
            frame_score(&img, w, h)
        );
        // кандидаты: не больше пяти, кратны шагу филмстрипа, без дублей
        let c = poster_candidates(Tick(90_000 * 600));
        assert!(c.len() <= 5, "кандидатов больше пяти: {c:?}");
        assert!(c.iter().all(|t| t % THUMB_STEP_LADDER[0] == 0), "не кратно шагу: {c:?}");
        let mut sorted = c.clone();
        sorted.dedup();
        assert_eq!(sorted, c, "дубли среди кандидатов");
        // короткий файл не должен давать отрицательных/дублирующих позиций
        let short = poster_candidates(Tick(1000));
        assert!(short.iter().all(|t| *t >= 0));
    }

    /// Тест 69 (п.1 р.7): негативные кэши — probe/тумба неизвестного медиа
    /// публикуют неудачу РОВНО ОДИН РАЗ, повторные вызовы не порождают
    /// новых заявок (раньше — полное открытие файла на каждом UI-кадре).
    #[test]
    fn t69_negative_caches_stop_retries() {
        let project = kt_model::Project::new(kt_model::ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000, pitch_engine: Default::default() });
        let svc = ThumbService::start(&project, None);
        let id = MediaId(99); // медиа НЕТ в карте сервиса
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        // длительность: заявка ставится, воркер публикует None
        assert_eq!(svc.duration_or_probe(id), None);
        loop {
            if svc.shared.durations.lock().unwrap().contains_key(&id) {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "воркер не опубликовал неудачу");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(svc.duration(id), None);
        // повторный вызов НЕ ставит заявку заново
        assert_eq!(svc.duration_or_probe(id), None);
        assert!(svc.shared.probe.lock().unwrap().is_empty(), "повторная probe-заявка");
        // тумба неизвестного медиа → Failed, повторный request — no-op
        let key = ThumbKey { media: id, t: 0 };
        svc.request(key);
        loop {
            if svc.state(&key) == ThumbState::Failed {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "ключ не помечен failed");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        svc.request(key);
        assert!(svc.shared.queue.lock().unwrap().0.is_empty(), "failed-ключ переочередился");
        // invalidate сбрасывает оба негативных кэша
        svc.invalidate_media(id);
        assert!(!svc.shared.durations.lock().unwrap().contains_key(&id));
        assert_eq!(svc.state(&key), ThumbState::Missing);
    }

    /// Тест 71 (п.1 р.7): LRU источников — вытеснение старейшего, без
    /// полного сброса (прежний clear() при >2 переоткрывал каждый файл).
    #[test]
    fn t71_source_lru() {
        let mut lru: SourceLru<u32> = SourceLru::new(4);
        for i in 0..4 {
            lru.insert(MediaId(i), i as u32);
        }
        assert_eq!(lru.len(), 4);
        // обращение к старейшему двигает его в хвост
        assert_eq!(lru.get_mut(MediaId(0)), Some(&mut 0));
        lru.insert(MediaId(9), 9);
        assert_eq!(lru.len(), 4, "вытеснился ровно один");
        assert!(!lru.contains(MediaId(1)), "старейший после touch(0) — это 1");
        assert!(lru.contains(MediaId(0)) && lru.contains(MediaId(9)));
        lru.insert(MediaId(10), 10);
        assert!(!lru.contains(MediaId(2)));
        assert!(lru.contains(MediaId(0)), "недавно использованный жив");
    }

    /// Тест 62 (п.11 р.6): квантование ключей тумб ОТ НУЛЯ ФАЙЛА — бин и
    /// таймлайн (с ненулевым source_in) дают ОДИН ключ на один момент
    /// файла; ключ кратен шагу и стабилен внутри бакета; скраб монотонен.
    #[test]
    fn t62_thumb_keys_unified_from_file_zero() {
        let step = 90_000i64;
        // кратность и стабильность внутри бакета
        for tl in [0i64, 1, 44_999, 89_999] {
            assert_eq!(thumb_key_t(0, tl, step), 0);
        }
        assert_eq!(thumb_key_t(0, 90_000, step), 90_000);
        // таймлайн с source_in=30_000 (не кратен шагу) и бин на тот же
        // момент файла 120_000 → одинаковый ключ
        let timeline_key = thumb_key_t(30_000, 90_000, step); // файл: 120_000
        let bin_key = thumb_key_t(0, 120_000, step);
        assert_eq!(timeline_key, bin_key, "общий кэш бин↔таймлайн");
        assert_eq!(timeline_key, 90_000, "бакет floor");
        // отрицательное не бывает
        assert_eq!(thumb_key_t(0, -5, step), 0);
        // скраб: монотонен по доле, края в [0, dur)
        let dur = 10 * 90_000i64;
        let mut prev = -1i64;
        for i in 0..=20 {
            let frac = i as f64 / 20.0 * 0.999;
            let t = thumb_key_t(0, (frac * dur as f64) as i64, step);
            assert!(t >= prev, "монотонность скраба");
            assert!((0..dur).contains(&t));
            prev = t;
        }
    }
}
