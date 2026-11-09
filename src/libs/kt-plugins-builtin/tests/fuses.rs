//! t2195 (fused-pointwise-runs; capability `plugins`): the built-in set's answer to "does this stage
//! fuse into a point-wise run" — the colour family's taps do; noise and greenscreen (a barrier kernel
//! of their own at a barrier position), every barrier, geometry and stream plugin, every audio kind,
//! transition and generator do not. A new point-wise plugin changes the list on purpose.

#[test]
fn t2195_the_built_in_fusable_stages() {
    let reg = kt_plugins_builtin::install();
    let fusing: Vec<&str> = reg.plugins().filter(|p| kt_plugin_api::fuses_in_runs(p.as_ref())).map(|p| &*p.manifest().id).collect();
    assert_eq!(fusing, ["color_grade", "duotone", "posterize", "vignette"], "the fusable point-wise stages");
    for p in reg.plugins() {
        let m = p.manifest();
        if let (Some(kt_plugin_api::VideoClass::PointWise), kt_plugin_api::Gpu::Wgsl(w)) = (m.decl.video_class, m.decl.gpu) {
            assert_eq!(kt_plugin_api::impure_tap(&m.id, w), None, "{}: a built-in tap is a pure point function", m.id);
        }
    }
}
