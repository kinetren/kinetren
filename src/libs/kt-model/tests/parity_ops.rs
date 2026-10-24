//! Tests U1 (UI parity): markers/region/tracks, apply_batch/apply_coalesced,
//! clipboard and region operations (the UI-parity plan).

use kt_model::ops::*;
use kt_model::*;
use kt_time::Tick;

fn base() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "a.mp4"));
    // A/V pair 0..90000 + a single audio 90000..180000
    p.clips.push(clip(10, Some(1), ClipKind::Video, 0, 0, 90_000, 0));
    p.clips.push(clip(11, Some(1), ClipKind::Audio, 0, 0, 90_000, 0));
    p.clips.push(clip(12, None, ClipKind::Audio, 0, 90_000, 90_000, 30_000));
    p.normalize();
    p.validate().unwrap();
    p
}

fn clip(
    id: u64,
    group: Option<u64>,
    kind: ClipKind,
    lane: u32,
    start: i64,
    dur: i64,
    src_in: i64,
) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: group.map(GroupId),
        kind,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(src_in),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

// ---- round-trip of the new fields -------------------------------------------

#[test]
fn new_fields_round_trip_and_old_json_loads() {
    let mut p = base();
    p.markers = vec![Marker::at(Tick(1000)), Marker::at(Tick(5000))];
    p.region = Some(Region { start: Tick(100), end: Tick(50_000) });
    p.loop_enabled = true;
    p.ensure_tracks(LaneKind::Video, 2);
    p.ensure_tracks(LaneKind::Audio, 3);
    let json = serde_json::to_string(&p).unwrap();
    let p2 = Project::from_json(&json).unwrap();
    assert_eq!(p, p2);

    // an “old” JSON without the new fields: we remove them and load
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let obj = v.as_object_mut().unwrap();
    // the “old” file knew the counters, and since R.62 the tracks are objects: we remove
    // both the vector and the former fields, so that exactly what the content gives is read
    for k in ["markers", "region", "loop_enabled", "tracks", "video_lanes", "audio_lanes"] {
        obj.remove(k);
    }
    let p3 = Project::from_json(&v.to_string()).unwrap();
    assert_eq!(p3.markers, vec![]);
    assert_eq!(p3.region, None);
    assert!(!p3.loop_enabled);
    assert_eq!(
        (p3.lanes(LaneKind::Video), p3.lanes(LaneKind::Audio)),
        (1, 1),
        "normalized from clips"
    );
}

// ---- apply_batch -------------------------------------------------------------

#[test]
fn apply_batch_is_atomic_and_single_undo() {
    let mut m = TimelineModel::new(base());
    let before = m.project().clone();
    // the 3rd command will fail (the clip does not exist)
    let err = m.apply_batch(vec![
        Command::AddMarker { at: Tick(1), duration: Tick::ZERO },
        Command::SetGain { id: ClipId(10), gain_db: -3.0 },
        Command::RemoveClip { id: ClipId(404) },
    ]);
    assert!(err.is_err());
    assert_eq!(m.project(), &before, "a failure of any command = zero changes");
    assert_eq!(m.undo_depth(), 0);

    // a successful batch = one undo record for the whole gesture
    m.apply_batch(vec![
        Command::RemoveClip { id: ClipId(10) },
        Command::RemoveClip { id: ClipId(11) },
    ])
    .unwrap();
    assert_eq!(m.undo_depth(), 1);
    m.undo().unwrap();
    assert_eq!(m.project(), &before, "one undo restores both clips");
}

#[test]
fn apply_coalesced_merges_gesture_into_one_undo() {
    let mut m = TimelineModel::new(base());
    let before = m.project().clone();
    for i in 0..20 {
        m.apply_coalesced(Command::SetGain { id: ClipId(12), gain_db: i as f32 }, 77).unwrap();
    }
    assert_eq!(m.undo_depth(), 1, "20 edits of one gesture = 1 undo entry");
    m.undo().unwrap();
    assert_eq!(m.project(), &before);
    // another token = a new record
    m.apply_coalesced(Command::SetGain { id: ClipId(12), gain_db: 1.0 }, 78).unwrap();
    m.apply_coalesced(Command::SetGain { id: ClipId(12), gain_db: 2.0 }, 79).unwrap();
    assert_eq!(m.undo_depth(), 2);
}

