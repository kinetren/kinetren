//! Test 19 (docs/porting-notes.md, item 22 of the user): copying a PIECE of an
//! event with a region + pasting it elsewhere — the render of the pasted block is
//! sample-exact equal to the render of the source range (a seamless join).

use kt_engine::{AudioRenderer, DirectFetcher};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::ops::{PasteIds, PasteNames, copy_region_block, paste_block};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

/// item 8.1 (R.28): a render with a FULLY available source must run without a
/// single underrun — otherwise the render asks for material outside its own window,
/// and on a live cache that is silence and a growing counter.
#[track_caller]
fn no_underruns(s: kt_engine::RenderStats) {
    assert_eq!(s.underruns, 0, "underrun with a fully available source");
}

const CH: usize = AUDIO_CHANNELS;

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)));
    f
}

#[test]
fn t19_copy_region_fragment_paste_renders_identically() {
    // clip 0..4c, the region cuts [1c, 2.5c) from its MIDDLE
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
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
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
        duration: Tick(90_000 * 4),
        source_in: Tick::from_audio_frames(48_000), // a source with an offset
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();

    let region = Region { start: Tick(90_000), end: Tick(90_000 * 5 / 2) };
    let block = copy_region_block(&p, region);
    assert_eq!(block.clips.len(), 1, "the region crosses one clip");

    // paste at 10c (an empty spot)
    let paste_at = Tick(90_000 * 10);
    let (mut next_clip, mut next_group, mut next_tr) = (100, 100, 100);
    let mut next_morph = 1u64;
    let mut next_cam = 1u64;
    let (cmds, _end, _) = paste_block(
        &block,
        paste_at,
        PasteIds {
            clip: &mut next_clip,
            group: &mut next_group,
            transition: &mut next_tr,
            morph: &mut next_morph,
            camera: &mut next_cam,
        },
        &PasteNames::default(),
    );
    let mut model = kt_model::TimelineModel::new(p);
    model.apply_batch(cmds).expect("paste_block is applied");
    let p2 = model.project().clone();

    // the render of the source range and of the pasted block
    let renderer = AudioRenderer::new(&p2);
    let frames = region.end.to_audio_frames() - region.start.to_audio_frames();
    let mut orig = vec![0f32; frames as usize * CH];
    no_underruns(renderer.render_frames(&mut direct(), region.start.to_audio_frames(), &mut orig));
    let mut pasted = vec![0f32; frames as usize * CH];
    no_underruns(renderer.render_frames(&mut direct(), paste_at.to_audio_frames(), &mut pasted));

    assert_eq!(orig, pasted, "the pasted piece must sound sample-exact like the original");
    assert!(orig.iter().any(|s| *s != 0.0), "the render must not be silence");
}

#[test]
fn t19_two_fragments_pasted_back_to_back_are_seamless() {
    // joining two pieces of ONE source back to back: [1c,2c) + [3c,4c) → at 10c
    // expectation: the output at the joint equals the direct read of the source without a click,
    // i.e. the render == the concatenation of two reference pieces.
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
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
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
        duration: Tick(90_000 * 6),
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
    p.validate().unwrap();

    let r1 = Region { start: Tick(90_000), end: Tick(90_000 * 2) };
    let r2 = Region { start: Tick(90_000 * 3), end: Tick(90_000 * 4) };
    let b1 = copy_region_block(&p, r1);
    let b2 = copy_region_block(&p, r2);

    let mut model = kt_model::TimelineModel::new(p);
    let (mut nc, mut ng, mut nt) = (100, 100, 100);
    let at1 = Tick(90_000 * 10);
    let mut nm = 1u64;
    let mut ncam = 1u64;
    let (cmds1, end1, _) = paste_block(
        &b1,
        at1,
        PasteIds {
            clip: &mut nc,
            group: &mut ng,
            transition: &mut nt,
            morph: &mut nm,
            camera: &mut ncam,
        },
        &PasteNames::default(),
    );
    model.apply_batch(cmds1).unwrap();
    let (cmds2, _, _) = paste_block(
        &b2,
        end1,
        PasteIds {
            clip: &mut nc,
            group: &mut ng,
            transition: &mut nt,
            morph: &mut nm,
            camera: &mut ncam,
        },
        &PasteNames::default(),
    );
    model.apply_batch(cmds2).unwrap();
    let p2 = model.project().clone();

    let renderer = AudioRenderer::new(&p2);
    let seg = 48_000usize; // 1 c each piece
    let mut spliced = vec![0f32; seg * 2 * CH];
    no_underruns(renderer.render_frames(&mut direct(), at1.to_audio_frames(), &mut spliced));

    let mut expected = vec![0f32; seg * 2 * CH];
    no_underruns(renderer.render_frames(
        &mut direct(),
        r1.start.to_audio_frames(),
        &mut expected[..seg * CH],
    ));
    no_underruns(renderer.render_frames(
        &mut direct(),
        r2.start.to_audio_frames(),
        &mut expected[seg * CH..],
    ));

    assert_eq!(spliced, expected, "the joining of pieces must be sample-exact without seams");
}
