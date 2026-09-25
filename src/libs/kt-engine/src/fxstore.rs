//! FxStateStore: состояние FX-цепочек по clip id, ВНЕ render graph
//! (инварианты №2–3). Владелец — аудио-render-поток; сюда не заходит ни
//! prefetch, ни UI. Пересборка AudioRenderer (любая правка модели) НЕ
//! уничтожает состояние: та же структура цепочки → тот же процессор,
//! параметры доезжают сглаженно (дефекты b, f старого движка).

use kt_fx::{ClipProcessor, TrackProcessor, chain_fingerprint};
use kt_model::{ClipId, FxInstance, TrackId};
use std::collections::HashMap;

#[derive(Default)]
pub struct FxStateStore {
    map: HashMap<ClipId, ClipProcessor>,
    /// Цепочки ДОРОЖЕК (Р.62, change B). Ключ — `TrackId`, а не «(вид,
    /// номер)»: номер — это ранг строки, и вставка дорожки выше
    /// перенумеровала бы соседей, то есть убила бы их состояние
    /// (инвариант 3). Отдельная карта вместо составного ключа: у дорожки
    /// и у события разные процессоры, и `HashMap` по enum-ключу стоил бы
    /// ветки на каждом обращении ради одного и того же.
    tracks: HashMap<TrackId, TrackProcessor>,
    /// Потоки суб-микса вложенных лент (Р.50 Б4, ключ исправлен в Р.55 С4).
    ///
    /// Ключ — адрес ЭКЗЕМПЛЯРА, а не ленты. Лента, поставленная в пяти
    /// местах, — это ПЯТЬ разных потоков через её цепочки, и делить между
    /// ними один реверб было бы слышно; до Р.55 ключ стоял по ленте, и
    /// комментарий на этом же месте сам себе противоречил. Под реверсом и
    /// растяжением (С4) экземпляры читают ленту ещё и в РАЗНОМ порядке,
    /// то есть дерутся бы за один курсор продюсера.
    ///
    /// `ClipId` уникален по всему документу и переживает трим, разрез и
    /// перенос (инвариант 3) — ровно тот же ключ, что у `map`.
    nests: HashMap<ClipId, NestStream>,
    /// КЛЮЧЕВЫЕ ВЕТКИ сайдчейна (Р.65): у каждой дорожки-ЦЕЛИ свой
    /// суб-стор, в котором живёт поток её ключа — состояния событий
    /// ключевой дорожки и её цепочки.
    ///
    /// Второй набор состояний нужен по той же причине, по какой у ленты
    /// свой `NestStream`: один и тот же процессор нельзя прокрутить дважды
    /// за кадр (ключевая ветка и собственный выход ключевой дорожки — это
    /// ДВА потока через одни и те же цепочки), а окна у них вдобавок
    /// разные, когда у цели есть упреждение.
    keys: HashMap<TrackId, Box<FxStateStore>>,
}

/// Поток суб-микса ОДНОГО экземпляра вложенной ленты.
///
/// Суб-микс — ПОТОКОВЫЙ АВТОМАТ: внутри ленты живут ревербы и дилеи со
/// своим состоянием, и пересчёт со случайного места даёт другой выход.
/// Поэтому лента рендерится ТОЛЬКО ВПЕРЁД и только по возрастанию своего
/// времени, а реверс и растяжение экземпляра действуют на СЭМПЛЫ готового
/// микса — так же, как на сэмплы файла. Обратное («рендерить ленту назад»)
/// дало бы третий, ничей результат: внутренний реверб обязан звучать
/// вперёд ВНУТРИ ленты, а зеркало снаружи даёт обратный реверб.
pub struct NestStream {
    /// Какая лента И какая её РЕДАКЦИЯ (Р.59, стадия 3). Экземпляр могли
    /// перевесить на другую ленту (`SetClipMedia`) или сменить ему
    /// переопределения — в обоих случаях поток начинается заново, а не
    /// наследует чужое состояние.
    pub seq: crate::video::NestKey,
    pub store: FxStateStore,
    /// Непрерывный отрезок уже посчитанных бёрстов, свежие в конце.
    /// Повторный запрос того же бёрста — обычное дело (`render_plain_clip`
    /// держит на нём курсор, а зеркало реверса читает его многократно), и
    /// пересчитывать потоковую цепочку второй раз нельзя.
    pub ring: std::collections::VecDeque<(i64, std::sync::Arc<kt_media::AudioBurst>)>,
    /// Следующий бёрст, который выдаст продюсер. Он идёт СТРОГО ВПЕРЁД.
    pub next: Option<i64>,
    /// Пре-ролл переезда в бёрстах — та же дисциплина, что у настоящего
    /// источника (инвариант 11: seek с pre-roll `max(хвост, 100 мс)`).
    /// У ленты БЕЗ хвостатых эффектов он нулевой, и реверс стоит ровно
    /// столько же, сколько прямое воспроизведение.
    pub preroll: u32,
    /// Размер блока канонической сетки. Переезд начинается с ГРАНИЦЫ
    /// блока, а не «от i назад на P»: тогда значение бёрста — чистая
    /// функция индекса, и байты не зависят ни от истории чтения, ни от
    /// того, каким куском его спросили.
    pub block: u32,
}

