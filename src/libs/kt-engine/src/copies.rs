//! The preview's copies of heavy sources (persistent-cache D3): the player's `source-copies` thread picks
//! each file's copy for the preview's quality, swaps it in, and makes a missing one at rest.
//!
//! - It parks with no clock while there is nothing to do: a config, the mode, the quality, a build, a
//!   source, play and pause wake it.
//! - A swap takes the PICTURE only (`MediaParts::swap_picture`): the playhead, the sound, its decoded
//!   bursts and its effects' state go on; the plan is the same, a copy answers the original's facts.
//! - A job runs at class background, on a pause (it waits while the transport plays or an export runs),
//!   with a short-lived source of its own (invariant 5); it stops when what it makes is not wanted.

use crate::player::{MediaParts, PreviewParts, Wake};
use kt_media::cache_folder::Folder;
use kt_media::copy::{self, CopyError, CopyFacts, CopyMaker, CopyMode, CopySource, Go};
use kt_model::MediaId;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A media's path as the project names it → the file; `None` — not a file.
pub type Resolve = Arc<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>;

/// What the host sets.
#[derive(Clone)]
pub struct Config {
    pub maker: Arc<dyn CopyMaker>,
    /// The project's cache folder.
    pub root: PathBuf,
    pub resolve: Resolve,
}

/// What a media plays from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Plays {
    Copy(u32, u32),
    Camera(PathBuf),
}

/// A media's copy as a card shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Card {
    /// Frames written of all.
    Making(u64, u64),
    /// The copy the preview reads.
    Copy(u32, u32),
    Camera,
    Failed(String),
}

#[derive(Default)]
struct State {
    path: PathBuf,
    /// The raise of the source this knowledge belongs to.
    raised: u64,
    /// `Some(None)` — the file cannot be read.
    facts: Option<Option<CopyFacts>>,
    /// It was measured decoding slower than real time.
    slow: bool,
    /// What `plays` and `make` were decided for: nothing is asked of the disk until it moves.
    decided: Option<Decided>,
    plays: Option<Plays>,
    /// The copy to make.
    make: Option<(u32, u32)>,
    making: Option<(u64, u64)>,
    failed: Option<String>,
}

/// The mode, whether the preview is 1/1, the decode size, whether it is slow, and the count of asks to
/// look again.
type Decided = (CopyMode, bool, (u32, u32), bool, u64);

#[derive(Default)]
pub struct Copies {
    cfg: Mutex<Option<Config>>,
    mode: Mutex<CopyMode>,
    states: Mutex<HashMap<MediaId, State>>,
    pub(crate) wake: Wake,
    /// Moves when a card may read otherwise, and who is told so.
    stamp: AtomicU64,
    notify: Mutex<Option<fn()>>,
    /// Copies written in this process.
    made: AtomicU64,
    /// Asks to look again at what lies on the disk.
    looks: AtomicU64,
    /// Nothing is left to do.
    settled: AtomicBool,
    /// No encoder opens on this machine: why.
    unavailable: Mutex<Option<String>>,
}

impl Copies {
    pub fn set_config(&self, cfg: Option<Config>) {
        *self.cfg.lock().unwrap() = cfg;
        *self.unavailable.lock().unwrap() = None;
        self.look_again();
    }

    pub fn set_mode(&self, m: CopyMode) {
        let was = std::mem::replace(&mut *self.mode.lock().unwrap(), m);
        if was != m {
            self.kick();
        }
    }

    /// Told (with no lock held) when a card may read otherwise.
    pub fn set_notify(&self, f: fn()) {
        *self.notify.lock().unwrap() = Some(f);
    }

    pub fn mode(&self) -> CopyMode {
        *self.mode.lock().unwrap()
    }

    /// Look again at what lies on the disk (a bound camera's copy, a cleared folder).
    pub fn look_again(&self) {
        self.looks.fetch_add(1, Ordering::AcqRel);
        self.states.lock().unwrap().values_mut().for_each(|s| s.failed = None);
        self.kick();
    }

    pub(crate) fn kick(&self) {
        self.settled.store(false, Ordering::Release);
        self.wake.notify();
    }

    /// What the card of a media says of its copy.
    pub fn card(&self, m: MediaId) -> Option<Card> {
        let g = self.states.lock().unwrap();
        let s = g.get(&m)?;
        match (&s.making, &s.plays, &s.failed) {
            (Some((a, b)), _, _) => Some(Card::Making(*a, *b)),
            (_, Some(Plays::Copy(w, h)), _) => Some(Card::Copy(*w, *h)),
            (_, Some(Plays::Camera(_)), _) => Some(Card::Camera),
            (_, _, Some(why)) => Some(Card::Failed(why.clone())),
            _ => None,
        }
    }

    /// Moves when a card may read otherwise.
    pub fn stamp(&self) -> u64 {
        self.stamp.load(Ordering::Relaxed)
    }

    pub fn made(&self) -> u64 {
        self.made.load(Ordering::Relaxed)
    }

    /// Nothing is left to swap or make (tests, the bench).
    pub fn settled(&self) -> bool {
        self.settled.load(Ordering::Acquire)
    }

