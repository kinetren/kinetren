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
use crate::prefetch::{prefetch_audio_pass, size_frames_cache, video_prefetch_pass_sized};
use crate::transport::Transport;
use crate::video::{VideoRenderer, video_cache_idx};
use crate::video_plan::{GpuBackendInfo, GpuExecError, PresentOptions, VideoGpuExec};
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
/// How long a start waits for its audio (cold-start-audio). Past it the render goes on with what
/// there is: a source that never answers must not stop playback.
const START_AUDIO_GRACE: std::time::Duration = std::time::Duration::from_millis(500);
const PREFETCH_AUDIO_LOOKAHEAD_FRAMES: i64 = 2 * AUDIO_SAMPLE_RATE as i64; // 2 c

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
    /// Frames the last callback handed to the device (a diagnostic for the perf panel:
    /// with the latency it tells a WASAPI-sized buffer from a PipeWire quantum,
    /// loop-play-latency-hold). One relaxed store per callback.
    pub callback_frames: AtomicI64,
    /// Presses of Play so far: the sound device's thread tries an open that failed again on the
    /// next one, though a pause and a Play may come between two of its looks (kept-mix-settle D7).
    pub plays: AtomicU64,
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
    /// How many notifies so far: a parked thread waits for it to move, so a notify that lands
    /// between its last look at the state and its wait is never lost.
    lock: Mutex<u64>,
    cv: std::sync::Condvar,
}

impl Wake {
    pub fn notify(&self) {
        let mut g = self.lock.lock().unwrap();
        *g = g.wrapping_add(1);
        self.cv.notify_all();
    }

    /// The notify count now: taken before a thread looks at the state it may park on.
    pub fn seen(&self) -> u64 {
        *self.lock.lock().unwrap()
    }

    pub fn wait_timeout(&self, d: std::time::Duration) {
        let g = self.lock.lock().unwrap();
        let _ = self.cv.wait_timeout(g, d).unwrap();
    }

    /// Park until a notify, with no clock (one-scheduler: a pause costs nothing). Every event that
    /// changes what a parked thread would do notifies: play, pause, seek, a rate, a quality, a loop
    /// region, a published build, a retuned or replaced source, the player's drop.
    pub fn wait_since(&self, seen: u64) {
        let mut g = self.lock.lock().unwrap();
        while *g == seen {
            g = self.cv.wait(g).unwrap();
        }
    }
}

/// Diagnostics of background loops (test 20: silence at idle).
#[derive(Default)]
pub struct LoopStats {
    pub render_passes: AtomicU64,
    pub prefetch_passes: AtomicU64,
    /// New builds the prefetch thread took (it builds none since edit-without-rebuild).
    pub prefetch_rebuilds: AtomicU64,
    /// The longest the audio render thread spent taking a new renderer in (`sync_fx` and the
    /// kept mix's recheck), in microseconds: the one part of a rebuild still on that thread.
    pub max_swap_us: AtomicU64,
}

pub struct Player {
    /// The scheduler's pause vote (one-scheduler, the owner's "Да", kkt-26.11.08 H15): while this
    /// player plays, thumbnails, effect cells, posters, reel strips, analysis and the index start
    /// nothing; dropped with the player.
    bg_pause: kt_time::sched::Vote,
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
    /// The shown position and its lap (loop-play-latency-hold): the UI releases the
    /// key's ratchet when the SHOWN frame folds, not when the head wraps.
    display_lap: Mutex<DisplayLap>,
    workers: Workers,
    natives: Natives,
    model_dirty: Arc<AtomicBool>,
    /// The one build per project version every consumer reads (edit-without-rebuild): the newest
    /// project handed over, and the renderers last published by the `engine-build` thread. A
    /// composite that holds an old renderer for seconds finishes on it and is abandoned (idle-cpu).
    builds: Arc<crate::builds::Builds>,
    build_thread: Option<std::thread::JoinHandle<()>>,
    /// The sources and the generators raised from the project, shared with the builder thread.
    media: Arc<MediaParts>,
    /// GPU executor of the video composite (R.29): injected from above
    /// (kt-app), lives separately from VideoRenderer — recreating the renderer
    /// on model edits does not lose it. DeviceLost “poisons” it until the end of the
    /// session: no picture, and the notice says why.
    video_gpu: Arc<Mutex<VideoGpuState>>,
    /// The cost of the last preview compose (morpher-event-grid-cost, D1).
    video_cost: Arc<Mutex<VideoCost>>,
    /// The preview's own thread (`compose_queue`): the UI asks and never waits.
    composer: Arc<crate::compose_queue::Composer>,
    compose_thread: Option<std::thread::JoinHandle<()>>,
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
    /// The effect ids of the project last handed to the prewarm (pipeline-prewarm D5).
    reach: Arc<Mutex<crate::prewarm::Reach>>,
    /// The UI's audio renderer and FX states (meters on a pause), synced by the builder thread.
    levels_probe: Arc<Probe>,
    pub loop_stats: Arc<LoopStats>,
    /// What the audio render thread's final-mix store holds (cached-range-plays), as it last
    /// published it.
    pub mix_coverage: crate::mix::MixCoverage,
    /// The moments the preview presented and the executor's key for each, with the versions they
    /// were presented at (cached-range-plays): the bars' "plays without work" for video.
    shown: Shown,
    /// Bumped when `shown` gains a moment or a moment's key changes.
    shown_gen: Arc<AtomicU64>,
}

/// State of the preview GPU executor (R.29).
#[derive(Default)]
struct VideoGpuState {
    exec: Option<Arc<dyn VideoGpuExec>>,
    /// Some(reason) — DeviceLost/Validation: the GPU is poisoned until the end of the
    /// session, the preview shows no picture, the UI shows a notice.
    poisoned: Option<String>,
}

/// Status of the video backend (for the notice and the performance panel row).
#[derive(Debug, Clone)]
pub enum VideoBackendStatus {
    Missing,
    Gpu(GpuBackendInfo),
    Poisoned { info: GpuBackendInfo, reason: String },
}

impl Player {
    /// `sources` — open sources by media id (ownership passes to the workers).
    pub fn new(
        project: Project,
        sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    ) -> (Player, rtrb::Consumer<f32>) {
        Player::with_cache(project, sources, CacheConfig::default())
    }

