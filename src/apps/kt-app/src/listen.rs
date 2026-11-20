//! The audio-reactive generators' series (gen-audio-reactive, F-TXT-010): one background service that
//! computes the series of every file a listening generator hears and hands them to the engine's
//! registry (`kt_engine::listen`).
//!
//! A lane of class `Background` (one-scheduler): it pauses while a player plays or an export runs, and
//! at rest it runs nothing. Each file is asked once per project; the sidecar keeps what was computed,
//! so a reopened project reads it. A new series raises `fresh`, and the next frame resyncs the
//! player's generators — their feed then holds it and their frames move. Nothing here runs on a
//! pointer move: the editor asks after a model edit and at an opening.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

struct Task {
    db_path: Option<PathBuf>,
    /// (the project's media path — the registry's key, the resolved file path)
    files: Vec<(String, String)>,
}

pub struct ListenService {
    queue: Arc<Mutex<Vec<Task>>>,
    /// The project the asked set is about (its sidecar path).
    project: Mutex<(Option<PathBuf>, HashSet<String>)>,
    fresh: Arc<AtomicBool>,
    lane: kt_time::sched::LaneRef,
}

pub fn service() -> &'static ListenService {
    static S: OnceLock<ListenService> = OnceLock::new();
    S.get_or_init(ListenService::start)
}

impl ListenService {
    fn start() -> ListenService {
        let queue: Arc<Mutex<Vec<Task>>> = Arc::new(Mutex::new(Vec::new()));
        let fresh = Arc::new(AtomicBool::new(false));
        let (q, f) = (queue.clone(), fresh.clone());
        let db: Mutex<Option<(PathBuf, kt_media::index::MediaIndexDb)>> = Mutex::new(None);
        let lane = kt_time::sched::Lane::service("listen", kt_time::sched::Class::Background, move |ctx| {
            use kt_time::sched::Pump;
            let Some(task) = ({
                let mut q = q.lock().unwrap();
                (!q.is_empty()).then(|| q.remove(0))
            }) else {
                return Pump::Idle;
            };
            let mut db = db.lock().unwrap_or_else(|e| e.into_inner());
            if db.as_ref().map(|(p, _)| p) != task.db_path.as_ref() {
                *db = task.db_path.as_ref().and_then(|p| {
                    let _w = kt_time::block::wait(kt_time::block::kind::SIDECAR);
                    kt_media::index::MediaIndexDb::open(p).ok().map(|d| (p.clone(), d))
                });
            }
            for (key, file) in task.files {
                if ctx.cancelled() {
                    return Pump::Idle;
                }
                let stop = || ctx.checkpoint();
                let store = db.as_mut().map(|(_, d)| d as &mut dyn kt_media::listen::SeriesStore);
                let open = || crate::util::open_source(&file).ok();
                if let Some(s) = kt_media::listen::series_for(std::path::Path::new(&file), store, open, &stop) {
                    kt_engine::listen::put(&key, Arc::new(s));
                    f.store(true, Ordering::Release);
                    crate::wake::now();
                }
            }
            Pump::More
        });
        ListenService { queue, project: Mutex::new((None, HashSet::new())), fresh, lane }
    }

    /// Ask the series of `files` (key, resolved path) for the project of `db_path`; a file asked
    /// before for this project is not asked again. Another project forgets the registry.
    pub fn ask(&self, db_path: Option<PathBuf>, files: Vec<(String, String)>) {
        let mut pr = self.project.lock().unwrap();
        if pr.0 != db_path {
            *pr = (db_path.clone(), HashSet::new());
            kt_engine::listen::clear();
        }
        let files: Vec<(String, String)> = files.into_iter().filter(|(k, _)| pr.1.insert(k.clone())).collect();
        if files.is_empty() {
            return;
        }
        self.queue.lock().unwrap().push(Task { db_path, files });
        self.lane.kick();
    }

    /// A series arrived since the last call: the player's generators are to be resynced.
    pub fn take_fresh(&self) -> bool {
        self.fresh.swap(false, Ordering::AcqRel)
    }

    /// Wait until nothing is queued or running (tests).
    #[cfg(test)]
    pub(crate) fn wait_idle(&self) -> bool {
        self.lane.wait_idle(kt_time::test_deadline())
    }
}
