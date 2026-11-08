//! What each pass costs on the device (gpu-pass-cost): a timestamp before and after every pass of
//! every plan the engines execute and every dispatch of the compute host, where the adapter has
//! `TIMESTAMP_QUERY`. Nothing waits for them: the durations come back with the frame's own readback
//! or with a map the next frame collects, and go into one process-wide sum the load watch takes
//! once a second ([`take`]). Where the adapter has no timestamps a frame is timed from its submit to
//! the device's "done" instead, and the record says so. Nothing is queried when nothing composes.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Queries of one frame: a pair per pass. Passes past the room run untimed and are counted.
pub(crate) const QUERIES: u32 = 1024;
/// Frames whose timestamps may wait for their map; past it a frame runs untimed (no poll came).
const PENDING_MAX: usize = 8;

/// Who asked for the frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// The preview's present.
    Preview,
    /// A frame read back: the offline engine, a probe, a test.
    Offline,
    /// The export's ring.
    Export,
}

/// What a pass is, resolved to its name only when its time is summed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PassLabel {
    Fixed(&'static str),
    /// A plugin's own pass: its index in the engine's plugin pipes.
    Plugin(usize),
    /// A host node inside a barrier plugin's graph: the plugin's index among the barriers, the node.
    Host(usize, &'static str),
}

/// A frame's facts beside its passes.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct FrameMeta {
    pub out: (u32, u32),
    /// The largest decoded source the frame read, and how many sources.
    pub decode: (u32, u32),
    pub layers: u32,
}

struct Pending {
    buf: wgpu::Buffer,
    labels: Vec<PassLabel>,
    untimed: u32,
    role: Role,
    meta: FrameMeta,
    /// 0 waiting, 1 mapped, 2 failed; `None` — not asked yet (asked after the submit).
    state: Option<Arc<AtomicU8>>,
}

/// The timer of one engine.
pub(crate) struct Timer {
    set: Option<wgpu::QuerySet>,
    resolve: Option<wgpu::Buffer>,
    period_ns: f64,
    adapter: Arc<str>,
    /// The names of the plugin pipes by index, and the barrier plugins' ids by their order.
    names: Vec<String>,
    owners: Vec<String>,
    labels: Vec<PassLabel>,
    untimed: u32,
    free: Vec<wgpu::Buffer>,
    pending: VecDeque<Pending>,
    /// The frame being encoded is timed (its labels count).
    on: bool,
}

