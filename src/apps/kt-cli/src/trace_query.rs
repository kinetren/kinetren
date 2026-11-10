//! `kt-cli trace` (structured-telemetry): questions asked of one run's trace. Every answer is
//! computed from the typed records; nothing here reads prose.
//!
//! - `summary` — the run, its machine, what it recorded;
//! - `loading [from to]` — each stretch the preview waited for a picture: the chain of waits ranked
//!   by time, and the critical path of the frame that ended it;
//! - `loop` — pass by pass over a loop region: what came from the kept composites and the kept mix,
//!   and why the rest did not;
//! - `during <from> <to>` — what ran in a window;
//! - `slow [n]` — the slowest frames and their critical path;
//! - `frame <id>` — one frame's life;
//! - `gpu` — the device rings (gpu-arbiter): admissions, waits, pieces and compute jobs by class,
//!   device submits a second, stopped stream histories, the display's present latency;
//! - `machine`, `dump [from to]` (JSON lines), `chrome <out.json>` (Perfetto / chrome://tracing).
//!
//! Times are seconds since the process started (`+193.201`), the clock of the text journal.

use kt_time::trace::{self, Ev, Rec, Run};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

const NS: f64 = 1e9;

fn s(t: u64) -> String {
    format!("+{:.3}", t as f64 / NS)
}

fn ms(ns: u64) -> String {
    format!("{:.1} ms", ns as f64 / 1e6)
}

fn secs(v: &str) -> Option<u64> {
    v.trim_start_matches('+').parse::<f64>().ok().map(|x| (x * NS) as u64)
}

fn tick_s(t: u64) -> String {
    format!("{:.3} s", t as i64 as f64 / kt_time::TICK_HZ as f64)
}

/// The command line: `kt-cli trace [<dir|file>] <question> …`.
pub fn run(args: &[String]) -> i32 {
    let mut rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let mut pick: Option<usize> = None;
    if let Some(i) = rest.iter().position(|a| a == "--run") {
        pick = rest.get(i + 1).and_then(|v| v.parse().ok());
        rest.drain(i..(i + 2).min(rest.len()));
    }
    let path = match rest.first() {
        Some(p) if std::path::Path::new(p).exists() => {
            let p = std::path::PathBuf::from(p);
            rest.remove(0);
            p
        }
        _ => default_dir(),
    };
    let runs = match trace::load(&path) {
        Ok(r) if !r.is_empty() => r,
        Ok(_) => {
            eprintln!("no trace in {}", path.display());
            return 1;
        }
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 1;
        }
    };
    let run = match pick {
        Some(i) => match runs.get(i) {
            Some(r) => r,
            None => {
                eprintln!("run {i} of {}", runs.len());
                return 2;
            }
        },
        None => runs.last().unwrap(),
    };
    let q = rest.first().map(String::as_str).unwrap_or("summary");
    let arg = |i: usize| rest.get(i).and_then(|v| secs(v));
    let out = match q {
        "summary" => {
            let mut o = String::new();
            for (i, r) in runs.iter().enumerate() {
                let _ = writeln!(o, "run {i}: {} — {} records", r.label, r.recs.len());
            }
            o + &summary(run)
        }
        "loading" => loading(run, arg(1), arg(2)),
        "loop" => laps(run),
        "during" => match (arg(1), arg(2)) {
            (Some(a), Some(b)) => during(run, a, b),
            _ => "usage: kt-cli trace [dir] during <from_s> <to_s>\n".into(),
        },
        "slow" => slow(run, rest.get(1).and_then(|v| v.parse().ok()).unwrap_or(10)),
        "frame" => match rest.get(1).and_then(|v| v.parse().ok()) {
            Some(id) => frame(run, id),
            None => "usage: kt-cli trace [dir] frame <id>\n".into(),
        },
        "machine" => machine(run),
        "memory" => memory(run),
        "gpu" => gpu(run),
        "dump" => dump(run, arg(1), arg(2)),
        "chrome" => match rest.get(1) {
            Some(out) => match std::fs::write(out, chrome(run)) {
                Ok(()) => format!("wrote {out}: open it in https://ui.perfetto.dev or chrome://tracing\n"),
                Err(e) => format!("{out}: {e}\n"),
            },
            None => "usage: kt-cli trace [dir] chrome <out.json>\n".into(),
        },
        _ => {
            eprintln!(
                "usage: kt-cli trace [<dir|file>] [--run N] summary|loading [from to]|loop|during <from> <to>|slow [n]|frame <id>|gpu|machine|memory|dump [from to]|chrome <out.json>"
            );
            return 2;
        }
    };
    print!("{out}");
    0
}

