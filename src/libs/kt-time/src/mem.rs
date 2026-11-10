//! The memory ledger (memory-ledger, capability `media-index`): one budget across every cache that
//! holds pictures, mixes or device buffers.
//!
//! - A keeper opens an [`Account`] of its [`Tenant`] kind with its BASE — the working set it was
//!   proven with — and reports the bytes it holds with [`Account::set`] (relaxed atomics: no lock, no
//!   allocation, the audio render thread may report). The base is never cut.
//! - What a kind wants ABOVE its bases ([`want`]) is granted from ONE growth budget: half of what the
//!   process could still take beyond its bases — the memory available now (the host's probe, a
//!   control group's limit included) plus the growth the kinds already hold — in the order of value
//!   of [`Tenant`]: kept composites, then stream history, then decoded frames. Unknown memory grants
//!   nothing; a live [`Claim`] (an export) grants nothing either.
//! - A kind whose grant shrinks is told at once through its accounts' hooks ([`Account::on_shrink`]),
//!   after the ledger's lock is released and never for the kind that asked (its keeper asks under its
//!   own lock, and its hook would take it again).
//! - Nothing here runs on a clock: a sizing happens when a want or a claim changes, and writes its
//!   `mem_budget`/`mem_grant` trace records only when something changed.
//! - No cap changes a byte of a result (invariant 11): the keepers decide only what they keep.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// The kinds of keeper. The declaration order of the growing kinds IS the order of value: the
/// earlier is granted first and taken back last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum Tenant {
    /// Kept composites (the present memo): a hit costs one display pass.
    Kept = 0,
    /// Stream history (the stream memo): a hit saves the codec, the rest of the chain still runs.
    History = 1,
    /// Decoded frames (the burst cache): a hit saves a decode.
    Frames = 2,
    /// The kept final mix.
    Mix = 3,
    /// Uploaded source frames on a device.
    Upload = 4,
    /// Read-back composites (the readback memo).
    Readback = 5,
    /// Free transient textures of a device.
    Transient = 6,
    /// The compute host's free buffers.
    Compute = 7,
}

const KINDS: usize = 8;
/// The kinds that may grow above their bases, in the order of value.
pub const GROWING: [Tenant; 3] = [Tenant::Kept, Tenant::History, Tenant::Frames];

impl Tenant {
    pub const ALL: [Tenant; KINDS] = [
        Tenant::Kept,
        Tenant::History,
        Tenant::Frames,
        Tenant::Mix,
        Tenant::Upload,
        Tenant::Readback,
        Tenant::Transient,
        Tenant::Compute,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tenant::Kept => "kept",
            Tenant::History => "history",
            Tenant::Frames => "frames",
            Tenant::Mix => "mix",
            Tenant::Upload => "upload",
            Tenant::Readback => "readback",
            Tenant::Transient => "transient",
            Tenant::Compute => "compute",
        }
    }

    pub fn from_u32(v: u32) -> Option<Tenant> {
        Tenant::ALL.get(v as usize).copied()
    }

    fn grows(self) -> bool {
        GROWING.contains(&self)
    }
}

type Hook = Arc<dyn Fn(u64) + Send + Sync>;
type Probe = Box<dyn Fn() -> Option<u64> + Send + Sync>;

#[derive(Default)]
struct Kind {
    used: AtomicU64,
    base: AtomicU64,
    grant: AtomicU64,
}

#[derive(Default)]
struct State {
    wants: [u64; KINDS],
    claims: u32,
    hooks: Vec<(u64, Tenant, Hook)>,
    /// What the last sizing decided: (budget, granted, available MB, claims).
    last: Option<(u64, u64, u64, u32)>,
}

/// A ledger. The process has one ([`ledger`]); tests make their own.
pub struct Ledger {
    kinds: [Kind; KINDS],
    state: Mutex<State>,
    probe: Mutex<Option<Probe>>,
    next_id: AtomicU64,
    /// What the last [`Ledger::sample`] wrote, in MB: each kind's bytes, then the process's resident
    /// memory and the kinds' sum.
    sampled: Mutex<Option<[u64; KINDS + 2]>>,
}