    /// Why no copy is made on this machine (no hardware encoder), once found.
    pub fn unavailable(&self) -> Option<String> {
        self.unavailable.lock().unwrap().clone()
    }

    fn with(&self, m: MediaId, f: impl FnOnce(&mut State)) {
        if let Some(s) = self.states.lock().unwrap().get_mut(&m) {
            f(s);
        }
        self.stamp.fetch_add(1, Ordering::Relaxed);
        let notify = *self.notify.lock().unwrap();
        if let Some(f) = notify {
            f();
        }
    }
}

/// What a media should play from now, and the copy to make when there is none of that size.
struct Want {
    plays: Option<Plays>,
    make: Option<(u32, u32)>,
}

fn want(folder: &Folder, f: &CopyFacts, mode: CopyMode, full: bool, decode: (u32, u32), slow: bool) -> Want {
    let Some(n) = f.wanted(mode, full, decode) else { return Want { plays: None, make: None } };
    if !f.copyable {
        return Want { plays: None, make: None };
    }
    if let Some(p) = copy::camera_copy(folder, f.id) {
        return Want { plays: Some(Plays::Camera(p)), make: None };
    }
    let size = f.size_at(n);
    let have = copy::copies_of(folder, f.id);
    let best = copy::best_copy(&have, size);
    let held = best.is_some_and(|b| b.0 >= size.0 && b.1 >= size.1);
    let make = (!held && f.heavy(slow)).then_some(size);
    // a smaller one only meanwhile: with none to come, the original
    Want { plays: best.filter(|_| held || make.is_some()).map(|(w, h)| Plays::Copy(w, h)), make }
}

fn export_runs() -> bool {
    kt_time::sched::paused() & kt_time::sched::why::EXPORT != 0
}

pub(crate) fn run(c: Arc<Copies>, parts: PreviewParts, media: Arc<MediaParts>, stop: Arc<AtomicBool>) {
    loop {
        let seen = c.wake.seen();
        if stop.load(Ordering::Acquire) {
            return;
        }
        let cfg = c.cfg.lock().unwrap().clone();
        let built = parts.builds.latest();
        let Some(cfg) = cfg else {
            c.wake.wait_since(seen);
            continue;
        };
        if built.seq < parts.builds.want_seq() {
            // the build that lands notifies
            c.wake.wait_since(seen);
            continue;
        }
        let Ok(folder) = Folder::open(&cfg.root) else {
            c.wake.wait_since(seen);
            continue;
        };
        let (mode, q) = (c.mode(), parts.quality());
        let set = &built.project.settings;
        let full = q.width >= set.width && q.height >= set.height;
        // what the preview decodes each media at
        let mut decode: BTreeMap<u64, (u32, u32)> = BTreeMap::new();
        built.video.media_decode_sizes(q, &mut decode);
        let looks = c.looks.load(Ordering::Acquire);
        let period = built.video.grid.frame_step().max(1) as u64 * 1_000_000_000 / kt_time::TICK_HZ as u64;
        let mut job: Option<(MediaId, PathBuf, CopyFacts, (u32, u32))> = None;
        for m in &built.project.media {
            if stop.load(Ordering::Acquire) {
                return;
            }
            let Some(&d) = decode.get(&m.id.0) else { continue };
            let Some(path) = (cfg.resolve)(&m.path) else { continue };
            let raised = media.raised(m.id);
            let facts = {
                let mut g = c.states.lock().unwrap();
                let s = g.entry(m.id).or_default();
                if s.path != path || s.raised != raised {
                    // another file, or the host raised the original again: nothing is known of it
                    *s = State { path: path.clone(), raised, ..State::default() };
                }
                s.facts
            };
            let facts = match facts {
                Some(f) => f,
                // the file's index is read here, never on the interface's thread; not while it plays
                None if parts.transport.is_playing() => continue,
                None => {
                    let f = cfg.maker.facts(&folder, &path);
                    c.with(m.id, |s| s.facts = Some(f));
                    f
                }
            };
            let Some(f) = facts else { continue };
            let measured = media.picture_ns(m.id).is_some_and(|ns| ns > period);
            let (decided, known) = {
                let mut g = c.states.lock().unwrap();
                let s = g.entry(m.id).or_default();
                s.slow |= measured;
                let d = (mode, full, d, s.slow, looks);
                (d, s.decided == Some(d))
            };
            if !known {
                let w = want(&folder, &f, mode, full, d, decided.3);
                let plays = c.states.lock().unwrap().get(&m.id).and_then(|s| s.plays.clone());
                c.with(m.id, |s| (s.decided, s.make) = (Some(decided), w.make));
                if plays != w.plays {
                    swap(&c, &cfg, &media, &folder, m.id, &path, &f, raised, w.plays);
                }
            }
            let make = c.states.lock().unwrap().get(&m.id).filter(|s| s.failed.is_none()).and_then(|s| s.make);
            if let (None, Some(size)) = (&job, make) {
                job = Some((m.id, path, f, size));
            }
        }
        let unavailable = c.unavailable.lock().unwrap().is_some();
        let Some((id, path, f, size)) = job.filter(|_| !unavailable) else {
            c.settled.store(true, Ordering::Release);
            c.wake.wait_since(seen);
            continue;
        };
        c.settled.store(false, Ordering::Release);
        if parts.transport.is_playing() || export_runs() {
            c.wake.wait_since(seen);
            continue;
        }
        if let Some(left) = parts.rc.quiet_left() {
            c.wake.wait_timeout(left);
            continue;
        }
        make(&c, &cfg, &parts, &media, &folder, id, &path, &f, size, &stop);
    }
}

