//! The sound generators through the host (generators-placement-and-audio, capability
//! `generators`): t1155 — the `tone` kind and the SMPTE tone are one sine, byte for byte; t1156 —
//! a sound kind has no picture, and its bursts do not depend on the order they are read in.

use kt_media::MediaSource;
use kt_media::generator::GeneratorSource;
use kt_model::GeneratorKind;

fn kind(tag: &str, values: &[(&str, f64)]) -> GeneratorKind {
    let mut k = GeneratorKind::all().into_iter().find(|k| k.tag() == tag).unwrap_or_else(|| panic!("no kind {tag}"));
    if let GeneratorKind::Plugin(p) = &mut k {
        for (n, v) in values {
            assert!(p.set(n, *v), "{tag}: no row {n}");
        }
    }
    k
}

fn bits(src: &mut GeneratorSource, idx: i64) -> Vec<u32> {
    src.read_audio_burst(idx).expect("a burst").data.iter().map(|s| s.to_bits()).collect()
}

#[test]
fn t1155_the_tone_kind_and_the_smpte_tone_are_one_sine() {
    kt_plugins_builtin::install();
    let mut tone = GeneratorSource::new(kind("tone", &[("freq", 1000.0), ("level", -20.0)]));
    let mut smpte = GeneratorSource::new(kind("smpte", &[("tone", 1.0), ("tone_db", -20.0)]));
    assert!(smpte.info().has_audio && smpte.info().has_video, "SMPTE with its tone is picture and sound");
    for idx in [0i64, 7, 9000] {
        assert_eq!(bits(&mut tone, idx), bits(&mut smpte, idx), "burst {idx}: two sines");
    }
}

#[test]
fn t1156_a_sound_kind_has_no_picture_and_is_seek_stable() {
    kt_plugins_builtin::install();
    let kinds = [
        kind("tone", &[("freq", 523.25), ("level", -9.5), ("channels", 1.0)]),
        kind("audio_noise", &[("color", 1.0), ("level", -3.0), ("seed", 77.0), ("stereo", 1.0)]),
        kind("audio_noise", &[("color", 2.0), ("level", 0.0)]),
        kind("sweep", &[("freq_start", 60.0), ("freq_end", 9000.0), ("duration", 0.7), ("curve", 1.0)]),
        kind("beep", &[("freq", 880.0), ("period", 0.25), ("length", 12.0), ("channels", 2.0)]),
        kind("silence", &[]),
    ];
    for k in kinds {
        let tag = k.tag();
        let mut a = GeneratorSource::new(k.clone());
        assert!(!a.info().has_video && a.info().has_audio, "{tag}: sound without picture");
        let ahead: Vec<Vec<u32>> = (0..64).map(|i| bits(&mut a, i)).collect();
        let back: Vec<Vec<u32>> = (0..64).rev().map(|i| bits(&mut a, i)).collect();
        let mut b = GeneratorSource::new(k.clone());
        for (j, i) in [37i64, 2, 63, 0, 18, 18, 51].into_iter().enumerate() {
            assert_eq!(bits(&mut b, i), ahead[i as usize], "{tag}: burst {i} (read #{j}) moved with the order");
        }
        for (i, burst) in back.into_iter().rev().enumerate() {
            assert_eq!(burst, ahead[i], "{tag}: burst {i} differs read backwards");
        }
        for burst in &ahead {
            assert!(
                burst.iter().map(|b| f32::from_bits(*b)).all(|s| s.is_finite() && s.abs() <= 1.0),
                "{tag}: a sample outside ±1"
            );
        }
    }
}
