//! Dynamics audio effects as plugins (plugin-api-stage1, stage 3): gain, gate, limiter. Each node is
//! the R.47/R.48 node of kt-fx moved verbatim onto `kt_plugin_api::dsp`; its passport repeats the
//! enum variant's fields in their order, types and defaults (t973 holds the bytes of the file,
//! t973c the bytes of the sound).
//!
//! The compressor came last (stage 3b, D11.4): its sidechain `key` is a host-reference row
//! (`Store::Ref(RowRole::SidechainKey)`) — the slot holds the track id's number, the engine
//! routes the key signal by the row's role, and the kernel only receives the signal.

use kt_plugin_api::math::FrozenMath;
use std::sync::Arc;

use kt_plugin_api::dsp::{AUDIO_SAMPLE_RATE, CH, EnvFollower, SmoothedParam};
use kt_plugin_api::{
    AudioKernel, AudioState, Decl, HostOpts, Manifest, Media, ParamKind, ParamSpec, ParamUnit,
    ParamValues, Plugin, RowDecl, RowRole, SlotCurves, Store, UiDecl,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(Gain::new()), Arc::new(Gate::new()), Arc::new(Limiter::new()), Arc::new(Compressor::new())]
}

/// A numeric row of a moved kind: an `f32` field, written always or omitted at its default.
fn row(omit_default: bool) -> RowDecl {
    RowDecl { store: Store::F32, omit_default }
}

fn smooth(n: &'static str, lo: f64, hi: f64, d: f64, st: f64, u: ParamUnit) -> ParamSpec {
    ParamSpec::num(n, lo, hi, d, st, u, ParamKind::Smooth)
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

// ---------------------------------------------------------------------------
// gain
// ---------------------------------------------------------------------------

/// Volume and channel balance (R.33 C5).
pub struct Gain {
    m: Manifest,
}

impl Gain {
    pub fn new() -> Gain {
        Gain {
            m: Manifest {
                id: "gain".into(),
                media: Media::AudioFx,
                passport: vec![
                    smooth("db", -60.0, 12.0, 0.0, 0.1, ParamUnit::Db),
                    smooth("pan", -1.0, 1.0, 0.0, 0.01, ParamUnit::Norm),
                ],
                // `db` always written; `pan` was `skip_serializing_if = "is_zero"`
                rows: vec![row(false), row(true)],
                decl: Decl { state: true, ..Decl::default() },
                ui: UiDecl { name: "Gain", category_key: "fxreg.cat.volume", keywords_key: "fxreg.kw.gain", ..UiDecl::default() },
            },
        }
    }
}

plugin_boilerplate!(Gain);

impl AudioKernel for Gain {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(GainNode {
            static_gain: 1.0,
            static_pan_l: 1.0,
            static_pan_r: 1.0,
            gain_auto: false,
            pan_auto: false,
            base_db: 0.0,
            base_pan: 0.0,
            auto_gain: SmoothedParam::new(1.0),
            auto_pan_l: SmoothedParam::new(1.0),
            auto_pan_r: SmoothedParam::new(1.0),
        })
    }
}

/// Slot volume/balance. The constant path — the verbatim multipliers of the old
/// code (GOLDEN_AUDIO relies on them); with curves — a SmoothedParam with targets at the
/// chunk midpoint, a tick on every frame (the same loop order: first
/// volume per frame, then balance per frame).
struct GainNode {
    static_gain: f32,
    static_pan_l: f32,
    static_pan_r: f32,
    gain_auto: bool,
    pan_auto: bool,
    base_db: f64,
    base_pan: f64,
    auto_gain: SmoothedParam,
    auto_pan_l: SmoothedParam,
    auto_pan_r: SmoothedParam,
}

