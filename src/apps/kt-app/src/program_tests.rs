//! loudness-norms in the interface: the target, "Match loudness…", the program stage, the report.

use crate::app::KtApp;
use crate::i18n::{Lang, k, tr};
use crate::layout_tests::harness_lang_dt;
use crate::reach_tests::click_at;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use kt_model::*;
use kt_time::Tick;

const S: i64 = 90_000;

fn event(id: u64, lane: u32, start: i64, dur: i64, gain_db: f32) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    }
}

/// Two dialogue lines of one track and a title-less edit: 4 s each, 8 dB apart.
fn scene() -> Harness<'static, KtApp> {
    let mut h = harness_lang_dt(Lang::En, Some(0.02));
    crate::layout_tests::edit_project(h.state_mut(), |p| {
        let mut m = MediaRef::new(MediaId(1), "synth:sine:1000");
        m.duration = Some(Tick(600 * S));
        p.media.push(m);
        p.ensure_tracks(LaneKind::Audio, 1);
        p.clips.push(event(1, 0, 0, 4 * S, -18.0));
        p.clips.push(event(2, 0, 5 * S, 4 * S, -10.0));
    });
    for _ in 0..3 {
        h.step();
    }
    h
}

fn press(h: &mut Harness<'static, KtApp>, probe: &str) {
    let r = crate::widgets::field_probe::rect_of(probe).unwrap_or_else(|| panic!("no “{probe}” on screen"));
    click_at(h, r.center());
    h.step();
    h.step();
}

fn shown(probe: &str) -> bool {
    crate::widgets::field_probe::rect_of(probe).is_some()
}

/// Until `ok`, frame after frame.
fn until(h: &mut Harness<'static, KtApp>, what: &str, ok: impl Fn(&KtApp) -> bool) {
    let deadline = kt_time::test_deadline();
    while !ok(h.state()) {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(std::time::Duration::from_millis(5));
        h.step();
    }
    h.step();
}

fn idle(app: &KtApp) -> bool {
    app.analysis.current().is_none() && app.analysis.queued() == 0
}

/// The row's probes of this frame only: an old frame's rectangle is no answer.
fn fresh(h: &mut Harness<'static, KtApp>) {
    crate::widgets::field_probe::clear_seen();
    h.step();
}

/// t3922: Streaming is set in Project settings → the lines are matched from their track's menu window to
/// the project's target in one undo step → the program is matched from the row under the meters: the
/// stage has a gain, what was measured and reaches the target; the pick is remembered; inside a
/// composition there is no row and no loudness in its settings.
#[test]
fn t3922_the_target_the_match_and_the_program() {
    let mut h = scene();
    h.state_mut().open_project_settings();
    h.step();
    h.step();
    press(&mut h, "settings.loud");
    press(&mut h, "settings.loud.streaming");
    press(&mut h, "settings.apply");
    assert_eq!(h.state().model.doc().loudness, LoudnessTarget::of(LoudnessStandard::Streaming));

    let ids = h.state().audio_events_of(0);
    h.state_mut().open_match(ids);
    h.step();
    h.step();
    press(&mut h, "match.dialogue");
    press(&mut h, "match.go");
    until(&mut h, "the match never answered", idle);
    let gains: Vec<f32> = h.state().model.doc().clips.iter().map(|c| c.gain_db).collect();
    // a 1 kHz tone of amplitude 0.5 in both channels is −6.0 LUFS at 0 dB
    assert!(gains.iter().all(|g| (g + 8.0).abs() < 0.3), "both lines reach −14 LUFS: {gains:?}");
    h.state_mut().undo();
    h.step();
    let back: Vec<f32> = h.state().model.doc().clips.iter().map(|c| c.gain_db).collect();
    assert_eq!(back, [-18.0, -10.0], "one Undo restores both");

    press(&mut h, "program.row");
    assert!(h.state().program_win && shown("program.match"), "the row opens the stage's window");
    press(&mut h, "program.match");
    until(&mut h, "the program was never matched", idle);
    let g = h.state().model.doc().program.expect("the stage is there");
    let m = g.measured.expect("and what was measured");
    assert!((m.reached + 14.0).abs() < 0.1 && (m.lufs + g.gain_db - m.reached).abs() < 0.2, "{g:?}");
    until(&mut h, "the row never says the gain", |_| shown("program.gain"));
    assert!(h.query_by_label_contains("Measured:").is_some(), "the window names what was measured");
    h.state_mut().undo();
    h.step();
    assert_eq!(h.state().model.doc().program, None, "one Undo takes the stage away");

    // a number is remembered for the next time
    h.state_mut().open_match(vec![ClipId(1)]);
    h.state_mut().match_win.as_mut().unwrap().pick = 2;
    h.step();
    h.step();
    press(&mut h, "match.go");
    until(&mut h, "the second match never answered", idle);
    h.state_mut().open_match(vec![ClipId(2)]);
    assert_eq!(h.state().match_win.as_ref().unwrap().pick, 2, "the last pick stands");
    h.state_mut().match_win = None;
    assert_eq!(h.query_all_by_label(tr(k::loudness::MATCH_TITLE)).count(), 0);
}