/// Raise the media's source again, from what it should play from.
#[allow(clippy::too_many_arguments)]
fn swap(
    c: &Copies,
    cfg: &Config,
    media: &MediaParts,
    folder: &Arc<Folder>,
    id: MediaId,
    path: &std::path::Path,
    f: &CopyFacts,
    raised: u64,
    to: Option<Plays>,
) {
    let source = cfg.maker.original(folder, path).and_then(|o| match &to {
        None => Ok(o),
        Some(Plays::Copy(w, h)) => {
            let rel = copy::copy_rel(f.id, (*w, *h));
            let s = cfg.maker.copy(&folder.root().join(&rel))?;
            Ok(Box::new(CopySource::new(o, s, Some((folder.clone(), rel)))) as _)
        }
        Some(Plays::Camera(p)) => Ok(Box::new(CopySource::new(o, cfg.maker.copy(p)?, None)) as _),
    });
    match source {
        Ok(s) => {
            if !media.swap_picture(id, raised, s) {
                return;
            }
            // a copy smaller than the one that plays now serves nobody
            if let Some(Plays::Copy(w, h)) = to {
                for (a, b) in copy::copies_of(folder, f.id).into_iter().filter(|c| c.0 < w && c.1 < h) {
                    let rel = copy::copy_rel(f.id, (a, b));
                    if std::fs::remove_file(folder.root().join(&rel)).is_ok() {
                        folder.gone(&rel);
                    }
                }
            }
            c.with(id, |s| s.plays = to);
        }
        Err(why) => {
            // a copy that does not open: it is made again
            if let Some(Plays::Copy(w, h)) = to {
                let rel = copy::copy_rel(f.id, (w, h));
                let _ = std::fs::remove_file(folder.root().join(&rel));
                folder.gone(&rel);
            }
            c.with(id, |s| s.failed = Some(why));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn make(
    c: &Copies,
    cfg: &Config,
    parts: &PreviewParts,
    media: &MediaParts,
    folder: &Arc<Folder>,
    id: MediaId,
    path: &std::path::Path,
    f: &CopyFacts,
    size: (u32, u32),
    stop: &AtomicBool,
) {
    let rel = copy::copy_rel(f.id, size);
    let dest = folder.root().join(&rel);
    // about what an all-intra picture takes at the quality asked: the room is asked before the bytes come
    let bytes = size.0 as u64 * size.1 as u64 / 8 * f.frames.max(1);
    if !folder.ask_room(bytes) {
        c.with(id, |s| s.failed = Some("no room in the cache folder".into()));
        return;
    }
    let dir = dest.parent().unwrap_or(folder.root());
    if let Err(e) = std::fs::create_dir_all(dir) {
        c.with(id, |s| s.failed = Some(e.to_string()));
        return;
    }
    let tmp = dir.join(format!(".copy-{}x{}.{}.tmp", size.0, size.1, std::process::id()));
    let (mode, q, raised) = (c.mode(), parts.quality(), media.raised(id));
    let _class = kt_time::sched::with_class(kt_time::sched::Class::Background);
    let r = cfg.maker.make(path, &tmp, size, &mut |n| {
        // what it makes is for this quality, mode and file
        if stop.load(Ordering::Acquire) || c.mode() != mode || parts.quality() != q || media.raised(id) != raised {
            return Go::Stop;
        }
        if n % 16 == 0 {
            c.with(id, |s| s.making = Some((n, f.frames)));
        }
        if parts.transport.is_playing() || export_runs() { Go::Wait } else { Go::On }
    });
    let mut failed = None;
    match r {
        Ok(done) => match std::fs::rename(&tmp, &dest) {
            Ok(()) => {
                let len = std::fs::metadata(&dest).map_or(0, |m| m.len());
                folder.wrote(&rel, len, done.decode_us);
                c.made.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => failed = Some(e.to_string()),
        },
        Err(CopyError::Stopped) => {}
        Err(CopyError::NoEncoder(why)) => {
            // the owner's rule: no processor codec — no copy on this machine
            *c.unavailable.lock().unwrap() = Some(why);
        }
        Err(CopyError::Failed(why)) => failed = Some(why),
    }
    // a process that ends leaves it to the next open's sweep (found-defects 972)
    if !kt_time::shutdown::ending() {
        let _ = std::fs::remove_file(&tmp);
    }
    c.with(id, |s| {
        s.making = None;
        s.failed = failed;
        // what lies on the disk moved, or the job was stopped for a change: decided again
        s.decided = None;
        s.make = None;
    });
}
