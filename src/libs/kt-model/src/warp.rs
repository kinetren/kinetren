//! Time-stretch map: “how much material has been consumed by moment τ”.
//!
//! MOVED HERE FROM kt-fx (R.39) — **do not touch the body: GOLDEN_AUDIO and
//! GOLDEN_VIDEO rest on it**: the move was verbatim, the same f64 operations
//! in the same order. The SIMD-section rules of kt-fx/CLAUDE.md apply here word
//! for word: no FMA, no re-association, no f32, and no “causal” edits from
//! clippy inside this file.
//!
//! Why it is in the model and not in kt-fx: the map must be called by `clip_slice`
//! (`project.rs`) — the rule of a clip piece — and kt-model cannot depend on kt-fx
//! (the dependency direction is the other way). The arithmetic is pure:
//! `f64`/`i64` over `[(f64, f64)]`, no media, no DSP. kt-fx and kt-engine
//! call it from here, the public names kept by re-export.

use crate::{FxInstance, ParamCurve};
use kt_time::Tick;

/// The profile of one parameter in the EVENT DOMAIN: (fraction of output time
/// 0..1, value). Built from the clip curves (`ParamCurve`, keys in material ticks)
/// once at processor construction — after that the audio thread works only
/// with numbers and allocates nothing.
///
/// R.37: the fraction is computed LINEARLY from the start of the event, and the speed does not
/// compress it (the user's requirement: “t — the time span cut out by the current event,
/// WITHOUT accounting for slowing or speeding up”). The construction arithmetic is the same;
/// only the INTERPRETATION of the number changed.
pub type ParamPoints = [(f64, f64)];

/// Value of the profile at position tau (linear interpolation, edges clamped).
/// `base` is returned only for an EMPTY profile: a non-empty one overrides the
/// parameter entirely (v1 semantics — the curve outranks the handle).
pub fn param_at(base: f64, points: &ParamPoints, tau: f64, shapes: &[crate::SegShape]) -> f64 {
    let s = tau;
    if points.is_empty() {
        return base;
    }
    if s <= points[0].0 {
        return points[0].1;
    }
    for (i, w) in points.windows(2).enumerate() {
        if s <= w[1].0 {
            let span = (w[1].0 - w[0].0).max(1e-12);
            let a = (s - w[0].0) / span;
            // the segment's LEFT key carries the shape — both the sound and the graph call
            // ONE function `SegShape::at` (R.61); a short series of shapes
            // means “a straight line”, as for `rate`
            let shape = shapes.get(i).copied().unwrap_or(crate::SegShape::Linear);
            return shape.at(a, w[0].1, w[1].1);
        }
    }
    points[points.len() - 1].1
}

/// The rate value at output position tau. The rate profile is ALWAYS piecewise
/// linear: the analytic form of the warp map rests on this (S — a parabola,
/// T — a square root), and smoothing (`ease`) is ignored on it — the integral of
/// smoothstep would be of the fourth degree. R.37 made ease
/// in principle computable, but there is no gain from it.
pub fn rate_at(base: f64, points: &ParamPoints, tau: f64) -> f64 {
    param_at(base, points, tau, &[])
}

// ---------------------------------------------------------------------------
// EVENT DOMAIN (R.37, the user's decision): the rate curve is a function of the
// OUTPUT progress tau (the fraction of the event length), not of the consumed
// material. The key is still glued to the material (trim/Split/reverse
// work as in R.33), but the axis is LINEAR: the speed does not compress it.
//
// Why: a material-based domain made the rate curve's axis depend ON
// THE RATE CURVE ITSELF (move a value — the point moves across the screen),
// made an early speed key shift the moment of the later keys of OTHER parameters,
// and left the right part of the graph unreachable
// (one could reach only as far as duration·avg_rate).
//
// Map: rate(tau) is piecewise linear; S(tau) = ∫₀^tau rate(u)du — the fraction of
// material consumed by output progress tau; T(s) = S⁻¹.
// S is piecewise QUADRATIC (trapezoid), T — the root of a quadratic equation.
// Neither ln nor expm1: only + − × ÷ √, all correctly rounded per IEEE-754,
// that is, the map is bit-portable across platforms (the golden is still pinned to
// linux x86_64 by the sync resampler, but no longer by the map).
// The rate inside the map is clamped by the same clamp as Warp — otherwise the map would
// diverge from the actual consumption.
// ---------------------------------------------------------------------------

pub const RATE_CLAMP: (f64, f64) = (-32.0, 32.0);

pub fn eff_rate(base: f64, points: &ParamPoints, s: f64) -> f64 {
    rate_at(base, points, s).clamp(RATE_CLAMP.0, RATE_CLAMP.1)
}

