# Tasks: compiled-curves

## 1. Compiled formulas
- [x] 1.1 `Formula::compile` → closures over slots, bit for bit with `eval` (t2100)
- [x] 1.2 Interned segment formulas compiled once per text; `SegShape::Formula` without lock/alloc (t2101)
- [x] 1.3 `kt_time::cost` rows; a sampled segment-formula curve parses once (t2102)

## 2. Speed profiles once per content
- [x] 2.1 `rate_profile_keys` by a cursor and a compiled layer formula, bit for bit (t2103)
- [x] 2.2 Content-keyed bounded memo of the sampled profile and the clip warp map (t2104, t2105)
- [x] 2.3 Count guard: a rebuild of an unchanged project samples no profile (t2106)

## 3. LFO phase
- [x] 3.1 `ParamIntegral` prefix sums, bit for bit with `param_integral` (t2107)
- [x] 3.2 Modulation plugins bind it; count per sample O(log n) (t2108)

## 4. Links and stream rows
- [x] 4.1 `resolve_curve`, `formula_preview`: compiled formulas, cursor; no clone of events without derived curves (t2109)
- [x] 4.2 `rows_at` applies one slot (t2110)

## 5. Contracts and numbers
- [x] 5.1 Spec deltas and design (the class walked)
- [x] 5.2 Release numbers before/after (relinked copy, showcase, a synthetic dense-curve project), CPU at rest
  (machine load 50–54 throughout; counts are exact, times are best of three where marked):
  - relinked copy, per build: audio walked warp_segments 96 020 → 27 752, curve_walks 145 212 → 5 038;
    video warp_segments 166 968 → 78, curve_walks 229 114 → 3 374. Times: a rebuild of the unchanged
    project (best of 3) effective_clips 5.5–5.6, audio 8.8–9.0, video 13.7–15.3 ms against 5.9–6.9,
    9.2–11.2, 15.8–16.6 ms recompiling every curve — the curves were already a small part after
    builder-cost-guard. `kt-cli probe` prints the rebuild line now.
  - heavy showcase (no dense curves): builds unchanged (1 / 1 / 6 ms), rebuild video 5.5–7.1 ms against
    5.6–11.6.
  - dense project (the showcase with a 57 s event under a formula-layer speed curve and a tremolo whose
    frequency is a formula layer, `dense.kt`): live audio render 364 → 38 ms for 8 s (5 % → 0 % of real
    time), loop 1387 → 148 ms for 32 s; `AudioRenderer::new` 6 → 1 ms, `VideoRenderer::new` 11 → 7 ms.
  - CPU at rest (live `idle`, the heavy showcase, debug kt-app): pause 2 % of a core and nothing ran, an open
    effects window 2 %, the engine threads 0 % over the deck panel; one run red at load ~50 on a UI-frame check
    after hovering the preview, green on the rerun (found-defects row 162).
  - bytes: `kt-cli render` of `dense.kt` (104 s) identical between the two binaries; the relinked copy's
    whole `kt-cli render` (8087 s, 3.1 GB, its 9 missing files skipped) identical too.
- [x] 5.3 found-defects rows, What's new if noticeable

## 6. Check
- [x] 6.1 `cargo test` of the touched crates, clippy, target-prune
