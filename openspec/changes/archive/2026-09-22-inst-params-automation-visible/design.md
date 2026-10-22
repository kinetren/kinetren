# Design

## Context

See proposal.md — Why. What exists in the code today:

- The consumer window `draw_one_inst_params` (`src/apps/kt-app/src/pubparam.rs`) draws a row
  per handle with its own `draw_value_row`; next to a number there is the glyph `icons::PLUS`,
  which creates a `ParamCurve` with one key at `at` (the instance's material via
  `clip_material_at`), after which `DragValue` writes through
  `widgets::bound_apply(curve, None, at, v)` — the key at the cursor.
  There is no `automation::draw` widget in the window.
- The shared widget `automation::draw(ui, id, &AutoCtx, curves, &mut AutoView, playhead, hover)`
  with `AutoCtx { specs, range, view, linked, to_timeline, to_curve, frame_step }`. The generator
  on an event (`gen_ui.rs`, lines ~700–990) is the model for attaching a carrier WITHOUT a
  `FxInstance`: `CurveBind { curves, sel: view.sel, at }` for the fields and the same `AutoCtx`
  for the graph; `axis_linked` is read from `UiState`, the requests `set_linked`/`set_view`/`seek`
  come from `AutoResult`.
- The author window draws `min`/`max` from `PubRange` (`Option<f64>`), `NaN → 0.0`. There is no
  passport by address: there is only `Project::addr_type`/`addr_value` (`project.rs` ~6889–6950)
  with a `match` over `ParamOwner`.
- The owner's decision of R.59: the instance overrides the published item "by a value or by a curve
  in its own time"; curves are on numbers and colours. This remains, only the INTERFACE changes.

## Goals / Non-Goals

**Goals:**
- The override curve is visible and controlled by the same widget as that of the slot and the generator.
- The key at the cursor is written only to a visible curve; otherwise — a constant.
- The limits of a handle: the default = the passport of the target, in the file — only narrowing.

**Non-Goals:**
- Do not change the model `Override`/`PubRange`/format v9.
- Do not make a "recording mode" of automation (write/touch/latch) — there is none for the slot either.
- Do not touch the expansion strip and baking (t690).

## Decisions

### D1. The automation widget in the instance window — `automation::draw`, as for the generator

Under the grid of handles — a section "Automation", exactly as in the FX editor: a menu
"+ curve" (the key `fx.auto_add` is reused; the menu label — by the author's handle name, verbatim)
for the number and colour handles without a curve, then the `automation::draw` itself. `AutoCtx`:
`specs` — the passports of the handles by `addr_spec` with the `label` replaced by the author's name and
`min/max` by the narrowing; `range` — the instance's material `(source_in, source_in + used)`; `view` —
the main timeline window, translated into the instance's material by the same `to_curve`/`to_timeline`
as for the slot (reversal taken into account); `linked` — the shared lock from `UiState`; `frame_step`
of the project. `AutoView` lives in `InstParamsWin` (as `view` of the generator does).
The value fields are translated to `CurveBind` (`sel`, `at`) — `draw_value_row` receives
`Option<&mut CurveBind>` and stops touching `bound_apply` directly.

*Alternative — its own mini-table of keys in the instance window.* Rejected: a third keyframe editor is
exactly the fork that R.34 closed ("each carrier has its own editor — and they drifted apart").

### D2. The key at the cursor — only for a visible curve

The rule is one, from the FX editor: a field writes a key when the parameter has a curve
(`CurveBind::has_curve`), otherwise — a value. Since creating a curve is now possible only through the
labelled menu under the graph, no "hidden recording" remains by construction; t691 asserts this as an
observable consequence (two different values at two cursor positions without a curve → no key, the value
is one).

### D3. Limits: `addr_spec` in kt-model, the author window shows the passport of the target

`Project::addr_spec(seq, &ParamAddr) -> Option<ParamSpec>` — a mirror of `addr_type` with the same
exhaustive `match` over the owner (`FxSlot` → `param_specs()` of the slot; `Generator` → the passport
of the generator; `ClipAttr` → the passport of the attribute (level, speed, etc. — create `clip_attr_spec`
next to `clip_attr_type`); `Transition`/`Camera` → their passports; `Nested { pub_id }` → the passport
of the target of the inner publication with its narrowing applied on top).
Author window: the field shows `range.min.unwrap_or(spec.min)`; on editing, a value equal to the limit of
the target (within the step tolerance) is written as `None`; the button "to the target's limits" does
`PubRange::default()`. Consumer window: `DragValue.range(spec.min..=spec.max)`, on top of which
`PubRange::clamp` (order does not matter — both operations are clamps, as in the model).

*Alternative — store the limits of the target in `PubRange` at publication.* Rejected: the copy would
diverge from the passport when the version of the effect changes — the same argument as against a stored
template value (R.59).

### D4. Hints — by the status line, through `widgets::hint`

For the curve menu, the widget buttons and the "to the target's limits" button — `hint(...)` (rule 10
of kt-app: no pop-ups); the keys in five catalogues. t393 checks that the promise of the hint is kept.

## Risks / Trade-offs

- [The instance window grows in height with the graph] → the section is collapsible (`CollapsingHeader`
  as for the slot), expanded by default when there is at least one curve.
- [t683 pins the glyph "+" and the key through the field] → the test is rewritten for the menu and the
  widget in the same task as the code; it is checked that the old probe `inst.curve.<id>` is not found.
- [`addr_spec` for `Nested` at depth > 1] → recursion through the same `addr_spec` of the inner reel; the
  depth limit is already guaranteed by the model (t424).
- [The axis lock is shared across the program — the instance window will start moving the main timeline]
  → this is exactly the required behaviour (R.40 item 15), as for the slot; checked in t691 through `set_view`.

## Open Questions

None: the automation key on a colour (per channel) is already supported by the widget; the menu label —
the author's name verbatim (t684).
