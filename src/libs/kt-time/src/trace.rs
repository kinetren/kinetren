//! The structured trace (structured-telemetry; the owner, kkt-26.11.09: "у тебя телеметрия должна
//! быть такая, чтобы всё было ок а не возня в ASCII логи которые вообще мало что говорят"): typed
//! records of what the program did and why, read by a tool (`kt-cli trace`), never by eye.
//!
//! The rules, each with its test:
//! - **A record is a fixed typed layout** ([`Rec`], 64 bytes): the event, the time since the process
//!   started in nanoseconds, a duration for a span, an `id`, a `cause` and three numbers whose meaning
//!   the event defines ([`Ev`]). Names (an effect, a pass, a kernel) are interned: a record carries a
//!   32-bit hash, the string goes into the file once ([`name`]).
//! - **Ids join cause and effect across threads.** A frame's id rides with every request it makes:
//!   the thread sets its cause ([`with_cause`]) and every record it makes carries it; a decode
//!   request names its key ([`key_id`]), and the worker's records name the same key.
//! - **Recording never waits** (t2143). A record is stored into a lock-free ring: no lock, no
//!   allocation (after a thread's first record, which names the thread), no system call but one
//!   wake of the writer per burst of activity. A full ring drops the record and counts it; the
//!   writer says how many it lost. The audio CALLBACK records nothing (invariant 6); the render
//!   thread may.
//! - **At rest nothing wakes** (t2142). The writer parks when a drain found nothing; the first record
//!   after that unparks it; while records come it drains every [`BATCH`].
//! - **Bounded on disk** (t2141). Segments of [`Config::segment_bytes`] under one directory; past
//!   [`Config::budget_bytes`] the oldest segment of any run is deleted. Each segment repeats the
//!   names, threads, machine and run records, so it reads alone.
//! - Off until [`install`]: a test binary that installs nothing pays one relaxed load per record site.

use std::cell::{Cell, UnsafeCell};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const MAGIC: &[u8; 8] = b"KTTRACE1";
pub const FORMAT: u32 = 1;
/// The file extension of a segment.
pub const EXT: &str = "ktt";
/// While records come the writer drains this often.
pub const BATCH: Duration = Duration::from_millis(25);
const RING: usize = 1 << 16;

/// What a record is about. Append only: the numbers are in files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u16)]
pub enum Ev {
    /// The preview asked for a frame. `id` frame, `a` tick, `b` the frame it replaced (0 none),
    /// `aux` [`ask`] flags.
    FrameAsk = 1,
    /// A frame was not composed or its picture was thrown away. `id` frame, `aux` [`drop`] reason.
    FrameDrop = 2,
    /// A frame's plan was built (a span). `id` frame, `a` the executor's kept-composite key (0 none),
    /// `b` layers missing in the cache, `c` route ([`route`]), `aux` layers.
    FramePlan = 3,
    /// What a frame waits for. `id` frame, `aux` [`wait`] kind, `a`/`b`/`c` by kind, `dur` when timed.
    FrameWait = 4,
    /// The executor ran a frame (a span). `id` frame, `a` kept key, `b` pipelines drawn cold,
    /// `aux` [`exec`] flags.
    FrameExec = 5,
    /// A frame came out of the compose thread. `id` frame, `a` tick, `dur` from its ask,
    /// `aux` 1 complete / 0 incomplete.
    FrameDone = 6,
    /// The UI took a frame. `id` frame, `a` tick, `aux` [`show`] flags.
    FrameShow = 7,
    /// The preview's "Loading" scrim came (`aux` 1) or went (`aux` 0). `a` the tick on screen.
    Loading = 8,
    /// One layer of a composed frame's plan: `id` frame, `a` source, `b` cache index, `c` its
    /// content (a decode's serial, a stream stage's key), `aux` the layer's place.
    FrameLayer = 9,

    /// A request reached a decoder's queue. `id` [`key_id`], `a` source, `b` index, `c` packed size
    /// ([`size`]), `aux` priority, `cause` who asked (a frame, a pass).
    DecReq = 10,
    /// A queued request was withdrawn. `id` key, `a` source, `b` index.
    DecWithdraw = 11,
    /// A decoder read a span (a span). `a` source, `b` first index, `c` keys asked, `aux` pictures.
    DecSpan = 12,
    /// A decoded entry went into the cache. `id` key, `a` source, `b` index, `c` size.
    Decoded = 13,
    /// The cache evicted an entry. `id` key, `a` source, `b` index, `c` size, `aux` [`evict`] flags,
    /// `cause` the key whose insert pushed it out.
    CacheEvict = 14,
    /// A source's entries were dropped. `a` source, `b` entries.
    CacheInvalidate = 15,

    /// A stream stage ran for a frame (a span). `a` position, `b` positions still needed,
    /// `c` the position shown (held) or the position, `aux` [`stream`] outcome, `id` clip.
    StreamRun = 20,
    /// The prefetch thread advanced a stage's history (a span). `a` first position, `b` the target,
    /// `c` positions computed, `aux` [`stream`] outcome, `id` clip.
    StreamAdvance = 21,
    /// The stream memo evicted an entry. `id` its key, `a` bytes, `cause` the key put.
    StreamMemoEvict = 22,

    /// A compute job ran (a span). `aux` kernel [`name`], `a` dispatches.
    ComputeJob = 30,

    /// The executor kept a composite. `id` key, `a` bytes held after it.
    MemoStore = 40,
    /// The executor dropped a kept composite. `id` key, `cause` the key stored.
    MemoEvict = 41,
    /// A pipeline was warmed off a frame (a span). `aux` its [`name`].
    Warm = 42,
    /// A pipeline was drawn in a frame before it was warm. `aux` its [`name`].
    ColdDraw = 43,

