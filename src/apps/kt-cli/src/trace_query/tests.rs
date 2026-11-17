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

/// Test t2417 (catch-up-shows-loading): a Loading stretch whose scrim came with a stream stage short
/// says the history was catching up and by how much; one that came with nothing short does not.
#[test]
fn t2417_a_loading_stretch_over_a_catch_up_says_so() {
    let mut run = paused_wait();
    run.recs.push(rec(Ev::Loading, 180 * MS, |r| {
        r.aux = 1;
        r.b = 37;
    }));
    run.recs.push(rec(Ev::Loading, 900 * MS, |r| r.aux = 0));
    run.recs.push(rec(Ev::Loading, 1180 * MS, |r| r.aux = 1));
    run.recs.push(rec(Ev::Loading, 2016 * MS, |r| r.aux = 0));
    run.recs.sort_by_key(|r| r.t);
    let st = stretches(&run, 180 * MS);
    assert_eq!(st.iter().map(|s| s.catching_up).collect::<Vec<_>>(), vec![37, 0], "{st:?}");
    let out = loading(&run, None, None);
    assert_eq!(out.matches("catching up").count(), 1, "{out}");
    assert!(out.contains("catching up: 37 positions short"), "{out}");
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

/// t2396 (frames-cache-growth): `trace memory` prints the frames cache's sizings, each tenant's bytes
/// at the last busy second, the process's peak with the part the caches held and the rest named, and
/// the frames cache's evictions a second.
#[test]
fn t2396_memory_names_the_frames_cap_and_the_whole_process() {
    use kt_time::mem::Tenant;
    const MB: u64 = 1 << 20;
    const S: u64 = 1_000_000_000;
    let mut v = vec![
        rec(Ev::FramesCap, S, |r| {
            r.a = 1600 * MB;
            r.b = 200 * MB;
            r.c = 1200 * MB;
            r.aux = 720;
        }),
        rec(Ev::MemSample, 2 * S, |r| {
            r.a = 3000 * MB;
            r.b = 1000 * MB;
        }),
        rec(Ev::MemSample, 3 * S, |r| {
            r.a = 4700 * MB;
            r.b = 2500 * MB;
        }),
        rec(Ev::MemUsed, 3 * S, |r| {
            r.aux = Tenant::Frames as u32;
            r.a = 1500 * MB;
            r.b = 1664 * MB;
        }),
    ];
    for i in 0..5 {
        v.push(rec(Ev::CacheEvict, 4 * S + i, |_| {}));
    }
    v.push(rec(Ev::CacheEvict, 5 * S, |_| {}));
    let out = memory(&run_of(v));
    assert!(out.contains("frames cache 720 lines: cap 1600 MB, window 200 MB, loop 1200 MB"), "{out}");
    assert!(out.contains("frames    holds 1500 MB of cap 1664 MB"), "{out}");
    assert!(out.contains("process peak 4700 MB") && out.contains("the caches 2500 MB, the rest 2200 MB"), "{out}");
    assert!(out.contains("frames cache evictions: 6 in 2 seconds with any, at most 5 in one"), "{out}");
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
        // t2747: the preview's own device on GL fronts the person's frame without a display
        rec(Ev::GpuAdmit, 20 * MS, |r| {
            r.id = 5;
            r.aux = Class::Now as u32;
            r.c = 2;
            r.dur = MS;
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
        "ring 5 (fronts the person's frame)",
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

/// Test t2863 (datamosh-history-wait D1, found-defects 693): a job's pieces are one job however far
/// apart their records were written: the wait for its buffers is read from its LAST piece's end, and
/// a piece that ended before the job's admission waits is not a buffer wait.
#[test]
fn t2863_a_jobs_pieces_read_as_one_job() {
    use kt_time::sched::Class;
    let mut v = Vec::new();
    // two near jobs on the prefetch thread, each of five pieces 100 ms apart (the admission waits
    // between them), the last piece ending 3 ms before its buffers came back; the records are written
    // at the end, each with its own instant a microsecond apart
    for j in 0..2u64 {
        let base = j * 1000 * MS;
        let back = base + 405 * MS;
        v.push(rec(Ev::ComputeJob, base, |r| {
            r.th = 2;
            r.dur = 406 * MS;
            r.id = Class::Near as u64;
        }));
        for p in 0..5u64 {
            v.push(rec(Ev::GpuStart, back + p * 1000, |r| {
                r.th = 2;
                r.id = Class::Near as u64;
                r.a = base + p * 100 * MS;
                r.b = r.a + MS;
                r.c = r.a + 2 * MS;
                r.aux = 50;
            }));
        }
    }
    let o = gpu(&run_of(v));
    assert!(o.contains("10 on the device"), "{o}");
    assert!(o.contains("buffers after end p50 3.0 ms p95 3.0 ms"), "the buffers were read from every piece:\n{o}");
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

/// Test t2438 (gpu-throughput D6, found-defects 480, 486): `gpu` reports a class's admission waits
/// against its compute jobs' wall, the device's idle time between consecutive pieces and how many were
/// submitted before the earlier one ended, and the executors.
#[test]
fn t2438_gpu_tells_admission_from_device() {
    use kt_time::sched::Class;
    let mut v = Vec::new();
    // a near job of 100 ms wall that waited 60 ms for admission over three pieces
    v.push(rec(Ev::ComputeJob, 0, |r| {
        r.id = Class::Near as u64;
        r.dur = 100 * MS;
    }));
    for (i, wait) in [0u64, 30, 30].iter().enumerate() {
        v.push(rec(Ev::GpuAdmit, i as u64 * 30 * MS, |r| {
            r.aux = Class::Near as u32;
            r.dur = wait * MS;
            r.id = 1;
        }));
    }
    // pieces: [0,10) submitted at 0; [12,22) submitted at 11 (after the end: 2 ms idle); [22,32) submitted at 20
    // (a device start of 0 reads as "unknown": the pieces start a second in)
    for (a, b, c) in [(1000u64, 1000u64, 1010u64), (1011, 1012, 1022), (1020, 1022, 1032)] {
        v.push(rec(Ev::GpuStart, (c + 1) * MS, |r| {
            r.id = Class::Near as u64;
            r.a = a * MS;
            r.b = b * MS;
            r.c = c * MS;
            r.aux = 10;
        }));
        v.push(rec(Ev::GpuExec, a * MS, |r| r.id = 1));
    }
    v.sort_by_key(|r| r.t);
    let o = gpu(&run_of(v));
    for want in [
        "admission waits 60.0 ms of the compute jobs' wall 100.0 ms (60 %)",
        "pieces on the device 3: busy 30.0 ms, idle between consecutive pieces 2.0 ms (gaps under 100.0 ms), 2.0 ms of it with nothing queued",
        "1 of 2 submitted before the earlier ended",
        "executors: device 3 pieces, processor 0",
    ] {
        assert!(o.contains(want), "{want:?} missing:\n{o}");
    }
}

/// Test t2572 (load-report-into-trace): the load report's header is in the trace — `trace machine`
/// names the run, its start on the clock and every fact (each adapter, the cards the monitors hang
/// off, the power choice, the program's local clock); `summary` carries the same lines and `dump`
/// starts with them as one JSON line.
#[test]
fn t2572_machine_summary_and_dump_carry_the_machine() {
    let mut run = run_of(vec![rec(Ev::Load, MS, |r| r.a = 1000)]);
    run.pid = 4242;
    run.start_unix_ns = 1_762_498_800_250_000_000;
    run.machine = [
        ("clock", "2025-11-07 10:00:00.250+03:00"),
        ("cores", "16"),
        ("adapter.0", "AMD Radeon Graphics (RADV RAPHAEL_MENDOCINO) / IntegratedGpu / Vulkan / 1002:164e"),
        ("adapter.1", "llvmpipe (LLVM 19.1.1, 256 bits) / Cpu / Vulkan / 10005:0000"),
        ("monitors", "1002:164e"),
        ("power", "LowPower"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let m = machine(&run);
    for want in [
        "run: test (pid 4242)",
        "started: 2025-11-07 07:00:00.250 UTC",
        "clock: 2025-11-07 10:00:00.250+03:00",
        "adapter.0: AMD Radeon Graphics (RADV RAPHAEL_MENDOCINO) / IntegratedGpu / Vulkan / 1002:164e",
        "adapter.1: llvmpipe",
        "monitors: 1002:164e",
        "power: LowPower",
    ] {
        assert!(m.contains(want), "{want:?} missing:\n{m}");
    }
    let s = summary(&run);
    assert!(s.contains("  monitors: 1002:164e") && s.contains("  power: LowPower"), "{s}");
    let d = dump(&run, None, None);
    let head: serde_json::Value = serde_json::from_str(d.lines().next().unwrap()).expect("JSON");
    assert_eq!(head["machine"]["power"], "LowPower", "{d}");
    assert_eq!(d.lines().count(), run.recs.len() + 1, "the machine line, then one line a record");
}

/// Test t2760 (loop-laps-regression D1): a moment incomplete on one lap and complete on the next is
/// not "the frame's key changed": only a complete composite names the moment's key (the owner's
/// kkt-26.11.16 trace read 6-25 such moments a lap as unstable keys; every complete composite of a
/// moment had one key).
#[test]
fn t2760_an_incomplete_lap_is_not_a_key_change() {
    let mut v = vec![
        rec(Ev::Play, 0, |r| {
            r.aux = play::REGION;
            r.b = 9000;
            r.c = 9000 + 3000;
        }),
        rec(Ev::Play, MS, |r| r.aux = play::PLAY),
    ];
    let mut id = 200;
    let mut frame = |v: &mut Vec<Rec>, at: u64, key: u64, flags: u32| {
        id += 1;
        let i = id;
        v.push(rec(Ev::FrameAsk, at, |r| {
            r.id = i;
            r.a = 9000;
        }));
        v.push(rec(Ev::FrameExec, at + MS, |r| {
            r.id = i;
            r.a = key;
            r.dur = 30 * MS;
            r.aux = flags;
        }));
        v.push(rec(Ev::FrameDone, at + 32 * MS, |r| {
            r.id = i;
            r.a = 9000;
            r.aux = flags & exec::COMPLETE;
        }));
    };
    // lap 0: composed incomplete under another key (a stream stage short)
    frame(&mut v, 10 * MS, 77, 0);
    v.push(rec(Ev::Play, 60 * MS, |r| r.aux = play::WRAP));
    // lap 1: complete
    frame(&mut v, 70 * MS, 78, exec::COMPLETE | exec::MEMO_STORED);
    v.push(rec(Ev::Play, 120 * MS, |r| r.aux = play::PAUSE));
    let out = laps(&run_of(v));
    assert!(!out.contains("key changed"), "an incomplete lap read as a key change: {out}");
    assert!(out.contains("never kept on an earlier lap"), "{out}");
}

/// t2791 (render-speed D2): an export's stage records read back as where its time went — the real-time
/// factor, each busy stage's share of the wall, and the admission waits its submissions had on a ring
/// a display shares; a run without an export says so.
#[test]
fn t2791_trace_export_reads_where_an_export_spent_its_time() {
    use kt_time::trace::export_stage as st;
    let s = 1_000 * MS;
    let end = 30 * s;
    let mut v = vec![];
    for (aux, busy) in [(st::DECODE, 6 * s), (st::COMPOSE, 18 * s), (st::READBACK, 0), (st::GPU_WAIT, 15 * s)] {
        v.push(rec(Ev::ExportStage, end, |r| {
            r.aux = aux;
            r.a = busy;
            r.b = 20 * s;
            r.c = 250;
            r.id = 4;
        }));
    }
    v.push(rec(Ev::ExportStage, end, |r| {
        r.aux = st::WALL;
        r.a = 20 * s;
        r.b = 10 * s;
        r.c = 250;
        r.id = 4;
    }));
    // inside the export: two offline admissions on a ring with a display, one cut piece; before it, a
    // preview admission that is not the export's
    for (t, class, dur) in [(15 * s, kt_time::sched::Class::Offline, 2 * s), (20 * s, kt_time::sched::Class::Offline, s), (2 * s, kt_time::sched::Class::Near, 5 * s)] {
        v.push(rec(Ev::GpuAdmit, t, |r| {
            r.aux = class as u32;
            r.dur = dur;
            r.c = 1;
        }));
    }
    v.push(rec(Ev::GpuPiece, 21 * s, |r| r.aux = kt_time::sched::Class::Offline as u32));
    let out = export(&run_of(v));
    assert!(out.contains("export 0: 250 frames, 10.00 s of the timeline in 20.00 s — 0.50× real time (pipelined, 4 frame workers)"), "{out}");
    assert!(out.contains("compose          18.00 s busy    90.0 % of wall"), "{out}");
    assert!(out.contains("gpu wait         15.00 s busy    75.0 % of wall"), "{out}");
    assert!(!out.contains("readback"), "an idle stage is not listed: {out}");
    assert!(out.contains("2 export submissions waited 3000.0 ms for admission (15.0 % of wall), 1 pieces cut — a display shares the device"), "{out}");
    assert!(export(&run_of(vec![])).contains("no export in this run"));
}

/// Test t2926 (render-ahead D7): `trace loop` reads the device's busy share per lap from the `GpuFrame`
/// records harvested in it, and over the laps that had a moment not kept, the share while frames were
/// missing.
#[test]
fn t2926_a_lap_reads_its_device_share() {
    let mut run = looped();
    // lap 0 [first play .. 120 ms) and lap 1 [120 .. 240 ms) miss frames; lap 2 is all kept
    let first = lap_bounds(&run)[0].0;
    for (t, ns) in [(first + 20 * MS, 30 * MS), (first + 60 * MS, 30 * MS), (130 * MS, 60 * MS), (250 * MS, MS)] {
        run.recs.push(rec(Ev::GpuFrame, t, |r| r.a = ns));
    }
    run.recs.sort_by_key(|r| r.t);
    let b = lap_bounds(&run);
    let out = laps(&run);
    println!("{out}");
    let lap = |n: usize| out.lines().filter(|l| l.split_whitespace().nth(1).is_some_and(|w| w.starts_with('+'))).nth(n).unwrap().to_string();
    let pct = |ns: u64, (a, z): (u64, u64)| format!("device {:.0} %;", ns as f64 * 100.0 / (z - a) as f64);
    assert!(lap(0).contains(&pct(60 * MS, (b[0].0, b[0].1))), "{}", lap(0));
    assert!(lap(1).contains(&pct(60 * MS, (b[1].0, b[1].1))), "{}", lap(1));
    assert!(lap(2).contains(&pct(MS, (b[2].0, b[2].1))), "{}", lap(2));
    let wall = (b[0].1 - b[0].0) + (b[1].1 - b[1].0);
    let line = out.lines().find(|l| l.starts_with("device busy while frames were missing")).expect("the summary");
    assert!(line.contains(&format!("{:.0} %", 120.0 * MS as f64 * 100.0 / wall as f64)), "{line}");
    assert!(line.contains("over 2 laps"), "{line}");
    let g = gpu(&run);
    assert!(g.contains("frames timed 4 (preview 4"), "{g}");
}
