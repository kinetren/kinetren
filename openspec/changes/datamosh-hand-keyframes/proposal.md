# Proposal: datamosh-hand-keyframes

> DESIGN ONLY (spec session, 2026-11-25). Registry `kkt-26.11.10b` K9, the owner's own "на будущее": no change covered
> it. Start only when he asks. Tests reserved: t4292–t4294.

## Why

The owner, `kkt-26.11.10b` line 14, whole: "Запрос на будущее - надо в datamosh уметь раставлять кейфреймы руками, а не
только как сейчас с определенной скважностью, в других эффектах тоже надо такое правило ввести будет."
Today an I-frame comes every `gop` frames, at the stream's start, at the cut and on a `reset` edge; there is no row that
places one.

## What Changes

- A `key` row in Datamosh (a gate that takes a curve, as `reset` and `mosh`): its rising edge puts an I-frame at that
  position. With `gop` 0 the keyframes are the hand's alone, plus the start and the cut.
- A "Key here" button beside the row adds a Hold key of the row at the playhead in one step, a mark in the graph shows
  each; the sound half follows the same positions.
- The rule for the class: an effect that fires by a period gets the same gate row; the list is in the design.

## Capabilities

### Modified Capabilities

- `video-fx`: the datamosh's schedule and the gate rule.

## Impact

- `src/plugins/fx-video-codec` (`mosh.rs` schedule, `sound.rs`, passport row 20), `kt-app` FX-UI (a button), catalogs ×5. Not
  beside `track-stream-stage` (STREAM). Format: one optional row, omitted at 0.
