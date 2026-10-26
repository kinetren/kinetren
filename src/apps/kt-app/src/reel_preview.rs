//! Preview of a NESTED REEL: frames and envelope (R.56, item 452).
//!
//! **Owner's claim, verbatim:** “there are no previews for compositions for some reason” —
//! and, to be precise, “there is no preview on the timeline as for ordinary events - frames and
//! audio peaks.”
//!
//! Before R.56, an event-attachment on the timeline had a mini-map instead of
//! a filmstrip, and its audio half drew NOTHING: peaks are looked up by
//! the FILE's `MediaId`, while the reel has no `MediaRef` on purpose (item 378) — give it
//! a path, and the watchdog would paint it offline, and the index service would start
//! building peaks with ffmpeg. This was recorded as “a reel has no filmstrip and
//! cannot have one”, but only ONE solution was impossible — a thumbnail by
//! file. The reel has another option: its content can be RENDERED by the same
//! composition code as everything else (invariant 7).
//!
//! **Shape: a SERVICE modeled on `analysis`**, not a task: queue, dedup,
//! negative cache, one job at a time.
//!
//! **Key — (reel, `content_hash`)**, not a path. The reel's edit hash already
//! is computed by the engine (`kt_engine::content_hash`), and it also invalidates:
//! an edit inside the reel changes the hash, and the old strip simply stops
//! being requested. In memory — LRU by this key.
//!
//! **Disk tier — in the sidecar `<project>.ktindex.db` (R.58, item 463).**
//! Owner's claim: “Composition previews on the timeline, it seems, are not
//! cached, and every project open rebuilds them anew.” The R.56 note “not in the
//! sidecar — it is keyed by file identity” was a property of the
//! `ensure_media` API, not of the schema: there are no foreign keys in the DB, and the `reel_*`
//! tables are keyed by CONTENT — (reel, edit, media fingerprint). The edit
//! does not see a file swap (`content_hash` hashes events, not source
//! bytes), so the third part of the key is `media_fingerprint`: path,
//! size and mtime of each file in the reel's tree. It is computed ON THE TASK THREAD
//! (where the DB is opened too); the UI thread touches neither `fs::metadata` nor sqlite
//! (invariants 8/12). A hit — the strip and peaks come from disk without
//! a single decode; a miss — render and write in one transaction. A DB error —
//! a cache error: the preview still arrives. A project without a path — RAM only.
//!
//! **Invariant 5 is respected by the shape of the offline path:** its own short-lived
//! sources, not shared with anyone; the job is finite and cancelable; and no one
//! waits for real time. **Invariant 8 — drawing only POSTS A REQUEST
//! by key**, and the key depends on neither zoom nor scrolling.
//!
//! **A pause is mandatory, not merely desirable:** the reel's audio is rendered offline and
//! on a long reel it takes seconds of CPU; if it ran during playback it would take
//! that time from real time (invariant 6).

use eframe::egui;
use kt_media::Quality;
use kt_model::{MediaId, Project};
use kt_time::Tick;
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// The frame size of the strip is the same as for an ordinary thumbnail: they are drawn side by side
/// by one piece of code.
pub const REEL_THUMB_W: u32 = crate::thumbs::THUMB_W;
pub const REEL_THUMB_H: u32 = crate::thumbs::THUMB_H;

/// Frame cap per reel. This is not a “memory budget” but a promise about cost: for
/// an hour-long reel the step simply becomes larger, and the job stays finite.
pub const MAX_FRAMES_PER_REEL: usize = 48;

/// How many reels we keep ready (LRU). A 320×180 RGBA frame is 230 KiB.
pub const MAX_REELS_READY: usize = 6;

/// Negative cache cap (overflow means a full clear, as with thumbnails).
pub const MAX_FAILED: usize = 64;

/// Where the ready preview came from: computed, or read from the sidecar.
/// The service counts real renders by this — test t640 requires zero on the second
/// project open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReelOrigin {
    Rendered,
    Disk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReelKey {
    pub seq: MediaId,
    /// Edit of the reel content (`kt_engine::content_hash`).
    pub hash: u64,
}

