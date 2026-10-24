//! Test 49 (item 6 v1.4): the display order of tracks.
//! Since R.62 (change B) the rows are a VECTOR OF OBJECT TRACKS: `normalize_lane_order`
//! remains a pure function of normalizing the order of a permutation (duplicates and garbage
//! are stripped, missing ones are appended), `lane_order_insert/remove` — the shift-aware
//! synchronization of the ordered sequence. Old files with counters and `lane_order` are read
//! into tracks one way.

use kt_model::LaneKind::{Audio as A, Camera as C, Morph as M, Video as V};
use kt_model::*;

#[test]
fn t49_normalize_lane_order() {
    // empty → the block default
    assert_eq!(normalize_lane_order(&[], 2, 2, 1, 0), vec![(V, 0), (V, 1), (A, 0), (A, 1), (M, 0)]);
    // an arbitrary valid order is preserved
    let mixed = vec![(V, 0), (A, 0), (V, 1), (M, 0), (A, 1)];
    assert_eq!(normalize_lane_order(&mixed, 2, 2, 1, 0), mixed);
    // duplicates and nonexistent ones are stripped, missing ones are appended to the tail
    let dirty = vec![(V, 0), (V, 0), (A, 7), (M, 0), (V, 1)];
    assert_eq!(normalize_lane_order(&dirty, 2, 1, 1, 0), vec![(V, 0), (M, 0), (V, 1), (A, 0)]);
    // shrinking the track count drops the trailing ones
    assert_eq!(normalize_lane_order(&mixed, 1, 1, 0, 0), vec![(V, 0), (A, 0)]);
}

#[test]
fn t49_insert_remove_shift_aware() {
    // inserting a V track at position 1: the old (V,1) → (V,2), the new (V,1)
    // sits right above it, foreign kinds are untouched
    let mut rows = vec![(V, 0), (A, 0), (V, 1), (M, 0)];
    lane_order_insert(&mut rows, V, 1);
    assert_eq!(rows, vec![(V, 0), (A, 0), (V, 1), (V, 2), (M, 0)]);
    // insertion at the end of a kind (there is no position at) — to the tail of the vector
    lane_order_insert(&mut rows, A, 1);
    assert_eq!(rows, vec![(V, 0), (A, 0), (V, 1), (V, 2), (M, 0), (A, 1)]);
    // removal shifts the larger lanes down, the row disappears
    lane_order_remove(&mut rows, V, 1);
    assert_eq!(rows, vec![(V, 0), (A, 0), (V, 1), (M, 0), (A, 1)]);
    // normalization after operations is idempotent (the vector is already correct)
    assert_eq!(normalize_lane_order(&rows, 2, 2, 1, 0), rows);
    // And `Project::insert_track` does THE SAME: the row above the former `at`; if there is no rank
    // `at` — to the end of the BLOCK of its own kind (the natural order V..A..M..C,
    // not the tail of the list: in the application the `SetLaneOrder` sets the place anyway)
    let mut p = Project::new(settings30());
    p.ensure_tracks(M, 1);
    p.set_lane_order(&[(V, 0), (A, 0), (M, 0)]);
    p.insert_track(V, 0);
    assert_eq!(p.rows(), vec![(V, 0), (V, 1), (A, 0), (M, 0)]);
    p.insert_track(A, 1);
    assert_eq!(p.rows(), vec![(V, 0), (V, 1), (A, 0), (A, 1), (M, 0)]);
    p.remove_track(V, 0);
    assert_eq!(p.rows(), vec![(V, 0), (A, 0), (A, 1), (M, 0)]);
}

