//! Карта растяжения времени: «сколько материала съедено к моменту τ».
//!
//! ПЕРЕЕХАЛА СЮДА ИЗ kt-fx (Р.39) — **тело не трогать, на нём стоят
//! GOLDEN_AUDIO и GOLDEN_VIDEO**: перенос был дословным, те же операции f64
//! в том же порядке. Правила SIMD-раздела kt-fx/CLAUDE.md действуют здесь
//! слово в слово: ни FMA, ни переассоциации, ни f32, ни «causal» правок от
//! clippy внутри этого файла.
//!
//! Почему в модели, а не в kt-fx: карту обязан звать `clip_slice`
//! (`project.rs`) — правило куска клипа, — а kt-model зависеть от kt-fx не
//! может (направление зависимости обратное). Арифметика при этом чистая:
//! `f64`/`i64` над `[(f64, f64)]`, ни медиа, ни DSP. kt-fx и kt-engine
//! зовут её отсюда, публичные имена сохранены реэкспортом.

use crate::{FxInstance, ParamCurve};
use kt_time::Tick;

/// Профиль одного параметра в ДОМЕНЕ СОБЫТИЯ: (доля выходного времени
/// 0..1, значение). Строится из кривых клипа (`ParamCurve`, ключи в тиках
/// материала) один раз при сборке процессора — дальше аудио-поток работает
/// только с числами и ничего не аллоцирует.
///
/// Р.37: доля считается ЛИНЕЙНО от начала события, скорость её не сжимает
/// (требование пользователя: «t — временной промежуток, вырезанный текущим
/// событием, БЕЗ учёта замедления-ускорения»). Арифметика построения та же,
/// изменилась ТРАКТОВКА числа.
pub type ParamPoints = [(f64, f64)];

/// Значение профиля в позиции tau (линейная интерполяция, края зажаты).
/// `base` возвращается только у ПУСТОГО профиля: непустой перекрывает
/// параметр целиком (семантика v1 — кривая главнее ручки).
pub fn param_at(base: f64, points: &ParamPoints, tau: f64, ease: bool) -> f64 {
    let s = tau;
    if points.is_empty() {
        return base;
    }
    if s <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        if s <= w[1].0 {
            let span = (w[1].0 - w[0].0).max(1e-12);
            let mut a = (s - w[0].0) / span;
            if ease {
                a = a * a * (3.0 - 2.0 * a);
            }
            return w[0].1 * (1.0 - a) + w[1].1 * a;
        }
    }
    points[points.len() - 1].1
}

/// Значение rate в выходной позиции tau. Профиль rate ВСЕГДА кусочно
/// линеен: на этом стоит аналитическая форма карты варпа (S — парабола,
/// T — корень квадратного), и сглаживание (`ease`) на нём игнорируется —
/// интеграл от smoothstep был бы четвёртой степени. Р.37 сделал ease
/// принципиально считаемым, но выигрыша в нём нет.
pub fn rate_at(base: f64, points: &ParamPoints, tau: f64) -> f64 {
    param_at(base, points, tau, false)
}

// ---------------------------------------------------------------------------
// ДОМЕН СОБЫТИЯ (Р.37, решение пользователя): кривая rate — функция
// ВЫХОДНОГО прогресса tau (доли длины события), а не потреблённого
// материала. Ключ приклеен к материалу по-прежнему (обрезка/Split/реверс
// работают как с Р.33), но ось ЛИНЕЙНА: скорость её не сжимает.
//
// Зачем: домен по материалу делал ось кривой rate зависящей ОТ САМОЙ
// кривой rate (подвинул значение — точка уехала по экрану), заставлял
// ранний ключ скорости сдвигать момент срабатывания поздних ключей ДРУГИХ
// параметров и оставлял недостижимой правую часть графика (доехать можно
// было лишь до duration·avg_rate).
//
// Карта: rate(tau) кусочно-линеен; S(tau) = ∫₀^tau rate(u)du — доля
// материала, потреблённая к выходному прогрессу tau; T(s) = S⁻¹.
// S кусочно-КВАДРАТИЧНА (трапеция), T — корень квадратного уравнения.
// Ни ln, ни expm1: только + − × ÷ √, все корректно округлены по IEEE-754,
// то есть карта бит-переносима между платформами (golden всё ещё пришпилен
// к linux x86_64 синк-ресемплером, но уже не картой).
// Rate внутри карты зажат тем же клампом, что и Warp — иначе карта
// разошлась бы с фактическим потреблением.
// ---------------------------------------------------------------------------