/// Reel frames by the reel's OWN time: `frames[i]` is the moment `i * step`.
#[derive(Debug, Clone)]
pub struct ReelStrip {
    pub step: i64,
    pub frames: Vec<Option<Arc<egui::ColorImage>>>,
    /// The end of the reel content at render time. Drawing does NOT use it ON PURPOSE:
    /// a frame past the end must not be blanked — an instance's window can be longer
    /// than the content, and there the last frame is drawn, as for a file (t600).
    /// The field is stored because the disk tier writes and reads it
    /// (`reel_strip.end_ticks`, R.58 item 463): it shows the length the strip was
    /// computed for.
    #[allow(dead_code)]
    pub end: Tick,
}

impl ReelStrip {
    /// The frame at reel time `t` is the nearest one to the LEFT, as for a file's filmstrip.
    pub fn at(&self, t: i64) -> Option<&Arc<egui::ColorImage>> {
        if self.step <= 0 {
            return None;
        }
        let i = (t.max(0) / self.step) as usize;
        self.frames.get(i)?.as_ref()
    }

    /// Frame index — the texture is keyed by it too.
    pub fn index(&self, t: i64) -> usize {
        if self.step <= 0 { 0 } else { (t.max(0) / self.step) as usize }
    }
}

/// Ready reel preview. Peaks arrive LATER than frames: frames are cheap and visible
/// immediately, while the reel's audio is computed by offline rendering and takes seconds on a long reel.
/// Until then the waveform is honestly empty, not made up.
#[derive(Debug, Clone, Default)]
pub struct ReelPreview {
    pub strip: Option<ReelStrip>,
    pub peaks: Option<crate::util::Peaks>,
}

/// What the task sends along the way: first frames, then the result.
pub type ReelPartial = ReelStrip;

/// Task result: the peaks and where they came from.
pub type ReelDone = Result<(crate::util::Peaks, ReelOrigin), String>;

/// Cancelling a task is NOT a failure: the service distinguishes this from a render error
/// and does not put it into the negative cache (R.67).
pub const CANCELLED: &str = "cancelled";

type Job = crate::bg::BgJob<ReelPartial, ReelDone>;

/// Reel preview service.
#[derive(Default)]
pub struct ReelPreviews {
    running: Option<(ReelKey, Job)>,
    queue: VecDeque<(ReelKey, Box<ReelBody>)>,
    /// Ready + a last-access mark: `get` is called from drawing via `&self`, so the LRU is kept via
    /// `Cell`, not via `&mut`.
    ready: HashMap<ReelKey, (Arc<ReelPreview>, Cell<u64>)>,
    clock: Cell<u64>,
    failed: HashMap<ReelKey, String>,
    paused: bool,
    /// How many previews were ACTUALLY rendered (not taken from disk).
    renders: usize,
}

type ReelBody = dyn FnOnce(crate::bg::BgCtl, crate::bg::BgSend<ReelPartial>) -> ReelDone + Send;

impl ReelPreviews {
    /// Strip step: the smallest rung of the thumbnail ladder at which the frame count
    /// does not exceed the cap. The ladder is the same as the file's filmstrip ladder — so the keys
    /// sit in the same places.
    pub fn strip_step(len: Tick) -> i64 {
        let n = MAX_FRAMES_PER_REEL as i64;
        *crate::thumbs::THUMB_STEP_LADDER
            .iter()
            .find(|&&s| len.0 <= s * n)
            .unwrap_or(crate::thumbs::THUMB_STEP_LADDER.last().unwrap())
    }

    fn touch(&self, stamp: &Cell<u64>) {
        let c = self.clock.get() + 1;
        self.clock.set(c);
        stamp.set(c);
    }

    /// Ready preview; access refreshes the LRU entry.
    pub fn get(&self, key: &ReelKey) -> Option<Arc<ReelPreview>> {
        let (e, stamp) = self.ready.get(key)?;
        self.touch(stamp);
        Some(e.clone())
    }

    pub fn peaks(&self, key: &ReelKey) -> Option<&crate::util::Peaks> {
        let (e, stamp) = self.ready.get(key)?;
        self.touch(stamp);
        e.peaks.as_ref()
    }

