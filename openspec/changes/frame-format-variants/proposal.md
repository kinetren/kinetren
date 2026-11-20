# Proposal: frame-format-variants

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3485–t3504.

## Why

The owner took candidate ★15 on 2026-11-01 (`docs/ktprompts/kkt-26.11.01-candidates.md`, registry
`kkt-26.11.01-index.md` P1): "Сделал 16:9, нужен ещё 9:16 и квадрат. Хочу варианты кадра одного
таймлайна: у каждого своя поправка геометрии события поверх общей, safe-area сетки, в рендере «все
форматы сразу»."

Today a project has one raster (`ProjectSettings.width/height`); a vertical version is a second project
or a manual composition per format. Premiere's Auto Reframe and FCP's Smart Conform make a COPY, after
which the edits diverge (note F-PRJ-006, trick 1) — exactly what "варианты кадра одного таймлайна"
excludes.

## What Changes

- **Formats of the project**: besides the master raster, a project may declare formats (name, width,
  height, the default rule for events that follow the project: fit or fill). The cut, the sound, the
  effects and the tracks are shared; a format changes only framing.
- **Per-event overrides**: an event may carry, per format, its own Geometry node values and curves
  (`EventGeometry` + `geometry_auto`) and a presence flag (shown / hidden in that format). An event with
  no override is planned in the format's raster with its master geometry — positions are canvas
  fractions, so a PiP stays in its corner and a full-frame shot is re-conformed to the new raster by the
  format's rule (Q1).
- **The one plan with a raster input**: `frame_plan` takes a render target (raster + format); the
  format's overrides and presence are applied in the one door that hands events to the engine
  (`effective_clips`), so preview and export share it (invariant 7).
- **A format switch in the preview head** ("16:9 · 9:16 · 1:1"): the preview composes the chosen
  format; the on-screen geometry frame (R.64) writes the CURRENT format's override, and only on a gesture
  (rule 20); "Reset to master" removes an override.
- **Safe areas**: platform overlays per format (Q2), drawn by the preview only, never rendered.
- **Render all formats**: the render window lists the formats with checkboxes; one press queues one
  export per checked format, the file name suffixed by the format's name.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-plan`: the render target (raster + format) as an input of the one plan; the format's overrides
  in the event door; keys by format.
- `export`: an export renders a chosen format; "all formats" queues one export per format.
- `project-format`: formats and overrides raise the file by content; OTIO export flattens to the master.
- `ui-reach`: the format switch, the overrides by gesture, reset, safe-area overlays, the render list.

## Impact

- kt-model: `Project.formats: Vec<FrameFormat>`, `ClipEvent.formats: Vec<FormatOverride>`; commands
  `AddFormat`, `SetFormat`, `RemoveFormat` (removes its overrides in one step), `SetFormatOverride`,
  `ClearFormatOverride`; `effective_clips_for(format)`; validation; the format version by content.
- kt-engine: `RenderTarget` into the builds (`VideoRenderer` raster from the target), the player's
  current format, plan keys (the raster is already in them; the format's geometry values reach
  `fx_chain_hash` through the materialized node), the disk render cache key adds the format raster.
- kt-app: preview head switch, gizmos per format, overlays, render window list, i18n ×5.
- kt-otio: formats and overrides in `metadata["kt"]`; the exchange file shows the master only (a row in
  "What OTIO export loses").
- Idle: a format switch composes the shown moment once; nothing runs at rest.
- Feature note: F-PRJ-006 (new).

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20` A17, his Q26–Q27): Q1 (a), each event is fitted into the format by its own
geometry; Q2 (a), platform safe-area presets.

The questions as asked:

**Q1. How does an event WITHOUT an override sit in a 9:16 format?** Your words: "у каждого своя поправка
геометрии события поверх общей". Today the conform rule fits/fills each event's SOURCE into the raster.
- (a) **Each event is re-conformed into the format's raster with its master geometry** (*recommended*: a
  full-frame shot fills the vertical by its own centre, a PiP keeps its corner, a title reflows to the
  narrow width; you fix only what looks wrong).
- (b) The format is a crop of the finished master frame (its centre); an override moves an event inside
  that crop. Simpler to reason about, but a title or lower third is cut by the crop.

**Q2. Which safe-area overlays?** Your words: "safe-area сетки". Platforms' UI covers the right column and
the bottom of a vertical, differently per platform.
- (a) **Presets for YouTube Shorts, Instagram Reels, TikTok and a plain 90 %/80 % title-safe, one chosen
  per format, drawn in the preview only** (*recommended*).
- (b) Only the generic title/action-safe rectangles.
