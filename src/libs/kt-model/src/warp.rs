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

pub const RATE_CLAMP: (f64, f64) = (-32.0, 32.0);

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

// ---------------------------------------------------------------------------
// ЗНАКОВАЯ СКОРОСТЬ (Р.57, решение владельца): rate живёт в −32…32, ноль
// разрешён. Семантика — velocity-огибающая Vegas: ПОЗИЦИЯ в материале
// P(τ) = ∫₀^τ rate, минус идёт по материалу НАЗАД внутри события, ноль —
// стоп-кадр.
//
// Из этого следуют ДВЕ ОСИ, и путать их нельзя:
//
// * **P(τ) = ∫rate** — знаковая ПОЗИЦИЯ (адрес чтения). Немонотонна, и
//   обращать её нечем: у точки материала может быть несколько моментов
//   выхода. Это и есть `source_progress` — её тело НЕ ТРОНУТО, потому что
//   `seg_ds` знаконезависима.
// * **A(τ) = ∫|rate|** — накопленное ПОТРЕБЛЕНИЕ. Монотонна по построению,
//   и обращается (`output_time_for_consumption`). По ней живёт курсор
//   ресемплера: рубато ест поток со скоростью |rate| и знака не знает.
//
// На кривых, целиком лежащих в 0.25…32, A ≡ P ≡ прежняя S БИТ-В-БИТ:
// `fabs` положительного f64 — тождество, а разбиение сегмента в нуле
// скорости включается строго при `r0*r1 < 0`, чего на таких кривых не
// бывает. На этом стоят GOLDEN_AUDIO и GOLDEN_VIDEO (сторож — t603d).
// ---------------------------------------------------------------------------

/// Пол скорости для АУДИО: |rate| ниже — это удержание (hold), а не очень
/// медленное чтение. Одна константа на карту и на процессор: разойдись
/// они, потребление разъехалось бы с тем, что реально съел ресемплер.
///
/// Решение владельца: «тянуть до 0.01, тишина только в нуле».
pub const RATE_AUDIO_FLOOR: f64 = 0.01;

/// Модуль скорости с полом: ниже пола — ноль (удержание).
fn mag_floor(r: f64) -> f64 {
    let a = r.abs();
    if a < RATE_AUDIO_FLOOR { 0.0 } else { a }
}

/// Сегменты профиля, разбитые в НУЛЕ СКОРОСТИ, с |rate| на концах.
///
/// Разбиение включается строго при `r0*r1 < 0.0` — не `<=`. Сегмент,
/// КОНЧАЮЩИЙСЯ нулём, остаётся линейным по модулю, и лишнее слагаемое
/// `acc += 0.0` сдвинуло бы округление там, где ничего не меняется.
fn abs_segments(base: f64, points: &ParamPoints) -> Vec<(f64, f64, f64, f64)> {
    let segs = segments(base, points);
    let mut out = Vec::with_capacity(segs.len() + 2);
    for (t0, t1, r0, r1) in segs {
        if r0 * r1 < 0.0 && t1.is_finite() && t1 > t0 {
            let b = (r1 - r0) / (t1 - t0);
            let tz = t0 - r0 / b;
            out.push((t0, tz, mag_floor(r0), 0.0));
            out.push((tz, t1, 0.0, mag_floor(r1)));
        } else {
            out.push((t0, t1, mag_floor(r0), mag_floor(r1)));
        }
    }
    out
}