impl AudioState for GainNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (db, pan) = (k[0] as f32, k[1] as f32);
        self.base_db = db as f64;
        self.base_pan = pan as f64;
        self.static_gain = 10f32.fpowf(db / 20.0);
        // balance: linear law, at zero both multipliers are exactly 1.0 —
        // the previous path byte-for-byte
        let p = pan.clamp(-1.0, 1.0);
        self.static_pan_l = (1.0 - p).min(1.0);
        self.static_pan_r = (1.0 + p).min(1.0);
    }

    /// Whether the slot has curves — by the fact of the profiles (as gain_slots/pan_slots
    /// were previously collected from non-empty points).
    fn bind(&mut self, c: &dyn SlotCurves, _clip_len: i64) {
        self.gain_auto = !c.points("db").is_empty();
        self.pan_auto = !c.points("pan").is_empty();
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        if self.gain_auto {
            let db = c.value("db", self.base_db, s);
            self.auto_gain.set_target(10f32.fpowf(db as f32 / 20.0));
        }
        if self.pan_auto {
            let p = c.value("pan", self.base_pan, s).clamp(-1.0, 1.0) as f32;
            self.auto_pan_l.set_target((1.0 - p).min(1.0));
            self.auto_pan_r.set_target((1.0 + p).min(1.0));
        }
    }

    fn process(&mut self, buf: &mut [f32], _pos: i64, _key: Option<&[f32]>) {
        if self.gain_auto {
            for f in buf.chunks_exact_mut(CH) {
                let g = self.auto_gain.tick();
                for s in f.iter_mut() {
                    *s *= g;
                }
            }
        } else if self.static_gain != 1.0 {
            for s in buf.iter_mut() {
                *s *= self.static_gain;
            }
        }
        if self.pan_auto {
            for f in buf.chunks_exact_mut(CH) {
                let (l, r) = (self.auto_pan_l.tick(), self.auto_pan_r.tick());
                f[0] *= l;
                if CH > 1 {
                    f[1] *= r;
                }
            }
        } else if self.static_pan_l != 1.0 || self.static_pan_r != 1.0 {
            for f in buf.chunks_exact_mut(CH) {
                f[0] *= self.static_pan_l;
                if CH > 1 {
                    f[1] *= self.static_pan_r;
                }
            }
        }
    }

    fn reset(&mut self) {
        self.auto_gain.reset_to_target();
        self.auto_pan_l.reset_to_target();
        self.auto_pan_r.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// gate
// ---------------------------------------------------------------------------

/// Noise gate (R.47).
pub struct Gate {
    m: Manifest,
}

impl Gate {
    pub fn new() -> Gate {
        Gate {
            m: Manifest {
                id: "gate".into(),
                media: Media::AudioFx,
                passport: vec![
                    smooth("threshold_db", -80.0, 0.0, -40.0, 0.1, ParamUnit::Db),
                    smooth("attack_ms", 0.1, 50.0, 1.0, 0.1, ParamUnit::Ms),
                    smooth("hold_ms", 0.0, 500.0, 40.0, 1.0, ParamUnit::Ms),
                    smooth("release_ms", 5.0, 1000.0, 100.0, 1.0, ParamUnit::Ms),
                ],
                rows: vec![row(false), row(false), row(false), row(false)],
                decl: Decl { state: true, ..Decl::default() },
                ui: UiDecl { name: "Gate", category_key: "fxreg.cat.dynamics", keywords_key: "fxreg.kw.gate", ..UiDecl::default() },
            },
        }
    }
}

plugin_boilerplate!(Gate);

impl AudioKernel for Gate {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(GateNode {
            threshold_db: SmoothedParam::with_ms(-40.0, 20.0),
            attack_ms: 1.0,
            release_ms: 100.0,
            hold_frames: (0.040 * 48_000.0) as i64,
            env: EnvFollower::new(0.5, 0.5),
            open: false,
            hold_left: 0,
            gain: 0.0,
        })
    }
}

/// Gate: a fast detector (~0.5 ms) + hysteresis −6 dB + hold counter
/// (frames — survives the seam) + attack/release ramps.
struct GateNode {
    threshold_db: SmoothedParam,
    attack_ms: f32,
    release_ms: f32,
    hold_frames: i64,
    env: EnvFollower,
    open: bool,
    hold_left: i64,
    gain: f32,
}

