//! Test 544 (item 428, R.54): the effect thumbnail.
//!
//! The thumbnail is what a person CHOOSES an effect by, so it has two duties,
//! and both are checked here: to be deterministic (the icon has no right to flicker
//! from run to run) and to SHOW something — the sample must give each kind of effect
//! material on which that kind is visible. The reference of the sample is frozen by a hash:
//! an edit to `test_pattern` changes all the program's icons at once, and it may be
//! done only deliberately.

use kt_fx::video::{Frame, fit_into, test_pattern};
use kt_fx::video::VideoFxCtx;
use kt_model::FxInstance;
use xxhash_rust::xxh3::xxh3_64;

/// The reference size — the size of the source's thumb (kt-app: 320×180): it is on
/// this size that the service computes the chain before shrinking the result into the cell.
const RW: u32 = 320;
const RH: u32 = 180;

fn hash(f: &Frame) -> u64 {
    xxh3_64(&f.rgba)
}

fn thumb(base: &Frame, fx: &[FxInstance]) -> Frame {
    let mut rgba = vec![0u8; (base.w * base.h * 4) as usize];
    fit_into(base, base.w, base.h, &mut rgba);
    let f = Frame { w: base.w, h: base.h, rgba, has_alpha: base.has_alpha };
    let ctx = VideoFxCtx { out_w: base.w, out_h: base.h, u: 0.5, burst_index: 0, cam3d: None, in_scale: 1, lead: None };
    kt_gpu::testkit::chain(fx, f, &ctx)
}

/// Goldens of the sample. They change ONLY together with a deliberate edit to
/// `test_pattern` — this is the reference by which the icons of all kinds look.
const GOLDEN_PATTERN: [(&str, u32, u32, bool, u64); 3] = [
    ("ref", RW, RH, false, 0x923630d77778ab3f),
    ("ref-alpha", RW, RH, true, 0x36a58a5ecdcc259a),
    ("icon", 96, 54, false, 0x4621960666a971f2),
];

#[test]
fn t544_thumb_pattern_is_frozen_and_deterministic() {
    kt_plugins_builtin::install();
    for (name, w, h, alpha, want) in GOLDEN_PATTERN {
        let a = test_pattern(w, h, alpha);
        let b = test_pattern(w, h, alpha);
        assert_eq!(hash(&a), hash(&b), "{name}: pattern is not deterministic");
        assert_eq!(a.w, w.max(4));
        assert_eq!(a.h, h.max(4));
        assert_eq!(a.rgba.len(), (a.w as usize) * (a.h as usize) * 4);
        assert_eq!(a.has_alpha, alpha);
        assert_eq!(hash(&a), want, "{name}: pattern hash moved (0x{:016x})", hash(&a));
    }
}

/// The sample must give EVERY effect class material: otherwise the icon would honestly
/// show “nothing changed” and lie about the effect.
#[test]
fn t544b_pattern_feeds_every_class_of_effect() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    // FxInstance::all() holds the built-in plugins (plugin-api-stage1)
    kt_plugins_builtin::install();
    let p = test_pattern(RW, RH, false);
    let base = hash(&p);

    // asymmetry: a mirror on ANY axis is visible
    for (h, v) in [(true, false), (false, true), (true, true)] {
        let m = thumb(&p, &[kt_model::FxInstance::plugin_with("mirror", &[f64::from(u8::from(h)), f64::from(u8::from(v))]).unwrap()]);
        assert_ne!(hash(&m), base, "mirror {h}/{v}: pattern is symmetric");
    }
    // keying: the green plate gives transparency
    let g = thumb(&p, &[FxInstance::greenscreen([0, 255, 0], 70, 25)]);
    assert!(g.has_alpha, "greenscreen: no alpha in the result");
    assert!(
        g.rgba.chunks_exact(4).any(|px| px[3] == 0),
        "greenscreen: nothing was keyed out (pattern has no green patch?)"
    );
    // high frequencies: blur and outline change the frame
    for fx in [FxInstance::blur(8), FxInstance::edges(48), FxInstance::builtin("sharpen")] {
        let f = thumb(&p, std::slice::from_ref(&fx));
        assert_ne!(hash(&f), base, "{}: pattern has no detail to work on", fx.tag());
    }
    // colour: the ramp and the bars
    for fx in [FxInstance::builtin("posterize"), FxInstance::builtin("duotone"), FxInstance::vhs(4, 0.4)] {
        let f = thumb(&p, std::slice::from_ref(&fx));
        assert_ne!(hash(&f), base, "{}: pattern has no colour to work on", fx.tag());
    }
}

/// An identity chain does not touch the bytes, but a VARIANT does. This is the condition for the
/// icon: for Pan/Pan 3D/Color Grade/Corner Pin/Drop Shadow the template is
/// identity, and the first variant serves such a kind as its icon (kt-app,
/// t543). If the variant turned out to be a copy of the template, the icon would show
/// the source frame — and the test on the kt-app side would not see this, since it
/// compares DATA, not the picture.
#[test]
fn t544c_identity_is_identity_and_a_variant_is_not() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    // color_grade is a plugin (plugin-api-stage1, stage 4)
    kt_plugins_builtin::install();
    let p = test_pattern(RW, RH, false);
    let base = hash(&p);
    assert_eq!(hash(&thumb(&p, &[])), base, "empty chain moved the bytes");

    let identity = FxInstance::geometry_zoom(1.0);
    assert_eq!(hash(&thumb(&p, &[identity])), base, "Pan defaults are not identity any more");
    let zoomed = FxInstance::geometry_zoom(1.25);
    assert_ne!(hash(&thumb(&p, &[zoomed])), base, "a zoom variant shows nothing");

    // Color Grade and Corner Pin — the same identities by template
    assert_eq!(hash(&thumb(&p, &[FxInstance::builtin("color_grade")])), base);
    assert_eq!(hash(&thumb(&p, &[FxInstance::builtin("corner_pin")])), base);
}

/// A shadow exists only where there is transparency: on an opaque
/// frame `DropShadow` is identical BY CONSTRUCTION (the shadow falls from under the
/// alpha). Therefore the sample has two kinds, and the icon of the shadow effects needs
/// precisely the alpha variant.
#[test]
fn t544d_drop_shadow_needs_the_alpha_pattern() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    kt_plugins_builtin::install();
    let opaque = test_pattern(RW, RH, false);
    let alpha = test_pattern(RW, RH, true);
    let fx = [FxInstance::builtin("drop_shadow")];
    assert_eq!(
        hash(&thumb(&opaque, &fx)),
        hash(&opaque),
        "drop shadow on an opaque frame is no longer a passthrough"
    );
    assert_ne!(
        hash(&thumb(&alpha, &fx)),
        hash(&alpha),
        "drop shadow shows nothing even on the alpha pattern"
    );
}

