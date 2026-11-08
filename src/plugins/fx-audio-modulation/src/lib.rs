//! Modulation audio effects as plugins (plugin-api-stage1, stage 3): tremolo, auto_pan, vibrato,
//! chorus, flanger, phaser, pitch_shift, delay. Each node is the R.47 node of kt-fx moved verbatim onto
//! `kt_plugin_api::dsp`; its passport repeats the enum variant's fields in their order, types and
//! defaults (t973 holds the bytes of the file, t973c the bytes of the sound). The tail formulas are
//! the manifests' declarations — the static `chain_tail` reads them, t322 compares them with the
//! built states.

use kt_plugin_api::math::FrozenMath;
use std::sync::Arc;

use kt_plugin_api::curves::ParamIntegral;
use kt_plugin_api::dsp::{AUDIO_SAMPLE_RATE, CH, DelayLine, Freeverb, SmoothedParam, Tick, lfo_phase_of, tail_frames_for};
use kt_plugin_api::{
    AudioKernel, AudioState, CurveRead, Decl, HostOpts, Manifest, Media, ParamKind, ParamSpec, ParamUnit,
    ParamValues, Plugin, RowDecl, SlotCurves, Store, TimeWarp, UiDecl,
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
        Arc::new(PitchShift::new()),
        Arc::new(Delay::new()),
        Arc::new(SlowedReverb::new()),
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
        ui: UiDecl { name, category_key: "fxreg.cat.modulation", keywords_key: kw, ..UiDecl::default() },
    }
}

/// The same manifest under another picker category.
fn in_category(mut m: Manifest, category_key: &'static str) -> Manifest {
    m.ui.category_key = category_key;
    m
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
#[derive(Clone)]
struct LfoDrive {
    base_hz: f64,
    /// the frequency curve compiled into its prefix integral at bind (compiled-curves): the phase is
    /// read per sample, and the former walk from the first point was O(points) a sample
    rate: ParamIntegral,
    clip_len: i64,
}

impl LfoDrive {
    fn new(base_hz: f64) -> LfoDrive {
        LfoDrive { base_hz, rate: ParamIntegral::default(), clip_len: 1 }
    }

    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        let points = c.points("rate_hz");
        if self.rate.points() != points {
            self.rate = ParamIntegral::of(points);
        }
        self.clip_len = clip_len.max(1);
    }

    /// Phase in radians at frame `p` (clip-local).
    #[inline]
    fn phase(&self, p: i64) -> f64 {
        let tau = p as f64 / self.clip_len as f64;
        lfo_phase_of(self.base_hz, &self.rate, tau, self.clip_len) * std::f64::consts::TAU
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
#[derive(Clone)]
struct TremoloNode {
    lfo: LfoDrive,
    depth: SmoothedParam,
}

impl AudioState for TremoloNode {
    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }
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
        // at the CPU's level, the same bytes (audio-fx-cost, `kt_plugin_api::cpu`)
        kt_plugin_api::fast!({
            for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
                let d = self.depth.tick().clamp(0.0, 1.0);
                let g = 1.0 - d * (0.5 + 0.5 * self.lfo.phase(pos + i as i64).fsin() as f32);
                for s in fr.iter_mut() {
                    *s *= g;
                }
            }
        })
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
#[derive(Clone)]
struct AutoPanNode {
    lfo: LfoDrive,
    depth: SmoothedParam,
}

impl AudioState for AutoPanNode {
    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }
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
        // at the CPU's level, the same bytes (audio-fx-cost, `kt_plugin_api::cpu`)
        kt_plugin_api::fast!({
            for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
                let d = self.depth.tick().clamp(0.0, 1.0);
                let p = d * self.lfo.phase(pos + i as i64).fsin() as f32;
                fr[0] *= (1.0 - p).min(1.0);
                if CH > 1 {
                    fr[1] *= (1.0 + p).min(1.0);
                }
            }
        })
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
#[derive(Clone)]
struct VibratoNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    lines: [DelayLine; CH],
}

impl AudioState for VibratoNode {
    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }
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
        // at the CPU's level, the same bytes (audio-fx-cost, `kt_plugin_api::cpu`)
        kt_plugin_api::fast!({
            let sr = AUDIO_SAMPLE_RATE as f32;
            for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
                let d = self.depth_ms.tick().clamp(0.05, 5.0);
                let ph = self.lfo.phase(pos + i as i64).fsin() as f32;
                let delay = (1.0 + d * (0.5 + 0.5 * ph)) * sr / 1000.0;
                for (ch, s) in fr.iter_mut().enumerate() {
                    self.lines[ch].write(*s);
                    *s = self.lines[ch].read_frac(delay);
                }
            }
        })
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
#[derive(Clone)]
struct ChorusNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    mix: SmoothedParam,
    voices: u32,
    lines: [DelayLine; CH],
}