    /// Playback changed state. `aux` [`play`] kind, `a` head tick, `b`/`c` by kind (a region's ends,
    /// a count of repeats).
    Play = 50,

    /// The audio render thread filled the ring (a span). `a` frames rendered live, `b` frames handed
    /// out from the kept mix, `c` the render head (frames).
    AudioFill = 60,
    /// The kept-mix decision for a chunk. `a` first frame, `b` frames, `c` scene hash, `aux` [`mix`].
    MixChunk = 61,
    /// An audio renderer was built (a span), `a` the project version.
    AudioBuild = 62,

    /// A job ran (a span). `aux` [`job`] kind, `id` the job, `a`/`b` by kind.
    Job = 70,

    /// A UI frame (a span). `a` its number.
    UiFrame = 80,
    /// The model took an edit. `a` revision, `b` the player's project version.
    Edit = 81,
    /// Something kept was invalidated. `aux` [`inval`] what, `a` a version or source, `b` entries.
    Invalidate = 82,
    /// A work counter moved in a busy second. `aux` the `Work` index, `a` the delta.
    Work = 83,
    /// A busy second of the load watch. `a` percent of one core ×10.
    Load = 84,

    /// The writer lost records to a full ring. `a` how many.
    Dropped = 90,
}

impl Ev {
    pub const ALL: [Ev; 34] = [
        Ev::FrameAsk,
        Ev::FrameLayer,
        Ev::FrameDrop,
        Ev::FramePlan,
        Ev::FrameWait,
        Ev::FrameExec,
        Ev::FrameDone,
        Ev::FrameShow,
        Ev::Loading,
        Ev::DecReq,
        Ev::DecWithdraw,
        Ev::DecSpan,
        Ev::Decoded,
        Ev::CacheEvict,
        Ev::CacheInvalidate,
        Ev::StreamRun,
        Ev::StreamAdvance,
        Ev::StreamMemoEvict,
        Ev::ComputeJob,
        Ev::MemoStore,
        Ev::MemoEvict,
        Ev::Warm,
        Ev::ColdDraw,
        Ev::Play,
        Ev::AudioFill,
        Ev::MixChunk,
        Ev::AudioBuild,
        Ev::Job,
        Ev::UiFrame,
        Ev::Edit,
        Ev::Invalidate,
        Ev::Work,
        Ev::Load,
        Ev::Dropped,
    ];

    pub fn from_u16(v: u16) -> Option<Ev> {
        Ev::ALL.iter().copied().find(|e| *e as u16 == v)
    }

    pub fn name(self) -> &'static str {
        match self {
            Ev::FrameAsk => "frame_ask",
            Ev::FrameDrop => "frame_drop",
            Ev::FramePlan => "frame_plan",
            Ev::FrameWait => "frame_wait",
            Ev::FrameExec => "frame_exec",
            Ev::FrameDone => "frame_done",
            Ev::FrameShow => "frame_show",
            Ev::Loading => "loading",
            Ev::FrameLayer => "frame_layer",
            Ev::DecReq => "dec_req",
            Ev::DecWithdraw => "dec_withdraw",
            Ev::DecSpan => "dec_span",
            Ev::Decoded => "decoded",
            Ev::CacheEvict => "cache_evict",
            Ev::CacheInvalidate => "cache_invalidate",
            Ev::StreamRun => "stream_run",
            Ev::StreamAdvance => "stream_advance",
            Ev::StreamMemoEvict => "stream_memo_evict",
            Ev::ComputeJob => "compute_job",
            Ev::MemoStore => "memo_store",
            Ev::MemoEvict => "memo_evict",
            Ev::Warm => "warm",
            Ev::ColdDraw => "cold_draw",
            Ev::Play => "play",
            Ev::AudioFill => "audio_fill",
            Ev::MixChunk => "mix_chunk",
            Ev::AudioBuild => "audio_build",
            Ev::Job => "job",
            Ev::UiFrame => "ui_frame",
            Ev::Edit => "edit",
            Ev::Invalidate => "invalidate",
            Ev::Work => "work",
            Ev::Load => "load",
            Ev::Dropped => "dropped",
        }
    }
}

/// `FrameAsk.aux` bits.
pub mod ask {
    pub const PRESENT: u32 = 1;
    pub const PLAYING: u32 = 2;
    pub const RETRY: u32 = 4;
}

/// `FrameDrop.aux`.
pub mod drop {
    /// A newer ask replaced it before it was composed.
    pub const SUPERSEDED: u32 = 1;
    /// The model moved on while it was composed: nobody shows it.
    pub const STATE_LEFT: u32 = 2;
    /// No executor, a poisoned one, or a plan it refused.
    pub const NO_PICTURE: u32 = 3;
    /// A finished frame the UI never took (a newer one came first).
    pub const UNTAKEN: u32 = 4;
}

/// `FramePlan.c`.
pub mod route {
    pub const DISPLAY: u64 = 0;
    /// The slow-chain device, as bytes.
    pub const SLOW: u64 = 1;
    pub const BYTES: u64 = 2;
}

/// `FrameWait.aux`: what a frame waits for.
pub mod wait {
    /// A layer's decoded frame is not in the cache: `a` key, `b` source, `c` index.
    pub const DECODE: u32 = 1;
    /// A stream stage lacks history: `a` position, `b` positions needed, `c` clip.
    pub const STREAM_SHORT: u32 = 2;
    /// Playing, a stream stage showed its latest held frame: `a` position, `b` held, `c` clip.
    pub const STREAM_HELD: u32 = 3;
    /// A stream stage found a source frame missing: `a` position, `c` clip.
    pub const STREAM_SOURCE: u32 = 4;
    /// A lock (`dur`), `a` its [`super::name`].
    pub const LOCK: u32 = 5;
    /// The device finishing the frame before (`dur`).
    pub const DEVICE: u32 = 6;
    /// A nested reel's sub-composite is incomplete: `c` clip.
    pub const NEST: u32 = 7;
    /// The shared build of the state asked for (`dur`): the compose thread waited for it.
    pub const BUILD: u32 = 8;
}

