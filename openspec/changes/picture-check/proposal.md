# Proposal: picture-check

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` T15 (no). Note F-EXP-009. Test numbers:
> pending a new block from the orchestrator.

## Why

The owner, `v26.10` line 57: "а для видео такой же оценки нет? ну типо что-то про картинку я не знаю... то что
помогает от частых ошибок или что-то такое" — after the loudness report of the render (capability `export`,
"Loudness is measured in the export pass").

## What Changes

- **Check the edit** (instant, model only, no decode): flash frames (a picture piece shorter than the
  threshold, default two frames, left by a cut), gaps (time with no picture event at all — it renders black),
  missing media (placeholders), effect slots of a plugin this build lacks, events letterboxed or pillarboxed by
  the fit rule. Run from the Edit menu any time, and shown when the render window opens.
- **Picture statistics in the export pass**, beside the loudness: black spans (nearly every pixel below the
  black level for at least half a second), frozen spans (identical consecutive frames where the timeline has no
  intended hold — a still, a frame hold or speed 0 are excluded), levels outside the legal range (the share of
  pixels below 16 or above 235 over 1 %), and flashing (more than three large luminance flashes within one
  second — the photosensitive-epilepsy rule broadcasters check). Computed from the frames the export already
  reads back.
- **A report with jumps**: each finding names its kind, the time span and the event; a click moves the
  playhead there; findings can be put on the timeline as range markers in one step (Q1). Nothing is fixed by
  itself.

## Capabilities

### Modified Capabilities

- `export`: picture statistics measured in the export pass and reported with the loudness.
- `ui-reach`: Check the edit, the report and its jumps.

## Impact

- kt-model: `lint::check(project, fps) -> Vec<Finding>` (flash, gap, placeholder, missing plugin, bars).
- kt-engine: per-frame statistics on the export's collected frames (integer, sampled every 2nd pixel row and
  column), a span reducer, `ExportReport.picture`.
- kt-app: the report section in the render window, the Edit menu item, jumps, "Mark all" → range markers.
- Format: none (findings are not stored; marking makes ordinary markers).
- Idle: the lint runs on request (a pass over the events); statistics only inside an export.

## Questions to the owner

**Q1. Findings as markers?**
- (a) **The report is a list with jumps; a "Mark all" button turns the findings into range markers named by
  kind** (*recommended*: nothing appears on the timeline by itself; rule 20).
- (b) Findings become markers automatically after every check.

**Q2. The flashing check (photosensitive epilepsy).** It is the one picture rule with a legal weight for
broadcast; for social video it is a warning.
- (a) **Include it as a warning with its spans** (*recommended*).
- (b) Leave it out.
