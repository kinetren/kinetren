//! Modulation audio effects as plugins (plugin-api-stage1, stage 3): tremolo, auto_pan, vibrato,
//! chorus, flanger, phaser. Each node is the R.47 node of kt-fx moved verbatim onto
//! `kt_plugin_api::dsp`; its passport repeats the enum variant's fields in their order, types and
//! defaults (t973 holds the bytes of the file, t973c the bytes of the sound). The tail formulas are
//! the manifests' declarations — the static `chain_tail` reads them, t322 compares them with the
//! built states.

use std::sync::Arc;

use kt_plugin_api::dsp::{AUDIO_SAMPLE_RATE, CH, DelayLine, SmoothedParam, Tick, lfo_phase, tail_frames_for};
use kt_plugin_api::{
    AudioKernel, AudioState, Decl, HostOpts, Manifest, Media, ParamKind, ParamSpec, ParamUnit,
    ParamValues, Plugin, RowDecl, SlotCurves, Store, UiDecl,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(Tremolo::new()),
        Arc::new(AutoPan::new()),
        Arc::new(Vibrato::new()),
        Arc::new(Chorus::new()),
        Arc::new(Flanger::new()),
        Arc::new(Phaser::new()),
    ]
}

/// An `f32` row, always written (the modulation variants skipped no field).
fn f32_row() -> RowDecl {
    RowDecl { store: Store::F32, omit_default: false }
}

/// A `u32` row, always written.
fn u32_row() -> RowDecl {
    RowDecl { store: Store::U32, omit_default: false }
}

fn smooth(n: &'static str, lo: f64, hi: f64, d: f64, st: f64, u: ParamUnit) -> ParamSpec {
    ParamSpec::num(n, lo, hi, d, st, u, ParamKind::Smooth)
}

fn manifest(id: &'static str, passport: Vec<ParamSpec>, rows: Vec<RowDecl>, name: &'static str, kw: &'static str, decl: Decl) -> Manifest {
    Manifest {
        id: id.into(),
        media: Media::AudioFx,
        passport,
        rows,
        decl,
        ui: UiDecl { name, category_key: "fxreg.cat.modulation", keywords_key: kw },
    }
}

macro_rules! plugin_boilerplate {
    ($t:ident) => {
        impl Default for $t {
            fn default() -> $t {
                $t::new()
            }
        }

        impl Plugin for $t {
            fn manifest(&self) -> &Manifest {
                &self.m
            }
            fn audio(&self) -> Option<&dyn AudioKernel> {
                Some(self)
            }
        }
    };
}

/// The common LFO half of the modulation nodes: the phase is the integral of the frequency curve
/// over the clip position (`lfo_phase`), NOT a counter — seek/seam/wrap give the
/// same phase as a cold render (invariant 11). The node holds a COPY of the points of the
/// rate_hz curve (bind on apply_params/set_auto): the model is not
/// read in process (invariant 2).
struct LfoDrive {
    base_hz: f64,
    points: Vec<(f64, f64)>,
    clip_len: i64,
}

impl LfoDrive {
    fn new(base_hz: f64) -> LfoDrive {
        LfoDrive { base_hz, points: Vec::new(), clip_len: 1 }
    }

    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        self.points = c.points("rate_hz").to_vec();
        self.clip_len = clip_len.max(1);
    }

    /// Phase in radians at frame `p` (clip-local).
    #[inline]
    fn phase(&self, p: i64) -> f64 {
        let tau = p as f64 / self.clip_len as f64;
        lfo_phase(self.base_hz, &self.points, tau, self.clip_len) * std::f64::consts::TAU
    }
}

// ---------------------------------------------------------------------------
// tremolo
// ---------------------------------------------------------------------------

/// Tremolo (R.47): volume LFO.
pub struct Tremolo {
    m: Manifest,
}

impl Tremolo {
    pub fn new() -> Tremolo {
        Tremolo {
            m: manifest(
                "tremolo",
                vec![smooth("rate_hz", 0.1, 20.0, 5.0, 0.01, ParamUnit::Hz), smooth("depth", 0.0, 1.0, 0.6, 0.01, ParamUnit::Norm)],
                vec![f32_row(), f32_row()],
                "Tremolo",
                "fxreg.kw.tremolo",
                Decl { state: true, ..Decl::default() },
            ),
        }
    }
}