impl NestStream {
    /// Свежий поток. `pub`, потому что заводит его и продюсер в `audio.rs`:
    /// там читатель становится известен только в момент чтения.
    pub fn new_for(seq: crate::video::NestKey, preroll: u32, block: u32) -> NestStream {
        NestStream::new(seq, preroll, block)
    }

    fn new(seq: crate::video::NestKey, preroll: u32, block: u32) -> NestStream {
        NestStream {
            seq,
            store: FxStateStore::new(),
            ring: std::collections::VecDeque::new(),
            next: None,
            preroll,
            block,
        }
    }

    /// Начало блока, в котором лежит бёрст `i`.
    pub fn block_start(&self, i: i64) -> i64 {
        let w = self.block.max(1) as i64;
        i.div_euclid(w) * w
    }

    pub fn cached(&self, i: i64) -> Option<std::sync::Arc<kt_media::AudioBurst>> {
        self.ring.iter().find(|(k, _)| *k == i).map(|(_, b)| b.clone())
    }

    pub fn push(&mut self, i: i64, b: std::sync::Arc<kt_media::AudioBurst>) {
        // Потолок кольца — ФУНКЦИЯ ПРОЕКТА (блок плюс пре-ролл), а не
        // бюджет памяти: зависели бы байты выхода от бюджета — рухнул бы
        // инвариант 11 («аудио байт-в-байт всегда и везде»).
        let cap = (self.block as usize + self.preroll as usize).max(2);
        self.ring.push_back((i, b));
        while self.ring.len() > cap {
            self.ring.pop_front();
        }
    }

    pub fn rewind(&mut self) {
        self.ring.clear();
        self.next = None;
        self.store.reset_all();
    }
}

impl FxStateStore {
    pub fn new() -> FxStateStore {
        FxStateStore::default()
    }

    /// Синхронизировать процессор клипа с моделью:
    /// - структура цепочки та же → переиспользуем state, доносим параметры;
    /// - структура сменилась → пересоздаём ТОЛЬКО этот клип;
    /// - цепочка пуста → процессор не нужен.
    pub fn sync_clip(
        &mut self,
        id: ClipId,
        fx: &[FxInstance],
        auto: &kt_fx::AutoProfiles,
        src_in_frames: i64,
        clip_len_frames: i64,
        opts: kt_fx::ChainOpts,
    ) {
        if fx.is_empty() {
            self.map.remove(&id);
            return;
        }
        let fp = chain_fingerprint(fx);
        match self.map.get_mut(&id) {
            Some(p) if p.fingerprint() == fp => {
                p.set_clip_len(clip_len_frames);
                // ГЕОМЕТРИЯ доносится тоже (Р.43, п.293): `chain_fingerprint`
                // хэширует только ВИДЫ слотов, поэтому трим/разрез/punch-out/
                // переанкоринг морф-куска меняли `source_in` при том же id и
                // той же структуре цепочки — и процессор читал от старого
                // якоря, то есть «звук от другого события».
                p.set_src_in(src_in_frames);
                // кривые доносятся ДО параметров: apply_params смотрит,
                // у каких слотов есть кривая (громкость, скорость)
                p.set_auto(auto);
                p.apply_params(fx, false);
                // Упреждение — часть УСТРОЙСТВА цепочки, а не значение
                // параметра (Р.48, п.337): длина колец меняет выравнивание
                // потока, и донести её сглаживанием нельзя. Расхождение
                // (правка окна лимитера, нейтраль питча ↔ актив) —
                // пересборка процессора: следующий read сделает hard_seek
                // с прогревом, как настоящий seek.
                if p.lookahead_frames() != kt_fx::chain_lookahead(fx, auto) || p.opts() != opts {
                    self.map.insert(
                        id,
                        ClipProcessor::new_with_opts(
                            fx,
                            auto,
                            src_in_frames,
                            clip_len_frames,
                            opts,
                        ),
                    );
                }
            }
            _ => {
                self.map.insert(
                    id,
                    ClipProcessor::new_with_opts(fx, auto, src_in_frames, clip_len_frames, opts),
                );
            }
        }
    }

