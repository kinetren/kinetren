//! Offline analysis of material (R.49, item 372).
//!
//! One subsystem for “compute something over a whole file”: the loudness
//! of the source and the envelope for auto-ducking.
//!
//! There is NO audio-based synchronization here, and this is not an oversight: it is computed from
//! the peak pyramid already in memory — milliseconds without a single
//! decode. A task that does not read the file does not need a background subsystem:
//! a progress window and a queue would be ceremony around instant work.
//!
//! **The “one task” gate.** Analysis reads files whole and decodes them;
//! running three at once means taking the disk and CPU away from
//! PLAYBACK, which promises real time (invariant 6). Therefore
//! exactly one task runs; the others wait in the queue.
//!
//! **Deduplicated queue.** A person presses “compute” twice — this is one job,
//! not two. A duplicate in the queue would mean a second full decode of the same file.
//!
//! **Negative cache.** A task that failed (file missing, format
//! unreadable) is NOT re-queued by itself: otherwise the program would forever
//! hammer the unavailable file. The reason is remembered and shown; a retry
//! happens only on direct request.
//!
//! **Viewing does NOT place a request.** A bin card hint, hovering over a clip,
//! opening a window — none of this starts analysis. This is a direct relative of
//! invariant 8 (“scroll/zoom does not generate decode requests”): otherwise, by moving
//! the mouse over a bin, a person would start decoding dozens of files.

use kt_model::MediaId;

/// What to compute.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AnalysisKind {
    /// Media loudness: LUFS, dBTP, LRA.
    Loudness(MediaId),
}

impl AnalysisKind {
    /// Media the task reads. Needs stale protection: if the file was swapped,
    /// an unfinished task about it is no longer about this project.
    pub fn media(self) -> Vec<MediaId> {
        match self {
            AnalysisKind::Loudness(m) => vec![m],
        }
    }
}

/// Task result.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AnalysisOut {
    Loudness {
        media: MediaId,
        lufs: Option<f64>,
        dbtp: f64,
        lra: f64,
    },
}

/// Progress step: fraction 0..1.
pub type AnalysisProgress = f32;

type Job = crate::bg::BgJob<AnalysisProgress, Result<AnalysisOut, String>>;

/// Subsystem state.
#[derive(Default)]
pub struct Analysis {
    running: Option<(AnalysisKind, Job)>,
    queue: std::collections::VecDeque<AnalysisKind>,
    /// Negative cache: what failed and why.
    failed: std::collections::HashMap<AnalysisKind, String>,
    /// Progress window is open.
    pub open: bool,
    /// Model revision on which the current task was queued: the result for
    /// swapped media must not be applied.
    rev: u64,
    /// Ready task bodies queued via `request_with`.
    #[allow(clippy::type_complexity)]
    bodies: std::collections::HashMap<AnalysisKind, Box<AnalysisBody>>,
}

type AnalysisBody = dyn FnOnce(crate::bg::BgCtl, crate::bg::BgSend<AnalysisProgress>) -> Result<AnalysisOut, String>
    + Send;

impl Analysis {
    /// What is being computed right now.
    pub fn current(&self) -> Option<AnalysisKind> {
        self.running.as_ref().map(|(k, _)| *k)
    }

