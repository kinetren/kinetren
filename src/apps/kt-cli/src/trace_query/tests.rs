use super::*;
use kt_time::trace::{exec, mix, play, stream, wait};

const MS: u64 = 1_000_000;

fn rec(ev: Ev, t: u64, f: impl FnOnce(&mut Rec)) -> Rec {
    let mut r = Rec { t, ev: ev as u16, th: 1, ..Default::default() };
    f(&mut r);
    r
}

fn run_of(mut recs: Vec<Rec>) -> Run {
    recs.sort_by_key(|r| r.t);
    let mut run = Run { label: "test".into(), recs, ..Default::default() };
    run.threads.insert(1, "preview-compose".into());
    run.threads.insert(2, "prefetch".into());
    run.names.insert(77, "datamosh:search".into());
    run
}

/// A paused frame that waits 2 s: a decode, then a stream history computed by the prefetch thread.
fn paused_wait() -> Run {
    let key = 1u64 << 63 | 5;
    let mut v = vec![];
    for (id, at, done_ok) in [(1u64, 0, false), (2, 300 * MS, false), (3, 2000 * MS, true)] {
        v.push(rec(Ev::FrameAsk, at, |r| {
            r.id = id;
            r.a = 9000;
        }));
        v.push(rec(Ev::FramePlan, at + MS, |r| {
            r.id = id;
            r.dur = 2 * MS;
            r.aux = 2;
            r.b = u64::from(!done_ok);
        }));
        if !done_ok {
            v.push(rec(Ev::FrameWait, at + MS, |r| {
                r.id = id;
                r.aux = wait::DECODE;
                r.a = key;
            }));
        }
        v.push(rec(Ev::StreamRun, at + MS, |r| {
            r.id = 9;
            r.cause = id;
            r.dur = MS;
            r.a = 120;
            r.b = if done_ok { 0 } else { 60 };
            r.aux = if done_ok { stream::RENDERED } else { stream::SHORT };
        }));
        v.push(rec(Ev::FrameExec, at + 4 * MS, |r| {
            r.id = id;
            r.dur = 10 * MS;
            r.aux = if done_ok { exec::COMPLETE | exec::MEMO_STORED } else { 0 };
        }));
        v.push(rec(Ev::FrameDone, at + 15 * MS, |r| {
            r.id = id;
            r.a = 9000;
            r.aux = done_ok as u32;
        }));
    }
    v.push(rec(Ev::DecReq, MS, |r| {
        r.id = key;
        r.cause = 1;
        r.a = 3;
        r.b = 9000;
    }));
    v.push(rec(Ev::Decoded, 400 * MS, |r| {
        r.id = key;
        r.a = 3;
        r.b = 9000;
    }));
    v.push(rec(Ev::StreamAdvance, 410 * MS, |r| {
        r.th = 2;
        r.id = 9;
        r.dur = 1500 * MS;
        r.a = 120;
        r.b = 60;
        r.c = 60;
        r.aux = stream::DONE;
    }));
    for k in 0..60u64 {
        v.push(rec(Ev::ComputeJob, 420 * MS + k * 24 * MS, |r| {
            r.th = 2;
            r.dur = 20 * MS;
            r.aux = 77;
            r.b = 4 * MS;
        }));
    }
    run_of(v)
}