    /// The same with a cache of the given budget (a test whose history must not fit it,
    /// decode-request-storm t2044).
    pub fn with_cache(
        project: Project,
        sources: Vec<(MediaId, Box<dyn MediaSource>)>,
        config: CacheConfig,
    ) -> (Player, rtrb::Consumer<f32>) {
        let cache = Arc::new(BurstCache::new(config));
        let transport = Arc::new(Transport::new());
        let shared = Arc::new(AudioShared {
            consumed_frames: AtomicI64::new(0),
            flush: AtomicBool::new(false),
            callback_underruns: AtomicU64::new(0),
            playback_rate: AtomicU32::new(1.0f32.to_bits()),
            latency_smooth: AtomicI64::new(0),
            callback_frames: AtomicI64::new(0),
            output_latency_frames: AtomicI64::new(0),
            plays: AtomicU64::new(0),
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
        let video_gpu = Arc::new(Mutex::new(VideoGpuState::default()));
        let model_dirty = Arc::new(AtomicBool::new(true));
        let wake = Arc::new(Wake { lock: Mutex::new(0), cv: std::sync::Condvar::new() });
        let project_version = Arc::new(AtomicU64::new(0));
        let content_version = Arc::new(AtomicU64::new(0));
        let worker_spawns = Arc::new(AtomicU64::new(0));
        let media = Arc::new(MediaParts {
            workers: workers.clone(),
            natives: natives.clone(),
            cache: cache.clone(),
            decode_stats: decode_stats.clone(),
            worker_spawns: worker_spawns.clone(),
            content_version: content_version.clone(),
            wake: wake.clone(),
            gen_states: Mutex::new(HashMap::new()),
            gen_lock: Mutex::new(()),
        });
        // The opening builds once, here; every later version is built on `engine-build`. The
        // generator sources are raised by the player ITSELF, inside the build: their addresses are
        // derived from events, and the caller has nowhere to take them from.
        let project = Arc::new(project);
        let mut memo = crate::incremental::BuildMemo::default();
        let first = build_version(&media, &cache, &project, 0, 0, &mut memo, None);
        let probe = Arc::new(Probe::default());
        sync_probe(&probe, &first.audio, 0);
        let builds = Arc::new(crate::builds::Builds::new(first));
        let loop_stats = Arc::new(LoopStats::default());
        let stop = Arc::new(AtomicBool::new(false));
        let underruns = Arc::new(AtomicU64::new(0));
        let flight = Arc::new(crate::flight::FlightRecorder::new());
        let mix_coverage = crate::mix::MixCoverage::default();

        let (producer, consumer) =
            rtrb::RingBuffer::<f32>::new(RING_CAPACITY_FRAMES * AUDIO_CHANNELS);

        let render_thread = {
            let wake_r = wake.clone();
            let stats_r = loop_stats.clone();
            let cache = cache.clone();
            let transport = transport.clone();
            let shared = shared.clone();
            let workers = workers.clone();
            let builds = builds.clone();
            let dirty = model_dirty.clone();
            let stop = stop.clone();
            let underruns = underruns.clone();
            let meters = meters.clone();
            let master_gain = master_gain.clone();
            let loop_region = loop_region.clone();
            let flight_r = flight.clone();
            let natives_r = natives.clone();
            let mix_r = mix_coverage.clone();
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
                        builds,
                        dirty,
                        stop,
                        underruns,
                        meters,
                        master_gain,
                        loop_region,
                        flight: flight_r,
                        natives: natives_r,
                        mix_coverage: mix_r,
                    })
                })
                .expect("spawn audio render")
        };

        let preview_quality = Arc::new(Mutex::new(Quality { width: 640, height: 360 }));
        let shown: Shown = Arc::new(Mutex::new(std::collections::BTreeMap::new()));
        let prefetch_thread = {
            let wake_p = wake.clone();
            let stats_p = loop_stats.clone();
            let version_p = project_version.clone();
            let content_p = content_version.clone();
            let cache = cache.clone();
            let transport = transport.clone();
            let workers = workers.clone();
            let builds = builds.clone();
            let stop = stop.clone();
            let quality = preview_quality.clone();
            let region_p = loop_region.clone();
            let shared_p = shared.clone();
            let mix_p = mix_coverage.clone();
            let shown_p = shown.clone();
            let gpu_p = video_gpu.clone();
            std::thread::Builder::new()
                .name("prefetch".into())
                .spawn(move || {
                    prefetch_loop(
                        cache, transport, workers, builds, quality, stop, wake_p,
                        stats_p, version_p, content_p, region_p, shared_p, mix_p, shown_p, gpu_p,
                    )
                })
                .expect("spawn prefetch")
        };

        let video_cost = Arc::new(Mutex::new(VideoCost::default()));
        let shown_gen = Arc::new(AtomicU64::new(0));
        let composer = crate::compose_queue::Composer::new();
        let compose_thread = crate::compose_queue::spawn(
            composer.clone(),
            PreviewParts {
                cache: cache.clone(),
                transport: transport.clone(),
                workers: workers.clone(),
                builds: builds.clone(),
                video_gpu: video_gpu.clone(),
                video_cost: video_cost.clone(),
                preview_quality: preview_quality.clone(),
                project_version: project_version.clone(),
                content_version: content_version.clone(),
                shown: shown.clone(),
                shown_gen: shown_gen.clone(),
            },
        );
        let build_thread = {
            let ctx = BuildCtx {
                builds: builds.clone(),
                media: media.clone(),
                cache: cache.clone(),
                probe: probe.clone(),
                loop_region: loop_region.clone(),
                wake: wake.clone(),
                composer: composer.clone(),
                memo,
            };
            std::thread::Builder::new().name("engine-build".into()).spawn(move || build_loop(ctx)).expect("spawn the engine builder")
        };
        let player = Player {
            bg_pause: kt_time::sched::Vote::new(kt_time::sched::why::PLAYING),
            cache,
            natives,
            flight,
            transport,
            shared,
            meters,
            master_gain,
            loop_region,
            video_clock: Mutex::new((0, std::time::Instant::now(), 0)),
            display_lap: Mutex::new(DisplayLap::default()),
            workers,
            model_dirty,
            builds,
            build_thread: Some(build_thread),
            media,
            video_gpu,
            video_cost,
            composer,
            compose_thread: Some(compose_thread),
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
            reach: Arc::default(),
            loop_stats,
            levels_probe: probe,
            mix_coverage,
            shown,
            shown_gen,
        };
        player.prewarm_reach(&project);
        (player, consumer)
    }

    /// Hand the player a new project state. Nothing is built here (edit-without-rebuild): the
    /// `engine-build` thread builds the version once for every consumer, and raises its generator
    /// sources (a function of the PROJECT, their addresses derived from events) before it publishes.
    /// The app hands the model's shared snapshot (`Model::doc_arc`): no copy per edit.
    pub fn set_project(&self, p: impl Into<Arc<Project>>) {
        let p = p.into();
        kt_time::work::bump(kt_time::work::Work::ProjectSwap);
        self.prewarm_reach(&p);
        // the want first, the version after: whoever reads the new version finds its build asked
        let version = self.project_version.load(Ordering::Acquire) + 1;
        self.builds.want_project(p, version);
        self.project_version.store(version, Ordering::Release);
        // the edit in the trace: the build of this version and what it invalidates follow
        kt_time::trace::ev(kt_time::trace::Ev::Edit).b(version).emit();
        self.model_dirty.store(true, Ordering::Release);
        self.wake.notify();
    }

    /// Hand the effects the project reaches to the executor's and the compute host's prewarm when
    /// the set changed (pipeline-prewarm D5): an effect added compiles beside, before its first
    /// frame. Both return at once; a project whose set did not change asks nothing.
    fn prewarm_reach(&self, p: &Project) {
        let now = crate::prewarm::Reach { ids: crate::prewarm::effect_ids(p), runs: crate::prewarm::run_shapes(p) };
        {
            let mut last = self.reach.lock().unwrap();
            if *last == now {
                return;
            }
            *last = now;
        }
        self.prewarm_now();
    }

    fn prewarm_now(&self) {
        let (ids, runs) = self.reach.lock().unwrap().lists();
        let exec = self.video_gpu.lock().unwrap().exec.clone();
        if let Some(e) = exec {
            e.prewarm(&ids);
            e.prewarm_runs(&runs);
        }
        crate::prewarm::compute(&ids);
    }

    /// The newest project handed over (export, diagnostics): a pointer, not a copy.
    pub fn project(&self) -> Arc<Project> {
        self.builds.want.lock().unwrap().project.clone()
    }

    /// The last published build (edit-without-rebuild): what the render thread, prefetch, the
    /// probe and the preview read. A test compares its renderers' pointers.
    #[doc(hidden)]
    pub fn built(&self) -> Arc<crate::builds::Built> {
        self.builds.latest()
    }

    /// The build of the newest project state, waited for: a test's (or an offline tool's) way to
    /// read what the edit it just made built. Never called on the UI thread.
    pub fn wait_built(&self) -> Arc<crate::builds::Built> {
        self.builds.wait_current()
    }

    /// The want the last published build answers: it moves when the renderers the UI reads (the
    /// grid, the coverage bars) change — a key of their caches next to the model revision.
    pub fn built_seq(&self) -> u64 {
        self.builds.built_seq()
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
        // a parked render thread meets a new direction now, not at the next play
        self.wake.notify();
    }

    pub fn playback_rate(&self) -> f32 {
        f32::from_bits(self.shared.playback_rate.load(Ordering::Acquire))
    }

    pub fn set_preview_quality(&self, q: Quality) {
        *self.preview_quality.lock().unwrap() = q;
        // a parked prefetch warms the new size
        self.wake.notify();
    }

    pub fn preview_quality(&self) -> Quality {
        *self.preview_quality.lock().unwrap()
    }

    /// SOURCE VIEW (native × pixel aspect ratio) — for the interface, so that it can compute the
    /// placement frame with the same function as the render (R.64). The interface does not reach the
    /// decoder: the instance is owned by the worker (invariant 5), and the player already holds the
    /// source facts itself.
    pub fn source_view(&self, media: MediaId) -> Option<(f64, f64)> {
        self.natives.read().ok()?.view_of(media)
    }

    /// The LOGICAL raster of what is playing (the project's, or the entered reel's): the frame
    /// of "as is" is counted in its pixels, not in the preview canvas's (still-image-frame-rule).
    pub fn raster(&self) -> (u32, u32) {
        let w = self.builds.want.lock().unwrap();
        (w.project.settings.width, w.project.settings.height)
    }

    /// The notify count the render and prefetch threads park on: every input they act on moves it
    /// (play, pause, a seek, a rate, a region, a build, the drop). The sound device's thread parks on
    /// it too, so it opens on Play with no clock of its own (kept-mix-settle D5).
    pub fn wake(&self) -> Arc<Wake> {
        self.wake.clone()
    }

    pub fn play(&self) {
        self.flight.push(crate::flight::Ev::Play, self.transport.playhead_frames(), 0);
        self.shared.plays.fetch_add(1, Ordering::AcqRel);
        self.transport.set_playing(true);
        self.bg_pause.set(true);
        self.wake.notify();
    }

    pub fn pause(&self) {
        self.flight.push(crate::flight::Ev::Pause, self.transport.playhead_frames(), 0);
        self.transport.set_playing(false);
        self.bg_pause.set(false);
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
    /// so that the wrap does not start cold (test 14). Not a model edit: it builds nothing — the
    /// head is pinned with the renderer already built (edit-without-rebuild), and pinned again by
    /// the builder thread when a new version lands.
    pub fn set_loop_region(&self, region: Option<TickRange>) {
        let frames = region.map(|r| (r.start.to_audio_frames(), r.end.to_audio_frames()));
        *self.loop_region.lock().unwrap() = frames;
        self.cache.unpin_all();
        if let Some((s, _)) = frames {
            let built = self.builds.latest();
            pin_loop_head(&self.cache, &built.audio, s);
        }
        self.wake.notify();
    }

    /// The UI's FX states over the last published audio renderer (the meters on a pause). The
    /// builder thread syncs them when a version lands, off the UI thread; `None` for the moment it
    /// holds them.
    fn with_probe<R>(&self, f: impl FnOnce(&AudioRenderer, &mut FxStateStore) -> R) -> Option<R> {
        let mut probe = self.levels_probe.slot.lock().unwrap();
        let (_, renderer, store) = probe.as_mut()?;
        Some(f(renderer, store))
    }

    /// The project version of the UI's own audio renderer that answers now.
    #[doc(hidden)]
    pub fn probe_version(&self) -> Option<u64> {
        self.levels_probe.slot.lock().unwrap().as_ref().map(|(v, _, _)| *v)
    }

    /// Hold the builder thread before its next build until opened again: a test of what the UI does
    /// while a build takes its time.
    #[doc(hidden)]
    pub fn gate_builds(&self, closed: bool) {
        self.builds.set_gate(closed);
    }

    /// The former name of `gate_builds` (t1844): the probe has no builder of its own any more.
    #[doc(hidden)]
    pub fn gate_probe_builds(&self, closed: bool) {
        self.gate_builds(closed);
    }

    /// Peak levels on pause: one burst from position t through a one-off
    /// FX state and the cache (a miss = silence — enough for the meter).
    pub fn sample_levels_at(&self, t: Tick) -> (f32, f32) {
        // Probe cache (item 1): previously EVERY call (8 times/s on pause) cloned
        // the whole Project and recreated all FX. Now the graph is the shared build's and the
        // states are synced by the builder; the state is reset before measuring (determinism).
        let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
        self.with_probe(|renderer, store| {
            store.reset_all();
            let mut fetcher = CacheFetcher::new(self.cache.clone());
            renderer.render_frames_with(&mut fetcher, store, t.to_audio_frames(), &mut chunk);
        });
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
    /// (regression item 27: previously adding a file reset the whole cache). The version is
    /// built again with the new facts (the decode step, R.46) on the builder thread.
    pub fn add_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        self.media.add_media(media, source);
        self.builds.refresh();
    }

    /// Replace a media source ON THE FLY (R.33 item 10: a missing file came back
    /// to disk — the placeholder yields to the real one). See `MediaParts::replace_media`; the
    /// native size could change (placeholder → real file), so the version is built again with
    /// the new facts without waiting for a project edit.
    pub fn replace_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        self.media.replace_media(media, source);
        self.builds.refresh();
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

    /// Bring the GENERATOR SOURCES into line with `p` now (see `MediaParts::sync_generators`).
    /// The player does it itself inside every build; this is the app's door for a source whose
    /// passport changed under a value edit, and the version is built again with the new facts.
    pub fn sync_generators(&self, p: &Project) {
        let _g = self.media.gen_lock.lock().unwrap();
        self.media.sync_generators(crate::gen_sources(p));
        self.builds.refresh();
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
        // the builder's generator sync reads the newest project under the same lock: it cannot
        // retune a source back to the values this edit replaces
        let _g = self.media.gen_lock.lock().unwrap();
        let live = self.media.gen_states.lock().unwrap().get(&addr).cloned();
        match live {
            Some(old) if !kt_media::factory::can_retune_gen(&old, &state) => false,
            Some(_) => {
                // the values of the stored project must remain truthful:
                // `Player::project()` is read by export and diagnostics
                let p = {
                    let mut w = self.builds.want.lock().unwrap();
                    let p = Arc::make_mut(&mut w.project);
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
                    w.project.clone()
                };
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
                self.media.update_gen_at(addr, state, stale.clone());
                for (a, st) in wanted {
                    if a == addr {
                        continue;
                    }
                    let live = self.media.gen_states.lock().unwrap().get(&a).cloned();
                    match live {
                        Some(o) if o == st => {}
                        Some(o) if kt_media::factory::can_retune_gen(&o, &st) => {
                            self.media.update_gen_at(a, st, stale.clone());
                        }
                        Some(_) => {
                            self.media.gen_states.lock().unwrap().insert(a, st.clone());
                            self.media.replace_media(a, kt_media::factory::open_gen(&st));
                            self.builds.refresh();
                        }
                        None => {}
                    }
                }
                true
            }
            None => false,
        }
    }

    /// Request a frame from the worker directly (R.53): the same product
    /// path as `compose_preview_into`, but without composite — needed by
    /// diagnostics and harness tests that check “the edit has reached
    /// the picture”.
    /// A Playback request is the frame on screen: the next preview composite
    /// withdraws it unless it is a layer of that frame (video-prefetch-thrash), so a
    /// caller that waits for the frame re-submits in its loop.
    pub fn request_video(&self, media: kt_model::MediaId, pts: Tick, q: Quality) {
        let step = self.renderer().grid.frame_step();
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
        let mut w = self.builds.want.lock().unwrap();
        if w.project.media.iter().any(|m| m.id == media && m.path != path)
            && let Some(m) = Arc::make_mut(&mut w.project).media.iter_mut().find(|m| m.id == media)
        {
            m.path = path.to_string();
        }
    }

    /// Remove media (symmetry with RemoveMedia): the worker is stopped, the cache of
    /// other sources is not touched.
    pub fn remove_media(&self, media: kt_model::MediaId) {
        self.media.remove_media(media);
    }

    /// The player's sources as a handle a job can carry (one-scheduler, row 148): adding,
    /// replacing and removing a source joins the old decoder's thread, which the UI thread must not
    /// wait for. The same bodies as `add_media`/`replace_media`/`remove_media`.
    pub fn sources(&self) -> Sources {
        Sources { media: self.media.clone(), builds: self.builds.clone() }
    }

    /// Decode qualities of video clips at the current preview Quality (R.46,
    /// item 322): the warmth bars must ask coverage about THEM, and
    /// not about Quality — otherwise a warmed clip is shown as cold.
    pub fn video_decode_qualities(&self) -> Vec<(kt_model::ClipId, Quality)> {
        let q = self.preview_quality();
        self.renderer().video_decode_qualities(q)
    }

    /// Warmth of attachments in the ticks of their content (R.50, B6): a reel
    /// has no source, and the bar must ask about its CONTENT.
    pub fn nest_coverage(&self) -> Vec<(kt_model::ClipId, Vec<std::ops::Range<i64>>)> {
        let q = self.preview_quality();
        self.renderer().nest_coverage(&self.cache, q)
    }

    /// The moment that is now AUDIBLE (for choosing the preview video frame):
    /// the playhead minus the output latency. When paused or without output — the playhead
    /// as is; the loop region is accounted for by the wrap (item 30 — A/V desync).
    pub fn video_playhead(&self) -> Tick {
        self.video_playhead_lap().0
    }

    /// `video_playhead` plus the display lap (`DisplayLap`): the number changes on the
    /// UI frame where the shown frame folds back into the loop region, or the display
    /// re-anchors. The preview's ratchet is released by it (and the generation), not by
    /// `Transport::wraps`: under an output latency the head wraps `latency` earlier.
    pub fn video_playhead_lap(&self) -> (Tick, u64) {
        if !self.transport.is_playing() {
            let mut dl = self.display_lap.lock().unwrap();
            dl.pause();
            return (self.transport.playhead(), dl.lap());
        }
        let latency = self.shared.latency_smooth.load(Ordering::Acquire);
        let head = self.transport.playhead_frames();
        let region = *self.loop_region.lock().unwrap();
        let rate = f32::from_bits(self.shared.playback_rate.load(Ordering::Acquire));
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
        let (frame, lap) = self.display_lap.lock().unwrap().step(DisplaySample {
            head,
            extra,
            wraps: self.transport.wraps(),
            generation: self.transport.generation(),
            jump_target: self.transport.last_seek_frames(),
            latency,
            region,
            rate,
        });
        (Tick::from_audio_frames(frame), lap)
    }

    /// UI: compose a preview frame for moment t (aligned to the fps grid).
    /// Missing layers are requested with Priority::Playback; returns
    /// (RGBA buffer, size, whether all layers are ready).
    /// The project's frame grid — the preview frame key for the UI (test 39): a tick
    /// aligned to it changes only at a video-frame boundary.
    pub fn video_grid(&self) -> kt_time::FpsGrid {
        self.renderer().grid
    }

    fn renderer(&self) -> Arc<VideoRenderer> {
        self.builds.latest().video.clone()
    }

    /// Injection of the video GPU executor (R.29). None — no picture.
    /// Clears the “poisoning” (a new engine = a new attempt).
    pub fn set_video_gpu(&self, exec: Option<Arc<dyn VideoGpuExec>>) {
        {
            let mut g = self.video_gpu.lock().unwrap();
            g.exec = exec;
            g.poisoned = None;
        }
        // a new executor has warmed nothing of this project yet
        self.prewarm_now();
        // and has not been told the loop to keep (the prefetch thread tells it)
        self.wake.notify();
    }

    /// Status of the video backend for the notice/performance panel.
    pub fn video_backend(&self) -> VideoBackendStatus {
        let g = self.video_gpu.lock().unwrap();
        match (&g.exec, &g.poisoned) {
            (Some(e), None) => VideoBackendStatus::Gpu(e.info().clone()),
            (Some(e), Some(r)) => {
                VideoBackendStatus::Poisoned { info: e.info().clone(), reason: r.clone() }
            }
            (None, _) => VideoBackendStatus::Missing,
        }
    }

    /// A variant of compose_preview with a reusable buffer (the preview runs 60 times/s:
    /// two allocations of a full RGBA frame per frame — a noticeable part of
    /// the UI frame cost, item 1 of round 5).
    pub fn compose_preview_into(&self, t: Tick, out: &mut Vec<u8>) -> (Quality, bool) {
        // the wall clock is legal here: the preview, not an offline path (invariant 11)
        let t0 = std::time::Instant::now();
        let (q, complete, gpu, cost, plan_ns, stages) = self
            .compose_preview_measured(t, Target::Bytes(out))
            .unwrap_or((self.preview_quality(), false, false, Default::default(), 0, Default::default()));
        *self.video_cost.lock().unwrap() =
            VideoCost { gpu, cost, compose_ns: t0.elapsed().as_nanos() as u64, plan_ns, stages };
        (q, complete)
    }

    /// Ask for the frame at `t` on the preview's own thread and go on (idle-cpu, stage 2): the UI
    /// never waits for a picture. `present` — on the display target, if the executor has one and
    /// the chain is fast; else as bytes. The newest ask replaces a waiting one; the answer comes
    /// from `take_composed`. Returns the ask's number.
    pub fn ask_preview(&self, t: Tick, present: Option<PresentOptions>) -> u64 {
        use kt_time::trace::ask;
        let flags = if present.is_some() { ask::PRESENT } else { 0 }
            | if self.transport.is_playing() { ask::PLAYING } else { 0 };
        self.composer.ask(t, present, self.preview_parts().versions(), flags)
    }

    /// The newest finished frame, waiting for it at most `wait`.
    pub fn take_composed(&self, wait: std::time::Duration) -> Option<crate::compose_queue::Composed> {
        self.composer.take(wait)
    }

    /// The bytes of a finished frame go back for the next one.
    pub fn give_back_preview_buffer(&self, buf: Vec<u8>) {
        self.composer.give_back(buf);
    }

    /// Called on the preview's thread when a frame is finished.
    pub fn set_compose_wake(&self, f: Arc<dyn Fn() + Send + Sync>) {
        self.composer.set_wake(f);
    }

    /// The executor a slow chain moves to: a device of its own (`compose_queue`). The setter goes
    /// to a thread: building that device takes seconds.
    pub fn slow_gpu_setter(&self) -> impl FnOnce(Option<Arc<dyn VideoGpuExec>>) + Send + 'static {
        let composer = self.composer.clone();
        let reach = self.reach.clone();
        move |exec| {
            // the slow chains of this project compose there: warm them as on the main device
            if let Some(e) = &exec {
                let (ids, runs) = reach.lock().unwrap().lists();
                e.prewarm(&ids);
                e.prewarm_runs(&runs);
            }
            composer.set_slow(exec)
        }
    }

    /// Compose the frame at `t` onto the executor's display target, without a readback
    /// (preview-throughput D2). `None` — no executor, a poisoned one, or a plan it refused
    /// (`TooLarge`).
    pub fn present_preview(&self, t: Tick, o: PresentOptions) -> Option<(Quality, bool)> {
        // the wall clock is legal here: the preview, not an offline path (invariant 11)
        let t0 = std::time::Instant::now();
        {
            let g = self.video_gpu.lock().unwrap();
            if g.poisoned.is_some() || !g.exec.as_ref().is_some_and(|e| e.can_present()) {
                return None;
            }
        }
        let (q, complete, gpu, cost, plan_ns, stages) =
            self.compose_preview_measured(t, Target::Display(o))?;
        *self.video_cost.lock().unwrap() =
            VideoCost { gpu, cost, compose_ns: t0.elapsed().as_nanos() as u64, plan_ns, stages };
        Some((q, complete))
    }

    /// Wait for what `present_preview` handed to the device (`preview_bench`: a real window's
    /// swapchain does this; without it a headless run would race ahead of the device).
    pub fn finish_present(&self) {
        let exec = self.video_gpu.lock().unwrap().exec.clone();
        if let Some(e) = exec {
            e.finish_present();
        }
    }

    /// The cost of the last preview compose: the backend that ACTUALLY composed it, what it
    /// ran and how long it took (morpher-event-grid-cost, D1; the performance panel).
    pub fn video_cost(&self) -> VideoCost {
        *self.video_cost.lock().unwrap()
    }

    fn compose_preview_measured(
        &self,
        t: Tick,
        target: Target<'_>,
    ) -> Option<(Quality, bool, bool, kt_fx::video::FrameCost, u64, crate::video_plan::GpuStages)> {
        self.preview_parts().compose(t, target)
    }

    fn preview_parts(&self) -> PreviewParts {
        PreviewParts {
            cache: self.cache.clone(),
            transport: self.transport.clone(),
            workers: self.workers.clone(),
            builds: self.builds.clone(),
            video_gpu: self.video_gpu.clone(),
            video_cost: self.video_cost.clone(),
            preview_quality: self.preview_quality.clone(),
            project_version: self.project_version.clone(),
            content_version: self.content_version.clone(),
            shown: self.shown.clone(),
            shown_gen: self.shown_gen.clone(),
        }
    }

    /// The layers of the frame at `t` the cache does not hold (a diagnostic of `preview_bench`).
    pub fn missing_layers(&self, t: Tick) -> Vec<(MediaId, (u32, u32))> {
        let q = self.preview_quality();
        let layers = self.renderer().layers_at(t, q);
        layers
            .into_iter()
            .filter(|(m, p, dq)| {
                !self.cache.contains_video(source_id(*m), Some(*dq), video_cache_idx(*p))
            })
            .map(|(m, _, dq)| (m, (dq.width, dq.height)))
            .collect()
    }

    pub fn compose_preview(&self, t: Tick) -> (Vec<u8>, Quality, bool) {
        let mut out = Vec::new();
        let (q, complete) = self.compose_preview_into(t, &mut out);
        (out, q, complete)
    }
}

