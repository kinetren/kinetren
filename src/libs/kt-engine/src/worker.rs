//! MediaWorker: a thread that EXCLUSIVELY owns one MediaSource (invariant No. 5).
//!
//! Clients (playback / prefetch / thumbnails) do not touch the decoder — they put
//! requests into the priority queue; the worker inserts the result into the BurstCache.
//! This removes the main source of the old engine's slowdowns: prefetch and playback
//! fought over one decoder through a mutex, thrashing it with seeks (defect h).
//!
//! Queue rules:
//! - a smaller Priority is strictly first (Playback > NearPrefetch > ...);
//! - at equal priority — requests FORWARD from the decoder cursor, the nearest
//!   first (anti ping-pong: do not jump back while there is work ahead);
//!   backward only when forward is empty;
//! - each key is served at most once per submission (dedup); a key already queued or already
//!   taken into the span being decoded is not queued again and is not counted as a request
//!   (decode-request-storm: the preview re-asks its whole frame on every composite);
//! - what is already in the cache is not decoded again;
//! - behind the cursor, a run of queued keys is read FORWARD from its start, not one key per seek
//!   from its end (decode-request-storm);
//! - a transport seek bumps the generation: requests of old generations are dropped.

use crate::Priority;
use kt_cache::{BurstCache, SourceId};
use kt_media::{MediaSource, Quality};
use kt_model::MediaId;
use kt_time::Tick;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Counters of real accesses to the decoders (for the perf panel and tests 21).
#[derive(Default)]
pub struct DecodeStats {
    pub audio_decodes: AtomicU64,
    pub video_decodes: AtomicU64,
    /// Video decode spans (batches) and the frames decoded by spans.
    pub video_span_batches: AtomicU64,
    pub video_span_frames: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RequestKind {
    /// Audio burst of the source.
    Audio { idx: i64 },
    /// Video frame by the source pts (in ticks), aligned by the caller.
    /// `step` — the frame grid step (ticks): the width of the coverage (item 20).
    Video { pts: Tick, quality: Option<(u32, u32)>, step: i64 },
}

impl RequestKind {
    pub fn video(pts: Tick, quality: Option<Quality>) -> RequestKind {
        RequestKind::Video { pts, quality: quality.map(|q| (q.width, q.height)), step: 1 }
    }

