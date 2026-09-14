//! Аудио-процессор клипа: цепочка FX поверх (возможно варпнутого)
//! источника. Публичная поверхность крейта реэкспортируется из lib.rs —
//! пути вызывающих (`kt_fx::ClipProcessor` и т.д.) не изменились.

mod nodes;
pub mod primitives;
mod warp;

pub use nodes::Freeverb;
pub use primitives::SmoothedParam;
pub use warp::Warp;

use crate::{CH, MIN_PREROLL_FRAMES, PullKind};
use kt_model::FxInstance;
use kt_time::Tick;
use nodes::reverb_tail_for_delay;

use kt_model::{AutoProfiles, ParamPoints, eff_rate, rate_at, source_progress};


/// Длина хвоста цепочки БЕЗ создания state (для расширения окна клипа
/// в графе до того, как процессор построен).
/// Запас источника на прайминг rubato (sinc_len 128 × oversampling) и
/// чанк-квантование варпа: реальный курсор источника может обгонять
/// rate_integral на групповую задержку ресемплера (~3к кадров).
pub const WARP_SOURCE_MARGIN_FRAMES: i64 = 8192;

/// Окно источника (кадры ОТНОСИТЕЛЬНО source_in), которое ClipProcessor
/// реально потребит, выдавая клип-локальные кадры `local`, включая pre-roll
/// перед local.start и прайминг варпа. Для цепочки без варпа — линейно и
/// без запаса. ЕДИНЫЙ источник правды для prefetch (wanted) и рендера
/// (window_end): расхождение этих двух расчётов = вечный промах кэша и
/// тишина (регрессия п.13, тест 17).
pub fn chain_source_window(
    fx: &[FxInstance],
    auto: &AutoProfiles,
    clip_len_frames: i64,
    local: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    chain_source_window_ext(fx, auto, clip_len_frames, 0..clip_len_frames.max(1), local)
}

/// То же, но для клипа с «ручками» перехода (п.138): material — допустимый
/// клип-локальный диапазон [−head_ext, clip_len + tail_ext); нормировка
/// автоматизации остаётся по clip_len. Для material = 0..len поведение
/// байт-идентично chain_source_window (тест 17 не двигается).
pub fn chain_source_window_ext(
    fx: &[FxInstance],
    auto: &AutoProfiles,
    clip_len_frames: i64,
    material: std::ops::Range<i64>,
    local: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    let len = clip_len_frames.max(1);
    let preroll = chain_tail(fx, auto).to_audio_frames().max(MIN_PREROLL_FRAMES);
    let l0 = (local.start - preroll).max(material.start).min(material.end);
    let l1 = local.end.clamp(material.start, material.end);
    let has_warp = fx.iter().any(|f| matches!(f, FxInstance::SlowedReverb { .. }));
    if !has_warp {
        return l0..l1.max(l0);
    }
    let rate = fx
        .iter()
        .find_map(|f| match f {
            FxInstance::SlowedReverb { rate, .. } => Some(*rate),
            _ => None,
        })
        .unwrap_or(1.0);
    let automation = auto.rate_points(fx);
    // окно источника не уходит ниже нуля только когда материал начинается
    // с тела клипа (старое поведение); левая ручка легально отрицательна
    let s_floor = if material.start >= 0 { 0 } else { i64::MIN };
    let t0 = l0 as f64 / len as f64;
    let t1 = l1 as f64 / len as f64;
    let s0 = ((len as f64 * source_progress(rate, automation, t0)).floor() as i64)
        .saturating_sub(WARP_SOURCE_MARGIN_FRAMES)
        .max(s_floor);
    let s1 = (len as f64 * source_progress(rate, automation, t1)).ceil() as i64
        + WARP_SOURCE_MARGIN_FRAMES;
    s0..s1.max(s0)
}

