//! Round trip “loaded → EDITED → saved → loaded” (R.49).
//!
//! kt-otio had not a single such test: the round trips checked loading and writing
//! of an UNTOUCHED project. Yet it is editing that creates new
//! events — the right half of a cut, bake pieces, insertion, clips from the bin —
//! and they have no envelope brought from the file. Everything that lived in the envelope
//! (material timecode, foreign references) vanished for them silently.

use kt_model::*;
use kt_otio::{load_project, save_project};
use kt_time::Tick;

/// A file with the hour of embedded timecode: `available_range` does not start at zero.
fn doc_with_timecode(src_start: f64, avail_start: f64) -> String {
    format!(
        r#"{{"OTIO_SCHEMA":"Timeline.1","name":"tc","tracks":{{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {{"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":480,"rate":24}},
    "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":{src_start},"rate":24}}}},
    "media_reference":{{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
    "available_range":{{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":3890,"rate":24}},
    "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":{avail_start},"rate":24}}}},
    "available_image_bounds":null,"metadata":{{}}}},
    "effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}},
    "global_start_time":null,"metadata":{{}}}}"#
    )
}

/// Test 377 (R.49 A8): an event BORN from an edit is written in the same
/// coordinates of the material as its neighbour from the file.
///
/// The hour of embedded timecode (Sol Levante has exactly one hour) is the norm for broadcast and
/// cinema files. `available_range` lay in the CLIP's envelope, but a newborn
/// (the right half of a cut, a bake piece, an insertion, an event from the bin) has no
/// envelope. The observable consequence: two halves of ONE cut travelled into the file in
/// DIFFERENT coordinate systems — the left from the hour, the right from zero. Our round trip did not
/// notice this (we subtract exactly what we wrote), while a foreign editor saw the right half at the 10th second
/// instead of 1:00:10.
///
/// The cure — a MEDIA envelope: `available_range` describes the FILE, not the event.
#[test]
fn t377_born_clip_keeps_media_start_timecode() {
    let hour = 86_400.0; // hour @24 fps
    let b = load_project(&doc_with_timecode(hour + 240.0, hour), None).expect("file reads");
    let mut m = TimelineModel::new(b.project);
    let src = m.project().clips[0].clone();
    assert_eq!(src.source_in, Tick(240 * 90_000 / 24), "10 s from the start of the file");

    // we CUT in half: the right half is a newborn, it has no envelope
    let mid = src.start + Tick(src.duration.0 / 2);
    let next = m.project().clips.iter().map(|c| c.id.0).max().unwrap() + 1;
    m.apply(Command::Split { id: src.id, at: mid, new_id: ClipId(next), new_group: None })
        .expect("split");
    assert_eq!(m.project().clips.len(), 2, "after the split, two events");

    let right = m.project().clips.iter().max_by_key(|c| c.start.0).unwrap().clone();
    assert!(right.source_in > src.source_in, "right half enters the material later");

    // round: saved and read back
    let out = save_project(m.project(), &b.retained, None);
    let back = load_project(&out, None).expect("own output is readable");
    let mut got: Vec<_> = back.project.clips.clone();
    got.sort_by_key(|c| c.start.0);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].source_in, src.source_in, "left half shifted");
    assert_eq!(
        got[1].source_in, right.source_in,
        "right half was moved to the start timecode of the material: \
         on a clock timecode this is an hour of emptiness instead of a picture"
    );

    // the main consequence: in the FILE both halves lie in the coordinates of the material
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let tick_of = |n: &serde_json::Value| -> i64 {
        (n["value"].as_f64().unwrap() * 90_000.0 / n["rate"].as_f64().unwrap()).round() as i64
    };
    let hour_ticks = (hour as i64) * 90_000 / 24;
    let kids0 = &v["tracks"]["children"][0]["children"];
    for (i, want) in [src.source_in, right.source_in].iter().enumerate() {
        assert_eq!(
            tick_of(&kids0[i]["source_range"]["start_time"]),
            hour_ticks + want.0,
            "clip {i} was written NOT in material coordinates: a foreign editor will see it \
             an hour earlier than it is"
        );
    }
    let kids = &v["tracks"]["children"][0]["children"];
    for i in 0..2 {
        // the writer normalizes the clip into the Clip.2 schema — the reference map
        let refs = &kids[i]["media_references"];
        let key = kids[i]["active_media_reference_key"].as_str().expect("no active key");
        let ar = &refs[key]["available_range"];
        assert!(!ar.is_null(), "clip {i}: available_range lost");
        assert_eq!(ar["start_time"]["value"].as_f64().unwrap(), hour);
    }
}

/// Test 378 (R.49 A8): the media envelope does not overwrite the clip's OWN envelope.
/// A foreign editor may have left special references on one event (several variants of material,
/// its own `active_key`); the media level is a fallback path for
/// newborns, not a common denominator for all.
#[test]
fn t378_media_refs_do_not_override_clip_own_refs() {
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {"OTIO_SCHEMA":"Clip.1","name":"c1","source_range":{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}},
    "media_references":{"DEFAULT_MEDIA":{"OTIO_SCHEMA":"ExternalReference.1","name":"",
    "target_url":"a.mov","available_range":null,"available_image_bounds":null,"metadata":{}},
    "proxy":{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a_proxy.mov",
    "available_range":null,"available_image_bounds":null,"metadata":{}}},
    "active_media_reference_key":"proxy",
    "effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("file with two references is readable");
    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let c = &v["tracks"]["children"][0]["children"][0];
    assert_eq!(c["active_media_reference_key"], "proxy", "active key of the clip was substituted");
    assert!(
        c["media_references"]["proxy"].is_object()
            && c["media_references"]["DEFAULT_MEDIA"].is_object(),
        "one of the references of the clip was lost: {}",
        c["media_references"]
    );
}