    /// Video request on the project grid: the coverage will be closed by the band [pts,
    /// pts+step) — the cache bar is honest (item 20).
    pub fn video_on_grid(pts: Tick, quality: Option<Quality>, frame_step: i64) -> RequestKind {
        RequestKind::Video {
            pts,
            quality: quality.map(|q| (q.width, q.height)),
            step: frame_step.max(1),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DecodeRequest {
    pub kind: RequestKind,
    pub prio: Priority,
    pub generation: u64,
}

/// (prio, pts-ticks, quality) — the key of the video queue.
type VideoQueueKey = (Priority, i64, Option<(u32, u32)>);

#[derive(Default)]
struct Queue {
    // Separate maps by type: audio and video have THEIR OWN decoder cursors —
    // a shared cursor would make the video choice zigzag following the audio
    // (items 14/15: constant backward seeks → a full reopen on every frame).
    // A BTreeMap gives both the order and the dedup.
    audio: BTreeMap<(Priority, i64), u64>, // pos = burst idx
    video: BTreeMap<VideoQueueKey, (u64, i64)>, // pos = pts in ticks; (gen, step)
    /// Reconfiguration of the source (R.53, item 411): NOT a request. It is not
    /// cancelled by the generation (otherwise a seek in the middle of an eyedropper drag would silently discard it,
    /// and the source would stay with the old parameters forever),
    /// is not delayed by priority; the last one wins.
    /// Along with the payload comes a list of FOREIGN addresses that go stale because of
    /// this reconfiguration (R.59, stage 2): the memos of sub-composites of the reels in
    /// which this event lies. They are cleared by THE SAME thread, exactly where it clears its own cache —
    /// otherwise a composite on the OLD frame could run between “invalidated” and “applied”
    /// and write the stale result again.
    retune: Option<(Retune, Vec<SourceId>)>,
    /// The keys of the span the decoder holds now (pts, decode size): taken off the queue, not yet
    /// in the cache. A submit of one of them is a duplicate, not new work (decode-request-storm).
    in_flight: std::collections::HashSet<(i64, Option<(u32, u32)>)>,
    closed: bool,
}

/// What reconfigures a live source (R.59, stage 2).
///
/// The channel is the same and the rule is the same (“not a request, the last one wins”);
/// only the payload changed: for a generator on an event the values do not fit into the path —
/// there are curves — so they travel as STATE.
#[derive(Debug, Clone)]
pub enum Retune {
    Path(String),
    Gen(kt_model::GenState),
}

impl Queue {
    fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.video.is_empty() && self.retune.is_none()
    }
}

struct Shared {
    queue: Mutex<Queue>,
    cond: Condvar,
    min_generation: AtomicU64,
    idle: AtomicBool,
}

/// The worker's handle. Drop does NOT stop the thread — call `shutdown()`.
pub struct MediaWorker {
    shared: Arc<Shared>,
    join: Option<std::thread::JoinHandle<()>>,
    pub media: MediaId,
}

impl MediaWorker {
    pub fn spawn(
        media: MediaId,
        source: Box<dyn MediaSource>,
        cache: Arc<BurstCache>,
    ) -> MediaWorker {
        Self::spawn_with_stats(media, source, cache, Arc::new(DecodeStats::default()))
    }

    pub fn spawn_with_stats(
        media: MediaId,
        mut source: Box<dyn MediaSource>,
        cache: Arc<BurstCache>,
        stats: Arc<DecodeStats>,
    ) -> MediaWorker {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            cond: Condvar::new(),
            min_generation: AtomicU64::new(0),
            idle: AtomicBool::new(true),
        });
        let sh = shared.clone();
        let source_id = crate::source_id(media);
        let join = std::thread::Builder::new()
            .name(format!("media-worker-{}", media.0))
            .spawn(move || worker_loop(&sh, source_id, source.as_mut(), &cache, &stats))
            .expect("spawn media worker");
        MediaWorker { shared, join: Some(join), media }
    }

    pub fn submit(&self, req: DecodeRequest) {
        let mut q = self.shared.queue.lock().unwrap();
        if req.generation < self.shared.min_generation.load(Ordering::Acquire) {
            return;
        }
        // dedup: the same key updates the generation but does not duplicate the work, and only a
        // key that enters the queue is a request (the journal's `decode_requests`): the preview
        // re-asks every layer of its frame on every composite, and counting each repeat made a
        // frame waiting for its history read as 1500 requests a second (decode-request-storm)
        let new = match req.kind {
            RequestKind::Audio { idx } => q.audio.insert((req.prio, idx), req.generation).is_none(),
            RequestKind::Video { pts, quality, .. } if q.in_flight.contains(&(pts.0, quality)) => return,
            RequestKind::Video { pts, quality, step } => {
                q.video.insert((req.prio, pts.0, quality), (req.generation, step)).is_none()
            }
        };
        if new {
            kt_time::work::bump(kt_time::work::Work::DecodeRequest);
            trace_req(self.source_id(), req.kind, req.prio);
        }
        self.shared.idle.store(false, Ordering::Release);
        drop(q);
        self.shared.cond.notify_one();
    }

    /// Reconfigure a LIVE source under a new path (R.53, item 411): for a
    /// generator this is an edit of parameters; the decoder thread is NOT
    /// stopped and the renderers are NOT rebuilt. The frames of the old parameters
    /// are discarded by the WORKER ITSELF — the same thread that inserts them,
    /// so a race “old frame appended after the cleanup” cannot happen.
    pub fn retune(&self, path: &str) {
        self.retune_with(Retune::Path(path.to_string()), Vec::new());
    }

