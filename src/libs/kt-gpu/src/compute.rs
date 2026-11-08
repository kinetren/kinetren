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

/// Timestamps of one job: a pair per dispatch; dispatches past the room run untimed.
const STAMPS: u32 = 64;

/// What the pool keeps at most: a job of a 4K frame fits many times over.
const POOL_BYTES: u64 = 1 << 30;

/// The buffers of finished jobs by size and usage: a kernel asks for the same set every frame.
#[derive(Default)]
struct Pool {
    free: HashMap<(u64, u32), Vec<wgpu::Buffer>>,
    bytes: u64,
}

pub struct ComputeDevice {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: String,
    /// A timestamp pair per dispatch (gpu-pass-cost), where the adapter has them.
    stamps: Option<(wgpu::QuerySet, wgpu::Buffer)>,
    pipes: Mutex<HashMap<(String, usize, usize), Arc<Pipe>>>,
    pool: Mutex<Pool>,
}

/// Open the device: what `kt_plugin_api::compute::install` takes.
pub fn open() -> Result<Arc<dyn ComputeHost>, String> {
    Ok(open_device()?)
}

pub fn open_device() -> Result<Arc<ComputeDevice>, String> {
    // the process's one instance and opening lock (segv-crashes): see `context::instance`
    let g = crate::context::opening();
    let instance = crate::context::instance(&g, wgpu::Backends::PRIMARY);
    // WARP is listed beside a real device (lavapipe on a Windows runner): pick past it
    let mut adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::PRIMARY));
    adapters.retain(|a| !a.get_info().name.to_lowercase().contains("microsoft basic render"));
    adapters.sort_by_key(|a| match a.get_info().device_type {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        _ => 2,
    });
    let adapter = adapters.into_iter().next().ok_or("no compute device")?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("kt-gpu compute"),
        required_features: crate::pass_time::wanted_features(&adapter),
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    let stamps = device.features().contains(wgpu::Features::TIMESTAMP_QUERY).then(|| {
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("kt-gpu compute time"),
            ty: wgpu::QueryType::Timestamp,
            count: STAMPS,
        });
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kt-gpu compute time resolve"),
            size: STAMPS as u64 * 8,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        (set, buf)
    });
    let adapter = adapter.get_info().name;
    Ok(Arc::new(ComputeDevice { device, queue, adapter, stamps, pipes: Mutex::new(HashMap::new()), pool: Mutex::default() }))
}

