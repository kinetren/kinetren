//! The device times measured in earlier runs (gpu-throughput D2, found-defects 481): a pass or a
//! dispatch never measured in this process claimed the whole bound, so the first walk of a run was one
//! dispatch a piece. The arbiter predicts it from what this device measured before, kept on disk next
//! to the pipeline caches: one line per (device, label, work units) → ns.
//!
//! A cache invisible to bytes: a prediction moves where a frame or a job is cut, never what it computes
//! (t2253). Keyed by the adapter's name and driver, so a new driver starts empty; a stale line is
//! overwritten by the next measurement. No folder set (every test) — memory only.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The file's name inside the cache folder; the version names its format.
const FILE: &str = "pass-times-v1.tsv";

/// Saved at most this often while something changed.
const SAVE_EVERY: Duration = Duration::from_secs(30);

/// Lines kept at most: a label has a few sizes, a device a few hundred labels.
const MAX_LINES: usize = 8192;

type Sizes = Vec<(u64, u64)>;

#[derive(Default)]
pub struct Seeds {
    /// (device, label) → (work units, ns), sorted by units.
    map: HashMap<(String, String), Sizes>,
    lines: usize,
    dirty: bool,
    saved: Option<Instant>,
    path: Option<PathBuf>,
    loaded: bool,
}

/// The device's key: its name and driver.
pub fn device_key(info: &wgpu::AdapterInfo) -> String {
    format!("{} | {} {}", info.name, info.driver, info.driver_info).replace(['\t', '\n'], " ")
}

impl Seeds {
    /// A table reading and writing `path` (tests); `None` — memory only.
    pub fn at(path: Option<PathBuf>) -> Seeds {
        Seeds { path, ..Default::default() }
    }

    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let Some(text) = self.path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) else { return };
        for line in text.lines() {
            let mut f = line.split('\t');
            let (Some(d), Some(l), Some(u), Some(ns)) = (f.next(), f.next(), f.next(), f.next()) else { continue };
            let (Ok(u), Ok(ns)) = (u.parse::<u64>(), ns.parse::<u64>()) else { continue };
            self.put(d, l, u, ns);
        }
        self.dirty = false;
    }

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

    /// A measurement of `label` over `units` of work on `device`.
    pub fn note(&mut self, device: &str, label: &str, units: u64, ns: u64) {
        self.load();
        if self.put(device, label, units, ns) {
            self.dirty = true;
        }
    }

    /// What `label` over `units` of work is predicted to take on `device`: its measured time at that
    /// size, else the nearest measured size's scaled linearly by work (never below a smaller size's).
    pub fn predict(&mut self, device: &str, label: &str, units: u64) -> Option<u64> {
        self.load();
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

    /// Write the table when it changed and its last write is older than `SAVE_EVERY` (or `now`).
    pub fn save(&mut self, now: bool) {
        if !self.dirty || (!now && self.saved.is_some_and(|t| t.elapsed() < SAVE_EVERY)) {
            return;
        }
        self.saved = Some(Instant::now());
        let Some(path) = self.path.clone() else {
            self.dirty = false;
            return;
        };
        let mut out = String::new();
        let mut keys: Vec<&(String, String)> = self.map.keys().collect();
        keys.sort();
        for k in keys {
            for (u, ns) in &self.map[k] {
                out.push_str(&format!("{}\t{}\t{u}\t{ns}\n", k.0, k.1));
            }
        }
        // a whole file or the old one: written aside, then renamed over it
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, out).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
            self.dirty = false;
        }
    }
}

static SEEDS: Mutex<Option<Seeds>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Seeds) -> R) -> R {
    let mut g = SEEDS.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.get_or_insert_with(|| Seeds::at(crate::pipeline_cache::cache_dir().map(|d| d.join(FILE))));
    f(s)
}

/// The process's table: a measurement.
pub fn note(device: &str, label: &str, units: u64, ns: u64) {
    with(|s| {
        s.note(device, label, units, ns);
        s.save(false);
    });
}

/// The process's table: a prediction.
pub fn predict(device: &str, label: &str, units: u64) -> Option<u64> {
    with(|s| s.predict(device, label, units))
}

/// Write what changed now (an engine's drop, the process's end).
pub fn flush() {
    with(|s| s.save(true));
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
        let mut a = Seeds::at(Some(path.clone()));
        a.note("card | radv", "datamosh:search", 3600, 2_000_000);
        a.note("card | radv", "datamosh:search", 14_400, 7_000_000);
        a.note("card | radv", "vhs_tape:s4", 921_600, 5_000_000);
        a.save(true);
        let mut b = Seeds::at(Some(path.clone()));
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
        b.save(true);
        assert_eq!(Seeds::at(Some(path.clone())).predict("card | radv", "datamosh:search", 3600), Some(1_000_000));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
