# Design

## Context

See proposal.md — Why. Today's chain: `to_rgba` (`src/libs/kt-ffmpeg/src/source_impl.rs`)
scales to `Quality` without regard to the aspect ratio; `execute_plan_cpu` (`src/libs/kt-engine/src/video.rs`)
and the mirror in `src/libs/kt-gpu/src/engine.rs` place the layer only when `(w, h) == Quality`;
`pan_frame` (`src/libs/kt-fx/src/video.rs`) computes the frame in the coordinates of the already stretched
source; `place_media_at` (`src/apps/kt-app/src/app.rs`) reads only the duration
and the presence of tracks from `info`; `ProjectSettings` — `fps/width/height/sample_rate/pitch_engine/default_shape`;
`Sequence` has no settings of its own (the compositions spec, the R.50 decision).

Constraints: invariant 4 (order == read, quality in the cache key), 7 (one plan, CPU is the
byte reference, GPU is within tolerances, the pair `geom.rs` ↔ `geom_fused.wgsl`), 8 (thumbnails from the sidecar),
11 (golden on matching proportions do not move), UI rules 4/9/13/16/19/20.

## Goals / Non-Goals

**Goals:**
- The conform rule as the LAYER'S GEOMETRY in the plan, one function for decode, placement, thumbnails,
  previews and export.
- Transparent margins; "as is" with overflow beyond the frame; rotation and anchor in the node.
- The level of the rule: project + an exception on the event; reels — their own raster and fps.
- Nothing moves byte for byte where the proportions match.

**Non-Goals:**
- Masks in the node (the place is left, a separate change); conforming the source frame rate
  (blend/duplication); color management; an on-screen transform control (F-UI-001).
- Clip-level blend mode (F-CMP-003 — on the track, change B).

## Decisions

- **The decoder contract does not change, the client does.** `conform_geometry(native_display,
  canvas, rule, crop, zoom) -> LayerGeom { decode: (w, h), rect: [x, y, w, h] in fractions of canvas }`
  — one pure function in kt-fx (without FFmpeg, the crate invariant). The decode size is the placement rectangle × the scale step S, aligned to even, with the ceiling `MAX_INTER_DIM`;
  for "as is" — the native (with the ceiling). The decoder still delivers EXACTLY what was requested, so
  `Quality` remains the cache key and the order equals the read. The alternative "the decoder always delivers
  native inside the frame, the plan fills in" was rejected: "fill" would have to upscale from the inscribed frame — blur by construction.
- **Native — in screen pixels.** `MediaInfo` gets `sar` (num/den); the frame's view =
  `width × sar`; all conforming is computed from the view, and so are thumbnails and "Max (by sources)". Otherwise DV 4:3 (720×576, SAR 16:15) is yet another silent stretch.
