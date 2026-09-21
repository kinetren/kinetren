//! Аудио-рендер таймлайна: выбор событий, sample-accurate чтение источников,
//! equal-power кроссфейд, gain, суммирование дорожек, loop-wrap.
//!
//! Семантика закреплена тестами 1–3, 14, 136 (Р.24: перекрытий больше нет —
//! кроссфейд задаётся явным TransitionEvent, инвариант «клипы одной дорожки
//! встык» держит kt-model):
//! - переход на стыке → equal-power кроссфейд по окну (outgoing √(1−t),
//!   incoming √t), «ручки» читают источник за краями использованного куска;
//! - разные дорожки суммируются без весов;
//! - gain клипа (dB → линейный) применяется после весов.
//!
//! Всё в домене сэмплов 48 kHz (кадров): границы клипов округляются к сэмплу
//! floor'ом один раз, дальше арифметика точная (инвариант №10 — детерминизм).

use crate::fetch::AudioFetcher;
use kt_media::AudioBurst;
use kt_model::{ClipEvent, ClipKind, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS};
use std::sync::Arc;

/// Клип, подготовленный к аудио-рендеру (границы в кадрах 48 kHz).
#[derive(Debug, Clone)]
struct AudioClip {
    id: kt_model::ClipId,
    start: i64,
    end: i64,
    /// Конец с учётом правой «ручки» перехода и хвоста FX (реверб дозванивает
    /// за границей клипа — инвариант «tail», дефект f старого движка).
    ext_end: i64,
    source_in: i64,
    lane: u32,
    gain: f32,
    media: kt_model::MediaId,
    fx: Vec<kt_model::FxInstance>,
    /// Кривые автоматизации клипа в SOURCE-домене (Р.33): вход для
    /// процессора и для расчёта окна источника.
    auto: kt_fx::AutoProfiles,
    reversed: bool,
    /// АДРЕСНАЯ карта знаковой скорости (Р.57): по какому адресу лежит
    /// материал, съеденный к моменту `a`. `None` — карта монотонна, адрес
    /// равен потреблению, и весь прежний путь дословен.
    addr: Option<std::sync::Arc<Vec<kt_fx::AddrKnot>>>,
    /// Окна кроссфейда из переходов (Р.24, п.136), [w0, w1) в кадрах:
    /// клип — incoming (нарастает √t) / outgoing (затухает √(1−t)).
    /// Дизъюнктны по валидации модели.
    fade_in: Option<(i64, i64)>,
    fade_out: Option<(i64, i64)>,
    /// Кривая кроссфейда перехода (Р.33): equal-power — прежняя формула,
    /// поэтому GOLDEN_AUDIO не двигается. Хранится по одной на сторону:
    /// у клипа могут быть разные переходы слева и справа.
    fade_in_curve: kt_model::AudioCurve,
    fade_out_curve: kt_model::AudioCurve,
    /// «Ручки»: материал за краями тела, звучит только под окнами переходов.
    head_ext: i64,
    tail_ext: i64,
    /// Окно пришло из НАЛОЖЕННОГО перехода (Р.51, п.399): кривую в нём
    /// даёт `fade_weight`, а зашитый equal-power `overlap_weight`
    /// глушится через `overlap_at` — иначе двойное затухание.
    fade_in_excl: bool,
    fade_out_excl: bool,
}

/// Вес кроссфейда по окнам переходов клипа. Позиционный и направления не
/// знает (реверс-транспорт использует его же). Формулы — дословно из
/// weight_at (тест 2): для сконвертированного перекрытия веса бит-в-бит.
/// За правым краем fade_out — 1.0: там звучит только FX-хвост (реверб),
/// его не взвешивают (правило render_fx_clip).
fn fade_weight(clip: &AudioClip, f: i64) -> f32 {
    if let Some((w0, w1)) = clip.fade_in
        && f < w1
    {
        let t = (f - w0) as f64 / (w1 - w0) as f64;
        return clip.fade_in_curve.rise(t.max(0.0)) as f32;
    }
    if let Some((w0, w1)) = clip.fade_out
        && f >= w0
        && f < w1
    {
        let t = (f - w0) as f64 / (w1 - w0) as f64;
        // затухающая сторона — та же формула от (1 − t)
        return clip.fade_out_curve.rise((1.0 - t).max(0.0)) as f32;
    }
    1.0
}

/// Результат рендера непрерывного диапазона.
///
/// **Единица обоих счётчиков — КАДРЫ 48 кГц** (Р.43, п.289). До этого путь
/// без FX считал кадры, а путь с FX — промахи БЁРСТОВ, то есть одно и то же
/// голодание давало числа, различающиеся в `AUDIO_BURST_FRAMES` = 1024 раза.
/// Пользователь читал их как одну величину и был прав, что «счётчик сломан».
/// Кадры выбраны потому, что они сравнимы с длиной события и с задержкой
/// вывода; бёрсты не сравнимы ни с чем.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderStats {
    /// Сколько КАДРОВ выхода не удалось прочитать из источника (подставлена
    /// тишина).
    pub underruns: u64,
    /// Промахи ПРЕ-РОЛЛА в КАДРАХ (прогрев FX после seek) — на выход эти
    /// кадры не идут, поэтому в `underruns` не входят (п.166). Диагностика:
    /// систематический рост = префетч не греет зону прогрева.
    pub warmup_misses: u64,
    /// Бёрсты СУБ-МИКСА, посчитанные впустую: пре-ролл переезда продюсера
    /// вложенной ленты (Р.55, С4). Не underrun — их не слышно, — но по ним
    /// видно цену реверса и растяжения на вложении и то, разошлось ли окно
    /// заказа с тем, что читает продюсер.
    pub nest_warmups: u64,
}

/// Корзина индекса времени — одна секунда выхода.
const INDEX_BUCKET_FRAMES: i64 = kt_time::AUDIO_SAMPLE_RATE as i64;

pub struct AudioRenderer {
    clips: Vec<AudioClip>,
    /// Индекс «какие события живут в этой секунде» (Р.43, п.305).
    ///
    /// Без него каждый бёрст перебирал ВЕСЬ монтаж: на проекте пользователя
    /// (1973 события) это 0.4 мкс на событие × 1973 × 107 бёрстов в секунду,
    /// то есть цена МЕСТА росла линейно с размером ПРОЕКТА, даже если
    /// соседние события стоят за час оттуда. Замер: 7 событий — 3.8 мс/с,
    /// те же 7 плюс 2000 далёких — 91.7 мс/с.
    ///
    /// Корзины хранят индексы В ВОЗРАСТАЮЩЕМ ПОРЯДКЕ, и выборка идёт по
    /// ним же: порядок суммирования клипов обязан остаться прежним —
    /// сложение f32 не ассоциативно, и на нём стоит GOLDEN_AUDIO.
    index: Vec<Vec<u32>>,
    /// Начало первой корзины в кадрах (события бывают и левее нуля из-за
    /// «ручек» переходов).
    index_from: i64,
    /// Кто кого накрывает на каждой дорожке — строится раз (п.305).
    /// Пусто при выключенном переслаивании: тогда вес тождественно 1.0.
    covers: OverlapMap,
    /// «Это событие вообще с кем-нибудь пересекается?» — по событию.
    ///
    /// Двоичный поиск по карте дёшев против перебора, но он всё равно
    /// работа НА КАЖДЫЙ КАДР, и на простом монтаже съедал половину времени
    /// рендера (2.32 против 1.09 мс/с). У события, которое ни с кем не
    /// перекрывается, `overlap_weight` всё равно вернул бы 1.0 — значит
    /// искать незачем, и один bool закрывает весь путь. Такими в реальном
    /// монтаже оказывается подавляющее большинство событий.
    overlaps: Vec<bool>,
    /// Переслаивание (Р.28): перекрытия клипов законны и звучат как в v1 —
    /// equal-power на двоих, 1/N при трёх и более. Выключено — перекрытий в
    /// модели нет по построению, и вес всегда 1.0.
    overlap_layers: bool,
    /// Настройки проекта, влияющие на устройство цепочек (Р.48: движок
    /// питча). Донести их сглаживанием нельзя — смена пересобирает
    /// процессоры (сверка в FxStateStore).
    opts: kt_fx::ChainOpts,
    /// Рендереры вложенных лент (Р.50, Б4).
    ///
    /// Вложение — ВИРТУАЛЬНОЕ МЕДИА под фетчером, а не ветка в миксере.
    /// Поэтому `render_plain_clip`, `render_fx_clip`, веса,
    /// `chain_source_window_ext`, `fill_source` и ПОРЯДОК СЛОЖЕНИЯ f32 не
    /// меняются ни на строку: вложение — просто один из клипов в
    /// `active_clips`, и GOLDEN_AUDIO не двигается.
    nests: std::collections::HashMap<kt_model::MediaId, AudioRenderer>,
}