/// Test t2146: "why was the preview Loading": the stretch of a paused frame's wait, its waits ranked
/// by time (the stream history first, the decode next), and the critical path of its last composite.
#[test]
fn t2146_loading_ranks_the_waits() {
    let run = paused_wait();
    let st = stretches(&run, 180 * MS);
    assert_eq!(st.len(), 1, "{st:?}");
    assert_eq!(st[0].frames, vec![1, 2, 3]);
    assert!(st[0].shown);
    let out = loading(&run, None, None);
    println!("{out}");
    let lines: Vec<&str> = out.lines().filter(|l| l.contains(" ms  ")).collect();
    assert!(lines[0].contains("stream history"), "{out}");
    assert!(lines[0].contains("computed 60 positions"), "{out}");
    assert!(out.contains("compute datamosh:search ×60"), "{out}");
    assert!(out.contains("decoded frames (1 keys"), "{out}");
    assert!(out.contains("the last composite (3)") && out.contains("done at") && out.contains("complete"), "{out}");
    // the UI's own scrim records decide the stretch when it recorded them
    let mut ui = run.clone();
    ui.recs.push(rec(Ev::Loading, 180 * MS, |r| r.aux = 1));
    ui.recs.push(rec(Ev::Loading, 2016 * MS, |r| r.aux = 0));
    ui.recs.sort_by_key(|r| r.t);
    let st = stretches(&ui, 180 * MS);
    assert_eq!(st.len(), 1);
    assert_eq!(st[0].to, 2016 * MS);
}

/// A loop of three ticks over three laps: kept composites, one evicted, one whose key moved; the
/// kept mix replays on the second lap but for a chunk never kept in the lap context.
fn looped() -> Run {
    let mut v = vec![
        rec(Ev::Play, 0, |r| {
            r.aux = play::REGION;
            r.b = 9000;
            r.c = 9000 + 3 * 3000;
        }),
        rec(Ev::Play, MS, |r| r.aux = play::PLAY),
    ];
    let mut id = 100;
    let mut frame = |v: &mut Vec<Rec>, at: u64, tick: u64, key: u64, flags: u32| {
        id += 1;
        let i = id;
        v.push(rec(Ev::FrameAsk, at, |r| {
            r.id = i;
            r.a = tick;
        }));
        v.push(rec(Ev::FrameExec, at + MS, |r| {
            r.id = i;
            r.a = key;
            r.dur = if flags & exec::MEMO_HIT != 0 { MS } else { 30 * MS };
            r.aux = flags;
        }));
        v.push(rec(Ev::FrameDone, at + 32 * MS, |r| {
            r.id = i;
            r.a = tick;
            r.aux = 1;
        }));
        if flags & exec::MEMO_STORED != 0 {
            v.push(rec(Ev::MemoStore, at + 31 * MS, |r| r.id = key));
        }
    };
    let stored = exec::COMPLETE | exec::MEMO_STORED;
    let hit = exec::COMPLETE | exec::MEMO_HIT;
    // lap 0
    frame(&mut v, 10 * MS, 9000, 11, stored);
    frame(&mut v, 50 * MS, 12000, 12, stored);
    frame(&mut v, 90 * MS, 15000, 13, stored);
    v.push(rec(Ev::Play, 120 * MS, |r| r.aux = play::WRAP));
    // lap 1: tick 1 kept; tick 2 evicted meanwhile; tick 3's key moved
    v.push(rec(Ev::MemoEvict, 125 * MS, |r| {
        r.id = 12;
        r.cause = 99;
    }));
    frame(&mut v, 130 * MS, 9000, 11, hit);
    frame(&mut v, 170 * MS, 12000, 12, stored);
    frame(&mut v, 210 * MS, 15000, 14, stored);
    v.push(rec(Ev::MixChunk, 135 * MS, |r| r.aux = mix::REPLAY));
    v.push(rec(Ev::MixChunk, 175 * MS, |r| {
        r.aux = mix::KEPT;
        r.c = 1 | 1 << 8;
    }));
    v.push(rec(Ev::Play, 240 * MS, |r| r.aux = play::WRAP));
    // lap 2: all kept
    frame(&mut v, 250 * MS, 9000, 11, hit);
    frame(&mut v, 290 * MS, 12000, 12, hit);
    frame(&mut v, 330 * MS, 15000, 14, hit);
    v.push(rec(Ev::Play, 360 * MS, |r| r.aux = play::PAUSE));
    run_of(v)
}

