//! windows-fill-their-area (kkt-26.11.01 item 12, registry K12): every window grows and its body
//! fills it; the Guide's tables and index; the picker gallery; the toolbar's overflow.
//!
//! The owner: "if you make the Guide window bigger its elements will not take the whole area, and
//! again you look through a needle's eye on a big window … and so everywhere … I am tired of
//! reminding". Each window was fixed on its own before and the class came back in the next one, so
//! the guard walks EVERY window of the registry (the openers of t779) and acts as the person does:
//! it drags the window's corner.

use crate::app::KtApp;
use crate::layout_tests::{harness_sized, opened_layers};
use crate::reach_tests::{OpenWin, window_openers};
use eframe::egui;
use egui_kittest::Harness;

/// Windows exempt from "it grows and its body fills it", with the reason (≥ 25 characters).
const MODAL: &[(&str, &str)] = &[
    ("kt_recovery", "a modal question at start: egui's Modal has no resize, and the program waits on the answer"),
    ("kt_ask", "a modal question on close: egui's Modal has no resize, one line and a row of answers"),
];

/// Windows that grow but have no body to fill — none today: egui sizes a window by its content,
/// so a dialog without a filling body cannot be made taller at all (found by this test on the
/// paste-prefix and preset-name dialogs). An entry needs its reason (≥ 25 characters).
const SHORT: &[(&str, &str)] = &[];

/// Drag a window's bottom-right corner from `from` to `to` the way a person does: press, a small
/// move past egui's drag threshold, the rest, a still frame, the release (three-step rule, R.27).
pub(crate) fn drag_corner(h: &mut Harness<'static, KtApp>, from: egui::Pos2, to: egui::Pos2) {
    h.hover_at(from);
    h.step();
    h.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    h.event(egui::Event::PointerMoved(from + egui::vec2(8.0, 8.0)));
    h.step();
    for i in 1..=6 {
        let t = i as f32 / 6.0;
        h.event(egui::Event::PointerMoved(from + (to - from) * t));
        h.step();
    }
    h.step();
    h.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    h.step();
    h.step();
}

/// The window's outer rectangle (its area) by the layer an opener added.
pub(crate) fn window_rect(h: &Harness<'static, KtApp>, layer: egui::LayerId) -> egui::Rect {
    egui::AreaState::load(&h.ctx, layer.id).map(|s| s.rect()).expect("the window is drawn")
}

/// Enlarge the window on `layer` to about `w`×`h` by its corner; returns the new rectangle.
pub(crate) fn enlarge(
    h: &mut Harness<'static, KtApp>,
    layer: egui::LayerId,
    w: f32,
    hgt: f32,
) -> egui::Rect {
    let r0 = window_rect(h, layer);
    let screen = h.ctx.content_rect();
    let from = r0.right_bottom() - egui::vec2(2.0, 2.0);
    let to = egui::pos2(
        (r0.left() + w).min(screen.right() - 8.0),
        (r0.top() + hgt).min(screen.bottom() - 8.0),
    );
    drag_corner(h, from, to);
    window_rect(h, layer)
}

