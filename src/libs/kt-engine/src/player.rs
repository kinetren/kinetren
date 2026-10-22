//! Player: the realtime playback core.
//!
//! Threads and ownership (docs/architecture.md):
//! - **the audio render thread** (here) owns the FxStateStore and the AudioRenderer,
//!   fills the SPSC ring; advances the playhead exactly by what the audio callback consumed
//!   (invariant 6);
//! - **the cpal callback** (kt-app) only reads the ring and publishes the counter
//!   of consumed frames — no allocations, no locks;
//! - **MediaWorkers** exclusively own the decoders (invariant 5);
//! - **the prefetch thread** idempotently tops up the cache;
//! - the UI thread reads the playhead, composites video from the cache, sends model edits.
//!
//! Playback never waits for decode: a cache miss = silence + underrun counter,
//! the missing item is immediately requested with Priority::Playback.

use crate::audio::AudioRenderer;
use crate::fetch::CacheFetcher;
use crate::fxstore::FxStateStore;
use crate::prefetch::prefetch_audio_pass;
use crate::transport::Transport;
use crate::video::{VideoRenderer, video_cache_idx};
use crate::video_plan::{GpuBackendInfo, GpuExecError, VideoGpuExec};
use crate::worker::{DecodeRequest, DecodeStats, MediaWorker, RequestKind};
use crate::{Priority, source_id};
use kt_cache::{BurstCache, CacheConfig};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick, TickRange};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A ring of 4 bursts ≈ 85 ms at 1×. At high speeds the margin melts
/// proportionally (at 8× this is ~10 ms — less than one sleep cycle of the
/// render thread), so the capacity is taken with a margin for ENGINE_RATE_MAX
/// (item 4 round 8).
const RING_CAPACITY_FRAMES: usize = AUDIO_BURST_FRAMES * 16;
/// Maximum playback speed (item 4 round 8: the shuttle's “second wind”
/// up to ±8×).
pub const ENGINE_RATE_MAX: f32 = 8.0;
/// Master volume ceiling (+24 dB — the top of the UI fader).
pub const MASTER_GAIN_MAX: f32 = 16.0;
const PLAYBACK_AUDIO_LOOKAHEAD_FRAMES: i64 = AUDIO_SAMPLE_RATE as i64 / 5; // 200 ms
const PREFETCH_AUDIO_LOOKAHEAD_FRAMES: i64 = 2 * AUDIO_SAMPLE_RATE as i64; // 2 c
const PREFETCH_VIDEO_LOOKAHEAD_TICKS: i64 = 2 * kt_time::TICK_HZ;

/// The part shared with the audio callback: ring + consumption counter + flush.
pub struct AudioShared {
    /// Frames actually consumed by the callback (for advancing the playhead).
    pub consumed_frames: AtomicI64,
    /// A request to the callback to discard everything from the ring (after a seek).
    pub flush: AtomicBool,
    /// Callback underruns (the ring was empty while playing).
    pub callback_underruns: AtomicU64,
    /// Playback speed (f32 bits; 1.0 = normal). The callback reads the
    /// ring with this step (linear interpolation) — the pitch drifts as in
    /// Vegas (varispeed, item 4); the playhead itself moves faster/slower,
    /// since consumed counts the SOURCE frames.
    pub playback_rate: AtomicU32,
    /// Estimate of the output latency in 48k frames (cpal timestamp playback − callback):
    /// the playhead moves at the moment the callback TAKES the frames, but they are heard
    /// later by this latency — video for the “audible now” moment (item 30).
    pub output_latency_frames: AtomicI64,
    /// Smoothed latency (EMA in the render thread): the raw estimate from
    /// PipeWire/ALSA jitters by ±a frame between callbacks → the preview frame key
    /// jumped across grid cells and back — visible jerks with honest
    /// decode (round 5, item 1). Video reads THIS field.
    pub latency_smooth: AtomicI64,
}

/// Peak levels of what goes into the ring (f32 bits; the UI does dB,
/// decay and peak-hold). fetch_max is correct: the bit pattern of non-negative
/// f32 is monotonic.
pub struct Meters {
    pub peak_l: AtomicU32,
    pub peak_r: AtomicU32,
}

impl Meters {
    fn fold(&self, chunk: &[f32]) {
        let mut pl = 0f32;
        let mut pr = 0f32;
        for fr in chunk.chunks_exact(AUDIO_CHANNELS) {
            pl = pl.max(fr[0].abs());
            pr = pr.max(fr[1].abs());
        }
        self.peak_l.fetch_max(pl.to_bits(), Ordering::AcqRel);
        self.peak_r.fetch_max(pr.to_bits(), Ordering::AcqRel);
    }

    /// Read and reset (swap-to-zero).
    pub fn take(&self) -> (f32, f32) {
        (
            f32::from_bits(self.peak_l.swap(0, Ordering::AcqRel)),
            f32::from_bits(self.peak_r.swap(0, Ordering::AcqRel)),
        )
    }
}

/// Wake-up of background threads: in pause they park on a condvar instead of a
/// busy loop (item 1: 500 wake-ups/s from audio-render + a clone of the Project every
/// 100 ms of prefetch warmed the system even with an empty project).
pub struct Wake {
    lock: Mutex<()>,
    cv: std::sync::Condvar,
}

impl Wake {
    fn notify(&self) {
        let _g = self.lock.lock().unwrap();
        self.cv.notify_all();
    }

    fn wait_timeout(&self, d: std::time::Duration) {
        let g = self.lock.lock().unwrap();
        let _ = self.cv.wait_timeout(g, d).unwrap();
    }
}

/// Diagnostics of background loops (test 20: silence at idle).
#[derive(Default)]
pub struct LoopStats {
    pub render_passes: AtomicU64,
    pub prefetch_passes: AtomicU64,
    pub prefetch_rebuilds: AtomicU64,
}

pub struct Player {
    pub cache: Arc<BurstCache>,
    /// “Black box” of playback (R.28, item 10): a compact ring of
    /// transport events and counters — floating bugs are caught with it.
    pub flight: Arc<crate::flight::FlightRecorder>,
    pub transport: Arc<Transport>,
    pub shared: Arc<AudioShared>,
    pub meters: Arc<Meters>,
    /// Master volume (linear, f32 bits), applied in the render thread.
    master_gain: Arc<AtomicU32>,
    /// Loop region in 48k frames (None = off).
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
    /// Display clock: interpolation between playhead quanta (see
    /// display_extrapolation). (last head, moment, EMA of the step).
    video_clock: Mutex<(i64, std::time::Instant, i64)>,
    workers: Workers,
    natives: Natives,
    project: Arc<Mutex<Project>>,
    model_dirty: Arc<AtomicBool>,
    video: Mutex<VideoRenderer>,
    /// GPU executor of the video composite (R.29): injected from above
    /// (kt-app), lives separately from VideoRenderer — recreating the renderer
    /// on model edits does not lose it. DeviceLost “poisons” it until the end of the
    /// session → CPU fallback + notice.
    video_gpu: Mutex<VideoGpuState>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
    render_thread: Option<std::thread::JoinHandle<()>>,
    prefetch_thread: Option<std::thread::JoinHandle<()>>,
    pub underruns: Arc<AtomicU64>,
    pub decode_stats: Arc<DecodeStats>,
    wake: Arc<Wake>,
    /// Project version: prefetch rebuilds the renderers only on a change.
    project_version: Arc<AtomicU64>,
    /// “The source content has changed” (R.53, item 411) — SEPARATE from
    /// the project version. Prefetch wakes on it, but does NOT rebuild the renderers:
    /// the plan does not depend on generator parameters (law P).
    content_version: Arc<AtomicU64>,
    /// Counter of raised decoder threads (guard t500/t502).
    worker_spawns: Arc<AtomicU64>,
    /// Values of the generators that are NOW set in the live sources
    /// (R.59, stage 2). They cannot be asked of the sources themselves —
    /// they are exclusively owned by the worker thread (invariant 5) — and the diff of
    /// “what changed” is needed here.
    gen_states: Mutex<HashMap<MediaId, kt_model::GenState>>,
    /// Graph cache for sample_levels_at (item 1): (version, renderer, store).
    levels_probe: Mutex<Option<(u64, AudioRenderer, FxStateStore)>>,
    pub loop_stats: Arc<LoopStats>,
}

/// State of the preview GPU executor (R.29).
#[derive(Default)]
struct VideoGpuState {
    exec: Option<Arc<dyn VideoGpuExec>>,
    /// Some(reason) — DeviceLost/Validation: the GPU is poisoned until the end of the
    /// session, the preview goes via the CPU fallback, the UI shows a notice.
    poisoned: Option<String>,
}

/// Status of the video backend (for the notice and the performance panel row).
#[derive(Debug, Clone)]
pub enum VideoBackendStatus {
    Cpu,
    Gpu(GpuBackendInfo),
    Poisoned { info: GpuBackendInfo, reason: String },
}