// ---- markers / region / tracks ----------------------------------------------

#[test]
fn markers_sorted_and_region_survives_undo() {
    let mut m = TimelineModel::new(base());
    m.apply(Command::AddMarker { at: Tick(5000), duration: Tick::ZERO }).unwrap();
    m.apply(Command::AddMarker { at: Tick(1000), duration: Tick::ZERO }).unwrap();
    assert_eq!(
        m.project().markers,
        vec![Marker::at(Tick(1000)), Marker::at(Tick(5000))],
        "sorting on insertion"
    );
    m.apply(Command::MoveMarker { from: Tick(1000), to: Tick(9000) }).unwrap();
    assert_eq!(m.project().markers, vec![Marker::at(Tick(5000)), Marker::at(Tick(9000))]);

    // the region is set AFTER the commands and must survive undo
    m.set_region(Some(Region { start: Tick(0), end: Tick(10) }));
    m.set_loop_enabled(true);
    m.undo().unwrap();
    assert_eq!(m.project().region, Some(Region { start: Tick(0), end: Tick(10) }));
    assert!(m.project().loop_enabled);
    m.redo().unwrap();
    assert_eq!(m.project().region, Some(Region { start: Tick(0), end: Tick(10) }));
}

#[test]
fn insert_lane_shifts_only_same_kind() {
    let mut m = TimelineModel::new(base());
    m.apply(Command::InsertTrack { kind: LaneKind::Audio, at: 0 }).unwrap();
    assert_eq!(m.project().lanes(LaneKind::Audio), 2);
    assert_eq!(m.project().lanes(LaneKind::Video), 1, "video untouched");
    for c in &m.project().clips {
        match c.kind {
            ClipKind::Audio => assert_eq!(c.lane, 1, "audio clips shifted"),
            ClipKind::Video => assert_eq!(c.lane, 0),
        }
    }
    // a nonexistent track cannot be deleted, nor the last video one
    assert!(m.apply(Command::RemoveTrack { kind: LaneKind::Audio, at: 5 }).is_err());
    assert!(matches!(
        m.apply(Command::RemoveTrack { kind: LaneKind::Video, at: 0 }),
        Err(CommandError::LastTrack(LaneKind::Video))
    ));
}

#[test]
fn remove_media_refuses_while_in_use() {
    let mut m = TimelineModel::new(base());
    assert_eq!(
        m.apply(Command::RemoveMedia { id: MediaId(1) }),
        Err(CommandError::MediaInUse(MediaId(1)))
    );
    let ids: Vec<ClipId> = m.project().clips.iter().map(|c| c.id).collect();
    m.apply_batch(ids.into_iter().map(|id| Command::RemoveClip { id }).collect()).unwrap();
    m.apply(Command::RemoveMedia { id: MediaId(1) }).unwrap();
    assert!(m.project().media.is_empty());
}

// ---- clipboard / region operations -------------------------------------------

#[test]
fn paste_block_keeps_pairs_and_chains() {
    let m = TimelineModel::new(base());
    let block = copy_group(m.project(), ClipId(10));
    assert_eq!(block.clips.len(), 2, "the whole link group");

    let mut next_clip = 100u64;
    let mut next_group = 50u64;
    let mut next_tr = 500u64;
    let mut next_morph = 1u64;
    let mut next_cam = 1u64;
    let (cmds, end1, _) = paste_block(
        &block,
        Tick(200_000),
        PasteIds {
            clip: &mut next_clip,
            group: &mut next_group,
            transition: &mut next_tr,
            morph: &mut next_morph,
            camera: &mut next_cam,
        },
        &PasteNames::default(),
    );
    let mut m2 = TimelineModel::new(m.project().clone());
    m2.apply_batch(cmds).unwrap();
    let pasted: Vec<&ClipEvent> = m2.project().clips.iter().filter(|c| c.id.0 >= 100).collect();
    assert_eq!(pasted.len(), 2);
    assert_eq!(pasted[0].link_group, pasted[1].link_group, "the pair stayed a pair");
    assert_eq!(pasted[0].link_group, Some(GroupId(50)), "NEW group");
    assert_eq!(end1, Tick(290_000));

    // chain-paste: the second block butted against the end of the first
    let (cmds2, end2, _) = paste_block(
        &block,
        end1,
        PasteIds {
            clip: &mut next_clip,
            group: &mut next_group,
            transition: &mut next_tr,
            morph: &mut next_morph,
            camera: &mut next_cam,
        },
        &PasteNames::default(),
    );
    m2.apply_batch(cmds2).unwrap();
    assert_eq!(end2, Tick(380_000));
    assert!(m2.project().clips.iter().any(|c| c.start == end1), "butt-joined without a gap");
}

