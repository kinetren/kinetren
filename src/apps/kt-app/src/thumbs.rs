//! ThumbService: filmstrip thumbnails for video clips.
//!
//! Does NOT touch the playback decoders (a correction to invariant No. 8: paint
//! may only ENQUEUE requests to this dedicated worker). Its own
//! short-lived FfmpegSource per media; the jpeg is persisted to the sidecar
//! index DB (table thumbnails) — reopening the project draws
//! the strip without decode.

use crate::util::{open_source, resolve_media_path};
use eframe::egui;
use kt_media::Quality;
use kt_model::{MediaId, Project};
use kt_time::Tick;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const THUMB_W: u32 = 320;
pub const THUMB_H: u32 = 180;

/// Thumb size BY THE SOURCE'S LOOK (R.62, t719): the look (width × SAR,
/// height) fits into the frame `THUMB_W × THUMB_H`, sides even. Before
/// R.62 the order was exactly 320×180, and the decoder returns a frame of exactly the requested
/// quality — a vertical clip came out squashed, and the filmstrip was
/// the first place where the person saw a conform defect.
pub fn thumb_quality(view: (f64, f64)) -> Quality {
    let (vw, vh) = (view.0.max(1.0), view.1.max(1.0));
    let k = (THUMB_W as f64 / vw).min(THUMB_H as f64 / vh);
    let even = |v: f64| ((v.round() as u32).max(2)) & !1;
    Quality { width: even(vw * k), height: even(vh * k) }
}

/// Placement of a thumb in a filmstrip cell: the picture fits by the cell's HEIGHT;
/// a narrow one is centered, with margins at the sides — the event's body (the row color);
/// a wide one (wider than the cell — the event's edge) keeps its scale
/// and is cut on the right, as before. Returns (offset from the cell's left edge,
/// full width of the picture); visible width = `min(fit_w, cell_w − off)`.
pub fn thumb_cell_fit(cell_w: f32, cell_h: f32, tex_w: f32, tex_h: f32) -> (f32, f32) {
    let aspect = if tex_h > 0.0 { tex_w / tex_h } else { 16.0 / 9.0 };
    let fit_w = (cell_h * aspect).max(1.0);
    (((cell_w - fit_w) * 0.5).max(0.0), fit_w)
}
/// JPEG quality in the sidecar — one for the file thumbs and the reel strip frames
/// (`reel_frame`, R.58): they are drawn side by side, by one code path.
pub const THUMB_JPEG_QUALITY: u8 = 80;

/// RGBA → JPEG at sidecar quality. `None` — the size does not match the buffer
/// or the encoder refused.
pub(crate) fn encode_thumb_jpeg(width: u32, height: u32, rgba: Vec<u8>) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, THUMB_JPEG_QUALITY);
    let img = image::RgbaImage::from_raw(width, height, rgba)?;
    image::DynamicImage::ImageRgba8(img).write_with_encoder(enc).ok()?;
    Some(buf)
}

/// JPEG from the sidecar → picture for a texture. Thumbs and reel frames go
/// through this on a FRESH render too: display does not depend on where the
/// frame came from — from disk or from under the encoder.
pub(crate) fn decode_thumb_jpeg(jpeg: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory_with_format(jpeg, image::ImageFormat::Jpeg).ok()?;
    let rgba = img.to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}
/// LRU of ready textures (port of kMaxPreviewCacheEntries=384).
pub const MAX_READY: usize = 384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThumbKey {
    pub media: MediaId,
    /// A moment in ticks, aligned by the caller to the step grid.
    pub t: i64,
}

/// Source step for thumbnails (port of videoClipThumbnailSourceStepMs):
/// one thumb for a clip ≤5 s, otherwise dur/slots, clamped to 1..15 s.
/// The thumb step is QUANTIZED to a fixed ladder (item 2): a continuous
/// dependence on zoom produced a new set of keys on every wheel click —
/// the filmstrip was reloaded entirely. Adjacent rungs reuse the cache
/// (the keys of the coarse rung are a subset of the fine one's).
pub const THUMB_STEP_LADDER: [i64; 7] =
    [45_000, 90_000, 180_000, 450_000, 1_350_000, 4_050_000, 10_800_000]; // 0.5/1/2/5/15/45 s/2 min

/// Thumb key: the source moment, quantized to the step grid FROM THE FILE'S ZERO
/// (item 11 round 6, test 62). Previously the timeline quantized from source_in — with a
/// source_in not a multiple of the step, the bin and the timeline produced DIFFERENT keys for one
/// moment of the file (double decode + a swollen sidecar DB).
pub fn thumb_key_t(source_in: i64, timeline_local: i64, step: i64) -> i64 {
    (((source_in + timeline_local).max(0)) / step.max(1)) * step.max(1)
}

/// Thumb key for the moment `tl` in ticks FROM THE START OF THE EVENT — ONE formula
/// “pixel → source frame” for the filmstrip and for the picker's thumbnails (R.54).
///
/// R.37 item 3 already required that the event's preview compute the SAME map
/// as the render, and moved the stretch into `ClipWarp::material_off`. One hole remained,
/// and it was found while assembling the picker: a reversed event WITHOUT a
/// warp has no map (`clip_warp` returns `None` — reverse is not a warp), and
/// the old `map_or(tl, …)` showed the material DIRECTLY. That is, the filmstrip of
/// a reversed event drew not what plays: the first column is not
/// the last frame of the piece, but the first.
pub fn material_at(
    warp: Option<&kt_model::ClipWarp>,
    duration: Tick,
    reversed: bool,
    tl: i64,
) -> i64 {
    match warp {
        // the warp itself knows about reverse (the axis — the actually consumed one)
        Some(w) => w.material_off(duration, reversed, tl),
        None if reversed => (duration.0 - 1 - tl).max(0),
        None => tl,
    }
}

