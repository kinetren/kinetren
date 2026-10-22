# Design

## Context

See proposal.md — Why. Below is only what exists in the code and determines the shape of the solution.

- **The curve today** (`src/libs/kt-model/src/project.rs`): `ParamCurve { param, keys,
  source: CurveSource }`, the shape sits on the key (`AutoKey::shape: SegShape`, a closed
  function `SegShape::at(s, a, b)`); the sources `Keys | Link | Formula` are resolved into keys
  BEFORE the audio stream by the pass `links.rs::resolve_*` (the frame grid, `t` from `source_in`).
  `apply_curves` skips a curve without keys — a formula without resolution silently does not sound.
- **The formula preview** (`links.rs::formula_preview`) computes `t` from `from` — the left
  edge of the graph WINDOW, while the render computes it from `me.source_in`. Two truths about one `t`.
- **The graph** (`src/apps/kt-app/src/automation.rs::graph`) draws `Shape::line` over
  `c.keys` — straight lines between keys; t698 asserts `curve_value`, not the drawing.
- **"The shape of all segments"** (`widgets::shape_all_control`) is called from `fx_editor.rs` and
  `gen_ui.rs` with `curves[view.active]`; the active curve is selected only by clicking its name in
  the widget's toolbar; for the head of an instance (`pubparam.rs::draw_inst_head`) this block is not called.
- **The field with a curve** (`widgets::param_row_bound*` → `bound_apply`) writes a key on
  `r.changed()`; egui's `DragValue::range`/`Slider` clamp the incoming value and mark the response
  as changed — a zigzag value beyond the passport produces a key on every frame of play.
- **The link**: `CurveSource::Link { path, expr, stops }` and `LinkPath::parse` exist, but there
  is no UI door to them in any kt-app file (only the display of the name `name <- path` and "unlink").
- **Overrides** (`published.rs::apply_overrides`, `validate_overrides`): curves with a
  source are rejected (`CurveSourceUnsupported`), the keys are translated from the timeline time
  into the target's material by `retime_curve`.
- **The geometry frame** (`geomctl.rs`): `geom_ctl` starts as `GeomMode::Off`, lives in `KtApp`,
  is not written to `user_store`, and is switched by an item of the preview's context menu
  (`app.rs::cycle_geom_mode`). `frame_schema` draws `schema_quad` in `painter_at(rect)`.
- **The head of a strip** (`fx_editor.rs` ~545, `widgets::HeadSlot`): the label
  `📌 Composition parameters`, `color: theme::ACCENT`, is not truncated.
- **The reel preview** (`reel_preview.rs`): the frames go to the UI at once, while they are written
  to the sidecar together with the peaks after the audio stage in one transaction; `Err(cancelled)` falls into `failed`.
- **The reader** (`kt-otio`): `check_kt_version` refuses only "too new"; a v5 file
  falls inside `MorphTargets` (an untagged `Repr` without a `null` variant); the migrations of v<3, v4,
  v8, `ease` live in the reader. The import of a legacy `.ktproj` is a separate door `import_legacy`
  (the model of a door).
- **Stills**: `kt-ffmpeg` opens the file through libavformat (image2/png_pipe gives one
  packet, `duration` 0 or N/A); there are no picture fixtures, and `MediaInfo.duration` for them is
  zero; "Loading…" is a scrim when `!complete` lasts longer than 180 ms (`app.rs`).
- **Paths**: `project_to_timeline` writes `media_target_url(dir, path)` — a relative
  model path passes through as is; adding media stores the path as it was given.
- **Tag `tested-48`** (19.09, 3a9160e) — the build on which the owner saw the effects of
  region `rendereg3`.

## Goals / Non-Goals

**Goals:**

- One curve model, in which keys, shapes (including a formula on a segment), a formula layer and
  a link coexist and are removed separately; one rule for `t`; one UI block for all carriers; the graph draws with the evaluator.
- A key — only by a person's gesture.
- A separate door for importing old versions, with fixes that retire by version.
- Stills under a fixture and a test; reel previews survive to disk.
- The frame in the preview — by default and with memory; the head of a strip is short.
- The "user path" mechanism in the specs and screenshots in the report.