impl Player {
    /// `sources` — open sources by media id (ownership passes to the workers).
    pub fn new(
        project: Project,
        sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    ) -> (Player, rtrb::Consumer<f32>) {
        let cache = Arc::new(BurstCache::new(CacheConfig::default()));
        let transport = Arc::new(Transport::new());
        let shared = Arc::new(AudioShared {
            consumed_frames: AtomicI64::new(0),
            flush: AtomicBool::new(false),
            callback_underruns: AtomicU64::new(0),
            playback_rate: AtomicU32::new(1.0f32.to_bits()),
            latency_smooth: AtomicI64::new(0),
            output_latency_frames: AtomicI64::new(0),
        });
        let meters = Arc::new(Meters { peak_l: AtomicU32::new(0), peak_r: AtomicU32::new(0) });
        let master_gain = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let loop_region: Arc<Mutex<Option<(i64, i64)>>> = Arc::new(Mutex::new(None));
        let decode_stats = Arc::new(DecodeStats::default());
        let mut workers = HashMap::new();
        // native source sizes (R.46, item 321): the decode scale step lives
        // in the renderer, and the native size can be learned only from an open source
        let mut nat = crate::video::SourceFacts::default();
        for (media, src) in sources {
            nat.observe(media, src.info());
            workers.insert(
                media,
                MediaWorker::spawn_with_stats(media, src, cache.clone(), decode_stats.clone()),
            );
        }
        let workers: Workers = Arc::new(std::sync::RwLock::new(workers));
        let natives: Natives = Arc::new(std::sync::RwLock::new(nat));
        let video = Mutex::new(VideoRenderer::new_with_facts(&project, &natives.read().unwrap()));
        let video_gpu = Mutex::new(VideoGpuState::default());
        let project = Arc::new(Mutex::new(project));
        let model_dirty = Arc::new(AtomicBool::new(true));
        let wake = Arc::new(Wake { lock: Mutex::new(()), cv: std::sync::Condvar::new() });
        let project_version = Arc::new(AtomicU64::new(0));
        let content_version = Arc::new(AtomicU64::new(0));
        let worker_spawns = Arc::new(AtomicU64::new(0));
        let loop_stats = Arc::new(LoopStats::default());
        let stop = Arc::new(AtomicBool::new(false));
        let underruns = Arc::new(AtomicU64::new(0));
        let flight = Arc::new(crate::flight::FlightRecorder::new());

        let (producer, consumer) =
            rtrb::RingBuffer::<f32>::new(RING_CAPACITY_FRAMES * AUDIO_CHANNELS);

        let render_thread = {
            let wake_r = wake.clone();
            let stats_r = loop_stats.clone();
            let cache = cache.clone();
            let transport = transport.clone();
            let shared = shared.clone();
            let workers = workers.clone();
            let project = project.clone();
            let dirty = model_dirty.clone();
            let stop = stop.clone();
            let underruns = underruns.clone();
            let meters = meters.clone();
            let master_gain = master_gain.clone();
            let loop_region = loop_region.clone();
            let flight_r = flight.clone();
            let natives_r = natives.clone();
            std::thread::Builder::new()
                .name("audio-render".into())
                .spawn(move || {
                    audio_render_loop(RenderCtx {
                        producer,
                        cache,
                        transport,
                        wake: wake_r,
                        loop_stats: stats_r,
                        shared,
                        workers,
                        project,
                        dirty,
                        stop,
                        underruns,
                        meters,
                        master_gain,
                        loop_region,
                        flight: flight_r,
                        natives: natives_r,
                    })
                })
                .expect("spawn audio render")
        };

        let preview_quality = Arc::new(Mutex::new(Quality { width: 640, height: 360 }));
        let prefetch_thread = {
            let wake_p = wake.clone();
            let stats_p = loop_stats.clone();
            let version_p = project_version.clone();
            let content_p = content_version.clone();
            let cache = cache.clone();
            let transport = transport.clone();
            let workers = workers.clone();
            let natives_p = natives.clone();
            let project = project.clone();
            let stop = stop.clone();
            let quality = preview_quality.clone();
            let region_p = loop_region.clone();
            std::thread::Builder::new()
                .name("prefetch".into())
                .spawn(move || {
                    prefetch_loop(
                        cache, transport, workers, natives_p, project, quality, stop, wake_p,
                        stats_p, version_p, content_p, region_p,
                    )
                })
                .expect("spawn prefetch")
        };

        let player = Player {
            cache,
            natives,
            flight,
            transport,
            shared,
            meters,
            master_gain,
            loop_region,
            video_clock: Mutex::new((0, std::time::Instant::now(), 0)),
            workers,
            project,
            model_dirty,
            video,
            video_gpu,
            preview_quality,
            stop,
            render_thread: Some(render_thread),
            prefetch_thread: Some(prefetch_thread),
            underruns,
            decode_stats,
            wake,
            project_version,
            content_version,
            worker_spawns,
            gen_states: Mutex::new(HashMap::new()),
            loop_stats,
            levels_probe: Mutex::new(None),
        };
        // generator sources are raised by the player ITSELF: their addresses
        // are derived from events, and the caller has nowhere to take them from
        let p = player.project.lock().unwrap().clone();
        player.sync_generators(&p);
        (player, consumer)
    }

    pub fn set_project(&self, p: Project) {
        self.project_version.fetch_add(1, Ordering::AcqRel);
        // the set of generators is a function of the PROJECT, and it must be computed by whoever
        // accepts the project: the addresses are derived from events, and
        // the caller has nowhere to take them from.
        self.sync_generators(&p);
        self.wake_after_project_swap(p);
    }

    fn wake_after_project_swap(&self, p: Project) {
        *self.video.lock().unwrap() =
            VideoRenderer::new_with_facts(&p, &self.natives.read().unwrap());
        *self.project.lock().unwrap() = p;
        self.model_dirty.store(true, Ordering::Release);
        self.wake.notify();
    }

    pub fn project(&self) -> Project {
        self.project.lock().unwrap().clone()
    }

    /// How many times the project was swapped out whole. Each swap is a clone of the whole
    /// project and a rebuild of both renderers, i.e. a stop of the
    /// audio-render thread; on a large edit this is audible (R.43, item 296).
    /// Exposed outward for the sake of the test “a convenience edit does not rebuild the engine”.
    pub fn project_version(&self) -> u64 {
        self.project_version.load(Ordering::Acquire)
    }

    /// Playback speed (item 4, a slider as in Vegas).
    /// A speed change resets the extrapolation window: it was learned on the
    /// steps of the PREVIOUS speed, and after entering slow-mo it still allows, for ~170 ms,
    /// a run forward four times longer — with a subsequent snap back
    /// exactly at the moment of switching on (item 14, round 14).
    pub fn set_playback_rate(&self, rate: f32) {
        if let Ok(mut vc) = self.video_clock.lock() {
            vc.2 = 0;
            vc.1 = std::time::Instant::now();
        }
        // negative = reverse transport (J4 v1.4); 0 = consumption stop
        // (silence, the playhead stands still). The callback uses |rate|; the sign is read
        // only by the render thread.
        self.shared
            .playback_rate
            .store(rate.clamp(-ENGINE_RATE_MAX, ENGINE_RATE_MAX).to_bits(), Ordering::Release);
    }

    pub fn playback_rate(&self) -> f32 {
        f32::from_bits(self.shared.playback_rate.load(Ordering::Acquire))
    }

    pub fn set_preview_quality(&self, q: Quality) {
        *self.preview_quality.lock().unwrap() = q;
    }

    pub fn preview_quality(&self) -> Quality {
        *self.preview_quality.lock().unwrap()
    }

    /// SOURCE VIEW (native × pixel aspect ratio) — for the interface, so that it can compute the
    /// placement frame with the same function as the render (R.64). The interface does not reach the
    /// decoder: the instance is owned by the worker (invariant 5), and the player already holds the
    /// source facts itself.
    pub fn source_view(&self, media: MediaId) -> Option<(f64, f64)> {
        self.natives.read().ok()?.views.get(&media).copied()
    }

    pub fn play(&self) {
        self.flight.push(crate::flight::Ev::Play, self.transport.playhead_frames(), 0);
        self.transport.set_playing(true);
        self.wake.notify();
    }

    pub fn pause(&self) {
        self.flight.push(crate::flight::Ev::Pause, self.transport.playhead_frames(), 0);
        self.transport.set_playing(false);
        self.wake.notify();
    }

    /// Preview master volume (linear; mute = 0).
    /// Preview master volume. A CEILING IS MANDATORY (item 7 round 9): a UI glitch
    /// once sent 10^20 here (a diverging fader animation), and this
    /// multiplied a whole burst — “painful to the ears”. NaN/inf are damped to 1.0.
    pub fn set_master_gain(&self, linear: f32) {
        let g = if linear.is_finite() { linear.clamp(0.0, MASTER_GAIN_MAX) } else { 1.0 };
        self.master_gain.store(g.to_bits(), Ordering::Release);
    }

    pub fn master_gain(&self) -> f32 {
        f32::from_bits(self.master_gain.load(Ordering::Acquire))
    }

    /// Loop region (None = off). The region head is pinned in the cache
    /// so that the wrap does not start cold (test 14).
    pub fn set_loop_region(&self, region: Option<TickRange>) {
        let frames = region.map(|r| (r.start.to_audio_frames(), r.end.to_audio_frames()));
        *self.loop_region.lock().unwrap() = frames;
        self.cache.unpin_all();
        if let Some((s, _)) = frames {
            let renderer = AudioRenderer::new(&self.project.lock().unwrap().clone());
            for (media, bursts) in
                renderer.wanted_audio_bursts(s..s + kt_time::AUDIO_SAMPLE_RATE as i64 / 2)
            {
                self.cache.pin_range(source_id(media), kt_cache::BurstKind::Audio, bursts);
            }
        }
        self.wake.notify();
    }

    /// Peak levels on pause: one burst from position t through a one-off
    /// FX state and the cache (a miss = silence — enough for the meter).
    pub fn sample_levels_at(&self, t: Tick) -> (f32, f32) {
        // Probe cache (item 1): previously EVERY call (8 times/s on pause) cloned
        // the whole Project and recreated all FX. Now the graph lives until the project
        // version changes; the state is reset before measuring (determinism).
        let version = self.project_version.load(Ordering::Acquire);
        let mut probe = self.levels_probe.lock().unwrap();
        let rebuild = probe.as_ref().map(|(v, _, _)| *v != version).unwrap_or(true);
        if rebuild {
            let renderer = AudioRenderer::new(&self.project.lock().unwrap().clone());
            let mut store = FxStateStore::new();
            renderer.sync_fx(&mut store);
            *probe = Some((version, renderer, store));
        }
        let (_, renderer, store) = probe.as_mut().unwrap();
        store.reset_all();
        let mut fetcher = CacheFetcher::new(self.cache.clone());
        let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
        renderer.render_frames_with(&mut fetcher, store, t.to_audio_frames(), &mut chunk);
        let gain = self.master_gain();
        let mut pl = 0f32;
        let mut pr = 0f32;
        for fr in chunk.chunks_exact(AUDIO_CHANNELS) {
            pl = pl.max((fr[0] * gain).abs());
            pr = pr.max((fr[1] * gain).abs());
        }
        (pl, pr)
    }

    pub fn seek(&self, t: Tick) {
        self.transport.seek(t);
        // workers drop requests of old generations
        let generation = self.transport.generation();
        for w in self.workers.read().unwrap().values() {
            w.cancel_generations_below(generation);
        }
        self.shared.flush.store(true, Ordering::Release);
        self.wake.notify();
    }

