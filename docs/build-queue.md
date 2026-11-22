# Build queue of the open changes

The order in which the open OpenSpec changes (`openspec/changes/*`, 40 on 2026-11-20, orch `941b25cd`) go
to executors, three at a time (`docs/orchestrator.md`: "три исполнителя"; the owner: every free slot used).
Written by the spec session; the orchestrator owns it and moves rows as work lands. Revised 2026-11-22 for
the owner's second round (kkt-26.11.20b B1–B4) and the three changes added since (B10–B12).

**How a wave is made.** Each change is given the AREAS it writes (below). Two changes in one wave share
no area, so two executors never edit the same structures. A wave may start only when the changes it
needs first have merged. A change whose owner question is open does not start. Its slot goes to the
next ready row of a later wave whose areas are free.

**One shared thing is allowed in parallel: the `.kt` version number.** A change that "raises the file by
content" takes the next free `PROJECT_FILE_VERSION_*` when it merges. If two land in one stage, the
second renumbers at its merge (a one-line conflict in `project_file.rs`). These changes are marked **v+**.

## Areas (what "touches" means)

| Area | Hot files |
|---|---|
| MODEL | `kt-model/src/project.rs` (the `Project`/`Sequence`/`ClipEvent` structs), `model.rs`, `ops.rs`, `project_file.rs`, `kt-otio` mapping |
| MASKS | `kt-model` `MaskShape`, `kt-fx/src/mask.rs`, the mask plan in `kt-engine` |
| STREAM | `kt-plugin-api/src/stream.rs`, `kt-engine/src/video_stream.rs`, `plugins/fx-video-codec` |
| GPU | `kt-gpu/src/engine.rs`, `pool.rs`, `compiled.rs`, `shaders/common.wgsl` and the core shaders |
| GEOM | `kt-fx/src/geom.rs` ↔ `kt-gpu/src/shaders/geom_fused.wgsl` (a pair, one commit), `plugins/fx-video-distort` |
| DECODE | `kt-ffmpeg/src/source_impl.rs`, `colour.rs`, `kt-media/src/ffmpeg.rs`, `factory.rs`, `kt-engine/src/worker.rs` |
| EXPORT | `kt-engine/src/export.rs`, `export_pipeline.rs`, `kt-ffmpeg/src/muxer_impl.rs`, `format.rs`, `encoders.rs`, `ExportReport` |
| CACHE | `kt-engine/src/render_cache.rs`, `kt-media/src/index.rs` (the sidecar), the disk tiers |
| AUDIO | `kt-engine/src/audio.rs`, `mix.rs`, `fxstore.rs` |
| PLAYER | `kt-engine/src/player.rs` (prefetch, present, at-rest jobs) |
| FX-UI | `kt-app` `node_editor.rs`, `fx_editor.rs`, `widgets` chain parts |
| PREVIEW-UI | `kt-app` `geomctl.rs`, `centerctl.rs`, the preview's overlays and gestures |
| TIMELINE-UI | `kt-app` `timeline.rs`, `controls.rs` command rows |
| SHELL | `kt-app` `app.rs` (`run`, the frame, the window layers), `window_shell.rs`, `window_frame.rs` |
| DIALOG | `kt-app` `render_dialog.rs` |
| COLOR-FX | `plugins/fx-video-color` |
| LIBRARY | `kt-app` `ffmpeg_setup.rs` → `library/`, `kt-ffmpeg/src/search.rs`, `kt-dirs` |

## Not executor work, or already in flight

| Change | State | What it needs |
|---|---|---|
| `fx-graph` | in flight (24/31 tasks; v1d merged) | its executor finishes section 3 (FX window with partners, morpher window, live scenarios, idle) |
| `disk-bytes-once` | done (4/4) | archive only (`/opsx:archive`) |
| `loop-play-latency-hold` | 13/16; the rest is a wine probe and the owner's repeat | an owner check, not a slot; archive after his answer |
| `export-host-copy` | stages 1–3 merged; Q1 answered "Сделать сейчас" (A13) | the dma-buf handoff, a wave-2 slot below |
| `fx-lens-and-camera` | blocked | `plugin-api-stage1` task 4.1 (a positioned tap, geometry primitives in the API, a barrier on the GPU); its presets (task 1.x) may go with any wave |
| `flatpak-flathub` | 11/22, owner's Open Questions 1–6 not asked | the questions first; needs `hw-encode-context`'s VAAPI encode |

## Waves (owner-answered work)

★ = the owner said "build now" (kkt-26.11.20).

