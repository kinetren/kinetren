//! Ownership of the wgpu device. Two ways of creating it:
//! - `from_parts` — preview: Arc handles of the LIVE eframe device
//!   (egui_wgpu::RenderState); there is no separate adapter for the preview;
//! - `new_headless` — export/kt-cli/tests: its own adapter without a surface,
//!   the limits repeat the egui-wgpu rule (downlevel_webgl2 on GL, otherwise
//!   default; max_texture_dimension_2d 8192): shader behaviour in the GUI
//!   and in headless MUST match.

use std::sync::Arc;

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

impl GpuContext {
    /// The eframe device (preview): handles from `egui_wgpu::RenderState`.
    pub fn from_parts(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        adapter_info: wgpu::AdapterInfo,
    ) -> GpuContext {
        GpuContext { device, queue, adapter_info }
    }

    /// Headless device (export, kt-cli, tests). PowerPreference is
    /// the same as in the GUI: WGPU_POWER_PREF or LowPower (PRIME, item 39).
    pub fn new_headless() -> Result<GpuContext, GpuInitError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:
                wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::LowPower),
            ..Default::default()
        }))
        .map_err(|e| GpuInitError::NoAdapter(e.to_string()))?;
        let base_limits = if adapter.get_info().backend == wgpu::Backend::Gl {
            wgpu::Limits::downlevel_webgl2_defaults()
        } else {
            wgpu::Limits::default()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("kt-gpu headless"),
            required_limits: wgpu::Limits { max_texture_dimension_2d: 8192, ..base_limits },
            ..Default::default()
        }))
        .map_err(|e| GpuInitError::Device(e.to_string()))?;
        Ok(GpuContext {
            device: Arc::new(device),
            queue: Arc::new(queue),
            adapter_info: adapter.get_info(),
        })
    }
}