/// The memory ledger over the run (memory-ledger): every sizing with the grants it changed, the
/// preview's decoded-frame cache sizings and the process's memory a busy second (frames-cache-growth),
/// then each tenant's last cap and bytes, the frames cache's evictions a second and the peak of the
/// process.
fn memory(run: &Run) -> String {
    use kt_time::mem::Tenant;
    let mb = |b: u64| format!("{:.0} MB", b as f64 / (1u64 << 20) as f64);
    let mut o = String::new();
    let mut last: std::collections::BTreeMap<u32, (u64, u64, u64)> = Default::default();
    // (peak resident, its moment, the ledger's sum then)
    let mut peak: Option<(u64, u64, u64)> = None;
    let mut used: std::collections::BTreeMap<u32, (u64, u64)> = Default::default();
    let mut evicts: std::collections::BTreeMap<u64, u64> = Default::default();
    for r in &run.recs {
        match kind(r) {
            Some(Ev::CacheEvict) => *evicts.entry(r.t / 1_000_000_000).or_default() += 1,
            Some(Ev::FramesCap) => {
                let _ = writeln!(
                    o,
                    "{:>9}    frames cache {} lines: cap {}, window {}, loop {}",
                    s(r.t),
                    r.aux,
                    mb(r.a),
                    mb(r.b),
                    if r.c == 0 { "none".to_string() } else { mb(r.c) }
                );
            }
            Some(Ev::MemSample) => {
                if r.a > 0 && peak.is_none_or(|p| r.a > p.0) {
                    peak = Some((r.a, r.t, r.b));
                }
            }
            Some(Ev::MemUsed) => {
                used.insert(r.aux, (r.a, r.b));
            }
            Some(Ev::MemBudget) => {
                let _ = writeln!(
                    o,
                    "{:>9}  budget {} of growth, {} granted, {} available{}",
                    s(r.t),
                    mb(r.a),
                    mb(r.b),
                    if r.c == 0 { "unknown".to_string() } else { format!("{} MB", r.c) },
                    if r.aux > 0 { format!(", claimed by {} export(s)", r.aux) } else { String::new() }
                );
            }
            Some(Ev::MemGrant) => {
                let name = Tenant::from_u32(r.aux).map_or("?", Tenant::name);
                let _ = writeln!(
                    o,
                    "{:>9}    {name:<9} want {} → cap {}, holds {}{}",
                    s(r.t),
                    mb(r.a),
                    mb(r.b),
                    mb(r.c),
                    if r.id == 1 { "  (the device refused its growth)" } else { "" }
                );
                last.insert(r.aux, (r.a, r.b, r.c));
            }
            _ => {}
        }
    }
    if o.is_empty() && peak.is_none() && used.is_empty() {
        return "no memory ledger records in this run\n".into();
    }
    let _ = writeln!(o, "last:");
    for (t, (want, cap, held)) in &last {
        let name = Tenant::from_u32(*t).map_or("?", Tenant::name);
        let _ = writeln!(o, "  {name:<9} cap {}, holds {}, wants {}", mb(*cap), mb(*held), mb(*want));
    }
    if !used.is_empty() {
        let _ = writeln!(o, "bytes at the last busy second:");
        for (t, (held, cap)) in &used {
            let name = Tenant::from_u32(*t).map_or("?", Tenant::name);
            let _ = writeln!(o, "  {name:<9} holds {} of cap {}", mb(*held), mb(*cap));
        }
    }
    if let Some((rss, t, caches)) = peak {
        let _ = writeln!(
            o,
            "process peak {} at {}: the caches {}, the rest {} (decoders, driver, interface)",
            mb(rss),
            s(t),
            mb(caches),
            mb(rss.saturating_sub(caches))
        );
    }
    if !evicts.is_empty() {
        let n: u64 = evicts.values().sum();
        let worst = evicts.values().max().copied().unwrap_or(0);
        let _ = writeln!(o, "frames cache evictions: {n} in {} seconds with any, at most {worst} in one", evicts.len());
    }
    o
}

/// The program's trace directory: `KINETREN_DATA_DIR`, else the data directory.
fn default_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("KINETREN_DATA_DIR") {
        return std::path::PathBuf::from(d).join("trace");
    }
    kt_dirs::data_dir().join("trace")
}

fn kind(r: &Rec) -> Option<Ev> {
    r.kind()
}

fn is(r: &Rec, e: Ev) -> bool {
    r.ev == e as u16
}

pub fn summary(run: &Run) -> String {
    let mut o = String::new();
    let first = run.recs.first().map_or(0, |r| r.t);
    let last = run.recs.iter().map(|r| r.end()).max().unwrap_or(0);
    let _ = writeln!(o, "-- {} (pid {}), {} … {}, {} files --", run.label, run.pid, s(first), s(last), run.files.len());
    let mut by: BTreeMap<u16, (u64, u64)> = BTreeMap::new();
    for r in &run.recs {
        let e = by.entry(r.ev).or_default();
        e.0 += 1;
        e.1 += r.dur;
    }
    for (ev, (n, d)) in by {
        let name = Ev::from_u16(ev).map_or("?", |e| e.name());
        if d > 0 {
            let _ = writeln!(o, "  {name:<18} {n:>8}  {}", ms(d));
        } else {
            let _ = writeln!(o, "  {name:<18} {n:>8}");
        }
    }
    let lost: u64 = run.recs.iter().filter(|r| is(r, Ev::Dropped)).map(|r| r.a).sum();
    if lost > 0 {
        let _ = writeln!(o, "  LOST {lost} records to a full ring");
    }
    let _ = writeln!(o, "  threads: {}", {
        let mut v: Vec<_> = run.threads.iter().collect();
        v.sort();
        v.iter().map(|(k, n)| format!("{k}={n}")).collect::<Vec<_>>().join(", ")
    });
    let _ = writeln!(o, "  journal lines: {}", run.texts.len());
    o
}

fn class_name(c: u64) -> &'static str {
    kt_time::sched::Class::ALL.get(c as usize).map_or("?", |c| c.name())
}

