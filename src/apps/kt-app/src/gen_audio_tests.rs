//! generators-placement-and-audio (kkt-26.11.01 item 4, registry K4): "Create a generator" lives in
//! Edit, in a clip track's right-click menu (at the clicked place) and in the bin — not in File —
//! and the sound kinds are real generators. The owner: "WHY THE HELL LEAVE IT IN FILE?! … of
//! course the audio ones are needed — you hide some generators in the tests — pull them out".
//! Tests t1163–t1166; the contract — `openspec/specs/generators` and `ui-reach`.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use kt_model::*;
use kt_time::Tick;

const SOUND: &[&str] = &["tone", "audio_noise", "sweep", "beep", "silence"];

/// Hover the menu node whose label STARTS with `label` (a submenu opens on hover).
fn hover_item(h: &mut Harness<'_, KtApp>, label: &str) {
    h.query_all_by_label_contains(label)
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with(label)))
        .unwrap_or_else(|| panic!("no menu item “{label}”"))
        .hover();
    h.step();
    h.step();
}

fn right_click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
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

fn press_probe(h: &mut Harness<'_, KtApp>, name: &str) {
    let at = crate::widgets::field_probe::rect_of(name).unwrap_or_else(|| panic!("no {name}")).center();
    h.hover_at(at);
    h.step();
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.step();
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    for _ in 0..3 {
        h.step();
    }
}

/// Edit › "Create a generator…" (one item since kkt-26.11.20 A1), then the kind `tag` picked in the
/// window's Kind gallery — the person's path to any picture kind. The window stays open.
pub(crate) fn create_from_edit(h: &mut Harness<'_, KtApp>, tag: &str) {
    h.get_by_label(tr(k::menu::edit::TITLE)).click();
    h.step();
    h.step();
    hover_item(h, tr(k::bin::menu::NEW_GENERATOR));
    h.get_by_label(tr(k::bin::menu::NEW_GENERATOR)).click();
    for _ in 0..4 {
        h.step();
    }
    if crate::gen_ui::first_kind_for(ClipKind::Video).is_some_and(|kd| kd.tag() == tag) {
        return;
    }
    pick_kind(h, tag);
}

/// Pick `tag` in the open generator window's Kind gallery, scrolling the gallery to its cell.
pub(crate) fn pick_kind(h: &mut Harness<'_, KtApp>, tag: &str) {
    let cell = format!("genw.kind.{tag}");
    crate::widgets::field_probe::forget(&cell);
    press_probe(h, "genw.kind.pick");
    // a cell under the gallery's fold is not drawn: the person scrolls the gallery to it
    let pick = crate::widgets::field_probe::rect_of("genw.kind.pick").expect("the Kind field");
    let over = pick.center() + egui::vec2(0.0, 160.0);
    // (the gallery may open scrolled: down first, then back up)
    let wheel = |h: &mut Harness<'_, KtApp>, dy: f32| {
        h.hover_at(over);
        h.event(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, dy), modifiers: egui::Modifiers::NONE, phase: egui::TouchPhase::Move });
        h.step();
        h.step();
    };
    for i in 0..80 {
        let dy = if i < 40 { -60.0 } else { 60.0 };
        if crate::widgets::field_probe::rect_of(&cell).is_some() {
            // the first frame that draws a cell draws it at the fold: two more notches bring it in whole
            if i > 0 {
                wheel(h, dy);
                wheel(h, dy);
            }
            break;
        }
        wheel(h, dy);
    }
    // the wheel scrolls with an animation: the cell is pressed where it comes to rest
    for _ in 0..12 {
        h.step();
    }
    crate::widgets::field_probe::forget(&cell);
    h.step();
    press_probe(h, &cell);
}

fn kind_label(tag: &str) -> &'static str {
    crate::gen_ui::kind_name(&GeneratorKind::default_for_tag(tag).expect("a registered kind"))
}

/// The clips added since `before` (by id).
fn new_clips(h: &Harness<'_, KtApp>, before: usize) -> Vec<ClipEvent> {
    let mut v = h.state().model.project().clips.clone();
    v.sort_by_key(|c| c.id.0);
    v.split_off(before.min(v.len()))
}

