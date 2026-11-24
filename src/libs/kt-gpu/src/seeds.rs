//! The device times measured in earlier runs (gpu-throughput D2, found-defects 481): a pass or a
//! dispatch never measured in this process claimed the whole bound, so the first walk of a run was one
//! dispatch a piece. The arbiter predicts it from what this device measured before, kept on disk next
//! to the pipeline caches: one line per (device, label, work units) → ns.
//!
//! A cache invisible to bytes: a prediction moves where a frame or a job is cut, never what it computes
//! (t2253). Keyed by the adapter's name and driver, so a new driver starts empty; a stale line is
//! overwritten by the next measurement. No folder set (every test) — memory only. Missed or late lines cost
//! cuts, so nobody waits for the file.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// The file's name inside the cache folder; the version names its format.
const FILE: &str = "pass-times-v1.tsv";

/// Saved at most this often while something changed.
const SAVE_EVERY: Duration = Duration::from_secs(30);

/// Lines kept at most: a label has a few sizes, a device a few hundred labels.
const MAX_LINES: usize = 8192;

type Sizes = Vec<(u64, u64)>;

/// The times themselves, in memory.
#[derive(Default)]
struct Table {
    /// (device, label) → (work units, ns), sorted by units.
    map: HashMap<(String, String), Sizes>,
    lines: usize,
    dirty: bool,
    saved: Option<Instant>,
    /// The file's lines are in the table (or there is no file).
    loaded: bool,
    /// The file thread runs.
    working: bool,
}

/// The device's key: its name and driver.
pub fn device_key(info: &wgpu::AdapterInfo) -> String {
    format!("{} | {} {}", info.name, info.driver, info.driver_info).replace(['\t', '\n'], " ")
}

impl Table {
    fn put(&mut self, device: &str, label: &str, units: u64, ns: u64) -> bool {
        let key = (device.to_string(), label.to_string());
        if !self.map.contains_key(&key) && self.lines >= MAX_LINES {
            return false;
        }
        let sizes = self.map.entry(key).or_default();
        match sizes.binary_search_by_key(&units, |s| s.0) {
            Ok(i) if sizes[i].1 == ns => return false,
            Ok(i) => sizes[i].1 = ns,
            Err(i) => {
                sizes.insert(i, (units, ns));
                self.lines += 1;
            }
        }
        true
    }

    /// A table of the file's lines.
    fn parsed(text: &str) -> Table {
        let mut t = Table::default();
        for line in text.lines() {
            let mut f = line.split('\t');
            let (Some(d), Some(l), Some(u), Some(ns)) = (f.next(), f.next(), f.next(), f.next()) else { continue };
            let (Ok(u), Ok(ns)) = (u.parse::<u64>(), ns.parse::<u64>()) else { continue };
            t.put(d, l, u, ns);
        }
        t
    }

    /// The file's table comes in under what this process measured meanwhile.
    fn take_file(&mut self, mut file: Table) {
        for ((d, l), sizes) in &self.map {
            for &(u, ns) in sizes {
                file.put(d, l, u, ns);
            }
        }
        (self.map, self.lines, self.loaded) = (file.map, file.lines, true);
    }

    fn predict(&self, device: &str, label: &str, units: u64) -> Option<u64> {
        let sizes = self.map.get(&(device.to_string(), label.to_string()))?;
        let i = sizes.partition_point(|s| s.0 < units);
        if let Some(s) = sizes.get(i).filter(|s| s.0 == units) {
            return Some(s.1);
        }
        let below = i.checked_sub(1).map(|j| sizes[j]);
        let above = sizes.get(i).copied();
        let scale = |(u, ns): (u64, u64)| if u == 0 { ns } else { (ns as u128 * units.max(1) as u128 / u as u128) as u64 };
        let near = match (below, above) {
            (Some(b), Some(a)) => {
                if units - b.0 <= a.0 - units { b } else { a }
            }
            (Some(b), None) => b,
            (None, Some(a)) => a,
            (None, None) => return None,
        };
        Some(scale(near).max(below.map_or(0, |b| b.1)))
    }