impl AudioState for ChorusNode {
    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }
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
        // at the CPU's level, the same bytes (audio-fx-cost, `kt_plugin_api::cpu`)
        kt_plugin_api::fast!({
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
                        let delay = (CHORUS_BASE_MS + d * (0.5 + 0.5 * ph.fsin() as f32)) * sr / 1000.0;
                        wet += self.lines[ch].read_frac(delay);
                    }
                    wet /= nv as f32;
                    *s = dry * (1.0 - k) + wet * k;
                }
            }
        })
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
#[derive(Clone)]
struct FlangerNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    lines: [DelayLine; CH],
}

impl AudioState for FlangerNode {
    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }
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
        // at the CPU's level, the same bytes (audio-fx-cost, `kt_plugin_api::cpu`)
        kt_plugin_api::fast!({
            let sr = AUDIO_SAMPLE_RATE as f32;
            for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
                let d = self.depth_ms.tick().clamp(0.1, 8.0);
                let fb = self.feedback.tick().clamp(-0.9, 0.9);
                let k = self.mix.tick().clamp(0.0, 1.0);
                let ph = self.lfo.phase(pos + i as i64).fsin() as f32;
                let delay = (FLANGER_BASE_MS + d * (0.5 + 0.5 * ph)) * sr / 1000.0;
                for (ch, s) in fr.iter_mut().enumerate() {
                    let dry = *s;
                    let wet = self.lines[ch].read_frac(delay);
                    self.lines[ch].write(dry + wet * fb);
                    *s = dry * (1.0 - k) + wet * k;
                }
            }
        })
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
#[derive(Clone)]
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
    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }
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
        // at the CPU's level, the same bytes (audio-fx-cost, `kt_plugin_api::cpu`)
        kt_plugin_api::fast!({
            let sr = AUDIO_SAMPLE_RATE as f32;
            let n = self.stages.clamp(2, 8) as usize;
            for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
                let d = self.depth.tick().clamp(0.0, 1.0);
                let fb = self.feedback.tick().clamp(0.0, 0.9);
                let k = self.mix.tick().clamp(0.0, 1.0);
                let ph = 0.5 + 0.5 * self.lfo.phase(pos + i as i64).fsin() as f32;
                // center sweep: 200..2000 Hz exp, depth compresses the range
                let f = 200.0 * (10.0f32).fpowf(ph * d);
                let t = (std::f32::consts::PI * f / sr).ftan();
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
        })
    }

    fn reset(&mut self) {
        self.z = [[0.0; 8]; CH];
        self.fb_state = [0.0; CH];
        self.depth.reset_to_target();
        self.feedback.reset_to_target();
        self.mix.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// pitch_shift (R.48, item 339)
// ---------------------------------------------------------------------------

/// Pitch lookahead (R.48, item 339): a grain of a two-octave upshift reads the
/// input four times faster than the output, and by the end of the grain it needs material
/// `LG·4` frames ahead plus the WSOLA search window. The constant is ONE for both
/// engines: the vocoder needs less, the excess is simply history for it.
/// Less than `MIN_PREROLL_FRAMES` (4800) — the seek warm-up always fills
/// the ring with real material.
pub const PITCH_LOOKAHEAD_FRAMES: i64 = 4608;

/// The WSOLA synthesis grid step and the vocoder hop (absolute, from the clip zero —
/// therefore slicing the buffers does not affect the result).
const PITCH_HOP: i64 = 512;
/// WSOLA grain length (Hann window, 50 % overlap: the sum of windows ≡ 1).
const PITCH_GRAIN: usize = 1024;
/// Half-width of the WSOLA search and its step (candidates are enumerated in
/// ascending |off| — the tie-break is deterministic).
const PITCH_SEARCH: i64 = 256;
const PITCH_SEARCH_STEP: i64 = 8;
/// Frame size of the phase vocoder.
const PV_N: usize = 2048;
/// Rings: the input must cover [output − search, input + chunk].
const PITCH_RING: usize = 8192;
/// OLA accumulator: must hold the processor chunk (1024) PLUS the full
/// synthesis window (2048 for the vocoder) — otherwise a grain synthesized
/// ahead would overwrite frames not yet handed out, and the result would depend on the
/// slicing of the buffers (exactly what t338 caught on the 2048 ring).
const PITCH_ACC: usize = 4096;

/// Pitch neutral: both the base and the curves at zero. A neutral node does not touch
/// the buffer at all (byte-for-byte the previous sound) and requires no lookahead.
pub fn pitch_is_neutral(semitones: f32, cents: f32, st_points: &[(f64, f64)], ct_points: &[(f64, f64)]) -> bool {
    semitones == 0.0
        && cents == 0.0
        && st_points.iter().all(|(_, v)| *v == 0.0)
        && ct_points.iter().all(|(_, v)| *v == 0.0)
}

/// The manifest's look-ahead: for pitch the neutral depends on the CURVES — a "zero" base is not
/// neutral while a live curve is present.
fn pitch_lookahead(p: &ParamValues, c: &dyn SlotCurves) -> i64 {
    let neutral = pitch_is_neutral(p.get(0) as f32, p.get(1) as f32, c.points("semitones"), c.points("cents"));
    if neutral { 0 } else { PITCH_LOOKAHEAD_FRAMES }
}

/// The keys of the pitch's echo row (pitch-tail-choice D1): the echo of the effects standing before
/// the pitch is CUT at the event's end (the default) or RINGS on after it.
pub const ECHO_BEFORE: &[&str] = &["cut", "ring"];

/// Pitch change without a change of duration (R.48, item 339).
pub struct PitchShift {
    m: Manifest,
}

impl PitchShift {
    pub fn new() -> PitchShift {
        PitchShift {
            m: in_category(manifest(
                "pitch_shift",
                vec![
                    smooth("semitones", -24.0, 24.0, 0.0, 1.0, ParamUnit::St),
                    smooth("cents", -100.0, 100.0, 0.0, 1.0, ParamUnit::None),
                    // the echo of the effects before the pitch at the event's end (pitch-tail-choice)
                    ParamSpec::choice_row("echo_before", ECHO_BEFORE, 0),
                ],
                // both were `skip_serializing_if = "is_zero"`; the echo row is written only when it rings
                vec![
                    RowDecl { store: Store::F32, omit_default: true },
                    RowDecl { store: Store::F32, omit_default: true },
                    RowDecl { store: Store::Choice(ECHO_BEFORE), omit_default: true },
                ],
                "Pitch Shift",
                "fxreg.kw.pitch_shift",
                // a grain reads around its moment: at an event edge it needs the material the event was
                // cut from (pitch-event-edges D1)
                Decl {
                    state: true,
                    lookahead: Some(pitch_lookahead),
                    reads_across_edges: true,
                    echo_row: Some("echo_before"),
                    ..Decl::default()
                },
            ), "fxreg.cat.timestretch"),
        }
    }
}

plugin_boilerplate!(PitchShift);

impl AudioKernel for PitchShift {
    /// The engine is a PROJECT setting (`pitch_engine`: 0 — WSOLA, 1 — the vocoder); a change of
    /// it rebuilds the processor.
    fn build(&self, opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(PitchNode {
            vocoder: opts.get("pitch_engine") == Some(1.0),
            semitones: 0.0,
            cents: 0.0,
            st_points: Vec::new(),
            ct_points: Vec::new(),
            st_curve: None,
            ct_curve: None,
            clip_len: 1,
            la: 0,
            ring: vec![0.0; PITCH_RING * CH],
            written: i64::MIN,
            clip_next: i64::MIN,
            deltas: Vec::new(),
            acc: vec![0.0; PITCH_ACC * CH],
            next_grain: 0,
            pv: None,
        })
    }
}

/// Phase vocoder state: FFT plans and phase accumulators.
#[derive(Clone)]
struct PvState {
    fwd: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    inv: std::sync::Arc<dyn realfft::ComplexToReal<f32>>,
    /// Phases per CHANNEL: the stereo sides must not interfere.
    prev_phase: [Vec<f32>; CH],
    synth_phase: [Vec<f32>; CH],
    window: Vec<f32>,
    time: Vec<f32>,
    spec: Vec<realfft::num_complex::Complex<f32>>,
    out_spec: Vec<realfft::num_complex::Complex<f32>>,
    mag: Vec<f32>,
    freq: Vec<f32>,
}

impl PvState {
    fn new() -> PvState {
        let mut planner = realfft::RealFftPlanner::<f32>::new();
        let fwd = planner.plan_fft_forward(PV_N);
        let inv = planner.plan_fft_inverse(PV_N);
        let bins = PV_N / 2 + 1;
        PvState {
            window: (0..PV_N)
                .map(|i| {
                    let x = std::f32::consts::TAU * i as f32 / PV_N as f32;
                    0.5 - 0.5 * x.fcos()
                })
                .collect(),
            prev_phase: std::array::from_fn(|_| vec![0.0; bins]),
            synth_phase: std::array::from_fn(|_| vec![0.0; bins]),
            time: vec![0.0; PV_N],
            spec: fwd.make_output_vec(),
            out_spec: inv.make_input_vec(),
            mag: vec![0.0; bins],
            freq: vec![0.0; bins],
            fwd,
            inv,
        }
    }

    fn reset(&mut self) {
        for p in &mut self.prev_phase {
            p.fill(0.0);
        }
        for p in &mut self.synth_phase {
            p.fill(0.0);
        }
    }
}

/// Pitch change without a change of duration (R.48, item 339). A LOOKAHEAD node:
/// it reads the input with a lookahead of `PITCH_LOOKAHEAD_FRAMES` and gives its output
/// aligned — there is no delay for the engine (item 337).
///
/// Both engines synthesize on the ABSOLUTE grid of the STREAM position: grain
/// number g always covers the output frames [g·HOP, g·HOP + GRAIN), and the
/// pitch coefficient is taken by the grain's CLIP position — therefore the result depends
/// neither on the slicing of the buffers nor on the number of calls (class t325a).
///
/// The stream position (pitch-loop-silence, D1) is set from the clip position at a real
/// seek and then only advances: a SEAM (a loop wrap, a clip moved during playback) keeps the
/// ring, the accumulator and the grid, exactly as the reverb keeps its tail. Until the first seam
/// the stream position IS the clip position, so every render without a seam is the former one
/// bit for bit (t338).
#[derive(Clone)]
struct PitchNode {
    vocoder: bool,
    semitones: f32,
    cents: f32,
    /// The curves' points: the neutral test reads them.
    st_points: Vec<(f64, f64)>,
    ct_points: Vec<(f64, f64)>,
    /// The curves read BY POSITION, with their segment shapes (R.61): the host's reader, so the
    /// node sees the same row of shapes as the rest of the program. `None` — not bound (the base).
    st_curve: Option<Box<dyn CurveRead>>,
    ct_curve: Option<Box<dyn CurveRead>>,
    clip_len: i64,
    /// 0 — neutral (the node is transparent).
    la: i64,
    /// Input ring by absolute STREAM positions.
    ring: Vec<f32>,
    /// Stream position of the next recording frame; `i64::MIN` — the stream has not yet existed.
    written: i64,
    /// The clip position the next input frame is expected at: another one is a seam.
    clip_next: i64,
    /// `(stream position, clip − stream)` from that stream position on: the clip position of a
    /// grain, by which its pitch curve is read. One entry per seam inside the look-ahead window.
    deltas: Vec<(i64, i64)>,
    /// The output OLA accumulator (also by absolute stream positions).
    acc: Vec<f32>,
    /// Number of the next grain to synthesize.
    next_grain: i64,
    pv: Option<Box<PvState>>,
}

impl PitchNode {
    fn refresh_la(&mut self) {
        let neutral = pitch_is_neutral(self.semitones, self.cents, &self.st_points, &self.ct_points);
        let la = if neutral { 0 } else { PITCH_LOOKAHEAD_FRAMES };
        if la != self.la {
            self.la = la;
            self.reset_state();
        }
        if la > 0 && self.vocoder && self.pv.is_none() {
            self.pv = Some(Box::new(PvState::new()));
        }
    }

    /// Pitch coefficient at STREAM position `s`: the curves are read at its clip position.
    fn ratio_at(&self, s: i64) -> f64 {
        let delta = self.deltas.iter().rev().find(|(from, _)| *from <= s).map_or(0, |(_, d)| *d);
        let p = s + delta;
        let tau = p as f64 / self.clip_len as f64;
        let read = |c: &Option<Box<dyn CurveRead>>, base: f32| c.as_ref().map_or(base as f64, |c| c.value(base as f64, tau));
        let st = read(&self.st_curve, self.semitones);
        let ct = read(&self.ct_curve, self.cents);
        (2f64).fpowf((st + ct / 100.0) / 12.0).clamp(0.25, 4.0)
    }

    #[inline]
    fn ring_at(&self, frame: i64, ch: usize) -> f32 {
        let i = frame.rem_euclid(PITCH_RING as i64) as usize;
        self.ring[i * CH + ch]
    }

    /// Linear interpolation of the input at a fractional position.
    #[inline]
    fn ring_frac(&self, pos: f64, ch: usize) -> f32 {
        let i = pos.floor();
        let f = (pos - i) as f32;
        let a = self.ring_at(i as i64, ch);
        let b = self.ring_at(i as i64 + 1, ch);
        a * (1.0 - f) + b * f
    }

    #[inline]
    fn acc_at(&mut self, frame: i64, ch: usize) -> &mut f32 {
        let i = frame.rem_euclid(PITCH_ACC as i64) as usize;
        &mut self.acc[i * CH + ch]
    }

    /// Start of a stream (a cold start or a real seek, after `reset`): the rings and the grid
    /// begin at the clip position, which the stream position equals from here on.
    fn restart(&mut self, in_pos: i64) {
        self.ring.fill(0.0);
        self.acc.fill(0.0);
        self.written = in_pos;
        self.clip_next = in_pos;
        self.deltas.clear();
        self.deltas.push((i64::MIN, 0));
        self.next_grain = (in_pos - self.la).div_euclid(PITCH_HOP);
        if let Some(pv) = &mut self.pv {
            pv.reset();
        }
    }

    /// WSOLA grain: reads the input at speed `r` from the grain position and
    /// mixes into the accumulator with a Hann window. The read offset is chosen
    /// by cross-correlation with the already synthesized overlap — this is what
    /// preserves the shape of the strike (unlike the vocoder).
    fn grain_wsola(&mut self, g: i64) {
        let start = g * PITCH_HOP;
        let r = self.ratio_at(start);
        let off = self.best_offset(start, r);
        let base = (start + off) as f64;
        for k in 0..PITCH_GRAIN {
            let w = hann(k, PITCH_GRAIN);
            let src = base + k as f64 * r;
            for ch in 0..CH {
                let v = self.ring_frac(src, ch) * w;
                *self.acc_at(start + k as i64, ch) += v;
            }
        }
    }

    /// WSOLA search: candidates are enumerated in ascending |off|, the strictly
    /// greater score wins — so on a tie the smaller shift remains
    /// (determinism without dependence on the traversal order).
    fn best_offset(&self, start: i64, r: f64) -> i64 {
        let mut cands: Vec<i64> = vec![0];
        let mut d = PITCH_SEARCH_STEP;
        while d <= PITCH_SEARCH {
            cands.push(-d);
            cands.push(d);
            d += PITCH_SEARCH_STEP;
        }
        let overlap = PITCH_HOP as usize;
        let mut best = 0i64;
        let mut best_score = f64::NEG_INFINITY;
        for off in cands {
            let base = (start + off) as f64;
            let (mut num, mut den) = (0f64, 0f64);
            for k in (0..overlap).step_by(2) {
                let src = base + k as f64 * r;
                let cand = (self.ring_frac(src, 0) + self.ring_frac(src, 1)) as f64;
                let i = (start + k as i64).rem_euclid(PITCH_ACC as i64) as usize;
                let prev = (self.acc[i * CH] + self.acc[i * CH + 1]) as f64;
                num += cand * prev;
                den += cand * cand;
            }
            let score = num / (den.sqrt() + 1e-9);
            if score > best_score {
                best_score = score;
                best = off;
            }
        }
        best
    }

    /// Phase vocoder grain: the frame spectrum is carried into the bins `k·r` with a
    /// recomputation of the phase from the instantaneous frequency. Harmonics travel cleanly;
    /// the transient is smeared — the price of the spectral method.
    fn grain_vocoder(&mut self, g: i64) {
        let start = g * PITCH_HOP;
        let r = self.ratio_at(start) as f32;
        let bins = PV_N / 2 + 1;
        let hop = PITCH_HOP as f32;
        let expected = std::f32::consts::TAU * hop / PV_N as f32;
        for ch in 0..CH {
            {
                let pv = self.pv.as_deref_mut().expect("vocoder state");
                for i in 0..PV_N {
                    pv.time[i] = 0.0;
                }
            }
            // analysis: the window from the ring
            for i in 0..PV_N {
                let v = self.ring_at(start + i as i64, ch);
                let pv = self.pv.as_deref_mut().expect("vocoder state");
                pv.time[i] = v * pv.window[i];
            }
            let pv = self.pv.as_deref_mut().expect("vocoder state");
            pv.fwd.process(&mut pv.time, &mut pv.spec).expect("fft");
            // Analysis: the magnitude and the INSTANTANEOUS frequency of each bin
            // (the principal deviation of the phase against the expected one per hop).
            for k in 0..bins {
                let c = pv.spec[k];
                let m = (c.re * c.re + c.im * c.im).sqrt();
                let phase = c.im.fatan2(c.re);
                let mut dp = phase - pv.prev_phase[ch][k] - k as f32 * expected;
                dp -= std::f32::consts::TAU * (dp / std::f32::consts::TAU).round();
                pv.prev_phase[ch][k] = phase;
                pv.mag[k] = m;
                pv.freq[k] = k as f32 * expected + dp;
            }
            // Synthesis by the INVERSE mapping (as in video geometry):
            // the target bin j asks the source at position j/r and gets the
            // single frequency — with a direct carry k → round(k·r) several sources would
            // share one bin, and the pitch drifted
            // (measured: 612 Hz instead of 659 at +7 semitones).
            for j in 0..bins {
                let src = j as f32 / r;
                let k0 = src.floor() as usize;
                if k0 + 1 >= bins {
                    pv.out_spec[j] = realfft::num_complex::Complex::new(0.0, 0.0);
                    continue;
                }
                let f = src - k0 as f32;
                let m = pv.mag[k0] * (1.0 - f) + pv.mag[k0 + 1] * f;
                let w = (pv.freq[k0] * (1.0 - f) + pv.freq[k0 + 1] * f) * r;
                pv.synth_phase[ch][j] += w;
                let (sn, cs) = pv.synth_phase[ch][j].fsin_cos();
                pv.out_spec[j] = realfft::num_complex::Complex::new(m * cs, m * sn);
            }
            // DC and Nyquist of a real signal are real — realfft checks this and otherwise
            // refuses to compute
            pv.out_spec[0].im = 0.0;
            let last = bins - 1;
            pv.out_spec[last].im = 0.0;
            pv.inv.process(&mut pv.out_spec, &mut pv.time).expect("ifft");
            let norm = 1.0 / PV_N as f32;
            for i in 0..PV_N {
                pv.time[i] *= pv.window[i] * norm;
            }
            for i in 0..PV_N {
                let v = self.pv.as_deref().expect("vocoder state").time[i];
                *self.acc_at(start + i as i64, ch) += v;
            }
        }
    }

    fn reset_state(&mut self) {
        self.ring.fill(0.0);
        self.acc.fill(0.0);
        self.written = i64::MIN;
        self.clip_next = i64::MIN;
        self.deltas.clear();
        self.next_grain = 0;
        if let Some(pv) = &mut self.pv {
            pv.reset();
        }
    }
}

impl AudioState for PitchNode {
    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (semitones, cents) = (k[0] as f32, k[1] as f32);
        self.semitones = semitones.clamp(-24.0, 24.0);
        self.cents = cents.clamp(-100.0, 100.0);
        self.refresh_la();
    }

    /// Pitch curves (the same pattern as the LFO nodes): the value is taken
    /// by the GRAIN position, not by the smoothing ticks — otherwise the result would depend
    /// on the number of calls.
    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        self.st_points = c.points("semitones").to_vec();
        self.ct_points = c.points("cents").to_vec();
        self.st_curve = Some(c.owned("semitones"));
        self.ct_curve = Some(c.owned("cents"));
        self.clip_len = clip_len.max(1);
        self.refresh_la();
    }

    // pitch reads curves BY GRAIN POSITION (bind), it does not need targets

    fn process(&mut self, buf: &mut [f32], in_pos: i64, _key: Option<&[f32]>) {
        // at the CPU's level, the same bytes (audio-fx-cost, `kt_plugin_api::cpu`)
        kt_plugin_api::fast!({
            if self.la == 0 {
                return; // neutral: do not touch the buffer at all
            }
            let n = (buf.len() / CH) as i64;
            if self.written == i64::MIN {
                self.restart(in_pos);
            } else if self.clip_next != in_pos {
                // a SEAM: the stream continues, only the clip position under it jumps
                self.deltas.push((self.written, in_pos - self.written));
                // the curve lookup needs the entries back to the oldest grain still to be
                // synthesized (the output side, `la` + a vocoder frame behind the input)
                let keep_from = self.written - self.la - PV_N as i64;
                while self.deltas.len() > 1 && self.deltas[1].0 <= keep_from {
                    self.deltas.remove(0);
                }
            }
            self.clip_next = in_pos + n;
            let in_pos = self.written;
            // 1) input into the ring
            for (i, fr) in buf.chunks_exact(CH).enumerate() {
                let idx = (in_pos + i as i64).rem_euclid(PITCH_RING as i64) as usize;
                for (ch, v) in fr.iter().enumerate() {
                    self.ring[idx * CH + ch] = *v;
                }
            }
            self.written = in_pos + n;
            // 2) synthesis of the grains covering the needed output
            let out_pos = in_pos - self.la;
            let out_end = out_pos + n;
            while self.next_grain * PITCH_HOP < out_end {
                let g = self.next_grain;
                if self.vocoder {
                    self.grain_vocoder(g)
                } else {
                    self.grain_wsola(g)
                }
                self.next_grain += 1;
            }
            // 3) output from the accumulator (what was read is zeroed — the ring
            //    is reused by the next grains)
            for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
                let t = out_pos + i as i64;
                for (ch, v) in fr.iter_mut().enumerate() {
                    let a = self.acc_at(t, ch);
                    *v = *a;
                    *a = 0.0;
                }
            }
        })
    }

    fn lookahead(&self) -> i64 {
        self.la
    }

    fn reset(&mut self) {
        self.reset_state();
    }
}