/// Δτ over which a segment with starting rate r0 and slope b = dr/dτ
/// consumes Δs of material — the root of (b/2)·dt² + r0·dt − ds = 0.
///
/// The citardauq form (multiplied by the conjugate), and NOT the schoolbook
/// (−r0+√…)/b: it is numerically stable AND does not require a branch b ≈ 0 —
/// at b = 0 it collapses by itself to ds/r0. The denominator is ≥ r0 ≥ RATE_CLAMP.0,
/// it is never zero; the discriminant at the solution point equals rate(τ)², that is,
/// is non-negative by construction (max(0) — only from arithmetic rounding).
fn seg_dt(ds: f64, r0: f64, b: f64) -> f64 {
    2.0 * ds / (r0 + (r0 * r0 + 2.0 * b * ds).max(0.0).sqrt())
}

/// Δs consumed over Δτ on a segment with starting rate r0 and slope
/// b = dr/dτ: the trapezoid ∫(r0 + b·u)du = r0·dt + b·dt²/2. Exact, without branches.
fn seg_ds(dt: f64, r0: f64, b: f64) -> f64 {
    r0 * dt + 0.5 * b * dt * dt
}

/// Segments (t0, t1, r0, r1) of the profile in the EVENT domain; the last one — a tail
/// with a constant rate to infinity (t1 = f64::INFINITY). The body is
/// domain-neutral: it simply walks the array of points.
///
/// IMPORTANT: the ends are clamped BEFORE interpolation, while `eff_rate` clamps AFTER —
/// the values diverge if a key lies outside RATE_CLAMP. Such a key cannot be set from the UI
/// (`set_param` clamps by the spec), but `migrate_legacy_slowed` copies the legacy value as is,
/// so we clamp at the profile input as well
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
    // The profile ALWAYS starts at tau = 0 (R.39). A key to the left of `source_in`
    // is legal — it is hidden by a trim or a cut — and its normalized
    // position is NEGATIVE. The flat segment from zero above is added only
    // when `points[0].0 > 0.0`, so without the trim `source_progress`
    // would integrate FROM THIS POINT: `S(0) != 0`, and the clip would read
    // the material with a shift. A negative point is created by EVERY TrimStart and
    // the right half of EVERY Split.
    //
    // The trim lives here, not in `source_progress`: then it and
    // `output_time_for_source` remain mutually inverse for free, and the branches
    // “beyond the edges” (`segments(...)[0].2`, item 138) extrapolate with the speed
    // AT ZERO, not with the speed of the hidden key — which is what “beyond the edges
    // — the edge values” means.
    //
    // With `points[0].0 >= 0.0` none of the branches below is executed, so the previous
    // path stays byte-for-byte intact (GOLDEN_* rest on it).
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

/// T(s) = S⁻¹: the output progress (fractions of the event length) at which
/// the source fraction s is consumed. Strictly increasing (rate ≥ RATE_CLAMP.0 > 0).
/// s < 0 — the left “handle” of the transition (item 138): linear extrapolation
/// at the speed of the first segment.
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

/// S(t) = ∫₀ᵗ rate(u)du: the source fraction consumed by output progress
/// t (t may be > 1 — the tail beyond the clip; rate beyond the last kf is constant;
/// t < 0 — the left “handle” of the transition, item 138: linearly at the speed of the first
/// segment, “beyond the edges — the edge values”). The handles' material does NOT shift
/// the normalization of automation — the denominator remains clip_len.
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
// SIGNED SPEED (R.57, the owner's decision): rate lives in −32…32, zero
// is allowed. The semantics are Vegas's velocity envelope: the POSITION in the material
// P(τ) = ∫₀^τ rate, minus runs BACKWARD through the material inside the event, zero —
// a freeze frame.
//
// Two AXES follow from this, and they must not be confused:
//
// * **P(τ) = ∫rate** — the signed POSITION (the read address). Non-monotonic, and
//   there is nothing to invert it with: a point of the material may have several exit moments.
//   This is `source_progress` itself — its body is NOT TOUCHED, because
//   `seg_ds` is sign-independent.
// * **A(τ) = ∫|rate|** — the accumulated CONSUMPTION. Monotonic by construction,
//   and invertible (`output_time_for_consumption`). The resampler's cursor lives on it:
//   rubato eats the stream at the speed |rate| and does not know the sign.
//
// On curves lying entirely within 0.25…32, A ≡ P ≡ the former S BIT FOR BIT:
// `fabs` of a positive f64 is the identity, and splitting a segment at zero
// speed is enabled strictly when `r0*r1 < 0`, which never happens on such curves.
// GOLDEN_AUDIO and GOLDEN_VIDEO rest on this (guard — t603d).
// ---------------------------------------------------------------------------

/// Speed floor for AUDIO: |rate| below it is a hold, not a very slow reading.
/// One constant for the map and for the processor: if they diverged,
/// the consumption would drift from what the resampler actually ate.
///
/// The owner's decision: “pull down to 0.01, silence only at zero”.
pub const RATE_AUDIO_FLOOR: f64 = 0.01;

/// Speed magnitude with a floor: below the floor — zero (hold).
fn mag_floor(r: f64) -> f64 {
    let a = r.abs();
    if a < RATE_AUDIO_FLOOR { 0.0 } else { a }
}

