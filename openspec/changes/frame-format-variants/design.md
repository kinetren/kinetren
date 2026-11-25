# Design: frame-format-variants

> DESIGN ONLY, revision 2 (2026-11-24), base `229c694b`. Q1 a, Q2 a decided (A17). Implementation-ready after `shared-events`.

## D0. What revision 1 said and the code (or his words) contradict

1. **An override REPLACES the event's geometry.** His ★15: "у каждого своя **поправка** геометрии события **поверх общей**". A
   replacement freezes: a later move of the master, or a Ken Burns of it, no longer reaches the vertical. The format keeps a
   CORRECTION over the common geometry (D2): the master goes on living under it.
2. **"A title reflows to the narrow width"** (the option he chose says so). kt-text breaks lines only at `\n`, and `size` is a
   fraction of the frame HEIGHT: a 0.07 title is 76 px in 1920×1080 and 134 px in 1080×1920, 1.8 times larger in a frame 1.8
   times narrower; a lower third of 24 letters is cut on both sides. D3 makes the promise true.
3. **`RenderTarget` through the builds, `effective_clips_for`.** The engine's door is `resolve_reel` for every reel
   (`incremental::resolve`), used by the video build, the audio build, `gen_sources`, and the sizes are read from
   `project.settings` in seven places of `video.rs` and in export, kt-cli, streamcost. A target threaded through them all is
   a dozen chances to miss one. A format is a DERIVED PROJECT (D2), as the Viewer's cut project is.
4. **"The reel's inside has no format".** A reel with no raster of its own is composed at the PROJECT's raster
   (`video.rs:1341`), so in a format it is composed at the format's. Its events may carry corrections as the root's do.
5. **A preview head.** There is none. The place is a corner button of the preview beside the zoom's
   (`preview-canvas-and-gizmos`); the toolbar is untouched.
6. **The plan key has the raster.** `put_plan` hashes the output size, not the logical raster (`project_size`): 1080×1920 at ½ and
   540×960 at full compose at one size with other pixel-valued rows (blur in project px). Within one project only formats
   make that possible: the key gains the logical raster WHEN it is a format's (t3487), so the master's keys do not move.
7. Format ids 0 for the master: an `Option<FormatId>` says it without a magic number.

## Context

`ProjectSettings` (`project.rs`), `EventGeometry` (`:5651`, ten numbers and `rule`), `ClipEvent.geometry/geometry_auto`,
`Sequence.raster` (`:1115`), `links::resolve_derived` in `morph_pass` (`:7012`), `push_project_to_player` (`app.rs:4673`, the one
sender), `launch_render` → `start_render_to` (`render_dialog.rs:812`, one `RenderJob`), the geometry frame `geomctl.rs` (its
commit at `:536`), `settings_window.rs`, `gen_sources` (`video.rs:3031`), kt-text `layout`.

## Goals / Non-Goals

**Goals:** one edit, several rasters; a correction only where a person moved something, over the living master; one plan;
batch render. **Non-Goals:** subject-following reframe (`point-tracker` may write corrections); another fps or sound per
format; per-format effect chains (a format is framing).

## D1. The model

```text
Project.formats: Vec<FrameFormat>                    // empty = not written
FrameFormat { id: FormatId(u32), name, width, height, rule: ConformRule, safe: SafePreset }
ClipEvent.framing: Vec<Framing>                      // empty = not written, sorted by format
Framing { format, dx, dy: f64 /* canvas fractions */, zoom: f64 /* × */, rot: f64 /* deg */,
          rule: Option<Conform>, crop: Option<[f64; 4]>, auto: Vec<ParamCurve> /* a row of its own */, hidden: bool }
Track.format_masks: Vec<(FormatId, Vec<Mask>)>       // canvas-space track masks of a format
```
Names are unique (`unique_name`), 16–16384 px, even. Commands `AddFormat`, `SetFormat`, `RemoveFormat` (drops its framings
and masks in the same step), `SetFraming { id, format, framing: Option<Framing> }`, `SetTrackFormatMasks`. All are content.
`Split`, `clip_slice`, trims, copy/paste carry `framing` as they carry `geometry` (its curves move with the keys);
`punch_out`, morpher pieces and `FrameHold` too; a block pasted into a project without that format drops it and says so.
An audio event has none. `ParamOwner` gets none: a framing is not published or linked to in this change.

## D2. `Project::in_format(id, &dyn TitleWrap) -> Project`

A clone (with `shared-events`: pointer copies, a copy per corrected event) in which
- `settings.width/height/conform` are the format's;
- in the root and in every reel without a raster of its own, for each picture event:
  - `hidden` → the event is removed (its transitions and link group members stay; a transition whose picture event is gone is
    reconciled as a removed side);
  - rows: `pos_x += dx`, `pos_y += dy`, `zoom *= zoom`, `rot += rot`, in the base AND in every key of the master's curve of
    that row (a shift and a scale keep every segment shape exactly: the tension and the period are relative). A row that
    `auto` names is replaced by that curve. `rule`/`crop` replace when present. Results are clamped to the passport;
  - order: the corrections of base and keyed curves first, then `links::resolve_derived` (so an event linked to a corrected
    one follows it), then the corrections of rows that were derived;
- track masks are the format's where given;
- titles are refitted (D3);
- `formats` is kept (the id of the one applied is the derived project's `shown: Option<FormatId>`, `#[serde(skip)]`, refused
  by `validate` in the model's canon; it is only the key's extra of D0.6).