/// Test 1163: the menu rows — the 19 former ones, the six shapes of gen-shapes, the four moving
/// backgrounds of gen-backgrounds and
/// the two technical ones of gen-technical (`v26.10` G5) in their order, then the five sound kinds, all in
/// the Audio group, each named in the catalog, every row and every choice variant captioned.
#[test]
fn t1163_the_sound_kinds_are_menu_rows_in_the_audio_group() {
    crate::plugins::install();
    let all = GeneratorKind::all();
    assert_eq!(all.len(), 39, "31 picture rows + 5 sound rows + 3 audio-reactive picture rows");
    let tags: Vec<&str> = all[31..36].iter().map(|k| k.tag()).collect();
    assert_eq!(tags, SOUND, "the sound rows follow the 31 picture rows in BUILTIN_IDS order");
    let react: Vec<&str> = all[36..].iter().map(|k| k.tag()).collect();
    assert_eq!(react, ["waveform", "spectrum", "levelrings"], "the audio-reactive rows close the list (gen-audio-reactive)");
    for k in &all {
        let sound = SOUND.contains(&k.tag());
        assert_eq!(crate::gen_ui::is_sound(k), sound, "{}: a sound kind is exactly a synthesizing one", k.tag());
        assert_eq!(
            crate::gen_ui::GenGroup::of(k) == crate::gen_ui::GenGroup::Audio,
            sound,
            "{}: the Audio group holds the sound kinds and only them",
            k.tag()
        );
    }
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        for k in &all[31..36] {
            let name = crate::gen_ui::kind_name(k);
            assert_ne!(name, k.tag(), "{lang:?}: {} has no catalog name", k.tag());
            let GeneratorKind::Plugin(p) = k else { unreachable!() };
            for spec in p.specs() {
                let label = if spec.ty == ParamType::Flag {
                    crate::widgets::flag_label(&spec.name)
                } else {
                    crate::widgets::param_label_scoped(ParamScope::Generator, &spec.name)
                };
                assert_ne!(label, spec.name, "{lang:?}: {}.{} has no caption", k.tag(), spec.name);
                if let ParamType::Choice { variants } = &spec.ty {
                    for v in variants.iter() {
                        assert!(
                            crate::gen_ui::choice_label(&spec.name, v).is_some(),
                            "{}.{}: the variant {v} has no caption",
                            k.tag(),
                            spec.name
                        );
                    }
                }
            }
        }
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Test 1164: Edit → "Create a generator…" with an event of audio track 2 selected and the cursor at
/// 3 s gives ONE sound clip at 3 s on audio track 2 — no video clip — in one history step, and opens
/// its form. The kind goes by the selected track (kkt-26.11.20 A1: one item, by the track); File
/// does not offer the item.
/// - **PATH** click an event on audio track 2 → click "Edit" → click "Create a generator…" (no list of
///   kinds) → a tone at the cursor on audio track 2, its window open
#[test]
fn t1164_edit_creates_a_sound_generator_at_the_cursor() {
    let mut h = crate::layout_tests::harness();
    h.state_mut().insert_lane_after_row(LaneKind::Audio, Some((LaneKind::Audio, 0)));
    let tone = crate::gen_ui::first_kind_for(ClipKind::Audio).expect("a sound kind");
    h.state_mut().create_generator_at(tone.clone(), Tick(20 * 90_000), Some((ClipKind::Audio, 1)));
    h.state_mut().fx_editors.clear();
    let sel = h.state().model.project().clips.iter().max_by_key(|c| c.id.0).map(|c| c.id).expect("the event");
    h.state().player.seek(Tick(3 * 90_000));
    h.step();
    let before = h.state().model.project().clips.len();
    // File does not offer it (its Esc clears the selection too: the event is selected after it)
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();
    h.step();
    let gen_item = tr(k::bin::menu::NEW_GENERATOR);
    assert!(
        !crate::layout_tests::menu_labels(&mut h).iter().any(|l| l.starts_with(gen_item)),
        "File still offers “Create a generator”"
    );
    h.key_press(egui::Key::Escape);
    h.step();
    h.state_mut().selected = [sel].into_iter().collect();
    h.get_by_label(tr(k::menu::edit::TITLE)).click();
    h.step();
    h.step();
    hover_item(&mut h, gen_item);
    let labels = crate::layout_tests::menu_labels(&mut h);
    assert!(!labels.iter().any(|l| l.starts_with(tr(k::genkind::GROUP_AUDIO))), "Edit lists kinds again: {labels:?}");
    h.get_by_label(gen_item).click();
    h.step();
    h.step();
    let made = new_clips(&h, before);
    assert_eq!(made.len(), 1, "one clip: a sound kind has no picture: {made:?}");
    let c = &made[0];
    assert_eq!((c.kind, c.lane, c.start), (ClipKind::Audio, 1, Tick(3 * 90_000)), "at the cursor on the selected audio track");
    assert_eq!(c.r#gen.as_ref().map(|g| g.tag()), Some(tone.tag()));
    assert!(h.state().fx_editors.iter().any(|e| e.clip == c.id), "the form opens on the new event");
    h.state_mut().undo();
    h.step();
    assert_eq!(h.state().model.project().clips.len(), before, "one history step");
}

/// Test 3712 (generator-menu-one-item, kkt-26.11.20 A1): Edit and the bin's empty-space menu hold ONE
/// "Create a generator…" item, one body: nothing selected → a solid colour on video track 1 at the
/// cursor; an event of video track 2 selected → a solid colour on video track 2. Its window opens, the
/// kind is changed there.
/// - **PATH** nothing selected → Edit → "Create a generator…" → a solid colour on V1 at the cursor, its
///   window open; click an event on V2 → right click the bin's empty space → "Create a generator…" →
///   a solid colour on V2 at the cursor
#[test]
fn t3712_edit_and_the_bin_create_by_the_selected_track() {
    let mut h = crate::layout_tests::harness();
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Video, 0)));
    h.state_mut().selected.clear();
    h.state().player.seek(Tick(4 * 90_000));
    h.step();
    let solid = crate::gen_ui::first_kind_for(ClipKind::Video).expect("a picture kind").tag();
    let before = h.state().model.project().clips.len();
    h.get_by_label(tr(k::menu::edit::TITLE)).click();
    h.step();
    h.step();
    hover_item(&mut h, tr(k::bin::menu::NEW_GENERATOR));
    h.get_by_label(tr(k::bin::menu::NEW_GENERATOR)).click();
    h.step();
    h.step();
    let made = new_clips(&h, before);
    assert_eq!(made.len(), 1, "{made:?}");
    assert_eq!((made[0].kind, made[0].lane, made[0].start), (ClipKind::Video, 0, Tick(4 * 90_000)), "nothing selected: video track 1");
    assert_eq!(made[0].r#gen.as_ref().map(|g| g.tag()), Some(solid));
    assert!(h.state().fx_editors.iter().any(|e| e.clip == made[0].id), "its window opens");
    h.state_mut().fx_editors.clear();
    // an event on V2 selected; the bin's empty space
    h.state_mut().create_generator_at(GeneratorKind::solid(9, 9, 9), Tick(30 * 90_000), Some((ClipKind::Video, 1)));
    h.state_mut().fx_editors.clear();
    let sel = h.state().model.project().clips.iter().max_by_key(|c| c.id.0).map(|c| c.id).expect("the event");
    h.state_mut().selected = [sel].into_iter().collect();
    h.step();
    let before = h.state().model.project().clips.len();
    let bin = h.state().bin_rect;
    assert!(bin.is_positive(), "the bin did not report its rectangle");
    right_click_at(&mut h, egui::pos2(bin.center().x, bin.bottom() - 10.0));
    let labels = crate::layout_tests::menu_labels(&mut h);
    assert!(!labels.iter().any(|l| l.starts_with(tr(k::genkind::GROUP_BG))), "the bin lists kinds again: {labels:?}");
    h.get_by_label(tr(k::bin::menu::NEW_GENERATOR)).click();
    h.step();
    h.step();
    let made = new_clips(&h, before);
    assert_eq!(made.len(), 1, "{made:?}");
    assert_eq!((made[0].kind, made[0].lane, made[0].start), (ClipKind::Video, 1, Tick(4 * 90_000)), "the selected event's track");
}

