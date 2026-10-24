//! Test 36: morpher in the render.
//! The engine sees only effective_clips: a render with a morpher is sample-exact
//! equal to (a) a render after BakeMorpher and (b) a render of an equivalent project
//! without a morpher. The FX batch acts live (fix of defect D1 of v1).

use kt_engine::{AudioRenderer, DirectFetcher};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

/// item 8.1 (R.28): a render with a FULLY available source must run without a
/// single underrun — otherwise the render asks for material beyond its own window,
/// and on a live cache that is silence plus a growing counter.
#[track_caller]
fn no_underruns(s: kt_engine::RenderStats) {
    assert_eq!(s.underruns, 0, "underrun with a fully available source");
}

const CH: usize = AUDIO_CHANNELS;
const LEN: Tick = Tick(90_000 * 2);

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 })),
    );
    f.add_source(MediaId(2), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)));
    f
}

fn project_with_clip(media: MediaId, source_in: Tick, fx: Vec<FxInstance>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:440"));
    p.media.push(MediaRef::new(MediaId(2), "synth:ramp"));
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
        duration: LEN,
        source_in,
        media,
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn render(p: &Project) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let frames = LEN.to_audio_frames() as usize;
    let mut out = vec![0f32; frames * CH];
    no_underruns(r.render_frames(&mut direct(), 0, &mut out));
    out
}

/// Replace-morpher: a render == a render of a project where the clip points from the very start
/// to the new source with a shifted source_in; bake changes nothing.
#[test]
fn t36_replace_renders_as_new_source_and_bake_is_noop_for_render() {
    let mut p = project_with_clip(MediaId(1), Tick(9_000), vec![]);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(90 * 1500),
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: MorphPatch {
            media: Some(MediaId(2)),
            source_in_delta: Tick(4_500),
            absolute_source_in: None,
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.validate().unwrap();

    let with_morph = render(&p);
    let expected = render(&project_with_clip(MediaId(2), Tick(13_500), vec![]));
    assert_eq!(with_morph, expected, "morpher == a direct clip on the new source");
    assert_ne!(with_morph, render(&project_with_clip(MediaId(1), Tick(9_000), vec![])));

    let mut model = TimelineModel::new(p);
    model.apply(Command::BakeMorpher { id: MorphId(1) }).unwrap();
    assert_eq!(render(model.project()), expected, "bake does not change the render");
}

/// The EffectBatch acts live: a render with a morpher == a render of a clip with the FX
/// appended directly into the chain (in v1 a batch without a bake did not sound — defect D1).
#[test]
fn t36_effect_batch_is_live() {
    let gain = FxInstance::gain(6.0);
    let mut p = project_with_clip(MediaId(1), Tick(0), vec![]);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(90 * 1500),
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: MorphPatch {
            video_fx: vec![],
            audio_fx: vec![gain.clone()],
            video_bypass: vec![],
            audio_bypass: vec![],
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.validate().unwrap();

    let with_morph = render(&p);
    let expected = render(&project_with_clip(MediaId(1), Tick(0), vec![gain]));
    assert_eq!(with_morph, expected, "the FX batch sounds without bake");

    let mut model = TimelineModel::new(p);
    model.apply(Command::BakeMorpher { id: MorphId(1) }).unwrap();
    assert_eq!(render(model.project()), expected, "bake materializes the same thing");
}