/// t3923: after the program is matched an event's level is raised: the row and the window say "changed
/// since measured" until it is matched again; a picture event says nothing.
#[test]
fn t3923_an_edit_marks_the_program_as_changed() {
    let mut h = scene();
    h.state_mut().match_program();
    until(&mut h, "the program was never matched", idle);
    assert!(h.state().model.doc().program.is_some());
    // the builder's count of what is heard lands in a later frame
    until(&mut h, "the builder never counted the sound", |a| a.player.built().sound.is_some());
    fresh(&mut h);
    assert!(shown("program.row") && !shown("program.changed"), "nothing changed yet");

    let mut title = event(9, 0, 0, S, 0.0);
    title.kind = ClipKind::Video;
    assert!(h.state_mut().apply(Command::AddClip(title)));
    let heard = h.state().model.doc().program.unwrap().measured.unwrap().sound;
    until(&mut h, "the builder is behind", |a| a.player.wait_built().sound == Some(heard));
    fresh(&mut h);
    assert!(!shown("program.changed"), "a picture is not heard");

    assert!(h.state_mut().apply(Command::SetGain { id: ClipId(1), gain_db: -6.0 }));
    until(&mut h, "the builder never saw the louder event", |a| a.player.wait_built().sound != Some(heard));
    fresh(&mut h);
    assert!(shown("program.changed"), "the row does not say the sound changed");
    h.state_mut().program_win = true;
    h.step();
    fresh(&mut h);
    assert!(shown("program.win_changed"), "the window does not say it");
    press(&mut h, "program.match");
    until(&mut h, "the program was never matched again", idle);
    let heard = h.state().model.doc().program.unwrap().measured.unwrap().sound;
    until(&mut h, "the builder is behind", |a| a.player.wait_built().sound == Some(heard));
    fresh(&mut h);
    assert!(!shown("program.changed"), "the mark goes when it is matched");
}

/// t4203: the measuring job stands while the player plays and goes on after it; a sound edited before it
/// ends drops its result with a notice, and no gain is written; asked again it lands.
#[test]
fn t4203_the_job_yields_to_playback_and_to_edits() {
    crate::alone::own_process!();
    let mut h = scene();
    h.state_mut().player.play();
    h.step();
    h.state_mut().match_program();
    for _ in 0..40 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(!idle(h.state()) && h.state().model.doc().program.is_none(), "the job ran while the player played");
    }
    assert!(h.state_mut().apply(Command::SetGain { id: ClipId(1), gain_db: -3.0 }));
    h.state_mut().player.pause();
    until(&mut h, "the job never went on after the play", idle);
    assert_eq!(h.state().model.doc().program, None, "a gain of another sound was written");
    let said = h.state().notices.iter().any(|n| n.text == tr(k::loudness::SOUND_CHANGED));
    assert!(said, "nothing says why");
    h.state_mut().match_program();
    until(&mut h, "the second job never ended", idle);
    assert!(h.state().model.doc().program.is_some(), "asked again it lands");
}