/// What [`snapshot`] reads: per kind (bytes held, bases, grant), the growth budget of the last sizing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub used: [u64; KINDS],
    pub base: [u64; KINDS],
    pub grant: [u64; KINDS],
    pub budget: u64,
}

impl Snapshot {
    pub fn used(&self, t: Tenant) -> u64 {
        self.used[t as usize]
    }
    pub fn base(&self, t: Tenant) -> u64 {
        self.base[t as usize]
    }
    pub fn grant(&self, t: Tenant) -> u64 {
        self.grant[t as usize]
    }
}

/// The growth budget for `available` bytes free and `held` bytes of growth already held, with
/// `claims` live claims: half of what the process could still take beyond its bases.
pub fn growth_budget(available: Option<u64>, held: u64, claims: u32) -> u64 {
    match available {
        Some(a) if claims == 0 => a.saturating_add(held) / 2,
        _ => 0,
    }
}

/// The grants of the growing kinds (in [`GROWING`]'s order) for their wants within `budget`.
pub fn grants(wants: [u64; 3], budget: u64) -> [u64; 3] {
    let mut left = budget;
    wants.map(|w| {
        let g = w.min(left);
        left -= g;
        g
    })
}

impl Default for Ledger {
    fn default() -> Self {
        Ledger::new()
    }
}

impl Ledger {
    pub fn new() -> Ledger {
        Ledger {
            kinds: Default::default(),
            state: Mutex::new(State::default()),
            probe: Mutex::new(None),
            next_id: AtomicU64::new(1),
            sampled: Mutex::new(None),
        }
    }

    /// One busy second's memory (frames-cache-growth): a `mem_sample` record when the process's
    /// `resident` bytes or the kinds' sum moved by a MB since the last sample, a `mem_used` record for
    /// each kind whose bytes moved by a MB. The caller samples once per busy second and never at
    /// rest; an unchanged second writes nothing. The records written.
    pub fn sample(&self, resident: Option<u64>) -> usize {
        let snap = self.snapshot();
        let sum: u64 = snap.used.iter().sum();
        let mut now = [0u64; KINDS + 2];
        for (i, u) in snap.used.iter().enumerate() {
            now[i] = u >> 20;
        }
        now[KINDS] = resident.unwrap_or(0) >> 20;
        now[KINDS + 1] = sum >> 20;
        let mut last = self.sampled.lock().unwrap_or_else(|p| p.into_inner());
        let was = last.unwrap_or([u64::MAX; KINDS + 2]);
        let mut wrote = 0;
        if now[KINDS..] != was[KINDS..] {
            let avail = self.available().map_or(0, |a| a >> 20);
            crate::trace::ev(crate::trace::Ev::MemSample).a(resident.unwrap_or(0)).b(sum).c(avail).emit();
            wrote += 1;
        }
        for t in Tenant::ALL {
            let i = t as usize;
            if now[i] != was[i] {
                crate::trace::ev(crate::trace::Ev::MemUsed)
                    .aux(t as u32)
                    .a(snap.used[i])
                    .b(snap.base[i] + snap.grant[i])
                    .emit();
                wrote += 1;
            }
        }
        *last = Some(now);
        wrote
    }

    /// How the ledger learns the memory the process can still take (the host injects it; `None` —
    /// unknown, and nothing above the bases is granted).
    pub fn set_probe(&self, probe: impl Fn() -> Option<u64> + Send + Sync + 'static) {
        *self.probe.lock().unwrap_or_else(|p| p.into_inner()) = Some(Box::new(probe));
    }

    /// The memory available now, by the probe.
    pub fn available(&self) -> Option<u64> {
        self.probe.lock().unwrap_or_else(|p| p.into_inner()).as_ref().and_then(|p| p())
    }

    /// The grant of `t` above its bases now.
    pub fn grant(&self, t: Tenant) -> u64 {
        self.kinds[t as usize].grant.load(Ordering::Relaxed)
    }

