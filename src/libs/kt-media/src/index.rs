//! The media index and the reel previews of a project, as files of its cache folder
//! (project-cache-folder; capability media-index). Both are CACHES: an entry lost, torn or foreign
//! is a miss and nothing else. They solve defects m (a storm of thumbnail/waveform decodes on
//! scroll/zoom) and j (rescanning files on every launch).
//!
//! - [`MediaStore`]: what is known of a media file, under `media/<content id>/` — the xxh3-128 of
//!   the whole file, so a file renamed, copied or used by two projects has one entry. `paths.tsv`
//!   maps a path, its size and modification time to the id, so the file is not hashed on every open.
//! - [`ReelStore`]: the previews of reels, under `reels/`, by (reel, revision, files' fingerprint).

use crate::cache_folder::{self, Folder, HEADER};
use crate::{MediaSource, PEAK_BLOCK_FRAMES};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

/// The version of the PIXELS a media's thumbs hold (source-colour-matrix): thumbs written under
/// another version are not read, and the first write under this one drops them. 1: the decode
/// converts with the source's own matrix and range (before, BT.601 for every source).
pub const THUMB_PIXELS_VERSION: u32 = 1;

// the kinds of entry files and their versions: a version moves one kind only
const PEAKS: (&[u8; 4], u32) = (b"PEAK", 1);
const PACKETS: (&[u8; 4], u32) = (b"PKTS", 1);
const THUMBS: (&[u8; 4], u32) = (b"THMB", THUMB_PIXELS_VERSION);
const THUMB_SIZE: (&[u8; 4], u32) = (b"THSZ", THUMB_PIXELS_VERSION);
const POSTER: (&[u8; 4], u32) = (b"POST", 1);
const COMPLETE: (&[u8; 4], u32) = (b"DONE", 1);
const LOUDNESS: (&[u8; 4], u32) = (b"LOUD", 1);
const SERIES: &[u8; 4] = b"SERI";
const SYNC: (&[u8; 4], u32) = (b"SYNC", 1);
const STRIP: (&[u8; 4], u32) = (b"RSTR", 1);
const REEL_PEAKS: (&[u8; 4], u32) = (b"RPKS", 1);
const REEL_REF: (&[u8; 4], u32) = (b"RREF", 1);

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// What happens to the media record on opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaEntryStatus {
    /// The content is known (size/mtime matched, or the hash is one the folder holds).
    Reused,
    /// Content the folder has nothing of — sections must be built anew.
    Rebuilt,
}

/// What a media file IS: the xxh3-128 of its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentId(pub [u8; 16]);

