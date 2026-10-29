//! Tone audio effects as plugins (plugin-api-stage1, stage 3): filter, eq, distortion, bitcrusher,
//! stereo_width. Each node is the R.47 node of kt-fx moved verbatim onto `kt_plugin_api::dsp`; its
//! passport repeats the enum variant's fields in their order, types and defaults (t973 holds the
//! bytes of the file, GOLDEN_AUDIO the bytes of the sound).

use std::sync::Arc;

use kt_plugin_api::dsp::{AUDIO_SAMPLE_RATE, Biquad, BiquadKind, CH, SmoothedParam, TUNE_STEP, apply_width, softclip};
use kt_plugin_api::{
    AudioKernel, AudioState, Decl, HostOpts, Manifest, Media, ParamKind, ParamSpec, ParamUnit,
    Plugin, RowDecl, SlotCurves, Store, UiDecl,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(Eq::new()), Arc::new(Distortion::new()), Arc::new(Bitcrusher::new()), Arc::new(StereoWidth::new())]
}

/// Stereo width m/s (R.47): 0 — mono, 1 — as recorded, 2 — wider.
pub struct StereoWidth {
    m: Manifest,
}

impl StereoWidth {
    pub fn new() -> StereoWidth {
        StereoWidth {
            m: Manifest {
                id: "stereo_width".into(),
                media: Media::AudioFx,
                // the variant's field: `width: f32`, default 1.0, always written
                passport: vec![ParamSpec::num("width", 0.0, 2.0, 1.0, 0.01, ParamUnit::Ratio, ParamKind::Smooth)],
                rows: vec![RowDecl { store: Store::F32, omit_default: false }],
                decl: Decl { state: true, ..Decl::default() },
                ui: UiDecl {
                    name: "Stereo Width",
                    category_key: "fxreg.cat.stereo",
                    keywords_key: "fxreg.kw.stereowidth",
                },
            },
        }
    }
}

impl Default for StereoWidth {
    fn default() -> StereoWidth {
        StereoWidth::new()
    }
}

impl Plugin for StereoWidth {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn audio(&self) -> Option<&dyn AudioKernel> {
        Some(self)
    }
}

impl AudioKernel for StereoWidth {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(WidthNode { width: SmoothedParam::with_ms(1.0, 20.0) })
    }
}

/// Stereo m/s width. width == 1.0 — an EXPLICIT pass-through (m/s repacking is
/// not byte-identical).
struct WidthNode {
    width: SmoothedParam,
}

