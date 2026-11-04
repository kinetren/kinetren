//! Test 25 (item 5): encoder choice and mp4 roundtrip.
//! Requires the system FFmpeg (the kt-ffmpeg muxer) — like conformance test 10.

use kt_engine::export::{ExportOptions, export_mp4_with};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn tiny_project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    for (id, kind, lane) in [(1u64, ClipKind::Video, 0u32), (2, ClipKind::Audio, 0)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            masks: Default::default(),
            id: ClipId(id),
            link_group: Some(GroupId(1)),
            kind,
            lane,
            start: Tick(0),
            duration: Tick(90_000), // 1 c
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 10), AudioPattern::Sine { hz: 440.0 }))
            as Box<dyn MediaSource>,
    )]
}

#[test]
fn t25_force_libx264_and_roundtrip_via_own_source() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("t25.mp4").to_string_lossy().to_string();
    let opts = ExportOptions {
        force_encoder: Some("libx264".into()),
        quality: 28,
        video_bitrate: None,
        audio_bitrate: 128_000,
        resolution: None,
        placeholders: Vec::new(),
        ..Default::default()
    };
    let stats = export_mp4_with(
        &tiny_project(),
        sources(),
        sources(),
        &out,
        None,
        &opts,
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .expect("export with force libx264");
    assert_eq!(stats.encoder, "libx264", "the force encoder must be selected");
    assert_eq!(stats.video_frames, 30);

    // roundtrip with OUR OWN decoder: duration and streams converge
    let mut src = kt_media::ffmpeg::FfmpegSource::open(&out).expect("opening the result");
    let info = src.info().clone();
    assert!(info.has_video && info.has_audio);
    let dur_s = info.duration.0 as f64 / 90_000.0;
    assert!((dur_s - 1.0).abs() < 0.15, "duration ~1 s, got {dur_s:.3}");
    let f = src
        .read_video_frame(Tick(0), Some(Quality { width: 320, height: 180 }))
        .expect("frame 0 is read");
    assert_eq!((f.width, f.height), (320, 180));
    let b = src.read_audio_burst(0).expect("audio is read");
    let energy: f32 = b.data.iter().map(|s| s * s).sum();
    assert!(energy > 1e-3, "audio is not silence");
}

#[test]
fn t25_force_unknown_encoder_is_explicit_error() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("bad.mp4").to_string_lossy().to_string();
    let opts =
        ExportOptions { force_encoder: Some("no_such_encoder".into()), ..Default::default() };
    let err = export_mp4_with(
        &tiny_project(),
        sources(),
        sources(),
        &out,
        None,
        &opts,
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicBool::new(false)),
        |_| {},
    );
    assert!(err.is_err(), "an unknown force encoder = an explicit error, not a fallback");
}

#[test]
fn t25_auto_encoder_selects_something() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("auto.mp4").to_string_lossy().to_string();
    let stats = export_mp4_with(
        &tiny_project(),
        sources(),
        sources(),
        &out,
        None,
        &ExportOptions::default(),
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .expect("export with auto selection");
    assert!(!stats.encoder.is_empty(), "the name of the selected encoder must be visible (UI)");
}

