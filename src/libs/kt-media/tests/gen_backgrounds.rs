//! gen-backgrounds (`v26.10` G5, the backgrounds group): the moving gradient, plasma, particles and
//! patterns. Test t3707; the contract — `openspec/specs/generators/spec.md`.

use kt_media::generator::GeneratorSource;
use kt_media::{MediaSource, Quality};
use kt_model::{GeneratorKind, HasParams};
use kt_time::Tick;

fn frame(k: &GeneratorKind, t: i64) -> Vec<u8> {
    let mut src = GeneratorSource::new(k.clone());
    src.read_video_frame(Tick(t), Some(Quality { width: 96, height: 54 })).expect("a frame").rgba.to_vec()
}

/// Test 3707: every moving background moves by itself and repeats its bytes at one moment; its colours
/// and numbers reach the picture (all three colours red → a red frame; a pattern per choice; the
/// particles' count).
#[test]
fn t3707_the_moving_backgrounds_move_and_obey_their_rows() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    for tag in ["meshgrad", "plasma", "particles", "pattern"] {
        let mut k = GeneratorKind::default_for_tag(tag).expect(tag);
        k.set_param("speed", 0.5);
        let a = frame(&k, 0);
        assert!(a.chunks(4).all(|p| p[3] == 255), "{tag}: a background is opaque");
        assert_ne!(a, frame(&k, 90_000), "{tag}: does not move by itself");
        assert_eq!(a, frame(&k, 0), "{tag}: one moment draws other bytes");
    }
    for tag in ["meshgrad", "plasma"] {
        let mut k = GeneratorKind::default_for_tag(tag).expect(tag);
        for stem in ["color", "color2", "color3"] {
            for (c, v) in [("r", 200.0), ("g", 20.0), ("b", 30.0)] {
                k.set_param(&format!("{stem}_{c}"), v);
            }
        }
        let f = frame(&k, 45_000);
        assert!(f.chunks(4).all(|p| (p[0] as i32 - 200).abs() <= 1 && p[1] <= 21 && p[2] <= 31), "{tag}: three red colours do not give a red frame");
    }
    let base = GeneratorKind::default_for_tag("pattern").expect("pattern");
    let mut seen = Vec::new();
    for (i, key) in ["stripes", "dots", "grid", "waves"].iter().enumerate() {
        let mut k = base.clone();
        assert!(k.set_param("pattern", i as f64), "{key}: the choice row");
        let f = frame(&k, 0);
        assert!(!seen.contains(&f), "{key}: draws what another pattern draws");
        seen.push(f);
    }
    let mut few = GeneratorKind::default_for_tag("particles").expect("particles");
    few.set_param("count", 4.0);
    let mut many = few.clone();
    many.set_param("count", 200.0);
    let lit = |f: &[u8]| f.chunks(4).filter(|p| p[0] > 60).count();
    assert!(lit(&frame(&many, 0)) > lit(&frame(&few, 0)), "more particles light no more of the frame");
}
