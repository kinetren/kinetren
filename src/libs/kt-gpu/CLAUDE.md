# kt-gpu

> The crate's contracts and mandatory tests — `openspec/specs/{video-plan,video-fx}/spec.md`; the reasons for the decisions — `design.md` next to it.

The GPU backend of video FX and of the composite (R.29): wgpu, fragment WGSL passes.
It implements `kt_engine::VideoGpuExec`; kt-engine does NOT depend on the crate —
the executor is injected from above (kt-app: the eframe device, kt-cli/export:
headless). It is the ONE executor of the frame plan (compute-kernels-render): with no device there
is no picture, and an export refuses.

## Invariants

- **wgpu is STRICTLY the same major version as the one inside eframe** (the workspace pin
  in src/Cargo.toml): the preview shares an `Arc<wgpu::Device>` with egui-wgpu — another
  major = incompatible types. Upgrading eframe = upgrading wgpu HERE TOO.
- **One `wgpu::Instance` per backend set per process, one device opening at a time** (segv-crashes):
  every headless device, the compute host and `adapter_facts` take `context::opening()` and ask
  `context::instance(&opening, backends)`. Never call `wgpu::Instance::new` elsewhere: devices opened
  at once on threads, each with its own instance, raced the Vulkan loader into SIGSEGV (kt-cli export
  without a display every run, t985 33 of 60). Instances are leaked on purpose. t2086, t2087.
  **Except a GL device**: GL adapters of one instance share its one EGL context, so a GL device opens
  from `own_instance` (dropped with it); on the shared one, the offline engine and the export's
  engines hit EGL_BAD_ACCESS in `GpuFxEngine::build` and every render from the editor died. t2088.
- **A wgpu error never ends the program** (gpu-errors-survive, kkt-26.11.15 N1): every opener calls
  `errors::watch(device, role)` (uncaptured-error handler + device-lost callback → the sink, counted
  per role); a NEW way to open or receive a device calls it too, or its first error is wgpu's default
  panic. A new composing entry point of `GpuFxEngine` runs through `Inner::guarded` (thread-local
  scopes → `GpuExecError::Validation`) and calls `yield_to_display` first; anything else that submits
  on the window's device off the UI thread calls `errors::yield_to_display()`: a refused
  `Surface::configure` (another thread submitted while it waited for idle) holds those for two UI
  frames. t2720–t2724.
- **On GL the window's device is the UI thread's alone** (gl-context-holder, found-defects 651): every
  wgpu-hal gles call holds the adapter's ONE context lock through the whole call (a link, a variant
  compile at a draw, a fence wait up to 2.1 s, a Mesa disk-cache read), and egui's paint panics after
  six seconds without it. `display_is_ui_only(info)` is true on GL; there the preview runs on
  `preview_engine_off_display()` (its own GL device, a `sibling` per project) and the bytes path. A NEW
  caller of the window's device off the UI thread is the defect this closed. Every wgpu call that can
  hold a device registers in `holds` (`hold` for one call, `within` for an entry point's span): a
  long one on the window's device is a journal line, and the panic report lists what is in flight.
  **On GL every call of ours into a device passes its `Compiled::gl_gate` first** (no timeout): the
  six-second panic hits any waiting thread, and a stalled call of ours killed the preview's own compose
  thread. A new entry point or background job on a device takes the gate before `inner`, never inside
  a job's own calls. An engine is made (`sibling`) and released (`Drop`, its `inner` a `ManuallyDrop`)
  inside its gate (t2748): never let a thread inside a device's gate drop the last reference of an engine
  of that device, or it waits for itself. The preview engine off the display keeps its bytes-path composites in the present
  memo (`keep_composites_for_bytes`): a covered loop replays there too. t2740–t2746.
- **On llvmpipe a loop costs more than its body** (render-speed-no-card): one with a bound the compiler
  sees, up to 32 iterations, is unrolled and its table folds; any other runs through execution masks
  with table reads lane by lane, ten times the cost. Write taps out, or specialize the text
  (`wgsl_taps`, `geom_specialized`), before reaching for a smaller kernel. Measure a pass with minima
  of repeats: a single reading on a loaded machine moves by 2×.
- **Fragment passes, NOT compute**: the default device of egui-wgpu on the
  GL backend is created with downlevel_webgl2 limits (compute is unavailable),
  and the live check runs llvmpipe via `WGPU_BACKEND=gl`. Everything here
  MUST work on llvmpipe. That is the GL device only: llvmpipe on VULKAN runs
  compute shaders (`compute.rs`, F-EXT-004), and so does any device
  created outside egui-wgpu's GL path.
- **The compute host (`compute.rs`) is a device of its own** (compute-kernels): Vulkan, DX12 or Metal
  whatever `WGPU_BACKEND` says, because the interface may be on GL. WARP is refused here too. Only
  Vulkan through llvmpipe has run it so far; DX12 and Metal are untested.
- **All textures are Rgba8Uint, but a word pass's** (`BarrierKernel::word_passes`: `R32Uint`, a
  32-bit word per texel in `.x`, render-speed-no-card D3) — textureLoad returns raw bytes without
  unorm losses; integer kernels (composite, noise, mirror, tiling) are
  byte-for-byte equal to the stored frames, bilinear ones — within tolerance.
  There are no samplers at all: the sub-texel filtering grid wanders between drivers —
  bilinear done by hand (4 texelLoad).