#[test]
fn t49_lane_order_serde_roundtrip() {
    // Since R.35 the row order lives IN THE MODEL (a morpher layer acts “on everything below”,
    // so the order affects the picture); since R.62 it is the order of the track vector, and there
    // is no separate field in either the model or the file
    let mut p = Project::new(settings30());
    p.ensure_tracks(M, 1);
    p.set_lane_order(&[(M, 0), (V, 0), (A, 0)]);
    assert_eq!(p.rows(), vec![(M, 0), (V, 0), (A, 0)]);
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("lane_order"), "there is no separate order field any more");
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(loaded, p, "round trip of row order");
    assert_eq!(loaded.rows(), vec![(M, 0), (V, 0), (A, 0)], "rows in the same order");

    // the natural order is read the same way
    let mut nat = p.clone();
    nat.set_lane_order(&[(V, 0), (A, 0), (M, 0)]);
    let json2 = serde_json::to_string(&nat).unwrap();
    assert_eq!(Project::from_json(&json2).unwrap(), nat);
    assert_eq!(nat.rows(), vec![(V, 0), (A, 0), (M, 0)], "and it still gives the order");

    // a track row: row_of by order, not by track number
    assert_eq!(p.row_of((M, 0)), Some(0));
    assert_eq!(p.row_of((A, 0)), Some(2));
    assert_eq!(p.row_of((C, 0)), None, "there are no camera tracks");
}

/// Test 241 (R.35 C1): the row order is a state of the MODEL and goes through undo; insertion
/// and removal of a track move it INSIDE their own command.
#[test]
fn t241_lane_order_is_model_state() {
    use kt_model::{Command, TimelineModel};
    let mut m = TimelineModel::new(Project::new(settings30()));
    // inserting a morph track ITSELF puts the row into the order
    m.apply(Command::InsertTrack { kind: M, at: 0 }).unwrap();
    assert_eq!(m.project().rows(), vec![(V, 0), (A, 0), (M, 0)]);

    // a permutation is a command, and it is rolled back
    let want = vec![(M, 0), (V, 0), (A, 0)];
    m.apply(Command::SetLaneOrder { order: want.clone() }).unwrap();
    assert_eq!(m.project().rows(), want, "order was applied");
    m.undo().unwrap();
    assert_eq!(m.project().rows(), vec![(V, 0), (A, 0), (M, 0)], "undo restored the order");
    m.redo().unwrap();
    assert_eq!(m.project().rows(), want, "redo restored the permutation");

    // removing a track takes its row with it without manual synchronization
    m.apply(Command::RemoveTrack { kind: M, at: 0 }).unwrap();
    assert_eq!(m.project().rows(), vec![(V, 0), (A, 0)]);

    // garbage in the command is normalized, not passed into the model
    m.apply(Command::SetLaneOrder { order: vec![(V, 0), (V, 0), (M, 7), (A, 0)] }).unwrap();
    assert_eq!(m.project().rows(), vec![(V, 0), (A, 0)], "duplicates and nonsense are discarded");
}

/// R.29: camera tracks are a fourth kind in the display order (the tail of the V..A..M..C block),
/// insert/remove work with the same machinery.
#[test]
fn t188_camera_lanes_in_display_order() {
    assert_eq!(normalize_lane_order(&[], 1, 1, 1, 2), vec![(V, 0), (A, 0), (M, 0), (C, 0), (C, 1)]);
    // an arbitrary valid order with cameras is preserved
    let mixed = vec![(C, 0), (V, 0), (A, 0)];
    assert_eq!(normalize_lane_order(&mixed, 1, 1, 0, 1), mixed);
    // a nonexistent camera is stripped
    assert_eq!(normalize_lane_order(&mixed, 1, 1, 0, 0), vec![(V, 0), (A, 0)]);
    let mut rows = vec![(V, 0), (C, 0)];
    lane_order_insert(&mut rows, C, 0);
    assert_eq!(rows, vec![(V, 0), (C, 0), (C, 1)]);
    lane_order_remove(&mut rows, C, 0);
    assert_eq!(rows, vec![(V, 0), (C, 0)]);
    let mut p = Project::new(settings30());
    p.ensure_tracks(C, 2);
    assert_eq!(p.rows(), vec![(V, 0), (A, 0), (C, 0), (C, 1)]);
}

// ---- R.56, item 449: TRACK NUMBER = ROW RANK --------------------------------

