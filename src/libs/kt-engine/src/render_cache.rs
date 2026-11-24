//! The disk tier of preview composites (render-cache-disk; the owner, kkt-26.11.17 S5 "Сам в фоне";
//! review R3). A slow moment, once composed, is kept on disk and played from it later — the next lap,
//! after an eviction, after a restart — with no decode, no stream stage and no chain.
//!
//! - **The key** (D1; project-cache-folder D1b): [`DiskKey`] — [`durable_hash`] of the probe plan, every
//!   field the executor's plan key hashes but the frames' per-process content serials, mixed with what
//!   those serials stood for, collected while the plan is built ([`Leaves`]): a file's path, size and
//!   modification time, a generator's values, every file of the project for a stream stage. Its salt is
//!   what made the pixels ([`Made`]): the decoder's major, the plugins that ran, the composite code
//!   outside them. Not the build: an update keeps what it did not change.
//! - **The store** (D2): [`DiskStore`], segments of up to [`SEGMENT_FRAMES`] moments, each a video
//!   written by the host's [`SegmentCodec`] (a hardware encoder) with a manifest beside it, in the
//!   project's cache folder under `results/v<FORMAT>/<device class>/frames/`, written in that folder's
//!   cycle. A frame of another salt is a miss, and its segment goes when it is met.
//! - **The reads** (D3) are the player's (`PreviewParts::restore_from_disk`), before anything composes.
//! - **The prerender** (D4) is the player's `render-cache` thread ([`run_prerender`]): at rest, after
//!   [`QUIET`] with no input, the moments the cost map measured at least half a frame period
//!   ([`candidates`]), stopped by any input, Play, an export or the close.
//!
//! No export reads or writes it (D5): the export's bytes never depend on what the preview kept.

use crate::video_plan::{FramePlan, LayerInput, LayerSrc, PlanLayer};
use kt_media::cache_folder::{Folder, Tenant};
pub use kt_media::segment::{RawSegments, SegmentCodec, SegmentReader, SegmentWriter};
use kt_model::MediaId;
use kt_plugin_api::Family;
use kt_time::trace::{self, render_cache as rc, Ev};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use xxhash_rust::xxh3::Xxh3;

/// The layout's version: a change of the manifest, the folders or the key's fields moves it.
pub const FORMAT: u32 = 3;
/// How long no input must have come before the prerender works (D4: FCP's trigger, shorter).
pub const QUIET: Duration = Duration::from_millis(1500);
/// The most frames one segment holds (ten seconds at 25 fps): the eviction's grain. A small folder's
/// segments are shorter (`DiskStore::segment_frames`).
pub const SEGMENT_FRAMES: usize = 250;
/// Readers kept open by the reader thread.
const READERS: usize = 4;
const MANIFEST: &str = "idx";
const MANIFEST_HEAD: &str = "kinetren render cache segment";

/// `KT_RENDER_CACHE=off`: no tier whatever the host set (an A/B).
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("KT_RENDER_CACHE").as_deref() != Ok("off"))
}

/// `KT_RENDER_CACHE_MB`: a cap of the frames alone (tests, the live check); without it the cache
/// folder's room bounds them.
pub fn cap_from_env() -> Option<u64> {
    std::env::var("KT_RENDER_CACHE_MB").ok().and_then(|v| v.parse::<u64>().ok()).map(|mb| mb << 20)
}

/// Delete `<data>/render-cache`, where the former builds kept the frames of every project
/// (project-cache-folder D5).
pub fn remove_former(data: &Path) {
    let dir = data.join("render-cache");
    if dir.is_dir() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A composite: tightly packed straight RGBA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

fn hex(v: u64) -> String {
    format!("{v:016x}")
}

/// The store's folder inside the cache folder, of one layout and one device class.
fn frames_rel(device: &str) -> String {
    format!("results/v{FORMAT}/{}/frames", hex(xxhash_rust::xxh3::xxh3_64(device.as_bytes())))
}

/// A segment's manifest (D2, D7): a text file beside the video, the codec and the encoder that wrote
/// it, the frame size and rate, and the durable key and salt of every frame in order — serializable on its
/// own, so a later reader (the cached final render, F-EXP-003) can tell what a file holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub codec: String,
    pub encoder: String,
    pub w: u32,
    pub h: u32,
    pub fps: u32,
    pub keys: Vec<DiskKey>,
}

impl Manifest {
    pub fn to_text(&self) -> String {
        let mut s = format!(
            "{MANIFEST_HEAD} v{FORMAT}\ncodec {}\nencoder {}\nsize {} {}\nfps {}\n",
            self.codec, self.encoder, self.w, self.h, self.fps
        );
        for k in &self.keys {
            s.push_str(&format!("key {} {}\n", hex(k.key), hex(k.salt)));
        }
        s
    }

    pub fn parse(text: &str) -> Option<Manifest> {
        let mut lines = text.lines();
        if lines.next()? != format!("{MANIFEST_HEAD} v{FORMAT}") {
            return None;
        }
        let mut m = Manifest { codec: String::new(), encoder: String::new(), w: 0, h: 0, fps: 0, keys: Vec::new() };
        for l in lines {
            let (k, v) = l.split_once(' ')?;
            match k {
                "codec" => m.codec = v.into(),
                "encoder" => m.encoder = v.into(),
                "size" => {
                    let (a, b) = v.split_once(' ')?;
                    m.w = a.parse().ok()?;
                    m.h = b.parse().ok()?;
                }
                "fps" => m.fps = v.parse().ok()?,
                "key" => {
                    let (a, b) = v.split_once(' ')?;
                    m.keys.push(DiskKey { key: u64::from_str_radix(a, 16).ok()?, salt: u64::from_str_radix(b, 16).ok()? });
                }
                _ => {}
            }
        }
        (m.w > 0 && m.h > 0 && !m.keys.is_empty()).then_some(m)
    }
}

struct Seg {
    bytes: u64,
    /// The use order: larger is newer; segments found at open are numbered by their file times,
    /// all below `session`.
    last: u64,
    w: u32,
    h: u32,
    keys: Vec<DiskKey>,
}

#[derive(Default)]
struct Index {
    segs: HashMap<u64, Seg>,
    /// Key → (segment, frame, salt).
    keys: HashMap<u64, (u64, u32, u64)>,
    used: u64,
    seq: u64,
    /// Uses above this were made by this process.
    session: u64,
    /// Bytes a frame took in the segments written so far (the room a moment needs).
    frame_bytes: (u64, u64),
}

impl Index {
    fn insert(&mut self, id: u64, seg: Seg) {
        for (i, k) in seg.keys.iter().enumerate() {
            self.keys.insert(k.key, (id, i as u32, k.salt));
        }
        self.used += seg.bytes;
        self.frame_bytes.0 += seg.bytes;
        self.frame_bytes.1 += seg.keys.len() as u64;
        self.segs.insert(id, seg);
    }

    fn remove(&mut self, id: u64) -> Option<Seg> {
        let seg = self.segs.remove(&id)?;
        for k in &seg.keys {
            if self.keys.get(&k.key).is_some_and(|(s, ..)| *s == id) {
                self.keys.remove(&k.key);
            }
        }
        self.used -= seg.bytes;
        Some(seg)
    }
}