/// Hann window with 50 % overlap: the sum of two adjacent windows ≡ 1.
#[inline]
fn hann(i: usize, n: usize) -> f32 {
    let x = std::f32::consts::TAU * i as f32 / n as f32;
    0.5 - 0.5 * x.fcos()
}

// ---------------------------------------------------------------------------
// delay (R.47, item 329): three numbers and a flag row `pingpong`
// ---------------------------------------------------------------------------

const DELAY_MAX_MS: f32 = 1500.0;
const DELAY_MAX_FRAMES: usize = (48_000.0 * (DELAY_MAX_MS / 1000.0)) as usize + 8;

/// The echo's tail: t60 of the decaying repeats, while the echo is heard — its `mix` above zero or
/// its OWN mix curve above zero somewhere. (Before stage 3 the static formula read the mix curve of
/// the chain's Slowed+Reverb slot instead of the echo's: a delay at mix 0 with a rising mix curve
/// was given no tail, and a neighbour's curve gave a silent delay one — t990.)
fn delay_tail(p: &ParamValues, c: &dyn SlotCurves) -> i64 {
    let (time_ms, feedback, mix) = (p.get(0) as f32, p.get(1) as f32, p.get(2) as f32);
    if !(mix > 0.0 || c.points("mix").iter().any(|(_, v)| *v > 0.0)) {
        return 0;
    }
    tail_frames_for(time_ms.clamp(1.0, 1500.0) / 1000.0 * 48_000.0, feedback.clamp(0.01, 0.9)).to_audio_frames()
}

