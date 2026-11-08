# Design: compiled-curves

Row 2 of the order table of `docs/engine-audit.md` (M2, M7; JIT tier T1, §3.1). The owner's go,
2026-11-09: «запускай compiled-curves и pipeline-prewarm».

## The mechanism

A curve, a speed profile, a link or a formula is a function of a few inputs that change only when a
person edits them. The code re-derived it at the rate of its READERS — per sample, per frame, per
build, per consumer — instead of at the rate of its EDITS. It is invisible on hand-made data (a curve
of four keys) and grows with generated data (a 5 ms grid, a key per frame).

## Decisions

### D1. Formulas compile to closures over slots (T1, not T3)
`Formula::compile(vars)` resolves names to slot indices once and returns a tree of boxed closures.
The interpreter and the compiled form share ONE body for the binary operators (`bin`) and the
functions (`call`), so an operation cannot drift between the two. The evaluation order is the
interpreter's: operands left to right, the first error wins, `if` evaluates one branch, an unknown
name fails when evaluated (a text with an unknown name is refused by validation anyway). Rejected:
a flat bytecode with a value stack (needs jumps for `if`, gains nothing measurable over closures at
our formula sizes); Cranelift (T3; §3.3 of the audit: nothing measured is bound by interpretation
once the map and the allocations are gone).

Interned segment formulas (`ExprId`) keep their compiled form in a thread-local table indexed by the
handle: the interner only grows, so the index is stable, and the reading takes no lock. A text that
does not parse is remembered as such (the segment reads as a straight line, as before).