plugin_boilerplate!(Tremolo);

impl AudioKernel for Tremolo {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(TremoloNode { lfo: LfoDrive::new(5.0), depth: SmoothedParam::with_ms(0.6, 20.0) })
    }
}

/// Tremolo: g = 1 − depth·(0.5 + 0.5·sin φ).
struct TremoloNode {
    lfo: LfoDrive,
    depth: SmoothedParam,
}

impl AudioState for TremoloNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (rate_hz, depth) = (k[0] as f32, k[1] as f32);
        self.lfo.base_hz = rate_hz as f64;
        self.depth.set_target(depth.clamp(0.0, 1.0));
    }

    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        self.lfo.bind(c, clip_len);
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.depth.set_target(c.value("depth", self.depth.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth.tick().clamp(0.0, 1.0);
            let g = 1.0 - d * (0.5 + 0.5 * self.lfo.phase(pos + i as i64).sin() as f32);
            for s in fr.iter_mut() {
                *s *= g;
            }
        }
    }

    fn reset(&mut self) {
        self.depth.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// auto_pan
// ---------------------------------------------------------------------------

/// Autopan (R.47): balance LFO.
pub struct AutoPan {
    m: Manifest,
}

impl AutoPan {
    pub fn new() -> AutoPan {
        AutoPan {
            m: manifest(
                "auto_pan",
                vec![smooth("rate_hz", 0.05, 10.0, 1.0, 0.01, ParamUnit::Hz), smooth("depth", 0.0, 1.0, 0.8, 0.01, ParamUnit::Norm)],
                vec![f32_row(), f32_row()],
                "AutoPan",
                "fxreg.kw.autopan",
                Decl { state: true, ..Decl::default() },
            ),
        }
    }
}

plugin_boilerplate!(AutoPan);

impl AudioKernel for AutoPan {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(AutoPanNode { lfo: LfoDrive::new(1.0), depth: SmoothedParam::with_ms(0.8, 20.0) })
    }
}

/// Autopan: p = depth·sin φ, the pan law — as in Gain.
struct AutoPanNode {
    lfo: LfoDrive,
    depth: SmoothedParam,
}

impl AudioState for AutoPanNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (rate_hz, depth) = (k[0] as f32, k[1] as f32);
        self.lfo.base_hz = rate_hz as f64;
        self.depth.set_target(depth.clamp(0.0, 1.0));
    }

    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        self.lfo.bind(c, clip_len);
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.depth.set_target(c.value("depth", self.depth.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth.tick().clamp(0.0, 1.0);
            let p = d * self.lfo.phase(pos + i as i64).sin() as f32;
            fr[0] *= (1.0 - p).min(1.0);
            if CH > 1 {
                fr[1] *= (1.0 + p).min(1.0);
            }
        }
    }

    fn reset(&mut self) {
        self.depth.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// vibrato
// ---------------------------------------------------------------------------

/// Maximum vibrato delay: 1 ms base + 5 ms depth + margin.
const VIB_MAX_FRAMES: usize = (48_000.0 * 0.008) as usize;

fn vibrato_tail(_p: &ParamValues, _c: &dyn SlotCurves) -> i64 {
    VIB_MAX_FRAMES as i64
}

/// Vibrato (R.47): a modulated delay, 100 % wet.
pub struct Vibrato {
    m: Manifest,
}

impl Vibrato {
    pub fn new() -> Vibrato {
        Vibrato {
            m: manifest(
                "vibrato",
                vec![smooth("rate_hz", 0.1, 12.0, 5.0, 0.01, ParamUnit::Hz), smooth("depth_ms", 0.05, 5.0, 0.5, 0.05, ParamUnit::Ms)],
                vec![f32_row(), f32_row()],
                "Vibrato",
                "fxreg.kw.vibrato",
                Decl { state: true, tail: Some(vibrato_tail), ..Decl::default() },
            ),
        }
    }
}

plugin_boilerplate!(Vibrato);

impl AudioKernel for Vibrato {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(VibratoNode {
            lfo: LfoDrive::new(5.0),
            depth_ms: SmoothedParam::with_ms(0.5, 20.0),
            lines: std::array::from_fn(|_| DelayLine::new(VIB_MAX_FRAMES)),
        })
    }
}