    /// Add a media source WITHOUT recreating the player: a new MediaWorker
    /// on the shared cache; transport, ring, FX state and the warmed cache are untouched
    /// (regression item 27: previously adding a file reset the whole cache).
    pub fn add_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        self.worker_spawns.fetch_add(1, Ordering::AcqRel);
        self.remember_native(media, source.as_ref());
        let w = MediaWorker::spawn_with_stats(
            media,
            source,
            self.cache.clone(),
            self.decode_stats.clone(),
        );
        if let Some(old) = self.workers.write().unwrap().insert(media, w) {
            old.shutdown();
        }
        self.rebuild_video_renderer();
    }

    /// Replace a media source ON THE FLY (R.33 item 10: a missing file came back
    /// to disk — the placeholder yields to the real one). The order matters:
    /// first stop the worker, then evict its frames from the cache, and only then
    /// raise the new one — otherwise the old one would manage to write a placeholder
    /// frame into the cache after the invalidation. The transport, position and cache of
    /// the other sources are not touched.
    pub fn replace_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        self.worker_spawns.fetch_add(1, Ordering::AcqRel);
        if let Some(old) = self.workers.write().unwrap().remove(&media) {
            old.shutdown();
        }
        // the native size of the new file may differ from the placeholder — the decode scale step
        // must be recomputed at the next renderer rebuild (R.46)
        self.remember_native(media, source.as_ref());
        self.cache.invalidate_source(kt_cache::SourceId(media.0));
        let w = MediaWorker::spawn_with_stats(
            media,
            source,
            self.cache.clone(),
            self.decode_stats.clone(),
        );
        self.workers.write().unwrap().insert(media, w);
        // the native size could change (placeholder → real file) — the preview renderer
        // must recompute the steps without waiting for a project edit
        self.rebuild_video_renderer();
        // the set of sources has changed — prefetch must wake up and
        // warm the cache anew (at idle it skips passes while
        // the state signature is the same, and without a version bump it would not notice).
        // A CONTENT version, not a project one: the renderer's own rebuild
        // the swap has already done above, and the plan is indifferent to the source.
        self.content_version.fetch_add(1, Ordering::AcqRel);
        self.wake.notify();
    }

    /// Retune a source to a new path WITHOUT recreation (R.53,
    /// item 411): the decoder stream is alive, the renderers are intact, the project is not cloned.
    /// The caller must ask `kt_media::factory::can_retune` — otherwise
    /// the source will reject the path, and the edit silently will not apply.
    ///
    /// `false` — there is no worker (the media is not raised): the caller does
    /// an ordinary swap.
    pub fn retune_media(&self, media: kt_model::MediaId, path: &str) -> bool {
        let workers = self.workers.read().unwrap();
        let Some(w) = workers.get(&media) else { return false };
        w.retune(path);
        drop(workers);
        // the source content changed: prefetch wakes up and warms the
        // cache anew, the renderers are NOT rebuilt
        self.content_version.fetch_add(1, Ordering::AcqRel);
        self.wake.notify();
        true
    }

    /// Bring the GENERATOR SOURCES into line with the project (R.59,
    /// stage 2).
    ///
    /// A diff by ADDRESS: a generator that appeared gets a stream, one that disappeared
    /// loses it; for the remaining ones the values arrive by retuning — the stream
    /// lives, the cache of foreign sources is intact, the renderers are not rebuilt.
    /// Recreation remains for exactly one case: the source's passport
    /// changed (SMPTE with the tone switched on becomes a SOUNDING medium, and
    /// that is already a different set of clips), and then `can_retune_gen` honestly
    /// says “no”.
    pub fn sync_generators(&self, p: &Project) {
        let wanted = crate::gen_sources(p);
        let want_ids: std::collections::HashSet<MediaId> =
            wanted.iter().map(|(id, _)| *id).collect();
        let stale: Vec<MediaId> = self
            .workers
            .read()
            .unwrap()
            .keys()
            .filter(|m| m.0 & kt_model::GEN_TAG != 0 && !want_ids.contains(m))
            .copied()
            .collect();
        for m in stale {
            self.remove_media(m);
            self.cache.invalidate_source(kt_cache::SourceId(m.0));
        }
        for (addr, state) in wanted {
            let live = self.gen_states.lock().unwrap().get(&addr).cloned();
            match live {
                Some(old) if old == state => {}
                Some(old) if kt_media::factory::can_retune_gen(&old, &state) => {
                    self.update_gen_at(addr, state, Vec::new());
                }
                Some(_) => {
                    self.gen_states.lock().unwrap().insert(addr, state.clone());
                    self.replace_media(addr, kt_media::factory::open_gen(&state));
                }
                None => {
                    self.gen_states.lock().unwrap().insert(addr, state.clone());
                    self.add_media(addr, kt_media::factory::open_gen(&state));
                }
            }
        }
    }

    /// New VALUES of one event's generator — without rebuilding
    /// the renderers and without a project version bump (R.59, stage 2; the same
    /// law P as for `set_media_path`).
    ///
    /// `project` is needed to learn the event's PARENT REELS: their
    /// sub-composites are memoized by the model's `content_hash`, and the model
    /// does not move on an edit of values — without an explicit invalidation the reel
    /// would keep returning the old picture.
    pub fn update_gen(
        &self,
        clip: kt_model::ClipId,
        kind: kt_model::GeneratorKind,
        auto: Vec<kt_model::ParamCurve>,
    ) -> bool {
        let addr = crate::gen_source_addr(clip);
        let state = kt_model::GenState { kind, auto };
        let live = self.gen_states.lock().unwrap().get(&addr).cloned();
        match live {
            Some(old) if !kt_media::factory::can_retune_gen(&old, &state) => false,
            Some(_) => {
                // the values of the stored project must remain truthful:
                // `Player::project()` is read by export and diagnostics
                let mut p = self.project.lock().unwrap();
                let put = |cs: &mut Vec<kt_model::ClipEvent>| {
                    for c in cs.iter_mut().filter(|c| c.id == clip) {
                        c.r#gen = Some(state.kind.clone());
                        c.gen_auto = state.auto.clone();
                    }
                };
                put(&mut p.clips);
                for i in 0..p.sequences.len() {
                    let mut cs = std::mem::take(&mut p.sequences[i].clips);
                    put(&mut cs);
                    p.sequences[i].clips = cs;
                }
                let stale: Vec<kt_cache::SourceId> = crate::reel_chain_of_clip(&p, clip)
                    .into_iter()
                    .map(crate::nest_source_id)
                    .collect();
                // ALL addresses that carry the state of this event
                // (R.59, stage 3). For an instance with overrides the
                // source is ITS OWN — the address is salted by the set — and the retune
                // must reach each of them: an edit of the font size in a title
                // of the template would otherwise not reach the second copy before the
                // first plan rebuild. The list is computed by the same
                // `gen_sources` that raises the sources.
                let wanted = crate::gen_sources(&p);
                drop(p);
                self.update_gen_at(addr, state, stale.clone());
                for (a, st) in wanted {
                    if a == addr {
                        continue;
                    }
                    let live = self.gen_states.lock().unwrap().get(&a).cloned();
                    match live {
                        Some(o) if o == st => {}
                        Some(o) if kt_media::factory::can_retune_gen(&o, &st) => {
                            self.update_gen_at(a, st, stale.clone());
                        }
                        Some(_) => {
                            self.gen_states.lock().unwrap().insert(a, st.clone());
                            self.replace_media(a, kt_media::factory::open_gen(&st));
                        }
                        None => {}
                    }
                }
                true
            }
            None => false,
        }
    }

    fn update_gen_at(
        &self,
        addr: MediaId,
        state: kt_model::GenState,
        stale: Vec<kt_cache::SourceId>,
    ) {
        self.gen_states.lock().unwrap().insert(addr, state.clone());
        let workers = self.workers.read().unwrap();
        let Some(w) = workers.get(&addr) else { return };
        w.retune_gen(state, stale);
        drop(workers);
        self.content_version.fetch_add(1, Ordering::AcqRel);
        self.wake.notify();
    }

    /// Request a frame from the worker directly (R.53): the same product
    /// path as `compose_preview_into`, but without composite — needed by
    /// diagnostics and harness tests that check “the edit has reached
    /// the picture”.
    pub fn request_video(&self, media: kt_model::MediaId, pts: Tick, q: Quality) {
        let step = self.video.lock().unwrap().grid.frame_step();
        if let Some(w) = self.workers.read().unwrap().get(&media) {
            w.submit(DecodeRequest {
                kind: RequestKind::video_on_grid(pts, Some(q), step),
                prio: Priority::Playback,
                generation: self.transport.generation(),
            });
        }
    }

    pub fn content_version(&self) -> u64 {
        self.content_version.load(Ordering::Acquire)
    }

    /// How many times a decoder thread was raised over the player's life (R.53):
    /// a guard that an edit of generator parameters does NOT recreate the
    /// source (t500, t502). Not read in the product.
    pub fn worker_spawns(&self) -> u64 {
        self.worker_spawns.load(Ordering::Acquire)
    }

    /// An edit of the media path in the stored project WITHOUT a version bump and rebuild
    /// (R.53, item 412): `Player::project()` must remain truthful,
    /// but the plan is indifferent to the path — the renderers know only `MediaId`.
    pub fn set_media_path(&self, media: kt_model::MediaId, path: &str) {
        let mut p = self.project.lock().unwrap();
        if let Some(m) = p.media.iter_mut().find(|m| m.id == media) {
            m.path = path.to_string();
        }
    }

    /// Remove media (symmetry with RemoveMedia): the worker is stopped, the cache of
    /// other sources is not touched.
    pub fn remove_media(&self, media: kt_model::MediaId) {
        if let Some(w) = self.workers.write().unwrap().remove(&media) {
            w.shutdown();
        }
        self.natives.write().unwrap().forget(media);
    }

    /// Rebuild the preview renderer for the current natives (R.46, item 321):
    /// the prefetch thread does this itself by project version, but the player's renderer
    /// lives until the model is swapped — without this a returning file would not change
    /// the step until the first timeline edit.
    fn rebuild_video_renderer(&self) {
        let p = self.project.lock().unwrap().clone();
        *self.video.lock().unwrap() =
            VideoRenderer::new_with_facts(&p, &self.natives.read().unwrap());
    }

    /// Remember the source facts (R.46 item 321 — native, R.56 item 448 —
    /// proceduralness). A source without video gets neither of them.
    fn remember_native(&self, media: kt_model::MediaId, src: &dyn kt_media::MediaSource) {
        self.natives.write().unwrap().observe(media, src.info());
    }

    /// Decode qualities of video clips at the current preview Quality (R.46,
    /// item 322): the warmth bars must ask coverage about THEM, and
    /// not about Quality — otherwise a warmed clip is shown as cold.
    pub fn video_decode_qualities(&self) -> Vec<(kt_model::ClipId, Quality)> {
        let q = self.preview_quality();
        self.video.lock().unwrap().video_decode_qualities(q)
    }

    /// Warmth of attachments in the ticks of their content (R.50, B6): a reel
    /// has no source, and the bar must ask about its CONTENT.
    pub fn nest_coverage(&self) -> Vec<(kt_model::ClipId, Vec<std::ops::Range<i64>>)> {
        let q = self.preview_quality();
        self.video.lock().unwrap().nest_coverage(&self.cache, q)
    }

    /// The moment that is now AUDIBLE (for choosing the preview video frame):
    /// the playhead minus the output latency. When paused or without output — the playhead
    /// as is; the loop region is accounted for by the wrap (item 30 — A/V desync).
    pub fn video_playhead(&self) -> Tick {
        if !self.transport.is_playing() {
            return self.transport.playhead();
        }
        let latency = self.shared.latency_smooth.load(Ordering::Acquire);
        let head = self.transport.playhead_frames();
        let region = *self.loop_region.lock().unwrap();
        let rate = f32::from_bits(self.shared.playback_rate.load(Ordering::Acquire));
        let dir: i64 = if rate < 0.0 { -1 } else { 1 };
        // interpolation between audio clock quanta (display_extrapolation)
        let extra = {
            let mut vc = self.video_clock.lock().unwrap();
            let now = std::time::Instant::now();
            if head != vc.0 {
                let step = (head - vc.0).abs();
                // a jump greater than a second = seek/wrap, do not spoil the EMA
                if step < AUDIO_SAMPLE_RATE as i64 {
                    vc.2 = if vc.2 == 0 { step } else { (vc.2 * 3 + step) / 4 };
                }
                vc.0 = head;
                vc.1 = now;
            }
            let elapsed = (now.duration_since(vc.1).as_secs_f64()
                * AUDIO_SAMPLE_RATE as f64
                * rate.abs() as f64) as i64;
            display_extrapolation(elapsed, vc.2)
        };
        Tick::from_audio_frames(video_display_frame(head + extra * dir, latency, region, dir, rate))
    }

    /// UI: compose a preview frame for moment t (aligned to the fps grid).
    /// Missing layers are requested with Priority::Playback; returns
    /// (RGBA buffer, size, whether all layers are ready).
    /// The project's frame grid — the preview frame key for the UI (test 39): a tick
    /// aligned to it changes only at a video-frame boundary.
    pub fn video_grid(&self) -> kt_time::FpsGrid {
        self.video.lock().unwrap().grid
    }

    /// Injection of the video GPU executor (R.29). None — forced CPU.
    /// Clears the “poisoning” (a new engine = a new attempt).
    pub fn set_video_gpu(&self, exec: Option<Arc<dyn VideoGpuExec>>) {
        let mut g = self.video_gpu.lock().unwrap();
        g.exec = exec;
        g.poisoned = None;
    }

    /// Status of the video backend for the notice/performance panel.
    pub fn video_backend(&self) -> VideoBackendStatus {
        let g = self.video_gpu.lock().unwrap();
        match (&g.exec, &g.poisoned) {
            (Some(e), None) => VideoBackendStatus::Gpu(e.info().clone()),
            (Some(e), Some(r)) => {
                VideoBackendStatus::Poisoned { info: e.info().clone(), reason: r.clone() }
            }
            (None, _) => VideoBackendStatus::Cpu,
        }
    }

    /// A variant of compose_preview with a reusable buffer (the preview runs 60 times/s:
    /// two allocations of a full RGBA frame per frame — a noticeable part of
    /// the UI frame cost, item 1 of round 5).
    pub fn compose_preview_into(&self, t: Tick, out: &mut Vec<u8>) -> (Quality, bool) {
        let q = self.preview_quality();
        let mut video = self.video.lock().unwrap();
        let aligned = video.grid.align_floor(t);
        let generation = self.transport.generation();
        let step = video.grid.frame_step();
        // the decode quality comes FROM THE REQUEST (R.46, item 318): a clip with a zooming
        // chain is warmed larger than Quality, and exactly that must be asked for
        for (media, src_pts, dq) in video.layers_at(aligned, q) {
            let idx = video_cache_idx(src_pts);
            // contains, not get: the probe must not refresh the LRU (item 19)
            if !self.cache.contains_video(source_id(media), Some(dq), idx)
                && let Some(w) = self.workers.read().unwrap().get(&media)
            {
                w.submit(DecodeRequest {
                    kind: RequestKind::video_on_grid(src_pts, Some(dq), step),
                    prio: Priority::Playback,
                    generation,
                });
            }
        }
        // GPU executor by default (R.29); TooLarge — CPU only for
        // this plan, DeviceLost/Validation — “poison” until the end of the session
        if let Some(exec) = {
            let g = self.video_gpu.lock().unwrap();
            if g.poisoned.is_none() { g.exec.clone() } else { None }
        } {
            let gone = {
                let plan = video.frame_plan(&self.cache, aligned, q);
                match exec.execute_into(&plan, out) {
                    Ok(complete) => return (q, complete),
                    Err(GpuExecError::TooLarge { .. }) => None,
                    Err(e) => Some(format!("{e:?}")),
                }
            };
            if let Some(reason) = gone {
                self.video_gpu.lock().unwrap().poisoned = Some(reason);
            }
        }
        let complete = video.compose_from_cache_into(&self.cache, aligned, q, out);
        (q, complete)
    }

    pub fn compose_preview(&self, t: Tick) -> (Vec<u8>, Quality, bool) {
        let mut out = Vec::new();
        let (q, complete) = self.compose_preview_into(t, &mut out);
        (out, q, complete)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.wake.notify();
        if let Some(t) = self.render_thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self.prefetch_thread.take() {
            let _ = t.join();
        }
        // We will stop the workers by taking them out of the Arc: the render/prefetch threads are already
        // finished, we are the last owner
        if let Some(lock) = Arc::get_mut(&mut self.workers) {
            for (_, w) in lock.get_mut().unwrap().drain() {
                w.shutdown();
            }
        }
    }
}

