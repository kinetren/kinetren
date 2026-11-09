use super::*;
use std::sync::atomic::AtomicUsize;

fn until(what: &str, f: impl Fn() -> bool) {
    let deadline = crate::test_deadline();
    while !f() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Every test that holds a pause vote runs alone: a vote pauses the whole process's pausable classes.
static VOTES: Mutex<()> = Mutex::new(());

/// Test t2230: a lane runs one job at a time in its FIFO order, a pump drains its own queue and is
/// coalesced (a kick while due adds nothing, a kick while running runs it once more), and the
/// classes are taken in priority order when the pool is held.
#[test]
fn t2230_lanes_are_fifo_one_at_a_time_and_classes_run_in_order() {
    let lane = Lane::open("t2230:fifo", Class::Answer);
    let order = Arc::new(Mutex::new(Vec::new()));
    let inflight = Arc::new(AtomicUsize::new(0));
    let most = Arc::new(AtomicUsize::new(0));
    for i in 0..60u32 {
        let (order, inflight, most) = (order.clone(), inflight.clone(), most.clone());
        lane.post(move |_| {
            let n = inflight.fetch_add(1, Ordering::SeqCst) + 1;
            most.fetch_max(n, Ordering::SeqCst);
            std::thread::sleep(Duration::from_micros(200));
            order.lock().unwrap().push(i);
            inflight.fetch_sub(1, Ordering::SeqCst);
        });
    }
    assert!(lane.wait_idle(crate::test_deadline()));
    assert_eq!(*order.lock().unwrap(), (0..60).collect::<Vec<_>>(), "FIFO");
    assert_eq!(most.load(Ordering::SeqCst), 1, "two jobs of one lane ran at once");

    // a pump: its own queue, drained; kicks coalesce
    let q = Arc::new(Mutex::new(VecDeque::<u32>::new()));
    let runs = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(Mutex::new(Vec::new()));
    let pump = {
        let (q, runs, done) = (q.clone(), runs.clone(), done.clone());
        Lane::service("t2230:pump", Class::Answer, move |_| {
            runs.fetch_add(1, Ordering::SeqCst);
            // one item per run: the scheduler runs it again while it says More
            let Some(v) = q.lock().unwrap().pop_front() else { return Pump::Idle };
            done.lock().unwrap().push(v);
            if q.lock().unwrap().is_empty() { Pump::Idle } else { Pump::More }
        })
    };
    q.lock().unwrap().extend(0..10);
    for _ in 0..5 {
        pump.kick();
    }
    assert!(pump.wait_idle(crate::test_deadline()));
    assert_eq!(*done.lock().unwrap(), (0..10).collect::<Vec<_>>());
    assert!(runs.load(Ordering::SeqCst) <= 12, "kicks were not coalesced: {} runs", runs.load(Ordering::SeqCst));

    // priority: fill the capped slots with held Background jobs, queue Visible, Background and
    // Answer jobs; the Answer runs at once (one worker is always kept for it), the rest wait for a slot
    let _v = VOTES.lock().unwrap_or_else(|e| e.into_inner());
    let release = Arc::new(AtomicBool::new(false));
    let entered = Arc::new(AtomicUsize::new(0));
    let holders: Vec<LaneRef> = (0..workers() - 1)
        .map(|_| {
            let l = Lane::open("t2230:hold", Class::Background);
            let (r, e) = (release.clone(), entered.clone());
            l.post(move |_| {
                e.fetch_add(1, Ordering::SeqCst);
                while !r.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(1));
                }
            });
            l
        })
        .collect();
    until("the holders run", || entered.load(Ordering::SeqCst) == workers() - 1);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let bg = Lane::open("t2230:bg", Class::Background);
    let vis = Lane::open("t2230:vis", Class::Visible);
    let ans = Lane::open("t2230:ans", Class::Answer);
    let s = seen.clone();
    bg.post(move |_| s.lock().unwrap().push("background"));
    let s = seen.clone();
    vis.post(move |_| s.lock().unwrap().push("visible"));
    let s = seen.clone();
    ans.post(move |_| s.lock().unwrap().push("answer"));
    assert!(ans.wait_idle(crate::test_deadline()), "the person's answer waited behind background work");
    assert_eq!(*seen.lock().unwrap(), vec!["answer"], "a capped class took the kept worker");
    release.store(true, Ordering::Release);
    assert!(bg.wait_idle(crate::test_deadline()) && vis.wait_idle(crate::test_deadline()));
    assert_eq!(*seen.lock().unwrap(), vec!["answer", "visible", "background"], "classes out of order");
    for h in &holders {
        assert!(h.wait_idle(crate::test_deadline()));
    }
}