/// Сколько кадров источника событие потребляет ЦЕЛИКОМ: S(1)·len, без
/// запаса ресемплера и без пре-ролла.
///
/// Это **ОСЬ ЗЕРКАЛА** у reversed-клипа (Р.43, п.292) — ровно та же
/// величина, что `ClipWarp::used` у видео. До Р.43 аудио зеркалило от
/// `chain_source_window_ext(..).end`, а туда входит
/// `WARP_SOURCE_MARGIN_FRAMES` = 8192 кадра (170.7 мс): зеркальный
/// замедленный клип играл материал, сдвинутый на эти 170 мс, и звучал
/// «звуком от другого события» (претензия Р.43 п.3), а картинка на том же
/// событии оставалась на месте — A/V расходились.
pub fn chain_source_used(fx: &[FxInstance], auto: &AutoProfiles, clip_len_frames: i64) -> i64 {
    let len = clip_len_frames.max(1);
    let Some(rate) = fx.iter().find_map(|f| match f {
        FxInstance::SlowedReverb { rate, .. } => Some(*rate),
        _ => None,
    }) else {
        return len;
    };
    (len as f64 * source_progress(rate, auto.rate_points(fx), 1.0)).ceil() as i64
}

pub fn chain_tail(fx: &[FxInstance], auto: &AutoProfiles) -> Tick {
    let wet = auto.mix_points(fx);
    fx.iter()
        .map(|f| match f {
            FxInstance::SlowedReverb { mix, reverb_delay_ms, decay, .. }
                if *mix > 0.0 || wet.iter().any(|(_, v)| *v > 0.0) =>
            {
                reverb_tail_for_delay(*reverb_delay_ms, *decay)
            }
            _ => Tick::ZERO,
        })
        .max()
        .unwrap_or(Tick::ZERO)
}


/// Отпечаток СТРУКТУРЫ цепочки (виды FX по порядку). Совпадает → state
/// переиспользуется, параметры доезжают через set_params (сглаженно).
pub fn chain_fingerprint(fx: &[FxInstance]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in fx {
        std::mem::discriminant(f).hash(&mut h);
    }
    h.finish()
}

/// Стейтфул-процессор аудио-клипа. Владелец — аудио-render-поток (kt-engine,
/// FxStateStore). Читается строго последовательно; разрыв → reset + pre-roll.
pub struct ClipProcessor {
    fingerprint: u64,
    /// Клип-локальный кадр, который выйдет следующим (маркер непрерывности).
    next_local: i64,
    src_in: i64,
    warp: Option<Warp>,
    reverb: Option<Freeverb>,
    /// Множитель Gain-слотов БЕЗ кривой (константный путь — байт-в-байт
    /// как до Р.33, на нём стоит GOLDEN_AUDIO).
    static_gain: f32,
    /// СИЛА СЛОЯ (Р.38 п.12): слот смешения, индекс начала вклада слоя и
    /// базовое значение. `None` — слоя с силой нет, и весь путь ниже
    /// байт-в-байт прежний (на нём стоит GOLDEN_AUDIO).
    mix_slot: Option<usize>,
    mix_from: usize,
    base_layer_mix: f64,
    /// Реверб принадлежит СОБЫТИЮ, а не слою: тогда он звучит и в сухой
    /// стороне смешения. Движок держит ровно один реверб на цепочку,
    /// поэтому выбор сводится к одному флагу.
    reverb_is_layer: bool,
    /// Множитель Gain-слотов СЛОЯ без кривой: он идёт только в мокрую
    /// сторону. Без слоя равен 1.0 — цикл не выполняется вовсе.
    layer_gain: f32,
    /// Баланс каналов (Р.33 С5): множители левого и правого. Оба 1.0 —
    /// прежний путь, цикл не выполняется вовсе.
    pan_l: f32,
    pan_r: f32,
    /// Слоты Gain с КРИВОЙ баланса и сглаживание их множителей.
    pan_automated: bool,
    pan_slots: Vec<(usize, f64)>,
    auto_pan_l: SmoothedParam,
    auto_pan_r: SmoothedParam,
    /// Сглаженная громкость автоматизированных Gain-слотов: цель ставится
    /// раз в чанк, к ней едем one-pole — шаг чанка не щёлкает.
    auto_gain: SmoothedParam,
    /// Есть хоть один Gain-слот с кривой?
    gain_automated: bool,
    /// Автоматизированные Gain-слоты: (индекс, базовое значение db).
    gain_slots: Vec<(usize, f64)>,
    rate: f64,
    base_mix: f32,
    /// Кривые клипа в SOURCE-домене. Владеет ими аудио-render-поток
    /// (инвариант №2): ни локов, ни обращений к модели на кадре.
    auto: AutoProfiles,
    /// Индекс слота с варпом — по нему берутся профили rate/mix.
    warp_slot: usize,
    /// Длина клипа в кадрах — знаменатель прогресса автоматизации.
    clip_len: i64,
    /// «Ручки» перехода (п.138): материал клип-локально расширен до
    /// [−head_ext, clip_len + tail_ext). Нормировку автоматизации НЕ
    /// двигают — знаменатель остаётся clip_len.
    head_ext: i64,
    tail_ext: i64,
    /// Курсор источника для пути без варпа (rate == 1, без автоматизации).
    plain_cursor: i64,
}

