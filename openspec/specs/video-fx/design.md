# Video effects — why it is so

## What hurt and for whom

- There were 11 kinds, and the owner directly asked for "not enough effects for video,
  10-20 more are needed, both 2D and 3D" (course towards After Effects); at the same time —
  "too few control parameters in all effects".
- Every geo step resampled again: a 2×2 grid → a ×2 zoom turned
  a checkerboard into a flat grey (contrast 0.00). A barrier in the middle of a series finished
  off the rest. The owner: old behaviour is an artefact of implementation, not a style;
  there will be no switches.
- The chain traversal lived in two copies — in the CPU executor and in the GPU
  op builder; a divergence of segmentation was caught only by eye.
- "3D only on GPU": the CPU reference of determinism had a hole, export without a GPU
  refused.
- The effect picker was a text list. The owner: "some kind of icon would not hurt, and a pre-rendered thumbnail would not hurt either… only trying it after a second
  (what if it was a misclick), and if the computer cannot cope — slow down the pace".
- A forgotten arm of the classification of a new kind silently made it a barrier and broke
  geo series; the wgsl dispatcher of primitives ended with a trailing `else`, and the fifth
  primitive would have run as Proj.

## Decisions and why

- **One chain planner in kt-fx.** Segmentation, legacy/fused routing, absorption of point-wise ones, K barriers, snapshots of the layer's strength —
  one function; both backends execute it. The invariant "the composition code is one"
  is fixed at the chain level: divergence is impossible by construction.
- **Geo series — concatenation of inverse maps.** Primitives Affine / Clamp /
  Fold / Proj / Warp; a series of any length — one pass with supersampling over the
  Jacobian. A series of length 1 without minification goes through the old kernel, so
  the goldens of the first ten were never regenerated.
- **K×-barriers with conjugation.** The barrier is executed at K× only where
  the detail is really squeezed (squeeze-cap): K = quantum of min(tail zoom, compression).
  The physical scale is contributed by two edge affines; the step arithmetic stays
  1× — otherwise the integer grid of EventGrid would lay out differently.
  `MAX_INTER_DIM = 4096` is a project constant: export must not depend on the machine.
- **Pan3d — an f64 homography on CPU.** The quad is planar, z_clip ≡ 0, so this is
  a 3×3 matrix, executable backwards by an ordinary loop; the CPU is again the full
  reference, export without a GPU is legal.
- **The formula of the point-wise kind lives in one f64 text** (tap functions), and
  all four executors call it: CPU barrier, CPU tap, GPU pass,
  GPU tap. Classification of the kind is mandatory and locked by build locks.
- **One Warp primitive with modes** instead of five kinds: the road (four copies of
  the primitive loop, uniforms, PerPixel supersampling) is paid once.
  Normalisation is in the affines: the primitive works in the space of magnitudes ~1, and
  this keeps f32 parity without a WARP tolerance class.
- **Thumbnail = "fit + apply"** through the single entry with `in_scale = 1` at the reference thumbnail size 320×180. Video FX are
  pure functions without state, so a picture in a cell costs a fraction of
  a millisecond and must coincide with the result of a click.
- **Built-in variants are instances of the same kind**, set by name through
  `set_param`: the click goes the old way, no new commands or timeline entities
  appear; a new parameter touches none of the 77 places.
- **The thumbnail service is the shape of a service** (an endless queue of small requests),
  without a decoder, DB or persistence: the base frame is given by the caller from ready thumbs.
- **The strength axis blends two curated points** (variants), not a "main parameter":
  the useful range is already set by the extreme variants.
- **Alpha rule — three classes of step**, Preserves stays silent: otherwise geometry
  erased the neighbours' declarations and the title lay down as an opaque rectangle.

## Rejected and why

- **A node graph of grading** (F-CLR-006). "The chain is an ordered list" is simultaneously
  the key of the post-FX cache, the shape of both byte-matching
  executors, `Item.effects` of the OTIO format and the only strip widget.
  A graph cancels this in four places at once and gives birth to a new class
  of CPU↔GPU divergences without tests and references. The value of a graph — a matte as
  a signal — is taken off by the "key source" field of an effect, without a DAG.
- **Switches for merge/K** (UI or env) — a direct answer of the owner:
  the old behaviour is an artefact, not a style.
- **A "main parameter" for each kind** (the FCP axis): VHS and Color Grade have four
  equal ones, Mirror has no numeric ones, Corner Pin has eight coordinates.
- **Persisting thumbnails into the index DB and a background job "render after
  adding the source"**: the expensive half (decoding) already lies as a JPEG in the thumbs,
  overlaying the chain is cheaper than reading sqlite. The owner agreed.
- **"A clean frame + one effect" in the cell**: after a click it would come out
  not what is shown. The strip counts the event's chain plus the candidate.
- **Only category glyphs / only a picture** for the icon: for 27 kinds no suitable
  glyphs can be found (there were cases of tofu); took a glyph plus a thumbnail.