impl ComputeDevice {
    /// A buffer of at least `size` bytes; true when it comes from the pool and holds a past job.
    fn take(&self, size: u64, usage: wgpu::BufferUsages) -> (wgpu::Buffer, bool) {
        let size = size.max(16).next_multiple_of(16);
        let mut pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(b) = pool.free.get_mut(&(size, usage.bits())).and_then(Vec::pop) {
            pool.bytes -= size;
            return (b, true);
        }
        drop(pool);
        crate::created();
        (self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage, mapped_at_creation: false }), false)
    }

    fn give(&self, buffers: impl IntoIterator<Item = wgpu::Buffer>) {
        let mut pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        for b in buffers {
            if pool.bytes + b.size() <= POOL_BYTES {
                pool.bytes += b.size();
                pool.free.entry((b.size(), b.usage().bits())).or_default().push(b);
            }
        }
    }

    /// What the pool holds now.
    pub fn pooled_bytes(&self) -> u64 {
        self.pool.lock().unwrap_or_else(|e| e.into_inner()).bytes
    }

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
        crate::created();
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
        let mut enc = d.create_command_encoder(&Default::default());
        let bufs: Vec<wgpu::Buffer> = job
            .buffers
            .iter()
            .map(|init| match init {
                Init::Bytes(b) => {
                    let (buf, _) = self.take(b.len() as u64, usage);
                    self.queue.write_buffer(&buf, 0, b);
                    buf
                }
                Init::Zero(n) => {
                    let (buf, used) = self.take(*n, usage);
                    if used {
                        enc.clear_buffer(&buf, 0, None);
                    }
                    buf
                }
            })
            .collect();
        let mut uniforms = Vec::with_capacity(job.steps.len());
        let timed = self.stamps.as_ref().map_or(0, |_| job.steps.len().min(STAMPS as usize / 2));
        for (i, (s, p)) in job.steps.iter().zip(&pipes).enumerate() {
            let (uniform, _) = self.take(s.uniform.len() as u64, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
            self.queue.write_buffer(&uniform, 0, s.uniform);
            let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }];
            for (i, b) in s.read.iter().chain(s.write).enumerate() {
                entries.push(wgpu::BindGroupEntry { binding: i as u32 + 1, resource: bufs[b.0].as_entire_binding() });
            }
            let bind = d.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &p.layout, entries: &entries });
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: self.stamps.as_ref().filter(|_| i < timed).map(|(set, _)| wgpu::ComputePassTimestampWrites {
                    query_set: set,
                    beginning_of_pass_write_index: Some(2 * i as u32),
                    end_of_pass_write_index: Some(2 * i as u32 + 1),
                }),
            });
            cp.set_pipeline(&p.pipe);
            cp.set_bind_group(0, &bind, &[]);
            cp.dispatch_workgroups(s.groups[0], s.groups[1], s.groups[2]);
            drop(cp);
            uniforms.push(uniform);
        }
        let staging: Vec<wgpu::Buffer> = job
            .reads
            .iter()
            .map(|b| {
                let size = bufs[b.0].size();
                let (st, _) = self.take(size, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
                enc.copy_buffer_to_buffer(&bufs[b.0], 0, &st, 0, size);
                st
            })
            .collect();
        // the dispatches' timestamps come back with the job's own buffers
        let stamp_read = self.stamps.as_ref().filter(|_| timed > 0).map(|(set, resolve)| {
            let bytes = timed as u64 * 16;
            enc.resolve_query_set(set, 0..2 * timed as u32, resolve, 0);
            let (st, _) = self.take(bytes, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
            enc.copy_buffer_to_buffer(resolve, 0, &st, 0, bytes);
            st
        });
        kt_time::work::bump(kt_time::work::Work::DeviceSubmit);
        let submitted = std::time::Instant::now();
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
        let waited = submitted.elapsed().as_nanos() as u64;
        match stamp_read {
            Some(st) => {
                let (tx, rx) = std::sync::mpsc::channel();
                let bytes = timed as u64 * 16;
                st.slice(..bytes).map_async(wgpu::MapMode::Read, move |r| {
                    let _ = tx.send(r);
                });
                // the job's own wait has passed: this map is ready at the next poll
                d.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
                if let Ok(Ok(())) = rx.recv() {
                    let ticks: Vec<u64> = st
                        .slice(..bytes)
                        .get_mapped_range()
                        .map_err(|e| e.to_string())?
                        .chunks_exact(8)
                        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
                        .collect();
                    st.unmap();
                    let period = self.queue.get_timestamp_period() as f64;
                    let ds: Vec<(&str, u64)> = job.steps[..timed]
                        .iter()
                        .enumerate()
                        .map(|(i, s)| (s.name, (ticks[2 * i + 1].saturating_sub(ticks[2 * i]) as f64 * period) as u64))
                        .collect();
                    crate::pass_time::note_job(&self.adapter, ds.iter().map(|d| d.1).sum(), &ds);
                }
                self.give([st]);
            }
            None => crate::pass_time::note_job(&self.adapter, waited, &[]),
        }
        self.give(bufs.into_iter().chain(uniforms).chain(staging));
        Ok(out)
    }
}

/// Compile one compute shader on `ctx` and run nothing (t1651, FXC on the Windows runner).
pub fn compile(ctx: &crate::GpuContext, text: &str) -> Result<(), String> {
    let d = &ctx.device;
    let validation = d.push_error_scope(wgpu::ErrorFilter::Validation);
    let internal = d.push_error_scope(wgpu::ErrorFilter::Internal);
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(text.into()),
    });
    let _ = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    match pollster::block_on(internal.pop()).or(pollster::block_on(validation.pop())) {
        Some(e) => Err(e.to_string()),
        None => Ok(()),
    }
}