/// Test t2147: "pass N over the loop": per lap the kept and composed frames, and for every frame
/// composed again on a later lap the reason (evicted, key moved), and the kept mix's misses.
#[test]
fn t2147_loop_names_why_a_frame_was_composed_again() {
    let run = looped();
    let b = lap_bounds(&run);
    assert_eq!(b.len(), 3, "{b:?}");
    let out = laps(&run);
    println!("{out}");
    let rows: Vec<&str> =
        out.lines().filter(|l| l.split_whitespace().nth(1).is_some_and(|w| w.starts_with('+'))).collect();
    let cols = |l: &str| l.split_whitespace().map(str::to_string).collect::<Vec<_>>();
    assert_eq!(cols(rows[0])[3..6], ["3", "0", "3"], "lap 0: three composed");
    assert_eq!(cols(rows[1])[3..6], ["3", "1", "2"], "lap 1: one kept, two composed again");
    assert!(out.contains("1 × evicted from the kept composites before this lap"), "{out}");
    assert!(out.contains("1 × the frame's key changed since the last lap"), "{out}");
    assert!(out.contains("lap context never kept ×1"), "{out}");
    let lap2 = cols(rows.last().unwrap());
    assert_eq!(lap2[3..6], ["3", "3", "0"], "lap 2: all kept");
}

/// Test t2148: "the slowest frames and their critical path": ordered by ask → done, each with its
/// plan, stream runs, compute and executor lines.
#[test]
fn t2148_slow_frames_carry_their_critical_path() {
    let mut run = paused_wait();
    if let Some(d) = run.recs.iter_mut().find(|r| r.kind() == Some(Ev::FrameDone) && r.id == 3) {
        d.t += 30 * MS;
    }
    for k in 0..3u64 {
        run.recs.push(rec(Ev::ComputeJob, 2001 * MS + k, |r| {
            r.cause = 3;
            r.dur = 5 * MS;
            r.aux = 77;
        }));
    }
    run.recs.sort_by_key(|r| r.t);
    let out = slow(&run, 2);
    println!("{out}");
    let heads: Vec<&str> = out.lines().filter(|l| l.starts_with("-- frame")).collect();
    assert_eq!(heads.len(), 2);
    assert!(heads[0].contains("frame 3") && heads[0].contains("45.0 ms"), "{out}");
    assert!(out.contains("compute datamosh:search ×3: 15.0 ms"), "{out}");
    assert!(out.contains("stream stage of clip 9 at position 120: short of history (60 positions lacked)"), "{out}");
    assert!(out.contains("executor at"), "{out}");
}

/// Test t2149: "what ran in this window", one frame's life, and the exports a tool reads: JSON
/// lines and a Chrome trace that parses, with the threads named.
#[test]
fn t2149_window_frame_and_exports() {
    let run = paused_wait();
    let out = during(&run, 400 * MS, 1000 * MS);
    println!("{out}");
    assert!(out.contains("prefetch"), "{out}");
    assert!(out.contains("compute_job:datamosh:search"), "{out}");
    let f = frame(&run, 1);
    assert!(f.contains("asked +0.001 (prio 0) → decoded +0.400"), "{f}");
    let d = dump(&run, None, None);
    assert_eq!(d.lines().count(), run.recs.len());
    assert!(d.lines().all(|l| serde_json::from_str::<serde_json::Value>(l).is_ok()));
    let c: serde_json::Value = serde_json::from_str(&chrome(&run)).expect("chrome JSON");
    let evs = c["traceEvents"].as_array().unwrap();
    assert!(evs.iter().any(|e| e["name"] == "thread_name" && e["args"]["name"] == "prefetch"));
    assert!(evs.iter().any(|e| e["name"] == "compute_job:datamosh:search" && e["ph"] == "X"));
}

