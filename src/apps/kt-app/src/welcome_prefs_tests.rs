//! welcome-window-and-settings (kkt-26.11.01 items 1 and 13, registry rows K1, K13): the start
//! window in three panes that scroll apart, What's new as the Guide draws it, the lessons in the
//! Start pane; File › Preferences… with every preference, one switch in one place, and the
//! preferences surviving a project open.
//!
//! The owner: "Стартовое окно уродское … не ресайзится и в нем whats new суженный, и при скроле
//! whatsnew скролится всё подряд" and "нафига мне галочка welcome window at start И в самом window
//! и еще продублирована в View?!?! Да и пора бы формировать раздел настроек в file".

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::layout_tests::harness_sized;
use crate::reach_tests::open_preferences;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

fn temp_store(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("kt-wp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A store with `n` recent rows (the files need not exist: a missing row is drawn flagged).
fn store_with_recent(dir: &std::path::Path, n: usize) -> crate::user_store::StartStore {
    let mut s = crate::user_store::StartStore::at(Some(dir.to_path_buf()));
    s.begin_session(None);
    for i in (0..n).rev() {
        s.touch_recent(&dir.join(format!("p{i}.kt")), None);
    }
    s
}

fn welcome_layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Middle, egui::Id::new("kt_welcome"))
}

fn rect_of(h: &Harness<'static, KtApp>, label: &str) -> egui::Rect {
    // a recent card is a button named by its project, and its name is also a label under it
    let card = h.query_by_role_and_label(egui::accesskit::Role::Button, label);
    card.or_else(|| h.query_by_label(label)).unwrap_or_else(|| panic!("no “{label}” in the tree")).rect()
}

fn wheel(h: &mut Harness<'static, KtApp>, at: egui::Pos2, dy: f32) {
    h.hover_at(at);
    h.step();
    for _ in 0..4 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, dy),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    h.step();
}

fn r_right(h: &Harness<'static, KtApp>) -> f32 {
    crate::window_fill_tests::window_rect(h, welcome_layer()).right()
}

/// How many recent cards share the first card's row.
fn cards_in_first_row(h: &Harness<'static, KtApp>) -> usize {
    let tops: Vec<f32> = (0..12)
        .filter_map(|i| h.query_by_role_and_label(egui::accesskit::Role::Button, &format!("p{i}")).map(|n| n.rect().top()))
        .collect();
    let first = tops[0];
    tops.iter().filter(|t| (**t - first).abs() < 1.0).count()
}

