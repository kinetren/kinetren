# Design

## Context

See proposal.md — Why. State at `plugin-api-stage1` (kt26.10.31):

- **The CPU barrier position:** `kt_fx::video::apply_barrier` matches twelve `FxInstance`
  variants. Its kernels live in `kt-fx/src/video.rs`, and their AVX2 twins in `video_simd.rs`:
  `noise_avx2`, `glow_add_avx2`, `luma_avx2`, `sobel_row_avx2`, `vhs_row_avx2` and the shared
  `bilinear_into_avx2`, which the geometry resample also uses. The twins are gated by
  `KT_SIMD`/runtime detection and are byte-equal to the scalar code (t198).
- **The scratch pool and the K× rule:** buffers come from `FxScratch`. The planner (`plan_chain`)
  decides K for every barrier (`MAX_INTER_DIM`, the decode-scale seed), and each kernel takes
  `res_scale`.
- **The GPU side:** `kt-gpu/src/engine.rs` has one `FxOp` and one or more `Pipe`s per kind.
  - Glow, Sharpen and DropShadow are THREE passes: a mip down, a mip up and a final add/under.
  - Glow on alpha also wraps the mip in `Premul`/`Unpremul` (R.53 item 418).
  - The other kinds are one pass each.
  - Uniforms are precomputed on the CPU (`noise_seed`, `cos`/`sin` of the halftone angle).
- **Point-wise plugins already assemble** their own pass from the registry (`FxOp::PluginTap`,
  D5). A geometry plugin is a primitive program (D11.2). Barriers have no contract.
- **Noise and greenscreen hold both positions:** inside a fused geometry series they are TAPS of
  `geom_fused.wgsl`/`geom.rs` (`tap_noise`, the f64 continuum); at the barrier position they are
  integer kernels with K× and an AVX2 twin (the D11.1 finding).

## Goals / Non-Goals

**Goals:**
- Every barrier kind is a plugin with the same bytes on the CPU, the same tolerance class on the
  GPU and the same SIMD parity.
- A barrier plugin unknown to the host runs on both executors without host code naming it.
- `FxInstance` holds only the non-effects (Geometry, CamMotion, LayerMix) plus `Plugin`/`Missing`.

**Non-Goals:**
- No external hosts (wasm, dlopen/OFX).
- No new barrier kinds; the catalogue waves come later (`effects-transitions-catchup`).
- No change of a kind's look, parameters or file text.
- No K× rule per plugin: K stays the host's decision.

## Decisions

### D1. A barrier is a manifest class with three kernels
`Decl.class = Barrier` and `VideoKernel::barrier() -> Option<&dyn BarrierKernel>`:

- `cook(p) -> Cooked`: the parameters once per plan (clamps, f64 precompute), as `cook_at` for
  taps.
- `frame(&Cooked, Frame, &BarrierCtx, &mut dyn Scratch) -> Frame` is the CPU text, the byte
  reference. `Frame` is the host's frame type, moved into the API so a kernel hands the frame
  back without a copy (rgba, w, h, has_alpha). `BarrierCtx` = `res_scale` K, `burst_index`,
  `simd` and the raster host (D3).
- **The SIMD twin is a branch inside `frame`, taken only when `cx.simd`.** The HOST decides
  `simd` once per frame from runtime detection and `KT_SIMD`, so `KT_SIMD=0` stays ONE switch
  (R.32) and no plugin reads the environment. t1053 runs every barrier with `simd` true and false
  and compares the bytes.
- `Decl.alpha: AlphaRel` (Preserves / Produces / Destroys) is the alpha rule's per-step relation,
  declared rather than matched.

*Revised in stage 2 (2026-10-31):* the first draft had a separate `frame_simd(...) -> bool`. It
was a second entry with the same arguments; the twin needs nothing but the host's permission, so
`cx.simd` carries it. *Alternative rejected:* letting `frame` read `KT_SIMD` and detect features
itself — every plugin would carry the switch, and the scalar reference would stop being ONE switch
away.

### D2. The GPU side is a declared pass graph of host nodes and plugin passes
`BarrierKernel::gpu(&Cooked, &GpuCtx) -> PassGraph` returns a short list of nodes:

- `Resample { to: Size }`: the host's bilinear, the mirror of the CPU resample.
  `Size::{Same, Div(r), Abs(w,h)}`.
- `Premul` and `Unpremul`: the host's conversions (R.53).
- `Pass { wgsl: &'static str, entry, inputs: [Slot; ≤3], uniforms: [f32; 16] }`: a fragment pass
  over `common.wgsl`. `Slot::{Src, Prev, Saved(n)}`, and `Save(n)` keeps the previous output.