#[test]
fn copy_region_crops_and_rebases() {
    let p = base();
    let r = Region { start: Tick(30_000), end: Tick(120_000) };
    let block = copy_region_block(&p, r);
    // clips 10,11 (0..90000) are cropped on the right; clip 12 (90000..180000) — on the left
    assert_eq!(block.clips.len(), 3);
    let c10 = block.clips.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(c10.start, Tick(0), "rebase to the region zero");
    assert_eq!(c10.duration, Tick(60_000));
    assert_eq!(c10.source_in, Tick(30_000), "source shift by the left cut");
    let c12 = block.clips.iter().find(|c| c.id == ClipId(12)).unwrap();
    assert_eq!(c12.start, Tick(60_000));
    assert_eq!(c12.source_in, Tick(30_000), "there was no crop on the left — source_in is original");
}

#[test]
fn punch_out_keeps_source_alignment_and_unlinks() {
    let mut m = TimelineModel::new(base());
    let r = Region { start: Tick(20_000), end: Tick(70_000) };
    let mut next = 100u64;
    let cmds = punch_out_region(m.project(), r, &mut next);
    m.apply_batch(cmds).unwrap();
    let p = m.project();
    // clips 10/11/ are cut out, 12 did not intersect — untouched
    assert!(p.clips.iter().any(|c| c.id == ClipId(12)));
    assert!(!p.clips.iter().any(|c| c.id == ClipId(10)));
    // from each of the pair — the left and right remainders
    let remnants: Vec<&ClipEvent> = p.clips.iter().filter(|c| c.id.0 >= 100).collect();
    assert_eq!(remnants.len(), 4);
    for c in &remnants {
        assert_eq!(c.link_group, None, "remainders without link (old semantics)");
        if c.start == Tick(70_000) {
            assert_eq!(c.source_in, Tick(70_000), "the right remainder continues the source");
            assert_eq!(c.duration, Tick(20_000));
        } else {
            assert_eq!(c.start, Tick(0));
            assert_eq!(c.duration, Tick(20_000));
        }
    }
}

#[test]
fn delete_region_pulls_linked_partners() {
    let mut m = TimelineModel::new(base());
    // the region touches only clip 12, but clips 10/11 (the pair) outside the region
    // must survive; we shift 11 so that the region intersects only it
    m.apply(Command::MoveClip { id: ClipId(12), new_start: Tick(500_000), new_lane: 0 }).unwrap();
    let r = Region { start: Tick(10_000), end: Tick(20_000) }; // intersects 10 and 11
    let cmds = delete_region(m.project(), r);
    m.apply_batch(cmds).unwrap();
    assert_eq!(m.project().clips.len(), 1, "the pair deleted entirely, 12 remained");
    assert_eq!(m.project().clips[0].id, ClipId(12));
}

#[test]
fn trim_group_to_cursor() {
    let mut m = TimelineModel::new(base());
    let cmds = trim_group_start_to(m.project(), ClipId(10), Tick(30_000));
    assert_eq!(cmds.len(), 2, "both members of the group");
    m.apply_batch(cmds).unwrap();
    for id in [10u64, 11] {
        let c = m.clip(ClipId(id)).unwrap();
        assert_eq!(c.start, Tick(30_000));
        assert_eq!(c.source_in, Tick(30_000));
    }
    let cmds = trim_group_end_to(m.project(), ClipId(10), Tick(60_000));
    m.apply_batch(cmds).unwrap();
    assert_eq!(m.clip(ClipId(10)).unwrap().duration, Tick(30_000));
}