- **No atomics and no variable blending order**: the composite is a ping-pong of
  two accumulators by the shader (the formula `alpha_over` is not expressible
  by fixed-function blending). This is the condition of determinism on the device (t183).
- **Transitions (R.33; plugins since plugin-api-stage1, D12): a kind's `trans_<id>` WGSL lives
  in its family crate.** `assembled_composite` pastes every kind's function
  and its `if (mode == 64+i)` arm at `// @PLUGIN_TRANS_ARMS`; `trans()` itself keeps only the
  fallback mix. Helpers more than one kind reads (`samp`, `front_mask`, `hpanel`, `axis_*`, …) stay
  in composite.wgsl; a helper of one kind travels with it. The mode and 8 numbers arrive in
  u1.z/f0/f1 from `TransMix`; the clear bits (u1.w) mark the empty side of a solo fade. The kind
  (its ID, not its registry-dependent mode) and its parameters MUST enter `plan_key` (`put_mix`),
  otherwise ReadbackCache will return the frame of another transition. Test t224
  runs each mode over several phases — and its list of cases MUST contain both axes of
  direction and both states of the flags: on the defaults
  (`Dir::Left` = (−1,0)) an error in the Y term of the formula is not visible.
- **Shared parametrics** — from kt-fx: `pan_frame`/`cam_at`/`noise_seed`. The plan computes them;
  the shaders read the result.
- **A fused pass runs as written for its program shape where it can** (`geom_specialized`,
  render-speed-no-card D4): the text is built from `geom_fused.wgsl`'s sections, so an edit of an arm
  there is an edit of the shape's arm in `engine.rs` in the SAME COMMIT; t1766 holds the two equal.
- **The merged geo pass (R.44): `geom_fused.wgsl` executes the program of `kt_fx::geom`.**
  The program of primitives (Affine/Clamp/Fold/Proj) is built in kt-fx:
  the coefficients are multiplied in f64 by the plan, and the f32 values rounded ONCE arrive here
  in a 1024 B uniform (its own bind group layout; `FxOp::slot_units()` = 4
  arena slots — the sum MUST predict the consumption exactly, as
  passes() does). The number of supersampling taps of affine series is also computed
  by the plan and arrives in the uniform; Proj series measure the Jacobian per pixel by the SAME
  formula `ss_taps`. The tap table of the absorbed pointwise effects
  (R.45 item 312) — slots 46+2i of the same uniform, the counter in the h.w bits
  24..31; the `res_scale` of the K×-barriers (item 314) goes into the free slots of the uniforms
  vhs (u[6]) and noise (u[4]), Edges.d and the Blur steps/Glow divisor arrive
  already effective. `chain_ops` returns the maximum of the INTERMEDIATE
  sizes, and `resolve_chain` checks it against max_dim → a clean
  `TooLarge` (otherwise create_texture of the K×-buffers would poison the device
  with validation). The decode scale step (R.46) arrives as `LayerSrc.src_scale` in
  `chain_ops` and in `plan_key` (a frame of the same key at another size is another
  picture); WGSL does not touch it — the scale lives in the primitives of the program,
  which the plan builds (t309). For a NESTED reel (R.58, item 459) the scale step of the REEL
  (`NestSrc.nest_scale`) and the input of the instance chain (`src_scale`) are different
  numbers: when the step is higher than the input, a `FxOp::Resample` covering the whole frame is placed before `chain_ops`, tolerance
  `FrameTol::BILINEAR` (t632). `nest_scale` goes into `plan_key`; a chain that ended with a foreign
  size is no longer skipped silently
  (`complete = false`, t182). Edits — only as a pair with
  kt-fx/src/{geom,plan}.rs, in the same commit (t181 geo/tap/k2 cases,
  t309 s2 cases, FrameTol::GEOM).