/// Test t2243 (one-scheduler): `during` names what the scheduler did in the window — each job by
/// its class and lane, a pause and a resume, a cancel, and a blocking call on the UI thread.
#[test]
fn t2243_during_names_scheduled_jobs_pauses_and_blocking_calls() {
    use kt_time::sched::Class;
    let mut run = run_of(vec![
        rec(Ev::SchedQueue, 10 * MS, |r| {
            r.id = 5;
            r.aux = Class::Visible as u32;
            r.a = 41;
        }),
        rec(Ev::SchedRun, 11 * MS, |r| {
            r.id = 5;
            r.aux = Class::Visible as u32;
            r.a = 41;
            r.dur = 30 * MS;
        }),
        rec(Ev::SchedPause, 50 * MS, |r| {
            r.aux = 1;
            r.a = kt_time::sched::why::PLAYING as u64;
        }),
        rec(Ev::SchedCancel, 60 * MS, |r| {
            r.id = 6;
            r.aux = kt_time::sched::why_cancel::CLOSED;
            r.a = 42;
        }),
        rec(Ev::SchedPause, 90 * MS, |r| r.aux = 0),
        rec(Ev::Block, 95 * MS, |r| {
            r.aux = kt_time::block::kind::FSYNC;
            r.a = kt_time::block::role::UI as u64;
            r.dur = 7 * MS;
        }),
    ]);
    run.names.insert(41, "thumbs".into());
    run.names.insert(42, "analysis".into());
    let o = during(&run, 0, 200 * MS);
    for want in [
        "sched_queue:visible:thumbs",
        "sched_run:visible:thumbs",
        "sched_pause",
        "sched_resume",
        "sched_cancel:analysis",
        "block:ui:fsync",
    ] {
        assert!(o.contains(want), "{want} missing in:\n{o}");
    }
}

/// t2300 (memory-ledger): the ledger's sizings and the grants they changed are records, and
/// `trace memory` prints both sizings and the stream tenant's last cap.
#[test]
fn t2300_the_ledger_is_in_the_trace_and_the_query_reads_it() {
    use kt_time::mem::{Ledger, Tenant};
    const MB: u64 = 1 << 20;
    let l: &'static Ledger = Box::leak(Box::new(Ledger::new()));
    l.set_probe(|| Some(2000 * MB));
    let kept = l.open(Tenant::Kept, 0);
    let hist = l.open(Tenant::History, 256 * MB);
    let items = kt_time::trace::testing::capture(|| {
        hist.want(800 * MB);
        kept.want(900 * MB); // takes back the stream grant
        kept.want(900 * MB); // the same: nothing
    });
    // this ledger's records only: other tests of the process record their own
    let mine: Vec<Rec> = kt_time::trace::testing::recs(&items)
        .into_iter()
        .filter(|r| matches!(r.kind(), Some(Ev::MemBudget | Ev::MemGrant)))
        .collect();
    let budgets: Vec<_> = mine.iter().filter(|r| is(r, Ev::MemBudget)).collect();
    assert_eq!(budgets.len(), 2, "two sizings: {mine:?}");
    let grants: Vec<_> = mine.iter().filter(|r| is(r, Ev::MemGrant)).collect();
    assert!(grants.iter().any(|r| r.aux == Tenant::History as u32 && r.b == 356 * MB), "the stream grant taken back");
    // the first sizing names every growing tenant, the second the composites' new grant; the third,
    // unchanged, writes nothing
    assert_eq!(grants.iter().filter(|r| r.aux == Tenant::Kept as u32).count(), 2, "the unchanged sizing wrote nothing");
    let out = memory(&run_of(mine));
    assert_eq!(out.matches("budget ").count(), 2, "{out}");
    assert!(out.contains("history   cap 356 MB"), "{out}");
}

