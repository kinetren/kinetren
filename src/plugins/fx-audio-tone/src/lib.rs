//! Tone audio effects as plugins (plugin-api-stage1, stage 3): filter, eq, distortion, bitcrusher,
//! stereo_width. Each node is the R.47 node of kt-fx moved verbatim onto `kt_plugin_api::dsp`; its
//! passport repeats the enum variant's fields in their order, types and defaults (t973 holds the
//! bytes of the file, GOLDEN_AUDIO the bytes of the sound).

use std::sync::Arc;

use kt_plugin_api::dsp::{CH, SmoothedParam, apply_width};
use kt_plugin_api::{
    AudioKernel, AudioState, Decl, HostOpts, Manifest, Media, ParamKind, ParamSpec, ParamUnit,
    Plugin, RowDecl, SlotCurves, Store, UiDecl,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(StereoWidth::new())]
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
