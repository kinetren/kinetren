//! The cache folder of a project (project-cache-folder): `<project folder>/kinetren-cache/`, one per
//! project folder, shared by the projects in it. It holds only what can be computed again.
//!
//! - **The place** (D1, D4): [`beside`] names it; [`Folder::open`] makes it and tests that it takes
//!   files. A folder that takes none, and an untitled project, cache under `<data>/project-caches`.
//! - **An entry file** (D1) starts with a 32-byte header naming its kind, the kind's version, the
//!   length and a checksum, and appears by a rename: a torn or foreign one is a miss and is removed.
//! - **The cycle** (D3): a writer asks the folder for room; past it the folder drops what is worth
//!   least. It runs on the writer's thread, under the folder's `lock`, and only when something writes.
//!
//! The UI thread opens and touches none of it.

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kt_time::trace::{self, Ev, cache_folder as cf};

/// The folder's name beside the project.
pub const DIR_NAME: &str = "kinetren-cache";
/// The cap of a folder the person set none for.
pub const DEFAULT_CAP: u64 = 20_000_000_000;
/// How long an entry this process used is never dropped.
const PROTECT: Duration = Duration::from_secs(60);
/// The least the cycle leaves free on the disk the person's work is on.
const RESERVE: u64 = 2_000_000_000;
/// How stale the folder's count may be when the cycle drops (another process writes there too).
const RECOUNT: Duration = Duration::from_secs(60);
/// How rarely the stamps are written while the folder is written to.
const STAMPS_EVERY: Duration = Duration::from_secs(30);
const USAGE: &str = "usage.tsv";
const TAG: &str = "CACHEDIR.TAG";
const TAG_TEXT: &str = "Signature: 8a477f597d28d172789f06886806bc55\n\
# This folder is a cache of Kinetren. Backup tools that know the Cache Directory Tagging\n\
# specification skip it: https://bford.info/cachedir/\n";
/// A project kept in git takes none of it.
const IGNORE: &str = ".gitignore";
const README: &str = "README.txt";
const README_TEXT: &str = "Kinetren keeps recomputable data here: waveforms, thumbnails, previews.\n\
You can delete this folder at any time; nothing of your work is in it.\n";

/// `<project folder>/kinetren-cache` of a project file.
pub fn beside(project: &Path) -> PathBuf {
    let abs = std::path::absolute(project).unwrap_or_else(|_| project.to_path_buf());
    abs.parent().map(Path::to_path_buf).unwrap_or_default().join(DIR_NAME)
}

// ---- the data root -------------------------------------------------------------------------------

fn data_cell() -> &'static Mutex<Option<PathBuf>> {
    static D: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    D.get_or_init(Default::default)
}

/// The user's data folder, said once by a binary at its start. Unsaid, a folder that takes no files
/// has no second place and no registry is kept: a library or a test writes nothing of the user's. A
/// folder is listed in the registry by the program that shows the list (`registry::note`), never by
/// being opened: a headless run over a temporary project leaves no row.
pub fn set_data_root(data: &Path) {
    *lock(data_cell()) = Some(data.to_path_buf());
}

fn data_root() -> Option<PathBuf> {
    lock(data_cell()).clone()
}

/// Where this process's untitled projects cache (D4), under the data root.
pub fn untitled() -> Option<PathBuf> {
    static NAME: OnceLock<String> = OnceLock::new();
    let name = NAME.get_or_init(|| format!("untitled-{}-{}", std::process::id(), now_ms()));
    Some(kt_dirs::project_caches_root(&data_root()?).join(name))
}

/// Remove the folders untitled sessions left by a crash: those whose owner's lock is free (D4).
pub fn sweep_untitled() {
    let Some(data) = data_root() else { return };
    let mine = untitled();
    let Ok(rd) = std::fs::read_dir(kt_dirs::project_caches_root(&data)) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if Some(&p) == mine.as_ref() || !e.file_name().to_string_lossy().starts_with("untitled-") {
            continue;
        }
        let free = match std::fs::OpenOptions::new().read(true).write(true).open(p.join(OWNER)) {
            Ok(f) => f.try_lock().is_ok(),
            // a folder just being made has no owner yet: it stays for the next start
            Err(_) => false,
        };
        if free {
            let _ = std::fs::remove_dir_all(&p);
            registry::remove(&p);
        }
    }
}

const OWNER: &str = "owner";

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

// ---- tenants and entries -------------------------------------------------------------------------

/// Whose an entry is; the order the cycle empties them in (the first three by one value order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tenant {
    /// Final preview frames (`results/`).
    Frames,
    /// Kept runs of the heard mix (`mix/`).
    Mix,
    /// Copies of heavy sources (`media/<id>/copy-*`).
    Copy,
    /// Reel previews (`reels/`).
    Reel,
    /// The media index (`media/`, `pairs/`).
    Media,
}

impl Tenant {
    pub const ALL: [Tenant; 5] = [Tenant::Frames, Tenant::Mix, Tenant::Copy, Tenant::Reel, Tenant::Media];

    pub fn name(self) -> &'static str {
        cf::tenant_name(self as u64)
    }

    fn by_value(self) -> bool {
        matches!(self, Tenant::Frames | Tenant::Mix | Tenant::Copy)
    }
}

/// The tenant of an entry's name.
fn tenant_of(entry: &str) -> Option<Tenant> {
    match entry.split('/').next()? {
        "results" => Some(Tenant::Frames),
        "mix" => Some(Tenant::Mix),
        "media" if entry.contains("/copy-") => Some(Tenant::Copy),
        "media" | "pairs" => Some(Tenant::Media),
        "reels" => Some(Tenant::Reel),
        _ => None,
    }
}