/// The layers of a composed plan in the trace: what the kept key is made of, so a key that moved
/// between two laps names the layer that moved (a re-decode, a held stream frame).
fn trace_layers(frame: u64, plan: &crate::video_plan::FramePlan<'_>) {
    use crate::video_plan::PlanLayer;
    use kt_time::trace::{self, Ev};
    let mut i = 0u32;
    let mut put = |s: &crate::video_plan::LayerSrc<'_>| {
        trace::ev(Ev::FrameLayer).id(frame).a(s.source.0).b(s.cache_idx as u64).c(s.content).aux(i).emit();
        i += 1;
    };
    for l in &plan.layers {
        match l {
            PlanLayer::One { src, .. } => put(src),
            PlanLayer::Dissolve { a, b, .. } => {
                put(a);
                put(b);
            }
        }
    }
}

/// A lock the frame took, its wait in the trace when it stood a millisecond or more.
fn locked<'a, T>(m: &'a Mutex<T>, frame: u64, what: &'static str) -> std::sync::MutexGuard<'a, T> {
    if let Ok(g) = m.try_lock() {
        return g;
    }
    let t = std::time::Instant::now();
    let g = m.lock().unwrap_or_else(|p| p.into_inner());
    let ns = t.elapsed().as_nanos() as u64;
    if ns >= 1_000_000 {
        use kt_time::trace;
        trace::ev(trace::Ev::FrameWait).id(frame).aux(trace::wait::LOCK).a(trace::name(what) as u64).lasted(ns).emit();
    }
    g
}

/// What a composite needs of the player, shared with its thread.
#[derive(Clone)]
pub(crate) struct PreviewParts {
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    workers: Workers,
    builds: Arc<crate::builds::Builds>,
    video_gpu: Arc<Mutex<VideoGpuState>>,
    video_cost: Arc<Mutex<VideoCost>>,
    preview_quality: Arc<Mutex<Quality>>,
    project_version: Arc<AtomicU64>,
    content_version: Arc<AtomicU64>,
    shown: Shown,
    shown_gen: Arc<AtomicU64>,
}

impl PreviewParts {
    pub(crate) fn compose(
        &self,
        t: Tick,
        target: Target<'_>,
    ) -> Option<(Quality, bool, bool, kt_fx::video::FrameCost, u64, crate::video_plan::GpuStages)> {
        self.compose_via(None, None, t, target)
    }

    pub(crate) fn can_present(&self) -> bool {
        let g = self.video_gpu.lock().unwrap();
        g.poisoned.is_none() && g.exec.as_ref().is_some_and(|e| e.can_present())
    }

    /// One frame on the device at a time: the next is not submitted before this one is done.
    pub(crate) fn wait_presented(&self) {
        let exec = self.video_gpu.lock().unwrap().exec.clone();
        if let Some(e) = exec {
            e.wait_presented();
        }
    }

    /// The cells `versions` reads, for a check that outlives a borrow (a stream stop, gpu-arbiter).
    pub(crate) fn version_cells(&self) -> (Arc<AtomicU64>, Arc<AtomicU64>) {
        (self.project_version.clone(), self.content_version.clone())
    }

    pub(crate) fn versions(&self) -> (u64, u64) {
        (self.project_version.load(Ordering::Acquire), self.content_version.load(Ordering::Acquire))
    }

    pub(crate) fn quality(&self) -> Quality {
        *self.preview_quality.lock().unwrap()
    }

    pub(crate) fn shown_gen(&self) -> u64 {
        self.shown_gen.load(Ordering::Relaxed)
    }

    pub(crate) fn note_cost(&self, cost: VideoCost) {
        *self.video_cost.lock().unwrap() = cost;
    }

