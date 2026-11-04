//! The CPU preview composes ahead of the display (preview-throughput D4).
//!
//! The CPU executor is one frame at a time, and a heavy edit costs more than a frame's time, so
//! on the UI thread it could only fall behind. Here worker threads compose the frames the display
//! is about to reach, in the export's own way (`compose_export_with`: a shared renderer, a scratch
//! per worker), and the UI thread picks a finished frame up. A frame not ready is composed where
//! it always was, so the worst case is the former path.
//!
//! - The workers read the cache and nothing else: no source, no decode request (invariant 5). A
//!   frame whose layers are not all there is left for later, never composed incomplete.
//! - A frame is a function of (project, content, quality, tick): those are the key, so an edit
//!   cannot show a stale picture.
//! - They wait on a condvar while the transport stands: nothing runs in a pause (invariant 12).
//! - A stream stage (datamosh) keeps the synchronous path: its frame depends on the order of the
//!   frames before it, as in the export.

use crate::player::{AudioShared, lookahead_windows};
use crate::source_id;
use crate::transport::Transport;
use crate::video::{ExportScratch, SourceFacts, VideoRenderer, video_cache_idx};
use kt_cache::BurstCache;
use kt_media::Quality;
use kt_model::Project;
use kt_time::Tick;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

/// What a finished frame depends on, besides its tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub project: u64,
    pub content: u64,
    pub quality: Quality,
}

/// Everything the workers read of the player.
pub(crate) struct Sources {
    pub cache: Arc<BurstCache>,
    pub transport: Arc<Transport>,
    pub shared: Arc<AudioShared>,
    pub project: Arc<Mutex<Project>>,
    pub natives: Arc<RwLock<SourceFacts>>,
    pub loop_region: Arc<Mutex<Option<(i64, i64)>>>,
}

#[derive(Default)]
struct State {
    stamp: Option<Stamp>,
    renderer: Option<Arc<VideoRenderer>>,
    building: bool,
    /// The tick on screen, and when the UI last said so: the CPU path is the one composing.
    head: i64,
    heard: Option<Instant>,
    ready: BTreeMap<i64, Vec<u8>>,
    busy: BTreeSet<i64>,
    pool: Vec<Vec<u8>>,
    /// What a composite takes here, smoothed: a frame nearer than this cannot arrive in time.
    compose_ms: f64,
    /// The tick the display is holding on, and since when (`may_hold`).
    held: Option<(i64, Instant)>,
    stop: bool,
}

struct Inner {
    src: Sources,
    state: Mutex<State>,
    cv: Condvar,
    composed: AtomicU64,
    hits: AtomicU64,
    /// How far ahead of the display the workers start, in ticks. The decode look-ahead starts
    /// there: measured from the display, its 0.64 s (1080p, two layers) left 0.24 s past a
    /// 0.4 s composite, and the workers stood waiting for frames at 16 fps.
    lead_ticks: Arc<AtomicI64>,
}

enum Job {
    Build(Stamp),
    Compose { renderer: Arc<VideoRenderer>, t: i64, stamp: Stamp, buf: Vec<u8> },
}

pub(crate) struct ComposeAhead {
    inner: Arc<Inner>,
    threads: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

/// How many workers: half the cores (the decoders and the UI keep the rest), bounded by the memory
/// of their scratches like the export's (about eight output frames each).
fn workers_for(q: Quality) -> usize {
    const WORKER_MEMORY: usize = 1 << 30;
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let per_worker = q.width as usize * q.height as usize * 4 * 8;
    (cores / 2).clamp(1, 16).min((WORKER_MEMORY / per_worker.max(1)).max(1))
}

/// How many frames ahead: the ones too near to start, every worker busy plus a few ready, at most
/// 256 MB of finished frames.
fn window_frames(workers: usize, q: Quality, lead: usize) -> usize {
    const MAX_BYTES: usize = 256 << 20;
    (lead + workers + 4).min(MAX_BYTES / (q.width as usize * q.height as usize * 4).max(1)).max(1)
}

impl Inner {
    /// The ticks the display reaches next, nearest first (folded into the loop region), and how
    /// many of the first are too near to be worth starting: a composite begun for them ends
    /// after the display has left, and the wasted worker is the frame after it, late.
    fn targets(
        &self,
        st: &State,
        renderer: &VideoRenderer,
        q: Quality,
        workers: usize,
    ) -> (Vec<i64>, usize) {
        let rate = f32::from_bits(self.src.shared.playback_rate.load(Ordering::Acquire));
        if rate <= 0.0 {
            return (Vec::new(), 0);
        }
        let head = st.head;
        let step = renderer.grid.frame_step().max(1);
        let stride = step * (rate.ceil() as i64).max(1);
        let frame_ms = stride as f64 * 1000.0 / kt_time::TICK_HZ as f64 / rate as f64;
        let lead = (st.compose_ms / frame_ms).ceil() as usize;
        let n = window_frames(workers, q, lead) as i64;
        let region = self
            .src
            .loop_region
            .lock()
            .unwrap()
            .map(|(s, e)| (Tick::from_audio_frames(s).0, Tick::from_audio_frames(e).0));
        let mut out = Vec::new();
        for r in lookahead_windows(head + stride, n * stride, region) {
            let mut t = renderer.grid.align_floor(Tick(r.start)).0;
            while t < r.end {
                out.push(t);
                t += stride;
            }
        }
        (out, lead)
    }