/// The same translation, quantized to the step grid FROM THE FILE'S ZERO — the ready
/// thumb key (shared cache bin↔timeline↔picker, test 62).
pub fn material_key_t(
    source_in: i64,
    warp: Option<&kt_model::ClipWarp>,
    duration: Tick,
    reversed: bool,
    tl: i64,
    step: i64,
) -> i64 {
    thumb_key_t(source_in, material_at(warp, duration, reversed, tl), step)
}

pub fn thumb_source_step(clip_dur: Tick, inner_width_px: f32, thumb_w_px: f32) -> i64 {
    if clip_dur.0 <= 5 * 90_000 {
        // short clip: one thumb, the key is stable (a multiple of 0.5 s)
        return THUMB_STEP_LADDER[0].max(clip_dur.0 / 2).max(45_000);
    }
    let slots = (inner_width_px / thumb_w_px).max(1.0) as i64;
    let rough = clip_dur.0 / slots.max(1);
    // the nearest rung ABOVE (rarer — cheaper)
    *THUMB_STEP_LADDER
        .iter()
        .find(|&&s| s >= rough)
        .unwrap_or(&THUMB_STEP_LADDER[THUMB_STEP_LADDER.len() - 1])
}

/// Thumb state for the UI: Failed is NOT re-requested (item 1 round 7 — the eternal
/// cycle “request → seek+reopen → error” was spinning the FFmpeg-open counter).
#[derive(Debug, PartialEq)]
pub enum ThumbState {
    Ready(Arc<egui::ColorImage>),
    Failed,
    Missing,
}

/// The worker's cache of open sources: LRU evicting the OLDEST (item 1 round 7:
/// the previous `clear()` at >2 meant a full reopening of every file at
/// ≥4 active media — an FFmpeg-open storm). Generic for the unit test without FFmpeg.
pub(crate) struct SourceLru<V> {
    cap: usize,
    map: HashMap<MediaId, V>,
    order: VecDeque<MediaId>,
}