/// The entry a file of the folder belongs to (`rel` with `/`): what is dropped as one. `None` — the
/// folder's own files.
fn entry_of(rel: &str) -> Option<(Tenant, String)> {
    let parts: Vec<&str> = rel.split('/').collect();
    let stem = |r: &str| r.rsplit_once('.').map_or(r, |(a, _)| a).to_string();
    let entry = match parts.as_slice() {
        ["media", id, f] if f.starts_with("copy-") => format!("media/{id}/{}", stem(f)),
        ["media", id, _] => format!("media/{id}"),
        ["pairs", p] => format!("pairs/{p}"),
        ["reels", "refs", _] => return None,
        ["reels", k, _] => format!("reels/{k}"),
        ["results", .., _] | ["mix", .., _] => stem(rel),
        _ => return None,
    };
    Some((tenant_of(&entry)?, entry))
}

#[derive(Clone, Debug, Default)]
struct Use {
    bytes: u64,
    /// Last use in any process, ms since the epoch.
    last_ms: u64,
    /// What one read of it saves, µs (frames, mix, copies).
    cost_us: u64,
    reads: u64,
    /// Last use in this process.
    now: Option<Instant>,
}

impl Use {
    /// What a byte of it is worth: the cost a read saves × the reads. One read ahead is counted, so
    /// what was just written is not worth nothing until its first lap.
    fn value(&self) -> f64 {
        (self.cost_us as f64) * (self.reads + 1) as f64 / self.bytes.max(1) as f64
    }
}

/// Why the cycle dropped entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    Cap,
    Disk,
    Quota,
    /// Asked by the person or a command.
    Asked,
}

impl Why {
    fn code(self) -> u64 {
        match self {
            Why::Cap => cf::why::CAP,
            Why::Disk => cf::why::DISK,
            Why::Quota => cf::why::QUOTA,
            Why::Asked => cf::why::ASKED,
        }
    }
}

/// Where a folder's files lie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Beside the project.
    Project,
    /// Under the user's data folder: the project is untitled.
    Untitled,
    /// Under the user's data folder: the project's folder took no file, and what it answered.
    NoFiles(String),
}

/// What the person is told about a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// The first drop: the folder is full at `bytes`, the oldest results are replaced.
    Full { folder: PathBuf, bytes: u64 },
    /// The project's folder takes no files.
    Elsewhere { folder: PathBuf, why: String },
}

#[derive(Default)]
struct Counts {
    hits: u64,
    misses: u64,
    drops: u64,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<String, (Tenant, Use)>,
    /// When the tree was last walked; `None` — never in this process, or the system contradicted it.
    counted: Option<Instant>,
    /// Stamps not yet in `usage.tsv`, and when they were last written.
    dirty: bool,
    saved: Option<Instant>,
    counts: BTreeMap<Tenant, Counts>,
    skipped: u64,
    /// The cap and the free space when the folder last said it was full.
    said: Option<(u64, u64)>,
    dropped: Vec<String>,
}

/// A temporary name of this process's writes.
static SEQ: AtomicU64 = AtomicU64::new(0);

pub struct Folder {
    root: PathBuf,
    base: Option<PathBuf>,
    place: Place,
    cap: AtomicU64,
    inner: Mutex<Inner>,
    pub(crate) paths: Mutex<crate::index::PathMap>,
    /// The untitled folder's owner lock, held for the process's life.
    _owner: Option<std::fs::File>,
    seams: Mutex<Seams>,
}

/// What a test sets.
#[derive(Default)]
struct Seams {
    fail_writes: Vec<std::io::ErrorKind>,
    free: Option<u64>,
    protect: Option<Duration>,
    walks: u64,
}