/// Echo (R.47): repeats with feedback, a darkening loop filter and ping-pong.
pub struct Delay {
    m: Manifest,
}

impl Delay {
    pub fn new() -> Delay {
        Delay {
            m: manifest(
                "delay",
                vec![
                    smooth("time_ms", 1.0, 1500.0, 350.0, 1.0, ParamUnit::Ms),
                    smooth("feedback", 0.0, 0.9, 0.35, 0.01, ParamUnit::Norm),
                    smooth("mix", 0.0, 1.0, 0.35, 0.01, ParamUnit::Norm),
                    // `#[serde(default, skip_serializing_if = "Not::not")] pingpong: bool`
                    ParamSpec::flag_row("pingpong", false),
                ],
                vec![f32_row(), f32_row(), f32_row(), RowDecl { store: Store::Bool, omit_default: true }],
                "Delay",
                "fxreg.kw.delay",
                Decl { state: true, tail: Some(delay_tail), ..Decl::default() },
            ),
        }
    }
}

plugin_boilerplate!(Delay);

impl AudioKernel for Delay {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(DelayNode {
            time_ms: SmoothedParam::with_ms(350.0, 100.0),
            feedback: SmoothedParam::with_ms(0.35, 20.0),
            mix: SmoothedParam::with_ms(0.35, 20.0),
            pingpong: false,
            lines: std::array::from_fn(|_| DelayLine::new(DELAY_MAX_FRAMES)),
            loop_lp: [0.0; CH],
        })
    }
}

