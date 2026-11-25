# Proposal: picture-check

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` T15 (no). Note F-EXP-009. Tests reserved:
> t3908–t3910.

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

(Q72, Q73 of `docs/owner-checks.md`; re-read against `fc60a5d9`, 2026-11-25: `loudness-norms` has built the sound's half of this: findings at their times under the result's table, a click moves the playhead. The picture's findings
join that list.)

**Your items whole** (`docs/ktprompts/v26.10.txt` lines 56–57): "14. в окне рендера нет акцента у loudless и второй строки to если я например прорендерю отрывок с сильным неприятным gain на одном канале или например на тишине у другого
или всё вместе..., на цифры вообще внимание не обратишь такие. а еще не понятно что является нормой у каждого параметра.. ну и оформить это получше надо, а не просто текстом насрать на отъебись" and "15. а для видео такой же
оценки нет? ну типо что-то про картинку я не знаю... то что помогает от частых ошибок или что-то такое".

**Now**: after a render the window shows the sound against its norm and the sound's findings with times. Of the picture: nothing. A gap renders black, a one-frame leftover stays, and nobody says.

**Q72 (Q1). Where do the picture's findings go besides that list?** Example: a 20-minute edit with 3 gaps, 11 leftovers of one frame and 2 frozen places.
- (a) **The list with clicks; a button "Mark all" puts 16 range markers on the timeline when you want them** (*recommended*): nothing appears by itself.
- (b) Markers after every check by themselves: 16 markers come, and must be removed or re-made at the next check; your own markers stand among them.
- (c) The list only, no markers at all.

**Q73 (Q2). The check of flashing** (more than three large flashes within a second: broadcasters refuse such files, some people get seizures). It is the only check with a rule of law behind it; on music videos and fast cuts with flash transitions it
will often find places you made on purpose.
- (a) **A warning with its places, like the others** (*recommended*): on a typical clip with flash transitions it may list several intended places; it blocks nothing.
- (b) Off by default, a tick in the render settings turns it on: nobody is told who did not ask.
- (c) Not made.
