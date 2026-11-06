//! control-layouts (L1, L2 of `docs/ktprompts/v26.10.txt`): the styles, the keys, the wheel, the
//! panels, the window and the settings code. Each test acts from where the person acts.
//!
//! The owner: "предлагать выбрать "Layout & Controls style" … Первой плиточкой идет наша и подпись
//! "like Vegas"" and "в итоге получить "код сохранения" настроек, чтобы можно было при
//! переустановке его ввести".

use crate::app::KtApp;
use crate::controls::{Arrangement, Choice, Chord, Cmd, CodeError, Controls, Style, Wheel, WheelAct};
use crate::i18n::{k, tr};
use crate::layout_tests::{harness_sized, r72_transport_scene};
use eframe::egui;
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn chord(key: Key, ctrl: bool, shift: bool, alt: bool) -> Chord {
    Chord { key, ctrl, shift, alt }
}

fn style(h: &mut Harness<'static, KtApp>, s: Style) {
    h.state_mut().apply_controls(Choice::of(s));
    h.step();
    h.step();
}

fn probe(name: &str) -> egui::Rect {
    crate::widgets::field_probe::rect_of(name).unwrap_or_else(|| panic!("no probe “{name}”"))
}

fn click(h: &mut Harness<'static, KtApp>, name: &str) {
    let p = probe(name).center();
    h.hover_at(p);
    h.step();
    h.drag_at(p);
    h.step();
    h.drop_at(p);
    h.step();
    h.step();
    h.step();
}

fn open_window(h: &mut Harness<'static, KtApp>) {
    h.state_mut().start.controls_ui.open = true;
    for _ in 0..4 {
        h.step();
    }
}

fn open_customize(h: &mut Harness<'static, KtApp>) {
    open_window(h);
    h.state_mut().start.controls_ui.customize = true;
    for _ in 0..4 {
        h.step();
    }
}

/// The list down to one command, as the person finds it by the search field.
fn find_row(h: &mut Harness<'static, KtApp>, cmd: Cmd) {
    h.state_mut().start.controls_ui.filter = crate::controls::cmd_name(cmd).to_string();
    for _ in 0..3 {
        h.step();
    }
}

fn clips_scene() -> Harness<'static, KtApp> {
    let (h, _) = crate::layout_tests::harness_three_clips_dt(None);
    h
}

/// A wheel turn with modifiers held, over the tracks.
fn wheel_at(h: &mut Harness<'static, KtApp>, m: Modifiers, dy: f32) {
    let r = h.state().timeline_rect;
    let p = egui::pos2(r.center().x, r.top() + 90.0);
    h.hover_at(p);
    h.step();
    for _ in 0..3 {
        h.event_modifiers(
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, dy),
                modifiers: m,
                phase: egui::TouchPhase::Move,
            },
            m,
        );
        h.step();
    }
    h.step();
}