**Non-Goals:**

- A recording mode for automation during play (the owner: "only fix it").
- A stack of modifiers with time scopes (the owner chose "segment + layer").
- Bezier handles and speed handles (F-AUT-001 "candidate", `rate` — only direct).
- Relink and media collection (media-index, deferred).
- Editing the reader for v5 (the decision: import only).

## Decisions

- **A formula on a segment is the variant `SegShape::Formula { expr }`, not a new source.**
  This is what the owner decided about the zigzag (R.61: "the shape is on the segment between two keys"), and a formula on a stretch is the same case of "on this piece": it has a start and an end, set by keys. The variables `s`, `a`, `b`, `t` (seconds from the left key). The ends are not forced (as with a step): the formula is free to give a discontinuity, that is its business. `SegShape::at` for a formula calls the evaluator; the parsing is cached by the text in the caller (the widget and the resolver), the shape itself stays `Copy`-like in semantics, but holds a `String` — therefore `SegShape` loses `Copy`; the places that copied the shape clone it. The alternative "a formula-segment as a Link with a window" was rejected: the address of the stretch would live outside the key and would break when a key is inserted.
- **The layer is the field `ParamCurve.layer: Option<String>`, resolved AFTER the keys.** The order is fixed: `k = curve_value(keys)` → `layer(k, t, src)`. Not a stack and not a list: the owner chose "segment + layer", and a stack with scopes was rejected in Non-Goals. `CurveSource::Formula`
  remains a legal file form and on reading is semantically equal to "a layer over empty keys" (it is not rewritten in the file — the bytes of old files do not move). Removing the layer = `None`;
  removing the source does not touch the layer. The resolver flattens the layer and the segment formulas into keys on the frame grid in the same pass as the links: the render still sees only `Keys` with
  `Linear`; a project without formulas pays nothing (the call counter is 0 — t705/t710 remain).
- **`t` is counted from the start of the material everywhere**: `formula_preview` receives `source_in`
  instead of `from` and takes the window only as the sampling range. This is a defect, not a choice.
- **The graph draws by sampling values**, not by keys: `curve_value` at every pixel
  (the step is max(1 px, frame)), with the key diamonds on top. For a curve with a layer — two lines: `k`
  (solid) and the result (dashed). The test asserts the pixels of the line against `curve_value`, not the model
  (the lesson of t698: "the graph is redrawn by the same function" was prose). Allocations —
  one reusable `Vec<Pos2>` in `AutoView` (invariant 12).
- **The handle of a shape parameter is at the middle of the segment.** The main parameter (tension/amplitude)
  — a vertical drag, periods — Shift+horizontal, the number next to it for the duration of the drag (as with
  speed — a value overlay, not a popup). A grab zone of 14 px (rule 1). A segment-formula
  has no handles (its parameter is text).
- **One block "curve as a parameter" (`widgets::curve_block`)** is called from the slot, the generator
  and the head of an instance with one signature; the header — the name and colour of the selected curve;
  the choice of curve — by name, line or field (`AutoView.active` is set also from the field via
  `CurveBind`). For an instance `AutoView` already lives in `gen_views`. A block without a curve is not
  drawn: "the shape of all segments" on an invisible curve was the source of "the second field does not
  change".
- **A key only from a gesture: `param_row_bound` compares "what was shown" and "what the widget returned" and writes only on `r.dragged() || r.has_focus() && changed || reset`**,
  plus a valve: the value is shown already CLAMPED by the passport before the widget, so that the widget has nothing to clamp. The test reproduces 120 frames of play with a zigzag beyond
  the passport — on the previous code this sets 120 keys.
- **The door of a link — a menu "Driven…" in the block**: the first level — "this event / other
  events of the reel (name, time)", the second — properties of the carriers by their passports (the slots' `param_specs`,
  geometry, the generator). It gives `CurveSource::Link { path }` with `expr: None`; the text of the recalculation — in the same formula field (as now for Link). It does not cross the reel boundary — the list of other events is built inside the current reel, and a refusal of the model is shown as a line.