/// Profile segments split AT SPEED ZERO, with |rate| at the ends.
///
/// The split is enabled strictly when `r0*r1 < 0.0` — not `<=`. A segment
/// ENDING at zero stays linear modulo, and an extra term
/// `acc += 0.0` would shift the rounding where nothing changes.
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

/// A(t) = ∫₀ᵗ |rate(u)|du — how much material the chain has EATEN by output
/// progress t. Monotonically non-decreasing; it grows strictly everywhere except on
/// a plateau of zero speed.
///
/// The body repeats `source_progress` step by step (the same loop, the same
/// `seg_ds`, the same order of addition) — otherwise bit-identity on
/// positive curves cannot be held.
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

/// A⁻¹: the output progress at which `a` of the material has been EATEN.
///
/// Plateau rule: at zero speed the consumption stands still, and the answer
/// is taken as the LEFT edge of the plateau — the first moment when the counter reached `a`.
/// The answer `+∞` means “this much is never eaten” (the tail of the profile —
/// zero speed).
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
            // exactly at the boundary (and on a plateau) — the LEFT edge: `seg_dt` with
            // a zero `need` and zero speed would give 0/0
            return if need <= 0.0 { acc_t } else { acc_t + seg_dt(need, r0, b) };
        }
        acc_t += seg_t;
        acc_a += seg_a;
    }
    // zero-speed tail: `acc_t` is already infinite if the tail was
    // non-zero; otherwise this much material is never eaten
    if acc_a >= a { acc_t } else { f64::INFINITY }
}

/// The SPAN of POSITION over [t0, t1]: (minimum P, maximum P).
///
/// For a monotonic profile it returns EXACTLY `(P(t0), P(t1))` — the bit-identity of the
/// prefetch windows and of the mirror axis rests on this.
pub fn span_over(base: f64, points: &ParamPoints, t0: f64, t1: f64) -> (f64, f64) {
    let (a, b) = (source_progress(base, points, t0), source_progress(base, points, t1));
    let (mut lo, mut hi) = (a.min(b), a.max(b));
    // the extrema of P — only at speed zeros: between them the sign is constant
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

/// Span of position over the WHOLE event, in ticks. `span_ticks` — the length of the event.
pub fn warp_span_ticks(base: f64, points: &ParamPoints, span_ticks: i64) -> (i64, i64) {
    let (lo, hi) = span_over(base, points, 0.0, 1.0);
    ((span_ticks as f64 * lo).round() as i64, (span_ticks as f64 * hi).round() as i64)
}

/// Limits of the MAGNITUDE of speed over the whole profile (for the resampler: it must
/// know what spread of ratios to expect, before the first chunk).
/// Zero is raised to the floor: on a hold the resampler does not work at all.
pub fn rate_reach(base: f64, points: &ParamPoints) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
    // over the SPLIT segments: a curve that changes sign passes through zero
    // INSIDE a segment, and over the ends alone the minimum would come out overstated —
    // and the resampler would then reject the change of ratio (R.57).
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

/// Does the profile go BACKWARD through the material anywhere at all?
///
/// This is the predicate “the map is non-monotonic”: it gates everything built on
/// reading forward (address runs, prefetch windows, the block scheme of the sub-mix).
pub fn rate_signed(base: f64, points: &ParamPoints) -> bool {
    segments(base, points).iter().any(|&(_, _, r0, r1)| r0 < 0.0 || r1 < 0.0)
}

/// A direction-change node: the moment, accumulated consumption, position and
/// direction AFTER the node (+1 forward, −1 backward, 0 hold).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignKnot {
    pub tau: f64,
    pub a: f64,
    pub p: f64,
    pub dir_after: i8,
}

/// Direction-change nodes of the profile on [0, ∞). The first is always at zero.
/// Neighbouring nodes with the same direction are collapsed: runs must be
/// as long as possible — audio copies memory in chunks along them.
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
        // the direction RIGHT AFTER the node: the speed in the middle of the next
        // micro-step, not at the point itself (at the point it may be zero
        // exactly at the sign change)
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

