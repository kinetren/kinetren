//! Ownership of the wgpu device. Two ways of creating it:
//! - `from_parts` — preview: Arc handles of the LIVE eframe device
//!   (egui_wgpu::RenderState); there is no separate adapter for the preview;
//! - `new_headless` — export/kt-cli/tests: its own adapter without a surface,
//!   the limits repeat the egui-wgpu rule (downlevel_webgl2 on GL, otherwise
//!   default; max_texture_dimension_2d 8192): shader behaviour in the GUI
//!   and in headless MUST match.
//!
//! Every headless device, the compute host and the adapter list come from ONE `wgpu::Instance` per
//! backend set (`instance`), opened under one lock (`open_lock`), and the instances are never
//! dropped (segv-crashes, found-defects 131): the Vulkan loader's instance-level entry points are not
//! safe to race — `export_engines` opened two to four instances on threads at once and the process
//! died in `vkEnumerateInstanceExtensionProperties` beside a `vkCreateInstance` (row 131, every run on
//! a software adapter) — and the load watch created and destroyed a whole instance (every ICD loaded,
//! every driver's config parsed) each five seconds of a hot spell, beside the preview's device.
//!
//! The one exception is a device on the GL backend (`WGPU_BACKEND=gl`, a machine without Vulkan):
//! wgpu's GL adapters of one instance share that instance's ONE EGL context, so two devices of one
//! instance make it current on two threads and the second dies with EGL_BAD_ACCESS (t2088: every
//! render from the editor under the live scenarios' GL died in `GpuFxEngine::build` beside the
//! offline engine). A GL device therefore opens from an instance of its own (`own_instance`), still
//! under the opening lock; wgpu keeps it alive as long as its device, and EGL counts the display's
//! references, so dropping it with its device ends only its own context.

use std::sync::{Arc, Mutex, MutexGuard};

/// Held while a device is opened or the adapters are listed: one opening at a time in the process.
pub(crate) type Opening = MutexGuard<'static, ()>;

pub(crate) fn opening() -> Opening {
    static OPEN: Mutex<()> = Mutex::new(());
    OPEN.lock().unwrap_or_else(|e| e.into_inner())
}

/// The process's instance for `backends` (the environment's descriptor otherwise, `WGPU_BACKEND`
/// respected), created at the first ask under the opening lock and kept for the process's life: an
/// instance is the loader and every driver it loaded, and destroying one at exit races the threads
/// still on its devices.
pub(crate) fn instance(_opening: &Opening, backends: wgpu::Backends) -> &'static wgpu::Instance {
    static ALL: Mutex<Vec<(wgpu::Backends, &'static wgpu::Instance)>> = Mutex::new(Vec::new());
    let mut all = ALL.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, i)) = all.iter().find(|(b, _)| *b == backends) {
        return i;
    }
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    desc.backends = backends;
    let i: &'static wgpu::Instance = Box::leak(Box::new(wgpu::Instance::new(desc)));
    INSTANCES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    all.push((backends, i));
    i
}

/// An instance for ONE GL device: GL adapters of an instance share its EGL context (see the module
/// doc). Not kept: it lives as long as the device opened from it.
fn own_instance(_opening: &Opening) -> wgpu::Instance {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    desc.backends = wgpu::Backends::GL;
    wgpu::Instance::new(desc)
}

static INSTANCES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Shared instances this process created (t2087): one per backend set, however many devices it
/// opened. A GL device's own instance is not one of them.
#[doc(hidden)]
pub fn instances_created() -> usize {
    INSTANCES.load(std::sync::atomic::Ordering::Relaxed)
}

/// The environment's backends (`WGPU_BACKEND`), what `new_without_display_handle_from_env` takes.
pub(crate) fn env_backends() -> wgpu::Backends {
    wgpu::InstanceDescriptor::new_without_display_handle_from_env().backends
}

#[derive(Debug)]
pub enum GpuInitError {
    NoAdapter(String),
    Device(String),
    Pipeline(String),
    /// An adapter the engine does not run on (Windows' WARP software rasterizer).
    Unsupported(String),
}