impl AudioState for GateNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (thr, att, hold, rel) = (k[0] as f32, k[1] as f32, k[2] as f32, k[3] as f32);
        self.threshold_db.set_target(thr.clamp(-80.0, 0.0));
        self.attack_ms = att.clamp(0.1, 50.0);
        self.release_ms = rel.clamp(5.0, 1000.0);
        self.hold_frames = ((hold.clamp(0.0, 500.0) / 1000.0) * 48_000.0) as i64;
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.threshold_db.set_target(c.value("threshold_db", self.threshold_db.target as f64, s) as f32);
        self.attack_ms = (c.value("attack_ms", self.attack_ms as f64, s) as f32).clamp(0.1, 50.0);
        self.release_ms = (c.value("release_ms", self.release_ms as f64, s) as f32).clamp(5.0, 1000.0);
        let hold = c.value("hold_ms", (self.hold_frames as f64) / 48.0, s) as f32;
        self.hold_frames = ((hold.clamp(0.0, 500.0) / 1000.0) * 48_000.0) as i64;
    }

    fn process(&mut self, buf: &mut [f32], _pos: i64, _key: Option<&[f32]>) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        let att_step = 1.0 / (self.attack_ms / 1000.0 * sr).max(1.0);
        let rel_step = 1.0 / (self.release_ms / 1000.0 * sr).max(1.0);
        for fr in buf.chunks_exact_mut(CH) {
            let thr_db = self.threshold_db.tick();
            let open_thr = 10f32.fpowf(thr_db / 20.0);
            // hysteresis: closing at 6 dB below opening
            let close_thr = 10f32.fpowf((thr_db - 6.0) / 20.0);
            let peak = fr.iter().fold(0f32, |m, s| m.max(s.abs()));
            let env = self.env.tick(peak);
            if env >= open_thr {
                self.open = true;
                self.hold_left = self.hold_frames;
            } else if env < close_thr {
                if self.hold_left > 0 {
                    self.hold_left -= 1;
                } else {
                    self.open = false;
                }
            }
            self.gain = if self.open { (self.gain + att_step).min(1.0) } else { (self.gain - rel_step).max(0.0) };
            for s in fr.iter_mut() {
                *s *= self.gain;
            }
        }
    }

    fn reset(&mut self) {
        self.env.reset();
        self.open = false;
        self.hold_left = 0;
        self.gain = 0.0;
        self.threshold_db.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// limiter
// ---------------------------------------------------------------------------

/// The length of the limiter’s lookahead window in frames (R.48, item 338). ONE home of the
/// formula: both the node (the ring geometry) and the manifest's look-ahead declaration (which
/// the static `chain_lookahead` reads) call it — they cannot diverge by construction.
pub fn limiter_la_frames(lookahead_ms: f32) -> i64 {
    (lookahead_ms.clamp(1.0, 20.0) as f64 / 1000.0 * 48_000.0).ceil() as i64
}

fn limiter_lookahead(p: &ParamValues, _c: &dyn SlotCurves) -> i64 {
    limiter_la_frames(p.get(2) as f32)
}

/// Look-ahead limiter (R.48).
pub struct Limiter {
    m: Manifest,
}

impl Limiter {
    pub fn new() -> Limiter {
        Limiter {
            m: Manifest {
                id: "limiter".into(),
                media: Media::AudioFx,
                passport: vec![
                    smooth("threshold_db", -60.0, 0.0, -6.0, 0.1, ParamUnit::Db),
                    smooth("ceiling_db", -24.0, 0.0, -0.3, 0.1, ParamUnit::Db),
                    // the window: the ring's length is the node's device, a change rebuilds
                    // the processor (R.48, item 337)
                    ParamSpec::num("lookahead_ms", 1.0, 20.0, 5.0, 0.5, ParamUnit::Ms, ParamKind::Structural),
                    smooth("release_ms", 5.0, 1000.0, 80.0, 1.0, ParamUnit::Ms),
                ],
                rows: vec![row(false), row(false), row(false), row(false)],
                decl: Decl { state: true, lookahead: Some(limiter_lookahead), ..Decl::default() },
                ui: UiDecl { name: "Limiter", category_key: "fxreg.cat.dynamics", keywords_key: "fxreg.kw.limiter", ..UiDecl::default() },
            },
        }
    }
}

plugin_boilerplate!(Limiter);

impl AudioKernel for Limiter {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(LimiterNode {
            threshold_db: SmoothedParam::with_ms(-6.0, 20.0),
            ceiling_db: SmoothedParam::with_ms(-0.3, 20.0),
            release_ms: 80.0,
            la: 0,
            ring: Vec::new(),
            pos: 0,
            win: std::collections::VecDeque::new(),
            fed: 0,
            gain: 1.0,
        })
    }
}