/// Фетчер, за которым вложенные ленты выглядят обычными источниками.
///
/// Суб-микс НЕ кэшируется в `BurstCache` и не может: его производство —
/// прогон FX-состояния ленты, а состояние трогает ТОЛЬКО аудио-render-поток
/// (инвариант 2). Значит продюсер живёт здесь.
///
/// **Три ветки и ни одной больше** (Р.55, С4):
///
/// 1. бёрст уже в кольце — отдать, состояние не трогать;
/// 2. он впереди курсора не дальше блока — ДОГНАТЬ вперёд, промежуточные в
///    кольцо (так работает ускоренное растяжение и мелкие прыжки);
/// 3. иначе (назад, далеко вперёд, первое обращение) — КАНОНИЧЕСКИЙ
///    ПЕРЕЕЗД: сброс, старт с границы блока минус пре-ролл, вперёд до
///    нужного. Каноничность сетки — то, что делает значение бёрста чистой
///    функцией индекса: двузначность «дошли непрерывно / переехали»
///    остаётся ровно одна, и она та же, что уже узаконена парой
///    `seam`/`hard_seek` у обычного клипа.
///
/// Четвёртой ветки быть не должно: каждая новая — новый сорт
/// недетерминизма, которого в аудио не бывает (инвариант 11).
struct NestFetcher<'a> {
    inner: &'a mut dyn AudioFetcher,
    reels: &'a std::collections::HashMap<kt_model::MediaId, AudioRenderer>,
    states: &'a mut std::collections::HashMap<kt_model::ClipId, crate::fxstore::NestStream>,
    /// Пре-ролл и блок — свойства ЛЕНТЫ, посчитанные при сборке рендерера.
    prerolls: &'a std::collections::HashMap<kt_model::MediaId, (u32, u32)>,
    /// Кто читает СЕЙЧАС (`begin_clip`). У вложения свой поток на каждый
    /// экземпляр, и без этого пять мест одной ленты дрались бы за один
    /// курсор продюсера.
    cur: Option<kt_model::ClipId>,
    /// Сколько бёрстов суб-микса посчитано впустую (пре-ролл переезда).
    /// Это НЕ underrun: их не слышно, но по ним видно, что окно заказа
    /// разошлось с тем, что читает продюсер.
    warmups: u64,
    /// РАЗРЕШЁН ли продюсер. В сыром реверс-транспорте (`render_frames_raw`)
    /// лента играется dry, у неё нет состояния вовсе, и блоки не нужны.
    raw: bool,
}

impl NestFetcher<'_> {
    /// Отрендерить бёрст суб-микса ВПЕРЁД, продвинув состояние ленты.
    fn produce(
        reel: &AudioRenderer,
        inner: &mut dyn AudioFetcher,
        st: &mut crate::fxstore::NestStream,
        idx: i64,
        raw: bool,
    ) -> std::sync::Arc<kt_media::AudioBurst> {
        let mut data = Box::new([0.0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS]);
        let at = idx * AUDIO_BURST_FRAMES as i64;
        if raw {
            reel.render_frames_raw(inner, at, &mut data[..]);
        } else {
            reel.render_frames_with(inner, &mut st.store, at, &mut data[..]);
        }
        st.next = Some(idx + 1);
        std::sync::Arc::new(kt_media::AudioBurst { idx, data })
    }
}

impl AudioFetcher for NestFetcher<'_> {
    fn begin_clip(&mut self, id: kt_model::ClipId) {
        self.cur = Some(id);
        self.inner.begin_clip(id);
    }

    fn audio_burst(
        &mut self,
        media: kt_model::MediaId,
        idx: i64,
    ) -> Option<std::sync::Arc<kt_media::AudioBurst>> {
        let Some(reel) = self.reels.get(&media) else { return self.inner.audio_burst(media, idx) };
        // Читатель обязан быть объявлен: без него пять экземпляров одной
        // ленты делили бы один поток. Ловим в debug, а в release честно
        // отдаём тишину, а не чужой звук.
        debug_assert!(self.cur.is_some(), "submix read without begin_clip: reader unknown");
        let who = self.cur?;
        let (preroll, block) = self.prerolls.get(&media).copied().unwrap_or((0, 1));
        let raw = self.raw;
        let st = {
            let e = self.states.entry(who);
            use std::collections::hash_map::Entry;
            match e {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => {
                    v.insert(crate::fxstore::NestStream::new_for(media, preroll, block))
                }
            }
        };
        if st.seq != media {
            *st = crate::fxstore::NestStream::new_for(media, preroll, block);
        }
        st.preroll = preroll;
        st.block = block;
        // 1) уже посчитан
        if let Some(b) = st.cached(idx) {
            return Some(b);
        }
        // 2) догон вперёд — не дальше блока: дальше дешевле переехать
        let gap = block.max(1) as i64;
        let catch_up = matches!(st.next, Some(n) if n <= idx && idx - n < gap);
        if !catch_up {
            // 3) КАНОНИЧЕСКИЙ ПЕРЕЕЗД от границы блока минус пре-ролл
            st.rewind();
            st.next = Some(st.block_start(idx) - preroll as i64);
        }
        let mut from = st.next.unwrap_or(idx);
        // пре-ролл левее нуля ленты считать незачем: там тишина
        if from < 0 {
            from = 0;
            st.next = Some(0);
        }
        let mut out = None;
        for i in from..=idx {
            let b = Self::produce(reel, self.inner, st, i, raw);
            if i == idx {
                out = Some(b.clone());
            } else {
                self.warmups += 1;
            }
            st.push(i, b);
        }
        out
    }
}

/// Карты перекрытий ПО НОМЕРУ ДОРОЖКИ: обращение идёт на каждый кадр,
/// поэтому здесь массив, а не хэш-таблица.
type OverlapMap = Vec<LaneCover>;

/// Карта перекрытий одной дорожки (Р.43, п.305).
///
/// `overlap_weight` спрашивает одно: какие события ЭТОЙ дорожки накрывают
/// кадр. Набор меняется только на границах событий, поэтому его можно
/// посчитать РАЗ подметанием и дальше отвечать двоичным поиском. Раньше
/// функция перебирала ВЕСЬ монтаж НА КАЖДЫЙ КАДР: на проекте пользователя
/// (1973 события) это 1973 × 48000 × число активных клипов проверок в
/// секунду, и цена МЕСТА росла линейно с размером ПРОЕКТА — события за час
/// оттуда стоили столько же, сколько соседние. Замер: 7 событий — 3.6 мс
/// на секунду звука, те же 7 плюс 2000 далёких — 92 мс.
///
/// Наборы хранятся в ВОЗРАСТАЮЩЕМ порядке индексов: `overlap_weight`
/// берёт «первые два» именно в нём, и на этом стоит GOLDEN_AUDIO.
#[derive(Default)]
struct LaneCover {
    /// Границы интервалов, возрастающие; интервалов на один меньше.
    bounds: Vec<i64>,
    /// Накрывающие события каждого интервала.
    cover: Vec<Vec<u32>>,
}

impl LaneCover {
    fn at(&self, frame: i64) -> &[u32] {
        if self.bounds.len() < 2 {
            return &[];
        }
        match self.bounds.binary_search(&frame) {
            Ok(i) if i + 1 < self.bounds.len() => &self.cover[i],
            Ok(_) => &[],
            Err(0) => &[],
            Err(i) if i < self.bounds.len() => &self.cover[i - 1],
            Err(_) => &[],
        }
    }
}