- **Placement is part of the geometry series.** The leading node "Geometry" — the affine source → frame
  (rule × crop × position × zoom × rotation around the anchor) in the common program
  `kt-fx::geom` with a new edge primitive `Transparent` (instead of the `Clamp` plateau) and a mirror
  in `geom_fused.wgsl`. Effects BEFORE the leading node work on a buffer of decode size
  (source space), AFTER — on a canvas of output size. Exactly so in Vegas ("effects
  applied before or after any event panning and cropping"), and this gives "as is" without
  a special case: if the canvas is smaller than the source, the source sticks out past the edge.
- **The rectangle and position — in FRACTIONS OF THE CANVAS, the crop — in fractions of the source.** So the 1/4 preview looks like the frame, and export in another resolution conforms by the same number; the UI shows the project raster's pixels, the model stores fractions. Rotation — degrees, the anchor — fractions of the source (default 0.5/0.5).
- **The leading node — a chain slot with the `lead` flag.** Validation: a video event has exactly one
  leading node; only it carries the rule (`Conform::Project | Fit | Fill | Stretch | AsIs`);
  removing the leading node — a refusal with a reason (rule 16), reordering is ordinary; additional
  "Geometry" nodes without a rule are ordinary slots. Audio events have no such node. Dropping an event
  creates a leading node with the rule `Project` and identity parameters → bytes as before
  on matching proportions (an identity hash, like the previous Pan without zoom).
- **Place for a mask.** The node is designed as `Geometry { rule, crop, pos, zoom, rot,
  anchor }`; a mask will become a neighboring field of the node (`mask: Option<…>`) — it is not written until it exists
  (rule 9), but the edge primitive `Transparent` and the order "mask after crop, before
  placement" are fixed here so that the masks change does not rewrite the node.
- **The rule's level.** `ProjectSettings.conform` (default Fit) acts retroactively on
  events with `Conform::Project`; an event with an explicit rule does not change (rule 20). This is
  exactly the Resolve + FCP pair, whose absence produced kde-invent #538 and Olive #1106.
- **Transparent margins, black — only the frame background.** The layer's margins are alpha 0; the composite with no layers
  under them keeps the previous background. Lesson of Pitivi #1378: a black margin on the top layer erases everything beneath it.
- **The decode step — from the conform size.** The formula S is the same as before, "Quality" in it is the
  decode size from the geometry; "as is" with a native larger than the canvas — step 1 (decode =
  native, ceiling 4096).
- **Filmstrip thumbnails** are ordered as "a view of the source in a 320×180 frame" (the sidecar table is additive: a new size — a new key), drawn with the margins in the row's background color.
- **The project settings window** — one body with the preview panel's combo box (`Command::SetSettings`,
  rule 17). The raster — two validated fields (even, 16…8192); fps — a list of divisors of
  90 000 in the range 1…120 (24/25/30/48/50/60/100/120 etc.) plus a field with the validation
  `TICK_HZ % fps == 0` (29.97 is not expressible — a refusal with a reason, rule 16). It opens from
  the project menu and from the address in the status line. The flag `settings_explicit` is set
  when the window is applied.
- **The first clip sets the project** only when there are no events in the root AND the settings are not explicit;
  the source's view (with SAR, even) and fps are taken, if it divides 90 000; otherwise fps is not touched and
  the status line says why. Inside a reel — it does not apply (the reel has its own settings,
  the default is the project).
- **The mismatch message** — via `KtApp::hint()` (one hint in one place); it is not an error style
  (the action was performed), but it names the address: "Event geometry /
  Project settings".
- **Revision: reels get their own raster and fps (the owner's decision in R.62)** — the previous
  "inherit everything" (R.50, compositions/design.md "Rejected") is lifted by direct request.
  `Sequence.raster: Option<(u32,u32)>`, `Sequence.fps: Option<u32>`; `None` — the project.
  The native of an instance = the reel's raster (1:1 view), conformed by the instance's leading node like a file.
  Its own fps — quantization of ALL the instance's internal time to the reel's grid
  (`floor(t·fps_l/90000)·90000/fps_l`) before expanding the sub-plan; at `None` — continuous, as in
  R.56. Audio: the 48 kHz grid is shared, the reel's fps does not affect sound. Law E: the requirement is
  reworded as "with settings equal to the project's"; tests t434/t435/t447 remain as they are (their reels have no settings), a negative test is added. The reel's scale step is computed from
  its raster; the sub-composite memo key includes the reel's raster and fps (otherwise changing the reel's settings
  would return the old frame).
- **Generators: native = the container's raster.** A title in a square 1080×1080 reel must be computed in its raster; 4096×4096 as a "detail margin" is replaced by the honest native of the container
  × the step margin (formula S gives the same S for a generator with a native ≥ Quality).
- **Format v11 by content:** a leading node with the rule `Project` and identity parameters is NOT written (a file without new features stays byte-stable at the previous version); the project rule is written only when it is not the default; `Pan` in an old file is read into the leading node
  (`mode` → rule, `pan/zoom` → position/zoom), one-way. The flag `settings_explicit`
  is written only as `true`.
- **kt-cli probe** prints for a video event the rule, the rectangle and the decode size — the first
  thing asked for the complaint "the picture is stretched/cropped".

### Clarifications during implementation (stages 1–2)