impl AudioState for WidthNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let width = k[0] as f32;
        self.width.set_target(width.clamp(0.0, 2.0));
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.width.set_target(c.value("width", self.width.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], _pos: i64, _key: Option<&[f32]>) {
        if self.width.value() == 1.0 && self.width.target == 1.0 {
            return;
        }
        for fr in buf.chunks_exact_mut(CH) {
            let w = self.width.tick().clamp(0.0, 2.0);
            apply_width(fr, w);
        }
    }

    fn reset(&mut self) {
        self.width.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// eq, distortion, bitcrusher (moved from kt-fx `audio/nodes.rs`, stage 3)
// ---------------------------------------------------------------------------

/// A numeric row of a moved kind: an `f32` field, written always or omitted at its default.
fn row(omit_default: bool) -> RowDecl {
    RowDecl { store: Store::F32, omit_default }
}

fn audio_manifest(id: &'static str, passport: Vec<ParamSpec>, rows: Vec<RowDecl>, ui: UiDecl, decl: Decl) -> Manifest {
    Manifest { id: id.into(), media: Media::AudioFx, passport, rows, decl, ui }
}

/// The Q of the EQ shelves and the mid-band Q floor — the SAME numbers for the
/// sound and for the frequency response graph (R.48, item 347).
pub const EQ_SHELF_Q: f32 = 0.707;
pub const EQ_MID_Q_FLOOR: f32 = 0.3;

/// Frequency response of the whole equalizer at frequency `at_hz`, dB. Values — in the order
/// of the passport: low_gain, low_freq, mid_gain, mid_freq, mid_q,
/// high_gain, high_freq. The cascade = the sum of dB; the filters are tuned by the same
/// `tune` that sounds, so the curve in the UI equals what is heard.
pub fn eq_mag_db(v: &[f32; 7], at_hz: f32) -> f64 {
    let mut b = Biquad::default();
    let mut db = 0.0;
    b.tune(&BiquadKind::LowShelf, v[1], EQ_SHELF_Q, v[0]);
    db += b.mag_db(at_hz);
    b.tune(&BiquadKind::Peak, v[3], v[4].max(EQ_MID_Q_FLOOR), v[2]);
    db += b.mag_db(at_hz);
    b.tune(&BiquadKind::HighShelf, v[6], EQ_SHELF_Q, v[5]);
    db += b.mag_db(at_hz);
    db
}

/// Three-band equaliser (R.47): shelf + peak + shelf.
pub struct Eq {
    m: Manifest,
}

impl Eq {
    pub fn new() -> Eq {
        let p = |n, lo, hi, d, st, u| ParamSpec::num(n, lo, hi, d, st, u, ParamKind::Smooth);
        Eq {
            m: audio_manifest(
                "eq",
                vec![
                    p("low_gain_db", -24.0, 24.0, 0.0, 0.1, ParamUnit::Db),
                    p("low_freq_hz", 20.0, 2000.0, 200.0, 10.0, ParamUnit::Hz),
                    p("mid_gain_db", -24.0, 24.0, 0.0, 0.1, ParamUnit::Db),
                    p("mid_freq_hz", 100.0, 10000.0, 1000.0, 10.0, ParamUnit::Hz),
                    p("mid_q", 0.3, 8.0, 1.0, 0.01, ParamUnit::None),
                    p("high_gain_db", -24.0, 24.0, 0.0, 0.1, ParamUnit::Db),
                    p("high_freq_hz", 1000.0, 20000.0, 4000.0, 10.0, ParamUnit::Hz),
                ],
                // the gains and mid_q were `skip_serializing_if` at their default, the freqs always written
                vec![row(true), row(false), row(true), row(false), row(true), row(true), row(false)],
                UiDecl { name: "EQ", category_key: "fxreg.cat.filter", keywords_key: "fxreg.kw.eq" },
                Decl { state: true, ..Decl::default() },
            ),
        }
    }
}

impl Default for Eq {
    fn default() -> Eq {
        Eq::new()
    }
}

impl Plugin for Eq {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn audio(&self) -> Option<&dyn AudioKernel> {
        Some(self)
    }
}

impl AudioKernel for Eq {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(EqNode {
            params: std::array::from_fn(|i| SmoothedParam::with_ms(EqNode::DEFAULTS[i], 20.0)),
            bands: Default::default(),
        })
    }
}

struct EqNode {
    /// The seven smoothed EQ parameters (low_g, low_f, mid_g, mid_f, mid_q, high_g, high_f).
    params: [SmoothedParam; 7],
    bands: [[Biquad; 3]; CH],
}

