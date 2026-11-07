//! The load watch (idle-cpu, kkt-26.11.07 item E3: the owner had no tool to bring proof with, and
//! might not reach a menu in such lag). A thread of its own, on from the start, with no setup: once a second it reads
//! which threads held the processor and which work counters moved, and keeps the last quarter of
//! an hour in memory. A spell of load writes the file by itself, so a person in such lag has
//! nothing to reach: `load-report.txt` in the data directory is there to hand over. It stays on the
//! machine: nothing in telemetry reads it.
//!
//! The price at rest is one read of `/proc/self/task` a second and no write (t1839). It never
//! touches the UI thread, and the UI only reads the last sample.

use kt_time::work::{self, Work};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const REPORT_FILE: &str = "load-report.txt";
/// A quarter of an hour of seconds.
const KEEP: usize = 900;
const PERIOD: Duration = Duration::from_secs(1);
/// Under a live scenario: the scenario's own limits read this often.
const BENCH_PERIOD: Duration = Duration::from_millis(250);
/// A spell of load: this share of one core, for this many samples in a row.
const HOT_PERCENT: f32 = 100.0;
const HOT_SAMPLES: usize = 3;
/// A hot spell rewrites the file this seldom.
const WRITE_EVERY: Duration = Duration::from_secs(5);
const TOP_THREADS: usize = 6;

/// One second of the program.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sample {
    /// Seconds since the Unix epoch.
    pub at: u64,
    /// Percent of one core, over all threads.
    pub cpu: f32,
    /// The busiest threads, grouped by name.
    pub threads: Vec<(String, f32)>,
    /// Per second, in the order of `Work::ALL`.
    pub work: Vec<u32>,
}

impl Sample {
    /// Nothing moved: what the performance panel calls rest.
    pub fn at_rest(&self) -> bool {
        self.work.iter().all(|&n| n == 0)
    }
}

static LAST: Mutex<Option<Sample>> = Mutex::new(None);
static WRITE: AtomicBool = AtomicBool::new(false);

/// The last second, for the performance panel.
pub fn last() -> Option<Sample> {
    LAST.lock().unwrap().clone()
}

pub fn report_path() -> PathBuf {
    crate::journal::data_dir().join(REPORT_FILE)
}

/// Write the report at the next sample, whatever the load.
pub fn write_now() {
    WRITE.store(true, Ordering::Release);
}

/// Thread times by name, in processor ticks (100 per second on Linux). Threads of one pool are
/// one row: `llvmpipe-3` and `llvmpipe-4` are one fact.
#[cfg(target_os = "linux")]
fn thread_ticks() -> std::collections::BTreeMap<String, u64> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(dir) = std::fs::read_dir("/proc/self/task") else { return out };
    for t in dir.flatten() {
        if let Ok(stat) = std::fs::read_to_string(t.path().join("stat"))
            && let Some((name, ticks)) = parse_stat(&stat)
        {
            *out.entry(pool_name(name).to_string()).or_insert(0) += ticks;
        }
    }
    out
}

/// No per-thread times here yet: the counters still say what ran.
#[cfg(not(target_os = "linux"))]
fn thread_ticks() -> std::collections::BTreeMap<String, u64> {
    Default::default()
}

/// A thread's name and its user + system ticks from `/proc/<pid>/task/<tid>/stat`. The name may
/// hold spaces and brackets, so it is cut at the LAST `)`.
pub(crate) fn parse_stat(stat: &str) -> Option<(&str, u64)> {
    let name = &stat[stat.find('(')? + 1..stat.rfind(')')?];
    let rest: Vec<&str> = stat[stat.rfind(')')? + 2..].split_whitespace().collect();
    let num = |i: usize| rest.get(i)?.parse::<u64>().ok();
    Some((name, num(11)? + num(12)?))
}

