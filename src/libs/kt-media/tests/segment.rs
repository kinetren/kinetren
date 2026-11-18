//! Video segments of the preview's disk tier through our libav (render-cache-disk D2, the owner's
//! kkt-26.11.18b U4). t2986.
#![cfg(feature = "ffmpeg")]

use kt_media::segment::SegmentCodec;
use kt_media::segment::ffmpeg::FfmpegSegments;

const W: u32 = 320;
const H: u32 = 180;

/// A moving picture with edges and flat areas, as a composite has.
fn frame(i: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let r = ((x + i * 7) % 256) as u8;
            let g = if (x / 40 + y / 30 + i) % 2 == 0 { 40 } else { 210 };
            let b = ((y * 255) / H) as u8;
            v.extend_from_slice(&[r, g, b, 255]);
        }
    }
    v
}

/// Test t2986 (render-cache-disk D2): on this machine the default codec writes with a hardware
/// encoder or refuses, naming why (never a processor encoder); forced to an encoder (the path a test
/// and the live check take where no hardware encoder opens), a segment of every-frame keyframes is
/// written in-process, names its encoder, and each frame read back alone, in any order, is the frame
/// written within the codec's tolerance.
#[test]
fn t2986_a_segment_is_written_by_the_hardware_encoder_or_refused() {
    if kt_ffmpeg::rescan().is_err() {
        eprintln!("t2986: no FFmpeg: skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // the default: hardware only
    let hw = FfmpegSegments { forced: None };
    match hw.writer(&dir.path().join("hw.mkv"), W, H, 25) {
        Ok(w) => {
            assert!(w.encoder() != "libx264" && w.encoder() != "libx265", "a processor encoder opened: {}", w.encoder());
            eprintln!("t2986: hardware encoder {}", w.encoder());
        }
        Err(why) => {
            assert!(why.contains("no hardware encoder"), "{why}");
            assert!(!dir.path().join("hw.mkv").exists(), "a refused segment leaves no file");
            eprintln!("t2986: {why}");
        }
    }
    // forced: the whole path
    let codec = FfmpegSegments { forced: Some("libx264".into()) };
    let path = dir.path().join("seg.mkv");
    let mut w = match codec.writer(&path, W, H, 25) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("t2986: this FFmpeg has no libx264 ({e}): the forced path is skipped");
            return;
        }
    };
    assert_eq!(w.encoder(), "libx264");
    for i in 0..6 {
        w.push(&frame(i)).unwrap();
    }
    let bytes = w.finish().unwrap();
    assert!(bytes > 0 && bytes < 6 * (W * H * 4) as u64 / 4, "{bytes} bytes for six frames");
    let mut r = codec.reader(&path).unwrap();
    let mut out = Vec::new();
    for i in [3u32, 0, 5, 1, 4, 2] {
        r.frame(i, W, H, &mut out).unwrap();
        let want = frame(i);
        assert_eq!(out.len(), want.len());
        let diff: u64 = out.iter().zip(&want).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64).sum();
        let mean = diff as f64 / out.len() as f64;
        assert!(mean < 3.0, "frame {i}: mean difference {mean:.2}");
    }
}