impl EqNode {
    const DEFAULTS: [f32; 7] = [0.0, 200.0, 0.0, 1000.0, 1.0, 0.0, 4000.0];
    const NAMES: [&'static str; 7] =
        ["low_gain_db", "low_freq_hz", "mid_gain_db", "mid_freq_hz", "mid_q", "high_gain_db", "high_freq_hz"];
}

impl AudioState for EqNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        for (p, v) in self.params.iter_mut().zip(k) {
            p.set_target(*v as f32);
        }
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        for (i, p) in self.params.iter_mut().enumerate() {
            let v = c.value(Self::NAMES[i], p.target as f64, s) as f32;
            p.set_target(v);
        }
    }

    fn process(&mut self, buf: &mut [f32], _pos: i64, _key: Option<&[f32]>) {
        for block in buf.chunks_mut(TUNE_STEP * CH) {
            let mut v = [0f32; 7];
            for _ in 0..block.len() / CH {
                for (dst, p) in v.iter_mut().zip(self.params.iter_mut()) {
                    *dst = p.tick();
                }
            }
            // all gains at zero — plain pass-through: the EQ is transparent byte-for-byte
            if v[0] == 0.0 && v[2] == 0.0 && v[5] == 0.0 {
                continue;
            }
            for ch in 0..CH {
                // the same numbers as the frequency response graph (R.48): the same constants
                self.bands[ch][0].tune(&BiquadKind::LowShelf, v[1], EQ_SHELF_Q, v[0]);
                self.bands[ch][1].tune(&BiquadKind::Peak, v[3], v[4].max(EQ_MID_Q_FLOOR), v[2]);
                self.bands[ch][2].tune(&BiquadKind::HighShelf, v[6], EQ_SHELF_Q, v[5]);
            }
            for fr in block.chunks_exact_mut(CH) {
                for (ch, s) in fr.iter_mut().enumerate() {
                    let mut x = *s;
                    for b in &mut self.bands[ch] {
                        x = b.process(x);
                    }
                    *s = x;
                }
            }
        }
    }

    fn reset(&mut self) {
        for chb in &mut self.bands {
            for b in chb {
                b.reset();
            }
        }
        for p in &mut self.params {
            p.reset_to_target();
        }
    }
}

/// Soft saturation (R.47): softclip with level compensation.
pub struct Distortion {
    m: Manifest,
}

impl Distortion {
    pub fn new() -> Distortion {
        let p = |n, lo, hi, d, st, u| ParamSpec::num(n, lo, hi, d, st, u, ParamKind::Smooth);
        Distortion {
            m: audio_manifest(
                "distortion",
                vec![
                    p("drive_db", 0.0, 36.0, 12.0, 0.1, ParamUnit::Db),
                    p("tone", 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm),
                    p("mix", 0.0, 1.0, 1.0, 0.01, ParamUnit::Norm),
                ],
                vec![row(false), row(false), row(true)],
                UiDecl { name: "Distortion", category_key: "fxreg.cat.distort", keywords_key: "fxreg.kw.distortion" },
                Decl { state: true, ..Decl::default() },
            ),
        }
    }
}

impl Default for Distortion {
    fn default() -> Distortion {
        Distortion::new()
    }
}

impl Plugin for Distortion {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn audio(&self) -> Option<&dyn AudioKernel> {
        Some(self)
    }
}

impl AudioKernel for Distortion {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(DistNode {
            drive_db: SmoothedParam::with_ms(12.0, 20.0),
            tone: SmoothedParam::with_ms(0.5, 20.0),
            mix: SmoothedParam::with_ms(1.0, 20.0),
            lp: [0.0; CH],
        })
    }
}

/// Soft saturation with level compensation and tone.
struct DistNode {
    drive_db: SmoothedParam,
    tone: SmoothedParam,
    mix: SmoothedParam,
    lp: [f32; CH],
}

impl AudioState for DistNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (drive_db, tone, mix) = (k[0] as f32, k[1] as f32, k[2] as f32);
        self.drive_db.set_target(drive_db);
        self.tone.set_target(tone.clamp(0.0, 1.0));
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.drive_db.set_target(c.value("drive_db", self.drive_db.target as f64, s) as f32);
        self.tone.set_target(c.value("tone", self.tone.target as f64, s) as f32);
        self.mix.set_target(c.value("mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], _pos: i64, _key: Option<&[f32]>) {
        for fr in buf.chunks_exact_mut(CH) {
            let g = 10f32.powf(self.drive_db.tick() / 20.0);
            let comp = 1.0 / g.sqrt();
            let tone = self.tone.tick().clamp(0.0, 1.0);
            // tone: one-pole LP 500..12000 Hz (exp-map)
            let f = 500.0 * (12_000.0f32 / 500.0).powf(tone);
            let alpha = 1.0 - (-2.0 * std::f32::consts::PI * f / AUDIO_SAMPLE_RATE as f32).exp();
            let k = self.mix.tick().clamp(0.0, 1.0);
            for (ch, s) in fr.iter_mut().enumerate() {
                let dry = *s;
                let mut y = softclip(dry * g) * comp;
                self.lp[ch] += (y - self.lp[ch]) * alpha;
                y = self.lp[ch];
                *s = dry * (1.0 - k) + y * k;
            }
        }
    }

    fn reset(&mut self) {
        self.lp = [0.0; CH];
        self.drive_db.reset_to_target();
        self.tone.reset_to_target();
        self.mix.reset_to_target();
    }
}

