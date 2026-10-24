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