#[test]
fn t1700_our_style_keeps_every_key_of_the_old_handler() {
    let c = Controls::default();
    let rows: &[(Chord, Option<Cmd>)] = &[
        (chord(Key::S, false, false, false), Some(Cmd::Split)),
        (chord(Key::S, false, true, false), Some(Cmd::Split)), // Shift never stopped it
        (chord(Key::S, true, false, false), Some(Cmd::Save)),
        (chord(Key::S, true, true, false), Some(Cmd::Save)),
        (chord(Key::S, false, false, true), Some(Cmd::TrackSolo)),
        (chord(Key::M, false, false, false), Some(Cmd::Marker)),
        (chord(Key::M, false, false, true), Some(Cmd::TrackMute)),
        (chord(Key::I, false, false, false), Some(Cmd::RegionIn)),
        (chord(Key::O, false, false, false), Some(Cmd::RegionOut)),
        (chord(Key::L, false, false, false), Some(Cmd::Loop)),
        (chord(Key::L, true, false, false), Some(Cmd::GotoTimecode)),
        (chord(Key::F, true, false, false), Some(Cmd::Find)),
        (chord(Key::Z, true, false, false), Some(Cmd::Undo)),
        (chord(Key::Z, true, true, false), Some(Cmd::Redo)),
        (chord(Key::Y, true, false, false), Some(Cmd::Redo)),
        (chord(Key::A, false, false, false), Some(Cmd::NavPrev)),
        (chord(Key::D, false, false, false), Some(Cmd::NavNext)),
        (chord(Key::Home, false, false, false), Some(Cmd::NavPrev)),
        (chord(Key::Home, true, false, false), Some(Cmd::NavPrev)), // any modifier jumped
        (chord(Key::End, false, false, false), Some(Cmd::NavNext)),
        (chord(Key::Home, false, true, false), Some(Cmd::ProjectStart)),
        (chord(Key::End, false, true, false), Some(Cmd::ProjectEnd)),
        (chord(Key::A, false, true, false), Some(Cmd::RegionPrev)),
        (chord(Key::D, false, true, false), Some(Cmd::RegionNext)),
        (chord(Key::ArrowLeft, false, false, false), Some(Cmd::FramePrev)),
        (chord(Key::ArrowRight, false, false, false), Some(Cmd::FrameNext)),
        (chord(Key::ArrowLeft, false, true, false), None), // the arrows wanted no modifier
        (chord(Key::Delete, false, false, false), Some(Cmd::Delete)),
        (chord(Key::Delete, true, false, false), Some(Cmd::Delete)),
        (chord(Key::Backspace, false, false, false), Some(Cmd::Delete)),
        (chord(Key::C, true, false, false), Some(Cmd::Copy)),
        (chord(Key::V, true, false, false), Some(Cmd::Paste)),
        (chord(Key::J, false, false, false), None), // free, as they were
        (chord(Key::K, false, false, false), None),
        (chord(Key::ArrowUp, false, false, false), None),
    ];
    for (c0, want) in rows {
        assert_eq!(c.lookup(*c0), *want, "{c0:?}");
    }
    assert_eq!(c.play_key(), Some(Key::Space));
    // and the labels the menus always printed
    for (cmd, label) in [(Cmd::Save, "Ctrl+S"), (Cmd::Undo, "Ctrl+Z"), (Cmd::Redo, "Ctrl+Shift+Z"), (Cmd::Delete, "Del"), (Cmd::Split, "S")] {
        assert_eq!(c.label(cmd), label);
    }
    let d = Choice::default();
    assert_eq!((d.style, d.layout, d.wheel), (Style::Own, Arrangement::Classic, Wheel::OWN));
}

#[test]
fn t1701_every_style_is_a_clean_table() {
    for s in Style::ALL {
        let mut seen = std::collections::HashMap::new();
        for (cmd, keys) in s.bindings() {
            assert!(crate::controls::COMMANDS.iter().any(|c| c.0 == *cmd), "{s:?}: {cmd:?} is not in the list");
            assert!(keys.len() <= crate::controls::MAX_CHORDS, "{s:?} {cmd:?}: a code carries fewer keys");
            for ch in *keys {
                assert!(crate::controls::assignable(ch.key), "{s:?} {cmd:?}: {ch:?} cannot be in a code");
                assert!(!crate::controls::reserved(ch.key), "{s:?} {cmd:?}: {ch:?}");
                if let Some(other) = seen.insert(*ch, *cmd) {
                    panic!("{s:?}: {} is on both {other:?} and {cmd:?}", ch.label());
                }
            }
        }
        // the Space machine is the same in every style (rounds 6, 16, R.72)
        assert_eq!(Controls::new(Choice::of(s)).play_key(), Some(Key::Space), "{s:?}");
        // the wheel zooms somewhere, and in the other programs on Alt
        let w = s.wheel();
        if s != Style::Own {
            assert_eq!(w.alt, WheelAct::Zoom, "{s:?}");
        }
    }
    // the commands' list has one row per number, with no holes
    let mut numbers: Vec<u8> = crate::controls::COMMANDS.iter().map(|c| c.0 as u8).collect();
    numbers.sort();
    assert_eq!(numbers, (0..numbers.len() as u8).collect::<Vec<_>>());
}

#[test]
fn t1702_a_code_given_today_opens_tomorrow() {
    // literals, not `to_code()`: a renumbered table must turn this red
    let own = Choice::default().to_code();
    assert_eq!(own, FROZEN_OWN, "the factory code changed: a person's written code stops working");
    assert_eq!(Choice::from_code(FROZEN_OWN).unwrap(), Choice::default());
    let mut c = Choice::of(Style::Premiere);
    c.layout = Arrangement::TimelineOnTop;
    c.wheel.plain = WheelAct::Zoom;
    c.bind(Cmd::Split, Some(0), chord(Key::Q, false, false, false));
    c.bind(Cmd::Marker, None, chord(Key::F5, true, true, true));
    assert_eq!(c.to_code(), FROZEN_CUSTOM);
    assert_eq!(Choice::from_code(FROZEN_CUSTOM).unwrap(), c);
}