impl ClipProcessor {
    pub fn new(
        fx: &[FxInstance],
        auto: &AutoProfiles,
        src_in_frames: i64,
        clip_len_frames: i64,
    ) -> ClipProcessor {
        let mut p = ClipProcessor {
            fingerprint: chain_fingerprint(fx),
            next_local: i64::MIN,
            src_in: src_in_frames,
            warp: None,
            reverb: None,
            static_gain: 1.0,
            mix_slot: None,
            mix_from: usize::MAX,
            base_layer_mix: 1.0,
            reverb_is_layer: false,
            layer_gain: 1.0,
            pan_l: 1.0,
            pan_r: 1.0,
            pan_automated: false,
            pan_slots: Vec::new(),
            auto_pan_l: SmoothedParam::new(1.0),
            auto_pan_r: SmoothedParam::new(1.0),
            auto_gain: SmoothedParam::new(1.0),
            gain_automated: false,
            gain_slots: Vec::new(),
            rate: 1.0,
            base_mix: 0.0,
            auto: auto.clone(),
            warp_slot: fx
                .iter()
                .position(|f| matches!(f, FxInstance::SlowedReverb { .. }))
                .unwrap_or(0),
            clip_len: clip_len_frames.max(1),
            head_ext: 0,
            tail_ext: 0,
            plain_cursor: src_in_frames,
        };
        p.apply_params(fx, true);
        p
    }

    /// Профиль скорости этого клипа (пустой — варпа/кривой нет).
    fn rate_points(&self) -> &ParamPoints {
        self.auto.points(self.warp_slot, "rate")
    }

    /// Обновить кривые БЕЗ сброса состояния: правка кейфрейма не должна
    /// глушить хвост реверба (инвариант №3).
    pub fn set_auto(&mut self, auto: &AutoProfiles) {
        if &self.auto != auto {
            self.auto = auto.clone();
        }
    }

    /// Обновить длину клипа (trim) без сброса состояния.
    pub fn set_clip_len(&mut self, clip_len_frames: i64) {
        self.clip_len = clip_len_frames.max(1);
    }

