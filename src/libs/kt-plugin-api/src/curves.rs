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
    kt_time::cost::add(kt_time::cost::Cost::IntegralWalk, points.len() as u64);
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

/// [`param_integral`] COMPILED once per curve (compiled-curves; found-defects row 152): the LFO
/// phase of a modulation node is this integral per SAMPLE, and the function walks the curve from its
/// first point each time — a formula layer on the frequency resolves to a point per frame, 10 800
/// points on a 6-minute event, so the walk per sample was unbounded.
///
/// Here the sum of every whole segment before each one is kept, added in the former loop's order,
/// so a reading is that loop's running sum plus the same partial term: the same bits (t2107). The
/// segment of `t` is found by halving on the segment ends; a curve whose points are not sorted
/// (never built so, but not refused either) reads through [`param_integral`] itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParamIntegral {
    points: Vec<(f64, f64)>,
    /// per non-empty segment: its right end, the index of its left point, the sum before it
    segs: Vec<(f64, usize, f64)>,
    /// the sum of every whole segment — the former loop's `acc` after it
    total: f64,
    sorted: bool,
}

impl ParamIntegral {
    pub fn of(points: &ParamPoints) -> ParamIntegral {
        let sorted = points.windows(2).all(|w| w[1].0 >= w[0].0);
        let mut segs = Vec::new();
        let mut acc = points.first().map_or(0.0, |p| p.1 * p.0);
        for (i, w) in points.windows(2).enumerate() {
            let (t0, v0) = w[0];
            let (t1, v1) = w[1];
            if t1 <= t0 {
                continue;
            }
            segs.push((t1, i, acc));
            // the whole segment exactly as the loop adds it: `hi` is `t1`
            let v_hi = v0 + (v1 - v0) * (t1 - t0) / (t1 - t0);
            acc += (v0 + v_hi) * 0.5 * (t1 - t0);
        }
        ParamIntegral { points: points.to_vec(), segs, total: acc, sorted }
    }

    pub fn points(&self) -> &ParamPoints {
        &self.points
    }

    /// [`param_integral`]`(base, points, t)`, the same bits.
    #[inline]
    pub fn at(&self, base: f64, t: f64) -> f64 {
        let points = &self.points[..];
        if points.is_empty() {
            return base * t;
        }
        if !self.sorted {
            return param_integral(base, points, t);
        }
        let first = points[0].1;
        if t <= points[0].0 {
            return first * t;
        }
        // the first segment whose right end reaches `t` — where the loop returns
        let k = self.segs.partition_point(|s| t > s.0);
        kt_time::cost::add(
            kt_time::cost::Cost::IntegralWalk,
            (usize::BITS - self.segs.len().leading_zeros()) as u64,
        );
        let Some(&(t1, i, before)) = self.segs.get(k) else {
            let (tl, vl) = points[points.len() - 1];
            return self.total + vl * (t - tl);
        };
        let ((t0, v0), (_, v1)) = (points[i], points[i + 1]);
        let mut acc = before;
        let hi = t.min(t1);
        if hi > t0 {
            let v_hi = v0 + (v1 - v0) * (hi - t0) / (t1 - t0);
            acc += (v0 + v_hi) * 0.5 * (hi - t0);
        }
        acc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 2107 (compiled-curves): the prefix integral reads `param_integral`'s bits everywhere —
    /// before, on and after every point, on empty segments (duplicate positions), on a single point,
    /// on an empty curve and on an unsorted one (which reads through the walk itself).
    #[test]
    fn t2107_prefix_integral_is_the_walk_bit_for_bit() {
        let mut seed = 2107u64;
        let mut next = || {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) as f64 / (1u64 << 31) as f64
        };
        let mut cases: Vec<Vec<(f64, f64)>> = vec![
            vec![],
            vec![(0.3, 4.0)],
            vec![(-0.2, 1.0), (0.0, 1.0), (0.0, 7.0), (0.5, 2.0), (0.5, 2.0), (1.0, 9.0)],
            vec![(0.0, 2.0), (0.4, 8.0), (0.2, 1.0), (1.0, 3.0)], // unsorted
        ];
        for n in [2usize, 7, 100, 3000] {
            let mut t = -0.1;
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                if i % 11 != 3 {
                    t += next() / n as f64;
                }
                v.push((t, next() * 20.0 - 2.0));
            }
            cases.push(v);
        }
        for (ci, pts) in cases.iter().enumerate() {
            let pre = ParamIntegral::of(pts);
            let mut probes: Vec<f64> = vec![-1.0, -0.0, 0.0, 1.0, 2.5];
            for p in pts {
                probes.extend([p.0, p.0 - 1e-9, p.0 + 1e-9]);
            }
            probes.extend((0..500).map(|_| next() * 1.4 - 0.2));
            for base in [0.0, 3.5] {
                for &t in &probes {
                    let want = param_integral(base, pts, t);
                    assert_eq!(pre.at(base, t).to_bits(), want.to_bits(), "case {ci}, base {base}, t {t}");
                }
            }
        }
    }
}