    /// Reconfigure to new generator VALUES (R.59, stage 2).
    /// `stale` — addresses whose memoized results depend on this
    /// source (the reels that are ancestors of the event).
    pub fn retune_gen(&self, state: kt_model::GenState, stale: Vec<SourceId>) {
        self.retune_with(Retune::Gen(state), stale);
    }

    fn retune_with(&self, r: Retune, stale: Vec<SourceId>) {
        let mut q = self.shared.queue.lock().unwrap();
        q.retune = Some((r, stale));
        self.shared.idle.store(false, Ordering::Release);
        drop(q);
        self.shared.cond.notify_one();
    }

    /// Withdraw the queued VIDEO requests of priority `prio` that `keep(pts, quality)`
    /// rejects (video-prefetch-thrash): the look-ahead the prefetch no longer asks for —
    /// the playhead has passed it — is not decoded into an entry nobody reads. A span
    /// already taken by the decoder is not interrupted.
    pub fn withdraw_video(&self, prio: Priority, keep: impl Fn(i64, Option<(u32, u32)>) -> bool) {
        let source = self.source_id();
        let mut q = self.shared.queue.lock().unwrap();
        q.video.retain(|&(p, pts, quality), _| {
            let kept = p != prio || keep(pts, quality);
            if !kept {
                trace_withdraw(source, kt_cache::BurstKind::Video { quality }, pts);
            }
            kept
        });
    }

    fn source_id(&self) -> SourceId {
        crate::source_id(self.media)
    }

    /// Cancel all requests of generations < `min_generation` (transport seek).
    pub fn cancel_generations_below(&self, min_generation: u64) {
        self.shared.min_generation.store(min_generation, Ordering::Release);
        let source = self.source_id();
        let mut q = self.shared.queue.lock().unwrap();
        q.audio.retain(|&(_, idx), g| {
            let kept = *g >= min_generation;
            if !kept {
                trace_withdraw(source, kt_cache::BurstKind::Audio, idx);
            }
            kept
        });
        q.video.retain(|&(_, pts, quality), (g, _)| {
            let kept = *g >= min_generation;
            if !kept {
                trace_withdraw(source, kt_cache::BurstKind::Video { quality }, pts);
            }
            kept
        });
        drop(q);
        self.shared.cond.notify_one();
    }

    /// true when the queue is empty and the worker is not busy decoding.
    pub fn is_idle(&self) -> bool {
        self.shared.idle.load(Ordering::Acquire) && self.shared.queue.lock().unwrap().is_empty()
    }