- **The leading node is an event field (`ClipEvent.geometry` + `geometry_pos` + `geometry_auto`),
  not an `fx` slot.** The insertion into the chain lives in two places: the engine materializes the node into
  `VideoClip.fx` at the position `geometry_pos` (in the count of active slots; `LayerMix.from` past the
  insertion point shifts by one), and the FX editor — only inside the strip call, translating
  indices at the boundary (`ChainSel::Lead` for the selected node). Reason: slot indices are the
  addresses of curves, publications (`ParamOwner::FxSlot`) and the context menu; a node in `fx` would shift
  all of them, and old files would get curves on the wrong slots.
- **The identity of the leading node is in context.** The default parameters make the node "not an effect"
  exactly when the conform frame equals the canvas (`geometry_is_identity_in`);
  otherwise it must place a frame of foreign proportions into the frame. The scheduler treats an identity
  node as transparent (like audio FX), so that single CamMotion/Mirror nodes next to it do not lose
  the old kernel (t297).
- **A procedural source (generator, title, placeholder) and a reel — view `None`.** The frame is a
  function of the requested quality, there are no proportions: the frame = canvas, the node is identity. The generator's
  native 4096×4096 does not enter the rule in this case (t663/t670).
- **The context of the leading node is part of both memo keys** (`postfx_key_hash` on CPU,
  `plan_key` on GPU): the same chain on the same frame with a different rule — a different picture;
  without this, t714b read a stale value (found by a CPU↔GPU discrepancy).
- **Pan is abolished without a mode migration.** `kt:pan` in an old file is read into an
  additional "Geometry" node on the SAME slot (indices do not move), the curves
  `pan_x/pan_y` move to `pos_x/pos_y` via `pan_to_pos(pan, zoom) =
  −pan·(1−1/z)/2`; the mode is not carried over — all three produced one frame (F-MED-004).
  `CamMotion` remains an effect with the old kernel and the same conversion on read.
