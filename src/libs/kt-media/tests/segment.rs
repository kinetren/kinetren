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
            let g = if (x / 40 + y / 30 + i).is_multiple_of(2) { 40 } else { 210 };
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
    let t0 = std::time::Instant::now();
    for i in 0..6 {
        w.push(&frame(i)).unwrap();
    }
    let bytes = w.finish().unwrap();
    eprintln!("t2986: libx264 {W}×{H}: {:.1} ms a frame written, {} bytes a frame", t0.elapsed().as_secs_f64() * 1e3 / 6.0, bytes / 6);
    assert!(bytes > 0 && bytes < 6 * (W * H * 4) as u64 / 4, "{bytes} bytes for six frames");
    let mut r = codec.reader(&path).unwrap();
    let mut out = Vec::new();
    let t0 = std::time::Instant::now();
    for i in [3u32, 0, 5, 1, 4, 2] {
        r.frame(i, W, H, &mut out).unwrap();
        let want = frame(i);
        assert_eq!(out.len(), want.len());
        let diff: u64 = out.iter().zip(&want).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64).sum();
        let mean = diff as f64 / out.len() as f64;
        assert!(mean < 3.0, "frame {i}: mean difference {mean:.2}");
    }
    eprintln!("t2986: {:.1} ms a frame read back out of order", t0.elapsed().as_secs_f64() * 1e3 / 6.0);
}

/// The frames of a segment as the processor decodes them: the reference of t3129.
fn software_frames(path: &std::path::Path, n: u32) -> Vec<Vec<u8>> {
    use kt_media::MediaSource;
    let mut src = kt_media::ffmpeg::FfmpegSource::open(&path.to_string_lossy()).unwrap();
    assert!(matches!(src.video_decode(), kt_ffmpeg::VideoDecode::Software(_)), "the plain open decodes on the processor");
    (0..n)
        .map(|i| {
            let f = src
                .read_video_frame(kt_time::Tick(i as i64 * 3600), Some(kt_media::Quality { width: W, height: H }))
                .unwrap();
            f.rgba[..(W * H * 4) as usize].to_vec()
        })
        .collect()
}