    pub fn shutdown(mut self) {
        {
            let mut q = self.shared.queue.lock().unwrap();
            q.closed = true;
        }
        self.shared.cond.notify_all();
        if let Some(j) = self.join.take() {
            // the decoder's last span: never on the UI or the render thread (one-scheduler)
            let _w = kt_time::block::wait(kt_time::block::kind::JOIN);
            let _ = j.join();
        }
    }
}

/// A request entered the queue (structured-telemetry): its key, and the thread's cause — the frame
/// or the pass that asked.
fn trace_req(source: SourceId, kind: RequestKind, prio: Priority) {
    use kt_time::trace::{self, Ev};
    if !trace::on() {
        return;
    }
    let (bk, idx) = match kind {
        RequestKind::Audio { idx } => (kt_cache::BurstKind::Audio, idx),
        RequestKind::Video { pts, quality, .. } => (kt_cache::BurstKind::Video { quality }, pts.0),
    };
    let (w, h) = bk.trace_size();
    trace::ev(Ev::DecReq)
        .id(kt_cache::trace_id(source, bk, idx))
        .a(source.0)
        .b(idx as u64)
        .c(trace::size(w, h))
        .aux(prio as u32)
        .emit();
}

fn trace_withdraw(source: SourceId, kind: kt_cache::BurstKind, idx: i64) {
    kt_time::trace::ev(kt_time::trace::Ev::DecWithdraw)
        .id(kt_cache::trace_id(source, kind, idx))
        .a(source.0)
        .b(idx as u64)
        .emit();
}

/// A decoded entry went into the cache.
fn trace_decoded(source: SourceId, kind: kt_cache::BurstKind, idx: i64) {
    use kt_time::trace::{self, Ev};
    if !trace::on() {
        return;
    }
    let (w, h) = kind.trace_size();
    trace::ev(Ev::Decoded).id(kt_cache::trace_id(source, kind, idx)).a(source.0).b(idx as u64).c(trace::size(w, h)).emit();
}

fn worker_loop(
    shared: &Shared,
    source_id: SourceId,
    source: &mut dyn MediaSource,
    cache: &BurstCache,
    stats: &DecodeStats,
) {
    /// How far forward from the first key we extend the span.
    const SPAN_MAX_AHEAD_TICKS: i64 = 2 * kt_time::TICK_HZ;

    // The cursors “where the decoder was” — SEPARATE for audio and video (see Queue).
    let mut cursor_audio: i64 = 0;
    let mut cursor_video: i64 = 0;
    // Negative cache (item 19g): keys for which the decoder returned nothing
    // (past EOF) — are not decoded again, otherwise prefetch would
    // drive the decoder into endless seeks every 100 ms.
    let mut failed_video: std::collections::HashSet<(i64, Option<(u32, u32)>)> =
        std::collections::HashSet::new();
    let mut failed_audio: std::collections::HashSet<i64> = std::collections::HashSet::new();
    loop {
        enum Job {
            Audio {
                idx: i64,
            },
            /// The first key + the tail added forward (any priorities, every decode size:
            /// `read_video_span_qualities` decodes a picture once and converts it to each).
            VideoSpan {
                keys: Vec<SpanKey>,
                /// The generation of the first key: with it an unfinished tail
                /// returns to the queue and is honestly cancelled by a seek.
                generation_of_span: u64,
                /// Priority of the first key: the span is interrupted only for the sake of
                /// STRICTLY more urgent audio. At equal priority
                /// interrupting is not allowed — the video cursor would zigzag behind the
                /// audio, and that is a ping-pong seek (defect h, guard t21).
                prio_of_span: crate::Priority,
            },
        }
        let job = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if q.closed {
                    return;
                }
                // RECONFIGURATION — first, before the task is chosen (item 411):
                // the keys that must be computed with the already
                // new parameters come after.
                if let Some((r, stale)) = q.retune.take() {
                    drop(q);
                    let applied = match &r {
                        Retune::Path(path) => source.retune(path),
                        Retune::Gen(state) => source.retune_gen(state),
                    };
                    debug_assert!(
                        applied,
                        "source rejected retune: the caller must ask factory::can_retune first"
                    );
                    if applied {
                        // frames of the old parameters are discarded by THE SAME thread
                        // that inserts them: there is no race by construction
                        cache.invalidate_source(source_id);
                        for s in stale {
                            cache.invalidate_source(s);
                        }
                        failed_video.clear();
                        failed_audio.clear();
                    }
                    q = shared.queue.lock().unwrap();
                    continue;
                }
                q.in_flight.clear();
                match pick_next(&q, cursor_audio, cursor_video) {
                    Some(Pick::Audio(key)) => {
                        q.audio.remove(&key);
                        break Job::Audio { idx: key.1 };
                    }
                    Some(Pick::Video(key)) => {
                        let (_prio, first_pts, quality) = key;
                        let (first_gen, first_step) = q.video.remove(&key).unwrap_or((0, 1));
                        // Span top-up ABOVE priorities (item 19a): the Playback key
                        // becomes the START of the span, and the NearPrefetch tail rides along on
                        // the same decode pass. Before, the top-up took only its own
                        // priority: while even one Playback key was alive (the frame incomplete),
                        // the spans degenerated into “one frame at a time with seeks” —
                        // the cache stopped growing.
                        //
                        // ABOVE decode sizes too (preview-throughput): a file asked at two sizes
                        // at one moment (a sharp layer over its blurred backdrop) went out as
                        // two spans, and the second, behind the cursor, seeked back and rebuilt
                        // the decoder: 9 pictures decoded per frame asked on the showcase. A
                        // native-size key (None, a thumbnail's) keeps a span of its own.
                        let mut keys = vec![SpanKey { pts: first_pts, quality, step: first_step }];
                        let horizon = first_pts + SPAN_MAX_AHEAD_TICKS;
                        let mut room =
                            (cache.max_video_bytes() / 2).saturating_sub(key_bytes(quality));
                        let mut cand: Vec<(VideoQueueKey, i64)> = Vec::new();
                        for p in [
                            crate::Priority::Playback,
                            crate::Priority::NearPrefetch,
                            crate::Priority::FarPrefetch,
                            crate::Priority::Thumbnail,
                        ] {
                            cand.extend(
                                q.video
                                    .range((p, first_pts, None)..)
                                    .take_while(|(k, _)| k.0 == p && k.1 <= horizon)
                                    .filter(|(k, _)| {
                                        k.2.is_some() == quality.is_some() && (k.1, k.2) != (first_pts, quality)
                                    })
                                    .map(|(&k, &(_, step))| (k, step)),
                            );
                        }
                        cand.sort_by_key(|(k, _)| (k.1, k.2));
                        cand.dedup_by_key(|(k, _)| (k.1, k.2));
                        for (k, step) in cand {
                            let bytes = key_bytes(k.2);
                            if cache.contains_video(source_id, k.2.map(into_q), k.1)
                                || failed_video.contains(&(k.1, k.2))
                            {
                                q.video.remove(&k);
                            } else if keys.len() < SPAN_MAX_KEYS && bytes <= room {
                                room -= bytes;
                                keys.push(SpanKey { pts: k.1, quality: k.2, step });
                                q.video.remove(&k);
                            }
                        }
                        q.in_flight.extend(keys.iter().map(|k| (k.pts, k.quality)));
                        break Job::VideoSpan {
                            keys,
                            generation_of_span: first_gen,
                            prio_of_span: key.0,
                        };
                    }
                    None => {
                        shared.idle.store(true, Ordering::Release);
                        q = shared.cond.wait(q).unwrap();
                    }
                }
            }
        };
        shared.idle.store(false, Ordering::Release);
        match job {
            Job::Audio { idx } => {
                cursor_audio = idx * kt_time::AUDIO_BURST_TICKS;
                read_audio(source, source_id, cache, stats, &mut failed_audio, idx);
            }
            Job::VideoSpan { keys, generation_of_span, prio_of_span } => {
                cursor_video = keys.iter().map(|k| k.pts).max().unwrap_or(cursor_video);
                let missing: Vec<SpanKey> = keys
                    .into_iter()
                    .filter(|k| {
                        !cache.contains_video(source_id, k.quality.map(into_q), k.pts)
                            && !failed_video.contains(&(k.pts, k.quality))
                    })
                    .collect();
                if missing.is_empty() {
                    continue;
                }
                stats.video_decodes.fetch_add(missing.len() as u64, Ordering::Relaxed);
                let mut span = kt_time::trace::span(kt_time::trace::Ev::DecSpan);
                span.a(source_id.0).b(missing[0].pts as u64).c(missing.len() as u64);
                if missing.len() > 1 {
                    stats.video_span_batches.fetch_add(1, Ordering::Relaxed);
                    stats.video_span_frames.fetch_add(missing.len() as u64, Ordering::Relaxed);
                }
                let mut emitted: std::collections::HashSet<ReadKey> =
                    std::collections::HashSet::new();
                // A SPAN IS DECODED IN CHUNKS, and between them the worker looks into
                // the queue (R.63 item 13).
                //
                // Why this is not an optimization but a defect. A span tops up to 64
                // keys and hands them to the decoder in ONE call: at 1080p this is
                // 22–61 ms per frame, i.e. up to 4 seconds during which the worker
                // does NOT look into the queue AT ALL. A Playback audio burst arriving at this time
                // waits for the whole span, however high its priority is —
                // and the player goes silent. This is exactly what is visible in the
                // owner's flight log: after each jump, 1–6 s of complete
                // silence, at a render cost of 2% of real time, while
                // the audio burst itself decodes in 14 µs against 22–61 ms for
                // a frame. An abandoned span tail loses nothing: the prefetch pass
                // is idempotent and will re-ask for what is missing (t9).
                //
                // A chunk is counted in MOMENTS: the sizes of one moment stay together, the
                // picture is decoded once for all of them.
                const SPAN_CHUNK: usize = 4;
                let mut res = Ok(());
                let mut done_keys = 0usize;
                for chunk in span_chunks(&missing, SPAN_CHUNK) {
                    res = read_chunk(source, chunk, &mut |k| {
                        emitted.insert(k);
                    }, source_id, cache);
                    done_keys += chunk.len();
                    if res.is_err() {
                        break;
                    }
                    // audio is MORE URGENT than this span and cannot wait — we give up the stream
                    let audio_waits = {
                        let q = shared.queue.lock().unwrap();
                        q.audio.keys().next().is_some_and(|(p, _)| *p < prio_of_span)
                    };
                    if !audio_waits {
                        // Audio of the span's own priority is read here, and the span goes on
                        // (preview-throughput D5). The rule "a span is not given up for equal
                        // priority" keeps the VIDEO cursor from zigzagging, and this does not move
                        // it: audio has its own demuxer. Left waiting behind a long span, the
                        // bursts of the look-ahead turn urgent only when the render has already
                        // missed them (t1529).
                        loop {
                            let next = {
                                let mut q = shared.queue.lock().unwrap();
                                let key = q.audio.keys().next().copied();
                                key.filter(|k| k.0 == prio_of_span).map(|k| {
                                    q.audio.remove(&k);
                                    k.1
                                })
                            };
                            let Some(idx) = next else { break };
                            cursor_audio = idx * kt_time::AUDIO_BURST_TICKS;
                            read_audio(source, source_id, cache, stats, &mut failed_audio, idx);
                        }
                    }
                    if audio_waits {
                        // UNFINISHED WORK IS RETURNED TO THE QUEUE, not dropped:
                        // the keys were already taken off it when the span was assembled, and
                        // silently losing them would mean hoping someone re-asks. Prefetch will re-ask,
                        // but a thumbnail or a one-off request will not. We put them back
                        // as a near prefetch:
                        // the span's Playback key is always
                        // FIRST and already computed.
                        let mut q = shared.queue.lock().unwrap();
                        for k in missing.iter().skip(done_keys) {
                            q.video
                                .entry((prio_of_span, k.pts, k.quality))
                                .or_insert((generation_of_span, k.step));
                        }
                        break;
                    }
                }
                span.aux(emitted.len() as u32);
                drop(span);
                if res.is_ok() {
                    // Keys not closed by the decoder (EOF) go into the negative cache,
                    // but ONLY those we actually reached: the abandoned span tail was not decoded at all,
                    // and marking it “dead” would mean those frames would
                    // NEVER be shown.
                    for k in missing.iter().take(done_keys) {
                        if !emitted.contains(&(k.pts, k.quality)) {
                            failed_video.insert((k.pts, k.quality));
                        }
                    }
                }
            }
        }
    }
}