- **The frame of a finite background task for thumbnails**: it is about a task with
  progress, while thumbnails are a queue that lives while the picker is open.
- **A directional (linear) blur, a grid Warper, the shapes/tracking "Censor",
  sepia/invert as separate kinds, light/shading of a cylinder** — they overlap with
  radial-zoom, the Duotone presets, or belong to masks/tracking.
- **A separate SSE tier of SIMD**: the f64 kernels need 4 lanes, and a bit-exact second
  implementation would have to be kept forever; the levels are only Scalar/AVX2.
- **`Mutex` for the texture cache**: only the UI thread touches the textures; `RefCell`
  in the service, because the FX/morpher windows hold `&self.model` in the ui closure.

## Non-obvious details

- `plan_chain` (`src/libs/kt-fx/src/plan.rs`) — the only place of
  decisions; `chain_head_zoom` there gives the net zoom of the prefix up to the first
  stop, and kt-engine uses it to decide the decode scale step of the clip. The byte
  fast-path — only when `S == 1 && all K == 1`.
- A single `Mirror` goes to Legacy ONLY when `k_in == k_out`
  (`src/libs/kt-fx/src/geom.rs`, `build_geom_exec`): a permutation of
  pixels is correct at any uniform scale, but not at a change of it; otherwise the
  chain [grid, vhs, mirror, blur, zoom] tripped the plan's assertion.
- If you edit `src/libs/kt-fx/src/geom.rs` or `plan.rs` — you edit
  `src/libs/kt-gpu/src/shaders/geom_fused.wgsl` in the same commit; the shared
  parametrics (`pan_frame`/`cam_at`/`noise_seed`) in `video.rs` are pub precisely
  for the mirrors. The arm of a new primitive — into all four copies of the loop
  (map_pos, map_pos_rec on CPU; geo_map, geo_sample in WGSL) in one commit.
- Supersampling (`ss_taps`, `src/libs/kt-fx/src/geom.rs`): switched on strictly at len > 1 + 1/64, quantum 1/32 px, ceiling 4×4. For affine
  programs the Jacobian is computed once in f64 and travels into the uniform; only
  Proj/Warp measure per pixel. A dead straight probe behind the silhouette
  is replaced by the inverse one — otherwise the edge of the identity map lost
  byte-exactness.
- Tap ops (`TapOp { after_prim, kind }`, `src/libs/kt-fx/src/geom.rs`)
  are applied to the value of the subtap after bilinear, before the box average; the grain of
  Noise and the coordinate of Vignette are taken from ITS OWN stage (`stage_w/h`):
  the vignette before the zoom darkens the stage, the zoom cuts the corners — this is the contract, not
  a defect. Only INTERNAL point-wise ops are absorbed; leading/trailing ones are
  barriers, so that the bytes of K=1 chains stand.
- `res_scale` in the barrier kernels (`src/libs/kt-fx/src/video.rs`,
  `apply_barrier`) is an internal parameter, not user-facing and not in the
  format; K multiplies the derived quantities after the clamp. VHS takes the line
  context from the logical line y/K; Noise — the cell size·K (the pattern is bitwise
  equal to 1×); Halftone — the whole pattern in logical coordinates.
- Sharpen (`src/libs/kt-fx/src/video.rs`): `radius ≥ 2` is the minimum of the spec,
  because step 1 gives a base equal to the source, and a zero effect.
- Pixelate — an integer copy of the centre of the cell C = size·K, parity EXACT;
  DropShadow — offset in integer px ×K, an input without alpha is passthrough on both
  paths, and when the GPU alpha is unknown the formula degenerates to identity at 255
  (the early cut-off is an optimisation, not a different result);
  RadialBlur — exactly 16 taps on both backends (`src/libs/kt-fx/src/video.rs`).
- Pan3d (`pan3d_forward`, `src/libs/kt-fx/src/geom.rs`): the semantics of the world are frozen by the format; t.z ≤ 0 and uv outside [0,1]² — transparency, |det| ≈ 0 — a
  transparent layer. CornerPin builds the homography in closed form and puts it into the
  existing Proj; its arm does NOT inherit `let Some(c3) = cam3d else
  continue` — the camera is not needed. Kaleidoscope — a triangular wave without
  int-modulo: floor is identical in f64 and f32.
- The key of CPU memoisation of post-FX (`postfx_key_hash`,
  `src/libs/kt-engine/src/video.rs`) contains the resolved camera: it is chosen per frame, and the static hash of the chain does not see it.
- `round()` in WGSL rounds halves to even, in Rust — away from zero: the parameter is
  quantised once (×1024) and then computed in integers on both paths
  (`src/libs/kt-fx/src/video.rs`, VHS). A kernel that edits the frame
  in place must write ALL channels — VHS did not touch green outside the scan line, and the mirrors diverged.
