//! What the preview's composites cost, moment by moment (render-honest D2, found-defects 711): the
//! device time of each complete composite with its size, so the render window can predict a render
//! of the same range at its own size. The preview's clock is legal here (invariant 11 binds the
//! offline paths); nothing composes differently because of it.

use kt_time::Tick;
use std::collections::{BTreeMap, VecDeque};

/// Moments kept at most: an hour of 60 fps. A moment past it drops the oldest recorded.
const MAX_MOMENTS: usize = 216_000;

/// The newest measured composite per aligned moment and size.
#[derive(Default)]
pub struct PreviewCosts {
    at: BTreeMap<(i64, u32, u32), Sample>,
    /// The moments in the order they were first recorded, for the bound.
    order: VecDeque<(i64, u32, u32)>,
    seq: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sample {
    /// The order of the record: the newest names the preview's size.
    seq: u64,
    ns: u64,
}

/// The preview's measured cost over a range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewCost {
    /// Moments of the range with a measurement.
    pub moments: u64,
    /// Device nanoseconds per output pixel over them.
    pub ns_per_px: f64,
    /// The size of the newest of them: the preview's size.
    pub size: (u32, u32),
}

impl PreviewCost {
    /// The device time `frames` frames of `w`×`h` take at this cost, in seconds.
    pub fn predict_secs(&self, frames: u64, (w, h): (u32, u32)) -> f64 {
        frames as f64 * self.ns_per_px * w as f64 * h as f64 / 1e9
    }
}

impl PreviewCosts {
    /// The composite of the aligned moment `t` at `w`×`h` took `ns` of device time.
    pub fn note(&mut self, t: Tick, ns: u64, (w, h): (u32, u32)) {
        if ns == 0 || w == 0 || h == 0 {
            return;
        }
        self.seq += 1;
        let k = (t.0, w, h);
        if self.at.insert(k, Sample { seq: self.seq, ns }).is_none() {
            self.order.push_back(k);
            if self.order.len() > MAX_MOMENTS
                && let Some(old) = self.order.pop_front()
            {
                self.at.remove(&old);
            }
        }
    }

    /// Every measured moment's device ns at `w`×`h` (the disk prerender's choice, render-cache-disk D4):
    /// its time at that size, else its time at the size nearest by pixels, scaled by them
    /// (persistent-cache D8) — a moment light at a small preview is not light at a large one.
    pub fn at_size(&self, (w, h): (u32, u32)) -> BTreeMap<i64, u64> {
        let want = (w as f64 * h as f64).max(1.0);
        let mut out: BTreeMap<i64, u64> = BTreeMap::new();
        let mut best = (i64::MIN, f64::MAX);
        for (&(t, sw, sh), s) in &self.at {
            let px = sw as f64 * sh as f64;
            let far = if (sw, sh) == (w, h) { -1.0 } else { (px / want).ln().abs() };
            if best.0 == t && far >= best.1 {
                continue;
            }
            best = (t, far);
            let ns = if far < 0.0 { s.ns } else { (s.ns as f64 * want / px) as u64 };
            out.insert(t, ns);
        }
        out
    }

    /// The cost over `[a, b)`, or `None` when no moment there was measured.
    pub fn over(&self, a: Tick, b: Tick) -> Option<PreviewCost> {
        if b.0 <= a.0 {
            return None;
        }
        let (mut moments, mut ns, mut px) = (0u64, 0f64, 0f64);
        let mut newest: Option<(u64, (u32, u32))> = None;
        // the newest record of each moment, whatever its size
        let mut last: Option<(i64, (u32, u32), Sample)> = None;
        let mut take = |m: Option<(i64, (u32, u32), Sample)>| {
            let Some((_, size, s)) = m else { return };
            moments += 1;
            ns += s.ns as f64;
            px += size.0 as f64 * size.1 as f64;
            if newest.is_none_or(|(n, _)| s.seq >= n) {
                newest = Some((s.seq, size));
            }
        };
        for (&(t, w, h), s) in self.at.range((a.0, 0, 0)..(b.0, 0, 0)) {
            match &mut last {
                Some(l) if l.0 == t => {
                    if s.seq > l.2.seq {
                        *l = (t, (w, h), *s);
                    }
                }
                _ => take(last.replace((t, (w, h), *s))),
            }
        }
        take(last);
        let (_, size) = newest?;
        Some(PreviewCost { moments, ns_per_px: ns / px, size })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t2902 (render-honest D2): the range's cost is the newest records' device time over their
    /// pixels, with how many moments were measured and the preview's size; a moment outside the
    /// range does not count, and a range with no record has no cost.
    #[test]
    fn t2902_the_previews_costs_over_a_range() {
        let mut c = PreviewCosts::default();
        let step = 3600; // 25 fps
        c.note(Tick(0), 9_000_000, (640, 360));
        c.note(Tick(step), 2_000_000, (640, 360));
        c.note(Tick(2 * step), 4_000_000, (640, 360));
        c.note(Tick(10 * step), 50_000_000, (640, 360));
        // the newest record of a moment replaces the older one
        c.note(Tick(0), 3_000_000, (640, 360));
        let got = c.over(Tick(0), Tick(3 * step)).expect("measured");
        assert_eq!((got.moments, got.size), (3, (640, 360)));
        let px = 640.0 * 360.0;
        assert!((got.ns_per_px - 9_000_000.0 / (3.0 * px)).abs() < 1e-9, "{got:?}");
        // 75 frames at 1920×1080: nine times the pixels of a 3 ms frame
        assert!((got.predict_secs(75, (1920, 1080)) - 75.0 * 0.003 * 9.0).abs() < 1e-6);
        assert_eq!(c.over(Tick(3 * step), Tick(9 * step)), None, "nothing measured there");
        assert_eq!(c.over(Tick(5), Tick(5)), None);
        // another size of a moment: the newer of the two counts, once
        c.note(Tick(step), 10_000_000, (1920, 1080));
        let got = c.over(Tick(0), Tick(3 * step)).expect("measured");
        assert_eq!((got.moments, got.size), (3, (1920, 1080)));
        // a zero measurement (no timer yet) is no record
        c.note(Tick(4 * step), 0, (640, 360));
        assert_eq!(c.over(Tick(4 * step), Tick(5 * step)), None);
    }

    /// A moment is judged at the size it is asked at (persistent-cache D8): its own time there, else the
    /// nearest size's scaled by pixels.
    #[test]
    fn a_moment_is_judged_at_its_size() {
        let mut c = PreviewCosts::default();
        c.note(Tick(0), 2_000_000, (640, 360));
        c.note(Tick(10), 2_000_000, (640, 360));
        c.note(Tick(10), 50_000_000, (1920, 1080));
        let big = c.at_size((1920, 1080));
        assert_eq!((big[&0], big[&10]), (18_000_000, 50_000_000), "scaled by pixels, and measured");
        let small = c.at_size((640, 360));
        assert_eq!((small[&0], small[&10]), (2_000_000, 2_000_000), "the larger size's time does not replace it");
        // the nearest of two sizes answers for a third
        assert_eq!(c.at_size((1280, 720))[&10], 50_000_000 * 4 / 9);
    }
}