/// Look-ahead limiter (R.48, item 338). The ceiling is guaranteed ALGEBRAICALLY:
/// the sliding-maximum window covers the material [t, t+La], including the output
/// frame itself, so the reduction gets ahead of the peak and the attack is instant —
/// there is no distortion artifact. The output is aligned with the chain input: the ring
/// delays the signal by exactly La, which the processor subtracted from the position.
struct LimiterNode {
    threshold_db: SmoothedParam,
    ceiling_db: SmoothedParam,
    release_ms: f32,
    /// Window geometry: changes ONLY on structural — a divergence from the
    /// chain is caught by FxStateStore, which rebuilds the processor entirely.
    la: i64,
    /// Alignment ring (la frames × CH).
    ring: Vec<f32>,
    pos: usize,
    /// Monotone queue of the sliding maximum of |x|: values decrease, at the front
    /// the window maximum. O(1) per frame, amortized.
    win: std::collections::VecDeque<(i64, f32)>,
    /// How many frames have been fed into the window (the age of the elements).
    fed: i64,
    gain: f32,
}

impl AudioState for LimiterNode {
    fn apply(&mut self, k: &[f64], structural: bool) {
        let (thr, ceil, lookahead_ms, release) = (k[0] as f32, k[1] as f32, k[2] as f32, k[3] as f32);
        self.threshold_db.set_target(thr.clamp(-60.0, 0.0));
        self.ceiling_db.set_target(ceil.clamp(-24.0, 0.0));
        self.release_ms = release.clamp(5.0, 1000.0);
        let la = limiter_la_frames(lookahead_ms);
        if structural || self.ring.is_empty() {
            self.la = la;
            self.ring = vec![0.0; la.max(1) as usize * CH];
            self.pos = 0;
            self.win.clear();
            self.fed = 0;
        }
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.threshold_db.set_target(c.value("threshold_db", self.threshold_db.target as f64, s) as f32);
        self.ceiling_db.set_target(c.value("ceiling_db", self.ceiling_db.target as f64, s) as f32);
        self.release_ms = (c.value("release_ms", self.release_ms as f64, s) as f32).clamp(5.0, 1000.0);
    }

    fn process(&mut self, buf: &mut [f32], _pos: i64, _key: Option<&[f32]>) {
        if self.la <= 0 {
            return;
        }
        let rel_coef = 1.0 - (-1.0 / (self.release_ms / 1000.0 * 48_000.0)).fexp();
        for fr in buf.chunks_exact_mut(CH) {
            let thr = 10f32.fpowf(self.threshold_db.tick() / 20.0);
            let ceil = 10f32.fpowf(self.ceiling_db.tick() / 20.0);
            // the INCOMING frame goes into the window (position t + La)
            let peak = fr.iter().fold(0f32, |m, v| m.max(v.abs()));
            while let Some(&(_, v)) = self.win.back() {
                if v <= peak {
                    self.win.pop_back();
                } else {
                    break;
                }
            }
            self.win.push_back((self.fed, peak));
            let cutoff = self.fed - self.la;
            while let Some(&(age, _)) = self.win.front() {
                if age < cutoff {
                    self.win.pop_front();
                } else {
                    break;
                }
            }
            self.fed += 1;
            // the output — the frame delayed by La (position t)
            for v in fr.iter_mut() {
                std::mem::swap(&mut self.ring[self.pos], v);
                self.pos += 1;
            }
            if self.pos >= self.ring.len() {
                self.pos = 0;
            }
            // the maximum over [t, t+La] — including the output frame itself, therefore
            // out = wmax * g ≤ ceiling strictly
            let wmax = self.win.front().map(|&(_, v)| v).unwrap_or(0.0);
            let target = (thr / wmax.max(thr)) * (ceil / thr);
            if target < self.gain {
                self.gain = target;
            } else {
                self.gain += (target - self.gain) * rel_coef;
            }
            for v in fr.iter_mut() {
                *v *= self.gain;
            }
        }
    }

    fn lookahead(&self) -> i64 {
        self.la
    }

