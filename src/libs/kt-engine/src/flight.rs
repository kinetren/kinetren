//! “Black box” of playback (R.28, item 10).
//!
//! The user's request, verbatim: “we need to think through a clever logging system
//! that will help future agents with floating playback bugs; it has to be compact,
//! because I could just jump around several super-small loop regions, which
//! could generate
//! tons of logs.”
//!
//! Hence three rules:
//! 1. **A fixed-size ring.** No matter how much you jump — exactly
//!    `CAP` records of memory, the oldest ones are evicted. Not a file on disk during
//!    play, no RAM growth.
//! 2. **Deduplication of consecutive repeats.** An identical event (the same kind and the
//!    same fields) does not multiply records: it increments the counter `n` and the time of
//!    the last repeat: a hundred wraps of the same loop — ONE
//!    line `WRAP … ×100`.
//! 3. **A counter snapshot once a second, and only if something changed.**
//!    In silence the log does not grow at all.
//!
//! Written ONLY by the render/prefetch thread. The recorder does not enter the cpal callback
//! (invariant 6: no allocations, no locks) — the callback's counters are taken from
//! atomics at the next snapshot.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Instant;

/// Ring capacity. 512 records — this is tens of seconds of the most turbulent work
/// (jumps + snapshots), about 40 KB of text in a dump.
pub const CAP: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ev {
    Play,
    Pause,
    /// Transport jump (generation): a = the destination frame.
    Seek,
    /// Loop region change: a..b in frames (a<0 — region removed).
    Region,
    /// Region wrap: a = the frame from which the cycle started.
    Wrap,
    /// Direction change: a = sign.
    Dir,
    /// Mini-seek without a generation bump (armed/disarmed flip, sign change).
    Reset,
    /// Counters snapshot (see `Snap`).
    Snap,
    /// Stall: the playhead did not move although playback is running. a = ms of idleness.
    Stall,
}

impl Ev {
    pub fn tag(self) -> &'static str {
        match self {
            Ev::Play => "PLAY",
            Ev::Pause => "PAUSE",
            Ev::Seek => "SEEK",
            Ev::Region => "REGION",
            Ev::Wrap => "WRAP",
            Ev::Dir => "DIR",
            Ev::Reset => "RESET",
            Ev::Snap => "SNAP",
            Ev::Stall => "STALL",
        }
    }
}

/// The snapshot counters are deltas per second, not absolute values: exactly the
/// delta shows “the left number grows on every repeat”.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Snap {
    pub render_underruns: u64,
    pub callback_underruns: u64,
    pub warmup_misses: u64,
    pub head_frames: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rec {
    /// Ms since the start of recording (monotonic).
    pub at_ms: u64,
    pub kind: Ev,
    pub a: i64,
    pub b: i64,
    pub snap: Snap,
    /// How many times in a row it repeated (dedup).
    pub n: u32,
}

pub struct FlightRecorder {
    ring: Mutex<VecDeque<Rec>>,
    t0: Instant,
    /// Stall episodes since the start, and the head of the last one: the interface reads them
    /// every frame, and the ring would mean a lock and a copy (invariant 12). `clear` keeps them.
    stalls: AtomicU64,
    stall_head: AtomicI64,
    sink: std::sync::OnceLock<FlightSink>,
}

/// What the sink is told (telemetry-journal): a record as it is pushed, or a run of repeats that a
/// different record has just closed — with its count and the time of its last repeat in `rec`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sunk {
    New(Rec),
    /// `now_ms` is the recorder's clock at the closing push: the last repeat was `now_ms - at_ms` ago.
    Repeats { rec: Rec, now_ms: u64 },
}

/// The telemetry journal's door into the ring. Called by whichever thread pushes (the render thread,
/// `Player::play/pause`), outside the ring's lock; it must not block (a channel send).
pub type FlightSink = Box<dyn Fn(Sunk) + Send + Sync>;

/// One freeze of the playhead is ONE episode (R.72, M5). The engine asked "stands ≥ 1 s?" every
/// pass and pushed a record each time, so one freeze flooded the 512-entry ring and the "froze"
/// counter overstated it. At rate 0 the head stands by request: no stall.
///
/// The wait starts at the first PLAYING look, never at the last look before it: the render thread
/// parks on a pause with no clock (one-scheduler), so the look before a play can be seconds old,
/// and counting from it reported "playback froze" for the rest itself, on every play after a
/// second of rest (t2303).
#[derive(Debug, Default)]
pub struct StallWatch {
    head: i64,
    since_ms: u64,
    stalled: bool,
    /// The last look was a playing one at a rate: `since_ms` is a moment of this playing stretch.
    armed: bool,
}

impl StallWatch {
    pub fn new(head: i64) -> StallWatch {
        StallWatch { head, since_ms: 0, stalled: false, armed: false }
    }