impl Timer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, adapter: &str, names: Vec<String>, owners: Vec<String>) -> Timer {
        let has = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let set = has.then(|| {
            device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("kt-gpu pass time"),
                ty: wgpu::QueryType::Timestamp,
                count: QUERIES,
            })
        });
        let resolve = has.then(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kt-gpu pass time resolve"),
                size: QUERIES as u64 * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        });
        Timer {
            set,
            resolve,
            period_ns: queue.get_timestamp_period() as f64,
            adapter: adapter.into(),
            names,
            owners,
            labels: Vec::new(),
            untimed: 0,
            free: Vec::new(),
            pending: VecDeque::new(),
            on: false,
        }
    }

    /// A new frame begins encoding.
    pub fn begin(&mut self) {
        self.labels.clear();
        self.untimed = 0;
        self.on = self.set.is_some() && self.pending.len() < PENDING_MAX;
    }

    /// The timestamp writes of the next pass, or `None` (no timestamps, no room).
    pub fn pass(&mut self, label: PassLabel) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let i = self.slot(label)?;
        Some(wgpu::RenderPassTimestampWrites {
            query_set: self.set.as_ref()?,
            beginning_of_pass_write_index: Some(2 * i),
            end_of_pass_write_index: Some(2 * i + 1),
        })
    }

    fn slot(&mut self, label: PassLabel) -> Option<u32> {
        if !self.on {
            return None;
        }
        let i = self.labels.len() as u32;
        if 2 * i + 2 > QUERIES {
            self.untimed += 1;
            return None;
        }
        self.labels.push(label);
        Some(i)
    }

    /// The frame's queries resolved and copied into a buffer of its own, before `finish`.
    pub fn end(&mut self, device: &wgpu::Device, enc: &mut wgpu::CommandEncoder, role: Role, meta: FrameMeta) {
        if !self.on || self.labels.is_empty() {
            self.on = false;
            return;
        }
        self.on = false;
        let (Some(set), Some(resolve)) = (&self.set, &self.resolve) else { return };
        let n = self.labels.len() as u32 * 2;
        enc.resolve_query_set(set, 0..n, resolve, 0);
        let buf = self.free.pop().unwrap_or_else(|| {
            crate::created();
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kt-gpu pass time read"),
                size: QUERIES as u64 * 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        });
        enc.copy_buffer_to_buffer(resolve, 0, &buf, 0, n as u64 * 8);
        self.pending.push_back(Pending {
            buf,
            labels: std::mem::take(&mut self.labels),
            untimed: self.untimed,
            role,
            meta,
            state: None,
        });
    }

    /// After the submit: the frame's map is asked; without timestamps, the submit-to-done time.
    pub fn submitted(&mut self, queue: &wgpu::Queue, role: Role, meta: FrameMeta) {
        if self.set.is_none() {
            let (at, adapter) = (Instant::now(), self.adapter.clone());
            queue.on_submitted_work_done(move || {
                let ns = at.elapsed().as_nanos() as u64;
                with_agg(|a| {
                    a.note_frame(&adapter, false, role, meta);
                    a.frame_ns += ns;
                });
            });
            return;
        }
        for p in self.pending.iter_mut().filter(|p| p.state.is_none()) {
            let state = Arc::new(AtomicU8::new(0));
            let s = state.clone();
            p.buf.slice(..p.labels.len() as u64 * 16).map_async(wgpu::MapMode::Read, move |r| {
                s.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
            });
            p.state = Some(state);
        }
    }

    /// The frames whose timestamps are back, oldest first, into the process-wide sum. Never waits:
    /// a non-blocking poll runs the callbacks of maps that finished (the preview never waits on its
    /// own, so without it a map would wait for someone else's submit).
    pub fn harvest(&mut self, device: &wgpu::Device) {
        if self.pending.iter().any(|p| p.state.is_some()) {
            let _ = device.poll(wgpu::PollType::Poll);
        }
        while let Some(p) = self.pending.front() {
            let st = p.state.as_ref().map_or(0, |s| s.load(Ordering::Acquire));
            if st == 0 {
                break;
            }
            let p = self.pending.pop_front().unwrap();
            if st == 1 {
                let n = p.labels.len();
                let ticks: Vec<u64> = match p.buf.slice(..n as u64 * 16).get_mapped_range() {
                    Ok(data) => data.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect(),
                    Err(_) => Vec::new(),
                };
                p.buf.unmap();
                if ticks.len() < 2 * n {
                    self.free.push(p.buf);
                    continue;
                }
                let ns = |t: u64| (t as f64 * self.period_ns) as u64;
                let passes: Vec<(String, u64)> = p
                    .labels
                    .iter()
                    .enumerate()
                    .map(|(i, l)| (self.name(*l), ns(ticks[2 * i + 1].saturating_sub(ticks[2 * i]))))
                    .collect();
                let first = ticks.iter().step_by(2).copied().min().unwrap_or(0);
                let last = ticks.iter().skip(1).step_by(2).copied().max().unwrap_or(0);
                let span = ns(last.saturating_sub(first));
                let adapter = self.adapter.clone();
                with_agg(|a| {
                    a.note_frame(&adapter, true, p.role, p.meta);
                    a.untimed += p.untimed;
                    a.span_ns += span;
                    for (name, d) in passes {
                        a.frame_ns += d;
                        let e = a.passes.entry(name).or_default();
                        e.0 += d;
                        e.1 += 1;
                    }
                });
            }
            self.free.push(p.buf);
        }
    }

    fn name(&self, l: PassLabel) -> String {
        match l {
            PassLabel::Fixed(s) => s.to_string(),
            PassLabel::Plugin(i) => self.names.get(i).cloned().unwrap_or_else(|| format!("plugin {i}")),
            PassLabel::Host(owner, node) => {
                format!("{}:{node}", self.owners.get(owner).map_or("plugin", String::as_str))
            }
        }
    }
}

/// The name of a barrier plugin's pass `i`: its own (`BarrierKernel::pass_names`) or its index.
pub(crate) fn barrier_pass_name(id: &str, names: &[&str], i: usize) -> String {
    match names.get(i) {
        Some(n) => format!("{id}:{n}"),
        None => format!("{id}:p{i}"),
    }
}

/// One second of the device, as the load watch records it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Second {
    /// The adapters that composed, by name.
    pub adapters: Vec<String>,
    /// Every frame carried timestamps; false — some frames had only their submit-to-done time.
    pub timestamps: bool,
    pub frames: u32,
    pub preview: u32,
    pub offline: u32,
    pub export: u32,
    /// The device time of the frames: the sum of their passes (or of their submit-to-done times).
    pub frame_ns: u64,
    /// From the first pass's start to the last pass's end, summed over the timed frames.
    pub span_ns: u64,
    /// Name, total, count; the largest total first.
    pub passes: Vec<(String, u64, u32)>,
    /// Passes past a frame's query room.
    pub untimed: u32,
    pub out: (u32, u32),
    pub decode: (u32, u32),
    pub layers: u32,
    /// The compute host's jobs and their dispatches.
    pub jobs: u32,
    pub job_ns: u64,
    pub dispatches: Vec<(String, u64, u32)>,
}

fn ms(ns: u64) -> f64 {
    ns as f64 / 1e6
}

