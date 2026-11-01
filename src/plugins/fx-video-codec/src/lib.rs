//! The codec family (datamosh-codec-path): `datamosh` — a SIMULATION of a broken inter-frame
//! stream. The effect runs its own block codec (motion estimation, a closed prediction loop over a
//! reference buffer, a quantised DCT residual, a GOP) and breaks the stream the decoder receives by
//! its rows: dropped I-frames, a repeated P-frame (bloom), a frozen reference, edited or
//! transplanted vectors, lost residual and lost macroblocks (F-FX-011).
//!
//! It is a `VideoClass::Stream` source stage: the host runs it on the CPU in the plan's common code
//! and hands both executors the decoded picture (design D2, D4).

pub mod codec;
pub mod mosh;

use std::sync::Arc;

use kt_plugin_api::stream::{SecondFrom, Side, StreamInput, StreamKernel, StreamLayout, StreamMemo, StreamOut};
use kt_plugin_api::{
    Decl, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store, UiDecl, UiVariant, VideoClass,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(Datamosh::new())]
}

/// Row indices (passport order).
pub mod rows {
    pub const GOP: usize = 0;
    pub const QP: usize = 1;
    pub const LINES_ROW: usize = 2;
    pub const BLOCK: usize = 3;
    pub const SEARCH: usize = 4;
    pub const BEFORE: usize = 5;
    pub const MOSH: usize = 6;
    pub const RESET: usize = 7;
    pub const BLOOM: usize = 8;
    pub const FREEZE: usize = 9;
    pub const MV_SCALE: usize = 10;
    pub const MV_ROTATE: usize = 11;
    pub const MV_SWAP: usize = 12;
    pub const VECTORS_FROM: usize = 13;
    pub const RESIDUAL: usize = 14;
    pub const LOSS: usize = 15;
    pub const SEED: usize = 16;
}

/// The datamosh: an own codec broken by its rows.
pub struct Datamosh {
    m: Manifest,
}

impl Default for Datamosh {
    fn default() -> Datamosh {
        Datamosh::new()
    }
}

/// A codec quantity that may change from frame to frame — it takes curves (the owner, 2026-11-01:
/// "this effect has no curves"). The kernel reads it per position (`StreamLayout::rows_at`).
fn live(name: &'static str, min: f64, max: f64, def: f64, step: f64, unit: ParamUnit) -> ParamSpec {
    ParamSpec::num(name, min, max, def, step, unit, ParamKind::Smooth)
}

/// A row that shapes the stream's STRUCTURE (its size, its block grid, where it starts): no curve.
fn structural(name: &'static str, min: f64, max: f64, def: f64, step: f64, unit: ParamUnit) -> ParamSpec {
    ParamSpec::num(name, min, max, def, step, unit, ParamKind::Structural)
}