/// Test 1125 (windows-fill-their-area): every window grows when its corner is dragged and its body
/// fills it.
#[test]
fn t1125_every_window_grows_and_its_body_fills_it() {
    let mut problems: Vec<String> = Vec::new();
    let mut checked = 0;
    for (why_list, what) in [(MODAL, "modal"), (SHORT, "short")] {
        for (id, why) in why_list {
            assert!(why.chars().count() >= 25, "“{id}” ({what}): the reason “{why}” is too short");
            assert!(
                window_openers().iter().any(|(o, _, _)| o == id),
                "“{id}” ({what}) is not a window of the registry"
            );
        }
    }
    for (id, how, closes) in window_openers() {
        let OpenWin::By(open) = how else { continue };
        if MODAL.iter().any(|(m, _)| *m == id) {
            continue;
        }
        let mut h = harness_sized(1920.0, 1200.0);
        let layers = opened_layers(&mut h, open);
        for _ in 0..4 {
            h.step();
        }
        let Some(&layer) = layers.iter().find(|l| l.order == egui::Order::Middle) else {
            problems.push(format!("“{id}”: the opener drew no window"));
            continue;
        };
        let r0 = window_rect(&h, layer);
        let (tw, th) = (1500.0f32.max(r0.width() + 300.0), 1000.0f32.max(r0.height() + 300.0));
        let r1 = enlarge(&mut h, layer, tw, th);
        let want_w = (tw).min(h.ctx.content_rect().right() - 8.0 - r0.left());
        let want_h = (th).min(h.ctx.content_rect().bottom() - 8.0 - r0.top());
        if r1.width() < want_w - 40.0 || r1.height() < want_h - 40.0 {
            problems.push(format!(
                "“{id}”: dragged from {:.0}×{:.0} toward {want_w:.0}×{want_h:.0}, the window is {:.0}×{:.0} — \
                 it does not grow (not resizable, or something caps it)",
                r0.width(),
                r0.height(),
                r1.width(),
                r1.height()
            ));
            continue;
        }
        checked += 1;
        if SHORT.iter().any(|(s, _)| *s == id) {
            continue;
        }
        let Some(body) = crate::widgets::fill_probe::get(&h.ctx, layer) else {
            problems.push(format!(
                "“{id}”: no filling body was drawn (`widgets::window_body` / `window_fill`) — the \
                 window's content cannot take its area"
            ));
            continue;
        };
        let actions = matches!(closes, crate::reach_tests::Closes::Action);
        // the action row and the window's margin below the body
        let slack = if actions { 34.0 + 30.0 } else { 30.0 };
        if body.bottom() < r1.bottom() - slack || body.width() < r1.width() - 40.0 {
            problems.push(format!(
                "“{id}”: the window is {:.0}×{:.0} at {:?}, but its body covers only {:.0}×{:.0} down to \
                 y={:.0} — an enlarged window must give its room to the body",
                r1.width(),
                r1.height(),
                r1.min,
                body.width(),
                body.height(),
                body.bottom()
            ));
        }
    }
    assert!(checked >= 12, "only {checked} windows were enlarged — the openers were lost");
    assert!(problems.is_empty(), "windows do not fill their area:\n  {}", problems.join("\n  "));
}

