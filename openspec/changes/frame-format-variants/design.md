# Design: frame-format-variants

> DESIGN ONLY, revision 1 (2026-11-20), base `28c3919d`. Q1–Q2 in `proposal.md`; the defaults follow the
> recommended options.

## Context

Read on `28c3919d`:

- `ProjectSettings { fps, width, height, conform, … }` and `EventGeometry { rule, crop_*, pos_*, zoom,
  … }` with `ClipEvent.geometry_pos` / `geometry_auto` (`src/libs/kt-model/src/project.rs`, R.62 A): the
  leading Geometry node is an EVENT FIELD the engine materializes into the chain; its `rule: Conform`
  resolves against the project's `ConformRule`.
- `Sequence.raster`: a reel's own raster; an instance is conformed into the parent by the same rule.
- `LeadCtx.raster` carries the logical size for "as is" (video-plan design): the plan already separates
  the composite raster from the canvas rendered (the preview's quality).
- `Project::effective_clips()` is the ONE door of events to the engine (morphers, cuts, overrides of
  published parameters are applied there).
- The disk render cache's `durable_hash` hashes what `plan_hash` hashes, the raster included.
- The on-screen geometry frame (R.64) writes `SetGeometry` on a gesture.

## Goals / Non-Goals

**Goals:** one edit, several rasters; overrides only where a person moved something; one plan; batch
render.

**Non-Goals:** automatic subject-following reframe (needs `point-tracker`; a later stage writes overrides
from a track); a different fps per format (the edit's time is one); per-format effect chains (a format is
framing, not grading); per-format sound.

## D1. The model

```text
Project.formats: Vec<FrameFormat>                     // empty = not written
FrameFormat { id: FormatId(u32), name: String, width: u32, height: u32,
              rule: ConformRule /* for events whose rule is Project */, safe: Option<SafePreset> }
ClipEvent.formats: Vec<FormatOverride>                // empty = not written, sorted by format
FormatOverride { format: FormatId, geometry: Option<EventGeometry>,
                 geometry_auto: Option<Vec<ParamCurve>>, hidden: bool }
Track.format_masks: Vec<(FormatId, Vec<Mask>)>        // a canvas-space track mask per format
```

The master is not a `FrameFormat`: it is `ProjectSettings` (format id 0 in the UI). Names are unique
(the `unique_name` rule). Removing a format removes every override of it in one undo step.
`validate`: an override names an existing format; a link group's members share presence per format (an
A/V pair: hiding the video in a format does not mute its sound — sound is format-free; presence
concerns pictures only).

## D2. The door: `effective_clips_for(target)`

`RenderTarget { format: Option<FormatId> }` → the raster is the format's or the master's. For a format,
the door replaces each picture event's `geometry` / `geometry_auto` by its override where present, drops
events `hidden` in it, and resolves `Conform::Project` against the format's `rule`. Then everything
proceeds as today (morphers, slices, published overrides — in that order; a morpher patch's own geometry
slots are effects and are not overridden). Reels: an instance's override places the reel; the reel's
inside has no format (a composition is authored in its own raster).

Positions are canvas fractions and crops are source fractions in `EventGeometry` already, so a geometry
without an override means the same RELATIVE placement in any raster (Q1 a). A title (a generator) lays
out for the raster it is drawn at, so it reflows.

## D3. The engine

`Player::set_format(Option<FormatId>)` → a new build want (`builds.rs`) with the target; the renderer is
built from `effective_clips_for(target)` with `LeadCtx.raster` = the format raster; the preview quality
is a fraction of that raster. Keys: the raster is in `plan_key`; the overrides reach the materialized
Geometry node and so `fx_chain_hash`; `durable_hash` gains the target raster (it already hashes the plan
size). The audio renderer is unaffected by a format (no rebuild of sound on a switch: the audio build is
skipped when only the target changed, edit-without-rebuild's rule "the swap costs what the edit changed").

## D4. The interface

- Preview head: a format chooser (a gallery: "Master 16:9 · Vertical 9:16 · Square 1:1 · +"), the "+"
  opens the project settings section "Formats" (a section of the existing settings window, UI rule 22),
  with presets 9:16 (1080×1920), 1:1 (1080×1080), 4:5 (1080×1350) and custom.
- With a format chosen, the geometry frame and the Geometry node's fields edit the format's override
  (created on the first gesture, never by opening); a badge "9:16 override" on the event in the timeline
  row (a corner mark — an occupied place, rule 13) and an item "Reset framing to master" in the event's
  right-click menu; "Hide in this format" / "Show".
- Safe areas: a toggle beside the format chooser draws the format's preset as an overlay mesh.

## D5. Render

The render window gains a "Formats" list (master + each format, checkboxes, the master checked by
default). One press queues exports in order through the existing render job; each file
`<name>-<format>.<ext>`; settings shared (codec, quality), the raster per format. kt-cli: `--format
<name>|all`.

## D6. Format and OTIO

`.kt`: the next free version by content (`has_formats()`). OTIO: the exchange file is the master; formats
and overrides travel only in `metadata["kt"]` (a row in "What OTIO export loses"); a foreign editor sees
the 16:9 edit.

## D7. Class walk — "the frame size read from the project"

| Reader of the raster | Today | With formats |
|---|---|---|
| `frame_plan` / `LeadCtx.raster` | project settings | the target's raster |
| masks' reference frame (`kt_fx::mask::px_of`) | owner frame | event masks travel with the event; track masks per format (D1) |
| titles / generators' raster | project size | target raster (reflow) |
| camera world scale (`Cam3d.world_scale`) | quality / project width | quality / target width |
| disk render cache key | plan size | + target raster |
| reel strip / poster | project raster | master only (the bin shows the edit) |
| scopes (change `scopes`) | presented composite | measures the shown format |
| export settings size | project or custom | per format |
| on-screen geometry frame (R.64) | project canvas | the shown format's canvas, writes its override |
| safe-area / guides (F-TL-016 guides) | project canvas | per format |
| the subtitle layer (change `subtitles-and-text-editing`) | raster | target raster; the style's margins per format |

## D8. Idle

A format switch is one build want and one composite of the shown moment; nothing else runs. Overlays are
a cached mesh per (format, preset, size).

## Rejected

- **A copy per format** (Premiere/FCP): the edits diverge — the owner's request is the opposite.
- **A composition per format** (AE pattern): works today by hand, but every edit then happens inside one
  level of nesting; formats as overrides keep the edit at the root.
- **Per-format fps** — one edit has one time.