    /// «Ручки» перехода (п.138): разрешить чтение [−head_ext,
    /// clip_len + tail_ext). Без сброса состояния (структура цепочки та же);
    /// dry-гейт сдвигается на tail_ext, нормировка автоматизации — нет.
    pub fn set_material_ext(&mut self, head_ext: i64, tail_ext: i64) {
        self.head_ext = head_ext.max(0);
        self.tail_ext = tail_ext.max(0);
    }

    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Донести параметры. `structural` = первый раз (создание объектов).
    pub fn apply_params(&mut self, fx: &[FxInstance], structural: bool) {
        let mut gain = 1.0f32;
        let (mut pan_l, mut pan_r) = (1.0f32, 1.0f32);
        self.gain_automated = false;
        self.pan_automated = false;
        // слот СИЛЫ слоя (Р.38 п.12) ищется до всего остального: от него
        // зависит, что попадёт в сухую сторону смешения
        self.mix_slot = fx.iter().position(|f| f.is_layer_mix());
        (self.mix_from, self.base_layer_mix) = match self.mix_slot {
            Some(i) => match &fx[i] {
                FxInstance::LayerMix { amount, from } => ((*from as usize).min(i), *amount),
                _ => (usize::MAX, 1.0),
            },
            None => (usize::MAX, 1.0),
        };
        self.reverb_is_layer = fx
            .iter()
            .position(|f| matches!(f, FxInstance::SlowedReverb { .. }))
            .map(|i| i >= self.mix_from)
            .unwrap_or(true);
        let mut layer_gain = 1.0f32;
        for (slot, f) in fx.iter().enumerate() {
            match f {
                // громкость с кривой считается по материалу в produce();
                // без кривой — прежним одним множителем на весь буфер
                FxInstance::Gain { db, pan } => {
                    if self.auto.points(slot, "db").is_empty() {
                        // громкость СЛОЯ идёт только в мокрую сторону:
                        // сила 0 обязана вернуть событие как было
                        if slot >= self.mix_from {
                            layer_gain *= 10f32.powf(db / 20.0);
                        } else {
                            gain *= 10f32.powf(db / 20.0);
                        }
                    } else {
                        self.gain_automated = true;
                    }
                    // баланс: линейный закон, на нуле оба множителя ровно
                    // 1.0 — прежний путь байт-в-байт
                    if self.auto.points(slot, "pan").is_empty() {
                        let p = pan.clamp(-1.0, 1.0);
                        pan_l *= (1.0 - p).min(1.0);
                        pan_r *= (1.0 + p).min(1.0);
                    } else {
                        self.pan_automated = true;
                    }
                }
                FxInstance::SlowedReverb {
                    rate,
                    mix,
                    reverb_delay_ms,
                    damp,
                    decay,
                    wet_gain,
                    predelay_ms,
                    width,
                    ..
                } => {
                    self.rate = rate.clamp(0.25, 32.0);
                    self.base_mix = *mix;
                    if structural {
                        self.reverb =
                            Some(Freeverb::new(
                            *mix,
                            *reverb_delay_ms,
                            *damp,
                            *decay,
                            *wet_gain,
                            *predelay_ms,
                            *width,
                        ));
                    } else if let Some(rv) = &mut self.reverb {
                        rv.set_params(
                            *mix,
                            *reverb_delay_ms,
                            *damp,
                            *decay,
                            *wet_gain,
                            *predelay_ms,
                            *width,
                        );
                    }
                    let has_rate_curve = !self.rate_points().is_empty();
                    if let Some(w) = &mut self.warp
                        && !has_rate_curve
                    {
                        // с автоматизацией produce() выставит по позиции сам
                        w.set_rate(self.rate);
                    }
                }
                // видео-FX аудио-процессором игнорируются
                _ => {}
            }
        }
        self.static_gain = gain;
        self.layer_gain = layer_gain;
        self.pan_l = pan_l;
        self.pan_r = pan_r;
        self.gain_slots = fx
            .iter()
            .enumerate()
            .filter_map(|(i, f)| match f {
                FxInstance::Gain { db, .. } if !self.auto.points(i, "db").is_empty() => {
                    Some((i, *db as f64))
                }
                _ => None,
            })
            .collect();
        self.pan_slots = fx
            .iter()
            .enumerate()
            .filter_map(|(i, f)| match f {
                FxInstance::Gain { pan, .. } if !self.auto.points(i, "pan").is_empty() => {
                    Some((i, *pan as f64))
                }
                _ => None,
            })
            .collect();
    }

    /// Баланс автоматизированных Gain-слотов в позиции материала s.
    /// Возвращает пару множителей (левый, правый) — тот же линейный
    /// закон, что у константного пути.
    fn pan_at(&self, s: f64) -> (f32, f32) {
        let (mut l, mut r) = (1.0f32, 1.0f32);
        for (slot, base) in &self.pan_slots {
            let p = self.auto.value(*slot, "pan", *base, s).clamp(-1.0, 1.0) as f32;
            l *= (1.0 - p).min(1.0);
            r *= (1.0 + p).min(1.0);
        }
        (l, r)
    }

    /// Множитель автоматизированных Gain-слотов в позиции материала s.
    fn gain_at(&self, s: f64) -> f32 {
        let mut g = 1.0f32;
        for (slot, base_db) in &self.gain_slots {
            let db = self.auto.value(*slot, "db", *base_db, s);
            g *= 10f32.powf(db as f32 / 20.0);
        }
        g
    }