/// A request of the reader thread: the segment, its frame, its size, where to answer.
type ReadAsk = (PathBuf, u32, u32, u32, std::sync::mpsc::Sender<Option<Vec<u8>>>);

/// The disk tier's segments of one device class (D2): videos written by the host's [`SegmentCodec`]
/// and their manifests, the frames of the project's cache folder.
pub struct DiskStore {
    folder: Arc<Folder>,
    /// `dir` inside the folder, with `/`.
    rel: String,
    dir: PathBuf,
    /// A cap of the frames alone (`KT_RENDER_CACHE_MB`).
    cap: Option<u64>,
    /// Moves when segments went away: what the bars drew of them is no more.
    gone: Arc<AtomicU64>,
    codec: Arc<dyn SegmentCodec>,
    ix: Mutex<Index>,
    /// The thread that owns every open segment reader (invariant 5: a decoder has one owner).
    reader: Mutex<Option<(std::sync::mpsc::Sender<ReadAsk>, std::thread::JoinHandle<()>)>>,
}

/// Why a write kept nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum PutError {
    /// No room without evicting what this process used (the prerender's rule).
    Full,
    /// No encoder of the codec opens on this machine: the reason (D2 — no hardware encoder, no tier).
    NoEncoder(String),
    Io(String),
}

/// A segment writer's layout in the exact conversion's terms.
fn yuv_of(p: kt_media::segment::SegmentPlanes) -> crate::yuv::YuvLayout {
    use crate::yuv::{YuvLayout, YuvMatrix, YuvPix};
    YuvLayout {
        pix: match p.code {
            0 => YuvPix::Yuv420p,
            1 => YuvPix::Nv12,
            2 => YuvPix::Yuv420p10,
            3 => YuvPix::P010,
            _ => YuvPix::Yuv422p10,
        },
        matrix: if p.bt709 { YuvMatrix::Bt709 } else { YuvMatrix::Bt601 },
        full: p.full,
    }
}

/// A segment being written by the prerender's thread: frames go to the encoder as they come, the
/// manifest is written when it is committed.
pub struct SegmentBuild {
    id: u64,
    writer: Box<dyn SegmentWriter>,
    manifest: Manifest,
    /// The device time its moments took, ns.
    cost: u64,
}

impl SegmentBuild {
    /// A moment of it took the device `ns` to compose: what a read of it saves.
    pub fn cost(&mut self, ns: u64) {
        self.cost += ns;
    }

    pub fn push(&mut self, key: DiskKey, rgba: &[u8]) -> Result<(), String> {
        self.writer.push(rgba)?;
        self.manifest.keys.push(key);
        Ok(())
    }

    /// The planes the segment's encoder takes, as the exact conversion names them (export-host-copy
    /// D4); `None` — it takes RGBA.
    pub fn planes(&self) -> Option<crate::yuv::YuvLayout> {
        self.writer.planes().map(yuv_of)
    }

    /// The next frame as the `planes` layout's packed planes, made on the composing device.
    pub fn push_planes(&mut self, key: DiskKey, planes: &[u8]) -> Result<(), String> {
        self.writer.push_planes(planes)?;
        self.manifest.keys.push(key);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.manifest.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.manifest.keys.is_empty()
    }

    pub fn encoder(&self) -> &str {
        &self.manifest.encoder
    }

    pub fn size(&self) -> (u32, u32) {
        (self.manifest.w, self.manifest.h)
    }
}

