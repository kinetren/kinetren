//! The generator contract (plugin-api-stage1, D13) and the helpers every generator kernel shares.
//!
//! A kind is five things, and its plugin carries all five: the frame (`frame`, the former
//! `render_at` arm of kt-media), the sound (`tone`/`gate` — the sine is the host's one `tone_1k`,
//! so the phase rule has one text), the declarations that were exhaustive matches (`alpha`,
//! `live`), the `gen:` path codec as a token table (kt-model's one codec reads it), and the
//! passport rows with the variant's stores, so the file text does not move.
//!
//! The rule of the frames stays the kernels': **float only in the preamble** — distance fields in
//! integer 1/256 px (`isqrt`, `coverage`), tables computed before the pixel loop.
//!
//! A SOUND kind (generators-placement-and-audio) has no picture: it fills its burst itself
//! (`Synth`) from the global sample number. Every sine of the generators — the host's `tone_1k`
//! and the kernels' — is the one `sine` below, so the phase rule and the bytes have one text.

use crate::math::FrozenMath;

/// The material tick a frame is drawn at (re-exported: a family crate depends on the API only, t975).
pub use kt_time::Tick;

/// The former `render_at` arm: straight RGBA of EXACTLY `w×h`; `pts` is the MATERIAL tick.
pub type Frame = fn(&[f64], Tick, u32, u32) -> Box<[u8]>;
/// The level in dBFS when the kind sounds with these values; `None` — silent media.
pub type Tone = fn(&[f64]) -> Option<f64>;
/// Whether global sample `n` (48 kHz) sounds; the default is every sample.
pub type Gate = fn(&[f64], i64) -> bool;
/// A sound kind's burst: the values, the GLOBAL sample number of the burst's first frame (48 kHz)
/// and an interleaved stereo buffer of `AUDIO_BURST_FRAMES` frames, zeroed by the host. A pure
/// function of its arguments: a burst never depends on the bursts read before it (seek, invariant 11).
pub type Synth = fn(&[f64], i64, &mut [f32]);

/// The one sine of the generators at global sample `n` (48 kHz): the phase is a function of the
/// sample number, so seek does not click. The host's 1 kHz tone is this with `hz = 1000.0` — the
/// same operations in the same order as before it moved here (t1048, t1155).
#[inline]
pub fn sine(n: i64, hz: f64) -> f64 {
    ((n as f64) * hz * std::f64::consts::TAU / 48_000.0).fsin()
}

/// A level in dBFS as a linear amplitude (AES17: a full-scale sine is 0 dBFS).
#[inline]
pub fn db_amp(db: f64) -> f64 {
    10f64.fpowf(db / 20.0)
}

/// A generator's kernel (D13).
pub trait GenKernel: Send + Sync {
    fn frame(&self, p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]>;
    fn tone(&self, p: &[f64]) -> Option<f64>;
    fn gate(&self, p: &[f64], n: i64) -> bool;
    fn decl(&self) -> &GenDecl;
    /// Whether the kind declares a host tone at all (with some values).
    fn has_tone(&self) -> bool {
        false
    }
    /// A sound kind's synthesizer; such a kind has no picture.
    fn synth(&self) -> Option<Synth> {
        None
    }
}