    /// Синхронизировать процессор ДОРОЖКИ с моделью — тем же правилом,
    /// что у события: структура цепочки та же → состояние живёт, значения
    /// доезжают сглаженно; сменилась (в том числе окно упреждения или
    /// движок питча) → пересборка; цепочка пуста → процессор не нужен.
    pub fn sync_track(&mut self, id: TrackId, fx: &[FxInstance], opts: kt_fx::ChainOpts) {
        if fx.is_empty() {
            self.tracks.remove(&id);
            return;
        }
        let fp = chain_fingerprint(fx);
        let empty = kt_fx::AutoProfiles::default();
        match self.tracks.get_mut(&id) {
            Some(p) if p.fingerprint() == fp => {
                p.apply_params(fx, false);
                if p.lookahead_frames() != kt_fx::chain_lookahead(fx, &empty) {
                    self.tracks.insert(id, TrackProcessor::new(fx, opts));
                }
            }
            _ => {
                self.tracks.insert(id, TrackProcessor::new(fx, opts));
            }
        }
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut TrackProcessor> {
        self.tracks.get_mut(&id)
    }

    /// Суб-стор КЛЮЧЕВОЙ ВЕТКИ дорожки-цели (заводится по первому
    /// обращению). Внутри — обычный стор: события ключевой дорожки и её
    /// цепочка живут там своими состояниями.
    pub fn key_mut(&mut self, target: TrackId) -> &mut FxStateStore {
        self.keys.entry(target).or_default()
    }

    /// Убрать ключевые ветки целей, которых больше нет (или которые
    /// перестали слушать чужой сигнал).
    pub fn retain_keys(&mut self, alive: &dyn Fn(TrackId) -> bool) {
        self.keys.retain(|id, _| alive(*id));
    }

    /// Убрать состояние клипов, которых больше нет в модели.
    pub fn retain_clips(&mut self, alive: &dyn Fn(ClipId) -> bool) {
        self.map.retain(|id, _| alive(*id));
    }

    /// Убрать состояние дорожек, которых больше нет в модели.
    pub fn retain_tracks(&mut self, alive: &dyn Fn(TrackId) -> bool) {
        self.tracks.retain(|id, _| alive(*id));
    }

    pub fn get_mut(&mut self, id: ClipId) -> Option<&mut ClipProcessor> {
        self.map.get_mut(&id)
    }

    /// Полный сброс (seek транспорта): каждый процессор сделает pre-roll
    /// при следующем чтении.
    /// Поток суб-микса экземпляра (заводится по первому обращению).
    /// Перевесили экземпляр на другую ленту — поток начинается заново.
    pub fn nest_mut(
        &mut self,
        id: ClipId,
        seq: crate::video::NestKey,
        preroll: u32,
        block: u32,
    ) -> &mut NestStream {
        let e = self.nests.entry(id).or_insert_with(|| NestStream::new(seq, preroll, block));
        if e.seq != seq {
            *e = NestStream::new(seq, preroll, block);
        } else {
            e.preroll = preroll;
            e.block = block;
        }
        e
    }

    /// Забрать состояния лент НАРУЖУ на время рендера.
    ///
    /// Нужно ровно для одного: суб-рендер зовётся ИЗ фетчера, а фетчер
    /// живёт рядом со стором — взять и то и другое мутабельно одновременно
    /// нечем. Вынутая карта развязывает заимствования, и `put_nests`
    /// возвращает её на место в том же вызове.
    pub fn take_nests(&mut self) -> HashMap<ClipId, NestStream> {
        std::mem::take(&mut self.nests)
    }

    pub fn put_nests(&mut self, v: HashMap<ClipId, NestStream>) {
        self.nests = v;
    }

    pub fn retain_nests(&mut self, alive: &dyn Fn(ClipId) -> bool) {
        self.nests.retain(|id, _| alive(*id));
    }

    pub fn reset_all(&mut self) {
        // seek уходит ВГЛУБЬ: цепочки внутри ленты — такие же потоковые
        // автоматы, и хвост реверба из другого места звучал бы «из прошлого»
        for n in self.nests.values_mut() {
            n.rewind();
        }
        for p in self.map.values_mut() {
            p.reset();
        }
        // цепочка ДОРОЖКИ — такой же потоковый автомат (Р.62)
        for p in self.tracks.values_mut() {
            p.reset();
        }
    }
}