    /// Number of real renders over the service's lifetime (disk hits
    /// are not counted).
    pub fn renders(&self) -> usize {
        self.renders
    }

    #[cfg(test)]
    pub fn ready_keys(&self) -> Vec<ReelKey> {
        self.ready.keys().copied().collect()
    }

    #[cfg(test)]
    pub fn failed_len(&self) -> usize {
        self.failed.len()
    }

    pub fn failure(&self, key: &ReelKey) -> Option<&str> {
        self.failed.get(key).map(|s| s.as_str())
    }

    pub fn is_running(&self) -> bool {
        self.running.as_ref().is_some_and(|(_, j)| j.is_running())
    }

    pub fn pending(&self) -> usize {
        self.queue.len() + usize::from(self.running.is_some())
    }

    /// Reel request. Returns `false` if there is nothing to do: already
    /// ready, already being computed, already queued, or already failed.
    ///
    /// It is called FROM DRAWING, so it must be cheap and idempotent:
    /// the key depends on neither zoom nor scrolling (invariant 8).
    pub fn request(
        &mut self,
        key: ReelKey,
        project: Project,
        index_db: Option<std::path::PathBuf>,
        base: Option<std::path::PathBuf>,
    ) -> bool {
        // A FULL entry (frames AND envelope) — nothing to do; a strip without
        // peaks (the job was cancelled at the audio stage, R.67) is SKIPPED by the request;
        // otherwise the envelope would not appear until the next edit of the reel
        let complete = self.ready.get(&key).is_some_and(|(e, _)| e.peaks.is_some());
        if complete
            || self.failed.contains_key(&key)
            || self.running.as_ref().is_some_and(|(k, _)| *k == key)
            || self.queue.iter().any(|(k, _)| *k == key)
        {
            return false;
        }
        let body = move |ctl: crate::bg::BgCtl, send: crate::bg::BgSend<ReelPartial>| {
            render_reel_preview(ctl, send, key, &project, base.as_deref(), index_db.as_deref())
        };
        self.queue.push_back((key, Box::new(body)));
        true
    }

    /// An edit inside the reel: old editions are no longer needed, and unfinished
    /// work on them even less so. The reel's negative cache is cleared ENTIRELY:
    /// invalidation is the only reason to try again (t69 for thumbnails).
    ///
    /// `keep` — the LIVE editions of this reel, not just one (R.59, stage 3):
    /// with overrides, one reel has as many editions as there are differing instances,
    /// and “keep one” would mean that two
    /// cuts would forever throw out each other's work.
    pub fn invalidate_seq(&mut self, seq: MediaId, keep: &std::collections::HashSet<u64>) {
        let stale = |k: &ReelKey| k.seq == seq && !keep.contains(&k.hash);
        self.ready.retain(|k, _| !stale(k));
        self.failed.retain(|k, _| k.seq != seq);
        self.queue.retain(|(k, _)| !stale(k));
        if let Some((k, j)) = &self.running
            && stale(k)
        {
            j.cancel();
            self.running = None;
        }
    }

    pub fn cancel_all(&mut self) {
        if let Some((_, j)) = &self.running {
            j.cancel();
        }
        self.queue.clear();
        self.running = None;
    }

    /// Pause for the time of playback and rendering: real time matters more
    /// than the picture on the strip (invariant 6).
    pub fn set_paused(&mut self, on: bool) {
        self.paused = on;
        if let Some((_, j)) = &self.running {
            j.set_paused(on);
        }
    }

