//! Test 544 (docs/porting-notes.md, item 428, R.54): the effect thumbnail.
//!
//! The thumbnail is what a person CHOOSES an effect by, so it has two duties,
//! and both are checked here: to be deterministic (the icon has no right to flicker
//! from run to run) and to SHOW something — the sample must give each kind of effect
//! material on which that kind is visible. The reference of the sample is frozen by a hash:
//! an edit to `test_pattern` changes all the program's icons at once, and it may be
//! done only deliberately.

use kt_fx::video::{Frame, apply_video_chain, fit_into, render_fx_thumb, test_pattern};
use kt_fx::video::{FxScratch, VideoFxCtx};
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
    let mut scratch = FxScratch::default();
    render_fx_thumb(base, fx, RW, RH, 0.5, 90_000, None, &mut scratch)
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
    let p = test_pattern(RW, RH, false);
    let base = hash(&p);

    // asymmetry: a mirror on ANY axis is visible
    for (h, v) in [(true, false), (false, true), (true, true)] {
        let m = thumb(&p, &[FxInstance::Mirror { horizontal: h, vertical: v }]);
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
    for fx in [FxInstance::blur(8), FxInstance::edges(48), FxInstance::sharpen()] {
        let f = thumb(&p, std::slice::from_ref(&fx));
        assert_ne!(hash(&f), base, "{}: pattern has no detail to work on", fx.tag());
    }
    // colour: the ramp and the bars
    for fx in [FxInstance::posterize(), FxInstance::duotone(), FxInstance::vhs(4, 0.4)] {
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
    let p = test_pattern(RW, RH, false);
    let base = hash(&p);
    assert_eq!(hash(&thumb(&p, &[])), base, "empty chain moved the bytes");

    let identity = FxInstance::geometry_zoom(1.0);
    assert_eq!(hash(&thumb(&p, &[identity])), base, "Pan defaults are not identity any more");
    let zoomed = FxInstance::geometry_zoom(1.25);
    assert_ne!(hash(&thumb(&p, &[zoomed])), base, "a zoom variant shows nothing");

    // Color Grade and Corner Pin — the same identities by template
    assert_eq!(hash(&thumb(&p, &[FxInstance::color_grade()])), base);
    assert_eq!(hash(&thumb(&p, &[FxInstance::corner_pin()])), base);
}

/// A shadow exists only where there is transparency: on an opaque
/// frame `DropShadow` is identical BY CONSTRUCTION (the shadow falls from under the
/// alpha). Therefore the sample has two kinds, and the icon of the shadow effects needs
/// precisely the alpha variant.
#[test]
fn t544d_drop_shadow_needs_the_alpha_pattern() {
    let opaque = test_pattern(RW, RH, false);
    let alpha = test_pattern(RW, RH, true);
    let fx = [FxInstance::drop_shadow()];
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

/// The thumbnail is the same chain as the engine's: `render_fx_thumb` must be
/// exactly “fit + apply”, with no arithmetic of its own. Otherwise the picture in the
/// picker would diverge from what results after a click (invariant No. 7 in
/// spirit: one composition code, the size is an execution parameter).
#[test]
fn t544e_thumb_is_fit_plus_chain_and_nothing_else() {
    let p = test_pattern(RW, RH, false);
    let fx = [FxInstance::blur(6), FxInstance::vhs(4, 0.4)];
    let (w, h) = (160, 90);

    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    fit_into(&p, w, h, &mut buf);
    let fitted = Frame { w, h, rgba: buf, has_alpha: p.has_alpha };
    let want = apply_video_chain(
        &fx,
        fitted,
        &VideoFxCtx {
            out_w: w,
            out_h: h,
            u: 0.5,
            burst_index: 90_000,
            cam3d: None,
            in_scale: 1,
            lead: None,
        },
    );

    let mut scratch = FxScratch::default();
    let got = render_fx_thumb(&p, &fx, w, h, 0.5, 90_000, None, &mut scratch);
    assert_eq!(hash(&got), hash(&want), "render_fx_thumb is not fit_into + apply_video_chain");

    // the same pool, second run: a “dirty” pool does not change the bytes (t68)
    let again = render_fx_thumb(&p, &fx, w, h, 0.5, 90_000, None, &mut scratch);
    assert_eq!(hash(&again), hash(&want), "a reused scratch pool changed the bytes");
}

/// The noise grain is pinned to `burst_index`: with a still cursor the cell
/// must be one and the same picture, otherwise the strip flickers on every
/// redrawn frame of the interface.
#[test]
fn t544f_noise_is_pinned_to_the_tick() {
    let p = test_pattern(RW, RH, false);
    let fx = [FxInstance::noise(0.25)];
    let mut s = FxScratch::default();
    let a = render_fx_thumb(&p, &fx, RW, RH, 0.5, 90_000, None, &mut s);
    let b = render_fx_thumb(&p, &fx, RW, RH, 0.5, 90_000, None, &mut s);
    let other = render_fx_thumb(&p, &fx, RW, RH, 0.5, 270_000, None, &mut s);
    assert_eq!(hash(&a), hash(&b), "same tick gave two different thumbs");
    assert_ne!(hash(&a), hash(&other), "noise ignores the tick");
}