No engine code knows formats but that key. Callers, all of them: `push_project_to_player` (root, entered reel or the
Viewer's project, after which the Viewer's cut is applied), the render job, kt-cli `--format`, the poster and the picker's
scene (the shown format: they take the project the player holds), the scopes. The strips of the bin keep the master.
The audio build of the derived project meets unchanged audio events, so nothing re-renders (t3487).

## D3. Titles in another shape

Two rules, both only in a derived project (the master's bytes and the file never move):
- **size**: `size_f = size × (h_m / min(w_m, h_m)) × (min(w_f, h_f) / h_f)`: the letter keeps its share of the SHORT side (76 px in
  1080×1920 for the 0.07 above); the same factor scales keys of a `size` curve;
- **wrap**: lines longer than 0.9 of the width break at spaces, greedily, by kt-text's own advances (`kt_text::wrap_lines`, a
  pure function beside `layout`; kt-media hands it as `TitleWrap`, kt-model has no font). The breaks become `\n` in the
  derived title's `text`, so its identity, cells and export follow. A word longer than the line is not broken. A title
  with a shown value (timecode, counter) and a title whose `offset` animates (a crawl) is not wrapped.
`FrameFormat` has no per-title text; a title that needs other words is another event hidden in the other format.

Computed on the recommended case: "Anna Petrova, director" at 0.07 → 22 letters ≈ 920 px at 76 px: one line in 1080×1920 at
the same place; "Interview recorded in Vladivostok" (33) → two lines.

## D4. The interface

- **The chooser**: a small button at the preview's lower left, only while the project has formats ("16:9 ▾"): the master, each
  format, "Safe areas" (a check), "Formats…". Probe `preview.format`. No key. It is a state of the session; opening a project
  shows the master.
- **Formats are made** in Project settings › Formats (a section): rows name, size, rule, safe preset; "+ 9:16", "+ 1:1", "+ 4:5",
  "+ Custom"; a cross with the count of corrections it would drop.
- **With a format shown**: the preview takes its raster and preview quality fraction; the geometry frame (R.64) and
  the pasteboard draw the corrected placement, and a drag writes `SetFraming` (the first gesture creates it, opening
  never does): the difference from the master at the playhead. The Geometry node's form shows the master's rows, and
  under them "Framing 9:16": X, Y, zoom, rotation, rule, crop, Hidden, Reset; ◆ on a row gives it a curve of its own.
  The event carries a corner mark when it has a framing of the shown format (the badge place of the level chip);
  its menu: "Reset framing (9:16)", "Hide in 9:16" / "Show in 9:16".
- The mask editor, title drag and effect centres in a format edit the MASTER's values (they are content of the effects); the
  status line says so once per session ("masks and effects are shared by all formats").
- **Safe areas**: `SafePreset::{None, TitleSafe, Shorts, Reels, TikTok}` as rectangles of fractions in a table with a date and a
  source per platform; one mesh over `img ∩ rect`, preview only.

## D5. Render

The render window gets a card "Formats" (only with formats): the master and each format with a check, the shown one checked.
Start queues `RenderQueue { left: VecDeque<(Option<FormatId>, PathBuf)> }`: the path dialog asks ONE name, the others take
`<stem>-<name>.<ext>`; files that exist are asked about together. One `RenderJob` at a time, the next started by the
result's arrival; Cancel stops the queue; the result panel lists every file. "Resolution" applies only when one target is
checked. kt-cli `export --format <name>|master|all`.

## D6. Format and OTIO

`.kt`: the next free version by content (`has_formats`: any format, framing or format mask). OTIO: the master edit; formats
travel in `metadata["kt"]` (the planned row stands).

## D7. Class walk: "the frame size read from the project"

| Reader | Now | With the derived project |
|---|---|---|
| `VideoRenderer::build_reel`, `LeadCtx`, `link_stages`, `clip_head_zoom`, `Cam3d.world_scale`, mask ops | `project.settings` | the format's, by construction |
| `Player::raster`, preview quality, `preview_display` | want's settings | the same; the quality fraction is kept on a switch as `set_settings` keeps it |
| export (`export.rs:704`), kt-cli (nine places), `streamcost`, `gpushare` | settings | `in_format` first, then today's code |
| `render_dialog` size line, `render_cost`, `preview_cost` | project size | per checked target; the prediction of a target never shown says "not measured" |
| generators in fractions of their own axis (shapes, plates) | frame | redrawn at the format's raster |
| shapes whose size is in fractions of the HEIGHT (R.53) | frame height | larger in a vertical, as titles were: the same short-side factor (`size_like` rows named by the kind's passport: `ParamUnit::Percent` of height — the list is in tasks 1.3) |
| camera events | world = project width | the format's width; a camera move keeps its share of the width |
| disk prerender | plan key | D0.6 |
| reel strips, poster of a reel, bin | master | master |
| `maskctl.rs:445` project height | project | the shown project's (`player.raster()`) |
| media import ("first clip sets the project") | project | the master only |
| safe zones of other kinds (guides) | none today | — |

## D8. Idle

A switch is one project version: one build and one composite. Overlays are one mesh per (preset, rectangle). Nothing at rest.

## Rejected

- **A copy per format**, **a composition per format** — the edit diverges or sinks a level.
- **An extra Geometry node after the lead** for the correction — the lead's output is cut by its canvas, and panning a filled
  16:9 shot inside 9:16 would show transparent edges; the correction must be in the lead itself.
- **A crop of the finished master** (Q1 b) — he chose (a).