    /// Completion fraction of the current task.
    pub fn progress(&self) -> f32 {
        self.running.as_ref().and_then(|(_, j)| j.progress).unwrap_or(0.0)
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Whether the work is ALIVE. This is what drives repainting — not `is_some()` of the
    /// window: a window left on screen after finish would heat the system
    /// around the clock (repeat of defect t38).
    pub fn is_running(&self) -> bool {
        self.running.as_ref().is_some_and(|(_, j)| j.is_running())
    }

    /// Why the task failed (negative cache).
    pub fn failure(&self, k: AnalysisKind) -> Option<&str> {
        self.failed.get(&k).map(|s| s.as_str())
    }

    /// Enqueue a task WITH A READY BODY.
    ///
    /// Needed where only the caller knows how to compute: for ducking, the body carries
    /// a snapshot of the project and the dialog settings, and the subsystem need not know about them.
    /// The “one task” gate and the queue work the same way.
    /// `retry` clears the negative cache: “try again” is a direct request of
    /// the person, and the program itself does not hammer the unavailable file.
    pub fn request_with<F>(&mut self, k: AnalysisKind, rev: u64, retry: bool, body: F)
    where
        F: FnOnce(
                crate::bg::BgCtl,
                crate::bg::BgSend<AnalysisProgress>,
            ) -> Result<AnalysisOut, String>
            + Send
            + 'static,
    {
        if retry {
            self.failed.remove(&k);
        } else if self.failed.contains_key(&k) {
            return; // already tried, did not succeed
        }
        if self.current() == Some(k) || self.queue.contains(&k) {
            return; // dedup: decoding the same file a second time is pointless
        }
        self.bodies.insert(k, Box::new(body));
        self.queue.push_back(k);
        self.rev = rev;
        self.open = true;
    }

    /// Cancel everything: the current task and the queue.
    pub fn cancel_all(&mut self) {
        if let Some((_, j)) = &self.running {
            j.cancel();
        }
        self.queue.clear();
    }

    /// Move the subsystem: take the finished result, start the next one.
    /// `spawn` builds the task body — the subsystem does not know how to compute it.
    ///
    /// Returns the finished result (if any) — together with the revision
    /// on which the task was queued: it must not be applied to another model.
    pub fn poll<F>(&mut self, spawn: F) -> Option<(AnalysisKind, u64, Result<AnalysisOut, String>)>
    where
        F: FnOnce(AnalysisKind) -> Job,
    {
        let mut out = None;
        if let Some((kind, job)) = &mut self.running {
            job.poll();
            if let Some(r) = job.done.take() {
                let kind = *kind;
                if let Err(e) = &r {
                    self.failed.insert(kind, e.clone());
                }
                out = Some((kind, self.rev, r));
                self.running = None;
            }
        }
        // gate “one task”: analysis decodes files whole, and three at once
        // would take the disk away from playback, which promises real time
        if self.running.is_none()
            && let Some(next) = self.queue.pop_front()
        {
            let job = match self.bodies.remove(&next) {
                Some(body) => crate::bg::BgJob::spawn("analysis", body),
                None => spawn(next),
            };
            self.running = Some((next, job));
        }
        if self.running.is_none() && self.queue.is_empty() {
            self.open = false;
        }
        out
    }
}

/// Default task body — for kinds the subsystem can handle by itself.
/// There are none for now: both loudness and synchronization are computed where
/// access to sources and pyramids exists, and arrive via `request_with`.
pub fn spawn_default(_k: AnalysisKind) -> Job {
    crate::bg::BgJob::spawn("analysis-noop", |_ctl, _send: crate::bg::BgSend<AnalysisProgress>| {
        Err("no body".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Placeholder body: immediately returns the given result.
    fn done(out: Result<AnalysisOut, String>) -> Job {
        crate::bg::BgJob::spawn("t408", move |_ctl, _send: crate::bg::BgSend<f32>| out)
    }

    fn wait(
        a: &mut Analysis,
        out: Result<AnalysisOut, String>,
    ) -> Option<(AnalysisKind, u64, Result<AnalysisOut, String>)> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(r) = a.poll(|_| done(out.clone())) {
                return Some(r);
            }
            if std::time::Instant::now() > deadline {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Test 408 (R.49 B6): the “one task” gate and the queue with dedup.
    ///
    /// Analysis decodes files whole: three tasks at once would take the disk from
    /// playback, which promises real time (invariant 6). And a duplicate
    /// in the queue would mean a second full decode of the same file.
    #[test]
    fn t408_one_job_at_a_time_and_queue_dedups() {
        let mut a = Analysis::default();
        let k1 = AnalysisKind::Loudness(MediaId(1));
        let k2 = AnalysisKind::Loudness(MediaId(2));
        let body = |out: Result<AnalysisOut, String>| {
            move |_ctl: crate::bg::BgCtl, _s: crate::bg::BgSend<f32>| out
        };
        let ok = Ok(AnalysisOut::Loudness {
            media: MediaId(1),
            lufs: Some(-20.0),
            dbtp: -6.0,
            lra: 3.0,
        });
        a.request_with(k1, 1, false, body(ok.clone()));
        a.request_with(k1, 1, false, body(ok.clone())); // the same request is not work
        a.request_with(k2, 1, false, body(ok.clone()));
        assert_eq!(a.queued(), 2, "a duplicate got into the queue: {}", a.queued());

        // the first is taken into work, the second waits
        a.poll(|_| done(ok.clone()));
        assert_eq!(a.current(), Some(k1), "not the first task was taken into work");
        assert_eq!(a.queued(), 1, "both were taken into work at once — the gate did not trigger");

        let (kind, rev, r) = wait(&mut a, ok.clone()).expect("the first task did not finish");
        assert_eq!(kind, k1);
        assert_eq!(rev, 1, "task revision lost");
        assert!(r.is_ok());
        let (kind, _, _) = wait(&mut a, ok.clone()).expect("the second task did not finish");
        assert_eq!(kind, k2, "the queue did not reach the second task");
        assert!(!a.is_running() && a.queued() == 0);
        assert!(!a.open, "the progress window stayed open on an empty queue");
    }

    /// Test 409 (R.49 B6): negative cache. A task that failed is NOT
    /// re-queued by itself — otherwise the program would forever hammer the unavailable
    /// file; a retry happens only on direct request.
    #[test]
    fn t409_failure_is_remembered_and_not_retried_by_itself() {
        let mut a = Analysis::default();
        let k = AnalysisKind::Loudness(MediaId(3));
        let err = |_c: crate::bg::BgCtl, _s: crate::bg::BgSend<f32>| Err("no file".to_string());
        a.request_with(k, 1, false, err);
        let (_, _, r) = wait(&mut a, Err("no file".into())).expect("the task did not finish");
        assert!(r.is_err());
        assert_eq!(a.failure(k), Some("no file"), "the reason for refusal was not remembered");

        // does not re-queue itself
        a.request_with(k, 1, false, err);
        assert_eq!(a.queued(), 0, "the program went into the unavailable file again");

        // on direct request — it retries
        a.request_with(k, 1, true, err);
        assert_eq!(a.queued(), 1, "“try again” did not work");
        assert!(a.failure(k).is_none(), "negative cache not cleared by a retry");
    }

    /// Test 410 (R.49 B6): Cancel stops both the current task and the queue,
    /// and `is_running` reports WORK, not the presence of a window — otherwise
    /// repainting would not stop after finish (repeat of defect t38).
    #[test]
    fn t410_cancel_clears_everything_and_running_means_running() {
        let mut a = Analysis::default();
        assert!(!a.is_running(), "an empty subsystem counts as running");
        let spin = |ctl: crate::bg::BgCtl, _s: crate::bg::BgSend<f32>| {
            while !ctl.should_stop() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err::<AnalysisOut, String>("cancelled".into())
        };
        a.request_with(AnalysisKind::Loudness(MediaId(1)), 1, false, spin);
        a.request_with(AnalysisKind::Loudness(MediaId(2)), 1, false, spin);

        // a task that keeps running until it is cancelled
        a.poll(spawn_default);
        assert!(a.is_running(), "a started task does not count as running");

        a.cancel_all();
        assert_eq!(a.queued(), 0, "the queue survived the cancellation");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while a.is_running() && std::time::Instant::now() < deadline {
            a.poll(|_| done(Err("unused".into())));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!a.is_running(), "a cancelled task keeps counting as running");
    }
}

#[cfg(test)]
mod sidechain_tests {
    use super::*;

    /// Test 774 (R.65): DUCKING TASKS NO LONGER EXIST.
    ///
    /// The snapshot was removed by direct decision of the owner: the live sidechain is computed
    /// in the audio render thread as an ordinary node, and offline analysis has nothing to do here.
    /// The guard holds two things: a task kind “ducking” does not exist
    /// (compile lock — `AnalysisKind` is exhausted by loudness), and the task framework itself
    /// is alive and still serves loudness.
    #[test]
    fn t774_no_ducking_job_in_the_queue() {
        // compile lock: the only kind of offline analysis is loudness
        let k = AnalysisKind::Loudness(MediaId(1));
        match k {
            AnalysisKind::Loudness(m) => assert_eq!(m, MediaId(1)),
        }
        assert_eq!(k.media(), vec![MediaId(1)], "volume still names its own media");

        // and no owner of the snapshot is left in the sources
        let src = std::fs::read_to_string("apps/kt-app/src/app.rs")
            .or_else(|_| std::fs::read_to_string("src/app.rs"))
            .expect("source app.rs");
        for dead in ["duck_spans", "duck_curve_commands", "start_ducking", "duck_pending"] {
            assert!(!src.contains(dead), "a trace of the ducking snapshot remained in the program: {dead}");
        }
    }
}