/// Test 24 (model part, item 16): reversed is serialized, undo/redo
/// work, old projects without the field are read (default=false).
#[test]
fn t24_reversed_roundtrip_and_undo() {
    let p = base();
    let mut m = TimelineModel::new(p);
    let id = m.project().clips[0].id;
    m.apply(Command::SetReversed { id, reversed: true }).unwrap();
    assert!(m.project().clips[0].reversed);
    // serialization round trip
    let json = serde_json::to_string(m.project()).unwrap();
    let p2 = Project::from_json(&json).unwrap();
    assert!(p2.clips[0].reversed);
    // undo/redo
    m.undo().unwrap();
    assert!(!m.project().clips[0].reversed);
    m.redo().unwrap();
    assert!(m.project().clips[0].reversed);
    // a project WITHOUT the reversed field is read (backward compatibility)
    let stripped = json.replace("\"reversed\": true,", "").replace("\"reversed\":true,", "");
    let p3 = Project::from_json(&stripped).unwrap();
    assert!(!p3.clips[0].reversed);
}

/// Item 13: UiState (viewport) is saved into the project and read; old files
/// without a ui section load as None.
#[test]
fn ui_state_round_trip_and_old_json() {
    let mut p = base();
    p.ui = Some(UiState {
        view_start: 12_345.0,
        ticks_per_px: 450.0,
        cursor: 90_000,
        lane_order: vec![],
        ..Default::default()
    });
    let json = serde_json::to_string(&p).unwrap();
    let p2 = Project::from_json(&json).unwrap();
    assert_eq!(p2.ui, p.ui);
    // an old JSON without ui
    let stripped: String = json
        .lines()
        .filter(|l| {
            !l.contains("\"ui\"")
                && !l.contains("view_start")
                && !l.contains("ticks_per_px")
                && !l.contains("\"cursor\"")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let _ = stripped; // the json structure is nested — it is safer to assemble it without ui:
    let mut p3 = base();
    p3.ui = None;
    let plain = serde_json::to_string(&p3).unwrap();
    assert!(!plain.contains("\"ui\""), "None is not serialized");
    assert_eq!(Project::from_json(&plain).unwrap().ui, None);
}

/// Test 29 (item 6): a trim is applied to the WHOLE link group (v1) — the edges of the pair
/// no longer drift apart.
#[test]
fn t29_trim_applies_to_link_group() {
    let p = base(); // the pair 10(V)+11(A) 0..90000
    let mut m = TimelineModel::new(p);
    m.apply(Command::TrimStart { id: ClipId(10), new_start: Tick(30_000) }).unwrap();
    let v = m.project().clips.iter().find(|c| c.id == ClipId(10)).unwrap().clone();
    let a = m.project().clips.iter().find(|c| c.id == ClipId(11)).unwrap().clone();
    assert_eq!(v.start, a.start, "the partner must get the same cut");
    assert_eq!(v.duration, a.duration);

    m.apply(Command::TrimEnd { id: ClipId(11), new_end: Tick(80_000) }).unwrap();
    let v = m.project().clips.iter().find(|c| c.id == ClipId(10)).unwrap().clone();
    let a = m.project().clips.iter().find(|c| c.id == ClipId(11)).unwrap().clone();
    assert_eq!(v.start + v.duration, a.start + a.duration, "the right edges of the pair must match");
}

/// Test 30 (item 6): magnetization of a drifted pair converges and is idempotent.
#[test]
fn t30_magnetize_desynced_pair_converges() {
    let mut p = base();
    // a drifted, but linked pair; neighbour 12 removed — the shifted
    // clip 11 would now honestly catch ClipOverlap (invariant of R.24)
    p.clips.retain(|c| c.id != ClipId(12));
    p.clips.iter_mut().find(|c| c.id == ClipId(11)).unwrap().start = Tick(15_000);
    p.normalize();
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);

    let mut ng = 100u64;
    let cmds = kt_model::ops::magnetize_commands(m.project(), ClipId(10), Tick(90_000), &mut ng)
        .expect("partner exists");
    assert!(!cmds.is_empty());
    m.apply_batch(cmds).unwrap();
    let v = m.project().clips.iter().find(|c| c.id == ClipId(10)).unwrap().clone();
    let a = m.project().clips.iter().find(|c| c.id == ClipId(11)).unwrap().clone();
    assert_eq!(v.start, Tick(0), "the selected clip must NOT shift");
    assert_eq!(a.start, v.start, "partner pulled along");
    assert_eq!(a.start + a.duration, v.start + v.duration);
    assert_eq!(a.link_group, v.link_group);

    // again — no-op
    let again =
        kt_model::ops::magnetize_commands(m.project(), ClipId(10), Tick(90_000), &mut ng).unwrap();
    assert!(again.is_empty(), "already magnetized = no-op");
}

/// Test 30 (items 7-8): reorder/deletion of a track through batches is undoable.
#[test]
fn t30_lane_delete_via_batch_is_single_undo() {
    let mut p = base();
    // a second audio track with a clip
    p.ensure_tracks(LaneKind::Audio, 2);
    p.clips.push(clip(20, None, ClipKind::Audio, 1, 200_000, 90_000, 0));
    p.normalize();
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    let before = m.project().clone();

    // deletion of track A1 (lane 0): its clips are deleted, the upper ones move down
    // since R.62 this is ONE command: the track takes its events, the upper ones move down
    m.apply(Command::RemoveTrack { kind: LaneKind::Audio, at: 0 }).unwrap();
    assert_eq!(m.project().lanes(LaneKind::Audio), 1);
    assert!(m.project().clips.iter().all(|c| c.kind != ClipKind::Audio || c.lane == 0));

    m.undo().unwrap();
    assert_eq!(m.project(), &before, "one undo restores everything");
}

/// Test 89 (item 1 of round 10): markers with names — the old format (an array of numbers)
/// is read, the name survives the round trip, the order is deterministic even with
/// coinciding ticks, MoveMarker carries the name.
#[test]
fn t89_named_markers() {
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
    // an old project: markers are just numbers
    p.markers = vec![Marker::at(Tick(5000)), Marker::at(Tick(1000))];
    let old_json = r#"{"format_version":1,"settings":{"fps":30,"width":640,"height":360,"sample_rate":48000},"media":[],"clips":[],"markers":[1000,5000]}"#;
    let loaded = Project::from_json(old_json).expect("the old format must be readable");
    assert_eq!(loaded.markers, vec![Marker::at(Tick(1000)), Marker::at(Tick(5000))]);
    assert!(loaded.markers.iter().all(|m| m.name.is_empty()));

    // new format: the name survives the round trip and is not written when empty
    p.markers = vec![Marker::named(Tick(1000), "start"), Marker::at(Tick(5000))];
    p.validate().unwrap();
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("start"), "the name must serialize");
    let back = Project::from_json(&json).unwrap();
    assert_eq!(back.markers, p.markers);

    // duplicates by time collapse deterministically (stable sort)
    let mut p2 = p.clone();
    p2.markers = vec![Marker::named(Tick(2000), "first"), Marker::named(Tick(2000), "second")];
    p2.normalize();
    assert_eq!(p2.markers, vec![Marker::named(Tick(2000), "first")]);

    // MoveMarker carries the name (previously it was lost)
    let mut m = TimelineModel::new(p.clone());
    m.apply(Command::MoveMarker { from: Tick(1000), to: Tick(3000) }).unwrap();
    let moved = m.project().markers.iter().find(|k| k.t == Tick(3000)).unwrap();
    assert_eq!(moved.name, "start", "the name must move together with the marker");

    // SetMarkerName on a nonexistent marker — an error
    assert!(m.apply(Command::SetMarkerName { at: Tick(999_999), name: "x".into() }).is_err());
}

