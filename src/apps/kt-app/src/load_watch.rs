//! The load watch (idle-cpu, kkt-26.11.07 item E3: the owner had no tool to bring proof with, and
//! might not reach a menu in such lag). A thread of its own, on from the start, with no setup: once a second it reads
//! which threads held the processor and which work counters moved, and hands each busy second to
//! the telemetry journal. A spell of load writes the file by itself, so a person in such lag has
//! nothing to reach: `load-report.txt` in the data directory is there to hand over. It stays on the
//! machine: nothing in telemetry reads it.
//!
//! The price at rest is one read of `/proc/self/task` a second and no write (t1839). It never
//! touches the UI thread, and the UI only reads the last sample.
//!
//! telemetry-journal (kkt-26.11.07b G6): each busy second goes into the telemetry journal AT that
//! second, and the report is the journal's last quarter of an hour with clock time — it said "487 s
//! ago" from the moment it was written, and the owner could not say what moment was what. The
//! Diagnostics notes (evictions, no-read drops, low video hit, offline media) are raised here too,
//! at the second they happen: inside a UI frame they waited for the next mouse move (t1966).

use kt_time::work::{self, Work};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const REPORT_FILE: &str = "load-report.txt";
const PERIOD: Duration = Duration::from_secs(1);
/// Under a live scenario: the scenario's own limits read this often.
const BENCH_PERIOD: Duration = Duration::from_millis(250);
/// A spell of load: this share of one core, for this many samples in a row.
const HOT_PERCENT: f32 = 100.0;
const HOT_SAMPLES: usize = 3;
/// A hot spell rewrites the file this seldom.
const WRITE_EVERY: Duration = Duration::from_secs(5);
const TOP_THREADS: usize = 6;
/// A thread holding this share of a core while every counter stands still is work nobody counts
/// (input-without-work): the owner's report read "at rest" at 196 % while prefetch and the audio
/// render rebuilt the engine. The watch's own thread is not counted against it.
const UNCOUNTED_PERCENT: f32 = 10.0;

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
    /// What a UI frame cost the UI thread in that second, on average, in milliseconds of its own
    /// processor time (`frame_cost`, ui-frame-cost); none when no frame ran.
    pub frame_ms: Option<f32>,
    /// What the GPU did in that second, pass by pass (gpu-pass-cost); none when nothing composed.
    pub gpu: Option<kt_gpu::pass_time::Second>,
}

impl Sample {
    /// Nothing moved and nothing held the processor: what the performance panel calls rest.
    pub fn at_rest(&self) -> bool {
        self.counted_nothing() && !self.uncounted()
    }

    fn counted_nothing(&self) -> bool {
        self.work.iter().all(|&n| n == 0)
    }

    /// A thread was busy and no counter saw it: never rest, and a gap in the counters to close.
    pub fn uncounted(&self) -> bool {
        self.counted_nothing()
            && self.threads.iter().any(|(n, p)| n != "load-watch" && *p >= UNCOUNTED_PERCENT)
    }
}

/// What the watch reads of the player to raise the Diagnostics notes; handed over by each `KtApp`.
pub struct PlayerProbe {
    pub cache: std::sync::Arc<kt_cache::BurstCache>,
    pub transport: std::sync::Arc<kt_engine::transport::Transport>,
    pub offline: std::sync::Arc<Mutex<std::collections::HashSet<kt_model::MediaId>>>,
}

/// A spike of evictions: this many per second.
const EVICTION_SPIKE: f32 = 500.0;
/// A low video hit while playing, said at most this often.
const LOW_HIT_PERCENT: f32 = 80.0;
const LOW_HIT_EVERY: Duration = Duration::from_secs(10);

/// What the previous sample saw of the probe.
#[derive(Default)]
pub(crate) struct NoteState {
    cache: Option<kt_cache::CacheStats>,
    offline: usize,
    hit_warned: Option<Instant>,
}

static LAST: Mutex<Option<Sample>> = Mutex::new(None);
static PROBE: Mutex<Option<PlayerProbe>> = Mutex::new(None);

/// The player the notes watch from now on (a new project builds a new one).
pub fn watch(p: PlayerProbe) {
    *PROBE.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
}