    fn has_warp(&self) -> bool {
        (self.rate - 1.0).abs() > 1e-9
            || self.rate_points().iter().any(|(_, r)| (r - 1.0).abs() > 1e-9)
    }

    pub fn tail(&self) -> Tick {
        self.reverb.as_ref().map(|r| r.tail()).unwrap_or(Tick::ZERO)
    }

    pub fn tail_frames(&self) -> i64 {
        self.tail().to_audio_frames()
    }

    /// Полный сброс состояния и permission на новый seek.
    pub fn reset(&mut self) {
        self.warp = None;
        if let Some(r) = &mut self.reverb {
            r.reset();
        }
        self.next_local = i64::MIN;
    }

    /// Материальный якорь события (кадры от нуля файла).
    pub fn src_in(&self) -> i64 {
        self.src_in
    }

    /// Переставить материальный якорь, СОХРАНИВ состояние FX (Р.43, п.293).
    ///
    /// `chain_fingerprint` знает только ВИДЫ слотов, поэтому трим, разрез,
    /// punch-out и переанкоринг морф-куска меняли `source_in` при том же
    /// `ClipId` и той же структуре цепочки — и процессор продолжал читать
    /// от СТАРОГО якоря. `reset()` не помогал: `hard_seek`/`seam` считают
    /// позицию как `self.src_in + source_offset(local)`. Симптом —
    /// «звук от другого события откуда-то взялся».
    ///
    /// Это ШОВ, а не seek: выход непрерывен, реверб обязан дозвонить
    /// (инвариант №3).
    pub fn set_src_in(&mut self, src_in: i64) {
        if self.src_in == src_in {
            return;
        }
        self.src_in = src_in;
        if self.next_local != i64::MIN {
            let local = self.next_local;
            self.seam(local);
        }
    }

    /// Прочитать кадры [local, local + out/CH) клип-локального потока.
    /// `pull(src_frame, buf)` — источник (false = underrun).
    /// Возвращает счётчик underrun'ов warp-пути.
    pub fn read(
        &mut self,
        local: i64,
        out: &mut [f32],
        pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool,
    ) -> u64 {
        if self.next_local != local {
            if self.next_local == i64::MIN {
                // Холодный старт или seek (`reset`): reset + pre-roll,
                // чтобы реверб и ресемплер были прогреты детерминированно.
                self.hard_seek(local, pull);
            } else {
                // ШОВ: позиция прыгнула, а ВЫХОД непрерывен (заворот
                // loop-региона, сдвиг клипа мышью при игре). Реверб
                // дозванивает через шов — как в v1; пре-ролла нет, греть
                // нечего. Это же снимает пересоздание sinc-ресемплера на
                // каждом круге (при регионе короче бёрста их было
                // несколько за один callback — «шумит компьютер», п.166).
                self.seam(local);
            }
        }
        let before = self.warp.as_ref().map(|w| w.underruns()).unwrap_or(0);
        self.produce(PullKind::Output, local, out, pull);
        self.next_local = local + (out.len() / CH) as i64;
        self.warp.as_ref().map(|w| w.underruns()).unwrap_or(0) - before
    }

    /// Смещение источника (кадры от src_in) для клип-локальной позиции:
    /// S(t) — инверсия карты source-домена (см. output_time_for_source).
    fn source_offset(&self, local: i64) -> i64 {
        let t = local as f64 / self.clip_len as f64;
        (self.clip_len as f64 * source_progress(self.rate, self.rate_points(), t)).round() as i64
    }

    /// Разрыв ПОЗИЦИИ при непрерывном ВЫХОДЕ: источник переставляется,
    /// состояние FX живёт. Ни reset'а, ни пре-ролла (см. `read`).
    fn seam(&mut self, local: i64) {
        let src_start = self.src_in + self.source_offset(local);
        if self.has_warp() {
            let t = local as f64 / self.clip_len as f64;
            let r = rate_at(self.rate, self.rate_points(), t);
            match &mut self.warp {
                Some(w) => {
                    w.set_rate(r);
                    w.seek_source(src_start);
                }
                None => self.warp = Some(Warp::new(r, src_start)),
            }
        } else {
            self.warp = None;
        }
        self.plain_cursor = src_start;
    }

