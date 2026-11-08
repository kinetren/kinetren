//! The compute door (compute-kernels, F-EXT-004): a kernel names WGSL compute shaders and buffers
//! as one [`Job`]; the host runs it on its own device and hands the named buffers back. A plugin
//! sees nothing of wgpu. The bindings of a step's shader are fixed: 0 is the uniform, then `read`
//! as read-only storage, then `write` as read-write storage.

use std::sync::{Arc, Mutex, OnceLock};

/// A buffer of one job: an index into [`Job::buffers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Buf(pub usize);

/// How a job's buffer starts.
#[derive(Debug, Clone, Copy)]
pub enum Init<'a> {
    Bytes(&'a [u8]),
    /// That many zero bytes.
    Zero(u64),
}

/// One dispatch.
#[derive(Debug, Clone, Copy)]
pub struct Step<'a> {
    /// What the telemetry journal's GPU record calls this dispatch (`datamosh:search`).
    pub name: &'a str,
    pub shader: &'a str,
    pub uniform: &'a [u8],
    pub read: &'a [Buf],
    pub write: &'a [Buf],
    pub groups: [u32; 3],
}

/// The steps run in order; `reads` come back in order.
#[derive(Debug, Clone, Copy)]
pub struct Job<'a> {
    pub buffers: &'a [Init<'a>],
    pub steps: &'a [Step<'a>],
    pub reads: &'a [Buf],
}

/// The host's compute device. `run` blocks until the buffers are back; an error names what the
/// device refused and never panics.
pub trait ComputeHost: Send + Sync {
    fn run(&self, job: &Job<'_>) -> Result<Vec<Vec<u8>>, String>;
}

type Open = fn() -> Result<Arc<dyn ComputeHost>, String>;

static OPEN: OnceLock<Open> = OnceLock::new();
static HOST: OnceLock<Result<Arc<dyn ComputeHost>, String>> = OnceLock::new();
static FAILED: Mutex<Option<String>> = Mutex::new(None);

/// Name how the process opens its device, once; it is opened at the first use, so a project with no
/// compute kernel never creates one.
pub fn install(open: Open) {
    let _ = OPEN.set(open);
}

/// The device, or `None` — none was installed, none opened, or a job failed on it.
pub fn host() -> Option<&'static Arc<dyn ComputeHost>> {
    if FAILED.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
        return None;
    }
    HOST.get_or_init(|| OPEN.get().map_or(Err("no compute host".into()), |open| open())).as_ref().ok()
}

/// Mark the device unusable for this run: a kernel calls it when a job fails, so the next frame
/// takes the kernel's no-device path and [`unavailable`] says why.
pub fn fail(reason: String) {
    *FAILED.lock().unwrap_or_else(|e| e.into_inner()) = Some(reason);
}

/// Why no kernel can compute, or `None` while the device works.
pub fn unavailable() -> Option<String> {
    if let Some(r) = FAILED.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Some(r);
    }
    host().is_none().then(|| HOST.get().and_then(|h| h.as_ref().err().cloned()).unwrap_or_default())
}