/// A(t) = ∫₀ᵗ |rate(u)|du — сколько материала цепочка СЪЕЛА к выходному
/// прогрессу t. Монотонно не убывает; растёт строго везде, кроме плато
/// нулевой скорости.
///
/// Тело повторяет `source_progress` шаг в шаг (тот же цикл, тот же
/// `seg_ds`, тот же порядок сложения) — иначе бит-тождество на
/// положительных кривых не удержать.
pub fn consumed_abs(base: f64, points: &ParamPoints, t: f64) -> f64 {
    if t < 0.0 {
        return t * mag_floor(segments(base, points)[0].2);
    }
    let mut acc_s = 0.0;
    for (t0, t1, r0, r1) in abs_segments(base, points) {
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

/// A⁻¹: выходной прогресс, при котором СЪЕДЕНО `a` материала.
///
/// Правило плато: на нулевой скорости потребление стоит, и ответом
/// берётся ЛЕВЫЙ край плато — первый момент, когда счётчик достиг `a`.
/// Ответ `+∞` значит «столько не съедается никогда» (хвост профиля —
/// нулевая скорость).
pub fn output_time_for_consumption(base: f64, points: &ParamPoints, a: f64) -> f64 {
    if a < 0.0 {
        let r = mag_floor(segments(base, points)[0].2);
        return if r == 0.0 { 0.0 } else { a / r };
    }
    let (mut acc_t, mut acc_a) = (0.0, 0.0);
    for (t0, t1, r0, r1) in abs_segments(base, points) {
        let (seg_t, b) = if t1.is_finite() && t1 > t0 {
            (t1 - t0, (r1 - r0) / (t1 - t0))
        } else {
            (f64::INFINITY, 0.0)
        };
        let seg_a = if seg_t.is_finite() {
            seg_ds(seg_t, r0, b)
        } else if r0 == 0.0 {
            0.0
        } else {
            f64::INFINITY
        };
        if acc_a + seg_a >= a {
            let need = a - acc_a;
            // ровно на границе (и на плато) — ЛЕВЫЙ край: `seg_dt` при
            // нулевом `need` и нулевой скорости дал бы 0/0
            return if need <= 0.0 { acc_t } else { acc_t + seg_dt(need, r0, b) };
        }
        acc_t += seg_t;
        acc_a += seg_a;
    }
    // хвост нулевой скорости: `acc_t` уже бесконечен, если хвост был
    // ненулевым; иначе столько материала не съедается никогда
    if acc_a >= a { acc_t } else { f64::INFINITY }
}

/// Пролёт ПОЗИЦИИ на [t0, t1]: (минимум P, максимум P).
///
/// Для монотонного профиля возвращает РОВНО `(P(t0), P(t1))` — на этом
/// стоит бит-тождество окон префетча и оси зеркала.
pub fn span_over(base: f64, points: &ParamPoints, t0: f64, t1: f64) -> (f64, f64) {
    let (a, b) = (source_progress(base, points, t0), source_progress(base, points, t1));
    let (mut lo, mut hi) = (a.min(b), a.max(b));
    // экстремумы P — только в нулях скорости: между ними знак постоянен
    for (s0, s1, r0, r1) in segments(base, points) {
        if !(s1.is_finite() && s1 > s0) || r0 * r1 >= 0.0 {
            continue;
        }
        let tz = s0 - r0 / ((r1 - r0) / (s1 - s0));
        if tz > t0.min(t1) && tz < t0.max(t1) {
            let v = source_progress(base, points, tz);
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    (lo, hi)
}

/// Пролёт позиции за ВСЁ событие, в тиках. `span_ticks` — длина события.
pub fn warp_span_ticks(base: f64, points: &ParamPoints, span_ticks: i64) -> (i64, i64) {
    let (lo, hi) = span_over(base, points, 0.0, 1.0);
    ((span_ticks as f64 * lo).round() as i64, (span_ticks as f64 * hi).round() as i64)
}

/// Пределы МОДУЛЯ скорости по всему профилю (для ресемплера: он должен
/// знать, какой разброс отношений его ждёт, ещё до первого чанка).
/// Ноль поднимается до пола: на удержании ресемплер не работает вовсе.
pub fn rate_reach(base: f64, points: &ParamPoints) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
    // по РАЗБИТЫМ сегментам: кривая, меняющая знак, проходит через ноль
    // ВНУТРИ сегмента, и по одним концам минимум вышел бы завышенным — а
    // ресемплер потом отверг бы смену отношения (Р.57).
    for (_, _, r0, r1) in abs_segments(base, points) {
        for r in [r0.abs(), r1.abs()] {
            lo = lo.min(r);
            hi = hi.max(r);
        }
    }
    if !lo.is_finite() {
        lo = base.abs();
        hi = base.abs();
    }
    (lo.max(RATE_AUDIO_FLOOR), hi.max(RATE_AUDIO_FLOOR))
}

/// Ходит ли профиль НАЗАД по материалу хоть где-нибудь.
///
/// Это и есть предикат «карта немонотонна»: он гейтит всё, что построено
/// на чтении вперёд (раны адреса, окна префетча, блочная схема суб-микса).
pub fn rate_signed(base: f64, points: &ParamPoints) -> bool {
    segments(base, points).iter().any(|&(_, _, r0, r1)| r0 < 0.0 || r1 < 0.0)
}

/// Узел смены направления: момент, накопленное потребление, позиция и
/// направление ПОСЛЕ узла (+1 вперёд, −1 назад, 0 удержание).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignKnot {
    pub tau: f64,
    pub a: f64,
    pub p: f64,
    pub dir_after: i8,
}

/// Узлы смены направления профиля на [0, ∞). Первый всегда в нуле.
/// Соседние узлы с одинаковым направлением схлопнуты: раны должны быть
/// как можно длиннее — по ним аудио копирует память кусками.
pub fn sign_knots(base: f64, points: &ParamPoints) -> Vec<SignKnot> {
    let dir_of = |r: f64| -> i8 {
        if r.abs() < RATE_AUDIO_FLOOR {
            0
        } else if r < 0.0 {
            -1
        } else {
            1
        }
    };
    let mut taus: Vec<f64> = vec![0.0];
    for (t0, t1, r0, r1) in abs_segments(base, points) {
        let _ = (r0, r1);
        if t0 > 0.0 {
            taus.push(t0);
        }
        if t1.is_finite() && t1 > t0 {
            taus.push(t1);
        }
    }
    taus.retain(|t| t.is_finite());
    taus.sort_by(|a, b| a.partial_cmp(b).unwrap());
    taus.dedup();
    let mut out: Vec<SignKnot> = Vec::with_capacity(taus.len());
    for tau in taus {
        // направление СРАЗУ ПОСЛЕ узла: скорость в середине следующего
        // микро-шага, а не в самой точке (в точке она может быть нулём
        // ровно на смене знака)
        let dir = dir_after(base, points, tau);
        let k = SignKnot {
            tau,
            a: consumed_abs(base, points, tau),
            p: source_progress(base, points, tau),
            dir_after: dir,
        };
        match out.last() {
            Some(prev) if prev.dir_after == dir => {}
            _ => out.push(k),
        }
    }
    if out.is_empty() {
        out.push(SignKnot { tau: 0.0, a: 0.0, p: 0.0, dir_after: dir_of(base) });
    }
    out
}

/// Направление сразу ПОСЛЕ момента `tau` (ноль в самой точке смены знака
/// направлением не считается).
fn dir_after(base: f64, points: &ParamPoints, tau: f64) -> i8 {
    for (t0, t1, r0, r1) in abs_segments(base, points) {
        if tau >= t1 {
            continue;
        }
        // на этом сегменте модуль линеен; знак берём у ИСХОДНОГО профиля
        let mid = if t1.is_finite() { (tau.max(t0) + t1) * 0.5 } else { tau.max(t0) + 0.5 };
        let r = eff_rate(base, points, mid);
        let _ = (r0, r1);
        return if r.abs() < RATE_AUDIO_FLOOR {
            0
        } else if r < 0.0 {
            -1
        } else {
            1
        };
    }
    0
}

/// Потреблённый материал В ТИКАХ к выходному прогрессу `tau` (Р.37 п.2).
/// Единственный вход в карту для ВИДЕО: считать через `to_audio_frames`
/// нельзя — 1 кадр 48 кГц = 1.875 тика, и округление промахивается мимо
/// границы КАДРА; у аудио это невидимо, у видео — сразу не тот кадр.
pub fn warp_material_ticks(base: f64, points: &ParamPoints, span_ticks: i64, tau: f64) -> i64 {
    (span_ticks as f64 * source_progress(base, points, tau)).round() as i64
}

/// ∫₀ᵗ p(u)du для ПРОИЗВОЛЬНОГО параметра БЕЗ клампа значений (Р.47,
/// п.324): фаза LFO аудио-модуляций = интеграл кривой rate_hz по позиции
/// клипа. `source_progress` для этого не годится — она пришивает значения
/// к RATE_CLAMP (0.25..32), а частота LFO живёт в своих пределах спеки.
/// Кусочно-линейный профиль, трапеция точная; за последней точкой —
/// константа; t < 0 — линейно значением первого сегмента.
pub fn param_integral(base: f64, points: &ParamPoints, t: f64) -> f64 {
    if points.is_empty() {
        return base * t;
    }
    let first = points[0].1;
    if t <= points[0].0 {
        // до первого ключа профиль плоский его значением (как segments)
        return first * t;
    }
    let mut acc = first * points[0].0;
    for w in points.windows(2) {
        let (t0, v0) = w[0];
        let (t1, v1) = w[1];
        if t1 <= t0 {
            continue;
        }
        let hi = t.min(t1);
        if hi > t0 {
            let v_hi = v0 + (v1 - v0) * (hi - t0) / (t1 - t0);
            acc += (v0 + v_hi) * 0.5 * (hi - t0);
        }
        if t <= t1 {
            return acc;
        }
    }
    let (tl, vl) = points[points.len() - 1];
    acc + vl * (t - tl)
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
        Self::from_curves_rev(auto, src_in_frames, clip_len_frames, false)
    }

    /// То же с учётом РЕВЕРСА (Р.39).
    ///
    /// Нормировка `(k.t − src_in)/len` равна выходной доле ключа только
    /// потому, что UI пишет ключ в `src_in + p·duration`. У зеркального
    /// клипа UI пишет его ЗЕРКАЛЬНО (`clip_material_at` учитывает реверс),
    /// и равенство ломается: ключ, поставленный в начале события, DSP
    /// отрабатывал в конце. Дожило до Р.39 потому, что ни один тест во
    /// всём репозитории не сочетал `reversed` с непустым `fx_auto`.
    pub fn from_curves_rev(
        auto: &[Vec<ParamCurve>],
        src_in_frames: i64,
        clip_len_frames: i64,
        reversed: bool,
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
                            let pts: Vec<(f64, f64)> = c
                                .keys
                                .iter()
                                .map(|k| {
                                    let f = k.t.to_audio_frames() - src_in_frames;
                                    let v = if clamp {
                                        k.v.clamp(RATE_CLAMP.0, RATE_CLAMP.1)
                                    } else {
                                        k.v
                                    };
                                    let u = f as f64 / len;
                                    (if reversed { 1.0 - u } else { u }, v)
                                })
                                .collect();
                            // зеркало переворачивает порядок точек, а
                            // карта идёт по массиву слева направо
                            let pts: Vec<(f64, f64)> = if reversed {
                                let mut v = pts;
                                v.reverse();
                                v
                            } else {
                                pts
                            };
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

    /// Сглаживание кривой параметра: узлам, читающим значение ПО
    /// ПОЗИЦИИ (LFO, питч), нужен тот же флаг, что и `value`.
    pub fn ease_of(&self, slot: usize, param: &str) -> bool {
        self.slots
            .get(slot)
            .and_then(|s| s.iter().find(|c| c.param == param))
            .is_some_and(|c| c.ease)
    }

    /// Значение параметра слота в позиции материала s.
    pub fn value(&self, slot: usize, param: &str, base: f64, s: f64) -> f64 {
        param_at(base, self.points(slot, param), s, self.ease_of(slot, param))
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
            FxInstance::SlowedReverb { rate, .. } => Some(rate.clamp(RATE_CLAMP.0, RATE_CLAMP.1)),
            _ => None,
        })
        .unwrap_or(1.0)
}

/// Средняя скорость цепочки с учётом автоматизации: определяет, сколько
/// источника съедает клип (для source-окна и длительности).
pub fn chain_avg_rate(fx: &[FxInstance], auto: &AutoProfiles) -> f64 {
    fx.iter()
        .find_map(|f| match f {
            FxInstance::SlowedReverb { rate, .. } => Some(avg_rate(*rate, auto.rate_points(fx))),
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
    /// Материал в тиках, который событие реально СЪЕДАЕТ: A(1)·span.
    /// Со знаковой скоростью (Р.57) это уже не «докуда дошли»: кривая
    /// может уйти назад и вернуться, съев вдвое больше, чем прошла.
    used: i64,
    /// ПРОЛЁТ позиции (мин, макс) в тиках от `source_in` (Р.57). Это и
    /// есть окно материала события: по нему считаются прогретость,
    /// префетч и ось зеркала. На положительных кривых — (0, used).
    span_min: i64,
    span_max: i64,
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
    /// Окно материала (мин, макс) в тиках от `source_in` (Р.57).
    pub fn span(&self) -> (i64, i64) {
        (self.span_min, self.span_max)
    }
    /// Длина окна материала — ею меряют ступень филмстрипа и прогретость.
    pub fn span_len(&self) -> i64 {
        (self.span_max - self.span_min).max(1)
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
        // Ось зеркала — ПРОЛЁТ позиции, а не «сколько съедено» (Р.57).
        // На положительных кривых пролёт равен (0, used), и выражение
        // сворачивается в прежнее `(used − consumed).max(0)` — то же
        // целое, те же биты.
        if reversed {
            (self.span_min + self.span_max - consumed).max(self.span_min)
        } else {
            consumed
        }
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
    reversed: bool,
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
    let prof = AutoProfiles::from_curves_rev(
        auto,
        source_in.to_audio_frames(),
        duration.to_audio_frames().max(1),
        reversed,
    )
    .rate_points(fx)
    .to_vec();
    let used = (span as f64 * consumed_abs(base, &prof, 1.0)).round() as i64;
    let (span_min, span_max) = warp_span_ticks(base, &prof, span);
    Some(ClipWarp { base, prof, used, span_min, span_max, blend })
}

/// Растягивает ли цепочка ВРЕМЯ на самом деле.
///
/// Наличие слота и растяжение — разные вещи: `SlowedReverb` со скоростью 1×
/// даёт хвост, но материал потребляет 1:1. `warps_time()` отвечает на
/// вопрос «умеет ли этот ВИД», и для вето на резку куска этого достаточно;
/// там, где спрашивают «поедет ли время» (вложение, Р.50), нужен ответ про
/// ЗНАЧЕНИЯ.
pub fn chain_warps_time(fx: &[FxInstance], auto: &[Vec<ParamCurve>]) -> bool {
    fx.iter().enumerate().any(|(i, f)| match f {
        FxInstance::SlowedReverb { rate, .. } => {
            *rate != 1.0
                || auto.get(i).is_some_and(|cs| {
                    cs.iter().any(|c| c.param == "rate" && c.keys.iter().any(|k| k.v != 1.0))
                })
        }
        _ => false,
    })
}

/// Карта клипа из его активной цепочки.
pub fn clip_warp(c: &crate::ClipEvent) -> Option<ClipWarp> {
    // дешёвый отсев ДО аллокаций `active_fx`/`active_fx_auto`:
    // `effective_clips` зовёт это на каждый кусок каждого клипа
    if c.fx_off
        || !c
            .fx
            .iter()
            .enumerate()
            .any(|(i, f)| f.warps_time() && !c.fx_bypass.get(i).copied().unwrap_or(false))
    {
        return None;
    }
    clip_warp_of(&c.active_fx(), &c.active_fx_auto(), c.source_in, c.duration, c.reversed)
}

#[cfg(test)]
mod reversed_tests {
    use super::*;
    use crate::{AutoKey, ParamCurve};

    /// Тест 271 (Р.39, Д2): у зеркального клипа кривая не переворачивается.
    ///
    /// Нормировка `(k.t − src_in)/len` равна выходной доле ключа только
    /// потому, что UI пишет ключ в `src_in + p·duration`. У реверснутого
    /// клипа UI пишет зеркально — и без зеркала ЗДЕСЬ ключ, поставленный
    /// в начале события, отрабатывался в конце.
    ///
    /// Ни один тест в репозитории не сочетал `reversed` с непустым
    /// `fx_auto` — поэтому дефект и дожил.
    #[test]
    fn t271_reversed_curve_is_not_played_backwards() {
        // клип: source_in 0, длина 1 с; ключ на 0.25 материала = 0.75
        // выходной доли у зеркального клипа
        let len = kt_time::Tick(90_000).to_audio_frames();
        let curves = vec![vec![ParamCurve {
            param: "db".into(),
            ease: false,
            keys: vec![
                AutoKey { t: kt_time::Tick(0), v: 0.0 },
                AutoKey { t: kt_time::Tick(90_000 / 4), v: 10.0 },
                AutoKey { t: kt_time::Tick(90_000), v: 0.0 },
            ],
            source: Default::default(),
        }]];

        let fwd = AutoProfiles::from_curves_rev(&curves, 0, len, false);
        let rev = AutoProfiles::from_curves_rev(&curves, 0, len, true);

        // прямой клип: пик на выходной доле 0.25
        assert!((fwd.value(0, "db", 0.0, 0.25) - 10.0).abs() < 1e-9);
        // зеркальный: ТОТ ЖЕ материал проигрывается в 0.75 выхода
        assert!(
            (rev.value(0, "db", 0.0, 0.75) - 10.0).abs() < 1e-9,
            "пик обязан прийтись на зеркальную долю: {}",
            rev.value(0, "db", 0.0, 0.75)
        );
        assert!(
            rev.value(0, "db", 0.0, 0.25) < 5.0,
            "и НЕ на прямую: {}",
            rev.value(0, "db", 0.0, 0.25)
        );
        // прямой путь не тронут: from_curves == from_curves_rev(false)
        assert_eq!(AutoProfiles::from_curves(&curves, 0, len), fwd);
    }
}

#[cfg(test)]
mod signed_tests {
    use super::*;

    /// ЗАМОРОЖЕННАЯ копия карты ДО Р.57 — оракул бит-тождества.
    ///
    /// Не «похожая функция»: дословный старый текст с прежним клампом.
    /// Любая лишняя операция на положительном пути (abs, ветка, второе
    /// сложение) немедленно разойдётся в последнем бите — и t603d покажет
    /// это раньше, чем покраснеет GOLDEN.
    mod frozen {
        pub fn segments(base: f64, points: &[(f64, f64)]) -> Vec<(f64, f64, f64, f64)> {
            let cl = |r: f64| r.clamp(0.25, 32.0);
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

        fn seg_ds(dt: f64, r0: f64, b: f64) -> f64 {
            r0 * dt + 0.5 * b * dt * dt
        }

        pub fn source_progress(base: f64, points: &[(f64, f64)], t: f64) -> f64 {
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
    }

    /// Тест 603a (Р.57): знаковая карта ходит НАЗАД и возвращается, а
    /// потребление при этом только растёт.
    #[test]
    fn t603a_signed_map_walks_back_and_consumption_only_grows() {
        // 1 → −1 линейно: позиция поднимается до середины и возвращается
        let a = vec![(0.0, 1.0), (1.0, -1.0)];
        assert!((source_progress(1.0, &a, 1.0) - 0.0).abs() < 1e-12, "кривая обязана вернуться");
        assert!((source_progress(1.0, &a, 0.5) - 0.25).abs() < 1e-12, "вершина не там");
        // потребление — 0.5 (два треугольника по 0.25) и монотонно
        assert!((consumed_abs(1.0, &a, 1.0) - 0.5).abs() < 1e-12);
        let mut prev = 0.0;
        for i in 0..=200 {
            let t = i as f64 / 200.0;
            let c = consumed_abs(1.0, &a, t);
            assert!(c >= prev - 1e-15, "A(t) обязана быть неубывающей: {c} < {prev}");
            prev = c;
            // A⁻¹ возвращает момент, а не «какой-нибудь корень»
            let back = output_time_for_consumption(1.0, &a, c);
            assert!((back - t).abs() < 1e-6, "A⁻¹(A({t})) = {back}");
        }
    }

    /// Тест 603b (Р.57): разбиение в нуле скорости ТОЧНО, а не примерно —
    /// сверка с независимым численным ∫|rate|.
    #[test]
    fn t603b_zero_crossing_split_is_exact() {
        let a = vec![(0.0, 2.0), (0.4, 2.0), (0.8, -2.0), (1.0, -1.0)];
        let numeric = |t_target: f64| -> f64 {
            let n = 400_000;
            let h = t_target / n as f64;
            let mut acc = 0.0;
            for i in 0..n {
                let t0 = i as f64 * h;
                let t1 = t0 + h;
                acc += 0.5 * (eff_rate(1.0, &a, t0).abs() + eff_rate(1.0, &a, t1).abs()) * h;
            }
            acc
        };
        for t in [0.2, 0.5, 0.6, 0.75, 1.0] {
            let exact = consumed_abs(1.0, &a, t);
            let num = numeric(t);
            assert!((exact - num).abs() < 1e-5, "A({t}): точно {exact}, численно {num}");
        }
        // а ПОЗИЦИЯ в тот же момент меньше потребления — она ходила назад
        assert!(source_progress(1.0, &a, 1.0) < consumed_abs(1.0, &a, 1.0));
    }

    /// Тест 603c (Р.57): на плато нулевой скорости потребление стоит, а
    /// A⁻¹ отдаёт ЛЕВЫЙ край плато — первый момент, когда счётчик дошёл.
    #[test]
    fn t603c_zero_plateau_holds_consumption() {
        let a = vec![(0.0, 1.0), (0.25, 1.0), (0.25, 0.0), (0.5, 0.0), (0.5, 1.0), (1.0, 1.0)];
        let at = |t: f64| consumed_abs(1.0, &a, t);
        assert!((at(0.25) - 0.25).abs() < 1e-12);
        assert!((at(0.5) - 0.25).abs() < 1e-12, "на плато потребление обязано стоять");
        assert!((at(1.0) - 0.75).abs() < 1e-12);
        // позиция на плато тоже стоит — это и есть стоп-кадр
        assert!((source_progress(1.0, &a, 0.4) - 0.25).abs() < 1e-12);
        assert!((output_time_for_consumption(1.0, &a, 0.25) - 0.25).abs() < 1e-9, "левый край");
        assert!(output_time_for_consumption(1.0, &a, 0.2500001) > 0.5 - 1e-6);
    }

    /// Тест 603d (Р.57): на кривых, целиком лежащих в 0.25…32, новая карта
    /// БИТ-В-БИТ равна замороженной старой.
    ///
    /// Это сторож GOLDEN_AUDIO и GOLDEN_VIDEO: они молчат, пока байты не
    /// поехали, но узнать об этом лучше здесь — тут видно, ЧТО поехало.
    #[test]
    fn t603d_positive_curves_are_bit_identical_to_the_old_map() {
        let profiles: Vec<Vec<(f64, f64)>> = vec![
            vec![],
            vec![(0.0, 0.5), (1.0, 2.0)],
            vec![(-1.0, 1.0), (1.0, 2.0)],
            vec![(0.3, 1.0), (0.7, 0.5)],
            vec![
                (0.21616990640748734, 1.0),
                (0.2935781137508999, 0.35),
                (0.3822750179985601, 0.62),
                (0.6397019809160477, 0.6),
                (0.7557084786003034, 1.0),
            ],
        ];
        for base in [0.25, 0.82, 1.0, 32.0] {
            for prof in &profiles {
                for i in -30..=170 {
                    let t = i as f64 / 100.0;
                    let want = frozen::source_progress(base, prof, t);
                    let got = source_progress(base, prof, t);
                    assert_eq!(
                        got.to_bits(),
                        want.to_bits(),
                        "P разошлась с оракулом: base={base} prof={prof:?} t={t}"
                    );
                    let cons = consumed_abs(base, prof, t);
                    assert_eq!(
                        cons.to_bits(),
                        want.to_bits(),
                        "A разошлась с оракулом (на положительных они одно и то же)"
                    );
                }
                // пролёт: минимум в нуле, максимум — прежнее `used`
                let span = 360_000i64;
                let (lo, hi) = warp_span_ticks(base, prof, span);
                assert_eq!(lo, 0, "у положительной кривой пролёт начинается в нуле");
                let old_used =
                    (span as f64 * frozen::source_progress(base, prof, 1.0)).round() as i64;
                assert_eq!(hi, old_used, "максимум пролёта обязан равняться прежнему used");
            }
        }
    }

    /// Тест 603e (Р.57): экстремумы пролёта ищутся В НУЛЯХ СКОРОСТИ, а не
    /// только по концам окна.
    #[test]
    fn t603e_span_finds_extrema_inside_segments() {
        // вверх и назад: максимум посреди события
        let up_down = vec![(0.0, 1.0), (1.0, -1.0)];
        let (lo, hi) = span_over(1.0, &up_down, 0.0, 1.0);
        assert!((lo - 0.0).abs() < 1e-12 && (hi - 0.25).abs() < 1e-12, "({lo}, {hi})");
        // назад и вверх: минимум посреди
        let down_up = vec![(0.0, -1.0), (1.0, 1.0)];
        let (lo, hi) = span_over(1.0, &down_up, 0.0, 1.0);
        assert!((lo + 0.25).abs() < 1e-12 && (hi - 0.0).abs() < 1e-12, "({lo}, {hi})");
        // МОНОТОННАЯ кривая обязана давать РОВНО концы — на этом стоит
        // бит-тождество окон префетча
        let mono = vec![(0.0, 0.5), (1.0, 2.0)];
        for (t0, t1) in [(0.0, 1.0), (0.2, 0.8), (-0.3, 1.4)] {
            let (lo, hi) = span_over(1.0, &mono, t0, t1);
            let (a, b) = (source_progress(1.0, &mono, t0), source_progress(1.0, &mono, t1));
            assert_eq!(lo.to_bits(), a.min(b).to_bits());
            assert_eq!(hi.to_bits(), b.max(a).to_bits());
        }
    }

    /// Тест 603f (Р.57): зеркало «Реверс» сочетается со знаковой картой —
    /// оно берётся вокруг ПРОЛЁТА, и на положительных кривых формула
    /// сворачивается в прежнюю.
    #[test]
    fn t603f_mirror_axis_is_the_span() {
        use crate::{ClipEvent, ClipId, ClipKind, FrameMode, FxInstance, MediaId};
        let dur = Tick(360_000);
        let mk = |rate: f64, reversed: bool| ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(0),
            duration: dur,
            source_in: Tick(720_000),
            media: MediaId(1),
            gain_db: 0.0,
            reversed,
            fx: vec![FxInstance::SlowedReverb {
                rate,
                mix: 0.0,
                reverb_delay_ms: 60.0,
                damp: 0.2,
                decay: 0.84,
                wet_gain: 1.0,
                predelay_ms: 0.0,
                width: 1.0,
                frames: FrameMode::Nearest,
                automation: vec![],
            }],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        };
        // положительная 0.5: пролёт (0, used) — прежняя формула дословно
        let w = crate::clip_warp(&mk(0.5, true)).unwrap();
        assert_eq!(w.span(), (0, w.used()));
        for tl in [0, 90_000, 180_000, 360_000] {
            let want = (w.used() - w.material_off(dur, false, tl)).max(0);
            assert_eq!(w.material_off(dur, true, tl), want, "прежняя формула на tl={tl}");
        }
        // ОТРИЦАТЕЛЬНАЯ −0.5: событие идёт по материалу назад, пролёт
        // целиком в минусе, а зеркало разворачивает его вперёд
        let n = crate::clip_warp(&mk(-0.5, false)).unwrap();
        assert_eq!(n.span(), (-180_000, 0), "пролёт минусовой кривой");
        assert_eq!(n.material_off(dur, false, 0), 0);
        assert_eq!(n.material_off(dur, false, dur.0), -180_000, "ушли назад по материалу");
        let nr = crate::clip_warp(&mk(-0.5, true)).unwrap();
        assert_eq!(nr.material_off(dur, true, 0), -180_000, "зеркало — вокруг ПРОЛЁТА");
        assert_eq!(nr.material_off(dur, true, dur.0), 0);
    }

    /// Тест 603g (Р.57): клампы стали симметричными, ноль проходит.
    #[test]
    fn t603g_clamp_is_symmetric_and_zero_passes() {
        assert_eq!(RATE_CLAMP, (-32.0, 32.0));
        let a = vec![(0.0, -40.0), (1.0, 40.0)];
        assert_eq!(eff_rate(1.0, &a, 0.0), -32.0);
        assert_eq!(eff_rate(1.0, &a, 1.0), 32.0);
        // ноль остаётся нулём: пол — свойство АУДИО, а не карты позиции
        let z = vec![(0.0, 0.0), (1.0, 0.0)];
        assert_eq!(eff_rate(1.0, &z, 0.5), 0.0);
        assert_eq!(source_progress(1.0, &z, 1.0), 0.0);
        assert!(rate_signed(1.0, &a), "кривая с минусом обязана называться знаковой");
        assert!(!rate_signed(1.0, &[(0.0, 0.5), (1.0, 2.0)]), "положительная — нет");
        // охват модуля видит НОЛЬ ВНУТРИ сегмента, а не только концы
        let reach = rate_reach(1.0, &a);
        assert!(reach.0 <= RATE_AUDIO_FLOOR + 1e-12, "минимум охвата: {reach:?}");
        assert!((reach.1 - 32.0).abs() < 1e-12);
    }
}
