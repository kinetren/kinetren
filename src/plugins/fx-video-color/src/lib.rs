//! Colour video effects as plugins (plugin-api-stage1): color_grade, duotone, posterize, vignette.
//! Posterize is the pilot (D9): the smallest kernel that crosses all four executor positions — the
//! CPU barrier, the CPU tap of the fused pass, its own GPU pass and the GPU tap arm.

use std::sync::Arc;

use kt_plugin_api::{
    Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store,
    UiDecl, VideoClass, VideoKernel,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(Posterize::new())]
}

/// Posterize by channels (R.47): point-wise (inside a geometry series the supersampling softens
/// the steps — accepted, THRESHOLD).
pub struct Posterize {
    m: Manifest,
}

impl Posterize {
    pub fn new() -> Posterize {
        Posterize {
            m: Manifest {
                id: "posterize".into(),
                media: Media::VideoFx,
                // the row the variant declared: levels u32 2..32, default 6
                passport: vec![ParamSpec::num("levels", 2.0, 32.0, 6.0, 1.0, ParamUnit::Count, ParamKind::Smooth)],
                rows: vec![RowDecl { store: Store::U32, omit_default: false }],
                decl: Decl {
                    video_class: Some(VideoClass::PointWise),
                    gpu: Gpu::Wgsl(include_str!("posterize.wgsl")),
                    ..Decl::default()
                },
                ui: UiDecl { name: "Posterize", category_key: "fxreg.cat.color", keywords_key: "fxreg.kw.posterize" },
            },
        }
    }

    fn levels(p: &[f64]) -> u32 {
        (p.first().copied().unwrap_or(6.0).round() as u32).clamp(2, 32)
    }
}

/// `n` levels per channel, RGB only.
#[inline(always)]
fn quantize(n: f64, v: &mut [f64; 4]) {
    for c in v.iter_mut().take(3) {
        let i = (*c * n / 256.0).floor().clamp(0.0, n - 1.0);
        *c = i * 255.0 / (n - 1.0);
    }
}

impl Default for Posterize {
    fn default() -> Posterize {
        Posterize::new()
    }
}

impl Plugin for Posterize {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn video(&self) -> Option<&dyn VideoKernel> {
        Some(self)
    }
}

impl VideoKernel for Posterize {
    /// `levels` clamped and cast once per frame (the variant held a `u32`).
    fn cook(&self, p: &[f64]) -> Cooked {
        [Posterize::levels(p) as f64, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }
    /// Quantization PER CHANNEL (the standard — AE Posterize); alpha untouched.
    fn tap(&self, k: &Cooked, v: &mut [f64; 4]) {
        quantize(k[0], v);
    }
    /// The barrier loop with `n` taken through its `u32` range once per frame: the same value,
    /// and the compiler knows `n - 1 >= 1` (the variant's `u32` field gave it that; without it the
    /// loop was 6 % slower than the variant's, plugin-api-stage1 2.6). Inside the fused pass the
    /// cooked value is read as is — there the per-sample cast cost 8 %.
    fn frame(&self, k: &Cooked, rgba: &mut [u8]) {
        let n = (k[0] as u32).clamp(2, 32) as f64;
        kt_plugin_api::frame_with(rgba, |v| quantize(n, v));
    }
    fn gpu_params(&self, p: &[f64]) -> [f32; 7] {
        [Posterize::levels(p) as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }
}