/// Test 1126 (windows-fill-their-area): a window keeps the size the person gave it — per FAMILY
/// for the windows opened per object, by its own id for a single window.
#[test]
fn t1126_a_window_keeps_the_size_it_was_given() {
    use kt_model::GeneratorKind;
    let mut h = harness_sized(1920.0, 1200.0);
    let fx_layer = |clip: kt_model::ClipId| egui::LayerId::new(egui::Order::Middle, egui::Id::new(("fx", clip)));
    h.state_mut().create_generator(GeneratorKind::solid(200, 40, 40));
    h.step();
    let first_id = h.state().model.project().clips[0].id;
    h.state_mut().fx_editors.clear();
    h.state_mut().open_fx_editor(first_id);
    for _ in 0..5 {
        h.step();
    }
    let first = fx_layer(first_id);
    let r0 = window_rect(&h, first);
    let big = enlarge(&mut h, first, r0.width() + 500.0, r0.height() + 300.0);
    assert!(big.width() > r0.width() + 300.0, "the effects window did not grow: {r0:?} → {big:?}");
    let fam = || egui::Id::new(("kt_win_size", "fx"));
    let kept = h.ctx.data_mut(|d| d.get_persisted::<egui::Vec2>(fam()));
    assert!(
        kept.is_some_and(|k| (k.x - big.width()).abs() < 4.0),
        "the family did not keep the size it was given: {kept:?} vs {big:?}"
    );
    // closed the way its Done/cross closes it: the window session ends
    h.state_mut().fx_editors.clear();
    h.step();
    h.step();
    // a NEW event: its effects window has never been shown
    h.state_mut().create_generator(GeneratorKind::solid(40, 200, 40));
    h.step();
    let second_id = h.state().model.project().clips.iter().map(|c| c.id).find(|c| *c != first_id).expect("a second event");
    if h.state().fx_editors.is_empty() {
        h.state_mut().open_fx_editor(second_id);
    }
    for _ in 0..5 {
        h.step();
    }
    let second = window_rect(&h, fx_layer(second_id));
    assert!(
        (second.width() - big.width()).abs() < 4.0 && (second.height() - big.height()).abs() < 4.0,
        "the new event's effects window opened at {:.0}×{:.0}, not at the {:.0}×{:.0} its family was given",
        second.width(),
        second.height(),
        big.width(),
        big.height()
    );

    // a single window: the Guide reopens at its own last size
    h.state_mut().fx_editors.clear();
    h.step();
    h.state_mut().start.guide.open = true;
    for _ in 0..4 {
        h.step();
    }
    let guide = egui::LayerId::new(egui::Order::Middle, egui::Id::new("kt_guide"));
    let g0 = window_rect(&h, guide);
    let g1 = enlarge(&mut h, guide, g0.width() + 400.0, g0.height() + 250.0);
    assert!(g1.width() > g0.width() + 200.0, "the Guide did not grow: {g0:?} → {g1:?}");
    h.state_mut().start.guide.open = false;
    h.step();
    h.step();
    h.state_mut().start.guide.open = true;
    for _ in 0..3 {
        h.step();
    }
    let g2 = window_rect(&h, guide);
    assert!(
        (g2.width() - g1.width()).abs() < 4.0 && (g2.height() - g1.height()).abs() < 4.0,
        "the Guide reopened at {g2:?}, not at the size it was left {g1:?}"
    );
}

/// Fixed sizes allowed in kt-app code: (file, the text on the line, why). Everything else that
/// matches `FIXED` is a cap or a fixed column the window cannot grow past.
const ALLOWED_FIXED: &[(&str, &str, &str)] = &[
    ("app.rs", "set_max_width(560.0)", "the recovery offer is a modal (egui's Modal has no resize): its text wraps at a reading width"),
    ("app.rs", "set_max_width(440.0)", "the question on close is a modal (egui's Modal has no resize): its text wraps at a reading width"),
    ("app.rs", "desired_width(180.0)", "the paste-prefix field shares its row with the side switch after it; an endless field would push the switch out"),
    ("automation.rs", "desired_width(220.0)", "the formula field shares its row with its marks; R.67 item 3 was fields growing past the window"),
    ("bin_panel.rs", "desired_width(150.0)", "the media bin's search: a panel whose width the owner set (R.13), not a window"),
    ("pubparam.rs", "desired_width(180.0)", "a handle's name in the author's list shares its row with the move buttons and the default"),
    ("pubparam.rs", "desired_width(200.0)", "a text value in a parameter grid: egui's Grid sizes a column by its widest cell, an endless field takes the row"),
    ("settings_window.rs", "desired_width(56.0)", "a frame-size number field sized for its longest value (five digits)"),
    ("settings_window.rs", "desired_width(48.0)", "the custom frame-rate field sized for its longest value"),
    ("settings_window.rs", "desired_width(120.0)", "the template name field shares its row with Save and Cancel"),
    ("timeline.rs", "desired_width(140.0)", "an inline name field on the timeline itself (a marker, an area): a panel, not a window"),
    ("timeline.rs", "desired_width(120.0)", "an inline name field on the timeline itself (a track label): a panel, not a window"),
    ("track_window.rs", "desired_width(160.0)", "the track name in the properties grid: egui's Grid sizes a column by its widest cell"),
    ("widgets.rs", "desired_width(72.0)", "a number field of the shared row sized for its longest value (R.33, one widget per meaning)"),
    ("widgets.rs", "content_rect().height() * 0.62", "a modal's body (no resize) stays content-sized and capped by the screen so its buttons show"),
    ("widgets.rs", "desired_width(220.0)", "the media picker's search sets its popup's width: a popup has no window to fill"),
];

