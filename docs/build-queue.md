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

## Running now (2026-11-24)

| Change | Executor | Areas | What is left |
|---|---|---|---|
| `fx-graph-dag` v2d (and the rest of v2b: 2.3c, 2.5–2.7, t3228) | fx-graph | FX-UI; MODEL in small (`model.rs`/`ops.rs`: the editor's slot commands keep the wiring); `kt-cli` `probe` | `durable-moment-key` is in, so t3228, the stream memo's want for a read's stage (STREAM) and 2.6 (GPU) may go |
| `persistent-cache` (branch `persistent-cache`, 14d30724) | mosh-sharp | CACHE, PLAYER (`copies.rs`, the swap), DECODE (`copy.rs`), kt-ffmpeg (4 lines of the muxer) | 3.3's last clause, 3.4, section 4 (SHELL) and 5; found-defects 1065 |
| own-data-dir (row 829) | features-slow | `kt-dirs`, `kt-cli` `main.rs`, `trace_query.rs`, `tests/cli.rs`, the live stand | — |

## Built: to archive, not to build

| Change | State |
|---|---|
| `project-cache-folder`, `durable-moment-key` | archived |
| `export-host-copy` | built but for 0.2 (his answer is A13) and 3.2 (the idle check): close and archive; its D6 is `export-into-surface` |
| `disk-bytes-once` | 4 of 4: archive |
| `stream-picture-stays` | built; 2.4 (the compute generator's twin) is "not this change's shape" by its own text: it moves to found-defects; 4.2 is row 663's closing and the archive |
| `fx-graph` | v1a–v1d merged. 3.5–3.6 (live `vhs_sound`, `stream_sound`, `camera_mic`, `morph_pair`; idle with both windows open) go into v2d's live work; 3.1–3.2 are built as 3.10 and v1d; 5.0 is the archived `track-device-sound`; 6.1 is `fx-graph-dag`. The executor marks them and archives |
| `loop-play-latency-hold` | 13 of 16; a wine probe and the owner's repeat: an owner check, not a slot |
| `window-chrome`, `track-device-sound` and the rest of stages 22–24 | archived |

## Next ready row per free area

A row here needs nothing unmerged except what the last column names. ★ = the owner said "build now" (kkt-26.11.20).

| Area | Next row | With the three above |
|---|---|---|
| DECODE | ★ `device-decode-sources` | **section 1 and 2.1 may start now** (kt-ffmpeg, `fixtures/fake_va`, `kt-time/mem.rs`). 2.2 on waits for `persistent-cache` (`kt-media/ffmpeg.rs`, `copy.rs`, `worker.rs`) and section 3 for own-data-dir (`trace_query.rs`, kt-cli) |
| TESTS | `rest-is-said` (revision 2: 966, 967, 969 are fixed and cut; left 958 `idle`, 960, 961 and the class) | section 1 (`kt-time`, nextest, finish.sh) and 3 now; 2 and 4 after `persistent-cache` (PLAYER, `render_cache.rs`) |
| EXPORT, DIALOG | `render-result-check` (S), then ★ `export-into-surface` (A13) or `loudness-norms` (its base is the unmerged branch `render-loudness-norms`): one at a time | now |
| MODEL (files v2d does not write) | `timecode-is-text` (`genmedia.rs`, `project_file.rs`, `legacy_import.rs`, `gen_ui.rs`) | now; format 8 is merged |
| MODEL (all of it) | ★ `shared-events` | **after v2d merges and not beside `persistent-cache`** (`worker.rs`, `player.rs` signatures); alone in kt-model/kt-engine until its section 3 is merged |
| AUDIO | ★ `track-stage-preroll`, then ★ `durable-history-and-mix` (the mix tier) | after `persistent-cache` (both write `player.rs`) |
| COLOR-FX | `chroma-key-upgrade` **v+** | now (the views stand in the Viewer's badge; only `Viewer::cut` is a text merge with v2d) |
| MASKS | `magic-wand-mask` **v+** | now; its model field is one enum variant, a text merge with v2d |
| PREVIEW-UI | `preview-canvas-and-gizmos` | now: it stood after `frame-format-variants` by area order only, its design asks nothing of it |
| SHELL | `memory-breakdown` | now (it and device-decode 2.1 both append a `Tenant`: the second to merge takes the next number) |
| LIBRARY | `download-library` | after own-data-dir (`kt-dirs`) |
| GPU | `scopes` | after v2d's 2.6; not beside `export-into-surface` (`engine.rs`) |
| new crates | `wasm-plugin-host`; `openfx-host` after it | now |
| release build | `arm-build` (Linux aarch64) | now; it shares no file with anybody |
| the plan | `simulated-elements` | after `motion-driven-params` (itself after `shared-events`); `durable-moment-key` is in |

**Recommended next three**, as the slots free:
1. ★ `device-decode-sources` at the first free slot: section 1 first, the rest once `persistent-cache` is in.
2. ★ `shared-events` as soon as v2d is in and `persistent-cache` too. Every later MODEL row waits for it.
3. ★ `export-into-surface` (A13: "Сделать сейчас") after `render-result-check`; not beside `device-decode-sources` section 1 (both write the fake
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
| `track-stream-stage` | L | `fx-graph-dag` (a read's stage) | STREAM (`TrackStream`), `video.rs`, EXPORT (`stream_preroll`), AUDIO | answered A23 | t3379–t3394 |
| `persistent-cache` | L | **nothing** (corrected 2026-11-24, read at 14d30724: below) | CACHE, PLAYER, DECODE (`copy.rs`), SHELL in section 4 | answered B1, D1 | t3440–t3453, t4106 |
| `cached-final-render` | L | `render-result-check` (EXPORT) | EXPORT (a copy path, SEI), CACHE, DIALOG | answered A16 | t3322–t3335, t3870–t3876 |
| `hdr-sources-to-sdr`, 8-bit stages 1–4 | M | `device-decode-sources` | DECODE (`colour.rs`) | answered | t3800–t3811 |
| `hdr-sources-to-sdr`, 16-bit stage 5 **v+** | XL | every GPU, STREAM and COLOR-FX row | all core; runs with UI-only work beside it | answered B4 | t3877–t3899 |
| `simulated-elements` | XL | `motion-driven-params` | the plan (`video.rs`), GPU (a sprite pass), MODEL (`SetSets`), SHELL | answered (D3) | t3968–t3973, t3983–t3992, t4107 |
| ★ `export-into-surface` (A13, the dma-buf half of `export-host-copy`) | L | `render-result-check` (EXPORT); not beside `device-decode-sources` 1 (`fake_va`, `mem.rs`) | EXPORT, GPU (`surface.rs`, `engine.rs`, `yuv.wgsl`), kt-ffmpeg muxer, `hw_trial`; CACHE in section 5 | answered A13; his check C11 | t4125–t4136 |

### The model and the interface

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `timecode-is-text` | M | — | MODEL (`genmedia.rs`, `project_file.rs`, kt-otio import), `plugins/generators-test` | none | t3993–t3997 |
| `fx-preset-change` | M | `fx-graph-dag` | FX-UI, `kt-model/src/ops.rs` | answered A4 | t3595–t3596, t3932–t3934 |
| `preview-canvas-and-gizmos` | M | — | PREVIEW-UI, `controls.rs` (a command), kt-plugin-api `UiDecl.handles` | answered A26; implementation-ready 2026-11-24 | t3597–t3599, t4229–t4231 |
| `frame-format-variants` **v+** | L | `shared-events`; `preview-canvas-and-gizmos` (PREVIEW-UI) | MODEL, `kt-engine` `RenderTarget`, CACHE key (raster), PREVIEW-UI | answered A17 | t3485–t3492 |
| `multicam` **v+** | L | `shared-events` | MODEL, TIMELINE-UI, the preview area | answered A18 | t3505–t3514 |
| `motion-driven-params` **v+** | M | `shared-events` | MODEL (link addresses), PLAYER, automation | answered B7 | t3947–t3951, t3967 (t3947: see below) |
| `real-3d-groups` **v+** | L | `shared-events`; `fx-graph-dag` (GPU) | MODEL, the plan, GPU (a mesh pipeline) | answered B10 | t3952–t3956 |
| `auto-ducking-by-role` **v+** | M | `shared-events` | MODEL (`AudioRole`), AUDIO, `track_window.rs` | **open: Q74** | t3911–t3918 |
| `project-scale` **v+** | M | `shared-events` | MODEL (`project_file.rs`) | **open: Q1, Q2** | t3924–t3931 |
| `magic-wand-mask` **v+** | M | — | MASKS, PREVIEW-UI (`maskctl.rs`: not beside `preview-canvas-and-gizmos`) | answered A20; implementation-ready 2026-11-24 | t3535–t3543 |
| `title-word-effects` **v+** | M | `magic-wand-mask` | MASKS, `kt-text` | answered B8 | t3942–t3947 |
| `point-tracker` **v+** | L | `magic-wand-mask`, `shared-events` | MASKS, MODEL, automation links, PREVIEW-UI | answered A20 | t3520–t3531 |
| `geometry-fx-flex` **v+** | M | `preview-canvas-and-gizmos` | GEOM, PREVIEW-UI | answered A24 | t3570–t3576 |
| `ripple-and-trim-modes` | L | `multicam` (TIMELINE-UI) | MODEL (`ops::ripple_plan`), TIMELINE-UI | answered A9 (owner check C9) | t3415–t3432 |
| `subtitles-and-text-editing` **v+** | XL | `download-library` stage 1, `ripple-and-trim-modes` | MODEL, `kt-media` transcribe, `kt-text`, SHELL | answered A11 | t3460–t3474 |
| `presets-everywhere` | M | `fx-preset-change` | MODEL `ops.rs`, FX-UI, the user store | **open: Q1, Q2** | t3904–t3907 |
| `detached-windows` | M | — | SHELL | **open: Q1, Q2** | t3901–t3903 |
| `scopes` | M | v2d's 2.6 | GPU (a tap pass), PLAYER, SHELL | answered A8 | t3400–t3412 |
| `memory-breakdown` | M | — | `kt-time` `mem.rs`, kt-gpu `memory.rs`, wgpu's `counters`, SHELL | answered B12; implementation-ready 2026-11-24 | t3962–t3965, t4232–t4234 |

### Effects, render checks, the rest

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `render-result-check` | S | — | EXPORT (`ExportStats`), DIALOG, live `renderregion` | none | t3998 |
| `loudness-norms` | M | `render-result-check` (DIALOG); takes the unmerged `render-loudness-norms` in | EXPORT, AUDIO (the program stage), DIALOG | answered B6 | t3919–t3923, t3966, t4200–t4203 |
| `pause-detection` | M | — (Delete waits for `ripple-and-trim-modes`) | kt-media (`pauses.rs`, the pyramid's tail), a window | none | t3466, t4204–t4209 |
| `edit-analysis` **v+** | L | A3 `ripple-and-trim-modes`; `speech` `pause-detection`; stage B `download-library` | kt-plugin-api (`measure.rs`), kt-media, TIMELINE overlay and snap, links, a bin tab | none | t4211–t4228 |
| `picture-check` | M | `loudness-norms` (the findings list) | EXPORT, MODEL `lint`, DIALOG | **open: Q1, Q2** | t3908–t3910 |
| `chroma-key-upgrade` **v+** | M | — | COLOR-FX, kt-plugin-api + `kt-fx/geom.rs` (one hook), SHELL (the Viewer's badge) | answered A22; implementation-ready 2026-11-24 | t3558–t3567 |
| `pro-keyer` | L | `chroma-key-upgrade` | COLOR-FX, GPU, `BarrierKernel::held` | answered A22 | t3585–t3590 |
| `temporal-barrier-3dnr` **v+** | L | `track-stream-stage` (STREAM) | STREAM, GPU, `plugins/fx-video-camera` | answered A21 | t3545–t3551 |
| `download-library` | L | own-data-dir | LIBRARY, `kt-text`, the release build | answered B3 | t3850–t3869 |
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
- The five changes whose questions had not been put to him are Q66–Q74 now (and flatpak Q75).
- **Stale heads.** The `tasks.md` of `frame-format-variants`, `multicam`, `ripple-and-trim-modes`, `geometry-fx-flex`,
  `magic-wand-mask`, `preview-canvas-and-gizmos`, `subtitles-and-text-editing`, `pro-keyer`, `temporal-barrier-3dnr`
  and `scopes` still say "until the owner answers"; he did (A8, A9, A11, A17, A18, A20–A22, A24, A26). Their owner is
  spec-features.
- **One test number is reserved twice:** t3947, by `title-word-effects` and `motion-driven-params`. The second to be
  applied renumbers. The other collisions this file named are gone.

## Tests of spec-engine

Third block t4100–t4199: device-decode-sources t4100–t4102, track-stage-preroll t4103–t4104, (t4105 freed by D6),
persistent-cache t4106, simulated-elements t4107, durable-moment-key t4108–t4116, rest-is-said t4117–t4124,
export-into-surface t4125–t4136. The next free is t4137.