impl DiskStore {
    /// Open (create) the frames of `device` in the cache folder and read their manifests (a video with
    /// none, or of another codec, is removed).
    pub fn open(folder: Arc<Folder>, device: &str, cap: Option<u64>, codec: Arc<dyn SegmentCodec>) -> std::io::Result<DiskStore> {
        let rel = frames_rel(device);
        let dir = folder.root().join(&rel);
        std::fs::create_dir_all(&dir)?;
        let ext = codec.extension().to_string();
        let mut found: Vec<(std::time::SystemTime, u64, Manifest, u64)> = Vec::new();
        let mut videos: Vec<PathBuf> = Vec::new();
        for e in std::fs::read_dir(&dir)?.flatten() {
            let p = e.path();
            let x = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_string();
            if x == ext {
                videos.push(p);
                continue;
            }
            let Some(id) = p.file_stem().and_then(|s| s.to_str()).and_then(|s| u64::from_str_radix(s, 16).ok()) else {
                let _ = std::fs::remove_file(&p);
                continue;
            };
            if x != MANIFEST {
                // a temporary file a crash left, a format of another layout
                let _ = std::fs::remove_file(&p);
                continue;
            }
            let video = dir.join(format!("{}.{ext}", hex(id)));
            let m = std::fs::read_to_string(&p).ok().and_then(|t| Manifest::parse(&t)).filter(|m| m.codec == codec.codec());
            match (m, std::fs::metadata(&video)) {
                (Some(m), Ok(md)) => {
                    let t = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                    found.push((t, id, m, md.len()));
                }
                _ => {
                    let _ = std::fs::remove_file(&p);
                    let _ = std::fs::remove_file(&video);
                }
            }
        }
        found.sort_by_key(|f| (f.0, f.1));
        let mut ix = Index::default();
        for (_, id, m, bytes) in found {
            ix.seq += 1;
            let seg = Seg { bytes, last: ix.seq, w: m.w, h: m.h, keys: m.keys };
            ix.insert(id, seg);
        }
        ix.session = ix.seq;
        // a video whose manifest never came (a segment the process died writing)
        for v in videos {
            let id = v.file_stem().and_then(|s| s.to_str()).and_then(|s| u64::from_str_radix(s, 16).ok());
            if id.is_none_or(|id| !ix.segs.contains_key(&id)) {
                let _ = std::fs::remove_file(&v);
            }
        }
        // what the cycle dropped before the index was read is not in it
        let _ = folder.take_dropped(&rel);
        let store = DiskStore { folder, rel, dir, cap, gone: Arc::default(), codec, ix: Mutex::new(ix), reader: Mutex::new(None) };
        // a smaller cap than the store holds: the oldest go now
        store.make_room(0, true);
        Ok(store)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn codec(&self) -> &str {
        self.codec.codec()
    }

    /// The entry of a segment in the folder.
    fn entry(&self, id: u64) -> String {
        format!("{}/{}.{}", self.rel, hex(id), self.codec.extension())
    }

    /// Forget the segments the folder's cycle dropped.
    fn sync(&self) {
        let dropped = self.folder.take_dropped(&self.rel);
        if dropped.is_empty() {
            return;
        }
        let mut ix = self.ix.lock().unwrap();
        for e in dropped {
            let id = e.rsplit('/').next().and_then(|s| u64::from_str_radix(s, 16).ok());
            if let Some(seg) = id.and_then(|id| ix.remove(id)) {
                trace::ev(Ev::RenderCache).aux(rc::EVICT).a(seg.keys.len() as u64).b(seg.bytes).c(id.unwrap()).emit();
            }
        }
        self.gone.fetch_add(1, Ordering::Relaxed);
    }

    fn video(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{}.{}", hex(id), self.codec.extension()))
    }

    fn manifest_path(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{}.{MANIFEST}", hex(id)))
    }

    /// Does the store hold the picture? A frame that others made than what makes it now is a miss, and
    /// its segment goes (project-cache-folder D1b).
    pub fn contains(&self, key: DiskKey) -> bool {
        self.sync();
        self.find(key).is_some()
    }

    /// The same with nothing dropped, for the bars.
    fn holds(&self, key: DiskKey) -> bool {
        self.sync();
        self.ix.lock().unwrap().keys.get(&key.key).is_some_and(|k| k.2 == key.salt)
    }

    fn find(&self, key: DiskKey) -> Option<(u64, u32)> {
        let (id, frame, salt) = *self.ix.lock().unwrap().keys.get(&key.key)?;
        if salt != key.salt {
            self.drop_segment(id);
            return None;
        }
        Some((id, frame))
    }

    /// The bytes the store holds, and the frames.
    pub fn used(&self) -> (u64, usize) {
        let ix = self.ix.lock().unwrap();
        (ix.used, ix.keys.len())
    }


    /// The manifest of the segment that holds `key` (tests, diagnostics).
    pub fn manifest_of(&self, key: DiskKey) -> Option<Manifest> {
        let id = self.find(key)?.0;
        std::fs::read_to_string(self.manifest_path(id)).ok().and_then(|t| Manifest::parse(&t))
    }

    /// The picture under `key`, decoded by the reader thread; a segment that does not read back is
    /// removed and is a miss.
    pub fn get(&self, key: DiskKey) -> Option<Picture> {
        self.sync();
        let Some((id, frame)) = self.find(key) else {
            self.folder.missed(Tenant::Frames);
            return None;
        };
        let (w, h) = {
            let mut ix = self.ix.lock().unwrap();
            ix.seq += 1;
            let s = ix.seq;
            let seg = ix.segs.get_mut(&id)?;
            seg.last = s;
            (seg.w, seg.h)
        };
        let (tx, rx) = std::sync::mpsc::channel();
        self.ask((self.video(id), frame, w, h, tx))?;
        match rx.recv().ok().flatten() {
            Some(rgba) => {
                self.folder.used(&self.entry(id));
                Some(Picture { w, h, rgba })
            }
            None => {
                self.drop_segment(id);
                None
            }
        }
    }

    fn ask(&self, a: ReadAsk) -> Option<()> {
        let mut g = self.reader.lock().unwrap();
        if g.is_none() {
            let (tx, rx) = std::sync::mpsc::channel::<ReadAsk>();
            let codec = self.codec.clone();
            let t = std::thread::Builder::new()
                .name("render-cache-read".into())
                .spawn(move || read_loop(codec, rx))
                .ok()?;
            *g = Some((tx, t));
        }
        g.as_ref()?.0.send(a).ok()
    }

    fn drop_segment(&self, id: u64) {
        let gone = self.ix.lock().unwrap().remove(id);
        if gone.is_some() {
            let _ = std::fs::remove_file(self.manifest_path(id));
            let _ = std::fs::remove_file(self.video(id));
            self.folder.gone(&self.entry(id));
            self.gone.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Room for `bytes` more, in the frames' own cap and then in the folder's cycle; `any` — this
    /// process's segments may go too. `false` — no room.
    fn make_room(&self, bytes: u64, any: bool) -> bool {
        let mut gone = Vec::new();
        let ok = match self.cap {
            None => true,
            Some(cap) => {
                let mut ix = self.ix.lock().unwrap();
                while ix.used + bytes > cap {
                    let session = ix.session;
                    let oldest = ix.segs.iter().filter(|(_, s)| any || s.last <= session).min_by_key(|(_, s)| s.last).map(|(k, _)| *k);
                    let Some(id) = oldest else { break };
                    let s = ix.remove(id).unwrap();
                    gone.push((id, s.bytes, s.keys.len()));
                }
                ix.used + bytes <= cap
            }
        };
        if !gone.is_empty() {
            self.gone.fetch_add(1, Ordering::Relaxed);
        }
        for (id, b, n) in gone {
            let _ = std::fs::remove_file(self.manifest_path(id));
            let _ = std::fs::remove_file(self.video(id));
            self.folder.gone(&self.entry(id));
            trace::ev(Ev::RenderCache).aux(rc::EVICT).a(n as u64).b(b).c(id).emit();
        }
        if !ok || bytes == 0 {
            return ok;
        }
        let ok = self.folder.ask_room(bytes);
        self.sync();
        ok
    }

    /// The bytes a frame took on average so far (a guess of `w`×`h`/8 before anything was written).
    pub fn frame_bytes(&self, w: u32, h: u32) -> u64 {
        let ix = self.ix.lock().unwrap();
        match ix.frame_bytes {
            (b, n) if n > 0 => b / n,
            _ => w as u64 * h as u64 / 8,
        }
    }

    /// How many frames a segment takes: an eighth of all the store may hold at most, so a small folder
    /// is written in a cycle too, a segment in another's place.
    fn segment_frames(&self, w: u32, h: u32) -> usize {
        let all = self.cap.unwrap_or(u64::MAX).min(self.folder.cap());
        ((all / 8 / self.frame_bytes(w, h).max(1)) as usize).clamp(1, SEGMENT_FRAMES)
    }

    /// Room for `frames` of the segment being written, whose file no count holds yet: what it replaces
    /// goes before the bytes come, not after.
    fn room_for(&self, frames: usize, w: u32, h: u32) -> bool {
        self.make_room(frames as u64 * self.frame_bytes(w, h), false)
    }

    /// Would `bytes` more fit without evicting what this process used?
    pub fn has_room(&self, bytes: u64) -> bool {
        let mine = self.cap.is_none_or(|cap| {
            let ix = self.ix.lock().unwrap();
            let stale: u64 = ix.segs.values().filter(|s| s.last <= ix.session).map(|s| s.bytes).sum();
            ix.used - stale + bytes <= cap
        });
        mine && self.folder.has_room(bytes)
    }

    /// Start a segment of `w`×`h` at `fps`: `NoEncoder` when the codec opens no encoder here.
    pub fn begin(&self, w: u32, h: u32, fps: u32) -> Result<SegmentBuild, PutError> {
        let id = {
            let mut ix = self.ix.lock().unwrap();
            ix.seq += 1;
            // a new name for every segment of this folder: the time and the sequence
            xxhash_rust::xxh3::xxh3_64(format!("{}|{}|{:?}", ix.seq, std::process::id(), std::time::SystemTime::now()).as_bytes())
        };
        // the person may have deleted the folder while the program runs
        std::fs::create_dir_all(&self.dir).map_err(|e| PutError::Io(e.to_string()))?;
        let writer = self.codec.writer(&self.video(id), w, h, fps).map_err(PutError::NoEncoder)?;
        let manifest = Manifest { codec: self.codec.codec().into(), encoder: writer.encoder().into(), w, h, fps, keys: Vec::new() };
        Ok(SegmentBuild { id, writer, manifest, cost: 0 })
    }

    /// Close a segment and keep it: its frames read from now. `evict_session` — what this process
    /// used may be evicted for it (the prerender passes `false`: past the cap the segment is dropped).
    /// The bytes on disk.
    pub fn commit(&self, b: SegmentBuild, evict_session: bool) -> Result<u64, PutError> {
        let SegmentBuild { id, writer, manifest, cost } = b;
        let video = writer.finish().map_err(PutError::Io)?;
        if manifest.keys.is_empty() {
            let _ = std::fs::remove_file(self.video(id));
            return Ok(0);
        }
        let text = manifest.to_text();
        let bytes = video + text.len() as u64;
        if !self.make_room(bytes, evict_session) {
            let _ = std::fs::remove_file(self.video(id));
            // what a frame takes here is known from now: the next segment is cut to the room
            let mut ix = self.ix.lock().unwrap();
            ix.frame_bytes.0 += bytes;
            ix.frame_bytes.1 += manifest.keys.len() as u64;
            return Err(PutError::Full);
        }
        let tmp = self.dir.join(format!("{}.tmp", hex(id)));
        std::fs::write(&tmp, &text).map_err(|e| PutError::Io(e.to_string()))?;
        std::fs::rename(&tmp, self.manifest_path(id)).map_err(|e| PutError::Io(e.to_string()))?;
        let mut ix = self.ix.lock().unwrap();
        ix.seq += 1;
        let last = ix.seq;
        let mean = cost / 1000 / manifest.keys.len() as u64;
        ix.insert(id, Seg { bytes, last, w: manifest.w, h: manifest.h, keys: manifest.keys });
        drop(ix);
        self.folder.wrote(&self.entry(id), bytes, mean);
        Ok(bytes)
    }
}

impl Drop for DiskStore {
    fn drop(&mut self) {
        if let Some((tx, t)) = self.reader.lock().unwrap().take() {
            drop(tx);
            kt_time::shutdown::join("render-cache-read", t, kt_time::shutdown::deadline());
        }
    }
}

/// The reader thread: the open segment readers, the most recently used kept.
fn read_loop(codec: Arc<dyn SegmentCodec>, rx: std::sync::mpsc::Receiver<ReadAsk>) {
    let mut open: Vec<(PathBuf, Box<dyn SegmentReader>)> = Vec::new();
    while let Ok((path, frame, w, h, reply)) = rx.recv() {
        let i = match open.iter().position(|(p, _)| *p == path) {
            Some(i) => i,
            None => match codec.reader(&path) {
                Ok(r) => {
                    // which decoder reads the tier (row 775: the device's, the processor only where
                    // none opens) — once per reader, not per moment
                    let (hw, what) = match r.decoder() {
                        Ok(d) => (1, d),
                        Err(why) => (0, why),
                    };
                    trace::ev(Ev::RenderCache).aux(rc::READER).b(hw).id(trace::name(&what) as u64).emit();
                    if open.len() >= READERS {
                        open.remove(0);
                    }
                    open.push((path.clone(), r));
                    open.len() - 1
                }
                Err(_) => {
                    let _ = reply.send(None);
                    continue;
                }
            },
        };
        let r = open.remove(i);
        let mut out = Vec::new();
        let got = {
            let (p, mut rd) = r;
            let ok = rd.frame(frame, w, h, &mut out).is_ok();
            open.push((p, rd));
            ok
        };
        let _ = reply.send(got.then_some(out));
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
    /// A procedural source drew: the generators join the salt.
    gens: bool,
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
            l.gens |= rev != 0;
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
    // the rows as the curves gave them: a changed curve arithmetic is another picture under the same
    // chain
    put_debug(h, &&*s.fx);
    // the wiring, as `plan_hash` does (fx-graph-dag D3); a line adds nothing
    if !s.fx_in.is_empty() {
        put_debug(h, &s.fx_in);
    }
    for (slot, r) in &s.reads {
        h.update(&(*slot as u64).to_le_bytes());
        put_src(h, r);
    }
    if !s.masks.is_empty() {
        put_debug(h, &s.masks);
    }
    h.update(&s.src_scale.to_le_bytes());
    h.update(&s.u.to_bits().to_le_bytes());
    put_debug(h, &s.cam3d);
    put_debug(h, &s.lead);
    h.update(&[s.blend_mode as u8]);
    match &s.input {
        // a held picture is the frame it stands for (stream-picture-stays D3)
        LayerInput::Frame(_) | LayerInput::Held(_) => h.update(&[1u8]),
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

/// What a moment's picture is kept under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct DiskKey {
    /// What the picture is OF: the plan, its leaves, the quality it was asked at.
    pub key: u64,
    /// What MAKES it ([`Made`]).
    pub salt: u64,
}

impl DiskKey {
    /// Both, as one number of the process's own tables.
    pub fn id(self) -> u64 {
        self.key ^ self.salt.rotate_left(29)
    }
}

/// What makes pixels outside the plan (project-cache-folder D1b).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Made {
    /// The decoder's major (`avcodec_version()`).
    pub decoder: u32,
    /// The composite code outside the plugins' kernels.
    pub composite: u64,
    /// The decode's own conversion of the same file.
    pub pixels: u32,
}

include!(concat!(env!("OUT_DIR"), "/composite_path.rs"));

impl Made {
    /// What this build makes with the decoder of `major`.
    pub fn of(major: u32) -> Made {
        Made { decoder: major, composite: COMPOSITE_PATH_VERSION, pixels: kt_media::index::THUMB_PIXELS_VERSION }
    }

    /// The salt of a plan: with the versions of the plugins whose kernels run in it.
    pub fn salt(&self, plan: &FramePlan<'_>, leaves: &Leaves) -> u64 {
        let mut used: BTreeSet<(u8, &str)> = BTreeSet::new();
        plugins_of(plan, &mut used);
        let reg = kt_model::plugin_slot::registry();
        let mut h = Xxh3::new();
        h.update(&self.decoder.to_le_bytes());
        h.update(&self.composite.to_le_bytes());
        h.update(&self.pixels.to_le_bytes());
        for (f, id) in used {
            let family = if f == 0 { Family::Fx } else { Family::Transition };
            h.update(id.as_bytes());
            h.update(&kernel_version(reg, family, id).to_le_bytes());
        }
        // what drew a leaf is not in the plan: every generator, every stream kernel
        for p in reg.plugins() {
            let m = p.manifest();
            let ran = (leaves.gens && m.media.family() == Family::Generator) || (leaves.all_files && p.stream().is_some());
            if ran {
                h.update(m.id.as_bytes());
                h.update(&kernel_version(reg, m.media.family(), &m.id).to_le_bytes());
            }
        }
        h.digest()
    }
}

/// A plugin's version: the source it was built from and the WGSL text it runs.
fn kernel_version(reg: &kt_plugin_api::Registry, family: Family, id: &str) -> u64 {
    type Known = Mutex<HashMap<(Family, String, u64), u64>>;
    static WGSL: std::sync::OnceLock<Known> = std::sync::OnceLock::new();
    let source = reg.source(family, id);
    let Some(p) = reg.get(family, id) else { return source };
    let of = std::sync::Arc::as_ptr(p) as *const () as u64;
    let mut known = WGSL.get_or_init(Default::default).lock().unwrap();
    let wgsl = *known.entry((family, id.to_string(), of)).or_insert_with(|| {
        let mut h = Xxh3::new();
        match p.manifest().decl.gpu {
            kt_plugin_api::Gpu::Wgsl(t) | kt_plugin_api::Gpu::Warp(t) => h.update(t.as_bytes()),
            _ => {}
        }
        for t in p.barrier().map(|b| b.shaders()).unwrap_or_default() {
            h.update(t.as_bytes());
        }
        for s in p.compute_shaders() {
            h.update(s.text.as_bytes());
        }
        h.digest()
    });
    source.rotate_left(1) ^ wgsl
}

/// The plugins whose kernels run in a plan.
fn plugins_of<'p>(plan: &'p FramePlan<'_>, out: &mut BTreeSet<(u8, &'p str)>) {
    fn src<'p>(s: &'p LayerSrc<'_>, out: &mut BTreeSet<(u8, &'p str)>) {
        for f in s.fx.iter() {
            if let kt_model::FxInstance::Plugin(p) = f {
                out.insert((0, &p.id));
            }
        }
        for (_, r) in &s.reads {
            src(r, out);
        }
        if let LayerInput::Nest(n) = &s.input {
            plugins_of(&n.plan, out);
        }
    }
    let mix = |m: &crate::TransMix, out: &mut BTreeSet<(u8, &'p str)>| {
        if let Some(id) = crate::transitions::plugin_id(m.mode) {
            out.insert((1, id));
        }
    };
    for layer in &plan.layers {
        match layer {
            PlanLayer::One { src: s, fade, .. } => {
                if let Some(f) = fade {
                    mix(&f.mix, out);
                }
                src(s, out);
            }
            PlanLayer::Dissolve { a, b, mix: m, .. } => {
                mix(m, out);
                src(a, out);
                src(b, out);
            }
        }
    }
}