/// Echo: the ring is allocated to the maximum (a time change is a fractional read,
/// NOT structural); time is smoothed over 100 ms — tape glide without clicks;
/// the filter in the loop — one-pole LP (the tail darkens with each repeat,
/// a legacy of the F-AUD-009 feature); pingpong — cross feedback.
#[derive(Clone)]
struct DelayNode {
    time_ms: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    pingpong: bool,
    lines: [DelayLine; CH],
    loop_lp: [f32; CH],
}

impl AudioState for DelayNode {
    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (time_ms, feedback, mix, pingpong) = (k[0] as f32, k[1] as f32, k[2] as f32, k[3] != 0.0);
        self.time_ms.set_target(time_ms.clamp(1.0, DELAY_MAX_MS));
        self.feedback.set_target(feedback.clamp(0.0, 0.9));
        self.mix.set_target(mix.clamp(0.0, 1.0));
        self.pingpong = pingpong;
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.time_ms.set_target(c.value("time_ms", self.time_ms.target as f64, s) as f32);
        self.feedback.set_target(c.value("feedback", self.feedback.target as f64, s) as f32);
        self.mix.set_target(c.value("mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], _pos: i64, _key: Option<&[f32]>) {
        // at the CPU's level, the same bytes (audio-fx-cost, `kt_plugin_api::cpu`)
        kt_plugin_api::fast!({
            let sr = AUDIO_SAMPLE_RATE as f32;
            // loop filter ~4 kHz: the repeats darken
            let alpha = 1.0 - (-2.0 * std::f32::consts::PI * 4000.0 / sr).fexp();
            for fr in buf.chunks_exact_mut(CH) {
                let d = self.time_ms.tick().clamp(1.0, DELAY_MAX_MS) * sr / 1000.0;
                let fb = self.feedback.tick().clamp(0.0, 0.9);
                let k = self.mix.tick().clamp(0.0, 1.0);
                let mut wet = [0f32; CH];
                for (w, line) in wet.iter_mut().zip(self.lines.iter()) {
                    *w = line.read_frac(d);
                }
                for ch in 0..CH {
                    let dry = fr[ch];
                    // ping-pong: the feedback goes to the OPPOSITE channel
                    let fb_src = if self.pingpong { wet[(ch + 1) % CH] } else { wet[ch] };
                    self.loop_lp[ch] += (fb_src - self.loop_lp[ch]) * alpha;
                    self.lines[ch].write(dry + self.loop_lp[ch] * fb);
                    fr[ch] = dry * (1.0 - k) + wet[ch] * k;
                }
            }
        })
    }

    /// The built state always rings out (the static formula gates it by the mix).
    fn tail(&self) -> Tick {
        let frames = self.time_ms.target.clamp(1.0, DELAY_MAX_MS) / 1000.0 * 48_000.0;
        tail_frames_for(frames, self.feedback.target.clamp(0.01, 0.9))
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.reset();
        }
        self.loop_lp = [0.0; CH];
        self.time_ms.reset_to_target();
        self.feedback.reset_to_target();
        self.mix.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// slowed_reverb (plugin-api-stage1, 3b.2, D11.5)
// ---------------------------------------------------------------------------

/// The frame row's keys (R.37 C5): the file holds `"nearest"`/`"blend"`.
pub const FRAME_MODES: &[&str] = &["nearest", "blend"];

/// The tail of the reverb half: the freeverb's t60 along its longest line, while the reverb is
/// heard (its mix or its mix curve above zero) — the former `chain_tail` arm, verbatim.
fn slowed_tail(p: &ParamValues, c: &dyn SlotCurves) -> i64 {
    let (mix, delay_ms, decay) = (p.get(1) as f32, p.get(2) as f32, p.get(4) as f32);
    if !(mix > 0.0 || c.points("mix").iter().any(|(_, v)| *v > 0.0)) {
        return 0;
    }
    kt_plugin_api::dsp::reverb_tail_for_delay(delay_ms, decay).to_audio_frames()
}

/// Slowed+Reverb: the WARP half consumes the source at the signed rate (the host's machinery,
/// found through `Decl.time_warp`, not a kind — R.39/R.57 maps stay in kt-model); the REVERB half
/// is this kernel, the R.47 freeverb node verbatim. On a video instance only the warp acts (the
/// choice of the source frame), the reverb does not sound there.
pub struct SlowedReverb {
    m: Manifest,
}

impl SlowedReverb {
    pub fn new() -> SlowedReverb {
        let structural = |n, lo, hi, d, st| ParamSpec::num(n, lo, hi, d, st, ParamUnit::Ms, ParamKind::Structural);
        SlowedReverb {
            m: in_category(
                manifest(
                    "slowed_reverb",
                    vec![
                        // signed and including zero (R.57, the owner's decision): minus runs
                        // BACKWARD through the material, zero — a freeze frame
                        smooth("rate", -32.0, 32.0, 1.0, 0.01, ParamUnit::Ratio),
                        smooth("mix", 0.0, 1.0, 0.0, 0.01, ParamUnit::Norm),
                        // changing the length of comb lines rebuilds the reverb — not automatable
                        structural("reverb_delay_ms", 1.0, 500.0, 60.0, 1.0),
                        smooth("damp", 0.0, 0.99, 0.2, 0.01, ParamUnit::Norm),
                        smooth("decay", 0.0, 0.98, 0.84, 0.01, ParamUnit::Norm),
                        smooth("wet_gain", 0.0, 2.0, 1.0, 0.01, ParamUnit::Ratio),
                        // the length of the pre-delay line — the same geometry as reverb_delay_ms
                        structural("predelay_ms", 0.0, 200.0, 0.0, 1.0),
                        // how a VIDEO instance obtains an intermediate frame (R.37 C5); in the
                        // file between predelay_ms and width, where the variant had it
                        ParamSpec::choice_row("frames", FRAME_MODES, 0),
                        smooth("width", 0.0, 1.0, 1.0, 0.01, ParamUnit::Norm),
                    ],
                    vec![
                        RowDecl { store: Store::F64, omit_default: false },
                        f32_row(),
                        f32_row(),
                        f32_row(),
                        f32_row(),
                        f32_row(),
                        RowDecl { store: Store::F32, omit_default: true },
                        RowDecl { store: Store::Choice(FRAME_MODES), omit_default: true },
                        RowDecl { store: Store::F32, omit_default: true },
                    ],
                    "Slowed+Reverb",
                    "fxreg.kw.slowedreverb",
                    Decl {
                        state: true,
                        tail: Some(slowed_tail),
                        time_warp: Some(TimeWarp { rate_row: 0, frames_row: Some(7) }),
                        // the pre-R.33 keyframes: `migrate_legacy_slowed` turns them into curves
                        legacy_fields: &["automation"],
                        ..Decl::default()
                    },
                ),
                "fxreg.cat.timestretch",
            ),
        }
    }
}

plugin_boilerplate!(SlowedReverb);

impl AudioKernel for SlowedReverb {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(ReverbNode { rv: Freeverb::new(0.0, 100.0, 0.2, 0.7, 1.0, 0.0, 1.0), base_mix: 0.0 })
    }
}

/// The reverb half of the Slowed+Reverb slot (the warp half lives in a separate processor field —
/// the warp holds the monopoly on consuming the source).
#[derive(Clone)]
struct ReverbNode {
    rv: Freeverb,
    base_mix: f64,
}

impl AudioState for ReverbNode {
    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }
    fn apply(&mut self, k: &[f64], structural: bool) {
        // k[0] is the rate (the host's warp), k[7] the frame mode (video only)
        let (mix, delay, damp, decay, wet_gain, predelay, width) =
            (k[1] as f32, k[2] as f32, k[3] as f32, k[4] as f32, k[5] as f32, k[6] as f32, k[8] as f32);
        self.base_mix = mix as f64;
        if structural {
            self.rv = Freeverb::new(mix, delay, damp, decay, wet_gain, predelay, width);
        } else {
            self.rv.set_params(mix, delay, damp, decay, wet_gain, predelay, width);
        }
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        // mix is smoothed by a one-pole inside the reverb (the same call as before R.47 — the
        // bytes of a single-slot chain do not move)
        self.rv.set_mix(c.value("mix", self.base_mix, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], _pos: i64, _key: Option<&[f32]>) {
        // at the CPU's level, the same bytes (audio-fx-cost, `kt_plugin_api::cpu`)
        kt_plugin_api::fast!({
            self.rv.process(buf);
        })
    }

    fn tail(&self) -> Tick {
        self.rv.tail()
    }

    fn reset(&mut self) {
        self.rv.reset();
    }
}