    /// `via`: the executor of this composite, instead of the display's; `kept_on`: the display
    /// target a kept composite may be shown on instead (the slow-chain route's bytes otherwise).
    #[allow(clippy::type_complexity)]
    pub(crate) fn compose_via(
        &self,
        via: Option<&Arc<dyn VideoGpuExec>>,
        kept_on: Option<PresentOptions>,
        t: Tick,
        target: Target<'_>,
    ) -> Option<(Quality, bool, bool, kt_fx::video::FrameCost, u64, crate::video_plan::GpuStages)> {
        use kt_time::trace::{self, Ev};
        kt_time::work::bump(kt_time::work::Work::Composite);
        let t0 = std::time::Instant::now();
        // the frame's id: the compose thread's ask set it; a direct call (a bench, a test, the bytes
        // path) is a frame of its own
        let mut frame = trace::cause();
        let _own = (frame == 0 && trace::on()).then(|| {
            frame = trace::next_id();
            let flags = trace::ask::PLAYING * self.transport.is_playing() as u32;
            trace::ev(Ev::FrameAsk).id(frame).a(t.0 as u64).aux(flags).emit();
            trace::with_cause(frame)
        });
        let q = *self.preview_quality.lock().unwrap();
        // the renderer of the state asked for: this thread waits for its build, never the UI
        // (edit-without-rebuild); the pointer, not the lock: a plan with a stream stage takes as long
        // as its kernel, and an edit on the UI thread must not wait for it. A wait for the build is
        // the frame's wait in the trace
        let waited = std::time::Instant::now();
        let video = self.builds.wait_current().video.clone();
        let waited = waited.elapsed().as_nanos() as u64;
        if waited >= 1_000_000 {
            trace::ev(Ev::FrameWait).id(frame).aux(trace::wait::BUILD).lasted(waited).emit();
        }
        // playing, a stream stage short of its frame shows its latest one; paused, the frame itself
        // is waited for (decode-request-storm)
        video.set_stream_hold(self.transport.is_playing());
        let aligned = video.grid.align_floor(t);
        let mut target = target;
        // A KEPT MOMENT IS SHOWN BEFORE ANYTHING IS ASKED (structured-telemetry, kkt-26.11.09 I8: "если
        // хоть сто раз пройтись по небольшому куску - он все равно что-то loading делает"): the key of
        // the moment's composite comes from a plan that runs no stream stage, so a covered frame is
        // found without its history, its decodes or the look-ahead's leftovers — the plan of a
        // kept frame used to wait for a history the memo had dropped, and showed "Loading".
        if let Some(r) = self.show_kept(&video, frame, aligned, q, kept_on.or(match &target {
            Target::Display(o) => Some(*o),
            Target::Bytes(_) => None,
        })) {
            return Some(r);
        }
        let generation = self.transport.generation();
        let step = video.grid.frame_step();
        // the decode quality comes FROM THE REQUEST (R.46, item 318): a clip with a zooming
        // chain is warmed larger than Quality, and exactly that must be asked for
        let layers = video.layers_at(aligned, q);
        // Playback is THE FRAME ON SCREEN: a request for a frame the picture has left is
        // withdrawn (video-prefetch-thrash). It outranks the look-ahead, so when decoding
        // lagged, every frame shown queued one more, and the decoder spent itself on frames
        // nobody would read while the window ahead starved.
        for (media, w) in self.workers.read().unwrap().iter() {
            w.withdraw_video(Priority::Playback, |pts, wq| {
                layers
                    .iter()
                    .any(|(m, p, dq)| m == media && p.0 == pts && wq == Some((dq.width, dq.height)))
            });
        }
        let n_layers = layers.len();
        let mut missing = 0u64;
        for (media, src_pts, dq) in layers {
            let idx = video_cache_idx(src_pts);
            // contains, not get: the probe must not refresh the LRU (item 19)
            if !self.cache.contains_video(source_id(media), Some(dq), idx) {
                missing += 1;
                if trace::on() {
                    let kind = kt_cache::BurstKind::video(Some(dq));
                    trace::ev(Ev::FrameWait)
                        .id(frame)
                        .aux(trace::wait::DECODE)
                        .a(kt_cache::trace_id(source_id(media), kind, idx))
                        .b(media.0)
                        .c(idx as u64)
                        .emit();
                }
                if let Some(w) = self.workers.read().unwrap().get(&media) {
                    w.submit(DecodeRequest {
                        kind: RequestKind::video_on_grid(src_pts, Some(dq), step),
                        prio: Priority::Playback,
                        generation,
                    });
                }
            }
        }
        // TooLarge — no picture for this plan; DeviceLost/Validation — “poison” until the end of
        // the session
        let exec = {
            let g = locked(&self.video_gpu, frame, "video executor");
            if g.poisoned.is_none() { via.cloned().or_else(|| g.exec.clone()) } else { None }
        }?;
        let plan_span = trace::span(Ev::FramePlan);
        let plan = video.frame_plan(&self.cache, aligned, q);
        let plan_ns = t0.elapsed().as_nanos() as u64;
        let route = match (&target, via) {
            (_, Some(_)) => trace::route::SLOW,
            (Target::Display(_), None) => trace::route::DISPLAY,
            (Target::Bytes(_), None) => trace::route::BYTES,
        };
        let key = if trace::on() { exec.present_key(&plan) } else { None };
        if trace::on() {
            trace_layers(frame, &plan);
        }
        {
            let mut s = plan_span;
            s.id(frame).a(key.unwrap_or(0)).b(missing).c(route).aux(n_layers as u32);
        }
        let mut exec_span = trace::span(Ev::FrameExec);
        let done = match &mut target {
            Target::Bytes(out) => exec.execute_into(&plan, out),
            Target::Display(o) => exec.present(&plan, *o),
        };
        match done {
            Ok(complete) => {
                let stages = exec.last_stages();
                if stages.replayed {
                    kt_time::work::bump(kt_time::work::Work::CompositeReplay);
                }
                let held = video.stream_held();
                {
                    use trace::exec as x;
                    let flags = if complete { x::COMPLETE } else { 0 }
                        | if stages.replayed { x::MEMO_HIT } else { 0 }
                        | if stages.stored { x::MEMO_STORED } else { 0 }
                        | if held { x::STREAM_HELD } else { 0 }
                        | if via.is_some() { x::SLOW } else { 0 };
                    exec_span.id(frame).a(key.unwrap_or(0)).b(stages.cold as u64).aux(flags);
                }
                drop(exec_span);
                // the slow-chain device's picture is kept on the display's executor too: the next
                // lap shows it from there (structured-telemetry)
                let kept_by = match (&target, via) {
                    (Target::Display(_), _) => Some(exec.clone()),
                    (Target::Bytes(out), Some(_)) if complete && !held => {
                        let display = self.display_exec();
                        if let (Some(d), Some(key)) = (&display, exec.present_key(&plan)) {
                            d.keep(key, plan.out_w, plan.out_h, out);
                        }
                        display
                    }
                    _ => None,
                };
                // what the executor keeps for this moment, for the bars (cached-range-plays)
                if complete
                    && !held
                    && let Some(e) = kept_by
                    && let Some(key) = e.present_key(&plan)
                {
                    self.note_kept(aligned, key);
                }
                Some((q, complete, true, exec.last_cost(), plan_ns, stages))
            }
            Err(GpuExecError::TooLarge { .. } | GpuExecError::Unsupported(_)) => None,
            // the offline device has its own fate: the display's stands
            Err(_) if via.is_some() => None,
            Err(e) => {
                self.video_gpu.lock().unwrap().poisoned = Some(format!("{e:?}"));
                None
            }
        }
    }

    /// The display's executor, when it can show a picture.
    fn display_exec(&self) -> Option<Arc<dyn VideoGpuExec>> {
        let g = self.video_gpu.lock().unwrap();
        if g.poisoned.is_some() { None } else { g.exec.clone().filter(|e| e.can_present()) }
    }

