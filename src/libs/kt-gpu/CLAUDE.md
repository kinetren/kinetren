# kt-gpu

> The crate's contracts and mandatory tests — `openspec/specs/{video-plan,video-fx}/spec.md`; the reasons for the decisions — `design.md` next to it.

The GPU backend of video FX and of the composite (R.29): wgpu, fragment WGSL passes.
It implements `kt_engine::VideoGpuExec`; kt-engine does NOT depend on the crate —
the executor is injected from above (kt-app: the eframe device, kt-cli/export:
headless). The CPU path of kt-fx/kt-engine remains the byte reference and the fallback.

## Invariants

- **wgpu is STRICTLY the same major version as the one inside eframe** (the workspace pin
  in src/Cargo.toml): the preview shares an `Arc<wgpu::Device>` with egui-wgpu — another
  major = incompatible types. Upgrading eframe = upgrading wgpu HERE TOO.
- **Fragment passes, NOT compute**: the default device of egui-wgpu on the
  GL backend is created with downlevel_webgl2 limits (compute is unavailable),
  and the live check runs llvmpipe via `WGPU_BACKEND=gl`. Everything here
  MUST work on llvmpipe.
- **All textures are Rgba8Uint** — textureLoad returns raw bytes without
  unorm losses; integer kernels (composite, noise, mirror, tiling) are
  byte-for-byte equal to the CPU, bilinear ones — within tolerance (f32 against f64).
  There are no samplers at all: the sub-texel filtering grid wanders between drivers —
  bilinear done by hand (4 texelLoad, a mirror of `bilinear_into`).
- **No atomics and no variable blending order**: the composite is a ping-pong of
  two accumulators by the shader (the CPU formula `alpha_over` is not expressible
  by fixed-function blending). This is the condition of determinism on the device (t183).
- **Transitions (R.33; plugins since plugin-api-stage1, D12): a kind's `trans_<id>` WGSL is a
  mirror of its kernel in its family crate.** `assembled_composite` pastes every kind's function
  and its `if (mode == 64+i)` arm at `// @PLUGIN_TRANS_ARMS`; `trans()` itself keeps only the
  fallback mix. Helpers more than one kind reads (`samp`, `front_mask`, `hpanel`, `axis_*`, …) stay
  in composite.wgsl; a helper of one kind travels with it. The mode and 8 numbers arrive in
  u1.z/f0/f1 from `TransMix`; the clear bits (u1.w) mark the empty side of a solo fade. The kind
  (its ID, not its registry-dependent mode) and its parameters MUST enter `plan_key` (`put_mix`),
  otherwise ReadbackCache will return the frame of another transition. Test t224
  runs each mode over several phases — and its list of cases MUST contain both axes of
  direction and both states of the flags: on the defaults
  (`Dir::Left` = (−1,0)) an error in the Y term of the formula is not visible.
- **Shared parametrics with the CPU** — from kt-fx: `pan_frame`/`cam_at`/`noise_seed`.
  The formula may be changed only as a pair with the CPU implementation.
- **The merged geo pass (R.44): `geom_fused.wgsl` is a mirror of `kt_fx::geom`.**
  The program of primitives (Affine/Clamp/Fold/Proj) is built in kt-fx:
  the coefficients are multiplied in f64 on the CPU, and the f32 values rounded ONCE arrive here
  in a 1024 B uniform (its own bind group layout; `FxOp::slot_units()` = 4
  arena slots — the sum MUST predict the consumption exactly, as
  passes() does). The number of supersampling taps of affine series is also computed
  on the CPU and arrives in the uniform; Proj series measure the Jacobian per pixel by the SAME
  formula `ss_taps` (otherwise at the threshold the backends would choose different tap grids,
  and no FrameTol would cover that). The tap table of the absorbed pointwise effects
  (R.45 item 312) — slots 46+2i of the same uniform, the counter in the h.w bits
  24..31; the `res_scale` of the K×-barriers (item 314) goes into the free slots of the uniforms
  vhs (u[6]) and noise (u[4]), Edges.d and the Blur steps/Glow divisor arrive
  already effective. `chain_ops` returns the maximum of the INTERMEDIATE
  sizes, and `resolve_chain` checks it against max_dim → a clean
  `TooLarge` (otherwise create_texture of the K×-buffers would poison the device
  with validation). The decode scale step (R.46) arrives as `LayerSrc.src_scale` in
  `chain_ops` and in `plan_key` (a frame of the same key at another size is another
  picture); WGSL does not touch it — the scale lives in the primitives of the program,
  which the CPU builds (t309). For a NESTED reel (R.58, item 459) the scale step of the REEL
  (`NestSrc.nest_scale`) and the input of the instance chain (`src_scale`) are different
  numbers: when the step is higher than the input, a `FxOp::Resample` covering the whole frame is placed before `chain_ops` — a mirror of the CPU `fit_into`, tolerance
  `FrameTol::BILINEAR` (t632). `nest_scale` goes into `plan_key`; a chain that ended with a foreign
  size is no longer skipped silently
  (`complete = false`, t182). Edits — only as a pair with
  kt-fx/src/{geom,plan}.rs, in the same commit (t181 geo/tap/k2 cases,
  t309 s2 cases, FrameTol::GEOM).