impl<V> SourceLru<V> {
    pub fn new(cap: usize) -> Self {
        SourceLru { cap: cap.max(1), map: HashMap::new(), order: VecDeque::new() }
    }
    pub fn contains(&self, id: MediaId) -> bool {
        self.map.contains_key(&id)
    }
    pub fn get_mut(&mut self, id: MediaId) -> Option<&mut V> {
        if self.map.contains_key(&id) {
            self.order.retain(|k| *k != id);
            self.order.push_back(id);
        }
        self.map.get_mut(&id)
    }
    /// Discard a source (R.53, item 412): its parameters were reconfigured, and
    /// keeping the old one open would mean drawing old thumbs.
    pub fn remove(&mut self, id: MediaId) {
        self.map.remove(&id);
        self.order.retain(|k| *k != id);
    }
    pub fn insert(&mut self, id: MediaId, v: V) {
        self.map.insert(id, v);
        self.order.retain(|k| *k != id);
        self.order.push_back(id);
        while self.map.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

/// Worker queue: LIFO of pairs (key, near-tolerance) + a dedup set of keys.
type ThumbQueue = (VecDeque<(ThumbKey, i64)>, HashSet<ThumbKey>);

/// Limit on simultaneously open sources of the thumb worker.
const MAX_OPEN_SOURCES: usize = 4;
/// Cap of the thumb negative cache (overflow means a full clear, cheap).
const MAX_FAILED: usize = 4096;

struct Shared {
    /// Media durations. `None` = “tried to open, did not work” —
    /// a repeat request is NOT queued (item 1 round 7: the eternal probe cycle was opening
    /// the file on every UI frame).
    durations: Mutex<HashMap<MediaId, Option<Tick>>>,
    /// Explicit PENDING requests for a duration (the worker removes them by publishing
    /// the result — success or failure).
    probe: Mutex<std::collections::HashSet<MediaId>>,
    cond: std::sync::Condvar,
    /// LIFO + dedup by key; the second element of the pair is the near-search tolerance in the DB
    /// (ticks): 0 = only the exact key (timeline), >0 — scrubbing the bin may
    /// show the nearest ready thumb (item 14 round 7). ThumbKey is not extended —
    /// the shared cache bin↔timeline (test 62).
    queue: Mutex<ThumbQueue>,
    ready: Mutex<HashMap<ThumbKey, Arc<egui::ColorImage>>>,
    order: Mutex<VecDeque<ThumbKey>>, // for LRU eviction of ready
    /// Negative cache: keys for which opening/decoding/encoding failed.
    failed: Mutex<HashSet<ThumbKey>>,
    /// Posters: the moment of the most informative frame of the media (item 4 round 9).
    /// `None` inside — “searched, no informative frame” (audio/broken).
    posters: Mutex<HashMap<MediaId, Option<i64>>>,
    /// Requests for a poster search (served after the thumbs).
    poster_jobs: Mutex<std::collections::VecDeque<MediaId>>,
    /// Whether the media has video/audio (published by the worker's probe branch).
    kinds: Mutex<HashMap<MediaId, (bool, bool)>>,
    /// The LIVE media map: it is filled when files are added (item 3 — previously the
    /// map was frozen at start and new media got no thumbs).
    media: Mutex<HashMap<MediaId, String>>,
    /// Media whose OPEN source the worker must discard (R.53,
    /// item 412): after reconfiguring the generator's parameters its `SourceLru`
    /// would keep the source with the old parameters and draw old thumbs.
    drop_sources: Mutex<HashSet<MediaId>>,
    /// How many NEW requests were queued over the whole time — for the guard
    /// “scroll/zoom does not request decode” (invariant 8, t719): the queue
    /// is emptied by the worker itself, so its length proves nothing.
    requests: std::sync::atomic::AtomicUsize,
    /// Journal of requested keys — only under `cfg!(test)`: the harness learns
    /// which keys the filmstrip asked for, without repeating its arithmetic.
    request_log: Mutex<Vec<ThumbKey>>,
}

pub struct ThumbService {
    shared: Arc<Shared>,
}

impl ThumbService {
    pub fn start(project: &Project, project_path: Option<PathBuf>) -> ThumbService {
        let shared = Arc::new(Shared {
            durations: Mutex::new(HashMap::new()),
            probe: Mutex::new(std::collections::HashSet::new()),
            cond: std::sync::Condvar::new(),
            queue: Mutex::new((VecDeque::new(), HashSet::new())),
            ready: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            failed: Mutex::new(HashSet::new()),
            posters: Mutex::new(HashMap::new()),
            poster_jobs: Mutex::new(std::collections::VecDeque::new()),
            kinds: Mutex::new(HashMap::new()),
            media: Mutex::new(HashMap::new()),
            drop_sources: Mutex::new(HashSet::new()),
            requests: std::sync::atomic::AtomicUsize::new(0),
            request_log: Mutex::new(Vec::new()),
        });
        let media: HashMap<MediaId, String> = {
            let base = project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
            project
                .media
                .iter()
                // synthetic (test schema) has no thumbs; generators
                // (`gen:`) DO: their frame is drawn by the same factory (R.52)
                .filter(|m| kt_media::factory::synth_pattern(&m.path).is_none())
                .map(|m| (m.id, resolve_media_path(base.as_deref(), &m.path)))
                .collect()
        };
        *shared.media.lock().unwrap() = media;
        let db_path = project_path.map(|p| p.with_extension("ktindex.db"));
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("thumbs".into())
            .spawn(move || worker(sh, db_path))
            .expect("spawn thumbs");
        ThumbService { shared }
    }

    /// How many requests are waiting for the worker — for the guard “scroll/zoom does not request
    /// decode” (invariant 8, t719).
    #[cfg(test)]
    pub(crate) fn requested_total(&self) -> usize {
        self.shared.requests.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn requested_keys(&self) -> Vec<ThumbKey> {
        self.shared.request_log.lock().unwrap().clone()
    }

    /// Enqueue (dedup); drawing is never blocked.
    /// Register new media (adding a file to the project) — thumbs
    /// start building at once, without restarting the service.
    pub fn add_media(&self, id: MediaId, resolved_path: String) {
        if kt_media::factory::synth_pattern(&resolved_path).is_none() {
            self.shared.media.lock().unwrap().insert(id, resolved_path);
            // new/reinstalled media: the negative caches are stale
            self.invalidate_media(id);
        }
    }

    /// Reset the media negative caches (file is online again / reinstalled).
    pub fn invalidate_media(&self, id: MediaId) {
        self.shared.durations.lock().unwrap().remove(&id);
        self.shared.failed.lock().unwrap().retain(|k| k.media != id);
    }

    /// The media stayed the same, but the PICTURE changed (R.53, item 412:
    /// the generator's parameters were reconfigured). Negative caches are not enough —
    /// the READY thumbs, the poster, the media's look, and the open
    /// source in the worker went stale too: the key `ThumbKey{media,t}` does not depend
    /// on the parameters, so the card would show the old frame (B5).
    pub fn retune_media(&self, id: MediaId, resolved_path: String) {
        self.shared.media.lock().unwrap().insert(id, resolved_path);
        self.shared.ready.lock().unwrap().retain(|k, _| k.media != id);
        self.shared.order.lock().unwrap().retain(|k| k.media != id);
        self.shared.posters.lock().unwrap().remove(&id);
        self.shared.kinds.lock().unwrap().remove(&id);
        self.shared.drop_sources.lock().unwrap().insert(id);
        self.invalidate_media(id);
        self.shared.cond.notify_all();
    }

    pub fn request(&self, key: ThumbKey) {
        self.request_near(key, 0);
    }

    /// A request with near-search tolerance in the sidecar DB: the worker may show the
    /// NEAREST ready thumb within tol ticks instead of decoding (bin scrubbing,
    /// item 14 round 7 — filmstrip thumbs with a 0.5–2 s step are reused).
    pub fn request_near(&self, key: ThumbKey, tol: i64) {
        if self.shared.failed.lock().unwrap().contains(&key) {
            return; // negative cache: do not spin up the decode again
        }
        let mut q = self.shared.queue.lock().unwrap();
        if q.1.insert(key) {
            self.shared.requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if cfg!(test) {
                self.shared.request_log.lock().unwrap().push(key);
            }
            q.0.push_back((key, tol)); // pop_back = LIFO: the fresh ones first
            drop(q);
            self.shared.cond.notify_one();
        }
    }

    pub fn get(&self, key: &ThumbKey) -> Option<Arc<egui::ColorImage>> {
        self.shared.ready.lock().unwrap().get(key).cloned()
    }

    /// Key state: Failed should NOT be re-requested (the bin must not
    /// hold an eternal repaint cycle on it).
    pub fn state(&self, key: &ThumbKey) -> ThumbState {
        if let Some(img) = self.get(key) {
            return ThumbState::Ready(img);
        }
        if self.shared.failed.lock().unwrap().contains(key) {
            return ThumbState::Failed;
        }
        ThumbState::Missing
    }

    /// The poster moment (informative frame) or a REQUEST for its search —
    /// exactly one per media per session (item 4 round 9).
    pub fn poster_t_or_request(&self, id: MediaId) -> Option<i64> {
        if let Some(p) = self.shared.posters.lock().unwrap().get(&id) {
            return *p;
        }
        let mut jobs = self.shared.poster_jobs.lock().unwrap();
        if !jobs.contains(&id) {
            jobs.push_back(id);
            self.shared.cond.notify_one();
        }
        None
    }

    /// Whether the media has video and audio (for the bin card: audio is drawn as
    /// a waveform, rather than running the thumb path in vain).
    pub fn kinds(&self, id: MediaId) -> Option<(bool, bool)> {
        self.shared.kinds.lock().unwrap().get(&id).copied()
    }

    /// Media duration, if the worker has already opened the source (item 11 round 6).
    pub fn duration(&self, id: MediaId) -> Option<Tick> {
        self.shared.durations.lock().unwrap().get(&id).copied().flatten()
    }

    /// The duration or a ONE-TIME request for it: a repeat request is not queued
    /// either while pending (probe) or after a failure (durations=None) —
    /// item 1 round 7: the previous variant opened the file on every UI frame.
    pub fn duration_or_probe(&self, id: MediaId) -> Option<Tick> {
        if let Some(d) = self.shared.durations.lock().unwrap().get(&id) {
            return *d;
        }
        if self.shared.probe.lock().unwrap().insert(id) {
            self.shared.cond.notify_one();
        }
        None
    }
}

fn worker(shared: Arc<Shared>, db_path: Option<PathBuf>) {
    let mut db = db_path.and_then(|p| kt_media::index::MediaIndexDb::open(&p).ok());
    // LRU of open sources (item 1 round 7: previously clear() at >2)
    let mut sources: SourceLru<Box<dyn kt_media::MediaSource>> = SourceLru::new(MAX_OPEN_SOURCES);
    let mut db_ids: HashMap<MediaId, i64> = HashMap::new();
    loop {
        // discard sources whose parameters were reconfigured (item 412):
        // otherwise the worker would keep drawing with the old GeneratorSource
        {
            let dropped: Vec<MediaId> = shared.drop_sources.lock().unwrap().drain().collect();
            for id in dropped {
                sources.remove(id);
                db_ids.remove(&id);
            }
        }
        // condvar instead of 33 wakeups/s on an empty queue (item 1);
        // probe requests wake the loop even WITHOUT keys in the queue (item 1 round 7 —
        // otherwise they starve when the failed filter empties the queue)
        let next = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if let Some(kt) = q.0.pop_back() {
                    q.1.remove(&kt.0);
                    break Some(kt);
                }
                if !shared.probe.lock().unwrap().is_empty()
                    || !shared.poster_jobs.lock().unwrap().is_empty()
                {
                    break None;
                }
                let (guard, _timeout) =
                    shared.cond.wait_timeout(q, std::time::Duration::from_secs(2)).unwrap();
                q = guard;
            }
        };
        // duration requests (bin scrubbing): the packet index makes
        // opening cheap, no decode. The result is ALWAYS published —
        // failure too (None), so the UI does not re-request (item 1 round 7).
        let probes: Vec<MediaId> = shared.probe.lock().unwrap().drain().collect();
        for pid in probes {
            if shared.durations.lock().unwrap().contains_key(&pid) {
                continue;
            }
            let dur = shared.media.lock().unwrap().get(&pid).cloned().and_then(|path| {
                let opened: Option<Box<dyn kt_media::MediaSource>> =
                    if kt_model::is_pseudo_path(&path) || db.is_none() {
                        open_source(&path).ok()
                    } else {
                        kt_media::ffmpeg::FfmpegSource::open_indexed(&path, db.as_mut())
                            .ok()
                            .map(|s| Box::new(s) as _)
                    };
                opened.map(|src| {
                    let info = src.info();
                    // media look is obtained for free: the source is already open
                    shared.kinds.lock().unwrap().insert(pid, (info.has_video, info.has_audio));
                    info.duration
                })
            });
            shared.durations.lock().unwrap().insert(pid, dur);
        }
        // poster requests (item 4 round 9): we look for the informative frame — first among
        // ready thumbs in the DB (no decode at all), then by decoding
        // several candidates. Exactly one attempt per media: the result
        // (incl. “no frame”) is written to the DB and to memory.
        let poster_job = shared.poster_jobs.lock().unwrap().pop_front();
        if let Some(pid) = poster_job
            && !shared.posters.lock().unwrap().contains_key(&pid)
        {
            let found = find_poster(&shared, &mut db, &mut sources, pid);
            shared.posters.lock().unwrap().insert(pid, found);
        }
        let Some((key, near_tol)) = next else {
            continue;
        };
        let fail = |shared: &Shared, key: ThumbKey| {
            let mut f = shared.failed.lock().unwrap();
            if f.len() >= MAX_FAILED {
                f.clear();
            }
            f.insert(key);
        };
        if shared.ready.lock().unwrap().contains_key(&key) {
            continue;
        }
        let Some(path) = shared.media.lock().unwrap().get(&key.media).cloned() else {
            fail(&shared, key); // unknown media: do not request forever
            continue;
        };
        let path = &path;

        // 1) try the DB: exact key, then the nearest within tolerance (item 14 round 7).
        // Pseudo-sources (`gen:`, `synth:`) do not go to the DB at all (R.53,
        // item 413): its key is the FILE's size and mtime, which they do not have, and
        // `ensure_media` on every thumb would needlessly hit sqlite.
        let mut jpeg: Option<Vec<u8>> = None;
        let use_db = !kt_model::is_pseudo_path(path);
        if let Some(db) = &mut db
            && use_db
        {
            let db_id = *db_ids.entry(key.media).or_insert_with(|| {
                db.ensure_media(std::path::Path::new(path)).map(|(id, _)| id).unwrap_or(-1)
            });
            // thumbs written BEFORE the conform to the source's look are squashed:
            // without a thumb_meta row the sidecar is not read, but rebuilt
            if db_id >= 0 && db.thumb_size(db_id).ok().flatten().is_some() {
                jpeg = db.thumbnail(db_id, key.t).ok().flatten();
                if jpeg.is_none() && near_tol > 0 {
                    // the nearest ready thumb instead of decoding; published under the
                    // REQUESTED key, not written to the DB
                    jpeg =
                        db.thumbnail_near(db_id, key.t, near_tol).ok().flatten().map(|(_t, j)| j);
                }
            }
        }
        // 2) decode on a miss (+ write of the exact key to the DB)
        if jpeg.is_none() {
            if !sources.contains(key.media) {
                // synthetic — via open_source; files — via open_indexed
                // with the DB already open (reusing the packet index, test 34)
                let opened: Option<Box<dyn kt_media::MediaSource>> =
                    if kt_model::is_pseudo_path(path) || db.is_none() {
                        open_source(path).ok()
                    } else {
                        kt_media::ffmpeg::FfmpegSource::open_indexed(path, db.as_mut())
                            .ok()
                            .map(|s| Box::new(s) as _)
                    };
                match opened {
                    Some(src) => sources.insert(key.media, src),
                    None => {
                        // NOT a dummy source (item 1 round 7): an honest failure
                        shared.durations.lock().unwrap().entry(key.media).or_insert(None);
                        fail(&shared, key);
                        continue;
                    }
                }
            }
            let src = sources.get_mut(key.media).expect("source just inserted");
            // Duration AND LOOK of the media: the source is open, both come
            // for free. Only the probe branch published the look, and it skips itself
            // when the duration is already known (“continue” on
            // `durations.contains_key`) — a media for which a THUMB was asked for first
            // never got its look (R.53, item 416).
            // This showed as a black square instead of a waveform on an
            // audio card that entered the frame before its probe.
            shared.durations.lock().unwrap().insert(key.media, Some(src.info().duration));
            shared
                .kinds
                .lock()
                .unwrap()
                .insert(key.media, (src.info().has_video, src.info().has_audio));
            let mut ok = false;
            let tq = thumb_quality(src.info().view_size());
            if let Ok(frame) = src.read_video_frame(Tick(key.t), Some(tq))
                && let Some(buf) = encode_thumb_jpeg(frame.width, frame.height, frame.rgba.into())
            {
                if let (Some(db), Some(&db_id)) = (&db, db_ids.get(&key.media))
                    && db_id >= 0
                {
                    let _ = db.put_thumbnail(db_id, key.t, &buf);
                    let _ = db.put_thumb_size(db_id, frame.width, frame.height);
                }
                jpeg = Some(buf);
                ok = true;
            }
            if !ok {
                fail(&shared, key); // negative cache: without an eternal seek+reopen
                continue;
            }
        }
        // 3) jpeg -> ColorImage into ready (+LRU)
        if let Some(j) = jpeg
            && let Some(ci) = decode_thumb_jpeg(&j)
        {
            let mut ready = shared.ready.lock().unwrap();
            let mut order = shared.order.lock().unwrap();
            ready.insert(key, Arc::new(ci));
            order.push_back(key);
            while order.len() > MAX_READY {
                if let Some(old) = order.pop_front() {
                    ready.remove(&old);
                }
            }
        }
    }
}

/// “Informativeness” of a frame: the spread of average brightness across 16×9 blocks.
/// A black/single-color/blown-out frame gives ~0 — exactly such frames
/// sit at the start of clips and turned the bin card into a black square.
fn frame_score(rgba: &[u8], w: u32, h: u32) -> i64 {
    if w == 0 || h == 0 || rgba.len() < (w as usize * h as usize * 4) {
        return 0;
    }
    const BX: u32 = 16;
    const BY: u32 = 9;
    let mut means = [0f32; (BX * BY) as usize];
    let mut global = 0f32;
    for by in 0..BY {
        for bx in 0..BX {
            let (x0, x1) = (bx * w / BX, ((bx + 1) * w / BX).max(bx * w / BX + 1));
            let (y0, y1) = (by * h / BY, ((by + 1) * h / BY).max(by * h / BY + 1));
            let mut sum = 0f32;
            let mut n = 0f32;
            // sampling every 2nd pixel along both axes — enough with margin
            let mut y = y0;
            while y < y1.min(h) {
                let mut x = x0;
                while x < x1.min(w) {
                    let i = ((y * w + x) * 4) as usize;
                    let l = 0.299 * rgba[i] as f32
                        + 0.587 * rgba[i + 1] as f32
                        + 0.114 * rgba[i + 2] as f32;
                    sum += l;
                    n += 1.0;
                    x += 2;
                }
                y += 2;
            }
            let m = if n > 0.0 { sum / n } else { 0.0 };
            means[(by * BX + bx) as usize] = m;
            global += m;
        }
    }
    let cnt = (BX * BY) as f32;
    let mean = global / cnt;
    // solid black/blown-out — uninformative regardless of spread
    if !(8.0..=247.0).contains(&mean) {
        return 0;
    }
    let var = means.iter().map(|m| (m - mean) * (m - mean)).sum::<f32>() / cnt;
    var.sqrt() as i64
}

/// Poster candidates: fractions of the duration (lazily, up to 5 attempts —
/// the user's decision). Quantized with the same step as the filmstrip —
/// the keys land in the shared cache (test 62).
fn poster_candidates(dur: Tick) -> Vec<i64> {
    let step = THUMB_STEP_LADDER[0];
    let mut v: Vec<i64> = [0.02, 0.10, 0.25, 0.50]
        .iter()
        .map(|f| thumb_key_t(0, (dur.0 as f64 * f) as i64, step))
        .collect();
    v.push(0);
    v.sort_unstable();
    v.dedup();
    v
}

/// Minimum brightness spread at which a frame counts as informative.
const POSTER_MIN_SCORE: i64 = 6;

/// The poster moment for media WITHOUT a duration (generators): the middle of a
/// five-second clip, which the UI sets on drop.
pub const GEN_POSTER_T: i64 = 90_000 * 5 / 2;

fn find_poster(
    shared: &Shared,
    db: &mut Option<kt_media::index::MediaIndexDb>,
    sources: &mut SourceLru<Box<dyn kt_media::MediaSource>>,
    media: MediaId,
) -> Option<i64> {
    let path = shared.media.lock().unwrap().get(&media).cloned()?;
    let db_id = db
        .as_mut()
        .and_then(|d| d.ensure_media(std::path::Path::new(&path)).ok())
        .map(|(id, _)| id);
    // already searched in a previous session — take it from the DB
    if let (Some(d), Some(id)) = (db.as_ref(), db_id)
        && let Ok(Some(t)) = d.poster(id)
    {
        return (t >= 0).then_some(t);
    }
    let dur = shared.durations.lock().unwrap().get(&media).copied().flatten();
    // “NO DURATION” ≠ “not measured yet” (R.53, item 424). A generator's is honestly zero,
    // and the poster at t = 0 of an animated title is empty:
    // the typewriter at zero has not printed a single letter. We take a representative
    // moment — the middle of the length that the UI substitutes on drop.
    if dur == Some(Tick(0)) {
        return Some(GEN_POSTER_T);
    }
    let Some(dur) = dur.filter(|d| d.0 > 0) else {
        return None; // duration not yet known — the request will return later
    };
    let mut best: Option<(i64, i64)> = None; // (score, t)
    for t in poster_candidates(dur) {
        // 1) the frame could already have been built by the filmstrip — let's estimate without decode
        let ready = shared.ready.lock().unwrap().get(&ThumbKey { media, t }).cloned();
        let score = if let Some(img) = ready {
            let px: Vec<u8> = img.as_raw().to_vec();
            frame_score(&px, img.width() as u32, img.height() as u32)
        } else {
            // 2) decode of the candidate
            if !sources.contains(media) {
                let opened: Option<Box<dyn kt_media::MediaSource>> =
                    if kt_model::is_pseudo_path(&path) || db.is_none() {
                        open_source(&path).ok()
                    } else {
                        kt_media::ffmpeg::FfmpegSource::open_indexed(&path, db.as_mut())
                            .ok()
                            .map(|s| Box::new(s) as _)
                    };
                match opened {
                    Some(src) => sources.insert(media, src),
                    None => break,
                }
            }
            let src = sources.get_mut(media).expect("source just inserted");
            let tq = thumb_quality(src.info().view_size());
            match src.read_video_frame(Tick(t), Some(tq)) {
                Ok(f) => frame_score(&f.rgba, f.width, f.height),
                Err(_) => break,
            }
        };
        if best.as_ref().is_none_or(|(bs, _)| score > *bs) {
            best = Some((score, t));
        }
        // lazily: the first sufficiently “rich” frame is ours
        if score >= POSTER_MIN_SCORE {
            break;
        }
    }
    let chosen = best.filter(|(s, _)| *s > 0).map(|(_, t)| t);
    if let (Some(d), Some(id)) = (db.as_ref(), db_id) {
        let _ = d.put_poster(id, chosen.unwrap_or(-1), best.map(|(s, _)| s).unwrap_or(0));
    }
    chosen
}

#[cfg(test)]
mod key_tests {
    use super::*;

