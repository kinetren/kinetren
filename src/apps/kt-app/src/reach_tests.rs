//! REACHABILITY tests (R.38, items 252–255) — the mechanism of “there are no dead ends.”
//!
//! The trigger, verbatim: “half of it is dead ends… I am tired of reminding agents to
//! think about these details. MAKE THEM CHECK THIS, WHAT IS THE PROBLEM??”
//!
//! The problem is that the checklist in `apps/kt-app/CLAUDE.md` is PROSE, and prose
//! breaks nothing. Three R.38 complaints violate the rules recorded there
//! verbatim (1 “hit it with the mouse the first time”, 7 “the text fits”,
//! 17 “paired ways are identical”), and it did not help.
//!
//! Evidence of the failure mechanism: `RemoveArea`/`RenameArea`/
//! `SetAreaRange`/`SetAreaLanes` are implemented in `kt-model` and covered
//! by unit tests, yet the UI calls them NOT ONCE — an area can be neither removed
//! nor renamed. The model’s unit test is green while the UI has not reached it:
//! the tests checked the model, not REACHABILITY.
//!
//! Hence the four tests of this file. They do not replace `scripts/live-check.sh`
//! (the repository rule “the harness is not proof that the feature
//! works for the user”); they only make the error RED before
//! the user finds it.

use eframe::egui;
use std::path::{Path, PathBuf};

pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Strip comments: in the body of `enum Command` and in `ops.rs` there are no string
/// literals with `//`, so a full tokenizer is not needed — the same
/// level of strictness as the font scanner (`font_tests`).
pub(crate) fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut depth = 0usize; // nested /* */
    while i < b.len() {
        if depth > 0 {
            if b[i..].starts_with(b"/*") {
                depth += 1;
                i += 2;
            } else if b[i..].starts_with(b"*/") {
                depth -= 1;
                i += 2;
            } else {
                if b[i] == b'\n' {
                    out.push('\n');
                }
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            depth = 1;
            i += 2;
        } else if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

/// The body of a block starting with `{` at position `open`, by bracket balance.
fn balanced_body(src: &str, open: usize) -> &str {
    let b = src.as_bytes();
    let mut depth = 0usize;
    for (k, &c) in b.iter().enumerate().skip(open) {
        match c {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open + 1..k];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced brackets while parsing the source");
}

/// The names of the `Command` variants come from the model’s SOURCE, not from a list in
/// the test: a list in the test would become a second place of truth and would diverge exactly
/// when people stopped maintaining it.
fn command_variants() -> Vec<String> {
    let path = workspace_root().join("libs/kt-model/src/model.rs");
    let src =
        strip_comments(&std::fs::read_to_string(&path).expect("no libs/kt-model/src/model.rs"));
    let at = src.find("pub enum Command {").expect("model.rs has no `pub enum Command {`");
    let body = balanced_body(&src, src[at..].find('{').unwrap() + at);

    let b = body.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut item_start = true; // we are at the start of an enumeration element
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'{' | b'(' | b'[' => {
                depth += 1;
                i += 1;
                continue;
            }
            b'}' | b')' | b']' => {
                depth -= 1;
                i += 1;
                continue;
            }
            b',' if depth == 0 => {
                item_start = true;
                i += 1;
                continue;
            }
            _ => {}
        }
        if depth != 0 || c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'#' {
            // an attribute — skip it whole, do not reset the element start
            while i < b.len() && b[i] != b']' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if item_start && c.is_ascii_uppercase() {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(body[s..i].to_string());
            item_start = false;
            continue;
        }
        item_start = false;
        i += 1;
    }
    out
}

/// `Command::X` in the text — but exactly `X`, not `Xyz`.
///
/// Word boundaries here are not pedantry, but two real false results:
/// `Command::SetFx` is a prefix of `SetFxOff`/`SetFxAuto`/`SetFxBypass`, and
/// `egui::ViewportCommand::Close` contains the substring `Command::Close`.
fn constructs(hay: &str, name: &str) -> bool {
    let needle = format!("Command::{name}");
    let b = hay.as_bytes();
    hay.match_indices(&needle).any(|(i, _)| {
        let before = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        let j = i + needle.len();
        let after = j >= b.len() || !(b[j].is_ascii_alphanumeric() || b[j] == b'_');
        before && after
    })
}

/// Top-level elements of `ops.rs`: (function name, body).
fn ops_items() -> Vec<(String, String)> {
    let path = workspace_root().join("libs/kt-model/src/ops.rs");
    let src =
        strip_comments(&std::fs::read_to_string(&path).expect("no libs/kt-model/src/ops.rs"));
    let mut out = Vec::new();
    let mut starts: Vec<usize> = Vec::new();
    for (off, line) in src.match_indices('\n') {
        let rest = &src[off + 1..];
        if rest.starts_with("pub fn ") || rest.starts_with("fn ") {
            starts.push(off + 1);
        }
        let _ = line;
    }
    for (k, &s) in starts.iter().enumerate() {
        let e = starts.get(k + 1).copied().unwrap_or(src.len());
        let head = &src[s..];
        let name_at = head.find("fn ").unwrap() + 3;
        let name: String =
            head[name_at..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        out.push((name, src[s..e].to_string()));
    }
    out
}

/// The code from which the user actually reaches the model:
/// the non-test sources of kt-app plus the bodies of those `kt_model::ops`
/// functions that kt-app calls. The second level is mandatory: `LinkClips`
/// constructs only `ops::magnetize_commands`.
fn production_corpus() -> Vec<(String, String)> {
    let mut out = crate::font_tests::sources();
    let app: String = out.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("\n");
    for (name, body) in ops_items() {
        if app.contains(&format!("ops::{name}")) {
            out.push((format!("kt-model/ops.rs::{name}"), body));
        }
    }
    out
}

/// Commands that the user does NOT invoke from the GUI, with the reason.
///
/// The list starts EMPTY and must stay so: if a command lands here, it means
/// nobody calls it, and this must be explained in a sentence, not with the
/// word “TODO”. Once it is wired to the UI, it must be removed from here (otherwise the list will
/// cover everything within a year and stop meaning anything).
const NOT_FROM_UI: &[(&str, &str)] = &[];

/// Test 252 (R.38): there are no orphaned commands.
///
/// A command that is implemented and covered by unit tests, but that no line
/// of the application creates, is a feature the user cannot reach. This is exactly
/// how the named areas of R.35 went missing.
#[test]
fn t252_no_orphan_commands() {
    let variants = command_variants();

    // first — a self-check of the parser: a broken parser would yield an empty
    // list and a “green test that checks nothing”
    assert!(
        variants.len() >= 40,
        "the parser of Command variants broke: found {}, expected at least 40",
        variants.len()
    );
    for sentinel in ["AddClip", "SetFxAuto", "RemoveTrack"] {
        assert!(
            variants.iter().any(|v| v == sentinel),
            "the parser lost Command::{sentinel} — any further result means nothing"
        );
    }
    let model = std::fs::read_to_string(workspace_root().join("libs/kt-model/src/model.rs"))
        .expect("no model.rs");
    for v in &variants {
        assert!(
            constructs(&model, v),
            "Command::{v} is parsed from the enum, but model.rs does not handle it — \
             which means the parse caught the wrong thing"
        );
    }
    {
        let mut seen = variants.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), variants.len(), "the parse produced duplicate variants");
    }

    let corpus = production_corpus();

    // idioms that would blind the scan are forbidden by THE SAME test —
    // otherwise the mechanism is switched off with one `use` line
    for (file, src) in &corpus {
        assert!(
            !src.contains("Command::*"),
            "{file}: `use …Command::*` hides the construction of commands from test 252 — \
             write `Command::X` with the full path (the same rule as the i18n keys in t125)"
        );
        assert!(
            !src.contains("use kt_model::ops::") && !src.contains("use kt_model::ops;"),
            "{file}: ops functions are called by the full path `kt_model::ops::foo` — \
             a shortened import hides the second level of the scan from test 252"
        );
    }

    let text: String = corpus.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("\n");
    let allowed: Vec<&str> = NOT_FROM_UI.iter().map(|(n, _)| *n).collect();

    // hygiene of the exception list itself
    for (name, reason) in NOT_FROM_UI {
        assert!(
            variants.iter().any(|v| v == name),
            "NOT_FROM_UI mentions Command::{name}, which the model no longer has"
        );
        assert!(
            !constructs(&text, name),
            "Command::{name} is now called from the application — remove it from NOT_FROM_UI, \
             or the exceptions list will turn into a mess"
        );
        assert!(
            reason.trim().chars().count() >= 40,
            "the reason for Command::{name} is shorter than 40 characters: {reason:?}. \
             The reason must be a sentence, not “TODO”"
        );
    }

    let orphans: Vec<&String> = variants
        .iter()
        .filter(|v| !allowed.contains(&v.as_str()) && !constructs(&text, v))
        .collect();
    assert!(
        orphans.is_empty(),
        "orphaned commands: implemented (and usually covered by unit tests), but no line of the application \
         creates them — the user physically cannot invoke them. \
         Wire them into the UI or remove them from the model; a unit test on the model is NOT proof \
         of reachability, which is how the areas of R.35 went astray: {orphans:?}"
    );
}

/// A project with ALL entities of the timeline and ALL on-screen points from which
/// a person acts on one of them.
///
/// The scene is its own for each entity: the program clears region and selection on
/// Esc, and the menu cannot otherwise be closed — a shared harness would silently break the scene of
/// the next check.
///
/// **There are several points, and this is the main correction of R.40.** Before it, tracks were
/// probed ONLY in the row header (`gutter_x`) — a 34 px strip, where
/// I had myself put the item “add morpher here”. The test was green, while a
/// person who right-clicked on the track itself got the menu of empty space:
/// what was checked was the place where I put it, not the place where the person actually points.
fn scene(
    e: crate::timeline::TlEntity,
) -> (egui_kittest::Harness<'static, crate::app::KtApp>, Vec<(&'static str, egui::Pos2)>) {
    use crate::timeline::TlEntity as E;
    use kt_model::*;
    use kt_time::Tick;

    let mut h = crate::layout_tests::harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
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
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(0),
            duration: Tick(50_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        if e == E::Generator {
            // generator event: the media carries ONLY the KIND, and the VALUES live
            // on the event (R.59, stage 2) — otherwise `validate` rejects
            // the scene, and that is right: without values the generator has nothing
            // to draw
            p.media.push(MediaRef::new(MediaId(200), "gen:solid"));
            let mut genclip = p.clips[0].clone();
            genclip.id = ClipId(4);
            genclip.media = MediaId(200);
            genclip.start = Tick(600_000);
            genclip.r#gen = Some(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
            p.clips.push(genclip);
        }
        if e == E::Nest {
            // NESTING: a strip with one event inside and an instance on
            // a track — exactly what a person right-clicks
            let mut seq = Sequence::new(MediaId(100), "Intro");
            let mut inner = p.clips[0].clone();
            inner.id = ClipId(50);
            inner.start = Tick(0);
            seq.clips.push(inner);
            p.sequences.push(seq);
            let mut inst = p.clips[0].clone();
            inst.id = ClipId(3);
            inst.media = MediaId(100);
            inst.start = Tick(600_000);
            inst.source_in = Tick(0);
            p.clips.push(inst);
        }
        if e == E::Transition {
            // a crossfade between two events: the window is wider than the handle zone, and
            // the right click targets what is VISIBLE, not ±6 px at the joint
            let mut second = p.clips[0].clone();
            second.id = ClipId(2);
            second.start = Tick(50_000);
            p.clips.push(second);
            p.transitions.push(TransitionEvent {
                id: TransitionId(1),
                a: Some(ClipId(1)),
                b: Some(ClipId(2)),
                in_offset: Tick(20_000),
                out_offset: Tick(20_000),
                kind: TransitionKind::Crossfade,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            });
        }
        p.markers.push(Marker::named(Tick(60_000), "marker"));
        p.areas.push(MarkedArea {
            name: "area".into(),
            start: Tick(120_000),
            duration: Tick(50_000),
            lanes: vec![(LaneKind::Video, 0)],
            color: None,
        });
        p.region = Some(kt_model::Region { start: Tick(300_000), end: Tick(350_000) });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = 1000.0;
        // the morph block is placed NOT by region: otherwise it would cover the boundary
        // of the region, and the dispute over one pixel would be settled by the hit-test order
        app.player.seek(Tick(200_000));
        app.add_morph_layer();
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
        app.player.seek(Tick(250_000));
        app.add_camera_at_playhead(0);
        // the camera is shorter: the scene needs FREE space on its row —
        // without it, “right-click on the track” would probe the camera block itself
        let mut cam = app.model.project().cameras[0].clone();
        cam.duration = Tick(50_000);
        assert!(app.apply(Command::SetCamera(cam)));
        // the editor windows that placement opens by itself are closed: their captions
        // would get into the count of menu items
        app.morph_editors.clear();
    }
    h.step();
    h.step();

    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let pr = h.state().model.project().clone();
    assert_eq!(pr.morphers.len(), 1, "scene: morpher");
    assert_eq!(pr.cameras.len(), 1, "scene: camera");
    assert!(pr.region.is_some(), "scene: region");

    let morph_row = g.row_of((LaneKind::Morph, 0)).expect("morph row in the layout");
    let cam_row = g.row_of((LaneKind::Camera, 0)).expect("camera row in the layout");
    let video_row = g.row_of((LaneKind::Video, 0)).expect("video row in the layout");
    let gutter_x = g.rect.left() - 30.0;
    let ruler_y = g.rect.top() + 6.0;
    let m = &pr.morphers[0];
    let c = &pr.cameras[0];

    // empty space ON the track — to the right of all the scene’s events, but on its row
    // and ON SCREEN: a point outside the visible strip would “check” empty space past the edge
    let free_t = 500_000;
    let free_x = g.tick_to_x(free_t);
    assert!(
        free_x > g.rect.left() && free_x < g.rect.right(),
        "the “free space” point ({free_x}) is outside the track strip {:?} — the scene is broken",
        g.rect
    );
    for (a, b) in pr
        .clips
        .iter()
        .map(|c| (c.start.0, (c.start + c.duration).0))
        .chain(pr.morphers.iter().map(|m| (m.start.0, (m.start + m.duration).0)))
        .chain(pr.cameras.iter().map(|c| (c.start.0, (c.start + c.duration).0)))
    {
        assert!(
            free_t < a || free_t >= b,
            "the “free space” point fell into the event {a}..{b} — the scene is broken"
        );
    }
    let pos = match e {
        E::Clip => vec![(
            "event body",
            egui::pos2(g.tick_to_x(25_000), g.lane_y(ClipKind::Video, 0, 0.6)),
        )],
        E::Nest => vec![(
            "nested item body",
            egui::pos2(g.tick_to_x(625_000), g.lane_y(ClipKind::Video, 0, 0.6)),
        )],
        E::Generator => vec![(
            "generator event body",
            egui::pos2(g.tick_to_x(625_000), g.lane_y(ClipKind::Video, 0, 0.6)),
        )],
        E::Marker => vec![("flag on the ruler", egui::pos2(g.tick_to_x(60_000), ruler_y))],
        // the top outline of the region on its track
        E::Area => {
            vec![("area outline", egui::pos2(g.tick_to_x(145_000), g.row_top(video_row) + 1.0))]
        }
        E::Morpher => vec![(
            "morph block body",
            egui::pos2(g.tick_to_x(m.start.0 + m.duration.0 / 2), g.row_center_y(morph_row)),
        )],
        E::Camera => vec![(
            "camera block body",
            egui::pos2(g.tick_to_x(c.start.0 + c.duration.0 / 2), g.row_center_y(cam_row)),
        )],
        E::ClipLane => vec![
            ("row header", egui::pos2(gutter_x, g.row_center_y(video_row))),
            ("free space ON the track", egui::pos2(free_x, g.row_center_y(video_row))),
        ],
        E::MorphLane => vec![
            ("row header", egui::pos2(gutter_x, g.row_center_y(morph_row))),
            ("free space ON the track", egui::pos2(free_x, g.row_center_y(morph_row))),
        ],
        E::CameraLane => vec![
            ("row header", egui::pos2(gutter_x, g.row_center_y(cam_row))),
            ("free space ON the track", egui::pos2(free_x, g.row_center_y(cam_row))),
        ],
        E::Region => vec![("region edge", egui::pos2(g.tick_to_x(300_000), ruler_y))],
        // the transition window, away from the joint: the handles are there, and this is about the GESTURE
        E::Transition => {
            vec![("transition window", egui::pos2(g.tick_to_x(36_000), g.row_center_y(video_row)))]
        }
    };
    (h, pos)
}

/// A right-click WITHOUT pointer movement: the menu target and its items.
fn right_click(h: &mut egui_kittest::Harness<'_, crate::app::KtApp>, pos: egui::Pos2) {
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
}

/// Test 253 (R.38): every timeline entity has a full set of verbs.
///
/// The complaints, verbatim: “I do not understand how to delete a saved area region, I saved it
/// and now it hangs there, I do not understand how to rename it — right-click
/// gives nothing”, “I cannot add a morpher event to the morpher
/// track in any way”, “I do not understand how the camera track works.”
///
/// The test proves TWO things that the unit tests of the model do not prove:
/// the entity exists for the hit-test (it can be pointed at at all), and its menu
/// offers the mandatory operations. Plus, as a bonus — the hint above it does not
/// lie “empty space”.
#[test]
fn t253_every_entity_has_its_verbs() {
    use crate::timeline::TlEntity as E;
    use egui_kittest::kittest::Queryable;

    let mut problems: Vec<String> = Vec::new();
    for e in E::ALL {
        // its OWN harness for each PLACE: Esc in this program clears both
        // the region and the selection, so closing one entity’s menu silently
        // broke the scene of the next one (the region vanished — and “the region’s boundary
        // does not exist for the hit-test” was an artifact of the test, not a defect)
        let places = scene(e).1.len();
        for place_i in 0..places {
            let (mut h, all) = scene(e);
            let (place, pos) = all[place_i];

            // how many times each needed caption is visible BEFORE the menu (the same label
            // may live in the toolbar — we count the increment, not the presence)
            let want = verbs(e);
            let before: Vec<usize> = want
                .iter()
                .map(|(_, r)| match r {
                    Reach::Menu(l) => h.query_all_by_label_contains(l.as_str()).count(),
                    _ => 0,
                })
                .collect();

            right_click(&mut h, pos);

            let got = h.state().ctx_target.and_then(|t| t.entity());
            if got != Some(e) {
                problems.push(format!(
                    "{e:?} / {place}: right click gave the target {got:?} — from here the entity does \
                 not exist. Drawing is not enough: one needs a CtxTarget variant and a hit test FROM \
                 EVERY place the person acts from, not only from where \
                 the menu item was put"
                ));
                // the menu opened anyway (with an empty target) — close it, otherwise the
                // next right-click would CLOSE it instead of opening its own, and the other
                // entities would “lose” all their items at once
                continue;
            }

            for ((verb, reach), n0) in want.iter().zip(before) {
                match reach {
                    Reach::Menu(label) => {
                        // “by occurrence”: the track’s delete caption carries a counter
                        // (“Delete track (2 events)”), and an exact comparison
                        // would require repeating the formatting in the test
                        if h.query_all_by_label_contains(label.as_str()).count() <= n0 {
                            problems.push(format!(
                                "{e:?} / {verb:?} / {place}: the item “{label}” is not in the menu. \
                             It can be created, but not controlled — that is a dead end"
                            ));
                        }
                    }
                    Reach::Absent(why) | Reach::Elsewhere(why) => {
                        assert!(
                            why.trim().chars().count() >= 25,
                            "{e:?} / {verb:?}: the explanation “{why}” is shorter than 25 characters. \
                         “No verb” and “it is somewhere else” are accepted only with a \
                         reason in a sentence, otherwise the table turns into an excuse"
                        );
                    }
                }
            }

            // and the hint above the entity does not lie “empty space”
            h.hover_at(pos);
            h.step();
            h.step();
            let hint = h.state().status_line_shown.clone();
            if hint.0 == crate::i18n::tr(crate::i18n::k::tl::hint::EMPTY) {
                problems.push(format!(
                    "{e:?} / {place}: the pointer is ON the entity, but the status line says \
                 “empty place” — the hint lies (rule 10 of the checklist)"
                ));
            }
        }
    }

    assert!(problems.is_empty(), "dead ends in the timeline:\n  {}", problems.join("\n  "));
}

/// Test 254 (R.38 item 6): a click is a click.
///
/// The complaint, verbatim: “in the fx-chain widgets it is hard to activate
/// the parameter input”. The cause is not the button’s size: `ui.dnd_drag_source`
/// in egui 0.36 registers OVER the content a rectangle with
/// `Sense::drag()` and does this AFTER it, so `hit_test` returns
/// `click: None` — `.clicked()` inside never fires AT ALL. The user
/// was looking for an “active pixel” on the frame-rect strip above the label.
///
/// The test presses and releases AT THE CENTRE of the element WITHOUT pointer movement
/// (`Node::click()` does exactly this) and requires the consequence.
#[test]
fn t254_click_is_a_click() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    let mut h = crate::layout_tests::harness();
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
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8), FxInstance::Mirror { horizontal: true, vertical: false }],
        fx_bypass: vec![false, false],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // we target a slot NOT currently selected, whatever the window’s default is
    let sel_of = |h: &egui_kittest::Harness<'_, crate::app::KtApp>| {
        h.state().fx_editors[0].selected.get(&ClipId(1)).and_then(|s| s.fx()).unwrap_or(0)
    };
    let was = sel_of(&h);
    let want = 1 - was;
    let names = [
        crate::fx_editor::fx_display_name(&FxInstance::blur(8)),
        crate::fx_editor::fx_display_name(&FxInstance::Mirror {
            horizontal: true,
            vertical: false,
        }),
    ];
    let second = names[want].to_string();
    assert!(h.query_by_label(&second).is_some(), "slot “{second}” must be visible in the strip");

    // HERE IT IS, a person’s click: aim, press, release without moving the mouse
    h.get_by_label(&second).click();
    h.step();
    h.step();

    assert_eq!(
        sel_of(&h),
        want,
        "a click on slot “{second}” did not select it. The slot is wrapped in ui.dnd_drag_source — \
         in egui 0.36 it places a Sense::drag() rectangle AFTER and ON TOP of the label, and \
         hit_test returns click:None: the click is physically unreachable. The idiom of this \
         repository is one interaction with Sense::click_and_drag() and parsing \
         drag_started()/clicked() (bin_panel.rs, automation.rs, meters.rs, timeline.rs)"
    );

    // a click must not also reorder the slots: the gesture “pressed and released
    // in place” is a selection, not a drag
    let fx = h.state().model.project().clips[0].fx.clone();
    assert_eq!(fx.len(), 2, "click does not change the chain’s composition");
    assert!(matches!(fx[0], FxInstance::Blur { .. }), "and does not reorder the slots");

    // the slot’s cross is an ordinary button, but it stands IN THE SAME strip
    let crosses = h.get_all_by_label(crate::icons::CLEAR).count();
    assert!(crosses >= 2, "each slot must have a cross, found {crosses}");
}