const FROZEN_OWN: &str = "04000-6319M";
const FROZEN_CUSTOM: &str = "040G3-5JB20-09Y30-0503P-5A8";

#[test]
fn t1703_a_code_survives_how_a_person_writes_it() {
    let mut c = Choice::of(Style::Resolve);
    c.bind(Cmd::Loop, Some(0), chord(Key::Q, false, false, false));
    c.unbind(Cmd::Marker, 0);
    let code = c.to_code();
    assert!(code.chars().all(|ch| ch == '-' || ch.is_ascii_digit() || ch.is_ascii_uppercase()), "{code}");
    assert_eq!(Choice::from_code(&code).unwrap(), c);
    assert_eq!(Choice::from_code(&code.to_lowercase()).unwrap(), c, "lower case");
    assert_eq!(Choice::from_code(&code.replace('-', " ")).unwrap(), c, "spaces");
    assert_eq!(Choice::from_code(&code.replace('-', "")).unwrap(), c, "no separators");
    assert_eq!(Choice::from_code(&code.replace('0', "O").replace('1', "I")).unwrap(), c, "misread digits");
    // a factory style is short enough to write down
    for s in Style::ALL {
        assert!(Choice::of(s).to_code().chars().filter(|ch| *ch != '-').count() <= 10, "{s:?}");
    }
}

#[test]
fn t1704_a_wrong_code_is_refused_and_says_why() {
    let mut c = Choice::of(Style::FinalCut);
    c.bind(Cmd::Split, Some(0), chord(Key::Q, false, false, false));
    let code = c.to_code();
    // every single-letter slip is caught
    for i in 0..code.len() {
        if code.as_bytes()[i] == b'-' {
            continue;
        }
        let mut bad = code.clone().into_bytes();
        bad[i] = if bad[i] == b'A' { b'B' } else { b'A' };
        let bad = String::from_utf8(bad).unwrap();
        assert!(Choice::from_code(&bad).is_err(), "{bad} passed");
    }
    assert_eq!(Choice::from_code("hello!"), Err(CodeError::Unreadable));
    assert_eq!(Choice::from_code("0123"), Err(CodeError::Unreadable));
    assert_eq!(Choice::from_code(""), Err(CodeError::Unreadable));
    // a later program's code: version 9 with a correct check
    let mut fresh = Choice::default().to_code().replace('-', "");
    fresh.replace_range(0..2, "14");
    assert!(Choice::from_code(&fresh).is_err());
    for e in [CodeError::Unreadable, CodeError::Typo, CodeError::Newer] {
        assert!(!tr(e.text()).is_empty());
    }
}

#[test]
fn t1705_changes_stay_on_top_of_the_style() {
    let mut c = Choice::of(Style::Own);
    c.bind(Cmd::Split, Some(0), chord(Key::Q, false, false, false));
    assert!(c.is_changed(Cmd::Split));
    assert!(!c.is_factory());
    // back to what the style has: nothing is stored
    c.bind(Cmd::Split, Some(0), chord(Key::S, false, false, false));
    assert!(!c.is_changed(Cmd::Split));
    assert!(c.is_factory());
    // a command with no key is a change the code carries
    c.unbind(Cmd::Marker, 0);
    assert_eq!(c.keys_of(Cmd::Marker), &[]);
    assert_eq!(Choice::from_code(&c.to_code()).unwrap().keys_of(Cmd::Marker), &[]);
    c.reset_keys(Cmd::Marker);
    assert!(c.is_factory());
    // a given-up key leaves the others of that command
    c.release(Cmd::NavPrev, chord(Key::A, false, false, false));
    assert_eq!(c.keys_of(Cmd::NavPrev), &[chord(Key::Home, false, false, false)]);
    let on = Controls::new(c);
    assert_eq!(on.lookup(chord(Key::A, false, false, false)), None);
}

