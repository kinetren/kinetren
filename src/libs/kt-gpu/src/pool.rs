//! Pools of GPU resources: transient textures of chains (ping-pong),
//! an LRU cache of loaded pre-FX frames and an LRU cache of readback results.
//! VRAM budgets are hard: the caches are the only holders of textures between
//! frames; transients are returned to the pool within a single execute.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use kt_media::VideoFrame;

/// All engine textures are Rgba8Uint (raw bytes 0..255, without unorm loss).
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Uint;
/// The target of a pass that keeps one 32-bit word per texel (`BarrierKernel::word_passes`).
pub(crate) const WORD_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;

pub(crate) struct Tex {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub w: u32,
    pub h: u32,
    pub format: wgpu::TextureFormat,
}

pub(crate) fn create_tex(device: &wgpu::Device, w: u32, h: u32, label: &str) -> Tex {
    create_tex_as(device, w, h, FORMAT, label)
}

fn create_tex_as(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat, label: &str) -> Tex {
    crate::created();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Tex { texture, view, w, h, format }
}

/// The format egui samples a registered texture in.
pub(crate) const DISPLAY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A target the UI draws (preview-throughput D2). COPY_SRC serves `read_display` alone.
pub(crate) fn create_display_tex(device: &wgpu::Device, w: u32, h: u32, label: &str) -> Tex {
    crate::created();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DISPLAY_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Tex { texture, view, w, h, format: DISPLAY_FORMAT }
}

fn tex_bytes(w: u32, h: u32) -> u64 {
    w as u64 * h as u64 * 4
}

/// Transients by size. Within a single execute a texture is returned to the
/// pool ONLY when its content is no longer read by any future pass.
pub(crate) struct TexturePool {
    free: HashMap<(u32, u32, wgpu::TextureFormat), Vec<Tex>>,
    bytes: u64,
    budget: u64,
}

impl TexturePool {
    pub fn new(budget: u64) -> TexturePool {
        TexturePool { free: HashMap::new(), bytes: 0, budget }
    }

    pub fn acquire(&mut self, device: &wgpu::Device, w: u32, h: u32) -> Tex {
        self.acquire_as(device, w, h, FORMAT)
    }

    pub fn acquire_as(&mut self, device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> Tex {
        if let Some(v) = self.free.get_mut(&(w, h, format))
            && let Some(t) = v.pop()
        {
            self.bytes -= tex_bytes(w, h);
            return t;
        }
        create_tex_as(device, w, h, format, "kt-gpu transient")
    }

    pub fn release(&mut self, t: Tex) {
        let b = tex_bytes(t.w, t.h);
        if self.bytes + b > self.budget {
            return; // we drop them: VRAM costs more than a repeated create
        }
        self.bytes += b;
        self.free.entry((t.w, t.h, t.format)).or_default().push(t);
    }
}

/// LRU of loaded pre-FX frames: (SERIAL NUMBER of the content, dims) →
/// texture.
///
/// The addresses `(source, cache_idx)` are NOT ENOUGH for the key (R.53, item 409):
/// a generator with new parameters, and a file that came back in place of a stub, have the same
/// address but different bytes; the cache would return the old texture,
/// and on the GPU path an edit of the title’s colour would not be visible (t496, t211).
/// The serial number is issued by `BurstCache` and is never reused.
pub(crate) struct UploadCache {
    map: HashMap<(u64, u32, u32), (Tex, u64)>,
    order: u64,
    bytes: u64,
    budget: u64,
    /// Evicted textures kept for the next upload of their size (export-throughput D5): an
    /// export uploads every source frame once, and a texture per frame was a device allocation
    /// per frame. A reused texture is rewritten by `write_texture`, which the queue orders after
    /// every submission already reading it.
    spare: HashMap<(u32, u32), Vec<Tex>>,
    /// Nanoseconds and bytes spent uploading, cumulative (preview-throughput, a diagnostic).
    pub spent_ns: u64,
    pub spent_bytes: u64,
}

/// Spare textures kept per size.
const SPARE_PER_SIZE: usize = 4;

impl UploadCache {
    pub fn new(budget: u64) -> UploadCache {
        UploadCache {
            map: HashMap::new(),
            order: 0,
            bytes: 0,
            budget,
            spare: HashMap::new(),
            spent_ns: 0,
            spent_bytes: 0,
        }
    }

