//! Аудио-процессор клипа: цепочка FX поверх (возможно варпнутого)
//! источника. Публичная поверхность крейта реэкспортируется из lib.rs —
//! пути вызывающих (`kt_fx::ClipProcessor` и т.д.) не изменились.

mod nodes;
pub mod primitives;
mod warp;

pub use nodes::{AudioNode, Freeverb};
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

/// Конфиг силы слоя (Р.38 п.12) в узловой цепочке.
struct MixCfg {
    /// Слот узла-маркера LayerMix (по нему — кривая amount).
    slot: usize,
    /// Слот начала вклада слоя: снапшот сухой стороны берётся перед
    /// первым узлом со слотом >= from.
    from: usize,
    base_amount: f64,
}

/// Стейтфул-процессор аудио-клипа. Владелец — аудио-render-поток (kt-engine,
/// FxStateStore). Читается строго последовательно; разрыв → reset + pre-roll.
pub struct ClipProcessor {
    fingerprint: u64,
    /// Клип-локальный кадр, который выйдет следующим (маркер непрерывности).
    next_local: i64,
    src_in: i64,
    warp: Option<Warp>,
    /// Узлы цепочки В ПОРЯДКЕ СЛОТОВ (Р.47, п.325): обработка идёт по
    /// порядку, как у видео и в DAW. Слот хранится рядом — по нему узел
    /// берёт свои кривые из AutoProfiles.
    nodes: Vec<(usize, AudioNode)>,
    /// Сила слоя: узел-маркер стоит В цепочке, а снапшот сухой стороны
    /// берётся перед ПЕРВЫМ узлом со слотом >= from (честная позиционная
    /// семантика — прежний флаг reverb_is_layer стал её следствием).
    mix: Option<MixCfg>,
    rate: f64,
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
            nodes: Vec::new(),
            mix: None,
            rate: 1.0,
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
            for (slot, node) in &mut self.nodes {
                if let AudioNode::Gain(g) = node {
                    g.bind_curves(&self.auto, *slot);
                }
            }
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
    /// Отпечаток цепочки совпал ⇒ последовательность видов по слотам та же
    /// ⇒ узлы сопоставляются со слотами 1:1 (инвариант №3: состояние
    /// живёт, параметры доезжают сглаженно).
    pub fn apply_params(&mut self, fx: &[FxInstance], structural: bool) {
        if structural {
            self.nodes = fx
                .iter()
                .enumerate()
                .filter_map(|(slot, f)| AudioNode::build(f).map(|n| (slot, n)))
                .collect();
        }
        debug_assert_eq!(
            self.nodes.len(),
            fx.iter().filter(|f| AudioNode::build(f).is_some()).count(),
            "node list out of sync with the chain"
        );
        self.mix = fx.iter().position(|f| f.is_layer_mix()).map(|i| match &fx[i] {
            FxInstance::LayerMix { amount, from } => {
                MixCfg { slot: i, from: (*from as usize).min(i), base_amount: *amount }
            }
            _ => MixCfg { slot: i, from: usize::MAX, base_amount: 1.0 },
        });
        for (slot, node) in &mut self.nodes {
            node.apply(&fx[*slot], structural);
            if let AudioNode::Gain(g) = node {
                g.bind_curves(&self.auto, *slot);
            }
        }
        // варп-половина Slowed+Reverb (потребление источника)
        for f in fx {
            if let FxInstance::SlowedReverb { rate, .. } = f {
                self.rate = rate.clamp(0.25, 32.0);
                let has_rate_curve = !self.rate_points().is_empty();
                if let Some(w) = &mut self.warp
                    && !has_rate_curve
                {
                    // с автоматизацией produce() выставит по позиции сам
                    w.set_rate(self.rate);
                }
            }
        }
    }


    fn has_warp(&self) -> bool {
        (self.rate - 1.0).abs() > 1e-9
            || self.rate_points().iter().any(|(_, r)| (r - 1.0).abs() > 1e-9)
    }

    pub fn tail(&self) -> Tick {
        self.nodes.iter().map(|(_, n)| n.tail()).max().unwrap_or(Tick::ZERO)
    }

    pub fn tail_frames(&self) -> i64 {
        self.tail().to_audio_frames()
    }

    /// Полный сброс состояния и permission на новый seek.
    pub fn reset(&mut self) {
        self.warp = None;
        for (_, n) in &mut self.nodes {
            n.reset();
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
        for (_, n) in &mut self.nodes {
            n.reset();
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
                // цели узлов — тем же правилом середины чанка (узел сам
                // решает, какие из его параметров ведут кривые)
                for (slot, node) in &mut self.nodes {
                    node.set_targets(&self.auto, *slot, s);
                }
            }
            // цель силы слоя на середину чанка — тем же правилом, что и
            // остальные параметры (Р.38 п.12: фейды это её кривая)
            let mix_k = self.mix.as_ref().map(|m| {
                let tau = (chunk_local as f64 + n as f64 * 0.5) / self.clip_len as f64;
                self.auto.value(m.slot, "amount", m.base_amount, tau).clamp(0.0, 1.0) as f32
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
            // ЧЕСТНЫЙ ПОРЯДОК СЛОТОВ (Р.47, п.325): узлы исполняются по
            // позиции в цепочке. Сухая сторона силы слоя — снимок ПЕРЕД
            // первым узлом со слотом >= from (реверб/громкость события
            // стоят раньше — попадают в сухую; узлы слоя — только в
            // мокрую: прежние reverb_is_layer/layer_gain стали следствием
            // позиции, а не флагами).
            let mut dry: Option<Vec<f32>> = None;
            for (slot, node) in &mut self.nodes {
                if let Some(m) = &self.mix
                    && *slot >= m.from
                    && dry.is_none()
                {
                    dry = Some(sub.to_vec());
                }
                if matches!(node, AudioNode::LayerMix) {
                    if let (Some(k), Some(d)) = (mix_k, &dry) {
                        let ik = 1.0 - k;
                        for (v, dv) in sub.iter_mut().zip(d.iter()) {
                            *v = dv * ik + *v * k;
                        }
                    }
                } else {
                    node.process(sub, chunk_local);
                }
            }
            done += n;
        }
    }
}