/// Decode one audio burst into the cache, unless it is there or known to be past the end.
fn read_audio(
    source: &mut dyn MediaSource,
    source_id: SourceId,
    cache: &BurstCache,
    stats: &DecodeStats,
    failed: &mut std::collections::HashSet<i64>,
    idx: i64,
) {
    if cache.contains_audio(source_id, idx) || failed.contains(&idx) {
        return;
    }
    stats.audio_decodes.fetch_add(1, Ordering::Relaxed);
    match source.read_audio_burst(idx) {
        Ok(burst) => {
            trace_decoded(source_id, kt_cache::BurstKind::Audio, idx);
            cache.insert_audio(source_id, Arc::new(burst))
        }
        Err(kt_media::MediaError::OutOfRange) => {
            failed.insert(idx);
        }
        Err(_) => {}
    }
}

/// A key the decoder answered: (pts, decode size).
type ReadKey = (i64, Option<(u32, u32)>);

/// One key of a video span.
#[derive(Debug, Clone, Copy)]
struct SpanKey {
    pts: i64,
    quality: Option<(u32, u32)>,
    /// The width of the coverage band on the project grid.
    step: i64,
}

/// How many keys one span holds at most.
const SPAN_MAX_KEYS: usize = 64;

/// A span's keys ordered by moment, cut after every `moments` distinct pts.
fn span_chunks(keys: &[SpanKey], moments: usize) -> Vec<&[SpanKey]> {
    let mut out = Vec::new();
    let (mut start, mut seen) = (0, 0);
    for i in 0..keys.len() {
        if i == 0 || keys[i].pts != keys[i - 1].pts {
            if seen == moments {
                out.push(&keys[start..i]);
                start = i;
                seen = 0;
            }
            seen += 1;
        }
    }
    if start < keys.len() {
        out.push(&keys[start..]);
    }
    out
}

