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

/// The material tick a frame is drawn at (re-exported: a family crate depends on the API only, t975).
pub use kt_time::Tick;

/// The former `render_at` arm: straight RGBA of EXACTLY `w×h`; `pts` is the MATERIAL tick.
pub type Frame = fn(&[f64], Tick, u32, u32) -> Box<[u8]>;
/// The level in dBFS when the kind sounds with these values; `None` — silent media.
pub type Tone = fn(&[f64]) -> Option<f64>;
/// Whether global sample `n` (48 kHz) sounds; the default is every sample.
pub type Gate = fn(&[f64], i64) -> bool;

/// A generator's kernel (D13).
pub trait GenKernel: Send + Sync {
    fn frame(&self, p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]>;
    fn tone(&self, p: &[f64]) -> Option<f64>;
    fn gate(&self, p: &[f64], n: i64) -> bool;
    fn decl(&self) -> &GenDecl;
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
}

impl Kind {
    /// The kind sounds: its level and, when not every sample sounds, its gate.
    pub fn with_tone(mut self, tone: Tone, gate: Option<Gate>) -> Kind {
        self.tone = Some(tone);
        self.gate = gate;
        self
    }
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
pub fn lowbias32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}