#[test]
fn t1706_a_style_moves_the_key_and_what_prints_it() {
    let mut h = clips_scene();
    let cuts = |h: &Harness<'static, KtApp>| h.state().model.project().clips.len();
    h.state().player.seek(kt_time::Tick(120_000));
    h.step();
    h.step();
    let before = cuts(&h);
    style(&mut h, Style::Premiere);
    // the old key no longer cuts in that style
    h.key_press(Key::S);
    h.step();
    assert_eq!(cuts(&h), before, "S split in the Premiere style");
    h.key_press_modifiers(Modifiers::CTRL, Key::K);
    h.step();
    assert_eq!(cuts(&h), before + 1, "Ctrl+K did not split");
    // the hint under the mouse says the same key
    let at = h.get_by_label(crate::icons::SCISSORS).rect().center();
    h.hover_at(at);
    h.step();
    h.step();
    let shown = h.state().hover_hint.clone().unwrap_or_default();
    assert!(shown.contains("Ctrl+K"), "the split button's hint says {shown:?}");
    // the menu item too
    h.get_by_label(tr(k::menu::edit::TITLE)).click();
    h.step();
    h.step();
    let item = format!("{}\tCtrl+K", tr(k::tl::menu::SPLIT));
    assert!(h.query_by_label(&item).is_some(), "the Edit menu has no “{item}”");
}

#[test]
fn t1707_j_k_l_run_the_transport_in_the_styles_that_have_them() {
    let mut h = r72_transport_scene();
    // ours: the letters stay free
    h.key_press(Key::K);
    h.key_press(Key::J);
    h.step();
    assert!(!h.state().player.transport.is_playing(), "J played in our style");
    style(&mut h, Style::Premiere);
    h.key_press(Key::L);
    h.step();
    h.step();
    assert!(h.state().player.transport.is_playing(), "L did not play");
    assert_eq!(h.state().player.playback_rate(), 1.0);
    h.key_press(Key::L);
    h.step();
    assert_eq!(h.state().player.playback_rate(), 2.0, "a repeat doubles");
    assert_eq!(h.state().speed_temp_slider(), Some(crate::app::rate_to_slider(2.0)), "the widget must show a temporary speed");
    h.key_press(Key::L);
    h.step();
    h.key_press(Key::L);
    h.step();
    assert_eq!(h.state().player.playback_rate(), 4.0, "the ceiling is the Space hold's");
    h.key_press(Key::J);
    h.step();
    assert_eq!(h.state().player.playback_rate(), -1.0, "the other way starts at 1×");
    h.key_press(Key::K);
    h.step();
    h.step();
    assert!(!h.state().player.transport.is_playing(), "K pauses");
    assert_eq!(h.state().player.playback_rate(), 1.0, "K gives the base speed back");
    // a pause by another door ends the run too
    h.key_press(Key::L);
    h.step();
    h.key_press(Key::L);
    h.step();
    h.key_press(Key::Space);
    h.step();
    h.step();
    h.step();
    assert!(h.state().shuttle_rate.is_none(), "Space must end a J/L run");
    assert_eq!(h.state().player.playback_rate(), 1.0);
}

#[test]
fn t1708_the_wheel_follows_the_style() {
    let mut h = clips_scene();
    for _ in 0..3 {
        h.step();
    }
    let tpp = |h: &Harness<'static, KtApp>| h.state().ticks_per_px;
    // ours, as it always was
    h.state_mut().tl_scroll = 0.0;
    let z0 = tpp(&h);
    wheel_at(&mut h, Modifiers::ALT, -40.0);
    assert_eq!(tpp(&h), z0, "Alt+wheel zoomed in our style");
    wheel_at(&mut h, Modifiers::CTRL, -40.0);
    assert_ne!(tpp(&h), z0, "Ctrl+wheel stopped zooming in our style");
    let z1 = tpp(&h);
    let v = h.state().view_start;
    wheel_at(&mut h, Modifiers::NONE, -40.0);
    assert_eq!(tpp(&h), z1, "the bare wheel zoomed");
    assert_ne!(h.state().view_start, v, "the bare wheel stopped scrolling time");
    // Premiere: Alt zooms, around the mouse
    style(&mut h, Style::Premiere);
    let z2 = tpp(&h);
    wheel_at(&mut h, Modifiers::ALT, -40.0);
    assert_ne!(tpp(&h), z2, "Alt+wheel did not zoom in the Premiere style");
    let z3 = tpp(&h);
    wheel_at(&mut h, Modifiers::CTRL, -40.0);
    assert_eq!(tpp(&h), z3, "Ctrl+wheel zoomed in the Premiere style");
    // a customised cell acts as well: the bare wheel made the zoom
    let mut c = Choice::of(Style::Own);
    c.wheel.plain = WheelAct::Zoom;
    h.state_mut().apply_controls(c);
    h.step();
    let z4 = tpp(&h);
    wheel_at(&mut h, Modifiers::NONE, -40.0);
    assert_ne!(tpp(&h), z4, "the bare wheel set to zoom did not zoom");
    // the hint names the same zoom
    assert!(Style::Premiere.wheel().hint().contains("Alt"));
    assert_eq!(Style::Premiere.wheel().zoom_prefix(), Some("Alt+"));
}