/// Decode a chunk into the cache. All sizes of one file go through one forward pass; keys of the
/// native size (never mixed with sized ones) through the plain span.
fn read_chunk(
    source: &mut dyn MediaSource,
    chunk: &[SpanKey],
    done: &mut dyn FnMut(ReadKey),
    source_id: SourceId,
    cache: &BurstCache,
) -> Result<(), kt_media::MediaError> {
    let step_of = |pts: i64, q: Option<(u32, u32)>| {
        chunk.iter().find(|k| k.pts == pts && k.quality == q).map_or(1, |k| k.step.max(1))
    };
    let sized: Vec<(Tick, Quality)> =
        chunk.iter().filter_map(|k| k.quality.map(|q| (Tick(k.pts), into_q(q)))).collect();
    if sized.len() == chunk.len() {
        return source.read_video_span_qualities(&sized, &mut |k, q, frame| {
            let wh = (q.width, q.height);
            done((k.0, Some(wh)));
            kt_time::work::bump(kt_time::work::Work::Decoded);
            trace_decoded(source_id, kt_cache::BurstKind::Video { quality: Some(wh) }, k.0);
            cache.insert_video_span(source_id, Some(q), k.0, step_of(k.0, Some(wh)), Arc::new(frame));
        });
    }
    let pts: Vec<Tick> = chunk.iter().map(|k| Tick(k.pts)).collect();
    source.read_video_span(&pts, None, &mut |k, frame| {
        done((k.0, None));
        kt_time::work::bump(kt_time::work::Work::Decoded);
        trace_decoded(source_id, kt_cache::BurstKind::Video { quality: None }, k.0);
        cache.insert_video_span(source_id, None, k.0, step_of(k.0, None), Arc::new(frame));
    })
}