    fn hard_seek(&mut self, local: i64, pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool) {
        if let Some(r) = &mut self.reverb {
            r.reset();
        }
        // pre-roll может заходить в левую «ручку», но не раньше начала
        // материала (−head_ext)
        let preroll = self.tail_frames().max(MIN_PREROLL_FRAMES).min((local + self.head_ext).max(0));
        let start = local - preroll;
        // позиция источника для local=start (интеграл автоматизации)
        let src_start = self.src_in + self.source_offset(start);
        if self.has_warp() {
            let t = start as f64 / self.clip_len as f64;
            let mut w = Warp::new(rate_at(self.rate, self.rate_points(), t), src_start);
            w.set_rate(rate_at(self.rate, self.rate_points(), t));
            self.warp = Some(w);
        } else {
            self.warp = None;
        }
        self.plain_cursor = src_start;
        // прогрев: рендерим в мусор
        let mut scratch = vec![0f32; 1024 * CH];
        let mut done = 0i64;
        while done < preroll {
            let n = (preroll - done).min(1024) as usize;
            self.produce(PullKind::Warmup, start + done, &mut scratch[..n * CH], pull);
            done += n as i64;
        }
    }

    /// Максимальный шаг оценки автоматизации: буфер режется на чанки этого
    /// размера НЕЗАВИСИМО от размера запроса — офлайн-рендер, отдающий клип
    /// одним куском, обязан следовать профилю так же, как playback
    /// по-burst'ово. (Регрессия: оценка один раз на весь буфер давала
    /// константное замедление по значению в середине клипа и оборванный
    /// конец источника; поймано на слух, закрыто тестами
    /// t4_automation_changes_rate_over_time / _consumes_source_by_integral.)
    const AUTOMATION_STEP_FRAMES: usize = 1024;

