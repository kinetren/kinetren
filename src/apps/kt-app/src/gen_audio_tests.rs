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

fn kind_label(tag: &str) -> &'static str {
    crate::gen_ui::kind_name(&GeneratorKind::default_for_tag(tag).expect("a registered kind"))
}

/// The clips added since `before` (by id).
fn new_clips(h: &Harness<'_, KtApp>, before: usize) -> Vec<ClipEvent> {
    let mut v = h.state().model.project().clips.clone();
    v.sort_by_key(|c| c.id.0);
    v.split_off(before.min(v.len()))
}

/// Test 1163: the menu rows — the 19 former ones in their order, then the five sound kinds, all in
/// the Audio group, each named in the catalog, every row and every choice variant captioned.
#[test]
fn t1163_the_sound_kinds_are_menu_rows_in_the_audio_group() {
    crate::plugins::install();
    let all = GeneratorKind::all();
    assert_eq!(all.len(), 24, "19 picture rows + 5 sound rows");
    let tags: Vec<&str> = all[19..].iter().map(|k| k.tag()).collect();
    assert_eq!(tags, SOUND, "the sound rows follow the former 19 in BUILTIN_IDS order");
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
        for k in &all[19..] {
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

/// Test 1164: Edit → Create a generator → Audio → Tone with the cursor at 3 s gives ONE audio clip
/// at 3 s on audio track 1 — no video clip — in one history step, and opens its form.
/// PATH: click "Edit" → hover "Create a generator" → hover "Audio" → click "Tone".
#[test]
fn t1164_edit_creates_a_sound_generator_at_the_cursor() {
    let mut h = crate::layout_tests::harness();
    h.state().player.seek(Tick(3 * 90_000));
    h.step();
    let before = h.state().model.project().clips.len();
    // File no longer offers it
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
    h.get_by_label(tr(k::menu::edit::TITLE)).click();
    h.step();
    h.step();
    hover_item(&mut h, gen_item);
    hover_item(&mut h, tr(k::genkind::GROUP_AUDIO));
    h.get_by_label(kind_label("tone")).click();
    h.step();
    h.step();
    let made = new_clips(&h, before);
    assert_eq!(made.len(), 1, "one clip: a sound kind has no picture: {made:?}");
    let c = &made[0];
    assert_eq!((c.kind, c.lane, c.start), (ClipKind::Audio, 0, Tick(3 * 90_000)), "at the cursor on audio track 1");
    assert_eq!(c.r#gen.as_ref().map(|g| g.tag()), Some("tone"));
    assert!(h.state().fx_editors.iter().any(|e| e.clip == c.id), "the form opens on the new event");
    h.state_mut().undo();
    h.step();
    assert_eq!(h.state().model.project().clips.len(), before, "one history step");
}

/// Test 1165: a right click on a track's body creates AT THE CLICKED PLACE, and the track's list
/// holds only what lands on it.
/// PATH: right click on audio track 2 at 6 s → "Create a generator" → only Audio → "Beep train";
/// right click on video track 2 at 2 s → picture groups, no Audio → Background → "Solid color".
#[test]
fn t1165_the_track_menu_creates_at_the_clicked_place() {
    let mut h = crate::layout_tests::harness();
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Video, 0)));
    h.state_mut().insert_lane_after_row(LaneKind::Audio, Some((LaneKind::Audio, 0)));
    h.step();
    h.step();
    let gen_item = tr(k::bin::menu::NEW_GENERATOR);
    for (lk, ck, sec, group, tag, other) in [
        (LaneKind::Audio, ClipKind::Audio, 6, tr(k::genkind::GROUP_AUDIO), "beep", tr(k::genkind::GROUP_BG)),
        (LaneKind::Video, ClipKind::Video, 2, tr(k::genkind::GROUP_BG), "solid", tr(k::genkind::GROUP_AUDIO)),
    ] {
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
        hover_item(&mut h, gen_item);
        let labels = crate::layout_tests::menu_labels(&mut h);
        assert!(labels.iter().any(|l| l.starts_with(group)), "{ck:?}: no group {group}: {labels:?}");
        assert!(!labels.iter().any(|l| l.starts_with(other)), "{ck:?}: {other} does not land on this track: {labels:?}");
        hover_item(&mut h, group);
        h.get_by_label(kind_label(tag)).click();
        h.step();
        h.step();
        let made = new_clips(&h, before);
        assert_eq!(made.len(), 1, "{tag}: one clip");
        let c = &made[0];
        assert_eq!((c.kind, c.lane), (ck, 1), "{tag}: on the clicked track");
        // the click's time, snapped by the timeline as any click is: within one pixel
        let px = g.tick_to_x(c.start.0) - g.tick_to_x(at.0);
        assert!(px.abs() <= 1.0, "{tag}: at the clicked time ({} vs {})", c.start.0, at.0);
        assert_eq!(c.r#gen.as_ref().map(|g| g.tag()), Some(tag));
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
