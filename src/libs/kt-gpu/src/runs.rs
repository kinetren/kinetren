//! Point-wise runs as one generated pass (fused-pointwise-runs): the text a run's shape compiles to,
//! and the trial that decides on the device whether a warm shape runs fused or as its parts.
//!
//! A run's SHAPE is the sequence of its stages' indices in `plugin_taps()`; its numbers (eight a
//! stage, what the stage's own pass reads from `P.f0`/`P.f1`) arrive in a 1024-byte uniform — the
//! geometry pass's layout, so 32 stages a pass at most. The pass loads the texel once, applies each
//! stage's `tap_<id>` in chain order and rounds and clamps after each one exactly as the stage's own
//! pass stores it, then stores once: the fused and the unfused forms are the same per-stage
//! operations on the same integers.

use std::collections::HashMap;

/// Stages a fused pass holds: two vec4 a stage in the 1024-byte uniform.
pub const MAX_RUN_STAGES: usize = crate::engine::GEOM_UNIFORM_BYTES / 32;

/// Tries of each form before the verdict; the minimum of each is compared (a reading on a loaded
/// machine moves by 2×: minima of repeats, interleaved).
pub(crate) const TRIES: usize = 5;

/// Requests of a trying shape after which, with too few samples back (the timer was full, the device
/// lost its timestamps), the shape stays fused: it moves fewer bytes on every device.
pub(crate) const TRY_LIMIT: u32 = 64;

/// The generated WGSL of a run of the point-wise plugins `ids` (in chain order); `taps` gives each
/// id's `tap_<id>` text. A pure function of the shape and the registry.
pub(crate) fn run_text(ids: &[&str], taps: &[(&'static str, &'static str)]) -> String {
    debug_assert!(ids.len() >= 2 && ids.len() <= MAX_RUN_STAGES, "a run holds 2..=MAX_RUN_STAGES stages");
    let mut s = String::from(
        "// A point-wise run (fused-pointwise-runs), generated: one load, the stages' taps with the\n\
         // rounding of their own passes after each, one store.\n\
         struct RunParams {\n    p: array<vec4<f32>, 64>,\n};\n\
         @group(0) @binding(0) var<uniform> R: RunParams;\n\
         @group(0) @binding(1) var T0: texture_2d<u32>;\n\
         @vertex\nfn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {\n    \
         let x = f32(i32(vi / 2u)) * 4.0 - 1.0;\n    let y = f32(i32(vi & 1u)) * 4.0 - 1.0;\n    \
         return vec4<f32>(x, y, 0.0, 1.0);\n}\n",
    );
    let mut seen: Vec<&str> = Vec::new();
    for id in ids {
        if !seen.contains(id) {
            seen.push(id);
            let (_, w) = taps.iter().find(|(t, _)| t == id).expect("a run of an unassembled tap");
            s.push_str(w);
            s.push('\n');
        }
    }
    s.push_str(
        "@fragment\nfn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {\n    \
         let t = textureLoad(T0, vec2<i32>(pos.xy), 0);\n    let c = floor(pos.xy);\n    \
         var v = vec4<f32>(f32(t.x), f32(t.y), f32(t.z), f32(t.w));\n",
    );
    for (k, id) in ids.iter().enumerate() {
        s.push_str(&format!(
            "    v = clamp(round(tap_{id}(v, R.p[{}], R.p[{}], c)), vec4<f32>(0.0), vec4<f32>(255.0));\n",
            2 * k,
            2 * k + 1
        ));
    }
    s.push_str("    return vec4<u32>(v);\n}\n");
    s
}

/// How an engine runs point-wise runs: as the device's trial decides (the product), always fused once a
/// shape is warm, or never (the reference form, for the tests and the instrument).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FuseRuns {
    #[default]
    Measured,
    Always,
    Never,
}

/// One stage of a run as the executor holds it: its own pass's index in `plugin_taps()` and the eight
/// numbers that pass reads (`P.f0`, `P.f1`); a positioned stage reads the pass's size in `b.zw`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stage {
    pub idx: usize,
    pub f: [f32; 8],
    pub positioned: bool,
}

