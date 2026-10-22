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
    fn tag(self) -> &'static str {
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
}

impl Default for FlightRecorder {
    fn default() -> Self {
        FlightRecorder::new()
    }
}

impl FlightRecorder {
    pub fn new() -> FlightRecorder {
        FlightRecorder { ring: Mutex::new(VecDeque::with_capacity(CAP)), t0: Instant::now() }
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

    pub fn push_at(&self, at_ms: u64, kind: Ev, a: i64, b: i64, snap: Snap) {
        let mut ring = self.lock();
        // consecutive dedup: “×N” instead of N lines (rule 2)
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
        if ring.len() == CAP {
            ring.pop_front();
        }
        ring.push_back(Rec { at_ms, kind, a, b, snap, n: 1 });
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
        let sec = |f: i64| f as f64 / kt_time::AUDIO_SAMPLE_RATE as f64;
        let mut out = String::new();
        for r in self.records() {
            out.push_str(&format!("{:8.3} {:6}", r.at_ms as f64 / 1000.0, r.kind.tag()));
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
            if r.n > 1 {
                out.push_str(&format!(" ×{}", r.n));
            }
            out.push('\n');
        }
        out
    }
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
}
