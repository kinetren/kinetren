//! Small files of the program's own written off the UI thread (found-defects 833): the workspace, the
//! recent list, the tour's progress, the start choice. A save, an open or a lesson wrote them in its
//! frame, and one write on a busy disk stands for seconds. The newest text of a path replaces one still
//! waiting; `read` answers with what is on its way, so a re-read sees what was just saved.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

type WriteFn = Box<dyn FnMut(&Path, &str, bool) + Send>;

#[derive(Default)]
struct St {
    /// In the order of the first ask: path, text, whether it must survive a power cut.
    queue: Vec<(PathBuf, String, bool)>,
    /// What the thread is writing now.
    at: Option<(PathBuf, String)>,
}

pub(crate) struct Later {
    st: Mutex<St>,
    cv: Condvar,
}

impl Later {
    pub(crate) fn start(mut write: WriteFn) -> Arc<Later> {
        let l = Arc::new(Later { st: Default::default(), cv: Condvar::new() });
        let me = l.clone();
        let _ = std::thread::Builder::new().name("small-files".into()).spawn(move || {
            loop {
                let (path, text, sync) = {
                    let mut st = me.st.lock().unwrap_or_else(|e| e.into_inner());
                    st.at = None;
                    me.cv.notify_all();
                    while st.queue.is_empty() {
                        st = me.cv.wait(st).unwrap_or_else(|e| e.into_inner());
                    }
                    let d = st.queue.remove(0);
                    st.at = Some((d.0.clone(), d.1.clone()));
                    d
                };
                write(&path, &text, sync);
            }
        });
        l
    }

    pub(crate) fn push(&self, path: &Path, text: String, sync: bool) {
        let mut st = self.st.lock().unwrap_or_else(|e| e.into_inner());
        match st.queue.iter_mut().find(|q| q.0 == path) {
            Some(q) => (q.1, q.2) = (text, q.2 || sync),
            None => st.queue.push((path.to_path_buf(), text, sync)),
        }
        self.cv.notify_all();
    }

    pub(crate) fn pending(&self, path: &Path) -> Option<String> {
        let st = self.st.lock().unwrap_or_else(|e| e.into_inner());
        let queued = st.queue.iter().find(|q| q.0 == path).map(|q| q.1.clone());
        queued.or_else(|| st.at.as_ref().filter(|a| a.0 == path).map(|a| a.1.clone()))
    }

    /// Wait until what was asked is written, at most `bound`; `false` — it is not.
    pub(crate) fn flush(&self, bound: Duration) -> bool {
        let st = self.st.lock().unwrap_or_else(|e| e.into_inner());
        let waits = |s: &mut St| s.at.is_some() || !s.queue.is_empty();
        let (mut st, _) = self.cv.wait_timeout_while(st, bound, waits).unwrap_or_else(|e| e.into_inner());
        !waits(&mut st)
    }
}

fn the() -> &'static Arc<Later> {
    static L: OnceLock<Arc<Later>> = OnceLock::new();
    L.get_or_init(|| Later::start(Box::new(put)))
}

/// The file itself: tmp + rename, with `sync` by the project's own atomic write.
fn put(path: &Path, text: &str, sync: bool) {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if sync {
        let _ = kt_otio::write_atomic(path, text);
        return;
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = kt_otio::replace_file(&tmp, path);
    }
}

/// Write `text` into `path` soon; a test's is in the file at once.
pub(crate) fn write(path: &Path, text: String, sync: bool) {
    if cfg!(test) {
        put(path, &text, sync);
    } else {
        the().push(path, text, sync);
    }
}

/// The text of `path`: what is on its way there, else the file's.
pub(crate) fn read(path: &Path) -> std::io::Result<String> {
    let pending = if cfg!(test) { None } else { the().pending(path) };
    pending.map_or_else(|| std::fs::read_to_string(path), Ok)
}

pub(crate) fn flush(bound: Duration) -> bool {
    cfg!(test) || the().flush(bound)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 3728 (found-defects 833): a small file is asked for while the disk answers nothing; a read
    /// meanwhile sees the text on its way, a newer text replaces one still waiting, and the end waits no
    /// longer than its bound.
    #[test]
    fn t3728_a_small_file_waits_for_no_disk() {
        let disk = Arc::new((Mutex::new(false), Condvar::new()));
        let wrote = Arc::new(Mutex::new(Vec::<(PathBuf, String, bool)>::new()));
        let l = {
            let (disk, wrote) = (disk.clone(), wrote.clone());
            Later::start(Box::new(move |p, t, s| {
                let mut free = disk.0.lock().unwrap();
                while !*free {
                    free = disk.1.wait(free).unwrap();
                }
                wrote.lock().unwrap().push((p.to_path_buf(), t.to_string(), s));
            }))
        };
        let (a, b) = (Path::new("a.json"), Path::new("b.json"));
        l.push(b, "b0".into(), false);
        // the thread holds `b0` in its hands: what follows waits in the queue
        let deadline = kt_time::test_deadline();
        while l.st.lock().unwrap().at.is_none() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        l.push(a, "a1".into(), true);
        l.push(a, "a2".into(), false);
        assert_eq!(l.pending(a).as_deref(), Some("a2"));
        assert_eq!(l.pending(b).as_deref(), Some("b0"), "the one being written is read too");
        assert!(!l.flush(Duration::ZERO) && wrote.lock().unwrap().is_empty());
        *disk.0.lock().unwrap() = true;
        disk.1.notify_all();
        assert!(l.flush(kt_time::TEST_WAIT));
        let want = vec![(b.to_path_buf(), "b0".to_string(), false), (a.to_path_buf(), "a2".to_string(), true)];
        assert_eq!(*wrote.lock().unwrap(), want, "the newest text, synced since the older one had to be");
        assert_eq!(l.pending(a), None);
    }
}