/// Test 1165: a right click on a track's body creates AT THE CLICKED PLACE — with ONE plain item since
/// generator-from-track (`v26.10` G1, the owner: "Не вижу смысла делать выпадающие меню с перечислением
/// всех видов … Достаточно в самом окне менять тип"): the track's first kind (a sound kind on an audio
/// track, a solid colour on a video track), and its window opens for the kind to be changed there.
/// - **PATH** right click on audio track 2 at 6 s → "Create a generator…" (no list of kinds) → a sound
///   event at 6 s on track 2, its window open; right click on video track 2 at 2 s → the same item →
///   a solid colour at 2 s on video track 2, its window open
#[test]
fn t1165_the_track_menu_creates_at_the_clicked_place() {
    let mut h = crate::layout_tests::harness();
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Video, 0)));
    h.state_mut().insert_lane_after_row(LaneKind::Audio, Some((LaneKind::Audio, 0)));
    h.step();
    h.step();
    let gen_item = tr(k::bin::menu::NEW_GENERATOR);
    for (lk, ck, sec, group) in [
        (LaneKind::Audio, ClipKind::Audio, 6, tr(k::genkind::GROUP_AUDIO)),
        (LaneKind::Video, ClipKind::Video, 2, tr(k::genkind::GROUP_BG)),
    ] {
        let want = GeneratorKind::all().into_iter().find(|kd| crate::gen_ui::lands_on(kd, ck)).expect("a kind for the track");
        let g = h.state().tl_geom.clone().expect("timeline geometry");
        let row = g.row_of((lk, 1)).expect("track 2");
        let at = Tick(sec * 90_000);
        // the click lands inside the track, at the time `at` (the cursor stays at 0)
        let pos = egui::pos2(g.tick_to_x(at.0), g.row_center_y(row));
        let before = h.state().model.project().clips.len();
        right_click_at(&mut h, pos);
        assert!(
            matches!(h.state().ctx_target, Some(crate::timeline::CtxTarget::Lane { kind, lane: 1 }) if kind == ck),
            "the click is on track 2's body"
        );
        let labels = crate::layout_tests::menu_labels(&mut h);
        assert!(!labels.iter().any(|l| l.starts_with(group)), "{ck:?}: the track menu lists kinds again: {labels:?}");
        h.get_by_label(gen_item).click();
        h.step();
        h.step();
        let made = new_clips(&h, before);
        assert_eq!(made.len(), 1, "{ck:?}: one clip");
        let c = &made[0];
        assert_eq!((c.kind, c.lane), (ck, 1), "{ck:?}: on the clicked track");
        // the click's time, snapped by the timeline as any click is: within one pixel
        let px = g.tick_to_x(c.start.0) - g.tick_to_x(at.0);
        assert!(px.abs() <= 1.0, "{ck:?}: at the clicked time ({} vs {})", c.start.0, at.0);
        assert_eq!(c.r#gen.as_ref().map(|g| g.tag()), Some(want.tag()));
        assert!(h.state().fx_editors.iter().any(|e| e.clip == c.id), "{ck:?}: the window opens for the kind to change");
        h.state_mut().fx_editors.clear();
        h.step();
    }
}