/// `llvmpipe-12` → `llvmpipe`, `av:vp9:df3` → `av:vp9`, `media-worker-5` → `media-worker`.
pub(crate) fn pool_name(name: &str) -> &str {
    let t = name.trim_end_matches(|c: char| c.is_ascii_digit());
    let t = t.strip_suffix(":df").or_else(|| t.strip_suffix(":df-")).unwrap_or(t);
    let t = t.trim_end_matches(['-', ':', '_']);
    if t.is_empty() { name } else { t }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// One sample from two readings, `dt` apart.
fn sample_of(
    ticks: &std::collections::BTreeMap<String, u64>,
    before: &std::collections::BTreeMap<String, u64>,
    totals: &[u64],
    was: &[u64],
    dt: f32,
    at: u64,
) -> Sample {
    let percent = |n: u64| n as f32 / dt.max(0.001);
    let mut threads: Vec<(String, f32)> = ticks
        .iter()
        .filter_map(|(n, t)| {
            let d = t.saturating_sub(before.get(n).copied().unwrap_or(0));
            (d > 0).then(|| (n.clone(), percent(d)))
        })
        .collect();
    let cpu = threads.iter().map(|(_, p)| p).sum();
    threads.sort_by(|a, b| b.1.total_cmp(&a.1));
    threads.truncate(TOP_THREADS);
    let work = totals
        .iter()
        .zip(was)
        .map(|(n, w)| (n.saturating_sub(*w) as f32 / dt.max(0.001)).round() as u32)
        .collect();
    Sample { at, cpu, threads, work }
}

/// A line of the report: how long ago, the load, who held it, what ran.
pub(crate) fn line(s: &Sample, now: u64) -> String {
    let threads: Vec<String> = s.threads.iter().map(|(n, p)| format!("{n} {p:.0}")).collect();
    let work: Vec<String> = Work::ALL
        .iter()
        .zip(&s.work)
        .filter(|(_, n)| **n > 0)
        .map(|(w, n)| format!("{} {n}", w.name()))
        .collect();
    format!(
        "{:>6} s ago  {:>5.0} %  {}  |  {}",
        now.saturating_sub(s.at),
        s.cpu,
        if threads.is_empty() { "-".to_string() } else { threads.join(", ") },
        if work.is_empty() { "at rest".to_string() } else { work.join(", ") },
    )
}

/// The report: what the machine is, newest second first.
pub(crate) fn report(ring: &std::collections::VecDeque<Sample>, now: u64) -> String {
    let mut out = format!("Kinetren load report\nversion {}\n{} {}\n", kt_version::FULL, std::env::consts::OS, std::env::consts::ARCH);
    out.push_str(&format!("logical processors: {}\n", std::thread::available_parallelism().map_or(0, |n| n.get())));
    out.push_str("devices:\n");
    for a in kt_gpu::adapter_facts() {
        out.push_str(&format!(
            "  {} / {:?} / {:?} / {:04x}:{:04x}\n",
            a.name, a.kind, a.backend, a.vendor, a.device
        ));
    }
    let driving: Vec<String> =
        kt_gpu::driving_cards().iter().map(|c| format!("{:04x}:{:04x}", c.vendor, c.device)).collect();
    out.push_str(&format!("monitors hang off: {}\n", if driving.is_empty() { "unknown".to_string() } else { driving.join(", ") }));
    out.push_str(&format!("the choice: {:?}\n", kt_gpu::display_power_preference()));
    out.push_str("\nseconds, percent of one core, threads, work per second; 100 % = one core\n");
    for s in ring.iter().rev() {
        out.push_str(&line(s, now));
        out.push('\n');
    }
    out
}

fn write_report(ring: &std::collections::VecDeque<Sample>) {
    let path = report_path();
    let tmp = path.with_extension("txt.tmp");
    if std::fs::write(&tmp, report(ring, unix_now())).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// The counters a live scenario reads at rest, when no frame writes the geometry dump.
fn bench_dump(path: &str, totals: &[u64], ticks: &std::collections::BTreeMap<String, u64>) {
    let work: Vec<String> =
        Work::ALL.iter().zip(totals).map(|(w, n)| format!("\"{}\":{n}", w.name())).collect();
    let threads: Vec<String> = ticks.iter().map(|(n, t)| format!("\"{n}\":{t}")).collect();
    let json = format!("{{\"work\":{{{}}},\"ticks\":{{{}}}}}\n", work.join(","), threads.join(","));
    let tmp = format!("{path}.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Start the watch: once per process.
pub fn start() {
    let bench = std::env::var("KT_GEOM_DUMP").ok().map(|p| format!("{p}.load.json"));
    let period = if bench.is_some() { BENCH_PERIOD } else { PERIOD };
    let _ = std::thread::Builder::new().name("load-watch".into()).spawn(move || {
        let mut ring = std::collections::VecDeque::with_capacity(KEEP);
        let mut before = thread_ticks();
        let mut was = work::totals();
        let mut t0 = Instant::now();
        let (mut hot, mut written): (usize, Option<Instant>) = (0, None);
        loop {
            std::thread::sleep(period);
            let (ticks, totals) = (thread_ticks(), work::totals());
            let dt = t0.elapsed().as_secs_f32();
            t0 = Instant::now();
            if let Some(p) = &bench {
                bench_dump(p, &totals, &ticks);
            }
            let s = sample_of(&ticks, &before, &totals, &was, dt, unix_now());
            before = ticks;
            was = totals;
            hot = if s.cpu >= HOT_PERCENT { hot + 1 } else { 0 };
            if ring.len() == KEEP {
                ring.pop_front();
            }
            ring.push_back(s.clone());
            *LAST.lock().unwrap() = Some(s);
            let due = hot >= HOT_SAMPLES && written.is_none_or(|t| t.elapsed() >= WRITE_EVERY);
            if due || WRITE.swap(false, Ordering::AcqRel) {
                write_report(&ring);
                written = Some(Instant::now());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test t1839 (idle-cpu): the watch reads a thread's name with spaces and brackets, groups a
    /// pool into one row, and a second of nothing is rest.
    #[test]
    fn t1839_samples_name_the_threads_and_the_work() {
        let stat = "123 (we (ird) name) S 1 2 3 4 5 6 7 8 9 10 700 40 0 0 20 0 1 0 0";
        assert_eq!(parse_stat(stat), Some(("we (ird) name", 740)));
        assert_eq!(pool_name("llvmpipe-12"), "llvmpipe");
        assert_eq!(pool_name("av:vp9:df13"), "av:vp9");
        assert_eq!(pool_name("media-worker-5"), "media-worker");
        assert_eq!(pool_name("kinetren"), "kinetren");
        let map = |v: &[(&str, u64)]| v.iter().map(|(n, t)| (n.to_string(), *t)).collect();
        let n = Work::ALL.len();
        let s = sample_of(&map(&[("a", 150), ("b", 10)]), &map(&[("a", 50), ("b", 10)]), &vec![30; n], &vec![0; n], 2.0, 9);
        assert_eq!(s.threads, vec![("a".to_string(), 50.0)], "a thread that did nothing is no row");
        assert_eq!(s.cpu, 50.0, "100 ticks over two seconds is half a core");
        assert_eq!(s.work, vec![15; n]);
        assert!(!s.at_rest());
        assert!(line(&s, 12).contains("3 s ago"), "{}", line(&s, 12));
        let rest = sample_of(&map(&[]), &map(&[]), &vec![0; n], &vec![0; n], 1.0, 9);
        assert!(rest.at_rest());
        assert!(line(&rest, 9).contains("at rest"));
    }
}
