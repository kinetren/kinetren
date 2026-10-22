# Engine architecture (src/)

Status: a high-level overview map of the engine. Contracts and the reasons behind decisions are in the OpenSpec capabilities:

| capability | about |
|---|---|
| [`time-model`](../openspec/specs/time-model/spec.md) · [why](../openspec/specs/time-model/design.md) | 90 kHz ticks, bursts, grids |
| [`transport`](../openspec/specs/transport/spec.md) · [why](../openspec/specs/transport/design.md) | clock, signed speed, loop |
| [`audio-render`](../openspec/specs/audio-render/spec.md) · [why](../openspec/specs/audio-render/design.md) | audio engine, FX state, mix, PDC, LUFS |
| [`video-plan`](../openspec/specs/video-plan/spec.md) · [why](../openspec/specs/video-plan/design.md) | one composition code, decode scale step, CPU reference/GPU |
| [`video-fx`](../openspec/specs/video-fx/spec.md) · [why](../openspec/specs/video-fx/design.md) | video effects registry, chain plan, picker |
| [`audio-fx`](../openspec/specs/audio-fx/spec.md) · [why](../openspec/specs/audio-fx/design.md) | audio node registry |
| [`transitions`](../openspec/specs/transitions/spec.md) · [why](../openspec/specs/transitions/design.md) | 33 kinds, overlap, inversion |
| [`automation`](../openspec/specs/automation/spec.md) · [why](../openspec/specs/automation/design.md) | properties, passport, curves, presets |
| [`morpher`](../openspec/specs/morpher/spec.md) · [why](../openspec/specs/morpher/design.md) | morpher layers, cutting, patches |
| [`compositions`](../openspec/specs/compositions/spec.md) · [why](../openspec/specs/compositions/design.md) | nested reels, the laws E/G, public parameters |
| [`generators`](../openspec/specs/generators/spec.md) · [why](../openspec/specs/generators/design.md) | generators and titles |
| [`media-index`](../openspec/specs/media-index/spec.md) · [why](../openspec/specs/media-index/design.md) | sources, decode, queue, sidecar, FFmpeg dlopen |
| [`export`](../openspec/specs/export/spec.md) · [why](../openspec/specs/export/design.md) | in-process export, golden tests, kt-cli |
| [`project-format`](../openspec/specs/project-format/spec.md) · [why](../openspec/specs/project-format/design.md) | project file `.kt` (own format, git), OTIO interchange doors (import/export), versions, passthrough |
| [`durability`](../openspec/specs/durability/spec.md) · [why](../openspec/specs/durability/design.md) | autosave, recovery, finish.sh, live scenarios |
| [`ui-timeline`](../openspec/specs/ui-timeline/spec.md) · [why](../openspec/specs/ui-timeline/design.md) | layout, repaint, i18n, gestures |
| [`ui-reach`](../openspec/specs/ui-reach/spec.md) · [why](../openspec/specs/ui-reach/design.md) | "there are no dead ends", right click |

## Flows and ownership

```
 cpal callback ◄── SPSC ring (rtrb, 2–4 bursts) ── audio-render thread ──► RenderGraph ──get──► BurstCache
      │ counter of consumed frames                      ▲ owns FxStateStore                     ▲ insert
      ▼                                                  │                                          │
  Transport (atomics: playhead, state, epoch) ◄──────────┘                     MediaWorker threads ×N (each owns its own FfmpegSource)
      ▲    ▲                                                                              ▲
 UI thread (egui): drawing, video requests, model edits ──► Prefetch thread ─────────────┘
                                                              Index worker (peak pyramid, thumbnails) — a separate short-lived FfmpegSource
```

Ownership rules:
- `FfmpegSource` belongs exclusively to its `MediaWorker`; clients communicate only through requests (`DecodeRequest {key, prio, generation}`) via a priority queue. Priorities: `Playback > NearPrefetch > FarPrefetch > Thumbnail`; at equal priority, the request closer to the decoder cursor goes first (this excludes ping-pong seeks). A transport seek bumps the generation, and stale requests are dropped.
- `FxStateStore` (HashMap<(ClipId, FxSlot), Box<dyn AudioFx>>) belongs only to the audio-render thread. No locks.
- `BurstCache` is shared, but it is the only shared structure (internal mutex/sharding). Key: `(source_id, kind{Audio | Video{quality}}, idx)`. Coverage is an interval set, maintained on insert/evict, and is exposed to the UI as a list of ranges.
- `Transport` is made of atomics; the playhead is advanced only by the audio-render thread, by exactly the number of frames consumed by the callback.

## Data flow

Playback: UI/transport → RenderGraph pulls by playhead → BurstCache (hit) or MediaWorker (miss, but playback does not wait: silence or last frame + underrun flag) → per-clip FX (with state from FxStateStore) → mix (equal-power crossfade for 2 overlapping clips on one track, 1/N for 3+; gain per clip) → SPSC → cpal. Video: the UI thread requests a frame by playhead on the fps grid, the compositor does alpha-over by z (track index), egui texture (double buffer).

Export: the same RenderGraph, clock = "as ready", Quality = project resolution; RGBA frames and f32 audio go into `kt-ffmpeg::Muxer` (libavformat/libavcodec of the system FFmpeg), which encodes and muxes mp4 on the fly. No external processes, no intermediate files.