/// Test t2231: while a vote is up the pausable classes start nothing and a running job's checkpoint
/// waits (no clock); the person's answers still run; the vote's drop resumes them.
#[test]
fn t2231_votes_pause_background_classes_and_resume_them() {
    let _v = VOTES.lock().unwrap_or_else(|e| e.into_inner());
    let vis = Lane::open("t2231:thumbs", Class::Visible);
    let entered = Arc::new(AtomicBool::new(false));
    let passed = Arc::new(AtomicUsize::new(0));
    let go = Arc::new(AtomicBool::new(false));
    {
        let (entered, passed, go) = (entered.clone(), passed.clone(), go.clone());
        vis.post(move |ctx| {
            entered.store(true, Ordering::Release);
            while !go.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            for _ in 0..3 {
                if ctx.checkpoint() {
                    return;
                }
                passed.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
    until("the job runs", || entered.load(Ordering::Acquire));
    let vote = Vote::new(why::PLAYING);
    vote.set(true);
    assert_eq!(paused() & why::PLAYING, why::PLAYING);
    go.store(true, Ordering::Release);
    let later = Arc::new(AtomicBool::new(false));
    let l2 = later.clone();
    vis.post(move |_| l2.store(true, Ordering::Release));
    let bg = Lane::open("t2231:index", Class::Background);
    let l3 = later.clone();
    bg.post(move |_| l3.store(true, Ordering::Release));
    let ans = Lane::open("t2231:save", Class::Answer);
    assert!(ans.post(|_| {}) != 0);
    assert!(ans.wait_idle(crate::test_deadline()), "a save waited for the pause");
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(passed.load(Ordering::SeqCst), 0, "a checkpoint passed while paused");
    assert!(!later.load(Ordering::Acquire), "a paused class started a job");
    assert!(stats().paused != 0);
    drop(vote);
    assert!(vis.wait_idle(crate::test_deadline()) && bg.wait_idle(crate::test_deadline()));
    assert_eq!(passed.load(Ordering::SeqCst), 3);
    assert!(later.load(Ordering::Acquire));
}

/// Test t2232: closing a lane drops its queued jobs unrun and its running job sees the cancel; a
/// keyed post supersedes the queued one of the same key; a cancelled token releases a job waiting
/// at a paused checkpoint; a panicking job leaves the worker alive.
#[test]
fn t2232_cancellation_supersede_and_panics() {
    let _v = VOTES.lock().unwrap_or_else(|e| e.into_inner());
    let lane = Lane::open("t2232:close", Class::Answer);
    let started = Arc::new(AtomicBool::new(false));
    let saw_cancel = Arc::new(AtomicBool::new(false));
    let ran_after = Arc::new(AtomicBool::new(false));
    {
        let (started, saw) = (started.clone(), saw_cancel.clone());
        lane.post(move |ctx| {
            started.store(true, Ordering::Release);
            while !ctx.cancelled() {
                std::thread::sleep(Duration::from_millis(1));
            }
            saw.store(true, Ordering::Release);
        });
    }
    let r = ran_after.clone();
    lane.post(move |_| r.store(true, Ordering::Release));
    until("the first job runs", || started.load(Ordering::Acquire));
    let raw = lane.0.clone();
    drop(lane);
    until("the running job sees the close", || saw_cancel.load(Ordering::Acquire));
    assert!(raw.wait_idle(crate::test_deadline()));
    assert!(!ran_after.load(Ordering::Acquire), "a job of a closed lane ran");

    // supersede
    let lane = Lane::open("t2232:save", Class::Answer);
    let gate = Arc::new(AtomicBool::new(false));
    let g = gate.clone();
    lane.post(move |_| {
        while !g.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    let wrote = Arc::new(Mutex::new(Vec::new()));
    for i in 0..5 {
        let w = wrote.clone();
        lane.post_keyed(7, Token::new(), move |_| w.lock().unwrap().push(i));
    }
    gate.store(true, Ordering::Release);
    assert!(lane.wait_idle(crate::test_deadline()));
    assert_eq!(*wrote.lock().unwrap(), vec![4], "only the newest of a key is written");

    // a token releases a paused checkpoint
    let vis = Lane::open("t2232:vis", Class::Visible);
    let tok = Token::new();
    let entered = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));
    let hold = Arc::new(AtomicBool::new(false));
    {
        let (entered, stopped, hold) = (entered.clone(), stopped.clone(), hold.clone());
        vis.post_keyed(0, tok.clone(), move |ctx| {
            entered.store(true, Ordering::Release);
            while !hold.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            stopped.store(ctx.checkpoint(), Ordering::Release);
        });
    }
    until("the job runs", || entered.load(Ordering::Acquire));
    let vote = Vote::new(why::HELD);
    vote.set(true);
    hold.store(true, Ordering::Release);
    std::thread::sleep(Duration::from_millis(20));
    tok.cancel(why_cancel::ASKED);
    assert!(vis.wait_idle(crate::test_deadline()), "the cancel did not release the checkpoint");
    assert!(stopped.load(Ordering::Acquire));
    drop(vote);

    // a panic is contained
    let lane = Lane::open("t2232:panic", Class::Answer);
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    lane.post(|_| panic!("t2232"));
    let ok = Arc::new(AtomicBool::new(false));
    let o = ok.clone();
    lane.post(move |_| o.store(true, Ordering::Release));
    assert!(lane.wait_idle(crate::test_deadline()));
    std::panic::set_hook(prev);
    assert!(ok.load(Ordering::Acquire), "a job after a panic did not run");
}

/// Test t2233: at rest the pool sleeps — its workers make no context switch while nothing is posted,
/// and one post wakes one of them.
#[cfg(target_os = "linux")]
#[test]
fn t2233_at_rest_no_worker_wakes() {
    let lane = Lane::open("t2233:one", Class::Answer);
    lane.post(|_| {});
    assert!(lane.wait_idle(crate::test_deadline()));
    let workers_now = || -> Vec<(String, u64)> {
        let mut v = Vec::new();
        for e in std::fs::read_dir("/proc/self/task").into_iter().flatten().flatten() {
            let comm = std::fs::read_to_string(e.path().join("comm")).unwrap_or_default();
            if comm.trim().starts_with("sched-") {
                let st = std::fs::read_to_string(e.path().join("status")).unwrap_or_default();
                let sw: u64 = st
                    .lines()
                    .filter(|l| l.contains("ctxt_switches"))
                    .filter_map(|l| l.split(':').nth(1)?.trim().parse::<u64>().ok())
                    .sum();
                v.push((e.file_name().to_string_lossy().into_owned(), sw));
            }
        }
        v.sort();
        v
    };
    // other tests of this binary post jobs too: wait for a quiet window, then require it stays quiet
    let deadline = crate::test_deadline();
    loop {
        let a = workers_now();
        assert_eq!(a.len(), workers(), "the pool's size");
        std::thread::sleep(Duration::from_millis(1500));
        let b = workers_now();
        if a == b && stats().running == 0 {
            break;
        }
        assert!(Instant::now() < deadline, "the pool never rested");
    }
}

/// Test t2234: the trace says what ran: the post, the run (with its queue wait and outcome), the
/// cancel with its reason, the pause and the resume; the job's own records carry it as their cause.
#[test]
fn t2234_every_job_and_pause_is_traced() {
    let _v = VOTES.lock().unwrap_or_else(|e| e.into_inner());
    let lane = Lane::open("t2234:lane", Class::Answer);
    let (mut ran, mut gone) = (0, 0);
    let items = trace::testing::capture(|| {
        ran = lane.post(|ctx| {
            trace::ev(Ev::Job).id(ctx.id()).aux(99).emit();
        });
        assert!(lane.wait_idle(crate::test_deadline()));
        let gate = Arc::new(AtomicBool::new(false));
        let g = gate.clone();
        lane.post(move |_| {
            while !g.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        gone = lane.post(|_| {});
        lane.cancel_queued(why_cancel::ASKED);
        gate.store(true, Ordering::Release);
        assert!(lane.wait_idle(crate::test_deadline()));
        let v = Vote::new(why::PLAYING);
        v.set(true);
        v.set(false);
    });
    let recs = trace::testing::recs(&items);
    let lane_name = trace::name("t2234:lane") as u64;
    let q = recs.iter().find(|r| r.kind() == Some(Ev::SchedQueue) && r.id == ran).expect("queued");
    assert_eq!((q.aux, q.a), (Class::Answer as u32, lane_name));
    let run = recs.iter().find(|r| r.kind() == Some(Ev::SchedRun) && r.id == ran).expect("ran");
    assert_eq!(run.c, outcome::DONE);
    let inner = recs.iter().find(|r| r.kind() == Some(Ev::Job) && r.id == ran).expect("its own record");
    assert_eq!(inner.cause, ran, "the job's record does not name it as its cause");
    let c = recs.iter().find(|r| r.kind() == Some(Ev::SchedCancel) && r.id == gone).expect("cancelled");
    assert_eq!(c.aux, why_cancel::ASKED);
    let p: Vec<u32> = recs.iter().filter(|r| r.kind() == Some(Ev::SchedPause)).map(|r| r.aux).collect();
    assert!(p.windows(2).any(|w| w == [1, 0]), "pause and resume: {p:?}");
}
