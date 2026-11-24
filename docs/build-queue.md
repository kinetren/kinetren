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
| DECODE | ★ `device-decode-sources` | **section 1 and 2.1 may start now** (kt-ffmpeg, `fixtures/fake_va`, `kt-time/mem.rs`). 2.2 on waits for `persistent-cache` (`kt-media/ffmpeg.rs`, `copy.rs`, `worker.rs`); 3 follows |
| TESTS | `rest-is-said` (revision 2: 966, 967, 969 are fixed and cut; left 958 `idle`, 960, 961 and the class) | section 1 (`kt-time`, nextest, finish.sh) and 3 now; 2 and 4 after `persistent-cache` (PLAYER, `render_cache.rs`) |
| EXPORT, DIALOG | `loudness-norms` runs; then ★ `export-into-surface` 4–6 (its 1–3 are GPU and kt-ffmpeg) or `cached-final-render` from 2.2: one at a time | after `loudness-norms` |
| any | **two small fixes that stand alone:** row 1068 (`adopt_sources`, cached-final-render 1.1: a wrong picture in a render) and row 1067 (a track's graph, track-stream-stage 1.0) | now; 1067 beside `shared-events` is a text merge in `video.rs` |
| MODEL (files v2d does not write) | `timecode-is-text` (`genmedia.rs`, `project_file.rs`, `legacy_import.rs`, `gen_ui.rs`) | now; format 8 is merged |
| MODEL (all of it) | ★ `shared-events` | **after v2d merges and not beside `persistent-cache`** (`worker.rs`, `player.rs` signatures); alone in kt-model/kt-engine until its section 3 is merged |
| AUDIO | ★ `track-stage-preroll`, then ★ `durable-history-and-mix` (the mix tier) | after `persistent-cache` (both write `player.rs`) |
| COLOR-FX | `chroma-key-upgrade` **v+** | now (the views stand in the Viewer's badge; only `Viewer::cut` is a text merge with v2d) |
| MASKS | `magic-wand-mask` **v+** | now; its model field is one enum variant, a text merge with v2d |
| PREVIEW-UI | `preview-canvas-and-gizmos` | now: it stood after `frame-format-variants` by area order only, its design asks nothing of it |
| SHELL | `memory-breakdown` | now (it and device-decode 2.1 both append a `Tenant`: the second to merge takes the next number) |
| LIBRARY | `download-library` | now: stages 1–4; stage 5 after its Q4 |
| GPU | `scopes` | now; not beside `export-into-surface` (`engine.rs`) |
| new crates | `wasm-plugin-host`; `openfx-host` after it | now |
| release build | `arm-build` (Linux aarch64) | now; it shares no file with anybody |
| the plan | `simulated-elements` | after `motion-driven-params` (itself after `shared-events`); `durable-moment-key` is in |

**Recommended next three**, as the slots free:
1. ★ `device-decode-sources` at the first free slot: section 1 first, the rest once `persistent-cache` is in.
2. ★ `shared-events` as soon as v2d is in and `persistent-cache` too. Every later MODEL row waits for it.
3. ★ `export-into-surface` (A13: "Сделать сейчас"): 1–3 now, 4 on after `loudness-norms`; its section 2 not beside `device-decode-sources` section 1 (both write the fake
   VAAPI driver and `mem.rs`).

## Order after that

### The engine

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| ★ `shared-events` | L | v2d, `persistent-cache` | MODEL (every event vector, mechanical), the builders' keys (`incremental.rs`, `audio.rs`, `video.rs`) | answered A19 | t3367–t3375 |
| ★ `device-decode-sources` | L | section 2.2 on: `persistent-cache` | DECODE, `kt-time` `mem.rs`, `trace.rs`; SHELL (a Preferences checkbox, a Diagnostics line) | answered A12 | t3300–t3311, t4100–t4102 |
| `rest-is-said` | M | sections 2 and 4: `persistent-cache` | TESTS, PLAYER (the prefetch's near part), `audio_out.rs`, `render_cache.rs` | none | t4117–t4124 |
| ★ `track-stage-preroll` | S | `persistent-cache` (`player.rs`) | AUDIO | answered A15 | t3361–t3366, t4103–t4104 |
| ★ `durable-history-and-mix` (the mix tier only: D6 withdrew the history on disk) | M | `track-stage-preroll` (`mix.rs`) | CACHE (`mix/`), AUDIO, PLAYER | answered A14, D6 | t3336, t3337, t3346–t3352 |
| `track-stream-stage` | L | section 1.0 (row 1067): nothing; the rest: row 1068's fix and `shared-events`' engine sections (`video.rs`, `audio.rs`) | STREAM, `video.rs` (`Look`, `streams_at`), GPU (one op in `engine.rs`), AUDIO (`audio.rs` `TrackStage`), FX-UI (`track_window.rs`) | answered A23; implementation-ready 2026-11-24 | t3379–t3398, t4137 |
| `persistent-cache` | L | **nothing** (corrected 2026-11-24, read at 14d30724: below) | CACHE, PLAYER, DECODE (`copy.rs`), SHELL in section 4 | answered B1, D1 | t3440–t3453, t4106 |
| `cached-final-render` | L | 1.1 (row 1068) nothing; 1.2 and 2.1 nothing; from 2.2 on `loudness-norms` (EXPORT, DIALOG) | EXPORT, kt-ffmpeg (`muxer_impl.rs`, new `nal.rs`, `gop.rs`), kt-plugin-api + plugins (two facts), CACHE (`Tenant::Render`), DIALOG | answered A16; implementation-ready 2026-11-24 | t3322–t3335, t3870–t3876, t4138–t4141 |
| `hdr-sources-to-sdr`, 8-bit stages 1–4 | M | `persistent-cache` (`ffmpeg.rs`, `copy.rs`); **not** `device-decode-sources` (its pictures take the same host conversion), but not beside its sections 1–2 | DECODE (`colour.rs`, new `hdr.rs`, `source_impl.rs`, `kt-media/ffmpeg.rs`) | answered; implementation-ready 2026-11-24 | t3800–t3807, t3810, t3811, t4142, t4143 |
| `hdr-sources-to-sdr`, 16-bit stage 5 **v+** | XL | every GPU, STREAM and COLOR-FX row | all core; runs with UI-only work beside it | answered B4 | t3877–t3899 |
| `simulated-elements` | XL | `motion-driven-params` | the plan (`video.rs`), GPU (a sprite pass), MODEL (`SetSets`), SHELL | answered (D3) | t3968–t3973, t3983–t3992, t4107 |
| ★ `export-into-surface` (A13, the dma-buf half of `export-host-copy`) | L | sections 1–3 nothing; 4 on `loudness-norms` (running: EXPORT); not beside `device-decode-sources` 1 (`fake_va`, `mem.rs`) | EXPORT, GPU (`surface.rs`, `engine.rs`, `yuv.wgsl`), kt-ffmpeg muxer, `hw_trial`; CACHE in section 5 | answered A13; his check C11 | t4125–t4136 |

### The model and the interface

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `timecode-is-text` | M | — | MODEL (`genmedia.rs`, `project_file.rs`, kt-otio import), `plugins/generators-test` | none | t3993–t3997 |
| `fx-preset-change` | M | — (v2d is merged) | FX-UI (`node_editor.rs`, `fx_registry.rs`); no kt-model | answered A4; implementation-ready 2026-11-24 | t3595–t3596, t3932–t3934, t4243–t4244 |
| `preview-canvas-and-gizmos` | M | — | PREVIEW-UI, `controls.rs` (a command), kt-plugin-api `UiDecl.handles` | answered A26; implementation-ready 2026-11-24 | t3597–t3599, t4229–t4231 |
| `frame-format-variants` **v+** | L | `shared-events`; `preview-canvas-and-gizmos` (PREVIEW-UI) | MODEL (`Project::in_format`), `kt-text` (`wrap_lines`), one field of `moment_key.rs`, PREVIEW-UI, DIALOG (the render queue) | answered A17; implementation-ready 2026-11-24 | t3485–t3492, t4235–t4238 |
| `multicam` **v+** | L | `shared-events` | MODEL, TIMELINE-UI, the preview area | answered A18 | t3505–t3514 |
| `motion-driven-params` **v+** | M | `shared-events` | MODEL (link addresses), PLAYER, automation | answered B7 | t3947–t3951, t3967 |
| `real-3d-groups` **v+** | L | `shared-events`; `fx-graph-dag` (GPU) | MODEL, the plan, GPU (a mesh pipeline) | answered B10 | t3952–t3956 |
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
| `loudness-norms` | M | `render-result-check` (DIALOG); takes the unmerged `render-loudness-norms` in | EXPORT, AUDIO (the program stage), DIALOG | answered B6 | t3919–t3923, t3966, t4200–t4203 |
| `pause-detection` | M | — (Delete waits for `ripple-and-trim-modes`) | kt-media (`pauses.rs`, the pyramid's tail), a window | none | t3466, t4204–t4209 |
| `edit-analysis` **v+** | L | A3 `ripple-and-trim-modes`; `speech` `pause-detection`; stage B `download-library` | kt-plugin-api (`measure.rs`), kt-media, TIMELINE overlay and snap, links, a bin tab | none | t4211–t4228 |
| `picture-check` | M | `loudness-norms` (the findings list) | EXPORT, MODEL `lint`, DIALOG | **open: Q1, Q2** | t3908–t3910 |
| `chroma-key-upgrade` **v+** | M | — | COLOR-FX, kt-plugin-api + `kt-fx/geom.rs` (one hook), SHELL (the Viewer's badge) | answered A22; implementation-ready 2026-11-24 | t3558–t3567 |
| `pro-keyer` | L | `chroma-key-upgrade` | COLOR-FX, GPU, `BarrierKernel::held` | answered A22 | t3585–t3590 |
| `temporal-barrier-3dnr` **v+** | L | `track-stream-stage` (STREAM) | STREAM, GPU, `plugins/fx-video-camera` | answered A21 | t3545–t3551 |
| `download-library` **v+** (stage 4) | L | — (own-data-dir is merged) | a new crate `kt-library`, `kt-ffmpeg/search.rs`, `kt-text`, `kt-media/generator.rs`, SHELL | answered B3; **Q4 of its proposal holds stage 5 only**; implementation-ready 2026-11-24 | t3850–t3869 |
| `wasm-plugin-host` | L | — | new crates, GPU (plugin WGSL) | answered B11 | t3957–t3961 |
| `openfx-host` | L | `wasm-plugin-host` | a host process, GPU interop | answered D4 | t3974–t3979 |
| `arm-build` | M | — | the Dockerfile, release scripts, CI | answered D5 | t3980–t3982 |
| `fx-lens-and-camera` | M | `plugin-api-stage1` 4.1 (a positioned tap, geometry primitives in the API, a GPU barrier); its presets (1.x) may go with anything | COLOR-FX, GEOM | — | t1010–t1015 |
| `flatpak-flathub` | M | his Open Questions 1–6 | packaging | **open** | — |

## Findings while ordering (for the orchestrator)

- **`persistent-cache` never needed the two** this file named. The one value order it was said to take from `durable-history-and-mix` is built
  in `project-cache-folder` (`cache_folder.rs` `Tenant::{Frames, Mix, Copy}`, `by_value`): the mix will join an order that stands. Sections 1–2 (frames) touch no
  decoder. Of section 3 only ONE call concerns `device-decode-sources`: `FfmpegCopies::copy` opens a copy by `FfmpegSource::open_hw`, as the disk
  tier's segments do. It works without the change; what it lacks until then is the bound on surfaces (a heavy project opens a display and
  a pool per copy and per cursor, uncounted), the exact NV12 layout (a copy's picture depends on whether a device opened: the lossy
  tier, never an export), and one factory. `device-decode-sources` D9 and 2.3 take `copy.rs` over when it comes. Section 4 is SHELL alone.
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
export-into-surface t4125–t4136, track-stream-stage t4137, cached-final-render t4138–t4141, hdr-sources-to-sdr t4142–t4143. The next
free is t4144.