/// `FrameExec.aux` bits.
pub mod exec {
    pub const COMPLETE: u32 = 1;
    /// The executor showed a kept composite: nothing was composed.
    pub const MEMO_HIT: u32 = 2;
    /// The composite was kept for the next lap.
    pub const MEMO_STORED: u32 = 4;
    /// A stream stage showed a held frame: not the moment's own picture, not kept as it.
    pub const STREAM_HELD: u32 = 8;
    /// Composed on the slow-chain device.
    pub const SLOW: u32 = 16;
}

/// `FrameShow.aux` bits.
pub mod show {
    pub const COMPLETE: u32 = 1;
    pub const NEW: u32 = 2;
    pub const ON_DISPLAY: u32 = 4;
}

/// `CacheEvict.aux` bits.
pub mod evict {
    /// Dropped before anybody read it.
    pub const UNREAD: u32 = 1;
    /// …while the latest look-ahead still wanted it (thrash).
    pub const WANTED: u32 = 2;
    pub const AUDIO: u32 = 4;
}

/// `StreamRun.aux` / `StreamAdvance.aux`.
pub mod stream {
    pub const RENDERED: u32 = 1;
    pub const HELD: u32 = 2;
    pub const SHORT: u32 = 3;
    pub const SOURCE_MISSING: u32 = 4;
    pub const DONE: u32 = 5;
    pub const CANCELLED: u32 = 6;
}

/// `Play.aux`.
pub mod play {
    pub const PLAY: u32 = 1;
    pub const PAUSE: u32 = 2;
    pub const SEEK: u32 = 3;
    pub const WRAP: u32 = 4;
    /// `b`..`c` the region (ticks), both 0 for none.
    pub const REGION: u32 = 5;
    pub const STALL: u32 = 6;
    /// `b` frames of underrun.
    pub const UNDERRUN: u32 = 7;
    pub const RESET: u32 = 8;
    pub const DIRECTION: u32 = 9;
    /// `c` repeats of the record before.
    pub const REPEATS: u32 = 10;
    /// `b` the rate ×1000 (signed, as i64).
    pub const RATE: u32 = 11;
    /// A start holds before its first frame: `b` 1 — the renderer is being built, 2 — its sound
    /// is not cached yet.
    pub const START_HOLD: u32 = 12;
}

/// `MixChunk.aux`.
pub mod mix {
    /// Handed out from the kept mix.
    pub const REPLAY: u32 = 1;
    /// Rendered live and kept.
    pub const KEPT: u32 = 2;
    /// Rendered live, not kept: a starved source or a pre-roll made it unclean.
    pub const NOT_CLEAN: u32 = 3;
    /// Rendered live, not kept: the store holds this chunk's first recording.
    pub const ALREADY: u32 = 4;
    /// Rendered live outside the store (reverse, offline, no store).
    pub const OUTSIDE: u32 = 5;
}

/// `Job.aux`.
pub mod job {
    pub const PREFETCH_PASS: u32 = 1;
    pub const THUMB: u32 = 2;
    pub const REEL_PREVIEW: u32 = 3;
    pub const FX_THUMB: u32 = 4;
    pub const POSTER: u32 = 5;
    pub const ANALYSIS: u32 = 6;
    pub const EXPORT_FRAME: u32 = 7;
    pub const VIDEO_BUILD: u32 = 8;
    pub const RECHECK_SHOWN: u32 = 9;
    pub const PEAKS: u32 = 10;
    pub const EXPORT: u32 = 11;
    /// The player's shared build of a project version (`engine-build`): `id` its seq, `a` the
    /// version, `b` the events it derived, `c` the events its memo holds after it (the rest reused).
    pub const SHARED_BUILD: u32 = 12;
}

/// `Invalidate.aux`.
pub mod inval {
    /// The player's renderers were rebuilt for a project version (`a`).
    pub const RENDERERS: u32 = 1;
    /// The kept final mix was cleared.
    pub const MIX_STORE: u32 = 2;
    /// Kept moments were dropped from the bars' map (`b` how many).
    pub const SHOWN: u32 = 3;
    /// A source's content changed (`a` source).
    pub const SOURCE: u32 = 4;
    /// The stream memo was cleared.
    pub const STREAM_MEMO: u32 = 5;
}

/// Text records' kinds.
pub mod text {
    /// `id` the name's hash, the text the name.
    pub const NAME: u16 = 1;
    /// `id` the thread's number, the text its name.
    pub const THREAD: u16 = 2;
    /// `key=value` about the machine.
    pub const MACHINE: u16 = 3;
    /// The run: program, version, pid.
    pub const RUN: u16 = 4;
    /// A journal line (`id` the journal's kind code), as the person reads it.
    pub const JOURNAL: u16 = 5;
}

/// One record. The meaning of `id`, `cause`, `a`, `b`, `c` and `aux` is the event's ([`Ev`]).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rec {
    /// Nanoseconds since the process started (a span: its start).
    pub t: u64,
    /// A span's length, nanoseconds.
    pub dur: u64,
    pub id: u64,
    pub cause: u64,
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub ev: u16,
    /// The thread's number in this run (its name is a [`text::THREAD`] record).
    pub th: u16,
    pub aux: u32,
}

const REC_BYTES: usize = 64;

impl Rec {
    pub fn kind(&self) -> Option<Ev> {
        Ev::from_u16(self.ev)
    }

    pub fn end(&self) -> u64 {
        self.t + self.dur
    }