struct RenderCtx {
    producer: rtrb::Producer<f32>,
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    wake: Arc<Wake>,
    loop_stats: Arc<LoopStats>,
    shared: Arc<AudioShared>,
    workers: Workers,
    project: Arc<Mutex<Project>>,
    dirty: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    meters: Arc<Meters>,
    master_gain: Arc<AtomicU32>,
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
    flight: Arc<crate::flight::FlightRecorder>,
    /// Facts of open sources: from them the render thread takes the END OF
    /// MATERIAL (R.63 item 2) — without it honest silence past the file's end
    /// would be counted as starvation.
    natives: Natives,
}

/// Worker map: RwLock, so that add_media/remove_media add decoders
/// WITHOUT recreating the player (cache/transport/ring survive media edits —
/// regression item 27: previously adding a file recreated the Player and reset
/// the whole warmed cache). Writing locks — only short ones, from the UI thread.
type Workers = Arc<std::sync::RwLock<HashMap<MediaId, MediaWorker>>>;
/// Facts about open sources (R.46 item 321 — native, R.56 item 448 —
/// proceduralness): only the open source knows them, and the renderer needs them
/// on every rebuild.
type Natives = Arc<std::sync::RwLock<crate::video::SourceFacts>>;

fn audio_render_loop(ctx: RenderCtx) {
    let RenderCtx {
        mut producer,
        cache,
        transport,
        wake,
        loop_stats,
        shared,
        workers,
        project,
        dirty,
        stop,
        underruns,
        meters,
        master_gain,
        loop_region,
        flight,
        natives,
    } = ctx;
    let mut store = FxStateStore::new();
    let mut renderer = AudioRenderer::new(&project.lock().unwrap().clone());
    renderer.sync_fx(&mut store);
    let mut fetcher = CacheFetcher::with_lengths(cache.clone(), &natives.read().unwrap().lengths);
    let mut fetcher_facts = natives.read().unwrap().lengths.len();
    // the local render position (frames); may lead the playhead by the ring
    let mut render_head: i64 = transport.playhead_frames();
    let mut last_generation = transport.generation();
    let mut last_dir: i64 = 1;
    // The “armedness” of the loop wrap — transport semantics (the wrap only
    // from inside the region, item 132): a seek past the end (for reverse — past the start)
    // disarms until the next jump, and render/prefetch/transport see
    // effective_region = None (linear playback of what is under the cursor).
    let mut last_region: Option<(i64, i64)> = *loop_region.lock().unwrap();
    let mut loop_armed = loop_arm_after_jump(render_head, last_region, 1);
    let mut last_effective_some = loop_armed && last_region.is_some();
    let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
    // “black box” counters (item 10): deltas per second, not absolutes
    let (mut last_ru, mut last_cu, mut last_warm) = (0u64, 0u64, 0u64);
    let mut warmups = 0u64;
    let mut last_snap_ms = 0u64;
    let mut last_head_seen = render_head;
    let mut last_head_ms = 0u64;

    while !stop.load(Ordering::Acquire) {
        if dirty.swap(false, Ordering::AcqRel) {
            renderer = AudioRenderer::new(&project.lock().unwrap().clone());
            renderer.sync_fx(&mut store); // state survives a rebuild (inv. 3)
        }
        // The source could appear AFTER the stream started (opening the project,
        // a missing file coming back): the fetcher's material edges must be
        // updated, otherwise the tail of new media would again count as
        // starvation. The check is cheap — the length of the map under the RwLock.
        {
            let facts = natives.read().unwrap();
            if facts.lengths.len() != fetcher_facts {
                fetcher_facts = facts.lengths.len();
                fetcher = CacheFetcher::with_lengths(cache.clone(), &facts.lengths);
            }
        }
        let generation = transport.generation();
        let generation_changed = generation != last_generation;
        if generation_changed {
            last_generation = generation;
            store.reset_all(); // pre-roll at the next read
            render_head = transport.playhead_frames();
            shared.consumed_frames.store(0, Ordering::Release);
            flight.push(crate::flight::Ev::Seek, render_head, 0);
        }

        {
            // EMA of the output latency (see AudioShared::latency_smooth)
            let raw = shared.output_latency_frames.load(Ordering::Acquire);
            let sm = shared.latency_smooth.load(Ordering::Acquire);
            shared.latency_smooth.store(sm + (raw - sm) / 8, Ordering::Release);
        }
        let region = *loop_region.lock().unwrap();
        // transport direction — the sign of playback_rate (J4 v1.4). A sign change
        // = a mini-seek without a generation bump: reset of FX, the ring and
        // render_head (the ring holds material of the previous direction).
        let rate_now = f32::from_bits(shared.playback_rate.load(Ordering::Acquire));
        let dir: i64 = if rate_now < 0.0 { -1 } else { 1 };
        let dir_changed = dir != last_dir;
        if dir_changed {
            flight.push(crate::flight::Ev::Dir, dir, 0);
            last_dir = dir;
            store.reset_all();
            render_head = transport.playhead_frames();
            shared.consumed_frames.store(0, Ordering::Release);
            shared.flush.store(true, Ordering::Release);
        }
        // recalculation of armedness — only on jumps (seek/direction/
        // region): with an armed loop render_head grows linearly, and membership
        // in the region is said by the transport's playhead
        if generation_changed || dir_changed || region != last_region {
            if region != last_region {
                match region {
                    Some((rs, re)) => flight.push(crate::flight::Ev::Region, rs, re),
                    None => flight.push(crate::flight::Ev::Region, -1, 0),
                }
            }
            loop_armed = loop_arm_after_jump(transport.playhead_frames(), region, dir);
            last_region = region;
        }
        // a single truth for render, prefetch and transport: a disarmed
        // loop looks like the absence of a region for all consumers
        let effective_region = if loop_armed { region } else { None };
        let effective_some = effective_region.is_some();
        if effective_some != last_effective_some && !generation_changed && !dir_changed {
            // A flip Some↔None on the fly (the region boundary was dragged across the
            // playhead, Loop switched off/on during playback) = a mini-seek without a
            // generation bump: the ring and render_head lived in another coordinate
            // system (the armed render_head leads the playhead by laps).
            flight.push(crate::flight::Ev::Reset, transport.playhead_frames(), 0);
            store.reset_all();
            render_head = transport.playhead_frames();
            shared.consumed_frames.store(0, Ordering::Release);
            shared.flush.store(true, Ordering::Release);
        }
        last_effective_some = effective_some;
        // we advance the playhead by what the callback consumed (with the sign of the
        // direction; the wrap with loop works both ways — test 43)
        let consumed = shared.consumed_frames.swap(0, Ordering::AcqRel);
        if consumed > 0 {
            let before = transport.playhead_frames();
            transport.advance_frames_wrapped(consumed * dir, effective_region);
            let after = transport.playhead_frames();
            // region wrap: the head jumped AGAINST the direction of play
            if (dir > 0 && after < before) || (dir < 0 && after > before) {
                flight.push(crate::flight::Ev::Wrap, after, 0);
            }
        }
        // once a second — a snapshot of the counter deltas; in silence nothing
        // is written (rule 3 of the “black box”)
        {
            let now = flight.now_ms();
            let head = transport.playhead_frames();
            if now.saturating_sub(last_snap_ms) >= 1000 {
                last_snap_ms = now;
                let ru = underruns.load(Ordering::Relaxed);
                let cu = shared.callback_underruns.load(Ordering::Relaxed);
                flight.push_snap(crate::flight::Snap {
                    render_underruns: ru - last_ru,
                    callback_underruns: cu - last_cu,
                    warmup_misses: warmups - last_warm,
                    head_frames: head,
                });
                (last_ru, last_cu, last_warm) = (ru, cu, warmups);
            }
            // sticking: playing, but the head stands still (defect 8.3 — “frames
            // are visible, no sound, playback freezes”).
            if transport.is_playing() {
                if head != last_head_seen {
                    last_head_seen = head;
                    last_head_ms = now;
                } else if now.saturating_sub(last_head_ms) >= 1000 {
                    flight.push(
                        crate::flight::Ev::Stall,
                        now.saturating_sub(last_head_ms) as i64,
                        head,
                    );
                }
            } else {
                last_head_seen = transport.playhead_frames();
                last_head_ms = now;
            }
        }

        if transport.is_playing() {
            let gain = f32::from_bits(master_gain.load(Ordering::Acquire));
            // request the nearest audio with Playback priority (idempotent);
            // in a loop we request via the wrap mapping
            match effective_region {
                Some((rs, re))
                    if re > rs && (if dir < 0 { render_head <= re } else { render_head >= rs }) =>
                {
                    let tl = rs + (render_head - rs).rem_euclid(re - rs);
                    // at speed >1 the lookahead window stretches: otherwise
                    // at 8× 200 ms turn into 25 ms of real time
                    let look =
                        (PLAYBACK_AUDIO_LOOKAHEAD_FRAMES as f32 * rate_now.abs().max(1.0)) as i64;
                    let ahead = look.min(re - rs);
                    if dir < 0 {
                        // reverse: we warm BEHIND the head, the wrap into the tail of the region
                        let start1 = (tl - ahead).max(rs);
                        prefetch_audio_pass(
                            &renderer,
                            &cache,
                            &workers.read().unwrap(),
                            start1..tl,
                            Priority::Playback,
                            generation,
                            dir < 0,
                        );
                        let rest = ahead - (tl - start1);
                        if rest > 0 {
                            prefetch_audio_pass(
                                &renderer,
                                &cache,
                                &workers.read().unwrap(),
                                re - rest..re,
                                Priority::Playback,
                                generation,
                                dir < 0,
                            );
                        }
                    } else {
                        let end1 = (tl + ahead).min(re);
                        prefetch_audio_pass(
                            &renderer,
                            &cache,
                            &workers.read().unwrap(),
                            tl..end1,
                            Priority::Playback,
                            generation,
                            dir < 0,
                        );
                        let rest = ahead - (end1 - tl);
                        if rest > 0 {
                            prefetch_audio_pass(
                                &renderer,
                                &cache,
                                &workers.read().unwrap(),
                                rs..rs + rest,
                                Priority::Playback,
                                generation,
                                dir < 0,
                            );
                        }
                    }
                }
                _ => {
                    let win = if dir < 0 {
                        (render_head
                            - (PLAYBACK_AUDIO_LOOKAHEAD_FRAMES as f32 * rate_now.abs().max(1.0))
                                as i64)
                            .max(0)..render_head
                    } else {
                        render_head
                            ..render_head
                                + (PLAYBACK_AUDIO_LOOKAHEAD_FRAMES as f32 * rate_now.abs().max(1.0))
                                    as i64
                    };
                    prefetch_audio_pass(
                        &renderer,
                        &cache,
                        &workers.read().unwrap(),
                        win,
                        Priority::Playback,
                        generation,
                        dir < 0,
                    );
                }
            }
            // we fill the ring: render_head is linear, loop mapping is inside
            // render_looped_with (the wrap does NOT bump the generation — FX live)
            while producer.slots() >= chunk.len() {
                let stats = if dir < 0 {
                    // reverse (J4 v1.4): the block BEFORE render_head, the material is
                    // reversed frame by frame before the ring; FX — dry
                    // (see render_frames_raw), loop-wrap via rem_euclid
                    let start = render_head - AUDIO_BURST_FRAMES as i64;
                    let st = match effective_region {
                        // the mirror of the forward guard: reverse enters the region
                        // FROM THE RIGHT (the armed approach above plays linearly re), and
                        // having gone below rs it keeps folding — the guard `> rs`
                        // here was killing the loop through the lap (item 133).
                        Some((rs, re)) if re > rs && render_head <= re => {
                            renderer.render_looped_raw(&mut fetcher, rs..re, start, &mut chunk)
                        }
                        _ => renderer.render_frames_raw(&mut fetcher, start, &mut chunk),
                    };
                    reverse_frames(&mut chunk);
                    render_head = start;
                    st
                } else {
                    let st = match effective_region {
                        Some((rs, re)) if re > rs && render_head >= rs => renderer
                            .render_looped_with(
                                &mut fetcher,
                                &mut store,
                                rs..re,
                                render_head,
                                &mut chunk,
                            ),
                        _ => renderer.render_frames_with(
                            &mut fetcher,
                            &mut store,
                            render_head,
                            &mut chunk,
                        ),
                    };
                    render_head += AUDIO_BURST_FRAMES as i64;
                    st
                };
                if stats.underruns > 0 {
                    underruns.fetch_add(stats.underruns, Ordering::Relaxed);
                }
                warmups += stats.warmup_misses;
                if gain != 1.0 {
                    for s in chunk.iter_mut() {
                        *s *= gain;
                    }
                }
                meters.fold(&chunk);
                for &s in chunk.iter() {
                    let _ = producer.push(s);
                }
            }
        }
        // render_head is reset ONLY on seek (generation): on pause the
        // ring keeps the already rendered tail, resume continues from it.
        loop_stats.render_passes.fetch_add(1, Ordering::Relaxed);
        if transport.is_playing() {
            // 8 ms: the ring holds ~85 ms (4 bursts), the callback eats ~21 ms at a
            // time — a four-fold margin; 2 ms were 500 wake-ups/s and
            // heated the CPU on weak machines (item 1 of round 5)
            std::thread::sleep(std::time::Duration::from_millis(8));
        } else {
            // pause: park until play/seek/edit (a safety timeout —
            // to pick up the “tail” consumed after the callback stops)
            wake.wait_timeout(std::time::Duration::from_millis(250));
        }
    }
}

