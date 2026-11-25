//! render-result-check (capability export; the owner, v26.10 line 55: "video 517/516 audio
//! 412672/413364"): an export's result carries what it announced beside what it
//! wrote, the two are equal on every range, and the file holds them. Requires the system FFmpeg.

use kt_engine::export::{ExportOptions, ExportStats};
use kt_media::MediaSource;
use kt_media::muxer::probe_output;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn project(fps: u32) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps,
        width: 160,
        height: 90,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    for (id, kind) in [(1u64, ClipKind::Video), (2, ClipKind::Audio)] {
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
            lane: 0,
            start: Tick(0),
            duration: Tick(90_000 * 4),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
            fx_in: Vec::new(),
        });
    }
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 4), AudioPattern::Sine { hz: 440.0 })) as Box<dyn MediaSource>)]
}

fn export(p: &Project, out: &std::path::Path, range: Option<(Tick, Tick)>) -> ExportStats {
    let off = Arc::new(AtomicBool::new(false));
    kt_engine::export_mp4_with_backends(
        p,
        sources(),
        sources(),
        &out.to_string_lossy(),
        range,
        &ExportOptions { force_encoder: Some("libx264".into()), ..Default::default() },
        &kt_gpu::testkit::backends(),
        &off,
        &off,
        |_| {},
    )
    .unwrap()
}

/// t3998 (render-result-check): ranges on the frame grid, off it, shorter than a burst and ending
/// inside a burst, at 24 and 30 fps — the frames and the samples written are the announced ones, the
/// samples are the range's, and the file holds the frames.
#[test]
fn t3998_frames_and_samples_end_at_their_totals() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if !kt_media::muxer::availability().video.iter().any(|(_, l)| l.iter().any(|e| e.name == "libx264")) {
        eprintln!("t3998 skipped: this FFmpeg has no libx264");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for fps in [24u32, 30] {
        let p = project(fps);
        let step = 90_000 / fps as i64;
        let ranges = [
            None,
            Some((Tick(step * 6), Tick(step * 30))),
            // a third of a frame after 1 s, 0.2 s long: his hand-drawn region
            Some((Tick(90_000 + step / 3), Tick(90_000 + step / 3 + 18_000))),
            // shorter than a burst (1024 samples are 1920 ticks)
            Some((Tick(90_007), Tick(90_007 + 1000))),
            // the end stands inside a burst and on no sample of it: 403 whole bursts and 692 samples
            Some((Tick(11), Tick(11 + 260_000))),
        ];
        for (k, r) in ranges.into_iter().enumerate() {
            let out = dir.path().join(format!("{fps}-{k}.mp4"));
            let s = export(&p, &out, r);
            assert_eq!(s.short(), (None, None), "{fps} fps, range {r:?}: {}", s.totals());
            assert!(s.announced_video > 0 && s.announced_audio > 0, "{}", s.totals());
            let (a, b) = r.unwrap_or((Tick(0), Tick(90_000 * 4)));
            assert_eq!(s.audio_frames as i64, b.to_audio_frames() - a.to_audio_frames(), "the sound is the range's");
            let file = probe_output(&out.to_string_lossy()).expect("our reader opens the output");
            assert_eq!(file.packets.len() as u64, s.video_frames, "{fps} fps, range {r:?}: the file's frames");
            // the picture covers the range: from the frame that contains its start to its end (D3)
            let (len, shown) = (b.0 - a.0, s.video_frames as i64 * step);
            assert!(shown >= len && shown < len + 2 * step, "{shown} ticks of picture for a range of {len}");
        }
    }
    // a result that fell short names both numbers of its pair, and one that announced nothing none
    let short = ExportStats { video_frames: 516, announced_video: 517, audio_frames: 412_672, announced_audio: 413_364, ..Default::default() };
    assert_eq!(short.short(), (Some((516, 517)), Some((412_672, 413_364))));
    assert_eq!(short.totals(), "frames 516/517, sound 412672/413364 samples");
    assert_eq!(ExportStats { video_frames: 5, ..Default::default() }.short(), (None, None));
}
