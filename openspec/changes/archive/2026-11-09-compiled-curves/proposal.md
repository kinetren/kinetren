# Proposal: compiled-curves

## Why

`engine-audit` (`docs/engine-audit.md` §3, row 2 of the order table; mechanisms M2 and M7) found that
curves, speed profiles, links and formulas are re-derived per sample, per frame and per build
instead of being compiled once per revision of what determines them. The owner's go, 2026-11-09:
«запускай compiled-curves и pipeline-prewarm»; his JIT verdict (H16/H17): «про JIT я вообще-то имел
ввиду не только шейдеры» — compiled CPU math is JIT tier T1. He also forbade tuning to one machine:
every limit here is a general rule (O(1)/O(log n) per question, memo keyed by content), proved by
counts on generated data and by bytes.

`builder-cost-guard` already made `sign_knots` linear, `curve_value` a halving and the link leader a
single lookup. What is left of the class:
- a segment formula (`SegShape::Formula`) is re-interpreted per evaluation: a global mutex, a text
  clone, a hash lookup, a `BTreeMap<String,f64>` of four allocated names and a boxed-tree walk with a
  `Vec` per call node — per sampled 5 ms point of a speed curve, per frame of a link, per pixel of a
  graph;
- a layer or link formula is evaluated the same way per grid point (`rate_profile_keys`,
  `resolve_curve`, `formula_preview`);
- a speed profile is sampled and laid out again by every `clip_warp` — per piece in `clip_slice`, per
  trim helper, per morph pass, per build, per consumer (2 audio + 2–3 video builds per edit): 96 020
  and 166 968 segments per audio/video build on the relinked copy, all for unchanged curves;
- an LFO's phase walks its frequency curve from the first point per sample (row 152);
- a stream stage clones the whole chain per position to read one slot (`rows_at`).

## What Changes

- **Compiled formulas**: `Formula::compile(vars)` → closures over numbered slots; the same IEEE
  operations in the same order as the interpreter, errors included. Interned segment formulas are
  compiled once per text (`ExprId`), no lock and no allocation per evaluation.
- **Speed profiles compiled once per content**: `rate_profile_keys` reads the curve by a cursor and the
  layer formula compiled; the sampled profile and the clip's warp map are memoized by the content
  that determines them (the curve's keys and source, the material window, the base, the reverse) in a
  bounded two-generation store shared by every thread. Exact keys, no hash guess; the same bytes with
  and without the memo.
- **LFO phase by a prefix integral** (`ParamIntegral` in the plugin API): partial sums kept in the
  former order of addition, a reading by halving.
- **Links**: compiled formulas, the leader's curve found once per curve; an event with nothing to
  resolve is not cloned per pass.
- **Stream stage rows**: the plan applies one slot's curves, no chain clone per position.
- Two-path byte tests (compiled vs interpreted, memo vs fresh) and count tests (`kt_time::cost`,
  new rows `formula_parses`, `profile_samples`, `integral_walks`) that fail on per-sample or per-build
  re-derivation.

## Capabilities

- `automation`: formulas are compiled once per text; derived curves resolve without per-sample
  interpretation.
- `time-model`: a speed profile is compiled once per content and memoized; readings are the same bits.
- `audio-fx`: an LFO's phase is read from a prefix integral.

## Impact

`kt-time` (cost rows), `kt-model` (`formula.rs`, `project.rs`, `warp.rs`, `links.rs`, `autoplan.rs`),
`kt-plugin-api` (`curves.rs`, `dsp.rs`), `fx-audio-modulation`, `kt-engine` (`video_stream.rs`).
Tests t2100–t2119.