/// Vibrato: 100 % wet, delay = 1 ms + depth·(0.5+0.5·sin φ).
struct VibratoNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    lines: [DelayLine; CH],
}

impl AudioState for VibratoNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (rate_hz, depth_ms) = (k[0] as f32, k[1] as f32);
        self.lfo.base_hz = rate_hz as f64;
        self.depth_ms.set_target(depth_ms.clamp(0.05, 5.0));
    }

    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        self.lfo.bind(c, clip_len);
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.depth_ms.set_target(c.value("depth_ms", self.depth_ms.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth_ms.tick().clamp(0.05, 5.0);
            let ph = self.lfo.phase(pos + i as i64).sin() as f32;
            let delay = (1.0 + d * (0.5 + 0.5 * ph)) * sr / 1000.0;
            for (ch, s) in fr.iter_mut().enumerate() {
                self.lines[ch].write(*s);
                *s = self.lines[ch].read_frac(delay);
            }
        }
    }

    fn tail(&self) -> Tick {
        Tick::from_audio_frames(VIB_MAX_FRAMES as i64)
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.reset();
        }
        self.depth_ms.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// chorus
// ---------------------------------------------------------------------------

const CHORUS_BASE_MS: f32 = 15.0;
const CHORUS_MAX_FRAMES: usize = (48_000.0 * 0.030) as usize;

fn chorus_tail(_p: &ParamValues, _c: &dyn SlotCurves) -> i64 {
    CHORUS_MAX_FRAMES as i64
}

/// Chorus (R.47): up to three modulated voices.
pub struct Chorus {
    m: Manifest,
}

impl Chorus {
    pub fn new() -> Chorus {
        Chorus {
            m: manifest(
                "chorus",
                vec![
                    smooth("rate_hz", 0.05, 8.0, 0.8, 0.01, ParamUnit::Hz),
                    smooth("depth_ms", 0.0, 12.0, 3.5, 0.1, ParamUnit::Ms),
                    smooth("mix", 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm),
                    // number of voices rebuilds the lines — not subject to a curve
                    ParamSpec::num("voices", 1.0, 3.0, 2.0, 1.0, ParamUnit::Count, ParamKind::Structural),
                ],
                vec![f32_row(), f32_row(), f32_row(), u32_row()],
                "Chorus",
                "fxreg.kw.chorus",
                Decl { state: true, tail: Some(chorus_tail), ..Decl::default() },
            ),
        }
    }
}

plugin_boilerplate!(Chorus);

impl AudioKernel for Chorus {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(ChorusNode {
            lfo: LfoDrive::new(0.8),
            depth_ms: SmoothedParam::with_ms(3.5, 20.0),
            mix: SmoothedParam::with_ms(0.5, 20.0),
            voices: 2,
            lines: std::array::from_fn(|_| DelayLine::new(CHORUS_MAX_FRAMES)),
        })
    }
}

/// Chorus: base 15 ms, voice k — phase +k·2π/voices, right channel +90°.
struct ChorusNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    mix: SmoothedParam,
    voices: u32,
    lines: [DelayLine; CH],
}