/// The direction right AFTER the moment `tau` (zero at the very point of a sign change
/// is not counted as a direction).
fn dir_after(base: f64, points: &ParamPoints, tau: f64) -> i8 {
    for (t0, t1, r0, r1) in abs_segments(base, points) {
        if tau >= t1 {
            continue;
        }
        // on this segment the magnitude is linear; the sign is taken from the SOURCE profile
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

/// Material CONSUMED, in TICKS, by output progress `tau` (R.37 item 2).
/// The only entry into the map for VIDEO: computing through `to_audio_frames`
/// is not allowed — 1 frame at 48 kHz = 1.875 ticks, and rounding misses the boundary
/// of a FRAME; for audio this is invisible, for video it is immediately the wrong frame.
pub fn warp_material_ticks(base: f64, points: &ParamPoints, span_ticks: i64, tau: f64) -> i64 {
    (span_ticks as f64 * source_progress(base, points, tau)).round() as i64
}

/// ∫₀ᵗ p(u)du for an ARBITRARY parameter WITHOUT clamping the values (R.47,
/// item 324): the LFO phase of audio modulations is the integral of the rate_hz curve
/// over the clip position. `source_progress` is not suitable for this — it stitches the values
/// to RATE_CLAMP (0.25..32), while the LFO frequency lives within its own spec limits.
/// A piecewise-linear profile, the trapezoid exact; beyond the last point
/// a constant; t < 0 — linearly at the value of the first segment.
pub fn param_integral(base: f64, points: &ParamPoints, t: f64) -> f64 {
    if points.is_empty() {
        return base * t;
    }
    let first = points[0].1;
    if t <= points[0].0 {
        // before the first key the profile is flat at its value (as in segments)
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

/// Average speed = the consumed fraction of the source over the whole clip: S(1).
pub fn avg_rate(base: f64, points: &ParamPoints) -> f64 {
    source_progress(base, points, 1.0)
}

/// Automation curves of the CLIP, translated into the SOURCE domain (R.33).
///
/// Keys in the model live in material ticks; the audio path counts in material
/// fractions (`s`), because the warp map is built on them. The translation
/// is done ONCE at processor construction; after that the audio thread sees
/// only numbers: no allocations, no access to the model in the render thread.
/// A curve of one slot parameter: name, smoothing, points (s, value).
#[derive(Debug, Clone, PartialEq)]
struct SlotCurve {
    param: String,
    /// Segment shape AFTER each point (R.61); for `rate` it is always empty —
    /// the speed profile is piecewise linear by law.
    shapes: Vec<crate::SegShape>,
    points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AutoProfiles {
    /// For the chain slot — its curves.
    slots: Vec<Vec<SlotCurve>>,
}

impl AutoProfiles {
    /// From the clip's curves (`ClipEvent::active_fx_auto`) into fractions of the material.
    /// `src_in_frames`/`clip_len_frames` — the same quantities by which
    /// the automation of slowed&reverb was normalized before R.33, so the
    /// profile of old projects numerically matches the former one.
    pub fn from_curves(
        auto: &[Vec<ParamCurve>],
        src_in_frames: i64,
        clip_len_frames: i64,
    ) -> AutoProfiles {
        Self::from_curves_rev(auto, src_in_frames, clip_len_frames, false)
    }

    /// The same, taking the REVERSE into account (R.39).
    ///
    /// The normalization `(k.t − src_in)/len` equals the output fraction of the key only
    /// because the UI writes the key at `src_in + p·duration`. For a mirrored
    /// clip the UI writes it MIRRORED (`clip_material_at` accounts for the reverse),
    /// and the equality breaks: a key placed at the start of the event was played by the DSP
    /// at the end. It survived until R.39 because not a single test in the whole repository
    /// combined `reversed` with a non-empty `fx_auto`.
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
                            // rate must remain piecewise linear: the analytic formulas of the warp map rest on it,
                            // and any other shape on it is rejected
                            // already by the command (R.61)
                            // — here is the safety net
                            let shapes: Vec<crate::SegShape> = if c.param == "rate" {
                                Vec::new()
                            } else {
                                c.keys.iter().map(|k| k.shape).collect()
                            };
                            // the value is clamped HERE: `segments` clamps the ends before interpolation,
                            // and `eff_rate` — after, and on a key outside RATE_CLAMP they diverged.
                            // Such a key cannot be set from the UI, but
                            // `migrate_legacy_slowed` copies a legacy number
                            // as is, and a mismatch of the map with the DSP means
                            // prefetch misses and silence
                            // (item 13/t17).
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
                            // the mirror reverses the order of the points, while
                            // the map walks the array from left to right
                            let pts: Vec<(f64, f64)> = if reversed {
                                let mut v = pts;
                                v.reverse();
                                v
                            } else {
                                pts
                            };
                            SlotCurve { param: c.param.clone(), shapes, points: pts }
                        })
                        .collect()
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_empty())
    }

    /// Profile of a slot parameter (an empty slice — there is no curve).
    pub fn points(&self, slot: usize, param: &str) -> &ParamPoints {
        self.slots
            .get(slot)
            .and_then(|s| s.iter().find(|c| c.param == param))
            .map_or(&[], |c| c.points.as_slice())
    }

    /// Shapes of the segments of a parameter curve: nodes that read the value BY
    /// POSITION (LFO, pitch) need the same series as `value`.
    pub fn shapes_of(&self, slot: usize, param: &str) -> &[crate::SegShape] {
        self.slots
            .get(slot)
            .and_then(|s| s.iter().find(|c| c.param == param))
            .map_or(&[], |c| &c.shapes)
    }

    /// Value of the slot parameter at material position s.
    pub fn value(&self, slot: usize, param: &str, base: f64, s: f64) -> f64 {
        param_at(base, self.points(slot, param), s, self.shapes_of(slot, param))
    }

    /// Index of the slot with the warp and its speed profile — the whole analytics of the
    /// source rests on it (how much material the clip will eat).
    fn warp_slot(fx: &[FxInstance]) -> Option<usize> {
        fx.iter().position(|f| matches!(f, FxInstance::SlowedReverb { .. }))
    }

    /// Speed profile of the chain (empty — there is no warp or it has no curve).
    pub fn rate_points(&self, fx: &[FxInstance]) -> &ParamPoints {
        Self::warp_slot(fx).map_or(&[], |i| self.points(i, "rate"))
    }

    /// Mix profile of the chain — needed to know whether the reverb rings.
    pub fn mix_points(&self, fx: &[FxInstance]) -> &ParamPoints {
        Self::warp_slot(fx).map_or(&[], |i| self.points(i, "mix"))
    }
}

/// Playback speed of the chain (1.0 = no warp).
pub fn chain_rate(fx: &[FxInstance]) -> f64 {
    fx.iter()
        .find_map(|f| match f {
            FxInstance::SlowedReverb { rate, .. } => Some(rate.clamp(RATE_CLAMP.0, RATE_CLAMP.1)),
            _ => None,
        })
        .unwrap_or(1.0)
}

/// Average speed of the chain with automation: determines how much of the
/// source the clip eats (for the source window and the duration).
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

    /// Point of the speed profile: (fraction of material, rate).
    fn kf(t: f64, rate: f64) -> (f64, f64) {
        (t, rate)
    }

    #[test]
    fn rate_interp_clamps_ends_and_is_linear() {
        let a = vec![kf(0.2, 1.0), kf(0.4, 0.5)];
        assert_eq!(rate_at(9.0, &a, 0.0), 1.0, "before the first point — its value (base is ignored)");
        assert_eq!(rate_at(9.0, &a, 0.3), 0.75);
        assert_eq!(rate_at(9.0, &a, 1.0), 0.5);
    }

    /// Source map: round-trip T∘S = id, monotonicity, a check against an
    /// independent numerical integrator (the song1 profile).
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
            assert!(s >= prev, "S(t) must be non-decreasing");
            prev = s;
            // round-trip
            let back = output_time_for_source(1.0, &a, s);
            assert!((back - t).abs() < 1e-9, "T(S({t})) = {back}");
        }
        // an independent numerical integrator: S(τ) by trapezoids OVER rate
        // (R.37: we integrate the speed over OUTPUT time, not 1/rate over the
        // material)
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
                "S({t}): analytic {analytic} vs numeric {numeric}"
            );
        }
        // S is a PARABOLA, not an exponential: on a linear ramp 1→2 over the whole
        // event the integral equals exactly the average speed. Without ln this is
        // computed exactly, and so we check it exactly.
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

    /// An anchor to the OUTPUT TIME (R.37, a mirror of the former test): an early
    /// speed bump changes the MATERIAL that plays at a late moment, but not the
    /// moment at which the late keys fire.
    #[test]
    fn early_keyframe_shifts_material_not_time() {
        let a = vec![kf(0.4, 1.0), kf(0.5, 0.5), kf(0.7, 0.5), kf(0.75, 1.0)];
        let mut b = vec![kf(0.1, 1.0), kf(0.18, 2.0), kf(0.26, 2.0), kf(0.34, 1.0)];
        b.extend(a.iter().cloned());
        for t in [0.45f64, 0.55, 0.65, 0.72, 0.90] {
            assert_eq!(
                rate_at(1.0, &a, t),
                rate_at(1.0, &b, t),
                "rate at time t={t} must not depend on an early bump"
            );
        }
        // and the MATERIAL finished by t=0.5 is larger under the bump
        let sa = source_progress(1.0, &a, 0.5);
        let sb = source_progress(1.0, &b, 0.5);
        assert!(sb > sa, "speeding up earlier = more material consumed: {sb} vs {sa}");
    }
}

