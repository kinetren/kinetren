//! The disk tier of preview composites (render-cache-disk; the owner, kkt-26.11.17 S5 "Сам в фоне";
//! review R3). A slow moment, once composed, is kept on disk and played from it later — the next lap,
//! after an eviction, after a restart — with no decode, no stream stage and no chain.
//!
//! - **The key** (D1): [`durable_hash`] of the probe plan — every field the executor's plan key hashes
//!   but the frames' per-process content serials — mixed with what those serials stood for, collected
//!   while the plan is built ([`Leaves`]): a file's path, size and modification time, a generator's
//!   values, every file of the project for a stream stage. Salted by the format, the program's build
//!   and the device: the folder `<root>/v1-<build>/<device>/`.
//! - **The store** (D2): [`DiskStore`], one lossless file per moment (a left-pixel byte delta,
//!   deflated), bounded, least recently used out; the folders of other builds go at open.
//! - **The reads** (D3) are the player's (`PreviewParts::restore_from_disk`), before anything composes.
//! - **The prerender** (D4) is the player's `render-cache` thread ([`run_prerender`]): at rest, after
//!   [`QUIET`] with no input, the moments the cost map measured at least half a frame period
//!   ([`candidates`]), stopped by any input, Play, an export or the close.
//!
//! No export reads or writes it (D5): the export's bytes never depend on what the preview kept.

use crate::video_plan::{FramePlan, LayerInput, LayerSrc, PlanLayer};
use kt_model::MediaId;
use kt_time::trace::{self, render_cache as rc, Ev};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use xxhash_rust::xxh3::Xxh3;

/// The files' format: a change of the header, the filter or the key's fields moves it.
pub const FORMAT: u32 = 1;
/// The default cap of the store (D2); `KT_RENDER_CACHE_MB` overrides it.
pub const DEFAULT_CAP: u64 = 2 << 30;
/// How long no input must have come before the prerender works (D4: FCP's trigger, shorter).
pub const QUIET: Duration = Duration::from_millis(1500);
const MAGIC: &[u8; 4] = b"KTRC";
const HEADER: usize = 24;
const EXT: &str = "ktf";

/// `KT_RENDER_CACHE=off`: no tier whatever the host set (an A/B).
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("KT_RENDER_CACHE").as_deref() != Ok("off"))
}

/// The cap: `KT_RENDER_CACHE_MB`, else [`DEFAULT_CAP`].
pub fn cap_from_env() -> u64 {
    std::env::var("KT_RENDER_CACHE_MB")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(|mb| mb << 20)
        .unwrap_or(DEFAULT_CAP)
}

/// A composite: tightly packed straight RGBA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// The file's bytes: the header and the deflated left-pixel delta of the RGBA.
pub fn encode(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut d = Vec::with_capacity(rgba.len());
    for (i, &v) in rgba.iter().enumerate() {
        d.push(if i >= 4 { v.wrapping_sub(rgba[i - 4]) } else { v });
    }
    let z = miniz_oxide::deflate::compress_to_vec(&d, 1);
    let mut out = Vec::with_capacity(HEADER + z.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT.to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&(rgba.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&z);
    out
}

/// The picture of a file's bytes; `None` — not ours, another format, short or corrupt.
pub fn decode(bytes: &[u8]) -> Option<Picture> {
    if bytes.len() < HEADER || &bytes[..4] != MAGIC {
        return None;
    }
    let word = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    if word(4) != FORMAT {
        return None;
    }
    let (w, h, len) = (word(8), word(12), word(16) as usize);
    if len != w as usize * h as usize * 4 {
        return None;
    }
    let mut d = miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes[HEADER..], len).ok()?;
    if d.len() != len {
        return None;
    }
    for i in 4..d.len() {
        d[i] = d[i].wrapping_add(d[i - 4]);
    }
    Some(Picture { w, h, rgba: d })
}

fn hex(v: u64) -> String {
    format!("{v:016x}")
}

/// The folder of one build of the format: `v<FORMAT>-<build hash>`.
fn build_dir_name(build: &str) -> String {
    format!("v{FORMAT}-{}", hex(xxhash_rust::xxh3::xxh3_64(build.as_bytes())))
}

struct Entry {
    bytes: u64,
    /// The use order: larger is newer. Entries found at open are numbered by their file times, all
    /// below `session`.
    last: u64,
}

#[derive(Default)]
struct Index {
    map: HashMap<u64, Entry>,
    used: u64,
    seq: u64,
    /// Uses above this were made by this process.
    session: u64,
}

impl Index {
    fn touch(&mut self, key: u64) {
        self.seq += 1;
        let s = self.seq;
        if let Some(e) = self.map.get_mut(&key) {
            e.last = s;
        }
    }
}

/// The disk tier's files of one build and one device (D2).
pub struct DiskStore {
    dir: PathBuf,
    cap: u64,
    ix: Mutex<Index>,
}

/// Why a write kept nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum PutError {
    /// No room without evicting what this process used (the prerender's rule).
    Full,
    Io(String),
}