impl std::fmt::Display for ContentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl ContentId {
    fn parse(s: &str) -> Option<ContentId> {
        let mut id = [0u8; 16];
        if s.len() != 32 || !s.is_ascii() {
            return None;
        }
        for (i, b) in id.iter_mut().enumerate() {
            *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
        }
        Some(ContentId(id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileIdentity {
    pub size: u64,
    pub mtime_ns: i64,
}

fn file_identity(path: &Path) -> std::io::Result<FileIdentity> {
    let md = std::fs::metadata(path)?;
    let mtime = md.modified()?;
    let ns = mtime.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as i64).unwrap_or(0);
    Ok(FileIdentity { size: md.len(), mtime_ns: ns })
}

fn hash_file(path: &Path) -> std::io::Result<ContentId> {
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(ContentId(hasher.digest128().to_le_bytes()))
}

/// `media/paths.tsv`: path, size, modification time → content id. Appended a line a write; a torn
/// last line is left unread. One per folder and process, shared by its lanes.
#[derive(Default)]
pub(crate) struct PathMap {
    /// The bytes of the file read so far, up to a whole line.
    read: u64,
    lines: usize,
    map: HashMap<String, (FileIdentity, ContentId)>,
    /// Files hashed whole for this folder (t3829: a renamed project folder hashes none).
    pub(crate) hashed: u64,
}

const PATHS: &str = "media/paths.tsv";

impl PathMap {
    /// Take the lines other lanes and processes wrote. Returns the file's length.
    fn refresh(&mut self, folder: &Folder) -> u64 {
        let p = folder.root().join(PATHS);
        let len = std::fs::metadata(&p).map_or(0, |m| m.len());
        if len < self.read {
            // compacted, or the person deleted the folder
            *self = PathMap { hashed: self.hashed, ..Default::default() };
        }
        if len > self.read
            && let Ok(mut f) = std::fs::File::open(&p)
            && f.seek(SeekFrom::Start(self.read)).is_ok()
        {
            let mut buf = Vec::new();
            let _ = f.read_to_end(&mut buf);
            let whole = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
            for l in String::from_utf8_lossy(&buf[..whole]).lines() {
                self.lines += 1;
                if let Some((k, v)) = path_line(l) {
                    self.map.insert(k, v);
                }
            }
            self.read += whole as u64;
        }
        len
    }

    fn note(&mut self, folder: &Folder, key: String, ident: FileIdentity, id: ContentId) -> std::io::Result<()> {
        let len = self.refresh(folder);
        if self.map.get(&key) == Some(&(ident, id)) {
            return Ok(());
        }
        let mut line = String::new();
        if len > self.read {
            // another process died writing the last line: ours starts on its own
            line.push('\n');
        }
        line.push_str(&format!("{}\t{}\t{}\t{id}\n", cache_folder::escape(&key), ident.size, ident.mtime_ns));
        let p = folder.root().join(PATHS);
        folder.made(p.parent().unwrap_or(folder.root()))?;
        std::fs::OpenOptions::new().append(true).create(true).open(&p)?.write_all(line.as_bytes())?;
        self.map.insert(key, (ident, id));
        self.refresh(folder);
        if self.lines > 2 * self.map.len() + 64 {
            self.compact(folder);
        }
        Ok(())
    }

    /// Rewrite the live lines. A line another process appends meanwhile is lost: a hash more.
    fn compact(&mut self, folder: &Folder) {
        let Ok(l) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(folder.root().join("lock")) else { return };
        if l.try_lock().is_err() {
            return;
        }
        let mut text = String::new();
        for (k, (i, id)) in &self.map {
            text.push_str(&format!("{}\t{}\t{}\t{id}\n", cache_folder::escape(k), i.size, i.mtime_ns));
        }
        let tmp = folder.root().join(format!("media/.paths.{}.tmp", std::process::id()));
        if std::fs::write(&tmp, &text).and_then(|()| std::fs::rename(&tmp, folder.root().join(PATHS))).is_ok() {
            (self.read, self.lines) = (text.len() as u64, self.map.len());
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

fn path_line(l: &str) -> Option<(String, (FileIdentity, ContentId))> {
    let f: Vec<&str> = l.split('\t').collect();
    let [path, size, mtime, id] = f.as_slice() else { return None };
    let ident = FileIdentity { size: size.parse().ok()?, mtime_ns: mtime.parse().ok()? };
    Some((cache_folder::unescape(path), (ident, ContentId::parse(id)?)))
}

/// A file's name in the map: relative to the project's folder inside it, absolute elsewhere.
fn path_key(folder: &Folder, file: &Path) -> String {
    let abs = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
    match folder.base().and_then(|b| abs.strip_prefix(b).ok()) {
        Some(rel) => cache_folder::slashed(rel),
        None => abs.to_string_lossy().into_owned(),
    }
}

/// A file's name in the map of a folder that does not lie beside it: absolute.
fn absolute_key(file: &Path) -> String {
    std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf()).to_string_lossy().into_owned()
}

/// Carry the map's lines from one folder into another (project-cache-folder D4, D6): a line relative to
/// the first one's project names the same file in the second.
pub(crate) fn carry_paths(from: &Folder, to: &Folder) -> std::io::Result<()> {
    let lines: Vec<(String, (FileIdentity, ContentId))> = {
        let mut m = from.paths.lock().unwrap_or_else(|e| e.into_inner());
        m.refresh(from);
        m.map.iter().map(|(k, v)| (k.clone(), *v)).collect()
    };
    let mut m = to.paths.lock().unwrap_or_else(|e| e.into_inner());
    for (k, (ident, id)) in lines {
        let file = match from.base() {
            Some(b) => b.join(&k),
            None => Path::new(&k).to_path_buf(),
        };
        let key = path_key(to, &file);
        if !m.map.contains_key(&key) {
            m.note(to, key, ident, id)?;
        }
    }
    Ok(())
}

/// Give the reels one project named in `to` to another (its file's new name).
pub(crate) fn carry_refs(to: &Folder, was: Option<&Path>, now: &Path, keep: bool) {
    let dir = to.root().join("reels/refs");
    let (was, now) = (format!("{}-", tag_of(was)), tag_of(Some(now)));
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(seq) = name.strip_prefix(&was) {
            let dest = dir.join(format!("{now}-{seq}"));
            let _ = if keep { std::fs::copy(e.path(), dest).map(|_| ()) } else { std::fs::rename(e.path(), dest) };
        }
    }
}

/// The records of a `thumbs` file: where each moment's newest JPEG lies, and how far it was read.
#[derive(Default)]
struct ThumbIx {
    read: u64,
    at: BTreeMap<i64, (u64, u32)>,
}

/// How many media's thumb records a store keeps.
const THUMB_IX: usize = 64;
const THUMB_REC: usize = 24;

/// The media index of a project: a lane's own handle on the shared folder.
pub struct MediaStore {
    folder: Arc<Folder>,
    thumbs: RefCell<HashMap<ContentId, ThumbIx>>,
    /// Thumb sizes this handle wrote: a filmstrip says its size with every thumb.
    sized: RefCell<HashMap<ContentId, (u32, u32)>>,
}

impl MediaStore {
    /// The media index in the cache folder `root` ([`cache_folder::beside`]), made if needed.
    pub fn open(root: &Path) -> Result<MediaStore, IndexError> {
        Ok(Self::of(Folder::open(root)?))
    }

    pub fn of(folder: Arc<Folder>) -> MediaStore {
        MediaStore { folder, thumbs: RefCell::default(), sized: RefCell::default() }
    }

    pub fn folder(&self) -> &Arc<Folder> {
        &self.folder
    }

    /// What `file` is: by its path, size and modification time, else by its bytes.
    pub fn ensure_media(&mut self, file: &Path) -> Result<(ContentId, MediaEntryStatus), IndexError> {
        let ident = file_identity(file)?;
        let key = path_key(&self.folder, file);
        let known = {
            let mut m = self.folder.paths.lock().unwrap_or_else(|e| e.into_inner());
            m.refresh(&self.folder);
            // a line of the folder before it had a project beside it is absolute
            m.map.get(&key).or_else(|| m.map.get(&absolute_key(file))).copied()
        };
        if let Some((i, id)) = known
            && i == ident
        {
            return Ok((id, MediaEntryStatus::Reused));
        }
        let id = hash_file(file)?;
        let held = known.is_some_and(|(_, k)| k == id) || self.folder.root().join(format!("media/{id}")).is_dir();
        let mut m = self.folder.paths.lock().unwrap_or_else(|e| e.into_inner());
        m.hashed += 1;
        m.note(&self.folder, key, ident, id)?;
        Ok((id, if held { MediaEntryStatus::Reused } else { MediaEntryStatus::Rebuilt }))
    }

    pub(crate) fn note_path(&self, key_of: &Path, ident: FileIdentity, id: ContentId) -> std::io::Result<()> {
        let key = path_key(&self.folder, key_of);
        self.folder.paths.lock().unwrap_or_else(|e| e.into_inner()).note(&self.folder, key, ident, id)
    }

    fn read(&self, id: ContentId, name: &str, k: (&[u8; 4], u32)) -> Option<Vec<u8>> {
        self.folder.read(&format!("media/{id}/{name}"), k.0, k.1)
    }

    fn write(&self, id: ContentId, name: &str, k: (&[u8; 4], u32), payload: &[u8]) -> Result<(), IndexError> {
        Ok(self.folder.write(&format!("media/{id}/{name}"), k.0, k.1, payload)?)
    }

    pub(crate) fn has(&self, id: ContentId, name: &str) -> bool {
        self.folder.root().join(format!("media/{id}/{name}")).exists()
    }

    // ---- results of offline analysis (R.49, item 369) --------------------------
    //
    // Integers ×100, not f64: the round-trip must be byte-exact, otherwise
    // “recalculated the same thing” would give a different number in the last digit, and
    // there would be no way to tell “recalculated” from “changed”.

    pub fn set_loudness(&self, id: ContentId, lufs: f64, dbtp: f64, lra: f64) -> Result<(), IndexError> {
        self.write(id, "loudness", LOUDNESS, &ints(&[lufs, dbtp, lra].map(|v| (v * 100.0).round() as i64)))
    }

    /// (LUFS, dBTP, LRA) or `None` if not measured yet
    pub fn loudness(&self, id: ContentId) -> Result<Option<(f64, f64, f64)>, IndexError> {
        Ok(self.read(id, "loudness", LOUDNESS).and_then(|b| match ints_of(&b).as_slice() {
            &[a, b, c] => Some((a as f64 / 100.0, b as f64 / 100.0, c as f64 / 100.0)),
            _ => None,
        }))
    }

    /// The audio-reactive analysis series of a media (gen-audio-reactive), `None` when it is not
    /// stored or was computed by another analysis version. Reached through `listen::SeriesStore`.
    pub fn audio_series(&self, id: ContentId, version: u32) -> Result<Option<Vec<u8>>, IndexError> {
        Ok(self.read(id, "series", (SERIES, version)))
    }

    pub fn put_audio_series(&self, id: ContentId, version: u32, data: &[u8]) -> Result<(), IndexError> {
        self.write(id, "series", (SERIES, version), data)
    }

    /// Shift of B relative to A in ticks, and confidence 0..1. The key is an ORDERED pair: the sign
    /// of the shift depends on the order. It names both CONTENTS, so a file replaced on either side
    /// misses (R.49).
    pub fn set_sync_offset(&self, a: ContentId, b: ContentId, ticks: i64, conf: f64) -> Result<(), IndexError> {
        let v = ints(&[ticks, (conf * 100.0).round() as i64]);
        Ok(self.folder.write(&format!("pairs/{a}-{b}"), SYNC.0, SYNC.1, &v)?)
    }

    pub fn sync_offset(&self, a: ContentId, b: ContentId) -> Result<Option<(i64, f64)>, IndexError> {
        Ok(self.folder.read(&format!("pairs/{a}-{b}"), SYNC.0, SYNC.1).and_then(|v| match ints_of(&v).as_slice() {
            &[t, c] => Some((t, c as f64 / 100.0)),
            _ => None,
        }))
    }

    pub fn is_complete(&self, id: ContentId) -> Result<bool, IndexError> {
        Ok(self.read(id, "complete", COMPLETE).is_some())
    }

    pub fn mark_complete(&self, id: ContentId) -> Result<(), IndexError> {
        self.write(id, "complete", COMPLETE, &[1])
    }

    // ---- peaks -------------------------------------------------------------------

    /// Build and save a peak pyramid from an audio source.
    /// Level 0 — a block of 256 frames; level n — a block of 256·2ⁿ. Values (min,max) i16.
    pub fn build_audio_peaks(&mut self, id: ContentId, source: &mut dyn MediaSource) -> Result<(), IndexError> {
        self.build_audio_peaks_until(id, source, &mut |_| {}, &|| false).map(|_| ())
    }

    /// The same with an observer of bursts (R.49, item 373): the material's loudness
    /// is computed on the pass that runs ANYWAY — a second full decode for the sake of
    /// LUFS on an hour-long source is minutes.
    pub fn build_audio_peaks_until(
        &mut self,
        id: ContentId,
        source: &mut dyn MediaSource,
        observe: &mut dyn FnMut(&crate::AudioBurst),
        stop: &dyn Fn() -> bool,
    ) -> Result<bool, IndexError> {
        let Some(pyramid) = PeakPyramid::build_until(source, observe, stop) else {
            return Ok(false);
        };
        self.put_pyramid(id, &pyramid)?;
        Ok(true)
    }

    pub(crate) fn put_pyramid(&self, id: ContentId, p: &PeakPyramid) -> Result<(), IndexError> {
        self.write(id, "peaks", PEAKS, &pyramid_bytes(p))
    }

    /// The whole pyramid of a media, one file read.
    pub fn pyramid(&self, id: ContentId) -> Result<Option<PeakPyramid>, IndexError> {
        Ok(self.read(id, "peaks", PEAKS).and_then(|b| pyramid_of(&b)))
    }

    /// A channel's pyramid level: Vec<(min, max)> by blocks.
    pub fn audio_peaks(&self, id: ContentId, level: u32, channel: u32) -> Result<Option<Vec<(i16, i16)>>, IndexError> {
        Ok(self.pyramid(id)?.and_then(|p| p.levels.into_iter().nth(level as usize)?.into_iter().nth(channel as usize)))
    }

    // ---- the packet index --------------------------------------------------------

    /// Save the packet index of a stream (`stream` = “audio” | “video”);
    /// values are pts in the stream's timebase, an i64 LE blob.
    pub fn put_packet_index(&self, id: ContentId, stream: &str, pts: &[i64]) -> Result<(), IndexError> {
        self.write(id, &format!("packets-{stream}"), PACKETS, &ints(pts))
    }

    /// The stream's packet index, if saved. None = not built
    /// (an empty Vec is a valid index of a file without such a stream).
    pub fn packet_index(&self, id: ContentId, stream: &str) -> Result<Option<Vec<i64>>, IndexError> {
        Ok(self.read(id, &format!("packets-{stream}"), PACKETS).map(|b| ints_of(&b)))
    }

    // ---- thumbs: one file of records a media, appended ------------------------------

    /// JPEG thumbnail at time t (ticks), if already built.
    pub fn thumbnail(&self, id: ContentId, t_ticks: i64) -> Result<Option<Vec<u8>>, IndexError> {
        Ok(self.thumb_in(id, t_ticks, t_ticks).map(|(_, j)| j))
    }

    /// The nearest thumbnail within tolerance max_dist ticks (item 14, round 7): the bin
    /// scrub reuses thumbs written by the filmstrip with another step.
    pub fn thumbnail_near(&self, id: ContentId, t_ticks: i64, max_dist: i64) -> Result<Option<(i64, Vec<u8>)>, IndexError> {
        let (a, b) = (t_ticks.saturating_sub(max_dist), t_ticks.saturating_add(max_dist));
        Ok(self.thumb_in(id, a, b).filter(|(t, _)| t.abs_diff(t_ticks) <= max_dist.unsigned_abs()))
    }

    /// The record nearest the middle of `a..=b`.
    fn thumb_in(&self, id: ContentId, a: i64, b: i64) -> Option<(i64, Vec<u8>)> {
        let rel = format!("media/{id}/thumbs");
        let mut all = self.thumbs.borrow_mut();
        if all.len() >= THUMB_IX && !all.contains_key(&id) {
            all.clear();
        }
        let ix = all.entry(id).or_default();
        ix.refresh(&self.folder, &rel);
        let mid = a + (b - a) / 2;
        let got = ix.at.range(a..=b).min_by_key(|(t, _)| t.abs_diff(mid)).map(|(t, at)| (*t, *at)).and_then(|(t, (at, len))| {
            let mut f = std::fs::File::open(self.folder.root().join(&rel)).ok()?;
            f.seek(SeekFrom::Start(at)).ok()?;
            let mut rec = vec![0u8; THUMB_REC + len as usize];
            f.read_exact(&mut rec).ok()?;
            let j = thumb_record(&rec).filter(|(rt, _)| *rt == t)?.1.to_vec();
            Some((t, j))
        });
        match got.is_some() {
            true => self.folder.used(&rel),
            false => self.folder.missed(cache_folder::Tenant::Media),
        }
        got
    }

    pub fn put_thumbnail(&self, id: ContentId, t_ticks: i64, jpeg: &[u8]) -> Result<(), IndexError> {
        Ok(self.folder.append(&format!("media/{id}/thumbs"), THUMBS.0, THUMBS.1, &thumb_bytes(t_ticks, jpeg))?)
    }

    pub(crate) fn put_thumbs(&self, id: ContentId, thumbs: &[(i64, Vec<u8>)]) -> Result<(), IndexError> {
        let all: Vec<u8> = thumbs.iter().flat_map(|(t, j)| thumb_bytes(*t, j)).collect();
        Ok(self.folder.append(&format!("media/{id}/thumbs"), THUMBS.0, THUMBS.1, &all)?)
    }

    /// Media poster: the moment of the most informative frame (item 4, round 9).
    /// `t_ticks < 0` — “no informative frame” (negative cache: audio,
    /// broken video) — not searched again.
    pub fn poster(&self, id: ContentId) -> Result<Option<i64>, IndexError> {
        Ok(self.read(id, "poster", POSTER).and_then(|b| ints_of(&b).first().copied()))
    }

    pub fn put_poster(&self, id: ContentId, t_ticks: i64, score: i64) -> Result<(), IndexError> {
        self.write(id, "poster", POSTER, &ints(&[t_ticks, score]))
    }

    /// The size at which the media's thumbs are written (R.62, t719): thumbs without it are not
    /// read, they are rebuilt on the first request. Thumbs whose pixels are of another
    /// `THUMB_PIXELS_VERSION` (written before the decode took the source's colour matrix) count as
    /// absent the same way.
    pub fn thumb_size(&self, id: ContentId) -> Result<Option<(u32, u32)>, IndexError> {
        Ok(self.read(id, "thumb-size", THUMB_SIZE).and_then(|b| match ints_of(&b).as_slice() {
            &[w, h] => Some((w as u32, h as u32)),
            _ => None,
        }))
    }

    /// Written BEFORE the thumb it describes: a media whose thumbs are of an older
    /// `THUMB_PIXELS_VERSION` loses them here, so none of them is read once the size is set.
    pub fn put_thumb_size(&mut self, id: ContentId, w: u32, h: u32) -> Result<(), IndexError> {
        let p = self.folder.root().join(format!("media/{id}/thumbs"));
        if p.exists() && cache_folder::header_of(&p).is_none_or(|h| !cache_folder::is_header(&h, THUMBS.0, THUMBS.1)) {
            let _ = std::fs::remove_file(&p);
        }
        if self.sized.borrow().get(&id) == Some(&(w, h)) && self.has(id, "thumb-size") {
            return Ok(());
        }
        self.write(id, "thumb-size", THUMB_SIZE, &ints(&[w as i64, h as i64]))?;
        self.sized.borrow_mut().insert(id, (w, h));
        Ok(())
    }
}

impl ThumbIx {
    /// Take the records appended since; a file of another kind or version is removed.
    fn refresh(&mut self, folder: &Folder, rel: &str) {
        let p = folder.root().join(rel);
        let len = std::fs::metadata(&p).map_or(0, |m| m.len());
        if len < self.read {
            *self = ThumbIx::default();
        }
        if len <= self.read {
            return;
        }
        let Ok(mut f) = std::fs::File::open(&p) else { return };
        if self.read == 0 {
            let mut h = [0u8; HEADER];
            if f.read_exact(&mut h).is_err() || !cache_folder::is_header(&h, THUMBS.0, THUMBS.1) {
                drop(f);
                let _ = std::fs::remove_file(&p);
                return;
            }
            self.read = HEADER as u64;
        } else if f.seek(SeekFrom::Start(self.read)).is_err() {
            return;
        }
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_err() {
            return;
        }
        let mut at = 0;
        while let Some((t, j)) = buf.get(at..).and_then(thumb_record) {
            self.at.insert(t, (self.read + at as u64, j.len() as u32));
            at += THUMB_REC + j.len();
        }
        self.read += at as u64;
        if at < buf.len() && torn(&buf[at..]) {
            // a record a process died writing: what comes after it could not be read
            drop(f);
            if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&p) {
                let _ = f.set_len(self.read);
            }
        }
    }
}

/// Is what follows the last whole record not just one still being written?
fn torn(rest: &[u8]) -> bool {
    match rest.get(..THUMB_REC) {
        // the record's whole length stands there and its check failed, or it is no record at all
        Some(h) => &h[..4] != b"KTHR" || rest.len() >= THUMB_REC + u32::from_le_bytes(h[12..16].try_into().unwrap()) as usize,
        None => false,
    }
}

/// `KTHR`, the moment, the length, the xxh3-64 of the JPEG, the JPEG.
fn thumb_bytes(t: i64, jpeg: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(THUMB_REC + jpeg.len());
    b.extend_from_slice(b"KTHR");
    b.extend_from_slice(&t.to_le_bytes());
    b.extend_from_slice(&(jpeg.len() as u32).to_le_bytes());
    b.extend_from_slice(&xxhash_rust::xxh3::xxh3_64(jpeg).to_le_bytes());
    b.extend_from_slice(jpeg);
    b
}

fn thumb_record(b: &[u8]) -> Option<(i64, &[u8])> {
    let h = b.get(..THUMB_REC)?;
    if &h[..4] != b"KTHR" {
        return None;
    }
    let t = i64::from_le_bytes(h[4..12].try_into().ok()?);
    let len = u32::from_le_bytes(h[12..16].try_into().ok()?) as usize;
    let sum = u64::from_le_bytes(h[16..24].try_into().ok()?);
    let j = b.get(THUMB_REC..THUMB_REC + len)?;
    (xxhash_rust::xxh3::xxh3_64(j) == sum).then_some((t, j))
}

fn ints(v: &[i64]) -> Vec<u8> {
    v.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn ints_of(b: &[u8]) -> Vec<i64> {
    b.chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().unwrap())).collect()
}

/// A pyramid in one payload: the levels, per level the channels, per channel its blocks' count and blob.
fn pyramid_bytes(p: &PeakPyramid) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&(p.levels.len() as u32).to_le_bytes());
    for level in &p.levels {
        b.extend_from_slice(&(level.len() as u32).to_le_bytes());
        for blocks in level {
            b.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
            b.extend_from_slice(&peaks_blob(blocks));
        }
    }
    b
}