#[cfg(test)]
mod zero_start_tests {
    use super::*;

    /// Test 269 (R.39, D0): the profile ALWAYS starts at zero.
    ///
    /// A key to the left of `source_in` is legal — a trim hides it. Its point
    /// is negative, and without the trim `source_progress` would integrate
    /// from it: the clip would read the material with a shift. The defect is alive and was there before R.39:
    /// a negative point is created by EVERY TrimStart and the right half
    /// of EVERY Split.
    #[test]
    fn t269_profile_starts_at_zero_with_a_hidden_key() {
        // ramp 1.0 → 2.0, the key at -1 is “hidden” to the left of the piece's start
        let hidden = [(-1.0, 1.0), (1.0, 2.0)];

        assert_eq!(
            source_progress(1.0, &hidden, 0.0),
            0.0,
            "at the start of an event no material is consumed yet: S(0) must be EXACTLY zero"
        );
        assert_eq!(
            warp_material_ticks(1.0, &hidden, 90_000, 0.0),
            0,
            "and in ticks too zero — otherwise the piece starts at the wrong frame"
        );
        // over [0,1] the speed goes 1.5 → 2.0, the trapezoid integral = 1.75
        let s1 = source_progress(1.0, &hidden, 1.0);
        assert!(
            (s1 - 1.75).abs() < 1e-12,
            "S(1) must be the analytic trapezoid 1.75, not {s1}"
        );

        // T = S⁻¹ remained the inverse: the trim is done in `segments`, so
        // both functions see the same profile
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let back = output_time_for_source(1.0, &hidden, source_progress(1.0, &hidden, t));
            assert!((back - t).abs() < 1e-9, "T(S({t})) = {back}");
        }