    /// Test 719 (R.62): the thumb is in the proportions of the source's LOOK, not the frame.
    /// A vertical 1080×1920 → 100×180 (even), anamorphic 720×576 with SAR
    /// 16:15 → look 768×576 → 240×180, wide 16:9 — the former 320×180
    /// (cache and goldens do not move). Placement in the cell centers it.
    #[test]
    fn t719_thumb_quality_follows_the_view_and_cell_fit_centres() {
        assert_eq!(thumb_quality((1920.0, 1080.0)), Quality { width: 320, height: 180 });
        assert_eq!(thumb_quality((1080.0, 1920.0)), Quality { width: 100, height: 180 });
        assert_eq!(thumb_quality((768.0, 576.0)), Quality { width: 240, height: 180 });
        assert_eq!(thumb_quality((640.0, 480.0)), Quality { width: 240, height: 180 });
        assert_eq!(thumb_quality((4000.0, 3000.0)), Quality { width: 240, height: 180 });
        let q = thumb_quality((1.0, 1.0));
        assert!(q.width % 2 == 0 && q.height % 2 == 0 && q.width >= 2 && q.height >= 2);
        // a 16:9 cell of height 36: the wide thumb fills it entirely…
        assert_eq!(thumb_cell_fit(64.0, 36.0, 320.0, 180.0), (0.0, 64.0));
        // …a vertical one fits by height and stands centered
        let (off, w) = thumb_cell_fit(64.0, 36.0, 100.0, 180.0);
        assert!((w - 20.0).abs() < 1e-3, "width by cell height: {w}");
        assert!((off - 22.0).abs() < 1e-3, "centering: {off}");
        // a cell already past the picture (the event's edge): offset zero, scale
        // as before — the picture is cut on the right, not squeezed
        let (off, w) = thumb_cell_fit(10.0, 36.0, 320.0, 180.0);
        assert_eq!((off, w), (0.0, 64.0));
        // a narrow picture in a cell cut by the event's edge is centered
        // within what remains
        let (off, w) = thumb_cell_fit(30.0, 36.0, 100.0, 180.0);
        assert!((w - 20.0).abs() < 1e-3 && (off - 5.0).abs() < 1e-3, "{off} {w}");
    }