    /// The moment `t` is kept under `key` (cached-range-plays): the bars show it.
    fn note_kept(&self, t: Tick, key: u64) {
        let versions = (self.project_version.load(Ordering::Acquire), self.content_version.load(Ordering::Acquire));
        let entry = (key, versions.0, versions.1);
        if self.shown.lock().unwrap().insert(t.0, entry) != Some(entry) {
            self.shown_gen.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Show the moment's kept composite if the display's executor holds it, by a probe plan that
    /// runs no stream stage (structured-telemetry).
    #[allow(clippy::type_complexity)]
    fn show_kept(
        &self,
        video: &VideoRenderer,
        frame: u64,
        t: Tick,
        q: Quality,
        on: Option<PresentOptions>,
    ) -> Option<(Quality, bool, bool, kt_fx::video::FrameCost, u64, crate::video_plan::GpuStages)> {
        use kt_time::trace::{self, Ev};
        let o = on?;
        let exec = self.display_exec()?;
        let t0 = std::time::Instant::now();
        let mut plan_span = trace::span(Ev::FramePlan);
        let key = {
            let probe = video.probe_plan(&self.cache, t, q);
            exec.present_key(&probe)?
        };
        let plan_ns = t0.elapsed().as_nanos() as u64;
        plan_span.id(frame).a(key).c(trace::route::DISPLAY);
        drop(plan_span);
        if !exec.holds(key) {
            return None;
        }
        let mut exec_span = trace::span(Ev::FrameExec);
        if !exec.present_kept(key, o) {
            exec_span.cancel();
            return None;
        }
        exec_span.id(frame).a(key).aux(trace::exec::COMPLETE | trace::exec::MEMO_HIT);
        drop(exec_span);
        kt_time::work::bump(kt_time::work::Work::CompositeReplay);
        self.note_kept(t, key);
        Some((q, true, true, Default::default(), plan_ns, exec.last_stages()))
    }

    /// Forget the kept moments the display's executor no longer holds, and check at most `budget`
    /// moments presented before an edit against the current plan (cached-range-plays). Runs on the
    /// preview's own thread: the executor's memo and a plan with a stream stage take as long as
    /// they take. `(changed, pending)`.
    pub(crate) fn recheck_shown(&self, budget: usize, stop: impl Fn() -> bool) -> (bool, bool) {
        let exec = {
            let g = self.video_gpu.lock().unwrap();
            if g.poisoned.is_none() { g.exec.clone() } else { None }
        };
        let Some(exec) = exec else { return (false, false) };
        let now = self.versions();
        // the executor is asked with the map free: its memo stands behind a whole composite, and
        // the bars read the map on the UI thread
        let keys: Vec<(i64, u64)> = self.shown.lock().unwrap().iter().map(|(t, (k, _, _))| (*t, *k)).collect();
        let gone: Vec<(i64, u64)> = keys.into_iter().filter(|(_, k)| !exec.holds(*k)).collect();
        let (stale, mut changed) = {
            let mut shown = self.shown.lock().unwrap();
            let before = shown.len();
            for (t, k) in gone {
                if shown.get(&t).is_some_and(|e| e.0 == k) {
                    shown.remove(&t);
                }
            }
            let stale: Vec<i64> =
                shown.iter().filter(|(_, (_, p, c))| (*p, *c) != now).map(|(t, _)| *t).collect();
            (stale, shown.len() != before)
        };
        if changed {
            kt_time::trace::ev(kt_time::trace::Ev::Invalidate).aux(kt_time::trace::inval::SHOWN).a(now.0).emit();
        }
        if stale.is_empty() {
            if changed {
                self.shown_gen.fetch_add(1, Ordering::Relaxed);
            }
            return (changed, false);
        }
        let q = self.quality();
        // the renderer of the state the moments are rechecked for: this runs on the compose
        // thread, which waits for that build (edit-without-rebuild)
        let video = self.builds.wait_current().video.clone();
        let mut done = 0;
        for &t in stale.iter().take(budget) {
            if stop() {
                break;
            }
            // the key-only plan: a stream stage names its frame's key and runs nothing
            // (structured-telemetry), so the check costs no history
            let plan = video.probe_plan(&self.cache, Tick(t), q);
            let key = exec.present_key(&plan);
            drop(plan);
            done += 1;
            let mut shown = self.shown.lock().unwrap();
            match (shown.get_mut(&t), key) {
                // the versions the plan was built at: an edit since then leaves it stale
                (Some(e), Some(k)) if e.0 == k && self.versions() == now => (e.1, e.2) = now,
                (Some(e), Some(k)) if e.0 == k => {}
                _ => {
                    shown.remove(&t);
                }
            }
            changed = true;
        }
        if changed {
            self.shown_gen.fetch_add(1, Ordering::Relaxed);
        }
        (changed, done < stale.len())
    }
}

impl Player {
    /// The moments that play without work (cached-range-plays): the executor holds the composite
    /// presented there, and it is still the composite the current plan asks for. A moment
    /// presented before an edit is checked again against the new plan on the preview's own thread
    /// (`PreviewParts::recheck_shown`): a plan can run a stream stage's kernel, and this is asked
    /// by the bars on the UI thread (ui-never-waits). `pending` says some are left; the check is
    /// then asked for, and the preview's thread wakes the UI when it has moved. Runs of ticks,
    /// each moment `[t, t + frame)`.
    pub fn final_video_runs(&self) -> (Vec<std::ops::Range<i64>>, bool) {
        let now = self.preview_parts().versions();
        let step = self.video_grid().frame_step();
        let shown = self.shown.lock().unwrap();
        let mut runs: Vec<std::ops::Range<i64>> = Vec::new();
        let mut pending = false;
        for (t, (_, p, c)) in shown.iter() {
            if (*p, *c) != now {
                pending = true;
                continue;
            }
            match runs.last_mut() {
                Some(r) if r.end >= *t => r.end = r.end.max(t + step),
                _ => runs.push(*t..t + step),
            }
        }
        drop(shown);
        if pending {
            self.composer.nudge();
        }
        (runs, pending)
    }

    /// Changes when what plays without work may have changed (a new kept moment, a published
    /// final mix): a part of the bars' stamp.
    pub fn final_stamp(&self) -> u64 {
        let mix = Arc::as_ptr(&self.mix_coverage.get()) as u64;
        self.shown_gen.load(Ordering::Relaxed).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ mix
    }

    /// The timeline ticks whose final mix the audio render thread holds and would hand out now
    /// (cached-range-plays), as it last published them.
    pub fn final_audio_runs(&self) -> Vec<std::ops::Range<i64>> {
        let region = *self.loop_region.lock().unwrap();
        self.mix_coverage
            .get()
            .playing(region)
            .into_iter()
            .map(|r| Tick::from_audio_frames(r.start).0..Tick::from_audio_frames(r.end).0)
            .collect()
    }
}

/// Where a preview composite goes.
pub(crate) enum Target<'a> {
    Bytes(&'a mut Vec<u8>),
    Display(PresentOptions),
}

/// The cost of one preview compose (morpher-event-grid-cost, D1).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VideoCost {
    /// The GPU executor composed it (false: no picture was composed).
    pub gpu: bool,
    pub cost: kt_fx::video::FrameCost,
    pub compose_ns: u64,
    /// The part of `compose_ns` before the executor: the order, the requests, `frame_plan`
    /// (preview-throughput).
    pub plan_ns: u64,
    pub stages: crate::video_plan::GpuStages,
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.builds.shutdown();
        self.wake.notify();
        // a build is tens of ms on a large project, and it ends at the stop
        if let Some(t) = self.build_thread.take() {
            let _ = t.join();
        }
        // not joined: a composite may take seconds, and this runs on the UI thread. It ends by
        // itself on what it holds.
        self.composer.stop();
        drop(self.compose_thread.take());
        if let Some(t) = self.render_thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self.prefetch_thread.take() {
            let _ = t.join();
        }
        // The render/prefetch threads are finished; the preview's thread may still hold the map
        let workers: Vec<_> = self.workers.write().unwrap().drain().collect();
        for (_, w) in workers {
            w.shutdown();
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
    builds: Arc<crate::builds::Builds>,
    dirty: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    meters: Arc<Meters>,
    master_gain: Arc<AtomicU32>,
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
    flight: Arc<crate::flight::FlightRecorder>,
    mix_coverage: crate::mix::MixCoverage,
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

/// The moments the preview presented: tick → (the executor's key, project and content versions).
type Shown = Arc<Mutex<std::collections::BTreeMap<i64, (u64, u64, u64)>>>;

/// The UI's own FX states over the shared audio renderer (`Player::with_probe`), with the project
/// version they answer for. Synced by the builder thread (`sync_probe`).
#[derive(Default)]
struct Probe {
    slot: Mutex<Option<(u64, Arc<AudioRenderer>, FxStateStore)>>,
}

/// The probe's FX states carried over to a new build: only the events whose fingerprint changed
/// are synced (invariant 3, as on the render thread). Out of the slot while it runs, so a meter
/// reading on the UI thread never waits for it (it reads silence for that moment).
fn sync_probe(probe: &Probe, audio: &Arc<AudioRenderer>, version: u64) {
    let old = probe.slot.lock().unwrap().take();
    let (old_r, mut store) = match old {
        Some((_, r, s)) => (Some(r), s),
        None => (None, FxStateStore::new()),
    };
    audio.sync_fx_since(old_r.as_deref(), &mut store);
    *probe.slot.lock().unwrap() = Some((version, audio.clone(), store));
}

/// Pin the first half second of the loop region in the cache (test 14).
fn pin_loop_head(cache: &BurstCache, renderer: &AudioRenderer, s: i64) {
    for (media, bursts) in renderer.wanted_audio_bursts(s..s + kt_time::AUDIO_SAMPLE_RATE as i64 / 2) {
        cache.pin_range(source_id(media), kt_cache::BurstKind::Audio, bursts);
    }
}

/// A hold the builder thread adds to every build, in milliseconds: a test's seam for "a rebuild
/// outlasts the ring" (t1935). The scene used to get it from a quadratic build; since
/// builder-cost-guard a build is a few ms, and a heavy enough honest scene would take gigabytes
/// and still depend on the machine's speed. Zero in the product.
static BUILD_HOLD_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Hold every later engine build of this process by `ms` (tests only).
#[doc(hidden)]
pub fn hold_builds_for_tests(ms: u64) {
    BUILD_HOLD_MS.store(ms, std::sync::atomic::Ordering::Relaxed);
}

/// How long the render thread stands after each burst of a START's pass (tests only, t2216): the
/// render thread put off between two bursts on a loaded machine, made to happen on any machine.
/// Zero in the product.
static START_BURST_HOLD_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Hold the render thread by `ms` after each burst of a start's pass (tests only).
#[doc(hidden)]
pub fn hold_start_bursts_for_tests(ms: u64) {
    START_BURST_HOLD_MS.store(ms, std::sync::atomic::Ordering::Relaxed);
}

/// The sources the player raised and what it knows of them, shared by the UI thread (a file
/// added, replaced, retuned) and the builder thread (the generator sources of a version).
/// See [`Player::sources`].
#[derive(Clone)]
pub struct Sources {
    media: Arc<MediaParts>,
    builds: Arc<crate::builds::Builds>,
}

impl Sources {
    pub fn add_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        self.media.add_media(media, source);
        self.builds.refresh();
    }

    pub fn replace_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        self.media.replace_media(media, source);
        self.builds.refresh();
    }

    pub fn remove_media(&self, media: kt_model::MediaId) {
        self.media.remove_media(media);
    }
}

pub(crate) struct MediaParts {
    workers: Workers,
    natives: Natives,
    cache: Arc<BurstCache>,
    decode_stats: Arc<DecodeStats>,
    worker_spawns: Arc<AtomicU64>,
    content_version: Arc<AtomicU64>,
    wake: Arc<Wake>,
    /// Values of the generators that are NOW set in the live sources
    /// (R.59, stage 2). They cannot be asked of the sources themselves —
    /// they are exclusively owned by the worker thread (invariant 5) — and the diff of
    /// “what changed” is needed here.
    gen_states: Mutex<HashMap<MediaId, kt_model::GenState>>,
    /// Held by whoever brings the generator sources into line with a project (the builder, a
    /// value edit): the builder reads the newest project under it, so it never retunes a source
    /// back to values a value edit has just replaced.
    gen_lock: Mutex<()>,
}

impl MediaParts {
    fn add_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        self.worker_spawns.fetch_add(1, Ordering::AcqRel);
        self.natives.write().unwrap().observe(media, source.info());
        let w = MediaWorker::spawn_with_stats(media, source, self.cache.clone(), self.decode_stats.clone());
        // the guard ends with the statement: a join under it held the render, prefetch and
        // preview threads for the old decoder's last span (ui-never-waits)
        let old = self.workers.write().unwrap().insert(media, w);
        if let Some(old) = old {
            old.shutdown();
        }
    }

    /// The order matters: first stop the worker, then evict its frames from the cache, and only
    /// then raise the new one — otherwise the old one would manage to write a placeholder frame
    /// into the cache after the invalidation. The transport, position and cache of the other
    /// sources are not touched.
    fn replace_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        self.worker_spawns.fetch_add(1, Ordering::AcqRel);
        let old = self.workers.write().unwrap().remove(&media);
        if let Some(old) = old {
            old.shutdown();
        }
        // the native size of the new file may differ from the placeholder — the decode scale step
        // must be recomputed at the next build (R.46)
        self.natives.write().unwrap().observe(media, source.info());
        self.cache.invalidate_source(kt_cache::SourceId(media.0));
        let w = MediaWorker::spawn_with_stats(media, source, self.cache.clone(), self.decode_stats.clone());
        self.workers.write().unwrap().insert(media, w);
        // the set of sources has changed — prefetch must wake up and
        // warm the cache anew (at idle it skips passes while
        // the state signature is the same, and without a version bump it would not notice).
        // A CONTENT version, not a project one: the plan is indifferent to the source.
        self.content_version.fetch_add(1, Ordering::AcqRel);
        self.wake.notify();
    }

    fn remove_media(&self, media: kt_model::MediaId) {
        let old = self.workers.write().unwrap().remove(&media);
        if let Some(w) = old {
            w.shutdown();
        }
        self.natives.write().unwrap().forget(media);
    }

    /// Bring the GENERATOR SOURCES into line with the project (R.59, stage 2); the caller holds
    /// `gen_lock`.
    ///
    /// A diff by ADDRESS: a generator that appeared gets a stream, one that disappeared
    /// loses it; for the remaining ones the values arrive by retuning — the stream
    /// lives, the cache of foreign sources is intact, the renderers are not rebuilt.
    /// Recreation remains for exactly one case: the source's passport
    /// changed (SMPTE with the tone switched on becomes a SOUNDING medium, and
    /// that is already a different set of clips), and then `can_retune_gen` honestly
    /// says “no”.
    fn sync_generators(&self, wanted: Vec<(MediaId, kt_model::GenState)>) {
        let want_ids: std::collections::HashSet<MediaId> = wanted.iter().map(|(id, _)| *id).collect();
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

    fn update_gen_at(&self, addr: MediaId, state: kt_model::GenState, stale: Vec<kt_cache::SourceId>) {
        self.gen_states.lock().unwrap().insert(addr, state.clone());
        let workers = self.workers.read().unwrap();
        let Some(w) = workers.get(&addr) else { return };
        w.retune_gen(state, stale);
        drop(workers);
        self.content_version.fetch_add(1, Ordering::AcqRel);
        self.wake.notify();
    }
}

/// Build one version for every consumer (edit-without-rebuild): its generator sources first (what
/// they raise is a fact of the video build), then the audio and the video renderer, in one
/// revisions scope (each reel resolved once) with the last build's events reused. `builds`: the
/// player's wants, when it exists — the generator sync reads the newest project.
fn build_version(
    media: &MediaParts,
    cache: &Arc<BurstCache>,
    project: &Arc<Project>,
    version: u64,
    seq: u64,
    memo: &mut crate::incremental::BuildMemo,
    builds: Option<&crate::builds::Builds>,
) -> crate::builds::Built {
    // an audio and a video renderer
    kt_time::work::add(kt_time::work::Work::EngineBuild, 2);
    // the build in the trace (structured-telemetry): its version, the events it derived and the
    // events its memo holds after it (the rest of them were reused from the last build)
    let mut span = kt_time::trace::span(kt_time::trace::Ev::Job);
    span.aux(kt_time::trace::job::SHARED_BUILD).id(seq).a(version);
    let (mut built, cost) = kt_time::cost::meter(|| crate::incremental::with_memo(memo, || {
        crate::video::revisions_scope(|| {
            let wanted = crate::gen_sources(project);
            {
                let _g = media.gen_lock.lock().unwrap();
                // a value edit replaced the stored project meanwhile: its values are the truth
                let newest = builds.map(|b| b.want.lock().unwrap().project.clone());
                match newest {
                    Some(n) if !Arc::ptr_eq(&n, project) => media.sync_generators(crate::gen_sources(&n)),
                    _ => media.sync_generators(wanted),
                }
            }
            // the facts cloned out: a UI writer (a source added or replaced) must not wait for
            // a whole-project build behind a read guard
            let facts = media.natives.read().unwrap().clone();
            let audio = Arc::new(AudioRenderer::new(project));
            let video = Arc::new(preview_video(project, &facts, cache));
            crate::builds::Built { seq, version, project: project.clone(), audio, video, cost: Default::default() }
        })
    }));
    built.cost = cost;
    let (held_a, held_v) = memo.len();
    span.b(cost[kt_time::cost::Cost::EventDerive as usize]).c((held_a + held_v) as u64);
    built
}

struct BuildCtx {
    builds: Arc<crate::builds::Builds>,
    media: Arc<MediaParts>,
    cache: Arc<BurstCache>,
    probe: Arc<Probe>,
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
    wake: Arc<Wake>,
    composer: Arc<crate::compose_queue::Composer>,
    memo: crate::incremental::BuildMemo,
}