impl std::fmt::Display for GpuInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuInitError::NoAdapter(e) => write!(f, "no wgpu adapter: {e}"),
            GpuInitError::Unsupported(e) => write!(f, "unsupported adapter: {e}"),
            GpuInitError::Device(e) => write!(f, "wgpu device: {e}"),
            GpuInitError::Pipeline(e) => write!(f, "pipeline: {e}"),
        }
    }
}

pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub adapter_info: wgpu::AdapterInfo,
}

/// What the trace says of a device a role opened (structured-telemetry): the adapter, its driver,
/// the features and limits the device got — so a design never needs the person's machine tuned by
/// hand: what it ran on is in the run's own record.
pub fn trace_device(role: &str, info: &wgpu::AdapterInfo, device: &wgpu::Device) {
    if !kt_time::trace::on() {
        return;
    }
    let l = device.limits();
    let f = device.features();
    let m = |k: &str, v: String| kt_time::trace::machine(&format!("gpu.{role}.{k}"), &v);
    m("adapter", info.name.clone());
    m("type", format!("{:?}", info.device_type));
    m("backend", format!("{:?}", info.backend));
    m("driver", format!("{} {}", info.driver, info.driver_info));
    m("pci", format!("{:04x}:{:04x}", info.vendor, info.device));
    m("timestamps", f.contains(wgpu::Features::TIMESTAMP_QUERY).to_string());
    m("pipeline_cache", f.contains(wgpu::Features::PIPELINE_CACHE).to_string());
    m("features", format!("{f:?}"));
    m(
        "limits",
        format!(
            "tex2d {} buffer {} storage {} workgroup {}x{}x{} invocations {}",
            l.max_texture_dimension_2d,
            l.max_buffer_size,
            l.max_storage_buffer_binding_size,
            l.max_compute_workgroup_size_x,
            l.max_compute_workgroup_size_y,
            l.max_compute_workgroup_size_z,
            l.max_compute_invocations_per_workgroup
        ),
    );
    if let Ok(b) = std::env::var("WGPU_BACKEND") {
        m("env_backend", b);
    }
}

impl GpuContext {
    /// The eframe device (preview): handles from `egui_wgpu::RenderState`.
    pub fn from_parts(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        adapter_info: wgpu::AdapterInfo,
    ) -> GpuContext {
        trace_device("display", &adapter_info, &device);
        GpuContext { device, queue, adapter_info }
    }

    /// Headless device (kt-cli probes, tests, previews). PowerPreference is
    /// the same as in the GUI: `display_power_preference`.
    pub fn new_headless() -> Result<GpuContext, GpuInitError> {
        Self::headless_with(display_power_preference())
    }

    /// The device of the D3D12 backend, whatever adapter it is; `None` where there is none.
    pub fn new_d3d12() -> Option<GpuContext> {
        let g = opening();
        let instance = instance(&g, wgpu::Backends::DX12);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        let info = adapter.get_info();
        let limits = wgpu::Limits { max_texture_dimension_2d: 8192, ..wgpu::Limits::default() };
        let (device, queue) = pollster::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: crate::pass_time::wanted_features(&adapter),
                required_limits: limits,
                ..Default::default()
            }),
        )
        .ok()?;
        Some(GpuContext { device: Arc::new(device), queue: Arc::new(queue), adapter_info: info })
    }

    /// The EXPORT's headless device (export-throughput D5): WGPU_POWER_PREF or HighPerformance.
    /// LowPower keeps the UI from heating the machine (invariant 12, PRIME item 39) — a cost paid
    /// every frame a person looks at. An export is a finite job the person WAITS for, on its own
    /// device; on a hybrid laptop the discrete card is the one built for it.
    pub fn new_headless_for_export() -> Result<GpuContext, GpuInitError> {
        Self::headless_with(wgpu::PowerPreference::HighPerformance)
    }

    fn headless_with(default: wgpu::PowerPreference) -> Result<GpuContext, GpuInitError> {
        let g = opening();
        let options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::from_env().unwrap_or(default),
            ..Default::default()
        };
        let request = |i: &wgpu::Instance| {
            pollster::block_on(i.request_adapter(&options)).map_err(|e| GpuInitError::NoAdapter(e.to_string()))
        };
        let mut adapter = request(instance(&g, env_backends()))?;
        if adapter.get_info().backend == wgpu::Backend::Gl {
            adapter = request(&own_instance(&g))?;
        }
        let base_limits = if adapter.get_info().backend == wgpu::Backend::Gl {
            wgpu::Limits::downlevel_webgl2_defaults()
        } else {
            wgpu::Limits::default()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("kt-gpu headless"),
            // the pass timer's timestamps where the adapter has them (gpu-pass-cost)
            required_features: crate::pass_time::wanted_features(&adapter),
            required_limits: wgpu::Limits { max_texture_dimension_2d: 8192, ..base_limits },
            ..Default::default()
        }))
        .map_err(|e| GpuInitError::Device(e.to_string()))?;
        let role = if default == wgpu::PowerPreference::HighPerformance { "export" } else { "headless" };
        trace_device(role, &adapter.get_info(), &device);
        Ok(GpuContext {
            device: Arc::new(device),
            queue: Arc::new(queue),
            adapter_info: adapter.get_info(),
        })
    }
}

