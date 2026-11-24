# Design: picture-check

> DESIGN ONLY, revision 1 (2026-11-20), base `ccae9d10`. Q1–Q2 in `proposal.md`; defaults recommended.

> Re-read 2026-11-25 against `fc60a5d9`: `kt_fx::audio::findings::{Finder, SoundSpan}`, `ExportStats.spans` and `render_dialog::sound_findings` (a row moves the playhead) exist. D3 uses that list: one `Finding` row type for both, the
> picture's spans beside the sound's. D2's statistics live in kt-engine's writer thread where `Finder` is fed.

## Context

- Loudness: measured in the export pass, reported by the render window (export spec, "Loudness is measured in
  the export pass, without master gain").
- The export's readback ring (export-readback): every composite arrives as RGBA bytes on the compose thread, in
  frame order through the writer (`export_pipeline.rs`); nothing the writer emits may depend on timing.
- Placeholders for missing media, `Missing` plugin slots (plugins spec), the conform rule (R.62), frame hold
  and still images (R.67), signed speed with 0 (R.57).

## D1. The edit lint (kt-model, instant)

`lint::check(project) -> Vec<Finding { kind, span, clip, note }>` over `effective_clips` of the root (reels as
instances):
- **Flash**: a picture piece on any picture track whose duration < `flash_frames` (default 2) frames and whose
  neighbours on the same track abut it (a cut leftover). A deliberately short generator (a flash effect) is
  reported too — a note, not an error.
- **Gap**: spans of the project range where no picture event is active on any track (it renders black).
- **Missing**: events on a placeholder source; slots of a `Missing` plugin.
- **Bars**: a picture event whose rule is fit and whose source aspect differs from the raster by more than 1 %
  (bars on the sides or top), with the bar width.
Pure, deterministic, O(events) through the existing per-element indexes (builder-cost-guard).

## D2. Export statistics (kt-engine)

Per collected frame (integer Rec.709 luma over every 2nd row/column):
- `black` = share of samples with Y < 20 ≥ 98 %;
- `freeze` = the frame's 64-bit hash equals the previous frame's AND the plan at that moment has no intended
  hold (no still source, no frame-hold event, speed not 0, no generator that is static by kind);
- `range` = share of samples with Y < 16 or Y > 235 (limited range) — over 1 % flags the frame;
- `flash` = the mean luma changes by more than 10 % of the range against the previous frame over more than a
  quarter of the frame area (a 4×4 block grid) in opposite directions within 0.5 s → one flash; > 3 flashes in any
  1 s window flags the span (the Harding/BT.1702 shape, simplified and stated as a warning).
A reducer turns flagged frames into spans (black and freeze ≥ 0.5 s; range ≥ 1 frame; flash as computed). The
numbers depend only on the frames (invariant 11: the same report every run), computed after the frame is
written, never steering anything.

## D3. The report

The render window's result shows "Picture" beside "Loudness": counts per kind; a list (kind, time, event name,
note); a click on a row moves the playhead and selects the event; "Mark all" (Q1 a) adds range markers named
"check: black", "check: flash"… through `unique_name`, one undo step. Edit → "Check the edit" runs D1 and shows the
same list in the Diagnostics window's section "Edit check" (a section of an existing window, UI rule 22).

## D4. Class walk — "a mistake a person ships because nothing said"

| Mistake | Found by | Here |
|---|---|---|
| loudness out of target | export (exists) | unchanged |
| flash frames, gaps | nothing | D1 |
| missing media / plugin | a notice at open (exists) | also in the list, at their time |
| bars from a wrong conform | nothing | D1 |
| black / frozen footage | nothing | D2 |
| illegal levels | scopes (change `scopes`) by sight | D2 measured |
| flashing | nothing | D2 |
| audio clipping | true peak at export (exists) | unchanged |

## D5. Idle

The lint runs when asked (or once when the render window opens); export statistics run inside an export only;
nothing at rest.

## Rejected

- **A pre-export pixel analysis of the whole edit by the live preview** — it would compose every frame on the
  display device; a low-size offline job may come later.
- **Auto-fixing** (closing gaps, deleting flash frames) — rule 20; ripple does it on the person's command.