/// The device rings (gpu-arbiter): who was admitted, who waited, what was cut, what the compute host
/// ran, and how long the display's presents waited for the device.
pub fn gpu(run: &Run) -> String {
    let mut o = String::new();
    let first = run.recs.first().map_or(0, |r| r.t);
    let last = run.recs.iter().map(|r| r.end()).max().unwrap_or(0);
    let span = (last.saturating_sub(first) as f64 / NS).max(1e-9);
    let _ = writeln!(o, "-- device rings over {span:.1} s --");
    // ring → class → (admissions, total wait, max wait, pieces)
    type Admits = BTreeMap<u64, (u64, u64, u64, u64)>;
    let mut rings: BTreeMap<u64, Admits> = BTreeMap::new();
    let mut shown: HashSet<u64> = HashSet::new();
    for r in &run.recs {
        if is(r, Ev::GpuAdmit) {
            let e = rings.entry(r.id).or_default().entry(r.aux as u64).or_default();
            e.0 += 1;
            e.1 += r.dur;
            e.2 = e.2.max(r.dur);
            if r.c == 1 {
                shown.insert(r.id);
            }
        } else if is(r, Ev::GpuPiece) {
            rings.entry(r.id).or_default().entry(r.aux as u64).or_default().3 += 1;
        }
    }
    for (ring, classes) in &rings {
        let _ = writeln!(o, "ring {ring}{}", if shown.contains(ring) { " (carries the display)" } else { "" });
        for (c, (n, wait, max, pieces)) in classes {
            let _ = writeln!(
                o,
                "  {:<10} {n:>6} admitted ({:.1}/s), waited {} (max {}), {pieces} pieces cut",
                class_name(*c),
                *n as f64 / span,
                ms(*wait),
                ms(*max)
            );
        }
    }
    let mut jobs: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
    for r in run.recs.iter().filter(|r| is(r, Ev::ComputeJob)) {
        let e = jobs.entry(r.id).or_default();
        e.0 += 1;
        e.1 += r.b;
    }
    for (c, (n, dev)) in &jobs {
        let _ = writeln!(o, "compute jobs {:<10} {n:>6} ({:.1}/s), device {}", class_name(*c), *n as f64 / span, ms(*dev));
    }
    // when the jobs ran on the device, on the CPU clock (found-defects 398): the wait before the
    // device started a job, its run, and the wait for its buffers after it ended
    // class → (start delays, runs, waits after the end, jobs with no timestamps, the worst error µs)
    type Starts = (Vec<u64>, Vec<u64>, Vec<u64>, u64, u32);
    let mut starts: BTreeMap<u64, Starts> = BTreeMap::new();
    for r in run.recs.iter().filter(|r| is(r, Ev::GpuStart)) {
        let e = starts.entry(r.id).or_default();
        if r.aux >= u32::MAX - 1 || r.b == 0 {
            e.3 += 1;
            continue;
        }
        e.0.push(r.b.saturating_sub(r.a));
        e.1.push(r.c.saturating_sub(r.b));
        e.2.push(r.t.saturating_sub(r.c));
        e.4 = e.4.max(r.aux);
    }
    for (c, (late, run_, signal, unknown, err)) in &mut starts {
        let q = |v: &mut Vec<u64>, f: f64| {
            v.sort_unstable();
            v.get(((v.len().max(1) - 1) as f64 * f) as usize).copied().unwrap_or(0)
        };
        let n = late.len();
        if n > 0 {
            let _ = writeln!(
                o,
                "  {:<10} {n:>6} on the device: start after submit p50 {} p95 {}, run p50 {}, buffers after end p50 {} p95 {} (clock error ≤ {} µs)",
                class_name(*c),
                ms(q(late, 0.5)),
                ms(q(late, 0.95)),
                ms(q(run_, 0.5)),
                ms(q(signal, 0.5)),
                ms(q(signal, 0.95)),
                err
            );
        }
        if *unknown > 0 {
            let _ = writeln!(o, "  {:<10} {unknown:>6} with no device timestamps", class_name(*c));
        }
    }
    let submits: u64 = run
        .recs
        .iter()
        .filter(|r| is(r, Ev::Work) && r.aux == kt_time::work::Work::DeviceSubmit as u32)
        .map(|r| r.a)
        .sum();
    if submits > 0 {
        let _ = writeln!(o, "device submits {submits} ({:.1}/s, busy seconds only)", submits as f64 / span);
    }
    let mut stops: BTreeMap<u32, u64> = BTreeMap::new();
    for r in run.recs.iter().filter(|r| is(r, Ev::StreamStop)) {
        *stops.entry(r.aux).or_default() += 1;
    }
    for (who, n) in &stops {
        let name = match *who {
            trace::stop::COMPOSE => "compose",
            trace::stop::PREFETCH => "prefetch",
            trace::stop::JOB => "job",
            _ => "?",
        };
        let _ = writeln!(o, "stream histories stopped by {name}: {n}");
    }
    let mut lat: Vec<u64> =
        run.recs.iter().filter(|r| is(r, Ev::FrameWait) && r.aux == trace::wait::DEVICE).map(|r| r.dur).collect();
    if !lat.is_empty() {
        lat.sort_unstable();
        let q = |f: f64| lat[((lat.len() - 1) as f64 * f) as usize];
        let _ = writeln!(
            o,
            "display presents {}: device wait p50 {} p95 {} max {}",
            lat.len(),
            ms(q(0.5)),
            ms(q(0.95)),
            ms(*lat.last().unwrap())
        );
    }
    o
}

pub fn machine(run: &Run) -> String {
    let mut o = String::new();
    for (k, v) in &run.machine {
        let _ = writeln!(o, "{k}: {v}");
    }
    o
}

// ---- frames --------------------------------------------------------------------------------------

/// One frame's records.
#[derive(Default, Clone, Debug)]
struct Life {
    id: u64,
    tick: u64,
    ask: Option<Rec>,
    plan: Option<Rec>,
    exec: Option<Rec>,
    done: Option<Rec>,
    show: Option<Rec>,
    drop: Option<Rec>,
    waits: Vec<Rec>,
    streams: Vec<Rec>,
    computes: Vec<Rec>,
    reqs: Vec<Rec>,
    layers: Vec<Rec>,
}

impl Life {
    fn complete(&self) -> bool {
        self.done.is_some_and(|d| d.aux == 1)
    }
    fn start(&self) -> u64 {
        self.ask.map_or_else(|| self.plan.map_or(0, |p| p.t), |a| a.t)
    }
    fn end(&self) -> u64 {
        self.done.map(|d| d.end()).or(self.drop.map(|d| d.t)).unwrap_or(self.start())
    }
}

fn lives(run: &Run) -> BTreeMap<u64, Life> {
    let mut m: BTreeMap<u64, Life> = BTreeMap::new();
    let frame_ids: HashSet<u64> = run.recs.iter().filter(|r| is(r, Ev::FrameAsk)).map(|r| r.id).collect();
    for r in &run.recs {
        let Some(k) = kind(r) else { continue };
        match k {
            Ev::FrameAsk => {
                let l = m.entry(r.id).or_default();
                l.id = r.id;
                l.tick = r.a;
                l.ask = Some(*r);
            }
            Ev::FramePlan => m.entry(r.id).or_default().plan = Some(*r),
            Ev::FrameExec => m.entry(r.id).or_default().exec = Some(*r),
            Ev::FrameDone => {
                let l = m.entry(r.id).or_default();
                l.done = Some(*r);
                l.tick = r.a;
            }
            Ev::FrameShow => m.entry(r.id).or_default().show = Some(*r),
            Ev::FrameDrop => m.entry(r.id).or_default().drop = Some(*r),
            Ev::FrameWait => m.entry(r.id).or_default().waits.push(*r),
            Ev::FrameLayer => m.entry(r.id).or_default().layers.push(*r),
            Ev::StreamRun if frame_ids.contains(&r.cause) => m.entry(r.cause).or_default().streams.push(*r),
            Ev::ComputeJob if frame_ids.contains(&r.cause) => m.entry(r.cause).or_default().computes.push(*r),
            Ev::DecReq if frame_ids.contains(&r.cause) => m.entry(r.cause).or_default().reqs.push(*r),
            _ => {}
        }
    }
    for (id, l) in m.iter_mut() {
        l.id = *id;
    }
    m
}

fn stream_word(aux: u32) -> &'static str {
    match aux {
        trace::stream::RENDERED => "rendered",
        trace::stream::HELD => "held (playing: the latest frame it had)",
        trace::stream::SHORT => "short of history",
        trace::stream::SOURCE_MISSING => "a source frame missing",
        trace::stream::DONE => "advanced to the frame",
        trace::stream::CANCELLED => "cancelled",
        _ => "?",
    }
}

fn exec_word(aux: u32) -> String {
    use trace::exec as x;
    let mut v = vec![];
    v.push(if aux & x::COMPLETE != 0 { "complete" } else { "INCOMPLETE" });
    if aux & x::MEMO_HIT != 0 {
        v.push("kept composite shown");
    }
    if aux & x::MEMO_STORED != 0 {
        v.push("kept for next lap");
    }
    if aux & x::STREAM_HELD != 0 {
        v.push("stream frame held");
    }
    if aux & x::SLOW != 0 {
        v.push("slow-chain device");
    }
    v.join(", ")
}

