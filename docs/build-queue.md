# Build queue of the open changes

The order in which the open OpenSpec changes (`openspec/changes/*`) go to executors, three at a time
(`docs/orchestrator.md`: "три исполнителя"; the owner: every free slot used). Written by the spec session; the
orchestrator owns it and moves rows as work lands. Rewritten 2026-11-24 (spec-engine) against orch `286c406c`
(stage 24), corrected the same day against `0c2a4065`, with spec-features' `442433da`.

**How a slot is filled.** Each change is given the AREAS it writes (below). Two changes run together only when they
share no area, or when the sharing is named here and is of different files. A change starts when what it needs first
has merged into orch. A change whose owner question is open does not start.

**One shared thing is allowed in parallel: the `.kt` version number.** A change that "raises the file by content"
takes the next free `PROJECT_FILE_VERSION_*` when it merges. If two land in one stage, the second renumbers at its
merge. These changes are marked **v+**.

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
| CACHE | `kt-cache`, `kt-engine/src/render_cache.rs`, `kt-media/src/cache_folder.rs`, `index.rs` |
| AUDIO | `kt-engine/src/audio.rs`, `mix.rs`, `fxstore.rs` |
| PLAYER | `kt-engine/src/player.rs` (prefetch, present, at-rest jobs) |
| FX-UI | `kt-app` `node_editor.rs`, `fx_editor.rs`, `widgets` chain parts |
| PREVIEW-UI | `kt-app` `geomctl.rs`, `centerctl.rs`, the preview's overlays and gestures |
| TIMELINE-UI | `kt-app` `timeline.rs`, `controls.rs` command rows |
| SHELL | `kt-app` `app.rs` (`run`, the frame, the window layers), `window_shell.rs`, `window_frame.rs` |
| DIALOG | `kt-app` `render_dialog.rs` |
| COLOR-FX | `plugins/fx-video-color` |
| LIBRARY | `kt-app` `ffmpeg_setup.rs` → `library/`, `kt-ffmpeg/src/search.rs`, `kt-dirs` |
| TESTS | `kt-time` test helpers, `.config/nextest.toml`, `scripts/live/kt_live.py`, `apps/kt-cli/tests/dist_guard.rs` |

## Running now (2026-11-25, orch after the merges of `persistent-cache`, `shared-events` 1.1 + row 835, `loudness-norms`)

| Change | Executor | Areas | What is left |
|---|---|---|---|
| row 1068 (`cached-final-render` 1.1), then ★ `export-into-surface` (A13), all sections | mosh-sharp | EXPORT, GPU (`surface.rs`, `engine.rs`, `yuv.wgsl`), kt-ffmpeg muxer, `hw_trial` | all; ★ `device-decode-sources` after it (the same files) |
| row 1067 (`track-stream-stage` 1.0), then ★ `shared-events` | fx-graph | MODEL (all of it), `video.rs`, `audio.rs`, `worker.rs`, `player.rs` signatures | sections 1.2 on; nothing holds section 3 any more |
| rows 1002, 911, 837, then 907, then `chroma-key-upgrade` | features-slow | `user_store.rs`, `kt-media/generator.rs`, live scripts; then COLOR-FX | all |

## Built: to archive, not to build

| Change | State |
|---|---|
| `project-cache-folder`, `durable-moment-key`, `persistent-cache`, `loudness-norms` | archived |
| `export-host-copy` | archived 2026-11-24 (0.2 = A13, 3.2 = `export-into-surface` 5.1) |
| `disk-bytes-once` | archived 2026-11-24 |
| `stream-picture-stays` | archived 2026-11-24; 2.4 is found-defects 909 |
| `fx-graph` | archived 2026-11-24 (t3006 in the harness closes 3.6) |
| `loop-play-latency-hold` | 13 of 16; a wine probe and the owner's repeat: an owner check, not a slot |
| `window-chrome`, `track-device-sound` and the rest of stages 22–24 | archived |

## Next ready row per free area

A row here needs nothing unmerged except what the last column names. ★ = the owner said "build now" (kkt-26.11.20).