/// What the host's generator machinery reads — DATA (D13).
#[derive(Debug, Clone)]
pub struct GenDecl {
    /// The frame has transparent pixels (the former exhaustive `kind_has_alpha`).
    pub alpha: bool,
    /// The frame moves with `pts` by itself (the former `depends_on_time`): the Quality cache is off.
    pub live: bool,
    /// The default frame is empty by meaning: the bin card writes the kind's name over it.
    pub blank: bool,
    /// The `gen:` path tokens, in the order `format_gen_path` writes them.
    pub path: &'static [PathTok],
    /// The rows that are part of the media ADDRESS (`gen_kind_path`): a sort whose name is a
    /// function of its value (the five tables).
    pub address: &'static [&'static str],
    /// The creation menu's rows: the values each sets over the defaults; empty — one row of defaults.
    pub menu: &'static [&'static [(&'static str, f64)]],
    /// A name that is a function of a row's value: the row and each value's catalog key.
    pub names: Option<(&'static str, &'static [&'static str])>,
    /// The eyedropper groups: each colour's stem (`<stem>_r/g/b`, `_a` when it has one) and its
    /// caption's catalog key.
    pub colors: &'static [(&'static str, &'static str)],
}

/// One `gen:` path token: its key and how its value is read and written.
#[derive(Debug, Clone, Copy)]
pub struct PathTok {
    pub key: &'static str,
    pub val: PathVal,
}

/// How a path token's value is read and written — exactly the former `apply_token` validators and
/// `format_gen_path` forms. A token at its rows' defaults is not written.
#[derive(Debug, Clone, Copy)]
pub enum PathVal {
    /// `rrggbb` over `<stem>_r/_g/_b`.
    Hex3(&'static str),
    /// `rrggbbaa` over `<stem>_r/_g/_b/_a`.
    Hex4(&'static str),
    /// Any finite number, written in its shortest exact form.
    Num(&'static str),
    /// A fraction 0..1 (out of range is a broken path, not a clamp).
    Unit(&'static str),
    /// A size up to two axis fractions, 0..2.
    Span(&'static str),
    /// An integer in `lo..=hi`.
    Int(&'static str, u32, u32),
    /// A flag that is off by default: `1` is on, anything else off; written `1` when on.
    Flag1(&'static str),
    /// A flag that is on by default: `0` is off, anything else on; written `0` when off.
    Flag0(&'static str),
    /// A choice by its PATH token (not its file key: `grey` against `grayscale`).
    Choice(&'static str, &'static [&'static str]),
}

/// One generator kind of a family crate.
pub struct Kind {
    m: crate::Manifest,
    g: GenDecl,
    frame: Frame,
    tone: Option<Tone>,
    gate: Option<Gate>,
    synth: Option<Synth>,
}

impl Kind {
    /// The kind sounds: its level and, when not every sample sounds, its gate.
    pub fn with_tone(mut self, tone: Tone, gate: Option<Gate>) -> Kind {
        self.tone = Some(tone);
        self.gate = gate;
        self
    }

    /// The kind synthesizes its sound (a sound kind; see `sound`).
    pub fn with_synth(mut self, synth: Synth) -> Kind {
        self.synth = Some(synth);
        self
    }
}

/// A sound kind: no picture (its frame is a transparent fill nobody composites, the bin card writes
/// its name over it), the `gen:` path tokens and the synthesizer.
pub fn sound(
    id: &'static str,
    name_key: &'static str,
    category: &'static str,
    rows: Vec<Row>,
    path: &'static [PathTok],
    synth: Synth,
) -> Kind {
    let g = GenDecl { alpha: false, live: false, blank: true, path, address: &[], menu: &[], names: None, colors: &[] };
    kind(id, name_key, category, rows, g, |_, _, w, h| fill(w, h, [0, 0, 0, 0])).with_synth(synth)
}

impl crate::Plugin for Kind {
    fn manifest(&self) -> &crate::Manifest {
        &self.m
    }
    fn generator(&self) -> Option<&dyn GenKernel> {
        Some(self)
    }
}

impl GenKernel for Kind {
    fn frame(&self, p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
        (self.frame)(p, pts, w, h)
    }
    fn tone(&self, p: &[f64]) -> Option<f64> {
        self.tone.and_then(|t| t(p))
    }
    fn gate(&self, p: &[f64], n: i64) -> bool {
        self.gate.is_none_or(|g| g(p, n))
    }
    fn decl(&self) -> &GenDecl {
        &self.g
    }
    fn has_tone(&self) -> bool {
        self.tone.is_some()
    }
    fn synth(&self) -> Option<Synth> {
        self.synth
    }
}

/// A passport row of a kind with its storage, in the former variant's FIELD order.
pub enum Row {
    /// A number row.
    Num(crate::ParamSpec, crate::Store),
    /// A flag row with its default.
    Flag(&'static str, bool),
    /// A choice row with its file keys (the default is the first).
    Choice(&'static str, &'static [&'static str]),
}

/// A smooth number row.
pub fn smooth(n: &'static str, lo: f64, hi: f64, d: f64, st: f64, u: crate::ParamUnit) -> Row {
    Row::Num(crate::ParamSpec::num(n, lo, hi, d, st, u, crate::ParamKind::Smooth), crate::Store::F64)
}

/// A colour component row (`u8`, 0..255).
pub fn rgb(n: &'static str, d: f64) -> Row {
    Row::Num(
        crate::ParamSpec::num(n, 0.0, 255.0, d, 1.0, crate::ParamUnit::Rgb8, crate::ParamKind::Smooth),
        crate::Store::U8,
    )
}

/// A structural count row (`u32`).
pub fn count(n: &'static str, lo: f64, hi: f64, d: f64) -> Row {
    Row::Num(
        crate::ParamSpec::num(n, lo, hi, d, 1.0, crate::ParamUnit::Count, crate::ParamKind::Structural),
        crate::Store::U32,
    )
}

/// A kind: its id (the file's tag and the path's), name and category (the creation menu's group)
/// catalog keys, the rows, its declarations and the frame arm.
pub fn kind(
    id: &'static str,
    name_key: &'static str,
    category: &'static str,
    rows: Vec<Row>,
    g: GenDecl,
    frame: Frame,
) -> Kind {
    use crate::{ParamSpec, RowDecl, Store};
    let mut passport = Vec::new();
    let mut decls = Vec::new();
    for r in rows {
        let (spec, store) = match r {
            Row::Num(s, st) => (s, st),
            Row::Flag(n, d) => (ParamSpec::flag_row(n, d), Store::Bool),
            Row::Choice(n, keys) => (ParamSpec::choice_row(n, keys, 0), Store::Choice(keys)),
        };
        passport.push(spec);
        decls.push(RowDecl { store, omit_default: false });
    }
    Kind {
        m: crate::Manifest {
            id: id.into(),
            media: crate::Media::Generator,
            passport,
            rows: decls,
            decl: crate::Decl::default(),
            ui: crate::UiDecl { name: id, category_key: category, name_key, ..crate::UiDecl::default() },
        },
        g,
        frame,
        tone: None,
        gate: None,
        synth: None,
    }
}

/// A frame of one colour.
pub fn fill(w: u32, h: u32, c: [u8; 4]) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let mut buf = vec![0u8; n * 4];
    for px in buf.chunks_exact_mut(4) {
        px.copy_from_slice(&c);
    }
    buf.into_boxed_slice()
}

/// A `u8` row's value (`fx_cast!`'s rule: the value is already the field's).
#[inline]
pub fn u8v(v: f64) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// The integer square root (Newton, no float in the pixel loop).
pub fn isqrt(v: i64) -> i64 {
    if v <= 0 {
        return 0;
    }
    let mut x = v as u64;
    let mut r = x;
    let mut t = x.div_ceil(2);
    while t < r {
        r = t;
        t = (r + x / r) / 2;
    }
    x = r;
    x as i64
}

/// Pixel coverage from the signed distance `d` (1/256 px) and the width of the
/// ramp `fe`: inside 255, outside 0, between linear. The same ramp as the
/// outline of a title (kt-text), so the edges of a shape and the edges of a glyph
/// look the same.
pub fn coverage(d: i64, fe: i64) -> i64 {
    let half = fe.max(1) / 2;
    if d <= -half {
        255
    } else if d >= half {
        0
    } else {
        255 - ((d + half) * 255) / fe.max(1)
    }
}

/// lowbias32 (Chris Wellons): the same avalanche hash as in kt-fx::video
/// (`noise_hash`): integer, deterministic, stateless.
#[inline]
pub fn lowbias32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// What every generator shader starts with: the uniform, the buffer of the frame, `pack` (rounds and
/// clamps 0..255 to a straight RGBA8 word) and `shape` (`p0`'s colour under the ramp of a signed
/// distance, as `coverage`). A kind's text adds `fn pixel(x: u32, y: u32) -> u32`.
const SHADER_PRELUDE: &str = "struct U { w: u32, h: u32, a: u32, b: u32, p0: vec4<f32>, p1: vec4<f32>, p2: vec4<f32>, p3: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read_write> frame: array<u32>;
fn pack(c: vec4<f32>) -> u32 {
    let v = vec4<u32>(clamp(floor(c + 0.5), vec4<f32>(0.0), vec4<f32>(255.0)));
    return v.x | (v.y << 8u) | (v.z << 16u) | (v.w << 24u);
}
fn shape(d: f32, fe: f32) -> u32 {
    let cov = clamp(0.5 - d / fe, 0.0, 1.0);
    return pack(vec4<f32>(u.p0.xyz, floor(u.p0.w * floor(cov * 255.0) / 255.0)));
}
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x < u.w && id.y < u.h { frame[id.y * u.w + id.x] = pixel(id.x, id.y); }
}
";

/// The whole compute shader of a kind's `pixel` text.
pub fn shader_text(pixel: &str) -> String {
    format!("{SHADER_PRELUDE}{pixel}")
}

/// A frame from a kind's shader on the compute device (generators round). No device: a transparent
/// frame, as a project with no picture has.
pub fn shader_frame(pixel: &str, w: u32, h: u32, ints: [u32; 2], p: [[f32; 4]; 4]) -> Box<[u8]> {
    use crate::compute::{self, Buf, Init, Job, Step};
    let bytes = w as u64 * h as u64 * 4;
    let blank = || vec![0u8; bytes as usize].into_boxed_slice();
    let Some(host) = compute::host() else { return blank() };
    let mut uniform = Vec::with_capacity(80);
    for v in [w, h, ints[0], ints[1]] {
        uniform.extend_from_slice(&v.to_le_bytes());
    }
    for v in p.iter().flatten() {
        uniform.extend_from_slice(&v.to_le_bytes());
    }
    let shader = shader_text(pixel);
    let step = Step { shader: &shader, uniform: &uniform, read: &[], write: &[Buf(0)], groups: [w.div_ceil(16), h.div_ceil(16), 1] };
    match host.run(&Job { buffers: &[Init::Zero(bytes)], steps: &[step], reads: &[Buf(0)] }) {
        Ok(mut out) => out.remove(0).into_boxed_slice(),
        Err(e) => {
            compute::fail(e);
            blank()
        }
    }
}
