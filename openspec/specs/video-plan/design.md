# Video composition — why it is this way

## What hurt and for whom

Each video effect was a separate rasterization into a buffer of output size, with 8-bit quantization between steps; the source was also shrunk in Quality by the decoder. On the owner's chain "2×2 grid → 3D pan → 2×2 grid → 3D pan → zoom back", after the second grid only a quarter of the detail remained, and the zoom showed mush made of enlarged pixels. The complaint, word for word, came down to "there is essentially no compositing"; the owner's proposal was tessellation and UV mapping "from scale"; the intuition is right per pixel, not per vertex. The second thing that hurt: 3D was computed only on the GPU — the CPU reference had holes, and export without a GPU refused. The third: three callers (preview, prefetch, export) each glued Quality onto the request themselves and matched only because they read one variable; on export with large frames this already produced "a black render with rare surviving frames" when a chunk did not survive until composition.

## Decisions and why

- **One frame plan, two executors.** `VideoRenderer::frame_plan` builds the layers, weights, levels, frame mapping and the allowed curves; the CPU and the GPU only execute. Everything that can be computed before the executor (curves, blending of pairs, instance overrides, transition parameters) is computed before it — then backend parity holds automatically, and the shaders know nothing about the model.
- **GPU by default, CPU is the reference and the full fallback** (the owner's decision: "always gl/vk by default, and only if not — notice and CPU"). GPU determinism is "float + tolerances" against the CPU; byte-for-byte is required only on one device. Pan3d was moved to an inverse homography in f64 in the shared geometry pass, so that the reference becomes complete and the "needs GPU" refusal disappears.
- **Export does not mix backends.** Cross-backend bit-exactness is unreachable, so switching the backend in the middle of a file is a visible seam. A GPU error in export is an export error; `TooLarge` — only this plan on the CPU (it computes the same plan).
- **Concatenation of geometry series always, with no switch** (the owner's decision: "there is no question of 'when to enable'"). A series of length 1 without minification is byte-for-byte equal to the old pass, so there is nothing to choose; the old behavior was an implementation artifact, not a style; the second path would be permanent support.
- **Chain decisions live in one kt-fx planner.** Before it, the CPU and the GPU segmented the chain with two copies of one traversal. Now a segmentation mismatch is impossible by construction; the executors receive the ops Run/Barrier/Snapshot/Mix.
- **Barriers at K× and the decode scale step are decided by "how much detail is actually squeezed".** K = quantum of min(zoom of the tail, compression above); the scale step S is per CLIP (the maximum net zoom over the whole duration — the owner's decision: one cache key per clip, exact coverage bars, whole decoder spans; the cost is that a clip zooming only at the end is decoded large in full). The fractional margin of the native size is rounded UP ("any file margin works"). The ceiling `MAX_INTER_DIM = 4096` is a project constant: a device limit would make export depend on the machine.
- **Decode quality is part of the request.** The scale step makes the size a function of the clip, and the scheme "everyone glues Quality on themselves" would immediately diverge: prefetch would warm one thing, the composite would read another. Therefore the request carries `(media, pts, Quality)`, and the coverage bars, export chunks and prefetch all ask the same thing.
- **Memoization of post-FX — video only, preview only.** The video post-FX frame is a function without history, while the audio chain is a streaming automaton (its output cannot be cached). In export each t is composed once, and full-size post-FX frames were only evicting the pre-frames of neighboring tracks — hence the lost layers.
- **The camera — one rule per program**, moved out of a private function for the sake of a second consumer (the picker's thumbnails): a rewritten rule would silently diverge.
- **"Procedural" (no frame grid) and "shapeless" (no proportions) are two facts** (still-image-frame-rule, ktprompts 26.10.18 item 9). R.62 equated them — every procedural source of that day (generator, title, placeholder) drew its frame at the requested quality. R.67 then gave a single picture `fps: None` for its timing, the picture became "procedural", and the renderer silently dropped its view: every frame rule of a still turned into stretch, a square photo was squashed into 16:9. Three readers held their own copy of the decision — the renderer's lead (`else if procedural { None }`), the on-screen frame (`Player::source_view`) and `kt-cli probe` — and the last two kept the view, so the window drew a fit frame the render did not use. Now `SourceFacts::observe` decides both facts in one place (shapeless = no fps and not a still), and all three read `view_of`/`lead_view`.
- **"As is" counts the pixels of the composite's raster, not of the canvas being rendered** (same change, found by its live scenario). The preview renders at a fraction of the project, so "1:1 in canvas pixels" drew the picture twice as large at ½ as the file gets it. `LeadCtx.raster` carries the logical size (the project, or a reel's own raster); when the canvas is the raster the factor is not applied, and the goldens keep their bytes.
- **A strength slot at exactly 0 does not run its layer** (morpher-event-grid-cost, D3). The morpher's
  fades put the strength at 0 at the block's edges, and the layer was computed in full and mixed away.
  The executors skip the slice from the snapshot to its strength slot and still run the mix, on
  (dry, dry): `dissolve_into` at k = 0 takes nothing from the wet side, so the bytes are the full
  path's. Only where the wet frame could not have another size (the CPU: the dry frame already at
  the output raster; the GPU: the slice did not change the size) and the slice is whole (no other
  stop inside). Not a strip of the chain: dropping the slots would let neighbouring geometry runs fuse
  across the removed stop and move bytes. t945 (CPU), t945b (GPU).
- **The compose cost is counted by the executor that ran it** (`kt_fx::video::FrameCost`, D1): the CPU
  through `FxScratch`, the GPU from its op list (`VideoGpuExec::last_cost`), the player adds the
  backend and the wall-clock compose time (the preview path — invariant 11 forbids it only offline).
  Sub-samples are a pure function of the program (`GeomProgram::sample_cost`), so a test can hold the
  cost, and both backends report the same number. Before it, "the fps drops" could not be split
  between the grid, the strength slot, the readback and a CPU fallback.

## Rejected and why

- **Tessellation / vertex UV mapping** — all five geometric effects are analytically invertible (affines, tile-modulo, planar homography), and the inverse per-pixel mapping covers the same more simply and more precisely; compute and tessellation are unavailable under the downlevel limits of egui-wgpu on GL (GL only: llvmpipe on Vulkan runs compute, F-EXT-004).
- **A merge switch (UI or env flag)** — rejected by the owner: if the merged result is mathematically equal to the step-by-step result minus the losses, there is nothing to choose.
- **Build the geometry program straight into K·out** — breaks the integer geometry of the grid (10 px / 4 columns at K = 2 give a different cell layout). Instead, conjugation: the program is in 1× semantics, the scales are contributed by two affines at the edges, and at K = 1 both are exactly identical.
- **Hysteresis for "flapping K" and for the scale step** on the frame where the threshold is crossed — forbidden by determinism; the size of the jump is bounded by the tolerance of the kernels' invariance to K.
- **Fixed-function blending for the composite** — the integer alpha_over formula with the factor (1−sa)·da cannot be expressed in it; ping-pong of two accumulators in the shader.
- **Samplers and compute on the GPU** — the sub-texel filtering grid drifts across drivers, and compute is unavailable on GL downlevel; bilinear done by hand with 4 texelLoad calls.
- **A separate SSE tier of SIMD** — the f64 kernels need 4 lanes, and a bit-exact second implementation would have to be kept forever; the levels are only Scalar/Avx2.
- **Ad-hoc epsilon in GPU tests** — without a tolerance class every test would invent its own threshold, and "borderline" cases would be tweaked instead of investigated.
- **Post-FX cache of kt-cache on the GPU** — per-layer readback would eat up the whole gain; the GPU has its own memoization of the whole plan keyed by the key.
- **Scale step per frame, not per clip** — would break the decoder spans and give two cache keys for one clip; the coverage bars would start lying.

## Non-obvious details

- A series of length 1 of Pan/CamMotion/Mirror without minification goes through the OLD kernels (`GeomExec::Legacy`, `src/libs/kt-fx/src/geom.rs`); a single EventGrid goes through the fused pass, but the tile-local phase reproduces the old arithmetic verbatim; therefore the goldens of t26 stand without regeneration. A single Mirror goes to Legacy ONLY when the edge scales are equal (`k_in == k_out`): the pixel permutation is correct at a uniform scale, but not at a change of scale between K×-segments — this was a latent debug crash (`src/libs/kt-fx/src/plan.rs`).
- Supersampling by the Jacobian at exactly ×2 minification arithmetically coincided with the old bilinear filter in the half-texel phase — the sanctioned regeneration of goldens was not needed. For affine programs the taps are computed once in f64 and carried as a uniform; Proj series measure the Jacobian per pixel with the SAME formula `ss_taps` (`src/libs/kt-fx/src/geom.rs`), otherwise the backends at the threshold would choose different tap grids, and no FrameTol would cover that.
- The fused geometry pass must be edited in PAIRS: `src/libs/kt-fx/src/geom.rs`/`plan.rs` ↔ `src/libs/kt-gpu/src/shaders/geom_fused.wgsl` (both copies of the primitive loop). The coefficients are multiplied in f64 on the CPU and passed as f32 rounded once; `FxOp::slot_units()` (`src/libs/kt-gpu/src/engine.rs`) must predict the consumption of the arena exactly, as `passes()` does.
- The leading barrier (before the first geometry run) at scale step S is executed at S× FORCIBLY — the frame under it is physically large, and the benefit formula does not vote there; the byte fast-path remains only when `S == 1 && all K == 1` (`src/libs/kt-fx/src/plan.rs::plan_chain`). The layer strength slice receives `in_scale` only if its input matches the decode frame in size; after the first resample and on the stops — 1× (`src/libs/kt-engine/src/video.rs`).
- `chain_ops` returns the maximum of the INTERMEDIATE sizes, and `resolve_chain` checks it against the device limit → a clean `TooLarge` (`src/libs/kt-gpu/src/engine.rs`); otherwise `create_texture` of the K×-buffer would poison the device with validation.
- The scale step of a clip is computed from the natives of OPEN sources (they are not in the sidecar): `VideoRenderer::new` does not know the natives (the step ≡ 1 — all earlier tests stay alive); the player and export call `new_with_natives`/`new_with_facts` (`src/libs/kt-engine/src/video.rs`). `replace_media` bumps the project version, and the prefetch thread rebuilt its renderer, while the player's renderer lived until the model edit — therefore a change in the composition of sources rebuilds the player's renderer too (`src/libs/kt-engine/src/player.rs`).
- The logical base `head_zoom` is the size of the PROJECT, not Quality: the zoom barely depends on Quality, but the step must match between preview and export. The sampler is a grid of 33 points + both ends + all curve keys + the camera boundaries (`src/libs/kt-engine/src/video.rs::clip_decode_scale`, `src/libs/kt-fx/src/plan.rs::chain_head_zoom`).
- The REEL's scale step and the INSTANCE chain's input are two different numbers: an empty chain, the layer strength, the leading stop and the barriers without a resample of the step do not consume it, and nobody would return an S×-frame to Quality for them. If they diverge — `NestSrc::fit_to` (`src/libs/kt-engine/src/video_plan.rs`), CPU `kt_fx::video::fit_into`, GPU `FxOp::Resample`; the reel's step is in `postfx_key_hash` and `plan_key`.
- A layer of a foreign size → `complete = false`; for a reel there is additionally a `debug_assert`: its size is our arithmetic, not a message from the decoder. Before this the composite skipped the layer and declared the frame complete, and export wrote it as good (`src/libs/kt-engine/src/video.rs::compose_from_cache*`, mirror in `src/libs/kt-gpu/src/engine.rs`).
- The key of post-FX memoization (`src/libs/kt-engine/src/video.rs::postfx_key_hash`) includes the resolved camera: it is animated and selected per frame, and the static `fx_hash` does not see it. The GPU is protected from this by `put_cam3d` in `plan_key` (`src/libs/kt-gpu/src/engine.rs`). The input step is also in the key: one clip at one Quality from input 1× and 2× gives different bytes.
- `fx_chain_hash` hashes both `fx` and `auto`, so a change of speed/curves invalidates the post-FX memo by itself; the idx memo is the timeline tick (`src/libs/kt-engine/src/video.rs`).
- `LayerBlend` stays in the layer as METADATA (neighbor + weight): the key of the kt-gpu plan cache needs it — the same two frames with a different weight give a different picture. `max_concurrent_layers` counts a blending layer as TWO frames; otherwise the export chunk would evict the neighbor before composition, and the export bytes would depend on the timing of the cache (`src/libs/kt-engine/src/video.rs`).
- `u` (the progress of the event) and `burst_index` (the noise seed) follow the OUTPUT time even under warp: otherwise the Ken Burns motion would be squeezed into the first third of a slowed clip, and the grain would freeze on repeated frames (`src/libs/kt-engine/src/video.rs::layer_src`).
- Reverse + stretch is "warp, then mirror": a mirror before the warp would drive the speed curve backwards and desynchronize A/V. The mirror axis is the span of the material, not the "silence beyond" boundary with the resampler's margin (`src/libs/kt-model/src/warp.rs`, `src/libs/kt-engine/src/video.rs::clip_material_pos`).
- The floor on the frame grid is set only by the FILE sheet: for a procedural source the frame is a function of the tick, and a reel has no floor at all (`clip_inner_tick == clip_material_pos`); otherwise a slowed title went in steps of two frames (`src/libs/kt-engine/src/video.rs`).
- The end of a transition on a cut clip is found by `kt_model::same_clip` + a check of the boundary, not by `c.id == id` — otherwise it went to piece №0 and silently disappeared (`src/libs/kt-engine/src/video.rs`, `VideoRenderer::new`).
- "The one that started LATER is on top" — the composition key is `(Reverse(lane), start)` (`src/libs/kt-engine/src/video.rs`); the timeline lane draws in the same order.
- All GPU textures are `Rgba8Uint`: `textureLoad` returns raw bytes without unorm losses, so the integer kernels equal the CPU byte-for-byte. wgpu is STRICTLY pinned to the same major version as eframe (the workspace pin in `src/Cargo.toml`): the preview shares an `Arc<wgpu::Device>` with egui-wgpu.
- The crossfade does not go into the transition kernels — it remains the integer `dissolve_into` (GOLDEN_VIDEO rests on it); the ext of volumetric transitions is computed by one `trans_ext` (`src/libs/kt-engine/src/transitions.rs`), f64→f32 on the CPU, and is not part of the keys — the function is already hashed.
- In the SIMD kernels, FMA, reassociation, rcp/rsqrt, f32 instead of f64, and `_mm256_round_pd(NEAREST)` (half-to-even versus half-away) are forbidden; `/255` is the magic `(x*0x8080_8081)>>39`, the usual `(t+(t>>8))>>8` is wrong at 255³ (`src/libs/kt-engine/src/video_simd.rs`, `src/libs/kt-fx/src/video_simd.rs`).
- A test trap: a fresh `BurstCache` renumbers the content anew, and kt-gpu's `UploadCache` is keyed ONLY by a serial number — two caches for one engine give identical keys for different frames; in a test there is one cache for all moments (`src/libs/kt-gpu/src/engine.rs`).
- Tests t315 and t296/t302 are red-proof: they are copied into a worktree at the commit before the round, and it is confirmed that they are red there; sharpness cannot be measured on `SyntheticSource` (the native is fictitious), a `CheckerSource` is needed (`src/libs/kt-media/src/synthetic.rs`).

## Deferred

- **Frame-rate conform of the source** (F-SPD-005): the rule "floor / nearest / blend" at the project level with an exception at the event level, in the same pair as the frame conform rule; the default "floor" is today's `clip_frame_keys`, the goldens do not move. Deferred from R.62: a different pipeline stage (the map tick→frame, not geometry).
- **Color management** (F-CLR-008): space and range tags in the source facts, the project's working space, conform as a CPU + WGSL pair, output tags. Deferred from R.62: a round the size of transitions; the first piece — tags and the range 16–235 with the default "as is", byte-for-byte.

- AVX2 of the fused geometry pass — by measurement and the 1.3× rule (today it is scalar f64: the owner's chain is 388 ms/frame at 1080p on the CPU fallback).
- Two clips of one file with different steps alternate spans of different qualities and recreate the sws context on every switch — accepted by the owner, amortized by the span length.
- `FrameTol::WARP` is not introduced: hard warps are held by `GEOM`; a failure of warp-hard — an investigation and the introduction of WARP ×2 from a measurement, not a tweak of GEOM.
- **R.62 B, tracks:** a CLIP-level blend mode — the event already has a level and transitions, and mixing two axes in one round would mean deciding for the owner. Track envelopes — see `audio-render/design.md`, the key is on another time axis. Skipping the decode of a muted track: mute excludes it from `build_plan`, but NOT from `wanted_frames` — the request and the read must match (invariant 4), otherwise unmuting gives a black frame until warm-up.

## Shape masks in the plan (shape-masks, 2026-11-01)

The owner asked for classic masks (kkt-26.11.01 S1). A mask enters the plan as an internal chain
point, the way the leading Geometry node and the morpher's strength already do: the picture's masks
as an ALPHA point right after the leading node (crop → mask → placement: the node reserved that
place in R.62, and a mask there follows the event's move, zoom and rotation), a slot's masks as a MIX
point (the morpher's wet/dry with a coverage instead of a scalar), a track's after its chain. The
coverage is rasterized where it is applied — a slot before the node runs on the source raster, after
it on the canvas, and only a rasterization at the point of use covers the same CONTENT in both.

Rejected: one canvas mask applied at the composite (the slot before the node would be masked in the
wrong space); a GPU rasterizer (no compute, no storage buffers, three textures — and a second
rasterizer held to a threshold tolerance). The GPU applies the CPU's coverage bytes in an integer
pass, so a mask adds no divergence of its own (t1274). A masked slot at index 0 is a leading stop
and forbids the decode step, as a morpher stop does. A slot after the node follows only the LEADING
node: a later Geometry node or a lens is not followed (said in the spec).

## The preview keeps up (preview-throughput, 2026-11-04)

The measurements are `kt-cli preview <project> <from> <to> [--size WxH] [--cpu]`: the real `Player`
(workers, prefetch, audio render) against an output device made of the wall clock, and a 60 Hz
display loop doing what `draw_preview` does, with the same ratchet (`preview_key_step`) and the
same "shown" and "skipped" counts as the performance panel. Release build, the showcase project,
34–46 s, footage in /dev/shm (on the disk the runs differ by 215 incomplete composites), the binaries
alternating so load falls on each alike.

**This machine has no real graphics card.** Every "GPU" number is llvmpipe on 32 cores. A real card
changes the size of the wait, not its cause; the decisions below follow from where the time is
spent and who waits, not from these numbers.

### D1. The worker decodes a moment once

The export's D7 and D9 stayed in the export. In the preview, one file at two decode sizes went out
as two spans; the second stood behind the cursor, and a read behind the cursor rebuilds the decoder.
The worker now builds one span across sizes and calls `read_video_span_qualities`, which also brings
the second decoder for a file read at two places, on the same worker thread (invariant 5: no new
owner).

A chunk is counted in moments: cut between two sizes of a moment, the decoder would be asked for
the same picture twice. The span's budget counts each key at its own size; the former count used
the first key's size, so a thumbnail-sized first key could carry a span larger than the cache. Keys
past the budget now stay queued: they were removed and dropped, and only the prefetch's next pass
brought them back, which a one-off request never gets (t1522 reached 2 of 13 keys on the old code).

### D2. The GPU preview's picture stays on the device

Every shown frame was composed on the device, waited for, copied to the CPU, premultiplied pixel by
pixel into a new allocation, and uploaded to the device again, all on the UI thread. `present` runs
the same resolve and the same passes as `execute_into` (invariant 7: one plan, one set of kernels),
then one display pass into an `Rgba8Unorm` texture that egui draws. `display.wgsl` reproduces
`Color32::from_rgba_unmultiplied` exactly: `round(value · alpha / 255)` in gamma space, so the
picture does not change (t1525, red without the premultiplication). An incomplete frame is not drawn
(t1526), which keeps "hold the last full frame". The ambient glow is the same pass at 32×18 with
the CPU loop's picks (t1527).

Readback stays where someone needs the bytes: export, tests, no engine, a refused plan.

`read_display` and `read_glow` exist for the tests. `kt-cli preview` calls `finish_present` once per
UI frame because a headless run has no swapchain to hold it back; the time is reported apart as
"device at the frame's end".

### D3. The first frame of a cut is asked early

After D1 and D2 all the remaining incomplete frames of 34–46 s stood at one cut (41.12–41.96):
52 composites waiting for a 4K file's first frame. The window is limited by bytes, about 0.7 s at
this size, and that decoder needed about 0.85 s to reach the frame, so the cut was late however fast
the rest was. A first frame costs one frame of cache and buys the whole lead-in. Past the bytes, up
to the same 2 s, `video_prefetch_wants` adds the layers that start a read (`jumps_to`), at most
`CUT_LEAD_KEYS`. The frames after them come in the window's turn (t1528).

A speed changes the stride of a read, so "the same read" is the same media and size within four
frames; a faster stride looks like a jump, and the key limit is what bounds it.

### D4. The CPU path composes ahead of the display

One thread cannot compose a frame in less than the frame's own cost: p95 381 ms at 1080p, on the UI
thread, 8–9 V-fps. `compose_ahead.rs` runs workers (half the cores, at most 16, bounded by the memory
of their scratches) over a shared renderer with `compose_export_with`, the export's own call: a
frame is a function of (cache, t, quality), so which worker made it does not show in its bytes
(t1530). The UI takes a finished frame by a swap.

- Workers read the cache and nothing else, and a frame whose layers are not all there is left for
  later, so they never decode and never compose incomplete (invariant 5).
- The key is (project version, content version, quality): a frame of another state of the project
  cannot reach the screen (t1532).
- They sleep on a condvar while the transport stands (invariant 12, t1531), and start on the first
  frame played on the CPU path, so a GPU session has no such threads.
- A stream stage keeps the synchronous path, as in the export (t1533).

Three things decided whether this worked, each found by measuring:

1. **A miss holds the picture, within a limit.** The first version composed a missed frame on the UI
   thread as well: a 500 ms stall that also threw the display past frames on their way, 30 % of them
   wasted. While the workers serve, a miss returns incomplete and the UI asks again next frame, the
   path a decode miss already takes. The hold is a promise, and the first version of it was not
   bounded: the workers serve forward play alone, so in reverse, at rate 0 or on a clock that stands
   nobody would bring the frame and the picture froze with the transport running. t947 caught it
   only as noise (2 failures of 4 runs of the suite). `may_hold` now answers yes for a frame in a
   worker's hands, or for three frames on one tick at a forward rate; past that the caller composes
   (t1536, red on an endless hold).
2. **Workers start past what cannot arrive in time.** A composite begun for a frame nearer than the
   composite's own smoothed cost ends after the display has left.
3. **The decode window starts at the workers' frontier.** Measured from the display, its 0.64 s left
   0.24 s past a 0.4 s composite, and the workers stood waiting for frames at 16 fps (`lead_ticks`,
   `shifted_head`).

### D5. The display orders no decode while the workers serve

With the window moved, 26–57 s came out with 38,026 frames of audio underrun where the old path had
none, all at 50–53 s: a 4K file with chromatic aberration and glow, its audio on the same worker.
Moving audio out from behind long spans did not cure it (34,698 and 5,573 after that change). The
evidence was the decoder: 3,165 pictures and 27 reopens against the GPU path's 2,230 and 17 on the
same range. On a miss the UI still submitted a Playback request for the frame on screen, which now
stands BEHIND the window, so the decoder seeked back through a 4K GOP for a frame about to be
skipped, on the worker that also reads that file's audio. The hold now comes before the requests:
0 underruns in three runs, 2,540 pictures, 20 reopens (t1534).

The first t1534 stayed green with the bug put back: the frame on screen was still in the cache, so
no request went out. It was rewritten without timing: the clock stands, and the frame asked lies past
the look-ahead, so only a Playback request can decode it.

Separately, a video span no longer holds audio of its own priority: the burst is read between
chunks and the span goes on, without moving the video cursor (t1529). This was not the cure for the
underruns above; it stands on its own, as a span of 64 keys could otherwise hold a burst for seconds.

### The hit rate counted a memo as a frame

A lookup of an empty post-FX memo counted as a video miss, though no frame was missing: on the CPU
path the rate read 41 % before this change and 57 % with the workers, with every decoded frame
there, and the panel's warning fires below 80 %. Only a frame now counts as a miss; a memo hit
still counts as a hit, which t-postfx and the mask tests read (t1535).

### The software adapter stays on the GPU path

The export refuses a software rasterizer in favour of CPU workers. The preview does not follow it:
in the real program at 1080p llvmpipe gives 19–20 V-fps and the CPU workers 18, both with no underrun.
The numbers do not justify a second rule, and `gpufx` keeps running the path real cards use.

### What a real window costs on a software rasterizer

Measured in the real release program under Xvfb, 1080p, llvmpipe: `queue.submit` takes about 30 ms
on both Vulkan and GL, against 0.5 ms headless. The first explanation, that the queue backs up behind
the previous frame, was tested and is WRONG: a headless run that never waits for the device stays
at 0.56 ms. What blocks is inside the software driver with a window attached; `perf` is not
available here, so the cause is not known and is not claimed. On this adapter the frame's cost is the
rasterizer's own work, whoever waits for it. A real card was not measured: none is available.

### Measured, before and after

Release builds, the showcase project (1080p, 25 fps, 104 s, 101 events), footage in /dev/shm, the
binaries alternating. **This machine has no real graphics card: every "GPU" row is llvmpipe on 32
cores, and no number here says what a real card does.** The machine carried other sessions' load
(5–20) throughout.

`kt-cli preview`, one frame's stages on the UI thread, ms per composite (34–46 s, 1080p):

| stage | before | D1 | D1–D3 |
|---|---|---|---|
| plan | 0.03 | 0.03 | 0.06 |
| upload | 2.0 | 3.5 | 3.5 |
| encode + submit | 0.2 | 0.25 | 0.3 |
| wait for the device | 11.5–13.2 | 18–23 | 0 |
| copy out | 3.5–4.1 | 4.5–5.0 | 0 |
| UI stall, p95 per UI frame | 43–57 | 40–55 | 7–11 |

At 640×360 the same frame was: plan 0.03, upload 1.4, encode 0.3, wait 5.5, copy 0.3 (the before
column, with 72 incomplete composites). "Before" composes fewer complete frames, so its per-composite
stages look cheaper; that is starvation, not speed. The device's work did not go away with D2: a
headless run waits for it at the end of the frame (15–19 ms), and in a window the swapchain does.

`kt-cli preview`, the whole edit, 0–104 s:

| path | V-fps | V-skipped | incomplete | underrun, frames | pictures | reopens | UI stall p95, ms |
|---|---|---|---|---|---|---|---|
| GPU 1080p, before | 11.3 | 374 | 2003 | 193,987 | 42,654 | 661 | 49.3 |
| GPU 1080p, after | 21.0 | 356 | 104 | 0 | 10,934 | 113 | 6.2 |
| CPU 1080p, before | 5.2 | 1588 | 766 | 160,426 | 53,479 | 842 | 227.5 |
| CPU 1080p, after | 15.0 | 0 | 2503 | 0 | 11,961 | 115 | 0.1 |
| GPU 640×360, before | 17.4 | 298 | 1119 | 109,205 | 55,296 | 852 | 20.1 |
| GPU 640×360, after | 22.0 | 282 | 70 | 0 | 10,814 | 113 | 2.7 |

On the CPU path "after", V-skipped and incomplete do not mean what they mean above: a held frame
counts as an incomplete composite per UI frame, and the ratchet moves past it. Read V-fps.

The real program, release, Xvfb, 1080p, the loop region 26–57 s, the panel's counters, 18 s each,
three rounds. A round counted only if the GPU "after" run reached 20 UI frames per second, a rule
set before the data; two earlier rounds with the machine swamped (under 6 UI fps) were discarded.

| path | V-fps | underrun, frames | UI frames/s |
|---|---|---|---|
| GPU, before | 8.7–10.3 | 75,983–82,767 | 21.7–26.8 |
| GPU, after | 17.0–19.7 | 0 | 24.3–29.7 |
| CPU, before | 4.4–4.7 | 63,695–79,695 | 14.4–15.7 |
| CPU, after | 17.6–18.4 | 0 | 48.4–52.4 |

"Before" here is the final tree with the old `worker.rs` and `prefetch.rs`, compose-ahead off and
`KT_PREVIEW_READBACK=1`: a temporary build, the base branch has neither the counters nor the size
variable. 25 V-fps is NOT reached in a window on this rasterizer: the composite and the UI's own
painting share the same CPU rasterizer (see the next section). Headless the GPU path reaches 24.7
on 34–46 s.

### Hardware decode: not needed

After D1 a picture costs about 2 ms out of the decoder plus 8–9 ms of RGBA conversion at about 2.7
Mpx, one worker per file, and every run's decoders kept up on 4K VP9 footage. The waste was the
decoding of pictures nobody asked for, not the speed of decoding. No architecture question arises.
The RGBA conversion is the larger half, on one thread per file: at 4K native size it is the next
limit to meet, but nothing measured here reaches it.

## Kept moments checked off the UI thread (ui-never-waits, archived 2026-11-08)

The bars' check of moments presented before an edit builds frame plans, and a plan with a stream stage
runs its kernel; `holds` takes the engine's lock, which the preview's thread holds for a whole
composite. Both were asked by `coverage_runs` on the UI thread. The check runs on the preview's
thread between frames (`PreviewParts::recheck_shown`, eight moments a pass, yielding to an ask); the
UI reads the map, and the executor is asked with the map free — t1843 caught the first version
calling `holds` under the map's lock. The display target and the glow are published in a slot of their
own at the end of `present`, so the UI's paint never takes the engine's lock (t1845).

## The pass timer's read buffers are made with it (gpu-pass-cost follow-up, 2026-11-08)

t2023 went red on the merged branch under a full `cargo nextest run --workspace` and green alone: "4
created in three warm presented frames". Traced: the three were the memo's kept composites, the fourth
the pass timer's read buffer. A frame's timestamps come back by a map the next frames collect; on a
loaded machine llvmpipe finishes later, more frames wait for their maps, and `Timer::end` made a buffer
whenever none was free. So the count of buffers followed the device's lag, a creation per frame exactly
when the machine was busiest. Now the timer makes its PENDING_MAX buffers (8 × 8 KiB) with itself and
`end` cannot create; a frame past them runs untimed, as `begin` already decided. The class, walked
across the sites `crate::created()` marks: the pool, the upload cache, the readback cache and the
memo are bounded by budget and recycle; the export ring has RING fixed slots; the arena and staging
grow only with the frame's shape; the compute host keeps its buffers by size. None grows with lag.
t2025 checks the timer with no poll at all; t2023 under 32 CPU burners was red 3 of 4 runs before.

## A build costs what the project holds, not its square (builder-cost-guard, 2026-11-08)

- **What hurt**: the video build shared the audio build's quadratic sites (transition ends, the reel revision per placement, the decode zoom probes walking curves from key 0).
- **Decision**: the same indexes and the same counting guard (t2062–t2067). On the owner's project the video build was never quadratic (18–20 ms); its cost is `edit-without-rebuild`'s.

The whole walk, the axes and the numbers: `openspec/changes/archive/2026-11-08-builder-cost-guard/design.md`.

## One wgpu instance per process (segv-crashes)

Opening wgpu devices at once on several threads, each with its own `wgpu::Instance`, races the Vulkan
loader's instance-level entry points: `kt-cli export` without a display died every run
(`export_engines` opens two to four devices on threads on a software adapter), t985 died 33 of 60
runs (a generator's first shader frame opened the compute host on a media worker beside the poster's
offline engine), and t986, t1312, t958 under load. The load watch's report header created and dropped
a whole instance — every ICD loaded, every driver's configuration parsed — each five seconds of a hot
spell. So `context::instance` keeps one leaked instance per backend set, `context::opening` lets one
device open at a time, and `adapter_facts` is computed once, in the editor before eframe opens its own
instance. Instances are never dropped: one destroyed at exit is one more native call racing the
threads on its devices. Not tuned to a loader version: the shape holds on any. Archived change
`segv-crashes`, design D3–D4.