/// The uniform of a fused piece: two vec4 a stage, a positioned stage's size filled in.
pub(crate) fn uniform(stages: &[Stage], w: u32, h: u32) -> [u8; crate::engine::GEOM_UNIFORM_BYTES] {
    let mut b = [0u8; crate::engine::GEOM_UNIFORM_BYTES];
    for (k, st) in stages.iter().enumerate() {
        let mut f = st.f;
        if st.positioned {
            f[6] = w as f32;
            f[7] = h as f32;
        }
        for (c, v) in f.iter().enumerate() {
            let o = k * 32 + c * 4;
            b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    b
}

/// What the trial decided for a warm shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Frames alternate between the fused pass and the own passes, each timed.
    Trying,
    Fused,
    /// The fused pass was slower than its parts here: the halves are shapes of their own.
    Split,
}

/// The trial of one warm shape: device time per pixel of the fused pass and of the sum of its parts.
#[derive(Debug)]
pub(crate) struct Trial {
    fused: Vec<f64>,
    own: Vec<f64>,
    asked: u32,
    pub verdict: Verdict,
}

impl Trial {
    /// A warm shape's trial; `timed` — the device has timestamps. Without them the fused pass stands.
    pub fn new(timed: bool) -> Trial {
        Trial { fused: Vec::new(), own: Vec::new(), asked: 0, verdict: if timed { Verdict::Trying } else { Verdict::Fused } }
    }

    /// A trying shape's next frame: `true` — fused. Interleaved, so a load that comes and goes falls
    /// on both forms alike.
    pub fn next_fused(&mut self) -> bool {
        self.asked += 1;
        if self.asked >= TRY_LIMIT && self.verdict == Verdict::Trying {
            self.verdict = Verdict::Fused;
        }
        self.asked % 2 == 1
    }

    /// A timed frame of the shape: ns per pixel of the fused pass, or of the stages' own passes summed.
    pub fn note(&mut self, fused: bool, ns_per_px: f64) {
        if self.verdict != Verdict::Trying {
            return;
        }
        if fused { &mut self.fused } else { &mut self.own }.push(ns_per_px);
        if self.fused.len() >= TRIES && self.own.len() >= TRIES {
            let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
            self.verdict = if min(&self.fused) <= min(&self.own) { Verdict::Fused } else { Verdict::Split };
        }
    }

    /// The minima so far (fused, own), for the trace and the instrument.
    pub fn minima(&self) -> (Option<f64>, Option<f64>) {
        let min = |v: &[f64]| v.iter().copied().reduce(f64::min);
        (min(&self.fused), min(&self.own))
    }
}

/// A frame's pass of a trying shape, for the timer (`pass_time`): which shape, which form, and the
/// pass's pixels. `piece` tells two pieces of one frame apart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Probe {
    pub shape: u32,
    pub fused: bool,
    pub piece: u32,
    pub px: u64,
}

/// A timed piece: its probe and the device time of its passes, summed.
pub(crate) type Sample = (Probe, u64);

/// Sum the passes of each probed piece of a frame: `(probe, ns)` per timed pass.
pub(crate) fn pieces(passes: &[(Probe, u64)]) -> Vec<Sample> {
    let mut by: HashMap<(u32, u32), (Probe, u64)> = HashMap::new();
    for (p, ns) in passes {
        by.entry((p.shape, p.piece)).and_modify(|e| e.1 += ns).or_insert((*p, *ns));
    }
    by.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t2193 (fused-pointwise-runs; capability `video-plan`): the switch is measured, never assumed.
    /// Fused tries slower than the parts split the shape; faster ones keep it; no timestamps — fused at
    /// once; a trial whose samples never come back ends fused; the generated text applies every stage
    /// in order with its own rounding.
    #[test]
    fn t2193_the_switch_is_measured_on_the_device() {
        // slower fused: split
        let mut t = Trial::new(true);
        assert_eq!(t.verdict, Verdict::Trying);
        let forms: Vec<bool> = (0..4).map(|_| t.next_fused()).collect();
        assert_eq!(forms, [true, false, true, false], "the tries interleave");
        for i in 0..TRIES {
            t.note(true, 3.0 + i as f64);
            assert_eq!(t.verdict, Verdict::Trying, "no verdict before both forms have their tries");
            t.note(false, 2.0 + i as f64);
        }
        assert_eq!(t.verdict, Verdict::Split, "a fused pass slower than its parts is split");
        t.note(true, 0.1);
        assert_eq!(t.verdict, Verdict::Split, "a verdict stands");
        // faster fused (a slow outlier among the tries does not decide: minima)
        let mut t = Trial::new(true);
        for i in 0..TRIES {
            t.note(true, if i == 0 { 9.0 } else { 1.0 });
            t.note(false, 2.0);
        }
        assert_eq!(t.verdict, Verdict::Fused);
        assert_eq!(t.minima(), (Some(1.0), Some(2.0)));
        // no timestamps on the device
        assert_eq!(Trial::new(false).verdict, Verdict::Fused, "without a timer the fused pass stands");
        // samples never back
        let mut t = Trial::new(true);
        for _ in 0..TRY_LIMIT {
            t.next_fused();
        }
        assert_eq!(t.verdict, Verdict::Fused, "a trial without samples ends fused");
        // the pieces of a frame are summed per piece
        let p = |piece, fused| Probe { shape: 7, fused, piece, px: 100 };
        let mut s = pieces(&[(p(0, false), 10), (p(0, false), 15), (p(1, true), 7)]);
        s.sort_by_key(|(p, _)| p.piece);
        assert_eq!(s, vec![(p(0, false), 25), (p(1, true), 7)]);
        // the text: one load, the stages in order each rounded, one store
        let taps = [("a", "fn tap_a(v: vec4<f32>, a: vec4<f32>, b: vec4<f32>, c: vec2<f32>) -> vec4<f32> { return v; }"),
                    ("b", "fn tap_b(v: vec4<f32>, a: vec4<f32>, b: vec4<f32>, c: vec2<f32>) -> vec4<f32> { return v; }")];
        let text = run_text(&["b", "a", "b"], &taps);
        assert_eq!(text.matches("textureLoad").count(), 1, "one load");
        assert_eq!(text.matches("fn tap_b(").count(), 1, "a tap's text once however often it runs");
        let calls: Vec<&str> = text.lines().filter(|l| l.contains("= clamp(round(tap_")).collect();
        assert_eq!(calls.len(), 3);
        assert!(calls[0].contains("tap_b(v, R.p[0], R.p[1], c)") && calls[1].contains("tap_a(v, R.p[2], R.p[3], c)") && calls[2].contains("tap_b(v, R.p[4], R.p[5], c)"));
        assert_eq!(MAX_RUN_STAGES, 32);
    }
}
