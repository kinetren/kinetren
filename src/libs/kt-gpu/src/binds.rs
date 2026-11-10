//! The executor's per-pass bookkeeping, kept per frame instead of per pass (local-cost-queue, row 138):
//! a bind group per pass cost 6.7 µs and a uniform write 2.2 µs on the composing thread (llvmpipe,
//! release), ~0.25 ms a frame of a heavy chain.
//!
//! - `BindCache`: the bind groups of the frames, keyed by the layout and the views they bind
//!   (`wgpu::TextureView` compares by its id). The uniform arena is bound with a dynamic offset, so
//!   a slot is not part of the key; a new arena empties the cache. Each top-level frame drops the
//!   entries the previous frame did not use, and the cache never holds more than `CAP`: a texture its
//!   owner released lives at most until the next frame starts.
//! - `Staged`: a frame's uniform bytes, written into the arena with one `write_buffer` before each
//!   submission (a frame the arbiter cuts flushes before each piece). Slots are allocated in
//!   ascending order inside a frame, so the staged bytes are one run; the gaps between a pass's
//!   bytes and its slot's end are written as zeros, which no pass reads.

use std::collections::HashMap;

/// The most bind groups kept; past it the cache starts over (a frame binds a few dozen).
pub const CAP: usize = 256;

/// Which layout a bind group is for.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Layout {
    /// The ordinary passes: the 128-byte uniform and three textures.
    Main,
    /// The geometry pass and the point-wise runs: the 1024-byte uniform and one texture.
    Geom,
}

/// A bind group's key: its layout and the views it binds (`None` past the layout's textures).
type Key = (Layout, [Option<wgpu::TextureView>; 3]);

#[derive(Default)]
pub struct BindCache {
    map: HashMap<Key, (wgpu::BindGroup, u64)>,
    frame: u64,
    /// Bind groups created and reused (tests: t2314).
    pub made: u64,
    pub reused: u64,
}

impl BindCache {
    /// A top-level frame starts: what the previous one did not bind goes.
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        let floor = self.frame.saturating_sub(1);
        self.map.retain(|_, (_, used)| *used >= floor);
    }

    /// The arena was replaced: every bind group names the old one.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// The bind group of `layout` over `views`, made by `make` when none is kept.
    pub fn get(
        &mut self,
        layout: Layout,
        views: [Option<&wgpu::TextureView>; 3],
        make: impl FnOnce() -> wgpu::BindGroup,
    ) -> wgpu::BindGroup {
        let key = (layout, views.map(|v| v.cloned()));
        if let Some((bg, used)) = self.map.get_mut(&key) {
            *used = self.frame;
            self.reused += 1;
            return bg.clone();
        }
        if self.map.len() >= CAP {
            self.map.clear();
        }
        let bg = make();
        self.made += 1;
        self.map.insert(key, (bg.clone(), self.frame));
        bg
    }
}

/// A frame's uniform bytes, waiting for the next submission.
#[derive(Default)]
pub struct Staged {
    base: u64,
    bytes: Vec<u8>,
}

impl Staged {
    /// `data` at arena offset `offset` (ascending within a frame).
    pub fn put(&mut self, offset: u64, data: &[u8]) {
        // a new frame (or one an error abandoned before its submission) starts a new run
        if self.bytes.is_empty() || offset < self.base + self.bytes.len() as u64 {
            self.bytes.clear();
            self.base = offset;
        }
        let at = (offset - self.base) as usize;
        if at > self.bytes.len() {
            self.bytes.resize(at, 0);
        }
        self.bytes.truncate(at);
        self.bytes.extend_from_slice(data);
    }

    /// Everything staged into `arena`, in one write.
    pub fn flush(&mut self, queue: &wgpu::Queue, arena: &wgpu::Buffer) {
        if !self.bytes.is_empty() {
            queue.write_buffer(arena, self.base, &self.bytes);
            self.bytes.clear();
        }
    }
}