/// Test 1290 (welcome-window-and-settings, ui-reach): three panes side by side, each scrolls
/// alone, the window fits the screen at its default and gives its room to the panes when enlarged.
///
/// - **PATH** start Kinetren → the welcome window: "Start" on the left, "Recent" with poster cards
///   in the middle, "What's new" on the right → turn the wheel over What's new → only the notes
///   move → turn it over Recent → only the cards move → drag the window's corner to the screen's
///   corner → the corner stays under the pointer, the cards take the width in more columns, the notes get a wider and taller column
#[test]
fn t1290_welcome_panes_scroll_apart_fit_and_grow() {
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let dir = temp_store("t1290");
    let mut h = harness_sized(1440.0, 860.0);
    h.state_mut().start.store = store_with_recent(&dir, 12);
    let ctx = h.ctx.clone();
    h.state_mut().open_welcome(&ctx, None, false);
    for _ in 0..5 {
        h.step();
    }
    let win = crate::window_fill_tests::window_rect(&h, welcome_layer());
    assert!(h.ctx.content_rect().contains_rect(win), "the welcome window sticks out of the screen: {win:?}");

    // the panes, left to right: the tour row (Start), the first card (Recent), the English line
    // (What's new) — each inside its own scroll
    let tour = || tr(k::welcome::TOUR).to_string();
    let english = tr(k::guide::ENGLISH);
    let (s0, r0, n0) = (rect_of(&h, &tour()), rect_of(&h, "p0"), rect_of(&h, english));
    assert!(s0.right() < r0.left() && r0.right() < n0.left(), "the panes are not Start | Recent | What's new: {s0:?} {r0:?} {n0:?}");
    for (name, title) in [("Start", tr(k::welcome::START)), ("Recent", tr(k::welcome::RECENT))] {
        assert!(h.query_by_label(title).is_some(), "the {name} pane has no title");
    }
    let body = crate::widgets::fill_probe::get(&h.ctx, welcome_layer()).expect("the panes are filling bodies");
    assert!(body.bottom() > win.bottom() - 70.0, "the panes stop short of the action row: {body:?} in {win:?}");

    // the wheel over What's new moves only What's new
    wheel(&mut h, n0.center() + egui::vec2(0.0, 120.0), -240.0);
    let (s1, r1) = (rect_of(&h, &tour()), rect_of(&h, "p0"));
    let n1 = h.query_by_label(english).map(|n| n.rect());
    assert!(n1.is_none_or(|n| n.top() < n0.top() - 50.0), "What's new did not scroll: {n0:?} → {n1:?}");
    assert_eq!((s1.top(), r1.top()), (s0.top(), r0.top()), "scrolling What's new moved another pane");

    // the wheel over Recent moves only Recent
    wheel(&mut h, r0.center(), -240.0);
    let r2 = h.query_by_role_and_label(egui::accesskit::Role::Button, "p0").map(|n| n.rect());
    assert!(r2.is_none_or(|r| r.top() < r0.top() - 50.0), "Recent did not scroll: {r0:?} → {r2:?}");
    assert_eq!(rect_of(&h, &tour()).top(), s0.top(), "scrolling Recent moved the Start pane");
    wheel(&mut h, r0.center(), 2000.0);

    // enlarged on a big screen: the panes take the room
    let mut h2 = harness_sized(1920.0, 1200.0);
    h2.state_mut().start.store = store_with_recent(&dir, 12);
    let ctx = h2.ctx.clone();
    h2.state_mut().open_welcome(&ctx, None, false);
    for _ in 0..5 {
        h2.step();
    }
    let before = cards_in_first_row(&h2);
    // the What's new pane spans from its first line's left edge to the window's right edge
    let news_before = r_right(&h2) - rect_of(&h2, english).left();
    let r = crate::window_fill_tests::window_rect(&h2, welcome_layer());
    let big = crate::window_fill_tests::enlarge(&mut h2, welcome_layer(), 1900.0, 1150.0);
    for _ in 0..3 {
        h2.step();
    }
    assert!(big.width() > r.width() + 200.0 && big.height() > r.height() + 150.0, "the window did not grow: {r:?} → {big:?}");
    // the corner stays under the pointer: an anchored window re-centred itself every frame and
    // its corner ran half as far as the hand (the live `welcome` step: dragged 600, grew 260)
    let screen = h2.ctx.content_rect();
    let to = egui::pos2((r.left() + 1900.0).min(screen.right() - 8.0), (r.top() + 1150.0).min(screen.bottom() - 8.0));
    let off = big.right_bottom() - to;
    assert!(off.x.abs() < 8.0 && off.y.abs() < 8.0, "the corner did not follow the pointer: {big:?}, the pointer at {to:?}");
    let body = crate::widgets::fill_probe::get(&h2.ctx, welcome_layer()).unwrap();
    assert!(body.bottom() > big.bottom() - 70.0, "the panes did not take the height: {body:?} in {big:?}");
    assert!(cards_in_first_row(&h2) > before, "Recent holds no more cards per row: {before}");
    let news = r_right(&h2) - h2.query_by_label(english).unwrap().rect().left();
    assert!(news >= 300.0 && news > news_before + 40.0, "What's new is {news:.0} wide (was {news_before:.0})");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 1291 (welcome-window-and-settings, ui-reach): What's new is the page from the running
/// version down, drawn by the Guide's renderer, headed by the newer-release line.
///
/// - **PATH** Help › What's new → the welcome window opens with the What's new pane at its top:
///   "Kinetren kt26.12 is out — Releases page", then "What's new in kt26.11" and its items →
///   scroll the pane down → the "kt26.10" notes follow
#[test]
fn t1291_whats_new_is_the_page_drawn_by_the_guide() {
    use crate::guide::Block;
    use crate::update_check::testing::{Canned, temp_dir};
    use crate::update_check::{API_URL, Checker, Consent, record_in, utc_day};
    use std::sync::Arc;
    crate::i18n::set_lang(crate::i18n::Lang::En);
    // the page: the running section first, every older `##` section after it
    let (label, blocks) = crate::guide::whats_new_from(kt_version::SHORT);
    let (_, only) = crate::guide::whats_new(kt_version::SHORT);
    assert!(blocks.len() > only.len(), "the pane holds only one section");
    assert_eq!(&blocks[..only.len()], only, "the page does not start at the running version");
    let older: Vec<&str> = blocks
        .iter()
        .filter_map(|b| match b {
            Block::Heading { level: 2, text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(!older.is_empty() && !older.contains(&label.as_str()), "older sections: {older:?}");

    // the newer release named by the state already on disk
    let dir = temp_dir("t1291");
    record_in(&dir, &utc_day(std::time::SystemTime::now()), Some("kt26.12.01-bbbbbbbb")).unwrap();
    let on = Consent { choice: true, block: None, url: API_URL.into(), ignore_day: false };
    let u = Arc::new(Checker::new(on, "kt26.10.20-aaaaaaaa", dir.clone(), Box::new(Canned::ok("{}"))));
    let mut h = harness_sized(1440.0, 860.0);
    h.state_mut().update = Some(u);
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label(tr(k::help::TITLE)).click();
    h.step();
    h.step();
    h.get_by_label(tr(k::help::WHATS_NEW)).click();
    for _ in 0..5 {
        h.step();
    }
    assert!(h.state().start.welcome.open, "Help › What's new did not open the window");
    let line = crate::update_check::title_hint("kt26.12.01-bbbbbbbb");
    let texts = crate::reach_tests::painted_texts(&h);
    let at = |s: &str| texts.iter().find(|t| t.src == s).map(|t| t.rect);
    let upd = at(&line).unwrap_or_else(|| panic!("no update line “{line}”"));
    let head = if label == kt_version::SHORT {
        crate::tf!(k::welcome::WHATS_NEW, &label)
    } else {
        crate::tf!(k::welcome::WHATS_NEW_OLDER, &label)
    };
    let title = at(&head).expect("the pane's title");
    assert!(upd.top() > title.bottom() && upd.top() - title.bottom() < 40.0, "the pane is not at its top: {title:?} / {upd:?}");
    // the Guide's renderer: no raw markup is painted — a `<kbd>` is a keycap
    assert!(texts.iter().all(|t| !t.src.contains("<kbd>") && !t.src.contains("**")), "raw markup in What's new");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 1292 (welcome-window-and-settings, ui-reach): the five lessons with their progress, a
/// lesson opens from its row; template rows draw their frame's shape.
///
/// - **PATH** Help › Welcome window… → in "Start" the lessons "1 · The first edit ✓", "2 · Effects
///   and automation 1/3", … → click "3 · Transitions" → the window closes, the lesson project
///   opens and the lessons panel reads "Lesson 3 of 5 · Transitions"
#[test]
fn t1292_lessons_and_template_shapes_in_the_start_pane() {
    crate::plugins::install();
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let dir = temp_store("t1292");
    let mut store = crate::user_store::StartStore::at(Some(dir.clone()));
    store.begin_session(None);
    let l1 = crate::lessons::LESSONS[0].steps.len() as u32;
    let l2 = crate::lessons::LESSONS[1].steps.len() as u32;
    for s in 1..=l1 {
        store.finish_step(1, s);
    }
    store.finish_step(2, 1);
    let mut h = harness_sized(1440.0, 860.0);
    h.state_mut().start.store = store;
    let ctx = h.ctx.clone();
    h.state_mut().open_welcome(&ctx, None, false);
    for _ in 0..4 {
        h.step();
    }
    let t = |n: usize| tr(crate::lessons::LESSONS[n].title);
    assert!(h.query_by_label(&format!("1 · {} {}", t(0), crate::icons::CHECK)).is_some(), "lesson 1 is not marked done");
    assert!(h.query_by_label(&format!("2 · {} 1/{l2}", t(1))).is_some(), "lesson 2 does not show its progress");
    for n in 2..crate::lessons::LESSONS.len() {
        assert!(h.query_by_label_contains(&format!("{} · {}", n + 1, t(n))).is_some(), "lesson {} is not listed", n + 1);
    }
    // the frame glyph keeps the template's aspect
    let p = egui::Painter::new(h.ctx.clone(), egui::LayerId::debug(), egui::Rect::EVERYTHING);
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 40.0));
    for tpl in crate::welcome::BUILTIN {
        let g = crate::welcome::frame_glyph(&p, rect, tpl.width, tpl.height, false);
        let want = tpl.width as f32 / tpl.height as f32;
        assert!((g.width() / g.height() - want).abs() < 0.02, "{}: the glyph is {:?}", tpl.label, g.size());
    }
    h.get_by_label_contains(&format!("3 · {}", t(2))).click();
    for _ in 0..4 {
        h.step();
    }
    assert!(!h.state().start.welcome.open, "the window stays open over the lesson");
    assert!(h.state().start.lessons.open && h.state().start.lessons.lesson == 3, "lesson 3 did not open");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 1293 (welcome-window-and-settings, ui-reach): File › Preferences… holds every preference,
/// and each acts at once.
///
/// - **PATH** File › Preferences… → a window "Preferences" with Start, Interface, Privacy and
///   updates, FFmpeg → tick "Economy UI" → pick "Русский" → the menus read "Файл / Правка / Вид" →
///   untick "Open the welcome window at start" → untick both privacy boxes → press "FFmpeg…" → the
///   FFmpeg window opens over it
#[test]
fn t1293_preferences_window_holds_every_preference() {
    use crate::telemetry::testing::Recording;
    use crate::telemetry::{EnvView, Telemetry, new_uuid, resolve_consent};
    use crate::update_check::testing::{Canned, temp_dir};
    use crate::update_check::{API_URL, Checker, Consent};
    use std::sync::Arc;
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let dir = temp_store("t1293");
    let udir = temp_dir("t1293");
    let mut h = harness_sized(1440.0, 860.0);
    h.state_mut().start.store = crate::user_store::StartStore::at(Some(dir.clone()));
    let t = Arc::new(
        Telemetry::new(resolve_consent(Some("phc_test"), &EnvView::default(), Some("on")), new_uuid(), Box::new(Recording::default()))
            .with_key("phc_test"),
    );
    let on = Consent { choice: true, block: None, url: API_URL.into(), ignore_day: false };
    let u = Arc::new(Checker::new(on, "kt26.10.20-aaaaaaaa", udir.clone(), Box::new(Canned::ok("{}"))));
    h.state_mut().telemetry = Some(t.clone());
    h.state_mut().update = Some(u.clone());
    open_preferences(&mut h);
    assert!(h.ctx.memory(|m| m.area_rect(egui::Id::new("kt_prefs"))).is_some(), "no `kt_prefs` window");
    for key in [k::prefs::SECTION_START, k::prefs::SECTION_INTERFACE, k::prefs::SECTION_PRIVACY, k::prefs::SECTION_FFMPEG] {
        assert!(h.query_by_label(tr(key)).is_some(), "the window has no section “{}”", tr(key));
    }
    h.get_by_label(tr(k::prefs::ECONOMY)).click();
    h.step();
    assert!(h.state().start.store.start.economy_ui, "Economy UI did not switch on");
    h.get_by_label(tr(k::prefs::WELCOME_AT_START)).click();
    h.step();
    assert!(!h.state().start.store.start.show_at_start, "the start switch did not turn off");
    h.get_by_label(tr(k::telemetry::CHECKBOX)).click();
    h.step();
    h.get_by_label(tr(k::update::CHECKBOX)).click();
    h.step();
    assert!(!t.consent().on() && t.stored_value() == Some("off"), "statistics did not switch off");
    assert!(!u.consent().on() && u.stored_value() == Some("off"), "the daily check did not switch off");
    h.get_by_label(tr(k::prefs::FFMPEG_OPEN)).click();
    h.step();
    h.step();
    assert!(h.state().ffmpeg_ui.open, "FFmpeg… did not open the FFmpeg window");
    h.state_mut().ffmpeg_ui.open = false;
    h.step();
    // the language: the combo shows Auto, the list names each language by itself
    h.get_by_role_and_label(egui::accesskit::Role::ComboBox, tr(k::prefs::LANGUAGE)).click();
    h.step();
    h.step();
    h.get_by_label(crate::i18n::Lang::Ru.endonym()).click();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(crate::i18n::current(), crate::i18n::Lang::Ru, "the language did not switch");
    assert!(h.query_by_label(tr(k::menu::file::TITLE)).is_some_and(|_| tr(k::menu::file::TITLE) == "Файл"));
    assert_eq!(h.state().start.store.start.lang.as_deref(), Some("ru"));
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&udir);
}

/// The preference labels of the program in the current language.
fn pref_labels() -> Vec<&'static str> {
    vec![
        tr(k::prefs::WELCOME_AT_START),
        tr(k::prefs::LANGUAGE),
        tr(k::prefs::ECONOMY),
        tr(k::telemetry::CHECKBOX),
        tr(k::update::CHECKBOX),
        tr(k::prefs::FFMPEG_OPEN),
    ]
}

/// Test 1294 (welcome-window-and-settings, ui-reach): one switch, one place — each preference has
/// one writer, in the Preferences window; View and the welcome window hold none; the texts that
/// say where a switch is name File › Preferences….
///
/// - **PATH** open View → no switch; Help › Welcome window… → no checkbox, a "Preferences…" button;
///   File › Preferences… → every switch, once
#[test]
fn t1294_one_switch_one_place() {
    // the writers, by their call (the two start checkboxes had DIFFERENT labels for one value)
    let dir = crate::reach_tests::workspace_root().join("apps/kt-app/src");
    let writers = [".set_show_at_start(", ".set_economy_ui(", "store.set_lang(", ".set_choice("];
    let mut sites: Vec<(&str, String)> = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let path = e.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let code = src.split("\n#[cfg(test)]").next().unwrap_or(&src);
        for (n, line) in code.lines().enumerate() {
            let l = line.trim_start();
            if l.starts_with("//") || l.starts_with("pub fn") || l.starts_with("fn ") {
                continue;
            }
            for w in writers {
                if line.contains(w) {
                    sites.push((w, format!("{name}:{}", n + 1)));
                }
            }
        }
    }
    for w in writers {
        let at: Vec<&String> = sites.iter().filter(|(x, _)| *x == w).map(|(_, s)| s).collect();
        let want = if w == ".set_choice(" { 2 } else { 1 }; // statistics and the daily check
        assert_eq!(at.len(), want, "the preference writer `{w}` is called from {at:?} — one switch, one place");
        assert!(at.iter().all(|s| s.starts_with("prefs_window.rs:")), "`{w}` is written outside Preferences: {at:?}");
    }

    // the old places hold no preference
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let mut h = harness_sized(1440.0, 860.0);
    h.step();
    h.get_by_label(tr(k::menu::view::TITLE)).click();
    h.step();
    h.step();
    let view = crate::layout_tests::menu_labels(&mut h);
    for p in pref_labels() {
        assert!(!view.iter().any(|l| l.contains(p)), "View still holds “{p}”: {view:?}");
    }
    h.key_press(egui::Key::Escape);
    h.step();
    let ctx = h.ctx.clone();
    h.state_mut().open_welcome(&ctx, None, false);
    for _ in 0..4 {
        h.step();
    }
    for p in pref_labels() {
        assert!(h.query_all_by_label_contains(p).count() == 0, "the welcome window holds “{p}”");
    }
    assert!(h.query_by_label(tr(k::menu::file::PREFERENCES)).is_some(), "the welcome window has no door to Preferences");

    // the texts that say where a switch is name File › Preferences…, in every language
    let word = |w: &str| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        let names = |text: &str, s: &str| {
            text.contains(s) || s.split_whitespace().all(|w| text.split_whitespace().any(|o| word(o) == word(w)))
        };
        for (what, text) in [
            ("statistics", tr(k::telemetry::SUMMARY)),
            ("update", tr(k::update::HOW_OFF)),
            ("FFmpeg", tr(k::ffmpeg::NOT_FOUND)),
        ] {
            assert!(
                names(text, tr(k::menu::file::TITLE)) && names(text, tr(k::menu::file::PREFERENCES)),
                "{lang:?}: the {what} text does not name File › Preferences…: {text}"
            );
            let view = word(tr(k::menu::view::TITLE));
            assert!(!text.split_whitespace().any(|o| word(o) == view), "{lang:?}: the {what} text still names View: {text}");
        }
    }
    crate::i18n::set_lang(crate::i18n::Lang::En);
}