impl DiskStore {
    /// Open (create) `<root>/v<FORMAT>-<build>/<device>/`, read its index and remove the other builds'
    /// folders under `root` (their pictures can never be asked again).
    pub fn open(root: &Path, build: &str, device: &str, cap: u64) -> std::io::Result<DiskStore> {
        let mine = build_dir_name(build);
        if let Ok(rd) = std::fs::read_dir(root) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let ours = name.starts_with('v') && name.contains('-') && name[1..].split('-').next().is_some_and(|n| n.parse::<u32>().is_ok());
                if ours && name != mine && e.path().is_dir() {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        let dir = root.join(mine).join(hex(xxhash_rust::xxh3::xxh3_64(device.as_bytes())));
        std::fs::create_dir_all(&dir)?;
        let mut found: Vec<(std::time::SystemTime, u64, u64)> = Vec::new();
        for e in std::fs::read_dir(&dir)?.flatten() {
            let p = e.path();
            let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
            if p.extension().and_then(|x| x.to_str()) != Some(EXT) {
                // a temporary file a crash left
                let _ = std::fs::remove_file(&p);
                continue;
            }
            let (Ok(key), Ok(md)) = (u64::from_str_radix(stem, 16), e.metadata()) else { continue };
            found.push((md.modified().unwrap_or(std::time::UNIX_EPOCH), key, md.len()));
        }
        found.sort();
        let mut ix = Index::default();
        for (_, key, bytes) in found {
            ix.seq += 1;
            ix.used += bytes;
            ix.map.insert(key, Entry { bytes, last: ix.seq });
        }
        ix.session = ix.seq;
        let store = DiskStore { dir, cap, ix: Mutex::new(ix) };
        // a smaller cap than the folder holds: the oldest go now
        store.make_room(0, true);
        Ok(store)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, key: u64) -> PathBuf {
        self.dir.join(format!("{}.{EXT}", hex(key)))
    }

    pub fn contains(&self, key: u64) -> bool {
        self.ix.lock().unwrap().map.contains_key(&key)
    }

    /// The bytes the store holds, and the entries.
    pub fn used(&self) -> (u64, usize) {
        let ix = self.ix.lock().unwrap();
        (ix.used, ix.map.len())
    }

    pub fn cap(&self) -> u64 {
        self.cap
    }

    /// The picture under `key`; a file that does not read back is removed and is a miss.
    pub fn get(&self, key: u64) -> Option<Picture> {
        if !self.contains(key) {
            return None;
        }
        let pic = std::fs::read(self.path(key)).ok().and_then(|b| decode(&b));
        let mut ix = self.ix.lock().unwrap();
        match pic {
            Some(p) => {
                ix.touch(key);
                Some(p)
            }
            None => {
                if let Some(e) = ix.map.remove(&key) {
                    ix.used -= e.bytes;
                }
                drop(ix);
                let _ = std::fs::remove_file(self.path(key));
                None
            }
        }
    }

    /// Evict until `bytes` more fit; `any` — this process's entries may go too. `false` — no room.
    fn make_room(&self, bytes: u64, any: bool) -> bool {
        let mut gone = Vec::new();
        let ok = {
            let mut ix = self.ix.lock().unwrap();
            while ix.used + bytes > self.cap {
                let session = ix.session;
                let oldest = ix.map.iter().filter(|(_, e)| any || e.last <= session).min_by_key(|(_, e)| e.last).map(|(k, _)| *k);
                let Some(k) = oldest else { break };
                let e = ix.map.remove(&k).unwrap();
                ix.used -= e.bytes;
                gone.push((k, e.bytes));
            }
            ix.used + bytes <= self.cap
        };
        for (k, b) in gone {
            let _ = std::fs::remove_file(self.path(k));
            trace::ev(Ev::RenderCache).aux(rc::EVICT).b(b).c(k).emit();
        }
        ok
    }

    /// Keep `rgba` under `key`. `evict_session` — entries this process used may be evicted for it;
    /// the prerender passes `false` and stops when only those are left (D2). The file's bytes.
    pub fn put(&self, key: u64, w: u32, h: u32, rgba: &[u8], evict_session: bool) -> Result<u64, PutError> {
        let bytes = encode(w, h, rgba);
        let n = bytes.len() as u64;
        if !self.make_room(n, evict_session) {
            return Err(PutError::Full);
        }
        let tmp = self.dir.join(format!("{}.tmp", hex(key)));
        std::fs::write(&tmp, &bytes).map_err(|e| PutError::Io(e.to_string()))?;
        std::fs::rename(&tmp, self.path(key)).map_err(|e| PutError::Io(e.to_string()))?;
        let mut ix = self.ix.lock().unwrap();
        ix.seq += 1;
        let last = ix.seq;
        if let Some(old) = ix.map.insert(key, Entry { bytes: n, last }) {
            ix.used -= old.bytes;
        }
        ix.used += n;
        Ok(n)
    }

    /// Would a write of about `bytes` fit without evicting what this process used?
    pub fn has_room(&self, bytes: u64) -> bool {
        let ix = self.ix.lock().unwrap();
        let stale: u64 = ix.map.values().filter(|e| e.last <= ix.session).map(|e| e.bytes).sum();
        ix.used - stale + bytes <= self.cap
    }
}

// ---- the durable key (D1) ----------------------------------------------------------------------

/// What the frames' per-process serials stood for, collected while a plan is built (D1).
#[derive(Default, Debug)]
pub struct Leaves {
    h: Xxh3Box,
    /// Files whose identity the key needs.
    files: BTreeSet<MediaId>,
    /// A stream stage reads another event's file: every file of the project joins the key.
    all_files: bool,
}

#[derive(Default)]
struct Xxh3Box(Xxh3);

impl std::fmt::Debug for Xxh3Box {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Xxh3")
    }
}