- **Help — a group in the block + a status line + Tab.** Popups are forbidden (ui-reach,
  rejected), therefore the list of names is a collapsible group, and completion is in place by the
  single candidate or by the common prefix (as in a shell); the list of candidates — in the status line. The content of the group is built from the evaluator's registry (`Fun::by_name`
  gets a reverse list `Fun::ALL` with the arity) — not a copy.
- **Field widths — by the gallery**: `plain_row`/`param_value` set the `DragValue` a fixed width by the gallery of the longest value of the passport (`min`, `max` with the unit); the formula field — a fixed 220 px; the choice of shape — by the gallery of the longest name
  (rule 3 of the checklist). The test measures rectangles at two window widths.
- **Overrides accept sources; resolution — in the domain of the reel on overlay.**
  `apply_overrides` before `retime_curve` calls the resolver with the context "the reel as material"
  (`source_in = 0`, the reel's duration), gets `Keys`, and only those it translates into the target's time. So a formula typed on the instance's axis sounds by that same axis (`t` = seconds of
  the instance). Validation accepts `Formula`/`Link` (inside the reel) and any shapes.
- **Import of old versions — `kt_otio::import_older(text, base) -> (ProjectBundle,
  Vec<Fixup>)`**: JSON pre-processing (`serde_json::Value`) by the table
  `FIXUPS: &[(u32 /*version for which it was made*/, fn(&mut Value) -> usize)]`, then the
  ordinary `timeline_to_project`. The first fix is `v5_null_morph_targets`. `load_project`
  on a file-read error with `v < KT_META_VERSION` wraps it in
  `Error::NeedsImport { version }` — kt-app shows the version and the name of the import item. The reader
  is not touched. The rule for agents (the root CLAUDE.md and kt-otio): fixes — only in
  `legacy_import.rs`, each with its version; the table "versions with fixes" — one place; removing
  support of a version deletes its fixes.
- **Stills — a property of the source, not a special path of the player.** `kt-ffmpeg` recognises
  "one picture" (the demuxer image2/png_pipe/jpeg_pipe, `nb_frames ≤ 1`, no audio) and
  returns `MediaInfo { duration: a PLACEHOLDER-like "infinity", fps: None, still:
  true }`; `read_video` at any tick returns the single decoded frame (a cache in the
  source, decoded once). The decode scale step and the plan do not change: for them it is a video with
  one frame. The stub of a missing file is marked as a FULL frame — and this is the reason for
  the "endless Loading…" of a vanished picture: `complete` was computed from the presence of a frame
  from the decoder, and the stub did not count as a frame. The alternative "a still as a generator" was rejected:
  a generator has no native and no decode scale step, while a 4K PNG must go through a step.
- **The path is made absolute when added** (`KtApp::add_media_path` and `kt-cli`):
  `std::path::absolute` against the cwd; the model does not store relative paths to files. This is
  a rule, not a migration: old files with a "crooked" relative path are not rewritten
  (they are fixed by relink, deferred), but new ones will not break this way.
- **Reel previews: frames — their own transaction right after the frame stage** (`put_reel_frames`),
  peaks — `put_reel_peaks` afterwards; reading accepts a strip without peaks (the envelope is empty,
  the request for peaks is placed separately). Cancellation returns `Cancelled` as a separate variant,
  which is not put into `failed`, and the key is re-queued on the first `poll` without a
  pause. The test repeats the shape of the owner's project (two reels one above the other, the set on
  the upper one, a vanished still) and opens it twice.
- **The frame: `geom_ctl` — in `user_store` (`geom_controls: bool`, default true)**, the
  "Transform" mode when enabled; the toggle button — in the corner of the preview (an icon, not
  a checkbox — rule 9), the menu item remains. The schema of the form — scale by the coverage `union(frame,
  rect)`; the frame is drawn inside the same schema with a dashed stroke.
- **The head of a strip — `📌 <reel name>` via `elided_name` (SLOT_NAME_CHARS), the colour of the slot.**
  The full name — as the row's tooltip (rule 10).