    fn all_layers_held(&self, renderer: &VideoRenderer, t: i64, q: Quality) -> bool {
        renderer.layers_at(Tick(t), q).into_iter().all(|(m, pts, dq)| {
            self.src.cache.contains_video(source_id(m), Some(dq), video_cache_idx(pts))
        })
    }

    fn next_job(&self, st: &mut State, workers: usize) -> Option<Job> {
        let heard = st.heard.is_some_and(|at| at.elapsed() < Duration::from_millis(500));
        if !heard || !self.src.transport.is_playing() {
            self.lead_ticks.store(0, Ordering::Release);
            return None;
        }
        let stamp = st.stamp?;
        let Some(renderer) = st.renderer.clone() else {
            if st.building {
                return None;
            }
            st.building = true;
            return Some(Job::Build(stamp));
        };
        if renderer.has_stream_stages() {
            self.lead_ticks.store(0, Ordering::Release);
            return None;
        }
        let (wanted, lead) = self.targets(st, &renderer, stamp.quality, workers);
        let ahead = wanted.get(lead).map_or(0, |t| (t - st.head).max(0));
        self.lead_ticks.store(ahead, Ordering::Release);
        // what the picture has left, or a seek moved away from, is not coming back
        let keep: BTreeSet<i64> = wanted.iter().copied().collect();
        let gone: Vec<i64> = st.ready.keys().filter(|t| !keep.contains(*t)).copied().collect();
        for t in gone {
            if let Some(b) = st.ready.remove(&t) {
                st.pool.push(b);
            }
        }
        let t = wanted.into_iter().skip(lead).find(|t| {
            !st.ready.contains_key(t)
                && !st.busy.contains(t)
                && self.all_layers_held(&renderer, *t, stamp.quality)
        })?;
        st.busy.insert(t);
        Some(Job::Compose { renderer, t, stamp, buf: st.pool.pop().unwrap_or_default() })
    }

    fn work(&self, workers: usize) {
        let mut scratch = ExportScratch::default();
        loop {
            let job = {
                let mut st = self.state.lock().unwrap();
                loop {
                    if st.stop {
                        return;
                    }
                    if let Some(j) = self.next_job(&mut st, workers) {
                        break j;
                    }
                    st = if self.src.transport.is_playing() {
                        // new frames reach the cache with no signal here
                        self.cv.wait_timeout(st, Duration::from_millis(4)).unwrap().0
                    } else {
                        self.cv.wait(st).unwrap()
                    };
                }
            };
            match job {
                Job::Build(stamp) => {
                    let p = self.src.project.lock().unwrap().clone();
                    let facts = self.src.natives.read().unwrap();
                    let r = Arc::new(VideoRenderer::new_with_facts(&p, &facts));
                    drop(facts);
                    let mut st = self.state.lock().unwrap();
                    st.building = false;
                    if st.stamp == Some(stamp) {
                        st.renderer = Some(r);
                    }
                    self.cv.notify_all();
                }
                Job::Compose { renderer, t, stamp, mut buf } => {
                    let q = stamp.quality;
                    let began = Instant::now();
                    let full = renderer.compose_export_with(
                        &self.src.cache,
                        Tick(t),
                        q,
                        &mut scratch,
                        &mut buf,
                    );
                    let ms = began.elapsed().as_secs_f64() * 1e3;
                    let mut st = self.state.lock().unwrap();
                    st.compose_ms += (ms - st.compose_ms) / 8.0;
                    st.busy.remove(&t);
                    if full && st.stamp == Some(stamp) && t > st.head {
                        self.composed.fetch_add(1, Ordering::Relaxed);
                        st.ready.insert(t, buf);
                    } else {
                        st.pool.push(buf);
                    }
                }
            }
        }
    }
}

impl ComposeAhead {
    pub fn new(src: Sources, lead_ticks: Arc<AtomicI64>) -> ComposeAhead {
        ComposeAhead {
            inner: Arc::new(Inner {
                src,
                state: Mutex::default(),
                cv: Condvar::new(),
                composed: AtomicU64::new(0),
                hits: AtomicU64::new(0),
                lead_ticks,
            }),
            threads: Mutex::new(Vec::new()),
        }
    }

