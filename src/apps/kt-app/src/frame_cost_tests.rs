//! ui-frame-cost (kkt-26.11.07b item 1): a UI frame costs what is visible, not the project. On the
//! owner's 1974 events a pointer over a still preview took a quarter of the processor: the media bin
//! copied the whole project several times a frame, the timeline built every event's speed map for a
//! hatching it then culled, and the waveform laid the speed curve out again for each pixel column.
//! The numbers on his project and on the heavy showcase are the live scenario `ui_frame`.

use crate::app::KtApp;
use crate::frame_cost::{thread_ns, visits};
use crate::layout_tests::harness_lang_dt;
use eframe::egui;
use egui_kittest::Harness;
use kt_model::*;
use kt_time::Tick;

/// Where the view ends: the scene's view starts at 0 with 1000 ticks a point.
const OFF_VIEW: i64 = 20_000_000;

fn event(id: u64, kind: ClipKind, lane: u32, start: i64, media: u64) -> ClipEvent {
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
        kind,
        lane,
        start: Tick(start),
        duration: Tick(50_000),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        // a speed map on every event: what the waveform and the tail past the material ask
        fx: vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// Three events in view on each of two rows, and `off` more past the view's right edge; the bin
/// holds `files` files, 30 of them already more than its column shows. Every file is shorter than
/// its events: each event has a tail past its material.
fn scene(off: usize, files: usize) -> Harness<'static, KtApp> {
    let mut h = harness_lang_dt(crate::i18n::Lang::En, Some(0.02));
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        for m in 0..files {
            let mut r = MediaRef::new(MediaId(m as u64 + 1), format!("synth:{}", m + 1));
            r.duration = Some(Tick(10_000));
            p.media.push(r);
        }
        let mut id = 0;
        for (i, start) in [0i64, 100_000, 200_000].into_iter().enumerate() {
            for kind in [ClipKind::Video, ClipKind::Audio] {
                id += 1;
                p.clips.push(event(id, kind, 0, start, 1 + (i % files) as u64));
            }
        }
        for i in 0..off {
            id += 1;
            let kind = if i % 2 == 0 { ClipKind::Video } else { ClipKind::Audio };
            p.clips.push(event(id, kind, 0, OFF_VIEW + (i as i64 / 2) * 60_000, 1 + (i % files) as u64));
        }
        app.model = TimelineModel::new(p);
        app.model_rev += 1;
        app.view_start = 0.0;
        app.ticks_per_px = 1000.0;
        app.player.seek(Tick(0));
    }
    // the caches of the new state are built here, once
    for _ in 0..4 {
        h.step();
    }
    h
}

/// A frame that is not a hover: it lays the events out again, as playback does.
fn full_frame(h: &mut Harness<'_, KtApp>) {
    h.input_mut().events.push(egui::Event::Key {
        key: egui::Key::F20,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    });
    h.step();
}

/// Ten frames' processor time of each scene, the cheapest of seven rounds, the scenes' rounds
/// INTERLEAVED (test-hygiene, found-defects 126). Measured one scene after the other, the two
/// numbers came from different moments of a loaded machine: at load ~60 t1914 read 8.5 ms against
/// 4.2 ms on correct code. Taken in turns, each scene's quietest round meets the other's.
fn frame_costs(scenes: &mut [&mut Harness<'_, KtApp>]) -> Vec<u64> {
    let mut best = vec![u64::MAX; scenes.len()];
    for _ in 0..7 {
        for (h, b) in scenes.iter_mut().zip(best.iter_mut()) {
            let t0 = thread_ns();
            for _ in 0..10 {
                full_frame(h);
            }
            *b = (*b).min(thread_ns() - t0);
        }
    }
    best
}

fn visits_per_frame(h: &mut Harness<'_, KtApp>) -> u64 {
    let v0 = visits();
    for _ in 0..8 {
        full_frame(h);
    }
    (visits() - v0) / 8
}

/// Test t1912: events, files and their tails outside the view add no drawing work to a frame.
/// Before the change the bin painted every card of its scroll, and the hatching of a tail past the
/// material built the speed map of every event of the project.
#[test]
fn t1912_what_is_outside_the_view_adds_no_work_to_a_frame() {
    let mut small = scene(0, 30);
    let mut big = scene(2000, 30);
    let mut files = scene(0, 300);
    let (a, b, c) = (visits_per_frame(&mut small), visits_per_frame(&mut big), visits_per_frame(&mut files));
    assert!(a >= 6, "the scene's own events were not drawn: {a} visits a frame");
    assert_eq!(a, b, "2000 events outside the view changed a frame's work");
    assert_eq!(a, c, "270 files below the bin's fold changed a frame's work");
    // the tails: the facts of the state hold every one, the frame draws those in view
    let events = big.state_mut().tl_facts();
    assert_eq!(events.tails.iter().flatten().count(), 2006, "every event is longer than its file");
    // the index answers the view with the view's events, and the hidden ones from their own place
    assert_eq!(events.hits(0, 300_000).count(), 6);
    assert_eq!(events.hits(OFF_VIEW, OFF_VIEW + 1).count(), 2);
}

/// Test t1913: a frame's processor time does not grow with the events outside the view, nor with
/// the files below the bin's fold. The time
/// is the test thread's own (`frame_cost::thread_ns`): a busy machine slows the frame but does not
/// charge it, and the bound is a ratio of two runs of this one thread. On the code before the
/// change the frame with 20000 hidden events cost several times the small one (a project copy per
/// bin question, a speed map per event for the tails).
#[test]
fn t1913_a_frames_processor_time_does_not_follow_the_project() {
    let mut small = scene(0, 30);
    let mut big = scene(20_000, 30);
    let mut files = scene(0, 1000);
    let costs = frame_costs(&mut [&mut small, &mut big, &mut files]);
    let (a, b, c) = (costs[0], costs[1], costs[2]);
    assert!(
        (b as f64) < (a as f64) * 1.5,
        "ten frames cost {:.1} ms with 20000 events outside the view against {:.1} ms without",
        b as f64 / 1e6,
        a as f64 / 1e6
    );
    // the bin's cards below the fold keep their rectangles (the selection band and the group
    // frames need them), and that arithmetic is all they cost
    assert!(
        (c as f64) < (a as f64) * 1.5,
        "ten frames cost {:.1} ms with 1000 files in the bin against {:.1} ms with 30",
        c as f64 / 1e6,
        a as f64 / 1e6
    );
}

/// Test t1914: an open window's frame does not grow with the project either. The effects window
/// copied the whole document three times a frame, the transition, camera, composition, render,
/// project settings and published-parameters windows once or twice each; on the owner's project
/// with the effects window open a frame cost 280 ms of the UI thread (with a value clamped every
/// frame into a false edit, which `input-without-work` removed).
#[test]
fn t1914_an_open_window_does_not_follow_the_project() {
    let open = |h: &mut Harness<'_, KtApp>| {
        h.state_mut().open_fx_editor(ClipId(1));
        h.state_mut().open_project_settings();
        for _ in 0..4 {
            h.step();
        }
        assert!(h.state().fx_editors.iter().any(|e| e.open), "the effects window did not open");
    };
    let mut small = scene(0, 30);
    let mut big = scene(20_000, 30);
    open(&mut small);
    open(&mut big);
    let costs = frame_costs(&mut [&mut small, &mut big]);
    let (a, b) = (costs[0], costs[1]);
    assert!(
        (b as f64) < (a as f64) * 1.5,
        "with the windows open ten frames cost {:.1} ms with 20000 events outside the view against {:.1} ms",
        b as f64 / 1e6,
        a as f64 / 1e6
    );
}
