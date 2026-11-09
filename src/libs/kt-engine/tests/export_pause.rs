//! t2261 (gpu-arbiter; capability `export`): a paused export waits on an event, never on a timer.
//! Requires the system FFmpeg (the kt-ffmpeg muxer), as test 25.

use kt_engine::export::ExportOptions;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::MediaSource;
use kt_model::*;
use kt_time::Tick;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// Test t2261: an export paused from its start stands without waking — its pause waits find the pause
/// still on no more times half a second later — and, resumed, writes every frame.
#[test]
fn t2261_a_paused_export_does_not_wake_until_resumed() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("t2261.mp4").to_string_lossy().to_string();
    let (cancel, pause) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(true)));
    let h = {
        let (cancel, pause) = (cancel.clone(), pause.clone());
        std::thread::spawn(move || {
            let opts = ExportOptions { force_encoder: Some("libx264".into()), quality: 28, ..Default::default() };
            kt_gpu::testkit::export_with(&tiny_project(), sources(), sources(), &out, None, &opts, &cancel, &pause, |_| {})
        })
    };
    let waits = || kt_engine::export::PAUSE_WAITS.load(Ordering::Relaxed);
    let deadline = kt_time::test_deadline();
    while waits() == 0 {
        assert!(Instant::now() < deadline, "the export never reached its pause");
        std::thread::sleep(Duration::from_millis(5));
    }
    // every waiting thread has looked once; a timer would look again every 40 ms
    std::thread::sleep(Duration::from_millis(100));
    let settled = waits();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(waits(), settled, "a paused export woke on its own");
    pause.store(false, Ordering::Release);
    kt_time::sched::notify_gate();
    let stats = h.join().unwrap().expect("the export resumes and finishes");
    assert_eq!(stats.video_frames, 30);
}