fn pyramid_of(b: &[u8]) -> Option<PeakPyramid> {
    let mut at = 0;
    let n = |at: &mut usize| -> Option<usize> {
        let v = b.get(*at..*at + 4)?;
        *at += 4;
        Some(u32::from_le_bytes(v.try_into().ok()?) as usize)
    };
    let mut levels = Vec::new();
    for _ in 0..n(&mut at)? {
        let mut level = Vec::new();
        for _ in 0..n(&mut at)? {
            let blocks = n(&mut at)?;
            level.push(peaks_from_blob(b.get(at..at + blocks * 4)?));
            at += blocks * 4;
        }
        levels.push(level);
    }
    Some(PeakPyramid { levels })
}

// ---- reel previews (R.58, item 463) --------------------------------------------------------------

/// The reel previews of a project.
///
/// A reel has no file: the key is the CONTENT. `seq` — the reel id (survives save→load, t431),
/// `hash` — the edit (`kt_engine::content_hash`), `media_fp` — the fingerprint of the reel's tree
/// files (path, size, mtime): the edit does not see a file's swap, and without the fingerprint a
/// swapped source would show the old strip.
///
/// The folder is shared by the projects in it, so a preview is kept under its whole key and each
/// project (`tag`) names the one version of each of its reels under `reels/refs/`; a version no
/// project names goes.
pub struct ReelStore {
    folder: Arc<Folder>,
    tag: String,
}

