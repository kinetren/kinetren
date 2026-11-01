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
    pub const LINES: usize = 2;
    pub const BLOCK: usize = 3;
    pub const SEARCH: usize = 4;
    pub const BEFORE: usize = 5;
    pub const DROP_AT: usize = 6;
    pub const DROP_COUNT: usize = 7;
    pub const BLOOM_AT: usize = 8;
    pub const BLOOM_LEN: usize = 9;
    pub const FREEZE_AT: usize = 10;
    pub const FREEZE_LEN: usize = 11;
    pub const MV_SCALE: usize = 12;
    pub const MV_ROTATE: usize = 13;
    pub const MV_SWAP: usize = 14;
    pub const VECTORS_FROM: usize = 15;
    pub const RESIDUAL: usize = 16;
    pub const LOSS: usize = 17;
    pub const SEED: usize = 18;
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

/// Every row is STRUCTURAL: a curve on `gop` or `drop_at` would make the anchor of frame N depend
/// on the curve at every earlier frame (design, Risks) — deferred, not forgotten.
fn num(name: &'static str, min: f64, max: f64, def: f64, step: f64, unit: ParamUnit) -> ParamSpec {
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
                    num("gop", 1.0, 60.0, 24.0, 1.0, ParamUnit::Count),
                    num("qp", 0.0, 51.0, 24.0, 1.0, ParamUnit::Count),
                    ParamSpec::choice_row("lines", mosh::LINES, 2),
                    ParamSpec::choice_row("block", mosh::BLOCKS, 0),
                    num("search", 1.0, 32.0, 16.0, 1.0, ParamUnit::Px),
                    num("before", 0.0, 60.0, 0.0, 1.0, ParamUnit::Count),
                    num("drop_at", 0.0, 3600.0, 0.0, 1.0, ParamUnit::Count),
                    num("drop_count", 0.0, 4.0, 0.0, 1.0, ParamUnit::Count),
                    num("bloom_at", 0.0, 3600.0, 0.0, 1.0, ParamUnit::Count),
                    num("bloom_len", 0.0, 60.0, 0.0, 1.0, ParamUnit::Count),
                    num("freeze_at", 0.0, 3600.0, 0.0, 1.0, ParamUnit::Count),
                    num("freeze_len", 0.0, 300.0, 0.0, 1.0, ParamUnit::Count),
                    num("mv_scale", -4.0, 4.0, 1.0, 0.05, ParamUnit::Ratio),
                    num("mv_rotate", -180.0, 180.0, 0.0, 1.0, ParamUnit::Deg),
                    ParamSpec::flag_row("mv_swap", false),
                    ParamSpec::choice_row("vectors_from", mosh::FROM, 0),
                    num("residual", 0.0, 1.0, 1.0, 0.01, ParamUnit::Percent),
                    num("loss", 0.0, 1.0, 0.0, 0.01, ParamUnit::Percent),
                    num("seed", 0.0, 9999.0, 0.0, 1.0, ParamUnit::Count),
                ],
                rows: vec![
                    u32_row,
                    u32_row,
                    RowDecl { store: Store::Choice(mosh::LINES), omit_default: true },
                    RowDecl { store: Store::Choice(mosh::BLOCKS), omit_default: true },
                    u32_row,
                    u32_row,
                    u32_row,
                    u32_row,
                    u32_row,
                    u32_row,
                    u32_row,
                    u32_row,
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
                    variants: VARIANTS,
                    // the first click is the classic: the I-frame at the cut is gone
                    template: &[("before", 12.0), ("drop_count", 1.0)],
                    ..UiDecl::default()
                },
            },
        }
    }
}

/// The presets (task 2.3): the classic cut mosh, bloom, a vector transplant, a melt.
pub const VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.mosh_cut", sets: &[("before", 12.0), ("drop_count", 1.0)] },
    UiVariant { name_key: "fxreg.var.mosh_bloom", sets: &[("bloom_at", 12.0), ("bloom_len", 24.0)] },
    UiVariant {
        name_key: "fxreg.var.mosh_transplant",
        sets: &[("vectors_from", 1.0), ("residual", 0.3), ("gop", 60.0)],
    },
    UiVariant {
        name_key: "fxreg.var.mosh_melt",
        sets: &[("before", 12.0), ("drop_count", 4.0), ("gop", 48.0), ("residual", 0.25), ("qp", 30.0)],
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
