//! What a build reuses (edit-without-rebuild): a reel resolved once per build, and an event's
//! derived state kept from one build to the next.
//!
//! - **Resolve once per build** (found-defects row 155). The audio build, the video build, the decode
//!   step (`nest_scales`) and the generator sources each resolved every reel (its morpher layers,
//!   its links), about five times per build. Inside [`crate::video::revisions_scope`] a reel instance
//!   (reel, overrides) is resolved once and shared.
//! - **An edit costs what it touched.** The player's `engine-build` thread keeps a [`BuildMemo`]:
//!   the audio event and the video layer (before transitions) of the last build, each keyed by a
//!   hash of EVERYTHING it is derived from — the model event's text (the same text the event's
//!   fingerprint `fp` hashes) plus its reel context (the source address, the track's level, the
//!   instance, the facts of its source, the cameras, the raster). An unchanged key is a copy, never
//!   a recomputation, so the memo is invisible to the bytes (invariant 11, t2004): a key that
//!   misses something a builder reads would be a stale event, and that is why the key is built
//!   from the same values the builder passes on.
//!
//! Outside a [`with_memo`] scope (an export, `kt-cli`, a test's fresh build) every event is derived
//! afresh; the count (`Cost::EventDerive`) is the same either way.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use kt_model::{MediaId, Override, Project, ResolvedReel};
use kt_time::cost::{self, Cost};

/// Whole renderers built in this process, audio and video (`Cost::RendererBuild` counts per
/// thread): a guard that one project version is built once for every consumer (t2000).
#[doc(hidden)]
pub static RENDERER_BUILDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn count_renderer_build() {
    cost::add(Cost::RendererBuild, 1);
    // a whole renderer on the UI or the render thread is a wait (one-scheduler): counted there
    drop(kt_time::block::wait(kt_time::block::kind::BUILD));
    RENDERER_BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// The derived state of the last build's events, by key; each entry carries the build it was last
/// used in, and a build keeps only its own (`with_memo` sweeps), so the memo is bounded by the
/// project.
#[derive(Default)]
pub struct BuildMemo {
    audio: HashMap<u64, (u64, crate::audio::AudioClip)>,
    video: HashMap<u64, (u64, crate::video::VideoClip)>,
    stamp: u64,
}

impl BuildMemo {
    /// The events it holds (audio, video).
    pub fn len(&self) -> (usize, usize) {
        (self.audio.len(), self.video.len())
    }

    pub fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.video.is_empty()
    }
}

/// A build scope's resolved reel instances: (project address, reel, overrides hash) → the reel.
type Resolved = HashMap<(usize, Option<MediaId>, u64), Arc<ResolvedReel<'static>>>;

thread_local! {
    static MEMO: RefCell<Option<BuildMemo>> = const { RefCell::new(None) };
    /// The reel instances resolved in the open revisions scope, keyed by the project's address:
    /// the project is borrowed for the whole scope and cannot change.
    static RESOLVED: RefCell<Option<Resolved>> =
        const { RefCell::new(None) };
}

/// Runs `f` (one build, or several builds of ONE project) with `memo` reused and refilled; what
/// this build did not use is dropped.
pub fn with_memo<R>(memo: &mut BuildMemo, f: impl FnOnce() -> R) -> R {
    let mut m = std::mem::take(memo);
    m.stamp += 1;
    MEMO.with(|slot| *slot.borrow_mut() = Some(m));
    struct Back<'a>(&'a mut BuildMemo);
    impl Drop for Back<'_> {
        fn drop(&mut self) {
            if let Some(mut m) = MEMO.with(|slot| slot.borrow_mut().take()) {
                let stamp = m.stamp;
                m.audio.retain(|_, (s, _)| *s == stamp);
                m.video.retain(|_, (s, _)| *s == stamp);
                *self.0 = m;
            }
        }
    }
    let _back = Back(memo);
    f()
}

