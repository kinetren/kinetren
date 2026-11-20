//! gen-technical (`v26.10` G5, the technical group): the sync flash and the safe-area guides.
//! Tests t3700–t3701; the contract — `openspec/specs/generators/spec.md`.

use kt_media::generator::GeneratorSource;
use kt_media::{MediaSource, Quality};
use kt_model::{GeneratorKind, HasParams};
use kt_time::Tick;

fn kind(id: &str) -> GeneratorKind {
    kt_plugins_builtin::install();
    GeneratorKind::default_for_tag(id).unwrap_or_else(|| panic!("the kind {id}"))
}

fn frame(src: &mut GeneratorSource, t: i64, w: u32, h: u32) -> Vec<u8> {
    src.read_video_frame(Tick(t), Some(Quality { width: w, height: h })).expect("a frame").rgba.to_vec()
}

fn px(f: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * w + x) * 4) as usize;
    [f[o], f[o + 1], f[o + 2], f[o + 3]]
}

/// Test 3700: the sync flash's picture and beep line up — the frame is white and the beep sounds for
/// `flash` frames at the start of every interval; between them the frame is dark with a bar filling
/// over the interval, and silent.
#[test]
fn t3700_the_flash_and_its_beep_line_up() {
    let mut k = kind("syncflash");
    k.set_param("interval", 2.0);
    let mut src = GeneratorSource::new(k);
    assert!(src.info().has_audio, "the beep is on by default: the kind has sound");
    let (w, h) = (64, 36);
    // the first frame of the interval (1 frame at 30 fps = 3000 ticks)
    for t in [0, 180_000, 180_000 + 2_999] {
        assert_eq!(px(&frame(&mut src, t, w, h), w, 32, 18), [235, 235, 235, 255], "{t}: no flash");
    }
    let mid = frame(&mut src, 90_000, w, h);
    assert_eq!(px(&mid, w, 32, 10), [16, 16, 16, 255], "between flashes the frame is dark");
    assert_eq!(px(&mid, w, 10, 35), [90, 200, 255, 255], "half way the bar covers the left half");
    assert_eq!(px(&mid, w, 50, 35), [16, 16, 16, 255], "half way the bar does not reach the right side");
    // the beep: sample 0..1/30 s of each 2 s sounds, the middle does not
    let loud = |b: &kt_media::AudioBurst| b.data.iter().any(|s| s.abs() > 1e-4);
    assert!(loud(&src.read_audio_burst(0).expect("a burst")), "no beep at the flash");
    assert!(loud(&src.read_audio_burst(2 * 48_000 / 1024).expect("a burst")), "no beep at the second flash");
    assert!(!loud(&src.read_audio_burst(48_000 / 1024).expect("a burst")), "a beep between flashes");
    // the beep off: no sound
    let mut quiet = kind("syncflash");
    quiet.set_flag("beep", false);
    assert!(!GeneratorSource::new(quiet).info().has_audio, "the beep off still sounds");
}

/// Test 3701: the safe areas are transparent except their guides; the action-safe frame stands at 93 %
/// and the title-safe at 90 % of the picture, at any preview size (fractions of the frame).
#[test]
fn t3701_the_safe_areas_are_guides_over_a_transparent_frame() {
    let k = kind("safearea");
    let mut src = GeneratorSource::new(k);
    for (w, h) in [(1920u32, 1080u32), (640, 360)] {
        let f = frame(&mut src, 0, w, h);
        assert_eq!(px(&f, w, w / 4, h / 4)[3], 0, "{w}: the picture under the guides is not transparent");
        // the outlines' left edges at (1 − frac)/2 of the width
        for frac in [0.93f64, 0.9] {
            let x = (w as f64 / 2.0 - (frac * w as f64 / 2.0).round()) as u32;
            assert!(px(&f, w, x, h / 3)[3] > 0, "{w}: no {frac} frame at x={x}");
            assert_eq!(px(&f, w, x.saturating_sub(3), h / 3)[3], 0, "{w}: the {frac} frame is not thin at x={x}");
        }
        assert!(px(&f, w, w / 2, h / 2)[3] > 0, "{w}: no centre cross");
    }
}