    /// Ask for `extra` bytes of `t` above its bases (0 — none) and size the ledger; the grant of `t`.
    /// An unchanged want sizes nothing.
    pub fn want(&self, t: Tenant, extra: u64) -> u64 {
        if !t.grows() {
            return 0;
        }
        {
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if s.wants[t as usize] == extra && s.last.is_some() {
                return self.grant(t);
            }
            s.wants[t as usize] = extra;
        }
        self.size(Some(t));
        self.grant(t)
    }

    /// The want of `t` above its bases.
    pub fn wanted(&self, t: Tenant) -> u64 {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).wants[t as usize]
    }

    /// Size the growth budget by the memory available now and tell the kinds whose grant shrank
    /// (except `asker`).
    pub fn size(&self, asker: Option<Tenant>) {
        let available = self.available();
        let held: u64 = GROWING
            .iter()
            .map(|t| {
                let k = &self.kinds[*t as usize];
                k.used.load(Ordering::Relaxed).saturating_sub(k.base.load(Ordering::Relaxed))
            })
            .sum();
        let mut shrunk: Vec<(Hook, u64)> = Vec::new();
        {
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let budget = growth_budget(available, held, s.claims);
            let wants = GROWING.map(|t| s.wants[t as usize]);
            let got = grants(wants, budget);
            let granted: u64 = got.iter().sum();
            let avail_mb = available.map_or(0, |a| a >> 20);
            for (t, g) in GROWING.iter().zip(got) {
                let k = &self.kinds[*t as usize];
                let old = k.grant.swap(g, Ordering::Relaxed);
                if old != g || s.last.is_none() {
                    crate::trace::ev(crate::trace::Ev::MemGrant)
                        .aux(*t as u32)
                        .a(s.wants[*t as usize])
                        .b(k.base.load(Ordering::Relaxed) + g)
                        .c(k.used.load(Ordering::Relaxed))
                        .emit();
                }
                if g < old && Some(*t) != asker {
                    shrunk.extend(s.hooks.iter().filter(|(_, ht, _)| ht == t).map(|(_, _, h)| (h.clone(), g)));
                }
            }
            let now = (budget, granted, avail_mb, s.claims);
            if s.last != Some(now) {
                s.last = Some(now);
                crate::trace::ev(crate::trace::Ev::MemBudget).a(budget).b(granted).c(avail_mb).aux(s.claims).emit();
            }
        }
        for (h, g) in shrunk {
            h(g);
        }
    }

    /// What every kind holds now, its bases and grant.
    pub fn snapshot(&self) -> Snapshot {
        let mut out = Snapshot::default();
        for (i, k) in self.kinds.iter().enumerate() {
            out.used[i] = k.used.load(Ordering::Relaxed);
            out.base[i] = k.base.load(Ordering::Relaxed);
            out.grant[i] = k.grant.load(Ordering::Relaxed);
        }
        out.budget = self.state.lock().unwrap_or_else(|p| p.into_inner()).last.map_or(0, |l| l.0);
        out
    }

    /// Open an account of `t` with its `base`.
    pub fn open(&'static self, t: Tenant, base: u64) -> Account {
        self.kinds[t as usize].base.fetch_add(base, Ordering::Relaxed);
        Account { ledger: self, tenant: t, base, held: AtomicU64::new(0), id: self.next_id.fetch_add(1, Ordering::Relaxed) }
    }

    /// Take the growth while the claim lives (an export).
    pub fn claim(&'static self) -> Claim {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).claims += 1;
        self.size(None);
        Claim { ledger: self }
    }
}

/// A keeper's account: its base, the bytes it holds.
pub struct Account {
    ledger: &'static Ledger,
    tenant: Tenant,
    base: u64,
    held: AtomicU64,
    id: u64,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account").field("tenant", &self.tenant).field("base", &self.base).field("held", &self.held()).finish()
    }
}