    fn to_bytes(self) -> [u8; REC_BYTES] {
        let mut o = [0u8; REC_BYTES];
        for (i, v) in [self.t, self.dur, self.id, self.cause, self.a, self.b, self.c].into_iter().enumerate() {
            o[i * 8..i * 8 + 8].copy_from_slice(&v.to_le_bytes());
        }
        o[56..58].copy_from_slice(&self.ev.to_le_bytes());
        o[58..60].copy_from_slice(&self.th.to_le_bytes());
        o[60..64].copy_from_slice(&self.aux.to_le_bytes());
        o
    }

    fn from_bytes(b: &[u8]) -> Rec {
        let u = |i: usize| u64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap());
        Rec {
            t: u(0),
            dur: u(1),
            id: u(2),
            cause: u(3),
            a: u(4),
            b: u(5),
            c: u(6),
            ev: u16::from_le_bytes([b[56], b[57]]),
            th: u16::from_le_bytes([b[58], b[59]]),
            aux: u32::from_le_bytes([b[60], b[61], b[62], b[63]]),
        }
    }
}

/// A text record (names, threads, the machine, the run, journal lines).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub t: u64,
    pub id: u64,
    pub th: u16,
    pub kind: u16,
    pub s: String,
}

// ---- the clock, threads, causes, ids -----------------------------------------------------------

struct Start {
    at: Instant,
    unix_ns: u64,
}

fn start() -> &'static Start {
    static S: OnceLock<Start> = OnceLock::new();
    S.get_or_init(|| Start {
        at: Instant::now(),
        unix_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64),
    })
}

/// The moment the process's clock starts (the journal and the trace share it). Call early.
pub fn start_instant() -> Instant {
    start().at
}

/// Unix nanoseconds at [`start_instant`].
pub fn start_unix_ns() -> u64 {
    start().unix_ns
}

/// Nanoseconds since [`start_instant`].
#[inline]
pub fn now_ns() -> u64 {
    start().at.elapsed().as_nanos() as u64
}

static NEXT_TH: AtomicU16 = AtomicU16::new(0);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static TH: Cell<u16> = const { Cell::new(0) };
    static CAUSE: Cell<u64> = const { Cell::new(0) };
}

/// This thread's number; its first call names the thread in the trace.
fn thread_no() -> u16 {
    TH.with(|t| {
        let v = t.get();
        if v != 0 {
            return v;
        }
        let v = NEXT_TH.fetch_add(1, Ordering::Relaxed).wrapping_add(1).max(1);
        t.set(v);
        let cur = std::thread::current();
        let name = cur.name().map(str::to_string).unwrap_or_else(|| format!("{:?}", cur.id()));
        push_text(Text { t: now_ns(), id: v as u64, th: v, kind: text::THREAD, s: name });
        v
    })
}

/// A new id for a frame, a job, a span others refer to. Never 0, never with the top bit (keys).
pub fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// The cause this thread's records carry now.
pub fn cause() -> u64 {
    CAUSE.with(|c| c.get())
}

/// Records made on this thread while the guard lives carry `id` as their cause.
#[must_use]
pub fn with_cause(id: u64) -> CauseGuard {
    let prev = CAUSE.with(|c| c.replace(id));
    CauseGuard(prev)
}

pub struct CauseGuard(u64);

impl Drop for CauseGuard {
    fn drop(&mut self) {
        CAUSE.with(|c| c.set(self.0));
    }
}

fn mix64(mut h: u64, v: u64) -> u64 {
    h ^= v.wrapping_add(0x9e37_79b9_7f4a_7c15).wrapping_add(h << 6).wrapping_add(h >> 2);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce5_e4b9);
    (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb)
}

/// The id of a cache key (the decode request, the decoded entry, the eviction and the frame waiting
/// for it name the same number). Audio: size `(0, 0)`. The top bit is set: never a [`next_id`].
pub fn key_id(source: u64, w: u32, h: u32, idx: i64) -> u64 {
    let mut x = mix64(0x006b_746b_6579, source);
    x = mix64(x, size(w, h));
    mix64(x, idx as u64) | (1 << 63)
}

/// A frame size packed into one number (`w << 32 | h`).
pub fn size(w: u32, h: u32) -> u64 {
    ((w as u64) << 32) | h as u64
}

// ---- names ---------------------------------------------------------------------------------------

static NAMES: [AtomicU32; 4096] = [const { AtomicU32::new(0) }; 4096];

fn fnv32(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h.max(1)
}

/// The number of a name; the first use writes the string into the trace. Lock-free; costs a hash.
pub fn name(s: &str) -> u32 {
    let h = fnv32(s);
    if !on() {
        return h;
    }
    let mask = NAMES.len() - 1;
    let mut i = h as usize & mask;
    for _ in 0..NAMES.len() {
        match NAMES[i].load(Ordering::Relaxed) {
            v if v == h => return h,
            0 => match NAMES[i].compare_exchange(0, h, Ordering::AcqRel, Ordering::Relaxed) {
                Ok(_) => {
                    push_text(Text { t: now_ns(), id: h as u64, th: 0, kind: text::NAME, s: s.to_string() });
                    return h;
                }
                Err(v) if v == h => return h,
                Err(_) => {}
            },
            _ => {}
        }
        i = (i + 1) & mask;
    }
    h
}

// ---- the ring ------------------------------------------------------------------------------------

struct Slot {
    /// `2·turn` — free for the producer of that turn; `2·turn + 1` — holds its record.
    seq: AtomicU64,
    rec: UnsafeCell<Rec>,
}

struct Ring([Slot; RING]);

// SAFETY: a slot's record is written only by the producer that won its position (the CAS on HEAD
// while `seq` said free) and read only by the one writer after `seq` said full (Acquire/Release).
unsafe impl Sync for Ring {}

