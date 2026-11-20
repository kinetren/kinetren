# Proposal: loudness-norms

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` T14 (no: defect → export), round R.74. Note
> F-AUD-005 (refreshed). Tests reserved: t3919–t3923.

## Why

The owner, `v26.10` line 56: "в окне рендера нет акцента у loudless и второй строки to если я например прорендерю
отрывок с сильным неприятным gain на одном канале или например на тишине у другого или всё вместе..., на цифры
вообще внимание не обратишь такие. а еще не понятно что является нормой у каждого параметра.. ну и оформить это
получше надо, а не просто текстом". Today the render window prints one line "LUFS / dBTP / LRA" and a grey "to
−23 LUFS: +N dB" (`render_dialog.rs`); the meter measures the mix as a whole, so one loud channel or one silent
channel does not show at all.

## What Changes

- **A target norm**, picked in the render window and kept in the render settings (and its presets): EBU R128
  (−23 LUFS ±0.5, true peak ≤ −1 dBTP), ATSC A/85 (−24 ±2, ≤ −2), Streaming (−14 ±1, ≤ −1), Podcast (−16 ±1, ≤ −1),
  each with LRA guidance; the streaming figures are labelled as measured by third parties.
- **The Sound section of the result is a table, not a line**: every measured value on its own row with its unit,
  the norm next to it and a state colour (within / within tolerance / outside — Resolve's blue/yellow/red
  logic in our theme); a hover says what the value means and what the norm is (the status bar, rule 10). The
  section's header carries the count of rows outside the norm, so a bad render is visible without reading
  numbers.
- **Per channel**: the left and right peak (dBFS), the left/right loudness balance (LU), and the share of time
  each channel is silent while the other sounds — the owner's "strong gain on one channel" and "silence on the
  other".
- **Findings with times**, in the same list as the picture check (change `picture-check`): spans above the
  true-peak ceiling, clipped samples, a channel silent ≥ 0.5 s while the other sounds, a balance beyond 6 LU for
  ≥ 1 s; a click jumps to the moment, "Mark all" makes range markers.
- **The offset line tells the whole truth**: "to the target: +N dB — the peaks would reach X dBTP (over the
  ceiling)" when raising would cross the ceiling (F-AUD-005, cons).

## Capabilities

### Modified Capabilities

- `export`: per-channel statistics and audio findings measured in the export pass; the target norm in the render
  settings.
- `ui-reach`: the Sound table, states, hovers, the target picker, findings with jumps.

## Impact

- kt-fx (`audio/loudness.rs`): per-channel K-weighted block energies and per-channel true peak in the same
  accumulator (the integrated number unchanged byte for byte), a per-burst record for the span reducer.
- kt-engine: `ExportReport.sound` (per channel, findings); the target norm in `ExportOptions` and its text form
  (`export_settings`, so kt-cli and presets carry it).
- kt-app: the table in `render_dialog.rs`, the norm picker, the shared findings list with `picture-check`; i18n ×5.
- Format: none (render settings live in `render-presets.json`; findings are not stored).
- Idle: everything is computed inside the export pass; the table is drawn from a finished report.

## Questions to the owner

**Q1. Which norm by default?** Your words: "не понятно что является нормой у каждого параметра". Today the window
compares only with −23 LUFS (broadcast, EBU R128). Video for YouTube and other platforms is turned down to about
−14 LUFS by the platform itself.
- (a) **Streaming (−14 LUFS, −1 dBTP) by default; the choice remembered and stored in render presets**
  (*recommended*: matches where a video goes; EBU is one click away).
- (b) EBU R128 (−23) by default, as today.

**Q2. A "normalize" button?** Today there is none: the window states the offset only (a decision of R.48 — the
target is a person's choice, and correcting a rendered file means encoding it again).
- (a) **No button; the line states the offset and whether the peaks would cross the ceiling** (*recommended*).
- (b) A button "Raise every track by N dB and render again" — one undo step, then a new render.