/// What counts as a fixed size: a screen fraction, a fixed scroll height or width, a fixed or
/// minimum column width, a fixed field width.
const FIXED: &[&str] = &[
    "content_rect().height() *",
    "content_rect().width() *",
    ".max_height(",
    ".max_width(",
    "set_width(",
    "set_min_width(",
    "set_max_width(",
    "desired_width(",
];

fn fixed_number(line: &str, pat: &str) -> bool {
    let Some(p) = line.find(pat) else { return false };
    if pat.starts_with("content_rect") {
        return true;
    }
    let arg = line[p + pat.len()..].trim_start();
    // a literal number or a SHOUTING constant is fixed; an expression of the room is not
    arg.chars().next().is_some_and(|c| c.is_ascii_digit())
        || (arg.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && arg.chars().take_while(|c| *c != ')').all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()))
}

/// Test 1127 (windows-fill-their-area): a fixed size in kt-app code carries a reason.
#[test]
fn t1127_a_fixed_size_carries_a_reason() {
    let dir = crate::reach_tests::workspace_root().join("apps/kt-app/src");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .expect("kt-app sources")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();
    let mut bad = Vec::new();
    let mut used = vec![false; ALLOWED_FIXED.len()];
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name.ends_with("_tests.rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("source");
        let code = src.split("\n#[cfg(test)]").next().unwrap_or(&src);
        for (n, line) in code.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for pat in FIXED {
                if !fixed_number(line, pat) {
                    continue;
                }
                match ALLOWED_FIXED.iter().position(|(f, t, _)| *f == name && line.contains(t)) {
                    Some(i) => used[i] = true,
                    None => bad.push(format!("{name}:{}: {}", n + 1, line.trim())),
                }
            }
        }
    }
    for (i, (f, t, why)) in ALLOWED_FIXED.iter().enumerate() {
        assert!(why.chars().count() >= 25, "{f} “{t}”: the reason “{why}” is too short");
        assert!(used[i], "{f} “{t}” is allowed, but the code no longer has it — remove the entry");
    }
    assert!(
        bad.is_empty(),
        "fixed sizes without a reason (a window cannot grow past them — kkt-26.11.01 item 12); make \
         them follow the room or list them in ALLOWED_FIXED with why:\n  {}",
        bad.join("\n  ")
    );
}

/// Open the Guide on a page, in a harness of the given size, at the given window size.
fn guide_on(w: f32, hgt: f32, file: &str, enlarge_to: Option<(f32, f32)>) -> Harness<'static, KtApp> {
    let mut h = harness_sized(w, hgt);
    let pi = crate::guide::pages().iter().position(|p| p.file == file).expect("the page");
    h.state_mut().start.guide.open = true;
    h.state_mut().start.guide.page = pi;
    for _ in 0..4 {
        h.step();
    }
    if let Some((ew, eh)) = enlarge_to {
        enlarge(&mut h, guide_layer(), ew, eh);
    }
    crate::guide::table_probe::clear();
    h.step();
    h.step();
    h
}

fn guide_layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Middle, egui::Id::new("kt_guide"))
}

