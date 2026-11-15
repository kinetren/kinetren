//! Frames left on the compute host's device (stream-front-on-device, found-defects 543): the output
//! of the slots in front of a stream stage, made by an engine on the compute device, handed to the
//! codec's job as `compute::Init::Held` — a copy on the device, no readback, no upload.

use std::sync::{Arc, Mutex};

/// The bytes of free buffers a device keeps for the next frames: a codec batch's worth (its
/// `BATCH_BYTES`), so a walk's batches after the first create nothing. The live ones are held by
/// the front memo and the batch.
const SPARE_BYTES: u64 = 192 << 20;

/// The free buffers of one device, reported to the memory ledger as compute buffers.
pub(crate) struct Spares {
    free: Mutex<(Vec<wgpu::Buffer>, u64)>,
    account: kt_time::mem::Account,
}

impl Default for Spares {
    fn default() -> Spares {
        Spares { free: Mutex::default(), account: kt_time::mem::open(kt_time::mem::Tenant::Compute, SPARE_BYTES) }
    }
}

impl Spares {
    /// A buffer of exactly `size` bytes: a spare, else a new one (`crate::created`).
    pub(crate) fn take(&self, device: &wgpu::Device, size: u64) -> wgpu::Buffer {
        let mut free = self.free.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = free.0.iter().position(|b| b.size() == size) {
            let b = free.0.swap_remove(i);
            free.1 -= size;
            self.account.set(free.1);
            return b;
        }
        drop(free);
        crate::created();
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kt-gpu held frame"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn give(&self, b: wgpu::Buffer) {
        let mut free = self.free.lock().unwrap_or_else(|e| e.into_inner());
        if free.1 + b.size() <= SPARE_BYTES {
            free.1 += b.size();
            free.0.push(b);
            self.account.set(free.1);
        }
    }
}

/// What `compute::Held::inner` is for this crate: the buffer and the device it lies on.
pub(crate) struct HeldBuf {
    pub(crate) buf: Option<wgpu::Buffer>,
    pub(crate) device: Arc<wgpu::Device>,
    spares: Arc<Spares>,
}

impl Drop for HeldBuf {
    fn drop(&mut self) {
        if let Some(b) = self.buf.take() {
            self.spares.give(b);
        }
    }
}

/// Wrap `buf` (`len` bytes of RGBA) as a held frame of `device`.
pub(crate) fn held(buf: wgpu::Buffer, len: u64, device: Arc<wgpu::Device>, spares: Arc<Spares>) -> kt_plugin_api::compute::Held {
    kt_plugin_api::compute::Held { len, inner: Arc::new(HeldBuf { buf: Some(buf), device, spares }) }
}

/// The buffer of `h` when it lies on `device`.
pub(crate) fn on<'a>(h: &'a kt_plugin_api::compute::Held, device: &Arc<wgpu::Device>) -> Option<&'a wgpu::Buffer> {
    let b = h.inner.downcast_ref::<HeldBuf>()?;
    Arc::ptr_eq(&b.device, device).then_some(b.buf.as_ref()?)
}
