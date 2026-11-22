# Proposal: fx-form-width

> DESIGN ONLY (spec session, 2026-11-20). A defect class, not a feature (no `docs/features/` note):
> registry `kkt-26.11.07-index.md` E7, "no: defect, `ui-reach`, change `fx-form-width`", on his word.
> Tests reserved: t3591–t3594 (the spec session's block).

## Why

The owner, `kkt-26.11.07` line 13: "агенты НЕ ДУМАЛИ ВООБЩЕ когда делали фильтр isp - там такая ПОРТЯНКА НА
ТРИ КИЛЛОМЕТРА ПАРАМЕТРОВ, и это при том что половина ширины этой портянки - ПУСТАЯ!!!!!!!! … Я СТОЛЬКО РАЗ
ПРОСИЛ НАВСЕГДА!!!!! ЗАПОМНИТЬ ЭТИ ПРОБЛЕМЫ!!!!" The repeated-requirements table of `docs/user-feedback.md`
holds it in bold: "A form leaves half of its width empty and runs long — a guard over every effect form".

The mechanism (a class, not one window): every numeric parameter is drawn by `widgets::params_grid_bound`
(`src/apps/kt-app/src/widgets.rs`), an `egui::Grid` of ONE logical column of rows (label, slider, value,
curve button). The window is resizable and fills its area (windows-fill-their-area), so when it is wide the
grid stays one row wide and the rest is empty, and when a kind has 60 rows (the camera ISP) the sheet runs
for screens. t779 checks buttons and scrolling, t780 groups and pairs — no guard looks at how the WIDTH is used.

## What Changes

- **A form flows into columns by its width**: `params_grid_bound` lays its rows into as many columns as fit
  (a column is never narrower than its readable minimum: the longest label of the five catalogues + a
  slider of 120 pt + the value), filling column by column so that reading order stays top-to-bottom; groups
  (`widgets::group`) are cards that flow the same way, a group never split across columns.
- **One rule for every place that draws rows**: the effect windows, the morpher, transition, camera, track,
  generator and title forms, the ISP pipeline tiles' expanded rows, project settings and render — all go
  through the same flow.
- **A guard over every form** (the owner's "навсегда"): for EVERY registered effect, transition and generator
  kind and every `WINDOWS` registry window, the form is laid out at three widths (narrow, the default, wide)
  and must (1) leave no more than a quarter of its width unused at the wide size, (2) grow no taller when made
  wider, (3) never draw a row below its minimum width, (4) keep the action buttons visible (t779's contract).
  A form that cannot pass declares why (a reason ≥ 25 characters), like the fixed-size windows of t1127.
- **The camera ISP's form** gets its parameters grouped by stage (they are: one group per pipeline block),
  so its wide window shows several blocks side by side.

## Capabilities

### Modified Capabilities

- `ui-reach`: the width rule of forms and its guard over every kind and window.

## Impact

- kt-app: `widgets::params_grid_bound` and `group` flow layout; the call sites that wrap grids in their own
  vertical layouts; `layout_tests.rs` / `reach_tests.rs` guard; i18n untouched.
- No model, engine or format change. Idle: layout only while drawing a window; nothing at rest.

## Questions to the owner

None: it is his rule, repeated. The limits of the guard (a quarter of the width; three widths) are chosen
here and stated in the guard so they can be read and tightened.