- SIMD (`src/libs/kt-fx/src/video_simd.rs`): forbidden are FMA,
  reassociation, rcp/rsqrt, f32 instead of f64, `_mm256_round_pd(NEAREST)`
  instead of emulating half-away; `/255` — the magic `(x*0x8080_8081)>>39` (the common
  formula is wrong at 255³). The usefulness threshold is 1.3×, below it — scalar.
- Golden loop `GOLDEN_ALL` (`src/libs/kt-fx/tests/video_fx.rs`, t328):
  a non-geo non-point-wise kind is run through `apply_barrier` — a forgotten arm
  is caught by a debug panic; `drop_shadow` on an opaque test frame is an
  identity hash, and that is the passthrough contract.
- `render_fx_thumb` (`src/libs/kt-fx/src/video.rs`): `in_scale` is always 1 —
  the step means "the frame is larger than the logic"; for a thumbnail it is the reverse; to substitute a step is to ask the plan for a non-existent detail reserve.
  `test_pattern` there is a reference, not a decoration: an edit changes the icons of all
  kinds at once.
- For an f32 parameter the even ceiling of the spec after `set_param`
  (`src/libs/kt-model/src/project.rs`) turns out to be OUTSIDE the spec: the clamp is in f64,
  the write into f32 rounds up (0.05 → 0.05000000074). The "Glitch" variant
  is pushed back by 0.045; t543 (`src/apps/kt-app/src/fx_registry.rs`) is deliberately strict.
- The cell recipe (`recipe_hash`, `src/apps/kt-app/src/fx_thumbs.rs`) is xxh3 over the `Debug` of the chain, not serde: half of the fields are under
  `skip_serializing_if`, and a new field with a default would drop out of the key.
  The key remembers `ThumbKey`, which does not depend on the generator's parameters —
  therefore `invalidate_media` sits next to `retune_media` of the thumbnail service.
- The picker (`fx_picker_with_head`, `src/apps/kt-app/src/fx_registry.rs`):
  `CloseOnClickOutside` is mandatory; the guard `ui.is_rect_visible` before
  a cell — otherwise one opening ordered 160 thumbnails, and an invisible cell
  caught a click into the timeline. A composite accessible name is concatenated inside
  `widget_info` — zero allocations without accessibility.
- The moment of the strip (`src/apps/kt-app/src/fx_registry.rs`,
  `src/apps/kt-app/src/thumbs.rs::material_at`): it is frozen only during
  playback; `set_busy(playing || export)` makes the worker sleep 25 ms between
  renders; the redraw gate is on `pending()`, not on "the picker is open",
  otherwise an open menu keeps 60 fps while idle.
- The strength axis (`axis_at`, `src/apps/kt-app/src/fx_registry.rs`): counted from the
  centre of the cell by absolute position, not by accumulating `drag_delta`
  (it drifts on skipped frames); on the release frame `dragged()` is already
  false — `drag_stopped()` is read. The quantisation is not a decoration: each
  position is its own recipe and its own render. `FxInstance::lerp`
  (`src/libs/kt-model/src/project.rs`) switches the non-numeric exactly at the
  midpoint.
- The hint of the axis has no arrow "→": the glyph U+2192 is absent from every font of the
  program, substituting a "similar symbol" is forbidden (`src/apps/kt-app/src/font_tests.rs`, t100).

## Deferred

- AVX2 of the fused geo pass and SIMD of the new kernels — by measurement and the 1.3× rule
  (CPU is the reference and fallback, the product computes on GPU).
- The tolerance class `FrameTol::WARP` — introduced ×2 from the measurement, if the warp-hard
  case t329 fails on some backend; tuning GEOM is forbidden.
- Coring in Sharpen — on complaint; a directional blur — as the RadialBlur mode.
- The "key source" field of an effect (a shared mask for two effects, Outside)
  — the only value of the graph that can be taken without a DAG.
- The "flapping" of K on the frame where the 1 + 1/64 threshold is crossed during zoom automation
  — the size of the jump is limited by the tolerance t303, there is no separate decision.

## D3D12 and WARP (windows-build, 2026-10-26)

- **FXC** (wgpu's D3D12 shader compiler on Windows) rejects a write through a dynamic vector
  index (`v[i] = …`, "not natively addressable") when it cannot unroll the loop; greenscreen,
  noise, composite (`blend_over`) and sharpen_add were rewritten with `select`/vector arithmetic,
  same per-channel math (parity stays byte-exact). Arrays with dynamic indices are fine.
- **That failure is an INTERNAL wgpu error**: with only a validation scope it reached the default
  handler and panicked — a crash instead of the CPU fallback (invariant 7). `GpuFxEngine::new`
  pushes an internal scope too.
- **WARP is not a GPU for the engine** (`GpuInitError::Unsupported`): on the GPU-less
  windows-latest runner the passes ran for minutes and died inside it; a machine with only WARP
  renders effects on the CPU. `KT_GPU_ALLOW_WARP=1` lets it through for experiments.
- The GPU path on real Windows hardware is still a manual check (no GPU on the CI runner).