thread_local! {
    static LEAVES: RefCell<Option<Leaves>> = const { RefCell::new(None) };
}

/// Run `f` (a plan build) collecting the leaves it reads.
pub(crate) fn with_leaves<R>(f: impl FnOnce() -> R) -> (R, Leaves) {
    let before = LEAVES.with(|l| l.borrow_mut().replace(Leaves::default()));
    let r = f();
    let got = LEAVES.with(|l| std::mem::replace(&mut *l.borrow_mut(), before)).unwrap_or_default();
    (r, got)
}

/// The plan builder's leaf (only while [`with_leaves`] collects): a file's frame (`file`), or a
/// procedural source's values (`rev`), and whether it is a stream stage.
pub(crate) fn note_leaf(source: u64, file: Option<MediaId>, rev: u64, stream: bool) {
    LEAVES.with(|l| {
        if let Some(l) = l.borrow_mut().as_mut() {
            l.h.0.update(&source.to_le_bytes());
            l.h.0.update(&rev.to_le_bytes());
            if let Some(m) = file {
                l.files.insert(m);
            }
            l.all_files |= stream;
        }
    });
}

/// The identities of the project's files (D1): path, size and modification time.
#[derive(Default, Clone, Debug)]
pub struct FileIds {
    pub by_media: HashMap<MediaId, u64>,
    /// All of them, for a stream stage.
    pub all: u64,
}

impl FileIds {
    /// Read the identities of `media` (paths relative to `base`); a missing file is its path and a mark.
    pub fn read(media: &[kt_model::MediaRef], base: Option<&Path>) -> FileIds {
        let mut by_media = HashMap::new();
        let mut all = Xxh3::new();
        for m in media {
            let p = Path::new(&m.path);
            let full = match base {
                Some(b) if p.is_relative() => b.join(p),
                _ => p.to_path_buf(),
            };
            let mut h = Xxh3::new();
            h.update(m.path.as_bytes());
            match std::fs::metadata(&full) {
                Ok(md) => {
                    h.update(&md.len().to_le_bytes());
                    let t = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).unwrap_or_default();
                    h.update(&t.as_nanos().to_le_bytes());
                }
                Err(_) => h.update(b"\0missing"),
            }
            let id = h.digest();
            all.update(&m.id.0.to_le_bytes());
            all.update(&id.to_le_bytes());
            by_media.insert(m.id, id);
        }
        FileIds { by_media, all: all.digest() }
    }
}

impl Leaves {
    /// The key's part from the leaves and the files' identities.
    pub fn digest(&self, files: &FileIds) -> u64 {
        let mut h = Xxh3::new();
        h.update(&self.h.0.digest().to_le_bytes());
        for m in &self.files {
            h.update(&m.0.to_le_bytes());
            h.update(&files.by_media.get(m).copied().unwrap_or(0).to_le_bytes());
        }
        if self.all_files {
            h.update(&files.all.to_le_bytes());
        }
        h.digest()
    }
}

/// The plan's hash as the executor's plan key takes it, without what a process numbers (D1): the
/// frames' content serials and the input frame's presence and size (both follow from the address,
/// the index and the step). A new field a plan key hashes is hashed here too (t2981).
pub fn durable_hash(plan: &FramePlan<'_>) -> u64 {
    let mut h = Xxh3::new();
    h.update(&FORMAT.to_le_bytes());
    put_plan(&mut h, plan);
    h.digest()
}

/// Debug text streamed into the hash, no string built.
fn put_debug(h: &mut Xxh3, v: &dyn std::fmt::Debug) {
    struct Into<'h>(&'h mut Xxh3);
    impl std::fmt::Write for Into<'_> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0.update(s.as_bytes());
            Ok(())
        }
    }
    let _ = std::fmt::Write::write_fmt(&mut Into(h), format_args!("{v:?}"));
}

fn put_mix(h: &mut Xxh3, m: &crate::TransMix) {
    match crate::transitions::plugin_id(m.mode) {
        Some(id) => {
            h.update(&[1u8]);
            h.update(id.as_bytes());
            h.update(&[0u8]);
        }
        None => h.update(&m.mode.to_le_bytes()),
    }
    for v in m.p {
        h.update(&v.to_bits().to_le_bytes());
    }
}

fn put_src(h: &mut Xxh3, s: &LayerSrc<'_>) {
    h.update(&s.source.0.to_le_bytes());
    h.update(&s.cache_idx.to_le_bytes());
    match &s.blend {
        None => h.update(&[0u8]),
        Some(b) => {
            h.update(&[1u8]);
            h.update(&b.cache_idx.to_le_bytes());
            h.update(&b.k.to_le_bytes());
        }
    }
    h.update(&s.fx_hash.to_le_bytes());
    if !s.masks.is_empty() {
        put_debug(h, &s.masks);
    }
    h.update(&s.src_scale.to_le_bytes());
    h.update(&s.u.to_bits().to_le_bytes());
    put_debug(h, &s.cam3d);
    put_debug(h, &s.lead);
    h.update(&[s.blend_mode as u8]);
    match &s.input {
        LayerInput::Frame(_) => h.update(&[1u8]),
        LayerInput::Nest(n) => {
            h.update(&[2u8]);
            h.update(&n.seq.0.to_le_bytes());
            h.update(&[n.blend as u8]);
            h.update(&n.nest_scale.to_le_bytes());
            h.update(&n.content_hash.to_le_bytes());
            h.update(&n.inner_t.0.to_le_bytes());
            put_plan(h, &n.plan);
        }
    }
}