    /// Advance the service: take the ready result, start the next one. `true` means something
    /// changed (a reason to redraw).
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        // take EVERYTHING from the job and release the borrow: putting it into the cache is already
        // work with `self`
        let taken = self.running.as_mut().map(|(key, job)| {
            let changed = job.poll();
            (*key, changed, job.progress.take(), job.done.take())
        });
        if let Some((key, changed, strip, done)) = taken {
            any |= changed;
            if let Some(strip) = strip {
                let mut e = self.get(&key).map(|a| (*a).clone()).unwrap_or_default();
                e.strip = Some(strip);
                self.put(key, e);
            }
            if let Some(done) = done {
                self.finish(key, done);
                self.running = None;
                any = true;
            }
        }
        // one job at a time: offline reel rendering decodes files, and three
        // at once would take the disk away from playback.
        if self.running.is_none()
            && let Some((key, body)) = self.queue.pop_front()
        {
            let job = crate::bg::BgJob::spawn("reel-preview", body);
            job.set_paused(self.paused);
            self.running = Some((key, job));
            any = true;
        }
        any
    }

    /// Put the ready result; over the cap, the reel that has gone longest without access is evicted
    /// (honest LRU, not an insertion queue).
    /// The task result as `poll` sees it (for test t795: a real background task
    /// cannot be cancelled precisely in the harness).
    #[cfg(test)]
    pub(crate) fn note_done_for_test(&mut self, key: ReelKey, done: ReelDone) {
        // the task has finished: it has already left the queue
        self.queue.retain(|(k, _)| *k != key);
        self.finish(key, done);
    }

    /// Ready (or half-ready) record — for test t795.
    #[cfg(test)]
    pub(crate) fn put_for_test(
        &mut self,
        key: ReelKey,
        strip: Option<ReelStrip>,
        peaks: Option<crate::util::Peaks>,
    ) {
        self.queue.retain(|(k, _)| *k != key);
        self.put(key, ReelPreview { strip, peaks });
    }

    /// Task result — ONE place (it is also called by test t795).
    ///
    /// CANCEL is not a failure (R.67): the task was cancelled by a pause, an edit or
    /// closing, and retrying it is legitimate. Only a render ERROR goes into the
    /// negative cache — otherwise the key would stay “failed” until the next
    /// edit of the reel, and the envelope would never appear.
    fn finish(&mut self, key: ReelKey, done: ReelDone) {
        match done {
            Ok((peaks, origin)) => {
                let mut e = self.get(&key).map(|a| (*a).clone()).unwrap_or_default();
                e.peaks = Some(peaks);
                self.put(key, e);
                if origin == ReelOrigin::Rendered {
                    self.renders += 1;
                }
            }
            Err(e) if e == CANCELLED => {}
            Err(e) => {
                if self.failed.len() >= MAX_FAILED {
                    self.failed.clear();
                }
                self.failed.insert(key, e);
            }
        }
    }

    pub(crate) fn put(&mut self, key: ReelKey, e: ReelPreview) {
        let stamp = Cell::new(0);
        self.touch(&stamp);
        self.ready.insert(key, (Arc::new(e), stamp));
        while self.ready.len() > MAX_REELS_READY {
            let Some(oldest) = self.ready.iter().min_by_key(|(_, (_, s))| s.get()).map(|(k, _)| *k)
            else {
                break;
            };
            self.ready.remove(&oldest);
        }
    }
}

/// Media the reel reads — including the reels nested within it.
fn used_media(p: &Project) -> Vec<MediaId> {
    let mut out: Vec<MediaId> = p
        .clips
        .iter()
        .map(|c| c.media)
        .chain(p.sequences.iter().flat_map(|s| s.clips.iter().map(|c| c.media)))
        .collect();
    out.sort_unstable();
    out.dedup();
    out.retain(|m| p.media.iter().any(|r| r.id == *m));
    out
}

/// Fingerprint of the reel tree's MEDIA (R.58): for each file — path, size and
/// mtime; for a pseudo-path — just a string. The reel's edit (`content_hash`)
/// does not see a file swap, and without this number a swapped source would show
/// the strip from before. One `fs::metadata` per media — called from the task
/// thread, not from the UI (invariant 8).
pub fn media_fingerprint(p: &Project, base: Option<&std::path::Path>) -> u64 {
    use xxhash_rust::xxh3::Xxh3;
    let mut h = Xxh3::new();
    for m in used_media(p) {
        let Some(r) = p.media.iter().find(|r| r.id == m) else { continue };
        let path = crate::util::resolve_media_path(base, &r.path);
        h.update(&m.0.to_le_bytes());
        h.update(path.as_bytes());
        h.update(&[0]);
        if kt_model::is_pseudo_path(&path) {
            continue;
        }
        match std::fs::metadata(&path) {
            Ok(md) => {
                let ns = md
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos() as i64)
                    .unwrap_or(0);
                h.update(&md.len().to_le_bytes());
                h.update(&ns.to_le_bytes());
            }
            Err(_) => h.update(b"missing"),
        }
    }
    h.digest()
}