/// Test 1295 (welcome-window-and-settings, ui-reach): the language and Economy UI survive File →
/// New and a restart; a language of an earlier build is taken once. Found by this change: the two
/// were fields of the app that File → New rebuilds.
///
/// - **PATH** File › Preferences… → "Русский", tick "Economy UI" → File › New → the menus stay
///   Russian, Preferences still shows both → quit and start → Russian, Economy UI on
#[test]
fn t1295_preferences_survive_new_project_and_restart() {
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let dir = temp_store("t1295");
    let mut h = harness_sized(1440.0, 860.0);
    h.state_mut().start.store = crate::user_store::StartStore::at(Some(dir.clone()));
    open_preferences(&mut h);
    h.get_by_label(tr(k::prefs::ECONOMY)).click();
    h.step();
    h.get_by_role_and_label(egui::accesskit::Role::ComboBox, tr(k::prefs::LANGUAGE)).click();
    h.step();
    h.step();
    h.get_by_label(crate::i18n::Lang::Ru.endonym()).click();
    for _ in 0..3 {
        h.step();
    }
    h.state_mut().new_project();
    for _ in 0..3 {
        h.step();
    }
    let st = &h.state().start.store;
    assert!(st.start.economy_ui, "File → New lost Economy UI");
    assert_eq!(st.lang_choice(None), Some(crate::i18n::Lang::Ru), "File → New lost the language");
    assert_eq!(crate::i18n::current(), crate::i18n::Lang::Ru);
    // the restart reads both back
    let again = crate::user_store::StartStore::at(Some(dir.clone()));
    assert!(again.start.economy_ui, "the restart lost Economy UI");
    assert_eq!(again.lang_choice(Some("es")), Some(crate::i18n::Lang::Ru), "the store's choice wins over the former value");
    // a store with no language takes the earlier build's eframe value once
    let empty = temp_store("t1295-empty");
    let fresh = crate::user_store::StartStore::at(Some(empty.clone()));
    assert_eq!(fresh.lang_choice(Some("es")), Some(crate::i18n::Lang::Es));
    assert_eq!(fresh.lang_choice(None), None, "nothing chosen: Auto");
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&empty);
}

