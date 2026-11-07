//! input-without-work (kkt-26.11.07b G5, G7; capabilities `ui-reach`, `transport`): input that
//! changes nothing does no work. The owner's VHS Tape held `dark_clip = -0.8`, its floor; the row
//! is stored as f32, -0.800000012, below the floor, so the percent field clamped it on every frame
//! and read "changed", the window wrote it back, and each mouse move rebuilt the whole engine
//! (447 ms of audio render plus the prefetch's two renderers on his 1974 events).
//!
//! The class, as tested here: no field writes from a frame of drawing (t1870, every passport at
//! its floor and ceiling); an open window over every effect, transition, track, camera and
//! morpher, with the pointer moving, changes no revision, no undo depth and no player project
//! (t1871, t1872); a command equal to what the model holds is no edit (t1873, t1874).

use crate::app::KtApp;
use eframe::egui;
use egui_kittest::Harness;
use kt_model::{FxInstance, HasParams, ParamSpec, ParamType};

/// Every number row of `p` at its floor (`max` false) or ceiling, as the carrier stores it.
fn at_bound<P: HasParams + ?Sized>(p: &mut P, max: bool) {
    let specs: Vec<ParamSpec> = p.param_specs().to_vec();
    for s in specs.iter().filter(|s| s.ty == ParamType::Num) {
        p.set_param(&s.name, if max { s.max } else { s.min });
    }
}

/// What an edit moves: the app's revision, the model's, the history and the player's project.
fn marks(h: &Harness<'static, KtApp>) -> (u64, u64, usize, u64) {
    let a = h.state();
    (a.model_rev, a.model.revision(), a.model.undo_depth(), a.player.project_version())
}

/// The pointer goes over the whole window, a frame per position.
fn sweep(h: &mut Harness<'static, KtApp>) {
    for y in (0..14).map(|i| 20.0 + i as f32 * 60.0) {
        for x in (0..8).map(|i| 40.0 + i as f32 * 180.0) {
            h.hover_at(egui::pos2(x, y));
            h.step();
        }
    }
}

/// Opened, settled, then the pointer sweeps: nothing that an edit moves may move.
fn assert_quiet(h: &mut Harness<'static, KtApp>, win: egui::Id, what: &str, problems: &mut Vec<String>) {
    h.step();
    h.step();
    // the window must be on screen, or the sweep proves nothing
    if egui::AreaState::load(&h.ctx, win).is_none() {
        problems.push(format!("{what}: the window is not drawn"));
        return;
    }
    let before = marks(h);
    sweep(h);
    let after = marks(h);
    if after != before {
        problems.push(format!(
            "{what}: (app revision, model revision, undo depth, player project) {before:?} -> {after:?}"
        ));
    }
}

/// One event of `kind` carrying `fx` in its chain; its effects window open on that slot.
fn fx_scene(fx: FxInstance, audio: bool) -> Harness<'static, KtApp> {
    use kt_model::*;
    let mut h = crate::layout_tests::harness();
    let app = h.state_mut();
    crate::layout_tests::edit_project(app, |p| {
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
            kind: if audio { ClipKind::Audio } else { ClipKind::Video },
            lane: 0,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(180_000),
            source_in: kt_time::Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![fx],
            fx_bypass: vec![false],
            fx_auto: vec![vec![]],
            opacity: 1.0,
            fx_off: false,
            masks: Default::default(),
        });
    });
    app.push_project_to_player();
    app.open_fx_editor(ClipId(1));
    if let Some(ed) = app.fx_editors.first_mut() {
        ed.selected.insert(ClipId(1), crate::widgets::ChainSel::Fx(0));
    }
    h
}

