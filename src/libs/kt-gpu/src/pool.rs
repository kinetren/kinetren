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
    /// The free transients, to the memory ledger (memory-ledger).
    account: kt_time::mem::Account,
}

impl TexturePool {
    pub fn new(budget: u64) -> TexturePool {
        TexturePool {
            free: HashMap::new(),
            bytes: 0,
            budget,
            account: kt_time::mem::open(kt_time::mem::Tenant::Transient, budget),
        }
    }

    pub fn acquire(&mut self, device: &wgpu::Device, w: u32, h: u32) -> Tex {
        self.acquire_as(device, w, h, FORMAT)
    }

    pub fn acquire_as(&mut self, device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> Tex {
        if let Some(v) = self.free.get_mut(&(w, h, format))
            && let Some(t) = v.pop()
        {
            self.bytes -= tex_bytes(w, h);
            self.account.set(self.bytes);
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
        self.account.set(self.bytes);
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
    /// key → (texture, recency, the frame that last read it).
    map: HashMap<(u64, u32, u32), (Tex, u64, u64)>,
    order: u64,
    bytes: u64,
    budget: u64,
    /// The frame being resolved (`begin_frame`): its uploads are never evicted or reused within it.
    frame: u64,
    /// An export's device (memory-ledger D7): an upload no frame of the last `keep_frames` read is
    /// dropped at the next frame's start. `None` — the preview's LRU within the budget.
    keep_frames: Option<u64>,
    /// The uploads, to the memory ledger (memory-ledger).
    account: kt_time::mem::Account,
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
    pub fn new(budget: u64, keep_frames: Option<u64>) -> UploadCache {
        UploadCache {
            map: HashMap::new(),
            order: 0,
            bytes: 0,
            budget,
            frame: 0,
            keep_frames,
            account: kt_time::mem::open(kt_time::mem::Tenant::Upload, budget),
            spare: HashMap::new(),
            spent_ns: 0,
            spent_bytes: 0,
        }
    }

    /// A frame's plan starts resolving: on an export's device the uploads no frame in flight reads
    /// go (memory-ledger D7).
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        if let Some(keep) = self.keep_frames {
            let floor = self.frame.saturating_sub(keep);
            let old: Vec<_> = self.map.iter().filter(|(_, (_, _, f))| *f < floor).map(|(k, _)| *k).collect();
            for k in old {
                self.drop_entry(k);
            }
            self.account.set(self.bytes);
        }
    }

    /// The uploads held now (tests).
    pub fn held(&self) -> usize {
        self.map.len()
    }

    fn drop_entry(&mut self, k: (u64, u32, u32)) {
        if let Some((t, _, _)) = self.map.remove(&k) {
            self.bytes -= tex_bytes(t.w, t.h);
            let spare = self.spare.entry((t.w, t.h)).or_default();
            if spare.len() < SPARE_PER_SIZE {
                spare.push(t);
            }
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
        let now = self.frame;
        if let Some((tex, stamp, used)) = self.map.get_mut(&key) {
            *stamp = self.order;
            *used = now;
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
        self.map.insert(key, (tex, self.order, self.frame));
        // never an upload the frame being resolved reads: a spare reused within the frame would be
        // rewritten before that frame's passes read it (the queue runs the writes first)
        while self.bytes > self.budget {
            let now = self.frame;
            let oldest = self.map.iter().filter(|(_, (_, _, f))| *f != now).min_by_key(|(_, (_, s, _))| *s).map(|(k, _)| *k);
            let Some(k) = oldest else { break };
            self.drop_entry(k);
        }
        self.account.set(self.bytes);
        view
    }
}

/// The composites the preview presented (cached-range-plays): plan key → the integer composite,
/// on the device. Presenting a key it holds runs the display pass alone — the frames of a covered
/// range are composed once, not once a lap. Only complete frames enter. An evicted texture goes
/// back to the pool it came from.
///
/// Its bound is its base plus what the memory ledger grants the kept composites (memory-ledger):
/// told of a loop (`keep_loop`), it wants every moment of it at the size it presents, plus an eighth.
/// A loop it cannot hold keeps a part: a composite is not admitted over one shown within the loop's
/// length of presents (D5). A device's refusal of its growth bounds it (`refuse`, D8).
pub(crate) struct PresentMemo {
    map: HashMap<u64, (Tex, u64)>,
    stamp: u64,
    bytes: u64,
    base: u64,
    /// Opened by the first keep or loop: an engine that never presents has none.
    account: Option<kt_time::mem::Account>,
    /// What a grant taken back calls (the engine's eviction under its lock).
    on_shrink: Option<std::sync::Arc<dyn Fn(u64) + Send + Sync>>,
    /// The moments of the loop told, 0 — none.
    loop_len: u64,
    /// The size of the last composite kept.
    size: (u32, u32),
    /// The growth past the base the device allowed, learned from its refusal.
    device_limit: Option<u64>,
}

impl PresentMemo {
    pub fn new(base: u64) -> PresentMemo {
        PresentMemo {
            map: HashMap::new(),
            stamp: 0,
            bytes: 0,
            base,
            account: None,
            on_shrink: None,
            loop_len: 0,
            size: (0, 0),
            device_limit: None,
        }
    }

    /// What it calls when the ledger takes its grant back (set once, before the first keep).
    pub fn set_on_shrink(&mut self, f: std::sync::Arc<dyn Fn(u64) + Send + Sync>) {
        self.on_shrink = Some(f);
    }

    pub fn take_on_shrink(&mut self) -> Option<std::sync::Arc<dyn Fn(u64) + Send + Sync>> {
        self.on_shrink.take()
    }

    fn account(&mut self) -> &kt_time::mem::Account {
        if self.account.is_none() {
            let a = kt_time::mem::open(kt_time::mem::Tenant::Kept, self.base);
            if let Some(f) = self.on_shrink.clone() {
                a.on_shrink(move |g| f(g));
            }
            self.account = Some(a);
        }
        self.account.as_ref().unwrap()
    }

    /// The bound now: the base, the ledger's grant, the device's limit.
    pub fn cap(&self) -> u64 {
        let grant = self.account.as_ref().map_or(0, |a| a.cap() - a.base());
        self.base + self.device_limit.map_or(grant, |l| grant.min(l))
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// The loop it should keep: `Some(moments)`, or none.
    pub fn keep_loop(&mut self, moments: Option<usize>) {
        self.loop_len = moments.unwrap_or(0) as u64;
        self.ask();
    }

    fn ask(&mut self) {
        let (w, h) = self.size;
        let want = self.loop_len.saturating_mul(tex_bytes(w, h)).saturating_mul(9) / 8;
        let extra = want.saturating_sub(self.base);
        self.account().want(extra);
    }

    pub fn holds(&self, key: u64) -> bool {
        self.map.contains_key(&key)
    }

    /// The kept composite and its size, marked recent.
    /// The stamp counts presents that showed or kept a composite, so a loop's lap is its length in
    /// stamps (a miss stamps nothing: the composite it leads to does).
    pub fn get(&mut self, key: u64) -> Option<(wgpu::TextureView, u32, u32)> {
        let (t, s) = self.map.get_mut(&key)?;
        self.stamp += 1;
        *s = self.stamp;
        Some((t.view.clone(), t.w, t.h))
    }

    /// Would keeping `w`×`h` grow it past its base?
    pub fn grows_past_base(&self, w: u32, h: u32) -> bool {
        self.bytes + tex_bytes(w, h) > self.base
    }

    /// Keep `t` under `key`; false — not admitted (a loop it cannot hold keeps what it has).
    pub fn insert(&mut self, key: u64, t: Tex, pool: &mut TexturePool) -> bool {
        self.account();
        if (t.w, t.h) != self.size {
            self.size = (t.w, t.h);
            if self.loop_len > 0 {
                self.ask();
            }
        }
        self.stamp += 1;
        self.bytes += tex_bytes(t.w, t.h);
        if let Some((old, _)) = self.map.insert(key, (t, self.stamp)) {
            self.bytes -= tex_bytes(old.w, old.h);
            pool.release(old);
        }
        let cap = self.cap();
        let mut kept = true;
        while self.bytes > cap && self.map.len() > 1 {
            // the least recent; the memo holds a few thousand frames at most
            let Some((oldest, shown)) = self.map.iter().filter(|(k, _)| **k != key).min_by_key(|(_, (_, s))| *s).map(|(k, (_, s))| (*k, *s)) else {
                break;
            };
            // a lap's presents, with the eighth a repeated moment may add
            let lap = self.loop_len + self.loop_len / 8 + 2;
            if self.loop_len > 0 && self.stamp - shown <= lap {
                // everything kept is the current lap's: the new one is not admitted over it
                if let Some((t, _)) = self.map.remove(&key) {
                    self.bytes -= tex_bytes(t.w, t.h);
                    pool.release(t);
                }
                kept = false;
                break;
            }
            self.evict(oldest, key, pool);
        }
        if kept {
            kt_time::trace::ev(kt_time::trace::Ev::MemoStore).id(key).a(self.bytes).b(self.map.len() as u64).emit();
        }
        self.evict_to_cap(pool);
        kept
    }

    fn evict(&mut self, k: u64, cause: u64, pool: &mut TexturePool) {
        if let Some((t, _)) = self.map.remove(&k) {
            self.bytes -= tex_bytes(t.w, t.h);
            pool.release(t);
            kt_time::trace::ev(kt_time::trace::Ev::MemoEvict).id(k).cause(cause).emit();
        }
    }

    /// Drop the least recently shown until the bound holds the rest (a grant taken back, a refusal).
    pub fn evict_to_cap(&mut self, pool: &mut TexturePool) {
        let cap = self.cap();
        while self.bytes > cap && !self.map.is_empty() {
            let Some(oldest) = self.map.iter().min_by_key(|(_, (_, s))| *s).map(|(k, _)| *k) else { break };
            self.evict(oldest, 0, pool);
        }
        if let Some(a) = &self.account {
            a.set(self.bytes);
        }
    }

    /// The device refused the growth (memory-ledger D8): from now on the memo grows at most three
    /// quarters of what it holds past its base, and drops the rest now.
    pub fn refuse(&mut self, pool: &mut TexturePool) {
        let past = self.bytes.saturating_sub(self.base);
        let limit = past - past / 4;
        self.device_limit = Some(self.device_limit.map_or(limit, |l| l.min(limit)));
        kt_time::trace::ev(kt_time::trace::Ev::MemGrant)
            .id(1)
            .aux(kt_time::mem::Tenant::Kept as u32)
            .a(self.loop_len.saturating_mul(tex_bytes(self.size.0, self.size.1)))
            .b(self.base + limit)
            .c(self.bytes)
            .emit();
        self.evict_to_cap(pool);
    }
}

/// LRU of ready composition frames: plan hash → (RGBA, complete). Keeps the
/// warm loop fast (the GPU path has no post-FX memoization).
pub(crate) struct ReadbackCache {
    map: HashMap<u64, (Vec<u8>, bool)>,
    order: VecDeque<u64>,
    bytes: u64,
    budget: u64,
    /// Opened by the first insert (memory-ledger): an engine that never reads back has none.
    account: Option<kt_time::mem::Account>,
}

impl ReadbackCache {
    pub fn new(budget: u64) -> ReadbackCache {
        ReadbackCache { map: HashMap::new(), order: VecDeque::new(), bytes: 0, budget, account: None }
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
        let budget = self.budget;
        self.account.get_or_insert_with(|| kt_time::mem::open(kt_time::mem::Tenant::Readback, budget)).set(self.bytes);
    }
}
