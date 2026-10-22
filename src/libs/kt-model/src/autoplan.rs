//! Compiled automation of the chain (R.59, item 467).
//!
//! Before this round the video path's curves were resolved BY NAME on every frame:
//! `layer_src` cloned the whole chain and for each curve called
//! `set_param(&str, …)`, which linearly searched the spec and compared strings.
//! Audio never had this — there curves are compiled ONCE when the processor is built
//! (`AutoProfiles` in `warp.rs`), and only arithmetic remains per block.
//! This module is the same thing for video.
//!
//! What the plan gives besides speed: it EXPLICITLY answers the question “does this clip
//! have live curves”. An empty plan is `None`, and the caller stays on the old
//! borrowing path (zero allocations, the bytes are as before).
//!
//! What the plan does NOT do: it does not touch the GENERATOR's curves. The source itself
//! resolves them by its own tick — a generator frame must remain a pure function
//! (source state, tick, quality), otherwise the cache key stops determining the frame
//! (invariant 4).

use crate::{FxInstance, HasParams, ParamCurve, curve_value};
use kt_time::Tick;

/// One compiled curve: where to write (slot + the INDEX of the passport row)
/// and with what (keys + the owner's clamp limits).
#[derive(Debug, Clone, PartialEq)]
struct PlannedCurve {
    slot: u32,
    idx: u16,
    keys: Box<[crate::AutoKey]>,
}

/// The chain's automation plan. Built when the renderer is constructed, lives as long
/// as it, applied per frame.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoPlan {
    curves: Vec<PlannedCurve>,
}

impl AutoPlan {
    /// Compile the chain's curves. `None` — there are no live curves at all:
    /// not one, all of them empty or all orphaned. This is the gate of
    /// “zero cost without use”.
    ///
    /// Orphans (a name that fits no variant) are discarded HERE — exactly as the name-based
    /// path silently skips them: `set_param` would return `false`.
    pub fn compile(fx: &[FxInstance], auto: &[Vec<ParamCurve>]) -> Option<AutoPlan> {
        let mut curves = Vec::new();
        for (slot, list) in auto.iter().enumerate() {
            let Some(f) = fx.get(slot) else { break };
            for c in list {
                if c.keys.is_empty() || !c.source.is_keys() {
                    continue;
                }
                let Some(idx) = f.param_idx(&c.param) else { continue };
                curves.push(PlannedCurve {
                    slot: slot as u32,
                    idx: idx as u16,
                    keys: c.keys.clone().into_boxed_slice(),
                });
            }
        }
        (!curves.is_empty()).then_some(AutoPlan { curves })
    }

    /// Resolve the curves at the material position `at`.
    ///
    /// The numbers must match the name-based path (`apply_fx_auto`) down to the
    /// last bit: the value is computed by the SAME `curve_value`, while the limit is
    /// applied by the OWNER itself — the clamp lives in one place, in the passport macro,
    /// and the plan has no clamp of its own on purpose.
    /// The guard is a frozen oracle in the test.
    pub fn apply(&self, fx: &mut [FxInstance], at: Tick) {
        for c in &self.curves {
            let Some(f) = fx.get_mut(c.slot as usize) else { continue };
            f.set_param_idx(c.idx as usize, curve_value(&c.keys, at));
        }
    }

    /// The ticks of all the plan's keys — the curve is probed at them where exactly
    /// the breaks matter (for example, the maximum zoom per clip).
    pub fn key_ticks(&self) -> impl Iterator<Item = Tick> + '_ {
        self.curves.iter().flat_map(|c| c.keys.iter().map(|k| k.t))
    }

    /// How many curves were compiled (for tests and diagnostics).
    pub fn len(&self) -> usize {
        self.curves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.curves.is_empty()
    }
}