pub const RATE_CLAMP: (f64, f64) = (0.25, 32.0);

pub fn eff_rate(base: f64, points: &ParamPoints, s: f64) -> f64 {
    rate_at(base, points, s).clamp(RATE_CLAMP.0, RATE_CLAMP.1)
}

/// Δτ, за которое сегмент со стартовым rate r0 и наклоном b = dr/dτ
/// съедает Δs материала — корень (b/2)·dt² + r0·dt − ds = 0.
///
/// Форма citardauq (умноженная на сопряжённое), а НЕ школьная
/// (−r0+√…)/b: она устойчива численно И не требует ветки b ≈ 0 —
/// при b = 0 сама сворачивается в ds/r0. Знаменатель ≥ r0 ≥ RATE_CLAMP.0,
/// нулём не бывает; дискриминант в точке решения равен rate(τ)², то есть
/// неотрицателен по построению (max(0) — только от арифметики).
fn seg_dt(ds: f64, r0: f64, b: f64) -> f64 {
    2.0 * ds / (r0 + (r0 * r0 + 2.0 * b * ds).max(0.0).sqrt())
}

/// Δs, потребляемая за Δτ на сегменте со стартовым rate r0 и наклоном
/// b = dr/dτ: трапеция ∫(r0 + b·u)du = r0·dt + b·dt²/2. Точно, без веток.
fn seg_ds(dt: f64, r0: f64, b: f64) -> f64 {
    r0 * dt + 0.5 * b * dt * dt
}

/// Сегменты (t0, t1, r0, r1) профиля по домену СОБЫТИЯ; последний — хвост
/// с константным rate до бесконечности (t1 = f64::INFINITY). Тело
/// доменно-нейтрально: оно просто идёт по массиву точек.
///
/// ВАЖНО: концы клампятся ДО интерполяции, а `eff_rate` клампит ПОСЛЕ —
/// значения расходятся, если ключ лежит вне RATE_CLAMP. Из UI такой ключ
/// не поставить (`set_param` клампит по спеке), но `migrate_legacy_slowed`
/// копирует легаси-значение как есть, поэтому клампим и на входе профиля
/// (`AutoProfiles::from_curves`).
fn segments(base: f64, points: &ParamPoints) -> Vec<(f64, f64, f64, f64)> {
    let cl = |r: f64| r.clamp(RATE_CLAMP.0, RATE_CLAMP.1);
    if points.is_empty() {
        return vec![(0.0, f64::INFINITY, cl(base), cl(base))];
    }
    let mut out = Vec::with_capacity(points.len() + 1);
    let first = cl(points[0].1);
    if points[0].0 > 0.0 {
        out.push((0.0, points[0].0, first, first));
    }
    for w in points.windows(2) {
        if w[1].0 > w[0].0 {
            out.push((w[0].0, w[1].0, cl(w[0].1), cl(w[1].1)));
        }
    }
    let last = cl(points[points.len() - 1].1);
    out.push((points[points.len() - 1].0, f64::INFINITY, last, last));
    // Профиль начинается в tau = 0 ВСЕГДА (Р.39). Ключ левее `source_in`
    // законен — его прячет обрезка или разрез, — и его нормированная
    // позиция ОТРИЦАТЕЛЬНА. Плоский сегмент от нуля выше добавляется лишь
    // при `points[0].0 > 0.0`, поэтому без подрезки `source_progress`
    // интегрировала бы ОТ ЭТОЙ ТОЧКИ: `S(0) != 0`, и клип читал бы
    // материал со сдвигом. Отрицательную точку создаёт КАЖДЫЙ TrimStart и
    // правая половина КАЖДОГО Split.
    //
    // Подрезка живёт здесь, а не в `source_progress`: тогда она и
    // `output_time_for_source` остаются взаимно обратными даром, а ветки
    // «за краями» (`segments(...)[0].2`, п.138) экстраполируют скоростью
    // В НУЛЕ, а не скоростью спрятанного ключа — что и значит «за
    // краями — крайние».
    //
    // При `points[0].0 >= 0.0` ни одна ветка ниже не исполняется, поэтому
    // прежний путь байт-в-байт цел (на нём стоят GOLDEN_*).
    while out.len() > 1 && out[0].1 <= 0.0 {
        out.remove(0);
    }
    if out[0].0 < 0.0 {
        let (t0, t1, r0, r1) = out[0];
        let b = if t1.is_finite() && t1 > t0 { (r1 - r0) / (t1 - t0) } else { 0.0 };
        out[0] = (0.0, t1, r0 + b * (0.0 - t0), r1);
    }
    out
}