impl ReelStore {
    /// The reel previews in the cache folder `root`, of the project file named `project` (`None` — an
    /// untitled one).
    pub fn open(root: &Path, project: Option<&Path>) -> Result<ReelStore, IndexError> {
        Ok(Self::of(Folder::open(root)?, project))
    }

    pub fn of(folder: Arc<Folder>, project: Option<&Path>) -> ReelStore {
        ReelStore { folder, tag: tag_of(project) }
    }

    fn entry(seq: i64, hash: u64, media_fp: u64) -> String {
        format!("{:016x}-{hash:016x}-{media_fp:016x}", seq as u64)
    }

    fn ref_path(&self, seq: i64) -> String {
        format!("reels/refs/{}-{:016x}", self.tag, seq as u64)
    }

    /// Name `entry` as this project's version of the reel; the version named before goes when no
    /// other project names it.
    fn take(&self, seq: i64, entry: &str) -> Result<(), IndexError> {
        let rel = self.ref_path(seq);
        let before = self.folder.read(&rel, REEL_REF.0, REEL_REF.1).and_then(|b| String::from_utf8(b).ok());
        if before.as_deref() == Some(entry) {
            return Ok(());
        }
        self.folder.write(&rel, REEL_REF.0, REEL_REF.1, entry.as_bytes())?;
        if let Some(b) = before {
            self.drop_unnamed(&b);
        }
        Ok(())
    }