impl Account {
    /// The keeper holds `bytes` now.
    pub fn set(&self, bytes: u64) {
        let old = self.held.swap(bytes, Ordering::Relaxed);
        let k = &self.ledger.kinds[self.tenant as usize];
        if bytes >= old {
            k.used.fetch_add(bytes - old, Ordering::Relaxed);
        } else {
            k.used.fetch_sub(old - bytes, Ordering::Relaxed);
        }
    }

    pub fn held(&self) -> u64 {
        self.held.load(Ordering::Relaxed)
    }

    pub fn base(&self) -> u64 {
        self.base
    }

    pub fn tenant(&self) -> Tenant {
        self.tenant
    }

    /// This keeper's bound: its base plus its kind's grant.
    pub fn cap(&self) -> u64 {
        self.base + self.ledger.grant(self.tenant)
    }

    /// Ask for `extra` bytes above the bases of this kind; the grant.
    pub fn want(&self, extra: u64) -> u64 {
        self.ledger.want(self.tenant, extra)
    }

    /// The want of this kind above its bases now (whoever of its keepers asked it).
    pub fn wanted(&self) -> u64 {
        self.ledger.wanted(self.tenant)
    }

    /// Call `hook` with the kind's new grant when a sizing shrinks it (not when this kind asked).
    pub fn on_shrink(&self, hook: impl Fn(u64) + Send + Sync + 'static) {
        let mut s = self.ledger.state.lock().unwrap_or_else(|p| p.into_inner());
        s.hooks.push((self.id, self.tenant, Arc::new(hook)));
    }
}

impl Drop for Account {
    fn drop(&mut self) {
        self.set(0);
        self.ledger.kinds[self.tenant as usize].base.fetch_sub(self.base, Ordering::Relaxed);
        let mut s = self.ledger.state.lock().unwrap_or_else(|p| p.into_inner());
        s.hooks.retain(|(id, _, _)| *id != self.id);
    }
}

/// While it lives the ledger grants nothing above the bases (an export runs).
pub struct Claim {
    ledger: &'static Ledger,
}

impl Drop for Claim {
    fn drop(&mut self) {
        {
            let mut s = self.ledger.state.lock().unwrap_or_else(|p| p.into_inner());
            s.claims = s.claims.saturating_sub(1);
        }
        self.ledger.size(None);
    }
}

/// The process's ledger.
pub fn ledger() -> &'static Ledger {
    static L: OnceLock<Ledger> = OnceLock::new();
    L.get_or_init(Ledger::new)
}

/// Open an account in the process's ledger.
pub fn open(t: Tenant, base: u64) -> Account {
    ledger().open(t, base)
}

/// The process's probe of the memory available (the host injects `kt_gpu::memory::available`).
pub fn set_probe(probe: fn() -> Option<u64>) {
    ledger().set_probe(probe);
}

/// The memory available now, by the process's probe.
pub fn available() -> Option<u64> {
    ledger().available()
}

static RESIDENT: OnceLock<fn() -> Option<u64>> = OnceLock::new();

/// How the process learns its own resident memory (the host injects `kt_gpu::memory::resident`).
pub fn set_resident_probe(probe: fn() -> Option<u64>) {
    let _ = RESIDENT.set(probe);
}

/// The process's resident memory now, by the host's probe; `None` — unknown.
pub fn resident() -> Option<u64> {
    RESIDENT.get().and_then(|p| p())
}

/// One busy second's memory records of the process's ledger ([`Ledger::sample`]).
pub fn sample() -> usize {
    ledger().sample(resident())
}

/// The process's ledger now.
pub fn snapshot() -> Snapshot {
    ledger().snapshot()
}

