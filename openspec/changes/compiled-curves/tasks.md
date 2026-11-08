# Tasks: compiled-curves

## 1. Compiled formulas
- [ ] 1.1 `Formula::compile` → closures over slots, bit for bit with `eval` (t2100)
- [ ] 1.2 Interned segment formulas compiled once per text; `SegShape::Formula` without lock/alloc (t2101)
- [ ] 1.3 `kt_time::cost` rows; a sampled segment-formula curve parses once (t2102)

## 2. Speed profiles once per content
- [ ] 2.1 `rate_profile_keys` by a cursor and a compiled layer formula, bit for bit (t2103)
- [ ] 2.2 Content-keyed bounded memo of the sampled profile and the clip warp map (t2104, t2105)
- [ ] 2.3 Count guard: a rebuild of an unchanged project samples no profile (t2106)

## 3. LFO phase
- [ ] 3.1 `ParamIntegral` prefix sums, bit for bit with `param_integral` (t2107)
- [ ] 3.2 Modulation plugins bind it; count per sample O(log n) (t2108)

## 4. Links and stream rows
- [ ] 4.1 `resolve_curve`, `formula_preview`: compiled formulas, cursor; no clone of events without derived curves (t2109)
- [ ] 4.2 `rows_at` applies one slot (t2110)

## 5. Contracts and numbers
- [ ] 5.1 Spec deltas and design (the class walked)
- [ ] 5.2 Release numbers before/after (relinked copy, showcase), CPU at rest
- [ ] 5.3 found-defects rows, What's new if noticeable

## 6. Check
- [ ] 6.1 `cargo test` of the touched crates, clippy, target-prune