/// T(s) = S⁻¹: выходной прогресс (доли длины события), при котором
/// потреблена source-доля s. Строго возрастает (rate ≥ RATE_CLAMP.0 > 0).
/// s < 0 — левая «ручка» перехода (п.138): линейная экстраполяция
/// скоростью первого сегмента.
pub fn output_time_for_source(base: f64, points: &ParamPoints, s: f64) -> f64 {
    if s < 0.0 {
        return s / segments(base, points)[0].2;
    }
    let (mut acc_t, mut acc_s) = (0.0, 0.0);
    for (t0, t1, r0, r1) in segments(base, points) {
        let (seg_t, b) = if t1.is_finite() && t1 > t0 {
            (t1 - t0, (r1 - r0) / (t1 - t0))
        } else {
            (f64::INFINITY, 0.0)
        };
        let seg_s = if seg_t.is_finite() { seg_ds(seg_t, r0, b) } else { f64::INFINITY };
        if acc_s + seg_s >= s {
            return acc_t + seg_dt(s - acc_s, r0, b);
        }
        acc_t += seg_t;
        acc_s += seg_s;
    }
    acc_t
}

/// S(t) = ∫₀ᵗ rate(u)du: source-доля, потреблённая к выходному прогрессу
/// t (t может быть > 1 — хвост за клипом; rate за последним kf константен;
/// t < 0 — левая «ручка» перехода, п.138: линейно скоростью первого
/// сегмента, «за краями — крайние»). Материал ручек НЕ сдвигает
/// нормировку автоматизации — знаменатель остаётся clip_len.
pub fn source_progress(base: f64, points: &ParamPoints, t: f64) -> f64 {
    if t < 0.0 {
        return t * segments(base, points)[0].2;
    }
    let mut acc_s = 0.0;
    for (t0, t1, r0, r1) in segments(base, points) {
        if t <= t0 {
            break;
        }
        let hi = if t1.is_finite() { t.min(t1) } else { t };
        let b = if t1.is_finite() && t1 > t0 { (r1 - r0) / (t1 - t0) } else { 0.0 };
        acc_s += seg_ds(hi - t0, r0, b);
        if hi >= t {
            break;
        }
    }
    acc_s
}

/// Потреблённый материал В ТИКАХ к выходному прогрессу `tau` (Р.37 п.2).
/// Единственный вход в карту для ВИДЕО: считать через `to_audio_frames`
/// нельзя — 1 кадр 48 кГц = 1.875 тика, и округление промахивается мимо
/// границы КАДРА; у аудио это невидимо, у видео — сразу не тот кадр.
pub fn warp_material_ticks(base: f64, points: &ParamPoints, span_ticks: i64, tau: f64) -> i64 {
    (span_ticks as f64 * source_progress(base, points, tau)).round() as i64
}

/// Средняя скорость = потреблённая доля источника за весь клип: S(1).
pub fn avg_rate(base: f64, points: &ParamPoints) -> f64 {
    source_progress(base, points, 1.0)
}