static SLOTS: Ring = Ring([const { Slot { seq: AtomicU64::new(0), rec: UnsafeCell::new(ZERO) } }; RING]);
const ZERO: Rec = Rec { t: 0, dur: 0, id: 0, cause: 0, a: 0, b: 0, c: 0, ev: 0, th: 0, aux: 0 };
static HEAD: AtomicU64 = AtomicU64::new(0);
static TAIL: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static ON: AtomicBool = AtomicBool::new(false);

/// Is the trace recording? One relaxed load.
#[inline]
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Store one record. Never blocks; a full ring drops it and counts it.
pub fn push(r: Rec) {
    if !on() {
        return;
    }
    let mut pos = HEAD.load(Ordering::Relaxed);
    loop {
        let slot = &SLOTS.0[pos as usize & (RING - 1)];
        let turn = pos / RING as u64;
        let seq = slot.seq.load(Ordering::Acquire);
        if seq == 2 * turn {
            match HEAD.compare_exchange_weak(pos, pos + 1, Ordering::AcqRel, Ordering::Relaxed) {
                Ok(_) => {
                    // SAFETY: this producer owns the slot until it publishes `seq` below
                    unsafe { *slot.rec.get() = r };
                    slot.seq.store(2 * turn + 1, Ordering::Release);
                    wake();
                    return;
                }
                Err(p) => pos = p,
            }
        } else if seq < 2 * turn {
            // the writer has not freed this slot from the turn before: full
            DROPPED.fetch_add(1, Ordering::Relaxed);
            wake();
            return;
        } else {
            pos = HEAD.load(Ordering::Relaxed);
        }
    }
}

/// Take what the ring holds, in order, up to `max`.
fn drain_ring(out: &mut Vec<Rec>, max: usize) -> usize {
    let mut n = 0;
    let mut pos = TAIL.load(Ordering::Relaxed);
    while n < max {
        let slot = &SLOTS.0[pos as usize & (RING - 1)];
        let turn = pos / RING as u64;
        if slot.seq.load(Ordering::Acquire) != 2 * turn + 1 {
            break;
        }
        // SAFETY: published by its producer; nobody writes it until `seq` says free again
        out.push(unsafe { *slot.rec.get() });
        slot.seq.store(2 * turn + 2, Ordering::Release);
        pos += 1;
        n += 1;
    }
    TAIL.store(pos, Ordering::Relaxed);
    n
}

fn ring_nonempty() -> bool {
    let pos = TAIL.load(Ordering::Relaxed);
    let slot = &SLOTS.0[pos as usize & (RING - 1)];
    slot.seq.load(Ordering::Acquire) == 2 * (pos / RING as u64) + 1
}

static TEXTS: Mutex<Vec<Text>> = Mutex::new(Vec::new());
static TEXTS_PUSHED: AtomicU64 = AtomicU64::new(0);
static TEXTS_DONE: AtomicU64 = AtomicU64::new(0);

/// A text record: never from the audio render thread's loop (it locks and allocates).
pub fn push_text(t: Text) {
    if !on() {
        return;
    }
    TEXTS.lock().unwrap_or_else(|p| p.into_inner()).push(t);
    TEXTS_PUSHED.fetch_add(1, Ordering::Release);
    wake();
}

/// A line of the text journal, mirrored on the same clock.
pub fn journal(code: u64, line: &str) {
    journal_at(now_ns(), code, line);
}

/// A journal line of an event at `t` ns (a run of repeats closed later, a long frame).
pub fn journal_at(t: u64, code: u64, line: &str) {
    if on() {
        push_text(Text { t, id: code, th: thread_no(), kind: text::JOURNAL, s: line.to_string() });
    }
}

/// A fact about the machine (`key=value`); repeated at the head of every segment.
pub fn machine(key: &str, value: &str) {
    if on() {
        push_text(Text { t: now_ns(), id: 0, th: 0, kind: text::MACHINE, s: format!("{key}={value}") });
    }
}

/// What every run knows of its machine without a device: the processor, cores, memory, the OS.
pub fn machine_basics() {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let cpu = read("/proc/cpuinfo");
    if let Some(m) = cpu.lines().find(|l| l.starts_with("model name")).and_then(|l| l.split_once(':')) {
        machine("cpu", m.1.trim());
    }
    if let Ok(n) = std::thread::available_parallelism() {
        machine("cores", &n.get().to_string());
    }
    let mem = read("/proc/meminfo");
    if let Some(m) = mem.lines().find(|l| l.starts_with("MemTotal")).and_then(|l| l.split_once(':')) {
        machine("memory", m.1.trim());
    }
    let os = read("/etc/os-release");
    if let Some(m) = os.lines().find(|l| l.starts_with("PRETTY_NAME=")) {
        machine("os", m.trim_start_matches("PRETTY_NAME=").trim_matches('"'));
    } else {
        machine("os", std::env::consts::OS);
    }
    let kernel = read("/proc/sys/kernel/osrelease");
    if !kernel.trim().is_empty() {
        machine("kernel", kernel.trim());
    }
    machine("arch", std::env::consts::ARCH);
}

// ---- building records ----------------------------------------------------------------------------

/// A record under construction, stamped now with this thread and its cause. Nothing when off.
#[must_use]
pub struct E(Rec);

/// An instant event.
#[inline]
pub fn ev(e: Ev) -> E {
    if !on() {
        return E(ZERO);
    }
    E(Rec { t: now_ns(), ev: e as u16, th: thread_no(), cause: cause(), ..ZERO })
}