/// Test 547 (R.54, item 431): the picker gallery reaches the MODEL.
///
/// It asserts the consequence the person sees: they opened the picker, pressed
/// the “Tight” cell of Blur — and the event carries Blur with EXACTLY its
/// parameters, not the template ones. It is probed from where the person acts:
/// a click on the cell, found by its caption.
///
/// Also, the popup’s new behaviour is fixed: egui menus default to
/// `CloseOnClick` — the popup closes on ANY click inside, and a gallery with
/// clickable thumbnails would be stillborn. The test presses into the search row
/// and requires that the popup REMAINS open.
#[test]
fn t547_gallery_click_reaches_the_model() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let mut h = crate::layout_tests::harness();
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
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // 1) open the picker — the same “+ effect…” button as before R.54
    let add = crate::i18n::tr(crate::i18n::k::fxreg::ADD).to_string();
    assert!(h.query_by_label(&add).is_some(), "picker button is missing");
    h.get_by_label(&add).click();
    h.step();
    h.step();

    // 2) the gallery is visible: the effect’s name and the captions of its variants
    assert!(h.query_by_label("Blur").is_some(), "effect row was not drawn");
    let heavy = {
        let e = crate::fx_registry::registry()
            .iter()
            .find(|e| e.name == "Blur")
            .expect("Blur in the registry");
        let v = &e.variants[1];
        // the accessible name of the cell is compound: the caption “Tight” exists in
        // six kinds, and they can be told apart only with the effect’s name
        (format!("{} {}", e.name, crate::i18n::tr(v.name)), v.fx.clone())
    };
    // before the search the Blur cell lies BELOW THE FOLD of the scroll, and it is
    // deliberately absent there: an invisible cell is neither drawn nor counted
    // (otherwise one opening of the gallery would order a hundred and fifty thumbnails —
    // found by the live scenario fxpicker)
    assert!(
        h.query_by_label(&heavy.0).is_none(),
        "cell “{}” is drawn below the fold: the visibility guard does not work",
        heavy.0
    );

    // 3) we TYPE in the search, as a person does: the gallery of 27 rows does not fit
    // in the scroll entirely, and the needed row may be BELOW the fold — exactly
    // why the picker has a search. The search row gets focus by itself
    // when the menu opens (this is what is fixed here).
    h.event(egui::Event::Text("blur".into()));
    h.step();
    h.step();
    assert!(
        h.query_by_label(&heavy.0).is_some(),
        "after a search the cell “{}” must be visible: input INSIDE the popup \
         does not close it (egui menus default to CloseOnClick, the gallery \
         needs MenuConfig::close_behavior(CloseOnClickOutside))",
        heavy.0
    );

    // 4) click the cell — and the model gets EXACTLY these parameters
    h.get_by_label(&heavy.0).click();
    h.step();
    h.step();
    let fx = h.state().model.project().clips[0].fx.clone();
    assert_eq!(fx.len(), 1, "click on a variant cell did not add the effect");
    assert_eq!(
        format!("{:?}", fx[0]),
        format!("{:?}", heavy.1),
        "something other than the variant was added (a template?)"
    );
    // the popup closed by a CHOICE (an explicit ui.close), rather than staying hanging
    assert!(h.query_by_label(&heavy.0).is_none(), "after the choice the popup stayed open");
}

/// A verb is an operation without which an entity becomes a dead end: you create it and
/// live with it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Verb {
    Create,
    Rename,
    Delete,
    /// Open the CONTENTS or the properties: the FX window for an event, the editor for a
    /// morpher, the view window for a transition, entering the inside of a nested reel (R.50).
    ///
    /// It was introduced because without it “enter the reel” had nowhere to be recorded,
    /// and entering is the only thing for which a nested reel differs from a clip.
    Open,
}

/// How the verb is REACHABLE.
enum Reach {
    /// The context-menu item of the entity with such a caption.
    Menu(String),
    /// The entity has no verb BY ITS NATURE. The reason is mandatory and
    /// is checked for length: a bare “no” without explanation is not accepted.
    Absent(&'static str),
    /// It is reachable not by the menu but by a gesture or a window — and this is proven by the NAMED
    /// test. A reference is mandatory: without it “somewhere over there” turns into
    /// an excuse, just like a prose checklist.
    Elsewhere(&'static str),
}

/// The ONLY table “entity → verbs”. The `match` is exhaustive:
/// a new timeline entity will not compile until its row is filled in.
fn verbs(e: crate::timeline::TlEntity) -> Vec<(Verb, Reach)> {
    use crate::i18n::{k, tr};
    use crate::timeline::TlEntity as E;
    let menu = |s: &str| Reach::Menu(s.to_string());
    match e {
        E::Clip => vec![
            (Verb::Create, Reach::Elsewhere("media bin, t99; insertion, t205")),
            (Verb::Rename, Reach::Absent("the event has no name — the label is the media name")),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (Verb::Open, menu(tr(k::tl::menu::FX))),
        ],
        E::Generator => vec![
            (
                Verb::Create,
                Reach::Elsewhere(
                    "“Generator” submenu — in the MAIN menu and in the bin background menu, one \
                     body for both entries; it places both the media and the clip under the cursor in one \
                     history step, t512/t560",
                ),
            ),
            (
                Verb::Rename,
                Reach::Absent(
                    "the generator has no name: the label is its KIND, and the kind is changed \
                     by the picker in the parameters window",
                ),
            ),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (Verb::Open, menu(tr(k::tl::menu::GEN_PARAMS))),
        ],
        E::Nest => vec![
            (Verb::Create, menu(tr(k::tl::menu::COLLAPSE))),
            (Verb::Rename, menu(tr(k::tl::menu::NEST_RENAME))),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (Verb::Open, menu(tr(k::tl::menu::NEST_ENTER))),
        ],
        E::Marker => vec![
            (
                Verb::Create,
                Reach::Elsewhere("M gesture and the MARKER_HERE item in the empty-place menu, t95/t170"),
            ),
            (Verb::Rename, menu(tr(k::tl::menu::RENAME))),
            (Verb::Delete, menu(tr(k::tl::menu::REMOVE_MARKER))),
            (
                Verb::Open,
                Reach::Elsewhere(
                    "double click on the range-marker strip makes a region from it, t206",
                ),
            ),
        ],
        E::Transition => vec![
            (
                Verb::Create,
                Reach::Elsewhere(
                    "event menu: TRANSITION_NEXT/TRANSITION_PREV with the neighbor, \
                     FADE_IN/FADE_OUT at a free edge; the corner gesture, t173",
                ),
            ),
            (Verb::Rename, Reach::Absent("the transition is unnamed: it sits between events, its “name” is its kind")),
            (Verb::Delete, menu(tr(k::tl::menu::REMOVE_TRANSITION))),
            (Verb::Open, menu(tr(k::tl::menu::TRANSITION_PROPS))),
        ],
        E::Area => vec![
            (Verb::Create, Reach::Elsewhere("SAVE_AREA item of the empty-place menu")),
            (Verb::Rename, menu(tr(k::tl::menu::RENAME))),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (
                Verb::Open,
                Reach::Absent("the area has no content: it is a name, a time and a set of tracks"),
            ),
        ],
        E::Morpher => vec![
            (
                Verb::Create,
                Reach::Elsewhere(
                    "ADD_MORPHER item in the empty-place menu; on the track — \
                                  the Create verb of MorphLane itself",
                ),
            ),
            (Verb::Rename, Reach::Absent("the morpher is unnamed: it is a layer of edits, not an object")),
            (Verb::Delete, menu(tr(k::tl::menu::REMOVE_MORPHER))),
            (Verb::Open, menu(tr(k::tl::menu::MORPH_EDITOR))),
        ],
        E::Camera => vec![
            (Verb::Create, Reach::Elsewhere("ADD_CAMERA_HERE item in the camera track menu")),
            (Verb::Rename, Reach::Elsewhere("name field in the camera window, t231b")),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (Verb::Open, menu(tr(k::cam::PROPS))),
        ],
        // The Create track — “put your own element here”: an event can only go into the bin,
        // while a morpher and a camera are placed FROM THE TRACK MENU.
        // The asymmetry of these three rows was the complaint of item 2 of R.38.
        E::ClipLane => vec![
            (Verb::Create, Reach::Elsewhere("the media bin places events by dragging, t99")),
            (Verb::Rename, menu(tr(k::tl::lane::RENAME))),
            (Verb::Delete, menu(tr(k::tl::lane::DELETE))),
            (Verb::Open, menu(tr(k::track::MENU_PROPS))),
        ],
        E::MorphLane => vec![
            (Verb::Create, menu(tr(k::tl::lane::ADD_MORPHER_HERE))),
            (Verb::Rename, menu(tr(k::tl::lane::RENAME))),
            (Verb::Delete, menu(tr(k::tl::lane::DELETE))),
            (Verb::Open, menu(tr(k::track::MENU_PROPS))),
        ],
        E::CameraLane => vec![
            (Verb::Create, menu(tr(k::tl::lane::ADD_CAMERA_HERE))),
            (Verb::Rename, menu(tr(k::tl::lane::RENAME))),
            (Verb::Delete, menu(tr(k::tl::lane::DELETE))),
            (Verb::Open, menu(tr(k::track::MENU_PROPS))),
        ],
        E::Region => vec![
            (Verb::Create, Reach::Elsewhere("drag on the ruler from an empty place, t96")),
            (Verb::Rename, Reach::Absent("region is unnamed; a named one is an area")),
            (Verb::Delete, menu(tr(k::tl::menu::REGION_CLEAR))),
            (
                Verb::Open,
                Reach::Absent("a region is two edges on the ruler, it has no content"),
            ),
        ],
    }
}

/// The text drawn in one frame: what was asked to be drawn, what was actually drawn,
/// how many rows it took.
///
/// One must look at the GALLERY itself, not at accesskit: a button’s accessibility label
/// is taken from the source text and never contains an ellipsis, so “truncated or wrapped”
/// cannot be told apart by it.
#[derive(Clone)]
pub(crate) struct PaintedText {
    pub src: String,
    pub drawn: String,
    pub rows: usize,
    pub elided: bool,
    pub rect: egui::Rect,
}

fn collect_texts(shape: &egui::epaint::Shape, out: &mut Vec<PaintedText>) {
    use egui::epaint::Shape;
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
        Shape::Text(t) => {
            let g = &t.galley;
            out.push(PaintedText {
                src: g.text().to_string(),
                drawn: g.rows.iter().map(|r| r.row.text()).collect::<Vec<_>>().join(""),
                rows: g.rows.len(),
                elided: g.elided,
                rect: g.rect.translate(t.pos.to_vec2()),
            });
        }
        _ => {}
    }
}

/// Drawn LINES (R.67): polylines and segments from the frame’s list of shapes —
/// what the person sees on the graph; claims about the curve’s drawing
/// are made from them, not from the model (lesson of t698).
pub(crate) fn painted_lines(h: &egui_kittest::Harness<'_, crate::app::KtApp>) -> Vec<Vec<egui::Pos2>> {
    fn walk(shape: &egui::epaint::Shape, out: &mut Vec<Vec<egui::Pos2>>) {
        use egui::epaint::Shape;
        match shape {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::LineSegment { points, .. } => out.push(points.to_vec()),
            Shape::Path(p) => out.push(p.points.clone()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in &h.output().shapes {
        walk(&cs.shape, &mut out);
    }
    out
}

pub(crate) fn painted_texts(h: &egui_kittest::Harness<'_, crate::app::KtApp>) -> Vec<PaintedText> {
    let mut out = Vec::new();
    for cs in &h.output().shapes {
        collect_texts(&cs.shape, &mut out);
    }
    out
}

/// The picker gallery on a clip of the given kind: open the FX window, expand
/// the picker and (optionally) filter by search — a gallery of 27 rows does not fit
/// in the scroll entirely, and the needed row may be BELOW the fold.
fn picker_harness(
    kind: kt_model::ClipKind,
    query: Option<&str>,
) -> egui_kittest::Harness<'static, crate::app::KtApp> {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let mut h = crate::layout_tests::harness();
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
        kind,
        lane: 0,
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
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    let add = crate::i18n::tr(crate::i18n::k::fxreg::ADD).to_string();
    h.get_by_label(&add).click();
    h.step();
    h.step();
    if let Some(q) = query {
        h.event(egui::Event::Text(q.into()));
        h.step();
        h.step();
    }
    h
}

/// Test 551 (R.54, item 433): the STRENGTH AXIS of a cell.
///
/// It asserts what the person does: pressed on a cell, moved sideways,
/// released — and the event carries NOT the named variant, but what they
/// saw under the cursor. And the converse, no less important: pressed and released without
/// moving — exactly the named variant stands (rule t254, otherwise “Tight”
/// would stop being reachable).
#[test]
fn t551_strength_axis_lands_between_variants() {
    use egui_kittest::kittest::Queryable;

    // the language is set EXPLICITLY and BEFORE the names are read: `tr` lives in the thread’s TLS, and the harness
    // sets the language only inside itself — the caption would be computed in English
    // while a Russian one would be looked up
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let e =
        crate::fx_registry::registry().iter().find(|e| e.name == "Blur").expect("Blur in the registry");
    let soft = e.variants[0].fx.get_param("radius").unwrap();
    let heavy = e.variants[1].fx.get_param("radius").unwrap();
    let label_heavy = format!("{} {}", e.name, crate::i18n::tr(e.variants[1].name));

    // --- (a) a click without movement gives EXACTLY the variant -------------------
    let mut h = picker_harness(kt_model::ClipKind::Video, Some("blur"));
    h.get_by_label(&label_heavy).click();
    h.step();
    h.step();
    let fx = h.state().model.project().clips[0].fx.clone();
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].get_param("radius"), Some(heavy), "click stopped giving the variant itself");

    // --- (b) a drag to the LEFT gives a mix with the neighbour on the left ---------------
    // the neighbouring point of the row to the left of “Tight” is “Soft”; we drag by half a
    // cell, that is, roughly midway between them
    let mut h = picker_harness(kt_model::ClipKind::Video, Some("blur"));
    let cell = h.get_by_label(&label_heavy).rect();
    let c = cell.center();
    h.drag_at(c);
    h.step();
    // egui’s drag threshold: first a small shift, then to the target (the same three-step
    // idiom as in all drag tests of the repository)
    h.hover_at(egui::pos2(c.x - 8.0, c.y));
    h.step();
    let target = egui::pos2(c.x - crate::fx_registry::CELL_DRAW_W / 2.0, c.y);
    h.hover_at(target);
    h.step();
    h.drop_at(target);
    h.step();
    h.step();

    let fx = h.state().model.project().clips[0].fx.clone();
    assert_eq!(fx.len(), 1, "wiring did not add the effect");
    let got = fx[0].get_param("radius").expect("radius");
    assert!(
        got > soft && got < heavy,
        "axis did not work: radius {got} must lie BETWEEN {soft} and {heavy}"
    );
    // the axis is quantized, not “as many pixels — as many values”
    let (j, tt) = crate::fx_registry::axis_at(1, -crate::fx_registry::CELL_DRAW_W / 2.0, 4);
    assert_eq!((j, tt), (1, 0.5), "midpoint between neighbors is computed differently");
}

/// Pure arithmetic of the axis: the edges stop, the centre of a cell is the variant itself.
#[test]
fn t551b_axis_math_clamps_and_snaps() {
    use crate::fx_registry::{CELL_DRAW_W, axis_at};
    // the row: [template, v0, v1, v2] — four points, cell i stands at i + 1
    let n = 4;
    // the centre of its own cell = the point itself (fraction 0 there)
    assert_eq!(axis_at(0, 0.0, n), (1, 0.0));
    assert_eq!(axis_at(1, 0.0, n), (2, 0.0));
    // a small hand tremor does not shift the value (axis quantization)
    assert_eq!(axis_at(1, 3.0, n), (2, 0.0));
    assert_eq!(axis_at(1, -3.0, n), (2, 0.0));
    // a cell to the left — the previous point of the row
    assert_eq!(axis_at(1, -CELL_DRAW_W, n), (1, 0.0));
    // you cannot go further left than the TEMPLATE: the row stops
    assert_eq!(axis_at(0, -10.0 * CELL_DRAW_W, n), (0, 0.0));
    // and further right than the last point too
    assert_eq!(axis_at(2, 10.0 * CELL_DRAW_W, n), (n - 2, 1.0));
    // a half-cell is exactly half of the fraction
    assert_eq!(axis_at(0, CELL_DRAW_W / 2.0, n), (1, 0.5));
}

/// Test 548 (R.54, item 431): the gallery layout.
///
/// Four assertions, and all are about what is VISIBLE: a cell’s caption is truncated
/// (not wrapped and not spilling out), the category heading is printed ONCE,
/// the gallery’s width does not depend on what is typed in the search, and in the audio picker
/// there are no cells at all.
#[test]
fn t548_gallery_layout_is_stable_and_elided() {
    use egui_kittest::kittest::Queryable;

    // ---- (a) the cell caption is truncated, not spreading it ---------------
    let h = picker_harness(kt_model::ClipKind::Video, Some("aberration"));
    let texts = painted_texts(&h);
    let cells: Vec<&PaintedText> = texts
        .iter()
        .filter(|t| {
            crate::fx_registry::registry()
                .iter()
                .flat_map(|e| e.variants.iter())
                .any(|v| crate::i18n::tr(v.name) == t.src)
        })
        .collect();
    assert!(!cells.is_empty(), "cell labels are not drawn at all");
    for c in &cells {
        assert_eq!(c.rows, 1, "label “{}” wrapped onto {} lines", c.src, c.rows);
        assert!(
            c.rect.width() <= crate::fx_registry::CELL_DRAW_W + 1.0,
            "label “{}” is wider than the cell: {} px",
            c.src,
            c.rect.width()
        );
    }

    // ---- (b) the category heading — ONCE --------------------------
    // The registry is sorted by meaning, and categories repeat in it:
    // “Stylization” was printed five times, “Blur” three times. Grouping at
    // drawing must collapse this.
    let h2 = picker_harness(kt_model::ClipKind::Video, None);
    for cat in [
        crate::i18n::k::fxreg::cat::STYLE,
        crate::i18n::k::fxreg::cat::BLUR,
        crate::i18n::k::fxreg::cat::THREE_D,
        crate::i18n::k::fxreg::cat::GEOMETRY,
    ] {
        let name = crate::i18n::tr(cat);
        let n = painted_texts(&h2).iter().filter(|t| t.src == name).count();
        assert!(n <= 1, "category heading “{name}” was printed {n} times");
    }

    // ---- (c) the gallery content neither spills out nor jumps --------------
    // “Nothing jumps” (rule 3) and “the text fits” (rule 7):
    // the row of cells must WRAP inside the declared width, and
    // what is typed in the search has no right to move the gallery.
    let content_box = |h: &egui_kittest::Harness<'_, crate::app::KtApp>| -> egui::Rect {
        let mut r = egui::Rect::NOTHING;
        for e in crate::fx_registry::registry().iter().filter(|e| e.video) {
            if let Some(n) = h.query_by_label(e.name) {
                r = r.union(n.rect());
            }
            for v in &e.variants {
                let l = format!("{} {}", e.name, crate::i18n::tr(v.name));
                if let Some(n) = h.query_by_label(&l) {
                    r = r.union(n.rect());
                }
            }
        }
        r
    };
    let wide = content_box(&h2);
    let narrow = content_box(&h);
    assert!(
        wide.width() <= crate::fx_registry::GALLERY_W,
        "gallery content is wider than the declared width: {} px versus {}",
        wide.width(),
        crate::fx_registry::GALLERY_W
    );
    assert!(
        (wide.left() - narrow.left()).abs() <= 1.0,
        "gallery drifted from the request: left edge {} versus {}",
        wide.left(),
        narrow.left()
    );

    // ---- (d) the audio picker has no pictures ----------------------------
    let ha = picker_harness(kt_model::ClipKind::Audio, None);
    assert!(ha.query_by_label("Gain").is_some(), "audio list was not drawn");
    for l in crate::fx_registry::registry()
        .iter()
        .flat_map(|e| e.variants.iter().map(|v| format!("{} {}", e.name, crate::i18n::tr(v.name))))
    {
        assert!(
            ha.query_by_label(&l).is_none(),
            "audio picker contains cell “{l}”: the sound thumbnail cannot be drawn"
        );
    }
}

/// Test 255 (R.38 items 7–8): a long name is truncated, not spreading the strip.
///
/// The complaints, verbatim: “if in a Source crumb you specify a source with a long name
/// — the whole chain flies apart along the length” and “also if you pile up many
/// crumbs — the last label runs into the edge”.
///
/// The chain strip lives in `horizontal_wrapped`, which sets
/// `TextWrapMode::Wrap`: the label wraps INSIDE ITSELF, and since the
/// file name has no spaces — letter by letter. That is what “flies apart” is.
/// Rule 7 of the checklist requires the opposite: measure and truncate.
/// The morpher chain strip with a source named `file`: the vertical
/// spread of its labels and the name labels themselves.
fn strip_metrics(file: &str) -> (f32, Vec<PaintedText>) {
    use kt_model::*;

    let mut h = crate::layout_tests::harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), format!("/tmp/{file}")));
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
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.add_morph_layer();
    h.step();
    h.step();

    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.patch.media = Some(MediaId(2));
    ev.patch.video_fx = vec![
        FxInstance::blur(8),
        FxInstance::Mirror { horizontal: true, vertical: false },
        FxInstance::edges(64),
    ];
    assert!(app.apply(Command::SetMorpher(ev)));
    h.step();
    h.step();

    let texts = painted_texts(&h);
    let want_src = crate::tf!(crate::i18n::k::mrph::SRC_SLOT, file.to_string());
    let names: Vec<&PaintedText> = texts
        .iter()
        .filter(|t| {
            t.src == want_src
                || t.src == crate::fx_editor::fx_display_name(&FxInstance::blur(8))
                || t.src == crate::fx_editor::fx_display_name(&FxInstance::edges(64))
        })
        .collect();
    assert!(names.len() >= 3, "the strip must have the head and the effect slots: {}", names.len());
    let top = names.iter().fold(f32::MAX, |a, t| a.min(t.rect.top()));
    let bot = names.iter().fold(f32::MIN, |a, t| a.max(t.rect.bottom()));
    let heads =
        texts.iter().filter(|t| t.src == want_src).map(|t| PaintedText { ..t.clone() }).collect();
    (bot - top, heads)
}