    /// Test 87 (item 4 round 9): the informativeness metric of a frame — black,
    /// white and single-color frames give 0, a picture with structure gives more
    /// than the threshold. Because of the hard t=0, bin cards were black squares.
    #[test]
    fn t87_frame_score_rejects_flat_frames() {
        use super::{POSTER_MIN_SCORE, frame_score, poster_candidates};
        let (w, h) = (64u32, 36u32);
        let flat = |v: u8| [v, v, v, 255].repeat((w * h) as usize);
        assert_eq!(frame_score(&flat(0), w, h), 0, "black frame");
        assert_eq!(frame_score(&flat(255), w, h), 0, "white frame");
        assert_eq!(frame_score(&flat(128), w, h), 0, "single-color frame");
        // “checkerboard” with large blocks — informative by definition
        let mut img = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let v = if (x / 8 + y / 6) % 2 == 0 { 20 } else { 230 };
                img[i] = v;
                img[i + 1] = v;
                img[i + 2] = v;
                img[i + 3] = 255;
            }
        }
        assert!(
            frame_score(&img, w, h) >= POSTER_MIN_SCORE,
            "a structural frame must pass the threshold: {}",
            frame_score(&img, w, h)
        );
        // candidates: no more than five, multiples of the filmstrip step, no duplicates
        let c = poster_candidates(Tick(90_000 * 600));
        assert!(c.len() <= 5, "more than five candidates: {c:?}");
        assert!(c.iter().all(|t| t % THUMB_STEP_LADDER[0] == 0), "not a multiple of the step: {c:?}");
        let mut sorted = c.clone();
        sorted.dedup();
        assert_eq!(sorted, c, "duplicates among the candidates");
        // a short file must not give negative/duplicate positions
        let short = poster_candidates(Tick(1000));
        assert!(short.iter().all(|t| *t >= 0));
    }

    /// Test 69 (item 1 round 7): negative caches — probe/thumb of an unknown media
    /// publishes failure EXACTLY ONCE; repeated calls do not spawn
    /// new requests (previously — a full file open on every UI frame).
    #[test]
    fn t69_negative_caches_stop_retries() {
        let project = kt_model::Project::new(kt_model::ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        });
        let svc = ThumbService::start(&project, None);
        let id = MediaId(99); // media is NOT in the service's map
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        // duration: request is queued, the worker publishes None
        assert_eq!(svc.duration_or_probe(id), None);
        loop {
            if svc.shared.durations.lock().unwrap().contains_key(&id) {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "worker did not publish the failure");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(svc.duration(id), None);
        // a repeat call does NOT queue the request again
        assert_eq!(svc.duration_or_probe(id), None);
        assert!(svc.shared.probe.lock().unwrap().is_empty(), "repeated probe request");
        // thumb of unknown media → Failed, a repeat request — no-op
        let key = ThumbKey { media: id, t: 0 };
        svc.request(key);
        loop {
            if svc.state(&key) == ThumbState::Failed {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "key is not marked failed");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        svc.request(key);
        assert!(svc.shared.queue.lock().unwrap().0.is_empty(), "failed key was re-queued");
        // invalidate resets both negative caches
        svc.invalidate_media(id);
        assert!(!svc.shared.durations.lock().unwrap().contains_key(&id));
        assert_eq!(svc.state(&key), ThumbState::Missing);
    }

    /// Test 476 (R.52, kin of t69): the thumb and probe of a GENERATOR are built without
    /// a file on disk — both without the sidecar DB and with it (the DB key is file metadata,
    /// which `gen:` does not have: the worker goes straight to the factory source).
    /// The service gives the look (video without audio), “no duration” (0) and
    /// a real thumb of the given color.
    #[test]
    fn t476_generator_thumbs_without_a_file() {
        let mut project = kt_model::Project::new(kt_model::ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        });
        let id = MediaId(1);
        project.media.push(kt_model::MediaRef::new(id, "gen:solid:c=ff8800"));
        // run twice: without a project path (no DB) and with a path (DB exists,
        // but the generator is closed to it by the file_identity key)
        let db_project = std::env::temp_dir().join(format!("kt-t476-{}.kt", std::process::id()));
        for project_path in [None, Some(db_project.clone())] {
            let with_db = project_path.is_some();
            let svc = ThumbService::start(&project, project_path);
            // a generous deadline: under a full parallel run the disk is busy
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
            assert_eq!(svc.duration_or_probe(id), None, "the first call is a request");
            while !svc.shared.durations.lock().unwrap().contains_key(&id) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "probe not published (db={with_db})"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert_eq!(
                svc.duration(id),
                Some(Tick(0)),
                "the generator has no duration (db={with_db})"
            );
            assert_eq!(svc.kinds(id), Some((true, false)), "video without audio (db={with_db})");
            let key = ThumbKey { media: id, t: 0 };
            svc.request(key);
            let img = loop {
                match svc.state(&key) {
                    ThumbState::Ready(img) => break img,
                    ThumbState::Failed => panic!("generator thumbnail failed (db={with_db})"),
                    _ => {}
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "thumbnail not built (db={with_db})"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            let c = img.pixels[img.pixels.len() / 2];
            let close = |a: u8, b: u8| a.abs_diff(b) <= 2;
            assert!(
                close(c.r(), 255) && close(c.g(), 136) && close(c.b(), 0),
                "thumbnail color did not match the generator parameters: {c:?} (db={with_db})"
            );
        }
        let _ = std::fs::remove_file(db_project.with_extension("ktindex.db"));
    }

    /// Test 71 (item 1 round 7): LRU of sources — eviction of the oldest, without a
    /// full reset (the previous clear() at >2 reopened every file).
    #[test]
    fn t71_source_lru() {
        let mut lru: SourceLru<u32> = SourceLru::new(4);
        for i in 0..4 {
            lru.insert(MediaId(i), i as u32);
        }
        assert_eq!(lru.len(), 4);
        // access to the oldest moves it to the tail
        assert_eq!(lru.get_mut(MediaId(0)), Some(&mut 0));
        lru.insert(MediaId(9), 9);
        assert_eq!(lru.len(), 4, "exactly one was evicted");
        assert!(!lru.contains(MediaId(1)), "the oldest after touch(0) is 1");
        assert!(lru.contains(MediaId(0)) && lru.contains(MediaId(9)));
        lru.insert(MediaId(10), 10);
        assert!(!lru.contains(MediaId(2)));
        assert!(lru.contains(MediaId(0)), "the recently used one is alive");
    }

    /// Test 62 (item 11 round 6): quantization of thumb keys FROM THE FILE'S ZERO — the bin and
    /// the timeline (with a nonzero source_in) give ONE key for one moment
    /// Test 549 (R.54, item 432): the map “event tick → source frame” —
    /// ONE for the filmstrip and for the picker's thumbnails, and it knows about reverse.
    ///
    /// R.37 item 3 already required computing the event's preview with the same map as the
    /// render, and moved the stretch into `ClipWarp::material_off`. One hole
    /// remained and was found while assembling the picker: a reversed event
    /// WITHOUT a warp has no map at all (`clip_warp` returns `None` — reverse is
    /// not a warp), and the old `map_or(tl, …)` took the material DIRECTLY. The test
    /// is red on the old formula: it gave 0 at the first pixel instead of the
    /// last frame of the piece.
    #[test]
    fn t549_material_map_is_one_and_knows_reverse() {
        let dur = kt_time::Tick(4 * 90_000);
        let step = THUMB_STEP_LADDER[1]; // 1 c
        // a forward event: pixel = material
        assert_eq!(material_at(None, dur, false, 0), 0);
        assert_eq!(material_at(None, dur, false, 90_000), 90_000);
        // reversed WITHOUT a warp: the first pixel is the LAST frame of the piece
        assert_eq!(material_at(None, dur, true, 0), dur.0 - 1);
        assert_eq!(material_at(None, dur, true, dur.0 - 1), 0);
        // monotonicity: in reverse the material decreases
        let mut prev = i64::MAX;
        for i in 0..=8 {
            let tl = i * dur.0 / 8;
            let m = material_at(None, dur, true, tl);
            assert!(m <= prev, "reverse must go backward: {m} after {prev}");
            prev = m;
        }
        // the key is the same quantum as for a forward event (shared cache, t62)
        let k = material_key_t(0, None, dur, true, 0, step);
        assert_eq!(k % step, 0, "key off the step grid");
        assert_eq!(k, ((dur.0 - 1) / step) * step);
        // source_in is added BEFORE quantization (from the file's zero)
        assert_eq!(
            material_key_t(30_000, None, dur, false, 90_000, step),
            material_key_t(0, None, dur, false, 120_000, step),
            "quantization must run from the file’s zero"
        );
        // beyond the event's edges the key does not go negative
        assert_eq!(material_key_t(0, None, dur, false, -10, step), 0);
    }

    /// file; the key is a multiple of the step and stable within a bucket; scrubbing is monotonic.
    #[test]
    fn t62_thumb_keys_unified_from_file_zero() {
        let step = 90_000i64;
        // multiplicity and stability within a bucket
        for tl in [0i64, 1, 44_999, 89_999] {
            assert_eq!(thumb_key_t(0, tl, step), 0);
        }
        assert_eq!(thumb_key_t(0, 90_000, step), 90_000);
        // timeline with source_in=30_000 (not a multiple of the step) and bin at the same
        // file moment 120_000 → the same key
        let timeline_key = thumb_key_t(30_000, 90_000, step); // file: 120_000
        let bin_key = thumb_key_t(0, 120_000, step);
        assert_eq!(timeline_key, bin_key, "shared bin↔timeline cache");
        assert_eq!(timeline_key, 90_000, "floor bucket");
        // a negative value does not occur
        assert_eq!(thumb_key_t(0, -5, step), 0);
        // scrub: monotonic by fraction, edges in [0, dur)
        let dur = 10 * 90_000i64;
        let mut prev = -1i64;
        for i in 0..=20 {
            let frac = i as f64 / 20.0 * 0.999;
            let t = thumb_key_t(0, (frac * dur as f64) as i64, step);
            assert!(t >= prev, "scrub monotonicity");
            assert!((0..dur).contains(&t));
            prev = t;
        }
    }
}