fn panel(h: &Harness<'static, KtApp>, id: &str) -> Option<egui::Rect> {
    egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new(id)).map(|p| p.outer_rect)
}

#[test]
fn t1709_each_arrangement_keeps_every_panel_and_the_window_controls() {
    let (w, hh) = (1440.0, 860.0);
    let mut h = harness_sized(w, hh);
    for _ in 0..4 {
        h.step();
    }
    // Classic: the default this round must not move
    let t0 = h.state().timeline_rect;
    let bin0 = panel(&h, "left_col").unwrap();
    assert!(t0.left() <= 1.0 && t0.right() >= w - 1.0, "the timeline left the full width: {t0:?}");
    assert!(bin0.top() <= 1.0, "the bin left the top: {bin0:?}");
    assert!(panel(&h, "header_row").is_none(), "Classic has no header row");
    let meters = panel(&h, "meters_col").unwrap();
    assert!(meters.bottom() <= t0.top() + 1.0, "the meters left the preview's side");

    let run = |h: &mut Harness<'static, KtApp>, a: Arrangement| {
        h.state_mut().apply_controls(Choice { layout: a, ..Choice::default() });
        for _ in 0..5 {
            h.step();
        }
    };
    run(&mut h, Arrangement::BinBesideTimeline);
    let (t, bin, m) = (h.state().timeline_rect, panel(&h, "left_col").unwrap(), panel(&h, "meters_col").unwrap());
    assert!(bin.right() <= t.left() + 1.0, "the bin must stand left of the timeline: {bin:?} {t:?}");
    assert!(m.left() >= t.right() - 1.0, "the meters must stand right of the timeline: {m:?} {t:?}");
    assert!((bin.top() - t.top()).abs() < 2.0, "one row: {bin:?} {t:?}");
    assert!(t.width() > 600.0, "the timeline shrank away: {t:?}");

    run(&mut h, Arrangement::MetersBesideTimeline);
    let (t, bin, m) = (h.state().timeline_rect, panel(&h, "left_col").unwrap(), panel(&h, "meters_col").unwrap());
    assert!(bin.bottom() <= t.top() + 1.0, "the bin stands above the timeline");
    assert!(m.left() >= t.right() - 1.0 && m.top() >= t.top() - 1.0, "the meters end the timeline's row");

    run(&mut h, Arrangement::TimelineOnTop);
    let (t, bin) = (h.state().timeline_rect, panel(&h, "left_col").unwrap());
    assert!(t.top() < 40.0, "the timeline must be on top: {t:?}");
    assert!(bin.top() >= t.bottom() - 1.0, "the bin stands under it");

    // the menu and the window buttons live in the columns in Classic: the row must carry them
    for a in [Arrangement::BinBesideTimeline, Arrangement::MetersBesideTimeline, Arrangement::TimelineOnTop] {
        run(&mut h, a);
        let file = h.get_by_label(tr(k::menu::file::TITLE)).rect();
        assert!(file.top() < 24.0, "{a:?}: the menu left the window's top: {file:?}");
        // the window buttons are painted: the hint of the corner says who is there
        h.hover_at(egui::pos2(w - 10.0, 10.0));
        h.step();
        h.step();
        assert_eq!(h.state().hover_hint.as_deref(), Some(tr(k::win::CLOSE)), "{a:?}: no close button in the corner");
    }
}