/// Test 255 (R.38 items 7–8): a long name is truncated, not spreading the strip.
///
/// The complaints, verbatim: “if in a Source crumb you specify a source with a long name
/// — the whole chain flies apart along the length” and “also if you pile up many
/// crumbs — the last label runs into the edge”.
///
/// The strip lives in `horizontal_wrapped`, which sets `TextWrapMode::Wrap`:
/// the label wrapped INSIDE ITSELF, and since the file name has no
/// spaces — letter by letter. Rule 7 of the checklist requires the opposite: measure and
/// truncate. Moving a crumb WHOLE onto the next row is legitimate at this point —
/// it is illegal to stretch the strip vertically with a name.
#[test]
fn t255_long_name_is_elided_not_wrapped() {
    let long = format!("{}.mov", "VeryLongSourceName".repeat(8));
    let (h_short, _) = strip_metrics("clip.mov");
    let (h_long, heads) = strip_metrics(&long);

    assert!(!heads.is_empty(), "label with the source name must be drawn");
    // BOTH places where the name is drawn (the source field and the strip head)
    for t in &heads {
        assert_eq!(
            t.rows, 1,
            "source name wrapped onto {} line(s) — it must be TRUNCATED, not \
             broken letter by letter (rule 7 of the checklist: measure and decide, \
             do not hope that the name is short)",
            t.rows
        );
        assert!(
            t.elided && t.drawn.ends_with('…'),
            "name is drawn in full, without truncation or ellipsis: {:?}",
            t.drawn
        );
    }

    assert!(
        h_long <= h_short + 1.0,
        "chain strip grew from {h_short:.0} px to {h_long:.0} px only because the source \
         has a long name — “the whole chain flies apart by length”. The name must be \
         truncated by width, not stretch the strip"
    );
}

/// Test 254b: `dnd_drag_source` is forbidden throughout the application.
///
/// One line closes a CLASS, not a single widget: any future
/// `dnd_drag_source` around a clickable element will again silently kill the click.
#[test]
fn t254b_no_dnd_drag_source_over_clickables() {
    // no comments: the explanation of WHY it is no longer written this way itself
    // contains this word — and the test would catch its own documentation
    let bad: Vec<String> = crate::font_tests::sources()
        .into_iter()
        .filter(|(_, s)| strip_comments(s).contains("dnd_drag_source"))
        .map(|(f, _)| f)
        .collect();
    assert!(
        bad.is_empty(),
        "{bad:?}: ui.dnd_drag_source places a Sense::drag() rectangle ON TOP of its \
         content (egui 0.36) and kills the click on it — hit_test returns click:None. \
         The repository idiom: one interaction with Sense::click_and_drag(), \
         drag_started() starts the drag, clicked() selects"
    );
}

/// Test 273 (R.39): arithmetic over `source_in` outside ONE rule does not exist.
///
/// The trigger: the comment above `clip_slice` promised “the rule of slices is ONE for the whole
/// project”, while the linear formula had been written out by hand in six more places —
/// so the repair of `Split` fixed neither the trims, nor punch-out, nor the crop
/// of a copied block. The same trick as t252 for orphaned commands:
/// a promise that cannot be checked stops being kept.
#[test]
fn t273_source_in_arithmetic_lives_in_one_place() {
    let root = workspace_root();
    // where the rule is and its place: the function itself and its helpers
    const HOME: &str = "libs/kt-model/src/project.rs";
    let mut bad: Vec<String> = Vec::new();
    for rel in ["libs/kt-model/src/model.rs", "libs/kt-model/src/ops.rs", HOME] {
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        for (n, line) in src.lines().enumerate() {
            // assigning source_in from an expression involving itself —
            // that is exactly “a slice’s own formula”.
            let t = line.trim();
            let assigns = t.contains("source_in =") || t.contains("source_in +=");
            if !assigns {
                continue;
            }
            // in the rule’s home such lines are legitimate
            if rel == HOME {
                continue;
            }
            bad.push(format!("{rel}:{}: {t}", n + 1));
        }
    }
    assert!(
        bad.is_empty(),
        "the material entry point of a slice is counted OUTSIDE the single rule \
         (`clip_slice`/`slice_source_in` in {HOME}). On a stretched clip its own formula \
         drifts, and fixing one gesture does not fix the neighbors — exactly how Split, trims, \
         punch-out and crop drifted apart before R.39:\n  {}",
        bad.join("\n  ")
    );
}