/// The key on disk: the plan's durable hash, its leaves and the quality it was asked at; what makes it.
pub fn disk_key(plan: &FramePlan<'_>, leaves: &Leaves, files: &FileIds, made: &Made) -> DiskKey {
    let mut h = Xxh3::new();
    h.update(&durable_hash(plan).to_le_bytes());
    h.update(&leaves.digest(files).to_le_bytes());
    DiskKey { key: h.digest(), salt: made.salt(plan, leaves) }
}

/// The key the display's memo holds a restored moment under: its present key mixed with the durable
/// one (D3) — a probe plan of an undecoded moment carries serial 0, and a memo entry under that key
/// alone could be shown after the file under the same media id was replaced.
pub fn restored_key(present: u64, durable: DiskKey) -> u64 {
    present ^ durable.id().rotate_left(17).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5245_5354_4f52_4544
}

// ---- what to prerender (D4) --------------------------------------------------------------------

/// The moments the prerender should compose, in order (D4): the moments measured at least `heavy_ns`
/// and not `done`, and the unmeasured neighbours of such moments that have a picture (`has_layer`) and
/// are not `done`; those inside the loop `region` first, then by their distance to `head`.
/// How far on each side of a heavy moment the prerender takes its unmeasured neighbours (found-defects
/// 826): a light one is composed once and ends the run, a heavy one goes into the run's segment.
pub const RUN_REACH: i64 = 8;

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
        // the unmeasured moments around it, up to `RUN_REACH` on each side (found-defects 826): with one
        // neighbour, a run whose costs arrived one at a time (the playhead stepping, a scrub) grew a
        // moment a pass, each a segment of its own — a segment goes on only forward. A measured moment
        // (heavy: its own entry; light: the run's end), one on disk or one with no picture ends the reach
        for dir in [-1i64, 1] {
            for k in 1..=RUN_REACH {
                let n = t + dir * k * step;
                if n < 0 || costs.contains_key(&n) || done(n) || !has_layer(n) {
                    break;
                }
                out.insert(n);
            }
        }
    }
    // a run of consecutive moments is taken from its START (found-defects 776): a segment goes on
    // only forward, and taking each moment by its distance to the head, both sides alternating, cut
    // the part of a run behind the head into a segment per moment — 54 moments in 14–16 files.
    // Runs go by the loop region first, then by their distance to the head.
    let mut runs: Vec<Vec<i64>> = Vec::new();
    for t in out {
        match runs.last_mut() {
            Some(r) if r.last().is_some_and(|&l| l + step == t) => r.push(t),
            _ => runs.push(vec![t]),
        }
    }
    let inside = |r: &[i64]| region.is_some_and(|(s, e)| r.iter().any(|&t| s <= t && t < e));
    let dist = |r: &[i64]| {
        let (a, b) = (r[0], r[r.len() - 1]);
        if (a..=b).contains(&head) { 0 } else { (a - head).abs().min((b - head).abs()) }
    };
    runs.sort_by_key(|r| (!inside(r), dist(r), r[0]));
    runs.concat()
}

