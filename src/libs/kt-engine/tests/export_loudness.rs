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
}