/// Lo-fi degradation (R.47): bit depth + sample hold.
pub struct Bitcrusher {
    m: Manifest,
}

impl Bitcrusher {
    pub fn new() -> Bitcrusher {
        let p = |n, lo, hi, d, st, u| ParamSpec::num(n, lo, hi, d, st, u, ParamKind::Smooth);
        Bitcrusher {
            m: audio_manifest(
                "bitcrusher",
                vec![
                    p("bits", 1.0, 16.0, 8.0, 0.1, ParamUnit::None),
                    p("rate_hz", 500.0, 48000.0, 12000.0, 100.0, ParamUnit::Hz),
                    p("mix", 0.0, 1.0, 1.0, 0.01, ParamUnit::Norm),
                ],
                vec![row(false), row(false), row(true)],
                UiDecl { name: "Bitcrusher", category_key: "fxreg.cat.distort", keywords_key: "fxreg.kw.bitcrusher" },
                Decl { state: true, ..Decl::default() },
            ),
        }
    }
}

impl Default for Bitcrusher {
    fn default() -> Bitcrusher {
        Bitcrusher::new()
    }
}

impl Plugin for Bitcrusher {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn audio(&self) -> Option<&dyn AudioKernel> {
        Some(self)
    }
}

impl AudioKernel for Bitcrusher {
    fn build(&self, _opts: &HostOpts) -> Box<dyn AudioState> {
        Box::new(CrushNode {
            bits: SmoothedParam::with_ms(8.0, 20.0),
            rate_hz: SmoothedParam::with_ms(12_000.0, 20.0),
            mix: SmoothedParam::with_ms(1.0, 20.0),
            held: [0.0; CH],
            held_cell: i64::MIN,
        })
    }
}

/// Lo-fi: bit-depth quantization + sample hold on the POSITION GRID
/// (floor(pos·rate/sr) — determinism: two renders of one clip give the same
/// artifacts, precedent — the seed of video noise).
struct CrushNode {
    bits: SmoothedParam,
    rate_hz: SmoothedParam,
    mix: SmoothedParam,
    held: [f32; CH],
    held_cell: i64,
}

impl AudioState for CrushNode {
    fn apply(&mut self, k: &[f64], _structural: bool) {
        let (bits, rate_hz, mix) = (k[0] as f32, k[1] as f32, k[2] as f32);
        self.bits.set_target(bits.clamp(1.0, 16.0));
        self.rate_hz.set_target(rate_hz.clamp(500.0, 48_000.0));
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64) {
        self.bits.set_target(c.value("bits", self.bits.target as f64, s) as f32);
        self.rate_hz.set_target(c.value("rate_hz", self.rate_hz.target as f64, s) as f32);
        self.mix.set_target(c.value("mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let bits = self.bits.tick().clamp(1.0, 16.0);
            let rate = self.rate_hz.tick().clamp(500.0, 48_000.0) as f64;
            let k = self.mix.tick().clamp(0.0, 1.0);
            let q = (2f32).powf(bits - 1.0);
            let p = pos + i as i64;
            // the hold grid is tied to the clip’s POSITION, not to a counter
            let cell = ((p as f64) * rate / AUDIO_SAMPLE_RATE as f64).floor() as i64;
            if cell != self.held_cell {
                self.held_cell = cell;
                for (ch, s) in fr.iter().enumerate() {
                    self.held[ch] = (s * q).round() / q;
                }
            }
            for (ch, s) in fr.iter_mut().enumerate() {
                *s = *s * (1.0 - k) + self.held[ch] * k;
            }
        }
    }

    fn reset(&mut self) {
        self.held = [0.0; CH];
        self.held_cell = i64::MIN;
        self.bits.reset_to_target();
        self.rate_hz.reset_to_target();
        self.mix.reset_to_target();
    }
}