/// Video lookahead of the prefetch: min(2 s, half the video budget) — otherwise
/// prefetch insertions of high qualities evict their own window (item 19b).
fn video_lookahead_ticks(q: Quality, frame_step: i64, max_video_bytes: usize) -> i64 {
    let frame_bytes = (q.width as usize * q.height as usize * 4).max(1);
    let frames = ((max_video_bytes / 2) / frame_bytes).max(2) as i64;
    (frames * frame_step).min(PREFETCH_VIDEO_LOOKAHEAD_TICKS)
}

/// Frame (48k) for display at a given output latency. A pure function —
/// tests 22-sync and 44. Inside the loop region the subtraction is wrapped over the region.
/// `dir` — the transport direction (J4): in reverse what is audible now
/// lies LATER on the timeline (the playhead moves left), so the sign of the
/// latency compensation is reversed.
/// The shown frame's key DOES NOT GO BACK (item 14, round 14).
///
/// Returns (key, cells skipped forward, whether there was a rollback). Backward the key
/// moves ONLY on a change of the transport generation (seek) or loop-wrap —
/// that is, by construction, and not “if the rollback is less than two cells”. The former
/// tolerance was set in cells and was blind to speed: at 0.25× a cell lives
/// 8 UI frames, any jump of the EMA latency overshot the tolerance and the picture
/// rolled back a couple of frames — exactly what people complained about.
pub fn preview_key_step(
    prev: Option<i64>,
    key: i64,
    step: i64,
    dir: i64,
    reanchor: bool,
) -> (i64, u64, bool) {
    let Some(prev) = prev.filter(|_| !reanchor) else { return (key, 0, false) };
    // “Backward” means against the playback direction: in reverse the key
    // must DECREASE, and a decrease is not a rollback
    let ahead = (key - prev) * dir.signum();
    if ahead < 0 {
        return (prev, 0, true);
    }
    let skipped = if step > 0 && ahead > step { ((ahead / step) - 1).max(0) as u64 } else { 0 };
    (key, skipped, false)
}