// ---- the player's tier ---------------------------------------------------------------------------

/// A moment the store holds: its durable key and the versions it was computed at.
type OnDisk = (DiskKey, (u64, u64));

/// What the host sets (D7).
#[derive(Clone)]
pub struct Config {
    /// What writes and reads the segments: the host's hardware video codec (kt-media
    /// `segment::ffmpeg::FfmpegSegments`), a test's `RawSegments`.
    pub codec: Arc<dyn SegmentCodec>,
    /// The project's cache folder (kt-media `cache_folder::beside`).
    pub root: PathBuf,
    /// What makes the pixels outside the plan.
    pub made: Made,
    /// The project file's folder: relative media paths resolve against it.
    pub base: Option<PathBuf>,
    /// A cap of the frames alone ([`cap_from_env`]).
    pub cap: Option<u64>,
}

/// The player's disk tier: its configuration, the store of the display's device, the files'
/// identities, the moments on disk for the bars, the input stamp and the prerender's wake.
#[derive(Default)]
pub struct RenderCache {
    cfg: Mutex<Option<Config>>,
    store: Mutex<Option<(String, Arc<DiskStore>)>>,
    /// Held while a store is opened.
    opening: Mutex<()>,
    files: Mutex<Option<(u64, u64, Arc<FileIds>)>>,
    /// Moments the store holds: tick → (durable key, the versions it was computed at).
    on_disk: Mutex<BTreeMap<i64, OnDisk>>,
    pub(crate) disk_gen: Arc<AtomicU64>,
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
    /// The quiet a test sets (ns, 0 — `QUIET`): a stimulus of input a test thread paces with its own
    /// sleeps, which a loaded machine stretches past 1.5 s (found-defects 828).
    quiet_for_tests: AtomicU64,
    heavy_for_tests: AtomicU64,
    /// The prerender parked with nothing left to do (or the cap full) since it last looked.
    settled: AtomicBool,
    /// Why the tier writes nothing on this machine (no encoder of the codec opens), once found.
    unavailable: Mutex<Option<String>>,
    /// Moments of a segment that found no room, to be taken again.
    lost: Mutex<Vec<i64>>,
    /// The encoder the last segment was written with.
    encoder: Mutex<Option<String>>,
}