impl Datamosh {
    pub fn new() -> Datamosh {
        let u32_row = RowDecl { store: Store::U32, omit_default: true };
        let f64_row = RowDecl { store: Store::F64, omit_default: true };
        Datamosh {
            m: Manifest {
                id: "datamosh".into(),
                media: Media::VideoFx,
                passport: vec![
                    // 0 — keyframes only at the stream start, the cut and the resets
                    structural("gop", 0.0, 600.0, 24.0, 1.0, ParamUnit::Count),
                    live("qp", 0.0, 51.0, 24.0, 1.0, ParamUnit::Count),
                    ParamSpec::choice_row("lines", mosh::LINES, 2),
                    ParamSpec::choice_row("block", mosh::BLOCKS, 0),
                    structural("search", 1.0, 64.0, 16.0, 1.0, ParamUnit::Px),
                    structural("before", 0.0, 300.0, 0.0, 1.0, ParamUnit::Count),
                    // the gates: on at 1, off at 0; drawn by curves (a Hold key on the beat)
                    live("mosh", 0.0, 1.0, 0.0, 1.0, ParamUnit::None),
                    live("reset", 0.0, 1.0, 0.0, 1.0, ParamUnit::None),
                    live("bloom", 0.0, 1.0, 0.0, 1.0, ParamUnit::None),
                    live("freeze", 0.0, 1.0, 0.0, 1.0, ParamUnit::None),
                    live("mv_scale", -16.0, 16.0, 1.0, 0.05, ParamUnit::Ratio),
                    live("mv_rotate", -180.0, 180.0, 0.0, 1.0, ParamUnit::Deg),
                    ParamSpec::flag_row("mv_swap", false),
                    ParamSpec::choice_row("vectors_from", mosh::FROM, 0),
                    live("residual", 0.0, 1.0, 1.0, 0.01, ParamUnit::Percent),
                    live("loss", 0.0, 1.0, 0.0, 0.01, ParamUnit::Percent),
                    structural("seed", 0.0, 9999.0, 0.0, 1.0, ParamUnit::Count),
                ],
                rows: vec![
                    u32_row,
                    f64_row,
                    RowDecl { store: Store::Choice(mosh::LINES), omit_default: true },
                    RowDecl { store: Store::Choice(mosh::BLOCKS), omit_default: true },
                    u32_row,
                    u32_row,
                    f64_row,
                    f64_row,
                    f64_row,
                    f64_row,
                    f64_row,
                    f64_row,
                    RowDecl { store: Store::Bool, omit_default: true },
                    RowDecl { store: Store::Choice(mosh::FROM), omit_default: true },
                    f64_row,
                    f64_row,
                    u32_row,
                ],
                decl: Decl { video_class: Some(VideoClass::Stream), ..Decl::default() },
                ui: UiDecl {
                    name: "Datamosh",
                    category_key: "fxreg.cat.style",
                    keywords_key: "fxreg.kw.datamosh",
                    // the template is the intact codec (its look: the encoded height and the
                    // quantiser); every breakage is a named variant, the classic cut first
                    variants: VARIANTS,
                    ..UiDecl::default()
                },
            },
        }
    }
}

/// The presets (task 2.3): the classic cut mosh (every I-frame after the stream start dropped —
/// the avidemux method; a `reset` curve key ends it on the beat), bloom, a vector transplant, a melt.
pub const VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.mosh_cut", sets: &[("before", 12.0), ("mosh", 1.0)] },
    UiVariant { name_key: "fxreg.var.mosh_bloom", sets: &[("bloom", 1.0)] },
    UiVariant {
        name_key: "fxreg.var.mosh_transplant",
        sets: &[("vectors_from", 1.0), ("residual", 0.3), ("gop", 0.0)],
    },
    UiVariant {
        name_key: "fxreg.var.mosh_melt",
        sets: &[("before", 12.0), ("mosh", 1.0), ("residual", 0.25), ("qp", 30.0)],
    },
];

impl Plugin for Datamosh {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn stream(&self) -> Option<&dyn StreamKernel> {
        Some(self)
    }
}

impl StreamKernel for Datamosh {
    fn lines(&self, p: &[f64]) -> u32 {
        mosh::Rows::from(p).lines
    }
    fn before(&self, p: &[f64]) -> u32 {
        mosh::Rows::from(p).before
    }
    fn second(&self, p: &[f64]) -> SecondFrom {
        mosh::Rows::from(p).second
    }
    fn needs(&self, p: &[f64], lay: &dyn StreamLayout, n: i64, memo: &StreamMemo) -> Vec<(Side, i64)> {
        mosh::needs(mosh::Rows::from(p), lay, n, memo)
    }
    fn render(&self, p: &[f64], input: &dyn StreamInput, n: i64, memo: &StreamMemo, out: &mut Vec<u8>) -> Option<StreamOut> {
        let (pic, key) = mosh::render(mosh::Rows::from(p), input, n, memo)?;
        let (w, h) = input.size();
        pic.to_rgba(w, h, out);
        Some(StreamOut { w: w.min(pic.w as u32), h: h.min(pic.h as u32), key })
    }
}