/// Подмести дорожку и записать, кто кого накрывает.
fn build_lane_covers(clips: &[AudioClip]) -> OverlapMap {
    let mut by_lane: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for (i, c) in clips.iter().enumerate() {
        by_lane.entry(c.lane).or_default().push(i as u32);
    }
    let lanes = clips.iter().map(|c| c.lane).max().map(|m| m as usize + 1).unwrap_or(0);
    let mut out: OverlapMap = (0..lanes).map(|_| LaneCover::default()).collect();
    for (lane, idx) in by_lane {
        // события подметания: конец раньше начала в одной точке —
        // интервал события полуоткрытый [start, end)
        let mut ev: Vec<(i64, i8, u32)> = Vec::with_capacity(idx.len() * 2);
        for &i in &idx {
            let c = &clips[i as usize];
            if c.end > c.start {
                ev.push((c.start, 1, i));
                ev.push((c.end, 0, i));
            }
        }
        ev.sort_unstable_by_key(|&(p, kind, i)| (p, kind, i));
        let mut cover: Vec<Vec<u32>> = Vec::new();
        let mut bounds: Vec<i64> = Vec::new();
        let mut active: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut k = 0usize;
        while k < ev.len() {
            let pos = ev[k].0;
            if let Some(&last) = bounds.last() {
                debug_assert!(pos >= last);
                cover.push(active.iter().copied().collect());
            }
            bounds.push(pos);
            while k < ev.len() && ev[k].0 == pos {
                if ev[k].1 == 1 {
                    active.insert(ev[k].2);
                } else {
                    active.remove(&ev[k].2);
                }
                k += 1;
            }
        }
        if bounds.len() >= 2 {
            debug_assert_eq!(cover.len() + 1, bounds.len());
            out[lane as usize] = LaneCover { bounds, cover };
        }
    }
    out
}

/// У каких событий вообще есть с кем перекрываться: событие попадает сюда,
/// если хоть один интервал его дорожки накрыт им И кем-то ещё.
fn overlapping_clips(clips: &[AudioClip], covers: &OverlapMap) -> Vec<bool> {
    let mut out = vec![false; clips.len()];
    for lane in covers {
        for set in &lane.cover {
            if set.len() > 1 {
                for &i in set {
                    out[i as usize] = true;
                }
            }
        }
    }
    out
}

/// Вес клипа от ПЕРЕКРЫТИЯ с соседями по дорожке (возврат семантики v1,
/// снесённой в Р.24 вместе с законностью перекрытий; porting-notes п.2:
/// «equal-power кроссфейд при перекрытии 2 клипов одной аудио-дорожки, 1/N
/// при 3+»). На клипах встык инертен — вес 1.0, поэтому проекты с явными
/// переходами (и GOLDEN-эталоны) не меняются.
fn overlap_weight(clips: &[AudioClip], covers: &OverlapMap, ci: usize, frame: i64) -> f32 {
    let lane = clips[ci].lane;
    let mut first = usize::MAX;
    let mut second = usize::MAX;
    let mut n = 0usize;
    let here = covers.get(lane as usize).map(|m| m.at(frame)).unwrap_or(&[]);
    for &i in here {
        let i = i as usize;
        let c = &clips[i];
        {
            debug_assert!(c.lane == lane && c.start <= frame && frame < c.end);
            if n == 0 {
                first = i;
            } else if n == 1 {
                second = i;
            }
            n += 1;
        }
    }
    match n {
        0 | 1 => 1.0,
        2 => {
            let (a, b) = (&clips[first], &clips[second]);
            let ov_start = a.start.max(b.start);
            let ov_end = a.end.min(b.end);
            let t = if ov_end > ov_start {
                (frame - ov_start) as f64 / (ov_end - ov_start) as f64
            } else {
                0.5
            };
            // equal-power: ранний затухает √(1−t), поздний нарастает √t
            if ci == first {
                ((1.0 - t).max(0.0)).sqrt() as f32
            } else {
                (t.max(0.0)).sqrt() as f32
            }
        }
        n => 1.0 / n as f32,
    }
}

impl AudioRenderer {
    pub fn new(project: &Project) -> AudioRenderer {
        AudioRenderer::build_reel(project, None, 0)
    }

    fn build_reel(
        project: &Project,
        seq: Option<kt_model::MediaId>,
        depth: usize,
    ) -> AudioRenderer {
        // морфер-слои уже применены (подмена источника, live FX batch) —
        // движок про морфер не знает (H1); вложения НЕ раскрываются: у
        // каждого свой суб-микс (Р.50, Б4)
        let effective = project.effective_reel(seq);
        let mut clips: Vec<AudioClip> = effective
            .iter()
            .filter(|c| c.kind == ClipKind::Audio)
            .map(|c: &ClipEvent| {
                let start = c.start.to_audio_frames();
                let end = (c.start + c.duration).to_audio_frames();
                let active = c.active_fx(); // bypass-слоты отфильтрованы (п.2)
                // кривые клипа в SOURCE-домен: аудио-поток дальше видит
                // только числа (Р.33)
                // РЕВЕРС входит в нормировку (Р.39): UI пишет ключ
                // зеркально (`clip_material_at`), и без зеркала здесь DSP
                // отрабатывал бы кривую задом наперёд
                let auto = kt_model::AutoProfiles::from_curves_rev(
                    &c.active_fx_auto(),
                    c.source_in.to_audio_frames(),
                    (end - start).max(1),
                    c.reversed,
                );
                let tail = kt_fx::chain_tail(&active, &auto).to_audio_frames();
                AudioClip {
                    id: c.id,
                    start,
                    end,
                    ext_end: end + tail,
                    source_in: c.source_in.to_audio_frames(),
                    lane: c.lane,
                    gain: db_to_linear(c.gain_db),
                    // адрес ИСТОЧНИКА, а не медиа (Р.59, стадия 2): у
                    // звучащего генератора (SMPTE с тоном, отсчёт с
                    // биипом) значения на событии, значит и поток свой
                    media: crate::clip_source_addr(c),
                    addr: kt_fx::chain_addr_knots(&active, &auto, (end - start).max(1))
                        .map(std::sync::Arc::new),
                    fx: active,
                    auto,
                    reversed: c.reversed,
                    fade_in: None,
                    fade_out: None,
                    fade_in_curve: Default::default(),
                    fade_out_curve: Default::default(),
                    head_ext: 0,
                    tail_ext: 0,
                    fade_in_excl: false,
                    fade_out_excl: false,
                }
            })
            .collect();
        // Окна кроссфейда из переходов (Р.24): границы окна флорятся в кадры
        // ТОЙ ЖЕ конвертацией, что границы клипов, — для сконвертированного
        // перекрытия окно бит-в-бит совпадает со старым weight_at.
        // Переходы — СВОЕЙ ленты (п.399: раньше тут стоял корень, и переход
        // внутри вложенной секвенции молча не кроссфейдил звук).
        let reel: &dyn kt_model::Reel = match seq.and_then(|id| project.sequence(id)) {
            Some(s) => s,
            None => project,
        };
        for t in reel.transitions() {
            // НАЛОЖЕННЫЙ переход (Р.51, п.399): окно — сама зона
            // перекрытия, ручек нет (материал у обеих сторон настоящий),
            // кривая работает вместо зашитого equal-power перекрытия
            if let Some((pa, pb)) = t.overlap_pair(reel) {
                let w0 = pb.start.to_audio_frames();
                let w1 = (pa.start + pa.duration).to_audio_frames();
                if w1 <= w0 {
                    continue;
                }
                if let Some(a) =
                    clips.iter_mut().find(|c| kt_model::same_clip(c.id, pa.id) && c.end == w1)
                {
                    a.fade_out = Some((w0, w1));
                    a.fade_out_curve = t.audio;
                    a.fade_out_excl = true;
                }
                if let Some(b) =
                    clips.iter_mut().find(|c| kt_model::same_clip(c.id, pb.id) && c.start == w0)
                {
                    b.fade_in = Some((w0, w1));
                    b.fade_in_curve = t.audio;
                    b.fade_in_excl = true;
                }
                continue;
            }
            let Some(joint) = t.joint(reel) else { continue };
            let w0 = (joint - t.in_offset).to_audio_frames();
            let w1 = (joint + t.out_offset).to_audio_frames();
            let jf = joint.to_audio_frames();
            if w1 <= w0 {
                continue;
            }
            // фейд (Р.25) — переход с одним живым концом: вес считается так
            // же, «партнёра» просто нет, и материал напротив — тишина
            // разрезанная морфером цель (Р.35): стык у того куска, чья
            // граница на нём стоит; id оригинала носит кусок №0
            if let Some(a) = t.a.and_then(|id| {
                clips.iter_mut().find(|c| kt_model::same_clip(c.id, id) && c.end == jf)
            }) {
                // reversed-FX: зеркальный якорь источника завязан на
                // window_end — ручку не расширяем (ограничение v1, п.136),
                // фейд-вес применяется как есть
                let ext = if a.reversed && !a.fx.is_empty() { 0 } else { w1 - a.end };
                a.fade_out = Some((w0, w1));
                a.fade_out_curve = t.audio;
                a.tail_ext = ext.max(0);
                a.ext_end += a.tail_ext;
            }
            if let Some(b) = t.b.and_then(|id| {
                clips.iter_mut().find(|c| kt_model::same_clip(c.id, id) && c.start == jf)
            }) {
                let ext = if b.reversed && !b.fx.is_empty() { 0 } else { b.start - w0 };
                b.fade_in = Some((w0, w1));
                b.fade_in_curve = t.audio;
                b.head_ext = ext.max(0);
            }
        }
        // Стабильный порядок — часть детерминизма и правила кроссфейда
        // («ранний» клип определяется по (start, media)).
        clips.sort_by_key(|c| (c.start, c.end, c.lane, c.media));
        let (index, index_from) = Self::build_index(&clips);
        // переслаивание — свойство ЛЕНТЫ, на которой лежат события
        let overlap_layers = match seq.and_then(|id| project.sequence(id)) {
            Some(s) => s.overlap_layers,
            None => project.overlap_layers,
        };
        let covers = if overlap_layers { build_lane_covers(&clips) } else { OverlapMap::new() };
        let overlaps = overlapping_clips(&clips, &covers);
        let mut nests = std::collections::HashMap::new();
        if depth < kt_model::MAX_NEST_DEPTH {
            for id in clips.iter().filter_map(|c| project.sequence(c.media).map(|s| s.id)) {
                nests
                    .entry(id)
                    .or_insert_with(|| AudioRenderer::build_reel(project, Some(id), depth + 1));
            }
        }
        AudioRenderer {
            clips,
            index,
            index_from,
            covers,
            overlaps,
            overlap_layers,
            opts: kt_fx::ChainOpts { pitch_engine: project.settings.pitch_engine },
            nests,
        }
    }