fn put_plan(h: &mut Xxh3, plan: &FramePlan<'_>) {
    h.update(&plan.t.0.to_le_bytes());
    h.update(&plan.out_w.to_le_bytes());
    h.update(&plan.out_h.to_le_bytes());
    for layer in &plan.layers {
        match layer {
            PlanLayer::One { src, alpha_k, scale_alpha, fade } => {
                h.update(&[1u8, *scale_alpha as u8]);
                h.update(&alpha_k.to_le_bytes());
                match fade {
                    None => h.update(&[0u8]),
                    Some(f) => {
                        h.update(&[1u8, f.incoming as u8]);
                        h.update(&f.k.to_le_bytes());
                        put_mix(h, &f.mix);
                    }
                }
                put_src(h, src);
            }
            PlanLayer::Dissolve { a, b, k, alpha_k, mix } => {
                h.update(&[2u8]);
                h.update(&k.to_le_bytes());
                h.update(&alpha_k.to_le_bytes());
                put_mix(h, mix);
                put_src(h, a);
                put_src(h, b);
            }
        }
    }
}

/// The key on disk: the plan's durable hash, its leaves and the quality it was asked at.
pub fn disk_key(plan: &FramePlan<'_>, leaves: &Leaves, files: &FileIds) -> u64 {
    let mut h = Xxh3::new();
    h.update(&durable_hash(plan).to_le_bytes());
    h.update(&leaves.digest(files).to_le_bytes());
    h.digest()
}

/// The key the display's memo holds a restored moment under: its present key mixed with the durable
/// one (D3) — a probe plan of an undecoded moment carries serial 0, and a memo entry under that key
/// alone could be shown after the file under the same media id was replaced.
pub fn restored_key(present: u64, durable: u64) -> u64 {
    present ^ durable.rotate_left(17).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5245_5354_4f52_4544
}

// ---- what to prerender (D4) --------------------------------------------------------------------

/// The moments the prerender should compose, in order (D4): the moments measured at least `heavy_ns`
/// and not `done`, and the unmeasured neighbours of such moments that have a picture (`has_layer`) and
/// are not `done`; those inside the loop `region` first, then by their distance to `head`.
pub fn candidates(
    costs: &BTreeMap<i64, u64>,
    step: i64,
    heavy_ns: u64,
    done: impl Fn(i64) -> bool,
    has_layer: impl Fn(i64) -> bool,
    region: Option<(i64, i64)>,
    head: i64,
) -> Vec<i64> {
    let step = step.max(1);
    let mut out = BTreeSet::new();
    for (&t, &ns) in costs {
        if ns < heavy_ns {
            continue;
        }
        if !done(t) {
            out.insert(t);
        }
        for n in [t - step, t + step] {
            if n >= 0 && !costs.contains_key(&n) && !done(n) && has_layer(n) {
                out.insert(n);
            }
        }
    }
    let mut v: Vec<i64> = out.into_iter().collect();
    let inside = |t: i64| region.is_some_and(|(s, e)| s <= t && t < e);
    v.sort_by_key(|&t| (!inside(t), (t - head).abs(), t));
    v
}

// ---- the player's tier ---------------------------------------------------------------------------

/// What the host sets (D7).
#[derive(Clone, Debug)]
pub struct Config {
    /// `<data>/render-cache`.
    pub root: PathBuf,
    /// The program's build and anything else that makes another picture of the same plan (the
    /// FFmpeg major): a new build's folder replaces the old.
    pub build: String,
    /// The project file's folder: relative media paths resolve against it.
    pub base: Option<PathBuf>,
    pub cap: u64,
}

/// The player's disk tier: its configuration, the store of the display's device, the files'
/// identities, the moments on disk for the bars, the input stamp and the prerender's wake.
#[derive(Default)]
pub struct RenderCache {
    cfg: Mutex<Option<Config>>,
    store: Mutex<Option<(String, Arc<DiskStore>)>>,
    files: Mutex<Option<(u64, u64, Arc<FileIds>)>>,
    /// Moments the store holds: tick → (durable key, the versions it was computed at).
    on_disk: Mutex<BTreeMap<i64, (u64, (u64, u64))>>,
    pub(crate) disk_gen: AtomicU64,
    /// The last input's trace clock (ns); 0 — none yet.
    last_input: AtomicU64,
    /// The prerender's park and its quiet wait.
    pub(crate) wake: crate::player::Wake,
    /// A heavy moment was measured since the prerender last looked.
    pub(crate) news: AtomicBool,
    /// Writes and restores so far (tests, the bench).
    pub(crate) stores: AtomicU64,
    pub(crate) restores: AtomicU64,
    /// The prerender is working on a moment now (tests).
    pub(crate) busy: AtomicBool,
    /// A test's threshold of "heavy" in ns over the rule's half period (0 — the rule).
    heavy_for_tests: AtomicU64,
}