impl RenderCache {
    pub fn set_config(&self, cfg: Option<Config>) {
        *self.cfg.lock().unwrap() = cfg.filter(|_| enabled());
        *self.store.lock().unwrap() = None;
        *self.files.lock().unwrap() = None;
        *self.unavailable.lock().unwrap() = None;
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
        // the device class: video is byte-equal on one device and one driver only
        let id = format!("{}|{}|{}", device.name, device.backend, device.driver);
        let open = || self.store.lock().unwrap().as_ref().filter(|(d, _)| *d == id).map(|(_, s)| s.clone());
        if let Some(s) = open() {
            return Some(s);
        }
        // one thread reads the manifests, outside the lock the bars ask under
        let _one = self.opening.lock().unwrap();
        if let Some(s) = open() {
            return Some(s);
        }
        let mut s = Folder::open(&cfg.root).and_then(|f| DiskStore::open(f, &id, cfg.cap, cfg.codec.clone())).ok()?;
        s.gone = self.disk_gen.clone();
        let s = Arc::new(s);
        *self.store.lock().unwrap() = Some((id, s.clone()));
        Some(s)
    }

    /// The key of a probe plan as the tier keeps it.
    pub(crate) fn key(&self, plan: &FramePlan<'_>, leaves: &Leaves, files: &FileIds) -> DiskKey {
        let made = self.cfg.lock().unwrap().as_ref().map_or(Made::of(0), |c| c.made);
        disk_key(plan, leaves, files, &made)
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
        let q = match self.quiet_for_tests.load(Ordering::Acquire) {
            0 => QUIET.as_nanos() as u64,
            ns => ns,
        };
        (since < q).then(|| Duration::from_nanos(q - since))
    }

    pub(crate) fn input_stamp(&self) -> u64 {
        self.last_input.load(Ordering::Acquire)
    }