        // a profile with a NON-NEGATIVE first point is not touched by a single bit:
        // GOLDEN_AUDIO and GOLDEN_VIDEO rest on this path
        let plain = [(0.3, 1.0), (0.7, 0.5)];
        assert_eq!(source_progress(1.0, &plain, 0.0), 0.0);
        let s = source_progress(1.0, &plain, 1.0);
        assert!((s - 0.75).abs() < 1e-12, "0.3·1 + 0.4·0.75 + 0.3·0.5 = 0.75, not {s}");
    }
}

// ---------------------------------------------------------------------------
// The map of a SPECIFIC clip (R.39). It used to live in kt-engine as three nearly
// identical structures (`VideoWarp`, `ClipWarp`, and an implicit one inside
// `clip_material_pos`); now there is one constructor, and the engine takes it from here.
// ---------------------------------------------------------------------------

/// Time stretch of the clip: the base, the speed profile and the
/// actually consumed material.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipWarp {
    base: f64,
    prof: Vec<(f64, f64)>,
    /// Material in ticks that the event ACTUALLY EATS: A(1)·span.
    /// With signed speed (R.57) this is no longer “how far we got”: the curve
    /// may go back and return, having eaten twice what it passed.
    used: i64,
    /// the SPAN of position (min, max) in ticks from `source_in` (R.57). This is
    /// the material window of the event: by it the warmth, prefetch and the mirror axis
    /// are computed. On positive curves — (0, used).
    span_min: i64,
    span_max: i64,
    /// “Mix” mode (R.37 stage 5): read by the layer's executor.
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
    /// Material window (min, max) in ticks from `source_in` (R.57).
    pub fn span(&self) -> (i64, i64) {
        (self.span_min, self.span_max)
    }
    /// Length of the material window — the filmstrip step and the warmth are measured by it.
    pub fn span_len(&self) -> i64 {
        (self.span_max - self.span_min).max(1)
    }
    pub fn blend(&self) -> bool {
        self.blend
    }

    /// How much material is eaten by the moment `tl` ticks from the start of the event.
    /// Reverse — “WARP, THEN MIRROR” (R.37 item 247): a mirror BEFORE the warp would drive
    /// the speed curve backwards and desynchronize A/V.
    pub fn material_off(&self, duration: Tick, reversed: bool, tl: i64) -> i64 {
        let dur = duration.0.max(1);
        let consumed = warp_material_ticks(self.base, &self.prof, dur, tl as f64 / dur as f64);
        // The mirror axis is the SPAN of position, not “how much is eaten” (R.57).
        // On positive curves the span equals (0, used), and the expression
        // collapses to the former `(used − consumed).max(0)` — the same
        // integer, the same bits.
        if reversed {
            (self.span_min + self.span_max - consumed).max(self.span_min)
        } else {
            consumed
        }
    }
}

/// Map from the CHAIN AND ITS CURVES. `None` — there is no warp, the mapping is linear.
///
/// The input must be ACTIVE (`active_fx`/`active_fx_auto`): disabled slots and the master switch
/// have already been discarded. `AutoProfiles::warp_slot` looks for the slot BY KIND,
/// so on unfiltered curves the speed profile would be computed
/// from someone else's slot.
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
    // The profile is built by THE SAME function as for audio: one domain for both
    // kinds, otherwise picture and sound would drift apart on the speed curve.
    // The key position is normalized in AUDIO FRAMES (as for audio) — the fraction
    // is dimensionless, and both sides get one number. In TICKS only the OUTPUT of the map is
    // computed (`warp_material_ticks`): there the 48 kHz grid
    // would miss the frame boundary.
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

/// Does the chain actually stretch TIME.
///
/// Having a slot and stretching are different things: `SlowedReverb` at speed 1×
/// gives a tail, but the material is consumed 1:1. `warps_time()` answers the
/// question “does this KIND warp”, and for the veto on cutting a piece this is enough;
/// where one asks “will the time move” (a nested item, R.50), an answer about the
/// VALUES is needed.
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

