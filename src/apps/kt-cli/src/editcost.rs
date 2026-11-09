//! `kt-cli edit-cost` (edit-without-rebuild): what one edit and one step of a slider drag cost the
//! player — the time the UI thread spends handing the edit over (the document snapshot, the swap, a
//! meter reading, a region change), the CPU the whole process spends until the engine has taken it,
//! and how many renderers were built. No sources are opened: a build does not read media.

use kt_engine::Player;
use kt_model::{ClipKind, Project};
use kt_time::work::{Work, count};
use kt_time::{Tick, TickRange};
use std::time::{Duration, Instant};

/// The process's CPU time (user + system, all threads), from `/proc/self/stat`.
fn cpu_s() -> f64 {
    let Ok(s) = std::fs::read_to_string("/proc/self/stat") else { return 0.0 };
    // the command name may hold spaces: the fields after it start past the last ')'
    let rest = s.rsplit_once(')').map(|(_, r)| r).unwrap_or("");
    let f: Vec<&str> = rest.split_whitespace().collect();
    let (ut, st) = (f.get(11).and_then(|v| v.parse::<f64>().ok()), f.get(12).and_then(|v| v.parse::<f64>().ok()));
    (ut.unwrap_or(0.0) + st.unwrap_or(0.0)) / 100.0
}

/// The engine threads are done with what was handed over: the build count stands for a while.
fn settle() {
    let deadline = Instant::now() + Duration::from_secs(30);
    let (mut last, mut still) = (count(Work::EngineBuild), 0);
    while still < 4 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(40));
        let now = count(Work::EngineBuild);
        still = if now == last { still + 1 } else { 0 };
        last = now;
    }
}

/// The app's handover of an edit: the document snapshot of the new state, given to the player.
fn hand_over(player: &Player, project: &Project) {
    player.set_project(std::sync::Arc::new(project.clone()));
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(((v.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0.0)
}

pub fn run(project: &Project, steps: usize) -> Result<(), String> {
    let mut project = project.clone();
    // the event a slider moves: the audio event in the middle of the edit (its volume)
    let audio: Vec<usize> =
        project.clips.iter().enumerate().filter(|(_, c)| c.kind == ClipKind::Audio).map(|(i, _)| i).collect();
    let Some(&target) = audio.get(audio.len() / 2) else { return Err("no audio event to edit".into()) };
    println!("edit-cost: {} events, the slider moves event {:?}", project.clips.len(), project.clips[target].id);
    let t = Instant::now();
    for _ in 0..5 {
        drop(project.clone());
    }
    println!("  document clone: {:.2} ms", t.elapsed().as_secs_f64() * 1e3 / 5.0);

    // one build of an edited version on this thread, best of five: fresh (every event derived, as
    // each consumer built before edit-without-rebuild) and incremental (the player's builder)
    let facts = kt_engine::SourceFacts::default();
    let build = |p: &Project| {
        kt_engine::video::revisions_scope(|| {
            drop(kt_engine::gen_sources(p));
            drop(kt_engine::AudioRenderer::new(p));
            drop(kt_engine::VideoRenderer::new_with_facts(p, &facts));
        })
    };
    let mut memo = kt_engine::incremental::BuildMemo::default();
    kt_engine::incremental::with_memo(&mut memo, || build(&project));
    let (mut fresh, mut incr, mut resolve) = (f64::INFINITY, f64::INFINITY, f64::INFINITY);
    for i in 0..5 {
        project.clips[target].gain_db = -0.01 * (i + 1) as f32;
        let t = Instant::now();
        build(&project);
        fresh = fresh.min(t.elapsed().as_secs_f64() * 1e3);
        let t = Instant::now();
        kt_engine::incremental::with_memo(&mut memo, || build(&project));
        incr = incr.min(t.elapsed().as_secs_f64() * 1e3);
        let t = Instant::now();
        drop(project.resolve_reel(None, &[]));
        resolve = resolve.min(t.elapsed().as_secs_f64() * 1e3);
    }
    println!(
        "  a build of an edited version (generator sources, audio, video in one scope): fresh {fresh:.1} ms, \
         incremental {incr:.1} ms (of which the root's resolve alone {resolve:.1} ms)"
    );

    let (player, _ring) = Player::new(project.clone(), Vec::new());
    let _ = player.sample_levels_at(Tick(0));
    settle();

    // one edit at a time, each let settle: the UI's share and the whole process's CPU per edit
    let (mut ui, mut cpu, mut derived) = (Vec::new(), Vec::new(), Vec::new());
    let b0 = count(Work::EngineBuild);
    for i in 0..steps {
        project.clips[target].gain_db = -0.1 * (i + 1) as f32;
        let c0 = cpu_s();
        let t = Instant::now();
        hand_over(&player, &project);
        let _ = player.sample_levels_at(Tick(0));
        ui.push(t.elapsed().as_secs_f64() * 1e3);
        settle();
        // the settle's own wait is ~160 ms of sleep, no CPU
        cpu.push((cpu_s() - c0) * 1e3);
        derived.push(player.built().cost[kt_time::cost::Cost::EventDerive as usize]);
    }
    println!("  events derived by each edit's build: {derived:?}");
    let builds = count(Work::EngineBuild) - b0;
    let cpu_mean = cpu.iter().sum::<f64>() / cpu.len().max(1) as f64;
    println!(
        "  one edit: UI thread p50 {:.2} ms, max {:.2} ms; process CPU {:.1} ms per edit; {:.1} renderer builds per edit",
        pct(&mut ui.clone(), 0.5),
        pct(&mut ui, 1.0),
        cpu_mean,
        builds as f64 / steps as f64
    );

    // a drag: a step every UI frame (16 ms) for two seconds, a meter reading each frame
    let (mut ui, frames) = (Vec::new(), 120usize);
    let b0 = count(Work::EngineBuild);
    let c0 = cpu_s();
    let t_all = Instant::now();
    for i in 0..frames {
        let frame = Instant::now();
        project.clips[target].gain_db = -0.05 * (i % 40) as f32;
        hand_over(&player, &project);
        let _ = player.sample_levels_at(Tick(0));
        ui.push(frame.elapsed().as_secs_f64() * 1e3);
        if let Some(rest) = Duration::from_millis(16).checked_sub(frame.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    settle();
    let wall = t_all.elapsed().as_secs_f64();
    let builds = count(Work::EngineBuild) - b0;
    println!(
        "  drag ({frames} steps at 60 Hz): UI thread p50 {:.2} ms, max {:.2} ms; process CPU {:.0} % of one core; {builds} renderer builds",
        pct(&mut ui.clone(), 0.5),
        pct(&mut ui, 1.0),
        (cpu_s() - c0) / wall * 100.0,
    );

    // region changes: not an edit
    let (mut ui, b0, c0) = (Vec::new(), count(Work::EngineBuild), cpu_s());
    for i in 0..20i64 {
        let t = Instant::now();
        player.set_loop_region(Some(TickRange::new(Tick(i * 9_000), Tick(90_000 * 4 + i * 9_000))));
        ui.push(t.elapsed().as_secs_f64() * 1e3);
    }
    settle();
    println!(
        "  20 region changes: UI thread p50 {:.2} ms, max {:.2} ms; process CPU {:.0} ms; {} renderer builds",
        pct(&mut ui.clone(), 0.5),
        pct(&mut ui, 1.0),
        (cpu_s() - c0) * 1e3,
        count(Work::EngineBuild) - b0
    );
    Ok(())
}