/// A write error to the sidecar is a CACHE error: the preview still arrives, and the
/// log gets one line, not one line per reel.
fn warn_db_once(what: &str, e: &dyn std::fmt::Display) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("reel preview: sidecar {what} failed: {e}");
    }
}

/// Job body: FRAMES first (cheap and visible at once), then the envelope.
///
/// Its own short-lived sources — exactly the three conditions of invariant 5: not
/// shared with anyone, no promised pace, the job is finite and cancelable.
///
/// Disk tier (R.58): before any decode the sidecar is asked by key
/// (reel, edit, media fingerprint); a hit returns the strip and peaks from
/// disk, a miss renders and stores the result in one transaction. Frames in both
/// cases pass through the JPEG sidecar quality — display does not depend on
/// where the frame came from.
pub fn render_reel_preview(
    ctl: crate::bg::BgCtl,
    send: crate::bg::BgSend<ReelPartial>,
    key: ReelKey,
    p: &Project,
    base: Option<&std::path::Path>,
    index_db: Option<&std::path::Path>,
) -> ReelDone {
    let seq_row = key.seq.0 as i64;
    let fp = media_fingerprint(p, base);
    // an opening that fails is a cache miss (the reel renders anew) — but not a SILENT one:
    // a lost disk tier must be visible in the log (t640/t795 under load showed a re-render
    // with no word on why)
    let mut db = index_db.and_then(|d| match kt_media::index::MediaIndexDb::open(d) {
        Ok(db) => Some(db),
        Err(e) => {
            warn_db_once("open", &e);
            None
        }
    });
    // frames have already come from disk — we skip this stage, computing only the peaks
    let mut disk_frames = false;
    if let Some(db) = &db {
        match db.reel_preview(seq_row, key.hash, fp) {
            Ok(Some(row)) => {
                let frames = row
                    .frames
                    .iter()
                    .map(|j| j.as_deref().and_then(crate::thumbs::decode_thumb_jpeg).map(Arc::new))
                    .collect();
                let (step_disk, end_disk, has_peaks) = (row.step, row.end_ticks, row.has_peaks);
                send.send(ReelStrip { step: step_disk, frames, end: Tick(end_disk) });
                if has_peaks {
                    return Ok((crate::util::pyramid_to_peaks(row.peaks), ReelOrigin::Disk));
                }
                // FRAMES are on disk, PEAKS are not yet (the job was cancelled at the
                // audio stage): the frames have already been delivered, and we finish the envelope
                // here — there will be no second render of the frames.
                disk_frames = true;
            }
            Ok(None) => {}
            Err(e) => warn_db_once("read", &e),
        }
    }

    // sources are opened TWICE — once per pass: video and audio
    // read them differently, and sharing one source between passes would mean
    // sharing the read position too (invariant 5: exactly one owner).
    let open = || -> Vec<(MediaId, Box<dyn kt_media::MediaSource>)> {
        used_media(p)
            .into_iter()
            .filter_map(|m| {
                let r = p.media.iter().find(|r| r.id == m)?;
                let path = crate::util::resolve_media_path(base, &r.path);
                crate::util::open_source_indexed(&path, index_db).ok().map(|s| (m, s))
            })
            .collect()
    };

    let end = kt_engine::export::project_range(p);
    let step = ReelPreviews::strip_step(end);
    let q = Quality { width: REEL_THUMB_W, height: REEL_THUMB_H };

    // --- frames -------------------------------------------------------
    let mut facts = kt_engine::video::SourceFacts::default();
    let mut src: HashMap<MediaId, Box<dyn kt_media::MediaSource>> = HashMap::new();
    for (m, s) in open() {
        facts.observe(m, s.info());
        src.insert(m, s);
    }
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig {
        max_audio_bytes: 1,
        max_video_bytes: 64 << 20,
    });
    let mut video = kt_engine::VideoRenderer::new_with_facts(p, &facts);
    let n = ((end.0 + step - 1) / step).max(1) as usize;
    let mut frames: Vec<Option<Arc<egui::ColorImage>>> = vec![None; n.min(MAX_FRAMES_PER_REEL)];
    let mut jpegs: Vec<Option<Vec<u8>>> = vec![None; frames.len()];
    let mut buf = Vec::new();
    for i in 0..frames.len() {
        // frames have already been delivered from disk — we compute only the envelope (R.67)
        if disk_frames {
            break;
        }
        if ctl.should_stop() {
            return Err(CANCELLED.into());
        }
        let t = video.grid.align_floor(Tick(i as i64 * step));
        let mut per_src: HashMap<(MediaId, Quality), Vec<Tick>> = HashMap::new();
        for (media, pts, dq) in
            video.wanted_frames(kt_time::TickRange::new(t, Tick(t.0 + video.grid.frame_step())), q)
        {
            per_src.entry((media, dq)).or_default().push(pts);
        }
        for ((media, dq), keys) in per_src {
            let Some(s) = src.get_mut(&media) else { continue };
            let sid = kt_engine::source_id(media);
            let missing: Vec<Tick> = keys
                .into_iter()
                .filter(|k| {
                    cache.get_video(sid, Some(dq), kt_engine::video_cache_idx(*k)).is_none()
                })
                .collect();
            if missing.is_empty() {
                continue;
            }
            let _ = s.read_video_span(&missing, Some(dq), &mut |k, frame| {
                cache.insert_video(sid, Some(dq), kt_engine::video_cache_idx(k), frame.into());
            });
        }
        video.compose_from_cache_into(&cache, t, q, &mut buf);
        if buf.len() == (q.width * q.height * 4) as usize
            && let Some(j) = crate::thumbs::encode_thumb_jpeg(q.width, q.height, buf.clone())
        {
            frames[i] = crate::thumbs::decode_thumb_jpeg(&j).map(Arc::new);
            jpegs[i] = Some(j);
        }
    }
    // frames go out IMMEDIATELY: audio takes longer, and there is no point waiting for it
    // (they were already delivered from disk above — we do not hit the screen with an empty strip a second time)
    if !disk_frames {
        send.send(ReelStrip { step, frames, end });
    }
    drop(src);

    // ...and IMMEDIATELY to disk (R.67): a strip that has reached the screen must
    // survive to disk even if the audio stage is cancelled. Previously the write went
    // as one piece and only after the peaks — a cancelled task left nothing behind,
    // and the reel was re-rendered on every project open.
    if let Some(db) = db.as_mut()
        && !disk_frames
    {
        let refs: Vec<Option<&[u8]>> = jpegs.iter().map(|j| j.as_deref()).collect();
        if let Err(e) = db.put_reel_frames(seq_row, key.hash, fp, step, end.0, &refs) {
            warn_db_once("write frames", &e);
        } else {
            let keep: Vec<i64> =
                std::iter::once(seq_row).chain(p.sequences.iter().map(|s| s.id.0 as i64)).collect();
            if let Err(e) = db.prune_reels(&keep) {
                warn_db_once("prune", &e);
            }
        }
    }

    // --- envelope ---------------------------------------------------
    if ctl.should_stop() {
        return Err(CANCELLED.into());
    }
    let mut acc = kt_media::index::PeakAcc::default();
    let cancel = ctl.cancel_flag().clone();
    kt_engine::export::render_raw_audio_range(
        p,
        open(),
        None,
        &cancel,
        |_| {},
        |chunk| {
            acc.push(chunk);
            Ok(())
        },
    )
    .map_err(|e| e.to_string())?;
    let pyramid = acc.finish();

    // --- ENVELOPE to disk --------------------------------------------
    // In its own transaction, to the row with the same key: the frames are already there.
    if let Some(db) = db.as_mut()
        && let Err(e) = db.put_reel_peaks(seq_row, key.hash, fp, &pyramid)
    {
        warn_db_once("write peaks", &e);
    }
    Ok((crate::util::pyramid_to_peaks(pyramid), ReelOrigin::Rendered))
}
