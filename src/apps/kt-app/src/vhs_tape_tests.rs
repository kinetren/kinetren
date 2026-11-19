//! t1193 (vhs-tape-simulation, D11; capability `video-fx`): the effects window of the VHS tape
//! simulation shows its deck face — a drum with its heads, the speed switch, the tracking field —
//! and a click on a head plus a condition writes that head's physical quantities into the MODEL.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::layout_tests::harness;
use eframe::egui;
use egui_kittest::Harness;

/// A project with one video event carrying the tape simulation; its effects window is open.
fn harness_with_tape() -> Harness<'static, KtApp> {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::builtin("vhs_tape")],
        fx_bypass: vec![false],
        fx_auto: vec![vec![]],
        opacity: 1.0,
        fx_off: false,
        masks: Default::default(),
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(kt_model::ClipId(1));
    h.step();
    h.step();
    h
}

/// A press and a release in place, as explicit events (the harness rule: `drop_at` also sends
/// PointerGone, which a click on a painted widget does not survive).
fn click(h: &mut Harness<'static, KtApp>, at: egui::Pos2) {
    h.hover_at(at);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
    h.step();
}

fn tape(h: &Harness<'static, KtApp>) -> kt_model::FxInstance {
    h.state().model.clip(kt_model::ClipId(1)).expect("the event").fx[0].clone()
}

#[test]
fn t1193_the_deck_face_sets_a_head_s_condition() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_tape();
    // the face: two heads on the drum, the speed switch, the tracking field
    let a = h.query_by_label("deck.head.0.0").expect("head A on the drum").rect();
    let b = h.query_by_label("deck.head.0.1").expect("head B on the drum").rect();
    assert!(a.center().x > b.center().x, "head A sits at 0°, head B at 180°");
    assert!(h.query_by_label(tr(k::deck::SP)).is_some() && h.query_by_label(tr(k::deck::LP)).is_some(), "no speed switch");
    assert!(h.query_by_label(tr(k::deck::PICK_HEAD)).is_some(), "the face does not say what to click");
    assert_eq!(crate::deck_face::cond_of(&tape(&h), 0, 1), crate::deck_face::Cond::Clean);

    // click head B, then "Chipped"
    click(&mut h, b.center());
    let chip = h.query_by_label(tr(k::deck::CHIPPED)).expect("the head's condition selector").rect();
    click(&mut h, chip.center());
    let f = tape(&h);
    let [s, g, o] = crate::deck_face::head_rows(0, 1);
    // the rows are stored as f32 (the file form), so compare within its precision
    let near = |row: &str, want: f64| (f.get_param(row).expect(row) - want).abs() < 1e-6;
    assert!(near(o, -6.0), "head B's output is not the chipped one");
    assert!(near(g, 0.6), "head B's gap is not the chipped one");
    assert!(near(s, 0.05), "head B's spacing is not the chipped one");
    assert_eq!(crate::deck_face::cond_of(&f, 0, 1), crate::deck_face::Cond::Chipped);
    // head A is untouched
    assert_eq!(crate::deck_face::cond_of(&f, 0, 0), crate::deck_face::Cond::Clean);

    // the speed switch writes the choice row
    let lp = h.query_by_label(tr(k::deck::LP)).expect("LP").rect();
    click(&mut h, lp.center());
    assert_eq!(tape(&h).get_param("speed"), Some(1.0), "LP did not reach the model");
    // two decks draw a second drum
    let two = h.query_by_label(tr(k::deck::TWO)).expect("two decks").rect();
    click(&mut h, two.center());
    assert_eq!(tape(&h).get_param("decks"), Some(1.0));
    assert!(h.query_by_label("deck.head.1.0").is_some(), "two decks: no play drum");
}

/// t1468 (sim-audio-pairs; capability `audio-fx`): VHS Tape on an event with linked audio puts the
/// deck's sound on that audio — visible in its chain with its own switch — and the effects window
/// shows the deck face with its Sound section on both halves; "Normal (linear)" pressed on the face
/// reaches both halves in the MODEL (the sound's own edits reach the picture: t1461).
#[test]
fn t1468_the_deck_s_sound_is_on_the_linked_audio() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    for (id, kind) in [(1, ClipKind::Video), (2, ClipKind::Audio)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: Some(GroupId(1)),
            kind,
            lane: 0,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(180_000),
            source_in: kt_time::Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
            masks: Default::default(),
        });
    }
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    // the person adds VHS Tape to the picture (the picker's door is SetFx)
    assert!(app.apply(Command::SetFx { id: ClipId(1), fx: vec![FxInstance::builtin("vhs_tape")] }));
    let audio = app.model.clip(ClipId(2)).unwrap().clone();
    assert_eq!(audio.fx.iter().map(|f| f.tag()).collect::<Vec<_>>(), ["vhs_tape_sound"], "no sound on the linked audio");
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    // the deck face on both halves, each with its Sound section
    let normals: Vec<egui::Rect> = h.query_all_by_label(tr(k::deck::NORMAL)).map(|n| n.rect()).collect();
    assert_eq!(normals.len(), 2, "the Sound section is not on both halves");
    assert!(h.query_all_by_label(tr(k::deck::SOUND)).count() == 2);
    // the first fold's face (the lower one is below the harness window's fold)
    let upper = *normals.iter().min_by(|a, b| a.center().y.total_cmp(&b.center().y)).unwrap();
    click(&mut h, upper.center());
    let m = &h.state().model;
    assert_eq!(m.clip(ClipId(1)).unwrap().fx[0].get_param("audio_out"), Some(1.0), "the deck did not take Normal");
    assert_eq!(m.clip(ClipId(2)).unwrap().fx[0].get_param("audio_out"), Some(1.0), "the deck's sound did not follow");
}