impl RenderCache {
    pub fn set_config(&self, cfg: Option<Config>) {
        *self.cfg.lock().unwrap() = cfg.filter(|_| enabled());
        *self.store.lock().unwrap() = None;
        *self.files.lock().unwrap() = None;
        self.on_disk.lock().unwrap().clear();
        self.disk_gen.fetch_add(1, Ordering::Relaxed);
        self.news.store(true, Ordering::Release);
        self.wake.notify();
    }

    pub fn is_on(&self) -> bool {
        self.cfg.lock().unwrap().is_some()
    }

    /// The store for the display's device, opened at the first ask.
    pub fn store(&self, device: &crate::video_plan::GpuBackendInfo) -> Option<Arc<DiskStore>> {
        let cfg = self.cfg.lock().unwrap().clone()?;
        let id = format!("{}|{}", device.name, device.backend);
        let mut g = self.store.lock().unwrap();
        if let Some((d, s)) = g.as_ref()
            && *d == id
        {
            return Some(s.clone());
        }
        match DiskStore::open(&cfg.root, &cfg.build, &id, cfg.cap) {
            Ok(s) => {
                let s = Arc::new(s);
                *g = Some((id, s.clone()));
                Some(s)
            }
            Err(_) => None,
        }
    }

    /// The identities of `project`'s files, read again when the state (`stamp`: build, content
    /// version) moved or `refresh` asks (the prerender at its start: a file edited outside).
    pub fn files(&self, project: &kt_model::Project, stamp: (u64, u64), refresh: bool) -> Arc<FileIds> {
        let base = self.cfg.lock().unwrap().as_ref().and_then(|c| c.base.clone());
        let mut g = self.files.lock().unwrap();
        if !refresh
            && let Some((a, b, f)) = g.as_ref()
            && (*a, *b) == stamp
        {
            return f.clone();
        }
        let f = Arc::new(FileIds::read(&project.media, base.as_deref()));
        *g = Some((stamp.0, stamp.1, f.clone()));
        f
    }

    /// The person acted (D4): an atomic stamp, no notify.
    pub fn note_input(&self) {
        self.last_input.store(trace::now_ns().max(1), Ordering::Release);
    }

    /// How long until the quiet is reached; `None` — it is.
    pub fn quiet_left(&self) -> Option<Duration> {
        let last = self.last_input.load(Ordering::Acquire);
        if last == 0 {
            return None;
        }
        let since = trace::now_ns().saturating_sub(last);
        let q = QUIET.as_nanos() as u64;
        (since < q).then(|| Duration::from_nanos(q - since))
    }

    pub(crate) fn input_stamp(&self) -> u64 {
        self.last_input.load(Ordering::Acquire)
    }