#[test]
fn t1710_the_tiles_ours_first_with_the_marks_credited() {
    let mut h = harness_sized(1440.0, 860.0);
    open_window(&mut h);
    let first = probe("controls.tile.0");
    for s in [Style::Premiere, Style::Resolve, Style::FinalCut, Style::AfterEffects] {
        let r = probe(&format!("controls.tile.{}", s as u8));
        assert!(r.left() > first.left() || r.top() > first.top(), "ours is the first tile");
        let name = crate::tf!(k::controls::LIKE, s.product());
        assert!(h.query_by_label_contains(&name).is_some(), "no tile “{name}”");
    }
    let ours = crate::tf!(k::controls::LIKE, "Vegas");
    assert!(h.query_by_label_contains(&ours).is_some(), "ours must say “{ours}”");
    assert!(h.query_by_label(tr(k::controls::TRADEMARKS)).is_some(), "the trademarks line");
    assert!(tr(k::controls::TRADEMARKS).contains("Final Cut Pro") && tr(k::controls::TRADEMARKS).contains("VEGAS"));
    assert!(h.query_by_label(tr(k::controls::TRADEMARKS_NOTE)).is_some(), "the not-connected line");
    assert!(h.query_by_label(tr(k::controls::CHANGE_LATER)).is_some(), "rule 19");
    // a click on a tile is the whole choice: keys, wheel and panels, at once
    click(&mut h, "controls.tile.1");
    let c = &h.state().start.controls;
    assert_eq!(c.choice.style, Style::Premiere);
    assert_eq!(c.label(Cmd::Split), "Ctrl+K");
    assert_eq!(c.choice.layout, Arrangement::BinBesideTimeline);
    assert!(h.query_by_label(tr(k::controls::missing::PREMIERE)).is_some(), "the tile says what is not here");
    // the window stays: the person looks at the result, and picks another
    click(&mut h, "controls.tile.0");
    assert_eq!(h.state().start.controls.choice, Choice::default());
    assert!(h.state().start.controls_ui.open);
    // the door from Preferences
    h.state_mut().start.controls_ui.open = false;
    crate::reach_tests::open_preferences(&mut h);
    h.step();
    click(&mut h, "prefs.controls");
    assert!(h.state().start.controls_ui.open, "Preferences has no way here");
}

#[test]
fn t1711_a_key_is_assigned_again_and_the_keys_stay_alive() {
    let mut h = clips_scene();
    open_customize(&mut h);
    let cap = format!("controls.key.{}.0", Cmd::Split as u8);
    let clips = h.state().model.project().clips.len();
    h.state().player.seek(kt_time::Tick(120_000));
    find_row(&mut h, Cmd::Split);
    // twice in a row (rule 14)
    for key in [Key::Q, Key::W] {
        click(&mut h, &cap);
        assert!(h.state().start.controls_ui.capture.is_some(), "the cap did not listen");
        h.key_press(key);
        h.step();
        h.step();
        assert!(h.state().start.controls_ui.capture.is_none());
        assert_eq!(h.state().start.controls.label(Cmd::Split), key.name());
        assert_eq!(h.state().model.project().clips.len(), clips, "the press that assigns must not act");
    }
    // listening, the program's keys are asleep, and Esc lets go without closing the window
    click(&mut h, &cap);
    h.key_press(Key::Escape);
    h.step();
    h.step();
    assert!(h.state().start.controls_ui.capture.is_none());
    assert!(h.state().start.controls_ui.open, "Esc in the capture closed the window");
    // a key that cannot be one says so and keeps listening
    click(&mut h, &cap);
    h.key_press(Key::Enter);
    h.step();
    h.step();
    assert!(h.state().start.controls_ui.capture.is_some());
    assert!(h.query_by_label(tr(k::controls::REFUSED_KEY)).is_some(), "the refusal is not shown");
    h.key_press(Key::Escape);
    h.step();
    // Play takes a bare key
    find_row(&mut h, Cmd::PlayPause);
    click(&mut h, &format!("controls.key.{}.0", Cmd::PlayPause as u8));
    h.key_press_modifiers(Modifiers::CTRL, Key::P);
    h.step();
    h.step();
    assert!(h.query_by_label(tr(k::controls::REFUSED_PLAY)).is_some());
    h.key_press(Key::Escape);
    h.step();
    // the new key works, closed or open, and the old one does not
    h.state_mut().start.controls_ui.open = false;
    h.step();
    h.key_press(Key::S);
    h.step();
    assert_eq!(h.state().model.project().clips.len(), clips, "the old key still splits");
    h.key_press(Key::W);
    h.step();
    assert_eq!(h.state().model.project().clips.len(), clips + 1, "the new key does not split");
}

