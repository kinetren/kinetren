//! Пулы GPU-ресурсов: транзиентные текстуры цепочек (ping-pong),
//! LRU-кэш загруженных пре-FX кадров и LRU-кэш readback-результатов.
//! Бюджеты VRAM жёсткие: кэши — единственные держатели текстур между
//! кадрами, транзиенты возвращаются в пул внутри одного execute.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use kt_cache::SourceId;
use kt_media::VideoFrame;

/// Все текстуры движка — Rgba8Uint (сырые байты 0..255 без unorm-потерь).
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Uint;

pub(crate) struct Tex {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub w: u32,
    pub h: u32,
}

pub(crate) fn create_tex(device: &wgpu::Device, w: u32, h: u32, label: &str) -> Tex {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Tex { texture, view, w, h }
}

fn tex_bytes(w: u32, h: u32) -> u64 {
    w as u64 * h as u64 * 4
}

/// Транзиенты по размеру. Внутри одного execute текстура возвращается в
/// пул ТОЛЬКО когда её содержимое больше не читает ни один будущий пасс.
pub(crate) struct TexturePool {
    free: HashMap<(u32, u32), Vec<Tex>>,
    bytes: u64,
    budget: u64,
}

impl TexturePool {
    pub fn new(budget: u64) -> TexturePool {
        TexturePool { free: HashMap::new(), bytes: 0, budget }
    }

    pub fn acquire(&mut self, device: &wgpu::Device, w: u32, h: u32) -> Tex {
        if let Some(v) = self.free.get_mut(&(w, h))
            && let Some(t) = v.pop()
        {
            self.bytes -= tex_bytes(w, h);
            return t;
        }
        create_tex(device, w, h, "kt-gpu transient")
    }

    pub fn release(&mut self, t: Tex) {
        let b = tex_bytes(t.w, t.h);
        if self.bytes + b > self.budget {
            return; // дропаем — VRAM дороже повторного create
        }
        self.bytes += b;
        self.free.entry((t.w, t.h)).or_default().push(t);
    }
}

/// LRU загруженных пре-FX кадров: (source, cache_idx, dims) → текстура.
/// Содержимое кадра для ключа детерминировано (передекод даёт те же
/// байты), generation не нужен.
pub(crate) struct UploadCache {
    map: HashMap<(u64, i64, u32, u32), (Tex, u64)>,
    order: u64,
    bytes: u64,
    budget: u64,
}

impl UploadCache {
    pub fn new(budget: u64) -> UploadCache {
        UploadCache { map: HashMap::new(), order: 0, bytes: 0, budget }
    }

    pub fn get_or_upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: SourceId,
        cache_idx: i64,
        frame: &Arc<VideoFrame>,
    ) -> wgpu::TextureView {
        let key = (source.0, cache_idx, frame.width, frame.height);
        self.order += 1;
        if let Some((tex, stamp)) = self.map.get_mut(&key) {
            *stamp = self.order;
            return tex.view.clone();
        }
        let tex = create_tex(device, frame.width, frame.height, "kt-gpu upload");
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
        let view = tex.view.clone();
        self.bytes += tex_bytes(frame.width, frame.height);
        self.map.insert(key, (tex, self.order));
        while self.bytes > self.budget && self.map.len() > 1 {
            let oldest = self.map.iter().min_by_key(|(_, (_, s))| *s).map(|(k, _)| *k);
            let Some(k) = oldest else { break };
            if let Some((t, _)) = self.map.remove(&k) {
                self.bytes -= tex_bytes(t.w, t.h);
            }
        }
        view
    }
}

/// LRU готовых кадров композиции: hash плана → (RGBA, complete). Держит
/// прогретый loop быстрым (у GPU-пути нет PostFxVideo-мемоизации CPU).
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
        // бамп recency
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