pub(crate) fn audio_clip(key: u64, derive: impl FnOnce() -> crate::audio::AudioClip) -> crate::audio::AudioClip {
    let hit = MEMO.with(|slot| {
        let mut slot = slot.borrow_mut();
        let m = slot.as_mut()?;
        let stamp = m.stamp;
        m.audio.get_mut(&key).map(|(s, c)| {
            *s = stamp;
            c.clone()
        })
    });
    if let Some(c) = hit {
        return c;
    }
    cost::add(Cost::EventDerive, 1);
    let c = derive();
    MEMO.with(|slot| {
        if let Some(m) = slot.borrow_mut().as_mut() {
            m.audio.insert(key, (m.stamp, c.clone()));
        }
    });
    c
}

pub(crate) fn video_clip(key: u64, derive: impl FnOnce() -> crate::video::VideoClip) -> crate::video::VideoClip {
    let hit = MEMO.with(|slot| {
        let mut slot = slot.borrow_mut();
        let m = slot.as_mut()?;
        let stamp = m.stamp;
        m.video.get_mut(&key).map(|(s, c)| {
            *s = stamp;
            c.clone()
        })
    });
    if let Some(c) = hit {
        return c;
    }
    cost::add(Cost::EventDerive, 1);
    let c = derive();
    MEMO.with(|slot| {
        if let Some(m) = slot.borrow_mut().as_mut() {
            m.video.insert(key, (m.stamp, c.clone()));
        }
    });
    c
}

/// Opens the resolve memo with the revisions scope (`video::revisions_scope`); `true` when this
/// call opened it.
pub(crate) fn open_resolved() -> bool {
    RESOLVED.with(|m| {
        let mut m = m.borrow_mut();
        let open = m.is_none();
        if open {
            *m = Some(HashMap::new());
        }
        open
    })
}

pub(crate) fn close_resolved() {
    RESOLVED.with(|m| *m.borrow_mut() = None);
}

/// `Project::resolve_reel`, once per reel instance inside a build scope.
pub(crate) fn resolve(project: &Project, seq: Option<MediaId>, overrides: &[Override]) -> Arc<ResolvedReel<'static>> {
    let key = (project as *const Project as usize, seq, if overrides.is_empty() { 0 } else { hash_debug(&overrides) });
    if let Some(r) = RESOLVED.with(|m| m.borrow().as_ref().and_then(|m| m.get(&key).cloned())) {
        return r;
    }
    cost::add(Cost::ReelResolve, 1);
    let r = project.resolve_reel(seq, overrides);
    let r = Arc::new(ResolvedReel {
        clips: r.clips,
        transitions: std::borrow::Cow::Owned(r.transitions.into_owned()),
        cameras: std::borrow::Cow::Owned(r.cameras.into_owned()),
    });
    RESOLVED.with(|m| {
        if let Some(m) = m.borrow_mut().as_mut() {
            m.insert(key, r.clone());
        }
    });
    r
}

/// A streaming hasher behind `fmt::Write`: the Debug text of a value hashed with no `String` built.
/// The digest equals `xxh3_64` of the text.
pub(crate) struct DebugHash(xxhash_rust::xxh3::Xxh3);

impl DebugHash {
    pub(crate) fn new() -> DebugHash {
        DebugHash(xxhash_rust::xxh3::Xxh3::new())
    }

    pub(crate) fn digest(&self) -> u64 {
        self.0.digest()
    }
}

impl std::fmt::Write for DebugHash {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.update(s.as_bytes());
        Ok(())
    }
}

/// `xxh3_64(format!("{v:?}"))` without the `String`.
pub(crate) fn hash_debug(v: &impl std::fmt::Debug) -> u64 {
    hash_fmt(format_args!("{v:?}"))
}

/// `xxh3_64(format!(…))` without the `String`.
pub(crate) fn hash_fmt(args: std::fmt::Arguments<'_>) -> u64 {
    let mut h = DebugHash::new();
    let _ = std::fmt::write(&mut h, args);
    h.digest()
}