### Wave 1

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| ★ `shared-events` | L | — | MODEL (the event vectors everywhere, mechanical), AUDIO and video builder keys (`incremental.rs`) | answered A19 (Q31 b); implementation-ready tasks | t3367–t3375 |
| ★ `stream-picture-stays` | L | — | STREAM (`render_held`), GPU (held layer input, planes→RGBA pass), SHELL (`run` opens the device), `kt-gpu` `context.rs`/`compute.rs`/`lib.rs` `export_engines` | answered A7 (Q51 b: build now, no measurement first) | t3812–t3822 |
| ★ `device-decode-sources` | L | — | DECODE, `kt-time/src/mem.rs` (tenant `Surfaces`), `fixtures/fake_va` | answered A12 (on by default + a checkbox in Settings) | t3300–t3311 |

`shared-events` touches every reader of `project.clips` across the workspace. That is mechanical, but
nothing that adds a field to `Project`/`Sequence`/`ClipEvent` runs beside it, so every MODEL change waits
for it.

### Wave 2

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| ★ `export-host-copy` (dma-buf) | M | — (stages 1–3 merged) | EXPORT (`muxer_impl.rs` VAAPI frames), GPU export pass | answered A13 | the change's remaining t3312–t3321 |
| `fx-graph-dag` | XL | `fx-graph` merged; `shared-events` (MODEL) | MODEL (`FxInput`, Merge kind), `kt-fx` `plan_chain`, GPU (held set, Merge pass), FX-UI | answered A5, A6 | t3200–t3229 |
| `project-cache-folder` | L | — | CACHE (`index.rs` → `cache_folder.rs`, `render_cache.rs` root, the persistent cache's `results/` store and its key layout), `kt-dirs`, the Settings page | answered B2 (one folder per project folder; cyclic writing with a notice); it hosts the persistent cache (B1) | t3824–t3843 |

`project-cache-folder` is now the gate of three owner priorities: the durable history (★), the persistent
cache of heavy effects (B1) and the cached render. Start it in this wave.

### Wave 3

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| ★ `durable-history-and-mix` | L | `project-cache-folder` stage 1 (the folder, the entry header); `stream-picture-stays` (STREAM) | STREAM (`StreamMemo` backing, `EntryCodec`), CACHE (the persistent cache's `results/…/history/`, the `mix/` tenant), AUDIO (`mix.rs` restore handoff), PLAYER (writes at rest) | answered A14 (build now, LZ4) | t3336–t3355 |
| `hdr-sources-to-sdr`, 8-bit stages 1–4 | M | `device-decode-sources` (DECODE) | DECODE (`colour.rs`, the HDR branch); the export default is built already (row 1054, t3603) | answered (Q50; A25 keeps the 8-bit path as the default) | t3800–t3811 |
| `track-stage-preroll` | S | — | AUDIO (`render_stages`, the seek path) | answered A15 (always the full tail) | t3361–t3366 |

### Wave 4

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `track-stream-stage` | L | `durable-history-and-mix` (the durable salt in `video_stream.rs`); `stream-picture-stays` | STREAM (`TrackStream`), `kt-engine/src/video.rs` track layer, EXPORT (`stream_preroll`), AUDIO (the sound half's track layout) | answered A23 | t3379–t3398 |
| `fx-preset-change` | M | `fx-graph-dag` (FX-UI selection in `node_editor.rs`) | FX-UI, `kt-model/src/ops.rs` (`insert_chain_block_at`) | answered A4 | t3595–t3596, t3932–t3934 |
| `persistent-cache` (was `disk-copies`; "Постоянный кэш") | L | `project-cache-folder` (the store's places, salt and cycle); `durable-history-and-mix` (shares CACHE); `device-decode-sources` (DECODE, for source copies) | CACHE (`results/…/layers/`, the source copies), the plan's kept layer input (`video_plan.rs`, `video.rs` layer path), PLAYER (the job at rest), DECODE (`CopySource`), EXPORT (`MuxConfig` times) | answered B1 (the name; heavy files AND the results of heavy effects); designed by spec-features (branch claude/spec-features-224d3d, 1e31056c) | t3440–t3459 |

### Wave 5

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `cached-final-render` | L | `export-host-copy`; `project-cache-folder` (`renders.tsv`) | EXPORT (a copy path in the writer, SEI), CACHE (durable key layout), DIALOG | answered A16 | t3322–t3335, t3870–t3876 |
| `download-library` | L | — | LIBRARY, `kt-text` (font packs), the release build (it packs the signed list) | answered B3 (the signed list for everything; plugins with the wasm host; a placeholder with Download) | t3850–t3869 |
| `frame-format-variants` **v+** | L | `shared-events` | MODEL (`Project.formats`, `ClipEvent.formats`), `kt-engine` `RenderTarget`, CACHE key (raster), PREVIEW-UI (format switch, gizmos per format) | answered A17 | t3485–t3504 |

### Wave 6

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `subtitles-and-text-editing` **v+** | XL | `download-library` stage 1 (the resolver, the window) | MODEL (`Transcript`, `LaneKind::Subtitle`), `kt-media` transcribe, `kt-text` per-word highlight, SHELL (the Text panel) | answered A11 | t3460–t3484 |
| `temporal-barrier-3dnr` **v+** | L | `track-stream-stage` (STREAM) | STREAM (`Temporal` host), GPU (state textures), `plugins/fx-video-camera` | answered A21 | t3545–t3557 |
| `preview-canvas-and-gizmos` | M | `frame-format-variants` (PREVIEW-UI) | PREVIEW-UI (the viewer transform), `kt-plugin-api` `ParamSpec.handle` | answered A26 | t3597–t3599 |

### Wave 7

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `geometry-fx-flex` **v+** | M | `preview-canvas-and-gizmos` (the centre crosshair) | GEOM, PREVIEW-UI (centre for Mirror and Tile) | answered A24 | t3570–t3584 |
| `chroma-key-upgrade` **v+** | M | — | COLOR-FX (the difference kernel), SHELL (the Viewer's view menu) | answered A22 | t3558–t3569 |
| `multicam` **v+** | L | `shared-events` | MODEL (`Sequence.multicam`, `ClipEvent.angle`), TIMELINE-UI, the preview area (the angle grid) | answered A18 | t3505–t3519 |

### Wave 8

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `pro-keyer` | L | `chroma-key-upgrade` (COLOR-FX) | COLOR-FX, GPU (a ~12-pass graph), `kt-plugin-api` `BarrierKernel::held`, `clip_frame_keys` | answered A22 | t3585–t3599 (t3597–t3599 are preview-canvas's: renumber at apply) |
| `magic-wand-mask` **v+** | M | — | MASKS (`MaskShape::Wand`, `kt-fx` coverage), PREVIEW-UI (gestures) | answered A20 | t3535–t3544 |
| `ripple-and-trim-modes` | L | `multicam` (TIMELINE-UI) | MODEL (`ops::ripple_plan`, two commands), TIMELINE-UI | answered A9 (Q8 a; owner check C9 when it ships) | t3415–t3439 |

### Wave 9

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `point-tracker` **v+** | L | `magic-wand-mask` (MASKS) | MASKS (the perspective mask transform), MODEL (`ClipEvent.trackers`), automation links, PREVIEW-UI | answered A20 (point and planar) | t3520–t3534 |
| `scopes` | M | `stream-picture-stays` (GPU) | GPU (a tap pass, new file), PLAYER (`measure_paused`), SHELL (a View window) | answered A8 (moved here from wave 4, whose slot the persistent cache took: the owner's "СУТЬ … В КЭШИРОВАНИИ") | t3400–t3414 |
| `hdr-sources-to-sdr`, 16-bit stage 5 **v+** | XL | waves 1–8's GPU, STREAM and COLOR-FX work merged (it rewrites the pool's format and the core shaders) | MODEL, DECODE, GPU, CACHE, EXPORT, FX manifests, SHELL (the display) — all core | answered B4 (1.0 = SDR white 203 cd/m²; 8-bit effects run marked; HDR on an HDR monitor at once; SDR + HDR10 + HLG export) | t3877–t3899 |

The 16-bit stage touches every core area. It runs with only non-core work beside it: the UI-only rows
of the next section.

## Added after 2026-11-20 (spec-features designs, owner answers B10–B12)

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `memory-breakdown` | M | — | `kt-time/src/mem.rs` (report-only tenants), every holder reports (kt-model estimate, kt-engine, kt-media, kt-text, kt-gpu pools), SHELL (the status counter, Diagnostics) | answered B12 (resident memory + VRAM in GB, the hover expands; the project's disk only) | t3962–t3967 |
| `real-3d-groups` **v+** | L | `shared-events` (MODEL); `stream-picture-stays` and `fx-graph-dag` (GPU) | MODEL (`Track.three_d`, `Mask.extrude`), the plan (`Layer::Group3d`), GPU (a mesh pipeline with depth) | answered B10 (intersecting planes and extruded shapes now; a track switch) | t3952–t3959 |
| `wasm-plugin-host` | L | — (new crates `kt-plugin-wasm`, `kt-plugin-sdk`) | a new host crate; GPU (plugin WGSL through naga and the pipeline cache); the plugin registry door; `download-library`'s plugin rows follow it | answered B11 (wasmtime + WGSL; layered scenes later) | t3957–t3963 |

These three take free slots by their areas: `wasm-plugin-host` touches no hot file and can fill any wave;
`memory-breakdown` touches SHELL only at its end; `real-3d-groups` waits for the GPU work of waves 1–2.

## Answered in the second round, revised by spec-features (kkt-26.11.20b B5–B9)

`auto-ducking-by-role` (B5: several key tracks, hand edits), `loudness-norms` (B6: project-level
normalization), `motion-driven-params` (B7: the edit's speed), `title-word-effects` (B8: many ranges
from the start), `window-chrome` (B9: the preview drag dropped) are being revised by spec-features. Their
rows below keep their areas; the "open questions" column no longer holds for them once the revision lands.

## Ready except for a question not yet put to the owner

These designs came from the `v26.10` registry rows (spec-features, 2026-11-20). Their questions are not in
`docs/owner-checks.md` yet. Once answered, each takes the first free slot whose areas are free.

| Change | Size | Needs first | Areas | Open questions | Tests |
|---|---|---|---|---|---|
| `title-word-effects` **v+** | M | `magic-wand-mask` (MASKS) | MASKS (`TextRange`, `Isolate`), `kt-text` coverage | Q1 how the word is chosen | t3942–t3947 |
| `auto-ducking-by-role` **v+** | M | `shared-events` | MODEL (`AudioRole`), AUDIO (role mixes as keys), `track_window.rs` | Q1 live or written curve, Q2 the R.65 key | t3911–t3918 |
| `motion-driven-params` **v+** | M | `shared-events` | MODEL (link addresses), PLAYER (`preview_speed`), automation | Q1 rewinding and the render | t3947–t3952 (t3947 shared with title-word-effects: renumber at apply) |
| `presets-everywhere` | M | `fx-preset-change` (the head menu) | MODEL `ops.rs` (clipboard blocks), FX-UI head menu, user store | Q1 copy or live link, Q2 built-in presets | t3904–t3907 |
| `loudness-norms` | S | — | EXPORT (`ExportReport.sound`), `kt-fx` loudness, DIALOG | Q1 default norm, Q2 a normalize button | t3919–t3923 |
| `picture-check` | M | `loudness-norms` (the shared findings list in DIALOG) | EXPORT (per-frame statistics), MODEL `lint`, DIALOG | Q1 findings as markers, Q2 the flashing check | t3908–t3910 |
| `project-scale` **v+** | M | `shared-events` (it measures before and after) | MODEL (`project_file.rs` single-pass read, chain table) | Q1 "does not hang" at a million, Q2 chains or events | t3924–t3931 |
| `detached-windows` | M | — | SHELL (`window_shell.rs`, viewports) | Q1 which windows, Q2 Wayland placement | t3901–t3903 |
| `window-chrome` | M | `detached-windows` (SHELL), `fx-graph` (FX-UI icon buttons) | SHELL, FX-UI | Q1 dragging a window by the preview | t3935–t3942 (t3942 shared with title-word-effects: renumber at apply) |

## Findings while ordering (for the orchestrator)

- **Test numbers reserved twice:** pro-keyer t3585–t3599 overlaps preview-canvas t3597–t3599, and
  title-word-effects t3942–t3947 overlaps window-chrome (t3942) and motion-driven-params (t3947). The
  executor that applies the second change renumbers. Better: the spec session renumbers before either
  starts.
- **`.kt` version by content:** eleven changes raise it (**v+**). Every such design says "the next free
  number at merge" (`hdr-sources-to-sdr` corrected in this commit); a merge that finds the number taken
  renumbers.
- **`project-cache-folder` is answered** (B2), so `durable-history-and-mix` (★) follows it in wave 3 as
  planned.
- **More test numbers reserved twice** by the changes added since: real-3d-groups t3952–t3959 overlaps
  motion-driven-params (t3952) and wasm-plugin-host t3957–t3963 (t3957–t3959); wasm-plugin-host overlaps
  memory-breakdown t3962–t3967 (t3962–t3963).
- **The persistent cache (B1) and project-cache-folder share one store, agreed with spec-features
  (2026-11-22):** `results/v<FORMAT>/<device class>/{frames, layers, history}`, `mix/`, source copies under
  `media/<content id>/`. `durable_hash` keys, plus the decoder major, the kernels' versions and
  `COMPOSITE_PATH_VERSION`, not the build: an update keeps what it did not change, and a stale entry is
  dropped on a miss. One order by value per byte across results, mix and copies (the history is not
  evicted first), under the folder's cyclic rule.