/// Test 90 (item 2 of round 10): track names — round trip, insertion/deletion
/// shift the indices, a permutation carries the name together with the content.
#[test]
fn t90_lane_names() {
    use LaneKind::{Audio, Video};
    let mut p = base();
    p.ensure_tracks(Video, 2);
    p.track_at_mut(Video, 0).unwrap().name = "Camera".into();
    p.track_at_mut(Video, 1).unwrap().name = "Titles".into();
    p.track_at_mut(Audio, 0).unwrap().name = "Voice".into();
    // the label: a name or a formula
    assert_eq!(p.track_label(Video, 0), "Camera");
    assert_eq!(p.track_label(Video, 5), "V6");
    assert_eq!(lane_label(Audio, 2), "A3");

    // inserting a track shifts those below it (the name travels with the object)
    p.insert_track(Video, 0);
    assert_eq!(p.track_label(Video, 1), "Camera");
    assert_eq!(p.track_label(Video, 2), "Titles");
    assert_eq!(p.track_label(Audio, 0), "Voice", "a foreign view is untouched");

    // deletion removes its own name and pulls up the rest
    p.remove_track(Video, 1);
    assert_eq!(p.track_label(Video, 1), "Titles");
    assert_eq!(p.track_label(Video, 0), "V1");

    // a permutation carries the name together with the content — the name lives on the
    // OBJECT, there is no separate table of names any more (R.62)
    p.set_lane_order(&[(Video, 1), (Video, 0), (Audio, 0)]);
    assert_eq!(p.track_label(Video, 0), "Titles");
    assert_eq!(p.track_label(Video, 1), "V2");

    // round trip through the project: the name is on the track, it is not in `ui`
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("Titles"));
    assert!(!json.contains("lane_names"));
    let q = Project::from_json(&json).unwrap();
    assert_eq!(q.track_label(Video, 0), "Titles");
    assert_eq!(q.track_label(Audio, 0), "Voice");
    let empty = UiState::default();
    assert!(!serde_json::to_string(&empty).unwrap().contains("lane_names"));
}

