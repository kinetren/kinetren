//! The disk tier of compiled pipelines (pipeline-prewarm D7): a wgpu `PipelineCache` per device where
//! the device has `Features::PIPELINE_CACHE` (wgpu offers it on Vulkan), read from and written to
//! `<dir>/<key>.bin`. Speed only: the driver validates the blob (a foreign or stale one is ignored,
//! `fallback: true`) and the frames are the same bytes with and without it (t2125).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// The folder the program keeps its pipeline caches in (the editor's and kt-cli's data folder). Not
/// set — no disk tier: tests set none, so no test writes into the tree or the home folder.
pub fn set_cache_dir(dir: PathBuf) {
    let _ = DIR.set(dir);
}

/// The wgpu the blobs come from: the workspace pin (src/Cargo.toml). wgpu itself keeps its format
/// compatible across versions; the key changes with it anyway, so an upgrade starts a fresh file.
const WGPU: &str = "wgpu-30";
/// The newest files the folder keeps.
const KEEP: usize = 4;
/// A blob above this is not written.
const MAX_BYTES: usize = 64 << 20;

pub(crate) struct DiskCache {
    cache: wgpu::PipelineCache,
    path: PathBuf,
    /// The size of what the file holds: a blob that did not grow is not written again.
    saved: AtomicUsize,
}

/// The file name: what decides whether a blob is valid — the adapter (vendor and device), the
/// driver and its version, wgpu, which part of the program (`role`) and every shader text it compiles.
fn key(info: &wgpu::AdapterInfo, role: &str, shaders: u64) -> Option<String> {
    let adapter = wgpu::util::pipeline_cache_key(info)?;
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    for part in [adapter.as_str(), &info.name, &info.driver, &info.driver_info, WGPU, role] {
        h.update(part.as_bytes());
        h.update(&[0]);
    }
    h.update(&shaders.to_le_bytes());
    Some(format!("{role}-{:016x}", h.digest()))
}

impl DiskCache {
    /// The device's cache, or `None` — no folder set, no feature, or a backend wgpu keys no cache for.
    pub(crate) fn open(device: &wgpu::Device, info: &wgpu::AdapterInfo, role: &str, shaders: u64) -> Option<DiskCache> {
        let dir = DIR.get()?;
        if !device.features().contains(wgpu::Features::PIPELINE_CACHE) {
            return None;
        }
        let path = dir.join(format!("{}.bin", key(info, role, shaders)?));
        let data = std::fs::read(&path).ok();
        // SAFETY: the data was written by `save` from `PipelineCache::get_data` of an adapter with
        // the same `pipeline_cache_key` (it is part of the file name); `fallback` drops it otherwise
        let cache = unsafe {
            device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                label: Some("kt-gpu pipeline cache"),
                data: data.as_deref(),
                fallback: true,
            })
        };
        Some(DiskCache { cache, path, saved: AtomicUsize::new(data.map_or(0, |d| d.len())) })
    }

    pub(crate) fn get(&self) -> &wgpu::PipelineCache {
        &self.cache
    }

    /// Write the blob when it grew: a temporary file and a rename, then the folder trimmed to the
    /// newest files.
    pub(crate) fn save(&self) {
        let Some(data) = self.cache.get_data() else { return };
        if data.len() <= self.saved.load(Ordering::Relaxed) || data.len() > MAX_BYTES {
            return;
        }
        let Some(dir) = self.path.parent() else { return };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let tmp = self.path.with_extension(format!("tmp{}", std::process::id()));
        if std::fs::write(&tmp, &data).is_ok() && std::fs::rename(&tmp, &self.path).is_ok() {
            self.saved.store(data.len(), Ordering::Relaxed);
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
        trim(dir);
    }
}

fn trim(dir: &std::path::Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "bin"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, p) in files.into_iter().skip(KEEP) {
        let _ = std::fs::remove_file(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(driver_info: &str) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: "llvmpipe".into(),
            vendor: 0x10005,
            device: 0,
            device_type: wgpu::DeviceType::Cpu,
            device_pci_bus_id: String::new(),
            driver: "llvmpipe".into(),
            driver_info: driver_info.into(),
            backend: wgpu::Backend::Vulkan,
            subgroup_min_size: 4,
            subgroup_max_size: 64,
            transient_saves_memory: None,
            limit_bucket: None,
        }
    }

    /// Test t2126 (pipeline-prewarm D7): the file a blob goes to changes with the driver's version,
    /// the shader texts and the part of the program, so a stale blob is never read; a backend wgpu
    /// keys no cache for (GL) has none; the folder keeps the four newest files.
    #[test]
    fn t2126_the_key_names_what_decides_validity_and_the_folder_is_bounded() {
        let a = key(&info("Mesa 25.0.7"), "engine", 1).unwrap();
        assert_ne!(a, key(&info("Mesa 25.1.0"), "engine", 1).unwrap(), "a driver update");
        assert_ne!(a, key(&info("Mesa 25.0.7"), "engine", 2).unwrap(), "another shader text");
        assert_ne!(a, key(&info("Mesa 25.0.7"), "compute", 1).unwrap(), "another part");
        assert_eq!(a, key(&info("Mesa 25.0.7"), "engine", 1).unwrap(), "the same inputs");
        assert_eq!(key(&wgpu::AdapterInfo { backend: wgpu::Backend::Gl, ..info("x") }, "engine", 1), None);

        let dir = std::env::temp_dir().join(format!("kt-t2126-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let now = std::time::SystemTime::now();
        for i in 0..7u64 {
            let p = dir.join(format!("engine-{i}.bin"));
            std::fs::write(&p, [0u8; 8]).unwrap();
            let f = std::fs::File::options().write(true).open(&p).unwrap();
            f.set_modified(now - std::time::Duration::from_secs(100 - i)).unwrap();
        }
        std::fs::write(dir.join("other.txt"), b"kept").unwrap();
        trim(&dir);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["engine-3.bin", "engine-4.bin", "engine-5.bin", "engine-6.bin", "other.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