- **`PresentMemo` keeps what the preview presented** (cached-range-plays): plan key → the integer composite on the device, complete frames only, 512 MB. `present` asks it FIRST and on a hit runs the display pass alone (`GpuStages.replayed`). preview-throughput D2 had moved the preview off the readback memo, and a covered range was composed again every lap. A new input to a frame must reach `plan_key`, or both memos show a stale picture.
- **The present memo is sized by the loop and the machine** (memory-ledger, t2292, t2293, t2295): its
  base is 512 MB; told of a loop (`VideoGpuExec::keep_loop`, the player's prefetch thread) it wants every
  moment of it at the size it presents plus an eighth, and its cap is the base plus the ledger's
  `Kept` grant. A loop it cannot hold keeps a part: a composite is not admitted over one shown within
  a lap of presents (`get` stamps only a hit, so the stamp counts presents). Past its base on a
  device that is not a CPU rasterizer, `memo_keep` creates the next frame's texture under an
  out-of-memory scope first; a refusal keeps nothing and bounds the growth at three quarters of what it
  held. Its account opens at the first keep or loop: an engine that never presents has none. A grant
  taken back (an export's claim) evicts at once through the hook set in `build`, which takes `inner`.
- **An export's device keeps the uploads its frames in flight read** (memory-ledger D7, t2298):
  `UploadCache::begin_frame` at every top-level `resolve_plan`; `new_for_export` drops what no frame of
  the last `RING` read. On every device an upload the frame being resolved reads is never evicted or
  reused as a spare within it (a reused spare is rewritten before the frame's passes read it).
  Upload, readback, transient and compute pools report to the ledger.
- **The kept composites are reached by key alone** (structured-telemetry): `present_kept(key)` shows one
  with no plan (the engine's probe plan found the key), `keep(key, bytes)` writes a composite another
  device made (the slow route) into the memo; stores and evictions go into the trace, and
  `GpuStages.stored`/`cold` say what a frame kept and how many pipelines it drew cold.
- **Every device a role opens describes itself to the trace** (`trace_device`: adapter, driver, features,
  limits): a new way to open a device calls it.
- **ReadbackCache is the readback path's memoization** (hash of the plan → frame), complete composites
  only, an LRU by generation (gpu-leftovers, rows 521/523): the post-FX cache
  of kt-cache (`PostFxVideo`) is not used by any executor (a per-layer readback would eat up the
  whole gain).
- Errors: `TooLarge` → no picture for this plan in the preview, an error in an export;
  `DeviceLost`/`Validation` → "poisoning" until the end of the session: no picture and a notice once
  (preview), or an export error.
- Pan3d (t187, ed. R.44): the world — project px × world_scale, X to the right, Y
  down, Z into the depth; rotations in degrees Rz·Rx·Ry ("ZXY", like the orientation
  in AE); a "50 mm" camera at the automatic distance (−(H/2)·cot(fov/2)) renders
  the layer at z=0 1:1. The semantics are frozen by the file format — they MUST NOT be changed.
  The EXECUTION since R.44 — the inverse homography by the Proj primitive of the merged
  geo pass (the vertex quad vs3d is removed); the mathematics lives in f64 in
  `pan3d_forward` (`fx-video-distort/src/pan3d.rs`), and the shader reads the result
  (t187 checks every run, t301 — an export carries it).

## Held frames (stream-front-on-device)

- **A frame can stay on the compute device** (`held.rs`, row 543): `front_on_compute` builds an engine on
  `compute::host_device()`'s device and marks it `hold_for_compute`; its `execute_held` is
  `execute_once`'s resolve and passes in the settled form, the result copied into a tightly packed
  buffer (a padded width packed row by row on the device), submitted and never waited for. A compute
  job takes it as `Init::Held`, copied on the device and ordered after the frame on the one queue; a
  buffer of another device is refused before anything is encoded (an `Err`, which the codec would turn
  into `compute::fail` — so only a `hold_for_compute` engine on THE compute device returns `Held`).
  Dropped buffers go back to the device's spares (a codec batch's bytes, under the ledger as compute).
  t2621, t2622.
- **A compute job's kept buffers are its own buffers** (stream-round-trips D1, t2708): `run_keeping` hands
  them out as `Held`; dropped, they return to the device's `Spares`, which `take` asks before the pool.
- **A job waits for ITS submission** (D9, row 661): reads are mapped first and one `PollType::Wait` names
  the job's submission index. `wait_indefinitely` waits for whatever another thread submitted after it
  (311 ms past the job's end at the median on the owner's iGPU).
- **Every wait names its own submission** (datamosh-history-wait D2, row 693): `crate::wait_for(device,
  sub)` — a read-back frame, a warm-up draw, a compute job, the calibration, the compute warm-up.
  `crate::wait_all` only where waiting for everything IS the job (`park`, `finish_present`); it is
  counted, and t2864 holds the frame paths at zero. A new wait for a device goes through `wait_for`.
- **A shape's readiness does NOT move the memo keys** (row 561, stream-front-on-device D6): tried
  process-wide and per device, both recomposed the loop the fill kept (t2334 red). Move them only on a
  difference the device itself measured.
- **A record never waits for a calibration and never uses a stale one** (row 562): the bracket lives in
  `best`, apart from the `calib` lock; a job that found another calibrating places no `gpu_start` on a
  calibration older than `CALIBRATION_AGE` (t2623).

## What a pass costs (gpu-pass-cost)

- **Every render pass of the engine and every compute dispatch is timed** where the device has
  `TIMESTAMP_QUERY` (`pass_time.rs`; every device we open asks for it, the eframe one through its
  `device_descriptor`). A new `begin_render_pass` passes `self.timer.pass(label)` as its
  `timestamp_writes`, or its time is lost from the journal's `GPU` record. A frame is `begin` … `end`
  (before `finish`) … `submitted` (after `submit`); `harvest` never waits.
- **A frame is counted by the device that composed it for the person** (gl-record-preview, t2747):
  `present` says `Preview`; a read-back frame says `Inner::readback_role` — `Preview` on an engine that
  keeps the preview's bytes (`keep_composites_for_bytes`, the GL preview off the display) or owns the
  window's surface, else `Offline`. A new caller of the window's engine's readback that is not the
  preview, or a new preview device, decides its role here, or the record lies about who asked.
- Labels: a fixed pipe by `Pipe::name`, a plugin's pass `id:name` (`BarrierKernel::pass_names`), a host
  node inside a barrier `id:node` by the barrier's index in `plugin_barriers()` (blur has no pipe of its
  own). `:` not `/` — the journal cuts a word with `/` to a file name.
- **Nothing is created per frame after warm-up** (t2023): a new per-frame `create_*` site calls
  `crate::created()` and must stay off the steady state. The preview memo's kept composite is the one
  allowed creation while it fills. Nothing is made "when none is free" either: a pool that grows only
  when the device lags is a per-frame creation on a loaded machine (the timer made a read buffer whenever
  the earlier maps had not come back; t2023 red under a full `nextest`, t2025). Make the bounded set up
  front and skip the work past it.
- **A pass writes its uniform through `stage` and binds through `bind_main`/`bind_geom`**
  (local-cost-queue, row 138; `binds.rs`): the bytes wait in `Staged` and go into the arena in one
  write right before a submission, so a NEW `queue.submit` of an encoder holding passes calls
  `self.staged.flush` first, or its passes read last frame's uniforms. Bind groups are kept a frame by
  (layout, views) and dropped at the next top-level frame when unused; a new arena clears them.
  `per_pass_binds(true)` is the old form (t2313), `kt-cli pass-cost --form binds` measures it.
- **Rank passes with the instrument, minima of repeats, interleaved A/B** (the machine is shared): llvmpipe
  compiles a pipeline variant at its first draw, so a run of two frames measures the compiler.
  `kt-cli pass-cost --fx … [--k K]` does it for a chain: each pass label's device time in the
  specialized form and under `cpu::with_general` (a barrier's graph before its fix), and the same-bytes
  check. Before replacing per-pixel math by a table, measure what it is of the pass: radial blur's 32
  `sin`/`cos` were 4 of 26 ms, and two table loads a tap cost more (local-cost-queue D2).

## A frame never compiles (pipeline-prewarm)

- **A software rasterizer compiles a pipeline at its first DRAW**, not at its creation: on an empty Mesa
  cache the first composite took 10.5 s and Camera ISP's first frame 4.6 s, while creating every pipeline
  took 1.9 s. Warming is a draw (`warm.rs`: one draw into a 6×6 target, zero uniforms), not a creation.
- **The pipelines are a `Compiled`** (`compiled.rs`): layouts, pipes, warm flags, geometry shapes, the
  disk cache and the warm worker (one thread, started by work, gone when the queue is empty — nothing at
  rest). `GpuFxEngine::prewarm_effects` returns at once and is reachable without `inner`. A new pipe goes
  into `Compiled` with a `Slot`, and its pass calls `self.c.drawn(slot)` before `set_pipeline`, or a cold
  draw goes uncounted (t2120).
- **A geometry shape never compiles in a frame**: `Compiled::shape` answers `None` until the worker has
  compiled AND warmed it, and the frame runs the interpreter (t1766 holds the bytes). Tests that need a
  shape call `settle()` first.
- **A warm draw never stands in front of a frame**: lavapipe runs (and compiles) every submission of a
  device on ONE queue thread. The worker submits one pipeline at a time after `wait_frames` (a new frame
  entry point takes `self.compiled.frame()`), and a watched software device warms a shadow device first
  (`open_shadow`: same adapter, its own queue; the driver's disk cache carries the code over). An export
  (`new_for_export`) has no shadow: it settles before its first frame.
- **`new_kept` only for the editor's display device**: `wgpu::Device` compares by its id inside its
  instance, so a device of another instance can equal one of ours; every other engine compiles its own.
- **Background GPU work holds the exit** (`exit.rs`, row 172, t2129): a thread of ours that compiles or
  draws off a frame takes `crate::exit::busy()` (refused once the process exits) and checks
  `exit::exiting()` between steps; an `atexit` hook waits for the guards, so libLLVM's destructors never
  run under a compile. A new background GPU thread takes the guard too.
- **The disk tier** (`pipeline_cache.rs`): every pipeline descriptor passes `cache`; the folder is set by
  the program (`set_cache_dir`), never by a test. Lavapipe's blob is its 96-byte header.

## Point-wise runs (fused-pointwise-runs)

- **A `PlanOp::PointRun` is ONE generated pass** (`runs.rs`): the stages' `tap_<id>` texts pasted
  into a text built per run SHAPE (the stages' indices in `plugin_taps()`), on the geometry pass's
  layout (1024-byte uniform, two vec4 a stage: 32 stages a piece, `MAX_RUN_STAGES`). Each stage is
  rounded and clamped as its own pass stores it, so fused and unfused are the same bytes on llvmpipe
  (t2190; on a card within 1 LSB a boundary); keep that rounding — posterize would move a whole level
  without it.
- **A frame's run form is the shape's state, and the keys name it** (gpu-leftovers D2, found-defects
  510): own passes while pending or tried, the verdict after; the trial's fused pass runs as a SHADOW
  beside the own passes (`RunPiece.shadow`), never as the frame's output. `compiled::forms_revision()`
  moves whenever a shape's output form can change and is mixed into `plan_key`; a composite whose resolve
  saw it move is not kept. A new way a run's form can change calls `forms_moved()`.
- **An export's frame runs the SETTLED form** (`Inner::settled`, set by `execute_once` and `submit`):
  every run fused, compiled in the frame when not warm (`Compiled::run_settled`), no trial. A new export
  entry point sets it, or the file depends on what the editor tried (t2516).
- **`FxOp::TapRun.pieces` is decided before the arena is sized** (`Inner::bind_runs`, in
  `resolve_chain`): `passes`/`slot_units` read the pieces (a shadow adds its pass), so the slot count
  stays exact. A shape is a `Compiled` run shape like a geometry shape: pending → its parts (the frame
  never compiles), warm → the trial (`runs::Trial`: the shadow and the own passes timed in one frame,
  the pass timer's `probe`, minima of five; slower fused → split in halves, each tried; no timestamps →
  fused). `fuse_runs(FuseRuns::{Always, Never})` forces
  a form for tests and `kt-cli fuse-cost`; `KT_FUSE_RUNS=never|always` sets an engine's mode at its
  creation, for an A/B of a project through `kt-cli preview`/`pause-scan`. t2190, t2192, t2193, t2197.
- A new point-wise plugin needs no code here: `fuses_in_runs` (kt-plugin-api) admits it, and its
  `tap_<id>` must read only its arguments (the registry refuses a texture or `P` read, t2194).

## The render-ahead producer's moments stay on the device (render-ahead D3)

- **`ahead(plan, keeper)` on a sibling of the keeper's engine** (the same `device_id`): `Inner::ahead_tex`
  is `present`'s resolve and passes without the display pass, admitted and cut on the ring by the caller's
  class (`Piece`, `Role::Preview`), submitted, never waited for; the composite texture moves into the
  keeper's present memo (`keep_tex`, the keeper's lock taken with ours released) and a done flag is set by
  the queue's work-done callback (`poll_done` drives it). Any other keeper — the bytes form
  (`kt_engine::ahead_by_bytes`: `execute_into`, then the keeper's `keep`). Nothing is kept when a run's
  form moved during the resolve. t2922 holds the three routes to one key and one composite.
- **A sibling of the window's engine inherits `surface` and `on_window`**: nobody block-waits on its
  device and its frames yield to a refused reconfigure, as the display's own do.

## The export's readback ring (export-throughput D5)

- `submit`/`collect`: the same resolve and passes as `execute`, the copy into the ring's staging
  buffer, the map requested; `collect` waits for THAT submission, `done(ticket)` asks without waiting
  (a `Poll` and the map's verdict). `readback_depth(w, h)` = `ring_depth`: `RING_BYTES` (256 MiB) of
  staging over the frame, 3 to `RING_MAX` (16) — a BOUND, not the depth used: the export hands on
  every finished frame and waits only on a full ring (export-readback D1). The owner's iGPU handed a
  1 ms frame back 52 ms after its submission; a ring of three made that a 57-frames-a-second export.
  A frame takes the first free slot below the depth; slots are made as first used and reported to
  the ledger as readback memory. The pass timer's `PENDING_MAX` is twice `RING_MAX`, or the frames
  past it run untimed. Uploads stay `KEEP_UPLOAD_FRAMES` (3): a deeper frame keeps its textures by
  reference. t2960. The
  arena and the pool's accumulators are reused by frames in flight — legal because
  `queue.write_buffer` and later passes are ordered after earlier submissions on the one queue;
  never write a buffer of a frame in flight by mapping it. The ring neither asks nor fills the
  readback memo (an export composes every moment once). t1457 holds the ring to `execute_into`.
- **An export frame is read back as the encoder's planes** (export-host-copy D3): `submit_yuv` runs
  `shaders/yuv.wgsl` (`Pipe::Yuv`, a fragment pass into a 4096-texel-wide word texture) after the
  composite, and the slot holds the packed planes; `ring_depth_bytes` counts their bytes. The pass is a
  mirror of `kt_engine::yuv::convert`, byte for byte (t3313): an edit of one is an edit of the other in
  the same commit, and a new encoder input format gets its layout in `kt_engine::yuv` and its arm in the
  pass. Integers only; the uniform carries the Y weights as u32 bits, the chroma weights as f32 values.
- `UploadCache` keeps up to four evicted textures per size for reuse.
- `export_engine()`: the export's device is HighPerformance (`new_headless_for_export`; the UI and the offline
  engine ask `display_power_preference`: a discrete card that alone drives the monitors, else
  LowPower, invariant 12, idle-cpu). A software rasterizer (llvmpipe/lavapipe) is a device like any other.
- `export_engines()` (compute-kernels-render, D4): llvmpipe runs one frame per queue, so a software
  adapter gets a device per frame worker, an eighth of the cores, 2 to 4, as many as the memory
  holds (`memory.rs`, gpu-render-hardening D1); a card gets one. `KT_VIDEO_ENGINES` overrides it.
  The file does not depend on their number (t1573). Each device adds 1.3 GB at 1080p.
- `offline_engine()`: a headless device out of the preview's queue; since scheduler-followups (row
  290) the preview's slow-chain route has it to itself (`serve_preview` registers its ring as one that
  fronts the person's frame: a software device's ring then bounds and cuts lower classes, t2337).
  Effect thumbnails, reel strips and posters (and the render-ahead producer under `KT_RENDER_AHEAD_DEVICE=side`) compose on `side_engine()`, its `sibling`:
  the same device, queue, ring and `Compiled` (nothing compiles, t2338), its own `Inner` and lock. Serials of frames are process-wide because it serves a fresh cache per job
  (t1574). A blended frame's upload key takes the pair and the weight (t250): it carries its base
  frame's serial.

## Tests

- **A cost is read with its frame** (t1939): the test kit's engine is one per test process, so
  `execute_into` then `last_cost` reads whatever composed last — a parallel test's frame under load.
  Measure through `testkit::compose_measured` / `chain_measured` (`GpuFxEngine::execute_measured`,
  one hold, no readback memo: a frame another test composed first would cost nothing).

- **The picture is held by stored frames** (compute-kernels-render, D5): `references/` keeps the last
  output of the deleted CPU composite, one file per test, read through `reference.rs` with the same
  `FrameTol`. The tests of other crates compose through `testkit.rs`: one engine per test process.
  Byte hashes of video (t11, t485, t946) hold on a software adapter only (`hash_holds`).
- `tests/parity.rs` — t181 (each effect and the chains, tolerances by the kind
  of arithmetic), t182 (the composite byte-for-byte), t183 (determinism on the
  device + a "dirty" pool of textures).
- `tests/gpu_render.rs` — t184 (GPU preview == GPU export, cold via
  a second engine and warm via ReadbackCache), t184b (GPU export is deterministic on the device).
- `tests/pan3d.rs` — t187 (identity, marker shift, transparent fields at
  scale<1, determinism).
- Without an adapter the tests are SKIPPED with a printed message; `KT_REQUIRE_GPU=1` turns the skip into a failure (CI with mesa/lavapipe, finish.sh on the user's machine).
- Tolerances — only `FrameTol`/`assert_frames_close`; ad-hoc epsilons
  are forbidden (invariant 11). Thresholds are set at ×2 from the measurement; a failure "at the edge" — an investigation and a record in the capability’s design.md, not a tweak.

## R.47 (items 332-335): 19 passes, Warp, barriers

- **A picked point-wise or geometry effect needs NO code here** (plugin-api-stage1): its WGSL
  (`tap_<id>`, `warp_<id>`) comes in its manifest and `assembled_shaders` pastes it — one dispatch
  number per plugin in registry order (`plugin_taps`/`plugin_warps`), never hashed into a key; the
  geometry steps without WGSL (`Proj`, `Mirror`, `Grid`) are the existing primitives.
- **A barrier needs NO code here either** (plugin-api-barriers):
  - its `BarrierKernel::shaders` are compiled once per engine over `common.wgsl`, after the
    point-wise plugins' own passes (`plugin_barriers`, `barrier_pipe_base`);
  - its `gpu` graph becomes one `FxOp::PluginBarrier`, whose `passes()` is its non-`Save` nodes —
    the arena prediction stays exact;
  - `chain_ops` applies its `Decl.alpha` to the fold;
  - an empty graph is a passthrough (drop_shadow on a frame the fold knows is opaque).
  The fixed pipes left are the host's own: resample, mirror, composite, layer_mix, unpremul,
  premul, grid_tile. A point-wise plugin with a barrier kernel (noise, greenscreen) gets both its
  tap and its graph.
- PIPE_COUNT grew by rounds (R.47 — 19, R.50 — +unpremul, R.53 — +premul) and shrank as the effects
  became plugins: the point-wise pipes left in plugin-api-stage1 4.1a/4.1b, every barrier pipe (noise,
  greenscreen, edges, vhs, glow_add, sharpen_add, pixelate, chroma_ab, radial_blur, shadow_under,
  halftone) in plugin-api-barriers. Seven remain, all the host's. A plugin's passes are counted from
  its graph (sharpen and drop_shadow: Resample down/up + the final pass with T1 — 3 slots).
- `geom_fused.wgsl`: primitive kind 4 — Warp (2 uniform slots, [mode,p0..p2]
  [p3..p6]); `mode` is the plugin's index in `plugin_warps()` (plugin-api-stage1, D11.2) and its
  arm is assembled at `// @PLUGIN_WARP_ARMS` in BOTH loops (`assembled_shaders`, t993); the
  formula is the plugin's `warp_<id>` WGSL. Its f64 `warp` stays as plan math (`probe_program_lens`
  reads it): edit both IN THE SAME COMMIT.
- **Premultiplication before the mip (R.53, item 418).** `premul.wgsl` and
  `unpremul.wgsl` do the conversions. Blur places a pair around two Resamples, Glow mips
  a premultiplied COPY and adds it as is; the decision is
  "is there alpha IN THE FRAME AT THIS POINT", i.e. the fold, and when the fold is silent —
  `src_has_alpha` (a parameter of `chain_ops`: without it a title's alpha would be judged by the
  fold alone).
  At `a == 255` both conversions are identities, so on opaque frames NOTHING is emitted and the goldens stay intact. Guard —
  the alpha cases of t181 (verified: without Premul on the GPU t181 turns red).
- **Edges SKIPS the alpha** (R.53, item 418): `a_out = max(a_in, line strength)`;
  it no longer touches `has_alpha` of the fold.
- DropShadow in chain_ops: when `has_alpha_known && !has_alpha` the op is NOT
  emitted (a passthrough); with unknown alpha it is emitted — the formula
  on an opaque input is identical in bytes (a_src=1 ⇒ out=src).
- The new kinds — t329 (a loop over the registry + tuned instances for
  identical defaults + warp-hard cases). FrameTol::WARP is NOT introduced:
  the hard warps keep GEOM (item 335); a failure of
  warp-hard — an investigation and the introduction of WARP at ×2 from the measurement, not a tweak of GEOM.

## R.51 (items 396-401): transitions

- The `P.m` uniforms of the composite are OCCUPIED: the ext scalars of volumetric transitions
  (the homographies of the panels from `kt_engine::transitions::trans_ext`, f64→f32 once). `ext` is not and MUST NOT be part of `put_mix`/`plan_key` — it is a deterministic function of what is already hashed.
- t224: 32 kinds; the volumetric ones (flip/cube/door/peel/roll) — a separate loop
  with `FrameTol::GEOM` (the panel edges and the perspective division). The mode numbers are a
  function of the registry (`MODE_PLUGIN + index`), they get neither into the project files (the
  tag travels) nor into cache keys (the id does).

## Signal grids and the strength mix (vhs-tape-simulation)

- A barrier's `Node::PassAt` renders at its own size: the VHS tape runs 14 passes on the standard's
  4 fsc grid (1135×576 PAL), its numbers carried as f32 bits or u16 halves in the Rgba8Uint texels,
  its per-frame parameters uploaded by three 16-word passes. `FrameTol::SIGNAL` is its class (an
  FM discriminator flips a click on one ULP at a carrier drowned in noise) — t1186.
- `FxOp::Snapshot`/`Mix` carry the alpha fold AT their points, and a mix sets the fold to
  dry ∨ wet. The end-of-chain flag for both sides was wrong as soon as
  a step destroying alpha stood between them (t1052).

## Windows / D3D12 (windows-build)
- FXC (the D3D12 shader compiler wgpu uses) rejects a write through a dynamic VECTOR index
  (`v[i] = …`) when it cannot unroll the loop: write vectors with `select` or whole-vector
  arithmetic. Arrays (`var a: array<…>`) with dynamic indices are fine. A shader the backend
  rejects is an INTERNAL wgpu error: `GpuFxEngine::new` pushes an internal scope next to the
  validation one, so opening the engine fails with an error instead of panicking.
- WARP ("Microsoft Basic Render Driver") is refused with `GpuInitError::Unsupported`: on the
  GPU-less Windows CI runner the passes ran for minutes and died inside it. A machine with only
  WARP has no picture; `KT_GPU_ALLOW_WARP=1` lets it through for experiments, and the GPU tests skip
  on it as they do without an adapter. `compile_check` builds every pipeline and runs no frame: t1650
  gets FXC, the compiler of every D3D12 machine, from WARP in CI.

## Shape masks (shape-masks)

- The GPU never rasterizes a mask: `chain_ops` asks the kt-fx rasterizer at the plan's chain positions
  (`mask_stops` join the plan's stops) and uploads the coverage (`coverage_frame`, the
  byte in `.r`, keyed by its hash with the top bit set) into `FxOp::MaskAlpha` (`mask.wgsl`,
  PIPE_COUNT 8) or `FxOp::MaskMix` (the coverage branch of `layer_mix.wgsl`, T2). A mask adds no
  divergence: masks alone agree EXACT (t1274).

## One ring per physical queue (gpu-arbiter)

- **Every submission but the display's present is admitted by its ring** (`arbiter.rs`): the devices of
  one hardware card share one ring whatever their backend; a software rasterizer's device is a ring of
  its own (its own queue thread). The waiting submission of the highest `kt_time::sched::Class` goes
  first; the class is the caller thread's (`sched::current_class`: compose now, prefetch near, export
  offline, a lane's job its lane's class, untagged background). A new thread that submits GPU or
  compute work sets its class.
- **A ring with a display bounds what a lower class leaves in front of the display's NEXT present**
  (gpu-throughput D1, render-ahead D5): every flight carries its predicted ns and submit instant, the ring
  drains them as a FIFO, and a lower class is admitted while the drain plus its claim ends within the bound
  past the later of the display's next present (a period after its last while it presents, else now) and
  the end of the display's own frame in flight. The bound is ONE frame period (`arbiter::bound`; half a
  period until render-ahead — the owner raised it, kkt-26.11.17 S4, it was never his "latency limit").
  Every lower class has the same room; there is no count of lower pieces (`AHEAD_PIECES` and the rooms by
  class, `room`, are gone). The display's present is COUNTED (`Ring::note_display` in
  `present`/`present_kept`), never admitted. A waiter wakes when the prediction makes room (it polls the
  flights' devices, never blocks on a surface device). A piece is cut at HALF the bound and no admission claims more (`piece_limit`, ahead-kept D2): a piece
  of the whole bound fitted only an empty ring, so the device idled between pieces (found-defects 732, t2930). A
  lower-class frame or compute job is cut at pass/dispatch boundaries by the pass timer's last MEASURED
  time of each label (`Timer::predict`, `ComputeDevice::last`), else by the disk seeds (`seeds.rs`,
  below); an unmeasured pass ends its piece. A new pass site calls `self.cut_point(enc, label)` before
  writing its uniform, or it is never cut. A compute ring is bounded too and cuts while a higher class
  waits. Same passes, same order, same bytes (t2253). t2430, t2433, t2923.
- **Every timed frame is a `GpuFrame` trace record** when its timestamps are harvested (render-ahead D7):
  `trace loop` reads the device's busy share per lap and while frames were missing from them (t2926).
- **The compute host admits EVERY submission** (gpu-leftovers D1, row 511): job pieces, the clock
  calibration (`try_lock`: a job never waits behind another's calibration admission) and the warm-up
  dispatch. A job writes one uniform arena and binds by dynamic offset, one bind group per (shader, slot
  size, buffers) within the job (row 520, t2511). A new compute submission goes through the ring.
- **A piece's completion frees the ring by a work-done callback**; a waiter completes the oldest piece by
  a blocking wait on THAT piece's device, never on a device with a surface (the window's present takes
  the device's lock for writing). So no piece waits for its own submitter to come back (the export's
  ring of three would deadlock). It blocks only when every flight is on ONE device and only a completion
  makes room; with flights on several devices (they finish out of each other's order) or a predicted
  fit it polls them all and sleeps on the ring's condvar at most `WAIT_TICK` (2 ms) (datamosh-history-wait
  D3, row 662, t2865). **Never a `PollType::Wait` with a timeout**: in wgpu 30 one that times out while
  another thread's poll retires the submission panics in `maintain` ("If the queue is empty…").
- **A cut piece does not wait for itself** (stream-history-on-device, t2356): `Piece::cut` submits and
  admits the next piece by its first pass's prediction against the time in flight; only a full bound or
  a higher class waits. A cut display-device piece is completed by non-blocking polls in `admit`.
- **`trace gpu` reads a job's pieces by the job's span on its thread** (t2863): every `gpu_start` carries
  its own instant, so a query keyed by the instant reads every piece as a job's last.
- **A compute job records when it ran on the device on the trace's clock** (`GpuStart`, D7 of
  stream-history-on-device, t2357): a calibration (an empty submission's timestamp bracketed by the CPU
  clock, the narrowest of three, again after 10 s) made only while the trace records. `STAMPS` is 2048: a
  batched Datamosh job carries hundreds of dispatches, and an untimed one is never predicted.