impl E {
    pub fn id(mut self, v: u64) -> E {
        self.0.id = v;
        self
    }
    pub fn cause(mut self, v: u64) -> E {
        self.0.cause = v;
        self
    }
    pub fn a(mut self, v: u64) -> E {
        self.0.a = v;
        self
    }
    pub fn b(mut self, v: u64) -> E {
        self.0.b = v;
        self
    }
    pub fn c(mut self, v: u64) -> E {
        self.0.c = v;
        self
    }
    pub fn aux(mut self, v: u32) -> E {
        self.0.aux = v;
        self
    }
    /// The event happened at `t` ns, not now.
    pub fn at(mut self, t: u64) -> E {
        self.0.t = t;
        self
    }
    /// A span that ended now and lasted `ns`.
    pub fn lasted(mut self, ns: u64) -> E {
        self.0.t = self.0.t.saturating_sub(ns);
        self.0.dur = ns;
        self
    }
    pub fn emit(self) {
        if self.0.ev != 0 {
            push(self.0);
        }
    }
}

/// A span: stamped at its start, recorded when dropped with its length.
#[must_use]
pub struct Span(Rec);

#[inline]
pub fn span(e: Ev) -> Span {
    Span(ev(e).0)
}

impl Span {
    pub fn id(&mut self, v: u64) -> &mut Span {
        self.0.id = v;
        self
    }
    pub fn a(&mut self, v: u64) -> &mut Span {
        self.0.a = v;
        self
    }
    pub fn b(&mut self, v: u64) -> &mut Span {
        self.0.b = v;
        self
    }
    pub fn c(&mut self, v: u64) -> &mut Span {
        self.0.c = v;
        self
    }
    pub fn aux(&mut self, v: u32) -> &mut Span {
        self.0.aux = v;
        self
    }
    pub fn cause(&mut self, v: u64) -> &mut Span {
        self.0.cause = v;
        self
    }
    /// Do not record it.
    pub fn cancel(mut self) {
        self.0.ev = 0;
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if self.0.ev != 0 {
            self.0.dur = now_ns().saturating_sub(self.0.t);
            push(self.0);
        }
    }
}

// ---- the writer ----------------------------------------------------------------------------------

/// Where and how much.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// The directory of the ring files; `None` — memory only (a test's capture).
    pub dir: Option<PathBuf>,
    pub segment_bytes: u64,
    /// All segments of all runs in the directory together.
    pub budget_bytes: u64,
    /// The program and its version, for the run record.
    pub label: String,
}

impl Config {
    pub fn new(dir: Option<PathBuf>, label: &str) -> Config {
        Config { dir, segment_bytes: 8 << 20, budget_bytes: 128 << 20, label: label.to_string() }
    }
}

static CONFIG: Mutex<Option<Config>> = Mutex::new(None);
static WRITER: OnceLock<std::thread::Thread> = OnceLock::new();
static PARKED: AtomicBool = AtomicBool::new(false);
static DRAINED: AtomicU64 = AtomicU64::new(0);
static WRITER_TID: AtomicU64 = AtomicU64::new(0);
static PAUSED: AtomicBool = AtomicBool::new(false);
static CAPTURE: Mutex<Option<Vec<Item>>> = Mutex::new(None);
static RUN_ID: AtomicU64 = AtomicU64::new(0);

/// One thing read back from a trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Rec(Rec),
    Text(Text),
}

#[inline]
fn wake() {
    if PARKED.load(Ordering::Relaxed) && PARKED.swap(false, Ordering::AcqRel)
        && let Some(w) = WRITER.get()
    {
        w.unpark();
    }
}

/// Start recording into `cfg.dir` (the writer thread starts once; a later call changes where the
/// next segment goes). Writes the run record and the machine's basics.
pub fn install(cfg: Config) {
    let label = cfg.label.clone();
    *CONFIG.lock().unwrap_or_else(|p| p.into_inner()) = Some(cfg);
    RUN_ID.store(mix64(start_unix_ns(), std::process::id() as u64), Ordering::Relaxed);
    if WRITER.get().is_none() {
        let spawned = std::thread::Builder::new().name("trace-writer".into()).spawn(writer);
        match spawned {
            Ok(h) => {
                let _ = WRITER.set(h.thread().clone());
            }
            Err(_) => return,
        }
    }
    ON.store(true, Ordering::Release);
    push_text(Text {
        t: now_ns(),
        id: RUN_ID.load(Ordering::Relaxed),
        th: 0,
        kind: text::RUN,
        s: format!("{label} pid={}", std::process::id()),
    });
    machine_basics();
}

/// Stop recording (tests): records made after this are not stored.
pub fn uninstall() {
    flush();
    ON.store(false, Ordering::Release);
}

/// How long [`flush`] waits at most: a panic hook must not hold a dying process longer.
pub const FLUSH_LIMIT: Duration = Duration::from_secs(2);