/// Test 426b (R.50): the search for a source FROM A CLIP lives in one home.
///
/// The reason — the price we knowingly pay for the chosen carrier of the reference.
/// The address of a nested reel lies in ONE space with the `MediaId` of files, but
/// a reel has no `MediaRef`: its body lives in `Project.sequences`. So code
/// asking `project.media` about `clip.media` gets `None` for a nested reel —
/// and that is not a refusal but SILENT wrong work: the watchdog would tint the
/// nested reel with the offline tint, `IndexService` would try to build peaks for it with
/// ffmpeg, and the command would refuse with “media not found”.
///
/// Exactly one thing is allowed: `Project::source()`. Iterating over `project.media` on its own
/// is legitimate (the bin, the paths dialog, assembling the path map — they really are about
/// FILES); comparing an element of that iteration with the clip’s `media` field is forbidden.
#[test]
fn t426b_media_lookup_from_a_clip_lives_in_one_place() {
    let root = workspace_root();
    // the rule’s home — `Project::source()` itself and its neighbours in the same file
    const HOME: &str = "libs/kt-model/src/project.rs";
    let mut bad: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    for dir in ["libs", "apps"] {
        collect_rs(&root.join(dir), &mut files);
    }
    for rel in files {
        if rel == HOME || rel.contains("/tests/") || rel.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(&rel)).expect(&rel));
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            if !t.contains("media.iter()") {
                continue;
            }
            // “compared with the `media` field of some event” — that is exactly
            // the search FROM A CLIP.
            let from_clip = t.contains(".media)")
                || t.contains(".media {")
                || t.contains("== c.media")
                || t.contains("== clip.media");
            if from_clip {
                bad.push(format!("{rel}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "the event’s source is looked up in `project.media` bypassing the single home          (`Project::source()` in {HOME}). A nested sequence’s `MediaRef` has none:          such a lookup returns None and silently turns the nested item into “offline media” —          not one refusal, not one error, just wrong work:\n  {}",
        bad.join("\n  ")
    );
}

/// Test 456b (R.50, B1): the DOCUMENT goes outward, not the working view.
///
/// Standing inside a reel, `model.project()` gives ITS contents in place of
/// the root — the whole interface, which knows nothing about nested reels, rests on this.
/// But the player, the file and the export need the document: to hand them the working view
/// would mean writing to disk a project whose root and reel contents had swapped
/// places. A person would notice this only at the next opening —
/// exactly the class of silent loss of work because of which R.49 was created.
#[test]
fn t456b_only_the_document_leaves_the_model() {
    let root = workspace_root();
    let mut bad: Vec<String> = Vec::new();
    for (file, src) in production_corpus() {
        if !file.contains("kt-app") && !file.ends_with(".rs") {
            continue;
        }
        let _ = &root;
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            // in one line: “hand outward” and “working view”
            let out = t.contains("set_project(")
                || t.contains("save_project(")
                || t.contains("render_raw_video")
                || t.contains("ExportBackends");
            if out && t.contains("model.project()") {
                bad.push(format!("{file}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "outward (to the player, to a file, to export) goes the WORKING VIEW instead of the document. \
         Standing inside a reel it keeps the reel’s content in the root’s place — writing that \
         would swap the root and the reel on disk. The fix is `model.document()` \
         (and for offline counting by what is at hand — `model.reel_document()`):\n  {}",
        bad.join("\n  ")
    );
}

/// Test 554 (R.55, C1): what the player plays is decided in ONE place.
///
/// t456b guards that the working view does not go outward. That is not enough: until
/// R.55 eight places each wrote `set_project(model.document())` on their own,
/// and “a reel plays inside the reel” would hold only until the first edit —
/// `apply_batch` would at once return the root to the player, and the defect would come back
/// unnoticed, because it leaves no visible trace (the sound is simply wrong).
///
/// Therefore direct forms are forbidden altogether: the source of what plays is
/// `KtApp::player_project()`, and it is the only one.
#[test]
fn t554_only_one_place_decides_what_plays() {
    let mut bad: Vec<String> = Vec::new();
    for (file, src) in production_corpus() {
        if !file.contains("kt-app") {
            continue;
        }
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            let direct = t.contains("set_project(self.model.document())")
                || t.contains("set_project(self.model.reel_document())")
                || t.contains("set_project(self.model.project())");
            if direct {
                bad.push(format!("{file}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "the player receives the project bypassing `player_project()`. Standing inside a reel, \
         the REEL must be what plays, and that must be decided in one place — otherwise the first \
         edit will return the root to the player, and only the user will hear it:\n  {}",
        bad.join("\n  ")
    );
    // self-check of the scan: the decision point itself must exist and
    // be exactly one (otherwise the test is green on an empty set)
    let src = std::fs::read_to_string(workspace_root().join("apps/kt-app/src/app.rs"))
        .expect("no app.rs");
    assert_eq!(
        src.matches("fn player_project(").count(),
        1,
        "the decision point “what plays” has either vanished or multiplied — \
         the prohibition above has stopped meaning anything"
    );
}

/// Test 560 (R.55, C6): the generator is created NOT ONLY from the bin.
///
/// **The owner’s complaint, verbatim:** “I do not know how to create a generator
/// media, we have the code but how to create it in the UI is not clear.. probably a lot
/// more things are unavailable from the UI.”
///
/// And indeed: there was exactly one entrance — a right-click on the EMPTY background of the bin.
/// Neither the main menu, nor a key, nor the timeline menu. t253 did not catch this:
/// it asks whether the verb is REACHABLE, not how many entrances there are
/// and how visible they are.
///
/// The test asserts two things at once: the item exists in the main menu AND the list of kinds
/// at both entrances is ONE (rule 17) — no second list has come into being.
#[test]
fn t560_generator_is_reachable_from_the_main_menu() {
    let root = workspace_root();
    let toolbar =
        std::fs::read_to_string(root.join("apps/kt-app/src/toolbar.rs")).expect("no toolbar.rs");
    assert!(
        toolbar.contains("gen_ui::generator_menu(ui)"),
        "the main menu has no “Create generator…” item — the only entry \
         left is the right click on the empty bin background, which the person did not find"
    );
    let bin = std::fs::read_to_string(root.join("apps/kt-app/src/bin_panel.rs"))
        .expect("no bin_panel.rs");
    assert!(
        bin.contains("gen_ui::generator_menu(ui)"),
        "the bin menu builds its own list of kinds — if they diverged, a new kind would appear \
         in one entry and not in the other (rule 17)"
    );
    // The MENU builds the list of kinds in exactly one file. The entrances must not have
    // their own enumeration: a new kind would land in one menu and miss the
    // other (`GeneratorKind::all()` legitimately also lives in the generator window —
    // there it is the kind picker and width measurement, not a menu).
    for (name, src) in [("toolbar.rs", &toolbar), ("bin_panel.rs", &bin)] {
        assert!(
            !src.contains("GeneratorKind::all()"),
            "{name} enumerates the generator kinds on its own — the list of kinds must be \
             one for all entries (rule 17)"
        );
    }
}

/// A recursive walk of `.rs` files with paths relative to the workspace root.
fn collect_rs(dir: &std::path::Path, out: &mut Vec<String>) {
    let root = workspace_root();
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == "vendor") {
                continue;
            }
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs")
            && let Ok(rel) = p.strip_prefix(&root)
        {
            // one separator on every OS: the guards compare against `libs/…` paths
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

// ---- RIGHT-CLICK IN THE PANELS (R.49, wave B) ---------------------------------

/// The verb of a panel object — what the person wants to do with it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PanelVerb {
    /// The right button responds at all.
    Menu,
    /// At least one ACTION in this menu, not just the header.
    Act,
}

/// The ONLY table “panel entity → what the right button answers with”.
/// The `match` is exhaustive: a new panel entity will not compile until
/// its row is filled in.
fn panel_verbs(e: crate::ctx_menu::PanelEntity) -> Vec<(PanelVerb, &'static str)> {
    use crate::ctx_menu::PanelEntity as P;
    match e {
        P::FxSlot => vec![
            (PanelVerb::Menu, "right click on a chain slot opens its menu"),
            (PanelVerb::Act, "enable or disable the slot, remove the slot from the chain"),
        ],
        P::FxHead => vec![
            (PanelVerb::Menu, "right click on the pinned chain head opens the menu"),
            (PanelVerb::Act, "enable or disable the morpher layer source"),
        ],
        P::CurveKey => vec![
            (PanelVerb::Menu, "right click on a key of the automation curve opens the menu"),
            (
                PanelVerb::Act,
                "delete this key; the right button itself does NOT delete — otherwise the promise \
                 “you can experiment safely with the right button” would be devalued everywhere",
            ),
        ],
        P::Bin => vec![
            (PanelVerb::Menu, "right click on the empty background of the media bin opens the menu"),
            (
                PanelVerb::Act,
                "switch the layout: free or grid; create a generator (R.52) — \
                 the only entry for media without a file",
            ),
        ],
        P::Preview => vec![
            (PanelVerb::Menu, "right click on the preview window opens its menu"),
            (PanelVerb::Act, "detach the preview into its own window or return it back"),
        ],
        P::Meters => vec![
            (PanelVerb::Menu, "right click on the meter scales and the fader opens the menu"),
            (PanelVerb::Act, "mute the master, reset the fader to 0 dB, reset the peak"),
        ],
    }
}

/// Test 395 (R.49 B3/B6): EVERY panel entity’s right button answers with something,
/// and this is recorded in one place.
///
/// The panel registry is separate from `TlEntity` on purpose: timeline targets have a shared
/// scene and a shared kind of identifier (time and track), panels have neither of
/// these. Lumped into one enum, they would give a `match` half of whose branches
/// never occur in their own place.
#[test]
fn t395_every_panel_entity_answers_right_click() {
    use crate::ctx_menu::PanelEntity as P;
    assert_eq!(P::ALL.len(), P::N, "literal ALL drifted from N");
    for e in P::ALL {
        let verbs = panel_verbs(e);
        assert!(
            verbs.iter().any(|(v, _)| *v == PanelVerb::Menu),
            "{e:?}: it is not said whether the right button responds"
        );
        for (_, why) in &verbs {
            assert!(
                why.trim().chars().count() >= 20,
                "{e:?}: the explanation “{why}” is shorter than 20 characters — so the row \
                 gets filled with “just to get it through”"
            );
        }
    }
    // the kind of target — an exhaustive match, every target belongs to someone
    use crate::ctx_menu::PanelTarget as T;
    use kt_model::ClipId;
    let all: Vec<T> = vec![
        T::FxSlot { chain: ClipId(1), slot: 0 },
        T::FxHead { chain: ClipId(1) },
        T::CurveKey { chain: ClipId(1), slot: 0, curve: 0, key: 0 },
        T::BinEmpty,
        T::Preview,
        T::Meters,
    ];
    let mut seen: Vec<crate::ctx_menu::PanelEntity> = all.iter().map(|t| t.entity()).collect();
    seen.sort_by_key(|e| *e as usize);
    seen.dedup();
    assert_eq!(seen.len(), P::N, "not every panel entity has its own target: {seen:?}");
}

/// Test 396 (R.49 B6): an object that responds to HOVER must also
/// respond to the right button.
///
/// A scan of the catalog, not of the code: an object’s hint is a promise “I am a thing,
/// you can work with me”, and a person who learned the right button on the timeline
/// presses it everywhere. Silence reads as “there is no object” — exactly the class
/// of dead ends that cost rounds R.33–R.40. A relative of “catalog keys nobody uses”.
#[test]
fn t396_hinted_objects_have_a_right_click_answer() {
    // object hints: each one is paired with a panel entity
    // that responds to the right button.
    const ANSWERED: &[(&str, &str)] = &[
        ("fx.slot_hint", "FxSlot"),
        ("mrph.src_slot_hint", "FxHead"),
        ("bin.hint_free", "Bin"),
        ("bin.hint_grid", "Bin"),
        ("hint.preview_click", "Preview"),
        ("meter.mute", "Meters"),
        ("meter.fader", "Meters"),
        ("meter.peak", "Meters"),
        ("fx.key_at", "CurveKey"),
    ];
    let src = crate::font_tests::sources();
    let named: Vec<&str> = crate::ctx_menu::PanelEntity::ALL
        .iter()
        .map(|e| match e {
            crate::ctx_menu::PanelEntity::FxSlot => "FxSlot",
            crate::ctx_menu::PanelEntity::FxHead => "FxHead",
            crate::ctx_menu::PanelEntity::CurveKey => "CurveKey",
            crate::ctx_menu::PanelEntity::Bin => "Bin",
            crate::ctx_menu::PanelEntity::Preview => "Preview",
            crate::ctx_menu::PanelEntity::Meters => "Meters",
        })
        .collect();
    for (key, entity) in ANSWERED {
        assert!(
            named.contains(entity),
            "{key}: kind “{entity}” is named, which is absent from the panel registry"
        );
        // the hint key must really exist in the code: otherwise
        // the table guards something invented.
        let path = {
            let mut parts: Vec<String> = key.split('.').map(str::to_string).collect();
            if let Some(last) = parts.last_mut() {
                *last = last.to_uppercase();
            }
            parts.join("::")
        };
        assert!(
            src.iter().any(|(_, s)| s.contains(&path)),
            "{key}: no such hint in the code — the table guards something made up"
        );
    }
    // and the converse: every panel entity is covered by at least one hint
    for e in named {
        assert!(
            ANSWERED.iter().any(|(_, x)| *x == e),
            "{e}: the entity is in the registry, but the hint it closes is missing \
             from the table — either the object is mute for hover, or the row was forgotten"
        );
    }
}

/// Test 508 (R.53, item 414): EVERY parameter has a caption from the catalog.
///
/// `param_label` on an unknown name returns the RAW English name
/// of the field — and does so silently, in all five languages. This is how the windows of R.52
/// came to show `line_spacing`, `plate_pad_x`, `angle_deg`, and the title’s
/// font size was captioned “Grain:” (another effect’s key, Noise). Neither t100 nor t125
/// catches this: the key simply does not exist.
#[test]
fn t508_every_param_has_a_label() {
    use kt_model::HasParams;
    let mut bad: Vec<String> = Vec::new();
    let mut check = |who: &str, f: &dyn HasParams, covered: &[&str]| {
        let scope = f.params_scope();
        for spec in f.param_specs() {
            // the colour components have no captions and must not: they are covered by
            // the EYEDROPPER (rule 13 — three 0..255 sliders for one colour is a waste
            // of space). But ALL of them must be covered: an uncovered
            // Rgb8 parameter would spill into the grid under its raw name.
            if spec.unit == kt_model::ParamUnit::Rgb8 {
                assert!(
                    covered.contains(&&*spec.name),
                    "{who}.{}: a color component did not land in any eyedropper — \
                     the person will see the raw field name",
                    spec.name
                );
                continue;
            }
            let label = crate::widgets::param_label_scoped(scope, &spec.name);
            // a caption equal to the field name — that is exactly “no key”
            if label == spec.name {
                bad.push(format!("{who}.{}", spec.name));
            }
        }
        for spec in f.flag_specs() {
            if crate::widgets::flag_label(spec.name) == spec.name {
                bad.push(format!("{who}.{} (flag)", spec.name));
            }
        }
    };
    for k in kt_model::GeneratorKind::all() {
        let mut covered: Vec<&str> = crate::gen_ui::color_groups(&k)
            .iter()
            .flat_map(|(_, names)| names.iter().copied())
            .collect();
        // the fourth component (the alpha of shapes, R.53 item 419) is covered by the same
        // eyedropper — the window passes it to `color_row` as a separate parameter.
        covered.extend(crate::gen_ui::color_alpha(&k));
        check(k.tag(), &k, &covered);
    }
    // for transitions the colour is drawn by its own eyedropper (the transition window, R.33 C4)
    const TRANS_COLOR: &[&str] = &["color_r", "color_g", "color_b", "tint_r", "tint_g", "tint_b"];
    for k in kt_model::TransitionKind::ALL {
        check(k.tag(), k, TRANS_COLOR);
    }
    check("camera", &kt_model::CameraParams::default(), &[]);
    assert!(
        bad.is_empty(),
        "parameters without a label in the catalog — the person sees the raw field name:\n  {}",
        bad.join("\n  ")
    );
}

/// Test 509 (R.53, item 414): every FLAG has a control.
///
/// A flag declared in the passport but drawn by no window —
/// is exactly the “orphaned feature” that t252 guards for commands:
/// the 1 kHz tone of SMPTE existed in the format since R.52, but there was
/// nothing to switch it on with. We scan the kt-app corpus for calls of the shared widget `flags_row`.
#[test]
fn t509_every_flag_has_a_control() {
    let root = workspace_root();
    let mut files: Vec<String> = Vec::new();
    collect_rs(&root.join("apps/kt-app"), &mut files);
    let mut draws_flags = false;
    for rel in &files {
        if rel.ends_with("_tests.rs") || rel.contains("/tests/") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        if src.contains("widgets::flags_row(") {
            draws_flags = true;
        }
    }
    let has_flags = kt_model::GeneratorKind::all().iter().any(|k| {
        use kt_model::HasParams;
        !k.flag_specs().is_empty()
    });
    assert!(
        !has_flags || draws_flags,
        "kinds declare flags, but no window calls widgets::flags_row — \
         there is no way for the person to enable them"
    );
}

/// Test 602 (R.56, item 452): the strip render outside the player lives in ONE place.
///
/// The offline path has the right to its own sources only under three conditions
/// (invariant 5): it shares them with no one, promises no tempo, and its task
/// is finite and cancellable. This can be verified by reading at ONE place; at
/// five it is no longer possible. Therefore `VideoRenderer`/`AudioRenderer` in kt-app
/// are created either by the render dialog (export) or by the strip preview service, and
/// by no one else.
#[test]
fn t602_reel_rendering_lives_in_one_place() {
    let mut bad: Vec<String> = Vec::new();
    for (file, src) in production_corpus() {
        if !file.contains("kt-app")
            || file.contains("render_dialog.rs")
            || file.contains("reel_preview.rs")
        {
            continue;
        }
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("//") {
                continue;
            }
            let renders = t.contains("VideoRenderer::new")
                || t.contains("AudioRenderer::new")
                || t.contains("render_raw_video_range")
                || t.contains("render_raw_audio_range");
            if renders {
                bad.push(format!("{file}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "offline render was started bypassing the reel preview service. Its own sources, \
         its own thread and its own cancel — this is a shape to watch in ONE \
         place, otherwise the next such path will take the disk away from playback:\n  {}",
        bad.join("\n  ")
    );
    // self-check of the scan: the place itself must exist and be one
    let src = std::fs::read_to_string(workspace_root().join("apps/kt-app/src/reel_preview.rs"))
        .expect("no reel_preview.rs");
    assert_eq!(
        src.matches("pub fn render_reel_preview(").count(),
        1,
        "the body of the offline reel render has either vanished or multiplied — \
         the prohibition above has stopped meaning anything"
    );
}

/// Test 637c (R.58, item 462): the TIME FLOOR for gestures — one rule per file.
///
/// In `timeline.rs` a TIME POSITION is not clamped by a bare `.max(0)`: for
/// gestures over EVENTS the floor is `self.time_floor()` (inside a reel zero is not a
/// wall), for the cursor/region/markers/areas it is `CURSOR_FLOOR` by name
/// (zero always, the owner’s decision). A literal `0` in this place is
/// a second rule written as a number, and exactly this is how, before R.58, inside a reel
/// “it was impossible to drop an event beyond its bounds”. With zero it is legitimate to clamp
/// only NON-positions: a row index, a LENGTH (fade, marker range) and
/// an offset into the material (`source_in`).
#[test]
fn t637c_gesture_time_floor_is_never_a_bare_zero() {
    let root = workspace_root();
    let rel = "apps/kt-app/src/timeline.rs";
    let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
    // what may be clamped with zero: rows, lengths, material
    const NOT_A_POSITION: [&str; 5] =
        ["row_at_y", "len.max(0)", "- start).max(0)", "- m.t.0).max(0)", "src_in"];
    let bad: Vec<String> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains(".max(0)") && !NOT_A_POSITION.iter().any(|p| l.contains(p)))
        .map(|(n, l)| format!("{rel}:{}: {}", n + 1, l.trim()))
        .collect();
    assert!(
        bad.is_empty(),
        "a bare `.max(0)` over a time position: use `self.time_floor()` for events or \
         `CURSOR_FLOOR` for the cursor/region/markers:\n  {}",
        bad.join("\n  ")
    );
    // and both rules really are in the file
    assert!(src.contains("time_floor()"), "events lost their time floor");
    assert!(src.contains("CURSOR_FLOOR"), "the cursor floor lost its name");
}

/// Test 669 (R.59, stage 2): the generator’s pictures are taken by the ADDRESS
/// of the EVENT, and a kind without a picture names itself.
///
/// The values moved to the event, and the media no longer has them: ask the
/// filmstrip `c.media` — and it will show the kind’s defaults, that is a black
/// rectangle instead of the typed title. There is one rule for the program —
/// `kt_engine::clip_source_addr`, and it also holds invariant 4 (“order =
/// read”).
///
/// The second half — rule 16: a bin card with a kind with defaults legitimately
/// gives an EMPTY frame (for a title the default is an empty string), and
/// one must not stay silent about this: the card writes the kind’s name right into the thumbnail.
#[test]
fn t669_generator_thumbs_follow_the_event_address() {
    use kt_model::GeneratorKind;

    // 1) the predicate “no frame will come” is honest
    assert!(crate::gen_ui::blank_default(&GeneratorKind::default_text()));
    assert!(crate::gen_ui::blank_default(&GeneratorKind::Transparent));
    assert!(!crate::gen_ui::blank_default(&GeneratorKind::Solid {
        color_r: 255,
        color_g: 136,
        color_b: 0
    }));

    // 2) the filmstrip and the coverage strip ask for the EVENT’S ADDRESS
    let root = workspace_root();
    for (rel, what) in [
        ("apps/kt-app/src/timeline.rs", "filmstrip"),
        ("apps/kt-app/src/app.rs", "coverage bar"),
        ("apps/kt-app/src/fx_editor.rs", "gallery base frame"),
    ] {
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        assert!(
            src.contains("clip_source_addr"),
            "{what} ({rel}) asks for the event’s media, not its SOURCE: \
             for a generator the values live on the event, and the media knows only the kind"
        );
    }

    // 3) the event’s thumbnail source is raised by this address and knows the VALUES
    let (mut h, _g) = crate::layout_tests::harness_three_clips_dt(Some(0.02));
    let made = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    h.state_mut().create_generator(made.clone());
    h.step();
    let (media, clip) = crate::layout_tests::made_generator(&h);
    let addr = kt_engine::gen_source_addr(clip);
    assert_ne!(addr, media, "event source address must differ from the media address");
    let _ = h.state().thumbs.duration_or_probe(addr);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some((v, a)) = h.state().thumbs.kinds(addr) {
            assert!(v && !a, "generator event source is video without audio");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the event’s thumbnail source never came up: the generator filmstrip will stay empty"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
        h.step();
    }
    assert_eq!(
        h.state().gen_states.get(&clip).map(|s| s.kind.clone()),
        Some(made),
        "source snapshot must carry the event’s VALUES"
    );

    // 4) the card of an empty kind writes the KIND’S NAME right into the thumbnail
    h.state_mut().create_generator(GeneratorKind::default_text());
    for _ in 0..4 {
        h.step();
    }
    let text_media = h.state().model.project().media.last().unwrap().id;
    let card = h.state().bin_card_rect(text_media).expect("title card is not drawn");
    let want = crate::gen_ui::kind_name(&GeneratorKind::default_text());
    let inside =
        painted_texts(&h).into_iter().any(|t| t.src == want && card.contains(t.rect.center()));
    assert!(
        inside,
        "a kind card without a frame stayed a BLACK rectangle — \
         the impossibility of showing a picture must be visible (rule 16)"
    );
}

/// Test 682 (R.59, stage 3): the publishing door is ONE, and it is reachable.
///
/// Three consequences are asserted, each from the point where the person acts:
/// (1) a right-click on a parameter’s label INSIDE a strip offers “Publish…”,
/// and on an already declared one — “Remove publication”; this holds both with full entry
/// and in an expanded strip;
/// (2) outside a strip this item is NOT PRESENT AT ALL (rule 9), and the status line explains
/// why (rule 16);
/// (3) the instance’s menu reaches BOTH windows — the consumer’s and the author’s.
#[test]
fn t682_publishing_is_reachable_from_inside_a_reel_only() {
    use crate::i18n::{k, tr};
    use crate::layout_tests::harness_news_template;
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    // the parameter’s label is the UPPER one of the same-named ones: the same name also stands on
    // the curve-choice button below (lesson of t281)
    let label_top =
        |h: &mut egui_kittest::Harness<'_, crate::app::KtApp>, label: &str| -> Option<egui::Rect> {
            h.query_all_by_label_contains(label)
                .map(|n| n.rect())
                .min_by(|a, b| a.top().total_cmp(&b.top()))
        };
    // the LANGUAGE is set by the harness, so the captions are asked for AFTER its
    // creation: a row assembled earlier would come in the default language,
    // and the test would look for an English word in a Russian window
    let labels = || {
        (
            tr(k::pubp::PUBLISH).to_string(),
            tr(k::pubp::UNPUBLISH).to_string(),
            // a declared row and an UNdeclared one: the first one’s door leads
            // outward, the second one’s — inward
            crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "size"),
            crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "line_spacing"),
        )
    };

    // ---- 1a) FULL ENTRY ---------------------------------------------
    {
        let (mut h, s) = harness_news_template();
        let (publish, unpublish, published_row, free_row) = labels();
        h.state_mut().enter_nest(s.a);
        h.step();
        h.state_mut().open_fx_editor_head(s.inner);
        h.step();
        h.step();
        let rect = label_top(&mut h, &free_row).expect("label of an undeclared row is missing");
        let before = h.query_all_by_label_contains(&publish).count();
        right_click(&mut h, rect.center());
        assert!(
            h.query_all_by_label_contains(&publish).count() > before,
            "inside a reel, the right click on the label did not offer publication"
        );
        let _ = (&unpublish, &published_row);
    }

    // ---- 1c) an already declared row offers REMOVAL, through the same door ----
    // Its own harness: the menu cannot be closed by Esc — it cascades and
    // also clears the window itself, and the next check would probe an empty screen.
    {
        let (mut h, s) = harness_news_template();
        let (_pub, unpublish, published_row, _free) = labels();
        h.state_mut().enter_nest(s.a);
        h.step();
        h.state_mut().open_fx_editor_head(s.inner);
        h.step();
        h.step();
        let rect = label_top(&mut h, &published_row).expect("no label of the declared row");
        let before = h.query_all_by_label_contains(&unpublish).count();
        right_click(&mut h, rect.center());
        assert!(
            h.query_all_by_label_contains(&unpublish).count() > before,
            "a declared handle has no reverse action — it would be a trap"
        );
    }

    // ---- 1b) EXPANDED STRIP ----------------------------------------
    {
        let (mut h, s) = harness_news_template();
        let (publish, _un, _pub_row, free_row) = labels();
        h.state_mut().expand_in_place(s.a);
        h.step();
        h.step();
        assert!(!h.state().model.bands().is_empty(), "strip did not expand — the scene is wrong");
        h.state_mut().open_fx_editor_head(s.inner);
        h.step();
        h.step();
        let rect = label_top(&mut h, &free_row).expect("there is no label in the strip");
        let before = h.query_all_by_label_contains(&publish).count();
        right_click(&mut h, rect.center());
        assert!(
            h.query_all_by_label_contains(&publish).count() > before,
            "in the expanded strip publication is unreachable, although the content is the same"
        );
    }

    // ---- 2) OUTSIDE A STRIP there is no item -------------------------------------
    {
        let (mut h, _s) = harness_news_template();
        let (publish, _un, _pr, _fr) = labels();
        let root = {
            let doc = h.state().model.document();
            h.state()
                .model
                .project()
                .clips
                .iter()
                .find(|c| c.r#gen.is_none() && doc.sequence(c.media).is_none())
                .map(|c| c.id)
                .expect("root event from the file")
        };
        assert!(
            h.state_mut().apply(Command::SetFx {
                id: root,
                fx: vec![FxInstance::Gain { db: 0.0, pan: 0.0 }],
            })
        );
        h.step();
        h.state_mut().open_fx_editor(root);
        h.step();
        h.step();
        let db = crate::widgets::param_label("db");
        let rect = label_top(&mut h, &db).expect("the gain label is not in the window");
        // FIRST the hint (the menu would cover it): the absence of the item does not stay
        // silent — the status line names the reason.
        h.hover_at(rect.center());
        h.step();
        h.step();
        let hint = h.state().hover_hint.clone().unwrap_or_default();
        assert!(
            hint.contains(tr(k::pubp::OUTSIDE_REEL)),
            "the hint did not explain where the publication lives: {hint:?}"
        );
        let before = h.query_all_by_label_contains(&publish).count();
        right_click(&mut h, rect.center());
        assert_eq!(
            h.query_all_by_label_contains(&publish).count(),
            before,
            "outside the reel the publication item appeared — it is a dead control (rule 9)"
        );
    }

    // ---- 3) the instance’s menu reaches BOTH windows --------------------
    {
        let (mut h, s) = harness_news_template();
        h.step();
        let g = h.state().tl_geom.clone().expect("timeline geometry");
        let c = h.state().model.clip(s.b).cloned().expect("second issue");
        let pos = egui::pos2(
            g.tick_to_x((c.start + kt_time::Tick(c.duration.0 / 2)).0),
            g.lane_y(kt_model::ClipKind::Video, c.lane, 0.5),
        );
        right_click(&mut h, pos);
        let labels: Vec<String> = h
            .query_all_by_label_contains("")
            .filter_map(|n| {
                use egui_kittest::kittest::NodeT;
                n.accesskit_node().label().map(|l| l.to_string())
            })
            .collect();
        for want in [tr(k::pubp::INST_OPEN), tr(k::pubp::LIST_OPEN)] {
            assert!(
                labels.iter().any(|l| l.contains(want)),
                "the instance menu does not lead to “{want}”: {labels:?}"
            );
        }
    }
}

/// Test 693 (R.60): curves of ANY carrier are shown by ONE widget.
///
/// The R.34 decision — “one animation mechanism for everything”. The composition
/// parameters window bypassed it: it had its own key recording (the field wrote a key
/// at the cursor) and had neither a graph nor a table. The owner: “I do not see
/// curves of automation anywhere… there is not one button in the automation window”
/// at all.
///
/// The guard is two-sided. From above — by CODE: the curve-recording primitives
/// (`bound_apply`/`bound_value`) live only where the widget itself lives;
/// from below — by CONSEQUENCE: every window that edits a value over time
/// has a call to the widget, and its controls have a hint in the status line
/// (rule 10: we have no pop-ups).
#[test]
fn t693_every_carrier_draws_its_curves_with_the_one_widget() {
    let root = workspace_root();

    // --- 1. curve primitives are not called around the widget ------------------
    const HOME: [&str; 2] = ["apps/kt-app/src/widgets.rs", "apps/kt-app/src/automation.rs"];
    let mut files: Vec<String> = Vec::new();
    for dir in ["libs", "apps"] {
        collect_rs(&root.join(dir), &mut files);
    }
    let mut bad: Vec<String> = Vec::new();
    for rel in &files {
        if HOME.contains(&rel.as_str()) || rel.contains("/tests/") || rel.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            // `CurveBind` is the legitimate door: it is the “field bound to a
            // curve”, and it lives in the same home as the widget.
            if (t.contains("bound_apply(") || t.contains("bound_value("))
                && !t.contains("bind.")
                && !t.contains("fn bound_")
            {
                bad.push(format!("{rel}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "writing to a curve bypassing the single widget ({}): this is how an \
         invisible keyframe editor appeared in the composition parameters window:\n  {}",
        HOME.join(", "),
        bad.join("\n  ")
    );

    // --- 2. every carrier has a call to the widget ---------------------
    for (rel, who) in [
        ("apps/kt-app/src/fx_editor.rs", "effect slot"),
        ("apps/kt-app/src/gen_ui.rs", "generator on the event"),
        ("apps/kt-app/src/camera_editor.rs", "camera"),
        ("apps/kt-app/src/pubparam.rs", "public composition parameter"),
    ] {
        let src = std::fs::read_to_string(root.join(rel)).expect(rel);
        assert!(
            src.contains("automation::draw("),
            "{who} ({rel}) edits the value over time but does not call the automation widget — \
             which means it grew its own keyframe editor"
        );
    }

    // --- 3. consequence: the control is named and explains itself -----------------
    use crate::i18n::{k, tr};
    use crate::layout_tests::harness_news_template;
    let (mut h, s) = harness_news_template();
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    let add = {
        use egui_kittest::kittest::Queryable;
        h.query_all_by_label(tr(k::fx::AUTO_ADD))
            .map(|n| n.rect())
            .next()
            .expect("the composition parameters window has no NAMED action “set up a curve”")
    };
    h.hover_at(add.center());
    h.step();
    h.step();
    let hint = h.state().hover_hint.clone();
    assert_eq!(
        hint.as_deref(),
        Some(tr(k::pubp::AUTO_ADD_HINT)),
        "the action “set up a curve” has no hint in the status line (rule 10): {hint:?}"
    );
}

/// Test 699b (R.61): there is NO clipboard state inside the program.
///
/// The owner identified the defect by an indirect sign: “judging by the fact that a button
/// can be “disabled” — there is some clipboard state in the code —
/// which is bad.” He was right: copying wrote both to the system clipboard and to
/// the `env_clipboard` mirror, and the button’s availability was decided BY THE MIRROR —
/// what was copied in the program’s second window could not be pasted.
///
/// The guard is two-sided: by CODE — there is no mirror and the paste button is not disabled;
/// by CONSEQUENCE — the clipboard parsing lives in one place and is called from the background.
#[test]
fn t699b_no_clipboard_state_lives_inside_the_program() {
    let root = workspace_root();
    let mut files: Vec<String> = Vec::new();
    collect_rs(&root.join("apps"), &mut files);

    // --- 1. no mirror of the envelope clipboard remains ----------------------
    let mut bad: Vec<String> = Vec::new();
    for rel in &files {
        if rel.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        for (n, line) in src.lines().enumerate() {
            // we look for the FIELD, not any occurrence of the string: `poll_env_clipboard`
            // is precisely the reading of the system clipboard, and it is legitimate.
            if line.contains(".env_clipboard") || line.contains("env_clipboard:") {
                bad.push(format!("{rel}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "the mirror of the envelope buffer came back — the button will again go dark on \
         content copied in another window:\n  {}",
        bad.join("\n  ")
    );

    // --- 2. the paste button is not disabled --------------------------------
    for rel in &files {
        if rel.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            assert!(
                !(t.contains("add_enabled") && t.contains("PASTE_ENV")),
                "{rel}:{}: the envelope paste button goes dark again — the buffer content \
                 must be read ON PRESS, not decide availability",
                n + 1
            );
        }
    }

    // --- 3. the system clipboard is touched by EXACTLY one place ------------------
    const HOME: &str = "apps/kt-app/src/envclip.rs";
    let mut readers: Vec<String> = Vec::new();
    for rel in &files {
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        if src.contains("arboard::") && rel != HOME {
            readers.push(rel.clone());
        }
    }
    assert!(
        readers.is_empty(),
        "the system clipboard is read bypassing the single home ({HOME}): {readers:?}"
    );
    let home = std::fs::read_to_string(root.join(HOME)).expect(HOME);
    assert!(
        home.contains("BgJob::spawn"),
        "reading the clipboard stopped being a background task: on X11 the window owns the \
         clipboard, and a synchronous request to itself will hang in the event loop"
    );
}

/// Test 720 (R.62): the project settings window is reachable from the “File” menu — from
/// the point where the person acts, and not by calling a function.
#[test]
fn t720_project_settings_reachable_from_the_file_menu() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    let mut h = crate::layout_tests::harness();
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();
    h.step();
    let labels = crate::layout_tests::menu_labels(&mut h);
    assert!(
        h.query_by_label(tr(k::menu::file::PROJECT_SETTINGS)).is_some(),
        "the “Project settings…” item must be in the “File” menu; visible: {labels:?}"
    );
    h.get_by_label(tr(k::menu::file::PROJECT_SETTINGS)).click();
    h.step();
    h.step();
    assert!(h.state().settings_win.is_some(), "the menu item opens a window");
    assert!(h.query_by_label(tr(k::settings::TITLE)).is_some());
    assert!(h.query_by_label(tr(k::settings::APPLY)).is_some());
}

/// Test 741 (R.62, change B, 4.5): a TRACK has a full set of verbs, and
/// the paired paths of one action are one body.
///
/// Two assertions that neither t253 (it is about entities and their menus)
/// nor t742 (it is about badges) makes:
///
/// 1. Mute, solo and the chain are present exactly where there is something for them to do.
///    The service row has them neither as a badge nor as a menu item: a dead
///    control violates rule 9, and “but it is in the menu” is no answer here.
/// 2. The badge, the menu item and the hotkey give ONE AND THE SAME result and
///    one history step. If they diverged, “muted from the menu” and “muted via the
///    badge” would behave differently on one track (rule 17).
#[test]
fn t741_track_has_every_verb_and_paired_paths_are_one_body() {
    use crate::timeline::{GUTTER_W, TrackBadge};
    use egui_kittest::kittest::Queryable;
    use kt_model::LaneKind::{Audio, Camera, Morph, Video};

    let scene = || {
        let (mut h, _) = crate::layout_tests::harness_three_clips_dt(None);
        {
            let app = h.state_mut();
            app.model.apply(kt_model::Command::InsertTrack { kind: Morph, at: 0 }).unwrap();
            app.model.apply(kt_model::Command::InsertTrack { kind: Camera, at: 0 }).unwrap();
        }
        h.step();
        let g = h.state().tl_geom.clone().expect("timeline geometry");
        (h, g)
    };

    // captions are taken AFTER the harness is created: the catalog language lives in the TLS
    // of the UI thread, and strings computed earlier would come in another language
    let mut problems: Vec<String> = Vec::new();
    for kind in [Video, Audio, Morph, Camera] {
        let (mut h, g) = scene();
        let switches = [
            crate::i18n::tr(crate::i18n::k::track::MENU_MUTE),
            crate::i18n::tr(crate::i18n::k::track::MENU_SOLO),
            crate::i18n::tr(crate::i18n::k::track::MENU_FX),
        ];
        let always = [
            crate::i18n::tr(crate::i18n::k::track::MENU_PROPS),
            crate::i18n::tr(crate::i18n::k::track::MENU_LOCK),
        ];
        let row = g.rows.iter().position(|r| *r == (kind, 0)).expect("row on screen");
        // we press ON THE TRACK, not in the header: a person points where
        // they see the row (lesson of R.40). The place is FREE: if the click hits an
        // event, its menu would open, and it would be the event, not the track, that gets checked
        let pos = egui::pos2(g.tick_to_x(400_000), g.row_center_y(row));
        let count = |h: &egui_kittest::Harness<'_, crate::app::KtApp>, l: &str| {
            h.query_all_by_label_contains(l).count()
        };
        let before: Vec<usize> =
            always.iter().chain(switches.iter()).map(|l| count(&h, l)).collect();
        right_click(&mut h, pos);
        let after: Vec<usize> =
            always.iter().chain(switches.iter()).map(|l| count(&h, l)).collect();
        for (i, l) in always.iter().enumerate() {
            if after[i] <= before[i] {
                problems.push(format!(
                    "{kind:?}: no “{l}” item in the row menu — there is nothing to control \
                     the track properties with"
                ));
            }
        }
        for (i, l) in switches.iter().enumerate() {
            let j = always.len() + i;
            let shown = after[j] > before[j];
            if kind.has_switches() && !shown {
                problems.push(format!("{kind:?}: no “{l}” item, yet there is something to mute here"));
            }
            if !kind.has_switches() && shown {
                problems.push(format!(
                    "{kind:?}: the item “{l}” on the service row does nothing, \
                     and there are no dead controls (rule 9)"
                ));
            }
        }
        // and the badges say exactly the same thing
        let badges = crate::timeline::row_badges(kind);
        let want = if kind.has_switches() { 4 } else { 1 };
        if badges.len() != want {
            problems.push(format!(
                "{kind:?}: badges {} instead of {want} — the row and the menu diverged",
                badges.len()
            ));
        }
    }
    assert!(problems.is_empty(), "track verbs:\n  {}", problems.join("\n  "));

    // --- 2. three paths — one result and one history step --------------------
    let mute_by = |what: &str| {
        let (mut h, g) = scene();
        let id = h.state().model.project().track_at(Video, 0).unwrap().id;
        let row = g.rows.iter().position(|r| *r == (Video, 0)).unwrap();
        let depth0 = h.state().model.undo_depth();
        match what {
            "badge" => {
                let r = TrackBadge::row_rects(
                    Video,
                    g.rect.left() - GUTTER_W,
                    g.row_top(row),
                    g.row_h(row),
                )
                .into_iter()
                .find(|(b, _)| *b == TrackBadge::Mute)
                .unwrap()
                .1;
                let pos = r.center();
                h.hover_at(pos);
                h.step();
                for pressed in [true, false] {
                    h.event(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                h.step();
                h.step();
            }
            "menu" => {
                right_click(&mut h, egui::pos2(g.tick_to_x(400_000), g.row_center_y(row)));
                h.get_by_label(crate::i18n::tr(crate::i18n::k::track::MENU_MUTE)).click();
                h.step();
                h.step();
            }
            "key" => {
                {
                    let app = h.state_mut();
                    app.selected.clear();
                    app.selected.insert(kt_model::ClipId(1));
                }
                h.step();
                h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::M);
                h.step();
            }
            _ => unreachable!(),
        }
        let t = h.state().model.project().track(id).unwrap().clone();
        (t, h.state().model.undo_depth() - depth0)
    };
    let (badge, steps_b) = mute_by("badge");
    let (menu, steps_m) = mute_by("menu");
    let (key, steps_k) = mute_by("key");
    assert!(badge.mute, "the badge did not mute the track");
    assert_eq!(badge, menu, "the badge and the menu item gave a DIFFERENT track — the bodies diverged");
    assert_eq!(badge, key, "the badge and the hotkey gave a DIFFERENT track — the bodies diverged");
    assert_eq!(
        (steps_b, steps_m, steps_k),
        (1, 1, 1),
        "muting must be one history step on every path"
    );
}

/// Test 752b (R.63 item 9): the publishing door is also present on the LEADING NODE.
///
/// The owner: “for what reason does the first geometry have no way to publish
/// parameters, while the second one does? This first geometry runs as an fx, does it not? or do we have
/// duplicate code?”. There are no duplicates — the leading node had no publishing ADDRESS, and the door
/// was closed by a single `!lead_sel`.
/// The assertion is made from the person’s point: select the node, right-click the zoom label,
/// the “Publish…” item is there (on the previous code it was not).
#[test]
fn t752b_publishing_is_reachable_at_the_lead_geometry() {
    use crate::i18n::{k, tr};
    use crate::layout_tests::harness_news_template;
    use egui_kittest::kittest::Queryable;

    let (mut h, s) = harness_news_template();
    let publish = tr(k::pubp::PUBLISH).to_string();
    let zoom = crate::widgets::param_label("zoom");
    h.state_mut().enter_nest(s.a);
    h.step();
    h.state_mut().open_fx_editor(s.inner);
    h.step();
    h.step();

    // we choose the LEADING node — its label stands in the strip of the chain
    let lead = crate::fx_editor::fx_display_name(&kt_model::EventGeometry::default().node());
    h.get_by_label(lead).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors[0].selected.get(&s.inner),
        Some(&crate::widgets::ChainSel::Lead),
        "the leading node is not selected — the test would probe an ordinary slot"
    );

    let rect = h
        .query_all_by_label_contains(&zoom)
        .map(|n| n.rect())
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .expect("the zoom label of the leading node is missing");
    let before = h.query_all_by_label_contains(&publish).count();
    right_click(&mut h, rect.center());
    assert!(
        h.query_all_by_label_contains(&publish).count() > before,
        "the leading node’s publication door is closed, although its properties are the same \
         as those of a slot: “or do we have duplicated code?”"
    );
}

/// The program’s WINDOW REGISTRY (R.63 item 12).
///
/// The owner: “I am banning any agents from spawning any extra
/// windows without an obvious purpose (like the window on program close). I am tired of agents
/// spewing these windows and not thinking.”
/// The ban is held not by a promise but by a list: each window here has
/// the reason why it cannot be part of an already existing place.
/// A new window not entered here fails t757 — and entering it means
/// writing this reason in words.
const WINDOWS: &[(&str, &str)] = &[
    ("fx", "event effect chain: the strip, parameters, automation, the source head and the composition parameters"),
    ("morph", "morpher layer: its own targets, window and patch — a separate entity of the edit"),
    ("transition", "transition at the joint: kind, curves, inversion"),
    ("camera", "camera event: its own, because it lives on the camera track"),
    ("track_window", "object track: name, level, switches, chain and ducking"),
    ("env_fit", "field matching on envelope paste — a question that must be answered before the paste"),
    ("project_settings", "project settings: frame size, frame rate, alignment rule"),
    ("kt_render_settings", "export parameters: filled in before the start and checked at once"),
    ("kt_render_progress", "export progress: it runs for minutes and can be cancelled"),
    ("kt_analysis", "progress of background analyses — the same for loudness, sync and ducking"),
    ("kt_notices", "message log: the status line shows the latest, the log shows all"),
    ("kt_search", "search through the edit: the list of finds lives as long as one walks through it"),
    ("kt_diag", "diagnostics and counters: the developer window, lives alongside the work"),
    ("seq_name", "name of the new reel: asked once when the selection is collapsed"),
    ("paste_names", "prefix for the names of the pasted items — asked before the paste"),
    ("preset_save", "name of the saved preset — asked once on save"),
    ("pub_name", "name of the published handle — asked once"),
    ("pub_unpublish", "confirmation of handle removal: the overrides will die in all places"),
    ("published_win", "author’s list of public parameters of the reel"),
    ("kt_recovery", "recovery of unsaved work at startup"),
    ("kt_ask", "question on close — that very “obviously meaningful” window from the owner’s words"),
    ("kt_ffmpeg", "where FFmpeg was found and how to get it: per machine, outside any project, and reachable when nothing plays (windows-build)"),
];

/// Test 757 (R.63 item 12): EVERY window of the program is in the registry, and vice versa.
///
/// The scanner looks for `egui::Window::new`/`Modal::new` in the kt-app sources and
/// takes the window’s id (it is mandatory since t129). A window without an entry is a new window
/// created silently; an entry without a window is garbage in the registry.
#[test]
fn t757_every_window_is_in_the_registry() {
    let root = workspace_root();
    let dir = root.join("apps/kt-app/src");
    let mut found: Vec<(String, String)> = Vec::new();
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .expect("kt-app sources")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        // the window tests themselves do not register windows
        if name.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(&path).expect("file"));
        let lines: Vec<&str> = src.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            if !line.contains("egui::Window::new") && !line.contains("Modal::new") {
                continue;
            }
            // the id is on this same line (modal) or on the nearest following lines
            let mut id = None;
            for probe in lines.iter().skip(n).take(4) {
                if let Some(p) = probe.find("egui::Id::new(") {
                    let rest = &probe[p + "egui::Id::new(".len()..];
                    let rest = rest.trim_start_matches('(');
                    if let Some(q1) = rest.find('"') {
                        let tail = &rest[q1 + 1..];
                        if let Some(q2) = tail.find('"') {
                            id = Some(tail[..q2].to_string());
                            break;
                        }
                    }
                }
            }
            match id {
                Some(id) => found.push((id, format!("{name}:{}", n + 1))),
                None => panic!(
                    "{name}:{}: the window has no `egui::Id::new(\"…\")` — there is nothing to tell it apart by, \
                     nor to enter it in the registry (rule t129)",
                    n + 1
                ),
            }
        }
    }
    assert!(!found.is_empty(), "the scanner found not a single window — it is broken");

    for (id, at) in &found {
        assert!(
            WINDOWS.iter().any(|(k, _)| k == id),
            "window “{id}” ({at}) is not entered in the WINDOWS registry. The owner forbade opening windows \
             without an obvious purpose — entering a window means writing in words \
             why it cannot be part of an existing place"
        );
    }
    for (id, why) in WINDOWS {
        assert!(
            found.iter().any(|(f, _)| f == id),
            "the registry has an entry “{id}” ({why}), but the program has no window with such an id"
        );
        assert!(
            why.chars().count() >= 25,
            "window “{id}” has no reason written in words: {why:?}"
        );
    }
    // windows that the owner called superfluous are no longer in the program
    for gone in ["duck", "inst_params"] {
        assert!(
            !found.iter().any(|(f, _)| f == gone),
            "window “{gone}” came back: it was removed by the owner’s direct demand (R.63)"
        );
    }
}

/// Test 766 (R.64): on-screen controls are REACHABLE and explain themselves.
///
/// A gesture that cannot be found does not exist (rule 1 of the checklist), and a handle
/// that stays silent in the status line is “guess what I will do” (rule 10).
/// Here one probes exactly the place from which the person acts: a right-click on the frame,
/// a menu item, hovering over a handle. As a separate assertion — that the controls
/// live in ONE place (`draw_preview`), therefore in the detached window they are
/// the same: there is no copy of the preview drawing in the program.
#[test]
fn t766_on_screen_controls_are_reachable_and_speak() {
    use egui_kittest::kittest::Queryable;
    let (mut h, clip) = crate::layout_tests::harness_geom_ctl();
    // the scene starts with DISABLED controls — and those are what must be found
    h.state_mut().geom_ctl = crate::geomctl::GeomMode::Off;
    h.step();

    // ---- switch: right-click on the frame → first item ------------
    let pos = h.state().preview_rect.center();
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    let label = crate::tf!(
        crate::i18n::k::geomctl::TOGGLE,
        crate::geomctl::mode_name(crate::geomctl::GeomMode::Off.next())
    );
    h.get_by_label_contains(&label).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().geom_ctl,
        crate::geomctl::GeomMode::Transform,
        "the preview menu item did not enable the controls: the gesture stayed unreachable"
    );

    // ---- the handle names ITSELF in the status line ------------------------
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let pl = h.state().geom_placement(clip, canvas).expect("placement");
    let img = h.state().preview_img_rect;
    let to_screen = |p: (f64, f64)| -> egui::Pos2 {
        egui::pos2(
            img.left() + (p.0 / canvas.0) as f32 * img.width(),
            img.top() + (p.1 / canvas.1) as f32 * img.height(),
        )
    };
    for (handle, key) in [
        (kt_fx::geom::Handle::Rot, crate::i18n::k::geomctl::HINT_ROT),
        (kt_fx::geom::Handle::TopLeft, crate::i18n::k::geomctl::HINT_SCALE),
    ] {
        h.hover_at(to_screen(handle.point(&pl)));
        h.step();
        h.step();
        let shown = h.state().status_line_shown.0.clone();
        assert_eq!(
            shown,
            crate::i18n::tr(key),
            "over the handle {handle:?} the status line says “{shown}” — \
             the hint must name the GESTURE that will happen here"
        );
    }

    // ---- controls are drawn IN ONE PLACE ----------------------------------
    // (so the detached window gets them from the same code: `draw_preview`
    // is called by both branches — the panel and the viewport)
    let src = std::fs::read_to_string("apps/kt-app/src/app.rs")
        .or_else(|_| std::fs::read_to_string("src/app.rs"))
        .expect("source app.rs");
    let calls = src.matches("self.geometry_controls(").count();
    assert_eq!(
        calls, 1,
        "geometry controls are called from {calls} places — a second copy will diverge \
         from the first silently (rule 17)"
    );
    assert!(
        src.matches("self.draw_preview(ui);").count() >= 2,
        "the preview is drawn in one call — the detached window stopped being the same code"
    );
}

/// Test 777 (R.65): the SIDECHAIN is REACHABLE and explains a refusal.
///
/// The R.40 rule: probe from where the person acts. Two paths lead to ducking:
/// the track window (the section) and the same track’s chain strip (the compressor with
/// its own key field); both must lead to ONE body (rule 17), and an impossible
/// choice must be named in words, not silently absent.
#[test]
fn t777_sidechain_is_reachable_and_names_its_refusals() {
    use egui_kittest::kittest::Queryable;
    use kt_model::LaneKind::Audio;
    let (mut h, _) = crate::layout_tests::harness_three_clips_dt(None);
    let (bg, key) = {
        let app = h.state_mut();
        crate::layout_tests::edit_project(app, |p| p.ensure_tracks(Audio, 2));
        let bg = app.model.project().track_at(Audio, 0).unwrap().id;
        let key = app.model.project().track_at(Audio, 1).unwrap().id;
        (bg, key)
    };
    h.step();

    // PATH 1: the track window — the ducking section with the key choice
    h.state_mut().open_track_window(bg);
    h.step();
    h.step();
    assert!(
        h.query_all_by_label_contains(crate::i18n::tr(crate::i18n::k::msg::DUCK_KEY)).count() > 0,
        "the track window has no key choice — the sidechain is unreachable from where the person stands"
    );

    // PATH 2: the track’s FX badge leads to the SAME window (one body, rule 17)
    h.state_mut().track_win = None;
    h.step();
    h.state_mut().open_track_fx(bg);
    h.step();
    assert_eq!(
        h.state().track_win.as_ref().map(|w| w.id),
        Some(bg),
        "the FX badge leads not to the track window — there are two paths, and the body must be one"
    );

    // REFUSAL NAMED: the model rejects the cycle with a reason, not silently
    {
        let app = h.state_mut();
        let mut c = kt_model::FxInstance::compressor();
        if let kt_model::FxInstance::Compressor { key: k, .. } = &mut c {
            *k = Some(key);
        }
        app.apply(kt_model::Command::SetTrackFx { id: bg, fx: vec![c.clone()], bypass: vec![] });
        // the reverse key — a cycle
        let mut back = kt_model::FxInstance::compressor();
        if let kt_model::FxInstance::Compressor { key: k, .. } = &mut back {
            *k = Some(bg);
        }
        let e = app
            .model
            .apply(kt_model::Command::SetTrackFx { id: key, fx: vec![back], bypass: vec![] })
            .unwrap_err();
        assert!(
            matches!(e, kt_model::CommandError::SidechainCycle(_)),
            "cycle rejected without a named reason: {e:?}"
        );
        assert!(
            !format!("{e}").is_empty(),
            "the rejection has no text — the person has nothing to show in the status line"
        );
    }
}

/// How to open the registry window in the harness — or why this is impossible (R.66).
enum OpenWin {
    /// The function prepares the scene and opens the window.
    By(fn(&mut egui_kittest::Harness<'static, crate::app::KtApp>)),
    /// A window cannot be opened in the harness; the reason is written in words, as in t253.
    Cant(&'static str),
}

/// How the window is closed: by its own action button or by the title cross.
/// The second is legitimate (log, search, diagnostics), but it is declared EXPLICITLY and with
/// a reason: otherwise “no buttons were found” would quietly pass as the norm.
enum Closes {
    Action,
    /// A window SHORT BY CONSTRUCTION (the question from the header, the rows and the row of
    /// buttons): there is nothing for scrolling to hide there. The reason is written in words, but
    /// “short” is CHECKED by a number of rows — otherwise the exception would become
    /// cheaper than the check.
    ActionShort { why: &'static str },
    Titlebar { title: fn() -> String, why: &'static str },
}

/// How many rows a window is allowed to show in order to count as short.
const SHORT_WINDOW_ROWS: usize = 25;

/// The window’s ACTION buttons: the test presses any one of them. The set is shared — the
/// program’s windows are closed and cancelled by the same words.
fn action_labels() -> Vec<String> {
    use crate::i18n::{k, tr};
    vec![
        tr(k::track::CLOSE).to_string(),
        tr(k::track::CANCEL).to_string(),
        tr(k::fx::CANCEL).to_string(),
        tr(k::render::CLOSE).to_string(),
        tr(k::render::CANCEL).to_string(),
        tr(k::settings::CANCEL).to_string(),
        tr(k::ask::RECOVERY_RESTORE).to_string(),
        tr(k::ask::RECOVERY_LATER).to_string(),
        tr(k::ffmpeg::RESCAN).to_string(),
    ]
}

/// Window table: the id from `WINDOWS` → how to open it.
fn window_openers() -> Vec<(&'static str, OpenWin, Closes)> {
    use kt_model::*;
    vec![
        ("fx", OpenWin::By(|h| {
            // the scene is a generator: the event is raised by the same path as in the
            // program, and it requires no file on disk.
            h.state_mut().create_generator(GeneratorKind::Solid {
                color_r: 200,
                color_g: 40,
                color_b: 40,
            });
            h.step();
            let id = first_clip(h);
            h.state_mut().open_fx_editor(id);
        }), Closes::Action),
        ("track_window", OpenWin::By(|h| {
            let app = h.state_mut();
            crate::layout_tests::edit_project(app, |p| p.ensure_tracks(LaneKind::Audio, 1));
            let id = app.model.project().track_at(LaneKind::Audio, 0).unwrap().id;
            app.open_track_window(id);
        }), Closes::Action),
        ("project_settings", OpenWin::By(|h| h.state_mut().open_project_settings()), Closes::Action),
        ("kt_notices", OpenWin::By(|h| h.state_mut().notices_open = true), Closes::Titlebar { title: || crate::i18n::tr(crate::i18n::k::win::NOTICES_TITLE).to_string(), why: "the message log is closed by the title cross: the list has no action of its own" }),
        ("kt_search", OpenWin::By(|h| h.state_mut().search_open = true), Closes::Titlebar { title: || crate::i18n::tr(crate::i18n::k::win::SEARCH_TITLE).to_string(), why: "search is closed by the title cross: the list of finds lives as long as one walks through it" }),
        ("kt_diag", OpenWin::By(|h| h.state_mut().perf.diag_open = true), Closes::Titlebar { title: || crate::i18n::tr(crate::i18n::k::win::DIAG_TITLE).to_string(), why: "diagnostics is closed by the title cross: it is a developer window with no actions" }),
        ("kt_recovery", OpenWin::By(|h| h.state_mut().recovery_open = true), Closes::Action),
        ("kt_ffmpeg", OpenWin::By(|h| h.state_mut().ffmpeg_ui.open = true), Closes::Action),
        ("kt_ask", OpenWin::By(|h| {
            h.state_mut().ask = Some(crate::app::Ask::DropProject);
        }), Closes::ActionShort {
            why: "question on close: a heading, one line and a row of buttons — the content has nowhere \
                  to grow, and the scroll has nothing to hide",
        }),
        ("morph", OpenWin::Cant(
            "the morpher layer must first be put on a morph track: the window opens by its id, and \
             its scene is built by a separate layer test",
        ), Closes::Action),
        ("transition", OpenWin::Cant(
            "the transition window opens by the id of an existing transition, and a transition appears only \
             from the overlap of two events — its own scene, it is in the transition tests",
        ), Closes::Action),
        ("camera", OpenWin::Cant(
            "a camera event lives on a camera track and opens by its own id — the scene with \
             the camera is built in the camera tests",
        ), Closes::Action),
        ("env_fit", OpenWin::Cant(
            "the field matching window appears only on an envelope paste that did not land \
             silently: its scene is the clipboard content, it is in the paste tests",
        ), Closes::Action),
        ("kt_render_settings", OpenWin::Cant(
            "the export parameters window is created together with its render session; the session requires \
             a real codec, which the harness does not have",
        ), Closes::Action),
        ("kt_render_progress", OpenWin::Cant(
            "the export progress window lives only while an encoding task is RUNNING; the harness cannot bring it up \
             without a real codec",
        ), Closes::Action),
        ("kt_analysis", OpenWin::Cant(
            "the analysis progress window lives while the background task is alive; the harness does not start it \
             without a real source file",
        ), Closes::Action),
        ("seq_name", OpenWin::Cant(
            "the name of the new reel is asked at the very moment the selection is collapsed — its own scene, \
             it is in the composition tests",
        ), Closes::Action),
        ("paste_names", OpenWin::Cant(
            "the prefix for names is asked when pasting a NAMED item from the clipboard — the clipboard scene \
             is built in the copy-paste tests",
        ), Closes::Action),
        ("preset_save", OpenWin::Cant(
            "the preset name is asked when saving a chain from the strip — the scene is in the \
             preset library tests",
        ), Closes::Action),
        ("pub_name", OpenWin::Cant(
            "the handle name is asked when publishing from inside the reel — the scene is in the \
             public parameters tests",
        ), Closes::Action),
        ("pub_unpublish", OpenWin::Cant(
            "the handle removal confirmation appears for an existing handle — the scene is in the \
             public parameters tests",
        ), Closes::Action),
        ("published_win", OpenWin::Cant(
            "the author’s list of handles opens from inside the reel by its address — the scene is in the \
             public parameters tests",
        ), Closes::Action),
    ]
}

fn first_clip(h: &egui_kittest::Harness<'static, crate::app::KtApp>) -> kt_model::ClipId {
    h.state().model.project().clips.first().map(|c| c.id).expect("an event is present in the scene")
}

/// Test 779 (R.66): the FORM CONTRACT — across all registry windows.
///
/// The owner’s direct request of R.64: “and do not forget to go over this in the future in all the other
/// widgets and windows too.” Prose did not hold it:
/// by R.66 body scrolling existed in EXACTLY ONE window out of twenty-one, and it appeared
/// only because t775 dropped a window with buttons hidden beyond the
/// edge. Here are two assertions, both mechanical:
///
/// 1. not a single own `DragValue` outside the shared widget — otherwise a number
///    acquires second limits and a second unit (a defect of R.33: Gain was
///    −24…24 in one window and −48…24 in another);
/// 2. every registry window that can be opened has its action button
///    VISIBLE and PRESSABLE in the low window — that is, the body scrolls, and the
///    buttons live outside the scroll.
#[test]
fn t779_window_form_contract() {
    // --- 1. not a single number outside the shared widget ------------------------
    let root = workspace_root();
    let dir = root.join("apps/kt-app/src");
    let mut bad: Vec<String> = Vec::new();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .expect("kt-app sources")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        // widgets.rs is the HOME of the row: that is where it is defined; the guard itself searches for this
        // name as a string and would count itself
        if name == "widgets.rs" || name == "reach_tests.rs" {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("the source is readable");
        // the test module is not counted: the text of assertions is not seen by a person
        let code = src.split("\n#[cfg(test)]").next().unwrap_or(&src);
        for (i, line) in code.lines().enumerate() {
            if line.contains("DragValue::new") {
                bad.push(format!("{name}:{}", i + 1));
            }
        }
    }
    let mut problems: Vec<String> = Vec::new();
    if !bad.is_empty() {
        problems.push(format!(
            "own numeric field bypassing the shared widget row (widgets::param_row / plain_row): {}. \
         Such a number has its own limits and its own unit — that is a second source of truth about them \
         (rule “one widget, not a copy”)",
            bad.join(", ")
        ));
    }

    // --- 2. action buttons are reachable in a LOW window ---------------------
    let openers = window_openers();
    let known: std::collections::HashSet<&str> = WINDOWS.iter().map(|(id, _)| *id).collect();
    for (id, _, _) in &openers {
        assert!(known.contains(id), "the window table has “{id}”, which is not in the WINDOWS registry");
    }
    for (id, _) in WINDOWS {
        assert!(
            openers.iter().any(|(o, _, _)| o == id),
            "window “{id}” is in the registry, but it is missing from the t779 table: \
             the form contract must be checked for EVERY window or have a reason"
        );
    }
    for (id, how, closes) in openers {
        let open = match how {
            OpenWin::Cant(why) => {
                assert!(
                    why.chars().count() >= 25,
                    "window “{id}”: the reason “{why}” is shorter than 25 characters — this way an excuse \
                     becomes cheaper than the check"
                );
                continue;
            }
            OpenWin::By(f) => f,
        };
        // a LOW window: the contents are deliberately above the screen
        let mut h = crate::layout_tests::harness_sized(1100.0, 420.0);
        open(&mut h);
        h.step();
        h.step();
        let screen = h.ctx.content_rect();
        match closes {
            Closes::Titlebar { title, why } => {
                assert!(
                    why.chars().count() >= 25,
                    "window “{id}”: the reason “{why}” is shorter than 25 characters"
                );
                // the title is drawn and visible — so the cross is reachable
                let t = title();
                use egui_kittest::kittest::Queryable;
                let seen = h
                    .query_all_by_label_contains(&t)
                    .map(|n| n.rect())
                    .any(|r| screen.contains_rect(r));
                if !seen {
                    problems.push(format!(
                        "“{id}”: the title “{t}” is not visible on screen — the cross that closes \
                         the window is unreachable"
                    ));
                }
            }
            Closes::ActionShort { why } => {
                assert!(why.chars().count() >= 25, "window “{id}”: the reason “{why}” is shorter than 25 characters");
                // “short” is measurable: as many rows as fit
                // without scrolling on any screen.
                use egui_kittest::kittest::{NodeT, Queryable};
                let rows = h
                    .query_all_by_label_contains("")
                    .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
                    .count();
                let mut bare = crate::layout_tests::harness_sized(1100.0, 420.0);
                bare.step();
                let base = bare
                    .query_all_by_label_contains("")
                    .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
                    .count();
                let own = rows.saturating_sub(base);
                if own > SHORT_WINDOW_ROWS {
                    problems.push(format!(
                        "“{id}” is declared short, but it shows {own} of its own rows (limit \
                         {SHORT_WINDOW_ROWS}) — so it needs scrolling"
                    ));
                }
                let labels = action_labels();
                let found = labels
                    .iter()
                    .flat_map(|l| h.query_all_by_label(l).map(|n| n.rect()).collect::<Vec<_>>())
                    .collect::<Vec<_>>();
                if !found.iter().any(|r| screen.contains_rect(*r)) {
                    problems.push(format!("“{id}”: action buttons are unreachable"));
                }
            }
            Closes::Action => {
                // The WINDOW BODY LIVES IN A SCROLL.
                // It is checked by the state of the scroll itself in egui’s memory:
                // the window is clamped by the screen and draws clipped nodes
                // with TRUNCATED rectangles —
                // neither a “node beyond the edge” nor “hidden rows” are visible in the
                // accessibility tree, so the consequence “part of the form is unreachable” is not observable
                // with the kittest instruments. Therefore here is an honest structural
                // check with a named reason: opening a window must create at least one
                // ScrollArea state (that it is not around empty space is held by the
                // live scenario `forms.py`, which turns the mouse wheel in the real
                // window), that is, the scroll really exists in the body.
                let scrolls = |h: &egui_kittest::Harness<'static, crate::app::KtApp>| {
                    h.ctx.memory(|m| m.data.count::<egui::scroll_area::State>())
                };
                let mut bare = crate::layout_tests::harness_sized(1100.0, 420.0);
                bare.step();
                let before = scrolls(&bare);
                let after = scrolls(&h);
                if after <= before {
                    problems.push(format!(
                        "“{id}”: the window body is not in a scroll area (ScrollArea states {after} versus \
                         {before} without the window) — the content that did not fit has nowhere to go"
                    ));
                }
                let labels = action_labels();
                let found: Vec<(String, egui::Rect)> = labels
                    .iter()
                    .flat_map(|l| {
                        use egui_kittest::kittest::Queryable;
                        h.query_all_by_label(l)
                            .map(move |n| (l.clone(), n.rect()))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                if found.is_empty() {
                    problems.push(format!(
                        "“{id}”: not a single action button was found in the window, and the title cross \
                         is not declared as a way to close it"
                    ));
                    continue;
                }
                if !found.iter().any(|(_, r)| screen.contains_rect(*r)) {
                    problems.push(format!(
                        "“{id}”: action buttons are beyond the screen edge {:?} — the window outgrew the screen \
                         and hid its actions (the body must scroll, the buttons must live outside \
                         the scroll)",
                        found.iter().map(|(l, r)| format!("{l} {r:?}")).collect::<Vec<_>>()
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "window form contract violated:\n  {}", problems.join("\n  "));
}

/// Test 783 (R.67): a project of an OLD version opens through its own door —
/// “File → Import project of an older version…” — while the ordinary “Open” rejects it
/// with the file’s version and the name of this door (rule 19: the dialog says where to
/// change the decision). The import repairs `targets: null` in the v5 morphers
/// (the form `untitled2_v26.09.26.otio`) and opens the document with the former path
/// and an asterisk. The reader is not touched by this — on the previous code
/// “Open” answered “metadata.kt is unreadable”, and there was no door.
#[test]
fn t783_older_project_goes_through_import_and_open_names_the_door() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    use crate::app::KtApp;
    use kt_model::{
        ClipEvent, ClipId, ClipKind, MediaId, MediaRef, MorphTargets, Project, ProjectSettings,
    };
    use kt_time::Tick;

    // --- fixture: v5 with null targets on a morpher ----------------------
    let dir = std::env::temp_dir().join(format!("kt-t783-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("old_v5.otio");
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
    p.media.push(MediaRef::new(MediaId(1), "synth:Ramp"));
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
    });
    p.validate().unwrap();
    let text = kt_otio::save_project(&p, &Default::default(), path.parent());
    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    old["metadata"]["kt"]["v"] = serde_json::json!(5);
    old["metadata"]["kt"]["morph_lanes"] = serde_json::json!(1);
    old["metadata"]["kt"]["morphers"] = serde_json::json!([{
        "id": 1, "lane": 0, "start": 0, "duration": 45000,
        "targets": null,
        "patch": { "media": 1, "source_in_delta": 4500 },
    }]);
    std::fs::write(&path, serde_json::to_string_pretty(&old).unwrap()).unwrap();

    // --- “Open” rejects with the version and the name of the door ---------------------
    let err = match KtApp::open(&path) {
        Ok(_) => panic!("v5 with null targets: ordinary Open must not read it"),
        Err(e) => e,
    };
    assert!(err.contains("5"), "error without the file version: {err}");
    assert!(
        err.contains(tr(k::menu::file::IMPORT_OLDER)),
        "error does not name the import item: {err}"
    );
    assert!(!err.contains("unreadable"), "the old text “unreadable” is not an address: {err}");

    // --- the door is in the “File” menu -----------------------------------
    let mut h = crate::layout_tests::harness();
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();
    h.step();
    let labels = crate::layout_tests::menu_labels(&mut h);
    assert!(
        h.query_by_label(tr(k::menu::file::IMPORT_OLDER)).is_some(),
        "the import item for an old version must be in the “File” menu; visible: {labels:?}"
    );
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();

    // --- import by path: the same body as the item (without the system dialog)
    h.state_mut().import_older_path(&path);
    h.step();
    h.step();
    let app = h.state();
    // R.70: the import gives an UNNAMED document of its own format with the hint
    // “save as <name>.kt alongside”; the source file is not the project’s path.
    assert_eq!(app.project_path, None, "after import the document has no path: the project file is not yet written");
    assert_eq!(
        app.save_hint.as_deref(),
        Some(path.with_extension("kt").as_path()),
        "save hint — .kt next to the source"
    );
    assert!(app.dirty_content, "after import the document is marked with edits (asterisk)");
    let doc = app.model.project();
    assert_eq!(doc.clips.len(), 1, "event arrived");
    assert_eq!(doc.morphers.len(), 1, "morpher arrived");
    assert_eq!(doc.morphers[0].targets, MorphTargets::Below, "null became “everything below”");
    let said: Vec<String> = app.notices.iter().map(|n| n.text.clone()).collect();
    assert!(
        said.iter().any(|t| t.contains("null → below")),
        "fix not named in notifications: {said:?}"
    );
    assert!(
        said.iter().any(|t| t.contains(&crate::tf!(k::msg::IMPORT_OLDER_DONE, 5))),
        "import did not report the version: {said:?}"
    );

    // --- saving writes ITS OWN format, and “Open” already reads it itself -------
    let out = crate::app::project_text_of(doc, &Default::default(), None);
    let out_path = dir.join("saved.kt");
    std::fs::write(&out_path, &out).unwrap();
    let reopened = KtApp::open(&out_path).expect("after import and save the file reads by itself");
    assert_eq!(reopened.model.project().morphers[0].targets, MorphTargets::Below);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 788 (R.67): the link door “Driven…” in the curve block — from the FX window:
/// “this event” → property → a link source with an address; the curve name carries the
/// address, “unbind” returns the keys. On the previous code there was no door: the model
/// and the link tests existed, but there was nothing to assign it to the person with.
#[test]
fn t788_link_door_assigns_a_source() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    let mut h = crate::layout_tests::harness_with_fx_window_two_slots();
    crate::layout_tests::reveal_curve_block(&mut h);
    h.get_by_label(tr(k::fx::DRIVE_BY)).click();
    h.step();
    h.step();
    // the submenu opens by a click (hovering in the harness does not expand it);
    // egui appends an arrow to the submenu button — we search by occurrence
    h.query_all_by_label_contains(tr(k::fx::DRIVE_THIS)).last().expect("“This event” submenu").click();
    h.step();
    h.step();
    // the second slot’s property — by the caption “Effect: parameter”
    let wanted = crate::widgets::param_label("radius_y");
    h.query_all_by_label_contains(&wanted).last().expect("property of the neighboring slot in the list").click();
    h.step();
    h.step();
    let c = h.state().model.project().clips[0].fx_auto[0][0].clone();
    match &c.source {
        kt_model::CurveSource::Link { path, .. } => {
            assert_eq!(path, "self/fx1/radius_y", "link address is wrong");
        }
        other => panic!("link not assigned: {other:?}"),
    }
    let texts: Vec<String> = painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(texts.iter().any(|t| t.contains("<- self/fx1/radius_y")), "curve name does not carry the address: {texts:?}");
    // “unbind” — the keys
    h.get_by_label(tr(k::fx::UNLINK)).click();
    h.step();
    h.step();
    assert!(h.state().model.project().clips[0].fx_auto[0][0].source.is_keys(), "“detach” did not restore the keys");
}

/// Test 789b (R.67): the formula help and the field itself are reachable from the FX window
/// (slot), from the generator form and from the head of a reel instance — the block is one.
#[test]
fn t789b_formula_help_reachable_from_every_carrier() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    // slot
    let h = crate::layout_tests::harness_with_fx_window();
    assert!(h.query_by_label(tr(k::fx::HELP)).is_some(), "slot has no help");
    assert!(h.query_by_label(tr(k::fx::DRIVE_BY)).is_some(), "slot has no link door");
    // reel instance
    let (mut h, s) = crate::layout_tests::harness_news_template();
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    h.get_by_label(tr(k::fx::AUTO_ADD)).click();
    h.step();
    h.step();
    h.query_all_by_label_contains("Font size").last().expect("“Font size” item").click();
    h.step();
    h.step();
    assert!(h.query_by_label(tr(k::fx::HELP)).is_some(), "instance has no help");
    assert!(h.query_by_label(tr(k::fx::DRIVE_BY)).is_some(), "instance has no link door");
}

/// Test 799 (R.67): the guard of the “USER PATH”. Every UI scenario with a kt-app test
/// from t783 on has a `PATH` line in its spec — what the person does and what they see;
/// a scenario without a gesture has the reason in words, not shorter than forty
/// characters. It is checked by RUNNING the script: it is also in `finish.sh`.
///
/// The owner: “HOW MANY TIMES DO I HAVE TO REPEAT SO THAT ALL AGENTS THINK ABOUT THE
/// USER FIRST? … THIS NEEDS RADICAL CHANGE!”.
#[test]
fn t799_ui_scenarios_describe_the_user_path() {
    if cfg!(windows) {
        eprintln!("t799 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    // CARGO_MANIFEST_DIR = src/apps/kt-app; the tree root is three levels up
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let script = root.join("src/scripts/specs-user-path.py");
    assert!(script.exists(), "guard is not in place: {}", script.display());

    let run = |dir: &std::path::Path| -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&script)
            .arg("--check")
            .current_dir(dir)
            .output()
            .expect("python3 for the spec guard");
        (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
    };

    // 1) on TODAY’S specs — green
    let (ok, said) = run(&root);
    assert!(ok, "the guard is red on the current specs: {said}");

    // 2) for a NEW scenario we delete the PATH line — it must turn red with
    //    the file address and the scenario name
    let specs: Vec<std::path::PathBuf> = ["ui-timeline", "ui-reach"]
        .iter()
        .flat_map(|cap| {
            let mut v = vec![root.join("openspec/specs").join(cap).join("spec.md")];
            if let Ok(rd) = std::fs::read_dir(root.join("openspec/changes")) {
                for e in rd.flatten() {
                    v.push(e.path().join("specs").join(cap).join("spec.md"));
                }
            }
            v
        })
        .filter(|p| p.exists())
        .collect();
    let victim = specs
        .iter()
        .find(|p| std::fs::read_to_string(p).unwrap_or_default().contains("- **PATH**"))
        .cloned()
        .expect("no spec has a PATH line — the rule does not apply at all");
    let orig = std::fs::read_to_string(&victim).unwrap();
    let broken = orig.replacen("- **PATH**", "- **WAS**", 1);
    std::fs::write(&victim, &broken).unwrap();
    let (ok_after, said_after) = run(&root);
    std::fs::write(&victim, &orig).unwrap();
    assert!(!ok_after, "the guard did NOT notice a scenario without a user PATH");
    assert!(
        said_after.contains("scenario") && said_after.contains("spec.md"),
        "the guard is silent about the address: {said_after}"
    );

    // 3) the guard itself stands in finish.sh — otherwise it is green and forgotten
    let finish = std::fs::read_to_string(root.join("src/scripts/finish.sh")).expect("finish.sh");
    assert!(
        finish.contains("specs-user-path.py"),
        "the guard is not called from finish.sh: the rule again holds only as prose"
    );
    assert!(
        finish.contains("live scenario steps"),
        "finish.sh does not print the list of PATH steps — the report again has no pictures"
    );
}

/// t800 (R.68): EVERY stage of `finish.sh` names its own cost, and the budget —
/// five minutes — is printed and flagged when exceeded.
///
/// The trigger, verbatim (the owner): “the tests in finish.sh or something like that —
/// take a very long time to run, we need to think of something”. A measurement showed a loop of
/// checks taking 21 minutes, and not one stage named its cost: there was nothing
/// to say about WHAT to speed up. A stage could be quietly moved out of the budget by printing its
/// heading past the wrapper — this is the same class as “red under someone else’s name” (R.55)
/// and “verdict at tail” (R.58), therefore the stage heading is printed only by the wrapper,
/// and the guard checks this.
#[test]
fn t800_finish_stages_name_their_price() {
    if cfg!(windows) {
        eprintln!("t800 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let script = root.join("src/scripts/finish-budget.py");
    let finish_path = root.join("src/scripts/finish.sh");
    assert!(script.exists(), "no cost accounting: {}", script.display());

    let run = |args: &[&str]| -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&script)
            .args(args)
            .current_dir(&root)
            .output()
            .expect("python3 for cost accounting");
        let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
        said.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.success(), said)
    };

    // 1) a self-check of the accounting: the overrun is named, the budget is named, a red
    //    stage is named red, a broken accounting turns red
    let (ok, said) = run(&["--selftest"]);
    assert!(ok, "the cost accounting self-check is red: {said}");

    // 2) every stage of the script goes through the timing wrapper
    let (ok, said) = run(&["--check-script", "src/scripts/finish.sh"]);
    assert!(ok, "the stage prints a header outside the measurement: {said}");

    // 3) a heading placed outside the wrapper is caught on the REAL script, not only
    //    on synthetic input: without this check the guard could look in the wrong place
    let finish = std::fs::read_to_string(&finish_path).expect("finish.sh");
    let broken = format!("{finish}\necho \"== stage outside the budget ==\"\n");
    let tmp = root.join("src/target/t800-finish-probe.sh");
    std::fs::create_dir_all(tmp.parent().unwrap()).ok();
    std::fs::write(&tmp, &broken).unwrap();
    let (ok_after, said_after) = run(&["--check-script", "src/target/t800-finish-probe.sh"]);
    std::fs::remove_file(&tmp).ok();
    assert!(!ok_after, "the guard did NOT notice a stage outside the measurement");
    assert!(
        said_after.contains("outside the stage wrapper") && said_after.contains("stage outside the budget"),
        "the guard is silent about the stage address: {said_after}"
    );

    // 4) the script really calls the accounting and holds the stages named by the spec:
    //    the workshop run is a stage of THIS script, a separate kt-gpu stage
    //    was removed as a duplicate (R.68), SIMD stays with its own environment
    for (need, why) in [
        ("finish-budget.py --report", "the script does not print the table of stage costs"),
        ("workshop tests", "the workshop run did not become a script stage"),
        ("SIMD conformance", "SIMD stage disappeared"),
        ("live scenarios", "live scenario stage disappeared"),
    ] {
        assert!(finish.contains(need), "{why} (no “{need}”)");
    }
    assert!(
        !finish.contains("cargo test -p kt-gpu"),
        "the kt-gpu stage remained a duplicate of the workshop run (R.68)"
    );
}

/// Is there a live check on the machine? Without Xvfb and a built program there is nowhere
/// to run the guard scenarios — the test is skipped, NAMING the reason (a vacuous skip
/// without a reason is “green because nothing was checked”, lesson of t194).
fn live_check_available(root: &std::path::Path) -> Option<String> {
    if std::process::Command::new("sh")
        .args(["-c", "command -v Xvfb"])
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        return Some("no Xvfb (sudo apt install xvfb openbox)".into());
    }
    if !root.join("target/debug/kinetren").exists() {
        return Some("no target/debug/kinetren (cargo build -p kt-app)".into());
    }
    None
}

fn run_live_check(
    root: &std::path::Path,
    workers: &str,
    display: &str,
    names: &[&str],
) -> (bool, String) {
    let out = std::process::Command::new("./scripts/live-check.sh")
        .args(names)
        .env("KT_LIVE_WORKERS", workers)
        // its own run directory: a real run of live scenarios may go on alongside, and
        // `live-check.sh` cleans the directory of verdicts
        .env("KT_LIVE_OUT", root.join("target/live-guard").to_string_lossy().as_ref())
        // its own screen numbers for EACH test: the tests run in parallel, and two runs on the same
        // screens would open windows on each other
        .env("KT_LIVE_DISPLAY", display)
        .current_dir(root)
        .output()
        .expect("scripts/live-check.sh");
    let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
    said.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), said)
}

/// t801 (R.68): a red in parallel is re-run ALONE, and only the
/// second failure is the verdict.
///
/// The rule “a test that fails under load is re-run separately” has been recorded since R.59
/// (§468: a second run alongside turned green scenarios red), but it was executed by
/// the agent’s memory. The owner allowed parallelism together with the mechanism —
/// so the mechanism must be checkable, and the re-run must be VISIBLE in the report:
/// a silent re-run would turn a flake into an invisible thing.
#[test]
fn t801_red_scenario_is_rerun_alone() {
    let root = workspace_root();
    if let Some(why) = live_check_available(&root) {
        eprintln!("t801 skipped: {why}");
        return;
    }
    // the run remembers the fake “red the first time” in a file — we remove it
    let mark = root.join("target/live-guard/_flaky/already-ran");
    std::fs::remove_file(&mark).ok();

    let (ok, said) = run_live_check(&root, "2", ":80", &["_flaky", "_red"]);
    assert!(!ok, "a run with an always-red scenario must be red:\n{said}");
    assert!(
        said.contains("PASSED ON THE SECOND TRY") && said.contains("_flaky"),
        "a solo rerun is not named in the report:\n{said}"
    );
    assert!(
        said.contains("RED TWICE") && said.contains("_red"),
        "a scenario that failed twice is not named red:\n{said}"
    );
    let verdict = std::fs::read_to_string(root.join("target/live-guard/verdicts/_flaky"))
        .unwrap_or_default();
    assert_eq!(verdict, "ok", "the re-run scenario stayed red");

    // a single run re-runs NOTHING: there a red result is the verdict
    std::fs::remove_file(&mark).ok();
    let (ok1, said1) = run_live_check(&root, "1", ":80", &["_flaky"]);
    assert!(!ok1, "a solo run must report red as it is:\n{said1}");
    assert!(
        said1.contains("the run is already single-process"),
        "a solo run silently re-ran the scenario:\n{said1}"
    );
}

/// t802 (R.68): the workers do not see each other — their own screen and their own storage.
///
/// This is precisely the closed cause of the ban on “two threads”: two kt-app on one
/// screen is aiming at someone else’s window (`probe.window()` searches by WM_CLASS), and a shared
/// storage mixes the workplace, the presets and the locks.
#[test]
fn t802_workers_are_isolated() {
    let root = workspace_root();
    if let Some(why) = live_check_available(&root) {
        eprintln!("t802 skipped: {why}");
        return;
    }
    let (ok, said) = run_live_check(&root, "3", ":84", &["_env1", "_env2", "_env3"]);
    assert!(ok, "stand-in scenarios must be green:\n{said}");
    let mut displays = std::collections::BTreeSet::new();
    let mut stores = std::collections::BTreeSet::new();
    for n in ["_env1", "_env2", "_env3"] {
        let p = root.join("target/live-guard").join(n).join("env.json");
        let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let get = |key: &str| -> String {
            text.split(&format!("\"{key}\": \""))
                .nth(1)
                .and_then(|s| s.split('"').next())
                .unwrap_or_default()
                .to_string()
        };
        displays.insert(get("display"));
        stores.insert(get("store"));
    }
    assert_eq!(displays.len(), 3, "workers share the display: {displays:?}");
    assert_eq!(stores.len(), 3, "workers share the store: {stores:?}");
    assert!(
        said.contains("workers 3"),
        "the run did not say how many workers it used:\n{said}"
    );
}

/// t803 (R.68): live scenarios wait for the PROGRAM, not for the clock.
///
/// Measurement of the round: 96% of a live run’s time is unconditional sleep (`nestparam`:
/// 130.5 s of 135.9 in 591 calls). The rule in prose would hold here only until the
/// next scenario, so it is held by the guard: raw pauses do not exist in scenarios; in the harness
/// they live in `pause`/`_poll`, and each `pause` has a reason in words not shorter than forty
/// characters (the same measure as the PATH guard).
#[test]
fn t803_live_scenarios_wait_for_the_program() {
    if cfg!(windows) {
        eprintln!("t803 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let root = workspace_root();
    let guard = root.join("scripts/live-sleeps.py");
    assert!(guard.exists(), "guard is not in place: {}", guard.display());
    // OWN COPY OF THE TREE (R.69): the test checks the guards by VIOLATIONS, while the working tree is
    // read at the same time by a neighbouring test (t808). A stand-in scenario placed in the working
    // `scripts/live` made the neighbour’s verdict random —
    // the very “flake by construction” that this round removes.
    let sandbox = std::env::temp_dir().join(format!("kt-t803-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&sandbox);
    std::fs::create_dir_all(sandbox.join("live")).unwrap();
    for entry in std::fs::read_dir(root.join("scripts/live")).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "py") {
            std::fs::copy(entry.path(), sandbox.join("live").join(entry.file_name())).unwrap();
        }
    }
    std::fs::copy(root.join("scripts/ui-probe.py"), sandbox.join("ui-probe.py")).unwrap();
    let run = |scripts: &std::path::Path| -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&guard)
            .arg("--check")
            .arg("--scripts")
            .arg(scripts)
            .current_dir(&root)
            .output()
            .expect("python3 for the pause guard");
        let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
        said.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.success(), said)
    };
    let real = root.join("scripts");

    let (ok, said) = run(&real);
    assert!(ok, "the guard is red on today’s scenarios: {said}");

    // a raw pause in a SCENARIO — red with the address
    let probe = sandbox.join("live/_t803_probe.py");
    std::fs::write(&probe, "import time\n\n\ndef main(out):\n    time.sleep(0.5)\n").unwrap();
    let (ok_raw, said_raw) = run(&sandbox);
    std::fs::remove_file(&probe).ok();
    assert!(!ok_raw, "the guard did not notice a raw pause in a scenario");
    assert!(
        said_raw.contains("_t803_probe.py") && said_raw.contains("raw `time.sleep`"),
        "the guard is silent about the pause address: {said_raw}"
    );

    // a pause without a clear reason — also red
    std::fs::write(
        &probe,
        "from kt_live import pause\n\n\ndef main(out):\n    pause(0.1, \"required\")\n",
    )
    .unwrap();
    let (ok_short, said_short) = run(&sandbox);
    std::fs::remove_file(&probe).ok();
    assert!(!ok_short, "the guard accepted an excuse instead of a pause reason");
    assert!(
        said_short.contains("the reason for the pause is shorter than"),
        "the guard did not name the short reason: {said_short}"
    );

    // the working tree stays untouched all this time
    let (ok_real, said_real) = run(&real);
    assert!(ok_real, "the working tree turned red from the guard test: {said_real}");
    let _ = std::fs::remove_dir_all(&sandbox);
}

/// t805 (R.68): the channel of the rig is dead without the environment variable; frames are counted
/// by the drawing, and “clean” means zero open windows.
///
/// The rig opens projects one after another in one process, and the proof of cleanliness
/// must be observable BEFORE the first gesture: otherwise a state leak between
/// scenarios gives “green because we looked in the wrong place”.
#[test]
fn t805_bench_channel_is_dead_without_env() {
    // 1) the product has no channel: no variable — no commands
    assert!(
        !crate::bench::enabled(),
        "bench channel is enabled without KT_LIVE_CTL — in the product it must be dead"
    );
    let dir = std::env::temp_dir().join("kt-t805");
    std::fs::create_dir_all(&dir).ok();
    let ctl = dir.join("ctl");
    std::fs::write(&ctl, "1 open /tmp/no-such.otio\n").unwrap();
    assert!(crate::bench::poll().is_none(), "the disabled channel read a command");
    std::fs::remove_file(&ctl).ok();

    // 2) the frame number grows FROM THE DRAWING
    let mut h = crate::layout_tests::harness();
    let before = crate::bench::frames();
    h.step();
    h.step();
    let after = crate::bench::frames();
    assert!(
        after >= before + 2,
        "the frame counter does not count drawing: {before} → {after}"
    );

    // 3) “clean” is zero windows; an open window is visible, a closed one is not
    assert_eq!(
        crate::bench::open_windows(&h.ctx),
        0,
        "a fresh program reported open windows"
    );
    h.state_mut().open_project_settings();
    h.step();
    h.step();
    assert!(
        crate::bench::open_windows(&h.ctx) >= 1,
        "the open settings window is not visible to the window counter"
    );
}

/// t804 (R.68): the live coverage report — what the person has NOT touched with the mouse even once.
///
/// The owner’s decision: “click through the complicated moments, or those nobody has clicked even once.”
/// To keep “even once” from being a matter of taste, the machine counts it: the program
/// publishes the rectangles of named widgets, the scenarios click them, and the report
/// names the difference. Only FALSE things make the report red: a scenario aims at a place
/// that does not exist, or a spec refers to a live scenario that is gone — that is, a check
/// “moved” into the headless harness and dropped there,
/// instead of having its address replaced.
///
/// **The move names the NEW ADDRESS** (R.68, task 5.2). The `MIGRATED` registry in the
/// guard is the list “scenario → the tests that hold its assertions”;
/// the guard requires that the scenario file be gone AND that every named test exist
/// in the sources. Without the registry, “move to non-gui” and “delete” are
/// indistinguishable by the tree — exactly the substitution that the rule
/// “tests from specs are not weakened” protects against.
#[test]
fn t804_live_coverage_names_untouched_places() {
    if cfg!(windows) {
        eprintln!("t804 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let root = workspace_root();
    let guard = root.join("scripts/live-coverage.py");
    assert!(guard.exists(), "guard is not in place: {}", guard.display());
    let run = |extra: &[&str]| -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&guard)
            .args(extra)
            .current_dir(&root)
            .output()
            .expect("python3 for the coverage report");
        let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
        said.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.success(), said)
    };

    // 1) today’s tree: the report names the number of untouched places
    let (ok, said) = run(&["--check"]);
    assert!(ok, "the coverage guard is red on the current tree: {said}");
    assert!(
        said.contains("never touched with the mouse") || said.contains("NEVER touched with the mouse"),
        "the report did not name the untouched places: {said}"
    );

    // 2) a scenario aims at a place the program does not publish
    let probe = root.join("scripts/live/_t804_probe.py");
    std::fs::write(
        &probe,
        "def main(out):\n    app.click_probe(\"nonexistentprobe.at_all\", \"place\")\n",
    )
    .unwrap();
    let (ok_phantom, said_phantom) = run(&["--check"]);
    std::fs::remove_file(&probe).ok();
    assert!(!ok_phantom, "the guard did not notice aiming at a nonexistent place");
    assert!(
        said_phantom.contains("nonexistentprobe"),
        "the guard did not name the nonexistent place: {said_phantom}"
    );

    // 3) a live scenario named by the spec has disappeared from disk — red:
    //    the check’s move MUST change the address, not drop the scenario
    let victim = root.join("scripts/live/deadend.py");
    let saved = std::fs::read_to_string(&victim).expect("live scenario deadend.py");
    std::fs::remove_file(&victim).unwrap();
    let (ok_gone, said_gone) = run(&["--check"]);
    std::fs::write(&victim, saved).unwrap();
    assert!(!ok_gone, "the guard did not notice a vanished scenario of the spec");
    assert!(
        said_gone.contains("deadend.py"),
        "the guard did not name the vanished scenario: {said_gone}"
    );

    // 4) the report names the move registry and its new addresses
    let (_, full) = run(&[]);
    assert!(
        full.contains("moved to headless"),
        "the report did not name which checks moved to headless: {full}"
    );

    // 5) a scenario declared moved has returned to disk — red:
    //    otherwise the registry would diverge from the tree and stop meaning anything
    //    The scenario is added to a COPY of the folder (KT_LIVE_DIR): writing into the real
    //    one raced with t808, which globs it in parallel (seen on the CI runner).
    let live_copy = std::env::temp_dir().join(format!("kt-t804-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&live_copy);
    std::fs::create_dir_all(&live_copy).unwrap();
    for e in std::fs::read_dir(root.join("scripts/live")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "py") {
            std::fs::copy(&p, live_copy.join(p.file_name().unwrap())).unwrap();
        }
    }
    std::fs::write(live_copy.join("nestzero.py"), "if __name__ == \"__main__\":\n    pass\n").unwrap();
    let out = std::process::Command::new("python3")
        .arg(&guard)
        .arg("--check")
        .env("KT_LIVE_DIR", &live_copy)
        .output()
        .expect("python3 for the coverage report");
    let (ok_back, said_back) = (
        out.status.success(),
        format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)),
    );
    let _ = std::fs::remove_dir_all(&live_copy);
    assert!(!ok_back, "the guard did not notice a returned scenario that had moved");
    assert!(
        said_back.contains("nestzero.py"),
        "the guard did not name the returned scenario: {said_back}"
    );
}

/// t806 (R.68, task 5.3): the DRIVE VERBS of the track have a live aim.
///
/// The owner’s decision in this round: “click through the complicated moments or those
/// nobody has clicked even once”. The live coverage report named ten such places,
/// and all ten are a second path to what `track.py` does with badges: the menu items of
/// the row and the controls of the track window. The live scenario
/// `trackverbs.py` clicked them through.
///
/// The guard holds what has been reached from BOTH sides, because they can diverge
/// in different ways:
///
/// * the program stopped publishing the rectangle of an item — the live
///   scenario has nothing to aim at, and it turns red with “the program gave up
///   the rectangle”, without saying that the cause is the removed probe;
/// * the scenario stopped pressing the item — the coverage quietly moved back, and
///   this could be noticed only by comparing two reports.
///
/// t741 next to it checks ANOTHER thing: that the set of items is complete and that an item with
/// a badge leads to one command. It looks at the captions and does not know whether
/// the item has an address for the mouse.
#[test]
fn t806_track_verbs_have_a_live_aim() {
    let root = workspace_root();
    // “a place of the program” → where the program publishes it
    let places: [(&str, &str); 10] = [
        ("track.menu.props", "timeline.rs"),
        ("track.menu.mute", "timeline.rs"),
        ("track.menu.solo", "timeline.rs"),
        ("track.menu.lock", "timeline.rs"),
        ("track.menu.fx", "timeline.rs"),
        ("track.mute", "track_window.rs"),
        ("track.solo", "track_window.rs"),
        ("track.lock", "track_window.rs"),
        ("track.blend.", "track_window.rs"),
        ("track.cancel", "track_window.rs"),
    ];
    let scenario_path = root.join("scripts/live/trackverbs.py");
    let scenario = std::fs::read_to_string(&scenario_path)
        .unwrap_or_else(|e| panic!("live track-verbs scenario is absent: {e}"));
    let mut problems: Vec<String> = Vec::new();
    for (name, file) in places {
        let src = std::fs::read_to_string(root.join("apps/kt-app/src").join(file))
            .unwrap_or_else(|e| panic!("cannot read {file}: {e}"));
        if !src.contains(&format!("\"{name}")) {
            problems.push(format!(
                "the program no longer publishes “{name}” to {file} — the live \
                 track-verbs scenario has nothing to aim at"
            ));
        }
        if !scenario.contains(&format!("\"{name}")) {
            problems.push(format!(
                "live scenario trackverbs.py does not press “{name}” — coverage slipped back \
                 silently (R.68, task 5.3)"
            ));
        }
    }
    assert!(problems.is_empty(), "track verbs without a live aim:\n  {}", problems.join("\n  "));
}

/// t808 (R.69): sleep is enumerated in ALL the live check and has a limit.
///
/// Two defects of the rule, found by the R.69 measurement, both about the rule
/// not holding what it promised:
///
/// * the pause guard looked at the harness and the scenarios and did NOT look at the mouse and keyboard
///   driver (`scripts/ui-probe.py`) — 123 s of the 387 s of sleep of the whole live
///   run lived outside its scope. Sleep hidden in the driver is no different
///   from sleep in the harness, and more dangerous, because it lives in a DEFAULT VALUE
///   of an argument: at the site of the gesture it is not seen at all;
/// * “a reason in words” proved a weak measure: all 265 s of sleep in the harness
///   were legitimate; each pause had an explanation longer than forty characters.
///   Therefore a ratchet limit appeared on the sum of the DECLARED seconds, like
///   `UNTOUCHED_LIMIT` in the coverage report (R.68).
///
/// The limit is static not by accident: it depends neither on the machine
/// nor on how many times the gesture was called. A verdict on the ACTUAL sleep would catch the load
/// instead of a regression — the actual number is printed by the run (`live-check.sh`).
#[test]
fn t808_live_sleep_is_enumerable_everywhere_and_capped() {
    if cfg!(windows) {
        eprintln!("t808 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let root = workspace_root();
    let guard = root.join("scripts/live-sleeps.py");
    let driver = root.join("scripts/ui-probe.py");
    assert!(guard.exists() && driver.exists(), "guard or driver is not in place");
    // OWN COPY OF THE TREE: the test checks the guards by VIOLATIONS, while the working tree
    // is read at the same time by a neighbouring test (t803) — by its own process under nextest
    // or by its own thread under `cargo test`. Corrupting a shared file would make the neighbour’s
    // verdict random; this is exactly the “flake by construction” that the round removes,
    // so the rule extends to its own tests too.
    let sandbox = std::env::temp_dir().join(format!("kt-t808-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&sandbox);
    std::fs::create_dir_all(sandbox.join("live")).unwrap();
    for entry in std::fs::read_dir(root.join("scripts/live")).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "py") {
            std::fs::copy(entry.path(), sandbox.join("live").join(entry.file_name())).unwrap();
        }
    }
    let sandbox_driver = sandbox.join("ui-probe.py");
    std::fs::copy(&driver, &sandbox_driver).unwrap();
    let run = |scripts: &std::path::Path| -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&guard)
            .arg("--check")
            .arg("--scripts")
            .arg(scripts)
            .current_dir(&root)
            .output()
            .expect("python3 for the pause guard");
        let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
        said.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.success(), said)
    };
    let real = root.join("scripts");

    // 1) today’s tree is green, and the guard NAMES the budget and the driver’s sleep
    let (ok, said) = run(&real);
    assert!(ok, "guard is red on the current tree: {said}");
    assert!(
        said.contains("declared sleep of the live check") && said.contains("against a limit of"),
        "the guard did not name the sum of the declared sleep and the limit: {said}"
    );
    assert!(
        said.contains("in the driver"),
        "the guard did not name the sleep of the mouse and keyboard driver: {said}"
    );

    // the copy is clean in exactly the same way as the working tree
    let (ok_copy, said_copy) = run(&sandbox);
    assert!(ok_copy, "the tree copy is red before any damage: {said_copy}");

    // 2) a raw pause IN THE DRIVER — red with the address (previously invisible)
    let saved = std::fs::read_to_string(&sandbox_driver).expect("driver is readable");
    std::fs::write(
        &sandbox_driver,
        format!("{saved}\n\ndef _t808_probe():\n    time.sleep(0.5)\n"),
    )
    .unwrap();
    let (ok_drv, said_drv) = run(&sandbox);
    std::fs::write(&sandbox_driver, &saved).unwrap();
    assert!(!ok_drv, "the guard did not notice a raw pause in the driver");
    assert!(
        said_drv.contains("ui-probe.py") && said_drv.contains("_t808_probe"),
        "the guard did not name the pause address in the driver: {said_drv}"
    );

    // 3) sleep above the declared limit — red with the most expensive pause
    let bumped = saved.replace("def move(x, y, pause=0.05):", "def move(x, y, pause=9.5):");
    assert_ne!(bumped, saved, "scene: the driver’s pause default is found");
    std::fs::write(&sandbox_driver, &bumped).unwrap();
    let (ok_budget, said_budget) = run(&sandbox);
    assert!(!ok_budget, "the guard missed a sleep above the declared limit");
    assert!(
        said_budget.contains("exceeded the limit") && said_budget.contains("the most expensive pause"),
        "the guard did not name the overspend and its source: {said_budget}"
    );

    // the working tree stays untouched all this time
    let (ok_real, said_real) = run(&real);
    assert!(ok_real, "the working tree turned red from the guard test: {said_real}");
    let _ = std::fs::remove_dir_all(&sandbox);
}

/// t807 (R.69): THE GESTURE ITSELF waits for input read by the program, not for the clock.
///
/// R.68 built the wait on an observed event (`wait_frame`,
/// `wait_seen`, `settle`) and brought it to the PROBES — but the gestures themselves stayed on
/// the clock: 539 s of actual sleep out of a 1587 s run. The difference is not only in
/// minutes: a fixed pause does not stretch under load, and this is exactly the
/// class of false reds because of which the parallelism ban lived through
/// nine rounds (§468).
///
/// The guard is needed because in prose the rule has already failed to hold once:
/// the decision “wait for the event” was recorded in R.68 and did not reach the gestures.
/// The same guard also caught a real loss in this round — the body of a drag, left
/// accidentally unchanged after bisection, when all runs were green.
#[test]
fn t807_a_gesture_waits_for_input_the_program_has_read() {
    if cfg!(windows) {
        eprintln!("t807 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let root = workspace_root();
    let guard = root.join("scripts/live-sleeps.py");
    let framework = root.join("scripts/live/kt_live.py");
    assert!(guard.exists() && framework.exists(), "guard or framework is not in place");
    // its own copy of the tree: the test breaks the harness on purpose, while nearby the same guard
    // runs t803 and t808 (see their analysis — a shared file made the neighbour’s verdict
    // random)
    let sandbox = std::env::temp_dir().join(format!("kt-t807-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&sandbox);
    std::fs::create_dir_all(sandbox.join("live")).unwrap();
    for entry in std::fs::read_dir(root.join("scripts/live")).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "py") {
            std::fs::copy(entry.path(), sandbox.join("live").join(entry.file_name())).unwrap();
        }
    }
    std::fs::copy(root.join("scripts/ui-probe.py"), sandbox.join("ui-probe.py")).unwrap();
    let sandbox_fw = sandbox.join("live/kt_live.py");
    let run = |scripts: &std::path::Path| -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&guard)
            .arg("--check")
            .arg("--scripts")
            .arg(scripts)
            .current_dir(&root)
            .output()
            .expect("python3 for the expectations guard");
        let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
        said.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.success(), said)
    };

    // 1) today’s harness is green: every gesture waits for input
    let (ok, said) = run(&root.join("scripts"));
    assert!(ok, "guard is red on the current framework: {said}");

    let saved = std::fs::read_to_string(&sandbox_fw).expect("framework is readable");

    // 2) a gesture stopped waiting for input — red WITH THE GESTURE’S NAME
    let deaf = saved.replace(
        "        self._after_gesture(mark, \"click\")",
        "        pass  # t807: gesture went deaf",
    );
    assert_ne!(deaf, saved, "scene: click wait found in the framework");
    std::fs::write(&sandbox_fw, &deaf).unwrap();
    let (ok_deaf, said_deaf) = run(&sandbox);
    assert!(!ok_deaf, "the guard did not notice a gesture that does not wait for the program");
    assert!(
        said_deaf.contains("`click`") && said_deaf.contains("_after_gesture"),
        "the guard did not name the deafened gesture: {said_deaf}"
    );

    // 3) a fixed pause INSIDE a gesture — red, even with a long reason
    let slept = saved.replace(
        "        self._after_gesture(mark, \"click\")",
        "        self._after_gesture(mark, \"click\")\n        pause(0.35, \
         \"t807: detailed reason longer than forty characters, and it is still a sleep\")",
    );
    std::fs::write(&sandbox_fw, &slept).unwrap();
    let (ok_slept, said_slept) = run(&sandbox);
    std::fs::write(&sandbox_fw, &saved).unwrap();
    assert!(!ok_slept, "the guard accepted a fixed pause inside a gesture");
    assert!(
        said_slept.contains("inside the gesture `click`"),
        "the guard did not name the place of the pause: {said_slept}"
    );

    // the working tree untouched
    let (ok_real, said_real) = run(&root.join("scripts"));
    assert!(ok_real, "the working tree turned red from the guard test: {said_real}");
    let _ = std::fs::remove_dir_all(&sandbox);
}

/// t809 (R.69): a screen point is read via X, while the snapshot remains the REFERENCE.
///
/// The colour of one point cost a full 1920×1080 screen snapshot via `xwd` and two
/// ImageMagick processes — 84 ms per read with ≈341 reads per live run.
/// A direct `XGetImage` gives the same in 0.04 ms.
///
/// The direct path cannot be taken on faith, and this is not caution: the eframe window
/// has a 32-bit visual, and a read of the root window by ImageMagick had already
/// returned BLACK where the picture was (R.33 item 228) — the scenario then
/// compared black with black and “passed”. Therefore the reference stays in the code
/// and stays the measure: a divergence sends the read back to it and says so.
///
/// The test checks BOTH halves: that the verification mechanism and the fallback are in place
/// (statically), and that the two paths give one colour in one frame (by a run
/// on the virtual screen).
#[test]
fn t809_a_screen_point_is_read_from_x_with_the_shot_as_reference() {
    if cfg!(windows) {
        eprintln!("t809 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let root = workspace_root();
    let driver = std::fs::read_to_string(root.join("scripts/ui-probe.py")).unwrap();
    let fw = std::fs::read_to_string(root.join("scripts/live/kt_live.py")).unwrap();

    // 1) the direct path is in the driver, while the reference and the verification are in the harness
    assert!(
        driver.contains("def pixels("),
        "the driver has no direct point read — the color again costs a snapshot of the whole screen"
    );
    for (what, needle) in [
        ("direct read", "_pixel_direct"),
        ("reference via xwd", "_pixel_shot"),
        ("check against the reference", "_pixel_path"),
    ] {
        assert!(fw.contains(needle), "not in the framework: {what} (`{needle}`)");
    }
    assert!(
        fw.contains("the whole run reads by the reference"),
        "a divergence from the reference must be NAMED, not silently switch the path"
    );

    // 2) the two paths give ONE colour in one frame
    if let Some(why) = live_check_available(&root) {
        eprintln!("t809: check of the read paths skipped — {why}");
        return;
    }
    let script = r##"
import subprocess, sys, time, os, importlib.util
disp = ":%d" % (90 + os.getpid() % 8)
xv = subprocess.Popen(["Xvfb", disp, "-screen", "0", "800x600x24", "-nolisten", "tcp"],
                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
os.environ["DISPLAY"] = disp
try:
    for _ in range(40):
        if subprocess.run(["xdpyinfo"], capture_output=True).returncode == 0:
            break
        time.sleep(0.25)
    spec = importlib.util.spec_from_file_location("ui_probe", sys.argv[1])
    probe = importlib.util.module_from_spec(spec); spec.loader.exec_module(probe)
    # SOMETHING NOT BLACK on screen: otherwise the check “black against black”
    # is green even on a broken read — exactly the trap this test
    # guards against (R.33 item 228). We draw it ourselves, without external utilities.
    root = probe.d.screen().root
    gc = root.create_gc(foreground=0x3070C0, background=0)
    root.fill_rectangle(gc, 0, 0, 800, 600)
    probe.d.sync()
    time.sleep(0.4)
    direct = probe.pixels(400, 300)[0]
    xwd = subprocess.run(["xwd", "-root", "-display", disp], capture_output=True).stdout
    png = "/tmp/_t809_%d.png" % os.getpid()
    subprocess.run(["convert", "xwd:-", png], input=xwd, capture_output=True, check=True)
    out = subprocess.run(["convert", png, "-format", "%[pixel:p{400,300}]", "info:-"],
                         capture_output=True, text=True, check=True).stdout
    os.unlink(png)
    nums = [int(v) for v in out.replace("(", ",").replace(")", ",").split(",")
            if v.strip().isdigit()][:3]
    print("direct=%s reference=%s" % (list(direct), nums))
    if sum(direct) == 0:
        raise SystemExit("direct read returned BLACK where the picture is")
    if any(abs(a - b) > 8 for a, b in zip(direct, nums)):
        raise SystemExit("paths diverged: %s versus %s" % (list(direct), nums))
finally:
    xv.terminate()
"##;
    let out = std::process::Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(root.join("scripts/ui-probe.py"))
        .current_dir(&root)
        .output()
        .expect("python3 for the read-path check");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "direct read diverged from the reference: {said}");
    assert!(said.contains("direct="), "the check did not name the colors read: {said}");
}

// ---- R.70: the project-file doors ----------------------------------------------

/// The shared scene of the doors: a project with a reel in two instances — so that the export
/// has something to check self-sufficiency against.
fn r70_project() -> kt_model::Project {
    use kt_model::*;
    use kt_time::Tick;
    let clip = |id: u64, media: u64, start: i64, dur: i64| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
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
    p.media.push(MediaRef::new(MediaId(1), "synth:Ramp"));
    let mut s = Sequence::new(MediaId(100), "Intro");
    s.clips.push(clip(11, 1, 0, 45_000));
    s.clips.push(clip(12, 1, 45_000, 45_000));
    p.sequences.push(s);
    p.clips.push(clip(1, 100, 0, 90_000));
    p.clips.push(clip(2, 100, 90_000, 90_000));
    p.normalize();
    p.validate().unwrap();
    p
}

/// A foreign exchange file with fields we do not know: exactly what must
/// come back on export.
fn r70_foreign_otio() -> String {
    serde_json::json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": "foreign",
        "vendor_field": {"deep": [1, 2]},
        "metadata": {"resolve": {"colorVersion": 3}},
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "children": [{
                "OTIO_SCHEMA": "Track.1",
                "kind": "Video",
                "children": [{
                    "OTIO_SCHEMA": "Clip.2",
                    "name": "known",
                    "custom_clip_field": 42,
                    "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                        "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 90000.0},
                        "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 90000.0, "rate": 90000.0}},
                    "media_references": {"DEFAULT_MEDIA": {
                        "OTIO_SCHEMA": "ExternalReference.1", "target_url": "synth:Ramp",
                        "available_range": null}},
                    "active_media_reference_key": "DEFAULT_MEDIA"
                }]
            }]
        }
    })
    .to_string()
}

fn r70_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kt-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Test 818 (R.70): “Open” accepts ONLY its own format. An exchange file and
/// a legacy file are rejected with the name of the import item in the text (rule 19), a file
/// of a future version — with the version number and without a partial read.
#[test]
fn t818_open_accepts_only_the_project_format_and_names_the_import_door() {
    use crate::i18n::{k, tr};
    let dir = r70_dir("t818");
    let p = r70_project();

    // its own format — opens, model field-by-field
    let ours = dir.join("film.kt");
    std::fs::write(&ours, crate::app::project_text_of(&p, &Default::default(), None)).unwrap();
    let app = crate::app::KtApp::open(&ours).expect("own file must open");
    assert_eq!(app.model.project().clips, p.clips);
    assert_eq!(app.model.project().sequences.len(), 1);
    assert_eq!(app.project_path.as_deref(), Some(ours.as_path()));
    drop(app);

    // an exchange file (our own export!) — refusal with the door, not a read
    let otio = dir.join("film.otio");
    std::fs::write(&otio, kt_otio::save_project(&p, &Default::default(), None)).unwrap();
    let err = crate::app::KtApp::open(&otio).err().expect("Open must not read an interchange file");
    assert!(err.contains(tr(k::menu::file::IMPORT_OLDER)), "error does not name the import door: {err}");
    assert!(err.contains("OpenTimelineIO"), "error does not say WHICH file this is: {err}");

    // legacy .ktproj — refusal with the same door, in its own text
    let legacy = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo.ktproj"));
    let err2 = crate::app::KtApp::open(legacy).err().expect("Open must not read a legacy file");
    assert!(err2.contains(tr(k::menu::file::IMPORT_OLDER)), "error does not name the import door: {err2}");
    assert!(err2.contains(".ktproj"), "error does not say this is legacy: {err2}");
    assert_ne!(err, err2, "two different files — two different texts");

    // a future version — the version number in the text, no partial read
    let text = std::fs::read_to_string(&ours).unwrap();
    let newer = text.replacen("\"kinetren\": 1", "\"kinetren\": 99", 1);
    assert_ne!(newer, text);
    let future = dir.join("future.kt");
    std::fs::write(&future, newer).unwrap();
    let err3 = crate::app::KtApp::open(&future).err().expect("a file of a future version must not be read");
    assert!(err3.contains("99"), "error does not name the version: {err3}");
    assert!(!err3.contains(tr(k::menu::file::IMPORT_OLDER)), "import does not repair a future version: {err3}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 819 (R.70): ONE import door for three entrances — a legacy `.ktproj`, our
/// `.otio` of an old envelope version, and a foreign `.otio`. The result is an unnamed
/// document of its own format with the hint “save as `<name>.kt` alongside”,
/// with an asterisk and the source untouched. A project file does not go through this door.
#[test]
fn t819_the_import_door_takes_legacy_old_and_foreign_and_yields_a_project() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    let dir = r70_dir("t819");

    // three entrances
    let legacy = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo.ktproj"));
    let old = dir.join("old_v5.otio");
    {
        let text = kt_otio::save_project(&r70_project(), &Default::default(), None);
        let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
        v["metadata"]["kt"]["v"] = serde_json::json!(5);
        std::fs::write(&old, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }
    let foreign = dir.join("foreign.otio");
    std::fs::write(&foreign, r70_foreign_otio()).unwrap();

    let mut h = crate::layout_tests::harness();
    // the item is in the “File” menu; the previous “Import .ktproj…” is gone
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();
    h.step();
    let labels = crate::layout_tests::menu_labels(&mut h);
    assert!(h.query_by_label(tr(k::menu::file::IMPORT_OLDER)).is_some(), "the door is not in the menu: {labels:?}");
    assert!(
        !labels.iter().any(|l| l.contains(".ktproj")),
        "the second import item is an extra door, legacy goes into the common one: {labels:?}"
    );
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();

    for (what, src) in [("legacy", legacy.to_path_buf()), ("old .otio", old.clone()), ("foreign .otio", foreign.clone())] {
        let before = std::fs::read(&src).unwrap();
        h.state_mut().import_older_path(&src);
        h.step();
        let app = h.state();
        assert!(!app.model.project().clips.is_empty(), "{what}: events did not arrive");
        assert_eq!(app.project_path, None, "{what}: the imported document must not have a path");
        assert_eq!(
            app.save_hint.as_deref(),
            Some(src.with_extension("kt").as_path()),
            "{what}: save hint — .kt next to the source"
        );
        assert!(app.dirty_content, "{what}: asterisk after import");
        assert_eq!(std::fs::read(&src).unwrap(), before, "{what}: import touched the source");
    }
    // foreign fields reached the passthrough — the export will return them (t820)
    assert!(!h.state().retained.is_empty(), "foreign passthrough was lost on import");

    // a project file does not go into the import door: the error names “Open”
    let ours = dir.join("ours.kt");
    std::fs::write(&ours, crate::app::project_text_of(&r70_project(), &Default::default(), None)).unwrap();
    let n0 = h.state().notices.len();
    h.state_mut().import_older_path(&ours);
    h.step();
    let said: Vec<String> = h.state().notices.iter().skip(n0).map(|n| n.text.clone()).collect();
    assert!(
        said.iter().any(|t| t.contains(tr(k::menu::file::OPEN))),
        "importing a project file must name “Open”: {said:?}"
    );
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 820 (R.70): export to OTIO — the named action `File → Export →
/// OpenTimelineIO…`, reachable from the menu; the file is self-sufficient (the reel body in
/// each instance), the foreign passthrough is returned, the project and the asterisk are
/// untouched: export is not saving.
#[test]
fn t820_export_otio_is_a_named_menu_action_and_leaves_the_project_alone() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    let dir = r70_dir("t820");

    // reachability: File → Export → OpenTimelineIO…
    let mut h = crate::layout_tests::harness();
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();
    h.step();
    assert!(
        h.query_by_label_contains(tr(k::menu::file::EXPORT)).is_some(),
        "the “Export” submenu is not in the “File” menu: {:?}",
        crate::layout_tests::menu_labels(&mut h)
    );
    h.get_by_label_contains(tr(k::menu::file::EXPORT)).click();
    h.step();
    h.step();
    let labels = crate::layout_tests::menu_labels(&mut h);
    assert!(
        h.query_by_label(tr(k::menu::file::EXPORT_OTIO)).is_some(),
        "the “OpenTimelineIO…” item is not in the export submenu: {labels:?}"
    );
    assert_eq!(tr(k::menu::file::EXPORT_OTIO), "OpenTimelineIO…", "the item is named after the industry term, not in our own way");
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();

    // a foreign project is imported, then saved in its own format
    let foreign = dir.join("foreign.otio");
    std::fs::write(&foreign, r70_foreign_otio()).unwrap();
    h.state_mut().import_older_path(&foreign);
    h.step();
    let kt = dir.join("film.kt");
    {
        let app = h.state_mut();
        let text = app.project_text(&app.model.document(), kt.parent());
        std::fs::write(&kt, &text).unwrap();
        app.project_path = Some(kt.clone());
        app.dirty_content = true; // export does not touch the asterisk — we verify this
    }
    let project_bytes = std::fs::read(&kt).unwrap();

    let dst = dir.join("film.otio");
    h.state_mut().export_otio_to(&dst);
    h.step();
    let exported = std::fs::read_to_string(&dst).expect("export did not write the file");
    assert!(exported.starts_with("{\n    \"OTIO_SCHEMA\": \"Timeline.1\""), "export is not OTIO: {exported:.80}");
    for needle in ["vendor_field", "colorVersion", "custom_clip_field"] {
        assert!(exported.contains(needle), "foreign “{needle}” did not come back on export");
    }
    assert_eq!(std::fs::read(&kt).unwrap(), project_bytes, "export rewrote the project file");
    assert!(h.state().dirty_content, "export cleared the asterisk — it is not a save");
    assert_eq!(h.state().project_path.as_deref(), Some(kt.as_path()), "export replaced the project path");
    let said: Vec<String> = h.state().notices.iter().map(|n| n.text.clone()).collect();
    assert!(
        said.iter().any(|t| t.contains(&crate::tf!(k::msg::EXPORTED, dst.display().to_string()))),
        "export did not report where it wrote: {said:?}"
    );

    // self-sufficiency: the reel in two instances — two Stacks with bodies
    {
        let app = h.state_mut();
        app.model = kt_model::TimelineModel::new(r70_project());
        app.retained = Default::default();
    }
    let dst2 = dir.join("reels.otio");
    h.state_mut().export_otio_to(&dst2);
    let tl = kt_otio::parse_document(&std::fs::read_to_string(&dst2).unwrap()).unwrap();
    let mut stacks = 0;
    for c in tl.tracks.children.iter() {
        if let kt_otio::Composable::Track(t) = c {
            for ch in &t.children {
                if let kt_otio::Composable::Stack(s) = ch {
                    stacks += 1;
                    assert!(!s.children.is_empty(), "the reel body must be inline in the export");
                }
            }
        }
    }
    assert_eq!(stacks, 2, "two instances — two Stacks with bodies");
    // and “Open” does not accept this file
    let err = crate::app::KtApp::open(&dst2).err().expect("export is not a project");
    assert!(err.contains(tr(k::menu::file::IMPORT_OLDER)), "{err}");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 821 (R.70): three extensions per window — three different outcomes, each
/// named: `.kt` — the question “open or insert”, `.otio` — the hint about the import door
/// with the exchange file’s name, `.ktproj` — the same door, in its own
/// text about legacy. The extensions are not confused or merged.
#[test]
fn t821_dropping_three_kinds_of_files_gives_three_named_outcomes() {
    use crate::i18n::{k, tr};
    let dir = r70_dir("t821");
    let p = r70_project();
    let kt = dir.join("film.kt");
    std::fs::write(&kt, crate::app::project_text_of(&p, &Default::default(), None)).unwrap();
    let otio = dir.join("film.otio");
    std::fs::write(&otio, kt_otio::save_project(&p, &Default::default(), None)).unwrap();
    let ktproj = dir.join("film.ktproj");
    std::fs::write(&ktproj, "{\"format_version\": 1}").unwrap();

    let mut h = crate::layout_tests::harness();

    // .kt — a question, the edit untouched
    h.state_mut().route_dropped(kt.clone(), None, None);
    h.step();
    assert!(matches!(h.state().ask, Some(crate::app::Ask::DropProject)), "a project file must prompt a question");
    assert_eq!(h.state().pending_drop.as_ref().map(|(p, _)| p.clone()), Some(kt.clone()));
    h.state_mut().answer_ask(crate::app::Answer::Cancel);
    h.step();

    // .otio — not a question, but a hint about the import door with the file’s name
    let n0 = h.state().notices.len();
    h.state_mut().route_dropped(otio.clone(), None, None);
    h.step();
    assert!(h.state().ask.is_none(), "an exchange file is not a project — there must be no question");
    let said_otio: Vec<String> = h.state().notices.iter().skip(n0).map(|n| n.text.clone()).collect();
    assert!(
        said_otio.iter().any(|t| t.contains(tr(k::menu::file::IMPORT_OLDER)) && t.contains("OpenTimelineIO")),
        "dropping .otio must name the import door and the file kind: {said_otio:?}"
    );

    // .ktproj — the same door, its own text
    let n1 = h.state().notices.len();
    h.state_mut().route_dropped(ktproj.clone(), None, None);
    h.step();
    assert!(h.state().ask.is_none());
    let said_legacy: Vec<String> = h.state().notices.iter().skip(n1).map(|n| n.text.clone()).collect();
    assert!(
        said_legacy.iter().any(|t| t.contains(tr(k::menu::file::IMPORT_OLDER)) && t.contains(".ktproj")),
        "dropping .ktproj must name the import door and the legacy: {said_legacy:?}"
    );
    assert_ne!(said_otio, said_legacy, "two kinds of file — two different texts");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// t832 (kinetren-rename): a project saved under the former name (`kinotrain`
/// at the root) — “Open” refuses it naming the former name Kinotrain and the import door; the
/// import door opens it as an untitled document with the star and the save
/// hint, content equal to the file, and the file untouched.
#[test]
fn t832_project_of_the_former_name_comes_in_through_the_import_door() {
    use crate::i18n::{k, tr};
    let dir = r70_dir("t832");
    let p = r70_project();
    let text = crate::app::project_text_of(&p, &Default::default(), None);
    let old = dir.join("old.kt");
    std::fs::write(&old, text.replacen("\"kinetren\"", "\"kinotrain\"", 1)).unwrap();
    let before = std::fs::read(&old).unwrap();
    assert_ne!(before, text.as_bytes(), "the scene must carry the former marker");

    let err = crate::app::KtApp::open(&old).err().expect("Open must refuse the former name");
    assert!(err.contains("Kinotrain"), "the error names the former name: {err}");
    assert!(err.contains(tr(k::menu::file::IMPORT_OLDER)), "the error names the import door: {err}");

    let mut h = crate::layout_tests::harness();
    h.state_mut().import_older_path(&old);
    h.step();
    let app = h.state();
    assert_eq!(app.model.project().clips, p.clips, "events equal to the file");
    assert_eq!(app.model.project().sequences.len(), p.sequences.len());
    assert_eq!(app.project_path, None, "an imported document has no path");
    assert_eq!(app.save_hint.as_deref(), Some(old.with_extension("kt").as_path()));
    assert!(app.dirty_content, "the star after the import");
    assert_eq!(std::fs::read(&old).unwrap(), before, "the import touched the source");
}

/// Test 840 (telemetry-posthog): the first-start notice lists what is sent and what is NOT
/// sent, is said once, and the switch is reachable in View next to Language; unchecking it
/// is remembered and cancels the session's `app_closed`.
#[test]
fn t840_telemetry_notice_once_and_switch_in_view() {
    use crate::i18n::{Lang, k, set_lang, tr};
    use crate::telemetry::testing::Recording;
    use crate::telemetry::{EnvView, Telemetry, new_uuid, resolve_consent};
    use egui_kittest::kittest::Queryable;
    use std::sync::Arc;

    // every catalog: the notice holds both lists and names the real menu item
    for lang in Lang::ALL {
        set_lang(lang);
        let notice = crate::telemetry::notice_text();
        let hint = crate::telemetry::hint_text(None);
        for part in [tr(k::telemetry::SENT), tr(k::telemetry::NOT_SENT)] {
            assert!(notice.contains(part) && hint.contains(part), "{lang:?}: {part}");
        }
        let off = tr(k::telemetry::HOW_OFF);
        // Arabic catalog strings come back in display order (words reordered, i18n::bidi):
        // there the item is named when each of its words is present
        let word = |w: &str| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
        let names = |s: &str| {
            off.contains(s)
                || s.split_whitespace().all(|w| off.split_whitespace().any(|o| word(o) == word(w)))
        };
        assert!(
            names(tr(k::menu::view::TITLE)) && names(tr(k::telemetry::CHECKBOX)),
            "{lang:?}: the notice must name View and the checkbox: {off}"
        );
    }

    let rec = Recording::default();
    let consent = resolve_consent(Some("phc_test"), &EnvView::default(), None);
    assert!(consent.first_start);
    let t =
        Arc::new(Telemetry::new(consent, new_uuid(), Box::new(rec.clone())).with_key("phc_test"));
    let mut h = crate::layout_tests::harness();
    h.state_mut().telemetry = Some(t.clone());
    for _ in 0..4 {
        h.step();
    }
    let said = |h: &egui_kittest::Harness<'_, crate::app::KtApp>| {
        h.state().notices.iter().filter(|n| n.text == crate::telemetry::notice_text()).count()
    };
    assert_eq!(said(&h), 1, "the notice is said exactly once");

    h.get_by_label(tr(k::menu::view::TITLE)).click();
    h.step();
    h.step();
    let labels = crate::layout_tests::menu_labels(&mut h);
    assert!(
        h.query_by_label(tr(k::telemetry::CHECKBOX)).is_some(),
        "the switch must be in View; seen: {labels:?}"
    );
    assert!(t.consent().on(), "on by default");
    h.get_by_label(tr(k::telemetry::CHECKBOX)).click();
    h.step();
    h.step();
    assert!(!t.consent().on(), "unchecking switches it off");
    assert_eq!(t.stored_value(), Some("off"), "the choice is remembered");
    assert_eq!(said(&h), 1);

    eframe::App::on_exit(h.state_mut());
    std::thread::sleep(std::time::Duration::from_millis(100));
    let events: Vec<String> =
        rec.events().iter().map(|e| e["event"].as_str().unwrap_or("").to_string()).collect();
    assert_eq!(events, ["app_started"], "switched off mid-session: no app_closed");

    // the restart with the stored choice: no second notice, nothing sent
    let consent = resolve_consent(Some("phc_test"), &EnvView::default(), t.stored_value());
    assert!(!consent.first_start && !consent.on());
    let t2 = Telemetry::new(consent, new_uuid(), Box::new(Recording::default()));
    assert!(!t2.frame("en"), "the notice is not said again");

    // File → New/Open rebuild KtApp in place: every constructor takes the process telemetry,
    // so a new document never sends a second app_started
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/app.rs")).unwrap();
    let ctors = src.matches("            lang_choice: None,\n").count();
    assert!(ctors >= 2);
    assert_eq!(src.matches("            telemetry: crate::telemetry::global(),\n").count(), ctors);
}

/// t850 (windows-build, ui-reach): from the “FFmpeg not found” window to playing media,
/// with the real loader. The search is limited to an empty folder (and, on Linux, the
/// system fallback is off), so the clip opens as a placeholder; Locate… refuses a folder
/// without DLLs and accepts a complete one — on Linux the system FFmpeg under its SONAMEs,
/// on Windows the pinned gyan.dev build — and the clip shows frames without a restart.
///
/// Needs a process of its own (nextest): in a shared `cargo test` process another test has
/// already loaded FFmpeg for good, and the test says so and stops.
#[test]
fn t850_ffmpeg_window_from_not_found_to_playing() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    if kt_ffmpeg::is_loaded() {
        eprintln!("t850 skipped: FFmpeg is already loaded in this process (run it under nextest)");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kt-t850-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let empty = dir.join("empty");
    let nodll = dir.join("nodll");
    let full = dir.join("full");
    for d in [&empty, &nodll, &full] {
        std::fs::create_dir_all(d).unwrap();
    }
    // a complete folder of this OS
    let full = if cfg!(windows) {
        let pinned = std::env::var_os("KT_FFMPEG_WIN_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ffmpeg-win/8.1.2")
        });
        if !pinned.join("avcodec-62.dll").exists() {
            eprintln!("t850 skipped: no pinned FFmpeg build in {}", pinned.display());
            return;
        }
        std::path::absolute(pinned).unwrap()
    } else {
        let sys = std::path::Path::new("/usr/lib/x86_64-linux-gnu");
        let Some(ffmpeg) = kt_ffmpeg::search::MAJORS.iter().map(|(f, _)| *f).find(|f| {
            kt_ffmpeg::search::LOAD_ORDER
                .iter()
                .all(|l| sys.join(kt_ffmpeg::search::file_name(kt_ffmpeg::search::Os::Linux, *l, *f)).exists())
        }) else {
            eprintln!("t850 skipped: no system FFmpeg 6/7/8 in {}", sys.display());
            return;
        };
        #[cfg(unix)]
        for l in kt_ffmpeg::search::LOAD_ORDER {
            let n = kt_ffmpeg::search::file_name(kt_ffmpeg::search::Os::Linux, l, ffmpeg);
            std::os::unix::fs::symlink(sys.join(&n), full.join(&n)).unwrap();
        }
        full
    };
    unsafe {
        std::env::set_var("KINETREN_FFMPEG_DIR", &empty);
        std::env::set_var("KINETREN_FFMPEG_SYSTEM", "0");
    }

    // a project with one video clip on a real fixture file
    let video = crate::util::tests::fixture("testsrc_h264.mp4");
    let path = dir.join("p.kt");
    let mut p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 25,
        width: 320,
        height: 240,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), &video));
    p.clips.push(crate::layout_tests::sample_clip(kt_model::ClipId(1)));
    p.validate().unwrap();
    std::fs::write(&path, crate::app::project_text_of(&p, &Default::default(), None)).unwrap();

    crate::i18n::set_lang(crate::i18n::Lang::En);
    let mut app = crate::app::KtApp::open(&path).expect("the project opens");
    app.recovery_on_drop = false;
    app.ffmpeg_ui.choice_file = dir.join("ffmpeg-dir");
    app.ffmpeg_ui.download_root = dir.join("downloads");
    app.ffmpeg_startup_check();
    assert_eq!(app.placeholder_media.len(), 1, "without FFmpeg the clip is a placeholder");
    assert!(app.ffmpeg_ui.open, "the FFmpeg window opens at startup without FFmpeg");
    let notices = app.notices.iter().filter(|n| n.text == tr(k::ffmpeg::NOT_FOUND)).count();
    assert_eq!(notices, 1, "one notice");

    let mut h = egui_kittest::Harness::builder()
        .with_size(eframe::egui::vec2(1440.0, 860.0))
        .build_ui_state(|ui, app: &mut crate::app::KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
    h.step();
    h.step();
    let empty_s = empty.to_string_lossy().to_string();
    assert!(
        h.query_all_by_label_contains(&empty_s).next().is_some(),
        "the window lists the empty folder as rejected"
    );

    // Locate… with a folder without DLLs: refused, the choice unchanged
    h.state_mut().ffmpeg_ui.test_pick = Some(nodll.clone());
    h.get_by_label(tr(k::ffmpeg::LOCATE)).click();
    h.step();
    h.step();
    let msg = h.state().ffmpeg_ui.message.clone().expect("a message");
    assert!(msg.0 && msg.1.contains(tr(k::ffmpeg::R_NO_AVCODEC)), "refused with the reason: {msg:?}");
    assert!(!dir.join("ffmpeg-dir").exists(), "a refused folder is not saved");

    // Locate… with a complete folder: saved, loaded, the clip recovers without a restart
    h.state_mut().ffmpeg_ui.test_pick = Some(full.clone());
    h.get_by_label(tr(k::ffmpeg::LOCATE)).click();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !h.state().placeholder_media.is_empty() && std::time::Instant::now() < deadline {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(kt_ffmpeg::backend_info().is_some(), "FFmpeg loaded: {}", kt_ffmpeg::search_report());
    assert!(h.state().placeholder_media.is_empty(), "the clip recovered without a restart");
    let saved = std::fs::read_to_string(dir.join("ffmpeg-dir")).unwrap();
    assert_eq!(saved.trim(), full.to_string_lossy(), "the choice is saved");
    let place = kt_ffmpeg::loaded_place().unwrap();
    assert!(
        h.query_all_by_label_contains(&place).next().is_some(),
        "the window shows the loaded folder {place}"
    );
    unsafe {
        std::env::remove_var("KINETREN_FFMPEG_DIR");
        std::env::remove_var("KINETREN_FFMPEG_SYSTEM");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