#[test]
fn t1712_a_taken_key_is_asked_and_never_taken_silently() {
    let mut h = harness_sized(1440.0, 1100.0);
    open_customize(&mut h);
    find_row(&mut h, Cmd::Split);
    click(&mut h, &format!("controls.key.{}.0", Cmd::Split as u8));
    h.key_press(Key::M);
    h.step();
    h.step();
    let t = h.state().start.controls_ui.taken.expect("a taken key must ask");
    assert_eq!((t.holder, t.at.cmd), (Cmd::Marker, Cmd::Split));
    let text = crate::tf!(k::controls::TAKEN, "M", crate::controls::cmd_name(Cmd::Marker));
    assert!(h.query_by_label(&text).is_some(), "the question must name the holder: {text}");
    assert_eq!(h.state().start.controls.label(Cmd::Split), "S", "asked, not done");
    h.step();
    click(&mut h, "controls.keep");
    assert_eq!(h.state().start.controls.label(Cmd::Marker), "M", "Leave it left nothing there");
    assert!(h.state().start.controls_ui.taken.is_none());
    // and again, taken
    click(&mut h, &format!("controls.key.{}.0", Cmd::Split as u8));
    h.key_press(Key::M);
    h.step();
    h.step();
    h.step();
    click(&mut h, "controls.take");
    let c = &h.state().start.controls;
    assert_eq!(c.label(Cmd::Split), "M");
    assert_eq!(c.label(Cmd::Marker), "");
    assert_eq!(c.label_all(Cmd::Marker), "—", "a hint with no key says so");
    // Ctrl+Space is the machine's reverse hold, in no list row but held all the same
    assert_eq!(c.holder(chord(Key::Space, true, false, false)), Some(Cmd::PlayPause));
    // the reset of one command, and of the whole style
    let reset = format!("controls.reset.{}", Cmd::Split as u8);
    h.step();
    click(&mut h, &reset);
    assert_eq!(h.state().start.controls.label(Cmd::Split), "S");
    click(&mut h, "controls.reset");
    assert_eq!(h.state().start.controls.choice, Choice::default());
}