fn settings30() -> ProjectSettings {
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

fn vclip(id: u64, lane: u32) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
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

fn three_video_lanes() -> Project {
    let mut p = Project::new(settings30());
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:440"));
    p.ensure_tracks(V, 3);
    // one event on each track, at DIFFERENT times: overlap has nothing to do with it, but
    // after renumbering the tracks must be told apart by id
    for (i, id) in [(0u32, 1u64), (1, 2), (2, 3)] {
        let mut c = vclip(id, i);
        c.start = kt_time::Tick(100_000 * i as i64);
        p.clips.push(c);
    }
    p
}

/// Test 588 (R.56, item 449). A permutation of rows RENUMBERS the content: the track number
/// within a kind is its rank among the rows of this kind.
///
/// Before R.56 the engine laid out video by NUMBER,
/// while the screen showed by `lane_order` —
/// a track could stand at the ceiling and be the bottom layer of the composite (“I dropped the
/// video there, and it is not visible”). The claim here is a CONSEQUENCE: the order of clip
/// delivery to the engine (`effective_clips`, sorted as in `VideoRenderer::build_reel`)
/// matches the order of the rows. Since R.62 the name and height live on the object track and travel with it.
#[test]
fn t588_lane_number_is_row_rank() {
    let mut p = three_video_lanes();
    // what is addressed by tracks: the area, a foreign track, the row’s name and height
    p.areas.push(MarkedArea {
        name: "main".into(),
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        lanes: vec![(V, 2)],
        color: None,
    });
    p.foreign.push(ForeignEvent {
        key: 7,
        kind: ClipKind::Video,
        lane: 2,
        start: kt_time::Tick(500_000),
        duration: kt_time::Tick(1_000),
    });
    {
        let t = p.track_at_mut(V, 2).unwrap();
        t.name = "Ceiling".into();
        t.height = 42.0;
    }
    let ceiling = p.track_at(V, 2).unwrap().id;
    p.validate().unwrap();

    let mut m = TimelineModel::new(p);
    // a person drags the bottom video row to the very top
    m.apply(Command::SetLaneOrder { order: vec![(V, 2), (V, 0), (V, 1), (A, 0)] }).unwrap();
    let p = m.project();

    // the numbers became ranks
    assert_eq!(p.rows(), vec![(V, 0), (V, 1), (V, 2), (A, 0)]);
    let lane_of = |id: u64| p.clips.iter().find(|c| c.id == ClipId(id)).unwrap().lane;
    assert_eq!(lane_of(3), 0, "the former bottom row became the TOP track");
    assert_eq!(lane_of(1), 1);
    assert_eq!(lane_of(2), 2);

    // side tables move with the same mapping, an object as an object
    assert_eq!(p.areas[0].lanes, vec![(V, 0)], "the region stayed on ITS track");
    assert_eq!(p.foreign[0].lane, 0, "a foreign track would have moved to the neighbor");
    let t = p.track_at(V, 0).unwrap();
    assert_eq!(t.id, ceiling, "track id is stable under reordering");
    assert_eq!(t.name, "Ceiling");
    assert_eq!(t.height, 42.0);

    // CONSEQUENCE: the composition order == the order of rows. The engine sorts by
    // `(Reverse(lane), start)` and draws the background first — so the LAST item in this vector
    // is the TOP layer, which is also the top row.
    let mut sorted = p.effective_clips();
    sorted.sort_by_key(|c| (std::cmp::Reverse(c.lane), c.start.0));
    let order: Vec<ClipId> = sorted.iter().map(|c| c.id).collect();
    assert_eq!(*order.last().unwrap(), ClipId(3), "the top row must be the top layer");

    // undo/redo return the numbers in full (history is snapshots)
    m.undo().unwrap();
    let p = m.project();
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane, 2, "undo did not restore the number");
    assert_eq!(p.areas[0].lanes, vec![(V, 2)]);
    assert_eq!(p.track_at(V, 2).unwrap().name, "Ceiling", "the name drifted from its track under undo");
    m.redo().unwrap();
    assert_eq!(m.project().clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane, 0);
}