/// Test 33 (item 9): the file REALLY contains all frames — dropping any (EAGAIN
/// without a retry in the muxer) is a failure. Count frames with our own decoder.
#[test]
fn t33_exported_file_contains_every_frame() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("t33.mp4").to_string_lossy().to_string();
    // 2 seconds = 60 frames at 30 fps
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    for (id, kind, lane) in [(1u64, ClipKind::Video, 0u32), (2, ClipKind::Audio, 0)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            masks: Default::default(),
            id: ClipId(id),
            link_group: Some(GroupId(1)),
            kind,
            lane,
            start: Tick(0),
            duration: Tick(180_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    let stats = export_mp4_with(
        &p,
        sources(),
        sources(),
        &out,
        None,
        &ExportOptions { force_encoder: Some("libx264".into()), ..Default::default() },
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .expect("export");
    assert_eq!(stats.video_frames, 60, "the engine delivered 60 frames");

    // count frames IN THE FILE: the span over all keys + frames_decoded
    let mut src = kt_media::ffmpeg::FfmpegSource::open(&out).unwrap();
    let step = 90_000 / 30;
    let keys: Vec<Tick> = (0..60).map(|i| Tick(i * step)).collect();
    let mut emitted = 0usize;
    src.read_video_span(&keys, Some(Quality { width: 320, height: 180 }), &mut |_, _| emitted += 1)
        .expect("reading all keys");
    assert_eq!(emitted, 60, "every grid key is covered by a frame");
    let (_reopens, decoded) = src.decode_debug_stats();
    assert!(
        decoded >= 60,
        "the file must contain all 60 frames (decoded {decoded}) — a drop in the muxer"
    );
}

/// A two-second (60 frames at 30 fps) version of the tiny project.
fn two_seconds() -> Project {
    let mut p = tiny_project();
    for c in &mut p.clips {
        c.duration = Tick(2 * 90_000);
    }
    p.validate().unwrap();
    p
}

/// One export through the event API: every event, the stats, the size of the finished file.
fn run_events(
    out: &str,
    encoder: Option<&str>,
) -> (Vec<kt_engine::export::ExportEvent>, kt_engine::export::ExportStats, u64) {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let opts = ExportOptions { force_encoder: encoder.map(str::to_string), ..Default::default() };
    let stats = {
        let seen = seen.clone();
        kt_engine::export_mp4_with_backends(
            &two_seconds(),
            sources(),
            sources(),
            out,
            None,
            &opts,
            &kt_engine::ExportBackends::default(),
            &Arc::new(AtomicBool::new(false)),
            &Arc::new(AtomicBool::new(false)),
            move |e| seen.lock().unwrap().push(e),
        )
        .expect("export")
    };
    let size = std::fs::metadata(out).unwrap().len();
    let events = seen.lock().unwrap().clone();
    (events, stats, size)
}

/// Test 954 (render-window-v1-parity, D1/D2): the encoder is named before the first frame, and
/// the byte counters add up.
///
/// The muxer chooses the encoder before any frame, but only the final stats named it, so the
/// progress window called it "auto encoder" for the whole run (the owner, kkt item 4; F-EXP-004).
#[test]
fn t954_encoder_named_before_the_first_frame_and_bytes_add_up() {
    use kt_engine::export::{ExportEvent, ExportLog, ExportPhase};
    let dir = tempfile::tempdir().unwrap();
    for (tag, forced) in [("forced", Some("libx264")), ("cascade", None)] {
        let out = dir.path().join(format!("t954-{tag}.mp4")).to_string_lossy().to_string();
        let (events, stats, size) = run_events(&out, forced);
        let first = events.iter().position(|e| !matches!(e, ExportEvent::Log(_))).expect("events");
        let ExportEvent::Opened(rep) = &events[first] else {
            panic!("{tag}: the first non-log event is not the encoder report: {:?}", events[first]);
        };
        let first_video = events
            .iter()
            .position(|e| matches!(e, ExportEvent::Progress(p) if p.phase == ExportPhase::Video))
            .expect("video progress");
        assert!(first < first_video, "{tag}: the report comes after a video step");
        assert_eq!(rep.encoder, stats.encoder, "{tag}: the report names another encoder");
        if forced.is_some() {
            assert_eq!((rep.encoder.as_str(), rep.hardware, rep.pix_fmt.as_str()), ("libx264", false, "yuv420p"));
            assert!(rep.options.iter().any(|(k, _)| k == "crf"), "{tag}: {:?}", rep.options);
        }
        // one "not opened" line per candidate tried before it, in the cascade's order
        let not_opened: Vec<(String, i32)> = events
            .iter()
            .filter_map(|e| match e {
                ExportEvent::Log(ExportLog::NotOpened { name, code }) => Some((name.clone(), *code)),
                _ => None,
            })
            .collect();
        assert_eq!(not_opened, rep.tried, "{tag}: the log and the report disagree");
        let order = kt_media::muxer::encoder_cascade();
        let at = |n: &str| order.iter().position(|o| o == n).unwrap();
        let mut last = None;
        for (n, code) in &rep.tried {
            assert!(*code < 0, "{tag}: {n} failed with a non-error code {code}");
            assert!(at(n) < at(&rep.encoder), "{tag}: {n} is not before {}", rep.encoder);
            assert!(last.is_none_or(|l| l < at(n)), "{tag}: the tried order is not the cascade's");
            last = Some(at(n));
        }
        // the byte counters never decrease, end above zero, and add up to no more than the file
        let steps: Vec<(u64, u64)> = events
            .iter()
            .filter_map(|e| match e {
                ExportEvent::Progress(p) => Some((p.video_bytes, p.audio_bytes)),
                _ => None,
            })
            .collect();
        assert!(steps.windows(2).all(|w| w[1].0 >= w[0].0 && w[1].1 >= w[0].1), "{tag}: bytes went down");
        let (v, a) = *steps.last().unwrap();
        assert!(v > 0 && a > 0, "{tag}: the last counters are zero: {v} {a}");
        assert!(v + a <= size, "{tag}: {v} + {a} bytes exceed the file's {size}");
        // the last step counts every frame and sample (the live window read "179 / 180" when done)
        let Some(ExportEvent::Progress(fin)) = events.iter().rev().find(|e| matches!(e, ExportEvent::Progress(_))) else {
            unreachable!()
        };
        assert_eq!(
            (fin.video_frames, fin.audio_frames),
            (fin.total_video_frames, fin.total_audio_frames),
            "{tag}: the last step is short of the totals"
        );
    }
}

/// Test 955 (render-window-v1-parity, D1): the event stream holds no clock — two exports of the
/// same project with `libx264` give the same events, element by element.
#[test]
fn t955_the_event_stream_holds_no_clock() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.mp4").to_string_lossy().to_string();
    let b = dir.path().join("b.mp4").to_string_lossy().to_string();
    let (ea, _, _) = run_events(&a, Some("libx264"));
    let (eb, _, _) = run_events(&b, Some("libx264"));
    assert_eq!(ea.len(), eb.len(), "the streams differ in length");
    for (i, (x, y)) in ea.iter().zip(&eb).enumerate() {
        assert_eq!(x, y, "event {i} differs");
    }
}

/// One export of `project` on THIS thread with the hardware seam set: the events and the result.
fn export_seamed(
    project: &Project,
    out: &str,
    encoder: Option<&str>,
    seam: kt_media::muxer::hw_seam::Seam,
) -> (Vec<kt_engine::export::ExportEvent>, Result<kt_engine::export::ExportStats, String>) {
    kt_media::muxer::hw_seam::set(seam);
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let opts = ExportOptions { force_encoder: encoder.map(str::to_string), ..Default::default() };
    let r = {
        let seen = seen.clone();
        kt_engine::export_mp4_with_backends(
            project,
            sources(),
            sources(),
            out,
            None,
            &opts,
            &kt_engine::ExportBackends::default(),
            &Arc::new(AtomicBool::new(false)),
            &Arc::new(AtomicBool::new(false)),
            move |e| seen.lock().unwrap().push(e),
        )
        .map_err(|e| e.to_string())
    };
    kt_media::muxer::hw_seam::set(Default::default());
    let events = seen.lock().unwrap().clone();
    (events, r)
}

/// Test 962 (hw-encode-context D3): without a device the cascade falls through the same way every
/// time, and a forced VAAPI is an export error naming the failing part — never a crash.
///
/// ktprompts v26.10 T16: "if you choose h264_vaapi the app just crashes at render". A device
/// candidate is now skipped BEFORE `avcodec_open2` when there is no device or no hardware table.
#[test]
fn t962_without_a_device_vaapi_falls_through_or_errs_never_crashes() {
    use kt_engine::export::{ExportEvent, ExportLog};
    use kt_media::muxer::hw_seam::Seam;
    let dir = tempfile::tempdir().unwrap();
    let no_device = Seam { device: Some("/nonexistent/kt-t962/renderD128".into()), ..Default::default() };
    let project = tiny_project();
    // the cascade, twice: the same encoder; VAAPI, if reached, skipped as "no VAAPI device" and never opened
    let mut chosen = Vec::new();
    for run in 0..2 {
        let out = dir.path().join(format!("cascade-{run}.mp4")).to_string_lossy().to_string();
        let (events, r) = export_seamed(&project, &out, None, no_device.clone());
        let stats = r.expect("the cascade still exports");
        chosen.push(stats.encoder.clone());
        for e in &events {
            match e {
                ExportEvent::Log(ExportLog::Skipped { name, reason }) if name == "h264_vaapi" => {
                    assert!(reason.starts_with("no VAAPI device"), "run {run}: {reason}");
                }
                ExportEvent::Log(ExportLog::NotOpened { name, .. }) => {
                    assert_ne!(name, "h264_vaapi", "run {run}: the VAAPI encoder was opened without a device");
                }
                _ => {}
            }
        }
    }
    assert_eq!(chosen[0], chosen[1], "the cascade is not deterministic");

    // forced, on a system whose cascade has no VAAPI (Windows): an error naming that, no file —
    // the device half cannot apply there (CI of 94e36e9e: "last error: " with nothing after it)
    if !kt_media::muxer::encoder_cascade().iter().any(|n| n == "h264_vaapi") {
        let out = dir.path().join("forced-elsewhere.mp4");
        let (_, r) = export_seamed(&project, &out.to_string_lossy(), Some("h264_vaapi"), no_device);
        let err = r.expect_err("a forced encoder outside the cascade must not export");
        assert!(err.contains("h264_vaapi: not an encoder this system tries"), "the error does not name the cause: {err}");
        assert!(!out.exists(), "a file was left behind");
        return;
    }
    // forced: an error naming the part, no file — first without a device, then without the table
    for (tag, seam, want) in [
        ("device", no_device.clone(), "no VAAPI device"),
        (
            "table",
            Seam { missing: Some("av_hwframe_transfer_data".into()), ..no_device.clone() },
            "hardware context functions missing: av_hwframe_transfer_data",
        ),
    ] {
        let out = dir.path().join(format!("forced-{tag}.mp4"));
        let (_, r) = export_seamed(&project, &out.to_string_lossy(), Some("h264_vaapi"), seam);
        let err = r.expect_err("a forced VAAPI without a device must not export");
        if err.contains("not in this FFmpeg build") {
            eprintln!("t962: this FFmpeg has no h264_vaapi ({err}); the forced half needs a build with it");
            return;
        }
        assert!(err.contains(want), "{tag}: the error does not name “{want}”: {err}");
        assert!(!out.exists(), "{tag}: a file was left behind");
    }
}

/// Test 1063 (ci-green-again): an encoder forced from OUTSIDE this system's cascade is an export
/// error that names it and the cascade — never "last error: " with nothing after it.
///
/// The Windows CI run of 94e36e9e forced `h264_vaapi`, which only the Linux cascade tries: the
/// loop skipped every candidate and the error said nothing. Each system forces the OTHER
/// system's own encoder, so the Linux job proves the same path the Windows job hit.
#[test]
fn t1063_a_forced_encoder_outside_the_cascade_names_itself() {
    let foreign = if cfg!(windows) { "h264_vaapi" } else { "h264_mf" };
    let cascade = kt_media::muxer::encoder_cascade();
    assert!(!cascade.iter().any(|n| n == foreign), "the scene degenerated: {foreign} is in {cascade:?}");
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("forced-foreign.mp4");
    let (_, r) = export_seamed(&tiny_project(), &out.to_string_lossy(), Some(foreign), Default::default());
    let err = r.expect_err("a forced encoder outside the cascade must not export");
    assert!(err.contains(&format!("{foreign}: not an encoder this system tries")), "{err}");
    assert!(err.contains(&cascade[0]), "the error does not list the cascade: {err}");
    assert!(!out.exists(), "a file was left behind");
}

/// Test 963 (hw-encode-context D8): sixty frames through VAAPI decode — proven on hardware only.
///
/// Opt-in and loud: with `KT_EXPECT_HW=vaapi` it FAILS unless VAAPI opens (a skip is not a pass,
/// the t865 rule); without the variable it prints why it did not run.
#[test]
fn t963_sixty_frames_through_vaapi_decode() {
    if std::env::var("KT_EXPECT_HW").as_deref() != Ok("vaapi") {
        eprintln!("t963 skipped: set KT_EXPECT_HW=vaapi on a machine with a VAAPI render node (make hw-test)");
        return;
    }
    // the bound recorded in design D8 (mean absolute difference per channel, 0..255)
    const BOUND: f64 = 6.0;
    let mut p = two_seconds();
    p.settings.width = 1280;
    p.settings.height = 720;
    p.validate().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("t963.mp4").to_string_lossy().to_string();
    let (events, r) = export_seamed(&p, &out, Some("h264_vaapi"), Default::default());
    let stats = r.unwrap_or_else(|e| panic!("KT_EXPECT_HW=vaapi, but VAAPI did not export: {e}"));
    assert_eq!(stats.encoder, "h264_vaapi");
    let report = events
        .iter()
        .find_map(|e| match e {
            kt_engine::export::ExportEvent::Opened(r) => Some(r.clone()),
            _ => None,
        })
        .expect("the open report");
    assert!(report.hardware, "{report:?}");
    let device = report.device.clone().expect("the report names the device");
    eprintln!("t963: h264_vaapi on {device}, {:?}", report.options);
    // the raw frames the encoder was fed, and what our decoder reads back
    let q = Quality { width: 1280, height: 720 };
    let mut raw: Vec<Vec<u8>> = Vec::new();
    kt_engine::export::render_raw_video(&p, sources(), q, |f| {
        raw.push(f.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(raw.len(), 60, "the raw render");
    let mut src = kt_media::ffmpeg::FfmpegSource::open(&out).unwrap();
    let keys: Vec<Tick> = (0..60).map(|i| Tick(i * 3000)).collect();
    let mut worst: f64 = 0.0;
    let mut decoded = 0;
    src.read_video_span(&keys, Some(q), &mut |k, f| {
        let i = (k.0 / 3000) as usize;
        let sum: u64 = f
            .rgba
            .chunks_exact(4)
            .zip(raw[i].chunks_exact(4))
            .map(|(a, b)| (0..3).map(|c| (a[c] as i64 - b[c] as i64).unsigned_abs()).sum::<u64>())
            .sum();
        let mean = sum as f64 / (1280.0 * 720.0 * 3.0);
        worst = worst.max(mean);
        decoded += 1;
    })
    .expect("decoding the VAAPI file");
    assert_eq!(decoded, 60, "every grid key is read back");
    eprintln!("t963: the worst frame's mean absolute difference {worst:.2} (bound {BOUND})");
    assert!(worst <= BOUND, "VAAPI frames differ by {worst:.2} > {BOUND}");
}

/// t1556 (hw-encode-context D4): t963 itself, on the fake driver of `fixtures/fake_va`, which keeps
/// every sample: what the pipelined export uploads to a surface is what the file decodes to, every
/// frame, in order. This is the only run of the upload a machine without a graphics card can give;
/// it proves our path through the real libva and FFmpeg, not any real driver. A child process,
/// because the driver is chosen by the environment libva reads at load time.
#[test]
fn t1556_the_upload_decodes_to_the_frames_it_was_given() {
    if std::env::var("KT_EXPECT_HW").is_ok() {
        return; // the child, or a real machine's run of t963
    }
    if !(cfg!(target_os = "linux") && kt_media::ffmpeg::backend_info().is_some()) {
        eprintln!("t1556 skipped: needs FFmpeg on Linux");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
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
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["t963_sixty_frames_through_vaapi_decode", "--exact", "--nocapture"])
        .env("KT_EXPECT_HW", "vaapi")
        .env("KINETREN_VAAPI_DEVICE", &node)
        .env("LD_PRELOAD", &driver)
        .env("LIBVA_DRIVERS_PATH", dir.path())
        .env("LIBVA_DRIVER_NAME", "ktfake")
        .output()
        .unwrap();
    let all = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if all.contains("not in this FFmpeg build") {
        eprintln!("t1556 skipped: this FFmpeg has no h264_vaapi");
        return;
    }
    assert!(out.status.success(), "{all}");
    assert!(all.contains("t963: the worst frame's mean absolute difference"), "t963 did not run: {all}");
    let worst: f64 = all
        .split("mean absolute difference ")
        .nth(1)
        .and_then(|t| t.split_whitespace().next())
        .and_then(|t| t.parse().ok())
        .expect("the measured difference");
    eprintln!("t1556: {worst}");
    // lossless to NV12: what is left is swscale's RGB → 4:2:0 → RGB round trip
    assert!(worst <= 3.0, "the file differs from the uploaded frames by {worst}");
}

/// t1456 (export-throughput D3): the video encoder is opened with its threads — `auto` for the
/// export, `1` for the serial reference — and says so in the open report; both files hold every
/// frame and the same audio length.
#[test]
fn t1456_the_encoder_gets_its_threads() {
    let dir = tempfile::tempdir().unwrap();
    for (serial, want) in [(false, "auto"), (true, "1")] {
        let out = dir.path().join(format!("t1456-{serial}.mp4")).to_string_lossy().to_string();
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let opts = ExportOptions { force_encoder: Some("libx264".into()), serial, ..Default::default() };
        let stats = {
            let seen = seen.clone();
            kt_engine::export_mp4_with_backends(
                &two_seconds(),
                sources(),
                sources(),
                &out,
                None,
                &opts,
                &kt_engine::ExportBackends::default(),
                &Arc::new(AtomicBool::new(false)),
                &Arc::new(AtomicBool::new(false)),
                move |e| seen.lock().unwrap().push(e),
            )
            .expect("export")
        };
        let report = seen
            .lock()
            .unwrap()
            .iter()
            .find_map(|e| match e {
                kt_engine::export::ExportEvent::Opened(r) => Some(r.clone()),
                _ => None,
            })
            .expect("the encoder is named");
        assert!(
            report.options.iter().any(|(k, v)| k == "threads" && v == want),
            "serial={serial}: {:?}",
            report.options
        );
        assert_eq!(stats.video_frames, 60);
        assert_eq!(stats.audio_frames, 2 * 48_000);
        let mut src = kt_media::ffmpeg::FfmpegSource::open(&out).unwrap();
        let keys: Vec<Tick> = (0..60).map(|i| Tick(i * 3000)).collect();
        src.read_video_span(&keys, Some(Quality { width: 320, height: 180 }), &mut |_, _| {}).unwrap();
        assert!(src.decode_debug_stats().1 >= 60, "serial={serial}: every frame is in the file");
    }
}
