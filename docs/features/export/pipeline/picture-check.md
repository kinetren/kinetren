---
id: F-EXP-009
title: A picture check like the loudness one — black, flash and frozen frames, gaps, levels
status: candidate
priority: B
cost: M
category: export/pipeline
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-engine, kt-app]
invariants: [7, 8, 11, 12]
ui_rules: [16]
round: null
point: null
tests: []
depends_on: [F-CLR-005]
checked: 2026-11-20
---

# F-EXP-009 — A picture check like the loudness one — black, flash and frozen frames, gaps, levels

## Essence

The render window already reports loudness. The owner asks for the same for the picture: something that
catches the frequent mistakes before the file goes out — a gap that renders black, a one-frame leftover of a
cut (a flash frame), a frozen frame where media went missing, a clip letterboxed by a wrong conform, levels
outside the legal range, a strobing passage.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | none built in | third-party QC; the render does not report the picture [NOT VERIFIED live] |
| Adobe Premiere | 26 | none built in for edit errors; Lumetri Scopes; "Gaps" via Sequence > Go to Gap | gaps can be navigated; no flash-frame report [NOT VERIFIED live] |
| DaVinci Resolve | 21 | the Cut page's flash-frame and gap indicators; Studio broadcast-safe checks | the Cut page shows flash frames and gaps by colour; Studio has broadcast-safe checks [NOT VERIFIED live] |
| Final Cut Pro | 12 | Range Check (luma/saturation) | zebra-like overlay of out-of-range levels [NOT VERIFIED live] |
| After Effects | 26 | none | — |

## The trick behind it

**1. Half of the common mistakes are visible in the EDIT, not the pixels.** A flash frame is an event shorter
than two frames left by a cut; a gap is time where the top layer has no event; a frozen frame is a placeholder
or a missing plugin. These need no decode — a pass over the model finds them instantly and names the event.
Only the other half (black frames inside footage, out-of-range levels, letterbox, strobing) needs pixels.

**2. The pixel half is free at export.** An export reads every composite back already; counting black/flat
frames, levels outside 16–235 and frame-to-frame luminance change costs a few integer passes per frame on bytes
that are there — the same place the loudness is measured.

**3. Strobing has a standard.** Broadcasters check photosensitive-epilepsy flashes (more than three large
luminance flashes in one second — the Harding/ITU-R BT.1702 rule); it is the one picture check with a legal
consequence, and it is computed from the same per-frame luma averages.

## Pros

- Catches mistakes that cost a re-delivery; the edit half is instant; the pixel half rides the export.

## Cons

- Intentional black or freeze frames are reported too: findings are notes to dismiss, not errors.

## What users say

The owner, `docs/ktprompts/v26.10.txt` line 57: "а для видео такой же оценки нет? ну типо что-то про картинку я
не знаю... то что помогает от частых ошибок или что-то такое".

NO QUOTES: no web access in this spec session (2026-11-20). The search would be the editors' forums and r/premiere, r/davinciresolve, r/VEGASPro for the feature by its name in the table; the editor rows are [NOT VERIFIED] for the same reason, and the next wave verifies them live.

## How it fits Kinotrain

### What already exists

- Loudness at export and in the render window (capability `export`, `src/libs/kt-engine/src/export.rs`).
- The export's readback ring (export-readback): every composite as bytes.
- Placeholders for missing media and `Missing` plugin slots (named notices).
- Markers with notes (marker-notes) to put findings on the timeline.

### What is missing

- The edit lint, the per-frame picture statistics, the report and its navigation.

### What hinders

- An analysis of the picture BEFORE export would compose every frame — it must be the export's by-product or a
  low-size offline job, never the live preview.

### Which invariants it touches

- **7, 11** — statistics from the composites of the one plan; the same numbers every run.
- **8, 12** — the lint is model-only; nothing at rest.
- UI rule 16 — a finding names what and where.
- CAPABILITY: openspec/specs/export/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | kt-model (lint), kt-engine (export statistics), kt-app (report, navigation) |
| Project format | none (findings are not stored; dismissals may be markers) |
| Tests | lint finds flash/gap/placeholder; export stats on synthetic black/strobe/out-of-range |
| Depends on | F-CLR-005 (shares the integer luma) |
| Breaks compatibility | no |

## Verdict

Take it in two halves: an instant edit check (flash frames, gaps, placeholders, missing plugins, letterbox by
conform) runnable any time, and picture statistics computed during export (black and frozen spans, out-of-range
levels, flash rate) reported beside the loudness; each finding jumps to its moment.

## Sources

- `docs/ktprompts/v26.10.txt` line 57 (T15); registry `v26.10-index.md` T15.
