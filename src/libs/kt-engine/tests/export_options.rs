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
