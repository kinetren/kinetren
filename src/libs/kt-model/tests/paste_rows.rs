//! Test 595 (docs/porting-notes.md, item 450, R.56):
//! track coordinates at the paste block.
//!
//! The owner’s claim, verbatim: “I copy ZEF last in a long
//! composition… and it takes one part above and another
//! part (audio) drops down, and at the very bottom the morpher, because there was really no room for it.” The cause:
//! the clipboard stored the numbers of the WORKING VIEW, where the expansion strip is
//! shifted by `base[view]`, and the offsets
//! differ between views.

use kt_model::*;
use kt_time::Tick;

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    }
}

fn clip(id: u64, kind: ClipKind, lane: u32) -> ClipEvent {
    ClipEvent {
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
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// A video+audio pair, a morpher and a camera: exactly what the owner copied.
fn block() -> ops::ClipBlock {
    ops::ClipBlock {
        clips: vec![clip(1, ClipKind::Video, 1), clip(2, ClipKind::Audio, 3)],
        morphers: vec![MorpherEvent {
            id: MorphId(1),
            lane: 0,
            start: Tick(0),
            duration: Tick(90_000),
            targets: MorphTargets::Below,
            patch: MorphPatch::default(),
            timed: false,
            strength: 1.0,
            fade_in: Tick::ZERO,
            fade_out: Tick::ZERO,
            enabled: true,
        }],
        cameras: vec![CameraEvent {
            ease: true,
            id: CameraId(1),
            lane: 1,
            start: Tick(0),
            duration: Tick(90_000),
            name: String::new(),
            cam: CameraParams::default(),
            keyframes: vec![],
            auto: vec![],
        }],
        ..Default::default()
    }
}

fn lanes(b: &ops::ClipBlock) -> (u32, u32, u32, u32) {
    (
        b.clips.iter().find(|c| c.kind == ClipKind::Video).unwrap().lane,
        b.clips.iter().find(|c| c.kind == ClipKind::Audio).unwrap().lane,
        b.morphers[0].lane,
        b.cameras[0].lane,
    )
}

#[test]
fn t595_block_is_shifted_to_the_clicked_row() {
    use LaneKind::*;

    // click on the TOP video row: all views shift by −1, but the morpher and
    // camera have no room below zero — their shift is CLAMPED, and this is named
    let (b, clamped) = block().shifted_to_row(Some((Video, 0)));
    assert_eq!(lanes(&b), (0, 2, 0, 0), "video not aligned with the clicked row");
    assert_eq!(clamped, vec![Morph], "clipping of the shift is not named");

    // click on the TOP audio row: delta −3, three views clamped
    let (b, clamped) = block().shifted_to_row(Some((Audio, 0)));
    assert_eq!(lanes(&b), (0, 0, 0, 0));
    assert_eq!(clamped, vec![Video, Morph, Camera]);

    // click on the camera row: delta +3, the pair keeps its distance
    let (b, clamped) = block().shifted_to_row(Some((Camera, 4)));
    assert_eq!(lanes(&b), (4, 6, 3, 4), "the relative position of the views shifted");
    assert!(clamped.is_empty());

    // no view in the block — do not move at all: nothing to guess
    let mut only_video = block();
    only_video.cameras.clear();
    let (b, clamped) = only_video.shifted_to_row(Some((Camera, 4)));
    assert_eq!(b.clips[0].lane, 1, "a block without cameras moved from a click on a camera row");
    assert!(clamped.is_empty());

    // no row (Ctrl+V outside the timeline) — the block as is
    let (b, _) = block().shifted_to_row(None);
    assert_eq!(lanes(&b), (1, 3, 0, 1));
}

#[test]
fn t595b_localize_converts_a_band_block_into_reel_coordinates() {
    // expansion strip: the views have DIFFERENT bases —
    // exactly what made the pair drift apart on paste
    let base = LaneBases { video: 3, audio: 2, morph: 1, camera: 0 };
    let len = LaneBases { video: 2, audio: 2, morph: 1, camera: 2 };
    let mut b = ops::ClipBlock {
        clips: vec![
            clip(1, ClipKind::Video, 4), // strip: locally 1
            clip(2, ClipKind::Audio, 2), // strip: locally 0
            clip(3, ClipKind::Video, 0), // ROOT — the foreign one
        ],
        morphers: block().morphers,
        cameras: block().cameras,
        ..Default::default()
    };
    b.morphers[0].lane = 1; // strip: locally 0
    b.cameras[0].lane = 1; // strip: locally 1
    let dropped = b.localize(base, len);
    assert_eq!(dropped, 1, "a root event was not thrown out of the reel block");
    assert_eq!(lanes(&b), (1, 0, 0, 1), "conversion to reel coordinates is wrong");

    // and back — identity
    b.offset_lanes(base);
    assert_eq!(lanes(&b), (4, 2, 1, 1), "the reverse conversion did not match");

    // how many tracks the block needs
    let need = b.lanes_needed();
    assert_eq!((need.video, need.audio, need.morph, need.camera), (5, 3, 2, 2));
}

/// Paste into an empty project creates tracks EXPLICITLY and in one history step.
#[test]
fn t595c_paste_grows_lanes_in_one_step() {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:440"));
    let mut m = TimelineModel::new(p);
    let (b, _) = block().shifted_to_row(Some((LaneKind::Camera, 4)));

    let need = b.lanes_needed();
    let mut cmds: Vec<Command> = Vec::new();
    for (k, n) in [
        (LaneKind::Video, need.video),
        (LaneKind::Audio, need.audio),
        (LaneKind::Morph, need.morph),
        (LaneKind::Camera, need.camera),
    ] {
        let have = m.project().lanes(k);
        for at in have..n {
            cmds.push(Command::InsertTrack { kind: k, at });
        }
    }
    let (mut c, mut g, mut t, mut mo, mut ca) = (100u64, 100u64, 100u64, 100u64, 100u64);
    let (paste, _end, _skipped) = ops::paste_block(
        &b,
        Tick(0),
        ops::PasteIds {
            clip: &mut c,
            group: &mut g,
            transition: &mut t,
            morph: &mut mo,
            camera: &mut ca,
        },
        &ops::PasteNames::default(),
    );
    cmds.extend(paste);
    let depth = m.undo_depth();
    m.apply_batch(cmds).expect("insertion with growing tracks");
    assert_eq!(m.undo_depth(), depth + 1, "insertion cost more than one history step");

    let p = m.project();
    assert_eq!(
        (
            p.lanes(LaneKind::Video),
            p.lanes(LaneKind::Audio),
            p.lanes(LaneKind::Morph),
            p.lanes(LaneKind::Camera)
        ),
        (5, 7, 4, 5)
    );
    let v = p.clips.iter().find(|c| c.kind == ClipKind::Video).unwrap();
    let a = p.clips.iter().find(|c| c.kind == ClipKind::Audio).unwrap();
    assert_eq!((v.lane, a.lane), (4, 6), "the pair arrived on the wrong tracks");
    assert_eq!(v.link_group, a.link_group, "the link group of the pair fell apart");
    assert_eq!(p.morphers[0].lane, 3);
    assert_eq!(p.cameras[0].lane, 4);
}