/// Take the process's growth while the claim lives.
pub fn claim() -> Claim {
    ledger().claim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    const MB: u64 = 1 << 20;
    /// The two tests read the trace: they run one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn test_ledger(available: Option<u64>) -> &'static Ledger {
        let l: &'static Ledger = Box::leak(Box::new(Ledger::new()));
        l.set_probe(move || available);
        l
    }

    /// t2290: grants go in the order of value within half of what is free plus the growth held;
    /// unknown memory grants nothing; the bases are never cut.
    #[test]
    fn t2290_grants_follow_the_order_of_value_within_the_budget() {
        let _one = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        // the pure rule
        assert_eq!(growth_budget(Some(10 * MB), 0, 0), 5 * MB);
        assert_eq!(growth_budget(Some(10 * MB), 4 * MB, 0), 7 * MB, "the growth already held counts back");
        assert_eq!(growth_budget(None, 4 * MB, 0), 0, "unknown memory");
        assert_eq!(growth_budget(Some(10 * MB), 0, 1), 0, "a claim");
        assert_eq!(grants([3, 4, 5], 100), [3, 4, 5], "plenty");
        assert_eq!(grants([3, 4, 5], 5), [3, 2, 0], "part: kept first, then history, frames nothing");
        // a ledger: plenty
        let l = test_ledger(Some(64 << 30));
        let kept = l.open(Tenant::Kept, 512 * MB);
        let hist = l.open(Tenant::History, 256 * MB);
        let frames = l.open(Tenant::Frames, 512 * MB);
        let up = l.open(Tenant::Upload, 256 * MB);
        assert_eq!(hist.want(800 * MB), 800 * MB);
        assert_eq!(kept.want(600 * MB), 600 * MB);
        assert_eq!(frames.want(100 * MB), 100 * MB);
        assert_eq!(kept.cap(), 1112 * MB);
        assert_eq!(up.cap(), 256 * MB, "a base-only kind is never granted");
        assert_eq!(up.want(1 << 30), 0);
        // a ledger: memory for part of it — 1 GB free, nothing held yet: 512 MB of growth
        let l = test_ledger(Some(1 << 30));
        let kept = l.open(Tenant::Kept, 512 * MB);
        let hist = l.open(Tenant::History, 256 * MB);
        let frames = l.open(Tenant::Frames, 512 * MB);
        hist.want(400 * MB);
        frames.want(100 * MB);
        kept.want(300 * MB);
        assert_eq!([kept.cap(), hist.cap(), frames.cap()], [812 * MB, 468 * MB, 512 * MB]);
        let s = l.snapshot();
        assert_eq!(s.budget, 512 * MB);
        assert!(GROWING.iter().map(|t| s.grant(*t)).sum::<u64>() <= s.budget);
        // the growth held counts back: the memos filled what they were granted, `available` fell
        kept.set(812 * MB);
        hist.set(468 * MB);
        l.set_probe(|| Some((1 << 30) - 512 * MB));
        l.size(None);
        assert_eq!([kept.cap(), hist.cap()], [812 * MB, 468 * MB], "filling a grant does not take it back");
        // unknown memory: every cap is its base
        let l = test_ledger(None);
        let kept = l.open(Tenant::Kept, 512 * MB);
        assert_eq!(kept.want(300 * MB), 0);
        assert_eq!(kept.cap(), 512 * MB);
    }

    /// t2291: a higher want takes back the lowest grant first and tells its keeper at once, not the
    /// asker; the same wants again change nothing and write nothing.
    #[test]
    fn t2291_a_higher_want_takes_back_the_lowest_first_and_only_a_change_acts() {
        let _one = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let l = test_ledger(Some(2000 * MB)); // 1000 MB of growth
        let kept = l.open(Tenant::Kept, 512 * MB);
        let hist = l.open(Tenant::History, 256 * MB);
        let told_hist = Arc::new(AtomicU64::new(u64::MAX));
        let told_kept = Arc::new(AtomicUsize::new(0));
        {
            let t = told_hist.clone();
            hist.on_shrink(move |g| t.store(g, Ordering::Relaxed));
            let t = told_kept.clone();
            kept.on_shrink(move |_| {
                t.fetch_add(1, Ordering::Relaxed);
            });
        }
        assert_eq!(hist.want(800 * MB), 800 * MB);
        assert_eq!(kept.want(300 * MB), 300 * MB);
        assert_eq!(told_hist.load(Ordering::Relaxed), 700 * MB, "the history was told its new grant");
        assert_eq!(hist.cap(), 956 * MB);
        assert_eq!(kept.want(900 * MB), 900 * MB, "the kept composites are granted first");
        assert_eq!(told_hist.load(Ordering::Relaxed), 100 * MB);
        assert_eq!(told_kept.load(Ordering::Relaxed), 0, "the asker is never told");
        // a claim takes everything back and tells the kept composites too
        let claim = l.claim();
        assert_eq!([kept.cap(), hist.cap()], [512 * MB, 256 * MB]);
        assert_eq!(told_kept.load(Ordering::Relaxed), 1);
        drop(claim);
        assert_eq!([kept.cap(), hist.cap()], [1412 * MB, 356 * MB], "the claim gone, the grants are back");
        // the same wants again: no record, no hook
        let told = told_kept.load(Ordering::Relaxed);
        let items = crate::trace::testing::capture(|| {
            kept.want(900 * MB);
            hist.want(800 * MB);
        });
        let mine = crate::trace::testing::recs(&items)
            .into_iter()
            .filter(|r| matches!(r.kind(), Some(crate::trace::Ev::MemBudget | crate::trace::Ev::MemGrant)))
            .count();
        assert_eq!(mine, 0, "an unchanged want writes nothing");
        assert_eq!(told_kept.load(Ordering::Relaxed), told);
        // a changed one writes the budget and the grants that moved
        let items = crate::trace::testing::capture(|| {
            kept.want(950 * MB);
        });
        let recs = crate::trace::testing::recs(&items);
        let grants: Vec<_> = recs.iter().filter(|r| r.kind() == Some(crate::trace::Ev::MemGrant)).collect();
        assert!(grants.iter().any(|r| r.aux == Tenant::Kept as u32 && r.b == 1462 * MB));
        assert!(grants.iter().any(|r| r.aux == Tenant::History as u32 && r.b == 306 * MB));
        // a dropped account leaves the base and its hook
        drop(hist);
        assert_eq!(l.snapshot().base(Tenant::History), 0);
    }

    /// t2395 (frames-cache-growth): a busy second's sample writes the process's memory and each
    /// tenant's bytes only when they moved; an unchanged second writes nothing.
    #[test]
    fn t2395_a_memory_sample_writes_only_what_moved() {
        let _one = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let l = test_ledger(Some(4000 * MB));
        let frames = l.open(Tenant::Frames, 512 * MB);
        let mix = l.open(Tenant::Mix, 64 * MB);
        let count = |items: &[crate::trace::Item]| {
            let recs = crate::trace::testing::recs(items);
            let of = |e| recs.iter().filter(|r| r.kind() == Some(e)).cloned().collect::<Vec<_>>();
            (of(crate::trace::Ev::MemSample), of(crate::trace::Ev::MemUsed))
        };
        frames.set(300 * MB);
        mix.set(10 * MB);
        let items = crate::trace::testing::capture(|| {
            l.sample(Some(2000 * MB));
        });
        let (proc_, used) = count(&items);
        assert_eq!(proc_.len(), 1, "the first sample names the process");
        assert_eq!((proc_[0].a, proc_[0].b), (2000 * MB, 310 * MB));
        assert_eq!(proc_[0].c, 4000, "the memory available in MB");
        let f = used.iter().find(|r| r.aux == Tenant::Frames as u32).expect("the frames' bytes");
        assert_eq!((f.a, f.b), (300 * MB, 512 * MB), "bytes held and the cap");
        // nothing moved: nothing written (a sample at rest would be a record a second)
        let items = crate::trace::testing::capture(|| {
            assert_eq!(l.sample(Some(2000 * MB)), 0);
        });
        let (proc_, used) = count(&items);
        assert!(proc_.is_empty() && used.is_empty());
        // the frames moved: their record alone, and the process's for the sum
        frames.set(500 * MB);
        let items = crate::trace::testing::capture(|| {
            l.sample(Some(2000 * MB));
        });
        let (proc_, used) = count(&items);
        assert_eq!(proc_.len(), 1);
        assert_eq!(used.iter().map(|r| r.aux).collect::<Vec<_>>(), vec![Tenant::Frames as u32]);
    }
}