    /// Разложить события по секундным корзинам. Событие попадает в каждую
    /// корзину, которую задевает ЕГО ПОЛНЫЙ пролёт — тело, левая «ручка» и
    /// хвост FX: ровно то, что проверяет фильтр в `render_frames_with`.
    fn build_index(clips: &[AudioClip]) -> (Vec<Vec<u32>>, i64) {
        if clips.is_empty() {
            return (Vec::new(), 0);
        }
        let lo = clips.iter().map(|c| c.start - c.head_ext).min().unwrap_or(0);
        let hi = clips.iter().map(|c| c.ext_end).max().unwrap_or(0);
        let from = lo.div_euclid(INDEX_BUCKET_FRAMES) * INDEX_BUCKET_FRAMES;
        let n = ((hi - from).max(0) / INDEX_BUCKET_FRAMES + 1) as usize;
        let mut index = vec![Vec::new(); n];
        for (i, c) in clips.iter().enumerate() {
            let a = ((c.start - c.head_ext - from) / INDEX_BUCKET_FRAMES).max(0) as usize;
            let b = (((c.ext_end - 1 - from) / INDEX_BUCKET_FRAMES).max(0) as usize).min(n - 1);
            for bucket in index.iter_mut().take(b + 1).skip(a) {
                bucket.push(i as u32);
            }
        }
        (index, from)
    }