#[test]
fn t1713_the_code_in_the_window_and_after_a_reinstall() {
    let dir = std::env::temp_dir().join(format!("kt-ctl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = harness_sized(1440.0, 860.0);
    h.state_mut().start.store = crate::user_store::StartStore::at(Some(dir.clone()));
    h.state_mut().load_controls(false);
    let mut mine = Choice::of(Style::Resolve);
    mine.bind(Cmd::Split, Some(0), chord(Key::Q, false, false, false));
    mine.layout = Arrangement::TimelineOnTop;
    let code = mine.to_code();
    open_window(&mut h);
    // a typo: refused, named, nothing moves
    let mut wrong = code.clone();
    wrong.replace_range(0..1, if wrong.starts_with('7') { "8" } else { "7" });
    h.state_mut().start.controls_ui.code_input = wrong;
    h.step();
    h.step();
    click(&mut h, "controls.code_apply");
    assert!(h.state().start.controls_ui.code_error.is_some());
    assert!(h.query_by_label(tr(CodeError::Typo.text())).is_some() || h.query_by_label(tr(CodeError::Newer.text())).is_some(), "the refusal must be seen");
    assert_eq!(h.state().start.controls.choice, Choice::default(), "a wrong code changed something");
    // the right one, in the person's own hand
    h.state_mut().start.controls_ui.code_input = code.to_lowercase();
    h.step();
    h.step();
    click(&mut h, "controls.code_apply");
    assert!(h.state().start.controls_ui.code_error.is_none());
    assert_eq!(h.state().start.controls.choice, mine, "the code was not applied");
    assert_eq!(h.state().start.controls.label(Cmd::Split), "Q");
    // the machine remembers: a new start reads the same thing
    let mut again = harness_sized(1440.0, 860.0);
    again.state_mut().start.store = crate::user_store::StartStore::at(Some(dir.clone()));
    again.state_mut().load_controls(false);
    assert_eq!(again.state().start.controls.choice, mine, "the choice did not survive a restart");
    // leaving a tile and coming back brings its changes (rule 23)
    h.state_mut().pick_style(Style::Own);
    assert_eq!(h.state().start.controls.choice, Choice::default());
    h.state_mut().pick_style(Style::Resolve);
    assert_eq!(h.state().start.controls.choice, mine, "the tile forgot its changes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn t1714_the_first_entry_asks_once() {
    let dir = std::env::temp_dir().join(format!("kt-ctl1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let plan = |store: &mut crate::user_store::StartStore, cli: Option<&str>| {
        crate::welcome::plan_start(store, None, cli, false, |_| false)
    };
    // a project passed by hand has no welcome window: the question waits
    let mut s = crate::user_store::StartStore::at(Some(dir.clone()));
    assert!(!plan(&mut s, Some("/nonexistent.kt")).offer_controls);
    // the first start that shows the window asks
    let mut s = crate::user_store::StartStore::at(Some(dir.clone()));
    assert!(plan(&mut s, None).offer_controls, "the first entry must ask");
    let mut h = harness_sized(1440.0, 860.0);
    h.state_mut().start.store = s;
    h.state_mut().load_controls(true);
    assert!(h.state().start.controls_ui.open);
    for _ in 0..3 {
        h.step();
    }
    let top = h.ctx.memory(|m| m.area_rect(egui::Id::new("kt_controls")));
    assert!(top.is_some(), "the question is not on screen");
    // the live run saw the welcome window drawn over the tiles: it waits for the answer
    h.state_mut().start.welcome.open = true;
    h.step();
    h.step();
    assert!(h.query_by_label(tr(k::welcome::TITLE)).is_none(), "the welcome window covers the question");
    // Esc keeps the default, the window goes, and it never asks again
    h.key_press(Key::Escape);
    h.step();
    h.step();
    h.step();
    assert!(!h.state().start.controls_ui.open, "Esc did not close it");
    assert_eq!(h.state().start.controls.choice, Choice::default());
    let mut s = crate::user_store::StartStore::at(Some(dir.clone()));
    assert!(!plan(&mut s, None).offer_controls, "asked twice");
    // a record of an older build: that person's first entry is behind them
    let old = dir.join("old");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("start.json"), r#"{"version":1,"welcome_at_start":true,"started_before":true}"#).unwrap();
    let mut s = crate::user_store::StartStore::at(Some(old));
    assert!(!plan(&mut s, None).offer_controls, "an update is not a first entry");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn t1715_no_key_is_printed_past_the_table() {
    // the labels in code: every accelerator of the menus comes from the table
    let src = std::fs::read_to_string(crate::reach_tests::workspace_root().join("apps/kt-app/src/toolbar.rs")).unwrap();
    for lit in ["\"Ctrl+", "\"Del\"", "\"S\")"] {
        assert!(!src.contains(lit), "a literal accelerator {lit} in toolbar.rs: it would lie in another style");
    }
    // the hints: a bound command's key is a placeholder in every language
    for lang in crate::i18n::Lang::ALL {
        for key in [k::tb::COPY, k::tb::PASTE, k::tb::UNDO, k::tb::REDO, k::tb::SEARCH, k::tb::SAVE, k::tb::CUT, k::tb::MARKER, k::tb::LOOP, k::tb::PLAY_PAUSE] {
            crate::i18n::set_lang(lang);
            let text = crate::i18n::tr(key);
            assert!(text.contains("{0}") || text.contains('0'), "{lang:?}: “{text}” prints a fixed key");
        }
    }
    // our style's hints read as they always did
    assert!(crate::tf!(k::tb::COPY, Controls::default().label_all(Cmd::Copy)).contains("Ctrl+C"));
    assert!(Wheel::OWN.hint().contains("Ctrl") && Wheel::OWN.hint().contains("Shift") && !Wheel::OWN.hint().contains("Alt"));
}

#[test]
fn t1716_a_new_project_keeps_the_style() {
    let mut h = harness_sized(1440.0, 860.0);
    style(&mut h, Style::Resolve);
    h.state_mut().new_project();
    for _ in 0..4 {
        h.step();
    }
    assert_eq!(h.state().start.controls.choice.style, Style::Resolve, "File → New dropped the style");
    assert_eq!(h.state().start.controls.label(Cmd::Split), "Ctrl+\\");
}