- **ReadbackCache is its own memoization** (hash of the plan → frame): the post-FX cache of
  kt-cache (`PostFxVideo`) is a mechanism ONLY of the CPU path; the GPU does not touch it
  (a per-layer readback would eat up the whole gain).
- Errors: `TooLarge` → the caller falls back to the CPU only for this plan;
  `DeviceLost`/`Validation` → "poisoning" until the end of the session + a notice
  (preview) or an export error (a file does not mix backends — invariant 7).
- Pan3d (t187, ed. R.44): the world — project px × world_scale, X to the right, Y
  down, Z into the depth; rotations in degrees Rz·Rx·Ry ("ZXY", like the orientation
  in AE); a "50 mm" camera at the automatic distance (−(H/2)·cot(fov/2)) renders
  the layer at z=0 1:1. The semantics are frozen by the file format — they MUST NOT be changed.
  The EXECUTION since R.44 — the inverse homography by the Proj primitive of the merged
  geo pass (the vertex quad vs3d is removed); the mathematics lives in f64 in
  `kt_fx::geom::pan3d_forward`, there is no GPU-only path any more — the CPU computes the same
  (t187 checks every run, t301 — export without GPU).

## Tests

- `tests/parity.rs` — t181 (each effect and the chains, tolerances by the kind
  of arithmetic), t182 (the composite byte-for-byte), t183 (determinism on the
  device + a "dirty" pool of textures).
- `tests/gpu_render.rs` — t184 (GPU preview == GPU export, cold via
  a second engine and warm via ReadbackCache; GPU export is deterministic and
  within tolerance of the CPU).
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
  formula is the plugin's `warp_<id>` WGSL, a mirror of its f64 `warp`, edited IN THE SAME COMMIT.
- **Premultiplication before the mip (R.53, item 418).** `premul.wgsl` is a
  mirror of `kt_fx::video::premultiply`, `unpremul.wgsl` is a mirror of
  `kt_fx::video::unpremultiply` (the home of both functions is in kt-fx, from where
  kt-engine takes them too). Blur places a pair around two Resamples, Glow mips
  a premultiplied COPY and adds it as is; the decision is
  "is there alpha IN THE FRAME AT THIS POINT", i.e. the fold, and when the fold is silent —
  `src_has_alpha` (a new parameter of `chain_ops`: without it the GPU would
  judge by the fold, and the CPU by `Frame.has_alpha`, and the titles would diverge).
  At `a == 255` both conversions are identities, so on opaque frames NOTHING is emitted and the goldens stay intact. Guard —
  the alpha cases of t181 (verified: without Premul on the GPU t181 turns red).
- **Edges SKIPS the alpha** (R.53, item 418): `a_out = max(a_in, line strength)` in both executors;
  it no longer touches `has_alpha` of the fold.
- DropShadow in chain_ops: when `has_alpha_known && !has_alpha` the op is NOT
  emitted (CPU passthrough); with unknown alpha it is emitted — the formula
  on an opaque input is identical in bytes (a_src=1 ⇒ out=src).
- Conformance of the new kinds — t329 (a loop over the registry + tuned instances for
  identical defaults + warp-hard cases). FrameTol::WARP is NOT introduced:
  the hard warps keep GEOM on both backends (item 335); a failure of
  warp-hard — an investigation and the introduction of WARP at ×2 from the measurement, not a tweak of GEOM.

## R.51 (items 396-401): transitions

- The `P.m` uniforms of the composite are OCCUPIED: the ext scalars of volumetric transitions
  (the homographies of the panels from `kt_engine::transitions::trans_ext`, f64→f32 on the
  CPU — both backends read the same bits). `ext` is not and MUST NOT be part of `put_mix`/`plan_key` — it is a deterministic function of what is already hashed.
- t224: 32 kinds; the volumetric ones (flip/cube/door/peel/roll) — a separate loop
  with `FrameTol::GEOM` (the panel edges and the perspective division). The mode numbers are a
  function of the registry (`MODE_PLUGIN + index`), they get neither into the project files (the
  tag travels) nor into cache keys (the id does).

## Windows / D3D12 (windows-build)
- FXC (the D3D12 shader compiler wgpu uses) rejects a write through a dynamic VECTOR index
  (`v[i] = …`) when it cannot unroll the loop: write vectors with `select` or whole-vector
  arithmetic. Arrays (`var a: array<…>`) with dynamic indices are fine. A shader the backend
  rejects is an INTERNAL wgpu error: `GpuFxEngine::new` pushes an internal scope next to the
  validation one, so the engine falls back to the CPU instead of panicking.
- WARP ("Microsoft Basic Render Driver") is refused with `GpuInitError::Unsupported`: on the
  GPU-less Windows CI runner the passes ran for minutes and died inside it. The CPU path is the
  full fallback; `KT_GPU_ALLOW_WARP=1` lets it through for experiments, and the GPU tests skip
  on it as they do without an adapter.

## Shape masks (shape-masks)

- The GPU never rasterizes a mask: `chain_ops` asks the kt-fx rasterizer at the same chain positions
  as the CPU (`mask_stops` join the plan's stops) and uploads the coverage (`coverage_frame`, the
  byte in `.r`, keyed by its hash with the top bit set) into `FxOp::MaskAlpha` (`mask.wgsl`,
  PIPE_COUNT 8) or `FxOp::MaskMix` (the coverage branch of `layer_mix.wgsl`, T2). A mask adds no
  divergence: masks alone agree EXACT (t1274).