The host checks the graph at registration (D5), compiles each distinct `wgsl` text ONCE per
executor, and spends `nodes.len()` uniform slots. That replaces the hand-written `passes()`, so
the slot arena stays exactly predicted (the `debug_assert` in `pass`). The uniforms are computed
in f64 and rounded to f32 once on the CPU, the same rule as for warp precompute and transition
`pack`.

Glow, for example, becomes `Save(0), [Premul], Resample{Div r}, Resample{Same}, [Unpremul],
Pass{glow_add, inputs [Saved 0, Prev]}`. That is the same three passes plus the premul pair it
has today.

*Alternative rejected:* a plugin issues encoder commands itself. That would put wgpu types in the
API and break "a GPU kernel joins the one composition path" (plugins requirement): readback
caching, the arena and device-loss poisoning are the host's.

### D3. Shared raster code stays ONE text, reached through the context
`BarrierCtx.raster: &dyn RasterHost` offers:

- `resample` — kt-fx's `bilinear_into`, with its AVX2 twin;
- `mip_down`/`mip_up` — the glow/sharpen/drop_shadow mip;
- `luma` — the integer luma, with its AVX2 twin;
- `noise_hash`/`noise_seed`.

The host implements it in kt-fx, so the Pan/geometry resample and a barrier's mip call the same
function. A plugin crate still depends only on the API.

*Alternative rejected:* moving these helpers into `kt-plugin-api`. The AVX2 intrinsics and the
geometry resample would then live in the API crate, and kt-fx's core resample would depend on a
plugin-facing module for its hot path. `pixfont` could move (a pure table); the resample cannot
without splitting its one text.

### D4. Dual-position kinds declare both kernels; the planner chooses as today
Noise and greenscreen declare a positioned tap (D11.1) AND a barrier kernel. `plan_chain` puts a
kind inside a fused series as a tap exactly where it does today, and at a series edge as a
barrier. The choice is a function of the chain, not of the kind, so t26/t302 do not move. A
plugin with only a tap is never a barrier, and a plugin with only a barrier kernel always breaks
the series.

### D5. Registration refuses an inconsistent barrier (t1054)
`Registry::register` refuses:

- a Barrier class without `frame`;
- a built-in barrier without `gpu` (built-ins keep the GPU default);
- a graph that reads `Saved(n)` before `Save(n)`, or whose output is not at the frame size (a
  graph of host nodes alone is legal: blur on the GPU is a mip down and up);
- a `Size` that can exceed `MAX_INTER_DIM` for the largest K;
- (a kernel marked `threads: Single` is refused for every plugin already, D4 of stage 1, so a SIMD
  path needs no refusal of its own).

These checks follow the D4 refusals of stage 1: the refusal comes before any frame.

### D6. Families and order
`fx-video-blur` takes blur, glow, sharpen, radial_blur and drop_shadow. `fx-video-stylize` takes
edges, vhs, noise, pixelate, halftone and chromatic_aberration. Greenscreen joins `fx-video-color`
(keying is a colour operation; vignette is already there). The picker keeps `VIDEO_ORDER`
(t1018); each plugin id equals its former tag, so file text is unchanged (t973). The serde
defaults (`default_twelve_u32`, `default_chroma_amount`…) become passport defaults, and
`skip_serializing_if` becomes the passport's "written only when not default" (the stage-1 rule
that kept the centre of radial_blur and the spin off the file).

### D7. Order of work: freeze, contract, one pilot, the rest, delete
1. Freeze first. t1051 is a pre-move CPU golden of all twelve kinds: the default and each built-in
   variant, at K=1 and K=2, with and without alpha. It is green on the variants before any code
   moves.
2. The contract and the pilot: **pixelate**. It is one pass, has no SIMD twin and reads no
   neighbours, so it proves the pass graph's simplest form.
3. Then the mip kinds: glow (premul, 3 passes, a twin), sharpen, drop_shadow.
4. Then the rest. Noise, greenscreen and vhs come last: they carry the burst index and the dual
   position.
5. Each step ends green and committed. The `FxOp`/`Pipe` arms and WGSL files of a moved kind are
   deleted in the same step.

## Risks / Trade-offs

- **A twin that diverges by one byte.** t198 runs over every registered barrier with a twin (not a
  list), and KT_SIMD=0 goldens stay in `finish.sh`.
- **Arena prediction.** The slot count becomes `nodes.len()` from the graph. A wrong count shows as
  the existing `debug_assert` in `pass`, and t1052 runs every graph in a debug build.
- **Dynamic dispatch on the barrier path.** The cost is one virtual call per frame per slot, not
  per pixel. The kernel's own loop is static inside the plugin crate.
- **Rebuild.** Moving the raster helpers behind a trait keeps kt-fx a core crate. An edit of a
  barrier kernel rebuilds only its family (t977). An edit of `RasterHost`'s implementation still
  rebuilds kt-fx and above, which is the correct cost: it is core code.

## Open Questions

None that change what gets built. The family split (D6) can be revised by the owner without
touching the contract.