| Area | Next row | With the three above |
|---|---|---|
| DECODE | ★ `device-decode-sources` (or `hdr-sources-to-sdr` 1–4: the same files, one at a time) | after `export-into-surface` by the orchestrator's order (mosh-sharp); `hdr` 1–4 may go first: it needs nothing unmerged |
| TESTS | `rest-is-said` (revision 3: 958 `idle`, 839, 960, 961 and the class) | every section now; 2.2 and 4 beside `shared-events` are text merges |
| EXPORT, DIALOG | `export-into-surface` runs; then `cached-final-render` from 2.2 | after `export-into-surface` 4 |
| MODEL (files v2d does not write) | `timecode-is-text` (`genmedia.rs`, `project_file.rs`, `legacy_import.rs`, `gen_ui.rs`) | now; format 8 is merged |
| MODEL (all of it) | ★ `shared-events` runs | alone in kt-model/kt-engine until its section 3 is merged; then `motion-driven-params`, `frame-format-variants`, `multicam`, `real-3d-groups` and the other **v+** rows, two at most at once (the version number) |
| AUDIO | ★ `track-stage-preroll`, then ★ `durable-history-and-mix` (the mix tier) | now: only `shared-events`' signatures in `audio.rs` (a text merge) |
| COLOR-FX | `chroma-key-upgrade` **v+** | now (the views stand in the Viewer's badge; only `Viewer::cut` is a text merge with v2d) |
| MASKS | `magic-wand-mask` **v+** | now; its model field is one enum variant, a text merge with v2d |
| PREVIEW-UI | `preview-canvas-and-gizmos` | now: it stood after `frame-format-variants` by area order only, its design asks nothing of it |
| SHELL | `memory-breakdown` | now (it and device-decode 2.1 both append a `Tenant`: the second to merge takes the next number) |
| LIBRARY | `download-library` | now: stages 1–4; stage 5 after its Q4 |
| GPU | `scopes` | now; not beside `export-into-surface` (`engine.rs`) |
| new crates | `wasm-plugin-host`; `openfx-host` after it | now (the network once, for the runtime's crates) |
| release build | `arm-build` (Linux aarch64) | now; it shares no file with anybody |
| the plan | `motion-driven-params`, then `simulated-elements` (five sessions); `track-stream-stage` | after `shared-events`; `motion-driven-params` and `track-stream-stage` 1.2–2.2 both rewrite `layer_src`/`PlanCtx`: one after the other, either first |

**Recommended next three**, as the slots free:
1. ★ `track-stage-preroll` (S): nothing holds it; `durable-history-and-mix` follows in the same hands.
2. `rest-is-said`: it says what the next reds are (its section 1 alone pays), and 4.2 is a defect a person with a slow disk has (839).
3. `hdr-sources-to-sdr` 1–4: phone footage is grey today (724); beside `export-into-surface` it shares no file.

## Order after that

### The engine

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| ★ `shared-events` | L | nothing (runs) | MODEL (every event vector, mechanical), the builders' keys (`incremental.rs`, `audio.rs`, `video.rs`) | answered A19 | t3367–t3375 |
| ★ `device-decode-sources` | L | nothing unmerged; not beside `export-into-surface` 2 nor `hdr-sources-to-sdr` 1–3 | DECODE, `kt-time` `mem.rs`, `trace.rs`; SHELL (a Preferences checkbox, a Diagnostics line) | answered A12 | t3300–t3311, t4100–t4102 |
| `rest-is-said` | M | nothing | TESTS, PLAYER (the prefetch's near part), `audio_out.rs`, `render_cache.rs`, `worker.rs` (one question) | none | t4117–t4124, t4144 |
| ★ `track-stage-preroll` | S | nothing | AUDIO, the start hold in `player.rs` | answered A15; re-read 2026-11-25 | t3361–t3366, t4103–t4104 |
| ★ `durable-history-and-mix` (the mix tier only: D6 withdrew the history on disk) | M | `track-stage-preroll` (`mix.rs`) | CACHE (`mix/`), AUDIO, PLAYER, kt-media (`sound_identity`) | answered A14, D6; re-read 2026-11-25 | t3336, t3337, t3346–t3352, t4145 |
| `track-stream-stage` | L | section 1.0 (row 1067): nothing; the rest: row 1068's fix and `shared-events`' engine sections (`video.rs`, `audio.rs`) | STREAM, `video.rs` (`Look`, `streams_at`), GPU (one op in `engine.rs`), AUDIO (`audio.rs` `TrackStage`), FX-UI (`track_window.rs`) | answered A23; implementation-ready 2026-11-24 | t3379–t3398, t4137 |
| `cached-final-render` | L | 1.1 is row 1068 (running); 1.2, 2.1 nothing; from 2.2 on `export-into-surface` 4 (EXPORT) | EXPORT, kt-ffmpeg (`muxer_impl.rs`, new `nal.rs`, `gop.rs`), kt-plugin-api + plugins (two facts), CACHE (`Tenant::Render`), DIALOG | answered A16; implementation-ready 2026-11-24 | t3322–t3335, t3870–t3876, t4138–t4141 |
| `hdr-sources-to-sdr`, 8-bit stages 1–4 | M | nothing unmerged; not beside `device-decode-sources` 1–2 | DECODE (`colour.rs`, new `hdr.rs`, `source_impl.rs`, `kt-media/ffmpeg.rs`) | answered; implementation-ready 2026-11-24; his check C13 | t3800–t3807, t3810, t3811, t4142, t4143 |
| `hdr-sources-to-sdr`, 16-bit stage 5 **v+** | XL (six sessions) | its 1–4, `shared-events`, `device-decode-sources`, `export-into-surface`, `track-stream-stage`, `scopes`, `chroma-key-upgrade` (it doubles what they write) | all core; runs with UI-only work beside it; 5.5b vendors egui-wgpu | answered B4; implementation-ready 2026-11-25 | t3877–t3899, t4155, t4156 |
| `simulated-elements` **v+** | XL (five sessions) | `motion-driven-params`, `shared-events`; better after `track-stream-stage` 1.2 | new `kt-sim`, the plan (`video.rs`), GPU (a sprite op), MODEL (`SetSets`), PLAYER (a thread), SHELL | answered (D3); implementation-ready 2026-11-25 | t3968–t3973, t3983–t3992, t4107 |
| ★ `export-into-surface` (A13, the dma-buf half of `export-host-copy`) | L | nothing (runs); not beside `device-decode-sources` 1 (`fake_va`, `mem.rs`) | EXPORT, GPU (`surface.rs`, `engine.rs`, `yuv.wgsl`), kt-ffmpeg muxer, `hw_trial`; CACHE in section 5 | answered A13; his check C11 | t4125–t4136 |

### The model and the interface

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `timecode-is-text` | M | — | MODEL (`genmedia.rs`, `project_file.rs`, kt-otio import), `plugins/generators-test` | none | t3993–t3997 |
| `fx-preset-change` | M | — (v2d is merged) | FX-UI (`node_editor.rs`, `fx_registry.rs`); no kt-model | answered A4; implementation-ready 2026-11-24 | t3595–t3596, t3932–t3934, t4243–t4244 |
| `preview-canvas-and-gizmos` | M | — | PREVIEW-UI, `controls.rs` (a command), kt-plugin-api `UiDecl.handles` | answered A26; implementation-ready 2026-11-24 | t3597–t3599, t4229–t4231 |
| `frame-format-variants` **v+** | L | `shared-events`; `preview-canvas-and-gizmos` (PREVIEW-UI) | MODEL (`Project::in_format`), `kt-text` (`wrap_lines`), one field of `moment_key.rs`, PREVIEW-UI, DIALOG (the render queue) | answered A17; implementation-ready 2026-11-24 | t3485–t3492, t4235–t4238 |
| `multicam` **v+** | L | `shared-events` | MODEL, TIMELINE-UI, the preview area | answered A18 | t3505–t3514 |
| `motion-driven-params` **v+** | M | `shared-events`; not beside `track-stream-stage` 1.2–2.2 | MODEL (`CurveSource`, `autoplan.rs`, `outer.rs`), `video.rs`, `video_stream.rs`, AUDIO, FX-UI (`automation.rs`) | answered B7; implementation-ready 2026-11-25 | t3947–t3951, t3967, t4146, t4147 |
| `real-3d-groups` **v+** | L | `shared-events`; not beside `track-stream-stage` 1.2–2.2, `scopes` (`engine.rs`) | MODEL, the plan, GPU (`mesh.rs`), GEOM (`pan3d.rs`), PREVIEW-UI, TIMELINE-UI | answered B10; implementation-ready 2026-11-25 | t3952–t3956, t4148–t4150 |
| `auto-ducking-by-role` **v+** | M | `shared-events` | MODEL (`AudioRole`), AUDIO, `track_window.rs` | **open: Q74** | t3911–t3918 |
| `project-scale` **v+** | M | `shared-events` | MODEL (`project_file.rs`) | **open: Q1, Q2** | t3924–t3931 |
| `magic-wand-mask` **v+** | M | — | MASKS, PREVIEW-UI (`maskctl.rs`: not beside `preview-canvas-and-gizmos`) | answered A20; implementation-ready 2026-11-24 | t3535–t3543 |
| `title-word-effects` **v+** | M | `magic-wand-mask` | MASKS, `kt-text` | answered B8 | t3942–t3946, t4246 |
| `point-tracker` **v+** | L | `magic-wand-mask`, `shared-events` | MASKS, MODEL, automation links, PREVIEW-UI | answered A20 | t3520–t3531 |
| `geometry-fx-flex` **v+** | M | `preview-canvas-and-gizmos` | GEOM, kt-plugin-api (`HandleDecl::Rect`), PREVIEW-UI | answered A24; implementation-ready 2026-11-24 | t3570–t3576, t4245 |
| `ripple-and-trim-modes` | L | `multicam` (TIMELINE-UI) | MODEL (`ops::ripple_plan`), TIMELINE-UI | answered A9 (owner check C9); **two questions, Q2 and Q3 of its proposal (defaults only, they hold nothing)**; implementation-ready 2026-11-24 | t3415–t3432, t4239–t4242 |
| `subtitles-and-text-editing` **v+** | XL | `download-library` stage 1, `ripple-and-trim-modes` | MODEL, `kt-media` transcribe, `kt-text`, SHELL | answered A11 | t3460–t3474 |
| `presets-everywhere` | M | `fx-preset-change` | MODEL `ops.rs`, FX-UI, the user store | **open: Q1, Q2** | t3904–t3907 |
| `detached-windows` | M | — | SHELL | **open: Q1, Q2** | t3901–t3903 |
| `scopes` | M | — (v2d is merged); not beside `export-into-surface` | GPU (`engine.rs`, `display.wgsl`), `compose_queue.rs`, `fx_thumbs.rs`, SHELL | answered A8; implementation-ready 2026-11-24 | t3400–t3405, t3410–t3412 |
| `memory-breakdown` | M | — | `kt-time` `mem.rs`, kt-gpu `memory.rs`, wgpu's `counters`, SHELL | answered B12; implementation-ready 2026-11-24 | t3962–t3965, t4232–t4234 |

### Effects, render checks, the rest

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `pause-detection` | M | — (Delete waits for `ripple-and-trim-modes`) | kt-media (`pauses.rs`, the pyramid's tail), a window | none | t3466, t4204–t4209 |
| `edit-analysis` **v+** | L | A3 `ripple-and-trim-modes`; `speech` `pause-detection`; stage B `download-library` | kt-plugin-api (`measure.rs`), kt-media, TIMELINE overlay and snap, links, a bin tab | none | t4211–t4228 |
| `picture-check` | M | — (`loudness-norms` is in) | EXPORT, MODEL `lint`, DIALOG | **open: Q1, Q2** | t3908–t3910 |
| `chroma-key-upgrade` **v+** | M | — | COLOR-FX, kt-plugin-api + `kt-fx/geom.rs` (one hook), SHELL (the Viewer's badge) | answered A22; implementation-ready 2026-11-24 | t3558–t3567 |
| `pro-keyer` | L | `chroma-key-upgrade` | COLOR-FX, GPU, `BarrierKernel::held` | answered A22 | t3585–t3590 |
| `temporal-barrier-3dnr` **v+** | L | `track-stream-stage` (STREAM) | STREAM, GPU, `plugins/fx-video-camera` | answered A21 | t3545–t3551 |
| `download-library` **v+** (stage 4) | L | — (own-data-dir is merged) | a new crate `kt-library`, `kt-ffmpeg/search.rs`, `kt-text`, `kt-media/generator.rs`, SHELL | answered B3; **Q4 of its proposal holds stage 5 only**; implementation-ready 2026-11-24 | t3850–t3869 |
| `wasm-plugin-host` | L | nothing | new `kt-plugin-wasm`, `kt-plugin-sdk`; kt-plugin-api (`Decl`), the mains, kt-cli, SHELL, FX-UI (a mark) | answered B11; implementation-ready 2026-11-25 | t3957–t3963, t4151, t4152 |
| `openfx-host` | L | `wasm-plugin-host` (the trial, the faulted kind); not beside `export-into-surface` 3, `track-stream-stage` 2 | new `kt-plugin-ofx`, GPU (`context.rs`, a foreign op), `kt-fx/plan.rs`, the mains, SHELL, FX-UI | answered D4; implementation-ready 2026-11-25 | t3974–t3979, t4153, t4154 |
| `arm-build` | M | — | the Dockerfile, release scripts, CI | answered D5 | t3980–t3982 |
| `fx-lens-and-camera` | M | `plugin-api-stage1` 4.1 (a positioned tap, geometry primitives in the API, a GPU barrier); its presets (1.x) may go with anything | COLOR-FX, GEOM | — | t1010–t1015 |
| `flatpak-flathub` | M | his Open Questions 1–6 | packaging | **open** | — |

## Findings while ordering (for the orchestrator)

- **`export-into-surface` is A13**: D6 of `export-host-copy` held it back on "wgpu offers none of that"; wgpu 30 imports dma-bufs. What stays
  unprovable without his card is in its D12; C11's text in `owner-checks.md` takes one sentence more (there).
- **The owner's one key does not let a render be reused after a trim** (rows 1069, cached-final-render D2): the cached render has a key of its own.
- **`hdr-sources-to-sdr` never needed `device-decode-sources`**: a device picture is laid out on the host and takes the same conversion, so there is no
  device twin to write.
- The five changes whose questions had not been put to him are Q66–Q74 now (and flatpak Q75).
- **Stale heads** left: `multicam`, `subtitles-and-text-editing`, `pro-keyer`, `temporal-barrier-3dnr` still say "until the owner
  answers"; he did (A11, A18, A21, A22). The six read on 2026-11-24 (scopes, download-library, frame-format-variants,
  ripple-and-trim-modes, geometry-fx-flex, fx-preset-change) are implementation-ready. Their owner is spec-features.
- t3947 is `motion-driven-params`' alone (`title-word-effects` took t4246). No number is reserved twice.

## Tests of spec-engine

Third block t4100–t4199: device-decode-sources t4100–t4102, track-stage-preroll t4103–t4104, (t4105 freed by D6),
persistent-cache t4106, simulated-elements t4107, durable-moment-key t4108–t4116, rest-is-said t4117–t4124,
export-into-surface t4125–t4136, track-stream-stage t4137, cached-final-render t4138–t4141, hdr-sources-to-sdr t4142–t4143, t4155–t4156,
rest-is-said t4144, durable-history-and-mix t4145, motion-driven-params t4146–t4147, real-3d-groups t4148–t4150, wasm-plugin-host t4151–t4152,
openfx-host t4153–t4154. The next free is t4157.