/// DISPLAY position: the output latency is subtracted from the timeline position.
///
/// `latency_frames` — the latency in WALL-CLOCK frames (that is how the cpal wrapper measures it),
/// and `playhead` is the TIMELINE position. Over `latency` seconds of real time,
/// the timeline goes `latency·|rate|`, so the scaled value must be subtracted
/// (item 14, round 14). Previously the full one was subtracted: at 0.25× that is four times
/// what is needed, and most importantly, the jitter of the EMA latency (hundreds of frames)
/// exceeded the display advance per UI frame (~200 frames at 0.25×)
/// and turned the picture BACKWARD.
pub fn video_display_frame(
    playhead: i64,
    latency_frames: i64,
    region: Option<(i64, i64)>,
    dir: i64,
    rate: f32,
) -> i64 {
    let scaled = (latency_frames.max(0) as f64 * rate.abs().max(0.0) as f64) as i64;
    let lat = scaled * dir.signum().clamp(-1, 1);
    match region {
        // the condition is two-sided, as in advance_frames_wrapped (p == e —
        // inside): a playhead past the region (a seek to the right of the end, a reverse approach
        // from the right) is shown linearly, not folded into the region (item 132)
        Some((rs, re)) if re > rs && playhead >= rs && playhead <= re => {
            let span = re - rs;
            rs + (playhead - rs - lat).rem_euclid(span)
        }
        _ => (playhead - lat).max(0),
    }
}

/// Whether the loop wrap is armed after a jump (seek / change of direction or
/// region): true if movement from `head` in the direction `dir` can naturally
/// cross the region's end from INSIDE — the semantics of advance_frames_wrapped
/// (edge inclusivity: p == e inside, for reverse p == s inside).
/// A disarmed loop (a seek past the end on forward, past the start on reverse)
/// plays linearly what is under the cursor until the next jump (item 132).
pub fn loop_arm_after_jump(head: i64, region: Option<(i64, i64)>, dir: i64) -> bool {
    match region {
        Some((rs, re)) if re > rs => {
            if dir < 0 {
                head >= rs
            } else {
                head <= re
            }
        }
        _ => true,
    }
}

