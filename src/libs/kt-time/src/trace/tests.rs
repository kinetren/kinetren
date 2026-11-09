use super::*;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kt-trace-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn ctx_switches(tid: u64) -> u64 {
    let s = std::fs::read_to_string(format!("/proc/self/task/{tid}/status")).unwrap_or_default();
    s.lines()
        .filter(|l| l.contains("ctxt_switches"))
        .filter_map(|l| l.split(':').nth(1)?.trim().parse::<u64>().ok())
        .sum()
}

/// Test t2140: records of three threads, with causes, names, spans and texts, come back from the
/// files whole and in time order, the threads and names resolved.
#[test]
fn t2140_records_come_back_from_the_files() {
    let _g = testing::lock();
    let dir = tmp("t2140");
    install(Config::new(Some(dir.clone()), "t2140"));
    let frame = next_id();
    let hs: Vec<_> = (0..3u64)
        .map(|i| {
            std::thread::Builder::new()
                .name(format!("t2140-{i}"))
                .spawn(move || {
                    let _c = with_cause(frame);
                    let n = name("datamosh:search");
                    for k in 0..100u64 {
                        ev(Ev::DecReq).id(key_id(7, 640, 360, k as i64)).a(7).b(k).c(size(640, 360)).aux(i as u32).emit();
                    }
                    let mut s = span(Ev::ComputeJob);
                    s.aux(n).a(i);
                    std::thread::sleep(Duration::from_millis(2));
                })
                .unwrap()
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    journal(5, "ACT window opened: Effects");
    flush();
    let runs = load(&dir).unwrap();
    let run = runs.iter().find(|r| r.label.starts_with("t2140")).expect("our run");
    let reqs: Vec<&Rec> = run.recs.iter().filter(|r| r.kind() == Some(Ev::DecReq) && r.cause == frame).collect();
    assert_eq!(reqs.len(), 300, "every request with the frame as its cause");
    assert!(reqs.iter().all(|r| r.id >> 63 == 1 && r.c == size(640, 360)));
    assert!(run.recs.windows(2).all(|w| w[0].t <= w[1].t), "time order");
    let jobs: Vec<&Rec> = run.recs.iter().filter(|r| r.kind() == Some(Ev::ComputeJob) && r.cause == frame).collect();
    assert_eq!(jobs.len(), 3);
    assert!(jobs.iter().all(|j| j.dur >= 2_000_000), "a span carries its length");
    assert!(jobs.iter().all(|j| run.name_of(j.aux) == "datamosh:search"));
    let names: Vec<String> = jobs.iter().map(|j| run.thread_of(j.th)).collect();
    for i in 0..3 {
        assert!(names.contains(&format!("t2140-{i}")), "{names:?}");
    }
    assert!(run.texts.iter().any(|t| t.kind == text::JOURNAL && t.s.contains("Effects")));
    assert!(run.machine.iter().any(|(k, _)| k == "cores"), "the machine is described");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test t2141: past its segment size the trace opens a new segment, the directory stays within its
/// budget, every segment reads alone (it repeats the names and threads), the newest record is kept.
#[test]
fn t2141_ring_files_are_bounded() {
    let _g = testing::lock();
    let dir = tmp("t2141");
    install(Config { dir: Some(dir.clone()), segment_bytes: 64 << 10, budget_bytes: 256 << 10, label: "t2141".into() });
    let n = name("t2141:job");
    let last = 20_000u64;
    for k in 1..=last {
        ev(Ev::Job).aux(n).id(k).emit();
        if k % 2000 == 0 {
            flush();
        }
    }
    flush();
    let segs = segments(&dir);
    let total: u64 = segs.iter().map(|p| std::fs::metadata(p).unwrap().len()).sum();
    assert!(segs.len() >= 3, "{} segments", segs.len());
    assert!(total <= (256 + 64) << 10, "{total} bytes");
    for s in &segs {
        let (_, _, _, items) = read_segment(s).unwrap();
        assert!(items.iter().any(|i| matches!(i, Item::Text(t) if t.kind == text::THREAD)), "{s:?} names its threads");
        assert!(items.iter().any(|i| matches!(i, Item::Text(t) if t.kind == text::NAME && t.s == "t2141:job")));
    }
    let runs = load(&dir).unwrap();
    assert!(runs.iter().flat_map(|r| &r.recs).any(|r| r.kind() == Some(Ev::Job) && r.id == last));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test t2142: with nothing recorded the writer thread makes no context switch for a second and a
/// half; one record wakes it and is written.
#[test]
fn t2142_at_rest_the_writer_sleeps() {
    let _g = testing::lock();
    let dir = tmp("t2142");
    install(Config::new(Some(dir.clone()), "t2142"));
    flush();
    // let the batch sleep end and the writer park
    std::thread::sleep(BATCH * 4);
    let tid = writer_tid();
    assert!(tid > 0);
    let before = ctx_switches(tid);
    std::thread::sleep(Duration::from_millis(1500));
    let after = ctx_switches(tid);
    assert_eq!(before, after, "the writer woke at rest");
    let id = next_id();
    ev(Ev::Edit).id(id).emit();
    flush();
    let runs = load(&dir).unwrap();
    assert!(runs.iter().flat_map(|r| &r.recs).any(|r| r.id == id));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test t2143 (part): a full ring drops, never waits, and the writer says how many it lost.
#[test]
fn t2143_a_full_ring_drops_and_says_so() {
    let _g = testing::lock();
    install(Config::new(None, "t2143"));
    flush();
    let lost0 = dropped();
    let items = {
        let _cap = CAPTURE.lock().unwrap().replace(Vec::new());
        let p = testing::pause_writer();
        let t0 = Instant::now();
        for k in 0..(RING as u64 + 5000) {
            ev(Ev::Job).id(k).emit();
        }
        let spent = t0.elapsed();
        assert!(spent < Duration::from_secs(5), "recording waited: {spent:?}");
        drop(p);
        flush();
        CAPTURE.lock().unwrap().take().unwrap_or_default()
    };
    assert!(dropped() - lost0 >= 5000);
    let said: u64 = testing::recs(&items).iter().filter(|r| r.kind() == Some(Ev::Dropped)).map(|r| r.a).sum();
    assert!(said >= 5000, "the writer said it lost {said}");
}