    fn reset(&mut self) {
        self.ring.fill(0.0);
        self.pos = 0;
        self.win.clear();
        self.fed = 0;
        self.gain = 1.0;
        self.threshold_db.reset_to_target();
        self.ceiling_db.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// compressor
// ---------------------------------------------------------------------------

/// Compressor (R.47) with a sidechain key (R.65): peak envelope, stereo link max(|l|,|r|), gr in dB.
pub struct Compressor {
    m: Manifest,
}

impl Compressor {
    pub fn new() -> Compressor {
        Compressor {
            m: Manifest {
                id: "compressor".into(),
                media: Media::AudioFx,
                passport: vec![
                    smooth("threshold_db", -60.0, 0.0, -18.0, 0.1, ParamUnit::Db),
                    smooth("ratio", 1.0, 20.0, 4.0, 0.1, ParamUnit::Ratio),
                    smooth("attack_ms", 0.1, 250.0, 10.0, 0.1, ParamUnit::Ms),
                    smooth("release_ms", 5.0, 2000.0, 150.0, 1.0, ParamUnit::Ms),
                    smooth("makeup_db", 0.0, 24.0, 0.0, 0.1, ParamUnit::Db),
                    // The SIDECHAIN KEY: a `TrackId`, NOT a track number — the number is the rank
                    // of the row, and inserting a track above would move the key onto someone
                    // else's signal (R.65). None is not written: a file without a sidechain stays
                    // byte-for-byte the same.
                    ParamSpec::ref_row("key"),
                ],
                rows: vec![
                    row(false),
                    row(false),
                    row(false),
                    row(false),
                    row(true),
                    RowDecl { store: Store::Ref(RowRole::SidechainKey), omit_default: true },
                ],
                decl: Decl { state: true, ..Decl::default() },
                ui: UiDecl { name: "Compressor", category_key: "fxreg.cat.dynamics", keywords_key: "fxreg.kw.compressor", ..UiDecl::default() },
            },
        }
    }
}

plugin_boilerplate!(Compressor);

impl AudioKernel for Compressor {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(CompNode {
            threshold_db: SmoothedParam::with_ms(-18.0, 20.0),
            ratio: SmoothedParam::with_ms(4.0, 20.0),
            makeup_db: SmoothedParam::with_ms(0.0, 20.0),
            env: EnvFollower::new(10.0, 150.0),
        })
    }
}

/// The R.47 node, verbatim (t973c holds its sound).
struct CompNode {
    threshold_db: SmoothedParam,
    ratio: SmoothedParam,
    makeup_db: SmoothedParam,
    env: EnvFollower,
}

impl AudioState for CompNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        // k[5] is the key's track id: an address the kernel never reads
        let (thr, ratio, att, rel, makeup) = (k[0] as f32, k[1] as f32, k[2] as f32, k[3] as f32, k[4] as f32);
        self.threshold_db.set_target(thr.clamp(-60.0, 0.0));
        self.ratio.set_target(ratio.clamp(1.0, 20.0));
        self.makeup_db.set_target(makeup.clamp(0.0, 24.0));
        self.env.set_times(att.clamp(0.1, 250.0), rel.clamp(5.0, 2000.0));
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.threshold_db.set_target(c.value("threshold_db", self.threshold_db.target as f64, s) as f32);
        self.ratio.set_target(c.value("ratio", self.ratio.target as f64, s) as f32);
        self.makeup_db.set_target(c.value("makeup_db", self.makeup_db.target as f64, s) as f32);
        // the R.47 node reads the attack/release curves around the kind's defaults, not the
        // slot's base (the times live inside the follower) — kept verbatim, the sound rests on it
        let att = c.value("attack_ms", 10.0, s) as f32;
        let rel = c.value("release_ms", 150.0, s) as f32;
        self.env.set_times(att.clamp(0.1, 250.0), rel.clamp(5.0, 2000.0));
    }

    /// `key` — sidechain: the detector listens to IT, while compression still acts on its own
    /// input. The compression law, limits and smoothing are the same — the key changes
    /// only the envelope source (R.65). A key shorter than the buffer (the last
    /// chunk of the window) gives a peak of 0 for the remainder — that is quieter than the threshold, i.e.
    /// “the key is silent”, not a panic.
    fn process(&mut self, buf: &mut [f32], _pos: i64, key: Option<&[f32]>) {
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let thr = self.threshold_db.tick();
            let ratio = self.ratio.tick().max(1.0);
            let makeup = 10f32.fpowf(self.makeup_db.tick() / 20.0);
            let peak = match key {
                None => fr.iter().fold(0f32, |m, s| m.max(s.abs())),
                Some(k) => k
                    .get(i * CH..i * CH + CH)
                    .map_or(0.0, |kf| kf.iter().fold(0f32, |m, s| m.max(s.abs()))),
            };
            let env = self.env.tick(peak).max(1e-6);
            let env_db = 20.0 * env.flog10();
            let over = (env_db - thr).max(0.0);
            let gr_db = over * (1.0 - 1.0 / ratio);
            let g = 10f32.fpowf(-gr_db / 20.0) * makeup;
            for s in fr.iter_mut() {
                *s *= g;
            }
        }
    }

    fn reset(&mut self) {
        self.env.reset();
        self.threshold_db.reset_to_target();
        self.ratio.reset_to_target();
        self.makeup_db.reset_to_target();
    }
}