impl AudioState for ChorusNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (rate_hz, depth_ms, mix, voices) = (k[0] as f32, k[1] as f32, k[2] as f32, k[3] as u32);
        self.lfo.base_hz = rate_hz as f64;
        self.depth_ms.set_target(depth_ms.clamp(0.0, 12.0));
        self.mix.set_target(mix.clamp(0.0, 1.0));
        self.voices = voices.clamp(1, 3);
    }

    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        self.lfo.bind(c, clip_len);
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.depth_ms.set_target(c.value("depth_ms", self.depth_ms.target as f64, s) as f32);
        self.mix.set_target(c.value("mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        let nv = self.voices.max(1);
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth_ms.tick().clamp(0.0, 12.0);
            let k = self.mix.tick().clamp(0.0, 1.0);
            let phi = self.lfo.phase(pos + i as i64);
            for (ch, s) in fr.iter_mut().enumerate() {
                let dry = *s;
                self.lines[ch].write(dry);
                let mut wet = 0.0f32;
                for v in 0..nv {
                    let ph = phi
                        + v as f64 * std::f64::consts::TAU / nv as f64
                        + if ch == 1 { std::f64::consts::FRAC_PI_2 } else { 0.0 };
                    let delay = (CHORUS_BASE_MS + d * (0.5 + 0.5 * ph.sin() as f32)) * sr / 1000.0;
                    wet += self.lines[ch].read_frac(delay);
                }
                wet /= nv as f32;
                *s = dry * (1.0 - k) + wet * k;
            }
        }
    }

    fn tail(&self) -> Tick {
        Tick::from_audio_frames(CHORUS_MAX_FRAMES as i64)
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.reset();
        }
        self.depth_ms.reset_to_target();
        self.mix.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// flanger
// ---------------------------------------------------------------------------

const FLANGER_BASE_MS: f32 = 0.5;
const FLANGER_MAX_FRAMES: usize = (48_000.0 * 0.010) as usize;

fn flanger_tail(_p: &ParamValues, _c: &dyn SlotCurves) -> i64 {
    tail_frames_for(FLANGER_MAX_FRAMES as f32, 0.9).to_audio_frames()
}

/// Flanger (R.47): a short modulated delay with feedback.
pub struct Flanger {
    m: Manifest,
}

impl Flanger {
    pub fn new() -> Flanger {
        Flanger {
            m: manifest(
                "flanger",
                vec![
                    smooth("rate_hz", 0.05, 4.0, 0.25, 0.01, ParamUnit::Hz),
                    smooth("depth_ms", 0.1, 8.0, 2.0, 0.1, ParamUnit::Ms),
                    smooth("feedback", -0.9, 0.9, 0.5, 0.01, ParamUnit::Norm),
                    smooth("mix", 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm),
                ],
                vec![f32_row(), f32_row(), f32_row(), f32_row()],
                "Flanger",
                "fxreg.kw.flanger",
                Decl { state: true, tail: Some(flanger_tail), ..Decl::default() },
            ),
        }
    }
}

plugin_boilerplate!(Flanger);

impl AudioKernel for Flanger {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(FlangerNode {
            lfo: LfoDrive::new(0.25),
            depth_ms: SmoothedParam::with_ms(2.0, 20.0),
            feedback: SmoothedParam::with_ms(0.5, 20.0),
            mix: SmoothedParam::with_ms(0.5, 20.0),
            lines: std::array::from_fn(|_| DelayLine::new(FLANGER_MAX_FRAMES)),
        })
    }
}

/// Flanger: base 0.5 ms, feedback into the ring (sign = phase inversion).
struct FlangerNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    lines: [DelayLine; CH],
}

impl AudioState for FlangerNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (rate_hz, depth_ms, feedback, mix) = (k[0] as f32, k[1] as f32, k[2] as f32, k[3] as f32);
        self.lfo.base_hz = rate_hz as f64;
        self.depth_ms.set_target(depth_ms.clamp(0.1, 8.0));
        self.feedback.set_target(feedback.clamp(-0.9, 0.9));
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        self.lfo.bind(c, clip_len);
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.depth_ms.set_target(c.value("depth_ms", self.depth_ms.target as f64, s) as f32);
        self.feedback.set_target(c.value("feedback", self.feedback.target as f64, s) as f32);
        self.mix.set_target(c.value("mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth_ms.tick().clamp(0.1, 8.0);
            let fb = self.feedback.tick().clamp(-0.9, 0.9);
            let k = self.mix.tick().clamp(0.0, 1.0);
            let ph = self.lfo.phase(pos + i as i64).sin() as f32;
            let delay = (FLANGER_BASE_MS + d * (0.5 + 0.5 * ph)) * sr / 1000.0;
            for (ch, s) in fr.iter_mut().enumerate() {
                let dry = *s;
                let wet = self.lines[ch].read_frac(delay);
                self.lines[ch].write(dry + wet * fb);
                *s = dry * (1.0 - k) + wet * k;
            }
        }
    }

    fn tail(&self) -> Tick {
        tail_frames_for(FLANGER_MAX_FRAMES as f32, 0.9)
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.reset();
        }
        self.depth_ms.reset_to_target();
        self.feedback.reset_to_target();
        self.mix.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// phaser