/// The `engine-build` thread: the newest want, built once and published; the probe's states
/// synced and the loop's head pinned with the new renderer; the render thread and the UI woken.
fn build_loop(ctx: BuildCtx) {
    let BuildCtx { builds, media, cache, probe, loop_region, wake, composer, mut memo } = ctx;
    let mut done = builds.built_seq();
    while builds.next_want(done).is_some() {
        builds.wait_gate();
        if builds.stop.load(Ordering::Acquire) {
            return;
        }
        // the newest want after the gate: the ones in between are never built
        let (project, version, seq) = {
            let w = builds.want.lock().unwrap();
            (w.project.clone(), w.version, w.seq)
        };
        let built = build_version(&media, &cache, &project, version, seq, &mut memo, Some(&builds));
        drop(project);
        let hold = BUILD_HOLD_MS.load(std::sync::atomic::Ordering::Relaxed);
        if hold > 0 {
            std::thread::sleep(std::time::Duration::from_millis(hold));
        }
        let audio = built.audio.clone();
        builds.publish(built);
        sync_probe(&probe, &audio, version);
        let region = *loop_region.lock().unwrap();
        if let Some((s, _)) = region {
            cache.unpin_all();
            pin_loop_head(&cache, &audio, s);
        }
        drop(audio);
        done = seq;
        // a parked render thread (a pause) takes it now, not at its next timeout
        wake.notify();
        composer.wake_ui();
    }
}

fn audio_render_loop(ctx: RenderCtx) {
    // this thread must not wait: a blocking call here is counted (one-scheduler)
    kt_time::block::mark(kt_time::block::role::RENDER);
    let RenderCtx {
        mut producer,
        cache,
        transport,
        wake,
        loop_stats,
        shared,
        workers,
        builds,
        dirty,
        stop,
        underruns,
        meters,
        master_gain,
        loop_region,
        flight,
        natives,
        mix_coverage,
    } = ctx;
    let mut store = FxStateStore::new();
    // the opening's build IS the first model: the flag the player starts with asks for nothing
    dirty.store(false, Ordering::Release);
    let first = builds.latest();
    let mut renderer: Arc<AudioRenderer> = first.audio.clone();
    // the want the renderer answers, and the newest one an edit on a pause asked
    let (mut have, mut wanted) = (first.seq, first.seq);
    drop(first);
    renderer.sync_fx(&mut store);
    // the final mix of what was heard (cached-range-plays): a covered range plays from it
    let mut mix = crate::mix::MixPlay::default();
    let mut mix_published = std::time::Instant::now();
    // A build asked on a pause: the next start waits for it (time stands before the first frame,
    // invariant 6), so Play after an edit plays the edit. The render thread itself never builds:
    // the player's `engine-build` thread builds each version once (edit-without-rebuild), and the
    // old renderer plays until the new one is published — on a large edit a build costs longer
    // than the ring holds (412 ms against 341 ms on the owner's project before builder-cost-guard).
    let mut build_holds_start = false;
    let mut traced_hold = 0u64;
    let mut fetcher = CacheFetcher::with_lengths(cache.clone(), &natives.read().unwrap().lengths);
    let mut fetcher_facts = natives.read().unwrap().lengths.len();
    // the local render position (frames); may lead the playhead by the ring
    let mut render_head: i64 = transport.playhead_frames();
    let mut last_generation = transport.generation();
    // The ring has not been filled since a play position was set: nothing has sounded, so the
    // clock stands (before the first frame, invariant 6) and waiting for the audio costs no sync.
    // Filling at once rendered a whole ring of silence ahead of the decoders.
    let mut starting = true;
    let mut start_wait: Option<std::time::Instant> = None;
    let mut last_dir: i64 = 1;
    // The “armedness” of the loop wrap — transport semantics (the wrap only
    // from inside the region, item 132): a seek past the end (for reverse — past the start)
    // disarms until the next jump, and render/prefetch/transport see
    // effective_region = None (linear playback of what is under the cursor).
    let mut last_region: Option<(i64, i64)> = *loop_region.lock().unwrap();
    let mut loop_armed = loop_arm_after_jump(render_head, last_region, 1);
    let mut last_effective_some = loop_armed && last_region.is_some();
    let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
    // The first pass after a start, held back from the callback until it is whole (test-hygiene,
    // found-defects 158). Allocated once, here: the loop allocates nothing.
    let mut start_pass: Vec<f32> = Vec::with_capacity(RING_CAPACITY_FRAMES * AUDIO_CHANNELS);
    // “black box” counters (item 10): deltas per second, not absolutes
    let (mut last_ru, mut last_cu, mut last_warm) = (0u64, 0u64, 0u64);
    let mut warmups = 0u64;
    let mut last_snap_ms = 0u64;
    // timed passes left after a pause or a parked wake (`PAUSE_TAIL_PASSES`)
    let mut tail_passes = 0u32;
    let mut seen = wake.seen();
    let mut stall = crate::flight::StallWatch::new(render_head);

    while !stop.load(Ordering::Acquire) {
        if dirty.swap(false, Ordering::AcqRel) {
            build_holds_start |= !transport.is_playing();
            wanted = builds.want_seq();
        }
        if builds.built_seq() > have {
            let b = builds.latest();
            let swap = std::time::Instant::now();
            let old = std::mem::replace(&mut renderer, b.audio.clone());
            have = b.seq;
            drop(b);
            // only what changed: the state survives a rebuild (inv. 3), and an event the
            // edit did not touch costs this thread nothing
            renderer.sync_fx_since(Some(&old), &mut store);
            mix.revalidate(&renderer);
            // dropping a whole renderer is not this thread's work either; the others may still
            // hold it, and then the last of them frees it
            if Arc::strong_count(&old) == 1 {
                let _ = std::thread::Builder::new().name("audio-drop".into()).spawn(move || drop(old));
            }
            loop_stats.max_swap_us.fetch_max(swap.elapsed().as_micros() as u64, Ordering::Relaxed);
        }
        if have >= wanted {
            build_holds_start = false;
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
                // a source came or went: what was rendered without it is not its sound
                mix.store.clear();
            }
        }
        let generation = transport.generation();
        let generation_changed = generation != last_generation;
        if generation_changed {
            starting = true;
            start_wait = None;
            last_generation = generation;
            store.reset_all(); // pre-roll at the next read
            render_head = transport.playhead_frames();
            mix.fx_reset(&renderer, render_head);
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
            starting = true;
            start_wait = None;
            store.reset_all();
            render_head = transport.playhead_frames();
            mix.fx_reset(&renderer, render_head);
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
                // the laps of another region will not be heard again
                mix.store.retain_laps(region);
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
            mix.fx_reset(&renderer, render_head);
            shared.consumed_frames.store(0, Ordering::Release);
            shared.flush.store(true, Ordering::Release);
            starting = true;
            start_wait = None;
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
            let rate = f32::from_bits(shared.playback_rate.load(Ordering::Acquire));
            if let Some(idle) = stall.tick(now, transport.is_playing(), rate, head) {
                flight.push(crate::flight::Ev::Stall, idle as i64, head);
            }
        }

        if transport.is_playing() {
            let gain = f32::from_bits(master_gain.load(Ordering::Acquire));
            // what the final-mix store holds needs no source (cached-range-plays): neither the
            // look-ahead nor a start orders it
            let held = |from: i64, n: i64| {
                dir > 0
                    && crate::mix::pieces(from, n, effective_region).iter().all(|&(ctx, tl, m)| {
                        mix.store.holds(ctx, tl, m, &|k| {
                            let u = AUDIO_BURST_FRAMES as i64;
                            renderer.scene_hash(k * u..(k + 1) * u)
                        })
                    })
            };
            let look_now =
                (PLAYBACK_AUDIO_LOOKAHEAD_FRAMES as f32 * rate_now.abs().max(1.0)) as i64;
            // request the nearest audio with Playback priority (idempotent);
            // in a loop we request via the wrap mapping
            match effective_region {
                _ if held(render_head, look_now) => {}
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
            // a start waits until the bursts of a whole ring are there: the fill below is one
            // pass over all of it, and what is missing now would be silence, not a late burst.
            // Plus the look-ahead margin: the first refill reads just past the ring.
            let audio_missing = starting && !held(render_head, RING_CAPACITY_FRAMES as i64) && {
                let ring = RING_CAPACITY_FRAMES as i64
                    + (PLAYBACK_AUDIO_LOOKAHEAD_FRAMES as f32 * rate_now.abs().max(1.0)) as i64;
                let wins: Vec<std::ops::Range<i64>> = if dir < 0 {
                    std::iter::once((render_head - ring).max(0)..render_head).collect()
                } else {
                    lookahead_windows(render_head, ring, effective_region)
                };
                let workers = workers.read().unwrap();
                wins.into_iter().fold(0, |n, win| {
                    n + prefetch_audio_pass(
                        &renderer,
                        &cache,
                        &workers,
                        win,
                        Priority::Playback,
                        generation,
                        dir < 0,
                    )
                }) > 0
            };
            let hold = build_holds_start
                || (audio_missing
                    && start_wait.get_or_insert_with(std::time::Instant::now).elapsed()
                        < START_AUDIO_GRACE);
            // THE START'S PASS IS HANDED OVER WHOLE (test-hygiene, found-defects 158). The callback
            // primes on the first frames in the ring and from then on time runs; the pass pushed
            // burst by burst, and a render thread put off between two bursts on a loaded machine
            // (a pass of the 1974-event edit rendered live is 83 ms in debug) let the callback eat
            // the first bursts and underrun before the ring had ever been full. Time stands before
            // the first frame, so holding the pass costs no sync.
            let whole_pass = starting && !hold;
            if !hold {
                starting = false;
                start_wait = None;
            }
            // a start held for its sound or its renderer, in the trace once per change
            let why_hold = if !hold { 0 } else if build_holds_start { 1 } else { 2 };
            if why_hold != traced_hold {
                traced_hold = why_hold;
                if why_hold != 0 {
                    kt_time::trace::ev(kt_time::trace::Ev::Play)
                        .aux(kt_time::trace::play::START_HOLD)
                        .a(kt_time::Tick::from_audio_frames(render_head).0 as u64)
                        .b(why_hold)
                        .emit();
                }
            }
            let mut fill = kt_time::trace::span(kt_time::trace::Ev::AudioFill);
            let (mut live_frames, mut kept_frames) = (0u64, 0u64);
            // we fill the ring: render_head is linear, loop mapping is inside
            // render_looped_with (the wrap does NOT bump the generation — FX live)
            let mut room = producer.slots();
            let mut pass_bursts = 0;
            while !hold && room >= chunk.len() {
                let stats = if dir < 0 {
                    kt_time::work::bump(kt_time::work::Work::AudioRender);
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
                    live_frames += AUDIO_BURST_FRAMES as u64;
                    st
                } else {
                    // through the final-mix store: a held chunk is handed out with no decode and
                    // no effect run; the rest renders live and is kept (cached-range-plays)
                    let out = mix.render(
                        &renderer,
                        &mut fetcher,
                        &mut store,
                        effective_region,
                        render_head,
                        &mut chunk,
                    );
                    kt_time::work::bump(if out.replayed {
                        kt_time::work::Work::MixReplay
                    } else {
                        kt_time::work::Work::AudioRender
                    });
                    if out.replayed {
                        kept_frames += AUDIO_BURST_FRAMES as u64;
                    } else {
                        live_frames += AUDIO_BURST_FRAMES as u64;
                    }
                    render_head += AUDIO_BURST_FRAMES as i64;
                    out.stats
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
                // the seam of t2216, a ring's worth of bursts: zero in the product
                pass_bursts += 1;
                let held_ms = if whole_pass && pass_bursts <= RING_CAPACITY_FRAMES / AUDIO_BURST_FRAMES {
                    START_BURST_HOLD_MS.load(Ordering::Relaxed)
                } else {
                    0
                };
                if held_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(held_ms));
                }
                if whole_pass {
                    start_pass.extend_from_slice(&chunk);
                    room -= chunk.len();
                } else {
                    for &s in chunk.iter() {
                        let _ = producer.push(s);
                    }
                    room = producer.slots();
                }
            }
            if !start_pass.is_empty() {
                // one commit: the callback sees nothing of the pass, then all of it
                if let Ok(c) = producer.write_chunk_uninit(start_pass.len()) {
                    c.fill_from_iter(start_pass.drain(..));
                }
                start_pass.clear();
            }
            if live_frames + kept_frames == 0 {
                fill.cancel();
            } else {
                fill.a(live_frames).b(kept_frames).c(render_head as u64);
            }
        }
        // the bars see what the store holds, four times a second at most
        if mix_published.elapsed() >= std::time::Duration::from_millis(250) {
            mix_published = std::time::Instant::now();
            mix.store.publish(&mix_coverage);
        }
        // render_head is reset ONLY on seek (generation): on pause the
        // ring keeps the already rendered tail, resume continues from it.
        loop_stats.render_passes.fetch_add(1, Ordering::Relaxed);
        if transport.is_playing() {
            // 8 ms: the ring holds ~85 ms (4 bursts), the callback eats ~21 ms at a
            // time — a four-fold margin; 2 ms were 500 wake-ups/s and
            // heated the CPU on weak machines (item 1 of round 5)
            std::thread::sleep(std::time::Duration::from_millis(8));
            tail_passes = 0;
        } else if consumed > 0 || tail_passes < PAUSE_TAIL_PASSES {
            // pause: the callback that was running when the pause came may still hand out a
            // block; its frames are picked up by a short pass or two, then the thread parks
            tail_passes += 1;
            wake.wait_timeout(std::time::Duration::from_millis(100));
        } else {
            // parked until play/seek/rate/region/build: no clock at rest (one-scheduler; it woke
            // every 250 ms before)
            mix.store.publish(&mix_coverage);
            wake.wait_since(seen);
            tail_passes = 0;
        }
        seen = wake.seen();
    }
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