/// Wait until everything recorded so far is in the file (a panic hook, a test), at most
/// [`FLUSH_LIMIT`].
pub fn flush() {
    if WRITER.get().is_none() {
        return;
    }
    let target = (HEAD.load(Ordering::Acquire), TEXTS_PUSHED.load(Ordering::Acquire));
    let until = Instant::now() + FLUSH_LIMIT;
    while Instant::now() < until {
        if TAIL.load(Ordering::Acquire) >= target.0.min(HEAD.load(Ordering::Acquire))
            && TEXTS_DONE.load(Ordering::Acquire) >= target.1
            && DRAINED.load(Ordering::Acquire) >= target.0
        {
            return;
        }
        if let Some(w) = WRITER.get() {
            PARKED.store(false, Ordering::Release);
            w.unpark();
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The writer thread's kernel id (Linux), for the at-rest test.
pub fn writer_tid() -> u64 {
    WRITER_TID.load(Ordering::Acquire)
}

/// Records lost to a full ring so far.
pub fn dropped() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

struct Seg {
    file: std::fs::File,
    bytes: u64,
}

struct Writer {
    seg: Option<Seg>,
    dir: Option<PathBuf>,
    seq: u32,
    /// Names, threads, machine and run records: repeated at each segment's head.
    heads: Vec<Text>,
    lost_said: u64,
}

fn writer() {
    if let Ok(p) = std::fs::read_link("/proc/thread-self")
        && let Some(tid) = p.file_name().and_then(|n| n.to_str()).and_then(|n| n.parse().ok())
    {
        WRITER_TID.store(tid, Ordering::Release);
    }
    let mut w = Writer { seg: None, dir: None, seq: 0, heads: Vec::new(), lost_said: 0 };
    let mut recs = Vec::with_capacity(4096);
    let mut buf: Vec<u8> = Vec::with_capacity(1 << 20);
    loop {
        if PAUSED.load(Ordering::Acquire) {
            std::thread::park_timeout(Duration::from_millis(5));
            continue;
        }
        let mut texts = std::mem::take(&mut *TEXTS.lock().unwrap_or_else(|p| p.into_inner()));
        recs.clear();
        let n = drain_ring(&mut recs, RING);
        let lost = DROPPED.load(Ordering::Relaxed);
        if lost != w.lost_said {
            recs.push(Rec { t: now_ns(), ev: Ev::Dropped as u16, a: lost - w.lost_said, ..ZERO });
            w.lost_said = lost;
        }
        let got = n + texts.len();
        if got > 0 || !recs.is_empty() {
            w.write(&mut texts, &recs, &mut buf);
        }
        TEXTS_DONE.fetch_add(texts.len() as u64, Ordering::AcqRel);
        DRAINED.store(TAIL.load(Ordering::Relaxed), Ordering::Release);
        if got > 0 {
            std::thread::sleep(BATCH);
            continue;
        }
        PARKED.store(true, Ordering::SeqCst);
        let pending = ring_nonempty() || !TEXTS.lock().unwrap_or_else(|p| p.into_inner()).is_empty();
        if pending {
            PARKED.store(false, Ordering::SeqCst);
            continue;
        }
        std::thread::park();
        PARKED.store(false, Ordering::SeqCst);
    }
}

impl Writer {
    fn write(&mut self, texts: &mut [Text], recs: &[Rec], buf: &mut Vec<u8>) {
        for t in texts.iter() {
            if matches!(t.kind, text::NAME | text::THREAD | text::MACHINE | text::RUN) {
                self.heads.push(t.clone());
            }
        }
        if let Ok(mut c) = CAPTURE.lock()
            && let Some(v) = c.as_mut()
        {
            v.extend(texts.iter().cloned().map(Item::Text));
            v.extend(recs.iter().copied().map(Item::Rec));
        }
        let cfg = CONFIG.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let Some(cfg) = cfg else { return };
        let Some(dir) = cfg.dir.clone() else {
            self.seg = None;
            return;
        };
        if self.dir.as_ref() != Some(&dir) || self.seg.is_none() {
            self.open(&dir, &cfg);
        }
        buf.clear();
        for t in texts.iter() {
            put_text(buf, t);
            self.roll(buf, &dir, &cfg);
        }
        for r in recs {
            buf.push(1);
            buf.extend_from_slice(&r.to_bytes());
            self.roll(buf, &dir, &cfg);
        }
        self.put(buf);
    }

    /// Past the segment's size the bytes so far go into it and a new segment opens.
    fn roll(&mut self, buf: &mut Vec<u8>, dir: &Path, cfg: &Config) {
        if self.seg.as_ref().is_some_and(|s| s.bytes + buf.len() as u64 >= cfg.segment_bytes) {
            self.put(buf);
            self.open(dir, cfg);
        }
    }

    fn put(&mut self, buf: &mut Vec<u8>) {
        if let Some(s) = self.seg.as_mut() {
            if s.file.write_all(buf).is_ok() {
                s.bytes += buf.len() as u64;
            } else {
                self.seg = None;
            }
        }
        buf.clear();
    }

    fn open(&mut self, dir: &Path, cfg: &Config) {
        self.seg = None;
        self.dir = Some(dir.to_path_buf());
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        self.seq += 1;
        let name = format!("{:013}-{}-{:05}.{EXT}", start_unix_ns() / 1_000_000, std::process::id(), self.seq);
        prune(dir, cfg.budget_bytes.saturating_sub(cfg.segment_bytes));
        let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(name)) else {
            return;
        };
        let mut head = Vec::with_capacity(4096);
        head.extend_from_slice(MAGIC);
        head.extend_from_slice(&FORMAT.to_le_bytes());
        head.extend_from_slice(&std::process::id().to_le_bytes());
        head.extend_from_slice(&RUN_ID.load(Ordering::Relaxed).to_le_bytes());
        head.extend_from_slice(&start_unix_ns().to_le_bytes());
        for t in &self.heads {
            put_text(&mut head, t);
        }
        if file.write_all(&head).is_ok() {
            self.seg = Some(Seg { file, bytes: head.len() as u64 });
        }
    }
}

fn put_text(buf: &mut Vec<u8>, t: &Text) {
    buf.push(2);
    buf.extend_from_slice(&t.t.to_le_bytes());
    buf.extend_from_slice(&t.id.to_le_bytes());
    buf.extend_from_slice(&t.th.to_le_bytes());
    buf.extend_from_slice(&t.kind.to_le_bytes());
    let s = t.s.as_bytes();
    let n = s.len().min(u16::MAX as usize * 4);
    buf.extend_from_slice(&(n as u32).to_le_bytes());
    buf.extend_from_slice(&s[..n]);
}

/// The segments of a directory, oldest first.
pub fn segments(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == EXT)).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Delete the oldest segments until the rest fit `keep` bytes.
fn prune(dir: &Path, keep: u64) {
    let segs = segments(dir);
    let sizes: Vec<u64> = segs.iter().map(|p| std::fs::metadata(p).map_or(0, |m| m.len())).collect();
    let mut total: u64 = sizes.iter().sum();
    for (p, s) in segs.iter().zip(sizes) {
        if total <= keep {
            break;
        }
        if std::fs::remove_file(p).is_ok() {
            total -= s;
        }
    }
}

// ---- reading -------------------------------------------------------------------------------------

/// One run as read back: records in time order, texts, names and threads resolved.
#[derive(Debug, Default, Clone)]
pub struct Run {
    pub id: u64,
    pub pid: u32,
    pub start_unix_ns: u64,
    pub recs: Vec<Rec>,
    pub texts: Vec<Text>,
    pub names: HashMap<u32, String>,
    pub threads: HashMap<u16, String>,
    pub machine: Vec<(String, String)>,
    pub label: String,
    pub files: Vec<PathBuf>,
}

impl Run {
    pub fn name_of(&self, h: u32) -> String {
        self.names.get(&h).cloned().unwrap_or_else(|| format!("#{h:08x}"))
    }
    pub fn thread_of(&self, th: u16) -> String {
        self.threads.get(&th).cloned().unwrap_or_else(|| format!("thread {th}"))
    }
}

/// Read one segment: `(run id, pid, start, items)`.
pub fn read_segment(path: &Path) -> std::io::Result<(u64, u32, u64, Vec<Item>)> {
    let mut data = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut data)?;
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}: not a trace", path.display()));
    if data.len() < 32 || &data[..8] != MAGIC {
        return Err(bad());
    }
    let pid = u32::from_le_bytes(data[12..16].try_into().unwrap());
    let run = u64::from_le_bytes(data[16..24].try_into().unwrap());
    let start = u64::from_le_bytes(data[24..32].try_into().unwrap());
    let mut items = Vec::new();
    let mut i = 32;
    // a killed writer may leave a torn last item: stop there
    while i < data.len() {
        match data[i] {
            1 if i + 1 + REC_BYTES <= data.len() => {
                items.push(Item::Rec(Rec::from_bytes(&data[i + 1..i + 1 + REC_BYTES])));
                i += 1 + REC_BYTES;
            }
            2 if i + 25 <= data.len() => {
                let u = |o: usize| u64::from_le_bytes(data[i + o..i + o + 8].try_into().unwrap());
                let th = u16::from_le_bytes([data[i + 17], data[i + 18]]);
                let kind = u16::from_le_bytes([data[i + 19], data[i + 20]]);
                let n = u32::from_le_bytes(data[i + 21..i + 25].try_into().unwrap()) as usize;
                if i + 25 + n > data.len() {
                    break;
                }
                let s = String::from_utf8_lossy(&data[i + 25..i + 25 + n]).into_owned();
                items.push(Item::Text(Text { t: u(1), id: u(9), th, kind, s }));
                i += 25 + n;
            }
            _ => break,
        }
    }
    Ok((run, pid, start, items))
}

