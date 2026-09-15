//! Аудио-процессор клипа: цепочка FX поверх (возможно варпнутого)
//! источника. Публичная поверхность крейта реэкспортируется из lib.rs —
//! пути вызывающих (`kt_fx::ClipProcessor` и т.д.) не изменились.

pub mod loudness;
mod nodes;
pub mod primitives;
mod warp;

pub use nodes::{AudioNode, Freeverb, eq_mag_db};
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
    // ВПЕРЁД — на упреждение цепочки (Р.48, п.337): столько источника
    // цепочка реально прочитает, отдавая тот же выход. Правка живёт в
    // ОДНОЙ функции, поэтому рендер и префетч расширяются одинаково
    // (разойдутся — вечный промах кэша, п.13/t17).
    let l1 = (local.end + chain_lookahead(fx, auto)).clamp(material.start, material.end);
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

/// Упреждение цепочки в кадрах (Р.48, п.337): на сколько ВХОД цепочки
/// обгоняет её выход. Эффект с окном анализа (лимитер, питч) читает
/// источник ВПЕРЁД, а выход отдаёт выровненным — для остального движка
/// задержка нулевая (playhead, фейды и A/V-синхрон не двигаются).
///
/// Величина входит в прогрев `hard_seek` и в окно источника
/// (`chain_source_window_ext` — одна функция кормит и рендер, и префетч),
/// но НЕ входит в `chain_tail`: «сколько греть» и «сколько звучать»
/// разведены (иначе клип звучал бы после собственного конца).
///
/// Формула вида обязана совпадать с `AudioNode::lookahead` того же вида —
/// согласие сторожит t322ж.
pub fn chain_lookahead(fx: &[FxInstance], auto: &AutoProfiles) -> i64 {
    fx.iter()
        .enumerate()
        .map(|(slot, f)| match f {
            FxInstance::Limiter { lookahead_ms, .. } => nodes::limiter_la_frames(*lookahead_ms),
            // у питча нейтраль зависит от КРИВЫХ: база «ноль» при живой
            // кривой нейтралью не является
            FxInstance::PitchShift { semitones, cents } => {
                let neutral = nodes::pitch_is_neutral(
                    *semitones,
                    *cents,
                    auto.points(slot, "semitones"),
                    auto.points(slot, "cents"),
                );
                if neutral { 0 } else { nodes::PITCH_LOOKAHEAD_FRAMES }
            }
            _ => 0i64,
        })
        .sum()
}

/// Настройки ПРОЕКТА, влияющие на исполнение цепочки (Р.48): движок
/// питча один на весь монтаж. Не значение параметра, а устройство узла —
/// смена пересобирает процессор (сверка в `FxStateStore`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChainOpts {
    pub pitch_engine: kt_model::PitchEngine,
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
            // звон резонансного среза: t60 = 6.9*Q/(pi*f0) — та же
            // формула, что у FilterNode::tail (согласие сторожит t322е)
            // модуляции с памятью: максимум их задержек (те же константы,
            // что в узлах — согласие сторожит t322е)
            // эхо: t60 затухающих повторов (гейт по mix — вкл. кривую,
            // как у реверба); формула та же, что в узле (t322е)
            FxInstance::Delay { time_ms, feedback, mix, .. }
                if *mix > 0.0 || wet.iter().any(|(_, v)| *v > 0.0) =>
            {
                crate::audio::primitives::tail_frames_for(
                    time_ms.clamp(1.0, 1500.0) / 1000.0 * 48_000.0,
                    feedback.clamp(0.01, 0.9),
                )
            }
            FxInstance::Vibrato { .. } => Tick::from_audio_frames((48_000.0 * 0.008) as i64),
            FxInstance::Chorus { .. } => Tick::from_audio_frames((48_000.0 * 0.030) as i64),
            FxInstance::Flanger { .. } => {
                crate::audio::primitives::tail_frames_for(48_000.0 * 0.010, 0.9)
            }
            FxInstance::Filter { cutoff_hz, resonance, .. } => {
                let secs = 6.9 * resonance.clamp(0.5, 10.0)
                    / (std::f32::consts::PI * cutoff_hz.clamp(20.0, 20_000.0));
                Tick::from_audio_frames((secs * 48_000.0) as i64)
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
    /// Настройки проекта, влияющие на устройство узлов (движок питча).
    opts: ChainOpts,
    /// Упреждение цепочки (Р.48, п.337): вход обгоняет выход на столько
    /// кадров. 0 — путь арифметически тождествен доpdc-версии.
    la: i64,
    /// Опережение содержимого, которое видит узел i: сумма упреждений
    /// узлов, стоящих НЕ РАНЬШЕ него (каждый упреждающий узел «съедает»
    /// своё окно собственным кольцом, и к концу цепочки остаётся 0).
    ahead: Vec<i64>,
    /// Опережение на позиции узла-маркера LayerMix (для кривой amount).
    mix_ahead: i64,
    /// Кольцо выравнивания сухой стороны LayerMix: сколько кадров
    /// «съедено» упреждающими узлами между снимком и смешением.
    dry_ring: Option<DryRing>,
}