    fn refs(&self) -> Vec<(String, String)> {
        let dir = self.folder.root().join("reels/refs");
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let b = self.folder.read(&format!("reels/refs/{name}"), REEL_REF.0, REEL_REF.1)?;
                Some((name, String::from_utf8(b).ok()?))
            })
            .collect()
    }

    fn drop_unnamed(&self, entry: &str) {
        if self.refs().iter().any(|(_, e)| e == entry) {
            return;
        }
        let _ = std::fs::remove_dir_all(self.folder.root().join("reels").join(entry));
        self.folder.gone(&format!("reels/{entry}/strip"));
    }

    /// The FRAMES of the strip — RIGHT AFTER the frame stage (R.67): previously the write was single
    /// and came only after the audio stage, so a strip that had already reached the screen did not
    /// survive to disk if the task was cancelled (pause, edit, close). `frames[i]` — the JPEG of frame
    /// `i * step`, `None` — the frame was absent.
    pub fn put_reel_frames(
        &mut self,
        seq: i64,
        hash: u64,
        media_fp: u64,
        step: i64,
        end_ticks: i64,
        frames: &[Option<&[u8]>],
    ) -> Result<(), IndexError> {
        let entry = Self::entry(seq, hash, media_fp);
        let mut b = ints(&[step, end_ticks, frames.len() as i64]);
        for j in frames {
            b.extend_from_slice(&j.map_or(u32::MAX, |j| j.len() as u32).to_le_bytes());
            b.extend_from_slice(j.unwrap_or_default());
        }
        // the peaks of the strip before are not this one's
        let _ = std::fs::remove_file(self.folder.root().join(format!("reels/{entry}/peaks")));
        self.folder.write(&format!("reels/{entry}/strip"), STRIP.0, STRIP.1, &b)?;
        self.take(seq, &entry)
    }

    /// The PEAKS of the same strip, LATER. They are written only beside a strip with the same key: if
    /// the edit diverged, the peaks are not ours.
    pub fn put_reel_peaks(&mut self, seq: i64, hash: u64, media_fp: u64, peaks: &PeakPyramid) -> Result<(), IndexError> {
        let entry = Self::entry(seq, hash, media_fp);
        if !self.folder.root().join(format!("reels/{entry}/strip")).exists() {
            return Ok(());
        }
        Ok(self.folder.write(&format!("reels/{entry}/peaks"), REEL_PEAKS.0, REEL_PEAKS.1, &pyramid_bytes(peaks))?)
    }

    /// The reel preview, if the disk holds EXACTLY this edit with the SAME
    /// files; any mismatch — `None`, and the reel is rendered anew.
    pub fn reel_preview(&self, seq: i64, hash: u64, media_fp: u64) -> Result<Option<ReelPreviewRow>, IndexError> {
        let entry = Self::entry(seq, hash, media_fp);
        let Some(b) = self.folder.read(&format!("reels/{entry}/strip"), STRIP.0, STRIP.1) else { return Ok(None) };
        let Some((step, end_ticks, frames)) = strip_of(&b) else { return Ok(None) };
        self.take(seq, &entry)?;
        // PEAKS MAY BE ABSENT: frames are written earlier, and a strip without an
        // envelope is a legitimate intermediate state (R.67).
        let peaks = self.folder.read(&format!("reels/{entry}/peaks"), REEL_PEAKS.0, REEL_PEAKS.1).and_then(|b| pyramid_of(&b));
        let has_peaks = peaks.is_some();
        Ok(Some(ReelPreviewRow { step, end_ticks, frames, peaks: peaks.unwrap_or(PeakPyramid { levels: Vec::new() }), has_peaks }))
    }

    /// Forget this project's previews of reels that are no longer in it. Returns the
    /// number of forgotten reels.
    pub fn prune_reels(&mut self, keep: &[i64]) -> Result<usize, IndexError> {
        let mine = format!("{}-", self.tag);
        let mut gone = 0;
        for (name, entry) in self.refs() {
            let seq = name.strip_prefix(&mine).and_then(|s| u64::from_str_radix(s, 16).ok()).map(|s| s as i64);
            if seq.is_some_and(|s| !keep.contains(&s)) {
                std::fs::remove_file(self.folder.root().join("reels/refs").join(&name))?;
                self.drop_unnamed(&entry);
                gone += 1;
            }
        }
        Ok(gone)
    }
}