Prefetch: an idempotent cycle — wanted keys (playhead lookahead ~2 s, visible range, head of the loop region pinned in the cache) minus coverage() → submit. There is no state between ticks.

## Sidecar index DB (`<project>.ktindex.db`, SQLite)

Keyed by media_id (identity key: path + size + mtime + xxhash + index_version): probe data; audio seek index (packet position → index of first sample) for sample-accurate seek; video keyframe index; audio peak pyramid (min/max, block of 256 samples, mip levels); thumbnail strip (~1–2 s, keyframes, ~160px JPEG). Rebuilding touches only the affected file: a size/mtime mismatch → xxhash check → rebuild; a bump of index_version → full rebuild; a corrupted DB → recreated without crashing. Schema — `media-index.md`.

## Audio FX

- Trait: `set_params` (smoothing by one-pole ~20 ms, does not touch buffers), `process` (f32 48k/2ch in/out), `tail() -> Tick`, `reset()` (only on seek).
- slowedreverb = rubato `SincFixedOut` (varispeed, fractional fixed-point source cursor — bursts are neither skipped nor duplicated) + freeverb (8 comb + 4 allpass per channel).
- Tails: the active audio window of a clip is extended by Σ tail of the chain — the reverb rings out past the clip's boundary.
- Seek: reset of all FX + pre-roll `max(tail, 100 ms)` discarded → deterministic warm-up.
- Post-FX audio is not cached (FX is cheaper than decode by orders of magnitude; the cache holds pre-FX). Video post-FX is the exception (R.7 item 2): a chain of 4 effects costs tens of ms per frame, so post-FX frames are memoized under a separate kind of kt-cache (`PostFxVideo{quality, fx_hash}`, idx = timeline tick) — this kind is invisible to coverage bars, prefetch and pinning (requests are kind-exact), and the truth about coverage remains pre-FX.

## Key decisions and "why it is so"

| Decision | Why |
|---|---|
| 90 kHz ticks | divisible by 24/25/30/60 fps and by the step of 1920 ticks of an audio burst; a legacy of the old engine, which worked well |
| f32 48k/2ch everywhere | one coordinate system for FX/mix; sources are converted at the decode boundary (swr) |
| Cache pre-FX; video post-FX — a memoization kind | audio FX ≪ 1 ms/burst — not cached; a video chain is tens of ms/frame, so it is cached under a key with fx_hash (invalidation is structural: a different hash = a different key), coverage is still computed ONLY from pre-FX |
| wgpu GPU compositing + CPU reference (R.29, concatenation R.44; before that — CPU in v1) | One composition code up to the backend: `VideoRenderer::frame_plan` decides layers/weights, there are two executors. CPU — the executable specification (byte goldens t11/t26/t67 untouched) and a FULL fallback with a notice; GPU (kt-gpu: fragment WGSL passes over Rgba8Uint, integer compositing byte-for-byte equal to CPU, bilinear within the t181 tolerance) — default in the preview (eframe device, readback into the old texture path) and in export (headless device per job; a GPU error = an export error, not a mix of backends). Consecutive geometric FX (Pan/CamMotion/Mirror/EventGrid/Pan3d) are fused into ONE pass of inverse mappings (transform concatenation, `kt-fx::geom`, items 307–309): no intermediate rasterizations and no 8-bit quantizations, supersampling by the Jacobian at minification; non-local FX and LayerMix are barriers. Pan3d — an inverse 3×3 homography, computed on both backends (R.44 removed "3D only on GPU" and the GpuRequired refusal). llvmpipe/lavapipe = "GPU is present". `KT_VIDEO_GPU=0/1` — forced choice |
| Export in-process via libav* of the system FFmpeg | no dependence on an external binary; the system's hardware encoders are available (nvenc/vaapi/qsv); zero temp files |
| No tokio | a pull engine on dedicated threads with queues; async gives nothing |
| kt-ffmpeg is locked behind the MediaSource trait | changing the FFmpeg major or the backend does not touch the rest of the code |

## Properties, automation, compositions, plugins

The property model (passport + value + source), zero cost when unused, who resolves curves, public parameters of a composition, the plugin contract, and physics as a function — are in `openspec/specs/automation/design.md` and `openspec/specs/compositions/design.md`.

## Runtime loading of FFmpeg (kt-ffmpeg)

The application **does not link** against libav* — otherwise the binary would be bound to the majors of the build machine (the original problem: `libavcodec.so.60: cannot open shared object
file` on a system with FFmpeg 7).

- `libs/kt-ffmpeg/vendor/ffmpeg-{6,7,8}/include` — vendored public headers; `build.rs` generates from them with bindgen **only types and constants** (three version modules). Functions are not generated.
- `src/loader.rs` iterates over the SONAMEs from newer majors to older ones (`libavcodec.so.62/61/60`), checks `avcodec_version()`, and loads a matching set of five libraries; ld.so pulls in the dependencies of the same version itself.
- `src/api.rs` — a vtable: ~45 functions are resolved through `dlsym`.
- `src/source_impl.rs` — the decoder implementation, included (`include!`) into each version module, so the struct layout always matches the loaded library. Dispatcher `Source::V6/V7/V8`.

Consequences: one binary works with the system FFmpeg 6/7/8 (with all its hardware acceleration); no plugins and no own build of FFmpeg; building the project does not require libav* dev packages (only libclang for bindgen). Limitation: this does not remove the glibc difference — if needed, the release is built in a container with an old glibc.