    pub fn get_or_upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        content: u64,
        frame: &Arc<VideoFrame>,
    ) -> wgpu::TextureView {
        let key = (content, frame.width, frame.height);
        self.order += 1;
        if let Some((tex, stamp)) = self.map.get_mut(&key) {
            *stamp = self.order;
            return tex.view.clone();
        }
        let mark = std::time::Instant::now();
        let tex = match self.spare.get_mut(&(frame.width, frame.height)).and_then(|v| v.pop()) {
            Some(t) => t,
            None => create_tex(device, frame.width, frame.height, "kt-gpu upload"),
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.width * 4),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d { width: frame.width, height: frame.height, depth_or_array_layers: 1 },
        );
        self.spent_ns += mark.elapsed().as_nanos() as u64;
        self.spent_bytes += frame.rgba.len() as u64;
        let view = tex.view.clone();
        self.bytes += tex_bytes(frame.width, frame.height);
        self.map.insert(key, (tex, self.order));
        while self.bytes > self.budget && self.map.len() > 1 {
            let oldest = self.map.iter().min_by_key(|(_, (_, s))| *s).map(|(k, _)| *k);
            let Some(k) = oldest else { break };
            if let Some((t, _)) = self.map.remove(&k) {
                self.bytes -= tex_bytes(t.w, t.h);
                let spare = self.spare.entry((t.w, t.h)).or_default();
                if spare.len() < SPARE_PER_SIZE {
                    spare.push(t);
                }
            }
        }
        view
    }
}

/// The composites the preview presented (cached-range-plays): plan key → the integer composite,
/// on the device. Presenting a key it holds runs the display pass alone — the frames of a covered
/// range are composed once, not once a lap. Only complete frames enter. An evicted texture goes
/// back to the pool it came from.
pub(crate) struct PresentMemo {
    map: HashMap<u64, (Tex, u64)>,
    stamp: u64,
    bytes: u64,
    budget: u64,
}

impl PresentMemo {
    pub fn new(budget: u64) -> PresentMemo {
        PresentMemo { map: HashMap::new(), stamp: 0, bytes: 0, budget }
    }

    pub fn holds(&self, key: u64) -> bool {
        self.map.contains_key(&key)
    }

    /// The kept composite, marked recent.
    pub fn get(&mut self, key: u64) -> Option<wgpu::TextureView> {
        self.stamp += 1;
        let stamp = self.stamp;
        self.map.get_mut(&key).map(|(t, s)| {
            *s = stamp;
            t.view.clone()
        })
    }

    pub fn insert(&mut self, key: u64, t: Tex, pool: &mut TexturePool) {
        self.stamp += 1;
        self.bytes += tex_bytes(t.w, t.h);
        if let Some((old, _)) = self.map.insert(key, (t, self.stamp)) {
            self.bytes -= tex_bytes(old.w, old.h);
            pool.release(old);
        }
        while self.bytes > self.budget && self.map.len() > 1 {
            // the least recent; the memo holds a few hundred frames at most
            let Some(oldest) = self.map.iter().min_by_key(|(_, (_, s))| *s).map(|(k, _)| *k) else {
                break;
            };
            if let Some((t, _)) = self.map.remove(&oldest) {
                self.bytes -= tex_bytes(t.w, t.h);
                pool.release(t);
            }
        }
    }
}

/// LRU of ready composition frames: plan hash → (RGBA, complete). Keeps the
/// warm loop fast (the GPU path has no post-FX memoization).
pub(crate) struct ReadbackCache {
    map: HashMap<u64, (Vec<u8>, bool)>,
    order: VecDeque<u64>,
    bytes: u64,
    budget: u64,
}

impl ReadbackCache {
    pub fn new(budget: u64) -> ReadbackCache {
        ReadbackCache { map: HashMap::new(), order: VecDeque::new(), bytes: 0, budget }
    }

    pub fn get(&mut self, key: u64) -> Option<(&[u8], bool)> {
        if !self.map.contains_key(&key) {
            return None;
        }
        // recency bump
        if let Some(p) = self.order.iter().position(|k| *k == key) {
            self.order.remove(p);
        }
        self.order.push_back(key);
        self.map.get(&key).map(|(v, c)| (v.as_slice(), *c))
    }

    pub fn insert(&mut self, key: u64, data: Vec<u8>, complete: bool) {
        if let Some((old, _)) = self.map.remove(&key) {
            self.bytes -= old.len() as u64;
            if let Some(p) = self.order.iter().position(|k| *k == key) {
                self.order.remove(p);
            }
        }
        self.bytes += data.len() as u64;
        self.map.insert(key, (data, complete));
        self.order.push_back(key);
        while self.bytes > self.budget && self.order.len() > 1 {
            let Some(k) = self.order.pop_front() else { break };
            if let Some((v, _)) = self.map.remove(&k) {
                self.bytes -= v.len() as u64;
            }
        }
    }
}