/// Read a trace: a segment file, or a directory of them. Runs oldest first.
pub fn load(path: &Path) -> std::io::Result<Vec<Run>> {
    let files = if path.is_dir() { segments(path) } else { vec![path.to_path_buf()] };
    let mut runs: Vec<Run> = Vec::new();
    for f in files {
        let Ok((id, pid, start, items)) = read_segment(&f) else { continue };
        let r = match runs.iter_mut().position(|r| r.id == id) {
            Some(i) => &mut runs[i],
            None => {
                runs.push(Run { id, pid, start_unix_ns: start, ..Default::default() });
                runs.last_mut().unwrap()
            }
        };
        r.files.push(f.clone());
        for it in items {
            match it {
                Item::Rec(x) => r.recs.push(x),
                Item::Text(t) => match t.kind {
                    text::NAME => {
                        r.names.insert(t.id as u32, t.s);
                    }
                    text::THREAD => {
                        r.threads.insert(t.id as u16, t.s);
                    }
                    text::MACHINE => {
                        if let Some((k, v)) = t.s.split_once('=')
                            && !r.machine.iter().any(|(a, b)| a == k && b == v)
                        {
                            r.machine.push((k.to_string(), v.to_string()));
                        }
                    }
                    text::RUN => r.label = t.s,
                    _ => r.texts.push(t),
                },
            }
        }
    }
    for r in &mut runs {
        r.recs.sort_by_key(|x| (x.t, x.ev));
        r.texts.sort_by_key(|x| x.t);
    }
    runs.sort_by_key(|r| r.start_unix_ns);
    Ok(runs)
}

/// Tests: what the writer drained while a capture was on.
pub mod testing {
    use super::*;

    static ONE: Mutex<()> = Mutex::new(());

    /// Record into memory for the closure's length (one capture at a time in a process); the items
    /// other threads recorded meanwhile come too — select by id.
    pub fn capture(f: impl FnOnce()) -> Vec<Item> {
        let _one = ONE.lock().unwrap_or_else(|p| p.into_inner());
        let was = CONFIG.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if !on() {
            install(Config::new(None, "test"));
        }
        flush();
        *CAPTURE.lock().unwrap() = Some(Vec::new());
        f();
        flush();
        let got = CAPTURE.lock().unwrap().take().unwrap_or_default();
        if was.is_none() {
            ON.store(false, Ordering::Release);
        }
        got
    }

    /// The records of a capture.
    pub fn recs(items: &[Item]) -> Vec<Rec> {
        items.iter().filter_map(|i| if let Item::Rec(r) = i { Some(*r) } else { None }).collect()
    }

    /// Hold the writer (a full ring) while the guard lives.
    pub fn pause_writer() -> PauseGuard {
        PAUSED.store(true, Ordering::Release);
        PauseGuard
    }

    pub struct PauseGuard;

    impl Drop for PauseGuard {
        fn drop(&mut self) {
            PAUSED.store(false, Ordering::Release);
            if let Some(w) = WRITER.get() {
                w.unpark();
            }
        }
    }

    /// The process's one test lock for tests that change the trace's config.
    pub fn lock() -> std::sync::MutexGuard<'static, ()> {
        ONE.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests;
