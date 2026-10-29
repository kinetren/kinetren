//! Curve maths an audio kernel needs inside `process`, with no model at hand (plugin-api-stage1,
//! D2a): the profile type and its integral. kt-model re-exports both; this is their one home.

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