    fn lines(&self) -> Vec<((String, String), Sizes)> {
        self.map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

/// The table and its file. A frame asks and tells the table only; the file is read and written by a
/// short thread of its own, never with the table's lock held (gpu-waits-named): the write stood in a
/// composite under this lock, and on a busy disk two frames stood 4.8 s (found-defects 835).
pub struct Seeds {
    table: Mutex<Table>,
    /// Held across a read or a write of the file; never with the table's lock.
    file: Mutex<()>,
    path: Option<PathBuf>,
}

impl Seeds {
    /// A table reading and writing `path`; `None` — memory only.
    pub fn at(path: Option<PathBuf>) -> Arc<Seeds> {
        let table = Table { loaded: path.is_none(), ..Default::default() };
        Arc::new(Seeds { table: Mutex::new(table), file: Mutex::new(()), path })
    }

    fn table(&self) -> MutexGuard<'_, Table> {
        crate::holds::lock(&self.table, "pass times", None)
    }

    /// Read the file on a thread, if it is not in the table yet.
    pub fn open(self: &Arc<Seeds>) {
        let mut t = self.table();
        if !t.loaded {
            self.start(&mut t);
        }
    }

    /// The file thread: the read before anything else (a write over an unread file would drop its
    /// lines), then the write when one is due.
    fn start(self: &Arc<Seeds>, t: &mut Table) {
        if t.working || self.path.is_none() || crate::exit::exiting() {
            return;
        }
        t.working = true;
        let me = self.clone();
        let spawned = std::thread::Builder::new().name("pass-times".into()).spawn(move || {
            me.sync(false);
            me.table().working = false;
        });
        if spawned.is_err() {
            t.working = false;
        }
    }

    /// A measurement of `label` over `units` of work on `device`.
    pub fn note(self: &Arc<Seeds>, device: &str, label: &str, units: u64, ns: u64) {
        let mut t = self.table();
        if t.put(device, label, units, ns) {
            t.dirty = true;
        }
        if t.dirty && t.saved.is_none_or(|s| s.elapsed() >= SAVE_EVERY) {
            self.start(&mut t);
        }
    }

    /// What `label` over `units` of work is predicted to take on `device`: its measured time at that
    /// size, else the nearest measured size's scaled linearly by work (never below a smaller size's).
    /// The file not read yet — only what this process measured.
    pub fn predict(self: &Arc<Seeds>, device: &str, label: &str, units: u64) -> Option<u64> {
        let mut t = self.table();
        if !t.loaded {
            self.start(&mut t);
        }
        t.predict(device, label, units)
    }

    /// Bring the table and the file together: read it if nobody has, write it when the table changed
    /// and its last write is older than `SAVE_EVERY` (or `now`).
    pub fn sync(&self, now: bool) {
        let Some(path) = &self.path else { return };
        let _file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        if !self.table().loaded {
            let text = crate::holds::file("pass times read", || std::fs::read_to_string(path).unwrap_or_default());
            let file = Table::parsed(&text);
            self.table().take_file(file);
        }
        let mut lines = {
            let mut t = self.table();
            if !t.dirty || (!now && t.saved.is_some_and(|s| s.elapsed() < SAVE_EVERY)) {
                return;
            }
            t.saved = Some(Instant::now());
            t.dirty = false;
            t.lines()
        };
        lines.sort();
        let mut out = String::new();
        for ((device, label), sizes) in lines {
            for (u, ns) in sizes {
                out.push_str(&format!("{device}\t{label}\t{u}\t{ns}\n"));
            }
        }
        // a whole file or the old one: written aside, then renamed over it
        let tmp = path.with_extension("tmp");
        let written = crate::holds::file("pass times write", || std::fs::write(&tmp, out).is_ok() && std::fs::rename(&tmp, path).is_ok());
        if !written {
            self.table().dirty = true;
        }
    }
}

static SEEDS: OnceLock<Arc<Seeds>> = OnceLock::new();

fn seeds() -> &'static Arc<Seeds> {
    SEEDS.get_or_init(|| Seeds::at(crate::pipeline_cache::cache_dir().map(|d| d.join(FILE))))
}

/// The cache folder is known: the file is read before the first frame asks.
pub fn open() {
    seeds().open();
}

/// The process's table: a measurement.
pub fn note(device: &str, label: &str, units: u64, ns: u64) {
    seeds().note(device, label, units, ns);
}

/// The process's table: a prediction.
pub fn predict(device: &str, label: &str, units: u64) -> Option<u64> {
    seeds().predict(device, label, units)
}