/// Test t1870: no number field writes from a frame of drawing. Every row of every effect,
/// transition, generator and track passport, at its floor and its ceiling as the carrier stores
/// it (an f32 row holds -0.8 below -0.8), drawn by the shared grid for frames with the pointer
/// over it, reports no change and leaves the value as it was.
#[test]
fn t1870_a_field_writes_nothing_from_a_frame() {
    crate::plugins::install();
    fn check<P: HasParams + Clone + PartialEq + std::fmt::Debug + 'static>(name: String, p: P, problems: &mut Vec<String>) {
        for max in [false, true] {
            let mut v = p.clone();
            at_bound(&mut v, max);
            let held = v.clone();
            let changed = std::rc::Rc::new(std::cell::Cell::new(false));
            let seen = changed.clone();
            let mut h = Harness::builder().with_size(egui::vec2(900.0, 1400.0)).build_ui_state(
                move |ui, v: &mut P| {
                    let ch = crate::widgets::params_grid_bound(ui, "t1870", v, &[], None, None);
                    if ch.any() {
                        seen.set(true);
                    }
                },
                v,
            );
            for i in 0..30 {
                h.hover_at(egui::pos2(60.0 + (i % 6) as f32 * 140.0, 10.0 + (i / 6) as f32 * 40.0));
                h.step();
            }
            let at = if max { "ceiling" } else { "floor" };
            if changed.get() {
                problems.push(format!("{name} at its {at}: a frame reported a change"));
            }
            if *h.state() != held {
                problems.push(format!("{name} at its {at}: a frame changed the values"));
            }
        }
    }
    let mut problems = Vec::new();
    for f in FxInstance::all() {
        check(format!("effect {}", f.tag()), f.clone(), &mut problems);
    }
    for t in kt_model::TransitionKind::all() {
        check(format!("transition {:?}", t), t.clone(), &mut problems);
    }
    for g in kt_model::GeneratorKind::all() {
        check(format!("generator {}", g.tag()), g, &mut problems);
    }
    for kind in [kt_model::LaneKind::Video, kt_model::LaneKind::Audio] {
        check(format!("{kind:?} track"), kt_model::Track::new(kt_model::TrackId(1), kind), &mut problems);
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Test t1871: the effects window over every effect kind, its numbers at the floor and then at
/// the ceiling, the slot selected (a device panel draws: VHS Tape's deck, Camera ISP's pipeline,
/// the codec's stream), the pointer moving over it: no revision moves, no undo step appears, the
/// player is handed no project. The owner's own VHS values are the first case.
#[test]
fn t1871_an_effects_window_under_the_pointer_changes_nothing() {
    crate::plugins::install();
    let mut problems = Vec::new();
    let mut his = FxInstance::builtin("vhs_tape");
    for (n, v) in [("standard", 1.0), ("speed", 2.0), ("doc", 0.0), ("comb", 0.0), ("rec_a_spacing", 0.09), ("rec_a_gap", 0.47), ("rec_a_output", -2.2), ("play_a_spacing", 0.2), ("tracking_um", 1.0), ("skew_um", 1.0), ("cn_db", 50.0), ("dropouts_per_s", 123.0), ("dropout_us", 60.0), ("switch_lines", 2.0), ("switch_skew_us", 3.5), ("white_clip", 2.5), ("dark_clip", -0.8), ("chroma_db", -3.0), ("tape_saturation", 1.0)] {
        his.set_param(n, v);
    }
    for audio in [false, true] {
        let mut f = his.clone();
        if audio {
            f = FxInstance::builtin("vhs_tape_sound");
            for s in his.param_specs() {
                if let Some(v) = his.get_param(&s.name) {
                    f.set_param(&s.name, v);
                }
            }
        }
        let mut h = fx_scene(f, audio);
        assert_quiet(&mut h, egui::Id::new(("fx", kt_model::ClipId(1))), &format!("the owner's VHS Tape (audio {audio})"), &mut problems);
    }
    for f in FxInstance::all() {
        if f.is_layer_mix() {
            continue; // a morpher's own slot, not an event's
        }
        for max in [false, true] {
            let mut v = f.clone();
            at_bound(&mut v, max);
            let mut h = fx_scene(v, f.is_audio());
            assert_quiet(&mut h, egui::Id::new(("fx", kt_model::ClipId(1))), &format!("{} at its {}", f.tag(), if max { "ceiling" } else { "floor" }), &mut problems);
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Test t1872: the other windows that edit a model value, open with their numbers at a bound and
/// the pointer moving: the transition window (every kind), the track window, the camera window
/// and the morpher window change nothing.
#[test]
fn t1872_every_editing_window_under_the_pointer_changes_nothing() {
    crate::plugins::install();
    use kt_model::*;
    let mut problems = Vec::new();
    for kind in TransitionKind::all() {
        for max in [false, true] {
            let (mut h, _) = crate::layout_tests::harness_three_clips_dt(None);
            let mut k = kind.clone();
            at_bound(&mut k, max);
            let t = TransitionEvent {
                id: TransitionId(77),
                // a fade at the start of the first event: a transition with an empty side
                a: None,
                b: Some(ClipId(1)),
                in_offset: kt_time::Tick(0),
                out_offset: kt_time::Tick(20_000),
                kind: k,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            };
            let app = h.state_mut();
            crate::layout_tests::edit_project(app, |p| p.transitions.push(t));
            app.push_project_to_player();
            h.state_mut().open_transition_editor(TransitionId(77));
            assert_quiet(&mut h, egui::Id::new(("transition", TransitionId(77))), &format!("transition window {kind:?} max {max}"), &mut problems);
        }
    }
    for kind in [LaneKind::Video, LaneKind::Audio] {
        for max in [false, true] {
            let (mut h, _) = crate::layout_tests::harness_three_clips_dt(None);
            let app = h.state_mut();
            let id = app.model.project().tracks.iter().find(|t| t.kind == kind).map(|t| t.id).expect("a track");
            crate::layout_tests::edit_project(app, |p| {
                if let Some(t) = p.tracks.iter_mut().find(|t| t.id == id) {
                    at_bound(t, max);
                }
            });
            app.push_project_to_player();
            app.open_track_window(id);
            assert_quiet(&mut h, egui::Id::new(("track_window", id.0)), &format!("{kind:?} track window max {max}"), &mut problems);
        }
    }
    {
        let mut h = crate::layout_tests::harness_sized(1600.0, 900.0);
        crate::layout_tests::owner_morpher_scene(&mut h);
        let app = h.state_mut();
        crate::layout_tests::edit_project(app, |p| {
            p.ensure_tracks(LaneKind::Camera, 1);
            p.cameras.push(CameraEvent {
                ease: true,
                id: CameraId(1),
                lane: 0,
                start: kt_time::Tick(0),
                duration: kt_time::Tick(120_000),
                name: String::new(),
                cam: CameraParams::default(),
                keyframes: vec![],
                auto: vec![],
            });
        });
        app.push_project_to_player();
        for ed in app.morph_editors.iter_mut() {
            ed.open = false;
        }
        app.open_camera_editor(CameraId(1));
        assert_quiet(&mut h, egui::Id::new(("camera", CameraId(1))), "camera window", &mut problems);
        for ed in h.state_mut().camera_editors.iter_mut() {
            ed.open = false;
        }
        h.state_mut().open_morph_editor(MorphId(1));
        assert_quiet(&mut h, egui::Id::new(("morph", 1u64)), "morpher window", &mut problems);
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Test t1874: the app's edit doors take a command equal to what the model holds as no edit: no
/// revision, no undo step, no project handed to the player, no dirty mark. A real change still
/// does all of it, once.
#[test]
fn t1874_an_equal_command_is_no_edit() {
    crate::plugins::install();
    let mut h = fx_scene(FxInstance::builtin("vhs_tape"), false);
    h.step();
    let fx = h.state().model.clip(kt_model::ClipId(1)).expect("the event").fx.clone();
    h.state_mut().dirty_content = false;
    let before = marks(&h);
    let app = h.state_mut();
    let g = app.gesture_counter + 1;
    assert!(app.apply_coalesced(kt_model::Command::SetFx { id: kt_model::ClipId(1), fx: fx.clone() }, g));
    assert!(app.apply(kt_model::Command::SetFx { id: kt_model::ClipId(1), fx: fx.clone() }));
    assert!(app.apply_batch(vec![kt_model::Command::SetFx { id: kt_model::ClipId(1), fx: fx.clone() }]));
    assert_eq!(marks(&h), before, "an equal command moved something");
    assert!(!h.state().dirty_content, "an equal command marked the project dirty");
    let mut changed = fx;
    changed[0].set_param("cn_db", 30.0);
    let app = h.state_mut();
    assert!(app.apply_coalesced(kt_model::Command::SetFx { id: kt_model::ClipId(1), fx: changed }, g));
    let after = marks(&h);
    assert_eq!((after.0, after.1, after.2), (before.0 + 1, before.1 + 1, before.2 + 1), "a real change is one edit");
    assert!(after.3 > before.3, "a real change reaches the player");
    assert!(h.state().dirty_content);
}

/// Test t1876: a held level drag hands the player a project only on frames where the level moved.
/// Every held frame swapped the whole project in, and a swap rebuilds the engine.
#[test]
fn t1876_a_held_level_drag_hands_the_player_nothing() {
    use crate::layout_tests::{r72_event, r72_scene, r72_view};
    use crate::timeline::{gain_to_level, level_y};
    use kt_model::ClipKind;
    crate::plugins::install();
    let mut c = r72_event(1, ClipKind::Audio, 0, 5, 10);
    c.gain_db = -6.0;
    let mut h = r72_scene(crate::i18n::Lang::En, None, vec![c]);
    h.state_mut().tl_v_zoom = Some(84.0);
    r72_view(&mut h, 0.0);
    let g = h.state().tl_geom.clone().unwrap();
    let (top, lh) = (g.lane_top(ClipKind::Audio, 0), g.lane_h_of(ClipKind::Audio, 0));
    let from = egui::pos2(g.tick_to_x(10 * 90_000), level_y(gain_to_level(-6.0), top, lh));
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    let up = from + egui::vec2(0.0, -10.0);
    h.hover_at(up);
    h.step();
    assert!(h.state().live_gain.is_some(), "scene: the level drag did not start");
    let held = h.state().player.project_version();
    for i in 0..12 {
        // the pointer holds still, then slides along the line: the level does not move
        h.hover_at(up + egui::vec2(if i < 6 { 0.0 } else { (i - 5) as f32 * 4.0 }, 0.0));
        h.step();
    }
    assert_eq!(h.state().player.project_version(), held, "a held level drag swapped the project in");
    h.hover_at(up + egui::vec2(0.0, -8.0));
    h.step();
    assert_eq!(h.state().player.project_version(), held + 1, "a moved level reaches the player once");
    h.drop_at(up + egui::vec2(0.0, -8.0));
    h.step();
}

/// Test t1877: a colour row written back from its colour button holds the colour it had: the
/// popup's float round trip is rounded, not truncated, and an equal colour is no write.
#[test]
fn t1877_a_colour_written_back_is_the_colour_held() {
    crate::plugins::install();
    let FxInstance::Plugin(mut p) = FxInstance::builtin("greenscreen") else { panic!("a plugin slot") };
    let rows = ["key_r", "key_g", "key_b"];
    for v in 0..=255u32 {
        let held = [v, 255 - v, v / 2];
        for (n, x) in rows.iter().zip(held) {
            p.set(n, x as f64);
        }
        // what the button hands back: the held colour through egui's HSV round trip
        let c = egui::ecolor::Hsva::from_rgb(held.map(|x| x as f32 / 255.0)).to_rgb();
        let before = p.clone();
        let wrote = crate::fx_editor::write_rgb8(&mut p, rows, c);
        assert!(!wrote && p == before, "colour {held:?} came back as {:?}", rows.map(|n| p.get(n)));
    }
}