- **rendereg3 — comparison of builds in Xvfb**: `git worktree add … tested-48`, a build of
  kt-app, both binaries open `untitled_v26.09.26.otio` (stubs instead of media — the FX
  on stubs is visible in the same way), stop at 1380/1390/1400 s, preview screenshots,
  a `compare` difference. If found — the cause in design, a regression test t798; if not found — the screenshots in the report and a question to the owner asking for media.
- **The guard "PATH"** — `src/scripts/specs-user-path.py --check`: parses
  `openspec/specs/{ui-timeline,ui-reach}/spec.md`, for scenarios with `TEST` in
  `src/apps/kt-app/` and a number ≥ 783 requires a line `- **PATH**` no shorter than 40 characters;
  it is called from `reach_tests` (t799) and `finish.sh`. The threshold by number is because the numbering
  is monotonic and rewriting two hundred earlier scenarios for the sake of the form is not needed. Live scenarios
  get the helper `step(name)` in `kt_live.py`, which takes a screenshot into
  `target/live-out/<scenario>/NN-<name>.png`; `finish.sh` prints the list after the version.

## What the comparison with `tested-48` showed (item 2 of the owner)

Task 7.1 is done, and the result is negative FOR A MEASURABLE REASON, not "not found":

- **The build `tested-48` (kt26.09.19) does NOT OPEN the file `untitled_v26.09.26.otio` at all**: the window shows 0 clips and 0 media, the title "untitled" — the envelope version of the file is higher than that build can read. There is nothing to compare the frames of the region with.
- **There is not a single source of this project on the machine**: the new build opens the file (70 events in the region's window, 46 notices about missing files), but the frame is empty, because all the material is stubs. The screenshots of both builds at 1380/1390/1400 s are in `target/live-out/rendereg3/`.
- **What was checked instead of the picture:** all 21,833 effects of the file are parsed by the current model (`FxInstance`), on loading not a single "effect dropped"; in the region the nested reel "Reel 3" (three levels) and the morpher layers that add fx to it (1 → 3) are in effect. That is, the data is intact — the question is only about the picture, which cannot be obtained without the media.

To close item 2 in substance, either the material (at least a couple of files from the region) or an export of this section by the old build as a reference is needed. This was asked of the owner in the report.

## Risks / Trade-offs

- [`SegShape` loses `Copy`] → the places with a copy of the shape — `keys[ki].shape`, `shape_all`, `insert_key` — switch to `clone()`; the compiler will find all of them.
- [Parsing of a segment formula in `curve_value` on every call] → a cache of the parsing by text in the resolver and in the widget; the render of segment formulas does not compute at all (flatten before the audio stream).
- [A layer over a source formula changes the semantics of old files] → no: `Formula` without keys = a layer over emptiness, the value is the same; t707 remains.
- [A still as an "infinite" source breaks `available_range`/trim] → the stub's duration is the same and trim already survives this (t208–t211); a test of trimming a still is in t796.
- [Import opens the file with the old path — an old version could be overwritten by mistake] → import does not write; saving with Ctrl+S writes the new form — this is the owner's goal ("gradually these edits will be removed"); a recovery copy of the old file is not created.
- [The button on the preview — a new layer over the frame] → one icon in the corner, drawn only on pause, without animation (invariant 12); t764 remains.
- [The "PATH" guard by number threshold skips scenarios added without a number] → a scenario without `TEST` is already rejected by `openspec validate`.
- [The comparison of tested-48 on stubs may not show the difference that the owner saw on media] → then a report with screenshots and an explicit request for media; the task is not considered closed as "no difference" without saying so.

## Migration Plan

Format: v13 by content (segment formula, layer) — files without them are written with the same bytes as before. Old files rejected by the reader go through the import; the fixes are removed when the version's support is dropped. The user storage gets `geom_controls` with the default "enabled" — the absence of the key is read as enabled. Sidecar: the table `reel_strip` gets a flag "peaks present"; rows without the flag are considered complete (the previous record was written only with peaks) — there is no `schema_version` bump (media-index: rejected).

## Open Questions

- Does a segment formula need access to `k` of the neighbouring stretches (for example, "continue the slope")? It does not change the specs: the variable is added by extending the set, not the file form; to be decided after the owner's first use.
