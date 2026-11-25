//! Tests 406-407 (R.49 B5): loudness is computed on the export pass.

use kt_engine::export::render_raw_audio;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

fn project_with(gain_db: f32, secs: f64) -> Project {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:1000"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick((90_000.0 * secs) as i64),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    });
    p.ensure_tracks(kt_model::LaneKind::Audio, 1);
    p.normalize();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn kt_media::MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 1000.0 }))
            as Box<dyn kt_media::MediaSource>,
    )]
}

/// Test 406 (R.49 B5): the export reports the loudness, and it hears the loudness of
/// the EVENT. The meter is fed by the ready chunk buffer — not a single new decode.
#[test]
fn t406_export_reports_loudness_of_the_mix() {
    let mut frames = 0u64;
    let loud = render_raw_audio(&project_with(0.0, 12.0), sources(), |b| {
        frames += (b.len() / AUDIO_CHANNELS) as u64;
        Ok(())
    })
    .unwrap();
    let a = loud.loudness.expect("the export must report loudness").lufs.expect("not silence");
    assert!(frames > 0, "audio was not rendered at all");

    // the same event 10 dB quieter — exactly 10 LU quieter
    let quiet = render_raw_audio(&project_with(-10.0, 12.0), sources(), |_| Ok(())).unwrap();
    let b = quiet.loudness.unwrap().lufs.unwrap();
    assert!(
        ((a - b) - 10.0).abs() <= 0.15,
        "export loudness does not hear the event level: {a:.2} against {b:.2} LUFS"
    );

    // the true peak is for the same material: `synth:sine` has amplitude 0.5,
    // that is −6.02 dBFS, and the true peak must land next to it
    let dbtp = loud.loudness.unwrap().dbtp;
    assert!(
        (dbtp + 6.02).abs() < 0.5,
        "true peak diverged from the source amplitude: {dbtp:.2} dBTP at −6.02 dBFS"
    );
}

/// Test 407 (R.49 B5): the master gain does NOT take part in the measurement — and cannot.
///
/// It lives only in `Player`, it is in neither `Project` nor the .otio, export does not
/// apply it, and it is set by the meters panel's fader: this is the CONTROL loudness.
/// The consequence that is checked: the number in the preview and the number in the file match
/// BY CONSTRUCTION, because the same code counts both over the same
/// material.
#[test]
fn t407_master_gain_is_not_part_of_the_measurement() {
    let p = project_with(0.0, 12.0);
    let a = render_raw_audio(&p, sources(), |_| Ok(())).unwrap().loudness.unwrap().lufs.unwrap();

    // the master gain is a property of the player; there is no such thing in the project model
    // and nowhere to write it. Guard: a field with this meaning must not
    // appear in the project settings unnoticed.
    let settings = format!("{:?}", p.settings);
    assert!(
        !settings.contains("master"),
        "the master gain leaked into the project settings ({settings}) — then the export must apply \
         it, and “monitor volume” would stop being monitor volume"
    );

    // a repeat of the same export gives the same number: the measurement is deterministic
    let b = render_raw_audio(&p, sources(), |_| Ok(())).unwrap().loudness.unwrap().lufs.unwrap();
    assert_eq!(a, b, "loudness measurement is non-deterministic");

    // the document's program stage IS part of the mix and of the measurement (loudness-norms): +3 dB
    // under its ceiling is 3 LU
    let mut staged = p.clone();
    staged.program = Some(ProgramStage { gain_db: 3.0, measured: None });
    let off = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let s = kt_engine::export::render_raw_audio_range(
        &staged,
        sources(),
        None,
        kt_engine::program::params(&staged),
        &off,
        |_| {},
        |_| Ok(()),
    )
    .unwrap();
    let c = s.loudness.unwrap().lufs.unwrap();
    assert!((c - a - 3.0).abs() <= 0.1, "a program gain of 3 dB measures {:.2} LU", c - a);
    assert!((s.program.unwrap().took_lu).abs() < 0.05, "the limiter took nothing: {:?}", s.program);
}

/// A 1 kHz tone whose channels have levels of their own by the second.
struct Sides(SyntheticSource);

impl kt_media::MediaSource for Sides {
    fn info(&self) -> &kt_media::MediaInfo {
        self.0.info()
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<kt_media::AudioBurst, kt_media::MediaError> {
        let mut b = self.0.read_audio_burst(idx)?;
        for (i, f) in b.data.chunks_exact_mut(AUDIO_CHANNELS).enumerate() {
            let at = idx * kt_time::AUDIO_BURST_FRAMES as i64 + i as i64;
            // 3–6 s: the left 12 dB over the right; 8–10 s: the right silent; 12–13 s: over the ceiling
            let (l, r) = match at / 48_000 {
                3..6 => (0.8, 0.2),
                8..10 => (0.4, 0.0),
                12 => (1.95, 1.95),
                _ => (0.4, 0.4),
            };
            f[0] *= l;
            f[1] *= r;
        }
        Ok(b)
    }
    fn read_video_frame(&mut self, pts: Tick, q: Option<kt_media::Quality>) -> Result<kt_media::VideoFrame, kt_media::MediaError> {
        self.0.read_video_frame(pts, q)
    }
    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        self.0.keyframe_before(pts)
    }
}

/// t3921: a stereo sound with the left channel 12 dB over the right for 3 s, the right one silent for
/// 2 s and a second over the ceiling is measured twice: the three findings stand at their times, the same
/// twice; a good stretch has none.
#[test]
fn t3921_the_findings_stand_at_their_times() {
    use kt_fx::audio::findings::SoundFinding;
    let sides = || -> Vec<(MediaId, Box<dyn kt_media::MediaSource>)> {
        vec![(MediaId(1), Box::new(Sides(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 1000.0 }))))]
    };
    let mut p = project_with(0.0, 16.0);
    p.loudness = LoudnessTarget::of(LoudnessStandard::Streaming);
    let run = || render_raw_audio(&p, sides(), |_| Ok(())).unwrap();
    let (a, b) = (run(), run());
    assert_eq!(a.spans, b.spans, "two measurements differ");
    let sec = |f: u64| f as f64 / 48_000.0;
    let got: Vec<_> = a.spans.iter().map(|s| (s.what, sec(s.from), sec(s.to))).collect();
    let want = [(SoundFinding::Louder(0), 3.0, 6.0), (SoundFinding::Silent(1), 8.0, 10.0), (SoundFinding::OverCeiling, 12.0, 13.0)];
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(&want) {
        assert!(g.0 == w.0 && (g.1 - w.1).abs() <= 0.11 && (g.2 - w.2).abs() <= 0.11, "{g:?} against {w:?}");
    }
    assert!((a.spans[0].value - 12.0).abs() < 0.2, "the difference: {:?}", a.spans[0]);
    assert!(a.spans[2].value > -1.0 && a.spans[2].value < 0.5, "the peak: {:?}", a.spans[2]);
}