/// Map of the clip from its active chain.
pub fn clip_warp(c: &crate::ClipEvent) -> Option<ClipWarp> {
    // a cheap filter BEFORE the allocations of `active_fx`/`active_fx_auto`:
    // `effective_clips` calls this for each piece of each clip
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

    /// Test 271 (R.39, D2): on a mirrored clip the curve is not reversed.
    ///
    /// The normalization `(k.t − src_in)/len` equals the output fraction of the key only
    /// because the UI writes the key at `src_in + p·duration`. For a reversed
    /// clip the UI writes it MIRRORED — and without the mirror HERE, a key placed
    /// at the start of the event was played at the end.
    ///
    /// Not a single test in the repository combined `reversed` with a non-empty
    /// `fx_auto` — which is why the defect survived.
    #[test]
    fn t271_reversed_curve_is_not_played_backwards() {
        // clip: source_in 0, length 1 s; a key at 0.25 of material = 0.75
        // of the output fraction for a mirrored clip
        let len = kt_time::Tick(90_000).to_audio_frames();
        let curves = vec![vec![ParamCurve {
            param: "db".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(kt_time::Tick(0), 0.0),
                AutoKey::at(kt_time::Tick(90_000 / 4), 10.0),
                AutoKey::at(kt_time::Tick(90_000), 0.0),
            ]),
            source: Default::default(),
        }]];

        let fwd = AutoProfiles::from_curves_rev(&curves, 0, len, false);
        let rev = AutoProfiles::from_curves_rev(&curves, 0, len, true);

        // direct clip: the peak at output fraction 0.25
        assert!((fwd.value(0, "db", 0.0, 0.25) - 10.0).abs() < 1e-9);
        // mirrored: the SAME material plays at 0.75 of the output
        assert!(
            (rev.value(0, "db", 0.0, 0.75) - 10.0).abs() < 1e-9,
            "the peak must fall on the mirrored share: {}",
            rev.value(0, "db", 0.0, 0.75)
        );
        assert!(
            rev.value(0, "db", 0.0, 0.25) < 5.0,
            "and NOT on the direct share: {}",
            rev.value(0, "db", 0.0, 0.25)
        );
        // direct path untouched: from_curves == from_curves_rev(false)
        assert_eq!(AutoProfiles::from_curves(&curves, 0, len), fwd);
    }
}

#[cfg(test)]
mod signed_tests {
    use super::*;

    /// A FROZEN copy of the map BEFORE R.57 — the oracle of bit-identity.
    ///
    /// Not a “similar function”: the verbatim old text with the former clamp.
    /// Any extra operation on the positive path (abs, a branch, a second addition)
    /// would immediately diverge in the last bit — and t603d will show
    /// this before GOLDEN turns red.
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

    /// Test 603a (R.57): the signed map goes BACKWARD and returns, while
    /// consumption keeps only growing.
    #[test]
    fn t603a_signed_map_walks_back_and_consumption_only_grows() {
        // 1 → −1 linearly: the position rises to the middle and comes back
        let a = vec![(0.0, 1.0), (1.0, -1.0)];
        assert!((source_progress(1.0, &a, 1.0) - 0.0).abs() < 1e-12, "the curve must return");
        assert!((source_progress(1.0, &a, 0.5) - 0.25).abs() < 1e-12, "peak is not there");
        // consumption is 0.5 (two triangles of 0.25) and monotonic
        assert!((consumed_abs(1.0, &a, 1.0) - 0.5).abs() < 1e-12);
        let mut prev = 0.0;
        for i in 0..=200 {
            let t = i as f64 / 200.0;
            let c = consumed_abs(1.0, &a, t);
            assert!(c >= prev - 1e-15, "A(t) must be non-decreasing: {c} < {prev}");
            prev = c;
            // A⁻¹ returns a moment, not “some root”
            let back = output_time_for_consumption(1.0, &a, c);
            assert!((back - t).abs() < 1e-6, "A⁻¹(A({t})) = {back}");
        }
    }

