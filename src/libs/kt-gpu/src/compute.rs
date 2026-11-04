//! The compute host (compute-kernels): `kt_plugin_api::compute` on a device of its own. The
//! interface's device may be GL, which has no compute, so this one asks for Vulkan, DX12 or Metal
//! whatever `WGPU_BACKEND` says. WARP is refused, as the engine refuses it. Jobs are serialised.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kt_plugin_api::compute::{ComputeHost, Init, Job};

/// One shader's pipeline; the layout follows from how many buffers a step binds.
struct Pipe {
    pipe: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

pub struct ComputeDevice {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipes: Mutex<HashMap<(String, usize, usize), Arc<Pipe>>>,
}

/// Open the device: what `kt_plugin_api::compute::install` takes.
pub fn open() -> Result<Arc<dyn ComputeHost>, String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    desc.backends = wgpu::Backends::PRIMARY;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    let info = adapter.get_info();
    if info.name.to_lowercase().contains("microsoft basic render") {
        return Err(format!("{}: software D3D12 is not used", info.name));
    }
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("kt-gpu compute"),
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    Ok(Arc::new(ComputeDevice { device, queue, pipes: Mutex::new(HashMap::new()) }))
}

impl ComputeDevice {
    fn pipe(&self, text: &str, reads: usize, writes: usize) -> Result<Arc<Pipe>, String> {
        let mut pipes = self.pipes.lock().unwrap_or_else(|e| e.into_inner());
        let key = (text.to_string(), reads, writes);
        if let Some(p) = pipes.get(&key) {
            return Ok(p.clone());
        }
        let d = &self.device;
        let entry = |binding: usize, ty| wgpu::BindGroupLayoutEntry {
            binding: binding as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let mut entries = vec![entry(0, wgpu::BufferBindingType::Uniform)];
        for i in 0..reads + writes {
            entries.push(entry(i + 1, wgpu::BufferBindingType::Storage { read_only: i < reads }));
        }
        // two scopes: a compiler's refusal is an internal error, a bad text a validation one
        let validation = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal = d.push_error_scope(wgpu::ErrorFilter::Internal);
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(text.into()),
        });
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries });
        let pipe_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipe_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(e) = pollster::block_on(internal.pop()).or(pollster::block_on(validation.pop())) {
            return Err(format!("compute shader refused: {e}"));
        }
        let p = Arc::new(Pipe { pipe, layout });
        pipes.insert(key, p.clone());
        Ok(p)
    }
}

impl ComputeHost for ComputeDevice {
    fn run(&self, job: &Job<'_>) -> Result<Vec<Vec<u8>>, String> {
        let d = &self.device;
        let pipes = job
            .steps
            .iter()
            .map(|s| self.pipe(s.shader, s.read.len(), s.write.len()))
            .collect::<Result<Vec<_>, _>>()?;
        let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        let buffer = |size: u64, usage| {
            d.create_buffer(&wgpu::BufferDescriptor { label: None, size: size.max(16).next_multiple_of(16), usage, mapped_at_creation: false })
        };
        let bufs: Vec<wgpu::Buffer> = job
            .buffers
            .iter()
            .map(|init| match init {
                Init::Bytes(b) => {
                    let buf = buffer(b.len() as u64, usage);
                    self.queue.write_buffer(&buf, 0, b);
                    buf
                }
                Init::Zero(n) => buffer(*n, usage),
            })
            .collect();
        let mut enc = d.create_command_encoder(&Default::default());
        for (s, p) in job.steps.iter().zip(&pipes) {
            let uniform = buffer(s.uniform.len() as u64, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
            self.queue.write_buffer(&uniform, 0, s.uniform);
            let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }];
            for (i, b) in s.read.iter().chain(s.write).enumerate() {
                entries.push(wgpu::BindGroupEntry { binding: i as u32 + 1, resource: bufs[b.0].as_entire_binding() });
            }
            let bind = d.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &p.layout, entries: &entries });
            let mut cp = enc.begin_compute_pass(&Default::default());
            cp.set_pipeline(&p.pipe);
            cp.set_bind_group(0, &bind, &[]);
            cp.dispatch_workgroups(s.groups[0], s.groups[1], s.groups[2]);
        }
        let staging: Vec<wgpu::Buffer> = job
            .reads
            .iter()
            .map(|b| {
                let size = bufs[b.0].size();
                let st = buffer(size, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
                enc.copy_buffer_to_buffer(&bufs[b.0], 0, &st, 0, size);
                st
            })
            .collect();
        self.queue.submit([enc.finish()]);
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(format!("compute job refused: {e}"));
        }
        let mut out = Vec::with_capacity(staging.len());
        for (st, b) in staging.iter().zip(job.reads) {
            let (tx, rx) = std::sync::mpsc::channel();
            st.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
            d.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
            rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
            let len = match job.buffers[b.0] {
                Init::Bytes(v) => v.len() as u64,
                Init::Zero(n) => n,
            };
            out.push(st.slice(..len).get_mapped_range().map_err(|e| e.to_string())?.to_vec());
            st.unmap();
        }
        Ok(out)
    }
}