/// Test 380 (R.49 A9): a derived event inherits the parent's envelope — name,
/// foreign effects and foreign metadata. Before R.49 an unnamed clip travelled into the file without
/// everything that a foreign editor had put there: a half of a cut “lost”
/// an incomprehensible time warp, and a foreign namespace.
#[test]
fn t380_born_clip_inherits_name_effects_and_metadata() {
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {"OTIO_SCHEMA":"Clip.1","name":"Shot 12A","source_range":{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{"OTIO_SCHEMA":"RationalTime.1","value":480,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}},
    "media_reference":{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
    "available_range":null,"available_image_bounds":null,"metadata":{}},
    "effects":[{"OTIO_SCHEMA":"Effect.1","name":"their effect","effect_name":"AlienBlur",
    "enabled":true,"metadata":{"vendor":{"strength":3}}}],
    "markers":[],"enabled":true,"metadata":{"resolve":{"clip_color":"Orange"}}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("file reads");
    let mut m = TimelineModel::new(b.project);
    let mut retained = b.retained;
    let src = m.project().clips[0].clone();

    let mid = src.start + Tick(src.duration.0 / 2);
    let next = ClipId(src.id.0 + 1);
    let births = m
        .apply_batch(vec![Command::Split { id: src.id, at: mid, new_id: next, new_group: None }])
        .expect("split");
    assert_eq!(births.parent_of(next), Some(src.id), "model did not name the parent");

    // exactly what the application does by the model's answer
    for (child, parent) in &births.pairs {
        if let Some(ex) = retained.clips.get(parent).cloned() {
            retained.clips.entry(*child).or_insert(ex);
        }
    }

    let out = save_project(m.project(), &retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let kids = &v["tracks"]["children"][0]["children"];
    for i in 0..2 {
        assert_eq!(kids[i]["name"], "Shot 12A", "half {i} lost the event name");
        assert_eq!(
            kids[i]["metadata"]["resolve"]["clip_color"], "Orange",
            "half {i} lost foreign metadata"
        );
        let fx = kids[i]["effects"].as_array().expect("no effects list");
        assert!(
            fx.iter().any(|e| e["effect_name"] == "AlienBlur"),
            "half {i} lost a foreign effect: {fx:?}"
        );
    }
}

/// Test 381 (R.49 A9): markers are distributed by the WRITER — by intersection with the
/// window of the material. The envelope is inherited whole on purpose: distribute them at the cut —
/// and undo would return the parent without markers, which had moved to the children.
#[test]
fn t381_markers_are_dealt_by_the_writer() {
    // three markers: in the first half (frame 100), in the second (frame 400) and on
    // the very boundary (frame 240 — already the second half)
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{"OTIO_SCHEMA":"RationalTime.1","value":480,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}},
    "media_reference":{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
    "available_range":null,"available_image_bounds":null,"metadata":{}},
    "effects":[],"markers":[
    {"OTIO_SCHEMA":"Marker.2","name":"early","color":"RED","marked_range":
    {"OTIO_SCHEMA":"TimeRange.1","duration":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":100,"rate":24}},"metadata":{}},
    {"OTIO_SCHEMA":"Marker.2","name":"boundary","color":"RED","marked_range":
    {"OTIO_SCHEMA":"TimeRange.1","duration":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":240,"rate":24}},"metadata":{}},
    {"OTIO_SCHEMA":"Marker.2","name":"late","color":"RED","marked_range":
    {"OTIO_SCHEMA":"TimeRange.1","duration":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":400,"rate":24}},"metadata":{}}],
    "enabled":true,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("file with markers is readable");
    let mut m = TimelineModel::new(b.project);
    let mut retained = b.retained;
    let src = m.project().clips[0].clone();
    assert_eq!(retained.clips.get(&src.id).map(|e| e.markers.len()), Some(3));

    let mid = src.start + Tick(src.duration.0 / 2);
    let next = ClipId(src.id.0 + 1);
    let births = m
        .apply_batch(vec![Command::Split { id: src.id, at: mid, new_id: next, new_group: None }])
        .expect("split");
    for (child, parent) in &births.pairs {
        if let Some(ex) = retained.clips.get(parent).cloned() {
            retained.clips.entry(*child).or_insert(ex);
        }
    }
    // the child's envelope received ALL three — the writer will distribute them
    assert_eq!(retained.clips.get(&next).map(|e| e.markers.len()), Some(3));

    let out = save_project(m.project(), &retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let kids = &v["tracks"]["children"][0]["children"];
    let names = |i: usize| -> Vec<String> {
        kids[i]["markers"]
            .as_array()
            .expect("no markers")
            .iter()
            .map(|m| m["name"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    assert_eq!(names(0), vec!["early"], "first half received foreign markers");
    assert_eq!(names(1), vec!["boundary", "late"], "second half lost its markers");
}