fn open_folders() -> &'static Mutex<HashMap<PathBuf, Arc<Folder>>> {
    static F: OnceLock<Mutex<HashMap<PathBuf, Arc<Folder>>>> = OnceLock::new();
    F.get_or_init(Default::default)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Folder {
    /// The folder at `preferred`, made and tested once per process: every lane of a project shares
    /// it. A place that takes no files falls back under the data root.
    pub fn open(preferred: &Path) -> std::io::Result<Arc<Folder>> {
        let key = std::path::absolute(preferred)?;
        if let Some(f) = lock(open_folders()).get(&key) {
            return Ok(f.clone());
        }
        // one lane makes it, the others wait: the open also says the notice and the registry's row
        static MAKE: Mutex<()> = Mutex::new(());
        let _one = lock(&MAKE);
        if let Some(f) = lock(open_folders()).get(&key) {
            return Ok(f.clone());
        }
        let f = Arc::new(Folder::make(&key, data_root().as_deref())?);
        lock(open_folders()).insert(key, f.clone());
        Ok(f)
    }

    /// The folder already open at `preferred`, without touching the disk (the UI's question).
    pub fn peek(preferred: &Path) -> Option<Arc<Folder>> {
        let key = std::path::absolute(preferred).ok()?;
        lock(open_folders()).get(&key).cloned()
    }

    /// Forget every open folder that lies at or under `dir`: its place moves or goes.
    pub fn forget(dir: &Path) {
        lock(open_folders()).retain(|_, f| !f.root.starts_with(dir));
    }

    fn make(preferred: &Path, data: Option<&Path>) -> std::io::Result<Folder> {
        let caches = data.map(kt_dirs::project_caches_root);
        let (root, place) = match write_test(preferred) {
            Ok(()) if caches.as_ref().is_some_and(|c| preferred.starts_with(c)) => (preferred.to_path_buf(), Place::Untitled),
            Ok(()) => (preferred.to_path_buf(), Place::Project),
            Err(e) => {
                let Some(c) = caches else { return Err(e) };
                let of = preferred.parent().unwrap_or(preferred).to_string_lossy().into_owned();
                let root = c.join(format!("{:016x}", xxhash_rust::xxh3::xxh3_64(of.as_bytes())));
                write_test(&root)?;
                (root, Place::NoFiles(e.to_string()))
            }
        };
        let base = match place {
            Place::Project => root.parent().map(Path::to_path_buf),
            _ => None,
        };
        let owner = match place {
            Place::Untitled => std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(root.join(OWNER))
                .ok()
                .filter(|f| f.try_lock().is_ok()),
            _ => None,
        };
        if synced(&root) == Some(CloudTool::Dropbox) {
            kt_dirs::mark_unsynced(&root);
        }
        let cap = registry::cap(&root).unwrap_or(DEFAULT_CAP);
        let f = Folder {
            root,
            base,
            place,
            cap: AtomicU64::new(cap),
            inner: Mutex::default(),
            paths: Mutex::default(),
            _owner: owner,
            seams: Mutex::default(),
        };
        f.mark();
        f.load_stamps();
        let (aux, named) = match &f.place {
            Place::Project => (cf::PLACE_PROJECT, f.root.display().to_string()),
            Place::Untitled => (cf::PLACE_UNTITLED, f.root.display().to_string()),
            Place::NoFiles(why) => (cf::PLACE_NO_FILES, format!("{}: {why}", f.root.display())),
        };
        trace::ev(Ev::CacheFolder).aux(aux).id(trace::name(&named) as u64).b(cap).emit();
        if let Place::NoFiles(why) = &f.place {
            say(Notice::Elsewhere { folder: preferred.parent().unwrap_or(preferred).to_path_buf(), why: why.clone() });
        }
        Ok(f)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The project's folder, when the cache lies in it: paths inside it are kept relative.
    pub fn base(&self) -> Option<&Path> {
        self.base.as_deref()
    }

    pub fn place(&self) -> &Place {
        &self.place
    }

    pub fn cap(&self) -> u64 {
        self.cap.load(Ordering::Relaxed)
    }

    /// Set the cap (Preferences): the registry keeps it, the next write obeys it.
    pub fn set_cap(&self, cap: u64) {
        self.cap.store(cap, Ordering::Relaxed);
        registry::set_cap(&self.root, cap);
    }

    /// The marks of a cache folder. A folder the person deleted while the program runs is made again
    /// by the next write.
    fn mark(&self) {
        for (name, text) in [(TAG, TAG_TEXT), (README, README_TEXT), (IGNORE, "*\n")] {
            let p = self.root.join(name);
            if !p.exists() {
                let _ = std::fs::write(p, text);
            }
        }
    }

    // ---- entries -----------------------------------------------------------------------------------

    /// The payload of the entry file `rel`, of `kind` at `version`. Anything else there — torn, of
    /// another kind or version — is a miss and is removed.
    pub fn read(&self, rel: &str, kind: &[u8; 4], version: u32) -> Option<Vec<u8>> {
        let p = self.root.join(rel);
        let got = match std::fs::read(&p) {
            Ok(mut b) if is_entry(&b, kind, version) => {
                b.drain(..HEADER);
                Some(b)
            }
            Ok(_) => {
                let _ = std::fs::remove_file(&p);
                None
            }
            Err(_) => None,
        };
        self.count_read(rel, got.is_some());
        got
    }

    fn count_read(&self, rel: &str, hit: bool) {
        let Some((t, entry)) = entry_of(rel) else { return };
        let mut g = lock(&self.inner);
        let c = g.counts.entry(t).or_default();
        if hit {
            c.hits += 1;
            let e = &mut g.entries.entry(entry).or_insert((t, Use::default())).1;
            e.last_ms = now_ms();
            e.now = Some(Instant::now());
            e.reads += 1;
            g.dirty = true;
        } else {
            c.misses += 1;
        }
    }

    /// Write the entry file `rel` whole, under a temporary name and a rename. No room is an error the
    /// caller takes as "not kept".
    pub fn write(&self, rel: &str, kind: &[u8; 4], version: u32, payload: &[u8]) -> std::io::Result<()> {
        let len = (HEADER + payload.len()) as u64;
        let before = self.with_room(len, || {
            let p = self.root.join(rel);
            let dir = p.parent().unwrap_or(&self.root);
            self.made(dir)?;
            let tmp = dir.join(format!(
                ".{}.{}-{}.tmp",
                p.file_name().unwrap_or_default().to_string_lossy(),
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let before = std::fs::metadata(&p).map_or(0, |m| m.len());
            let put = || -> std::io::Result<()> {
                let mut f = std::fs::File::create(&tmp)?;
                f.write_all(&header(kind, version, payload))?;
                f.write_all(payload)
            };
            if let Err(e) = put().and_then(|()| std::fs::rename(&tmp, &p)) {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
            Ok(before)
        })?;
        self.count_write(rel, len, before, 0);
        Ok(())
    }

    /// Append `bytes` to the file `rel`, made with a header of `kind` and `version` when it is not
    /// there: a file of records, each with its own check. One write call per append.
    pub fn append(&self, rel: &str, kind: &[u8; 4], version: u32, bytes: &[u8]) -> std::io::Result<()> {
        let wrote = self.with_room(bytes.len() as u64, || {
            let p = self.root.join(rel);
            self.made(p.parent().unwrap_or(&self.root))?;
            let mut f = std::fs::OpenOptions::new().append(true).create(true).open(&p)?;
            let mut buf = Vec::with_capacity(HEADER + bytes.len());
            if f.metadata()?.len() == 0 {
                buf.extend_from_slice(&header(kind, version, &[]));
            }
            buf.extend_from_slice(bytes);
            f.write_all(&buf)?;
            Ok(buf.len() as u64)
        })?;
        self.count_write(rel, wrote, 0, 0);
        Ok(())
    }

    /// Make a folder of an entry, and the marks again when the person deleted the whole cache.
    pub(crate) fn made(&self, dir: &Path) -> std::io::Result<()> {
        if !dir.is_dir() {
            std::fs::create_dir_all(dir)?;
            self.mark();
        }
        let mut s = lock(&self.seams);
        match s.fail_writes.is_empty() {
            true => Ok(()),
            false => Err(s.fail_writes.remove(0).into()),
        }
    }

    /// Run a write of `bytes` after asking for room; a system that refuses it for lack of space is the
    /// same event (D3): the folder counts itself again, drops, and the write is tried once more.
    fn with_room<T>(&self, bytes: u64, put: impl Fn() -> std::io::Result<T>) -> std::io::Result<T> {
        use std::io::ErrorKind as K;
        if !self.ask_room(bytes) {
            return Err(K::StorageFull.into());
        }
        match put() {
            Err(e) if matches!(e.kind(), K::StorageFull | K::QuotaExceeded) => {
                let why = if e.kind() == K::QuotaExceeded { Why::Quota } else { Why::Disk };
                if !self.refused(bytes, why) {
                    return Err(e);
                }
                put()
            }
            r => r,
        }
    }

    /// The system refused a write of `bytes` for lack of space (an owner that writes its own files
    /// says so): the folder counts itself again and drops. `true` — try once more.
    pub fn refused(&self, bytes: u64, why: Why) -> bool {
        lock(&self.inner).counted = None;
        let ok = self.drop_for(bytes, why);
        if !ok {
            self.skip(bytes);
        }
        ok
    }

    fn skip(&self, bytes: u64) {
        lock(&self.inner).skipped += 1;
        trace::ev(Ev::CacheFolder).aux(cf::SKIPPED).b(bytes).emit();
    }

    /// A file of an entry was written by its owner (a segment an encoder wrote): `bytes` on disk,
    /// what a read of it saves.
    pub fn wrote(&self, rel: &str, bytes: u64, cost_us: u64) {
        // a walk may have met the file while it was written
        let before = entry_of(rel).and_then(|(_, e)| lock(&self.inner).entries.get(&e).map(|(_, u)| u.bytes)).unwrap_or(0);
        self.count_write(rel, bytes, before, cost_us);
    }

    fn count_write(&self, rel: &str, now: u64, before: u64, cost_us: u64) {
        let Some((t, entry)) = entry_of(rel) else { return };
        {
            let mut g = lock(&self.inner);
            g.dirty = true;
            let e = &mut g.entries.entry(entry).or_insert((t, Use::default())).1;
            e.bytes = (e.bytes + now).saturating_sub(before);
            e.last_ms = now_ms();
            e.now = Some(Instant::now());
            e.cost_us = e.cost_us.max(cost_us);
        }
        self.save_stamps(false);
    }

    /// An entry was read by its owner (a segment's frame).
    pub fn used(&self, rel: &str) {
        self.count_read(rel, true);
    }

    /// A read of the owner's own lookup found nothing.
    pub fn missed(&self, t: Tenant) {
        lock(&self.inner).counts.entry(t).or_default().misses += 1;
    }

    /// An entry's owner removed its files.
    pub fn gone(&self, rel: &str) {
        if let Some((_, entry)) = entry_of(rel) {
            let mut g = lock(&self.inner);
            g.entries.remove(&entry);
            g.dirty = true;
        }
    }

    /// The entries under `prefix` the cycle dropped since the last call (an owner forgets its index
    /// of them).
    pub fn take_dropped(&self, prefix: &str) -> Vec<String> {
        let mut g = lock(&self.inner);
        if g.dropped.is_empty() {
            return Vec::new();
        }
        let (mine, left) = std::mem::take(&mut g.dropped).into_iter().partition(|e| e.starts_with(prefix));
        g.dropped = left;
        mine
    }

    // ---- room and the cycle (D3) -------------------------------------------------------------------

    fn free(&self) -> Option<(u64, u64)> {
        let seam = lock(&self.seams).free;
        match seam {
            Some(f) => Some((f, 0)),
            None => kt_dirs::disk_space(&self.root),
        }
    }

    /// The bytes the folder may hold now, and what bounds it.
    fn allowed(&self, used: u64) -> (u64, Why) {
        let cap = self.cap();
        match self.free() {
            Some((free, total)) => {
                let disk = used + free.saturating_sub(RESERVE.max(total / 50));
                if disk < cap { (disk, Why::Disk) } else { (cap, Why::Cap) }
            }
            None => (cap, Why::Cap),
        }
    }

    /// Room for `bytes` more, dropping what is worth least when there is none. `false` — nothing
    /// droppable is left: the write is skipped and counted. On the writer's thread.
    pub fn ask_room(&self, bytes: u64) -> bool {
        self.recount(None);
        let used = self.used_bytes();
        let (allowed, why) = self.allowed(used);
        if used + bytes <= allowed || self.drop_for(bytes, why) {
            return true;
        }
        self.skip(bytes);
        false
    }

    /// Would `bytes` more fit with nothing dropped that this process used in the last minute?
    pub fn has_room(&self, bytes: u64) -> bool {
        self.recount(None);
        let used = self.used_bytes();
        let (allowed, _) = self.allowed(used);
        let protect = self.protect();
        let loose: u64 = lock(&self.inner)
            .entries
            .values()
            .filter(|(_, u)| u.now.is_none_or(|t| t.elapsed() >= protect))
            .map(|(_, u)| u.bytes)
            .sum();
        used - loose + bytes <= allowed
    }

    fn protect(&self) -> Duration {
        lock(&self.seams).protect.unwrap_or(PROTECT)
    }

    /// Drop entries until `bytes` fit; with the system refusing, at least `bytes` of them.
    fn drop_for(&self, bytes: u64, why: Why) -> bool {
        // one process of the folder drops at a time; one that cannot now skips its write
        let Ok(f) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(self.root.join("lock")) else {
            return false;
        };
        if f.try_lock().is_err() {
            return false;
        }
        self.recount(Some(RECOUNT));
        let used = self.used_bytes();
        let (allowed, _) = self.allowed(used);
        let target = match why {
            // the system refused the write whatever the count says
            Why::Disk | Why::Quota if used + bytes <= allowed => used.saturating_sub(bytes),
            _ => match allowed.checked_sub(bytes) {
                Some(t) => t,
                // larger than all the folder may hold: no drop makes room for it
                None => return false,
            },
        };
        let protect = self.protect();
        let mut list: Vec<(Tenant, String, Use)> = lock(&self.inner)
            .entries
            .iter()
            .filter(|(_, (_, u))| u.now.is_none_or(|t| t.elapsed() >= protect))
            .map(|(k, (t, u))| (*t, k.clone(), u.clone()))
            .collect();
        list.sort_by(|a, b| {
            let group = |t: Tenant| if t.by_value() { Tenant::Frames } else { t };
            let value = |x: &(Tenant, String, Use)| if x.0.by_value() { x.2.value() } else { 0.0 };
            group(a.0).cmp(&group(b.0)).then(value(a).total_cmp(&value(b))).then(a.2.last_ms.cmp(&b.2.last_ms)).then(a.1.cmp(&b.1))
        });
        let mut left = used;
        let mut any = false;
        for (t, entry, u) in list {
            if left <= target {
                break;
            }
            self.remove_entry(t, &entry);
            left = left.saturating_sub(u.bytes);
            any = true;
            self.note_drop(t, entry, u.bytes, why);
        }
        if any {
            self.say_full(used);
            self.save_stamps(true);
        }
        left <= target
    }

    fn note_drop(&self, t: Tenant, entry: String, bytes: u64, why: Why) {
        let mut g = lock(&self.inner);
        g.entries.remove(&entry);
        g.dirty = true;
        g.counts.entry(t).or_default().drops += 1;
        g.dropped.push(entry);
        drop(g);
        trace::ev(Ev::CacheFolder).aux(cf::DROP).a(t as u64).b(bytes).c(why.code()).emit();
    }

    fn say_full(&self, bytes: u64) {
        let free = self.free().map_or(0, |f| f.0);
        let cap = self.cap();
        {
            let mut g = lock(&self.inner);
            let moved = |a: u64, b: u64| a.abs_diff(b) > a.max(b) / 10;
            if g.said.is_some_and(|(c, f)| !moved(c, cap) && !moved(f, free)) {
                return;
            }
            g.said = Some((cap, free));
        }
        say(Notice::Full { folder: self.root.clone(), bytes });
    }

    /// Remove the files of an entry.
    fn remove_entry(&self, t: Tenant, entry: &str) {
        let p = self.root.join(entry);
        if !t.by_value() {
            if p.is_dir() {
                // a source's copies are entries of their own
                for e in std::fs::read_dir(&p).into_iter().flatten().flatten() {
                    if !e.file_name().to_string_lossy().starts_with("copy-") {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
                let _ = std::fs::remove_dir(&p);
            } else {
                let _ = std::fs::remove_file(&p);
            }
            return;
        }
        let Some(dir) = p.parent() else { return };
        let stem = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n == stem || n.rsplit_once('.').is_some_and(|(s, _)| s == stem) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    /// Drop everything, or everything of one tenant (Preferences' Delete, `kt-cli cache clear`): the
    /// bytes freed.
    pub fn clear(&self, only: Option<Tenant>) -> u64 {
        self.recount(Some(Duration::ZERO));
        let list: Vec<(Tenant, String, u64)> = lock(&self.inner)
            .entries
            .iter()
            .filter(|(_, (t, _))| only.is_none_or(|o| o == *t))
            .map(|(k, (t, u))| (*t, k.clone(), u.bytes))
            .collect();
        let mut freed = 0;
        for (t, entry, bytes) in list {
            self.remove_entry(t, &entry);
            freed += bytes;
            self.note_drop(t, entry, bytes, Why::Asked);
        }
        if only.is_none() {
            let _ = std::fs::remove_dir_all(self.root.join("reels"));
            let _ = std::fs::remove_file(self.root.join("media").join("paths.tsv"));
        }
        self.save_stamps(true);
        freed
    }

    /// Drop down to the room the folder has now (`kt-cli cache prune`): the bytes freed.
    pub fn prune(&self) -> u64 {
        self.recount(Some(Duration::ZERO));
        let before = self.used_bytes();
        let (allowed, why) = self.allowed(before);
        if before > allowed {
            self.drop_for(0, why);
        }
        before.saturating_sub(self.used_bytes())
    }

    // ---- counting ----------------------------------------------------------------------------------

    /// Walk the tree when the count is older than `than` (`None` — only when there is none yet).
    fn recount(&self, than: Option<Duration>) {
        let due = match (lock(&self.inner).counted, than) {
            (None, _) => true,
            (Some(t), Some(d)) => t.elapsed() >= d,
            _ => false,
        };
        if !due {
            return;
        }
        lock(&self.seams).walks += 1;
        let mut found: HashMap<String, (Tenant, u64, u64)> = HashMap::new();
        walk(&self.root, &self.root, &mut |rel, md| {
            let name = rel.rsplit('/').next().unwrap_or(rel);
            let at = md.modified().ok();
            if name.starts_with('.') && name.ends_with(".tmp") {
                // a write a process died in
                if at.and_then(|t| t.elapsed().ok()).is_some_and(|d| d > Duration::from_secs(86_400)) {
                    let _ = std::fs::remove_file(self.root.join(rel));
                }
                return;
            }
            let Some((t, entry)) = entry_of(rel) else { return };
            let at = at.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
            let e = found.entry(entry).or_insert((t, 0, 0));
            e.1 += md.len();
            e.2 = e.2.max(at);
        });
        let mut g = lock(&self.inner);
        g.entries.retain(|k, _| found.contains_key(k));
        for (k, (t, bytes, at)) in found {
            let e = &mut g.entries.entry(k).or_insert((t, Use::default())).1;
            e.bytes = bytes;
            if e.last_ms == 0 {
                e.last_ms = at;
            }
        }
        let first = g.counted.is_none();
        g.counted = Some(Instant::now());
        drop(g);
        if first {
            self.trace_sizes();
        }
    }

    fn used_bytes(&self) -> u64 {
        lock(&self.inner).entries.values().map(|(_, u)| u.bytes).sum()
    }

    /// Bytes and entries per tenant, counted on the disk now.
    pub fn sizes(&self) -> BTreeMap<Tenant, (u64, usize)> {
        self.recount(Some(Duration::ZERO));
        self.known_sizes()
    }

    /// The same as last counted: the disk is not touched.
    pub fn known_sizes(&self) -> BTreeMap<Tenant, (u64, usize)> {
        let mut out = BTreeMap::new();
        for (t, u) in lock(&self.inner).entries.values() {
            let e: &mut (u64, usize) = out.entry(*t).or_default();
            e.0 += u.bytes;
            e.1 += 1;
        }
        out
    }

    /// Hits, misses and drops of a tenant in this process.
    pub fn counts(&self, t: Tenant) -> (u64, u64, u64) {
        lock(&self.inner).counts.get(&t).map_or((0, 0, 0), |c| (c.hits, c.misses, c.drops))
    }

    /// Writes skipped for lack of room in this process.
    pub fn skipped(&self) -> u64 {
        lock(&self.inner).skipped
    }

    /// Whether the folder writes in its cycle, as last counted.
    pub fn is_full(&self) -> bool {
        let said = lock(&self.inner).said.is_some();
        said || self.used_bytes() >= self.cap()
    }

    /// Whether the tree was counted in this process.
    pub fn is_counted(&self) -> bool {
        lock(&self.inner).counted.is_some()
    }

    /// Per tenant: bytes, hits, misses and drops into the trace (at the first count and at the end).
    pub fn trace_sizes(&self) {
        let sizes = self.known_sizes();
        for t in Tenant::ALL {
            let (bytes, _) = sizes.get(&t).copied().unwrap_or_default();
            let (h, m, d) = self.counts(t);
            if bytes + h + m + d > 0 {
                trace::ev(Ev::CacheFolder).aux(cf::SIZE).a(t as u64).b(bytes).c(h).id(m).emit();
            }
        }
    }

    // ---- stamps --------------------------------------------------------------------------------------

    fn load_stamps(&self) {
        let Ok(text) = std::fs::read_to_string(self.root.join(USAGE)) else { return };
        let mut g = lock(&self.inner);
        for (k, u) in text.lines().filter_map(stamp_of) {
            if let Some(t) = tenant_of(&k) {
                g.entries.insert(k, (t, u));
            }
        }
    }

    /// Write the stamps, at most every half minute unless `now`, merged with what another process
    /// wrote.
    fn save_stamps(&self, now: bool) {
        {
            let mut g = lock(&self.inner);
            if !g.dirty || (!now && g.saved.is_some_and(|t| t.elapsed() < STAMPS_EVERY)) {
                return;
            }
            g.saved = Some(Instant::now());
            g.dirty = false;
        }
        let mut all: BTreeMap<String, Use> = std::fs::read_to_string(self.root.join(USAGE))
            .unwrap_or_default()
            .lines()
            .filter_map(stamp_of)
            .collect();
        {
            let g = lock(&self.inner);
            if g.counted.is_some() {
                all.retain(|k, _| g.entries.contains_key(k));
            }
            for (k, (_, u)) in &g.entries {
                let e = all.entry(k.clone()).or_default();
                *e = Use {
                    bytes: u.bytes,
                    last_ms: e.last_ms.max(u.last_ms),
                    cost_us: e.cost_us.max(u.cost_us),
                    reads: e.reads.max(u.reads),
                    now: None,
                };
            }
        }
        let mut text = String::new();
        for (k, u) in &all {
            text.push_str(&format!("{k}\t{}\t{}\t{}\t{}\n", u.bytes, u.last_ms, u.cost_us, u.reads));
        }
        let tmp = self.root.join(format!(".{USAGE}.{}-{}.tmp", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
        if std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, self.root.join(USAGE))).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    /// The stamps and the registry's size of this folder, written now (the project's close).
    pub fn flush(&self) {
        if !lock(&self.inner).dirty {
            return;
        }
        self.save_stamps(true);
        self.trace_sizes();
        if lock(&self.inner).counted.is_some() {
            registry::set_bytes(&self.root, self.used_bytes());
        }
    }

    // ---- tests' seams --------------------------------------------------------------------------------

    /// The next writes answer these errors.
    #[doc(hidden)]
    pub fn fail_writes_for_tests(&self, kinds: &[std::io::ErrorKind]) {
        lock(&self.seams).fail_writes = kinds.to_vec();
    }

    /// The disk's free bytes as the folder sees them.
    #[doc(hidden)]
    pub fn set_free_for_tests(&self, free: Option<u64>) {
        lock(&self.seams).free = free;
    }

    /// How long a used entry is protected.
    #[doc(hidden)]
    pub fn set_protect_for_tests(&self, d: Option<Duration>) {
        lock(&self.seams).protect = d;
    }

    /// How often the tree was walked.
    #[doc(hidden)]
    pub fn walks_for_tests(&self) -> u64 {
        lock(&self.seams).walks
    }

    /// Files this process hashed whole to know what they are.
    #[doc(hidden)]
    pub fn hashed_for_tests(&self) -> u64 {
        lock(&self.paths).hashed
    }

    /// An entry's stamps as if it was last used `last_ms`, read `reads` times at `cost_us` a read, and
    /// not by this process.
    #[doc(hidden)]
    pub fn age_for_tests(&self, entry: &str, last_ms: u64, reads: u64, cost_us: u64) {
        self.recount(None);
        if let Some((_, u)) = lock(&self.inner).entries.get_mut(entry) {
            (u.last_ms, u.reads, u.cost_us, u.now) = (last_ms, reads, cost_us, None);
        }
    }

    #[doc(hidden)]
    pub fn holds_for_tests(&self, entry: &str) -> bool {
        lock(&self.inner).entries.contains_key(entry)
    }
}

fn stamp_of(l: &str) -> Option<(String, Use)> {
    let mut it = l.split('\t');
    let k = it.next()?.to_string();
    let mut n = || it.next()?.parse::<u64>().ok();
    let u = Use { bytes: n()?, last_ms: n()?, cost_us: n()?, reads: n()?, now: None };
    Some((k, u))
}

fn walk(root: &Path, dir: &Path, f: &mut dyn FnMut(&str, &std::fs::Metadata)) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let Ok(md) = e.metadata() else { continue };
        if md.is_dir() {
            walk(root, &e.path(), f);
        } else if let Ok(rel) = e.path().strip_prefix(root) {
            f(&slashed(rel), &md);
        }
    }
}

/// A relative path with `/` on every system.
pub(crate) fn slashed(rel: &Path) -> String {
    let parts: Vec<String> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    parts.join("/")
}

/// Does `dir` take a file and a rename?
fn write_test(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let a = dir.join(format!(".write-test.{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    let b = a.with_extension("ok");
    let r = std::fs::write(&a, b"x").and_then(|()| std::fs::rename(&a, &b)).and_then(|()| std::fs::remove_file(&b));
    if r.is_err() {
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }
    r
}

// ---- the entry header (D1) -------------------------------------------------------------------------

/// `KTC1`, the kind, its version, the payload's length, its xxh3-64, four bytes of room.
pub const HEADER: usize = 32;

pub(crate) fn header(kind: &[u8; 4], version: u32, payload: &[u8]) -> [u8; HEADER] {
    let mut h = [0u8; HEADER];
    h[..4].copy_from_slice(b"KTC1");
    h[4..8].copy_from_slice(kind);
    h[8..12].copy_from_slice(&version.to_le_bytes());
    h[12..20].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    h[20..28].copy_from_slice(&xxhash_rust::xxh3::xxh3_64(payload).to_le_bytes());
    h
}

/// Does `head` (the first [`HEADER`] bytes of a file) name `kind` at `version`?
pub(crate) fn is_header(head: &[u8], kind: &[u8; 4], version: u32) -> bool {
    head.len() >= HEADER && head[..4] == *b"KTC1" && head[4..8] == *kind && head[8..12] == version.to_le_bytes()
}

/// Is `b` a whole, unbroken entry file of `kind` at `version`?
fn is_entry(b: &[u8], kind: &[u8; 4], version: u32) -> bool {
    b.len() >= HEADER && b[..HEADER] == header(kind, version, &b[HEADER..])
}

/// The header of the file `path` alone.
pub(crate) fn header_of(path: &Path) -> Option<[u8; HEADER]> {
    let mut h = [0u8; HEADER];
    std::fs::File::open(path).ok()?.read_exact(&mut h).ok()?;
    Some(h)
}

// ---- a project that moves (D4, D6) ---------------------------------------------------------------------

/// The cache of a project follows it into another folder, in the background: `from` is where it lay
/// for the project `was` (`None` — an untitled one), `to` the folder beside `now`. An untitled
/// project's folder moves whole (its picture results only inside one file system); Save As copies what
/// is small and costly to compute again, the media index and the reel previews.
pub fn carry(from: &Path, to: &Path, was: Option<&Path>, now: &Path) -> std::io::Result<()> {
    let src = Folder::open(from)?;
    let dst = Folder::open(to)?;
    registry::note(&dst.root, Some(now));
    let moves = src.place == Place::Untitled;
    if src.root == dst.root {
        crate::index::carry_refs(&dst, was, now, true);
        return Ok(());
    }
    let mut names = Vec::new();
    walk(&src.root, &src.root, &mut |rel, _| names.push(rel.to_string()));
    for rel in names {
        let small = matches!(entry_of(&rel), Some((Tenant::Media | Tenant::Reel, _))) || rel.starts_with("reels/refs/");
        let picture = matches!(entry_of(&rel), Some((t, _)) if t.by_value());
        if !(small || (moves && picture)) || rel.ends_with(".tmp") {
            continue;
        }
        let (a, b) = (src.root.join(&rel), dst.root.join(&rel));
        if b.exists() {
            continue;
        }
        if let Some(d) = b.parent() {
            dst.made(d)?;
        }
        // an entry appears whole: a copy goes under a temporary name first
        if moves && std::fs::rename(&a, &b).is_ok() {
            continue;
        }
        if small {
            let tmp = b.with_file_name(format!(".carried.{}-{}.tmp", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
            if std::fs::copy(&a, &tmp).and_then(|_| std::fs::rename(&tmp, &b)).is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
        }
    }
    crate::index::carry_paths(&src, &dst)?;
    crate::index::carry_refs(&dst, was, now, !moves);
    {
        let stamps = lock(&src.inner).entries.clone();
        let mut g = lock(&dst.inner);
        for (k, v) in stamps {
            g.entries.entry(k).or_insert(v);
        }
        g.counted = None;
        g.dirty = true;
    }
    if moves {
        Folder::forget(&src.root);
        let _ = std::fs::remove_dir_all(&src.root);
        registry::remove(&src.root);
    }
    Ok(())
}

// ---- synced folders (D4) -----------------------------------------------------------------------------

/// A cloud tool that syncs a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudTool {
    Dropbox,
    OneDrive,
}

/// The tool whose root `dir` lies in, by the tool's own marks.
pub fn synced(dir: &Path) -> Option<CloudTool> {
    for a in dir.ancestors().skip(1) {
        if a.join(".dropbox.cache").exists() || a.join(".dropbox").is_file() {
            return Some(CloudTool::Dropbox);
        }
        let name = a.file_name().unwrap_or_default().to_string_lossy();
        if name == "OneDrive" || name.starts_with("OneDrive - ") {
            return Some(CloudTool::OneDrive);
        }
    }
    None
}

// ---- notices -------------------------------------------------------------------------------------------

static TOLD: AtomicBool = AtomicBool::new(false);

type Wake = Box<dyn Fn() + Send + Sync>;

fn notices() -> &'static (Mutex<Vec<Notice>>, OnceLock<Wake>) {
    static N: OnceLock<(Mutex<Vec<Notice>>, OnceLock<Wake>)> = OnceLock::new();
    N.get_or_init(Default::default)
}

/// Call `f` when a folder has something to tell (the host wakes its window).
pub fn on_notice(f: impl Fn() + Send + Sync + 'static) {
    let _ = notices().1.set(Box::new(f));
}

fn say(n: Notice) {
    let (list, wake) = notices();
    lock(list).push(n);
    TOLD.store(true, Ordering::Release);
    if let Some(f) = wake.get() {
        f();
    }
}

/// What the folders told since the last call; one load when there is nothing.
pub fn take_notices() -> Vec<Notice> {
    if !TOLD.load(Ordering::Acquire) {
        return Vec::new();
    }
    let mut g = lock(&notices().0);
    TOLD.store(false, Ordering::Release);
    std::mem::take(&mut *g)
}

// ---- the registry (D3) -----------------------------------------------------------------------------------

/// `<data>/cache-folders.tsv`: every cache folder the program made.
pub mod registry {
    use super::{data_root, escape, now_ms, unescape};
    use std::path::{Path, PathBuf};

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct Row {
        pub folder: PathBuf,
        /// The projects seen in it.
        pub projects: Vec<PathBuf>,
        pub last_ms: u64,
        pub bytes: u64,
        /// The cap the person set.
        pub cap: Option<u64>,
    }

    fn file() -> Option<PathBuf> {
        Some(kt_dirs::cache_folders_registry(&data_root()?))
    }

    fn parse(text: &str) -> Vec<Row> {
        text.lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let [folder, last, bytes, cap, projects] = f.as_slice() else { return None };
                Some(Row {
                    folder: PathBuf::from(unescape(folder)),
                    projects: projects.split('|').filter(|p| !p.is_empty()).map(|p| PathBuf::from(unescape(p))).collect(),
                    last_ms: last.parse().ok()?,
                    bytes: bytes.parse().ok()?,
                    cap: cap.parse().ok(),
                })
            })
            .collect()
    }

    /// The rows, as the file has them now.
    pub fn list() -> Vec<Row> {
        file().and_then(|f| std::fs::read_to_string(f).ok()).map(|t| parse(&t)).unwrap_or_default()
    }

    /// Change the rows under the registry's lock; a registry that cannot be written stays as it was.
    fn edit(f: impl FnOnce(&mut Vec<Row>)) {
        let Some(path) = file() else { return };
        let Some(dir) = path.parent() else { return };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let Ok(held) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path.with_extension("lock")) else {
            return;
        };
        // held for a read and a write of a small file: the wait is another process's same
        if held.lock().is_err() {
            return;
        }
        let mut rows = list();
        f(&mut rows);
        let mut text = String::new();
        for r in &rows {
            let projects: Vec<String> = r.projects.iter().map(|p| escape(&p.to_string_lossy())).collect();
            let cap = r.cap.map_or("-".into(), |c| c.to_string());
            text.push_str(&format!(
                "{}\t{}\t{}\t{cap}\t{}\n",
                escape(&r.folder.to_string_lossy()),
                r.last_ms,
                r.bytes,
                projects.join("|")
            ));
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        if std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, &path)).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    fn row<'a>(rows: &'a mut Vec<Row>, folder: &Path) -> &'a mut Row {
        let i = rows.iter().position(|r| r.folder == folder).unwrap_or_else(|| {
            rows.push(Row { folder: folder.to_path_buf(), ..Default::default() });
            rows.len() - 1
        });
        &mut rows[i]
    }

    /// The folder is used now, by `project`.
    pub fn note(folder: &Path, project: Option<&Path>) {
        edit(|rows| {
            let r = row(rows, folder);
            r.last_ms = now_ms();
            if let Some(p) = project
                && !r.projects.iter().any(|x| x == p)
            {
                r.projects.push(p.to_path_buf());
            }
        });
    }

    /// The size of a listed folder.
    pub fn set_bytes(folder: &Path, bytes: u64) {
        if !list().iter().any(|r| r.folder == folder) {
            return;
        }
        edit(|rows| {
            let r = row(rows, folder);
            (r.bytes, r.last_ms) = (bytes, now_ms());
        });
    }

    /// The cap of a listed folder.
    pub fn set_cap(folder: &Path, cap: u64) {
        if list().iter().any(|r| r.folder == folder) {
            edit(|rows| row(rows, folder).cap = Some(cap));
        }
    }

    pub fn cap(folder: &Path) -> Option<u64> {
        list().into_iter().find(|r| r.folder == folder)?.cap
    }

    /// The folder is no more.
    pub fn remove(folder: &Path) {
        edit(|rows| rows.retain(|r| r.folder != folder));
    }
}

/// A path as one field of a tab-separated line.
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\t' => out.push_str("%09"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            '|' => out.push_str("%7C"),
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn unescape(s: &str) -> String {
    ["%09\t", "%0A\n", "%0D\r", "%7C|", "%25%"].iter().fold(s.to_string(), |s, e| s.replace(&e[..3], &e[3..]))
}