/// What a project calls its versions by: the project file's name, so a renamed folder keeps it.
fn tag_of(project: Option<&Path>) -> String {
    let name = project.and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    format!("{:016x}", xxhash_rust::xxh3::xxh3_64(name.as_bytes()))
}

type Strip = (i64, i64, Vec<Option<Vec<u8>>>);

fn strip_of(b: &[u8]) -> Option<Strip> {
    let head = ints_of(b.get(..24)?);
    let n = usize::try_from(head[2]).ok()?;
    let mut at = 24;
    let mut frames = Vec::with_capacity(n.min(1024));
    for _ in 0..n {
        let len = u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?);
        at += 4;
        if len == u32::MAX {
            frames.push(None);
            continue;
        }
        frames.push(Some(b.get(at..at + len as usize)?.to_vec()));
        at += len as usize;
    }
    Some((head[0], head[1], frames))
}

/// A pyramid level blob: interleaved (min:i16, max:i16) LE per block — one
/// format for a media's and a reel's peaks.
pub(crate) fn peaks_blob(blocks: &[(i16, i16)]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(blocks.len() * 4);
    for &(mn, mx) in blocks {
        blob.extend_from_slice(&mn.to_le_bytes());
        blob.extend_from_slice(&mx.to_le_bytes());
    }
    blob
}