/// Задержка сухой стороны на целое число кадров (Р.48): между снимком
/// сухой стороны и узлом-маркером LayerMix может стоять упреждающий узел,
/// «съевший» часть опережения — без выравнивания бленд смешал бы разные
/// моменты материала. При нулевой дельте кольцо не создаётся вовсе.
struct DryRing {
    buf: Vec<f32>,
    pos: usize,
}

impl DryRing {
    fn new(frames: i64) -> DryRing {
        DryRing { buf: vec![0.0; frames.max(1) as usize * CH], pos: 0 }
    }

    /// Пропустить буфер через задержку НА МЕСТЕ (вход → кольцо, выход из
    /// кольца → буфер).
    fn run(&mut self, buf: &mut [f32]) {
        let len = self.buf.len();
        for v in buf.iter_mut() {
            std::mem::swap(&mut self.buf[self.pos], v);
            self.pos = (self.pos + 1) % len;
        }
    }

    fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

impl ClipProcessor {
    pub fn new(
        fx: &[FxInstance],
        auto: &AutoProfiles,
        src_in_frames: i64,
        clip_len_frames: i64,
    ) -> ClipProcessor {
        Self::new_with_opts(fx, auto, src_in_frames, clip_len_frames, ChainOpts::default())
    }

    /// То же, но с настройками проекта (движок питча). Продукт зовёт
    /// именно её; короткая `new` остаётся для тестов и офлайн-путей.
    pub fn new_with_opts(
        fx: &[FxInstance],
        auto: &AutoProfiles,
        src_in_frames: i64,
        clip_len_frames: i64,
        opts: ChainOpts,
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
            opts,
            la: 0,
            ahead: Vec::new(),
            mix_ahead: 0,
            dry_ring: None,
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
                node.bind_lfo(&self.auto, *slot, self.clip_len);
            }
            // нейтраль упреждающего узла может зависеть от кривых
            self.recompute_lookahead();
        }
    }

    /// Пересчитать упреждение цепочки и опережения узлов (Р.48, п.337).
    /// `ahead[i] = Σ lookahead узлов, стоящих не раньше i` — узел видит
    /// содержимое, обгоняющее выход ровно на столько; к концу цепочки
    /// сумма обнуляется, поэтому выход выровнен со входом.
    fn recompute_lookahead(&mut self) {
        let n = self.nodes.len();
        self.ahead.clear();
        self.ahead.resize(n, 0);
        let mut acc = 0i64;
        for i in (0..n).rev() {
            acc += self.nodes[i].1.lookahead();
            self.ahead[i] = acc;
        }
        self.la = acc;
        // опережение на позиции маркера силы слоя и дельта сухой стороны
        let (mix_ahead, dry_delta) = match &self.mix {
            Some(m) => {
                let at = |slot: usize| {
                    self.nodes
                        .iter()
                        .position(|(s, _)| *s >= slot)
                        .map(|i| self.ahead[i])
                        .unwrap_or(0)
                };
                let mix_at = self
                    .nodes
                    .iter()
                    .position(|(s, _)| *s == m.slot)
                    .map(|i| self.ahead[i])
                    .unwrap_or(0);
                (mix_at, (at(m.from) - mix_at).max(0))
            }
            None => (0, 0),
        };
        self.mix_ahead = mix_ahead;
        self.dry_ring = (dry_delta > 0).then(|| DryRing::new(dry_delta));
    }

    /// Упреждение цепочки в кадрах — та же величина, что отдаёт
    /// статическая `chain_lookahead` (сторожит t322ж). По ней
    /// `FxStateStore` понимает, что процессор пора пересобрать.
    pub fn lookahead_frames(&self) -> i64 {
        self.la
    }

    /// Настройки проекта, с которыми процессор построен: разошлись с
    /// моделью — процессор пересобирается (`FxStateStore`).
    pub fn opts(&self) -> ChainOpts {
        self.opts
    }

    /// Обновить длину клипа (trim) без сброса состояния.
    pub fn set_clip_len(&mut self, clip_len_frames: i64) {
        self.clip_len = clip_len_frames.max(1);
        for (slot, node) in &mut self.nodes {
            node.bind_lfo(&self.auto, *slot, self.clip_len);
        }
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
            if structural {
                node.set_engine(self.opts.pitch_engine);
            }
            node.apply(&fx[*slot], structural);
            if let AudioNode::Gain(g) = node {
                g.bind_curves(&self.auto, *slot);
            }
            node.bind_lfo(&self.auto, *slot, self.clip_len);
        }
        self.recompute_lookahead();
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
        if let Some(r) = &mut self.dry_ring {
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
        // источник встаёт под ВХОДНУЮ позицию: цепочка читает вперёд на
        // упреждение (Р.48, п.337). Кольца упреждающих узлов при этом
        // живут — шов дозванивает, как реверб (инвариант №3).
        let local = local + self.la;
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
        let preroll =
            self.tail_frames().max(MIN_PREROLL_FRAMES).min((local + self.head_ext).max(0));
        let start = local - preroll;
        // позиция источника для local=start (интеграл автоматизации).
        // Формула НЕ меняется: `start` — первая ВХОДНАЯ позиция прогрева,
        // а выходной цикл стартует на упреждение раньше (Р.48, п.337).
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
        if let Some(r) = &mut self.dry_ring {
            r.reset();
        }
        // Прогрев: рендерим в мусор. Выходной цикл начинается на
        // упреждение РАНЬШЕ (его первый чанк читает вход ровно с
        // `start`), а длина растёт на ту же величину — к моменту `local`
        // кольца упреждающих узлов заполнены настоящим материалом
        // [local, local + la). При la = 0 — прежний цикл дословно.
        let warm = preroll + self.la;
        let start_out = start - self.la;
        let mut scratch = vec![0f32; 1024 * CH];
        let mut done = 0i64;
        while done < warm {
            let n = (warm - done).min(1024) as usize;
            self.produce(PullKind::Warmup, start_out + done, &mut scratch[..n * CH], pull);
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
            // ВЫХОДНАЯ позиция чанка (что услышит слушатель) и ВХОДНАЯ
            // (что цепочка сейчас читает). Они разъезжаются ровно на
            // упреждение цепочки (Р.48, п.337); при la = 0 всё ниже
            // арифметически тождественно доpdc-версии.
            let chunk_local = local + done as i64;
            let chunk_in = chunk_local + self.la;
            // автоматизация: цели на середину чанка; rubato/one-pole сглаживают
            if !self.auto.is_empty() {
                // ДОМЕН СОБЫТИЯ (Р.37): цель чанка — по ВЫХОДНОМУ времени,
                // а не по позиции в материале. Ранний кейфрейм скорости
                // больше не сдвигает МОМЕНТ срабатывания поздних ключей —
                // ни своих, ни чужих (mix/gain/pan индексировались тем же
                // `s`). Правило середины чанка при этом стало ТОЧНЫМ:
                // подынтегральная функция линейна, а середина трапеции —
                // её точное среднее.
                // варп ведёт ВХОД цепочки: он и есть потребитель источника
                let tau = (chunk_in as f64 + n as f64 * 0.5) / self.clip_len as f64;
                let target_rate = eff_rate(self.rate, self.rate_points(), tau);
                if let Some(w) = &mut self.warp {
                    w.set_rate(target_rate);
                }
                // цели узлов — тем же правилом середины чанка, но по
                // позиции СОДЕРЖИМОГО, которое узел сейчас видит: кривая
                // остаётся приклеенной к материалу, а не к выходу
                let ahead = &self.ahead;
                for (i, (slot, node)) in self.nodes.iter_mut().enumerate() {
                    let s = (chunk_local + ahead[i]) as f64 + n as f64 * 0.5;
                    node.set_targets(&self.auto, *slot, s / self.clip_len as f64);
                }
            }
            // цель силы слоя на середину чанка — тем же правилом, что и
            // остальные параметры (Р.38 п.12: фейды это её кривая)
            let mix_ahead = self.mix_ahead;
            let mix_k = self.mix.as_ref().map(|m| {
                let tau =
                    ((chunk_local + mix_ahead) as f64 + n as f64 * 0.5) / self.clip_len as f64;
                self.auto.value(m.slot, "amount", m.base_amount, tau).clamp(0.0, 1.0) as f32
            });
            let sub = &mut out[done * CH..(done + n) * CH];
            // Гейт dry-выхода в OUTPUT-домене: за концом МАТЕРИАЛА (тело +
            // правая «ручка» перехода, п.138) сухой сигнал обязан быть нулём
            // (в хвосте дозванивает только реверб). Благодаря этому окно
            // ИСТОЧНИКА можно расширять с запасом на прайминг rubato, не
            // боясь утечки dry в хвост (дефект «тихий хвост»: sinc-ресемплер
            // видел нули за жёстко обрезанным окном и рано глушил конец).
            // Гейт — во ВХОДНОМ домене: за концом материала цепочка
            // получает детерминированную тишину ВПЕРЁД, и кольца
            // упреждающих узлов дожимают тело ровно к концу события.
            let gate_end = self.clip_len + self.tail_ext;
            if chunk_in >= gate_end {
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
                if chunk_in + (n as i64) > gate_end {
                    let cut = (gate_end - chunk_in).clamp(0, n as i64) as usize;
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
            let ahead = &self.ahead;
            let dry_ring = &mut self.dry_ring;
            for (i, (slot, node)) in self.nodes.iter_mut().enumerate() {
                if let Some(m) = &self.mix
                    && *slot >= m.from
                    && dry.is_none()
                {
                    let mut d = sub.to_vec();
                    // между снимком и маркером стоит упреждающий узел:
                    // сухую сторону надо задержать на съеденное им окно,
                    // иначе бленд смешал бы разные моменты материала
                    if let Some(r) = dry_ring.as_mut() {
                        r.run(&mut d);
                    }
                    dry = Some(d);
                }
                if matches!(node, AudioNode::LayerMix) {
                    if let (Some(k), Some(d)) = (mix_k, &dry) {
                        let ik = 1.0 - k;
                        for (v, dv) in sub.iter_mut().zip(d.iter()) {
                            *v = dv * ik + *v * k;
                        }
                    }
                } else {
                    node.process(sub, chunk_local + ahead[i]);
                }
            }
            done += n;
        }
    }
}