- **`wait_presented` never polls on a fixed period**: the work-done callback, a check after the
  measured device time, then waits growing from a quarter of it (`present_checks`, t2260), never past
  the frame's own time or 4 ms (`present_waits`, t2866: an uncapped doubling saw a late frame late).
- **Pass times outlive the process** (`seeds.rs`, gpu-throughput D2): `<shader-cache>/pass-times-v1.tsv`,
  (adapter name + driver, label, work units) → ns; a label never measured in the process is predicted
  from it (scaled by work from the nearest size). Notes only when a time is new or moved by a quarter;
  written at most every 30 s and in the exit hook; no cache dir (tests) — memory only. A run shape's
  label is never seeded (process-local number). A cache: it moves cuts, never bytes.
- **A compute job learns from its own pieces** (D2): at a cut the piece's timestamps are resolved into
  their own aligned part of the job's resolve buffer and mapped; the next `due` reads what came back
  and predicts every later dispatch of that name. **A job holds a timestamp set of its own** (a pool of
  `STAMP_SETS` made up front; none free — untimed, never a creation), and writes a `gpu_start` per
  PIECE with that piece's submit; every piece writes `gpu_exec` (98) naming its executor (D6).
- `KT_GPU_ARBITER=off` (or `arbiter::set_enabled(false)`) admits everything and cuts nothing: an A/B of
  a run; `kt-cli gpu-share` runs both on ONE device (t2254), `kt-cli trace gpu` reads the records.

## Effect graphs on the device (fx-graph-dag D3)

- `chain_ops` takes the layer's wiring (`LayerSrc.fx_in`) and runs `kt_fx::plan::lower`: `FxOp::Hold`
  keeps a texture out of the pool to the end of the chain, `Load` makes it current, `Merge` is TWO
  composite passes over a transparent accumulator (the Background Normal at full level, the
  Foreground by its blend mode at `mix`) and `Unpremul` — the composite's own kernels, which is what
  makes the Merge law hold byte for byte (t3204). A matte: `matte.wgsl` makes the coverage
  (alpha/luma, inverted, × mix) and `layer_mix.wgsl`'s coverage branch mixes the merge with the
  Background (t3202).
- **Mask points split around a graph step**: the masks that complete the slot before the position
  close first (a Hold keeps the masked frame), the next slot's snapshot opens after a Load.
- `plan_hash` (and the engine's `durable_hash`) hash the wiring when present; a line adds nothing.