- **Refusal outside the timeline — `KtApp::refuse`**: the status line shows it as an error
  (`HintKind::Error`, like the gesture's stop) with the status TTL, to the log and the retrospective; deleting
  the leading node with the cross is the first consumer.
- **A chain preset does not carry the leading node** (it is a property of the event); the chain clipboard
  as well.

### Clarifications during implementation (stages 3–4)

- **The settings window — one door for the project and for a reel.** "File / Project settings…" at the root
  edits the project; inside a reel (`model.editing()`) — its raster and fps with the checkboxes "as in
  the project". The project's conform rule is shown next to it as four words, not as a drop-down list: one click, everything visible.
- **The first clip sets the project inside `clip_cmds_for`**, not in `place_media_at`: the generator
  creation also goes there, and the adoption is gated by `info.fps.is_some()` — a procedural source has no
  proportions, so it does not set the project. `SetSettings` is put FIRST in the placement batch — one undo step.
- **A reel with its own raster = a file with a native raster.** The instance's view is the reel's raster,
  the base of the decode size is the rule's frame (`clip_base_quality`); the sub-plan is assembled at the base
  × step, `NestSrc::fit_to` takes the base from the plan size, not from the parent's canvas.
  A generator inside a reel is computed in its raster without participating itself (view None).
- **The reel's own fps — quantization of `clip_inner_tick`** (`floor(m·fps/90000)·90000/fps`)
  ONLY when `Sequence.fps = Some`; without it the path is as before (t579/t580 byte-for-byte).
- **The reel's raster and fps are part of the revision** (`hash_reel`): the sub-composite memo keys,
  the reel's preview in the sidecar and the GPU plan see the change of settings.
- **Generators (clarification of the spec):** the generator's native 4096×4096 remains as the step margin;
  it is not part of the conform rule — the procedural source gets view `None`, frame =
  the container's canvas. The delta of the `generators` spec is brought in line with this.

### Clarifications during implementation (stage 5 and stage 1 tails)

- **Thumbnails — by the source's VIEW, the sidecar is additive.** `thumb_quality(view)` fits the view into the
  320×180 frame (even); the `thumbnails` key did not change, but a `thumb_meta` table appeared
  (media → thumbnail size): sidecars from before R.62 have no row, their squashed thumbnails are not read
  and are rebuilt on the first request. A bump of `SCHEMA_VERSION` was rejected: it would wipe the index and
  peaks of all media for the sake of one table. The filmstrip cell centers a narrow thumbnail
  (`thumb_cell_fit`), a wide one is cut on the right as before (t719, unit + harness by shapes).
- **The project version is part of the preview key.** The live scenario `conform` showed: the rule
  was written to the file, but the preview on pause hung on the previous frame — the key saw the tick, the quality and
  the version of the source contents, but not the version of the PROJECT. Now `preview_project` checks
  against `Player::project_version()` (t722b, red on the old code). This also closes any other
  model edit while paused.
- **The geometry dump carries the status line** (`status`, as drawn; control characters
  replaced with a space — otherwise the JSON did not parse) and the probes `menu.file`, `bin.file`,
  `settings.conform.<rule>`, `settings.apply`: the conform scenario presses what the program itself gave, rather than guessing.
- **t728b** compares the FRACTIONS (fill width, center) of frames 1920×1080 and 1280×720 — similarity,
  not bytes: resampling at different resolutions is not obliged to match byte for byte.
- **SAR fixtures** (`testsrc_sar11/21.mp4`): the same pixels, different `setsar`; t711 checks
  `info().sar`/`view_size()` and the byte equality of the decode — SAR touches only the view.

- **The CamMotion migration — exact in the keys, within tolerance between keys (t230 revised).** The
  "Geometry" node stores the position `pos = pan_to_pos(pan, zoom)`; the legacy interpolated pan and zoom
  separately and multiplied them, and a smooth position curve does not reproduce the product. Baking a key on
  every frame for the sake of bytes was rejected (the file bloats, the curve is not editable); the owner
  said "backward compatibility is not needed". Measurement at 64×36: the mean deviation < 3,
  a single edge pixel up to 90 — the tolerance in the test `mean < 4`.
- **The early exit "chain is identical" in the step sampler takes the live plan into account** (t311):
  a "Geometry" node with zoom 1 and a curve 1 → 4 is statically identical; before R.62 the early exit was saved by Pan, which was not considered identity.

## Risks / Trade-offs

- [A geo-series with a transparent edge changes the bytes at the frame boundary of existing projects] →
  an identity affine with a frame equal to the canvas must yield the previous path byte for byte (K = 1,
  the edge is not reached); t715 holds GOLDEN_VIDEO and t11/t67/t184 without regeneration.
- [The decode step from the geometry gives a different cache key for clips with an explicit rule] →
  the order and the read are computed by one function; t714 checks order == read on all rules.
- [Effects before the leading node work on a buffer of a foreign size — kernels that are invariant to
  resolution but not to proportions (EventGrid, Vignette)] → this is the expected semantics
  (source space); in the UI the leading node is by default the first, and test t717 fixes
  both orders.
- [A reel's own fps breaks the intuition of "continuous time" of R.56] → quantization is enabled
  only by an explicit field; at `None` the path is as before; t724 — both.
- [The first clip changes the project settings "by itself"] → only an empty root AND implicit settings,
  a message with the address; t721 checks that explicit settings are not touched (rule 20).
- [The settings window is a new place in the UI] → rule 13: a dialog, not a strip; it opens by
  address; reach-test t720.
- [SAR fixtures require ffmpeg at generation time] → the fixture is in `src/fixtures/gen.sh`,
  the test under feature `ffmpeg`; the synthetic source carries `sar` without FFmpeg.

## Migration Plan

Compatibility is not required (the owner's decision). Reading old files: `Pan` → the leading node one-way;
events without a node get a leading node with the rule `Project` on read;
reels without settings — `None`. New files — v11 by content. Rollback — git.

## Open Questions

- Is a separate item "Match Output Aspect" (Vegas) needed as a command over the crop of the leading
  node — it is decided in the UI during implementation, the spec does not change.
