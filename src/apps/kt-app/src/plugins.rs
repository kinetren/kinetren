//! The plugin set the app installs (plugin-api-stage1, D6/D8): the built-in families. A test build
//! also registers `test_tint` through the test door, so t976 can prove that a plugin from another
//! door reaches the picker and the chain, and none of the reference lists.

use std::sync::Arc;

use kt_plugin_api::{Media, Plugin, Registry};

/// Install the process's registry once; every `KtApp` constructor and the picker's registry call it.
pub fn install() -> &'static Arc<Registry> {
    #[cfg(not(test))]
    let reg = kt_plugins_builtin::install();
    #[cfg(test)]
    let reg = kt_plugins_builtin::install_with(test_tint::register);
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    // the memory the preview's stream memo may grow into (stream-layout-per-build)
    kt_engine::video::set_memory_probe(kt_gpu::memory::available);
    kt_time::mem::set_resident_probe(kt_gpu::memory::resident);
    reg
}

/// The installed video-effect plugins in registration order (the picker's plugin rows).
pub fn video_fx() -> impl Iterator<Item = &'static Arc<dyn Plugin>> {
    install().plugins().filter(|p| p.manifest().media == Media::VideoFx)
}

/// The installed audio-effect plugins (the picker lists the ones its hand-made rows do not).
pub fn audio_fx() -> impl Iterator<Item = &'static Arc<dyn Plugin>> {
    install().plugins().filter(|p| p.manifest().media == Media::AudioFx)
}

/// The catalog keys the installed manifests name (t125 counts them as used, t976 requires them).
#[cfg(test)]
pub fn manifest_keys() -> Vec<&'static str> {
    install()
        .plugins()
        .flat_map(|p| {
            let m = p.manifest();
            // a generator's declaration names its eyedroppers' captions and its value names (D13)
            let gen_keys: Vec<&'static str> = p
                .generator()
                .map(|g| {
                    let d = g.decl();
                    d.colors.iter().map(|(_, k)| *k).chain(d.names.iter().flat_map(|(_, ks)| ks.iter().copied())).collect()
                })
                .unwrap_or_default();
            [m.ui.category_key, m.ui.keywords_key, m.ui.name_key]
                .into_iter()
                .chain(m.ui.variants.iter().map(|v| v.name_key))
                .chain(gen_keys)
        })
        .filter(|k| !k.is_empty())
        .collect()
}

#[cfg(test)]
pub mod test_tint {
    //! A point-wise video plugin that only the test door registers (t976).
    use std::sync::Arc;

    use kt_plugin_api::{
        Decl, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, Registry, RowDecl,
        Store, UiDecl, VideoClass, VideoKernel,
    };

    pub const ID: &str = "test_tint";

    pub fn register(reg: &mut Registry) {
        reg.register_for_test(Arc::new(TestTint::new())).expect("the test plugin registers");
    }

    pub struct TestTint {
        m: Manifest,
    }

    impl TestTint {
        fn new() -> TestTint {
            TestTint {
                m: Manifest {
                    id: ID.into(),
                    media: Media::VideoFx,
                    passport: vec![ParamSpec::num(
                        "amount",
                        0.0,
                        1.0,
                        0.5,
                        0.01,
                        ParamUnit::Norm,
                        ParamKind::Smooth,
                    )],
                    rows: vec![RowDecl { store: Store::F64, omit_default: false }],
                    decl: Decl { video_class: Some(VideoClass::PointWise), builtin: false, ..Decl::default() },
                    // the key by its constant, not a dotted literal: live-coverage.py (t804) reads
                    // every quoted dotted string in kt-app's sources as a published UI place
                    ui: UiDecl {
                        name: "Test Tint",
                        category_key: crate::i18n::key_name(crate::i18n::k::fxreg::cat::COLOR),
                        keywords_key: "",
                        ..UiDecl::default()
                    },
                },
            }
        }
    }

    impl Plugin for TestTint {
        fn manifest(&self) -> &Manifest {
            &self.m
        }
        fn video(&self) -> Option<&dyn VideoKernel> {
            Some(self)
        }
    }

    impl VideoKernel for TestTint {
        /// Pulls red toward 255 by `amount`.
        fn gpu_params(&self, p: &[f64]) -> [f32; 7] {
            [p.first().copied().unwrap_or(0.5) as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        }
    }
}