/// Write what changed now (the process's end).
pub fn flush() {
    seeds().sync(true);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test t2434 (gpu-throughput D2, found-defects 481): a new process predicts what an earlier one
    /// measured on the same device; a size never measured is scaled from the nearest measured one; another
    /// device asks in vain.
    #[test]
    fn t2434_a_new_process_predicts_what_an_earlier_one_measured() {
        let dir = std::env::temp_dir().join(format!("kt-seeds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE);
        let _ = std::fs::remove_file(&path);
        let a = Seeds::at(Some(path.clone()));
        a.note("card | radv", "datamosh:search", 3600, 2_000_000);
        a.note("card | radv", "datamosh:search", 14_400, 7_000_000);
        a.note("card | radv", "vhs_tape:s4", 921_600, 5_000_000);
        a.sync(true);
        let b = Seeds::at(Some(path.clone()));
        b.sync(false);
        assert_eq!(b.predict("card | radv", "datamosh:search", 3600), Some(2_000_000));
        assert_eq!(b.predict("card | radv", "vhs_tape:s4", 921_600), Some(5_000_000));
        // between the two sizes: the nearer one scaled by work, never below the smaller one's
        assert_eq!(b.predict("card | radv", "datamosh:search", 5400), Some(3_000_000));
        assert_eq!(b.predict("card | radv", "datamosh:search", 28_800), Some(14_000_000));
        assert_eq!(b.predict("card | radv", "datamosh:search", 100), Some(55_555));
        assert_eq!(b.predict("other | gl", "datamosh:search", 3600), None);
        assert_eq!(b.predict("card | radv", "datamosh:decode", 3600), None);
        // a new measurement overwrites the line
        b.note("card | radv", "datamosh:search", 3600, 1_000_000);
        b.sync(true);
        let c = Seeds::at(Some(path.clone()));
        c.sync(false);
        assert_eq!(c.predict("card | radv", "datamosh:search", 3600), Some(1_000_000));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Test t3250 (gpu-waits-named, found-defects 835): a frame neither waits for the file nor for a
    /// thread at it. The file is a pipe nobody reads (then one nobody writes): the write (the read)
    /// stands for as long as the test likes, and meanwhile measurements and predictions go on.
    #[cfg(unix)]
    #[test]
    fn t3250_a_standing_disk_holds_no_frame() {
        let dir = std::env::temp_dir().join(format!("kt-t3250-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fifo = |p: &std::path::Path| assert!(std::process::Command::new("mkfifo").arg(p).status().unwrap().success());
        let deadline = kt_time::test_deadline();
        let until = |what: &str, f: &dyn Fn() -> bool| {
            while !f() {
                assert!(Instant::now() < deadline, "{what}");
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        // the frame's half runs on a thread: were it to wait, the channel would stay silent
        let answers = |s: &Arc<Seeds>, ns: u64| {
            let (tx, rx) = std::sync::mpsc::channel();
            let s = s.clone();
            std::thread::spawn(move || {
                s.note("card", "glow:p0", 100, ns);
                let _ = tx.send(s.predict("card", "glow:p0", 100));
            });
            rx.recv_timeout(kt_time::TEST_WAIT).expect("a frame stood on the file")
        };

        let path = dir.join("write.tsv");
        std::fs::write(&path, "card\told:p0\t7\t70\n").unwrap();
        let tmp = path.with_extension("tmp");
        fifo(&tmp);
        let a = Seeds::at(Some(path.clone()));
        assert_eq!(answers(&a, 1), Some(1));
        until("the write never began", &|| a.file.try_lock().is_err() && a.table().saved.is_some());
        assert_eq!(answers(&a, 2), Some(2));
        assert_eq!(a.predict("card", "old:p0", 7), Some(70), "the file was read before it was written over");
        let written = std::fs::read_to_string(&tmp).unwrap();
        assert!(written.contains("old:p0\t7\t70") && written.contains("glow:p0\t100\t1"), "{written}");
        until("the file thread stayed", &|| !a.table().working);

        let path = dir.join("read.tsv");
        fifo(&path);
        let b = Seeds::at(Some(path.clone()));
        b.open();
        assert_eq!(answers(&b, 3), Some(3));
        std::fs::write(&path, "card\tglow:p0\t100\t9\ncard\tother:p0\t5\t50\n").unwrap();
        until("the file never came in", &|| b.predict("card", "other:p0", 5) == Some(50));
        assert_eq!(b.predict("card", "glow:p0", 100), Some(3), "what this process measured stands over the file");
        until("the file thread stayed", &|| !b.table().working);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