/// Test 1128 (windows-fill-their-area): a guide table has columns from its content.
#[test]
fn t1128_guide_tables_take_column_widths_from_their_content() {
    // the pure rule first: room to spare is shared by the natural widths; too little room keeps
    // every column at its longest word and gives the rest to the columns with more text
    let w = crate::guide::column_widths(&[60.0, 80.0], &[100.0, 300.0], 800.0);
    assert!((w[0] - 200.0).abs() < 0.5 && (w[1] - 600.0).abs() < 0.5, "spare room by natural widths: {w:?}");
    let w = crate::guide::column_widths(&[60.0, 80.0], &[100.0, 300.0], 240.0);
    assert!(w[0] >= 60.0 && w[1] >= 80.0 && (w.iter().sum::<f32>() - 240.0).abs() < 0.5, "narrow: {w:?}");
    assert!(w[1] - 80.0 > w[0] - 60.0, "the long column did not take the room: {w:?}");

    let page = crate::guide::pages().iter().find(|p| p.file == "shortcuts.md").expect("the shortcuts page");
    let tables: Vec<usize> = page
        .blocks
        .iter()
        .filter_map(|b| match b {
            crate::guide::Block::Table(rows) => Some(rows.iter().map(|r| r.len()).max().unwrap_or(0)),
            _ => None,
        })
        .collect();
    assert!(!tables.is_empty(), "the shortcuts page has no table — the scene is wrong");

    // wide: the Guide enlarged in a large screen
    let h = guide_on(1700.0, 1100.0, "shortcuts.md", Some((1600.0, 1000.0)));
    let drawn = crate::guide::table_probe::all();
    assert!(!drawn.is_empty(), "no table was drawn on the shortcuts page");
    let win = window_rect(&h, guide_layer());
    for t in &drawn {
        assert!(tables.contains(&t.widths.len()), "a table lost columns: {} drawn, the page has {tables:?}", t.widths.len());
        for c in 0..t.widths.len() {
            assert!(t.widths[c] + 0.5 >= t.min[c], "column {c} is narrower than its longest word: {t:?}");
        }
        // a SHORT first column (keys, a name) never wraps when the window is wide; a first
        // column of long phrases shares the room like any other
        assert!(
            t.nat[0] > t.room * 0.4 || t.widths[0] + 1.0 >= t.nat[0],
            "the first column wraps in a wide window: {:.0} of {:.0} — “their first column is narrow” (item 12)",
            t.widths[0],
            t.nat[0]
        );
        assert!(t.rect.width() <= t.room + 1.0 && t.rect.right() <= win.right(), "a table runs past the page: {t:?}");
    }
    // the cells are nodes a screen reader (and this test) can read
    use egui_kittest::kittest::Queryable;
    assert!(h.query_all_by_label_contains("play / pause").count() > 0, "a table cell is not readable");

    // narrow: the default Guide in a small screen — cells wrap, the table stays within its page
    let _h = guide_on(1000.0, 700.0, "shortcuts.md", None);
    for t in crate::guide::table_probe::all() {
        let min_sum: f32 = t.min.iter().sum();
        if min_sum <= t.room {
            assert!(t.rect.width() <= t.room + 1.0, "a narrow table runs past the page: {t:?}");
        }
        for c in 0..t.widths.len() {
            assert!(t.widths[c] + 0.5 >= t.min[c], "narrow: column {c} under its longest word: {t:?}");
        }
    }
}

/// The card of an index entry: the largest node named by its "what" (the card is a button with
/// that name; the label inside it has the same text).
fn card_rect(h: &Harness<'static, KtApp>, what: &str) -> egui::Rect {
    use egui_kittest::kittest::Queryable;
    h.query_all_by_label(what)
        .map(|n| n.rect())
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap_or_else(|| panic!("no card “{what}”"))
}

