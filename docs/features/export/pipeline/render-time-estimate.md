---
id: F-EXP-007
title: The render time said before the render, and why it differs from the preview
status: in-progress
priority: B
cost: S
category: export/pipeline
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-engine, kt-gpu, kt-app]
invariants: [7, 11, 12]
ui_rules: [9, 13, 16]
round: kkt-26.11.15
point: change render-honest (found-defects 710, 711; architecture review 2026-11-17 R8)
tests: [2900, 2901, 2902, 2903, 2904, 2905]
depends_on: [F-EXP-005, F-EXP-006]
trackers: []
services: []
checked: 2026-11-17
---

# F-EXP-007 — The render time said before the render, and why it differs from the preview

## Essence

A person who watched a stretch play smoothly expects its render to take about as long. When the
preview composes at a fraction of the output size, the render does several times the work per
frame, and nothing on screen says so: the render then looks "slower than the preview" (the
owner's kkt-26.11.15 item 3). Before pressing Render he wants two answers: how long it will take,
and why that differs from what he just watched.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Preview quality (Draft/Preview/Good/Best, Auto/Full/Half/Quarter) | the preview's size and quality is a toolbar choice the person sees all the time; the render dialog states the output size but no time until the render runs (then elapsed/remaining) [from use; not checked live] |
| Adobe Premiere | 26.x | Playback Resolution (Full, 1/2, 1/4) / Export "Estimated file size" | the program monitor's resolution is a visible drop-down; the export dialog estimates the file size, the time only while exporting [NOT VERIFIED: `helpx.adobe.com` unreachable here] |
| DaVinci Resolve | 21 | Timeline Proxy Resolution, render cache | the viewer may play a proxy or a cached render; Deliver shows the time remaining only during the render [NOT VERIFIED live] |
| Final Cut Pro | 12.3 | Viewer "Better Performance / Better Quality" | a viewer setting; the share shows progress, no estimate before [NOT VERIFIED] |
| After Effects | 26.x | Resolution/Down Sample Factor of the Composition panel | the panel's factor (Full, Half, Third, Quarter) is shown under the picture; the render queue's "Est. Remain" appears after the first frames [from documentation as remembered] |

## The trick behind it

**1. The difference is the preview's SIZE, and every editor shows it in the viewer, not in the
render dialog.** Each of the five keeps a visible preview resolution, so a person who knows it can
work out the 4× or 9×; none says the factor at the moment the person decides to render. Putting
the ratio in the render dialog ("the preview composes 640×360, the render 1920×1080: 9× the pixels
of a frame") answers the complaint where it arises.

**2. A prediction before the first frame needs a measurement of THIS project.** Remaining-time
counters extrapolate the render's own first frames; before it starts there is nothing to
extrapolate from but the preview. The preview already measures each composite's device time per
frame and size; scaled by the output's pixels it predicts the composing part of the render. It
says nothing of decoding and encoding, so the prediction is only as honest as the journal line
that sets it against the actual time after each render.

## Pros

- The "render slower than the preview" class becomes a number on screen, not a support question.
- Every render's journal line (predicted against actual) shows where the model is wrong on a given
  machine without a trace.

## Cons

- A stretch never previewed has no prediction; the dialog says so rather than guessing.
- Device time scales with pixels only roughly (a blur's taps scale with the frame, a fixed pass
  does not).

## What users say

NO QUOTES: the vendors' forums and help pages are unreachable from this environment (as for
F-EXP-004/005, 2026-11-17). The primary signal is the owner's: "финальный рендер у нас очень
медленный во всех версиях, медленее чем превью" (`docs/user-feedback.md`, 2026-11-15 item 3).

## How it fits Kinotrain

CAPABILITY: openspec/specs/export/
The render dialog (`src/apps/kt-app/src/render_dialog.rs`) states the preview-to-render pixel ratio and a time predicted from the preview's measured device time over the range (`src/libs/kt-engine/src/player.rs`); every render journals predicted against actual (invariants 7, 11: the clock steers no frame).

## Estimate

| What | Answer |
|---|---|
| Scope | S — one change |
| Where the main work is | kt-engine (`player.rs` cost ledger), kt-gpu (device time per frame), kt-app (`render_dialog.rs`) |
| Project format | not touched |
| Tests that will have to be written | the ledger over a range; the dialog's rows; the journal line; a live scenario |
| Depends on | F-EXP-005, F-EXP-006 |
| Breaks compatibility | no |

## Verdict

Take it, B: the ratio always, the prediction where the preview measured the range, and the
journal line that keeps the prediction honest.

## Sources

- `docs/architecture-review-26.11.17.md` R8; `docs/found-defects.md` rows 710, 711 (read 2026-11-17).
- Our code: `src/apps/kt-app/src/render_dialog.rs`, `src/libs/kt-gpu/src/pass_time.rs`,
  `src/libs/kt-engine/src/player.rs` (read 2026-11-17).