/// The critical path of one frame, a line per stage.
fn path(run: &Run, l: &Life, o: &mut String) {
    let t0 = l.start();
    let rel = |t: u64| ms(t.saturating_sub(t0));
    if let (Some(a), Some(p)) = (l.ask, l.plan) {
        let started = p.t.saturating_sub(a.t);
        if started > 1_000_000 {
            let _ = writeln!(o, "      queued on the compose thread {}", ms(started));
        }
    }
    for w in &l.waits {
        match w.aux {
            trace::wait::LOCK => {
                let _ = writeln!(o, "      lock {} for {}", run.name_of(w.a as u32), ms(w.dur));
            }
            trace::wait::DEVICE if w.dur > 1_000_000 => {
                let _ = writeln!(o, "      device finishing the frame before: {}", ms(w.dur));
            }
            trace::wait::BUILD => {
                let _ = writeln!(o, "      waited for the shared build of the project's state: {}", ms(w.dur));
            }
            _ => {}
        }
    }
    for st in &l.streams {
        let _ = writeln!(
            o,
            "      stream stage of clip {} at position {}: {} ({} positions lacked), {}",
            st.id,
            st.a,
            stream_word(st.aux),
            st.b,
            ms(st.dur)
        );
    }
    let mut kern: BTreeMap<u32, (u64, u64)> = BTreeMap::new();
    for c in &l.computes {
        let e = kern.entry(c.aux).or_default();
        e.0 += 1;
        e.1 += c.dur;
    }
    for (k, (n, d)) in kern {
        let _ = writeln!(o, "      compute {} ×{n}: {}", run.name_of(k), ms(d));
    }
    if let Some(p) = l.plan {
        let _ = writeln!(o, "      plan at {} took {} ({} layers, {} missing in the cache)", rel(p.t), ms(p.dur), p.aux, p.b);
    }
    if let Some(e) = l.exec {
        let cold = if e.b > 0 { format!(", {} pipelines compiled cold", e.b) } else { String::new() };
        let _ = writeln!(o, "      executor at {} took {}: {}{cold}", rel(e.t), ms(e.dur), exec_word(e.aux));
    }
    match (l.done, l.drop) {
        (Some(d), _) => {
            let _ = writeln!(o, "      done at {}: {}", rel(d.end()), if d.aux == 1 { "complete" } else { "INCOMPLETE" });
        }
        (None, Some(d)) => {
            let why = match d.aux {
                trace::drop::SUPERSEDED => "replaced by a newer ask before it was composed",
                trace::drop::STATE_LEFT => "the model moved on while it was composed",
                trace::drop::NO_PICTURE => "no executor, or the plan was refused",
                _ => "never taken",
            };
            let _ = writeln!(o, "      dropped at {}: {why}", rel(d.t));
        }
        _ => {}
    }
}

pub fn frame(run: &Run, id: u64) -> String {
    let all = lives(run);
    let Some(l) = all.get(&id) else { return format!("no frame {id}\n") };
    let mut o = format!("-- frame {id}, tick {}, asked {} --\n", tick_s(l.tick), s(l.start()));
    path(run, l, &mut o);
    let keys: HashSet<u64> = l.waits.iter().filter(|w| w.aux == trace::wait::DECODE).map(|w| w.a).collect();
    if !keys.is_empty() {
        let _ = writeln!(o, "    decodes it waited for:");
        for k in keys {
            decode_story(run, k, l.start(), &mut o);
        }
    }
    let _ = writeln!(o, "    records:");
    for r in run.recs.iter().filter(|r| r.id == id || r.cause == id) {
        let _ = writeln!(o, "      {}", line(run, r));
    }
    o
}

/// What happened to one cache key after `from`: requested, decoded, evicted (by what).
fn decode_story(run: &Run, key: u64, from: u64, o: &mut String) {
    let recs: Vec<&Rec> = run.recs.iter().filter(|r| r.id == key && r.t + 2_000_000_000 >= from).collect();
    let src = recs.first().map_or(0, |r| r.a);
    let idx = recs.first().map_or(0, |r| r.b);
    let mut parts = vec![];
    for r in recs {
        match kind(r) {
            Some(Ev::DecReq) => parts.push(format!("asked {} (prio {})", s(r.t), r.aux)),
            Some(Ev::DecWithdraw) => parts.push(format!("withdrawn {}", s(r.t))),
            Some(Ev::Decoded) => parts.push(format!("decoded {}", s(r.t))),
            Some(Ev::CacheEvict) => {
                let by = run.recs.iter().find(|x| x.id == r.cause && is(x, Ev::Decoded));
                let by = by.map_or(String::new(), |b| format!(" by the insert of source {} idx {}", b.a, tick_s(b.b)));
                let unread = if r.aux & trace::evict::UNREAD != 0 { " UNREAD" } else { "" };
                parts.push(format!("evicted{unread} {}{by}", s(r.t)))
            }
            _ => {}
        }
    }
    let _ = writeln!(o, "      source {src} at {}: {}", tick_s(idx), parts.join(" → "));
}

/// A stretch the preview waited for a picture of one tick.
#[derive(Debug, Clone)]
pub struct Stretch {
    pub from: u64,
    pub to: u64,
    pub tick: u64,
    pub frames: Vec<u64>,
    pub shown: bool,
}