/// One device as the choice sees it: wgpu's facts and the PCI ids sysfs is matched by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterFact {
    pub name: String,
    pub kind: wgpu::DeviceType,
    pub backend: wgpu::Backend,
    pub vendor: u32,
    pub device: u32,
}

/// A card in `/sys/class/drm` that has a monitor plugged in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrivingCard {
    pub vendor: u32,
    pub device: u32,
}

/// Which card the UI and the work that is not an export run on (idle-cpu). LowPower stays the
/// default: on hybrid graphics the discrete card copies every frame over PRIME and the whole
/// system stutters (item 39). But PRIME is about the card that does NOT drive the monitor: on a
/// desktop whose monitors hang off a discrete card, LowPower picks the processor's integrated
/// GPU (the owner's panel read "AMD Radeon Graphics (RADV RAPHAEL_MENDOCINO)"), every effect runs
/// on it, and each frame is copied to the card that shows it. So: a discrete card alone drives
/// the monitors → that card. A pure function; t1837.
pub fn power_preference_for(adapters: &[AdapterFact], driving: &[DrivingCard]) -> wgpu::PowerPreference {
    let kind_of = |c: &DrivingCard| {
        adapters.iter().find(|a| a.vendor == c.vendor && a.device == c.device).map(|a| a.kind)
    };
    let discrete = driving.iter().any(|c| kind_of(c) == Some(wgpu::DeviceType::DiscreteGpu));
    let integrated = driving.iter().any(|c| kind_of(c) != Some(wgpu::DeviceType::DiscreteGpu));
    if discrete && !integrated {
        wgpu::PowerPreference::HighPerformance
    } else {
        wgpu::PowerPreference::LowPower
    }
}

/// Every device this machine offers: the load report names them, so that "which card did it run
/// on" is a fact in the file, not a question to the person. Listed once per process: the load
/// watch asks at every hot spell, and each listing once loaded every driver anew (segv-crashes).
pub fn adapter_facts() -> Vec<AdapterFact> {
    static FACTS: std::sync::OnceLock<Vec<AdapterFact>> = std::sync::OnceLock::new();
    FACTS
        .get_or_init(|| {
            let g = opening();
            pollster::block_on(instance(&g, env_backends()).enumerate_adapters(wgpu::Backends::all()))
                .iter()
                .map(|a| {
                    let i = a.get_info();
                    AdapterFact { name: i.name, kind: i.device_type, backend: i.backend, vendor: i.vendor, device: i.device }
                })
                .collect()
        })
        .clone()
}

/// The cards with a monitor plugged in. Empty where sysfs has no such thing (Windows): the
/// choice then stays LowPower, and Windows' own per-application setting decides.
pub fn driving_cards() -> Vec<DrivingCard> {
    driving_cards_in(std::path::Path::new("/sys/class/drm"))
}