/// Test t3129 (hw-segment-decode, found-defects 775): a segment is read by the machine's hardware
/// decoder, the processor only where none opens. Here (no render node, or the machine's own) the
/// reader names what decodes and reads every frame; on the fake VA driver of `fixtures/fake_va` (a
/// child process: libva reads the driver from the environment at load) a segment its encoder wrote
/// decodes on the device — through the real libva and FFmpeg — to the frame written within the RGB →
/// 4:2:0 → RGB round trip and to the processor's decode within swscale's rounding of NV12 against
/// yuv420p, out of order, and the driver counts the pictures it decoded.
#[test]
fn t3129_segments_decode_on_the_device_the_processor_only_without_one() {
    if kt_ffmpeg::rescan().is_err() {
        eprintln!("t3129: no FFmpeg: skipped");
        return;
    }
    if std::env::var("KT_T3129_CHILD").is_ok() {
        return t3129_child();
    }
    let dir = tempfile::tempdir().unwrap();
    // this machine: a processor-written segment, whatever decodes it says so and reads it all
    let codec = FfmpegSegments { forced: Some("libx264".into()) };
    let path = dir.path().join("seg.mkv");
    if let Ok(mut w) = codec.writer(&path, W, H, 25) {
        for i in 0..4 {
            w.push(&frame(i)).unwrap();
        }
        w.finish().unwrap();
        let mut r = codec.reader(&path).unwrap();
        let want = software_frames(&path, 4);
        match r.decoder() {
            Ok(d) => eprintln!("t3129: this machine decodes on {d}"),
            Err(why) => {
                if cfg!(target_os = "linux") {
                    assert!(why.contains("VAAPI"), "the processor's reason names the device it looked for: {why}");
                }
                // the D3D11VA path (found-defects 775): under wine and on a Windows without the
                // device the reason names it too
                if cfg!(windows) {
                    assert!(why.contains("D3D11VA"), "the processor's reason names the device it looked for: {why}");
                }
                // the proof on a Windows machine with a graphics card is opt-in and loud, as t963's
                // VAAPI one: wine and the CI runner have no D3D11 video decode, so only this proves the
                // device's path (found-defects 775)
                assert!(
                    std::env::var("KT_EXPECT_HW").as_deref() != Ok("d3d11va"),
                    "KT_EXPECT_HW=d3d11va, and the segment decodes on the processor: {why}"
                );
                eprintln!("t3129: this machine decodes on the processor: {why}");
            }
        }
        let mut out = Vec::new();
        for i in [2u32, 0, 3, 1] {
            r.frame(i, W, H, &mut out).unwrap();
            let d = out.iter().zip(&want[i as usize]).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64).sum::<u64>();
            assert!(d as f64 / out.len() as f64 <= 1.0, "frame {i} differs from the processor's decode by {d}");
        }
    }
    // the fake driver: the device's path end to end
    if !cfg!(target_os = "linux") {
        return;
    }
    let driver = dir.path().join("ktfake_drv_video.so");
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/fake_va/ktfake_drv_video.c");
    let built = ["cc", "clang"].iter().any(|cc| {
        std::process::Command::new(cc)
            .args(["-shared", "-fPIC", "-O1", "-w", "-o"])
            .arg(&driver)
            .arg(source)
            .status()
            .is_ok_and(|s| s.success())
    });
    assert!(built, "no C compiler built the fake driver");
    let node = dir.path().join("renderD128");
    std::fs::write(&node, b"a regular file").unwrap();
    let log = dir.path().join("fake.log");
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["t3129_segments_decode_on_the_device_the_processor_only_without_one", "--exact", "--nocapture"])
        .env("KT_T3129_CHILD", dir.path())
        .env("KINETREN_VAAPI_DEVICE", &node)
        .env("LD_PRELOAD", &driver)
        .env("LIBVA_DRIVERS_PATH", dir.path())
        .env("LIBVA_DRIVER_NAME", "ktfake")
        .env("KT_FAKE_VA_LOG", &log)
        .output()
        .unwrap();
    let all = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if all.contains("t3129-child: no h264_vaapi") {
        eprintln!("t3129: this FFmpeg has no h264_vaapi: the device's path is skipped");
        return;
    }
    assert!(out.status.success(), "{all}");
    assert!(all.contains("t3129-child: ok"), "the child did not run: {all}");
    let log = std::fs::read_to_string(&log).unwrap_or_default();
    let decoded: u64 = log
        .lines()
        .filter_map(|l| l.strip_prefix("decoded "))
        .filter_map(|n| n.trim().parse::<u64>().ok())
        .sum();
    assert!(decoded >= 4, "the driver decoded {decoded} pictures (log: {log:?}); {all}");
    eprintln!("t3129: the fake device decoded {decoded} pictures");
}

/// The child of t3129, on the fake driver.
fn t3129_child() {
    let dir = std::path::PathBuf::from(std::env::var("KT_T3129_CHILD").unwrap());
    let codec = FfmpegSegments { forced: Some("h264_vaapi".into()) };
    let path = dir.join("hw.mkv");
    let mut w = match codec.writer(&path, W, H, 25) {
        Ok(w) => w,
        Err(e) => {
            println!("t3129-child: no h264_vaapi ({e})");
            return;
        }
    };
    for i in 0..4 {
        w.push(&frame(i)).unwrap();
    }
    w.finish().unwrap();
    let want = software_frames(&path, 4);
    let mut r = codec.reader(&path).unwrap();
    let on = r.decoder().unwrap_or_else(|why| panic!("the segment decodes on the processor: {why}"));
    assert!(on.starts_with("vaapi "), "{on}");
    let mut out = Vec::new();
    for i in [3u32, 1, 0, 2, 3, 1] {
        r.frame(i, W, H, &mut out).unwrap();
        // the device's picture comes in NV12, the processor's in yuv420p: swscale upsamples their
        // chroma by different paths, so the two meet within its rounding, not byte for byte; the
        // lossless I_PCM keeps both at the RGB → 4:2:0 → RGB round trip of the frame written (t1556)
        let mean = |a: &[u8], b: &[u8]| {
            a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as f64).sum::<f64>() / a.len() as f64
        };
        let to_sw = mean(&out, &want[i as usize]);
        let to_written = mean(&out, &frame(i));
        let sw_to_written = mean(&want[i as usize], &frame(i));
        println!("t3129-child: frame {i}: {to_sw:.3} from the processor's, {to_written:.3} from the written ({sw_to_written:.3} the processor's)");
        assert!(to_sw <= 2.0, "frame {i}: the device's decode is {to_sw:.3} from the processor's");
        assert!(to_written <= 3.0, "frame {i}: the device's decode is {to_written:.3} from the frame written");
    }
    assert_eq!(r.decoder(), Ok(on.clone()), "every picture came from the device");
    println!("t3129-child: ok on {on}");
}
