//! cached-range-plays: the bars' "plays without work" layer for video. t1936.

use kt_engine::{Player, PresentOptions};
use kt_media::{MediaSource, Quality};
use kt_model::demo::{DemoSources, demo_project};
use kt_time::Tick;

fn player(p: &kt_model::Project) -> Player {
    let sources: Vec<(kt_model::MediaId, Box<dyn MediaSource>)> = p
        .media
        .iter()
        .map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap()))
        .collect();
    let (player, _ring) = Player::new(p.clone(), sources);
    player.set_preview_quality(Quality { width: 320, height: 180 });
    player
}

/// Present the moment until it comes out complete (its frames decode in the background).
fn present_complete(player: &Player, t: Tick) {
    let deadline = kt_time::test_deadline();
    loop {
        if let Some((_, true)) = player.present_preview(t, PresentOptions::default()) {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "the moment {t:?} never came out complete");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn covers(runs: &[std::ops::Range<i64>], t: Tick) -> bool {
    runs.iter().any(|r| r.start <= t.0 && t.0 < r.end)
}

/// Ask until no kept moment is left unchecked against the current plan.
fn settled(player: &Player) -> Vec<std::ops::Range<i64>> {
    loop {
        let (runs, pending) = player.final_video_runs();
        if !pending {
            return runs;
        }
    }
}

/// t1936: the moments the preview presented are what the bars draw as playing without work; an
/// edit takes out exactly the moments of the edited event, the rest stay.
#[test]
fn t1936_the_bars_show_what_plays_without_work() {
    kt_plugins_builtin::install();
    let Some(gpu) = kt_gpu::test_engine() else { return };
    let mut p = demo_project(DemoSources::Synth);
    for c in &mut p.clips {
        c.opacity = 0.43; // a picture no other test presents on the shared engine
    }
    let player = player(&p);
    player.set_video_gpu(Some(gpu));
    let grid = player.video_grid();
    let ts: Vec<Tick> = (0..10).map(|i| grid.align_floor(Tick(90_000 + i * 60_000))).collect();
    assert!(settled(&player).is_empty(), "nothing presented, nothing plays free");
    for t in &ts {
        present_complete(&player, *t);
    }
    let runs = settled(&player);
    for t in &ts {
        assert!(covers(&runs, *t), "the presented moment {t:?} is not on the bars: {runs:?}");
    }
    // an event in some of the moments changes its level
    let edited = p
        .clips
        .iter()
        .find(|c| {
            c.kind == kt_model::ClipKind::Video
                && ts.iter().any(|t| c.start <= *t && *t < c.start + c.duration)
                && !ts.iter().all(|t| c.start <= *t && *t < c.start + c.duration)
        })
        .map(|c| c.id)
        .expect("scene guard: an event in some moments and not in others");
    let mut p2 = p.clone();
    let c = p2.clips.iter_mut().find(|c| c.id == edited).unwrap();
    c.opacity = 0.47;
    let span = (c.start, c.start + c.duration);
    player.set_project(p2);
    let runs = settled(&player);
    for t in &ts {
        let inside = span.0 <= *t && *t < span.1;
        assert_eq!(covers(&runs, *t), !inside, "at {t:?} (inside the edited event: {inside}): {runs:?}");
    }
}