/// Test 141 (item 141, R.24): range markers (marked_range).
#[test]
fn t141_range_markers() {
    let mut m = TimelineModel::new(base());
    // a range marker + a point marker
    m.apply(Command::AddMarker { at: Tick(1_000), duration: Tick(20_000) }).unwrap();
    m.apply(Command::AddMarker { at: Tick(50_000), duration: Tick::ZERO }).unwrap();
    let mk = &m.project().markers[0];
    assert!(mk.is_range());
    assert_eq!(mk.end(), Tick(21_000));
    assert!(!m.project().markers[1].is_range());

    // duration: a command, negative — an error, foreign — not found
    m.apply(Command::SetMarkerDuration { at: Tick(50_000), duration: Tick(5_000) }).unwrap();
    assert_eq!(m.project().markers[1].duration, Tick(5_000));
    assert!(matches!(
        m.apply(Command::SetMarkerDuration { at: Tick(50_000), duration: Tick(-1) }),
        Err(CommandError::TooShort)
    ));
    assert!(matches!(
        m.apply(Command::SetMarkerDuration { at: Tick(777), duration: Tick(1) }),
        Err(CommandError::MarkerNotFound(_))
    ));

    // a move carries the duration and the colour
    let mut p = m.project().clone();
    p.markers[0].color = Some("RED".into());
    let mut m = TimelineModel::new(p);
    m.apply(Command::MoveMarker { from: Tick(1_000), to: Tick(9_000) }).unwrap();
    let moved = m.project().markers.iter().find(|k| k.t == Tick(9_000)).unwrap();
    assert_eq!(moved.duration, Tick(20_000), "duration moves");
    assert_eq!(moved.color.as_deref(), Some("RED"), "color moves");

    // interim serde: the range/colour in the round trip, legacy forms are read
    let json = serde_json::to_string(m.project()).unwrap();
    let p2 = Project::from_json(&json).unwrap();
    assert_eq!(p2.markers, m.project().markers);
    let legacy = r#"{"format_version":1,
        "settings":{"fps":30,"width":64,"height":36,"sample_rate":48000},
        "media":[],"clips":[],
        "markers":[700, {"t":1000,"name":"x"}, {"t":2000,"duration":300,"name":"r"}]}"#;
    let p3 = Project::from_json(legacy).unwrap();
    assert_eq!(p3.markers.len(), 3);
    assert_eq!(p3.markers[0], Marker::at(Tick(700)));
    assert_eq!(p3.markers[1], Marker::named(Tick(1000), "x"));
    assert!(p3.markers[2].is_range() && p3.markers[2].duration == Tick(300));
}