/// The prefetch lookahead window taking into account the loop region: past the region's end
/// there is nothing to warm — playback will not go there, and the evicted region head
/// costs misses on every lap (the performance panel's prefetch-thrash, item 167).
/// A cursor outside the region — a linear window. The length is at most one lap: with a short
/// region 2 s of lookahead would mean dozens of requests for the same thing.
pub fn lookahead_windows(
    head: i64,
    len: i64,
    region: Option<(i64, i64)>,
) -> Vec<std::ops::Range<i64>> {
    match region {
        Some((rs, re)) if re > rs && head >= rs && head <= re => {
            let span = re - rs;
            let h = rs + (head - rs).rem_euclid(span);
            let want = len.min(span);
            let first = (re - h).min(want);
            let mut v: Vec<std::ops::Range<i64>> = std::iter::once(h..h + first).collect();
            if want > first {
                v.push(rs..rs + (want - first));
            }
            v
        }
        // (not `vec![a..b]` — clippy takes it for a Vec range)
        _ => std::iter::once(head..head + len).collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn prefetch_loop(
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    workers: Workers,
    natives: Natives,
    project: Arc<Mutex<Project>>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
    wake: Arc<Wake>,
    loop_stats: Arc<LoopStats>,
    project_version: Arc<AtomicU64>,
    content_version: Arc<AtomicU64>,
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
) {
    // renderers are cached until the project version changes: cloning the Project and
    // rebuilding the graphs every 100 ms was expensive even at idle (item 1)
    let mut cached: Option<(u64, AudioRenderer, VideoRenderer)> = None;
    // state of the last pass: at idle (pause, nothing changed)
    // the pass is skipped — previously the thread hammered 10 times/s with dozens of
    // cache mutex grabs even in complete silence (item 1 of v1.4). Warm-up
    // after a seek/edit/quality change is preserved: any change of
    // state removes the skip.
    // (playhead, project version, generation, quality, loop region)
    // content_version is in the pass state (otherwise an edit of generator
    // parameters at idle would not wake the prefetch), but it is NOT in the
    // RENDERER REBUILD condition: the plan does not depend on parameters
    // (law P, R.53 item 411).
    type PassState = (i64, u64, u64, u64, Quality, Option<(i64, i64)>);
    let mut last_state: Option<PassState> = None;
    while !stop.load(Ordering::Acquire) {
        wake.wait_timeout(std::time::Duration::from_millis(100));
        if stop.load(Ordering::Acquire) {
            return;
        }
        {
            let state = (
                transport.playhead_frames(),
                project_version.load(Ordering::Acquire),
                content_version.load(Ordering::Acquire),
                transport.generation(),
                *preview_quality.lock().unwrap(),
                *loop_region.lock().unwrap(),
            );
            if !transport.is_playing() && last_state == Some(state) {
                continue;
            }
            last_state = Some(state);
        }
        loop_stats.prefetch_passes.fetch_add(1, Ordering::Relaxed);
        let version = project_version.load(Ordering::Acquire);
        if cached.as_ref().map(|(v, _, _)| *v) != Some(version) {
            let p = project.lock().unwrap().clone();
            loop_stats.prefetch_rebuilds.fetch_add(1, Ordering::Relaxed);
            let vr = VideoRenderer::new_with_facts(&p, &natives.read().unwrap());
            cached = Some((version, AudioRenderer::new(&p), vr));
        }
        let (_, renderer, video) = cached.as_ref().unwrap();
        let generation = transport.generation();
        let head = transport.playhead_frames();
        let region = *loop_region.lock().unwrap();
        for win in lookahead_windows(head, PREFETCH_AUDIO_LOOKAHEAD_FRAMES, region) {
            prefetch_audio_pass(
                renderer,
                &cache,
                &workers.read().unwrap(),
                win,
                Priority::NearPrefetch,
                generation,
                false,
            );
        }
        // video lookahead: 2 s, but not more than half the cache budget (item 19b:
        // 4K frames were evicting their own window)
        let t0 = transport.playhead();
        let q = *preview_quality.lock().unwrap();
        let step = video.grid.frame_step();
        // the window budget — by the LARGEST decode quality (R.46, item 318):
        // at Quality the window would overflow the cache by exactly S^2 times
        let ahead =
            video_lookahead_ticks(video.max_decode_quality(q), step, cache.max_video_bytes());
        let region_ticks =
            region.map(|(s, e)| (Tick::from_audio_frames(s).0, Tick::from_audio_frames(e).0));
        let ranges: Vec<TickRange> = lookahead_windows(t0.0, ahead, region_ticks)
            .into_iter()
            .map(|r| TickRange::new(Tick(r.start), Tick(r.end)))
            .collect();
        for (media, src_pts, dq) in ranges.iter().flat_map(|r| video.wanted_frames(*r, q)) {
            if !cache.contains_video(source_id(media), Some(dq), video_cache_idx(src_pts))
                && let Some(w) = workers.read().unwrap().get(&media)
            {
                w.submit(DecodeRequest {
                    kind: RequestKind::video_on_grid(src_pts, Some(dq), step),
                    prio: Priority::NearPrefetch,
                    generation,
                });
            }
        }
    }
}

#[cfg(test)]
mod sync_tests {
    use super::video_display_frame;

    /// Test 22-sync (item 30): compensation of the output latency when choosing a video frame.
    #[test]
    fn t22_video_display_frame_compensates_latency() {
        // without latency — identity
        assert_eq!(video_display_frame(1000, 0, None, 1, 1.0), 1000);
        // subtraction
        assert_eq!(video_display_frame(48_000, 4_800, None, 1, 1.0), 43_200);
        // clamp to 0 at the start
        assert_eq!(video_display_frame(100, 4_800, None, 1, 1.0), 0);
        // negative latency does not move forward
        assert_eq!(video_display_frame(1000, -500, None, 1, 1.0), 1000);
        // wrap in the loop region: 50_500 − 1_000 → wrap to the region's end
        assert_eq!(video_display_frame(50_500, 1_000, Some((50_000, 60_000)), 1, 1.0), 59_500);
        // inside the region without crossing the boundary
        assert_eq!(video_display_frame(55_000, 1_000, Some((50_000, 60_000)), 1, 1.0), 54_000);
        // before the region — linear
        assert_eq!(video_display_frame(10_000, 1_000, Some((50_000, 60_000)), 1, 1.0), 9_000);
    }

    /// Test 44 (J4): in reverse the sign of the latency compensation is reversed —
    /// what is audible now lies LATER on the timeline (the playhead moves left).
    #[test]
    fn t44_latency_sign_flips_in_reverse() {
        assert_eq!(video_display_frame(48_000, 4_800, None, -1, 1.0), 52_800);
        assert_eq!(video_display_frame(48_000, 4_800, None, 1, 1.0), 43_200);
        // in the loop region the wrap goes both ways
        assert_eq!(video_display_frame(59_500, 1_000, Some((50_000, 60_000)), -1, 1.0), 50_500);
        assert_eq!(
            video_display_frame(1000, 0, None, -1, 1.0),
            1000,
            "zero delay is neutral"
        );
    }

    /// Test 134 (item 132, R.23): the conditions for belonging to a loop are consistent with
    /// the transport — a playhead PAST the region is shown linearly, and the
    /// armedness rule is mirrored for reverse.
    #[test]
    fn t134_loop_membership_matches_transport() {
        use super::loop_arm_after_jump;
        let reg = Some((50_000, 60_000));
        // to the right of the end — linear, not folded into the region (the very bug of item 132)
        assert_eq!(video_display_frame(65_000, 1_000, reg, 1, 1.0), 64_000);
        assert_eq!(video_display_frame(60_001, 0, reg, 1, 1.0), 60_001);
        // boundary inclusive, as p <= e in advance_frames_wrapped
        assert_eq!(video_display_frame(60_000, 0, reg, 1, 1.0), 50_000);
        // reverse approach from the right — also linear
        assert_eq!(video_display_frame(62_000, 1_000, reg, -1, 1.0), 63_000);
        // reverse to the left of the start — linear (as it was)
        assert_eq!(video_display_frame(48_000, 1_000, reg, -1, 1.0), 49_000);

        // armedness rule: forward — up to the end, inclusive
        assert!(loop_arm_after_jump(60_000, reg, 1));
        assert!(!loop_arm_after_jump(60_001, reg, 1));
        assert!(loop_arm_after_jump(10_000, reg, 1), "left: linear approach and loop");
        // reverse — the mirror: from the start, inclusive
        assert!(loop_arm_after_jump(50_000, reg, -1));
        assert!(!loop_arm_after_jump(49_999, reg, -1));
        assert!(loop_arm_after_jump(70_000, reg, -1), "right: approach and loop");
        // without a region / a degenerate region — always armed
        assert!(loop_arm_after_jump(0, None, 1));
        assert!(loop_arm_after_jump(0, Some((5, 5)), 1));
    }
}

/// Varispeed core of the callback (item 4): reads source frames via `next_frame`
/// with a fractional step `step` (source frames per output frame) and LINEAR
/// interpolation. `phase` — the fractional position between prev and cur (survives
/// callbacks). Returns the number of source frames consumed — it moves the
/// playhead, so the playback speed scales itself.
/// A pure function — test 32.
#[allow(clippy::too_many_arguments)]
pub fn varispeed_fill(
    next_frame: &mut dyn FnMut() -> Option<(f32, f32)>,
    step: f64,
    out_frames: usize,
    prev: &mut (f32, f32),
    cur: &mut (f32, f32),
    primed: &mut bool,
    phase: &mut f64,
    mut emit: impl FnMut(usize, (f32, f32)),
) -> i64 {
    let mut consumed = 0i64;
    // guard (test 41): a non-positive step does NOT spin while phase>=1.0 —
    // previously phase went to −∞ and the interpolation prev+(cur−prev)·a linearly
    // DIVERGED (dangerously loud). step<=0 = silence, zero consumption.
    if step <= 0.0 || step.is_nan() {
        for i in 0..out_frames {
            emit(i, (0.0, 0.0));
        }
        return 0;
    }
    if !*primed {
        // priming: two frames, so that the prev..cur interpolation is honest
        if let Some(f) = next_frame() {
            *prev = f;
            consumed += 1;
            *cur = next_frame().inspect(|_| consumed += 1).unwrap_or(f);
            *primed = true;
            *phase = 0.0;
        } else {
            // The stream has not started yet (game start, the ring after a flush):
            // there was no first frame, the clock has nothing to go from, and prev/cur
            // hold material BEFORE the seek — they must not be output. This is the
            // only case when time stands still (R.43, item 294).
            for i in 0..out_frames {
                emit(i, (0.0, 0.0));
            }
            return 0;
        }
    }
    // TIME GOES BY THE FRAMES OUTPUT, not by those successfully read (R.43,
    // item 294). Previously starvation of the ring did not increase `consumed`, and the
    // playhead moves exactly by `consumed` (`advance_frames_wrapped`) —
    // so every underrun frame was a frame the timeline did NOT advance over, and
    // the program physically played slower than real time.
    // The user's symptom: “by the way, M3TEOR is somehow slowed down” and “then there is
    // no sound or video at all in the main event”. The audible silence does not
    // disappear from this — it is honestly counted in
    // `callback_underruns`, — but the clock no longer lags behind the sound card.
    let mut starved = 0i64;
    for i in 0..out_frames {
        *phase += step;
        while *phase >= 1.0 {
            *phase -= 1.0;
            *prev = *cur;
            match next_frame() {
                Some(f) => {
                    *cur = f;
                    consumed += 1;
                }
                None => {
                    // underrun: we pull the last frame (without a click), but the frame
                    // WAS SOUNDED — time is advanced over it
                    *cur = *prev;
                    starved += 1;
                }
            }
        }
        let a = *phase as f32;
        emit(i, (prev.0 + (cur.0 - prev.0) * a, prev.1 + (cur.1 - prev.1) * a));
    }
    consumed + starved
}

/// Interpolation of the DISPLAY position between audio clock steps (item 1 of round 5,
/// test 56). The playhead moves in quanta of the audio callback (PipeWire often
/// 2048 samples = 42.7 ms) — without interpolation video physically cannot show more than
/// 1000/quantum distinguishable frames per second (observed: exactly 24 at a quantum of 2048,
/// regardless of quality and backend). Here the wall clock ONLY smooths the display between
/// updates of the master clock: audio remains the master (invariant 6),
/// offline paths do not read this (invariant 11).
/// Extrapolation is clamped to the size of the last step — the display does not run
/// ahead further than will actually be consumed by the next update.
pub fn display_extrapolation(elapsed_frames: i64, step_ema: i64) -> i64 {
    elapsed_frames.clamp(0, step_ema.max(0))
}

/// Frame-by-frame reversal of the interleaved stereo buffer: L/R stay a pair
/// (reverse transport, test 42).
pub fn reverse_frames(buf: &mut [f32]) {
    let n = buf.len() / AUDIO_CHANNELS;
    for i in 0..n / 2 {
        let j = n - 1 - i;
        for ch in 0..AUDIO_CHANNELS {
            buf.swap(i * AUDIO_CHANNELS + ch, j * AUDIO_CHANNELS + ch);
        }
    }
}

/// Ring read step for the cpal callback (test 45): always |rate| — the callback
/// does not know the direction (the material in the ring is already reversed by the render
/// thread, invariant 6 intact); None = |rate|≈0 → silence, zero consumption.
pub fn callback_step(device_step: f64, rate: f32) -> Option<f64> {
    let r = (rate as f64).abs();
    if r < 1e-3 { None } else { Some(device_step * r) }
}

#[cfg(test)]
mod display_clock_tests {
    use super::display_extrapolation;

    /// Test 56 (item 1 of round 5): display interpolation between audio clock
    /// quanta. Simulation: the playhead steps in quanta of 2048 (PipeWire
    /// 42.7 ms — ceiling 23.4 “raw” frames/s), the UI samples at 60 Hz;
    /// with interpolation the sequence of aligned 60 fps grid keys
    /// covers ALL cells in a row (zero skips) and is monotonic.
    #[test]
    fn t56_interpolated_display_hits_every_grid_cell() {
        const QUANTUM: i64 = 2048; // frames per playhead step
        const GRID: i64 = 800; // frames per 60 fps cell (48000/60)
        const UI_DT: i64 = 800; // UI sample every ~16.7 ms in frames
        let step_ema = QUANTUM;
        let mut shown = Vec::new();
        let mut prev_key = -1i64;
        for k in 0..600 {
            let wall = k * UI_DT; // “time” in frames
            let head = (wall / QUANTUM) * QUANTUM; // quantized playhead
            let elapsed = wall - head; // since the last step
            let display = head + display_extrapolation(elapsed, step_ema);
            let key = display / GRID;
            assert!(key >= prev_key, "monotonicity: {prev_key} -> {key}");
            if key != prev_key {
                if prev_key >= 0 {
                    assert_eq!(key, prev_key + 1, "cell skipped: {prev_key} -> {key}");
                }
                shown.push(key);
                prev_key = key;
            }
        }
        // all cells within the observation window are shown (60 frames/s, not 23)
        assert!(shown.len() >= 590, "shown {} cells of ~599", shown.len());

        // WITHOUT interpolation — ceiling 1000/42.7 ≈ 23.4 frames/s (regression)
        let mut raw_keys = std::collections::BTreeSet::new();
        for k in 0..600 {
            let head = ((k * UI_DT) / QUANTUM) * QUANTUM;
            raw_keys.insert(head / GRID);
        }
        assert!(
            raw_keys.len() < 250,
            "too few without key interpolation ({}) — otherwise the test is about something else",
            raw_keys.len()
        );
    }
}

#[cfg(test)]
mod varispeed_tests {
    use super::{callback_step, varispeed_fill};

    /// Test 41 (J4): a non-positive step is safe — silence, zero
    /// consumption, the phase does not run away (previously step<0 gave a diverging
    /// extrapolation — dangerously loud).
    #[test]
    fn t41_nonpositive_step_is_safe_silence() {
        for step in [-1.0f64, -0.25, 0.0] {
            let mut frames = (0..64).map(|i| (i as f32, -(i as f32)));
            let mut next = || frames.next();
            let (mut prev, mut cur, mut primed, mut phase) =
                ((1.0f32, 1.0f32), (2.0f32, 2.0f32), true, 0.25f64);
            let mut out = vec![(9.0f32, 9.0f32); 32];
            let consumed = varispeed_fill(
                &mut next,
                step,
                out.len(),
                &mut prev,
                &mut cur,
                &mut primed,
                &mut phase,
                |i, f| out[i] = f,
            );
            assert_eq!(consumed, 0, "step={step}: nothing consumed");
            assert!(out.iter().all(|&(l, r)| l == 0.0 && r == 0.0), "step={step}: silence");
            assert_eq!(phase, 0.25, "step={step}: phase untouched");
        }
    }

    /// Test 45 (J4): the callback step is always |rate| (the direction is applied by the
    /// render thread); |rate|≈0 → None (silence). Stereo pairing is secured
    /// by construction: the callback does not take half a frame (slots >= 2).
    #[test]
    fn t45_callback_step_uses_abs_rate() {
        assert_eq!(callback_step(1.0, 1.5), Some(1.5));
        assert_eq!(callback_step(1.0, -1.5), Some(1.5), "reverse = same step modulo");
        assert_eq!(callback_step(0.5, -2.0), Some(1.0));
        assert_eq!(callback_step(1.0, 0.0), None, "rate 0 — silence");
        assert_eq!(callback_step(1.0, -0.0004), None);
    }

    fn run(rate: f64, out_frames: usize, src_len: usize) -> (Vec<(f32, f32)>, i64) {
        // source ramp: frame n = (n, -n)
        let mut n = 0usize;
        let mut next = move || {
            if n < src_len {
                let v = n as f32;
                n += 1;
                Some((v, -v))
            } else {
                None
            }
        };
        let mut out = vec![(0.0, 0.0); out_frames];
        let (mut prev, mut cur, mut primed, mut phase) = ((0.0, 0.0), (0.0, 0.0), false, 0.0);
        let consumed = varispeed_fill(
            &mut next,
            rate,
            out_frames,
            &mut prev,
            &mut cur,
            &mut primed,
            &mut phase,
            |i, f| out[i] = f,
        );
        (out, consumed)
    }

    /// Test 32 (item 4): consumption = out×rate (±1 frame of priming),
    /// linear interpolation exact on the ramp, rate=1 — identity.
    #[test]
    fn t32_varispeed_consumption_and_interpolation() {
        // rate=1: identity
        let (out, consumed) = run(1.0, 100, 1000);
        assert_eq!(consumed, 102, "priming (2) + 100 steps");
        for (i, f) in out.iter().enumerate() {
            assert_eq!(f.0, (i + 1) as f32, "rate=1 == frame by frame");
            assert_eq!(f.1, -((i + 1) as f32));
        }

        // rate=0.5: half consumption, interpolation between frames exact
        let (out, consumed) = run(0.5, 100, 1000);
        assert!((consumed - 52).abs() <= 1, "0.5×100 + priming, got {consumed}");
        // ramp: the output value grows by exactly 0.5 per frame
        for w in out.windows(2) {
            let d = w[1].0 - w[0].0;
            assert!((d - 0.5).abs() < 1e-4, "linear ramp interpolation: step {d}");
        }

        // rate=2: double consumption
        let (_, consumed) = run(2.0, 100, 1000);
        assert!((consumed - 202).abs() <= 1, "2×100 + priming, got {consumed}");

        // the source ran dry: we do not panic, we pull the last frame — but TIME
        // goes (R.43, item 294). The former `consumed == 20` asserted the DEFECT:
        // on it the playhead froze, and the whole program played slower than
        // real time.
        let (out, consumed) = run(1.0, 100, 20);
        assert_eq!(consumed, 102, "the clock runs on DELIVERED frames, not on read ones");
        assert_eq!(out[99].0, 19.0, "past the end — last frame");
    }

    /// Test 284 (R.43 item 294): starvation of the ring does NOT stop the clock.
    ///
    /// The complaint, verbatim: “underruns grow… and then there is no
    /// sound or video at all in the main event, only M3TEOR appears
    /// next, by the way M3TEOR is somehow slowed down.”
    ///
    /// Mechanism: the playhead moves exactly by `consumed`
    /// (`advance_frames_wrapped`), while `consumed` counted only the frames
    /// SUCCESSFULLY read from the ring. So every callback-underrun frame
    /// — a frame the timeline did not advance over, and everything that plays after
    /// such a point lags behind real time. It was not M3TEOR that slowed down:
    /// the WHOLE program slowed down.
    #[test]
    fn t284_starved_ring_does_not_stop_the_clock() {
        let t = super::Transport::new();
        t.set_playing(true);
        let out_frames = 512usize;
        let callbacks = 8i64;
        let mut underruns = 0u64;

        // the ring is empty AFTER priming: the stream started and cut off
        let mut left = 2i64;
        let mut next = || {
            if left > 0 {
                left -= 1;
                Some((0.5f32, 0.5f32))
            } else {
                underruns += 1;
                None
            }
        };
        let (mut prev, mut cur, mut primed, mut phase) = ((0.0, 0.0), (0.0, 0.0), false, 0.0);
        let mut total = 0i64;
        for _ in 0..callbacks {
            let mut sink = |_: usize, _: (f32, f32)| {};
            total += varispeed_fill(
                &mut next,
                1.0,
                out_frames,
                &mut prev,
                &mut cur,
                &mut primed,
                &mut phase,
                &mut sink,
            );
        }
        t.advance_frames_wrapped(total, None);

        // +2 — the priming frames of the interpolation, as in t32
        let want = out_frames as i64 * callbacks + 2;
        assert_eq!(
            t.playhead_frames(),
            want,
            "the clock must run on DELIVERED frames: device played {}, \
             playhead moved {} (before R.43 it was 2 — exactly as many as \
             managed to read)",
            out_frames as i64 * callbacks,
            t.playhead_frames()
        );
        assert!(underruns > 0, "silence must honestly count as underruns");
    }

    /// Test 284b (R.43 item 294): while the FIRST frame has not arrived — the clock stands.
    /// This is the only exception: at the game start and right after a seek
    /// the ring is legitimately empty, and there is nothing to advance the playhead from.
    #[test]
    fn t284b_clock_waits_for_the_first_frame() {
        let mut next = || None;
        let (mut prev, mut cur, mut primed, mut phase) =
            ((7.0f32, 7.0f32), (9.0f32, 9.0f32), false, 0.0);
        let mut out = vec![(1.0f32, 1.0f32); 64];
        let consumed = varispeed_fill(
            &mut next,
            1.0,
            out.len(),
            &mut prev,
            &mut cur,
            &mut primed,
            &mut phase,
            |i, f| out[i] = f,
        );
        assert_eq!(consumed, 0, "no thread yet — time does not run");
        assert!(
            out.iter().all(|&(l, r)| l == 0.0 && r == 0.0),
            "and material BEFORE the seek is not delivered outward"
        );
    }

    /// Test 114 (item 14 round 14): the display at a SLOW speed does not roll back.
    ///
    /// We assemble the whole chain at once — the master clock step by speed,
    /// extrapolation, latency compensation (with JITTER, as the EMA moves it),
    /// alignment to the grid and the key ratchet. Previously each piece was checked
    /// in isolation at 1×, and the unit error (the wall-clock latency was subtracted
    /// from the timeline position without accounting for speed) hid between them:
    /// at 0.25× the picture went back a couple of frames —
    /// exactly the user's complaint.
    #[test]
    fn t114_display_key_never_goes_back_at_slow_rates() {
        let fps_step = 90_000 / 30; // a 30 fps cell in ticks
        // 0.01 and 0.05 — with R.57: the transport band now reaches them
        // (its former edge, 0.25, was the ONLY lower case here)
        for &rate in &[0.01f32, 0.05, 0.25, 0.5, 1.0, 2.0, 8.0] {
            for &dir in &[1i64, -1] {
                let quantum = 2048i64; // device frames per callback
                let step = (quantum as f64 * rate as f64) as i64; // head step
                let mut head = 480_000i64;
                let mut prev_key: Option<i64> = None;
                let mut rollbacks = 0;
                // 200 callbacks, with 8 UI frames between them
                for cb in 0..200 {
                    head += step * dir;
                    // latency jitter: the EMA wanders by ±a quantum
                    let latency = 2048 + (cb % 7) * 512;
                    for ui in 0..8 {
                        let elapsed = (quantum as f64 * rate as f64 * (ui as f64 / 8.0)) as i64;
                        let extra = super::display_extrapolation(elapsed, step);
                        let frames = super::video_display_frame(
                            head + extra * dir,
                            latency,
                            None,
                            dir,
                            rate,
                        );
                        let key = kt_time::Tick::from_audio_frames(frames).0.div_euclid(fps_step)
                            * fps_step;
                        let (key, _, rolled) =
                            super::preview_key_step(prev_key, key, fps_step, dir, false);
                        if rolled {
                            rollbacks += 1;
                        }
                        if let Some(p) = prev_key {
                            assert!(
                                (key - p) * dir >= 0,
                                "rate={rate} dir={dir}: display key moved back {p} → {key}"
                            );
                        }
                        prev_key = Some(key);
                    }
                }
                // The ratchet must be a RARE safety net, not a working
                // mode: if it triggers constantly — the display clock is trembling again
                // (that is exactly the bug at 0.25×).
                // The numbers were measured: with the old (unscaled) latency
                // at 0.25× the ratchet caught 186/189 rollbacks out of 1600 samples,
                // with the scaled one — 10/6. At 1× both formulas give
                // the same 34/30, i.e. the fix is neutral to the norm.
                if rate <= 1.0 {
                    assert!(
                        rollbacks < 50,
                        "rate={rate} dir={dir}: {rollbacks} rollbacks — display clock jitters"
                    );
                }
            }
        }
    }

    /// Ratchet: backward only on seek/re-anchoring, forward counts the skips.
    #[test]
    fn t114_preview_key_step_rules() {
        let step = 3000;
        assert_eq!(super::preview_key_step(None, 9000, step, 1, false), (9000, 0, false));
        // a rollback is forbidden at any size — even by 10 cells
        assert_eq!(super::preview_key_step(Some(9000), 6000, step, 1, false), (9000, 0, true));
        assert_eq!(super::preview_key_step(Some(9000), 0, step, 1, false), (9000, 0, true));
        // seek/pause — re-anchoring, the key goes where it is told
        assert_eq!(super::preview_key_step(Some(9000), 0, step, 1, true), (0, 0, false));
        // forward: the skip of cells is counted
        assert_eq!(super::preview_key_step(Some(9000), 12_000, step, 1, false), (12_000, 0, false));
        assert_eq!(super::preview_key_step(Some(9000), 21_000, step, 1, false), (21_000, 3, false));
        // REVERSE: decrease is the norm, increase is a rollback
        assert_eq!(super::preview_key_step(Some(9000), 6000, step, -1, false), (6000, 0, false));
        assert_eq!(super::preview_key_step(Some(9000), 12_000, step, -1, false), (9000, 0, true));
    }
}