/// The stretches: the UI's own "Loading" records when there are any; else from the first ask of a
/// tick until a complete frame of it, or another tick, longer than `min` ns.
pub fn stretches(run: &Run, min: u64) -> Vec<Stretch> {
    let all = lives(run);
    let mut out: Vec<Stretch> = vec![];
    let mut cur: Option<Stretch> = None;
    let mut order: Vec<&Life> = all.values().filter(|l| l.ask.is_some() || l.plan.is_some()).collect();
    order.sort_by_key(|l| l.start());
    for l in order {
        match cur.as_mut() {
            Some(c) if c.tick == l.tick && !c.shown => {
                c.frames.push(l.id);
                c.to = c.to.max(l.end());
                if l.complete() {
                    c.shown = true;
                    c.to = l.end();
                }
            }
            _ => {
                if let Some(c) = cur.take() {
                    out.push(c);
                }
                cur = Some(Stretch { from: l.start(), to: l.end(), tick: l.tick, frames: vec![l.id], shown: l.complete() });
            }
        }
    }
    out.extend(cur);
    let ui: Vec<&Rec> = run.recs.iter().filter(|r| is(r, Ev::Loading)).collect();
    if !ui.is_empty() {
        // the UI's scrim decides the stretches; the frames inside them are its waits
        let mut v = vec![];
        let mut on: Option<&Rec> = None;
        for r in ui {
            match (r.aux, on) {
                (1, None) => on = Some(r),
                (0, Some(a)) => {
                    v.push((a.t, r.t));
                    on = None;
                }
                _ => {}
            }
        }
        if let Some(a) = on {
            v.push((a.t, run.recs.last().map_or(a.t, |r| r.t)));
        }
        return v
            .into_iter()
            .map(|(a, b)| {
                let inside: Vec<&Stretch> = out.iter().filter(|x| x.to >= a && x.from <= b).collect();
                Stretch {
                    from: a.saturating_sub(180_000_000),
                    to: b,
                    tick: inside.last().map_or(0, |x| x.tick),
                    frames: inside.iter().flat_map(|x| x.frames.clone()).collect(),
                    shown: true,
                }
            })
            .collect();
    }
    out.into_iter().filter(|c| c.to - c.from >= min).collect()
}

/// Waits ranked by time in `[from, to)`, over the frames of a stretch and the threads that served
/// them.
fn waits(run: &Run, st: &Stretch, all: &BTreeMap<u64, Life>, o: &mut String) {
    let (from, to) = (st.from, st.to);
    let clip = |r: &Rec| r.end().min(to).saturating_sub(r.t.max(from));
    let mut rank: Vec<(u64, String)> = vec![];
    // the decodes the frames waited for
    let keys: HashSet<u64> = st
        .frames
        .iter()
        .filter_map(|f| all.get(f))
        .flat_map(|l| l.waits.iter().filter(|w| w.aux == trace::wait::DECODE).map(|w| w.a))
        .collect();
    if !keys.is_empty() {
        let mut worst = 0u64;
        let mut evicted = 0;
        for k in &keys {
            let done = run.recs.iter().find(|r| r.id == *k && is(r, Ev::Decoded) && r.t >= from);
            worst = worst.max(done.map_or(to, |d| d.t.min(to)).saturating_sub(from));
            evicted += run.recs.iter().filter(|r| r.id == *k && is(r, Ev::CacheEvict) && r.t >= from && r.t < to).count();
        }
        rank.push((worst, format!("decoded frames ({} keys; {evicted} of them evicted again inside the stretch)", keys.len())));
    }
    // stream stages: the composite's runs and the prefetch thread's advances
    let mut runs_short = 0u64;
    let mut runs_time = 0u64;
    let mut need_max = 0u64;
    for f in &st.frames {
        if let Some(l) = all.get(f) {
            for r in &l.streams {
                runs_time += r.dur;
                need_max = need_max.max(r.b);
                runs_short += u64::from(r.aux == trace::stream::SHORT || r.aux == trace::stream::SOURCE_MISSING);
            }
        }
    }
    let adv: Vec<&Rec> = run.recs.iter().filter(|r| is(r, Ev::StreamAdvance) && r.end() > from && r.t < to).collect();
    if runs_time > 0 || !adv.is_empty() {
        let at: u64 = adv.iter().map(|r| clip(r)).sum();
        let computed: u64 = adv.iter().map(|r| r.c).sum();
        let missing = adv.iter().filter(|r| r.aux == trace::stream::SOURCE_MISSING).count();
        rank.push((
            at + runs_time,
            format!(
                "stream history: {} composites short (up to {need_max} positions lacked), {} advances on the prefetch thread computed {computed} positions in {}, {missing} of them stopped by a missing source frame",
                runs_short,
                adv.len(),
                ms(at)
            ),
        ));
    }
    let mut kern: BTreeMap<u32, (u64, u64, u64)> = BTreeMap::new();
    for r in run.recs.iter().filter(|r| is(r, Ev::ComputeJob) && r.end() > from && r.t < to) {
        let e = kern.entry(r.aux).or_default();
        e.0 += 1;
        e.1 += clip(r);
        e.2 += r.b;
    }
    for (k, (n, d, dev)) in kern {
        rank.push((d, format!("compute {} ×{n} (device {})", run.name_of(k), ms(dev))));
    }
    let mevict = run.recs.iter().filter(|r| is(r, Ev::StreamMemoEvict) && r.t >= from && r.t < to).count();
    if mevict > 0 {
        let bytes: u64 = run.recs.iter().filter(|r| is(r, Ev::StreamMemoEvict) && r.t >= from && r.t < to).map(|r| r.a).sum();
        rank.push((0, format!("the stream memo evicted {mevict} entries ({:.0} MB) inside the stretch", bytes as f64 / 1e6)));
    }
    // the executor, the plan, locks, the device, warm-ups
    let mut exec = 0;
    let mut plan = 0;
    let mut lock = 0;
    let mut dev = 0;
    let mut build_wait = 0;
    let mut cold = 0;
    for f in &st.frames {
        if let Some(l) = all.get(f) {
            exec += l.exec.map_or(0, |e| e.dur);
            cold += l.exec.map_or(0, |e| e.b);
            plan += l.plan.map_or(0, |p| p.dur);
            for w in &l.waits {
                match w.aux {
                    trace::wait::LOCK => lock += w.dur,
                    trace::wait::DEVICE => dev += w.dur,
                    trace::wait::BUILD => build_wait += w.dur,
                    _ => {}
                }
            }
        }
    }
    if plan > 0 {
        rank.push((plan, format!("building plans ({} composites)", st.frames.len())));
    }
    if exec > 0 {
        rank.push((exec, format!("the executor ({cold} pipelines compiled cold)")));
    }
    if lock > 0 {
        rank.push((lock, "locks".into()));
    }
    if dev > 0 {
        rank.push((dev, "the device finishing earlier frames".into()));
    }
    if build_wait > 0 {
        rank.push((build_wait, "waiting for the shared build of an edit".into()));
    }
    let warm: u64 = run.recs.iter().filter(|r| is(r, Ev::Warm) && r.end() > from && r.t < to).map(clip).sum();
    if warm > 0 {
        rank.push((warm, "pipeline warm-ups".into()));
    }
    let builds: u64 = run
        .recs
        .iter()
        .filter(|r| {
            (is(r, Ev::AudioBuild)
                || (is(r, Ev::Job) && matches!(r.aux, trace::job::VIDEO_BUILD | trace::job::SHARED_BUILD)))
                && r.end() > from
                && r.t < to
        })
        .map(clip)
        .sum();
    if builds > 0 {
        rank.push((builds, "renderer builds".into()));
    }
    let passes: Vec<&Rec> =
        run.recs.iter().filter(|r| is(r, Ev::Job) && r.aux == trace::job::PREFETCH_PASS && r.end() > from && r.t < to).collect();
    if !passes.is_empty() {
        let d: u64 = passes.iter().map(|r| clip(r)).sum();
        rank.push((d, format!("prefetch passes ×{}", passes.len())));
    }
    rank.sort_by_key(|r| std::cmp::Reverse(r.0));
    for (d, what) in rank {
        let _ = writeln!(o, "    {:>10}  {what}", ms(d));
    }
}