/// Test 1296 (welcome-window-and-settings, ui-reach): View holds only what is viewed.
///
/// - **PATH** open View → three items: "Detach preview", "Diagnostics…", "Notices"
#[test]
fn t1296_view_holds_only_what_is_viewed() {
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        let mut h = harness_sized(1440.0, 860.0);
        h.step();
        h.get_by_label(tr(k::menu::view::TITLE)).click();
        h.step();
        h.step();
        let mut labels: Vec<String> = h
            .query_all_by_label_contains("")
            .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
            .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
            .filter(|l| [tr(k::menu::view::DETACH_PREVIEW), tr(k::menu::view::DIAGNOSTICS), tr(k::menu::view::NOTICES)].contains(&l.as_str())
                || pref_labels().iter().any(|p| l.contains(p)))
            .collect();
        labels.sort();
        let mut want = vec![
            tr(k::menu::view::DETACH_PREVIEW).to_string(),
            tr(k::menu::view::DIAGNOSTICS).to_string(),
            tr(k::menu::view::NOTICES).to_string(),
        ];
        want.sort();
        assert_eq!(labels, want, "{lang:?}: View holds more than what is viewed");
        let all = crate::layout_tests::menu_labels(&mut h);
        for p in pref_labels() {
            assert!(!all.iter().any(|l| l.contains(p)), "{lang:?}: View holds “{p}”");
        }
    }
    crate::i18n::set_lang(crate::i18n::Lang::En);
}
