//! The preview's picture on the device (preview-throughput D2): the composite goes into a texture
//! of the GPU engine, and this registers that texture with egui's renderer. Nothing comes back to
//! the CPU and goes up again; `draw_preview` falls back to the bytes path when there is no engine
//! (the harness) or a frame the executor refused.

use eframe::egui::TextureId;
use eframe::egui_wgpu::{RenderState, wgpu};
use std::sync::Arc;

pub(crate) struct PreviewDisplay {
    engine: Arc<kt_gpu::GpuFxEngine>,
    rs: RenderState,
    id: Option<TextureId>,
    glow: Option<TextureId>,
    /// What `id` shows now: (the target's generation, the filter).
    registered: Option<(u64, wgpu::FilterMode)>,
}

impl PreviewDisplay {
    pub fn new(engine: Arc<kt_gpu::GpuFxEngine>, rs: RenderState) -> PreviewDisplay {
        PreviewDisplay { engine, rs, id: None, glow: None, registered: None }
    }

    /// The target after a `present`: registered again only when it is a new texture or the
    /// filter changed. A frame costs one comparison here (invariant 12).
    pub fn sync(&mut self, smooth: bool) {
        let filter = if smooth { wgpu::FilterMode::Linear } else { wgpu::FilterMode::Nearest };
        let Some(t) = self.engine.display_target() else { return };
        if self.registered != Some((t.generation, filter)) {
            let mut r = self.rs.renderer.write();
            match self.id {
                Some(id) => {
                    r.update_egui_texture_from_wgpu_texture(&self.rs.device, &t.view, filter, id)
                }
                None => self.id = Some(r.register_native_texture(&self.rs.device, &t.view, filter)),
            }
            self.registered = Some((t.generation, filter));
        }
        if self.glow.is_none()
            && let Some(v) = self.engine.glow_view()
        {
            let mut r = self.rs.renderer.write();
            self.glow =
                Some(r.register_native_texture(&self.rs.device, &v, wgpu::FilterMode::Linear));
        }
    }

    pub fn id(&self) -> Option<TextureId> {
        self.id
    }

    pub fn glow_id(&self) -> Option<TextureId> {
        self.glow
    }
}

impl Drop for PreviewDisplay {
    fn drop(&mut self) {
        let mut r = self.rs.renderer.write();
        for id in [self.id, self.glow].into_iter().flatten() {
            r.free_texture(&id);
        }
    }
}