pub fn loading(run: &Run, from: Option<u64>, to: Option<u64>) -> String {
    let all = lives(run);
    let mut o = String::new();
    let list: Vec<Stretch> = stretches(run, 180_000_000)
        .into_iter()
        .filter(|x| from.is_none_or(|f| x.to >= f) && to.is_none_or(|t| x.from <= t))
        .collect();
    if list.is_empty() {
        return "the preview never waited 180 ms or more for a picture in this window\n".into();
    }
    for st in &list {
        let _ = writeln!(
            o,
            "-- waited {} … {} ({}) for the frame at {}: {} composites, {} --",
            s(st.from),
            s(st.to),
            ms(st.to - st.from),
            tick_s(st.tick),
            st.frames.len(),
            if st.shown { "then shown" } else { "never shown (left)" }
        );
        let _ = writeln!(o, "  where the time went, ranked:");
        waits(run, st, &all, &mut o);
        if let Some(l) = st.frames.iter().rev().filter_map(|f| all.get(f)).find(|l| l.complete()).or(st.frames.last().and_then(|f| all.get(f))) {
            let _ = writeln!(o, "  the last composite ({}):", l.id);
            path(run, l, &mut o);
        }
        let acts: Vec<String> = run
            .texts
            .iter()
            .filter(|t| t.t + 3_000_000_000 >= st.from && t.t <= st.to)
            .map(|t| format!("    {} {}", s(t.t), t.s))
            .collect();
        if !acts.is_empty() {
            let _ = writeln!(o, "  the journal around it:");
            for a in acts.iter().take(12) {
                let _ = writeln!(o, "{a}");
            }
        }
    }
    o
}

// ---- loops ---------------------------------------------------------------------------------------

/// A lap: `[from, to)` of a loop region's pass.
pub fn lap_bounds(run: &Run) -> Vec<(u64, u64, u64, u64)> {
    let mut out = vec![];
    let mut region = (0u64, 0u64);
    let mut start: Option<u64> = None;
    for r in run.recs.iter().filter(|r| is(r, Ev::Play)) {
        match r.aux {
            trace::play::REGION => {
                region = (r.b, r.c);
                if let Some(a) = start.take() {
                    out.push((a, r.t, region.0, region.1));
                }
                start = None;
            }
            trace::play::PLAY | trace::play::SEEK if region.1 > region.0 => {
                if let Some(a) = start.take() {
                    out.push((a, r.t, region.0, region.1));
                }
                start = Some(r.t);
            }
            trace::play::WRAP if region.1 > region.0 => {
                if let Some(a) = start.replace(r.t) {
                    out.push((a, r.t, region.0, region.1));
                }
            }
            trace::play::PAUSE => {
                if let Some(a) = start.take() {
                    out.push((a, r.t, region.0, region.1));
                }
            }
            _ => {}
        }
    }
    if let Some(a) = start {
        out.push((a, run.recs.last().map_or(a, |r| r.t), region.0, region.1));
    }
    out
}

/// The stream history positions the walks of `[a, b)` computed (`stream_walk`: a batch's first
/// position and its count) and how many of them were distinct.
pub fn walked_in(run: &Run, a: u64, b: u64) -> (u64, usize) {
    let mut all = 0;
    let mut seen = HashSet::new();
    for r in run.recs.iter().filter(|r| is(r, Ev::StreamWalk) && r.t >= a && r.t < b) {
        all += r.b;
        for k in 0..r.b {
            seen.insert(r.a.wrapping_add(k));
        }
    }
    (all, seen.len())
}