fn driving_cards_in(root: &std::path::Path) -> Vec<DrivingCard> {
    let hex = |p: std::path::PathBuf| -> Option<u32> {
        let t = std::fs::read_to_string(p).ok()?;
        u32::from_str_radix(t.trim().trim_start_matches("0x"), 16).ok()
    };
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir(root) else { return out };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        // `card1-HDMI-A-1`, a connector of `card1`
        let Some((card, _)) = name.split_once('-') else { continue };
        let connected = std::fs::read_to_string(e.path().join("status"))
            .is_ok_and(|s| s.trim() == "connected");
        if !connected {
            continue;
        }
        let dev = root.join(card).join("device");
        if let (Some(vendor), Some(device)) = (hex(dev.join("vendor")), hex(dev.join("device"))) {
            let c = DrivingCard { vendor, device };
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

/// `power_preference_for` on this machine, once per process; WGPU_POWER_PREF overrides it.
pub fn display_power_preference() -> wgpu::PowerPreference {
    static CHOICE: std::sync::OnceLock<wgpu::PowerPreference> = std::sync::OnceLock::new();
    *CHOICE.get_or_init(|| {
        wgpu::PowerPreference::from_env().unwrap_or_else(|| {
            let driving = driving_cards();
            if driving.is_empty() {
                return wgpu::PowerPreference::LowPower;
            }
            power_preference_for(&adapter_facts(), &driving)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(kind: wgpu::DeviceType, vendor: u32, device: u32) -> AdapterFact {
        AdapterFact { name: String::new(), kind, backend: wgpu::Backend::Vulkan, vendor, device }
    }

    /// Test t1837 (idle-cpu): the card that drives the monitor gets the work. A desktop's discrete
    /// card with the monitors on it wins over the processor's integrated GPU; hybrid graphics keep
    /// LowPower, or every frame goes over PRIME again (item 39); no facts change nothing.
    #[test]
    fn t1837_the_card_with_the_monitor_gets_the_work() {
        use wgpu::{DeviceType::*, PowerPreference::*};
        let (raphael, big) = (gpu(IntegratedGpu, 0x1002, 0x164e), gpu(DiscreteGpu, 0x10de, 0x2204));
        let soft = gpu(Cpu, 0x10005, 0);
        let on = |c: &AdapterFact| DrivingCard { vendor: c.vendor, device: c.device };
        let all = [raphael.clone(), big.clone(), soft.clone()];
        assert_eq!(power_preference_for(&all, &[on(&big)]), HighPerformance, "the monitor is on the big card");
        assert_eq!(power_preference_for(&all, &[on(&raphael)]), LowPower, "the monitor is on the processor");
        assert_eq!(power_preference_for(&all, &[on(&raphael), on(&big)]), LowPower, "one on each: PRIME decides");
        assert_eq!(power_preference_for(&all, &[]), LowPower, "no facts");
        assert_eq!(power_preference_for(std::slice::from_ref(&raphael), &[on(&raphael)]), LowPower);
        let unknown = DrivingCard { vendor: 1, device: 2 };
        assert_eq!(power_preference_for(&all, &[unknown]), LowPower, "a card no adapter names");
    }

    /// Test t1837b: `/sys/class/drm` as the kernel lays it out: a connector is named after its
    /// card, and only a connected one counts.
    #[test]
    fn t1837b_driving_cards_are_read_from_sysfs() {
        let root = std::env::temp_dir().join(format!("kt-t1837-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let card = |name: &str, vendor: &str, device: &str| {
            let d = root.join(name).join("device");
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("vendor"), vendor).unwrap();
            std::fs::write(d.join("device"), device).unwrap();
        };
        let port = |name: &str, status: &str| {
            let d = root.join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("status"), status).unwrap();
        };
        card("card0", "0x1002\n", "0x164e\n");
        card("card1", "0x10de\n", "0x2204\n");
        port("card0-HDMI-A-1", "disconnected\n");
        port("card1-DP-1", "connected\n");
        port("card1-DP-2", "connected\n");
        let got = driving_cards_in(&root);
        assert_eq!(got, vec![DrivingCard { vendor: 0x10de, device: 0x2204 }]);
        let _ = std::fs::remove_dir_all(&root);
        assert!(driving_cards_in(&root).is_empty(), "no sysfs: no facts");
    }
}