### D2. A speed profile is memoized by its content, exactly
Two stores in `warp.rs`: the material-fraction points of a speed curve (`rate_points`, read by the
audio chain through `AutoProfiles` and by `clip_warp_of`) and the whole `ClipWarp`. The key is a
word vector of exactly what the function reads (D2a); a hit compares the whole vector, so a stale
or colliding entry is impossible (the owner's cache rule, 2026-11-01: "keyed by what determines the
result, never a guess"). `ClipWarp` holds its points and integral behind `Arc`, so a hit is a
pointer copy.

- **D2a — what determines the profile.** `rate_profile_keys` reads the keys (tick, value, shape and
  its parameters; a segment formula by its interned handle — one handle per text for the process,
  the memo's lifetime), and of the source only the layer formula's text when there is no link. The
  points add the material window (`src_in_frames`, `clip_len_frames`) and the reverse; the map adds
  the base speed, the blend and the span in ticks. A link-driven speed curve arrives already
  resolved into keys by `effective_clips`, so its keys are its content.
- **D2b — bounded.** Two generations of `MEMO_POINTS` = 2¹⁸ points each: a generation that fills
  becomes the old one (the previous old is dropped), a hit in the old one moves to the new. A memory
  bound, not a tuning to a machine: ≈ 56 B a point for a map, ≈ 15 MB a generation at most; the
  relinked copy's working set is ~50 000 points.
- **D2c — computed outside the lock.** A long profile must not hold another consumer's build.
- **D2d — determinism.** A hit returns the value the pure function computed for the same key, so the
  bytes are identical with and without the store (t2104, t2105). `without_profile_memo` bypasses it
  on one thread: the two-path tests use it, and so does `build_cost.rs`, whose counts must be a
  function of the scene, not of what another test left in the shared store.

Rejected: a thread-local store (the builder thread, prefetch and the UI would each compute once —
and the audio builder thread is the one an edit waits for); a 64-bit content hash as the key (could
collide silently); memoizing `sign_knots`/`rate_signed` too (linear since builder-cost-guard; a
second store for the audio address map is the next step if a profile shows it, row below).

### D3. A cursor for in-order readings
`CurveCursor` keeps the segment index and steps forward; a position that goes back is found by
halving again. Both it and `curve_value` end in the same `segment_value`, so a reading is the same
bits whatever the order (t2101). Used where a curve is swept: the speed-curve sampling, the formula
layer's `k`, the link resolution's `k`, the formula preview.

### D4. The LFO phase by a prefix integral
`ParamIntegral` in `kt-plugin-api::curves` (a plugin cannot reach kt-model): per non-empty segment
its right end, its left point and the running sum before it, added exactly as the former loop adds a
whole segment (`hi = t1`, the same `v_hi` expression — which is not always `v1` in floating point).
A reading finds the first segment whose end reaches `t` by halving and adds the loop's partial term:
the loop's bits (t2107). An unsorted point list (never built so) reads through the walk. `LfoDrive`
rebuilds it only when the bound points change.

### D5. Links: what an edit did not make pending is not touched
`resolve_derived` skips, per pass, an event with no curve `resolve_curve` would change; a speed curve
without a link is left to `rate_profile_keys` (t913) and so no longer clones the reel by itself.

### D6. Stream stage rows
`rows_at` cloned the whole chain per position to read one slot; `AutoPlan::apply_slot` applies the
slot's own curves with the same `curve_value` and the same owner's clamp (t2110).

## The class walked (every subsystem that evaluates curves)

| Subsystem | Reader | Before | Now |
|---|---|---|---|
| Speed / warp (model) | `clip_warp` in `clip_slice`, trim helpers, `material_end_on_timeline`, `clip_material_span` | sampled + laid out per call | memoized by content (D2) |
| Speed / warp (audio build) | `AutoProfiles::from_curves_rev` | sampled per clip per build | memoized points (D2); `sign_knots`/`rate_signed`/`chain_tail` still lay segments once per build — linear, **left** (row 167) |
| Speed / warp (video build) | `clip_warp_of` per clip | sampled + 3 integrals per clip per build | memoized map |
| Speed sampling | `rate_profile_keys` | interpreter + map per grid point, halving per point | compiled formula, cursor (D1, D3) |
| Segment formulas | `SegShape::at_t` | lock + text clone + string lookup + map of 4 names | compiled per text, thread-local (D1) |
| Audio automation | `AutoProfiles::value`/`param_at`, `SlotCurves::value` | halving per block; name `find` among the slot's curves | unchanged (O(log n) a block) |
| LFO (6 modulation nodes) | `param_integral` per sample | O(points) a sample | prefix integral (D4) |
| Video automation | `AutoPlan::apply` | halving per frame (builder-cost-guard); chain clone per animated layer per frame | unchanged; the per-frame chain is the plan's owned `LayerSrc.fx` — **left** to the frame-plan template (row 168) |
| Stream stage rows | `rows_at` | chain clone per position | one slot (D6) |
| Decode-zoom probes | `max_zoom` probes, `owned.clone_from_slice` per key tick | linear in key ticks | unchanged (bounded by keys) |
| Links | `resolve_curve` | interpreter + map per frame; every event cloned per pass | compiled, cursor, pending events only (D1, D3, D5) |
| Formula preview (UI) | `formula_preview` | interpreter per column | compiled, cursor |
| Curve graphs (UI) | `curve_value` per pixel | halving; segment formula interpreted | segment formula compiled |
| Morpher | `apply_to`, `clip_slice` per piece; `morph_pass` ~5× per build | warp resampled per piece | warp memoized; the repeated passes stay — **row 155** (`edit-without-rebuild`) |
| Compositions' public params | `apply_overrides` → `resolve_curve_in` | interpreter per frame | compiled (through D1) |
| Generators | `GenState::kind_at` per frame | kind clone + name-addressed `set_param` per curve | unchanged — bounded by params, not by curve length; **left** (row 169) |
| Transitions | `TransAnim::mix_at` per frame | kind clone + name-addressed `set_param` | as generators (row 169) |
| Cameras, masks | `params_at`, `ClipMask::at` per frame | name-addressed, halving | as generators (row 169) |

## Measurements

See `tasks.md` §5.2 (release `kt-cli probe … --time`, the relinked copy and the heavy showcase).