/// One reading of the display clock (`DisplayLap::step`), all in 48 kHz frames.
#[derive(Clone, Copy, Debug)]
pub struct DisplaySample {
    /// The transport head, as the transport holds it (folded into the region).
    pub head: i64,
    /// The interpolation past the head (`display_extrapolation`), unsigned.
    pub extra: i64,
    /// `Transport::wraps`.
    pub wraps: u64,
    /// `Transport::generation`.
    pub generation: u64,
    /// `Transport::last_seek_frames`: where the jump of a new generation landed.
    pub jump_target: i64,
    /// The smoothed output latency in WALL-CLOCK frames.
    pub latency: i64,
    /// The loop region as the player holds it.
    pub region: Option<(i64, i64)>,
    pub rate: f32,
}

/// The shown position between UI frames (loop-play-latency-hold, D1–D3).
///
/// `video_display_frame` folds `head − latency` into the region. That is right in the
/// steady state, but its only lap marker was the head's wrap, and under a latency `L`
/// the head wraps `L` BEFORE the shown frame folds: the preview released its ratchet
/// on a continuous frame and then refused the real fold as a rollback — the picture
/// froze until a click (kkt 2026-10-27 item 1; wine/WASAPI has a latency of a whole
/// device buffer). Here the position is kept UNWRAPPED since the last anchor, so the
/// fold is an event of the shown frame itself:
/// - D1: `lap` grows on the frame where the shown position crosses the region's end
///   (in reverse — its start), and on every re-anchor;
/// - D2: after a jump (a seek, a resume, a change of direction) the shown frame is the
///   jump target until the head is `L·|rate|` past it — that time is the pre-roll's
///   silence, not the region's end;
/// - D3: within an anchor the shown position only moves forward (in the direction of
///   play), so a jittering latency folds at most once per lap.
///
/// A change of the region alone re-anchors WITHOUT the hold: the transport did not
/// jump and the queued sound plays on.
/// What a display anchor was taken for: (generation, loop region, direction).
type DisplayAnchor = (u64, Option<(i64, i64)>, i64);

#[derive(Clone, Debug, Default)]
pub struct DisplayLap {
    /// The anchor of the shown position; None — paused or never played.
    anchor: Option<DisplayAnchor>,
    /// The generation the last anchor saw: a different one means a seek landed at
    /// `jump_target`, otherwise the jump is a resume or a turn at the head.
    seen_gen: Option<u64>,
    /// `wraps` at the anchor: the head is unwrapped by the laps since then.
    wraps0: u64,
    /// The unwrapped shown position.
    shown: i64,
    /// How many times `shown` has crossed the region's edge since the anchor.
    folds: i64,
    lap: u64,
}

impl DisplayLap {
    /// Playback stopped: the next playing reading is a jump (D2).
    pub fn pause(&mut self) {
        self.anchor = None;
    }

    pub fn lap(&self) -> u64 {
        self.lap
    }

    /// The frame to show and the display lap.
    pub fn step(&mut self, s: DisplaySample) -> (i64, u64) {
        let dir: i64 = if s.rate < 0.0 { -1 } else { 1 };
        let scaled = (s.latency.max(0) as f64 * s.rate.abs().max(0.0) as f64) as i64;
        let region = s.region.filter(|&(rs, re)| re > rs);
        let span = region.map_or(0, |(rs, re)| re - rs);
        let key = (s.generation, region, dir);
        if self.anchor != Some(key) {
            let jump = match self.anchor {
                None => true,
                Some((g, _, d)) => g != s.generation || d != dir,
            };
            let target = if self.seen_gen != Some(s.generation) { s.jump_target } else { s.head };
            self.anchor = Some(key);
            self.seen_gen = Some(s.generation);
            self.wraps0 = s.wraps;
            self.shown = if jump { target } else { s.head + (s.extra - scaled) * dir };
            self.folds = region.map_or(0, |r| display_folds(self.shown, r, dir));
            self.lap += 1;
        }
        let laps = s.wraps.wrapping_sub(self.wraps0) as i64;
        let raw = s.head + (s.extra + laps * span - scaled) * dir;
        self.shown = if dir > 0 { self.shown.max(raw) } else { self.shown.min(raw) };
        // the same two-sided condition as `video_display_frame` and the transport: a
        // head outside the region plays linearly (item 132)
        let frame = match region {
            Some((rs, re)) if s.head >= rs && s.head <= re => {
                let folds = display_folds(self.shown, (rs, re), dir);
                if folds > self.folds {
                    self.lap += (folds - self.folds) as u64;
                    self.folds = folds;
                }
                // not yet in the region (entered from outside) — linear, not its end
                let before = if dir > 0 { self.shown < rs } else { self.shown >= re };
                if before { self.shown } else { rs + (self.shown - rs).rem_euclid(span) }
            }
            _ => self.shown,
        };
        (frame.max(0), self.lap)
    }
}

