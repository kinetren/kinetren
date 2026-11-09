//! One build per project version, shared (edit-without-rebuild, `docs/engine-audit.md` M1).
//!
//! An edit used to be built by every consumer on its own: the UI thread built a video renderer, the
//! `audio-build` thread an audio one, prefetch its own pair, the probe (meters, the loop's pinned head)
//! another — 2 audio and 2–3 video builds per edit, 88 ms of them on the UI thread on the owner's 1974
//! events (release, `kt-cli edit-cost`). Now the player's `engine-build` thread builds the version
//! once, in one revisions scope (each reel resolved once) and with the last build's per-event state
//! reused (`incremental::BuildMemo`), and publishes a [`Built`]: the audio render thread swaps to its
//! audio renderer, prefetch and the probe read it, the preview composes with its video renderer.
//!
//! Rules:
//! - Nothing waits for a build on the UI thread. `Player::set_project` stores the `Arc<Project>` and
//!   wakes this thread; the UI's readers (the grid, the coverage bars) see the last published build.
//! - A frame is composed only by the renderer of the state it was asked for: the compose thread waits
//!   for that build ([`Builds::wait_current`]), never the UI.
//! - A build is asked by a WANT: a project version, or the same version with new source facts (a
//!   source raised or replaced). The thread always builds the newest want; the ones in between are
//!   never built.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use kt_model::Project;

use crate::audio::AudioRenderer;
use crate::video::VideoRenderer;

/// A published build: what every consumer of one project version reads.
pub struct Built {
    /// The want it answers (`Builds::want_seq`).
    pub seq: u64,
    /// The project version it was built from.
    pub version: u64,
    pub project: Arc<Project>,
    pub audio: Arc<AudioRenderer>,
    pub video: Arc<VideoRenderer>,
    /// What this build walked and derived (`kt_time::cost`, on the builder thread): an edit's
    /// `EventDerive` is the events it touched.
    pub cost: kt_time::cost::Costs,
}

pub(crate) struct Want {
    /// The newest project the model handed over. Also the player's stored project
    /// (`Player::project`): a value edit that moves no plan (law P) replaces it in place.
    pub project: Arc<Project>,
    pub version: u64,
    pub seq: u64,
}

pub(crate) struct Builds {
    pub want: Mutex<Want>,
    want_cv: Condvar,
    built: Mutex<Arc<Built>>,
    built_cv: Condvar,
    built_seq: AtomicU64,
    /// A test's hold of the builder before it builds (`Player::gate_builds`).
    gate: Mutex<bool>,
    gate_cv: Condvar,
    pub stop: AtomicBool,
}

impl Builds {
    pub fn new(first: Built) -> Builds {
        Builds {
            want: Mutex::new(Want { project: first.project.clone(), version: first.version, seq: first.seq }),
            want_cv: Condvar::new(),
            built_seq: AtomicU64::new(first.seq),
            built: Mutex::new(Arc::new(first)),
            built_cv: Condvar::new(),
            gate: Mutex::new(false),
            gate_cv: Condvar::new(),
            stop: AtomicBool::new(false),
        }
    }

    /// Ask the build of a new project version.
    pub fn want_project(&self, project: Arc<Project>, version: u64) {
        let mut w = self.want.lock().unwrap();
        w.project = project;
        w.version = version;
        w.seq += 1;
        drop(w);
        self.want_cv.notify_all();
    }

    /// Ask the same version again: the source facts changed (a source raised or replaced).
    pub fn refresh(&self) {
        self.want.lock().unwrap().seq += 1;
        self.want_cv.notify_all();
    }

    pub fn want_seq(&self) -> u64 {
        self.want.lock().unwrap().seq
    }

    pub fn built_seq(&self) -> u64 {
        self.built_seq.load(Ordering::Acquire)
    }

    /// The last published build (a pointer clone).
    pub fn latest(&self) -> Arc<Built> {
        self.built.lock().unwrap().clone()
    }

    /// The build of the newest want, waited for (the compose thread, a test); the last published one
    /// if the player stops meanwhile.
    pub fn wait_current(&self) -> Arc<Built> {
        let want = self.want_seq();
        let mut g = self.built.lock().unwrap();
        while g.seq < want && !self.stop.load(Ordering::Acquire) {
            g = self.built_cv.wait_timeout(g, std::time::Duration::from_millis(50)).unwrap().0;
        }
        g.clone()
    }

    pub fn publish(&self, built: Built) {
        let seq = built.seq;
        // the old build is dropped outside the lock, and not necessarily here: a consumer may still
        // hold it, and the last pointer frees it where it goes
        let old = std::mem::replace(&mut *self.built.lock().unwrap(), Arc::new(built));
        self.built_seq.store(seq, Ordering::Release);
        self.built_cv.notify_all();
        drop(old);
    }

    /// The next want to build, waited for; `None` when the player stops.
    pub fn next_want(&self, done: u64) -> Option<(Arc<Project>, u64, u64)> {
        let mut w = self.want.lock().unwrap();
        loop {
            if self.stop.load(Ordering::Acquire) {
                return None;
            }
            if w.seq > done {
                return Some((w.project.clone(), w.version, w.seq));
            }
            w = self.want_cv.wait(w).unwrap();
        }
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        // the guards in statements of their own: notify under them, or a waiter can miss it
        drop(self.want.lock().unwrap());
        self.want_cv.notify_all();
        drop(self.built.lock().unwrap());
        self.built_cv.notify_all();
        self.set_gate(false);
    }

    pub fn set_gate(&self, closed: bool) {
        *self.gate.lock().unwrap() = closed;
        self.gate_cv.notify_all();
    }

    pub fn wait_gate(&self) {
        let mut g = self.gate.lock().unwrap();
        while *g && !self.stop.load(Ordering::Acquire) {
            g = self.gate_cv.wait(g).unwrap();
        }
    }
}