/// The notes of one second: what moved in the cache and the media since the previous one.
pub(crate) fn probe_notes(p: &PlayerProbe, st: &mut NoteState, dt: f32) -> Vec<String> {
    let mut out = Vec::new();
    let now = p.cache.stats();
    let was = st.cache.replace(now).unwrap_or(now);
    let per_s = |n: u64| n as f32 / dt.max(0.001);
    let ev = now.evictions.saturating_sub(was.evictions);
    if per_s(ev) > EVICTION_SPIKE {
        out.push(format!("cache eviction spike: {:.0}/s, the cache is too small", per_s(ev)));
    }
    let unread = now.unused_evictions.saturating_sub(was.unused_evictions);
    if unread > 0 {
        out.push(format!("prefetch thrash: {unread} cache entries dropped before the first read"));
    }
    let (vh, vm) = (now.video_hits.saturating_sub(was.video_hits), now.video_misses.saturating_sub(was.video_misses));
    let hit = if vh + vm > 0 { vh as f32 / (vh + vm) as f32 * 100.0 } else { 100.0 };
    if hit < LOW_HIT_PERCENT && p.transport.is_playing() && st.hit_warned.is_none_or(|t| t.elapsed() >= LOW_HIT_EVERY) {
        st.hit_warned = Some(Instant::now());
        out.push(format!("video cache hit {hit:.0} % while playing: decoding cannot keep up"));
    }
    let offline = p.offline.lock().map_or(st.offline, |o| o.len());
    if offline != st.offline {
        out.push(format!("media availability changed: offline {offline}"));
        st.offline = offline;
    }
    out
}
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
    Sample { at, cpu, threads, work, frame_ms: None, gpu: None }
}

/// A second as the journal records it: the load, who held it, what ran. The time is the journal's.
pub(crate) fn line(s: &Sample) -> String {
    let threads: Vec<String> = s.threads.iter().map(|(n, p)| format!("{n} {p:.0}")).collect();
    let work: Vec<String> = Work::ALL
        .iter()
        .zip(&s.work)
        .filter(|(_, n)| **n > 0)
        .map(|(w, n)| format!("{} {n}", w.name()))
        .collect();
    format!(
        "{:>5.0} %  {}  |  {}{}",
        s.cpu,
        if threads.is_empty() { "-".to_string() } else { threads.join(", ") },
        if !work.is_empty() {
            work.join(", ")
        } else if s.uncounted() {
            "NO COUNTER MOVED".to_string()
        } else {
            "at rest".to_string()
        },
        s.frame_ms.map_or(String::new(), |ms| format!(", ui frame {ms:.1} ms")),
    )
}

/// Seconds of the report.
pub const REPORT_SPAN_MS: i64 = crate::tele_journal::TAIL_SPAN_MS;

/// The machine: build, processors, devices, which card drives the monitors.
fn header() -> String {
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
    out
}

/// The report: the machine, then the journal's last quarter of an hour, oldest first, each record
/// with its clock time — load, playback, notes and what the person did.
pub(crate) fn report(header: &str, recs: &[crate::tele_journal::Rec]) -> String {
    let mut out = header.to_string();
    out.push_str(&format!("journal: {} in the data directory\n", crate::tele_journal::FILE));
    out.push_str("\nthe telemetry journal's last 15 minutes; LOAD: percent of one core (100 % = one core), threads | work per second\n");
    out.push_str("GPU: the device that second: adapter | frame and decode size | frames, device ms a frame, span | each pass's mean ms ×count, the largest total first | the compute host's jobs\n");
    for r in recs {
        out.push_str(&r.line);
        out.push('\n');
    }
    out
}

