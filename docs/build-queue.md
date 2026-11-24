# Build queue of the open changes

The order in which the open OpenSpec changes (`openspec/changes/*`) go to executors, three at a time
(`docs/orchestrator.md`: "три исполнителя"; the owner: every free slot used). Written by the spec session; the
orchestrator owns it and moves rows as work lands. Rewritten 2026-11-24 (spec-engine) against orch `286c406c`
(stage 24), with spec-features' `442433da`.

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
| `fx-graph-dag` v2d (and the rest of v2b: 2.3c, 2.5–2.7, t3228) | fx-graph | FX-UI; MODEL in small (`model.rs`/`ops.rs`: the editor's slot commands keep the wiring); `kt-cli` `probe` | 12 tasks. **Not beside durable-moment-key:** "the stream memo's want for a read's stage" and t3228 (STREAM) and 2.6's prewarm ids and held-frame count (GPU) wait until it merges |
| `durable-moment-key` | mosh-sharp | CACHE, PLAYER, STREAM (`ident`), GPU (`plan_key`), `kt-media` sources (the identity), `worker.rs` | all of it |
| own-data-dir (row 829) | features-slow | `kt-dirs`, `kt-cli` `main.rs`, `trace_query.rs`, `tests/cli.rs`, the live stand | — |

## Built: to archive, not to build

| Change | State |
|---|---|
| `project-cache-folder` | built; its archive is the first commit of branch `durable-moment-key` (`e19e9c5d`) |
| `disk-bytes-once` | 4 of 4: archive |
| `stream-picture-stays` | built; 2.4 (the compute generator's twin) is "not this change's shape" by its own text: it moves to found-defects; 4.2 is row 663's closing and the archive |
| `fx-graph` | v1a–v1d merged. 3.5–3.6 (live `vhs_sound`, `stream_sound`, `camera_mic`, `morph_pair`; idle with both windows open) go into v2d's live work; 3.1–3.2 are built as 3.10 and v1d; 5.0 is the archived `track-device-sound`; 6.1 is `fx-graph-dag`. The executor marks them and archives |
| `loop-play-latency-hold` | 13 of 16; a wine probe and the owner's repeat: an owner check, not a slot |
| `window-chrome`, `track-device-sound` and the rest of stages 22–24 | archived |

## Next ready row per free area

A row here needs nothing unmerged except what the last column names. ★ = the owner said "build now" (kkt-26.11.20).

| Area | Next row | With the three above |
|---|---|---|
| DECODE | ★ `device-decode-sources` | **section 1 and 2.1 may start now** (kt-ffmpeg, `fixtures/fake_va`, `kt-time/mem.rs`: no file of the three). 2.2 on waits for durable-moment-key (`kt-media/ffmpeg.rs`, `worker.rs`) and section 3 for own-data-dir (`trace_query.rs`, kt-cli) |
| TESTS | `rest-is-said` (the reds under load: rows 958, 960, 961, 966) | sections 1 and 3 (`kt-time`, nextest, finish.sh, the bench's end) now; section 2's actors in `player.rs` and `render_cache.rs`, and row 960, after durable-moment-key |
| EXPORT, DIALOG | `render-result-check` (S), then `loudness-norms` (its base is the unmerged branch `render-loudness-norms`) | now |
| MODEL (files v2d does not write) | `timecode-is-text` (`genmedia.rs`, `project_file.rs`, `legacy_import.rs`, `gen_ui.rs`) | now; format 8 is merged |
| MODEL (all of it) | ★ `shared-events` | **after v2d and durable-moment-key merge**, and alone in kt-model/kt-engine until its section 3 is merged |
| AUDIO | ★ `track-stage-preroll`, then ★ `durable-history-and-mix` (the mix tier) | after durable-moment-key (both write `player.rs`) |
| COLOR-FX | `chroma-key-upgrade` **v+** | now (its Viewer menu item waits for v2d) |
| MASKS | `magic-wand-mask` **v+** | now; its model field is one enum variant, a text merge with v2d |
| PREVIEW-UI | `preview-canvas-and-gizmos` | now: it stood after `frame-format-variants` by area order only, its design asks nothing of it |
| SHELL | `memory-breakdown` | now (it and device-decode 2.1 both append a `Tenant`: the second to merge takes the next number) |
| LIBRARY | `download-library` | after own-data-dir (`kt-dirs`) |
| GPU | `scopes` | after durable-moment-key and v2d's 2.6; its own need, `stream-picture-stays`, is built |
| new crates | `wasm-plugin-host`; `openfx-host` after it | now |
| release build | `arm-build` (Linux aarch64) | now; it shares no file with anybody |
| the plan | `simulated-elements` | after durable-moment-key and `motion-driven-params` (itself after `shared-events`) |

**Recommended next three**, as the slots free:
1. ★ `device-decode-sources` at the first free slot: section 1 first, the rest by the time durable-moment-key merges.
2. ★ `shared-events` as soon as v2d and durable-moment-key are in. Every later MODEL row waits for it.
3. `rest-is-said` beside either: the reds it ends are paid for in every `finish.sh`.

## Order after that

### The engine

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| ★ `shared-events` | L | v2d, `durable-moment-key` | MODEL (every event vector, mechanical), the builders' keys (`incremental.rs`, `audio.rs`, `video.rs`) | answered A19 | t3367–t3375 |
| ★ `device-decode-sources` | L | section 2.2 on: `durable-moment-key` | DECODE, `kt-time` `mem.rs`, `trace.rs`; SHELL (a Preferences checkbox, a Diagnostics line) | answered A12 | t3300–t3311, t4100–t4102 |
| `durable-moment-key` | M | — | CACHE, STREAM (`ident`), GPU (`plan_key`), PLAYER | none (D2, D7) | t4108–t4116 |
| `rest-is-said` | M | section 2 on: `durable-moment-key` | TESTS, PLAYER (the prefetch's near part), `audio_out.rs`, `render_cache.rs` | none | t4117–t4124 |
| ★ `track-stage-preroll` | S | `durable-moment-key` (`player.rs`) | AUDIO | answered A15 | t3361–t3366, t4103–t4104 |
| ★ `durable-history-and-mix` (the mix tier only: D6 withdrew the history on disk) | M | `track-stage-preroll` (`mix.rs`) | CACHE (`mix/`), AUDIO, PLAYER | answered A14, D6 | t3336, t3337, t3346–t3352 |
| `track-stream-stage` | L | `fx-graph-dag` (a read's stage) | STREAM (`TrackStream`), `video.rs`, EXPORT (`stream_preroll`), AUDIO | answered A23 | t3379–t3394 |
| `persistent-cache` | L | `durable-history-and-mix` (one value order), `device-decode-sources` | CACHE, PLAYER, DECODE (`CopySource`), EXPORT (`MuxConfig`) | answered B1, D1 | t3440–t3453, t4106 |
| `cached-final-render` | L | `render-result-check` (EXPORT) | EXPORT (a copy path, SEI), CACHE, DIALOG | answered A16 | t3322–t3335, t3870–t3876 |
| `hdr-sources-to-sdr`, 8-bit stages 1–4 | M | `device-decode-sources` | DECODE (`colour.rs`) | answered | t3800–t3811 |
| `hdr-sources-to-sdr`, 16-bit stage 5 **v+** | XL | every GPU, STREAM and COLOR-FX row | all core; runs with UI-only work beside it | answered B4 | t3877–t3899 |
| `simulated-elements` | XL | `durable-moment-key`, `motion-driven-params` | the plan (`video.rs`), GPU (a sprite pass), MODEL (`SetSets`), SHELL | answered (D3) | t3968–t3973, t3983–t3992, t4107 |
| ★ `export-host-copy`, the dma-buf half (A13) | — | **a design**: the change's D6 is a paragraph, no task, no test | EXPORT, GPU | answered A13 | t3320–t3321 free |

### The model and the interface

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `timecode-is-text` | M | — | MODEL (`genmedia.rs`, `project_file.rs`, kt-otio import), `plugins/generators-test` | none | t3993–t3997 |
| `fx-preset-change` | M | `fx-graph-dag` | FX-UI, `kt-model/src/ops.rs` | answered A4 | t3595–t3596, t3932–t3934 |
| `preview-canvas-and-gizmos` | M | — | PREVIEW-UI, `ParamSpec.handle` | answered A26 | t3597–t3599 |
| `frame-format-variants` **v+** | L | `shared-events`; `preview-canvas-and-gizmos` (PREVIEW-UI) | MODEL, `kt-engine` `RenderTarget`, CACHE key (raster), PREVIEW-UI | answered A17 | t3485–t3492 |
| `multicam` **v+** | L | `shared-events` | MODEL, TIMELINE-UI, the preview area | answered A18 | t3505–t3514 |
| `motion-driven-params` **v+** | M | `shared-events` | MODEL (link addresses), PLAYER, automation | answered B7 | t3947–t3951, t3967 (t3947: see below) |
| `real-3d-groups` **v+** | L | `shared-events`; `fx-graph-dag` (GPU) | MODEL, the plan, GPU (a mesh pipeline) | answered B10 | t3952–t3956 |
| `auto-ducking-by-role` **v+** | M | `shared-events` | MODEL (`AudioRole`), AUDIO, `track_window.rs` | **open: Q1**, not in owner-checks | t3911–t3918 |
| `project-scale` **v+** | M | `shared-events` | MODEL (`project_file.rs`) | **open: Q1, Q2** | t3924–t3931 |
| `magic-wand-mask` **v+** | M | — | MASKS, PREVIEW-UI | answered A20 | t3535–t3540 |
| `title-word-effects` **v+** | M | `magic-wand-mask` | MASKS, `kt-text` | answered B8 | t3942–t3947 |
| `point-tracker` **v+** | L | `magic-wand-mask`, `shared-events` | MASKS, MODEL, automation links, PREVIEW-UI | answered A20 | t3520–t3531 |
| `geometry-fx-flex` **v+** | M | `preview-canvas-and-gizmos` | GEOM, PREVIEW-UI | answered A24 | t3570–t3576 |
| `ripple-and-trim-modes` | L | `multicam` (TIMELINE-UI) | MODEL (`ops::ripple_plan`), TIMELINE-UI | answered A9 (owner check C9) | t3415–t3432 |
| `subtitles-and-text-editing` **v+** | XL | `download-library` stage 1, `ripple-and-trim-modes` | MODEL, `kt-media` transcribe, `kt-text`, SHELL | answered A11 | t3460–t3474 |
| `presets-everywhere` | M | `fx-preset-change` | MODEL `ops.rs`, FX-UI, the user store | **open: Q1, Q2** | t3904–t3907 |
| `detached-windows` | M | — | SHELL | **open: Q1, Q2** | t3901–t3903 |
| `scopes` | M | v2d's 2.6 | GPU (a tap pass), PLAYER, SHELL | answered A8 | t3400–t3412 |
| `memory-breakdown` | M | — | `kt-time` `mem.rs`, every holder, SHELL | answered B12 | t3962–t3965 |

### Effects, render checks, the rest

| Change | Size | Needs first | Areas | Owner question | Tests |
|---|---|---|---|---|---|
| `render-result-check` | S | — | EXPORT (`ExportStats`), DIALOG, live `renderregion` | none | t3998 |
| `loudness-norms` | M | `render-result-check` (DIALOG); takes the unmerged `render-loudness-norms` in | EXPORT, AUDIO (the program stage), DIALOG | answered B6 | t3919–t3923, t3966, t4200–t4203 |
| `picture-check` | M | `loudness-norms` (the findings list) | EXPORT, MODEL `lint`, DIALOG | **open: Q1, Q2** | t3908–t3910 |
| `chroma-key-upgrade` **v+** | M | — | COLOR-FX, SHELL (the Viewer's menu) | answered A22 | t3558–t3564 |
| `pro-keyer` | L | `chroma-key-upgrade` | COLOR-FX, GPU, `BarrierKernel::held` | answered A22 | t3585–t3590 |
| `temporal-barrier-3dnr` **v+** | L | `track-stream-stage` (STREAM) | STREAM, GPU, `plugins/fx-video-camera` | answered A21 | t3545–t3551 |
| `download-library` | L | own-data-dir | LIBRARY, `kt-text`, the release build | answered B3 | t3850–t3869 |
| `wasm-plugin-host` | L | — | new crates, GPU (plugin WGSL) | answered B11 | t3957–t3961 |
| `openfx-host` | L | `wasm-plugin-host` | a host process, GPU interop | answered D4 | t3974–t3979 |
| `arm-build` | M | — | the Dockerfile, release scripts, CI | answered D5 | t3980–t3982 |
| `fx-lens-and-camera` | M | `plugin-api-stage1` 4.1 (a positioned tap, geometry primitives in the API, a GPU barrier); its presets (1.x) may go with anything | COLOR-FX, GEOM | — | t1010–t1015 |
| `flatpak-flathub` | M | his Open Questions 1–6 | packaging | **open** | — |

## Findings while ordering (for the orchestrator)

- **A13's dma-buf half is not buildable yet.** `export-host-copy`'s tasks end at the prerender; what he ordered
  ("Сделать сейчас") is its D6, headed "Deferred", with no task, spec line or test. The former queue showed it in wave 2
  as ready. It needs a design before a slot.
- **Five changes hold questions nobody has put to him** (`owner-checks.md` says none are open): `detached-windows`,
  `presets-everywhere`, `project-scale`, `picture-check`, and `auto-ducking-by-role`'s Q1 (what counts as a title section).
- **Stale heads.** The `tasks.md` of `frame-format-variants`, `multicam`, `ripple-and-trim-modes`, `geometry-fx-flex`,
  `magic-wand-mask`, `preview-canvas-and-gizmos`, `subtitles-and-text-editing`, `pro-keyer`, `temporal-barrier-3dnr`
  and `scopes` still say "until the owner answers"; he did (A8, A9, A11, A17, A18, A20–A22, A24, A26). Their owner is
  spec-features.
- **One test number is reserved twice:** t3947, by `title-word-effects` and `motion-driven-params`. The second to be
  applied renumbers. The other collisions this file named are gone.

## Tests of spec-engine

Third block t4100–t4199: device-decode-sources t4100–t4102, track-stage-preroll t4103–t4104, (t4105 freed by D6),
persistent-cache t4106, simulated-elements t4107, durable-moment-key t4108–t4116, rest-is-said t4117–t4124. The next
free is t4125.
