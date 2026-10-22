//! Right-click IN THE PANELS — the second registry of targets (R.49, item 363).
//!
//! **Why the registry is SECOND, not an extension of `CtxTarget`.** Timeline
//! targets share one scene (one strip, one hit test by position, one menu
//! drawn at the end of the frame) and one kind of identifier: time and track.
//! Panels have none of this: a chain slot lives in the FX window, a card
//! in the bin, a fader in the meters; each is drawn by its own code and knows
//! its own target. Merging them into one enum would mean a `match` half of whose
//! branches never occur in their own place.
//!
//! **Why a registry at all.** Reconnaissance found twelve places where an object
//! answers a HOVER with a tooltip but answers the right button with nothing. The user
//! learns once: if the right button showed a menu somewhere, he presses it everywhere.
//! Silence reads as “there is no object” — exactly the kind of dead end that
//! cost the rounds R.33–R.40.
//!
//! The `PanelEntity` type exists for the COMPILER: a literal `ALL` of fixed
//! length breaks the build until a new panel entity is named, and test 395 is
//! an exhaustive `match` over the verbs.

use kt_model::ClipId;

/// The panel object that was right-clicked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelTarget {
    /// Chain effect slot: (the clip that owns the chain, slot index).
    FxSlot { chain: ClipId, slot: usize },
    /// Pinned head of the chain (the source of the morpher layer).
    FxHead { chain: ClipId },
    /// Automation curve key: (chain, slot, curve, key).
    CurveKey { chain: ClipId, slot: usize, curve: usize, key: usize },
    /// Free background of the media bin.
    BinEmpty,
    /// Preview window.
    Preview,
    /// Meters panel (scales or fader).
    Meters,
}

/// Kind of panel object — one for each “thing that is controlled”.
// The type exists for the COMPILER and is read by test 395; in the product path it is
// not called, and that is normal.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum PanelEntity {
    FxSlot = 0,
    FxHead,
    CurveKey,
    Bin,
    Preview,
    Meters,
}

#[allow(dead_code)]
impl PanelEntity {
    pub const N: usize = PanelEntity::Meters as usize + 1;
    /// The length is set by `N`: add a variant and the literal no longer matches the
    /// length, and the WHOLE application fails to build, not only the tests.
    pub const ALL: [PanelEntity; Self::N] = [
        PanelEntity::FxSlot,
        PanelEntity::FxHead,
        PanelEntity::CurveKey,
        PanelEntity::Bin,
        PanelEntity::Preview,
        PanelEntity::Meters,
    ];
}

impl PanelTarget {
    /// Which kind the target belongs to. The match is exhaustive: when you add a new target
    /// you must name its kind.
    #[allow(dead_code)]
    pub fn entity(self) -> PanelEntity {
        match self {
            PanelTarget::FxSlot { .. } => PanelEntity::FxSlot,
            PanelTarget::FxHead { .. } => PanelEntity::FxHead,
            PanelTarget::CurveKey { .. } => PanelEntity::CurveKey,
            PanelTarget::BinEmpty => PanelEntity::Bin,
            PanelTarget::Preview => PanelEntity::Preview,
            PanelTarget::Meters => PanelEntity::Meters,
        }
    }
}

impl crate::app::KtApp {
    /// Remember what was right-clicked in the panel.
    ///
    /// Targets CANCEL EACH OTHER: there is one menu on screen, and if a stale
    /// timeline target survived a right click in a panel, a test would read it and
    /// turn green on broken code (and code asking “what is selected there”
    /// would answer with yesterday’s object).
    pub fn set_panel_target(&mut self, t: PanelTarget) {
        self.ctx_panel = Some(t);
        self.ctx_target = None;
    }
}
