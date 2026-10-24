//! Test 550 (item 433, R.54): blending two states of one kind —
//! an operation under the FORCE AXIS in the picker.
//!
//! Its main property, the reason it exists at all: the ends are returned BYTE-FOR-BYTE.
//! This is what the rule “a click on a cell gives exactly its variant” rests on: if `t == 1`
//! returned a rebuilt value, a click on “Tight” after dragging could give something
//! half a unit different,
//! and no layout test would ever notice.

use kt_model::{FxInstance, PanMode};

fn dbg(f: &FxInstance) -> String {
    format!("{f:?}")
}

#[test]
fn t550_lerp_keeps_the_ends_and_moves_the_numbers() {
    let a = FxInstance::blur(4);
    let b = FxInstance::blur(24);

    // the ends — byte-for-byte, without rebuilding
    assert_eq!(dbg(&a.lerp(&b, 0.0)), dbg(&a));
    assert_eq!(dbg(&a.lerp(&b, 1.0)), dbg(&b));
    // and beyond 0..1 as well (the value is clamped)
    assert_eq!(dbg(&a.lerp(&b, -5.0)), dbg(&a));
    assert_eq!(dbg(&a.lerp(&b, 17.0)), dbg(&b));

    // the middle — halfway; an integer field is rounded, not truncated
    assert_eq!(a.lerp(&b, 0.5).get_param("radius"), Some(14.0));
    assert_eq!(a.lerp(&b, 0.25).get_param("radius"), Some(9.0));
    // the progression is monotonic
    let mut prev = 0.0;
    for i in 0..=10 {
        let v = a.lerp(&b, i as f64 / 10.0).get_param("radius").unwrap();
        assert!(v >= prev, "the move is not monotonic: {v} after {prev}");
        prev = v;
    }
}

/// A non-numeric value cannot be interpolated — it is taken from the NEAREST end and switches
/// exactly at the midpoint. Otherwise a “cropping mode” or a “rotation checkbox” would change
/// its value at some random point of the drag.
#[test]
fn t550b_non_numeric_comes_from_the_nearer_end() {
    // R.62: Pan is abolished — the non-numeric mode lives on CamMotion
    let fit = FxInstance::CamMotion {
        mode: PanMode::Fit,
        ease: true,
        time_scale: 1.0,
        keyframes: vec![],
    };
    let fill = FxInstance::CamMotion {
        mode: PanMode::Fill,
        ease: true,
        time_scale: 3.0,
        keyframes: vec![],
    };
    for (t, want) in [(0.1, PanMode::Fit), (0.49, PanMode::Fit), (0.5, PanMode::Fill)] {
        match fit.lerp(&fill, t) {
            FxInstance::CamMotion { mode, .. } => assert_eq!(mode, want, "t = {t}"),
            other => panic!("view was swapped: {other:?}"),
        }
    }
}

/// Different kinds do not mix: one gets the nearest end as it is, not a chimera
/// with foreign fields.
#[test]
fn t550c_different_kinds_do_not_blend() {
    let blur = FxInstance::blur(8);
    let glow = FxInstance::glow(0.9);
    assert_eq!(dbg(&blur.lerp(&glow, 0.2)), dbg(&blur));
    assert_eq!(dbg(&blur.lerp(&glow, 0.8)), dbg(&glow));
}

/// The mix stays WITHIN THE SPEC BOUNDS for all kinds of the registry: `set_param`
/// clamps, but a check is needed separately — it also catches a parameter whose `get_param`
/// stays silent (then the field would stay at the nearest end, and the motion along the
/// axis would be jerky).
#[test]
fn t550d_blend_stays_in_spec_for_every_kind() {
    for f in FxInstance::all() {
        let specs = f.param_specs();
        if specs.is_empty() {
            continue;
        }
        // The second end is 99 % of the range, NOT the flat maximum:
        // for an f32 field the flat ceiling
        // after `set_param` reads back slightly above the ceiling (the clamp is computed in f64,
        // the write into f32 rounds upward — item 427). This is a property of the model, not of
        // the blending, and feeding it in here would mean testing the wrong thing.
        let mut hi = f.clone();
        for s in specs {
            hi.set_param(&s.name, s.min + (s.max - s.min) * 0.99);
        }
        for i in 0..=8 {
            let t = i as f64 / 8.0;
            let mid = f.lerp(&hi, t);
            assert_eq!(
                std::mem::discriminant(&mid),
                std::mem::discriminant(f),
                "{}: kind changed at t = {t}",
                f.tag()
            );
            for s in specs {
                let v = mid.get_param(&s.name).unwrap_or(f64::NAN);
                assert!(
                    v.is_finite() && v >= s.min && v <= s.max,
                    "{} / {}: {v} outside [{}, {}] at t = {t}",
                    f.tag(),
                    s.name,
                    s.min,
                    s.max
                );
            }
        }
    }
}
