# Tasks

Round R.60. Each section — a commit after a green check (the auto-commit rule).
`openspec` — through Node 24: `export PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"`.

## 1. Chronicle and model

- [x] 1.1 `docs/user-feedback.md`: the section "Round R.60 (2026-09-22)" — the owner's claim verbatim (curves without a button, min/max at 0), the reconnaissance ("+" glyph, `bound_apply` at the cursor, `NaN → 0`), the accepted decisions D1–D4; check — the section is in place, the table "repeated" is untouched
- [x] 1.2 kt-model: `Project::addr_spec(seq, &ParamAddr) -> Option<ParamSpec>` next to `addr_type` (`src/libs/kt-model/src/project.rs`), `clip_attr_spec` next to `clip_attr_type` (`published.rs`), the `Nested` branch — recursively with the narrowing applied; check — t694 (`src/libs/kt-model/tests/pubparam.rs`): for each `ParamOwner` the passport exists, its type equals `addr_type`, for a vanished target `None`
- [x] 1.3 kt-model: normalization of the narrowing — `PubRange::narrowed(spec, lo, hi)` stores `None` where the value equals the limit of the target (tolerance — the step of the passport); check — t692 (`src/libs/kt-model/tests/pubparam.rs`): publication without a narrowing → `is_empty()`, narrowing `max` → only `max`, return → `None`; a file without narrowings is byte-for-byte as before (cross-checked with t681)

## 2. The author window: limits from the passport of the target

- [x] 2.1 `pubparam.rs`, `draw_published_win`: the fields `min`/`max` show `range.min.unwrap_or(spec.min)`/`spec.max` through `addr_spec`; the edit writes through `PubRange::narrowed`; the button "to the target's limits" (`icons::CLEAR` style, the label in i18n) → `PubRange::default()`; the hint `widgets::hint`; check — t692b (`src/apps/kt-app/src/layout_tests.rs`): after publication the probe `pub.row.min.<id>` shows `spec.min`, not 0; the narrowing and the reset reach the model
- [x] 2.2 i18n in the ×5 catalogues (`pubp.range_reset`, `pubp.range_hint`, `pubp.auto_add_hint`, `pubp.auto_hint`); with new glyphs — `python3 scripts/subset-fonts.py`; check — t100, t125, t128, t171 green

## 3. The consumer window: the automation widget instead of the glyph

- [x] 3.1 `InstParamsWin` gets `view: AutoView`; `draw_value_row` accepts `Option<&mut CurveBind>` and `spec` (the limits of the target + the narrowing) instead of a direct `bound_apply`; the glyph `+` and the probe `inst.curve.<id>` are removed; check — compilation, t683 rewritten (below)
- [x] 3.2 The section "Automation" under the grid: the menu `fx.auto_add` over the number/colour handles without a curve (the item — the author's name verbatim), the starting curve — a constant of the current value over the whole material of the instance (as for the slot), then `automation::draw` with `AutoCtx` from D1 (`range` = the material of the instance, `to_curve`/`to_timeline` through `clip_material_at` with the reversal, `linked` from `UiState`); `AutoResult.seek/set_linked/set_view` are passed to the player and the main timeline, as in `gen_ui.rs`; the result of the window is still a single `SetOverride` with a gesture; check — t691 (`src/apps/kt-app/src/layout_tests.rs`): without a curve two edits at different cursors → one value, zero keys; after the menu — the probe of the widget is found, two edits → two keys in the material of the instance, the table shows them; `set_view` with the lock moves the main timeline; the probe `inst.curve.*` is not found
- [x] 3.3 Rewrite t683: the part "a number with a curve places a key in the material of the instance" goes through the menu and `CurveBind`; the other assertions (each type with its own widget, `ClearOverride`) unchanged; check — t683, t684, t685 green
- [x] 3.4 Status-line hints for the curve menu and the widget buttons in the instance window (`widgets::hint`); check — t693 (`reach_tests.rs`): hovering over an action gives text in the status line (t392/t393 concern the level strip and accelerators, they have nothing to do with widget hints)
- [x] 3.5 Guard "one widget for all carriers": t693 (`src/apps/kt-app/src/reach_tests.rs`) — a scan of the kt-app sources: calls of `bound_apply`/`bound_value` only from `widgets.rs` (`CurveBind`) and `automation.rs`; the list of windows with editing over time (slot, generator, instance) — each has a probe of `automation::draw`; check — t693 green and red on a probe with a direct `bound_apply` (the probe is deleted)

## 4. Live scenario and finish

- [x] 4.1 `src/scripts/live/nestparam.py`: after the override of the width — open the curve menu, create a curve, move the cursor, change the value, check in the `.otio` two keys in `overrides[].curves` in the material of the instance and a screenshot with the graph; check — `./src/scripts/live-check.sh nestparam` green, screenshots in `target/live-out`
- [x] 4.2 The mandatory tests of the capability: `cargo test -p kt-model --test pubparam`, `cargo test -p kt-app` (reach t252–t255, layout t680–t685, t691–t693, font/i18n t100/t125/t128), `cargo test -p kt-engine --test nest_param` (t674–t688 — the curve model is not touched), `cargo clippy --all-targets`; check — everything green
- [x] 4.3 `openspec/specs/compositions/design.md` and `automation/design.md`: the section "Non-obvious details" — why the instance window has no editor of its own for keys and why the limits are not copied (D1/D3); `docs/features/timeline/structure/composition-public-parameters.md` — status "done" (freeze, `CAPABILITY:` is already there); check — `openspec validate --all --strict`, `features-index.py --check`
- [x] 4.4 `src/scripts/finish.sh` — rebuild of kt-app/kt-cli, the version `vYY.MM.DD-hash` in the report; check — the script is green, the version is named