    /// Индексы событий, задевающих `range`, В ВОЗРАСТАЮЩЕМ ПОРЯДКЕ —
    /// тот же набор и тот же порядок, что дал бы перебор всех клипов
    /// (сторож — тест 292).
    fn active_clips(&self, range: &std::ops::Range<i64>) -> Vec<usize> {
        let hit = |i: usize| {
            let c = &self.clips[i];
            c.start - c.head_ext < range.end && c.ext_end > range.start
        };
        if self.index.is_empty() {
            return Vec::new();
        }
        let a = (range.start - self.index_from).div_euclid(INDEX_BUCKET_FRAMES);
        let b = (range.end - 1 - self.index_from).div_euclid(INDEX_BUCKET_FRAMES);
        let (a, b) = (a.max(0) as usize, (b.max(0) as usize).min(self.index.len() - 1));
        if a >= self.index.len() {
            return Vec::new();
        }
        if a == b {
            return self.index[a].iter().map(|&i| i as usize).filter(|&i| hit(i)).collect();
        }
        // несколько корзин: слияние отсортированных списков с дедупом —
        // порядок обязан остаться возрастающим
        let mut out: Vec<usize> = Vec::new();
        for bucket in &self.index[a..=b] {
            for &i in bucket {
                let i = i as usize;
                if out.last() != Some(&i) && hit(i) {
                    out.push(i);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Пре-ролл и размер блока суб-микса ЭТОЙ ленты в бёрстах (Р.55, С4).
    ///
    /// Обе величины — ЧИСТЫЕ ФУНКЦИИ ПРОЕКТА, и это не педантизм: зависели
    /// бы они от бюджета памяти или от устройства — байты выхода зависели
    /// бы от них же, а аудио обязано быть байт-в-байт всегда и везде
    /// (инвариант 11).
    ///
    /// Пре-ролл — хвост самой хвостатой цепочки ленты, рекурсивно вместе с
    /// пре-роллом её собственных вложений. **У ленты без хвостатых
    /// эффектов он НУЛЕВОЙ, и реверс стоит ровно столько же, сколько
    /// прямое воспроизведение**; дорого становится только там, где внутри
    /// стоит реверб или дилей — и ровно настолько, насколько длинен его
    /// хвост.
    fn reel_preroll_bursts(&self) -> u32 {
        let mut tail = 0i64;
        for c in &self.clips {
            if !c.fx.is_empty() {
                tail = tail.max(kt_fx::chain_tail(&c.fx, &c.auto).to_audio_frames());
            }
            if let Some(sub) = self.nests.get(&c.media) {
                tail = tail.max(sub.reel_preroll_bursts() as i64 * AUDIO_BURST_FRAMES as i64);
            }
        }
        if tail <= 0 {
            return 0;
        }
        let p = tail.max(kt_fx::MIN_PREROLL_FRAMES);
        let b = AUDIO_BURST_FRAMES as i64;
        ((p + b - 1) / b).clamp(1, 4096) as u32
    }

    /// Пре-ролл и блок ленты, на которую смотрит этот экземпляр.
    fn nest_preroll(&self, media: kt_model::MediaId) -> (u32, u32) {
        match self.nests.get(&media) {
            Some(sub) => {
                let p = sub.reel_preroll_bursts();
                // Блок вчетверо длиннее пре-ролла: накладные переезда при
                // чтении НАЗАД выходят ровно (W + P)/W = 1.25×. Потолок и
                // пол — от памяти кольца (8 КиБ на бёрст), но САМ РАЗМЕР
                // от неё не зависит: он функция хвоста ленты.
                let w = (4 * p).clamp(32, 256);
                (p, w)
            }
            None => (0, 1),
        }
    }

    fn nest_prerolls(&self) -> std::collections::HashMap<kt_model::MediaId, (u32, u32)> {
        self.nests.keys().map(|id| (*id, self.nest_preroll(*id))).collect()
    }

    /// Синхронизировать FX-состояние с этим графом (вызывается владельцем
    /// стора — аудио-render-потоком — после каждой пересборки графа).
    pub fn sync_fx(&self, store: &mut crate::fxstore::FxStateStore) {
        for c in &self.clips {
            store.sync_clip(c.id, &c.fx, &c.auto, c.source_in, c.end - c.start, self.opts);
        }
        let alive: std::collections::HashSet<_> = self.clips.iter().map(|c| c.id).collect();
        store.retain_clips(&|id| alive.contains(&id));
        // цепочки ВНУТРИ лент синхронизируются тем же правилом: те же
        // цепочки — те же state-объекты, хвосты ревербов живут (инвариант 3)

        for c in &self.clips {
            let Some(sub) = self.nests.get(&c.media) else { continue };
            let (preroll, block) = self.nest_preroll(c.media);
            sub.sync_fx(&mut store.nest_mut(c.id, c.media, preroll, block).store);
        }
        let live: std::collections::HashSet<_> =
            self.clips.iter().filter(|c| self.nests.contains_key(&c.media)).map(|c| c.id).collect();
        store.retain_nests(&|id| live.contains(&id));
    }

    /// Рендер без внешнего FX-состояния (для клипов без FX эквивалентно
    /// `render_frames_with`; FX-клипы получат одноразовый store — годится
    /// для одноразовых офлайн-вызовов, но не для потокового плеера).
    pub fn render_frames(
        &self,
        fetcher: &mut dyn AudioFetcher,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        let mut store = crate::fxstore::FxStateStore::new();
        self.sync_fx(&mut store);
        self.render_frames_with(fetcher, &mut store, start_frame, out)
    }

    /// Рендер непрерывного диапазона таймлайна [start_frame, start_frame+out/2)
    /// в interleaved stereo `out`. Без loop-логики — wrap делает вызывающий
    /// (`render_looped`), разрезая диапазон по границе региона.
    /// `store` — долгоживущее FX-состояние аудио-render-потока.
    pub fn render_frames_with(
        &self,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        // Плоская лента — ПРЕЖНИЙ путь без единой лишней ветки: на нём
        // стоит GOLDEN_AUDIO.
        if self.nests.is_empty() {
            return self.render_reel(fetcher, store, start_frame, out);
        }
        // Состояния лент вынимаются НАРУЖУ: суб-рендер зовётся из фетчера,
        // а фетчер живёт рядом со стором — взять оба мутабельно
        // одновременно нечем.
        let mut states = store.take_nests();
        let prerolls = self.nest_prerolls();
        let stats = {
            let mut f = NestFetcher {
                inner: fetcher,
                reels: &self.nests,
                states: &mut states,
                prerolls: &prerolls,
                cur: None,
                warmups: 0,
                raw: false,
            };
            let mut st = self.render_reel(&mut f, store, start_frame, out);
            st.nest_warmups += f.warmups;
            st
        };
        store.put_nests(states);
        stats
    }

    fn render_reel(
        &self,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert_eq!(out.len() % AUDIO_CHANNELS, 0);
        out.fill(0.0);
        let frames = (out.len() / AUDIO_CHANNELS) as i64;
        let range = start_frame..start_frame + frames;
        let mut stats = RenderStats::default();

        let active = self.active_clips(&range);

        // Вклад каждого клипа считаем по-бурстово (источника), веса — по-сэмплово.
        for &ci in &active {
            let clip = &self.clips[ci];
            // КТО читает — до первого чтения (Р.55, С4): у вложения свой
            // поток суб-микса на каждый экземпляр. Один виртуальный вызов
            // на клип на бёрст, не на кадр.
            fetcher.begin_clip(clip.id);
            if !clip.fx.is_empty() {
                self.render_fx_clip(ci, fetcher, store, &range, out, &mut stats);
                continue;
            }
            self.render_plain_clip(ci, fetcher, &range, out, &mut stats);
        }
        stats
    }

    /// Клип без FX: прямое чтение source-burst'ов с весами кроссфейда.
    /// Общий код render_frames_with и render_frames_raw (реверс-превью).
    /// Вес перекрытия для клипа `ci` в кадре `f` (1.0, когда переслаивание
    /// выключено — тогда перекрытий в модели нет по построению).
    fn overlap_at(&self, ci: usize, f: i64) -> f32 {
        // окно наложенного перехода (Р.51): кривую уже применил
        // `fade_weight` — зашитый equal-power перекрытия глушится, иначе
        // двойное затухание. O(1), без скана переходов.
        let c = &self.clips[ci];
        if (c.fade_in_excl && c.fade_in.is_some_and(|(w0, w1)| f >= w0 && f < w1))
            || (c.fade_out_excl && c.fade_out.is_some_and(|(w0, w1)| f >= w0 && f < w1))
        {
            return 1.0;
        }
        if self.overlap_layers && self.overlaps[ci] {
            overlap_weight(&self.clips, &self.covers, ci, f)
        } else {
            1.0
        }
    }

    fn render_plain_clip(
        &self,
        ci: usize,
        fetcher: &mut dyn AudioFetcher,
        range: &std::ops::Range<i64>,
        out: &mut [f32],
        stats: &mut RenderStats,
    ) {
        let clip = &self.clips[ci];
        {
            let from = (clip.start - clip.head_ext).max(range.start);
            let to = (clip.end + clip.tail_ext).min(range.end);
            if from >= to {
                return;
            }
            let mut f = from;
            let mut cursor: Option<(i64, Arc<AudioBurst>)> = None; // (burst idx, data)
            let clip_len = clip.end - clip.start;
            while f < to {
                // реверс: локальный кадр k читает источник с конца (п.16);
                // local за телом («ручки» перехода, п.136) продолжает
                // формулу линейно в обе стороны
                let local = f - clip.start;
                let src = if clip.reversed {
                    clip.source_in + (clip_len - 1 - local)
                } else {
                    clip.source_in + local
                };
                // материал левее нуля источника — тишина БЕЗ underrun
                // (ручка длиннее материала — политика краёв)
                if src < 0 {
                    if clip.reversed {
                        break; // src дальше только убывает
                    }
                    f += (-src).min(to - f);
                    continue;
                }
                let bidx = src.div_euclid(AUDIO_BURST_FRAMES as i64);
                let within = src.rem_euclid(AUDIO_BURST_FRAMES as i64);
                // сколько кадров можем взять из этого source-burst'а подряд:
                // вперёд — до конца бёрста, при реверсе — вниз до его начала
                let take = if clip.reversed {
                    (within + 1).min(to - f)
                } else {
                    (AUDIO_BURST_FRAMES as i64 - within).min(to - f)
                };
                let burst = match &cursor {
                    Some((i, b)) if *i == bidx => Some(b.clone()),
                    _ => {
                        let b = fetcher.audio_burst(clip.media, bidx);
                        if let Some(b) = &b {
                            cursor = Some((bidx, b.clone()));
                        }
                        b
                    }
                };
                match burst {
                    Some(b) => {
                        for k in 0..take {
                            let w =
                                fade_weight(clip, f + k) * self.overlap_at(ci, f + k) * clip.gain;
                            let oi = ((f + k - range.start) as usize) * AUDIO_CHANNELS;
                            let sk = if clip.reversed { within - k } else { within + k };
                            let si = (sk as usize) * AUDIO_CHANNELS;
                            for ch in 0..AUDIO_CHANNELS {
                                out[oi + ch] += b.data[si + ch] * w;
                            }
                        }
                    }
                    // недостающий бёрст в зоне РУЧКИ (за краем медиа) —
                    // не underrun: считаем только кадры тела
                    None => {
                        stats.underruns +=
                            (clip.end.min(f + take) - clip.start.max(f)).max(0) as u64;
                    }
                }
                f += take;
            }
        }
    }

    /// Рендер БЕЗ FX-цепочек — реверс-превью транспорта (J4 v1.4): клипы
    /// с FX играют dry, т.к. потоковые состояния (реверб/варп) при подаче
    /// материала ЗАДОМ НАПЕРЁД бессмысленны — хвост реверба звучал бы «из
    /// будущего» (Vegas в reverse-скрабе так же). Обычное воспроизведение
    /// и экспорт этим путём НЕ ходят (инварианты №2/№7 целы).
    pub fn render_frames_raw(
        &self,
        fetcher: &mut dyn AudioFetcher,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert_eq!(out.len() % AUDIO_CHANNELS, 0);
        out.fill(0.0);
        let frames = (out.len() / AUDIO_CHANNELS) as i64;
        let range = start_frame..start_frame + frames;
        // ВЛОЖЕНИЯ И ЗДЕСЬ (Р.55, С4). До этого раунда сырой путь не
        // оборачивал фетчер, и `audio_burst(адрес ленты)` уходил в обычный
        // фетчер, где такого источника нет: реверс-транспорт над вложением
        // давал ТИШИНУ и растил underrun'ы по кадрам. При этом
        // `wanted_audio_bursts_raw` честно рекурсировал внутрь — заказ
        // уходил, а чтение до него не доходило (нарушение инварианта 4).
        //
        // Суб-микс сырого пути тоже DRY (решение «как в Vegas»), а у dry
        // состояния нет вовсе: бёрст — чистая функция индекса, ни блоков,
        // ни пре-ролла не нужно.
        if self.nests.is_empty() {
            return self.render_raw_reel(fetcher, &range, out);
        }
        let mut states = std::collections::HashMap::new();
        let prerolls = self.nest_prerolls();
        let mut f = NestFetcher {
            inner: fetcher,
            reels: &self.nests,
            states: &mut states,
            prerolls: &prerolls,
            cur: None,
            warmups: 0,
            raw: true,
        };
        self.render_raw_reel(&mut f, &range, out)
    }

    fn render_raw_reel(
        &self,
        fetcher: &mut dyn AudioFetcher,
        range: &std::ops::Range<i64>,
        out: &mut [f32],
    ) -> RenderStats {
        let mut stats = RenderStats::default();
        for ci in self.active_clips(range) {
            // сырой путь смотрит на ТЕЛО с ручками, а не на хвост FX
            if self.clips[ci].end + self.clips[ci].tail_ext > range.start {
                fetcher.begin_clip(self.clips[ci].id);
                self.render_plain_clip(ci, fetcher, range, out, &mut stats);
            }
        }
        stats
    }

    /// Loop-вариант render_frames_raw: linear-позиция wrap'ится в регион
    /// (rem_euclid корректен и для хода НАЗАД — отрицательных смещений).
    pub fn render_looped_raw(
        &self,
        fetcher: &mut dyn AudioFetcher,
        region: std::ops::Range<i64>,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert!(!region.is_empty());
        let len = region.end - region.start;
        let total = (out.len() / AUDIO_CHANNELS) as i64;
        let mut stats = RenderStats::default();
        let mut done = 0i64;
        while done < total {
            let tl = region.start + (start_frame - region.start + done).rem_euclid(len);
            let chunk = (region.end - tl).min(total - done);
            let sub = &mut out
                [(done as usize) * AUDIO_CHANNELS..((done + chunk) as usize) * AUDIO_CHANNELS];
            let s = self.render_frames_raw(fetcher, tl, sub);
            stats.underruns += s.underruns;
            done += chunk;
        }
        stats
    }

    /// FX-клип: клип-локальный поток через ClipProcessor (варп+реверб+gain
    /// цепочки), затем веса кроссфейда (в пределах реального клипа) и gain
    /// клипа. В зоне хвоста (за end) вес 1.0 — реверб дозванивает под
    /// соседями без взвешивания.
    fn render_fx_clip(
        &self,
        ci: usize,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        range: &std::ops::Range<i64>,
        out: &mut [f32],
        stats: &mut RenderStats,
    ) {
        let clip = &self.clips[ci];
        let from = (clip.start - clip.head_ext).max(range.start);
        let to = clip.ext_end.min(range.end);
        if from >= to {
            return;
        }
        // окно источника клипа: за его пределами — законная тишина.
        // Единый расчёт с wanted_audio_bursts (chain_source_window_ext); dry
        // за концом МАТЕРИАЛА глушит сам ClipProcessor (output-гейт, п.138).
        // У reversed-FX клипов material = телу (ручки отключены, п.136):
        // зеркальный якорь источника завязан на window_end.
        let clip_len = clip.end - clip.start;
        let material = -clip.head_ext..clip_len + clip.tail_ext;
        // Граница — ПОТРЕБЛЕНИЕ (Р.57): курсор ресемплера живёт на нём, а
        // адреса под знаковой кривой уходят и влево, и вправо. У
        // монотонной цепочки это то же число, что раньше.
        let src_window_end = clip.source_in
            + kt_fx::chain_consumption_window_ext(
                &clip.fx,
                &clip.auto,
                clip_len,
                material.clone(),
                material,
            )
            .end;
        // ОСЬ ЗЕРКАЛА ≠ конец окна (Р.43, п.292). Окно включает запас
        // ресемплера (`WARP_SOURCE_MARGIN_FRAMES`, 8192 кадра = 170.7 мс) —
        // это правильная граница «дальше тишина», но негодная ось: зеркаля
        // от неё, замедленный reversed-клип играл материал, сдвинутый на
        // эти 170 мс. Ось — фактически потреблённое `used`, ТА ЖЕ величина,
        // что у видео (`ClipWarp::used`). У цепочки без варпа обе совпадают,
        // поэтому GOLDEN_AUDIO не двигается.
        let src_mirror_end =
            clip.source_in + kt_fx::chain_source_used(&clip.fx, &clip.auto, clip_len);
        let media = clip.media;
        let mut pull_underruns = 0u64;
        let mut warmup_misses = 0u64;
        let Some(proc) = store.get_mut(clip.id) else { return };
        proc.set_material_ext(clip.head_ext, clip.tail_ext);
        let local = from - clip.start;
        let count = (to - from) as usize;
        let mut buf = vec![0f32; count * AUDIO_CHANNELS];
        {
            let reversed = clip.reversed;
            let win = SrcWindow {
                start: clip.source_in,
                end: src_window_end,
                mirror_end: src_mirror_end,
                addr: clip.addr.as_deref().map(|v| v.as_slice()),
            };
            let mut pull = |kind: kt_fx::PullKind, src_frame: i64, dst: &mut [f32]| -> bool {
                // промахи пре-ролла считаются отдельно: их не слышно
                let n = match kind {
                    kt_fx::PullKind::Output => &mut pull_underruns,
                    kt_fx::PullKind::Warmup => &mut warmup_misses,
                };
                fill_source(fetcher, media, src_frame, win, reversed, dst, n)
            };
            proc.read(local, &mut buf, &mut pull);
        }
        stats.underruns += pull_underruns;
        stats.warmup_misses += warmup_misses;
        for k in 0..count as i64 {
            let f = from + k;
            // веса действуют на теле и ручках (окна переходов); в зоне
            // FX-хвоста (за материалом) — 1.0: реверб дозванивает под
            // соседями без взвешивания
            let w = if f < clip.end + clip.tail_ext { fade_weight(clip, f) } else { 1.0 }
                * self.overlap_at(ci, f)
                * clip.gain;
            let oi = ((f - range.start) as usize) * AUDIO_CHANNELS;
            let bi = (k as usize) * AUDIO_CHANNELS;
            for ch in 0..AUDIO_CHANNELS {
                out[oi + ch] += buf[bi + ch] * w;
            }
        }
    }

    /// Рендер с loop-регионом: выходной поток длиной `frames` кадров, таймлайн
    /// позиции wrap'ятся по [region) per-sample (граница может не совпадать
    /// с burst'ом). Тест 14: непрерывность через wrap.
    pub fn render_looped(
        &self,
        fetcher: &mut dyn AudioFetcher,
        region: std::ops::Range<i64>,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        let mut store = crate::fxstore::FxStateStore::new();
        self.sync_fx(&mut store);
        self.render_looped_with(fetcher, &mut store, region, start_frame, out)
    }

    pub fn render_looped_with(
        &self,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        region: std::ops::Range<i64>,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert!(!region.is_empty());
        let len = region.end - region.start;
        let total = (out.len() / AUDIO_CHANNELS) as i64;
        let mut stats = RenderStats::default();
        let mut done = 0i64;
        while done < total {
            let tl = region.start + (start_frame - region.start + done).rem_euclid(len);
            let chunk = (region.end - tl).min(total - done);
            let sub = &mut out
                [(done as usize) * AUDIO_CHANNELS..((done + chunk) as usize) * AUDIO_CHANNELS];
            let s = self.render_frames_with(fetcher, store, tl, sub);
            stats.underruns += s.underruns;
            stats.warmup_misses += s.warmup_misses;
            done += chunk;
        }
        stats
    }

    /// Диапазоны source-burst'ов, нужные для воспроизведения [range) таймлайна
    /// (в кадрах). Основа prefetch'а: wanted − coverage → submit.
    pub fn wanted_audio_bursts(
        &self,
        range: std::ops::Range<i64>,
    ) -> Vec<(kt_model::MediaId, std::ops::Range<i64>)> {
        self.wanted_audio_bursts_for(range, false)
    }

    /// То же для СЫРОГО пути (реверс-транспорт, `render_frames_raw`).
    ///
    /// Р.43, п.295: сырой путь играет ВСЕ события через `render_plain_clip`,
    /// то есть читает источник ЛИНЕЙНО и варпа не знает вовсе (решение
    /// «dry, как Vegas», audio.rs). А префетч считал окно ПО ВАРПУ — и на
    /// каждом замедленном событии грел не те бёрсты. Промах при этом
    /// считается в КАДРАХ (`render_plain_clip`), то есть до 1024 за раз:
    /// именно так «underruns растут» на реверсе. Единый источник правды у
    /// префетча и рендера — то же правило, что у обычного пути (регрессия
    /// п.13, тест 17).
    pub fn wanted_audio_bursts_raw(
        &self,
        range: std::ops::Range<i64>,
    ) -> Vec<(kt_model::MediaId, std::ops::Range<i64>)> {
        self.wanted_audio_bursts_for(range, true)
    }

    fn wanted_audio_bursts_for(
        &self,
        range: std::ops::Range<i64>,
        raw: bool,
    ) -> Vec<(kt_model::MediaId, std::ops::Range<i64>)> {
        let mut out = Vec::new();
        for c in &self.clips {
            // «ручки» переходов входят в wanted — префетч греет их (п.136)
            let from = (c.start - c.head_ext).max(range.start);
            let to = (c.end + c.tail_ext).min(range.end);
            if from >= to {
                continue;
            }
            let (s0, s1) = if raw || c.fx.is_empty() {
                if c.reversed {
                    // зеркало: конец таймлайн-диапазона читает НАЧАЛО куска
                    let len = c.end - c.start;
                    (c.source_in + (len - (to - c.start)), c.source_in + (len - (from - c.start)))
                } else {
                    // без FX: источник течёт 1:1 с таймлайном
                    (c.source_in + (from - c.start), c.source_in + (to - c.start))
                }
            } else if c.reversed {
                // FX+реверс: окно потребления зеркалится вокруг ПОТРЕБЛЁННОГО
                // конца — той же оси, что у рендера (Р.43, п.292); ручки
                // отключены (п.136), from/to здесь совпадают с телом.
                // Абсолютный кламп ниже (`s0.max(0)`) режет заход за нулевой
                // кадр файла: расходись эти два расчёта — вечный промах кэша.
                let len = c.end - c.start;
                let used = kt_fx::chain_source_used(&c.fx, &c.auto, len);
                let w = kt_fx::chain_source_window(
                    &c.fx,
                    &c.auto,
                    len,
                    (from - c.start)..(to - c.start),
                );
                (c.source_in + used - w.end, c.source_in + used - w.start)
            } else {
                // с FX: варп потребляет источник по rate_integral, а не
                // линейно; wanted обязан покрывать РЕАЛЬНОЕ потребление
                // (регрессия п.13: линейный маппинг грел не те бёрсты →
                // глубже в клипе вечный промах кэша → тишина)
                let len = c.end - c.start;
                let material = -c.head_ext..len + c.tail_ext;
                let w = kt_fx::chain_source_window_ext(
                    &c.fx,
                    &c.auto,
                    len,
                    material,
                    (from - c.start)..(to - c.start),
                );
                (c.source_in + w.start, c.source_in + w.end.max(w.start + 1))
            };
            // ручка может выходить за нулевой кадр источника — там тишина,
            // бёрсты не заказываем
            let s0 = s0.max(0);
            let s1 = s1.max(s0 + 1);
            // ЗАКАЗ УХОДИТ ВГЛУБЬ (Р.50): у ленты источника нет, есть
            // содержимое. Спрашиваем её о ТОМ ЖЕ окне потребления, но в её
            // собственных кадрах — правда у префетча и рендера одна (п.13).
            if let Some(sub) = self.nests.get(&c.media) {
                // ЗАКАЗ УХОДИТ ВГЛУБЬ, но продюсер суб-микса читает ленту
                // ШИРЕ окна потребления: при чтении НАЗАД каждый бёрст —
                // переезд, а переезд начинается с границы блока минус
                // пре-ролл. Не расширить заказ значит промахиваться мимо
                // кэша на каждом переезде — вечный промах (п.13, t17).
                // Правило ОДНО на рендер и на префетч: разойдись они, и
                // это был бы тот же класс дефекта.
                let (preroll, block) = self.nest_preroll(c.media);
                // ГЕЙТ ПО НЕМОНОТОННОСТИ (Р.57): назад читает и зеркальный
                // экземпляр, и знаковая скорость — продюсеру всё равно,
                // что именно развернуло чтение.
                let w = nest_read_window(preroll, block, c.reversed || c.addr.is_some(), s0..s1);
                out.extend(sub.wanted_audio_bursts_for(w, raw));
                continue;
            }
            let b0 = s0.div_euclid(AUDIO_BURST_FRAMES as i64);
            let b1 = (s1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1;
            out.push((c.media, b0..b1));
        }
        merge_burst_ranges(out)
    }
}

/// Какой отрезок ВРЕМЕНИ ЛЕНТЫ реально прочитает продюсер суб-микса, чтобы
/// отдать кадры `s` (Р.55, С4).
///
/// ЕДИНЫЙ ИСТОЧНИК ПРАВДЫ для рендера и префетча — не «одинаковая формула
/// в двух местах», а одна функция: разойдись они, получился бы вечный
/// промах кэша (п.13, t17, t247d).
///
/// `blocks` — читаем ли НАЗАД. Гейт по НЕМОНОТОННОСТИ, а не по варпу:
/// варп с положительной скоростью монотонен (`fill_source` идёт по `virt`
/// вперёд), и в блоках не нуждается — на прямом пути заказ не двигается НИ
/// НА БЁРСТ, и «цена линейна» (t436) остаётся верным. Назад читают ровно
/// двое: зеркальный экземпляр и ЗНАКОВАЯ скорость (Р.57), у которой адрес
/// уходит по материалу обратно.
fn nest_read_window(
    preroll: u32,
    block: u32,
    blocks: bool,
    s: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    if !blocks {
        return s;
    }
    let b = AUDIO_BURST_FRAMES as i64;
    let w = (block.max(1) as i64) * b;
    let start = s.start.div_euclid(w) * w - (preroll as i64) * b;
    start.max(0)..s.end
}

/// Схлопнуть перекрывающиеся и соприкасающиеся диапазоны бёрстов одного
/// медиа (Р.43, п.297).
///
/// Пользователь строит стоп-кадр из СОТЕН микрособытий: в его проекте 885
/// аудио-событий по 90 тиков (4.8 сэмпла) стоят встык на 0.88 с и читают
/// ОДИН И ТОТ ЖЕ кадр источника. Каждое клало в `wanted` свою пару, и
/// `prefetch_audio_pass` делал на них 885 запросов `coverage` под мьютексом
/// кэша и submit'ил дубликаты — при том, что бёрст там ровно один.
/// Схлопывание ничего не меняет по смыслу (объединение множеств то же) и
/// снимает контеншен, о котором предупреждает `apps/kt-app/CLAUDE.md`.
fn merge_burst_ranges(
    mut v: Vec<(kt_model::MediaId, std::ops::Range<i64>)>,
) -> Vec<(kt_model::MediaId, std::ops::Range<i64>)> {
    if v.len() < 2 {
        return v;
    }
    v.sort_by_key(|(m, r)| (m.0, r.start, r.end));
    let mut out: Vec<(kt_model::MediaId, std::ops::Range<i64>)> = Vec::with_capacity(v.len());
    for (m, r) in v {
        match out.last_mut() {
            // соприкасающиеся тоже сливаем: между b1 и b1 дырки нет
            Some((pm, pr)) if *pm == m && r.start <= pr.end => pr.end = pr.end.max(r.end),
            _ => out.push((m, r)),
        }
    }
    out
}

pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Окно источника, в котором живёт событие (кадры от нуля файла).
///
/// `end` — граница «дальше тишина» (включает запас ресемплера), `mirror_end`
/// — ОСЬ ЗЕРКАЛА у reversed-события: фактически потреблённый конец, без
/// запаса. До Р.43 это была одна величина, и зеркало уезжало на 170.7 мс
/// (п.292).
#[derive(Debug, Clone, Copy)]
struct SrcWindow<'a> {
    start: i64,
    /// Граница ПОТРЕБЛЕНИЯ: дальше цепочка не ест (Р.57). У монотонной
    /// карты это же и граница адресов — прежнее число.
    end: i64,
    mirror_end: i64,
    /// Адресная карта знаковой скорости; `None` — адрес == потребление.
    addr: Option<&'a [kt_fx::AddrKnot]>,
}

/// Ран адреса, накрывающий потребление `a`: (адрес первого кадра,
/// направление, конец рана по потреблению).
fn addr_run_at(knots: &[kt_fx::AddrKnot], a: i64) -> (i64, i64, i64) {
    let i = match knots.binary_search_by_key(&a, |k| k.a) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    };
    let k = knots[i];
    let end = knots.get(i + 1).map(|n| n.a).unwrap_or(i64::MAX);
    let off = a - k.a;
    match k.dir {
        // назад: первый кадр ПОСЛЕ разворота — тот, что лежит перед
        // точкой разворота, иначе кадр развернулся бы дважды (палиндром)
        -1 => (k.p - 1 - off, -1, end),
        0 => (k.p, 0, end),
        _ => (k.p + off, 1, end),
    }
}

/// Заполнить `dst` кадрами источника [src_frame, +len): читает burst'ы через
/// фетчер; кадры вне [0, window_end) — тишина БЕЗ underrun'а (хвост/пре-ролл),
/// промах внутри окна — тишина с underrun'ом. Возвращает true (буфер всегда
/// валиден).
fn fill_source(
    fetcher: &mut dyn AudioFetcher,
    media: kt_model::MediaId,
    src_frame: i64,
    window: SrcWindow,
    reversed: bool,
    dst: &mut [f32],
    underruns: &mut u64,
) -> bool {
    match window.addr {
        None => fill_source_linear(fetcher, media, src_frame, window, reversed, dst, underruns),
        Some(knots) => {
            fill_source_signed(fetcher, media, src_frame, window, knots, reversed, dst, underruns)
        }
    }
}

/// Прежний путь ДОСЛОВНО: адрес равен потреблению, реверс — статическое
/// зеркало окна. На нём стоит GOLDEN_AUDIO.
fn fill_source_linear(
    fetcher: &mut dyn AudioFetcher,
    media: kt_model::MediaId,
    src_frame: i64,
    window: SrcWindow,
    reversed: bool,
    dst: &mut [f32],
    underruns: &mut u64,
) -> bool {
    dst.fill(0.0);
    let SrcWindow { start: win_start, end: window_end, mirror_end, .. } = window;
    let frames = (dst.len() / AUDIO_CHANNELS) as i64;
    let mut f = 0i64;
    while f < frames {
        // Реверс (п.16): виртуальный форвард-поток [win_start, window_end)
        // читает РЕАЛЬНЫЙ источник зеркально внутри окна; FX-цепочка (варп,
        // реверб) видит обычный форвард-поток и не знает о реверсе.
        let virt = src_frame + f;
        if virt >= window_end {
            break; // дальше только тишина
        }
        // зеркало считается от ПОТРЕБЛЁННОГО конца, а не от конца окна
        let src = if reversed { win_start + (mirror_end - 1 - virt) } else { virt };
        if src < 0 || virt < 0 {
            f += 1;
            continue;
        }
        let bidx = src.div_euclid(AUDIO_BURST_FRAMES as i64);
        let within = src.rem_euclid(AUDIO_BURST_FRAMES as i64);
        let take = if reversed {
            (within + 1).min(frames - f).min(window_end - virt)
        } else {
            (AUDIO_BURST_FRAMES as i64 - within).min(frames - f).min(window_end - virt)
        };
        match fetcher.audio_burst(media, bidx) {
            Some(b) => {
                let d0 = (f as usize) * AUDIO_CHANNELS;
                if reversed {
                    for k in 0..take as usize {
                        let si = (within as usize - k) * AUDIO_CHANNELS;
                        for ch in 0..AUDIO_CHANNELS {
                            dst[d0 + k * AUDIO_CHANNELS + ch] = b.data[si + ch];
                        }
                    }
                } else {
                    let s0 = (within as usize) * AUDIO_CHANNELS;
                    let n = (take as usize) * AUDIO_CHANNELS;
                    dst[d0..d0 + n].copy_from_slice(&b.data[s0..s0 + n]);
                }
            }
            // единица — КАДРЫ, как у пути без FX (Р.43, п.289): «1 за
            // промах бёрста» давало 1024-кратное расхождение счётчиков
            None => *underruns += take.max(0) as u64,
        }
        f += take;
    }
    true
}

/// Знаковая скорость (Р.57): виртуальный поток — ПОТРЕБЛЕНИЕ, а адрес
/// материала берётся из адресной карты ранами постоянного направления.
///
/// Цепочка (варп, реверб) по-прежнему видит обычный форвард-поток и о
/// направлении не знает — как и при реверсе. Флаг «Реверс» зеркалит
/// готовый адрес поверх карты: «варп, потом зеркало» (Р.37 п.247).
#[allow(clippy::too_many_arguments)]
fn fill_source_signed(
    fetcher: &mut dyn AudioFetcher,
    media: kt_model::MediaId,
    src_frame: i64,
    window: SrcWindow,
    knots: &[kt_fx::AddrKnot],
    reversed: bool,
    dst: &mut [f32],
    underruns: &mut u64,
) -> bool {
    dst.fill(0.0);
    let SrcWindow { start: win_start, end: window_end, mirror_end, .. } = window;
    let frames = (dst.len() / AUDIO_CHANNELS) as i64;
    let mut f = 0i64;
    while f < frames {
        let virt = src_frame + f;
        if virt >= window_end {
            break; // дальше только тишина
        }
        if virt < 0 {
            f += 1;
            continue;
        }
        let (addr, dir, run_end) = addr_run_at(knots, virt - win_start);
        if dir == 0 {
            // удержание: сюда доходить не должно (чанк гасится в
            // процессоре), но молча читать чужой адрес — хуже тишины
            f += (run_end - (virt - win_start)).clamp(1, frames - f);
            continue;
        }
        // зеркало события — ПОВЕРХ карты
        let (src, dir) = if reversed {
            (win_start + (mirror_end - win_start) - 1 - addr, -dir)
        } else {
            (win_start + addr, dir)
        };
        if src < 0 {
            f += 1;
            continue;
        }
        let bidx = src.div_euclid(AUDIO_BURST_FRAMES as i64);
        let within = src.rem_euclid(AUDIO_BURST_FRAMES as i64);
        let room = if dir < 0 { within + 1 } else { AUDIO_BURST_FRAMES as i64 - within };
        let take =
            room.min(frames - f).min(window_end - virt).min(run_end - (virt - win_start)).max(1);
        match fetcher.audio_burst(media, bidx) {
            Some(b) => {
                let d0 = (f as usize) * AUDIO_CHANNELS;
                if dir < 0 {
                    for k in 0..take as usize {
                        let si = (within as usize - k) * AUDIO_CHANNELS;
                        for ch in 0..AUDIO_CHANNELS {
                            dst[d0 + k * AUDIO_CHANNELS + ch] = b.data[si + ch];
                        }
                    }
                } else {
                    let s0 = (within as usize) * AUDIO_CHANNELS;
                    let n = (take as usize) * AUDIO_CHANNELS;
                    dst[d0..d0 + n].copy_from_slice(&b.data[s0..s0 + n]);
                }
            }
            // единица — КАДРЫ, как у остальных путей (Р.43, п.289)
            None => *underruns += take.max(0) as u64,
        }
        f += take;
    }
    true
}