    /// The moment `t` is on disk under `key` for `versions`.
    pub(crate) fn note_on_disk(&self, t: i64, key: DiskKey, versions: (u64, u64)) {
        if self.on_disk.lock().unwrap().insert(t, (key, versions)) != Some((key, versions)) {
            self.disk_gen.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The moments on disk for `versions`.
    pub(crate) fn disk_moments(&self, versions: (u64, u64)) -> Vec<i64> {
        let held = |k: &DiskKey| self.holds(*k);
        self.on_disk.lock().unwrap().iter().filter(|(_, (k, v))| *v == versions && held(k)).map(|(t, _)| *t).collect()
    }

    pub(crate) fn knows(&self, t: i64, versions: (u64, u64)) -> bool {
        let known = self.on_disk.lock().unwrap().get(&t).filter(|(_, v)| *v == versions).map(|(k, _)| *k);
        known.is_some_and(|k| self.holds(k))
    }

    /// The folder's stamps, once the prerender came to rest or ends.
    fn flush(&self) {
        let store = self.store.lock().unwrap().as_ref().map(|(_, s)| s.clone());
        if let Some(s) = store {
            s.folder.flush();
        }
    }

    /// Does the open store still hold it? The cycle may have dropped its segment since.
    fn holds(&self, key: DiskKey) -> bool {
        self.store.lock().unwrap().as_ref().is_some_and(|(_, s)| s.holds(key))
    }

    /// Writes and restores so far.
    pub fn counts(&self) -> (u64, u64) {
        (self.stores.load(Ordering::Relaxed), self.restores.load(Ordering::Relaxed))
    }

    /// What the tier does on this machine, for the person (Diagnostics) and the bench: `None` — off
    /// (no folder set); `Some(Err(why))` — it reads but writes nothing (no encoder of the codec opens,
    /// the owner's rule: no processor codec); `Some(Ok(encoder))` — the encoder of the last segment,
    /// empty before the first.
    pub fn status(&self) -> Option<Result<String, String>> {
        if !self.is_on() {
            return None;
        }
        if let Some(why) = self.unavailable.lock().unwrap().clone() {
            return Some(Err(why));
        }
        Some(Ok(self.encoder.lock().unwrap().clone().unwrap_or_default()))
    }

    /// Whether the prerender is working on a moment now.
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    /// The prerender has nothing left to do at rest (or its cap is full) and works on nothing: the
    /// bench waits for it. A kick clears it until the thread looked again.
    pub fn settled(&self) -> bool {
        self.settled.load(Ordering::Acquire) && !self.busy()
    }

    /// Look for work now (a test that measured its moments by hand; the host after a setting).
    pub fn kick(&self) {
        self.settled.store(false, Ordering::Release);
        self.news.store(true, Ordering::Release);
        self.wake.notify();
    }

    /// Tests: "heavy" is `ns` and more instead of half a frame period (`None` — the rule). A software
    /// rasterizer composes a test's small picture far under any period.
    /// Set the quiet (`None` — `QUIET`), for tests.
    pub fn set_quiet_for_tests(&self, quiet: Option<Duration>) {
        self.quiet_for_tests.store(quiet.map_or(0, |q| (q.as_nanos() as u64).max(1)), Ordering::Release);
    }

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
enum Outcome {
    /// Composed and measured heavy: its key, size and picture go into the open segment.
    Heavy(DiskKey, u32, u32, Vec<u8>, u64),
    Light,
    OnDisk,
    Full,
    Stopped(u64),
    Incomplete,
    NoExec,
}

/// The state the prerender's knowledge belongs to: the build, the versions, the quality, the region.
type StateKey = (u64, (u64, u64), kt_media::Quality, Option<(i64, i64)>);

/// The segment the prerender writes: its moments in order, the moment that would continue it, the
/// state it belongs to.
struct OpenSeg {
    build: SegmentBuild,
    moments: Vec<(i64, DiskKey)>,
    next: i64,
    state: StateKey,
    store: Arc<DiskStore>,
}

/// Close the open segment and keep it (D2): its moments are on disk from now.
fn commit(rc: &RenderCache, seg: &mut Option<OpenSeg>) {
    let Some(s) = seg.take() else { return };
    let encoder = s.build.encoder().to_string();
    let n = s.moments.len();
    match s.store.commit(s.build, false) {
        Ok(bytes) if n > 0 => {
            let id = trace::name(&encoder) as u64;
            trace::ev(Ev::RenderCache).aux(rc::STORE).id(id).a(n as u64).b(bytes).c(s.moments[0].0 as u64).emit();
            rc.stores.fetch_add(n as u64, Ordering::Relaxed);
            for (t, k) in &s.moments {
                rc.note_on_disk(*t, *k, s.state.1);
            }
            *rc.encoder.lock().unwrap() = Some(encoder);
        }
        Ok(_) => {}
        Err(e) => {
            // a segment longer than the room: its moments are taken again, in shorter ones
            if e == PutError::Full && n > 1 {
                rc.lost.lock().unwrap().extend(s.moments.iter().map(|(t, _)| *t));
            }
            trace::ev(Ev::RenderCache).aux(rc::STOPPED).b(rc::why::CAP).emit();
        }
    }
}

/// The player's `render-cache` thread (D4): parks with no clock while there is nothing to do; at rest,
/// after the quiet, composes the moments [`candidates`] names one at a time and writes the heavy ones
/// into a segment, a run of consecutive moments one video file holds.
pub(crate) fn run_prerender(parts: crate::player::PreviewParts, stop: Arc<std::sync::atomic::AtomicBool>) {
    use std::collections::HashSet;
    let rc = parts.rc.clone();
    let mut done: HashSet<i64> = HashSet::new();
    let mut tries: HashMap<i64, u32> = HashMap::new();
    let mut state: Option<StateKey> = None;
    let mut scanned: Option<(StateKey, i64)> = None;
    let mut worked = false;
    let mut full = false;
    // the size an encoder was seen to open at (found-defects 813): asked before any moment composes
    let mut opens: Option<(u32, u32)> = None;
    // the planes the segments' encoder takes, learnt from the same probe: a moment is converted on the
    // composing device and the encoder gets planes (export-host-copy D4)
    let mut layout: Option<crate::yuv::YuvLayout> = None;
    let mut seg: Option<OpenSeg> = None;
    loop {
        let seen = rc.wake.seen();
        if stop.load(Ordering::Acquire) {
            commit(&rc, &mut seg);
            rc.flush();
            return;
        }
        rc.news.store(false, Ordering::Release);
        for t in std::mem::take(&mut *rc.lost.lock().unwrap()) {
            done.remove(&t);
        }
        if !rc.is_on() || parts.transport.is_playing() || export_runs() {
            commit(&rc, &mut seg);
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
            commit(&rc, &mut seg);
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
            commit(&rc, &mut seg);
            state = Some(sk);
            done.clear();
            tries.clear();
            full = false;
        }
        let step = video.grid.frame_step().max(1);
        let head = parts.transport.playhead().0;
        // the bars' scan: once per state and per five seconds of the playhead's place (no device)
        let bucket = head.div_euclid(5 * kt_time::TICK_HZ);
        if scanned != Some((sk, bucket)) {
            scanned = Some((sk, bucket));
            scan(&parts, &rc, &store, &display, &video, &built.project, (built.seq, versions), q, region, head);
        }
        let unavailable = rc.unavailable.lock().unwrap().is_some();
        if full || unavailable {
            rc.settled.store(true, Ordering::Release);
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
        // the moment that continues the open segment first: a run stays one file
        let next = seg.as_ref().map(|s| s.next).filter(|n| list.contains(n));
        let Some(t) = next.or_else(|| list.first().copied()) else {
            commit(&rc, &mut seg);
            if worked {
                trace::ev(Ev::RenderCache).aux(rc::STOPPED).b(rc::why::DONE).emit();
                worked = false;
                rc.flush();
            }
            rc.settled.store(true, Ordering::Release);
            rc.wake.wait_since(seen);
            continue;
        };
        rc.settled.store(false, Ordering::Release);
        if let Some(left) = rc.quiet_left() {
            commit(&rc, &mut seg);
            rc.wake.wait_timeout(left);
            continue;
        }
        if seg.as_ref().is_some_and(|s| s.next != t || s.build.len() >= s.store.segment_frames(q.width, q.height)) {
            commit(&rc, &mut seg);
        }
        // an encoder at this size, asked BEFORE a moment composes (found-defects 813): the tier learnt
        // there was none only at its first heavy frame, and on a machine without one every quiet
        // composed and measured moments it could never write — the rest after a pause was not at rest
        if opens != Some((q.width, q.height)) {
            let fps = (kt_time::TICK_HZ / step).max(1) as u32;
            match store.begin(q.width, q.height, fps) {
                Ok(probe) => {
                    layout = probe.planes();
                    // an empty segment leaves no file
                    let _ = store.commit(probe, false);
                    opens = Some((q.width, q.height));
                }
                Err(PutError::NoEncoder(why)) => {
                    trace::ev(Ev::RenderCache).aux(rc::STOPPED).id(trace::name(&why) as u64).b(rc::why::NO_ENCODER).emit();
                    *rc.unavailable.lock().unwrap() = Some(why);
                    continue;
                }
                Err(_) => {}
            }
        }
        worked = true;
        rc.busy.store(true, Ordering::Release);
        let out = one(&parts, &rc, &store, &display, &video, &built.project, built.seq, t, q, versions, layout, &stop);
        rc.busy.store(false, Ordering::Release);
        match out {
            Outcome::Heavy(dk, w, h, rgba, ns) => {
                done.insert(t);
                if seg.as_ref().is_some_and(|s| s.build.size() != (w, h)) {
                    commit(&rc, &mut seg);
                }
                if seg.is_none() {
                    let fps = (kt_time::TICK_HZ / step).max(1) as u32;
                    match store.begin(w, h, fps) {
                        Ok(build) => seg = Some(OpenSeg { build, moments: Vec::new(), next: t, state: sk, store: store.clone() }),
                        Err(PutError::NoEncoder(why)) => {
                            // the owner's rule: no processor codec — the tier writes nothing here
                            trace::ev(Ev::RenderCache).aux(rc::STOPPED).id(trace::name(&why) as u64).b(rc::why::NO_ENCODER).emit();
                            *rc.unavailable.lock().unwrap() = Some(why);
                            worked = false;
                            continue;
                        }
                        Err(_) => continue,
                    }
                }
                let s = seg.as_mut().unwrap();
                if !s.store.room_for(s.build.len() + 1, w, h) {
                    commit(&rc, &mut seg);
                    full = true;
                    worked = false;
                    trace::ev(Ev::RenderCache).aux(rc::STOPPED).b(rc::why::CAP).emit();
                    continue;
                }
                let pushed = match layout {
                    Some(_) => s.build.push_planes(dk, &rgba),
                    None => s.build.push(dk, &rgba),
                };
                if pushed.is_ok() {
                    s.build.cost(ns);
                    s.moments.push((t, dk));
                    s.next = t + step;
                } else {
                    // a frame the encoder refused: the segment ends before it
                    commit(&rc, &mut seg);
                }
            }
            Outcome::Light | Outcome::OnDisk | Outcome::NoExec => {
                done.insert(t);
            }
            Outcome::Full => {
                commit(&rc, &mut seg);
                full = true;
                trace::ev(Ev::RenderCache).aux(rc::STOPPED).b(rc::why::CAP).emit();
                worked = false;
            }
            Outcome::Stopped(why) => {
                commit(&rc, &mut seg);
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
    let (a, b) = region.unwrap_or((head - 5 * kt_time::TICK_HZ, head + 5 * kt_time::TICK_HZ));
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
                let dk = rc.key(&plan, &leaves, &files);
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
    layout: Option<crate::yuv::YuvLayout>,
    stop: &Arc<std::sync::atomic::AtomicBool>,
) -> Outcome {
    use kt_time::Tick;
    let files = rc.files(project, (seq, versions.1), false);
    let (dk, w, h) = {
        let (plan, leaves) = video.probe_durable(&parts.cache, Tick(t), q);
        (rc.key(&plan, &leaves, &files), plan.out_w, plan.out_h)
    };
    if store.contains(dk) {
        rc.note_on_disk(t, dk, versions);
        return Outcome::OnDisk;
    }
    if !store.has_room(store.frame_bytes(w, h)) {
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
            // the segment's planes made on the composing device (export-host-copy D4), else RGBA
            let out = crate::video::stop_when(trace::stop::PRERENDER, check.clone(), || {
                let plan = video.frame_plan(&parts.cache, Tick(t), q);
                let r = match &layout {
                    Some(l) => exec.execute_once_yuv(&plan, l, &mut buf),
                    None => exec.execute_into(&plan, &mut buf),
                };
                (r, plan.out_w, plan.out_h)
            });
            let want = |w: u32, h: u32| layout.map_or(w as usize * h as usize * 4, |l| l.frame_bytes(w, h));
            match out {
                (Ok(true), w, h) if buf.len() == want(w, h) => break (w, h),
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
        trace::ev(Ev::RenderCache).aux(rc::LIGHT).a(t as u64).b(ns).c(dk.id()).emit();
        return Outcome::Light;
    }
    trace::ev(Ev::RenderCache).aux(rc::HEAVY).a(t as u64).b(ns).c(dk.id()).emit();
    span.c(1);
    Outcome::Heavy(dk, w, h, buf, ns)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pic(seed: u8, w: u32, h: u32) -> Vec<u8> {
        (0..w * h * 4).map(|i| (i as u8).wrapping_mul(seed).wrapping_add(seed)).collect()
    }

    /// Test t2980 (render-cache-disk D2, project-cache-folder D1b): the store keeps segments within its
    /// cap, least recently used out, a touched one stays; each frame reads back as written; the manifest
    /// names the codec, the encoder, the size and every key; a frame of another salt is a miss and its
    /// segment goes, nothing else; a truncated segment is a miss and removed; this process's segments
    /// are not evicted for the prerender; a codec with no encoder refuses the segment.
    #[test]
    fn t2980_the_store_keeps_segments_within_its_cap() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::open(&dir.path().join("kinetren-cache")).unwrap();
        let open = |device: &str, cap: Option<u64>, codec: Arc<dyn SegmentCodec>| DiskStore::open(folder.clone(), device, cap, codec).unwrap();
        let (w, h) = (32, 16);
        let frame = (w * h * 4) as u64;
        let codec: Arc<dyn SegmentCodec> = Arc::new(RawSegments);
        let key = |key: u64| DiskKey { key, salt: 7 };
        // three segments of two frames fit, a fourth does not
        let cap = 3 * (2 * frame + 300) + frame;
        let s = open("llvmpipe|vulkan", Some(cap), codec.clone());
        let write = |s: &DiskStore, k: u64, evict: bool| {
            let mut b = s.begin(w, h, 25).unwrap();
            b.push(key(k), &pic(k as u8, w, h)).unwrap();
            b.push(key(k + 100), &pic(k as u8 + 1, w, h)).unwrap();
            s.commit(b, evict)
        };
        for k in 1..=3u64 {
            write(&s, k, true).unwrap();
        }
        assert_eq!(s.get(key(103)).unwrap(), Picture { w, h, rgba: pic(4, w, h) }, "the second frame of a segment");
        assert_eq!(s.get(key(1)).unwrap().rgba, pic(1, w, h));
        let m = s.manifest_of(key(2)).unwrap();
        assert_eq!(
            (m.codec.as_str(), m.encoder.as_str(), m.w, m.h, m.fps, m.keys.clone()),
            ("raw", "raw", w, h, 25, vec![key(2), key(102)])
        );
        assert_eq!(Manifest::parse(&m.to_text()), Some(m), "the manifest is its own text");
        // segments 3 and 1 were read: 2 is the least recently used
        write(&s, 4, true).unwrap();
        assert!(!s.contains(key(2)) && !s.contains(key(102)), "the least recently used segment goes first");
        assert!(s.contains(key(1)) && s.contains(key(3)) && s.contains(key(4)));
        assert!(s.used().0 <= cap);
        // the prerender never evicts what this process used
        assert_eq!(write(&s, 5, false), Err(PutError::Full));
        assert!(!s.contains(key(5)));
        drop(s);
        // a second process: the segments are there; what makes frame 1 changed: a miss, and its segment
        // goes with nothing else touched
        let s = open("llvmpipe|vulkan", Some(cap), codec.clone());
        assert_eq!(s.used().1, 6);
        assert!(s.has_room(2 * frame + 300), "the previous process's segments may go for the prerender");
        let id = s.ix.lock().unwrap().keys[&1].0;
        assert!(!s.contains(DiskKey { key: 1, salt: 8 }), "a frame of another salt is a miss");
        assert!(!s.video(id).exists() && !s.manifest_path(id).exists() && !s.contains(key(101)), "its segment goes");
        assert!(s.contains(key(3)) && s.contains(key(4)));
        // a truncated segment is a miss and removed
        let id = s.ix.lock().unwrap().keys[&4].0;
        let v = s.video(id);
        std::fs::write(&v, b"short").unwrap();
        assert!(s.get(key(104)).is_none() && !v.exists() && !s.contains(key(4)), "a truncated segment is a miss and removed");
        assert_eq!(s.get(key(3)).unwrap().rgba, pic(3, w, h));
        // another device has its own folder
        assert_eq!(open("radv|vulkan", Some(cap), codec).used().1, 0);
        // no encoder: no segment
        struct NoEncoder;
        impl SegmentCodec for NoEncoder {
            fn codec(&self) -> &str {
                "h264"
            }
            fn writer(&self, _: &Path, _: u32, _: u32, _: u32) -> Result<Box<dyn SegmentWriter>, String> {
                Err("no hardware encoder opens".into())
            }
            fn reader(&self, _: &Path) -> Result<Box<dyn SegmentReader>, String> {
                Err("none".into())
            }
        }
        let n = open("x", None, Arc::new(NoEncoder));
        assert!(matches!(n.begin(w, h, 25), Err(PutError::NoEncoder(_))));
    }

    /// Test t2983 (render-cache-disk D4): the candidates are the heavy moments not done and their
    /// unmeasured neighbours with a picture, in runs of consecutive moments — the loop region's first,
    /// then by distance to the head, each from its start (found-defects 776); a light moment and what
    /// lies past it are not candidates.
    #[test]
    fn t2983_the_prerender_picks_what_was_measured_slow() {
        let step = 10;
        let heavy = 1000;
        // heavy 100, 110; light 120; unmeasured before 100; heavy 300 (done); heavy 500
        let costs: BTreeMap<i64, u64> = [(100, 2000), (110, 1500), (120, 10), (300, 5000), (500, 1000)].into();
        let done = |t: i64| t == 300;
        let has = |t: i64| t != 510;
        let v = candidates(&costs, step, heavy, done, has, None, 480);
        // 300 is on disk, its unmeasured neighbours are still measured; 120 is light, 130 lies past it,
        // 510 has no picture
        let range = |a: i64, b: i64| (a / step..=b / step).map(|i| i * step).collect::<Vec<_>>();
        let want = [range(420, 500), range(310, 380), range(220, 290), range(20, 110)].concat();
        assert_eq!(v, want, "runs nearest the head, each from its start, reaching RUN_REACH unmeasured moments");
        let v = candidates(&costs, step, heavy, done, has, Some((90, 115)), 480);
        assert_eq!(&v[..3], &[20, 30, 40], "the loop region's run first, from its start");
        // the head inside a run (found-defects 776): the whole run from its start, not outward from
        // the head, so it goes into one segment
        let run: BTreeMap<i64, u64> = (0..12).map(|i| (1000 + i * step, 5000)).collect();
        let v = candidates(&run, step, heavy, |_| false, |_| true, None, 1060);
        assert_eq!(v, (92..=119).map(|i| i * step).collect::<Vec<_>>(), "one run, its start first");
        assert!(candidates(&costs, step, 1_000_000, done, has, None, 0).is_empty(), "nothing heavy: nothing");
    }
}