/// Test 589 (R.56, item 449; R.62). The invariant is guarded by `validate`: an event on a
/// nonexistent track (a number past the last rank) is an error both at the root and in a reel;
/// reading (`normalize`) appends tracks.
#[test]
fn t589_validate_guards_canon_but_not_the_bench() {
    let mut p = three_video_lanes();
    // a track was removed, the event stayed — this is what a foreign or broken file looks like
    let gone = p.track_at(V, 2).unwrap().id;
    p.tracks.retain(|t| t.id != gone);
    assert!(
        matches!(p.validate(), Err(ProjectError::TrackMissing(None, V, 2))),
        "an event without a track at the ROOT must be an error: {:?}",
        p.validate()
    );

    // the same inside a reel
    let mut p2 = three_video_lanes();
    let mut s = Sequence::new(MediaId(9), "Reel");
    s.clips.push(vclip(10, 0));
    let mut c = vclip(11, 1);
    c.start = kt_time::Tick(200_000);
    s.clips.push(c);
    p2.sequences.push(s);
    assert!(
        matches!(p2.validate(), Err(ProjectError::TrackMissing(Some(MediaId(9)), V, 1))),
        "an event without a track in a REEL must be an error"
    );
    // and it is repaired by reading
    p2.normalize();
    p2.validate().expect("after normalize the document is canonical");
    assert_eq!(p2.sequence(MediaId(9)).unwrap().lanes(V), 2);
}

/// Test 590 (R.56, item 449; R.62). An old file knows only counters, `lane_order` and names in
/// `ui`: reading turns them into tracks — both at the root and in each reel —
/// one way,
/// and the content is renumbered to the row ranks.
#[test]
fn t590_load_canonicalizes_root_and_reels() {
    let p = three_video_lanes();
    let mut v = serde_json::to_value(&p).unwrap();
    // the envelope BEFORE R.62: without `tracks`, with counters, order and names in ui
    v.as_object_mut().unwrap().remove("tracks");
    v["video_lanes"] = serde_json::json!(3);
    v["audio_lanes"] = serde_json::json!(1);
    v["lane_order"] = serde_json::json!([["video", 2], ["audio", 0], ["video", 0], ["video", 1]]);
    v["ui"] = serde_json::json!({
        "view_start": 0.0, "ticks_per_px": 100.0, "cursor": 0,
        "lane_names": [["video", 2, "Ceiling"]],
        "lane_heights": [["video", 2, 42.0]]
    });
    v["sequences"] = serde_json::json!([{
        "id": 9, "name": "Reel", "video_lanes": 2, "audio_lanes": 1,
        "lane_order": [["video", 1], ["video", 0], ["audio", 0]],
        "clips": [
            serde_json::to_value(vclip(10, 0)).unwrap(),
            serde_json::to_value({ let mut c = vclip(11, 1); c.start = kt_time::Tick(200_000); c }).unwrap()
        ]
    }]);
    v["clips"].as_array_mut().unwrap().push(
        serde_json::to_value({
            let mut c = vclip(12, 0);
            c.media = MediaId(9);
            c.start = kt_time::Tick(900_000);
            c
        })
        .unwrap(),
    );

    let loaded = Project::from_json(&v.to_string()).expect("the old envelope reads");
    assert_eq!(loaded.rows(), vec![(V, 0), (A, 0), (V, 1), (V, 2)], "row order from lane_order");
    assert_eq!(
        loaded.clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane,
        0,
        "at the root the top row did not become track 0"
    );
    let top = loaded.track_at(V, 0).unwrap();
    assert_eq!(top.name, "Ceiling", "the name from ui.lane_names did not reach the track");
    assert_eq!(top.height, 42.0, "the height from ui.lane_heights did not reach the track");
    assert!(loaded.ui.as_ref().is_none_or(|u| u.lane_names.is_empty()), "legacy names were not cleared");
    let s = &loaded.sequences[0];
    assert_eq!(s.rows(), vec![(V, 0), (V, 1), (A, 0)]);
    assert_eq!(
        s.clips.iter().find(|c| c.id == ClipId(11)).unwrap().lane,
        0,
        "the reel is not canonicalized"
    );
    assert!(loaded.tracks.iter().all(|t| t.id.0 != 0), "ids were not assigned");
    assert!(s.tracks.iter().all(|t| t.id.0 != 0), "reel ids were not assigned");
    let all: Vec<TrackId> = loaded.tracks.iter().chain(s.tracks.iter()).map(|t| t.id).collect();
    let uniq: std::collections::HashSet<TrackId> = all.iter().copied().collect();
    assert_eq!(all.len(), uniq.len(), "track ids are not unique across the document");
}