// ---------------------------------------------------------------------------

/// Phaser (R.47): a swept allpass cascade.
pub struct Phaser {
    m: Manifest,
}

impl Phaser {
    pub fn new() -> Phaser {
        Phaser {
            m: manifest(
                "phaser",
                vec![
                    smooth("rate_hz", 0.05, 4.0, 0.4, 0.01, ParamUnit::Hz),
                    smooth("depth", 0.0, 1.0, 0.7, 0.01, ParamUnit::Norm),
                    smooth("feedback", 0.0, 0.9, 0.3, 0.01, ParamUnit::Norm),
                    // number of cascades rebuilds the state — not subject to a curve
                    ParamSpec::num("stages", 2.0, 8.0, 4.0, 1.0, ParamUnit::Count, ParamKind::Structural),
                    smooth("mix", 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm),
                ],
                vec![f32_row(), f32_row(), f32_row(), u32_row(), f32_row()],
                "Phaser",
                "fxreg.kw.phaser",
                Decl { state: true, ..Decl::default() },
            ),
        }
    }
}

plugin_boilerplate!(Phaser);

impl AudioKernel for Phaser {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(PhaserNode {
            lfo: LfoDrive::new(0.4),
            depth: SmoothedParam::with_ms(0.7, 20.0),
            feedback: SmoothedParam::with_ms(0.3, 20.0),
            mix: SmoothedParam::with_ms(0.5, 20.0),
            stages: 4,
            z: [[0.0; 8]; CH],
            fb_state: [0.0; CH],
        })
    }
}

/// Phaser: a cascade of 1-pole allpass filters, center sweep 200..2000 Hz (exp).
struct PhaserNode {
    lfo: LfoDrive,
    depth: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    stages: u32,
    z: [[f32; 8]; CH],
    fb_state: [f32; CH],
}

impl AudioState for PhaserNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (rate_hz, depth, feedback, stages, mix) = (k[0] as f32, k[1] as f32, k[2] as f32, k[3] as u32, k[4] as f32);
        self.lfo.base_hz = rate_hz as f64;
        self.depth.set_target(depth.clamp(0.0, 1.0));
        self.feedback.set_target(feedback.clamp(0.0, 0.9));
        self.mix.set_target(mix.clamp(0.0, 1.0));
        self.stages = stages.clamp(2, 8);
    }

    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        self.lfo.bind(c, clip_len);
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.depth.set_target(c.value("depth", self.depth.target as f64, s) as f32);
        self.feedback.set_target(c.value("feedback", self.feedback.target as f64, s) as f32);
        self.mix.set_target(c.value("mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        let n = self.stages.clamp(2, 8) as usize;
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth.tick().clamp(0.0, 1.0);
            let fb = self.feedback.tick().clamp(0.0, 0.9);
            let k = self.mix.tick().clamp(0.0, 1.0);
            let ph = 0.5 + 0.5 * self.lfo.phase(pos + i as i64).sin() as f32;
            // center sweep: 200..2000 Hz exp, depth compresses the range
            let f = 200.0 * (10.0f32).powf(ph * d);
            let t = (std::f32::consts::PI * f / sr).tan();
            let ap = (t - 1.0) / (t + 1.0);
            for (ch, s) in fr.iter_mut().enumerate() {
                let dry = *s;
                let mut x = dry + self.fb_state[ch] * fb;
                for st in 0..n {
                    // 1-pole allpass: y = a·x + z; z = x − a·y
                    let y = ap * x + self.z[ch][st];
                    self.z[ch][st] = x - ap * y;
                    x = y;
                }
                self.fb_state[ch] = x;
                *s = dry * (1.0 - k) + x * k;
            }
        }
    }

    fn reset(&mut self) {
        self.z = [[0.0; 8]; CH];
        self.fb_state = [0.0; CH];
        self.depth.reset_to_target();
        self.feedback.reset_to_target();
        self.mix.reset_to_target();
    }
}