/// Test 1129 (windows-fill-their-area): "I want to…" entries are separate cards.
#[test]
fn t1129_index_entries_are_separate_cards() {
    use egui_kittest::kittest::Queryable;
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let mut h = guide_on(1700.0, 1100.0, "getting-started.md", Some((1400.0, 1000.0)));
    crate::i18n::set_lang(crate::i18n::Lang::En);
    h.step();
    h.step();
    let idx = crate::guide::index();
    let mut rects: Vec<(egui::Rect, &str, &str)> = idx.iter().map(|e| (card_rect(&h, e.want), e.want, e.how)).collect();
    rects.sort_by(|a, b| a.0.top().total_cmp(&b.0.top()));
    for w in rects.windows(2) {
        let (a, b) = (w[0].0, w[1].0);
        assert!(b.top() - a.bottom() >= 1.0, "cards “{}” and “{}” touch or overlap: {a:?} {b:?}", w[0].1, w[1].1);
    }
    for (r, want, how) in &rects {
        let what = h
            .query_all_by_label(want)
            .map(|n| n.rect())
            .min_by(|a, b| a.area().total_cmp(&b.area()))
            .unwrap();
        let where_ = h.get_by_label_contains(how).rect();
        assert!(r.contains_rect(where_) && r.contains_rect(what), "“{want}”: its texts are outside its card");
        assert!(where_.top() >= what.bottom() - 1.0, "“{want}”: the “where” is not under the “what”");
    }
    // a click on the card's lower line opens its page at its heading
    let loop_e = idx.iter().find(|e| e.anchor == "markers-and-the-region").expect("the loop entry");
    h.get_by_label_contains(loop_e.how).click();
    h.step();
    h.step();
    let g = &h.state().start.guide;
    assert_eq!(crate::guide::pages()[g.page].file, loop_e.file, "the card's click opened another page");
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Test 1130 (windows-fill-their-area): the Guide's sides grow with the window.
#[test]
fn t1130_the_guides_sides_grow_with_the_window() {
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let mut h = guide_on(1700.0, 1100.0, "timeline.md", None);
    let first = crate::guide::index()[0].want;
    let small_card = card_rect(&h, first);
    let small_win = window_rect(&h, guide_layer());
    let small_body = crate::widgets::fill_probe::get(&h.ctx, guide_layer()).expect("the Guide's bodies");
    assert!(small_body.bottom() >= small_win.bottom() - 30.0, "the Guide's lists stop short: {small_body:?} in {small_win:?}");
    let big_win = enlarge(&mut h, guide_layer(), 1600.0, 1000.0);
    h.step();
    let big_card = card_rect(&h, first);
    let big_body = crate::widgets::fill_probe::get(&h.ctx, guide_layer()).expect("the Guide's bodies");
    assert!(big_card.width() > small_card.width() + 20.0, "the index side did not widen: {small_card:?} → {big_card:?}");
    assert!(big_card.width() <= 420.0, "the index side passed its ceiling: {big_card:?}");
    assert!(
        big_body.width() >= big_win.width() - 40.0 && big_body.bottom() >= big_win.bottom() - 30.0,
        "the enlarged Guide's sides do not fill it: {big_body:?} in {big_win:?}"
    );
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// The open picker's frame and the most effect cards one row of the gallery shows (the effects'
/// name buttons inside the popup that share a top).
fn picker_state(h: &Harness<'static, KtApp>) -> (egui::Rect, usize) {
    use egui_kittest::kittest::Queryable;
    let popup = h.ctx.data(|d| d.get_temp::<egui::Rect>(crate::fx_registry::popup_id())).expect("the picker is open");
    let mut tops: Vec<f32> = Vec::new();
    for e in crate::fx_registry::registry().iter().filter(|e| e.video) {
        for n in h.query_all_by_label(e.name) {
            let r = n.rect();
            if popup.contains_rect(r) {
                tops.push(r.top().round());
            }
        }
    }
    let row = tops.iter().map(|t| tops.iter().filter(|u| (*u - t).abs() < 2.0).count()).max().unwrap_or(0);
    (popup, row)
}

/// Test 1131 (windows-fill-their-area): the picker gallery follows the window and the screen.
#[test]
fn t1131_the_picker_gallery_follows_its_window() {
    use egui_kittest::kittest::Queryable;
    use kt_model::GeneratorKind;
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let mut h = harness_sized(1920.0, 1200.0);
    h.state_mut().create_generator(GeneratorKind::solid(200, 40, 40));
    h.step();
    let id = h.state().model.project().clips[0].id;
    h.state_mut().open_fx_editor(id);
    for _ in 0..5 {
        h.step();
    }
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new(("fx", id)));
    let add = crate::i18n::tr(crate::i18n::k::fxreg::ADD).to_string();
    h.get_by_label(&add).click();
    h.step();
    h.step();
    let win0 = window_rect(&h, layer);
    let (p0, cells0) = picker_state(&h);
    assert!(p0.width() >= crate::fx_registry::GALLERY_W, "the picker is under its floor: {p0:?}");
    assert!(p0.bottom() <= h.ctx.content_rect().bottom() + 0.5, "the picker runs off the screen: {p0:?}");
    assert!(p0.height() > 500.0, "the picker does not use the screen below it: {p0:?} (it was 450 + 160 fixed)");
    h.event(egui::Event::Text("z".into()));
    h.step();
    h.step();
    let pt = h.ctx.data(|d| d.get_temp::<egui::Rect>(crate::fx_registry::popup_id())).expect("the picker is open");
    assert!((pt.width() - p0.width()).abs() <= 1.0, "typing changed the picker's width: {p0:?} → {pt:?}");
    h.key_press(egui::Key::Backspace);
    h.step();
    // the button closes its own popup (Esc would close the window on top — the effects window)
    h.get_by_label(&add).click();
    h.step();
    h.step();
    let win1 = enlarge(&mut h, layer, 1400.0, win0.height() + 200.0);
    assert!(win1.width() > win0.width() + 600.0, "the effects window did not widen: {win0:?} → {win1:?}");
    h.get_by_label(&add).click();
    h.step();
    h.step();
    let (p1, cells1) = picker_state(&h);
    assert!(p1.width() > p0.width() + 500.0, "the picker did not follow its window: {p0:?} → {p1:?}");
    assert!(p1.width() <= win1.width(), "the picker is wider than its window: {p1:?} vs {win1:?}");
    assert!(cells0 > 0, "no effect card was found in the narrow picker");
    assert!(cells1 > cells0, "the wide gallery shows no more effect cards per row: {cells0} → {cells1}");
}

/// Test 1132 (windows-fill-their-area): the bottom toolbar never cuts a button off.
#[test]
fn t1132_the_toolbar_overflows_into_a_menu() {
    use egui_kittest::kittest::Queryable;
    let more = crate::icons::MORE;
    let render = crate::icons::RENDER;
    let mut h = harness_sized(1100.0, 800.0);
    for _ in 0..3 {
        h.step();
    }
    let screen = h.ctx.content_rect();
    let m = h.query_by_label(more).map(|n| n.rect()).expect("no “more” button in a 1100-wide window");
    assert!(screen.contains_rect(m), "the “more” button is cut off: {m:?}");
    assert!(
        h.query_all_by_label(render).all(|n| !screen.intersects(n.rect()) || screen.contains_rect(n.rect())),
        "Render is cut off at the window's edge"
    );
    assert!(h.query_by_label(render).is_none(), "Render is both in the row and expected in the menu");
    h.get_by_label(more).click();
    h.step();
    h.step();
    let r = h.get_by_label(render).rect();
    assert!(screen.contains_rect(r), "Render in the menu is off screen: {r:?}");
    h.get_by_label(render).click();
    h.step();
    h.step();
    assert!(h.state().render_setup.open, "Render in the “more” menu did not open the render window");

    let mut wide = harness_sized(1700.0, 900.0);
    for _ in 0..3 {
        wide.step();
    }
    assert!(wide.query_by_label(more).is_none(), "a “more” button in a window wide enough for the row");
    let r = wide.get_by_label(render).rect();
    assert!(wide.ctx.content_rect().contains_rect(r), "Render is not in the wide row: {r:?}");
}