    fn produce(
        &mut self,
        kind: PullKind,
        local: i64,
        out: &mut [f32],
        pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool,
    ) {
        let total = out.len() / CH;
        let mut done = 0usize;
        while done < total {
            let n = (total - done).min(Self::AUTOMATION_STEP_FRAMES);
            let chunk_local = local + done as i64;
            // автоматизация: цели на середину чанка; rubato/one-pole сглаживают
            if !self.auto.is_empty() {
                // ДОМЕН СОБЫТИЯ (Р.37): цель чанка — по ВЫХОДНОМУ времени,
                // а не по позиции в материале. Ранний кейфрейм скорости
                // больше не сдвигает МОМЕНТ срабатывания поздних ключей —
                // ни своих, ни чужих (mix/gain/pan индексировались тем же
                // `s`). Правило середины чанка при этом стало ТОЧНЫМ:
                // подынтегральная функция линейна, а середина трапеции —
                // её точное среднее.
                let tau = (chunk_local as f64 + n as f64 * 0.5) / self.clip_len as f64;
                let s = tau;
                let target_rate = eff_rate(self.rate, self.rate_points(), s);
                if let Some(w) = &mut self.warp {
                    w.set_rate(target_rate);
                }
                if let Some(r) = &mut self.reverb {
                    // mix сглаживается one-pole внутри реверба; остальные
                    // параметры реверба — тем же чанком
                    let slot = self.warp_slot;
                    r.set_mix(self.auto.value(slot, "mix", self.base_mix as f64, s) as f32);
                }
                // громкость: кривая на любом Gain-слоте цепочки
                if self.gain_automated {
                    self.auto_gain.set_target(self.gain_at(s));
                }
                if self.pan_automated {
                    let (l, r) = self.pan_at(s);
                    self.auto_pan_l.set_target(l);
                    self.auto_pan_r.set_target(r);
                }
            }
            // цель силы слоя на середину чанка — тем же правилом, что и
            // остальные параметры (Р.38 п.12: фейды это её кривая)
            let mix_k = self.mix_slot.map(|slot| {
                let tau = (chunk_local as f64 + n as f64 * 0.5) / self.clip_len as f64;
                self.auto.value(slot, "amount", self.base_layer_mix, tau).clamp(0.0, 1.0) as f32
            });
            let sub = &mut out[done * CH..(done + n) * CH];
            // Гейт dry-выхода в OUTPUT-домене: за концом МАТЕРИАЛА (тело +
            // правая «ручка» перехода, п.138) сухой сигнал обязан быть нулём
            // (в хвосте дозванивает только реверб). Благодаря этому окно
            // ИСТОЧНИКА можно расширять с запасом на прайминг rubato, не
            // боясь утечки dry в хвост (дефект «тихий хвост»: sinc-ресемплер
            // видел нули за жёстко обрезанным окном и рано глушил конец).
            let gate_end = self.clip_len + self.tail_ext;
            if chunk_local >= gate_end {
                // ЧАНК ЦЕЛИКОМ В ХВОСТЕ (Р.43, п.304): что бы ни выдал
                // варп, гейт ниже обнулит это ПОЛНОСТЬЮ — значит считать
                // нечего. Пропуск байт-идентичен ПО ПОСТРОЕНИЮ: результат
                // тот же `fill(0.0)`, только без sinc-ресемплера и без
                // чтения источника. В хвосте живёт один реверб, а он
                // получает те же нули.
                //
                // Состояние варпа при этом «отстаёт», и это безопасно:
                // вернуться в материал можно только прыжком `local`, а его
                // ловит `read` и переставляет источник через `seam`/
                // `hard_seek`. Непрерывное чтение `local` только растёт.
                sub.fill(0.0);
                self.plain_cursor += n as i64;
            } else {
                match &mut self.warp {
                    Some(w) => w.produce(kind, sub, pull),
                    None => {
                        if !pull(kind, self.plain_cursor, sub) {
                            sub.fill(0.0);
                        }
                        self.plain_cursor += n as i64;
                    }
                }
                if chunk_local + (n as i64) > gate_end {
                    let cut = (gate_end - chunk_local).clamp(0, n as i64) as usize;
                    sub[cut * CH..].fill(0.0);
                }
            }
            // СУХАЯ сторона смешения (Р.38 п.12): состояние ДО вклада
            // слоя. Реверб у движка ровно один на цепочку, поэтому вопрос
            // «чей он» решается одним флагом: реверб СОБЫТИЯ звучит и в
            // сухой стороне, реверб СЛОЯ — только в мокрой.
            // реверб СОБЫТИЯ звучит и в сухой стороне (снимок после
            // него), реверб СЛОЯ — только в мокрой (снимок до)
            let mut dry: Option<Vec<f32>> = match mix_k {
                Some(_) if self.reverb_is_layer => Some(sub.to_vec()),
                Some(_) => Some(Vec::new()),
                None => None,
            };
            if let Some(r) = &mut self.reverb {
                r.process(sub);
            }
            if let Some(d) = dry.as_mut()
                && d.is_empty()
            {
                *d = sub.to_vec();
            }
            // автоматизированная громкость — ПОСЛЕ реверба, как и
            // константная: цепочка кончается уровнем события
            if self.gain_automated {
                for f in 0..n {
                    let g = self.auto_gain.tick();
                    for ch in 0..CH {
                        sub[f * CH + ch] *= g;
                    }
                }
            }
            if self.pan_automated {
                for f in sub.chunks_exact_mut(CH) {
                    let (l, r) = (self.auto_pan_l.tick(), self.auto_pan_r.tick());
                    f[0] *= l;
                    if CH > 1 {
                        f[1] *= r;
                    }
                }
            }
            // громкость СЛОЯ — только на мокрую сторону, потом смешение
            if let Some(k) = mix_k {
                if self.layer_gain != 1.0 {
                    for v in sub.iter_mut() {
                        *v *= self.layer_gain;
                    }
                }
                if let Some(d) = &dry {
                    let ik = 1.0 - k;
                    for (v, dv) in sub.iter_mut().zip(d.iter()) {
                        *v = dv * ik + *v * k;
                    }
                }
            }
            done += n;
        }
        if self.static_gain != 1.0 {
            for s in out.iter_mut() {
                *s *= self.static_gain;
            }
        }
        // баланс — последним: на нуле множители 1.0 и цикл не идёт вовсе
        if self.pan_l != 1.0 || self.pan_r != 1.0 {
            for f in out.chunks_exact_mut(CH) {
                f[0] *= self.pan_l;
                if CH > 1 {
                    f[1] *= self.pan_r;
                }
            }
        }
    }
}