/// A span must not be able to evict itself: its keys take at most half the video budget, each
/// counted at its own decode size (item 19v). A native-size key has no known size: only
/// `SPAN_MAX_KEYS` bounds it, as before.
fn key_bytes(quality: Option<(u32, u32)>) -> usize {
    quality.map_or(0, |(w, h)| w as usize * h as usize * 4)
}

fn into_q((width, height): (u32, u32)) -> Quality {
    Quality { width, height }
}

enum Pick {
    Audio((Priority, i64)),
    Video(VideoQueueKey),
}

/// The first key of the run of queued keys that ends at `last` behind the cursor: at `last`'s
/// priority, at most `SPAN_MAX_KEYS` moments and the span's 2 s back (decode-request-storm). Going
/// back to the NEAREST key took one key per span — the span tops up only forward of its first key,
/// and forward of it everything was already done — so a run behind the cursor (a stream stage's
/// history asked after the decoder had passed it, a reverse look-ahead) cost a seek and a decode
/// from the keyframe per frame, read from its end to its start. From the run's start one span
/// reads it forward.
fn video_run_start(q: &Queue, last: VideoQueueKey) -> VideoQueueKey {
    const RUN_BACK_TICKS: i64 = 2 * kt_time::TICK_HZ;
    let mut start = last;
    let mut moments = 1usize;
    for (&k, _) in q.video.range(..last).rev() {
        if k.0 != last.0 || k.1 < last.1 - RUN_BACK_TICKS {
            break;
        }
        // a native-size key never shares a span with a sized one
        if k.2.is_some() != last.2.is_some() {
            continue;
        }
        if k.1 != start.1 {
            if moments == SPAN_MAX_KEYS {
                break;
            }
            moments += 1;
        }
        start = k;
    }
    start
}