fn write_report() {
    let path = report_path();
    let tmp = path.with_extension("txt.tmp");
    let recs = crate::tele_journal::global()
        .map(|j| j.since(j.now().unix_ms - REPORT_SPAN_MS))
        .unwrap_or_default();
    if std::fs::write(&tmp, report(&header(), &recs)).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// One second into the journal, at that second: a busy one as a LOAD record, the first rest after
/// a busy spell as one "at rest" record; a long rest writes nothing. Returns whether it was busy.
pub(crate) fn journal_second(j: &crate::tele_journal::Journal, at: crate::tele_journal::Stamp, s: &Sample, was_busy: bool) -> bool {
    use crate::tele_journal::Kind;
    let busy = !s.at_rest();
    if busy {
        j.record_at(at, Kind::Load, &line(s));
        // typed, for the query: the second's load and each counter that moved (structured-telemetry)
        let t = at.mono_ms * 1_000_000;
        kt_time::trace::ev(kt_time::trace::Ev::Load).at(t).a((s.cpu * 10.0) as u64).emit();
        for (i, n) in s.work.iter().enumerate().filter(|(_, n)| **n > 0) {
            kt_time::trace::ev(kt_time::trace::Ev::Work).at(t).aux(i as u32).a(*n as u64).emit();
        }
        // the device's second beside the processor's (gpu-pass-cost)
        if let Some(g) = &s.gpu {
            j.record_at(at, Kind::Gpu, &g.line());
        }
    } else if was_busy {
        j.record_at(at, Kind::Load, "at rest");
    }
    busy
}

/// The counters a live scenario reads at rest, when no frame writes the geometry dump.
fn bench_dump(path: &str, totals: &[u64], ticks: &std::collections::BTreeMap<String, u64>) {
    let work: Vec<String> =
        Work::ALL.iter().zip(totals).map(|(w, n)| format!("\"{}\":{n}", w.name())).collect();
    let threads: Vec<String> = ticks.iter().map(|(n, t)| format!("\"{n}\":{t}")).collect();
    // the frames' cost by stage (ui-frame-cost): frames measured and nanoseconds per stage
    let (frames, ns) = crate::frame_cost::totals();
    let stages: Vec<String> =
        crate::frame_cost::Stage::ALL.iter().zip(ns).map(|(st, n)| format!("\"{}\":{n}", st.name())).collect();
    let json = format!(
        "{{\"work\":{{{}}},\"ticks\":{{{}}},\"frame\":{{\"frames\":{frames},\"ns\":{{{}}}}}}}\n",
        work.join(","),
        threads.join(","),
        stages.join(",")
    );
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
        let mut before = thread_ticks();
        let mut was = work::totals();
        let mut cost_was = crate::frame_cost::totals();
        let mut t0 = Instant::now();
        let (mut hot, mut written): (usize, Option<Instant>) = (0, None);
        let (mut busy, mut notes) = (false, NoteState::default());
        loop {
            std::thread::sleep(period);
            let journal = crate::tele_journal::global();
            let at = journal.as_ref().map(|j| j.now());
            let (ticks, totals) = (thread_ticks(), work::totals());
            let dt = t0.elapsed().as_secs_f32();
            t0 = Instant::now();
            if let Some(p) = &bench {
                bench_dump(p, &totals, &ticks);
            }
            let mut s = sample_of(&ticks, &before, &totals, &was, dt, unix_now());
            let cost = crate::frame_cost::totals();
            s.frame_ms = crate::frame_cost::per_frame(&cost_was, &cost).map(|ms| ms[0]);
            // the device's passes of the same second: a lock and a swap, nothing when nothing composed
            s.gpu = kt_gpu::pass_time::take();
            cost_was = cost;
            before = ticks;
            was = totals;
            if let (Some(j), Some(at)) = (&journal, at) {
                busy = journal_second(j, at, &s, busy);
                if let Some(p) = PROBE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    for n in probe_notes(p, &mut notes, dt) {
                        j.record_at(at, crate::tele_journal::Kind::Note, &n);
                    }
                }
            }
            hot = if s.cpu >= HOT_PERCENT { hot + 1 } else { 0 };
            *LAST.lock().unwrap() = Some(s);
            let due = hot >= HOT_SAMPLES && written.is_none_or(|t| t.elapsed() >= WRITE_EVERY);
            if due || WRITE.swap(false, Ordering::AcqRel) {
                write_report();
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
        // telemetry-journal: no time relative to the writing; the journal's record carries the time
        assert!(!line(&s).contains("ago") && line(&s).starts_with("   50 %  a 50"), "{}", line(&s));
        let rest = sample_of(&map(&[]), &map(&[]), &vec![0; n], &vec![0; n], 1.0, 9);
        assert!(rest.at_rest());
        assert!(line(&rest).contains("at rest"));
    }

    /// Test t1880 (input-without-work): a second with prefetch and the audio render busy and no
    /// counter moving is not rest: the owner's report said "at rest" at 196 % (its lines 482
    /// and 512). The watch's own tick and a thread at a few percent still are.
    #[test]
    fn t1880_a_busy_thread_with_no_counter_is_not_rest() {
        let map = |v: &[(&str, u64)]| v.iter().map(|(n, t)| (n.to_string(), *t)).collect();
        let n = Work::ALL.len();
        let still = vec![0; n];
        let busy = sample_of(&map(&[("prefetch", 95), ("audio-render", 92)]), &map(&[]), &still, &still, 1.0, 9);
        assert!(!busy.at_rest() && busy.uncounted());
        let l = line(&busy);
        assert!(!l.contains("at rest") && l.contains("NO COUNTER MOVED"), "{l}");
        let quiet = sample_of(&map(&[("load-watch", 20), ("cpal_alsa_out", 1)]), &map(&[]), &still, &still, 1.0, 9);
        assert!(quiet.at_rest() && !quiet.uncounted(), "the watch's own tick is rest");
        // the work the owner's report could not see now has its rows
        for w in [Work::ModelEdit, Work::ProjectSwap, Work::EngineBuild] {
            assert!(Work::ALL.contains(&w), "{}", w.name());
        }
    }

    fn probe() -> PlayerProbe {
        PlayerProbe {
            cache: std::sync::Arc::new(kt_cache::BurstCache::new(kt_cache::CacheConfig {
                max_audio_bytes: 1 << 20,
                max_video_bytes: 1 << 20,
            })),
            transport: std::sync::Arc::new(kt_engine::transport::Transport::new()),
            offline: Default::default(),
        }
    }

    fn frame() -> std::sync::Arc<kt_media::VideoFrame> {
        std::sync::Arc::new(kt_media::VideoFrame {
            pts: kt_time::Tick(0),
            width: 4,
            height: 4,
            rgba: vec![0u8; 64].into_boxed_slice(),
            has_alpha: false,
        })
    }

    /// Test t1966 (telemetry-journal): the Diagnostics notes are raised by the watch at the second
    /// they happen, with no UI frame: inside a frame (`sample_perf`) a problem on a pause waited
    /// for the next mouse move and carried that frame's time (found-defects 109).
    #[test]
    fn t1966_a_note_is_stamped_at_its_second() {
        use crate::tele_journal::{Kind, testing::*};
        let clock = FakeClock::new(1_762_498_800_000, 0);
        let j = fresh("t1966", clock.clone(), crate::tele_journal::ROTATE_BYTES);
        let p = probe();
        let mut st = NoteState::default();
        assert!(probe_notes(&p, &mut st, 1.0).is_empty(), "the first look only remembers");
        // a frame dropped before its first read, and a file gone offline, in one second
        let src = kt_cache::SourceId(1);
        p.cache.insert_video(src, None, 0, frame());
        p.cache.insert_video(src, None, 0, frame());
        p.offline.lock().unwrap().insert(kt_model::MediaId(3));
        clock.advance(4_000);
        let at = j.now();
        let notes = probe_notes(&p, &mut st, 1.0);
        clock.advance(60_000); // the reader comes a minute later
        for n in &notes {
            j.record_at(at, Kind::Note, n);
        }
        let tail = j.tail(10);
        assert_eq!(tail.len(), 2, "{notes:?}");
        assert!(tail[0].line.contains("prefetch thrash: 1"), "{}", tail[0].line);
        assert!(tail[1].line.contains("offline 1"), "{}", tail[1].line);
        for r in &tail {
            assert!(r.line.starts_with("2025-11-07 07:00:04.000+00:00"), "{}", r.line);
        }
        assert!(probe_notes(&p, &mut st, 1.0).is_empty(), "nothing moved since");
    }

    /// Test t1967 (telemetry-journal): the load report is the journal's last quarter of an hour
    /// with clock time — a load second, an edit and a mark, none "s ago".
    #[test]
    fn t1967_report_says_clock_time_and_holds_the_actions() {
        use crate::tele_journal::{Kind, testing::*};
        let clock = FakeClock::new(1_762_498_800_000, 3 * 3600);
        let j = fresh("t1967", clock.clone(), crate::tele_journal::ROTATE_BYTES);
        let map = |v: &[(&str, u64)]| v.iter().map(|(n, t)| (n.to_string(), *t)).collect();
        let n = Work::ALL.len();
        let busy = sample_of(&map(&[("prefetch", 90)]), &map(&[]), &vec![1; n], &vec![0; n], 1.0, 0);
        let rest = sample_of(&map(&[]), &map(&[]), &vec![0; n], &vec![0; n], 1.0, 0);
        assert!(journal_second(&j, j.now(), &busy, false));
        clock.advance(1_000);
        assert!(!journal_second(&j, j.now(), &rest, true), "the first rest closes the spell");
        clock.advance(1_000);
        assert!(!journal_second(&j, j.now(), &rest, false));
        j.record(Kind::Act, "edit SetFx event 2132");
        clock.advance(500);
        j.record(Kind::Mark, "stall here");
        clock.advance(10 * 60_000); // the report is written ten minutes later
        let text = report("Kinetren load report\n", &j.since(j.now().unix_ms - REPORT_SPAN_MS));
        assert!(!text.contains("ago"), "{text}");
        let body: Vec<&str> = text.lines().filter(|l| l.starts_with("2025")).collect();
        assert_eq!(body.len(), 4, "a busy second, the end of the spell, an edit, a mark: {text}");
        assert!(body[0].starts_with("2025-11-07 10:00:00.000+03:00") && body[0].contains("LOAD") && body[0].contains("prefetch 90"));
        assert!(body[1].starts_with("2025-11-07 10:00:01.000+03:00") && body[1].ends_with("at rest"));
        assert!(body[2].starts_with("2025-11-07 10:00:02.000+03:00") && body[2].contains("ACT"));
        assert!(body[3].starts_with("2025-11-07 10:00:02.500+03:00") && body[3].ends_with("MARK   stall here"));
        // older than the quarter of an hour falls out of the report
        clock.advance(6 * 60_000);
        assert!(j.since(j.now().unix_ms - REPORT_SPAN_MS).len() < 4);
    }

    /// Test t2021 (gpu-pass-cost): a busy second that composed carries a GPU record at the same
    /// moment, after its LOAD record, with the adapter, the sizes and each pass's mean and count;
    /// a second that composed nothing carries none.
    #[test]
    fn t2021_a_busy_second_carries_the_gpu_record() {
        use crate::tele_journal::testing::*;
        let clock = FakeClock::new(1_762_498_800_000, 3 * 3600);
        let j = fresh("t2021", clock.clone(), crate::tele_journal::ROTATE_BYTES);
        let map = |v: &[(&str, u64)]| v.iter().map(|(n, t)| (n.to_string(), *t)).collect();
        let n = Work::ALL.len();
        let mut busy = sample_of(&map(&[("kinetren", 40)]), &map(&[]), &vec![1; n], &vec![0; n], 1.0, 0);
        busy.gpu = Some(kt_gpu::pass_time::Second {
            adapters: vec!["AMD Radeon Graphics (RADV RAPHAEL_MENDOCINO)".into()],
            timestamps: true,
            frames: 24,
            preview: 24,
            frame_ns: 24 * 7_300_000,
            span_ns: 24 * 7_500_000,
            passes: vec![("vhs_tape:s5-head-read".into(), 24 * 6_100_000, 24), ("composite".into(), 24 * 1_200_000, 24)],
            out: (1920, 1080),
            decode: (1920, 1080),
            layers: 1,
            ..Default::default()
        });
        assert!(journal_second(&j, j.now(), &busy, false));
        clock.advance(1_000);
        let quiet = sample_of(&map(&[("kinetren", 40)]), &map(&[]), &vec![1; n], &vec![0; n], 1.0, 0);
        assert!(journal_second(&j, j.now(), &quiet, true));
        let text = report("Kinetren load report\n", &j.since(j.now().unix_ms - REPORT_SPAN_MS));
        let body: Vec<&str> = text.lines().filter(|l| l.starts_with("2025")).collect();
        assert_eq!(body.len(), 3, "LOAD, GPU, LOAD: {text}");
        assert!(body[0].contains("LOAD") && body[1].contains("GPU    AMD Radeon"), "{text}");
        assert_eq!(body[0][..23], body[1][..23], "the GPU record carries its second's time: {text}");
        for part in ["frame 1920×1080, decode 1920×1080 ×1", "24 frames (preview 24), 7.30 ms a frame", "vhs_tape:s5-head-read 6.100 ×24", "composite 1.200 ×24"] {
            assert!(body[1].contains(part), "{part}: {text}");
        }
        assert!(body[2].contains("LOAD"), "a second that composed nothing has no GPU record: {text}");
    }
}