/// How many times an unwrapped position has crossed the region's edge in the
/// direction of play (`DisplayLap`): forward at `re` (where the fold gives `rs`),
/// in reverse below `rs`.
fn display_folds(shown: i64, (rs, re): (i64, i64), dir: i64) -> i64 {
    let span = re - rs;
    if dir > 0 {
        if shown < rs { 0 } else { (shown - rs).div_euclid(span) }
    } else if shown >= re {
        0
    } else {
        (re - 1 - shown).div_euclid(span)
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

/// How far past the frame on screen the prefetch thread computes a stream stage during playback
/// (decode-request-storm): the frames the display reaches before the next pass, twice over, at
/// least `STREAM_AHEAD_MIN`. A frame the display reaches is then held in the memo, and the
/// composite — which runs a stage only `compose_steps` short — shows it. A pass on a big edit takes
/// a few hundred milliseconds (its look-ahead walk), so a fixed few frames fell behind the display
/// and the picture stood. Bounded by what the memo keeps: a position holds about two frames of the
/// stream's size (the decoded picture and the clean encoder's planes), and a quarter of the memo is
/// the most the frames ahead may take.
const STREAM_AHEAD_MIN: usize = 4;

/// Timed passes the audio render thread makes after a pause (or a wake on a pause) before it parks
/// with no clock: the callback that was running when the pause came may hand out one more block,
/// and the playhead must take it (one-scheduler).
const PAUSE_TAIL_PASSES: u32 = 2;

/// Advance the stream stages at the frame on screen and, while playing, at the frames after it
/// (wrapping in the loop region), stopping at the first that still lacks source frames. `ahead` —
/// how many frames after it. `true` — something is still pending.
fn stream_advance_ahead(
    video: &VideoRenderer,
    cache: &BurstCache,
    shown: Tick,
    q: Quality,
    region: Option<(i64, i64)>,
    ahead: usize,
    kept: &dyn Fn(Tick) -> bool,
) -> bool {
    let step = video.grid.frame_step().max(1);
    let mut t = video.grid.align_floor(shown);
    let inside = region.is_some_and(|(s, e)| s <= t.0 && t.0 < e);
    for _ in 0..=ahead {
        // a kept moment needs no history (structured-telemetry)
        if !kept(t) && video.stream_advance(cache, t, q) {
            return true;
        }
        t = Tick(t.0 + step);
        if let Some((s, e)) = region.filter(|_| inside)
            && t.0 >= e
        {
            t = video.grid.align_floor(Tick(s));
        }
    }
    false
}

/// Size the preview's stream memo by the loop it must keep (stream-layout-per-build, found-defects
/// 143): a position keeps the stage's picture — half the stage's RGBA frame at most (Datamosh's
/// Y'CbCr 4:2:0 is 3/8 of it) — every position of the loop region, within a quarter of the memory
/// available — measured only when the loop or the stage's size changes (`last`), so a pass that
/// changes neither reads nothing.
fn size_stream_memo(video: &VideoRenderer, t: Tick, q: Quality, region: Option<(i64, i64)>, last: &mut Option<(usize, Option<usize>)>) {
    let frame = video.stream_frame_bytes(t, q);
    if frame == 0 {
        return;
    }
    let step = video.grid.frame_step().max(1);
    let positions = region.map(|(s, e)| ((e - s).max(0) / step) as usize + 1);
    let now = (frame / 2, positions);
    if *last == Some(now) {
        return;
    }
    *last = Some(now);
    crate::video::size_stream_memo(now.0, positions);
}

/// Tell the display's executor the loop region's moments when they or the executor change
/// (memory-ledger D4): it keeps every composite of the loop within what the memory ledger grants.
/// On this thread, not the UI's: the executor's lock may stand behind a composite.
fn tell_loop(
    video_gpu: &Mutex<VideoGpuState>,
    builds: &crate::builds::Builds,
    loop_region: &Mutex<Option<(i64, i64)>>,
    told: &mut Option<(Option<usize>, usize)>,
) {
    let exec = {
        let g = video_gpu.lock().unwrap();
        if g.poisoned.is_some() { None } else { g.exec.clone().filter(|e| e.can_present()) }
    };
    let Some(exec) = exec else { return };
    let step = builds.latest().video.grid.frame_step().max(1);
    let moments = (*loop_region.lock().unwrap()).map(|(s, e)| {
        let (s, e) = (Tick::from_audio_frames(s).0, Tick::from_audio_frames(e).0);
        ((e - s).max(0) / step) as usize + 1
    });
    let now = (moments, Arc::as_ptr(&exec) as *const () as usize);
    if *told != Some(now) {
        *told = Some(now);
        exec.keep_loop(moments);
    }
}

/// The frames ahead for a pass interval of `interval` (see `STREAM_AHEAD_MIN`).
fn stream_ahead_frames(video: &VideoRenderer, t: Tick, q: Quality, interval: std::time::Duration) -> usize {
    let fps = kt_time::TICK_HZ as f64 / video.grid.frame_step().max(1) as f64;
    let want = (interval.as_secs_f64() * fps * 2.0).ceil() as usize;
    let per_position = 2 * video.stream_frame_bytes(t, q).max(1);
    let room = crate::video::stream_memo().bytes_cap() / 4 / per_position;
    want.max(STREAM_AHEAD_MIN).min(room.max(1))
}

/// The live preview's renderer: its stream stages paced to the player's cache (decode-request-storm).
fn preview_video(p: &Project, facts: &crate::SourceFacts, cache: &Arc<BurstCache>) -> VideoRenderer {
    let mut vr = VideoRenderer::new_with_facts(p, facts);
    // the GPU rings bound what stands in front of the display by this project's frame (gpu-arbiter D3)
    kt_time::sched::set_frame_period_ns(vr.grid.frame_step().max(1) as u64 * 1_000_000_000 / kt_time::TICK_HZ as u64);
    vr.set_stream_pacing(Some(crate::StreamPacing::preview(cache.max_video_bytes())));
    // the cap the memory ledger moves after this build (frames-cache-growth)
    vr.set_stream_cap(cache.video_cap_reader());
    vr
}

#[allow(clippy::too_many_arguments)]
fn prefetch_loop(
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    workers: Workers,
    builds: Arc<crate::builds::Builds>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
    wake: Arc<Wake>,
    loop_stats: Arc<LoopStats>,
    project_version: Arc<AtomicU64>,
    content_version: Arc<AtomicU64>,
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
    shared: Arc<AudioShared>,
    mix: crate::mix::MixCoverage,
    kept_moments: Shown,
    video_gpu: Arc<Mutex<VideoGpuState>>,
) {
    // its stream advances and decodes are the look-ahead (gpu-arbiter D5)
    let _class = kt_time::sched::with_class(kt_time::sched::Class::Near);
    // the renderers are the player's shared build (edit-without-rebuild): this thread builds
    // none; it notes when it takes a new one
    let mut taken: Option<u64> = None;
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
    // The build the pass reads is in it too: a version's build lands after its edit
    // (edit-without-rebuild), and a pause must not skip the pass that takes it.
    type PassState = (i64, u64, u64, u64, Quality, Option<(i64, i64)>, u64);
    let mut last_state: Option<PassState> = None;
    // a stream stage at the frame on screen still lacks history (decode-request-storm): the pass
    // runs again while it does, paused or not — the decoder fills the window, this thread advances
    // the stage over it, and the next window is ordered. Nothing pending — the pause costs nothing.
    let mut stream_pending = false;
    // the wall clock is legal here: the live preview's prefetch, not an offline path (invariant 11)
    let mut last_pass = std::time::Instant::now();
    let mut memo_sized: Option<(usize, Option<usize>)> = None;
    // the loop the display's executor was told to keep, and which executor (memory-ledger D4)
    let mut loop_told: Option<(Option<usize>, usize)> = None;
    // what the decoded-frame cache was sized for (frames-cache-growth)
    let mut frames_sized = crate::prefetch::FramesSizing::default();
    // the last pass was skipped: nothing moved (one-scheduler: then the thread parks with no clock)
    let mut skipped = false;
    let mut seen = wake.seen();
    while !stop.load(Ordering::Acquire) {
        // playing, a stream stage short of history (the decoder fills it, nothing notifies) or a
        // state that just moved: a pass every 100 ms; else park until something notifies (it woke
        // every 100 ms on a pause before)
        if transport.is_playing() || stream_pending || !skipped {
            wake.wait_timeout(std::time::Duration::from_millis(100));
        } else {
            wake.wait_since(seen);
        }
        seen = wake.seen();
        if stop.load(Ordering::Acquire) {
            return;
        }
        tell_loop(&video_gpu, &builds, &loop_region, &mut loop_told);
        {
            let state = (
                transport.playhead_frames(),
                project_version.load(Ordering::Acquire),
                content_version.load(Ordering::Acquire),
                transport.generation(),
                *preview_quality.lock().unwrap(),
                *loop_region.lock().unwrap(),
                builds.built_seq(),
            );
            if !transport.is_playing() && last_state == Some(state) && !stream_pending {
                skipped = true;
                continue;
            }
            skipped = false;
            last_state = Some(state);
        }
        loop_stats.prefetch_passes.fetch_add(1, Ordering::Relaxed);
        kt_time::work::bump(kt_time::work::Work::PrefetchPass);
        // the pass in the trace: its requests and stage advances carry its id as their cause
        let pass = kt_time::trace::next_id();
        let _cause = kt_time::trace::with_cause(pass);
        let mut pass_span = kt_time::trace::span(kt_time::trace::Ev::Job);
        pass_span.id(pass).aux(kt_time::trace::job::PREFETCH_PASS).a(transport.is_playing() as u64);
        let built = builds.latest();
        if taken != Some(built.seq) {
            taken = Some(built.seq);
            kt_time::trace::ev(kt_time::trace::Ev::Invalidate)
                .aux(kt_time::trace::inval::RENDERERS)
                .a(built.version)
                .b(built.seq)
                .emit();
            loop_stats.prefetch_rebuilds.fetch_add(1, Ordering::Relaxed);
        }
        let (renderer, video) = (&*built.audio, &*built.video);
        let generation = transport.generation();
        let head = transport.playhead_frames();
        let region = *loop_region.lock().unwrap();
        // what the final-mix store holds is not ordered (cached-range-plays)
        let held = mix.get().playing(region);
        for win in lookahead_windows(head, PREFETCH_AUDIO_LOOKAHEAD_FRAMES, region) {
            if held.iter().any(|r| r.start <= win.start && r.end >= win.end) {
                continue;
            }
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
        // the window starts at the frame ON SCREEN, not at the audio playhead: under an
        // output latency the picture is `latency` behind (`video_display_frame`), and a
        // window from the playhead would leave those frames out — and withdraw their
        // requests below (video-prefetch-thrash). The reverse window stays as it was.
        let rate = f32::from_bits(shared.playback_rate.load(Ordering::Acquire));
        let shown = if rate > 0.0 {
            let latency = shared.latency_smooth.load(Ordering::Acquire);
            video_display_frame(head, latency, region, 1, rate)
        } else {
            head
        };
        let q = *preview_quality.lock().unwrap();
        let step = video.grid.frame_step();
        let region_ticks =
            region.map(|(s, e)| (Tick::from_audio_frames(s).0, Tick::from_audio_frames(e).0));
        let t0 = Tick::from_audio_frames(shown);
        // the stream stages of the frame on screen advance HERE, over what the decoder brought
        // since the last pass, before the window asks for more (decode-request-storm): a
        // composite runs a stage only a few positions short, so the UI thread never computes a
        // history, and a history ordered window by window is read before the cache drops it
        let interval = last_pass.elapsed();
        last_pass = std::time::Instant::now();
        // the moments whose composite the display's executor keeps play without work: neither
        // their decodes nor their stream history are asked (structured-telemetry, I8)
        let kept_now: std::collections::HashSet<i64> = {
            let versions = (project_version.load(Ordering::Acquire), content_version.load(Ordering::Acquire));
            kept_moments.lock().unwrap().iter().filter(|(_, (_, p, c))| (*p, *c) == versions).map(|(t, _)| *t).collect()
        };
        let kept = |t: Tick| kept_now.contains(&t.0);
        if video.has_stream_stages() {
            size_stream_memo(video, t0, q, region_ticks, &mut memo_sized);
        }
        stream_pending = video.has_stream_stages() && {
            let ahead = if transport.is_playing() { stream_ahead_frames(video, t0, q, interval) } else { 0 };
            // the advance stops once the playhead or the project leaves it (gpu-arbiter, row 147):
            // what it computed stays in the memo, and the next pass starts from where the playhead is
            let left = {
                let (transport, stop, version) = (transport.clone(), stop.clone(), project_version.clone());
                let (generation, at) = (transport.generation(), project_version.load(Ordering::Acquire));
                move || {
                    stop.load(Ordering::Acquire)
                        || transport.generation() != generation
                        || version.load(Ordering::Acquire) != at
                }
            };
            crate::video::stop_when(kt_time::trace::stop::PREFETCH, left, || {
                stream_advance_ahead(video, &cache, t0, q, region_ticks, ahead, &kept)
            })
        };
        let (missing, window) = video_prefetch_pass_sized(video, &cache, t0, q, region_ticks, &kept);
        // the cache asks the memory ledger for what this window and the loop need at this quality
        size_frames_cache(video, &cache, q, region_ticks, built.seq, window, &mut frames_sized);
        pass_span.b(missing.len() as u64).c(stream_pending as u64);
        let workers = workers.read().unwrap();
        // what the playhead has passed is no longer asked for: a request left in the queue
        // would be decoded behind the playhead into an entry nobody reads, in the time the
        // frames ahead need (video-prefetch-thrash). The look-ahead requests are this
        // loop's alone — Playback ones (the frame on screen) are not touched.
        type Keys = std::collections::HashSet<(i64, (u32, u32))>;
        let mut keep: HashMap<MediaId, Keys> = HashMap::new();
        for (media, src_pts, dq) in &missing {
            keep.entry(*media).or_default().insert((src_pts.0, (dq.width, dq.height)));
        }
        for (media, w) in workers.iter() {
            let k = keep.get(media);
            w.withdraw_video(Priority::NearPrefetch, |pts, q| {
                k.is_some_and(|k| q.is_some_and(|q| k.contains(&(pts, q))))
            });
        }
        for (media, src_pts, dq) in missing {
            if let Some(w) = workers.get(&media) {
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
        assert_eq!(video_display_frame(1000, 0, None, -1, 1.0), 1000, "zero delay is neutral");
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
mod display_lap_tests {
    use super::{DisplayLap, DisplaySample, video_display_frame};
    use crate::transport::Transport;

    const RS: i64 = 48_000;
    const RE: i64 = 96_000;

    fn sample(t: &Transport, latency: i64, rate: f32) -> DisplaySample {
        DisplaySample {
            head: t.playhead_frames(),
            extra: 0,
            wraps: t.wraps(),
            generation: t.generation(),
            jump_target: t.last_seek_frames(),
            latency,
            region: Some((RS, RE)),
            rate,
        }
    }

    /// Test 900 (loop-play-latency-hold, D1/D3): the display lap changes when the SHOWN
    /// frame folds — at the head position `rs + latency` (in reverse `re − latency`) —
    /// never at the head's wrap, exactly once per lap, also when the latency jitters
    /// by ±512 frames around the fold.
    #[test]
    fn t900_display_lap_follows_the_shown_fold() {
        for dir in [1i64, -1] {
            for lat in [0i64, 512, 4_800, 14_400] {
                let t = Transport::new();
                t.seek_frames(if dir > 0 { RS } else { RE });
                let mut dl = DisplayLap::default();
                let (_, lap0) = dl.step(sample(&t, lat, dir as f32));
                let step = 128;
                let mut changes: Vec<i64> = Vec::new();
                let mut prev_lap = lap0;
                let travel = 3 * (RE - RS) + lat + (RE - RS) / 2;
                let mut n = 0i64;
                while n * step < travel {
                    t.advance_frames_wrapped(step * dir, Some((RS, RE)));
                    n += 1;
                    let jitter = if lat >= 512 { if n % 2 == 0 { 512 } else { -512 } } else { 0 };
                    let (_, lap) = dl.step(sample(&t, lat + jitter, dir as f32));
                    assert!(lap >= prev_lap, "dir {dir} lat {lat}: the lap went back");
                    if lap != prev_lap {
                        assert_eq!(lap, prev_lap + 1, "dir {dir} lat {lat}: two folds at once");
                        changes.push(t.playhead_frames());
                    }
                    prev_lap = lap;
                }
                assert_eq!(changes.len(), 3, "dir {dir} lat {lat}: laps at heads {changes:?}");
                let tol = if lat >= 512 { 512 + step } else { step };
                for h in &changes {
                    let want = if dir > 0 { RS + lat } else { RE - lat };
                    assert!(
                        (h - want).abs() <= tol,
                        "dir {dir} lat {lat}: the lap changed at head {h}, the shown fold is at {want}"
                    );
                }
            }
        }
    }

    /// Test 902 (D2): after a jump to `p` the shown frame is `p` until the head is
    /// `latency·|rate|` past it — never a frame from the region's end; then the head minus
    /// the scaled latency; the first fold of the shown frame comes only after the head
    /// itself has wrapped once.
    #[test]
    fn t902_after_a_jump_the_shown_frame_starts_at_the_target() {
        let lat = 9_600i64;
        for rate in [1.0f32, 0.25] {
            for dir in [1i64, -1] {
                let signed = rate * dir as f32;
                let targets = if dir > 0 {
                    [RS, RS + (RE - RS) / 3, RS - 30_000, RE + 30_000]
                } else {
                    [RE, RS + (RE - RS) / 3, RE + 30_000, RS - 30_000]
                };
                for p in targets {
                    let t = Transport::new();
                    t.seek_frames(10);
                    let mut dl = DisplayLap::default();
                    // playing elsewhere first: the jump must not inherit that state
                    let _ = dl.step(sample(&t, lat, signed));
                    t.advance_frames_wrapped(5_000, Some((RS, RE)));
                    let _ = dl.step(sample(&t, lat, signed));
                    t.seek_frames(p);
                    let (_, lap_jump) = dl.step(sample(&t, lat, signed));
                    let scaled = (lat as f64 * rate as f64) as i64;
                    let step = 96;
                    let mut travelled = 0i64;
                    let mut wrapped = false;
                    let mut folded = false;
                    while travelled < 2 * (RE - RS) {
                        t.advance_frames_wrapped(step * dir, Some((RS, RE)));
                        travelled += step;
                        wrapped |= t.wraps() > 0;
                        let (frame, lap) = dl.step(sample(&t, lat, signed));
                        let ctx = format!("rate {rate} dir {dir} p {p} travelled {travelled}");
                        if lap != lap_jump {
                            assert!(
                                wrapped,
                                "{ctx}: the shown frame folded before the head wrapped"
                            );
                            folded = true;
                        }
                        if travelled < scaled {
                            assert_eq!(frame, p, "{ctx}: inside the hold the target must show");
                        } else if !wrapped {
                            let want = t.playhead_frames() - scaled * dir;
                            assert_eq!(
                                frame, want,
                                "{ctx}: after the hold, the head minus the latency"
                            );
                        } else if !folded {
                            // the shown frame still lies in the lap before the head's wrap:
                            // the steady-state rule says the same
                            let want = video_display_frame(
                                t.playhead_frames(),
                                lat,
                                Some((RS, RE)),
                                dir,
                                signed,
                            );
                            assert_eq!(frame, want, "{ctx}: between the wrap and the fold");
                        }
                    }
                    // a target before the region (forward) or after it (reverse) still
                    // loops once the head is inside; past the far edge the loop is disarmed
                    let armed = super::loop_arm_after_jump(p, Some((RS, RE)), dir);
                    assert_eq!(folded, armed, "rate {rate} dir {dir} p {p}: fold expectation");
                }
            }
        }
    }
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