/// t1486 (sim-audio-pairs; capability `audio-fx`): Datamosh and Camera ISP on an event with linked
/// audio put the stream's sound and the camera's microphone on that audio, and the effects window
/// shows their sound controls on both halves' panels; removing the effect takes its sound off.
#[test]
fn t1486_the_stream_and_the_camera_sound_on_the_linked_audio() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    for (kind, sound) in [("datamosh", "datamosh_sound"), ("camera_isp", "camera_isp_sound")] {
        let mut h = harness();
        // after the harness: it sets the catalogue language of this thread
        let label = if kind == "datamosh" { tr(k::fx::MOSH_SOUND) } else { tr(k::isp::block::MICROPHONE) };
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
        for (id, clip_kind) in [(1, ClipKind::Video), (2, ClipKind::Audio)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(id),
                link_group: Some(GroupId(1)),
                kind: clip_kind,
                lane: 0,
                start: kt_time::Tick(0),
                duration: kt_time::Tick(180_000),
                source_in: kt_time::Tick(0),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx: vec![],
                fx_bypass: vec![],
                fx_auto: vec![],
                opacity: 1.0,
                fx_off: false,
                masks: Default::default(),
            });
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        assert!(app.apply(Command::SetFx { id: ClipId(1), fx: vec![FxInstance::builtin(kind)] }));
        let tags: Vec<&str> = app.model.clip(ClipId(2)).unwrap().fx.iter().map(|f| f.tag()).collect();
        assert_eq!(tags, [sound], "{kind}: no sound on the linked audio");
        app.open_fx_editor(ClipId(1));
        h.step();
        h.step();
        assert_eq!(h.query_all_by_label(label).count(), 2, "{kind}: the sound controls are not on both halves");
        // the device leaves both halves
        assert!(h.state_mut().apply(Command::SetFx { id: ClipId(1), fx: vec![] }));
        assert!(h.state().model.clip(ClipId(2)).unwrap().fx.is_empty(), "{kind}: the sound outlived its device");
    }
}

/// t2993 (fx-graph D2; capability `morpher`): a morpher layer's device shows its sound half in the
/// morpher window, and that half's own checkbox switches the device's sound off for the layer —
/// the picture half stays on, in the MODEL, and one undo brings the sound back.
/// - **PATH** double click a morpher block carrying VHS Tape → the morpher window shows "VHS Tape"
///   and its sound half marked with the speaker in the chain → click the checkbox of the sound half →
///   the half is struck through → the picture half stays on
#[test]
fn t2993_a_layer_s_device_sound_switches_off_in_the_morpher_window() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    for (id, kind) in [(1, ClipKind::Video), (2, ClipKind::Audio)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: Some(GroupId(1)),
            kind,
            lane: 0,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(180_000),
            source_in: kt_time::Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
            masks: Default::default(),
        });
    }
    p.ensure_tracks(LaneKind::Morph, 1);
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    assert!(app.apply(Command::AddMorpher(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(1), ClipId(2)] },
        patch: MorphPatch { video_fx: vec![FxInstance::builtin("vhs_tape")], ..Default::default() },
        timed: true,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
        enabled: true,
    })));
    app.open_morph_editor(MorphId(1));
    h.step();
    h.step();
    // the two halves carry one name; the strip marks the sound half with the speaker
    let sound = format!("{} {}", crate::icons::CAT_VOLUME, crate::fx_editor::fx_display_name(&FxInstance::builtin("vhs_tape_sound")));
    let label = h.query_by_label(&sound).unwrap_or_else(|| panic!("the morpher window does not show the sound half “{sound}”")).rect();
    // the half's own checkbox: the nearest one left of its label on the same row
    let boxes: Vec<egui::Rect> = h.query_all_by_role(egui::accesskit::Role::CheckBox).map(|n| n.rect()).collect();
    let cb = boxes
        .iter()
        .filter(|r| (r.center().y - label.center().y).abs() < 8.0 && r.right() <= label.left() + 1.0)
        .max_by(|a, b| a.right().total_cmp(&b.right()))
        .copied()
        .expect("the sound half has no checkbox of its own");
    let depth = h.state().model.undo_depth();
    click(&mut h, cb.center());
    let m = &h.state().model;
    let patch = &m.project().morphers[0].patch;
    assert_eq!(patch.audio_bypass.first().copied(), Some(true), "the sound half did not switch off");
    assert!(!patch.video_bypass.first().copied().unwrap_or(false), "the picture half switched off with it");
    let eff = m.project().effective_clips();
    let a = eff.iter().find(|c| c.kind == ClipKind::Audio).unwrap();
    assert!(a.active_fx().iter().all(|f| f.tag() != "vhs_tape_sound"), "the layer's tape still sounds");
    assert_eq!(m.undo_depth(), depth + 1, "the switch is not one history step");
}