impl Second {
    /// `adapter | frame W×H, decode W×H ×N | F frames (preview P), X ms a frame, span S | pass mean ×n, …`
    pub fn line(&self) -> String {
        let mut out = self.adapters.join(" + ");
        if self.frames > 0 {
            let f = self.frames as u64;
            let roles: Vec<String> = [("preview", self.preview), ("offline", self.offline), ("export", self.export)]
                .iter()
                .filter(|(_, n)| *n > 0)
                .map(|(r, n)| format!("{r} {n}"))
                .collect();
            out.push_str(&format!(
                " | frame {}×{}, decode {}×{} ×{} | {} frames ({}), {:.2} ms a frame",
                self.out.0,
                self.out.1,
                self.decode.0,
                self.decode.1,
                self.layers,
                self.frames,
                roles.join(", "),
                ms(self.frame_ns) / f as f64
            ));
            if self.timestamps {
                out.push_str(&format!(", span {:.2}", ms(self.span_ns) / f as f64));
                let passes: Vec<String> =
                    self.passes.iter().map(|(n, t, c)| format!("{n} {:.3} ×{c}", ms(*t) / *c as f64)).collect();
                out.push_str(" | ");
                out.push_str(&passes.join(", "));
                if self.untimed > 0 {
                    out.push_str(&format!(", untimed passes {}", self.untimed));
                }
            } else {
                out.push_str(" from submit to done (no timestamps on this device)");
            }
        }
        if self.jobs > 0 {
            let d: Vec<String> =
                self.dispatches.iter().map(|(n, t, c)| format!("{n} {:.3} ×{c}", ms(*t) / *c as f64)).collect();
            out.push_str(&format!(
                " | compute {} jobs, {:.2} ms a job{}{}",
                self.jobs,
                ms(self.job_ns) / self.jobs as f64,
                if d.is_empty() { " from submit to done (no timestamps)" } else { ": " },
                d.join(", ")
            ));
        }
        out
    }
}

#[derive(Default)]
struct Agg {
    adapters: Vec<String>,
    stamped: u32,
    frames: u32,
    roles: [u32; 3],
    frame_ns: u64,
    span_ns: u64,
    passes: HashMap<String, (u64, u32)>,
    untimed: u32,
    out: (u32, u32),
    decode: (u32, u32),
    layers: u32,
    jobs: u32,
    job_ns: u64,
    dispatches: HashMap<String, (u64, u32)>,
}

impl Agg {
    fn note_frame(&mut self, adapter: &str, stamped: bool, role: Role, meta: FrameMeta) {
        self.note_adapter(adapter);
        self.frames += 1;
        self.stamped += stamped as u32;
        self.roles[role as usize] += 1;
        // the second's largest frame names the size: a thumbnail beside the preview is not it
        if meta.out.0 * meta.out.1 >= self.out.0 * self.out.1 {
            self.out = meta.out;
            self.decode = meta.decode;
            self.layers = meta.layers;
        }
    }

    fn note_adapter(&mut self, adapter: &str) {
        if !self.adapters.iter().any(|a| a == adapter) {
            self.adapters.push(adapter.to_string());
        }
    }
}

static AGG: Mutex<Option<Agg>> = Mutex::new(None);

fn with_agg(f: impl FnOnce(&mut Agg)) {
    let mut g = AGG.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Agg::default));
}

fn sorted(m: HashMap<String, (u64, u32)>) -> Vec<(String, u64, u32)> {
    let mut v: Vec<(String, u64, u32)> = m.into_iter().map(|(n, (t, c))| (n, t, c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// What the device did since the last call, or `None` when it did nothing.
pub fn take() -> Option<Second> {
    let a = AGG.lock().unwrap_or_else(|e| e.into_inner()).take()?;
    if a.frames == 0 && a.jobs == 0 {
        return None;
    }
    Some(Second {
        adapters: a.adapters,
        timestamps: a.frames > 0 && a.stamped == a.frames,
        frames: a.frames,
        preview: a.roles[Role::Preview as usize],
        offline: a.roles[Role::Offline as usize],
        export: a.roles[Role::Export as usize],
        frame_ns: a.frame_ns,
        span_ns: a.span_ns,
        passes: sorted(a.passes),
        untimed: a.untimed,
        out: a.out,
        decode: a.decode,
        layers: a.layers,
        jobs: a.jobs,
        job_ns: a.job_ns,
        dispatches: sorted(a.dispatches),
    })
}

/// A compute job's dispatch times (`compute.rs`), or its submit-to-done time without timestamps.
pub(crate) fn note_job(adapter: &str, job_ns: u64, dispatches: &[(&str, u64)]) {
    with_agg(|a| {
        a.note_adapter(adapter);
        a.jobs += 1;
        a.job_ns += job_ns;
        for (n, d) in dispatches {
            let e = a.dispatches.entry(n.to_string()).or_default();
            e.0 += d;
            e.1 += 1;
        }
    });
}

/// The features a device asks for so the timer works: `TIMESTAMP_QUERY` where the adapter has it.
pub fn wanted_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    adapter.features() & wgpu::Features::TIMESTAMP_QUERY
}
