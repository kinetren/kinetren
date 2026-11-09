//! The cost meter of builders (builder-cost-guard): how many elements the walks over a whole
//! structure touched, on THIS thread. A build is a pure function of the project, so the count is
//! too — a guard reads it at a project ×1, ×4 and ×16 and holds its growth to a declared bound,
//! with no clock in the check (t1671).
//!
//! Why it exists: a helper that rebuilds or rescans a whole structure, called once per element of
//! another, is invisible on hand-made data and explodes on generated data (a speed curve sampled
//! every 5 ms, a reel placed 400 times). `warp::sign_knots` rebuilt the segment list per sampled
//! point and took 393–488 ms of every build on the owner's project; nothing counted it.
//!
//! The rule for a new walk: a function that visits every element of something the project can
//! make long (events, keys, sampled points, placements) adds what it visited here. A relaxed
//! thread-local add and nothing else; it steers nothing (invariant 11).

use std::cell::Cell;

/// One kind of walk. Append only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Cost {
    /// A segment of a speed profile laid out (`warp::segments`): a reading of the map that lays
    /// the list out per call costs the whole curve.
    WarpSegment,
    /// An event looked at by a search over a reel's events (a transition's ends, a clip by id).
    ClipSearch,
    /// An event (of any depth) fed into a reel's revision hash.
    ReelHash,
    /// A key or point walked by a curve reading that starts from the curve's beginning.
    CurveWalk,
    /// An event of a reel walked by a pass over the whole reel (resolve, layers, layout).
    ReelPass,
    /// A formula text parsed (compiled-curves): a text is parsed once per text and evaluated
    /// compiled; a parse per sample or per frame is the interpreter coming back.
    FormulaParse,
    /// A point of a speed profile sampled or laid out (compiled-curves): the profile is compiled once
    /// per content and memoized, so a rebuild of an unchanged project samples none.
    ProfileSample,
    /// A segment walked by an integral reading that starts from the curve's beginning (an LFO phase
    /// per sample before compiled-curves).
    IntegralWalk,
    /// An event whose derived state a builder computed (edit-without-rebuild): the audio event or the
    /// video layer before transitions. The `engine-build` thread keeps them from one build to the
    /// next by the hash of what they derive from, so a rebuild after an edit derives the touched
    /// events only.
    EventDerive,
    /// A whole renderer built (`AudioRenderer::new`, `VideoRenderer::new_with_facts`, the player's
    /// shared build). A UI path counts none.
    RendererBuild,
    /// A reel instance resolved (its morpher layers, links, overrides) by a builder: once per
    /// instance per build (edit-without-rebuild, found-defects row 155).
    ReelResolve,
}

impl Cost {
    pub const ALL: [Cost; 11] = [
        Cost::WarpSegment,
        Cost::ClipSearch,
        Cost::ReelHash,
        Cost::CurveWalk,
        Cost::ReelPass,
        Cost::FormulaParse,
        Cost::ProfileSample,
        Cost::IntegralWalk,
        Cost::EventDerive,
        Cost::RendererBuild,
        Cost::ReelResolve,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Cost::WarpSegment => "warp_segments",
            Cost::ClipSearch => "clip_searches",
            Cost::ReelHash => "reel_hashes",
            Cost::CurveWalk => "curve_walks",
            Cost::ReelPass => "reel_passes",
            Cost::FormulaParse => "formula_parses",
            Cost::ProfileSample => "profile_samples",
            Cost::IntegralWalk => "integral_walks",
            Cost::EventDerive => "event_derives",
            Cost::RendererBuild => "renderer_builds",
            Cost::ReelResolve => "reel_resolves",
        }
    }
}

/// The counts of one measurement, in the order of [`Cost::ALL`].
pub type Costs = [u64; Cost::ALL.len()];

thread_local! {
    static COUNTS: [Cell<u64>; Cost::ALL.len()] = const { [const { Cell::new(0) }; Cost::ALL.len()] };
}

#[inline]
pub fn add(c: Cost, n: u64) {
    COUNTS.with(|k| k[c as usize].set(k[c as usize].get().wrapping_add(n)));
}

/// The running totals of this thread.
pub fn totals() -> Costs {
    COUNTS.with(|k| std::array::from_fn(|i| k[i].get()))
}

/// Runs `f` and returns what it counted on this thread.
pub fn meter<R>(f: impl FnOnce() -> R) -> (R, Costs) {
    let before = totals();
    let r = f();
    let after = totals();
    (r, std::array::from_fn(|i| after[i].wrapping_sub(before[i])))
}
