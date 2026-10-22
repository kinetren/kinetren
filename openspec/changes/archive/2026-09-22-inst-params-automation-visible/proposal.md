# Proposal

## Why

The owner (R.60, 2026-09-22) published an effect parameter outward from a composition and
discovered that in the "Composition parameters" window of the instance one can "put different
values in different places", and it turns out to be "something like automation of curves", while
"I don't see any automation curves anywhere… there isn't a single button in the automation window".
Second: "the min/max values are always set to 0 by default".

The reconnaissance confirms both: in the consumer window (`src/apps/kt-app/src/pubparam.rs`,
`draw_one_inst_params`) a numeric handle has an unnamed glyph button "+", which creates a curve with one
key under the cursor; after that EVERY edit of the field puts a key at the cursor position
(`bound_apply` with `key = None`), and the curve itself is not in the window — no graph, no key table,
no hint in the status line. This is a hidden automation recording mode, and it violates the decision of
R.34 "one animation mechanism for everything": the FX editor and the generator draw curves with the shared
widget (`automation::draw`, t221), while the instance window bypassed it.
Limits in the author window: `PubRange` stores an `Option`, and the field draws `NaN` as `0.0` — an empty
narrowing looks like "0", although the target has its own passport (`ParamSpec.min/max/default`), and it
is that which must be the default.

## What Changes

- The "Composition parameters" window of the instance draws the override curves with THE SAME automation
  widget as the FX editor (graph ↔ table, key at the cursor, copy-paste of envelopes), with a labelled
  button "Automation: add a curve…" instead of the glyph "+". A key is placed at the cursor only when the
  curve EXISTS and is VISIBLE; without a curve, editing the field is a constant.
- The time axis of the widget is the material of the instance (the reel's time), with the same axis lock
  for the whole program and the same cursor highlight as for the slot.
- The handle limits in the author window: the default is the limits of the TARGET by its passport
  (`min/max`), the field shows them rather than 0; only the NARROWING is written to the file (values equal
  to the target's limits remain `None`); reset to the target's limits — with one action. For this,
  `Project::addr_spec` appears in kt-model (the passport by address — a mirror of `addr_type`).
- The value field of the consumer is limited by the limits of the target, and on top of them by the
  author's narrowing (now without narrowing the limit is ±1e9).
- Status-line hints for the new controls (rule 10: no pop-ups).
- **BREAKING (UI behaviour, not the format):** the glyph "+" and the writing of keys without a visible curve
  disappear; the `.otio` file does not change (`Override.curves` already exists, v9).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `compositions`: the requirement "The publication interface — one door, the author's name verbatim, two
  badges" — the consumer window is obliged to SHOW the override curves with the automation widget; hidden
  recording of keys does not exist; the requirement "Public parameters — the template value is not stored…" —
  the default narrowing of the handle limits equals the passport of the target and is stored only as a
  narrowing.
- `automation`: the new requirement "Curves of any carrier are shown by one widget" — the carriers (slot,
  generator, NESTING) draw curves with one code; the passport by address (`addr_spec`) is the only source of
  limits and defaults for publication.

## Impact

- Code: `src/apps/kt-app/src/pubparam.rs` (the consumer window and the author window),
  `src/apps/kt-app/src/automation.rs` (no API changes, reuse), `src/libs/kt-model/src/project.rs`
  (`addr_spec`), the catalogues `i18n/*.toml` (new strings ×5 languages, subsets when new glyphs appear),
  `src/scripts/live/nestparam.py` (the step with a curve).
- Tests: t683 changes (the widget instead of the glyph), new t691–t694 (see specs), the reach tests
  t252–t255 must remain green (any UI edit), t392/t393 (the promises of the hints).
- The project format does not change; `GOLDEN_*` are not touched.
