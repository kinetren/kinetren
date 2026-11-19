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
    // the slots in front of a stream stage run on the compute device and feed the codec there
    // (stream-front-on-device)
    kt_engine::video::install_stream_front(kt_gpu::stream_front_engine);
    // the memory the preview's stream memo may grow into (stream-layout-per-build)
    kt_engine::video::set_memory_probe(kt_gpu::memory::available);
    kt_time::mem::set_resident_probe(kt_gpu::memory::resident);
    #[cfg(test)]
    crash_report_for_tests();
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

/// A test process that dies natively AFTER its test passed (found-defects 695, 785, 784: "double free
/// or corruption" at the exit of a reel test under a loaded full run, never reproduced alone) leaves a
/// raw report naming the thread and the stack: `<temp>/kt-test-crashes/<binary>-<pid>.raw`, the test in
/// its header; `kt_crash::symbolize_file` reads it with the same build. The system's core handler keeps
/// nothing of an unpackaged binary, so before this such an abort left no trace but its signal.
#[cfg(test)]
fn crash_report_for_tests() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join("kt-test-crashes");
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let exe = std::env::current_exe().ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_default();
        let args: Vec<String> = std::env::args().skip(1).collect();
        kt_crash::install(&kt_crash::Config {
            raw: dir.join(format!("{exe}-{}.raw", std::process::id())),
            header: format!("kinetren test binary {} — {}", kt_version::FULL, args.join(" ")),
            symbolizer: None,
        });
    });
}

/// t2979 (found-defects 784): a kt-app test process that aborts after installing the plugin set — what
/// every harness test does — leaves the raw report with the signal and the test's name in its header.
#[cfg(test)]
mod crash_report_tests {
    const CHILD: &str = "KT_T2979_CHILD";

    #[test]
    fn t2979_a_test_process_that_aborts_leaves_a_report() {
        if std::env::var_os(CHILD).is_some() {
            super::install();
            std::process::abort();
        }
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "plugins::crash_report_tests::t2979_a_test_process_that_aborts_leaves_a_report", "--nocapture"])
            .env(CHILD, "1")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .expect("the child runs");
        assert!(!out.status.success(), "the child aborted");
        let exe = std::env::current_exe().unwrap().file_stem().unwrap().to_string_lossy().into_owned();
        let dir = std::env::temp_dir().join("kt-test-crashes");
        // the child's pid is not ours: the newest report of this binary written since the start
        let since = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
        let raw = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(&format!("{exe}-")))
            .filter(|e| e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t >= since))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok().map(|t| (e.path(), t)))
            .find(|(_, t)| t.contains("t2979_a_test_process_that_aborts_leaves_a_report"));
        let (path, text) = raw.expect("a raw report of the aborted child");
        assert!(text.contains("signal: 6"), "the signal is named: {text:.300}");
        let _ = std::fs::remove_file(path);
    }
}