    /// `Some(ms)` — the moment the episode starts; nothing more until the head moves.
    pub fn tick(&mut self, now_ms: u64, playing: bool, rate: f32, head: i64) -> Option<u64> {
        if !playing || rate == 0.0 {
            *self = StallWatch { head, since_ms: now_ms, stalled: false, armed: false };
            return None;
        }
        if !self.armed || head != self.head {
            *self = StallWatch { head, since_ms: now_ms, stalled: false, armed: true };
            return None;
        }
        let idle = now_ms.saturating_sub(self.since_ms);
        if self.stalled || idle < 1000 {
            return None;
        }
        self.stalled = true;
        Some(idle)
    }
}

/// Every push goes into the trace as it happens, repeats included (structured-telemetry): the
/// query splits a loop into its laps by the wraps. Positions in ticks.
fn trace_play(r: &Rec) {
    use kt_time::trace::{self, play};
    if !trace::on() {
        return;
    }
    let tick = |f: i64| kt_time::Tick::from_audio_frames(f).0 as u64;
    let (kind, a, b, c) = match r.kind {
        Ev::Play => (play::PLAY, tick(r.a), 0, 0),
        Ev::Pause => (play::PAUSE, tick(r.a), 0, 0),
        Ev::Seek => (play::SEEK, tick(r.a), 0, 0),
        Ev::Region if r.a < 0 => (play::REGION, 0, 0, 0),
        Ev::Region => (play::REGION, 0, tick(r.a), tick(r.b)),
        Ev::Wrap => (play::WRAP, tick(r.a), 0, 0),
        Ev::Dir => (play::DIRECTION, 0, r.a as u64, 0),
        Ev::Reset => (play::RESET, tick(r.a), 0, 0),
        Ev::Snap => (play::UNDERRUN, tick(r.snap.head_frames), r.snap.callback_underruns, r.snap.render_underruns),
        Ev::Stall => (play::STALL, tick(r.b), r.a as u64, 0),
    };
    trace::ev(trace::Ev::Play).aux(kind).a(a).b(b).c(c).emit();
}

impl Default for FlightRecorder {
    fn default() -> Self {
        FlightRecorder::new()
    }
}

impl FlightRecorder {
    pub fn new() -> FlightRecorder {
        FlightRecorder {
            ring: Mutex::new(VecDeque::with_capacity(CAP)),
            t0: Instant::now(),
            stalls: AtomicU64::new(0),
            stall_head: AtomicI64::new(0),
            sink: std::sync::OnceLock::new(),
        }
    }

    /// Hand every record to `sink` as it is pushed (once per recorder; a second call is ignored).
    pub fn set_sink(&self, sink: FlightSink) {
        let _ = self.sink.set(sink);
    }

    pub fn now_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    /// An event without counters.
    pub fn push(&self, kind: Ev, a: i64, b: i64) {
        self.push_at(self.now_ms(), kind, a, b, Snap::default());
    }

    /// A counters snapshot. An empty one (all deltas zero and the head did not move)
    /// is not written — in silence the log does not grow.
    pub fn push_snap(&self, s: Snap) {
        if s.render_underruns == 0 && s.callback_underruns == 0 && s.warmup_misses == 0 {
            return;
        }
        self.push_at(self.now_ms(), Ev::Snap, 0, 0, s);
    }

    /// Stall episodes since the start and the head of the last one, in audio frames.
    pub fn stalls(&self) -> (u64, i64) {
        (self.stalls.load(Ordering::Acquire), self.stall_head.load(Ordering::Acquire))
    }

    pub fn push_at(&self, at_ms: u64, kind: Ev, a: i64, b: i64, snap: Snap) {
        if kind == Ev::Stall {
            self.stall_head.store(b, Ordering::Release);
            self.stalls.fetch_add(1, Ordering::AcqRel);
        }
        let rec = Rec { at_ms, kind, a, b, snap, n: 1 };
        trace_play(&rec);
        let closed = {
            let mut ring = self.lock();
            if let Some(last) = ring.back_mut()
                && last.kind == kind
                && last.a == a
                && last.b == b
                && last.snap == snap
            {
                last.n += 1;
                last.at_ms = at_ms;
                return;
            }
            let closed = ring.back().filter(|l| l.n > 1).copied();
            if ring.len() == CAP {
                ring.pop_front();
            }
            ring.push_back(rec);
            closed
        };
        if let Some(sink) = self.sink.get() {
            if let Some(c) = closed {
                sink(Sunk::Repeats { rec: c, now_ms: at_ms });
            }
            sink(Sunk::New(rec));
        }
    }

    pub fn records(&self) -> Vec<Rec> {
        self.lock().iter().copied().collect()
    }

    pub fn clear(&self) {
        self.lock().clear();
    }

