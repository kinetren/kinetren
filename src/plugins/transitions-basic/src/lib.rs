//! The basic transition kinds as plugins (plugin-api-stage1, stage 5, D12). Each kernel is the former
//! `TransMix::of` arm (its `pack`) and `kernel()` arm (its `pixel`) of kt-engine's `transitions.rs`,
//! verbatim; its WGSL is the former `trans()` arm of composite.wgsl. t1019 holds the pixels against
//! the enum, t224 the GPU against the CPU, t973 the file text.

use std::sync::Arc;

use kt_plugin_api::trans::{Canvas, Img, front_mask, mix4};
use kt_plugin_api::{
    Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store, TransKernel, UiDecl,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(Wipe::new())]
}

/// The direction row's keys (the file holds the former `Dir` text).
pub const DIRS: &[&str] = &["left", "right", "up", "down"];

/// The unit vector of a direction row's index in frame coordinates (X right, Y down) — `Dir::vec`.
fn dir_vec(i: f64) -> (f32, f32) {
    match i as u32 {
        1 => (1.0, 0.0),
        2 => (0.0, -1.0),
        3 => (0.0, 1.0),
        _ => (-1.0, 0.0),
    }
}

fn smooth(n: &'static str, lo: f64, hi: f64, d: f64, st: f64, u: ParamUnit) -> ParamSpec {
    ParamSpec::num(n, lo, hi, d, st, u, ParamKind::Smooth)
}

/// A row always written, in its field's type.
fn row(store: Store) -> RowDecl {
    RowDecl { store, omit_default: false }
}

fn manifest(id: &'static str, name: &'static str, category: &'static str, passport: Vec<ParamSpec>, rows: Vec<RowDecl>, wgsl: &'static str) -> Manifest {
    Manifest {
        id: id.into(),
        media: Media::Transition,
        passport,
        rows,
        decl: Decl { gpu: Gpu::Wgsl(wgsl), ..Decl::default() },
        ui: UiDecl { name, category_key: category, name_key: "", ..UiDecl::default() },
    }
}

// ---------------------------------------------------------------------------
// wipe
// ---------------------------------------------------------------------------

/// The edge moves across the frame: a linear wipe, `angle` gives a diagonal.
pub struct Wipe {
    m: Manifest,
}

impl Wipe {
    pub fn new() -> Wipe {
        let mut m = manifest(
            "wipe",
            "Wipe",
            "trn.cat.wipe",
            vec![
                ParamSpec::choice_row("dir", DIRS, 0),
                smooth("angle", -180.0, 180.0, 0.0, 1.0, ParamUnit::Deg),
                smooth("softness", 0.0, 1.0, 0.15, 0.01, ParamUnit::Norm),
            ],
            vec![row(Store::Choice(DIRS)), row(Store::F64), row(Store::F64)],
            include_str!("wipe.wgsl"),
        );
        m.ui.name_key = "trn.kind.wipe";
        Wipe { m }
    }
}

impl Default for Wipe {
    fn default() -> Wipe {
        Wipe::new()
    }
}

impl Plugin for Wipe {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn trans(&self) -> Option<&dyn TransKernel> {
        Some(self)
    }
}

impl TransKernel for Wipe {
    fn pack(&self, v: &[f64]) -> [f32; 8] {
        let mut p = [0.0f32; 8];
        let (dx, dy) = dir_vec(v[0]);
        let a = (v[1] as f32).to_radians();
        let (s, c) = (a.sin(), a.cos());
        // the angle turns the direction — the diagonal is obtained by it
        p[0] = dx * c - dy * s;
        p[1] = dx * s + dy * c;
        p[2] = v[2] as f32;
        p
    }

    fn pixel(&self, a: &Img<'_>, b: &Img<'_>, u: f32, v: f32, k: f32, p: &[f32; 8], _cx: &Canvas, _ext: &[f32; 16]) -> [f32; 4] {
        // projection onto the direction, normalized by the square's span
        let (dx, dy) = (p[0], p[1]);
        let ext = (dx.abs() + dy.abs()).max(1e-6);
        let s = ((u - 0.5) * dx + (v - 0.5) * dy + ext * 0.5) / ext;
        mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
    }
}