pub fn laps(run: &Run) -> String {
    let all = lives(run);
    let bounds = lap_bounds(run);
    if bounds.is_empty() {
        return "no loop region was played in this run\n".into();
    }
    let mut o = String::new();
    // the key each tick was kept under, and when; the keys evicted
    let mut stored: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut evicted: HashMap<u64, Vec<u64>> = HashMap::new();
    for r in &run.recs {
        if is(r, Ev::MemoStore) {
            stored.entry(r.id).or_default().push(r.t);
        } else if is(r, Ev::MemoEvict) {
            evicted.entry(r.id).or_default().push(r.t);
        }
    }
    let mut seen_key: HashMap<u64, u64> = HashMap::new();
    let mut seen_layers: HashMap<u64, Vec<Rec>> = HashMap::new();
    let _ = writeln!(
        o,
        "{:>4} {:>9} {:>7} {:>6} {:>6} {:>6} {:>6}  audio: kept/live — why the rest were not served",
        "lap", "at", "len", "frames", "kept", "made", "incmpl"
    );
    for (n, &(a, b, rs, re)) in bounds.iter().enumerate() {
        let frames: Vec<&Life> = all.values().filter(|l| l.start() >= a && l.start() < b).collect();
        let kept = frames.iter().filter(|l| l.exec.is_some_and(|e| e.aux & trace::exec::MEMO_HIT != 0)).count();
        let made = frames.iter().filter(|l| l.exec.is_some_and(|e| e.aux & trace::exec::MEMO_HIT == 0)).count();
        let inc = frames.iter().filter(|l| l.done.is_some_and(|d| d.aux == 0)).count();
        let lap_recs = run.recs.iter().filter(|r| is(r, Ev::MixChunk) && r.t >= a && r.t < b);
        // a warm-up after an effects reset is not a chunk of its own (kept-mix-settle)
        let (warms, mix): (Vec<&Rec>, Vec<&Rec>) = lap_recs.partition(|r| r.aux == trace::mix::WARM);
        let rep = mix.iter().filter(|r| r.aux == trace::mix::REPLAY).count();
        let mut why: BTreeMap<&str, usize> = BTreeMap::new();
        if !warms.is_empty() {
            why.insert("effects warmed after the store", warms.len());
        }
        for r in mix.iter().filter(|r| r.aux != trace::mix::REPLAY) {
            let w = match (r.c & 0xff, r.aux) {
                (_, trace::mix::NOT_CLEAN) => "not clean (starved/pre-roll)",
                (_, trace::mix::UNSETTLED) => "not kept: effects still settling",
                (1, _) if r.c >> 8 == 1 => "lap context never kept",
                (1, _) => "never kept/evicted",
                (2, _) => "scene changed",
                (3, _) => "kept in part",
                _ => "outside the store",
            };
            *why.entry(w).or_default() += 1;
        }
        let why = why.iter().map(|(k, v)| format!("{k} ×{v}")).collect::<Vec<_>>().join(", ");
        // the stream history walked in the lap: positions computed and how many of them distinct
        // (stream-history-on-device, found-defects 394: his run computed 1413 for 618)
        let (walked, distinct) = walked_in(run, a, b);
        let walk = if walked > 0 { format!("  history: {walked} positions walked, {distinct} distinct;") } else { String::new() };
        let _ = writeln!(
            o,
            "{:>4} {:>9} {:>7} {:>6} {:>6} {:>6} {:>6} {walk} {rep}/{} {why}",
            n,
            s(a),
            format!("{:.2}s", (b - a) as f64 / NS),
            frames.len(),
            kept,
            made,
            inc,
            mix.len() - rep
        );
        // the composites made again on a lap after the first, and why
        if n > 0 {
            let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
            for l in frames.iter().filter(|l| l.exec.is_some_and(|e| e.aux & trace::exec::MEMO_HIT == 0)) {
                let e = l.exec.unwrap();
                let key = e.a;
                let r = if key == 0 {
                    "no kept key (bytes path or no executor)".to_string()
                } else if let Some(t) = stored.get(&key).and_then(|v| v.iter().rev().find(|t| **t < l.start())) {
                    match evicted.get(&key).is_some_and(|v| v.iter().any(|e| e > t && *e < l.start())) {
                        true => "evicted from the kept composites before this lap".into(),
                        false => format!("kept at {} yet composed again", s(*t)),
                    }
                } else if e.aux & trace::exec::STREAM_HELD != 0 {
                    "a stream stage showed a held frame (not the moment's own)".into()
                } else if seen_key.get(&l.tick).is_some_and(|k| *k != key) {
                    let before = seen_layers.get(&l.tick).cloned().unwrap_or_default();
                    match l.layers.iter().zip(&before).find(|(a, b)| (a.a, a.b, a.c) != (b.a, b.b, b.c)) {
                        // a decode's serial is a count; a stream stage's content is a hash
                        Some((a, b)) if (a.a, a.b) == (b.a, b.b) && a.c < 1 << 40 && b.c < 1 << 40 => {
                            format!("the frame's key changed since the last lap: layer {} (source {}) was decoded again", a.aux, a.a)
                        }
                        Some((a, b)) if (a.a, a.b) == (b.a, b.b) => {
                            format!("the frame's key changed since the last lap: layer {} (source {}) is another stream frame", a.aux, a.a)
                        }
                        Some((a, _)) => format!("the frame's key changed since the last lap: layer {} reads another frame", a.aux),
                        None => "the frame's key changed since the last lap (an input of the plan moved)".into(),
                    }
                } else if e.aux & trace::exec::SLOW != 0 {
                    "composed on the slow-chain device for the first time (kept since)".into()
                } else if e.aux & trace::exec::COMPLETE == 0 {
                    "incomplete (never kept)".into()
                } else {
                    "never kept on an earlier lap".into()
                };
                *reasons.entry(r).or_default() += 1;
            }
            for (r, c) in reasons {
                let _ = writeln!(o, "         {c:>4} × {r}");
            }
        }
        for l in &frames {
            if let Some(e) = l.exec.filter(|e| e.a != 0) {
                seen_key.insert(l.tick, e.a);
                if !l.layers.is_empty() {
                    seen_layers.insert(l.tick, l.layers.clone());
                }
            }
        }
        let _ = rs + re;
    }
    o
}

// ---- windows -------------------------------------------------------------------------------------

pub fn during(run: &Run, from: u64, to: u64) -> String {
    let mut o = format!("-- {} … {} --\n", s(from), s(to));
    let inside: Vec<&Rec> = run.recs.iter().filter(|r| r.end() > from && r.t < to).collect();
    let clip = |r: &Rec| r.end().min(to).saturating_sub(r.t.max(from));
    let mut th: BTreeMap<u16, u64> = BTreeMap::new();
    for r in inside.iter().filter(|r| r.dur > 0 && !is(r, Ev::FrameDone) && !is(r, Ev::FrameWait)) {
        *th.entry(r.th).or_default() += clip(r);
    }
    let mut tv: Vec<_> = th.into_iter().collect();
    tv.sort_by_key(|r| std::cmp::Reverse(r.1));
    let _ = writeln!(o, "  busy by thread (spans):");
    for (t, d) in tv.iter().take(12) {
        let _ = writeln!(o, "    {:>10}  {}", ms(*d), run.thread_of(*t));
    }
    let mut by: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for r in &inside {
        let Some(k) = kind(r) else { continue };
        let name = match k {
            Ev::Job => format!("job:{}", job_name(r.aux)),
            Ev::ComputeJob | Ev::Warm | Ev::ColdDraw => format!("{}:{}", k.name(), run.name_of(r.aux)),
            Ev::Play => format!("play:{}", play_name(r.aux)),
            Ev::FrameDrop => format!("frame_drop:{}", r.aux),
            Ev::FrameDone => format!("frame_done:{}", if r.aux == 1 { "complete" } else { "incomplete" }),
            Ev::MixChunk => format!("mix_chunk:{}", r.aux),
            Ev::SchedQueue | Ev::SchedRun | Ev::SchedCancel => sched_word(run, k, r),
            Ev::SchedPause => format!("sched_{}", if r.aux == 1 { "pause" } else { "resume" }),
            Ev::Block => format!("block:{}:{}", if r.a == kt_time::block::role::UI as u64 { "ui" } else { "render" }, kt_time::block::kind::name(r.aux)),
            _ => k.name().to_string(),
        };
        let e = by.entry(name).or_default();
        e.0 += 1;
        e.1 += clip(r);
    }
    let _ = writeln!(o, "  records:");
    for (k, (n, d)) in by {
        if d > 0 {
            let _ = writeln!(o, "    {k:<34} {n:>7}  {}", ms(d));
        } else {
            let _ = writeln!(o, "    {k:<34} {n:>7}");
        }
    }
    let builds: Vec<&&Rec> = inside.iter().filter(|r| is(r, Ev::Job) && r.aux == trace::job::SHARED_BUILD).collect();
    if !builds.is_empty() {
        let _ = writeln!(o, "  shared builds:");
        for b in builds {
            let _ = writeln!(o, "    {} version {}: {} events derived, {} held after it, {}", s(b.t), b.a, b.b, b.c, ms(b.dur));
        }
    }
    let unread = inside.iter().filter(|r| is(r, Ev::CacheEvict) && r.aux & trace::evict::UNREAD != 0).count();
    let thrash = inside.iter().filter(|r| is(r, Ev::CacheEvict) && r.aux & trace::evict::WANTED != 0).count();
    if unread > 0 {
        let _ = writeln!(o, "  cache: {unread} entries evicted unread, {thrash} of them still wanted (thrash)");
    }
    let texts: Vec<_> = run.texts.iter().filter(|t| t.t >= from && t.t < to).collect();
    if !texts.is_empty() {
        let _ = writeln!(o, "  journal:");
        for t in texts {
            let _ = writeln!(o, "    {} {}", s(t.t), t.s);
        }
    }
    o
}