    /// A poison-tolerant lock (R.49, item 355). The “black box” reads from
    /// the PANIC HOOK — that is, exactly when someone has already panicked, including,
    /// possibly, under this lock. `unwrap()` would give a second panic
    /// inside the hook, and that is an immediate `abort` with no report at all: an instrument
    /// that breaks from the very accident it is installed for. The content of the
    /// ring on poisoning is still meaningful: it is a log of past events,
    /// not a half-finished structure.
    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<Rec>> {
        self.ring.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Text dump: one line per record, time in seconds since start,
    /// frames in seconds (48 kHz), so it reads without a calculator.
    pub fn dump(&self) -> String {
        let mut out = String::new();
        for r in self.records() {
            out.push_str(&format!("{:8.3} {}", r.at_ms as f64 / 1000.0, describe(&r)));
            if r.n > 1 {
                out.push_str(&format!(" ×{}", r.n));
            }
            out.push('\n');
        }
        out
    }
}

/// A record without its time and count: `SEEK   head=12.000`.
pub fn describe(r: &Rec) -> String {
    let sec = |f: i64| f as f64 / kt_time::AUDIO_SAMPLE_RATE as f64;
    let mut out = format!("{:6}", r.kind.tag());
    match r.kind {
        Ev::Snap => out.push_str(&format!(
            " u={}/{} w={} head={:.3}",
            r.snap.render_underruns,
            r.snap.callback_underruns,
            r.snap.warmup_misses,
            sec(r.snap.head_frames)
        )),
        Ev::Region => {
            if r.a < 0 {
                out.push_str(" off");
            } else {
                out.push_str(&format!(" {:.3}..{:.3}", sec(r.a), sec(r.b)));
            }
        }
        Ev::Stall => out.push_str(&format!(" {} ms head={:.3}", r.a, sec(r.b))),
        Ev::Play | Ev::Pause | Ev::Seek | Ev::Wrap | Ev::Reset => {
            out.push_str(&format!(" head={:.3}", sec(r.a)))
        }
        Ev::Dir => out.push_str(if r.a < 0 { " reverse" } else { " forward" }),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t373 (R.49, item 355): a poisoned ring IS READ. The “black box” calls the
    /// panic hook — that is, exactly when someone has already panicked,
    /// including under this lock. `unwrap()` would give a second panic inside the
    /// hook, and that is an immediate `abort` with no report at all.
    #[test]
    fn t373_poisoned_flight_ring_still_reads() {
        let f = std::sync::Arc::new(FlightRecorder::new());
        f.push(Ev::Play, 48_000, 0);

        // panic UNDER the lock — that is how it is really poisoned
        let g = f.clone();
        let res = std::thread::spawn(move || {
            let _held = g.ring.lock().unwrap();
            panic!("under the flight lock");
        })
        .join();
        assert!(res.is_err(), "the thread was required to panic");
        assert!(f.ring.is_poisoned(), "the mutex is not poisoned — the test checks nothing");

        // both writing, reading and dumping keep working
        f.push(Ev::Pause, 96_000, 0);
        let recs = f.records();
        assert_eq!(recs.len(), 2, "after poisoning the ring is lost");
        let dump = f.dump();
        assert!(dump.to_lowercase().contains("play"), "dump is empty: {dump:?}");
        f.clear();
        assert!(f.records().is_empty());
    }

    /// Test t1965 (telemetry-journal): the sink gets each record as it is pushed, and a run of
    /// repeats once, closed by the next record, with the time of its last repeat — a hundred wraps
    /// of a tiny loop are one line in the journal, not a hundred (R.28: "tons of logs").
    #[test]
    fn t1965_sink_gets_records_and_closed_repeats() {
        let f = FlightRecorder::new();
        let got = std::sync::Arc::new(Mutex::new(Vec::new()));
        let g = got.clone();
        f.set_sink(Box::new(move |s| g.lock().unwrap().push(s)));
        f.push_at(10, Ev::Play, 0, 0, Snap::default());
        for i in 0..100 {
            f.push_at(20 + i, Ev::Wrap, 48_000, 0, Snap::default());
        }
        f.push_at(500, Ev::Pause, 96_000, 0, Snap::default());
        let got = got.lock().unwrap();
        let kinds: Vec<String> = got
            .iter()
            .map(|s| match s {
                Sunk::New(r) => format!("new {}", r.kind.tag()),
                Sunk::Repeats { rec, now_ms } => format!("{} x{} last {} now {now_ms}", rec.kind.tag(), rec.n, rec.at_ms),
            })
            .collect();
        assert_eq!(kinds, ["new PLAY", "new WRAP", "WRAP x100 last 119 now 500", "new PAUSE"]);
        assert_eq!(describe(&f.records()[2]), "PAUSE  head=2.000");
    }
}