/// Choosing the next request: the minimum priority across BOTH maps; within
/// it, for each kind — the candidate forward from ITS OWN cursor (the nearest);
/// if both have a forward candidate — the smaller forward distance (tie → audio);
/// if both have nothing forward — the nearest backward likewise.
fn pick_next(q: &Queue, cursor_audio: i64, cursor_video: i64) -> Option<Pick> {
    let top_a = q.audio.keys().next().map(|k| k.0);
    let top_v = q.video.keys().next().map(|k| k.0);
    let top = match (top_a, top_v) {
        (Some(a), Some(v)) => a.min(v),
        (Some(a), None) => a,
        (None, Some(v)) => v,
        (None, None) => return None,
    };
    // burst idx → ticks, for comparing distances in one domain
    let a_cursor_idx = cursor_audio / kt_time::AUDIO_BURST_TICKS;
    let a_fwd = q.audio.range((top, a_cursor_idx)..).next().map(|(&k, _)| k).filter(|k| k.0 == top);
    let v_fwd =
        q.video.range((top, cursor_video, None)..).next().map(|(&k, _)| k).filter(|k| k.0 == top);
    match (a_fwd, v_fwd) {
        (Some(a), Some(v)) => {
            let da = a.1 * kt_time::AUDIO_BURST_TICKS - cursor_audio;
            let dv = v.1 - cursor_video;
            if da <= dv { Some(Pick::Audio(a)) } else { Some(Pick::Video(v)) }
        }
        (Some(a), None) => Some(Pick::Audio(a)),
        (None, Some(v)) => Some(Pick::Video(v)),
        (None, None) => {
            // backward: the nearest to its own cursor
            let a_back = q
                .audio
                .range(..(top, a_cursor_idx))
                .next_back()
                .map(|(&k, _)| k)
                .filter(|k| k.0 == top);
            let v_back = q
                .video
                .range(..(top, cursor_video, None))
                .next_back()
                .map(|(&k, _)| k)
                .filter(|k| k.0 == top);
            // the distance is the nearest key's, the pick is the start of its run (see below)
            let v_run = v_back.map(|v| (v, video_run_start(q, v)));
            match (a_back, v_run) {
                (Some(a), Some((v, start))) => {
                    let da = cursor_audio - a.1 * kt_time::AUDIO_BURST_TICKS;
                    let dv = cursor_video - v.1;
                    if da <= dv { Some(Pick::Audio(a)) } else { Some(Pick::Video(start)) }
                }
                (Some(a), None) => Some(Pick::Audio(a)),
                (None, Some((_, start))) => Some(Pick::Video(start)),
                (None, None) => {
                    // only entries of another priority/edge remain — any
                    q.audio
                        .keys()
                        .next()
                        .map(|&k| Pick::Audio(k))
                        .or_else(|| q.video.keys().next().map(|&k| Pick::Video(k)))
                }
            }
        }
    }
}
