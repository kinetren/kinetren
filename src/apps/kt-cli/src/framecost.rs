//! `kt-cli frame-cost` (frame-work-by-edit): what the per-frame questions of a video renderer cost
//! on a project, and on the same project grown by `--far N` one-second events placed after its end
//! (two lanes, butted with crossfades) — events no frame of the range shows. The plan of each frame
//! (with an empty cache: no media is read), the order of the range (`wanted_frames`), the reel
//! coverage bars and a fresh renderer build are timed, minima of repeats. A frame's cost must not
//! move with `--far`.

use kt_engine::VideoRenderer;
use kt_media::Quality;
use kt_model::{ClipEvent, ClipId, ClipKind, LaneKind, Project, TransitionEvent, TransitionId, TransitionKind};
use kt_time::{Tick, TickRange};
use std::time::Instant;

const SEC: i64 = 90_000;

/// `p` with `far` events after its end on two video lanes.
pub fn grow(p: &mut Project, far: u64) {
    if far == 0 {
        return;
    }
    let end = p.clips.iter().map(|c| (c.start + c.duration).0).max().unwrap_or(0) + 10 * SEC;
    let Some(template) = p.clips.iter().find(|c| c.kind == ClipKind::Video && c.r#gen.is_none()).cloned() else { return };
    let base = p.clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 1;
    let tbase = p.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
    p.ensure_tracks(LaneKind::Video, 2);
    for i in 0..far {
        for lane in 0..2u32 {
            let mut c: ClipEvent = template.clone();
            c.id = ClipId(base + 2 * i + lane as u64);
            c.lane = lane;
            c.start = Tick(end + i as i64 * SEC);
            c.duration = Tick(SEC);
            c.source_in = Tick(0);
            c.link_group = None;
            c.fx.clear();
            c.fx_bypass.clear();
            c.fx_auto.clear();
            c.masks = Default::default();
            c.overrides.clear();
            p.clips.push(c);
        }
        if i > 0 {
            p.transitions.push(TransitionEvent {
                id: TransitionId(tbase + i),
                a: Some(ClipId(base + 2 * (i - 1))),
                b: Some(ClipId(base + 2 * i)),
                in_offset: Tick(SEC / 10),
                out_offset: Tick(SEC / 10),
                kind: TransitionKind::Crossfade,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            });
        }
    }
}

fn best<R>(reps: usize, mut f: impl FnMut() -> R) -> f64 {
    (0..reps)
        .map(|_| {
            let t = Instant::now();
            drop(std::hint::black_box(f()));
            t.elapsed().as_secs_f64() * 1e6
        })
        .fold(f64::INFINITY, f64::min)
}

pub fn run(project: &Project, from: f64, to: f64, far: u64, q: Quality) -> Result<(), String> {
    let mut p = project.clone();
    grow(&mut p, far);
    let events = p.clips.len() + p.sequences.iter().map(|s| s.clips.len()).sum::<usize>();
    let build = best(3, || VideoRenderer::new(&p));
    let v = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let step = v.grid.frame_step();
    let (a, b) = (v.grid.align_floor(Tick((from * SEC as f64) as i64)), Tick((to * SEC as f64) as i64));
    let frames = ((b.0 - a.0) / step).max(1) as usize;
    let plan = best(5, || {
        let mut t = a;
        while t < b {
            drop(std::hint::black_box(v.frame_plan(&cache, t, q)));
            t = Tick(t.0 + step);
        }
    }) / frames as f64;
    let order = best(5, || v.wanted_frames(TickRange::new(a, b), q)) / frames as f64;
    let bars = best(5, || v.nest_coverage(&cache, q));
    println!(
        "frame-cost: events {events} (far {far}), frames {frames}: plan {plan:.1} us/frame, order {order:.1} us/frame, reel bars {bars:.1} us, build {:.2} ms",
        build / 1000.0
    );
    Ok(())
}
