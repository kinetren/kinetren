//! Transition cores (R.33 C4): the dispatch to the kinds' kernels and the packing of parameters for
//! the GPU.
//!
//! Invariant 7 requires ONE composition code for the preview and export, and the backend
//! counts as execution. Therefore the kind of transition is turned into a `TransMix`
//! — a flat pair (mode, 8 numbers) — EXACTLY once, here; the kind's WGSL (`trans_<id>`, assembled
//! into `kt-gpu/src/shaders/composite.wgsl`) reads them. Every kind but the crossfade is a plugin since plugin-api-stage1 (D12):
//! its kernel lives in its family crate under `src/plugins/transitions-*`, and an edit of a formula
//! goes into its Rust and its WGSL in one commit (t224).
//!
//! The crossfade (`mode == 0`) does NOT enter here: it stays
//! on the integer path `dissolve_into`, byte-for-byte as before
//! (GOLDEN_VIDEO does not move).

use kt_model::TransitionKind;

/// The crossfade's mode: the integer dissolve, never a kernel.
pub const MODE_CROSSFADE: u32 = 0;
/// A plugin kind's mode is `MODE_PLUGIN + its index` in the registry's transition list (D12) —
/// a function of the registry, never hashed into a key (`plan_key` carries the kind's id).
pub const MODE_PLUGIN: u32 = 64;

/// The installed transition kernels by their mode's index, resolved once (the registry does not
/// change after the install, and a pixel must not search it).
fn plugin_kernels() -> &'static [(&'static str, &'static dyn kt_plugin_api::TransKernel)] {
    static K: std::sync::OnceLock<Vec<(&'static str, &'static dyn kt_plugin_api::TransKernel)>> = std::sync::OnceLock::new();
    let build = || -> Vec<(&'static str, &'static dyn kt_plugin_api::TransKernel)> {
        kt_model::plugin_slot::registry()
            .transitions()
            .into_iter()
            .filter_map(|p| Some((&*p.manifest().id, p.trans()?)))
            .collect()
    };
    // a list built before the install would be empty for good: only a filled one is kept
    if let Some(k) = K.get() {
        return k;
    }
    let k = build();
    if k.is_empty() {
        return Box::leak(k.into_boxed_slice());
    }
    K.get_or_init(|| k)
}

/// The kernel of a plugin mode.
fn plugin_kernel(mode: u32) -> Option<&'static dyn kt_plugin_api::TransKernel> {
    plugin_kernels().get(mode.checked_sub(MODE_PLUGIN)? as usize).map(|(_, k)| *k)
}

/// The kind id of a plugin mode — what a cache key carries instead of the mode number, which is
/// a function of the registry's order (D12).
pub fn plugin_id(mode: u32) -> Option<&'static str> {
    plugin_kernels().get(mode.checked_sub(MODE_PLUGIN)? as usize).map(|(id, _)| *id)
}

/// The resolved transition for the executors: mode + 8 numbers (exactly the f0/f1
/// uniforms of the GPU). The fields have no semantics of their own — the kind's kernel defines it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransMix {
    pub mode: u32,
    pub p: [f32; 8],
}

impl Default for TransMix {
    fn default() -> TransMix {
        TransMix::CROSSFADE
    }
}

impl TransMix {
    pub const CROSSFADE: TransMix = TransMix { mode: MODE_CROSSFADE, p: [0.0; 8] };

    pub fn is_crossfade(&self) -> bool {
        self.mode == MODE_CROSSFADE
    }

    /// The only translation “kind from the model → numbers for the cores”.
    pub fn of(kind: &TransitionKind) -> TransMix {
        match kind {
            TransitionKind::Crossfade => TransMix::CROSSFADE,
            TransitionKind::Plugin(slot) => {
                match plugin_kernels().iter().position(|(id, _)| *id == slot.id) {
                    Some(i) => TransMix { mode: MODE_PLUGIN + i as u32, p: plugin_kernels()[i].1.pack(&slot.values) },
                    None => TransMix::CROSSFADE,
                }
            }
            // a kind this build lacks plays as the crossfade
            TransitionKind::Missing(_) => TransMix::CROSSFADE,
        }
    }
}

/// Precomputed frame scalars for VOLUMETRIC kinds (R.51, item 401): inverse homographies of the
/// panels, computed by the kind's `ext` in f64 and rounded ONCE to f32. The CPU kernel and WGSL read
/// the same bits — parity by construction, a separate f64 reference is not needed. They go into the
/// field `m` of the composite's uniform (free since R.44). For the other kinds — zeros; they are not
/// and must not be in the cache keys: ext is a deterministic function of the already hashed
/// (kind, p, k) — see the comment at `put_mix` in kt-gpu.
pub fn trans_ext(m: &TransMix, k: u32) -> [f32; 16] {
    plugin_kernel(m.mode).map_or([0.0; 16], |kern| kern.ext(&m.p, k))
}