pub(crate) fn peaks_from_blob(b: &[u8]) -> Vec<(i16, i16)> {
    b.chunks_exact(4)
        .map(|c| (i16::from_le_bytes([c[0], c[1]]), i16::from_le_bytes([c[2], c[3]])))
        .collect()
}

/// Reel preview from disk (R.58): a strip of JPEG frames at step `step` and a
/// peak pyramid in the same form that `PeakAcc::finish` builds.
#[derive(Debug)]
pub struct ReelPreviewRow {
    pub step: i64,
    pub end_ticks: i64,
    /// `frames[i]` — JPEG of frame `i * step`; `None` — the frame was absent.
    pub frames: Vec<Option<Vec<u8>>>,
    pub peaks: PeakPyramid,
    /// The envelope is ALREADY on disk. Frames are written before the peaks (R.67),
    /// and a strip without them is a legitimate state: frames are served from disk, and
    /// the peaks are appended by the next task.
    pub has_peaks: bool,
}

/// Peak pyramid (test 15 compares it with a naive recomputation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeakPyramid {
    /// levels[level][channel][block] = (min, max)
    pub levels: Vec<Vec<Vec<(i16, i16)>>>,
}

impl PeakPyramid {
    pub fn build(source: &mut dyn MediaSource) -> PeakPyramid {
        Self::build_observed(source, &mut |_| {})
    }