/// Test t2263 (gpu-arbiter): `gpu` reports the ring by class — admissions and their waits, the pieces
/// of a cut frame, a stopped history, compute jobs by class, and the display presents' wait.
#[test]
fn t2263_gpu_reports_the_ring_by_class() {
    use kt_time::sched::Class;
    let mut v = vec![
        rec(Ev::GpuAdmit, 0, |r| {
            r.id = 3;
            r.aux = Class::Now as u32;
            r.c = 1;
            r.dur = 2 * MS;
        }),
        rec(Ev::GpuAdmit, 10 * MS, |r| {
            r.id = 3;
            r.aux = Class::Near as u32;
            r.c = 1;
            r.dur = 9 * MS;
        }),
        rec(Ev::StreamStop, 30 * MS, |r| {
            r.id = 2;
            r.aux = kt_time::trace::stop::PREFETCH;
        }),
        rec(Ev::ComputeJob, 40 * MS, |r| {
            r.id = Class::Near as u64;
            r.b = 5 * MS;
        }),
    ];
    for i in 0..3 {
        v.push(rec(Ev::GpuPiece, (50 + i) * MS, |r| {
            r.id = 3;
            r.aux = Class::Offline as u32;
            r.a = i;
        }));
    }
    for (i, d) in [4u64, 6, 30].into_iter().enumerate() {
        v.push(rec(Ev::FrameWait, (100 + 10 * i as u64) * MS, |r| {
            r.aux = wait::DEVICE;
            r.dur = d * MS;
        }));
    }
    let o = gpu(&run_of(v));
    for want in [
        "ring 3 (carries the display)",
        "now",
        "near",
        "max 9.0 ms",
        "offline",
        "3 pieces cut",
        "compute jobs near",
        "stopped by prefetch: 1",
        "display presents 3: device wait p50 6.0 ms",
        "max 30.0 ms",
    ] {
        assert!(o.contains(want), "{want:?} missing:\n{o}");
    }
}

/// Test t2362 (stream-history-on-device, found-defects 398): `gpu` tells a late start from a late
/// signal: per class, how long a compute job waited before the device started it, its run on the
/// device and the wait for its buffers after it ended; a device with no timestamps is said so.
#[test]
fn t2362_gpu_tells_a_late_start_from_a_late_signal() {
    use kt_time::sched::Class;
    let mut v = Vec::new();
    // near: submitted at 0, started 40 ms later, ran 1 ms, buffers back 2 ms after the end
    for i in 0..3u64 {
        let base = i * 100 * MS;
        v.push(rec(Ev::GpuStart, base + 43 * MS, |r| {
            r.id = Class::Near as u64;
            r.a = base;
            r.b = base + 40 * MS;
            r.c = base + 41 * MS;
            r.aux = 12;
        }));
    }
    v.push(rec(Ev::GpuStart, 900 * MS, |r| {
        r.id = Class::Now as u64;
        r.a = 899 * MS;
        r.aux = u32::MAX;
    }));
    let o = gpu(&run_of(v));
    for want in ["near", "3 on the device: start after submit p50 40.0 ms", "run p50 1.0 ms", "buffers after end p50 2.0 ms", "≤ 12 µs", "1 with no device timestamps"] {
        assert!(o.contains(want), "{want:?} missing:\n{o}");
    }
}

/// Test t2364 (stream-history-on-device, found-defects 394): `loop` counts, per lap, the stream history
/// positions the walks computed and how many were distinct.
#[test]
fn t2364_loop_counts_the_history_walked_and_distinct() {
    let mut run = looped();
    let b = lap_bounds(&run);
    let (a0, _, _, _) = b[0];
    // lap 0: two batches over 0..40 and a re-walk of 20..30
    for (at, first, n) in [(1, 0u64, 32u64), (2, 32, 8), (3, 20, 10)] {
        run.recs.push(rec(Ev::StreamWalk, a0 + at * MS, |r| {
            r.a = first;
            r.b = n;
        }));
    }
    run.recs.sort_by_key(|r| r.t);
    assert_eq!(walked_in(&run, b[0].0, b[0].1), (50, 40));
    let out = laps(&run);
    assert!(out.contains("history: 50 positions walked, 40 distinct;"), "{out}");
}