/// Кривые автоматизации КЛИПА, переведённые в SOURCE-домен (Р.33).
///
/// Ключи в модели живут в тиках материала; аудио-путь считает в долях
/// материала (`s`), потому что на них построена карта варпа. Перевод
/// делается ОДИН раз при сборке процессора, дальше аудио-поток видит
/// только числа: ни аллокаций, ни обращений к модели в render-потоке.
/// Кривая одного параметра слота: имя, сглаживание, точки (s, значение).
#[derive(Debug, Clone, PartialEq)]
struct SlotCurve {
    param: String,
    ease: bool,
    points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AutoProfiles {
    /// По слоту цепочки — его кривые.
    slots: Vec<Vec<SlotCurve>>,
}

impl AutoProfiles {
    /// Из кривых клипа (`ClipEvent::active_fx_auto`) в доли материала.
    /// `src_in_frames`/`clip_len_frames` — те же величины, которыми
    /// нормировалась автоматизация slowed&reverb до Р.33, поэтому
    /// профиль старых проектов численно совпадает с прежним.
    pub fn from_curves(
        auto: &[Vec<ParamCurve>],
        src_in_frames: i64,
        clip_len_frames: i64,
    ) -> AutoProfiles {
        let len = clip_len_frames.max(1) as f64;
        AutoProfiles {
            slots: auto
                .iter()
                .map(|curves| {
                    curves
                        .iter()
                        .filter(|c| !c.keys.is_empty())
                        .map(|c| {
                            // rate обязан остаться кусочно-линейным: на нём
                            // стоят аналитические формулы карты варпа
                            let ease = c.ease && c.param != "rate";
                            // значение клампится ЗДЕСЬ: `segments` клампит
                            // концы до интерполяции, а `eff_rate` — после,
                            // и на ключе вне RATE_CLAMP они разъезжались.
                            // Из UI такой ключ не поставить, но
                            // `migrate_legacy_slowed` копирует легаси-число
                            // как есть, а расхождение карты с DSP — это
                            // промахи префетча и тишина (п.13/t17).
                            let clamp = c.param == "rate";
                            let pts = c
                                .keys
                                .iter()
                                .map(|k| {
                                    let f = k.t.to_audio_frames() - src_in_frames;
                                    let v = if clamp {
                                        k.v.clamp(RATE_CLAMP.0, RATE_CLAMP.1)
                                    } else {
                                        k.v
                                    };
                                    (f as f64 / len, v)
                                })
                                .collect();
                            SlotCurve { param: c.param.clone(), ease, points: pts }
                        })
                        .collect()
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_empty())
    }

    /// Профиль параметра слота (пустой срез — кривой нет).
    pub fn points(&self, slot: usize, param: &str) -> &ParamPoints {
        self.slots
            .get(slot)
            .and_then(|s| s.iter().find(|c| c.param == param))
            .map_or(&[], |c| c.points.as_slice())
    }

    fn ease(&self, slot: usize, param: &str) -> bool {
        self.slots
            .get(slot)
            .and_then(|s| s.iter().find(|c| c.param == param))
            .is_some_and(|c| c.ease)
    }

    /// Значение параметра слота в позиции материала s.
    pub fn value(&self, slot: usize, param: &str, base: f64, s: f64) -> f64 {
        param_at(base, self.points(slot, param), s, self.ease(slot, param))
    }

    /// Индекс слота с варпом и его профиль скорости — на нём стоит вся
    /// аналитика источника (сколько материала съест клип).
    fn warp_slot(fx: &[FxInstance]) -> Option<usize> {
        fx.iter().position(|f| matches!(f, FxInstance::SlowedReverb { .. }))
    }

    /// Профиль скорости цепочки (пустой — варпа нет или он без кривой).
    pub fn rate_points(&self, fx: &[FxInstance]) -> &ParamPoints {
        Self::warp_slot(fx).map_or(&[], |i| self.points(i, "rate"))
    }

    /// Профиль mix цепочки — нужен, чтобы знать, звенит ли реверб.
    pub fn mix_points(&self, fx: &[FxInstance]) -> &ParamPoints {
        Self::warp_slot(fx).map_or(&[], |i| self.points(i, "mix"))
    }
}

/// Скорость воспроизведения цепочки (1.0 = без варпа).
pub fn chain_rate(fx: &[FxInstance]) -> f64 {
    fx.iter()
        .find_map(|f| match f {
            FxInstance::SlowedReverb { rate, .. } => Some(rate.clamp(0.25, 32.0)),
            _ => None,
        })
        .unwrap_or(1.0)
}

/// Средняя скорость цепочки с учётом автоматизации: определяет, сколько
/// источника съедает клип (для source-окна и длительности).
pub fn chain_avg_rate(fx: &[FxInstance], auto: &AutoProfiles) -> f64 {
    fx.iter()
        .find_map(|f| match f {
            FxInstance::SlowedReverb { rate, .. } => {
                Some(avg_rate(*rate, auto.rate_points(fx)))
            }
            _ => None,
        })
        .unwrap_or(1.0)
}

#[cfg(test)]
mod automation_tests {
    use super::*;

    /// Точка профиля скорости: (доля материала, rate).
    fn kf(t: f64, rate: f64) -> (f64, f64) {
        (t, rate)
    }

    #[test]
    fn rate_interp_clamps_ends_and_is_linear() {
        let a = vec![kf(0.2, 1.0), kf(0.4, 0.5)];
        assert_eq!(rate_at(9.0, &a, 0.0), 1.0, "до первой точки — её значение (base игнорируется)");
        assert_eq!(rate_at(9.0, &a, 0.3), 0.75);
        assert_eq!(rate_at(9.0, &a, 1.0), 0.5);
    }

    /// Source-карта: round-trip T∘S = id, монотонность, сверка с
    /// независимым численным интегратором (профиль song1).
    #[test]
    fn source_map_roundtrip_and_matches_numeric() {
        let a = vec![
            kf(0.21616990640748734, 1.0),
            kf(0.2935781137508999, 0.35),
            kf(0.3822750179985601, 0.62),
            kf(0.6397019809160477, 0.6),
            kf(0.7557084786003034, 1.0),
        ];
        assert_eq!(source_progress(1.0, &a, 0.0), 0.0);
        let mut prev = 0.0;
        for i in 0..=100 {
            let t = i as f64 / 100.0;
            let s = source_progress(1.0, &a, t);
            assert!(s >= prev, "S(t) обязана быть неубывающей");
            prev = s;
            // round-trip
            let back = output_time_for_source(1.0, &a, s);
            assert!((back - t).abs() < 1e-9, "T(S({t})) = {back}");
        }
        // независимый численный интегратор: S(τ) трапециями ПО rate
        // (Р.37: интегрируем скорость по выходному времени, а не 1/rate
        // по материалу)
        let numeric_s_of_t = |t_target: f64| -> f64 {
            let n = 200_000;
            let mut acc = 0.0;
            let mut prev = rate_at(1.0, &a, 0.0).clamp(0.25, 32.0);
            for i in 1..=n {
                let t = t_target * i as f64 / n as f64;
                let r = rate_at(1.0, &a, t).clamp(0.25, 32.0);
                acc += (t_target / n as f64) * (prev + r) / 2.0;
                prev = r;
            }
            acc
        };
        for t in [0.1f64, 0.3, 0.5, 0.7] {
            let analytic = source_progress(1.0, &a, t);
            let numeric = numeric_s_of_t(t);
            assert!(
                (analytic - numeric).abs() < 1e-6,
                "S({t}): аналитика {analytic} vs численно {numeric}"
            );
        }
        // S — ПАРАБОЛА, а не экспонента: на линейном разгоне 1→2 за всё
        // событие интеграл равен ровно средней скорости. Без ln это
        // считается точно, поэтому и сверяем точно.
        let ramp = vec![kf(0.0, 1.0), kf(1.0, 2.0)];
        assert!((source_progress(1.0, &ramp, 1.0) - 1.5).abs() < 1e-15);
        assert!((source_progress(1.0, &ramp, 0.5) - 0.625).abs() < 1e-15);
        assert!((output_time_for_source(1.0, &ramp, 1.5) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn no_automation_map_is_linear() {
        assert_eq!(source_progress(0.5, &[], 0.5), 0.25);
        assert_eq!(avg_rate(0.82, &[]), 0.82);
        assert!((output_time_for_source(0.5, &[], 0.25) - 0.5).abs() < 1e-12);
    }

    /// Якорь к ВЫХОДНОМУ ВРЕМЕНИ (Р.37, зеркало прежнего теста): ранний
    /// бамп скорости меняет МАТЕРИАЛ, играющий в поздний момент, но не
    /// момент, в который срабатывают поздние ключи.
    #[test]
    fn early_keyframe_shifts_material_not_time() {
        let a = vec![kf(0.4, 1.0), kf(0.5, 0.5), kf(0.7, 0.5), kf(0.75, 1.0)];
        let mut b = vec![kf(0.1, 1.0), kf(0.18, 2.0), kf(0.26, 2.0), kf(0.34, 1.0)];
        b.extend(a.iter().cloned());
        for t in [0.45f64, 0.55, 0.65, 0.72, 0.90] {
            assert_eq!(
                rate_at(1.0, &a, t),
                rate_at(1.0, &b, t),
                "rate в момент t={t} не должен зависеть от раннего бампа"
            );
        }
        // а МАТЕРИАЛ, доигранный к t=0.5, при бампе больше
        let sa = source_progress(1.0, &a, 0.5);
        let sb = source_progress(1.0, &b, 0.5);
        assert!(sb > sa, "ускорение раньше = материала съедено больше: {sb} vs {sa}");
    }
}

#[cfg(test)]
mod zero_start_tests {
    use super::*;

    /// Тест 269 (Р.39, Д0): профиль ВСЕГДА начинается в нуле.
    ///
    /// Ключ левее `source_in` законен — его прячет обрезка. Его точка
    /// отрицательна, и без подрезки `source_progress` интегрировала бы
    /// от неё: клип читал бы материал со сдвигом. Дефект живой и до Р.39:
    /// отрицательную точку создаёт КАЖДЫЙ TrimStart и правая половина
    /// КАЖДОГО Split.
    #[test]
    fn t269_profile_starts_at_zero_with_a_hidden_key() {
        // рампа 1.0 → 2.0, ключ на -1 «спрятан» слева от начала куска
        let hidden = [(-1.0, 1.0), (1.0, 2.0)];

        assert_eq!(
            source_progress(1.0, &hidden, 0.0),
            0.0,
            "в начале события материал ещё не съеден: S(0) обязан быть РОВНО ноль"
        );
        assert_eq!(
            warp_material_ticks(1.0, &hidden, 90_000, 0.0),
            0,
            "и в тиках тоже ноль — иначе кусок начинается не с того кадра"
        );
        // на [0,1] скорость идёт 1.5 → 2.0, интеграл трапеции = 1.75
        let s1 = source_progress(1.0, &hidden, 1.0);
        assert!(
            (s1 - 1.75).abs() < 1e-12,
            "S(1) обязана быть аналитической трапецией 1.75, а не {s1}"
        );

        // T = S⁻¹ осталась обратной: подрезка сделана в `segments`, поэтому
        // обе функции видят один и тот же профиль
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let back = output_time_for_source(1.0, &hidden, source_progress(1.0, &hidden, t));
            assert!((back - t).abs() < 1e-9, "T(S({t})) = {back}");
        }

        // профиль с НЕОТРИЦАТЕЛЬНОЙ первой точкой не тронут ни на бит:
        // на этом пути стоят GOLDEN_AUDIO и GOLDEN_VIDEO
        let plain = [(0.3, 1.0), (0.7, 0.5)];
        assert_eq!(source_progress(1.0, &plain, 0.0), 0.0);
        let s = source_progress(1.0, &plain, 1.0);
        assert!((s - 0.75).abs() < 1e-12, "0.3·1 + 0.4·0.75 + 0.3·0.5 = 0.75, а не {s}");
    }
}

// ---------------------------------------------------------------------------
// Карта КОНКРЕТНОГО клипа (Р.39). Раньше жила в kt-engine тремя почти
// одинаковыми структурами (`VideoWarp`, `ClipWarp` и неявная внутри
// `clip_material_pos`); теперь конструктор один, а движок берёт его отсюда.
// ---------------------------------------------------------------------------

/// Растяжение времени клипа: база, профиль скорости и фактически
/// потреблённый материал.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipWarp {
    base: f64,
    prof: Vec<(f64, f64)>,
    /// Материал в тиках, который событие реально проходит: S(1)·span.
    used: i64,
    /// Режим «смешивание» (Р.37 С5): читается исполнителем слоя.
    blend: bool,
}

impl ClipWarp {
    pub fn base(&self) -> f64 {
        self.base
    }
    pub fn prof(&self) -> &ParamPoints {
        &self.prof
    }
    pub fn used(&self) -> i64 {
        self.used
    }
    pub fn blend(&self) -> bool {
        self.blend
    }

    /// Сколько материала съедено к моменту `tl` тиков от начала события.
    /// Реверс — «ВАРП, ПОТОМ ЗЕРКАЛО» (Р.37 п.247): зеркало ДО варпа гнало
    /// бы кривую скорости задом наперёд и рассинхронило A/V.
    pub fn material_off(&self, duration: Tick, reversed: bool, tl: i64) -> i64 {
        let dur = duration.0.max(1);
        let consumed = warp_material_ticks(self.base, &self.prof, dur, tl as f64 / dur as f64);
        if reversed { (self.used - consumed).max(0) } else { consumed }
    }
}

/// Карта по ЦЕПОЧКЕ И КРИВЫМ. `None` — варпа нет, отображение линейно.
///
/// Вход обязан быть АКТИВНЫМ (`active_fx`/`active_fx_auto`): выключенные
/// слоты и мастер-выключатель уже отброшены. `AutoProfiles::warp_slot`
/// ищет слот ПО ВИДУ, поэтому на неотфильтрованных кривых профиль скорости
/// считался бы с чужого слота.
pub fn clip_warp_of(
    fx: &[FxInstance],
    auto: &[Vec<ParamCurve>],
    source_in: Tick,
    duration: Tick,
) -> Option<ClipWarp> {
    let span = duration.0.max(1);
    let (base, blend) = fx.iter().find_map(|f| match f {
        FxInstance::SlowedReverb { rate, frames, .. } => {
            Some((*rate, *frames == crate::FrameMode::Blend))
        }
        _ => None,
    })?;
    // Профиль строится ТОЙ ЖЕ функцией, что у аудио: домен один на оба
    // вида, иначе картинка и звук разъехались бы на кривой скорости.
    // Позиция ключа нормируется в АУДИО-КАДРАХ (как у аудио) — доля
    // безразмерна, и обе стороны получают одно число. В ТИКАХ считается
    // только ВЫХОД карты (`warp_material_ticks`): там сетка 48 кГц
    // промахнулась бы мимо границы кадра.
    let prof = AutoProfiles::from_curves(
        auto,
        source_in.to_audio_frames(),
        duration.to_audio_frames().max(1),
    )
    .rate_points(fx)
    .to_vec();
    let used = warp_material_ticks(base, &prof, span, 1.0);
    Some(ClipWarp { base, prof, used, blend })
}

/// Карта клипа из его активной цепочки.
pub fn clip_warp(c: &crate::ClipEvent) -> Option<ClipWarp> {
    // дешёвый отсев ДО аллокаций `active_fx`/`active_fx_auto`:
    // `effective_clips` зовёт это на каждый кусок каждого клипа
    if c.fx_off || !c.fx.iter().enumerate().any(|(i, f)| {
        f.warps_time() && !c.fx_bypass.get(i).copied().unwrap_or(false)
    }) {
        return None;
    }
    clip_warp_of(&c.active_fx(), &c.active_fx_auto(), c.source_in, c.duration)
}