    /// The same, but EVERY burst that is read is shown to the observer
    /// (R.49, item 369).
    ///
    /// Why: the material's loudness and its envelope are computed on the pass that
    /// runs ANYWAY. A second pass over the file for LUFS would mean a second full
    /// decode — on an hour-long source that is minutes.
    ///
    /// The pyramid from the observer is NOT DEPENDENT: it only watches, and `build`
    /// remains byte-for-byte the same (t15 intact).
    pub fn build_observed(
        source: &mut dyn MediaSource,
        observe: &mut dyn FnMut(&crate::AudioBurst),
    ) -> PeakPyramid {
        Self::build_until(source, observe, &|| false).unwrap_or_else(|| PeakAcc::default().finish())
    }

    /// The same, asking `stop` before every burst: `None` when it said so, and nothing of the
    /// pass is kept. A pass is as long as the file says, and a program closing or a test ending
    /// has no reason to wait for it.
    ///
    /// A source with no sound, or one with no end (a still or a placeholder: "practically
    /// infinite", 1000 hours, every read an instant silence) has no envelope to measure. Read to
    /// its end it held a core for hours, grew without bound and starved every file queued
    /// behind it: one picture in a project and no waveform came any more (t1740).
    pub fn build_until(
        source: &mut dyn MediaSource,
        observe: &mut dyn FnMut(&crate::AudioBurst),
        stop: &dyn Fn() -> bool,
    ) -> Option<PeakPyramid> {
        let info = source.info();
        let mut acc = PeakAcc::default();
        if !info.has_audio || info.duration >= crate::placeholder::NO_END {
            return Some(acc.finish());
        }
        let duration_frames = info.duration.to_audio_frames().max(0);
        let total_bursts = duration_frames.div_euclid(AUDIO_BURST_FRAMES as i64).max(0);
        for bidx in 0..total_bursts {
            if stop() {
                return None;
            }
            let Ok(burst) = source.read_audio_burst(bidx) else { break };
            observe(&burst);
            acc.push(burst.data.as_slice());
        }
        Some(acc.finish())
    }
}

/// Peak-pyramid accumulator over a STREAM of samples (R.56, item 452).
///
/// Factored out of `build_observed` and used by it too — the arithmetic is one, and
/// t15 remains byte-for-byte. It is needed by the reel preview service: a reel has no
/// file, its envelope is taken from the ready sub-mix that `AudioRenderer` hands out
/// in chunks, not from a `MediaSource`.
#[derive(Default)]
pub struct PeakAcc {
    level0: Vec<Vec<(i16, i16)>>,
    block: Vec<(i16, i16)>,
    in_block: usize,
}

impl PeakAcc {
    /// The next chunk of f32 interleaved (a multiple of the channel count).
    pub fn push(&mut self, data: &[f32]) {
        if self.level0.is_empty() {
            self.level0 = vec![Vec::new(); AUDIO_CHANNELS];
            self.block = vec![(i16::MAX, i16::MIN); AUDIO_CHANNELS];
        }
        kt_plugin_api::fast!(self.push_at_level(data))
    }

    /// The fold at the CPU's level (kernel-specialization): a block's run of frames into a
    /// fixed-size array, the block pushed at its end. Integer min/max — the same values in any
    /// grouping (t2175).
    #[inline(always)]
    fn push_at_level(&mut self, data: &[f32]) {
        let mut rest = &data[..data.len() / AUDIO_CHANNELS * AUDIO_CHANNELS];
        while !rest.is_empty() {
            let n = (PEAK_BLOCK_FRAMES - self.in_block).min(rest.len() / AUDIO_CHANNELS);
            let mut b: [(i16, i16); AUDIO_CHANNELS] = std::array::from_fn(|ch| self.block[ch]);
            for fr in rest[..n * AUDIO_CHANNELS].chunks_exact(AUDIO_CHANNELS) {
                for (b, &x) in b.iter_mut().zip(fr) {
                    let v = (x.clamp(-1.0, 1.0) * 32767.0) as i16;
                    b.0 = b.0.min(v);
                    b.1 = b.1.max(v);
                }
            }
            self.block.copy_from_slice(&b);
            rest = &rest[n * AUDIO_CHANNELS..];
            self.in_block += n;
            if self.in_block == PEAK_BLOCK_FRAMES {
                for ch in 0..AUDIO_CHANNELS {
                    self.level0[ch].push(self.block[ch]);
                    self.block[ch] = (i16::MAX, i16::MIN);
                }
                self.in_block = 0;
            }
        }
    }

    pub fn finish(mut self) -> PeakPyramid {
        if self.level0.is_empty() {
            self.level0 = vec![Vec::new(); AUDIO_CHANNELS];
        }
        if self.in_block > 0 {
            for ch in 0..AUDIO_CHANNELS {
                self.level0[ch].push(self.block[ch]);
            }
        }
        let mut levels = vec![self.level0];
        while levels.last().unwrap()[0].len() > 1 {
            let prev = levels.last().unwrap();
            let mut next: Vec<Vec<(i16, i16)>> = vec![Vec::new(); AUDIO_CHANNELS];
            for ch in 0..AUDIO_CHANNELS {
                for pair in prev[ch].chunks(2) {
                    let mn = pair.iter().map(|p| p.0).min().unwrap();
                    let mx = pair.iter().map(|p| p.1).max().unwrap();
                    next[ch].push((mn, mx));
                }
            }
            levels.push(next);
        }
        PeakPyramid { levels }
    }
}