/// Test 1166: a sound generator's card in the bin is "audio without video" and writes the kind's
/// name over its empty frame.
/// PATH: Edit → Create a generator → Audio → "Audio noise" → the bin card reads "Audio noise".
#[test]
fn t1166_a_sound_kinds_card_is_named_not_black() {
    crate::plugins::install();
    for tag in SOUND {
        let k = GeneratorKind::default_for_tag(tag).expect("a sound kind");
        assert!(crate::gen_ui::blank_default(&k), "{tag}: the card writes the name over the empty frame");
        let src = kt_media::generator::GeneratorSource::new(k);
        let info = kt_media::MediaSource::info(&src);
        assert!(!info.has_video && info.has_audio, "{tag}: the probe is audio without video");
    }
    let mut h = crate::layout_tests::harness();
    h.state_mut().create_generator(GeneratorKind::default_for_tag("audio_noise").unwrap());
    for _ in 0..4 {
        h.step();
    }
    let name = kind_label("audio_noise");
    assert!(h.query_all_by_label_contains(name).count() > 0, "the bin card is not named “{name}”");
}

/// Test 3128 (generator-from-track): a generator window's Kind gallery holds the kinds of its event's
/// stream — a sound event's gallery is the sound kinds, nothing of the picture ones above them (the
/// live run found the sound kinds pushed under the popup's fold by twenty picture kinds).
/// - **PATH** right click on the audio track → "Create a generator…" → its window → the Kind field →
///   the gallery shows Tone, Audio noise, Sine sweep, Beep train, Silence — and no Solid colour
#[test]
fn t3128_a_sound_events_gallery_is_the_sound_kinds() {
    let mut h = crate::layout_tests::harness();
    h.step();
    let g = h.state().tl_geom.clone().expect("timeline geometry");
    let row = g.row_of((LaneKind::Audio, 0)).expect("audio track 1");
    right_click_at(&mut h, egui::pos2(g.tick_to_x(2 * 90_000), g.row_center_y(row)));
    h.get_by_label(tr(k::bin::menu::NEW_GENERATOR)).click();
    h.step();
    h.step();
    for _ in 0..3 {
        h.step();
    }
    for tag in SOUND.iter().copied().chain(["solid"]) {
        crate::widgets::field_probe::forget(&format!("genw.kind.{tag}"));
    }
    let pick = crate::widgets::field_probe::rect_of("genw.kind.pick").expect("the window's Kind field");
    h.hover_at(pick.center());
    h.step();
    h.event(egui::Event::PointerButton { pos: pick.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.step();
    h.event(egui::Event::PointerButton { pos: pick.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    for _ in 0..3 {
        h.step();
    }
    for tag in SOUND {
        assert!(crate::widgets::field_probe::rect_of(&format!("genw.kind.{tag}")).is_some(), "the sound kind {tag} is not in the gallery");
    }
    assert!(crate::widgets::field_probe::rect_of("genw.kind.solid").is_none(), "a picture kind is in a sound event's gallery");
}

/// Test 3702 (gen-technical, `v26.10` G5): the sync flash and the safe areas are reached from Edit ›
/// Create a generator › Signal check; the flash lands with its sound on audio track 1 beside its
/// picture, the safe areas as a picture alone; their rows have captions in every language.
/// - **PATH** Edit → Create a generator… → Signal check → Sync flash → a picture event and its beep on
///   A1 at the cursor, its window open; again → Safe areas → one picture event (transparent over what
///   is under it)
#[test]
fn t3702_the_technical_kinds_are_created_from_edit() {
    crate::plugins::install();
    let mut h = crate::layout_tests::harness();
    for (tag, clips) in [("syncflash", 2usize), ("safearea", 1)] {
        let before = h.state().model.project().clips.len();
        create_from_edit(&mut h, tag);
        let made = new_clips(&h, before);
        assert_eq!(made.len(), clips, "{tag}: {made:?}");
        assert!(made.iter().all(|c| c.r#gen.as_ref().map(|g| g.tag()) == Some(tag)), "{tag}: the kind");
        h.state_mut().fx_editors.clear();
        h.step();
    }
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        for k in ["syncflash", "safearea"] {
            let kind = GeneratorKind::default_for_tag(k).expect("the kind");
            for s in kind.param_specs().iter().filter(|s| s.ty == ParamType::Num && s.unit != kt_model::ParamUnit::Rgb8) {
                let l = crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, &s.name);
                assert_ne!(l, s.name.as_ref(), "{lang:?}: {k}.{} has no caption", s.name);
            }
            for f in kind.flag_specs() {
                assert_ne!(crate::widgets::flag_label(f.name), f.name, "{lang:?}: {k} flag {} has no caption", f.name);
            }
        }
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Test 3705 (text-counter-source): a title's window chooses where its text comes from; a countdown
/// shows its format and its start, a typed title shows neither (no dead control).
/// - **PATH** Edit → Create a generator… → Titles and countdown → Text → its window → Text: Countdown →
///   the Format row and the Start field appear → Text: Typed → they go
#[test]
fn t3705_a_title_chooses_its_source_in_its_window() {
    crate::plugins::install();
    let mut h = crate::layout_tests::harness();
    create_from_edit(&mut h, "text");
    let painted = |h: &Harness<'_, KtApp>, src: &str| crate::reach_tests::painted_texts(h).iter().any(|t| t.src == src);
    let from = tr(k::genp::COUNT_FROM);
    let at = crate::widgets::field_probe::rect_of("titlew.source.countdown").expect("the source row");
    h.hover_at(at.center());
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at.center(), button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        h.step();
    }
    for _ in 0..3 {
        h.step();
    }
    let made = h.state().model.project().clips.iter().find_map(|c| c.r#gen.clone()).expect("the title");
    assert!(matches!(made, GeneratorKind::Text { source: kt_model::TextSource::Countdown, .. }), "the click did not set the source: {made:?}");
    assert!(painted(&h, tr(k::titlew::COUNT_FORMAT)), "a countdown shows its format row");
    crate::layout_tests::reveal_until(&mut h, from);
    assert!(painted(&h, from), "a countdown shows its start");
    // back to the typed text: with the grid still in view (its size row drawn), the start row is gone
    let id = h.state().model.project().clips.iter().find(|c| c.r#gen.is_some()).map(|c| c.id).expect("the title");
    let mut typed = made.clone();
    assert!(typed.set_value("source", &kt_model::ParamValue::Choice("static".into())));
    assert!(h.state_mut().apply(kt_model::Command::SetClipGen { id, kind: Some(typed) }));
    for _ in 0..3 {
        h.step();
    }
    assert!(painted(&h, tr(k::genp::SIZE)), "the grid left the view");
    assert!(!painted(&h, from), "a typed title shows the counter's start");
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        for key in [k::titlew::SOURCE, k::titlew::SOURCE_STATIC, k::titlew::SOURCE_TIMECODE, k::titlew::SOURCE_STOPWATCH, k::titlew::SOURCE_COUNTDOWN, k::titlew::COUNT_FORMAT, k::titlew::SOURCE_NOTE, k::genp::COUNT_FROM, k::genp::COUNT_FPS] {
            assert!(!tr(key).is_empty(), "{lang:?}: an empty text");
        }
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Test 3708 (gen-backgrounds, `v26.10` G5): the four moving backgrounds are reached from Edit › Create
/// a generator › Background, each one picture event; every number, flag and choice row is captioned in
/// every language (the pattern's four choices too).
/// - **PATH** Edit → Create a generator… → Background → Moving gradient / Plasma / Particles / Pattern →
///   a picture event at the cursor, its window open with the colours and the speed
#[test]
fn t3708_the_moving_backgrounds_are_created_from_edit() {
    crate::plugins::install();
    let mut h = crate::layout_tests::harness();
    for tag in ["meshgrad", "plasma", "particles", "pattern"] {
        let before = h.state().model.project().clips.len();
        create_from_edit(&mut h, tag);
        let made = new_clips(&h, before);
        assert_eq!(made.len(), 1, "{tag}: {made:?}");
        assert_eq!(made[0].r#gen.as_ref().map(|g| g.tag()), Some(tag), "{tag}: the kind");
        h.state_mut().fx_editors.clear();
        h.step();
    }
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        for tag in ["meshgrad", "plasma", "particles", "pattern"] {
            let kind = GeneratorKind::default_for_tag(tag).expect("the kind");
            assert_ne!(crate::gen_ui::kind_name(&kind), tag, "{lang:?}: {tag} has no name");
            let GeneratorKind::Plugin(p) = &kind else { unreachable!() };
            for spec in p.specs().iter().filter(|s| s.unit != kt_model::ParamUnit::Rgb8) {
                let label = if spec.ty == ParamType::Flag {
                    crate::widgets::flag_label(&spec.name)
                } else {
                    crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, &spec.name)
                };
                assert_ne!(label, spec.name.as_ref(), "{lang:?}: {tag}.{} has no caption", spec.name);
            }
            for (key, _) in crate::gen_ui::color_groups(&kind) {
                assert!(!tr(*key).is_empty(), "{lang:?}: {tag}: a colour caption is empty");
            }
        }
        for key in ["stripes", "dots", "grid", "waves"] {
            assert!(crate::gen_ui::choice_label("pattern", key).is_some(), "{lang:?}: pattern {key} has no caption");
        }
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Test 3710 (gen-shapes, `v26.10` G5): the six shapes and plates are reached from Edit › Create a
/// generator › Shapes, each one picture event; every row and colour is captioned in every language.
/// - **PATH** Edit → Create a generator… → Shapes → Polygon / Star / Arrow / Border frame / Lower-third
///   plate / Progress bar → a picture event at the cursor over what is under it, its window open
#[test]
fn t3710_the_shapes_and_plates_are_created_from_edit() {
    crate::plugins::install();
    let mut h = crate::layout_tests::harness();
    let tags = ["polygon", "star", "arrow", "frame", "plate", "progress"];
    for tag in tags {
        let before = h.state().model.project().clips.len();
        create_from_edit(&mut h, tag);
        let made = new_clips(&h, before);
        assert_eq!(made.len(), 1, "{tag}: {made:?}");
        assert_eq!(made[0].r#gen.as_ref().map(|g| g.tag()), Some(tag), "{tag}: the kind");
        h.state_mut().fx_editors.clear();
        h.step();
    }
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        for tag in tags {
            let kind = GeneratorKind::default_for_tag(tag).expect("the kind");
            assert_ne!(crate::gen_ui::kind_name(&kind), tag, "{lang:?}: {tag} has no name");
            let GeneratorKind::Plugin(p) = &kind else { unreachable!() };
            for spec in p.specs().iter().filter(|s| s.unit != kt_model::ParamUnit::Rgb8) {
                let label = crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, &spec.name);
                assert_ne!(label, spec.name.as_ref(), "{lang:?}: {tag}.{} has no caption", spec.name);
            }
            for (key, _) in crate::gen_ui::color_groups(&kind) {
                assert!(!tr(*key).is_empty(), "{lang:?}: {tag}: a colour caption is empty");
            }
        }
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Test 3713 (generator-menu-one-item D2): a kind changed in the window keeps the generator's sound
/// in step — Sync flash brings its beep event on audio track 1, linked to the picture; a kind with
/// no sound (Safe areas) takes that beep away; one Ctrl+Z gives the flash and its beep back.
/// - **PATH** Edit → "Create a generator…" → the window's Kind field → Sync flash → a beep event
///   appears on A1 under the flash → Kind → Safe areas → the beep is gone → Ctrl+Z → the flash and
///   its beep again → right click the bin card → Change kind → Signal check → Safe areas → the beep is
///   gone again
#[test]
fn t3713_a_kind_switch_keeps_the_sound_pair_in_step() {
    crate::plugins::install();
    let mut h = crate::layout_tests::harness();
    let before = h.state().model.project().clips.len();
    create_from_edit(&mut h, "syncflash");
    let made = new_clips(&h, before);
    assert_eq!(made.len(), 2, "the flash and its beep: {made:?}");
    let (v, a) = (made.iter().find(|c| c.kind == ClipKind::Video).expect("picture"), made.iter().find(|c| c.kind == ClipKind::Audio).expect("beep"));
    assert!(v.link_group.is_some() && v.link_group == a.link_group, "the beep is linked to its picture");
    assert_eq!((a.lane, a.start, a.duration, a.media), (0, v.start, v.duration, v.media), "the beep lies under the flash on A1");
    assert_eq!(a.r#gen.as_ref().map(|g| g.tag()), Some("syncflash"));
    pick_kind(&mut h, "safearea");
    let now = new_clips(&h, before);
    assert_eq!(now.len(), 1, "the safe areas have no sound: {now:?}");
    assert_eq!(now[0].r#gen.as_ref().map(|g| g.tag()), Some("safearea"));
    h.state_mut().undo();
    h.step();
    let back = new_clips(&h, before);
    assert_eq!(back.len(), 2, "one Ctrl+Z gives the flash and its beep back: {back:?}");
    assert!(back.iter().all(|c| c.r#gen.as_ref().map(|g| g.tag()) == Some("syncflash")));
    // the bin card's "Change kind" is the same body
    h.state_mut().fx_editors.clear();
    h.step();
    let media = back[0].media;
    let card = h.state().bin_card_rect(media).expect("the generator's bin card");
    right_click_at(&mut h, card.center());
    hover_item(&mut h, tr(k::genw::CHANGE_KIND));
    hover_item(&mut h, tr(k::genkind::GROUP_CHECK));
    h.get_by_label(kind_label("safearea")).click();
    h.step();
    h.step();
    let now = new_clips(&h, before);
    assert_eq!(now.len(), 1, "the card's kind change takes the beep too: {now:?}");
}

/// Test 3718 (gen-audio-reactive, `v26.10` G5): Waveform, Spectrum and Level rings are reached from
/// Edit › Create a generator… and the window's Kind gallery, in their own group "Reacts to sound"; each
/// is one picture event whose window shows the track it hears; every row, flag, choice and colour is
/// captioned in every language.
/// - **PATH** Edit → Create a generator… → its window → Kind → Reacts to sound → Waveform / Spectrum /
///   Level rings → a picture event at the cursor, its window with "Hears track 1" first
#[test]
fn t3718_the_reactive_kinds_are_created_and_captioned() {
    crate::plugins::install();
    let tags = ["waveform", "spectrum", "levelrings"];
    for tag in tags {
        let kind = GeneratorKind::default_for_tag(tag).expect(tag);
        assert!(crate::gen_ui::GenGroup::of(&kind) == crate::gen_ui::GenGroup::Reactive, "{tag}: not in its group");
    }
    let mut h = crate::layout_tests::harness();
    for tag in tags {
        let before = h.state().model.project().clips.len();
        create_from_edit(&mut h, tag);
        let made = new_clips(&h, before);
        assert_eq!(made.len(), 1, "{tag}: one picture event: {made:?}");
        assert_eq!((made[0].kind, made[0].r#gen.as_ref().map(|g| g.tag())), (ClipKind::Video, Some(tag)), "{tag}");
        let painted = crate::reach_tests::painted_texts(&h);
        assert!(painted.iter().any(|t| t.src == tr(k::genp::TRACK)), "{tag}: the window does not show the track it hears");
        h.state_mut().fx_editors.clear();
        h.step();
    }
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        assert!(!tr(k::genkind::GROUP_REACTIVE).is_empty());
        for tag in tags {
            let kind = GeneratorKind::default_for_tag(tag).expect("the kind");
            assert_ne!(crate::gen_ui::kind_name(&kind), tag, "{lang:?}: {tag} has no name");
            let GeneratorKind::Plugin(p) = &kind else { unreachable!() };
            for spec in p.specs().iter().filter(|s| s.unit != kt_model::ParamUnit::Rgb8) {
                let label = if spec.ty == ParamType::Flag {
                    crate::widgets::flag_label(&spec.name)
                } else {
                    crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, &spec.name)
                };
                assert_ne!(label, spec.name.as_ref(), "{lang:?}: {tag}.{} has no caption", spec.name);
            }
            for (key, _) in crate::gen_ui::color_groups(&kind) {
                assert!(!tr(*key).is_empty(), "{lang:?}: {tag}: a colour caption is empty");
            }
        }
        for (row, key) in [("style", "line"), ("style", "filled"), ("style", "mirror"), ("style", "bars")] {
            assert!(crate::gen_ui::choice_label(row, key).is_some(), "{lang:?}: {row} {key} has no caption");
        }
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}