/// A scheduler record named by its class and lane (one-scheduler): "sched_run:visible:thumbs".
fn sched_word(run: &Run, k: Ev, r: &Rec) -> String {
    let class = match k {
        Ev::SchedCancel => "",
        _ => kt_time::sched::Class::ALL.get(r.aux as usize).map_or("?", |c| c.name()),
    };
    if class.is_empty() {
        format!("{}:{}", k.name(), run.name_of(r.a as u32))
    } else {
        format!("{}:{class}:{}", k.name(), run.name_of(r.a as u32))
    }
}

fn job_name(aux: u32) -> &'static str {
    use trace::job as j;
    match aux {
        j::PREFETCH_PASS => "prefetch_pass",
        j::THUMB => "thumbnail",
        j::REEL_PREVIEW => "reel_preview",
        j::FX_THUMB => "fx_thumbnail",
        j::POSTER => "poster",
        j::ANALYSIS => "analysis",
        j::EXPORT_FRAME => "export_frame",
        j::VIDEO_BUILD => "video_build",
        j::RECHECK_SHOWN => "recheck_kept",
        j::PEAKS => "peaks",
        j::EXPORT => "export",
        j::SHARED_BUILD => "shared_build",
        _ => "?",
    }
}

fn play_name(aux: u32) -> &'static str {
    use trace::play as p;
    match aux {
        p::PLAY => "play",
        p::PAUSE => "pause",
        p::SEEK => "seek",
        p::WRAP => "wrap",
        p::REGION => "region",
        p::STALL => "stall",
        p::UNDERRUN => "underrun",
        p::RESET => "reset",
        p::DIRECTION => "direction",
        p::REPEATS => "repeats",
        p::RATE => "rate",
        p::START_HOLD => "start_hold",
        p::DEVICE_OPEN => "device_open",
        p::DEVICE_CLOSE => "device_close",
        p::DEVICE_ERROR => "device_error",
        _ => "?",
    }
}

pub fn slow(run: &Run, n: usize) -> String {
    let all = lives(run);
    let mut v: Vec<&Life> = all.values().filter(|l| l.done.is_some()).collect();
    v.sort_by_key(|l| std::cmp::Reverse(l.end() - l.start()));
    let mut o = String::new();
    for l in v.into_iter().take(n) {
        let _ = writeln!(o, "-- frame {} at {}, tick {}: {} --", l.id, s(l.start()), tick_s(l.tick), ms(l.end() - l.start()));
        path(run, l, &mut o);
    }
    o
}

fn line(run: &Run, r: &Rec) -> String {
    let name = kind(r).map_or("?", |k| k.name());
    let mut x = format!("{} {name} [{}] id={} cause={} a={} b={} c={} aux={}", s(r.t), run.thread_of(r.th), r.id, r.cause, r.a, r.b, r.c, r.aux);
    if r.dur > 0 {
        let _ = write!(x, " dur={}", ms(r.dur));
    }
    x
}

pub fn dump(run: &Run, from: Option<u64>, to: Option<u64>) -> String {
    let mut o = String::new();
    for r in run.recs.iter().filter(|r| from.is_none_or(|f| r.t >= f) && to.is_none_or(|t| r.t < t)) {
        let name = kind(r).map_or("?", |k| k.name());
        let _ = writeln!(
            o,
            "{{\"t\":{},\"dur\":{},\"ev\":\"{name}\",\"th\":\"{}\",\"id\":{},\"cause\":{},\"a\":{},\"b\":{},\"c\":{},\"aux\":{}}}",
            r.t,
            r.dur,
            esc(&run.thread_of(r.th)),
            r.id,
            r.cause,
            r.a,
            r.b,
            r.c,
            r.aux
        );
    }
    o
}

fn esc(v: &str) -> String {
    v.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Chrome trace event JSON: spans as complete events per thread, instants as instant events, the
/// frame's cause as the `args`.
pub fn chrome(run: &Run) -> String {
    let mut o = String::from("{\"traceEvents\":[\n");
    let mut first = true;
    for (th, name) in &run.threads {
        let _ = write!(
            o,
            "{}{{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":{th},\"args\":{{\"name\":\"{}\"}}}}",
            if first { "" } else { ",\n" },
            esc(name)
        );
        first = false;
    }
    for r in &run.recs {
        let Some(k) = kind(r) else { continue };
        let name = match k {
            Ev::ComputeJob | Ev::Warm | Ev::ColdDraw => format!("{}:{}", k.name(), run.name_of(r.aux)),
            Ev::Job => format!("job:{}", job_name(r.aux)),
            Ev::SchedQueue | Ev::SchedRun | Ev::SchedCancel => sched_word(run, k, r),
            _ => k.name().to_string(),
        };
        let ts = r.t as f64 / 1e3;
        let args = format!("{{\"id\":{},\"cause\":{},\"a\":{},\"b\":{},\"c\":{},\"aux\":{}}}", r.id, r.cause, r.a, r.b, r.c, r.aux);
        let sep = if first { "" } else { ",\n" };
        first = false;
        if r.dur > 0 {
            let _ = write!(o, "{sep}{{\"name\":\"{name}\",\"ph\":\"X\",\"pid\":1,\"tid\":{},\"ts\":{ts:.3},\"dur\":{:.3},\"args\":{args}}}", r.th, r.dur as f64 / 1e3);
        } else {
            let _ = write!(o, "{sep}{{\"name\":\"{name}\",\"ph\":\"i\",\"s\":\"t\",\"pid\":1,\"tid\":{},\"ts\":{ts:.3},\"args\":{args}}}", r.th);
        }
    }
    o.push_str("\n]}\n");
    o
}

#[cfg(test)]
mod tests;
