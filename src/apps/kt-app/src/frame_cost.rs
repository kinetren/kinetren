//! What one UI frame costs, stage by stage (ui-frame-cost, the owner's kkt-26.11.07b item 1: "движение
//! мышкой на паузе ... по превью - в htop показывает cpu 25%").
//!
//! The work counters (`kt_time::work`) say HOW MANY frames ran; this says what each one cost and
//! where. The time is the UI thread's own processor time, not the wall clock: a frame that waits
//! for the device or the driver is not charged for the wait, so the number is the same on a card
//! and on a machine that rasterizes in software, where the driver's own threads do the drawing.
//! It steers nothing and no offline path reads it (invariant 11): Diagnostics shows it, the load
//! report writes it per second, the live scenario `ui_frame` reads it from `<dump>.load.json`.
//!
//! A stage nests in another where the list says so: `Events` is part of `Timeline`, `Composite` of
//! `Centre`. Two clock reads a stage, eight a frame: cheaper than anything they measure.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Stage {
    /// the whole of `draw_all`
    Frame,
    /// the timeline column: rulers, rows, events, curves, markers, hit tests
    Timeline,
    /// the events of the timeline laid out and painted (part of `Timeline`)
    Events,
    /// the media bin column
    Bin,
    /// the preview, its controls and the meters
    Centre,
    /// the preview's composite asked of the executor (part of `Centre`)
    Composite,
    /// the floating windows: effects, transitions, camera, track, composition, Diagnostics…
    Windows,
}

impl Stage {
    pub const ALL: [Stage; 7] =
        [Stage::Frame, Stage::Timeline, Stage::Events, Stage::Bin, Stage::Centre, Stage::Composite, Stage::Windows];

    pub const fn name(self) -> &'static str {
        match self {
            Stage::Frame => "frame",
            Stage::Timeline => "timeline",
            Stage::Events => "events",
            Stage::Bin => "bin",
            Stage::Centre => "centre",
            Stage::Composite => "composite",
            Stage::Windows => "windows",
        }
    }
}

static NS: [AtomicU64; Stage::ALL.len()] = [const { AtomicU64::new(0) }; Stage::ALL.len()];
static FRAMES: AtomicU64 = AtomicU64::new(0);

/// The calling thread's processor time in nanoseconds; where the platform has no such clock, a
/// monotonic one (a wait is then charged too).
pub fn thread_ns() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: a valid out-pointer to a timespec on the stack
        if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) } == 0 {
            return ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64;
        }
    }
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
}

/// How long the calling thread has stood ready to run with no processor, in nanoseconds (Linux:
/// the second field of `/proc/thread-self/schedstat`; elsewhere 0). A long frame's wall time is
/// its work, its waits and this: on a loaded machine a frame of 10 ms of work stood 410 ms, and
/// only this tells the machine from a lock (test-hygiene, `ui_wait` at load 142).
pub fn run_delay_ns() -> u64 {
    #[cfg(target_os = "linux")]
    {
        use std::io::{Read, Seek};
        thread_local! {
            static FILE: std::cell::RefCell<Option<std::fs::File>> =
                std::cell::RefCell::new(std::fs::File::open("/proc/thread-self/schedstat").ok());
        }
        return FILE.with(|f| {
            let mut f = f.borrow_mut();
            let Some(f) = f.as_mut() else { return 0 };
            let mut buf = [0u8; 96];
            if f.seek(std::io::SeekFrom::Start(0)).is_err() {
                return 0;
            }
            let n = f.read(&mut buf).unwrap_or(0);
            std::str::from_utf8(&buf[..n])
                .ok()
                .and_then(|t| t.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0)
        });
    }
    #[allow(unreachable_code)]
    0
}

/// A stage being measured: its time is added when it drops.
pub struct Span {
    stage: Stage,
    t0: u64,
    wall: std::time::Instant,
}

thread_local! {
    /// The current frame's WALL time by stage (ui-never-waits): a long frame's journal note says
    /// where the time went, waits included, which the processor time above cannot see.
    static WALL: std::cell::Cell<[u64; Stage::ALL.len()]> = const { std::cell::Cell::new([0; Stage::ALL.len()]) };
}

pub fn span(stage: Stage) -> Span {
    if stage == Stage::Frame {
        FRAMES.fetch_add(1, Ordering::Relaxed);
        WALL.with(|w| w.set([0; Stage::ALL.len()]));
    }
    Span { stage, t0: thread_ns(), wall: std::time::Instant::now() }
}

impl Drop for Span {
    fn drop(&mut self) {
        NS[self.stage as usize].fetch_add(thread_ns().saturating_sub(self.t0), Ordering::Relaxed);
        let ns = self.wall.elapsed().as_nanos() as u64;
        WALL.with(|w| {
            let mut a = w.get();
            a[self.stage as usize] += ns;
            w.set(a);
        });
    }
}

/// Where the last frame on this thread spent its wall time: "timeline 12, events 3, …" in whole
/// milliseconds, the stages of a millisecond or more.
pub fn last_frame_wall() -> String {
    let a = WALL.with(|w| w.get());
    Stage::ALL
        .iter()
        .filter(|s| **s != Stage::Frame && a[**s as usize] >= 1_000_000)
        .map(|s| format!("{} {}", s.name(), a[*s as usize] / 1_000_000))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Frames measured and nanoseconds per stage since the start, process-wide.
pub fn totals() -> (u64, [u64; Stage::ALL.len()]) {
    (FRAMES.load(Ordering::Relaxed), std::array::from_fn(|i| NS[i].load(Ordering::Relaxed)))
}

/// The average frame between two readings of [`totals`]: milliseconds per frame by stage, or
/// `None` when no frame ran.
pub fn per_frame(
    a: &(u64, [u64; Stage::ALL.len()]),
    b: &(u64, [u64; Stage::ALL.len()]),
) -> Option<[f32; Stage::ALL.len()]> {
    let n = b.0.saturating_sub(a.0);
    (n > 0).then(|| std::array::from_fn(|i| b.1[i].saturating_sub(a.1[i]) as f32 / n as f32 / 1e6))
}

thread_local! {
    static VISITS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// One event, transition, marker or card got per-item drawing work in this frame. Its total is
/// what the rule "a frame costs what is visible" is tested by (t1912): it must not move when
/// events are added outside the view. Per thread: the harness's apps share a process.
pub fn visit() {
    VISITS.with(|v| v.set(v.get() + 1));
}

/// The visits counted on this thread so far.
#[cfg(test)]
pub fn visits() -> u64 {
    VISITS.with(|v| v.get())
}
