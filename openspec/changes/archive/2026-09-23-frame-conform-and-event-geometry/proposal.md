# Proposal

## Why

A source with foreign proportions (4:3 in 16:9, vertical video, a 200×200 logo) is silently
stretched to the edges of the frame: `sws_scale` in the decoder pulls the frame exactly to the project size,
the compositor does not fit a foreign size but rejects it, the Pan modes Fit/Fill/Stretch give
one and the same frame byte for byte, and not a single regression test feeds material with mismatched
proportions. Neither the project, nor the event, nor the reel has a conform-rule field or a
settings window. The owner named this the first complaint of round R.62 ("stretched in a stupid way"); the reconnaissance F-MED-004 measured the defect back on 08.09 and gave the verdict "take it, priority A, as the layer's geometry rather than as a checkbox".

## What Changes

- **The layer's geometry in the frame plan.** One function of (native with pixel aspect ratio,
  output raster, rule, step) gives both the decode size and the layer's placement rectangle;
  the composite puts the layer into the rectangle with TRANSPARENT margins, as a pair of CPU + WGSL by tolerance
  classes. The contract "the decoder delivers exactly the requested quality" does not change — what changes is
  who computes the requested value. Filmstrip thumbnails — in the source's proportions inside the frame.
- **The conform rule — four: fit / fill / stretch / as is.** It lives at the PROJECT level (default "fit", acting retroactively on everyone who follows the project) with an EXCEPTION at the event level.
- **The "Geometry" node — a built-in first node of the video event's chain** (rule, crop,
  position, zoom, rotation, anchor): the leading node cannot be deleted but can be dragged along the chain
  (effects before it — in source space, after it — in the project frame); the same node
  can be added from the catalog again. The place for a mask is reserved by the node's design, the mask itself — a separate change. **BREAKING:** the FX `Pan` is abolished (old files migrate on
  read to the leading node); Pan3d and CamMotion remain effects over the conformed frame.
- **The project settings window:** an arbitrary raster, fps from divisors of 90 000, frequency, the default conform
  rule, the pitch engine, the segment shape; the combo boxes in the preview panel remain quick access to the same command.
- **Placing a source:** the first clip into an empty project with implicit settings sets the project's
  raster and fps; after that a proportion mismatch does not ask, but is reported in the status line
  with the address "where to change it".
- **Reels with their own raster and fps** (default — the project): an instance is conformed by the same
  rule as a file; its own fps — quantization of the reel's continuous time to its grid; law E
  holds with settings equal to the project's. The **revision** of the R.50 decision "inherit everything" —
  by the owner's direct request.
- File envelope version v11 (one per round R.62, a predicate by content).
- Deferred, with a note: conforming the source frame rate and color management — notes in
  `docs/features/`, "Deferred" in the capability's design.md.

## Capabilities

### New Capabilities

(none — frame conform falls into the existing capabilities)

### Modified Capabilities

- `video-plan`: the layer gets a placement rectangle and a conform rule; one function computes the decode size and the rectangle; transparent margins; the decode step is computed from the conform size; the "Geometry" node is part of the geo-series with a transparent edge.
- `video-fx`: `Pan` removed from the registry; the "Geometry" node is leading, mandatory, movable, re-addable; effects before the leading node work in the source space; Pan3d/CamMotion — over the conformed frame.
- `media-index`: the source facts carry the pixel aspect ratio and the frame view; thumbnails — in the source's proportions; the contract "a frame of exactly the requested quality" is preserved, the requested value is computed by the geometry function.
- `compositions`: its own width/height/fps for a reel (default — the project); the instance is conformed by the rule; quantization of reel time to its grid; law E — with equal settings; the reel's scale step — from its raster.
- `generators`: the generator's native — the raster of the container (the project or the reel), not 4096×4096; the frame is still of exactly the requested quality.
- `project-format`: v11 — the project rule, the leading geometry node, the reel settings; migration `Pan` → node on read; the flag of explicit project settings.
- `ui-timeline`: the project settings window; dropping the first clip; the mismatch message; the chain editor with an undeletable leading node; filmstrip thumbnails in the source's proportions.
- `ui-reach`: reachability of the settings window and the node's actions from the point where the person acts; a live scenario for a vertical source.
- `export`: export at a resolution different from the project conforms by the same rules (geometry in fractions of the frame); golden do not move on matching proportions.

## Impact

- `src/libs/kt-fx` (`video.rs` `pan_frame`/`pan_zoom` → node geometry, `geom.rs` +
  `kt-gpu` `geom_fused.wgsl` — the transparent edge as a pair), `src/libs/kt-engine`
  (`video_plan.rs` the layer's rectangle, `video.rs` `execute_plan_cpu` and `fits`,
  `decode_scale`, `export.rs`), `src/libs/kt-gpu/src/engine.rs` (layer placement),
  `src/libs/kt-media` (`Quality` remains, `MediaInfo.sar`, thumbnails), `src/libs/kt-ffmpeg`
  (`to_rgba` — only reads SAR; the scale is still to the requested size),
  `src/libs/kt-model` (`ProjectSettings.conform`, `settings_explicit`, `FxInstance::Geometry`,
  removal of `Pan`, `Sequence.raster/fps`), `src/libs/kt-otio` (v11, Pan migration),
  `src/apps/kt-app` (settings window, drop, status line, fx_editor, thumbs, i18n —
  five catalogs), `src/apps/kt-cli` (probe prints the rule and the rectangle).
- Tests: golden `GOLDEN_VIDEO`, t11/t67/t184 must remain green without regeneration;
  new t711–t728. Notes: F-MED-004 (after archiving — "done"), new F-SPD/F-CLR for the deferred items.
