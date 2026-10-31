//! Stylizing video effects as barrier plugins (plugin-api-barriers): pixelate.
//! Pixelate is the pilot (D7): one pass, no SIMD twin, no neighbours read — the pass graph's
//! simplest form.

use std::sync::Arc;

use kt_plugin_api::barrier::{BarrierCtx, BarrierKernel, Frame, GpuCtx, Node, Scratch, Slot};
use kt_plugin_api::{
    Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store, UiDecl, UiVariant,
    VideoClass,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(Pixelate::new())]
}

/// Mosaic (R.47): an integer copy of the cell's centre, C = size·K — EXACT parity with the GPU by
/// construction.
pub struct Pixelate {
    m: Manifest,
}

impl Pixelate {
    pub fn new() -> Pixelate {
        Pixelate {
            m: Manifest {
                id: "pixelate".into(),
                media: Media::VideoFx,
                // the row the variant declared: size u32 2..128, default 12
                passport: vec![ParamSpec::num("size", 2.0, 128.0, 12.0, 1.0, ParamUnit::Px, ParamKind::Smooth)],
                rows: vec![RowDecl { store: Store::U32, omit_default: false }],
                decl: Decl { video_class: Some(VideoClass::Barrier), gpu: Gpu::Barrier, ..Decl::default() },
                ui: UiDecl {
                    name: "Pixelate",
                    category_key: "fxreg.cat.style",
                    keywords_key: "fxreg.kw.pixelate",
                    variants: PIXELATE_VARIANTS,
                    ..UiDecl::default()
                },
            },
        }
    }
}

impl Default for Pixelate {
    fn default() -> Pixelate {
        Pixelate::new()
    }
}

const PIXELATE_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.fine", sets: &[("size", 5.0)] },
    UiVariant { name_key: "fxreg.var.coarse", sets: &[("size", 28.0)] },
    UiVariant { name_key: "fxreg.var.censor", sets: &[("size", 64.0)] },
];

impl Plugin for Pixelate {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn barrier(&self) -> Option<&dyn BarrierKernel> {
        Some(self)
    }
}

impl BarrierKernel for Pixelate {
    /// The cell `size` clamped and cast once per frame (the variant held a `u32`).
    fn cook(&self, p: &[f64]) -> Cooked {
        let size = (p.first().copied().unwrap_or(12.0).round().max(0.0) as u32).clamp(2, 128);
        [size as f64, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }

    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let c = k[0] as u32 * cx.res_scale;
        let mut out = scratch.take(f.rgba.len());
        for y in 0..f.h {
            let sy = (y / c * c + c / 2).min(f.h - 1);
            for x in 0..f.w {
                let sx = (x / c * c + c / 2).min(f.w - 1);
                let d = ((y * f.w + x) * 4) as usize;
                let s = ((sy * f.w + sx) * 4) as usize;
                out[d..d + 4].copy_from_slice(&f.rgba[s..s + 4]);
            }
        }
        let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
        scratch.put(f.rgba);
        res
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("pixelate.wgsl")]
    }

    /// One pass: `u0 = (w, h, cell)`.
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        let cell = k[0] as u32 * g.res_scale;
        out.push(Node::Pass {
            shader: 0,
            inputs: [Slot::Prev, Slot::None, Slot::None],
            u: [g.w, g.h, cell, 0, 0, 0, 0, 0],
            f: [0.0; 8],
        });
    }
}