    /// The UI composes `t` on the CPU path. The frame into `out` when it is ready: a swap, the
    /// frame's bytes are not copied. Threads start here, so a GPU session never has them.
    pub fn take(&self, t: i64, stamp: Stamp, out: &mut Vec<u8>) -> bool {
        if !self.inner.src.transport.is_playing() {
            return false;
        }
        {
            let mut threads = self.threads.lock().unwrap();
            if threads.is_empty() {
                let n = workers_for(stamp.quality);
                for i in 0..n {
                    let inner = self.inner.clone();
                    threads.push(
                        std::thread::Builder::new()
                            .name(format!("compose-ahead-{i}"))
                            .spawn(move || inner.work(n))
                            .expect("spawn compose worker"),
                    );
                }
            }
        }
        let mut st = self.inner.state.lock().unwrap();
        if st.stamp != Some(stamp) {
            // the previous frames are of another project state: none may reach the screen
            let old: Vec<Vec<u8>> = std::mem::take(&mut st.ready).into_values().collect();
            st.pool.extend(old);
            st.stamp = Some(stamp);
            st.renderer = None;
        }
        st.head = t;
        st.heard = Some(Instant::now());
        let hit = st.ready.remove(&t);
        let found = hit.is_some();
        if let Some(mut frame) = hit {
            std::mem::swap(out, &mut frame);
            st.pool.push(frame);
            self.inner.hits.fetch_add(1, Ordering::Relaxed);
        }
        drop(st);
        self.inner.cv.notify_all();
        found
    }

    /// May a display that missed `t` hold the picture and ask again next UI frame? Composing the
    /// frame on the UI thread as well would stop the picture for a composite and throw away what
    /// is on its way. It is a promise with a limit: a frame in a worker's hands is coming, and
    /// any other only while the display moves on by itself. On a clock that stands, at a rate
    /// the workers do not serve, or past three frames on one tick, nobody will bring this frame,
    /// and the caller composes it: a held picture must not become a frozen one.
    pub fn may_hold(&self, t: i64, stamp: Stamp) -> bool {
        let rate = f32::from_bits(self.inner.src.shared.playback_rate.load(Ordering::Acquire));
        let mut st = self.inner.state.lock().unwrap();
        let serving =
            st.stamp == Some(stamp) && self.inner.src.transport.is_playing() && rate > 0.0;
        let Some(renderer) = st.renderer.clone().filter(|r| serving && !r.has_stream_stages())
        else {
            return false;
        };
        if st.busy.contains(&t) {
            return true;
        }
        let since = match st.held {
            Some((tick, at)) if tick == t => at,
            _ => {
                let now = Instant::now();
                st.held = Some((t, now));
                now
            }
        };
        let frame_s = renderer.grid.frame_step().max(1) as f64 / kt_time::TICK_HZ as f64;
        since.elapsed().as_secs_f64() < 3.0 * frame_s / rate as f64
    }

    /// (frames composed ahead, frames the display took).
    pub fn counts(&self) -> (u64, u64) {
        (self.inner.composed.load(Ordering::Relaxed), self.inner.hits.load(Ordering::Relaxed))
    }

    pub fn shutdown(&self) {
        self.inner.state.lock().unwrap().stop = true;
        self.inner.cv.notify_all();
        for t in self.threads.lock().unwrap().drain(..) {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t1532: a frame finished for another state of the project never reaches the screen.
    #[test]
    fn t1532_a_frame_of_another_project_state_is_not_served() {
        let shared = Arc::new(AudioShared {
            consumed_frames: 0.into(),
            flush: false.into(),
            callback_underruns: 0.into(),
            playback_rate: 1.0f32.to_bits().into(),
            latency_smooth: 0.into(),
            callback_frames: 0.into(),
            output_latency_frames: 0.into(),
        });
        let transport = Arc::new(Transport::new());
        transport.set_playing(true);
        let ahead = ComposeAhead::new(
            Sources {
                cache: Arc::new(BurstCache::new(Default::default())),
                transport,
                shared,
                project: Arc::new(Mutex::new(Project::new(kt_model::ProjectSettings {
                    fps: 25,
                    width: 8,
                    height: 4,
                    sample_rate: 48000,
                    pitch_engine: Default::default(),
                    default_shape: Default::default(),
                    conform: Default::default(),
                    settings_explicit: true,
                }))),
                natives: Default::default(),
                loop_region: Default::default(),
            },
            Default::default(),
        );
        let q = Quality { width: 4, height: 2 };
        let a = Stamp { project: 1, content: 0, quality: q };
        let mut out = Vec::new();
        assert!(!ahead.take(0, a, &mut out), "nothing is ready yet");
        ahead.inner.state.lock().unwrap().ready.insert(3600, vec![7; 32]);
        assert!(ahead.take(3600, a, &mut out), "the same state takes it");
        assert_eq!(out, vec![7; 32]);

        ahead.inner.state.lock().unwrap().ready.insert(7200, vec![9; 32]);
        for b in [
            Stamp { project: 2, ..a },
            Stamp { project: 2, content: 1, ..a },
            Stamp { project: 2, content: 1, quality: Quality { width: 8, height: 4 } },
        ] {
            assert!(!ahead.take(7200, b, &mut out), "{b:?}");
            assert_eq!(out, vec![7; 32], "the picture is untouched");
            ahead.inner.state.lock().unwrap().ready.insert(7200, vec![9; 32]);
        }
        ahead.shutdown();
    }
}