    /// Test 603b (R.57): splitting at zero speed is EXACT, not approximate —
    /// cross-checked against an independent numeric ∫|rate|.
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
            assert!((exact - num).abs() < 1e-5, "A({t}): exact {exact}, numeric {num}");
        }
        // but the POSITION at the same moment is less than the consumption — it went backward
        assert!(source_progress(1.0, &a, 1.0) < consumed_abs(1.0, &a, 1.0));
    }

    /// Test 603c (R.57): on a plateau of zero speed consumption holds, while
    /// A⁻¹ returns the LEFT edge of the plateau — the first moment the counter reached it.
    #[test]
    fn t603c_zero_plateau_holds_consumption() {
        let a = vec![(0.0, 1.0), (0.25, 1.0), (0.25, 0.0), (0.5, 0.0), (0.5, 1.0), (1.0, 1.0)];
        let at = |t: f64| consumed_abs(1.0, &a, t);
        assert!((at(0.25) - 0.25).abs() < 1e-12);
        assert!((at(0.5) - 0.25).abs() < 1e-12, "on the plateau consumption must hold");
        assert!((at(1.0) - 0.75).abs() < 1e-12);
        // the position on the plateau also holds — that is the freeze frame
        assert!((source_progress(1.0, &a, 0.4) - 0.25).abs() < 1e-12);
        assert!((output_time_for_consumption(1.0, &a, 0.25) - 0.25).abs() < 1e-9, "left edge");
        assert!(output_time_for_consumption(1.0, &a, 0.2500001) > 0.5 - 1e-6);
    }

    /// Test 603d (R.57): on curves lying entirely within 0.25…32, the new map
    /// is BIT-FOR-BIT equal to the frozen old one.
    ///
    /// This is the guard for GOLDEN_AUDIO and GOLDEN_VIDEO: they stay silent until the bytes
    /// shift, but it is better to learn about it here — here one can see WHAT shifted.
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
                        "P diverged from the oracle: base={base} prof={prof:?} t={t}"
                    );
                    let cons = consumed_abs(base, prof, t);
                    assert_eq!(
                        cons.to_bits(),
                        want.to_bits(),
                        "A diverged from the oracle (on positive values they are the same)"
                    );
                }
                // span: minimum at zero, maximum — the previous `used`
                let span = 360_000i64;
                let (lo, hi) = warp_span_ticks(base, prof, span);
                assert_eq!(lo, 0, "on a positive curve the span starts at zero");
                let old_used =
                    (span as f64 * frozen::source_progress(base, prof, 1.0)).round() as i64;
                assert_eq!(hi, old_used, "the span maximum must equal the previous used");
            }
        }
    }

    /// Test 603e (R.57): span extremes are searched AT THE ZERO-SPEED POINTS, not only
    /// at the ends of the window.
    #[test]
    fn t603e_span_finds_extrema_inside_segments() {
        // up and back: the maximum in the middle of the event
        let up_down = vec![(0.0, 1.0), (1.0, -1.0)];
        let (lo, hi) = span_over(1.0, &up_down, 0.0, 1.0);
        assert!((lo - 0.0).abs() < 1e-12 && (hi - 0.25).abs() < 1e-12, "({lo}, {hi})");
        // back and up: the minimum in the middle
        let down_up = vec![(0.0, -1.0), (1.0, 1.0)];
        let (lo, hi) = span_over(1.0, &down_up, 0.0, 1.0);
        assert!((lo + 0.25).abs() < 1e-12 && (hi - 0.0).abs() < 1e-12, "({lo}, {hi})");
        // A MONOTONIC curve must give EXACTLY the ends — the bit-identity of the
        // prefetch windows rests on this
        let mono = vec![(0.0, 0.5), (1.0, 2.0)];
        for (t0, t1) in [(0.0, 1.0), (0.2, 0.8), (-0.3, 1.4)] {
            let (lo, hi) = span_over(1.0, &mono, t0, t1);
            let (a, b) = (source_progress(1.0, &mono, t0), source_progress(1.0, &mono, t1));
            assert_eq!(lo.to_bits(), a.min(b).to_bits());
            assert_eq!(hi.to_bits(), b.max(a).to_bits());
        }
    }

    /// Test 603f (R.57): the “Reverse” mirror combines with the signed map —
    /// it is taken around the SPAN, and on positive curves the formula
    /// collapses back into the previous one.
    #[test]
    fn t603f_mirror_axis_is_the_span() {
        use crate::{ClipEvent, ClipId, ClipKind, FrameMode, FxInstance, MediaId};
        let dur = Tick(360_000);
        let mk = |rate: f64, reversed: bool| ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
        // positive 0.5: span (0, used) — the previous formula verbatim
        let w = crate::clip_warp(&mk(0.5, true)).unwrap();
        assert_eq!(w.span(), (0, w.used()));
        for tl in [0, 90_000, 180_000, 360_000] {
            let want = (w.used() - w.material_off(dur, false, tl)).max(0);
            assert_eq!(w.material_off(dur, true, tl), want, "previous formula at tl={tl}");
        }
        // NEGATIVE −0.5: the event runs backward through the material, the span
        // lies entirely in the negative, and the mirror turns it forward
        let n = crate::clip_warp(&mk(-0.5, false)).unwrap();
        assert_eq!(n.span(), (-180_000, 0), "span of a negative curve");
        assert_eq!(n.material_off(dur, false, 0), 0);
        assert_eq!(n.material_off(dur, false, dur.0), -180_000, "went back through the material");
        let nr = crate::clip_warp(&mk(-0.5, true)).unwrap();
        assert_eq!(nr.material_off(dur, true, 0), -180_000, "mirror — around the SPAN");
        assert_eq!(nr.material_off(dur, true, dur.0), 0);
    }

    /// Test 603g (R.57): the clamps became symmetric, zero passes through.
    #[test]
    fn t603g_clamp_is_symmetric_and_zero_passes() {
        assert_eq!(RATE_CLAMP, (-32.0, 32.0));
        let a = vec![(0.0, -40.0), (1.0, 40.0)];
        assert_eq!(eff_rate(1.0, &a, 0.0), -32.0);
        assert_eq!(eff_rate(1.0, &a, 1.0), 32.0);
        // zero stays zero: the floor is a property of AUDIO, not of the position map
        let z = vec![(0.0, 0.0), (1.0, 0.0)];
        assert_eq!(eff_rate(1.0, &z, 0.5), 0.0);
        assert_eq!(source_progress(1.0, &z, 1.0), 0.0);
        assert!(rate_signed(1.0, &a), "a curve with a minus must be called signed");
        assert!(!rate_signed(1.0, &[(0.0, 0.5), (1.0, 2.0)]), "positive one — no");
        // the module’s coverage check sees ZERO INSIDE a segment, not only the ends
        let reach = rate_reach(1.0, &a);
        assert!(reach.0 <= RATE_AUDIO_FLOOR + 1e-12, "minimum reach: {reach:?}");
        assert!((reach.1 - 32.0).abs() < 1e-12);
    }
}