    /// The moment `t` is on disk under `key` for `versions`.
    pub(crate) fn note_on_disk(&self, t: i64, key: u64, versions: (u64, u64)) {
        if self.on_disk.lock().unwrap().insert(t, (key, versions)) != Some((key, versions)) {
            self.disk_gen.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The moments on disk for `versions`.
    pub(crate) fn disk_moments(&self, versions: (u64, u64)) -> Vec<i64> {
        self.on_disk.lock().unwrap().iter().filter(|(_, (_, v))| *v == versions).map(|(t, _)| *t).collect()
    }

    pub(crate) fn knows(&self, t: i64, versions: (u64, u64)) -> bool {
        self.on_disk.lock().unwrap().get(&t).is_some_and(|(_, v)| *v == versions)
    }

    /// Writes and restores so far.
    pub fn counts(&self) -> (u64, u64) {
        (self.stores.load(Ordering::Relaxed), self.restores.load(Ordering::Relaxed))
    }

    /// Whether the prerender is working on a moment now.
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    /// Look for work now (a test that measured its moments by hand; the host after a setting).
    pub fn kick(&self) {
        self.news.store(true, Ordering::Release);
        self.wake.notify();
    }

    /// Tests: "heavy" is `ns` and more instead of half a frame period (`None` — the rule). A software
    /// rasterizer composes a test's small picture far under any period.
    pub fn set_heavy_for_tests(&self, ns: Option<u64>) {
        self.heavy_for_tests.store(ns.unwrap_or(0), Ordering::Release);
    }

    /// The threshold of a heavy moment for a grid step (D4).
    pub(crate) fn heavy(&self, step_ticks: i64) -> u64 {
        match self.heavy_for_tests.load(Ordering::Acquire) {
            0 => heavy_ns(step_ticks),
            ns => ns,
        }
    }
}

/// Half a frame period of a grid step in ticks, in ns: a moment that takes the device that long cannot
/// keep real time beside the display, the stream history and the decode (D4).
pub fn heavy_ns(step_ticks: i64) -> u64 {
    step_ticks.max(1) as u64 * 1_000_000_000 / kt_time::TICK_HZ as u64 / 2
}

/// An export holds the pausable classes (its vote): the prerender waits it out.
fn export_runs() -> bool {
    kt_time::sched::paused() & kt_time::sched::why::EXPORT != 0
}

/// What became of a moment the prerender took.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Written,
    Light,
    OnDisk,
    Full,
    Stopped(u64),
    Incomplete,
    NoExec,
}

/// The state the prerender's knowledge belongs to: the build, the versions, the quality, the region.
type StateKey = (u64, (u64, u64), kt_media::Quality, Option<(i64, i64)>);

/// The player's `render-cache` thread (D4): parks with no clock while there is nothing to do; at rest,
/// after the quiet, composes the moments [`candidates`] names one at a time and writes the heavy ones.
pub(crate) fn run_prerender(parts: crate::player::PreviewParts, stop: Arc<std::sync::atomic::AtomicBool>) {
    use std::collections::HashSet;
    let rc = parts.rc.clone();
    let mut done: HashSet<i64> = HashSet::new();
    let mut tries: HashMap<i64, u32> = HashMap::new();
    let mut state: Option<StateKey> = None;
    let mut scanned: Option<(StateKey, i64)> = None;
    let mut worked = false;
    let mut full = false;
    loop {
        let seen = rc.wake.seen();
        if stop.load(Ordering::Acquire) {
            return;
        }
        rc.news.store(false, Ordering::Release);
        if !rc.is_on() || parts.transport.is_playing() || export_runs() {
            if worked {
                let why = if parts.transport.is_playing() { rc::why::PLAY } else { rc::why::EXPORT };
                trace::ev(Ev::RenderCache).aux(rc::STOPPED).b(why).emit();
                worked = false;
            }
            rc.wake.wait_since(seen);
            continue;
        }
        let Some(display) = parts.display_exec() else {
            rc.wake.wait_since(seen);
            continue;
        };
        let Some(store) = rc.store(display.info()) else {
            rc.wake.wait_since(seen);
            continue;
        };
        let built = parts.builds.latest();
        if built.seq < parts.builds.want_seq() {
            // a build in flight: its version's moments are the ones to make (it lands within a quiet)
            rc.wake.wait_timeout(QUIET);
            continue;
        }
        let video = built.video.clone();
        let q = parts.quality();
        let versions = parts.versions();
        let region = parts
            .loop_region
            .lock()
            .unwrap()
            .map(|(s, e)| (kt_time::Tick::from_audio_frames(s).0, kt_time::Tick::from_audio_frames(e).0));
        let sk = (built.seq, versions, q, region);
        if state != Some(sk) {
            state = Some(sk);
            done.clear();
            tries.clear();
            full = false;
        }
        let step = video.grid.frame_step().max(1);
        let head = parts.transport.playhead().0;
        // the bars' scan: once per state and per five seconds of the playhead's place (no device)
        let bucket = head.div_euclid(5 * kt_time::TICK_HZ as i64);
        if scanned != Some((sk, bucket)) {
            scanned = Some((sk, bucket));
            scan(&parts, &rc, &store, &display, &video, &built.project, (built.seq, versions), q, region, head);
        }
        if full {
            rc.wake.wait_since(seen);
            continue;
        }
        let costs = parts.preview_costs.lock().unwrap().snapshot();
        let list = candidates(
            &costs,
            step,
            rc.heavy(step),
            |t| done.contains(&t) || rc.knows(t, versions),
            |t| !video.layers_at(kt_time::Tick(t), q).is_empty(),
            region,
            head,
        );
        let Some(&t) = list.first() else {
            if worked {
                trace::ev(Ev::RenderCache).aux(rc::STOPPED).b(rc::why::DONE).emit();
                worked = false;
            }
            rc.wake.wait_since(seen);
            continue;
        };
        if let Some(left) = rc.quiet_left() {
            rc.wake.wait_timeout(left);
            continue;
        }
        worked = true;
        rc.busy.store(true, Ordering::Release);
        let out = one(&parts, &rc, &store, &display, &video, &built.project, built.seq, t, q, versions, &stop);
        rc.busy.store(false, Ordering::Release);
        match out {
            Outcome::Written | Outcome::Light | Outcome::OnDisk | Outcome::NoExec => {
                done.insert(t);
            }
            Outcome::Full => {
                full = true;
                trace::ev(Ev::RenderCache).aux(rc::STOPPED).b(rc::why::CAP).emit();
                worked = false;
            }
            Outcome::Stopped(why) => {
                trace::ev(Ev::RenderCache).aux(rc::ABANDON).a(t as u64).b(why).emit();
            }
            Outcome::Incomplete => {
                let n = tries.entry(t).or_insert(0);
                *n += 1;
                if *n >= 3 {
                    trace::ev(Ev::RenderCache).aux(rc::ABANDON).a(t as u64).b(rc::why::INCOMPLETE).emit();
                    done.insert(t);
                }
            }
        }
    }
}

/// The disk moments of the loop region (or five seconds round the playhead) for the bars (D3): durable
/// keys asked of the index, no device and no decode; only when the store holds anything.
#[allow(clippy::too_many_arguments)]
fn scan(
    parts: &crate::player::PreviewParts,
    rc: &RenderCache,
    store: &DiskStore,
    display: &Arc<dyn crate::video_plan::VideoGpuExec>,
    video: &crate::video::VideoRenderer,
    project: &kt_model::Project,
    stamp: (u64, (u64, u64)),
    q: kt_media::Quality,
    region: Option<(i64, i64)>,
    head: i64,
) {
    const MAX: usize = 1200;
    if store.used().1 == 0 {
        return;
    }
    let step = video.grid.frame_step().max(1);
    let (a, b) = region.unwrap_or((head - 5 * kt_time::TICK_HZ as i64, head + 5 * kt_time::TICK_HZ as i64));
    let files = rc.files(project, (stamp.0, stamp.1.1), false);
    let mut t = video.grid.align_floor(kt_time::Tick(a.max(0))).0;
    let (mut looked, mut found) = (0u64, 0u64);
    let input = rc.input_stamp();
    while t < b && (looked as usize) < MAX {
        if parts.transport.is_playing() || rc.input_stamp() != input {
            break;
        }
        if !video.layers_at(kt_time::Tick(t), q).is_empty() {
            looked += 1;
            let (plan, leaves) = video.probe_durable(&parts.cache, kt_time::Tick(t), q);
            if display.present_key(&plan).is_some() {
                let dk = disk_key(&plan, &leaves, &files);
                if store.contains(dk) {
                    rc.note_on_disk(t, dk, stamp.1);
                    found += 1;
                }
            }
        }
        t += step;
    }
    trace::ev(Ev::RenderCache).aux(rc::SCAN).a(looked).b(found).emit();
}

/// Prerender the moment `t` (D4): decode its layers at the lowest priority, advance its stream stages,
/// compose it at class `background`, measure it, write it when heavy. Stops at any input, Play, an
/// export or the close.
#[allow(clippy::too_many_arguments)]
fn one(
    parts: &crate::player::PreviewParts,
    rc: &Arc<RenderCache>,
    store: &DiskStore,
    display: &Arc<dyn crate::video_plan::VideoGpuExec>,
    video: &Arc<crate::video::VideoRenderer>,
    project: &kt_model::Project,
    seq: u64,
    t: i64,
    q: kt_media::Quality,
    versions: (u64, u64),
    stop: &Arc<std::sync::atomic::AtomicBool>,
) -> Outcome {
    use kt_time::Tick;
    let files = rc.files(project, (seq, versions.1), false);
    let (dk, w, h) = {
        let (plan, leaves) = video.probe_durable(&parts.cache, Tick(t), q);
        (disk_key(&plan, &leaves, &files), plan.out_w, plan.out_h)
    };
    if store.contains(dk) {
        rc.note_on_disk(t, dk, versions);
        return Outcome::OnDisk;
    }
    if !store.has_room(w as u64 * h as u64 * 2) {
        return Outcome::Full;
    }
    let Some(exec) = parts.ahead.exec_for(display) else { return Outcome::NoExec };
    let stamp = rc.input_stamp();
    let check = {
        let (stop, transport, rc) = (stop.clone(), parts.transport.clone(), rc.clone());
        move || stop.load(Ordering::Acquire) || transport.is_playing() || rc.input_stamp() != stamp || export_runs()
    };
    let why = || {
        if stop.load(Ordering::Acquire) {
            rc::why::CLOSED
        } else if parts.transport.is_playing() {
            rc::why::PLAY
        } else if export_runs() {
            rc::why::EXPORT
        } else {
            rc::why::INPUT
        }
    };
    // its device work goes behind every present and every preview piece (the rings admit by class)
    let _class = kt_time::sched::with_class(kt_time::sched::Class::Background);
    let id = trace::next_id();
    let _cause = trace::with_cause(id);
    let mut span = trace::span(Ev::Job);
    span.aux(trace::job::PRERENDER).id(id).a(t as u64);
    let at = std::time::Instant::now();
    let mut buf = Vec::new();
    let mut moved = (kt_plugin_api::stream::positions_walked(), parts.cache.video_inserted());
    let mut since = std::time::Instant::now();
    let (w, h) = loop {
        if check() {
            span.c(2);
            return Outcome::Stopped(why());
        }
        let missing = parts.request_layers(video, Tick(t), q, crate::Priority::Thumbnail);
        if missing == 0 {
            if video.has_stream_stages() {
                crate::video::stop_when(trace::stop::PRERENDER, check.clone(), || video.stream_advance(&parts.cache, Tick(t), q));
            }
            let out = crate::video::stop_when(trace::stop::PRERENDER, check.clone(), || {
                let plan = video.frame_plan(&parts.cache, Tick(t), q);
                (exec.execute_into(&plan, &mut buf), plan.out_w, plan.out_h)
            });
            match out {
                (Ok(true), w, h) if buf.len() == w as usize * h as usize * 4 => break (w, h),
                (Ok(_), _, _) => {}
                (Err(_), _, _) => {
                    span.c(2);
                    return Outcome::Incomplete;
                }
            }
        }
        // waiting for a decode or a walk: give up when nothing moved for three seconds
        let now = (kt_plugin_api::stream::positions_walked(), parts.cache.video_inserted());
        if now != moved {
            moved = now;
            since = std::time::Instant::now();
        } else if since.elapsed() > Duration::from_secs(3) {
            span.c(2);
            return Outcome::Incomplete;
        }
        std::thread::sleep(Duration::from_millis(4));
    };
    let dev = exec.last_device_ns();
    let ns = if dev > 0 { dev } else { at.elapsed().as_nanos() as u64 };
    parts.preview_costs.lock().unwrap().note(Tick(t), ns, (w, h));
    span.b(ns);
    let step = video.grid.frame_step();
    if ns < rc.heavy(step) {
        span.c(0);
        trace::ev(Ev::RenderCache).aux(rc::LIGHT).a(t as u64).b(ns).c(dk).emit();
        return Outcome::Light;
    }
    trace::ev(Ev::RenderCache).aux(rc::HEAVY).a(t as u64).b(ns).c(dk).emit();
    match store.put(dk, w, h, &buf, false) {
        Ok(n) => {
            span.c(1);
            trace::ev(Ev::RenderCache).aux(rc::STORE).a(t as u64).b(n).c(dk).emit();
            rc.stores.fetch_add(1, Ordering::Relaxed);
            rc.note_on_disk(t, dk, versions);
            Outcome::Written
        }
        Err(PutError::Full) => Outcome::Full,
        Err(PutError::Io(_)) => Outcome::Incomplete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pic(seed: u8, w: u32, h: u32) -> Vec<u8> {
        (0..w * h * 4).map(|i| (i as u8).wrapping_mul(seed).wrapping_add(seed)).collect()
    }

    /// Test t2980 (render-cache-disk D2): the store keeps pictures losslessly within its cap, least
    /// recently used out, a touched entry stays; another build's folder is removed at open; a
    /// truncated file is a miss and removed; this process's entries are not evicted for the prerender.
    #[test]
    fn t2980_the_store_keeps_pictures_losslessly_within_its_cap() {
        let root = tempfile::tempdir().unwrap();
        let other = root.path().join(build_dir_name("an older build"));
        std::fs::create_dir_all(other.join("dev")).unwrap();
        std::fs::write(other.join("dev").join("x.ktf"), b"old").unwrap();
        let (w, h) = (32, 16);
        let one = encode(w, h, &pic(3, w, h)).len() as u64;
        let s = DiskStore::open(root.path(), "build A", "llvmpipe|vulkan", one * 3 + one / 2).unwrap();
        assert!(!other.exists(), "the other build's folder is removed at open");
        for k in 1..=3u64 {
            s.put(k, w, h, &pic(k as u8, w, h), true).unwrap();
        }
        assert_eq!(s.get(2).unwrap(), Picture { w, h, rgba: pic(2, w, h) }, "lossless");
        assert_eq!(s.get(1).unwrap().rgba, pic(1, w, h));
        // touched 2 then 1: 3 is the least recently used
        s.put(4, w, h, &pic(4, w, h), true).unwrap();
        assert!(!s.contains(3) && s.contains(1) && s.contains(2) && s.contains(4), "the least recently used goes first");
        assert!(s.used().0 <= s.cap());
        // the prerender never evicts what this process used
        assert_eq!(s.put(5, w, h, &pic(5, w, h), false), Err(PutError::Full));
        assert!(!s.has_room(one));
        drop(s);
        // a second process: the entries are there; a truncated file is a miss and removed
        let s = DiskStore::open(root.path(), "build A", "llvmpipe|vulkan", one * 3 + one / 2).unwrap();
        assert_eq!(s.used().1, 3);
        assert!(s.has_room(one), "the previous process's entries may go for the prerender");
        let f = s.dir().join(format!("{}.{EXT}", hex(4)));
        let b = std::fs::read(&f).unwrap();
        std::fs::write(&f, &b[..b.len() / 2]).unwrap();
        assert!(s.get(4).is_none() && !f.exists() && !s.contains(4), "a truncated file is a miss and removed");
        assert_eq!(s.get(2).unwrap().rgba, pic(2, w, h));
        // another device has its own folder
        let d = DiskStore::open(root.path(), "build A", "radv|vulkan", one * 4).unwrap();
        assert_eq!(d.used().1, 0);
        assert!(decode(b"KTRC").is_none() && decode(&b[..HEADER]).is_none());
    }

    /// Test t2983 (render-cache-disk D4): the candidates are the heavy moments not done and their
    /// unmeasured neighbours with a picture, the loop region's first, then by distance to the head; a
    /// light moment and what lies past it are not candidates.
    #[test]
    fn t2983_the_prerender_picks_what_was_measured_slow() {
        let step = 10;
        let heavy = 1000;
        // heavy 100, 110; light 120; unmeasured 90, 130; heavy 300 (done); heavy 500
        let costs: BTreeMap<i64, u64> = [(100, 2000), (110, 1500), (120, 10), (300, 5000), (500, 1000)].into();
        let done = |t: i64| t == 300;
        let has = |t: i64| t != 510;
        let v = candidates(&costs, step, heavy, done, has, None, 480);
        // 300 is on disk, its unmeasured neighbours are still measured; 120 is light, 130 lies past it,
        // 510 has no picture
        assert_eq!(v, vec![490, 500, 310, 290, 110, 100, 90], "nearest the head");
        let v = candidates(&costs, step, heavy, done, has, Some((90, 115)), 480);
        assert_eq!(&v[..3], &[110, 100, 90], "the loop region's first");
        assert!(candidates(&costs, step, 1_000_000, done, has, None, 0).is_empty(), "nothing heavy: nothing");
    }

    #[test]
    fn the_filter_round_trips_any_bytes() {
        let raw: Vec<u8> = (0..4096u32).map(|i| (i * 7919 % 251) as u8).collect();
        assert_eq!(decode(&encode(32, 32, &raw)).unwrap().rgba, raw);
    }
}
