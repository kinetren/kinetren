# kt-fx

> The crate's contracts and mandatory tests — `openspec/specs/{audio-fx,video-fx,video-plan}/spec.md`; the reasons for the decisions — `design.md` next to it.

Audio FX: slowed (rubato varispeed) + reverb (freeverb) + gain. Without FFmpeg/audio hardware.

Video FX (`video.rs` + `geom.rs`): the v1 effects, RGBA8, CPU. Since R.29 these are the
**byte reference and the fallback** — the product computes video on the GPU (the kt-gpu crate,
WGSL mirrors of these kernels, conformance t181). If you edit a formula here, edit the
SHADER in kt-gpu in the same commit; the common parametrics
(`pan_frame`/`cam_at`/`noise_seed`) are `pub` for exactly this reason. Noise is a
counter-based hash (R.29, parallelisable: CPU and GPU byte for byte).

**The chain plan (R.45, `plan.rs`, items 311–314)**: `plan_chain` is
the SINGLE place of chain decisions (segmentation of windows, legacy/fused,
absorption of point-wise effects, K× barriers); the CPU (video.rs) and the GPU (chain_ops)
EXECUTE the ready plan. The internal Greenscreen/Noise are absorbed into runs by
tap ops (item 312, applied to the value of the sub-tap; the grain is nailed to the grid of
its own step); the barriers Blur/Glow/Edges/VHS are executed at K× when
"compression above + zoom below" (item 314: K = ss_taps(min(zoom of the tail, compression)),
the ceiling MAX_INTER_DIM=4096 — a project constant). The barrier kernels accept
an internal `res_scale` (pixel geometry ×K; at K=1 — literally the former
code); geo runs are rendered into the K×-target by CONJUGATION (two edge
affines; the arithmetic of the steps remains 1×). If you edit plan.rs/geom.rs, edit
`geom_fused.wgsl` in THE SAME COMMIT (t181).

**Decode scale step (R.46, item 317)**: `plan_chain` accepts `in_scale` —
the frame arrived at S× the logical size (decoding is larger than Quality). Phase A
is built on the logic of `in/in_scale`, phase C seeds the compression accumulator with
the step (the squeeze-cap existed precisely for this), and the leading barriers — up to
the first geo run — are executed at S× FORCIBLY (the frame under them is physically large; the benefit formula does not vote there); the byte fast-path
remains only when `S==1 && all K==1`. How much step a chain needs is
computed by `chain_head_zoom` (the accumulator of phase B at position 0 = the net zoom
of the prefix up to the first stop) — kt-engine uses it, deciding the clip's
step. A lone `Mirror` goes to `Legacy` ONLY when `k_in == k_out`: a pixel permutation is correct at any uniform scale, but not on
a change of scale (t308b — before R.46 this was a latent debug crash).

**Merging of geometry series (R.44, `geom.rs`, items 307–309)**: consecutive
Pan/CamMotion/Mirror/EventGrid/Pan3d — ONE pass of the inverse mappings;
`geom.rs` is the single place of decisions (segmentation `geo_run_end`,
`build_geom_exec`, the supersampling taps `ss_taps`); both backends MUST ask it, and not decide
for themselves. If you edit `geom.rs`, edit
`kt-gpu/src/shaders/geom_fused.wgsl` IN THE SAME COMMIT (t181, the geo cases).
The byte rules: a series of length 1 Pan/CamMotion/Mirror WITHOUT minification —
the old kernels byte for byte (t297); a lone EventGrid — in a merged pass,
but the tile-local phase reproduces the old kernel literally (the golden t26
stands without regeneration — at ×2 box-2 supersampling is arithmetically
equal to the old bilinear at the half phase). **Pan3d from R.44 is computed HERE**
(the inverse f64 homography `pan3d_forward`, the semantics of the world are frozen
by the format, identity at the defaults byte for byte — t298); the camera arrives
through `VideoFxCtx.cam3d`, without it Pan3d is a passthrough. Audio FX inside
a geo series are transparent; barriers (Blur/Glow/Edges/VHS/Noise/Greenscreen) and
the snapshot points of LayerMix break the series. The merged pass is SCALAR f64
(AVX2 is separate work under the 1.3× rule); `bilinear_into` + AVX2
remain behind the barriers (blur/glow/fit_into) and the legacy series.

## Effect thumbnails (R.54, item 428)

- **`render_fx_thumb` is the single entry for thumbnails** (the kind's icon, the variant cell,
  a library preset): "fit `fit_into` + `apply_video_chain_pooled`"
  and not a line of one's own arithmetic. Introducing a second path into the UI is not allowed —
  the picture in the picker MUST coincide with what comes out after a click (invariant No. 7 in spirit: one code, the size is a parameter of execution). Guard —
  t544e.
- **`in_scale` of a thumbnail is always 1.** The decode scale step (R.46) says "the frame arrived LARGER than the logical size", while for a thumbnail it is the reverse; confusing these two things means asking the chain plan for a non-existent margin of detail.
- **"A preview of the kind, not of the strength".** Some parameters are set in PIXELS
  (`Pixelate.size` up to 128, `Halftone.size`, the radii of blurs,
  `Vhs.rgb_shift`), therefore one chain at 96×54 and at 1080p shows a
  different STRENGTH. It must be computed at a REFERENCE size (for us — the thumbnail's size,
  320×180) and the result shrunk by `fit_into`, rather than computed straight at the cell's size.
- **`test_pattern` is frozen by the golden t544** — it is the reference by which the icons of all kinds
  look at once, and not a decoration. Its properties are chosen for the classes of effects and are checked by t544b: asymmetry along both axes
  (`Mirror`), a green plate (`Greenscreen`), bands and a ramp (colour), a fine grid and hard edges (`Blur`/`Sharpen`/`Edges`/`Pixelate`/
  `Halftone`). There are two variants — an opaque one and one with TRANSPARENT margins: on an opaque frame `DropShadow` is identical by construction (the shadow falls from under the alpha), and the icon of shadow effects needs an alpha variant (t544d).
- **Identical templates.** For `Pan`/`Pan 3D`/`Color Grade`/`Corner Pin`/
  `Drop Shadow` the defaults are identity (one identity hash in `GOLDEN_ALL`),
  therefore the icon of such a kind is the FIRST variant from the kt-app registry, and
  not the template. t544c holds both halves of the rule: the identity remained
  an identity, and the variant with zoom shows something.
- The grain of `noise`/`vhs` is nailed to `burst_index`: with a motionless cursor
  the cell MUST be one and the same picture, otherwise the strip flickers on
  every frame of the interface (t544f).

## SIMD (R.32) — read before editing any kernel in video.rs

- **A SIMD kernel MUST be BYTE-FOR-BYTE equal to the scalar one on any CPU.**
  The CPU path is the determinism reference (invariant 11); the golden files t26/t11/t67 are not regenerated; a divergence is a render regression, not "a different implementation". The scalar code is NOT refactored together with the kernel: it is the very definition of the bytes (`*_scalar` in video.rs).
- **Forbidden**: FMA (`fmadd` rounds differently), reassociation of sums/taps, rcp/rsqrt approximations, f32 instead of f64, changing MXCSR, and `_mm256_round_pd(NEAREST)` instead of the emulation of half-away (it is half-to-EVEN: 2.5 → 2, whereas `f64::round` gives 3). Lanes are only independent pixels/channels; the order of operations INSIDE a pixel is exactly the scalar one.
- Integer divisions: `/255` is the magic `(x*0x8080_8081)>>39` (the common `(t+(t>>8))>>8` is wrong at 255³!), `/32767` with sign — see noise. Any new division/rounding trick is closed by an EXHAUSTIVE micro-test.
- A new kernel — only as a pair with the scalar one plus a case in t198 (`tests/simd_parity.rs`), necessarily with NON-MULTIPLE sizes (tails) and extreme values. Usefulness threshold — 1.3×: below it we keep the scalar code (so it was decided for greenscreen/mirror; figures in porting-notes item 198).
- If you change an effect formula, you change the scalar code, the AVX2 kernel AND the WGSL shader of kt-gpu in one commit (t181 guards the third pair).
- All `unsafe` code is only in `video_simd.rs`; each kernel is called only after `simd::simd_level() == Avx2`. `KT_SIMD=0` — forced scalar, `KT_REQUIRE_SIMD=1` — forbids a vacuous skip in tests.

## API
- `ClipProcessor` — a stateful processor of one clip: warp (rubato `SincFixedOut`) + freeverb + the static gain of the chain. It is read strictly sequentially (`read(local, out, pull)`). There are TWO discontinuities, and they must not be confused (item 166): after `reset()` (a real seek — the player performs it) — `hard_seek` with pre-roll `max(tail, 100 ms)`; a position break during CONTINUOUS output (the loop region wrapping around, a clip shifted with the mouse during playback) — `seam`: the source is repositioned, the reverb and sinc live on. The pre-roll on every lap of the loop warmed the processor and grew into underruns — now it is not there.
- `PullKind` in the signature of `pull` (item 167): `Warmup` — a call from the pre-roll (frames go to waste), `Output` — to the output. A Warmup miss is not an underrun; the counter of the caller is split by this attribute.
- `chain_fingerprint(fx)` — the fingerprint of the STRUCTURE of the chain (kinds of FX, not values): if it matches, the state is reused (kt-engine::FxStateStore), and the parameters arrive smoothly via `apply_params`.
- `chain_tail(fx)` / `chain_rate(fx)` — static properties of the chain without creating a state (the graph extends the clip window by the tail before building the processor).

## Automation domain — EVENT TIME (R.37)

> **The warp map has moved to kt-model (R.39)** — `libs/kt-model/src/warp.rs`.
> It must be called by `clip_slice` (the clip-slice rule), and kt-model cannot depend on kt-fx. Only a re-export remains here: `kt_fx::warp_material_ticks` and the other names work as before; the formula still has one home.
> Everything below about it is true — read it in `warp.rs`.

- **The curve of any parameter is a function of the OUTPUT progress `tau`**, not of the consumed material. The keyframe is still stored in MATERIAL ticks (trimming/Split/reverse work as in R.33), but the axis is linear: the speed does not compress it. The user's requirement verbatim: "t — the time interval cut by the current event, WITHOUT accounting for slowing-speeding up".
- Why: the domain by material made the axis of the `rate` curve depend ON the `rate` curve itself, made an early speed keyframe shift the firing moment of the late keyframes of OTHER parameters, and left the right part of the graph unreachable (one could only get as far as `duration·avg_rate`).
- Map: `S(tau) = ∫₀^tau rate` — piecewise QUADRATIC (trapezoid), `T(s) = S⁻¹` — the root of the quadratic equation in the citardauq form (`2·ds/(r0+√(r0²+2·b·ds))`): stable and does NOT require a branch for `b ≈ 0`. Neither `ln` nor `expm1` — the map became bit-portable between platforms.
- **`source_progress` NO LONGER participates in sampling parameters** — only where the CONSUMPTION of the source is concerned: `chain_source_window_ext` (prefetch), `source_offset` (seam/hard_seek), `chain_avg_rate`. In `produce` the target of a chunk is taken directly by `tau`, and the midpoint rule became EXACT: the integrand is linear, and the midpoint of the trapezoid is its exact mean.
- `ease` on the `rate` curve is still forcibly off: the integral of smoothstep would be of the fourth degree. R.37 made it computable in principle, but there is no gain.
- **The value of a `rate` keyframe is clamped at the input of the profile** (`from_curves`): `segments` clamps the ends BEFORE interpolation, and `eff_rate` — AFTER, and on a keyframe outside `RATE_CLAMP` they diverged. Such a keyframe cannot be set from the UI, but `migrate_legacy_slowed` copies the legacy number as is, and a divergence of the map from the DSP is a prefetch miss and silence (item 13/t17).
- **Signed speed (R.57, item 454): the MODULE goes into the resampler.** Rubato consumes a virtual stream at speed |rate| and does not know the direction; the sign lives in the ADDRESS (`chain_addr_knots` → runs of constant direction, inside a run the address moves ±1 per virtual frame). The positions of the runs accumulate as INTEGERS — otherwise at a joint there would be a skipped or doubled frame, that is, a click exactly at the turning point.
- **Hold is not "slow" but silence without consumption.** |rate| below `RATE_AUDIO_FLOOR` (0.01): the cursor stands still, the output is zero, the nodes keep working (the reverb rings out on zeros). The decision is made PER CHUNK by the same midpoint rule as the other automation targets; it cannot be made per frame — the resampler cursor would not survive it.
- **`has_warp` looks at the MODULE.** A constant −1 of stretch does not give one, so it stays on the FLAT path — and "minus one" turns out to be an EXACT mirror (t616 checks the bytes), not an approximation.
- **A rubato refusal in a change of ratio is COUNTED** (`Warp::rejected`). Previously it was swallowed by `let _ =`, and a curve wider than the window ±32 around the anchor silently stuck to the previous ratio. A curve going below 0.25 builds the resampler in extended mode: anchor ≤ 2×, the window computed by the span.
- `pull(src_frame, buf)` — the source is fed by the caller (kt-engine); the media crate itself does not read.

## Alpha rule (R.53, item 417) — read before editing ANY video step

Each step of the chain relates to alpha in exactly one of three ways, and this
must be named explicitly:

- **Produces** — the step may CREATE transparency where there was none:
  `Pan3d` (outside the quad), `Greenscreen` (by key). It declares
  `has_alpha_known = true, has_alpha = true`.
- **Destroys** — the step GUARANTEES an opaque output (fills the alpha).
  It declares `known = true, alpha = false`. Today this is `Edges`.
- **Preserves** — the alpha of the input passes through. It stays silent: the fold
  does not touch it. ALL geometry belongs here, including `EventGrid`.

The fold is "the last one to declare wins" AMONG Produces and Destroys.
This is exactly why Preserves MUST stay silent: `EventGrid` declared itself
Destroys (a tile is a resampled input, the alpha of the input lives in it;
only the field OUTSIDE the grid is filled with black), and by this it erased the Produces of a
neighbour higher up the chain — a title or the output of a greenscreen landed in the composite as an
opaque rectangle. `known = false` means "nobody has spoken" — the alpha of the SOURCE is taken.
Tests: t519 (the fold), t520 (pixels), t521 (the composite, independently of kt-fx).

## Invariants (regressions for the hiccups of the old engine, defects a–f)
- Input/output only f32 48kHz/2ch interleaved; FX do not see the sample rate of the source (a).
- The warp consumes the source strictly sequentially through the internal cursor of rubato — no skips, no duplicates, no "stitching of seams" (a, c, d).
- `apply_params` NEVER resets the buffers: wet — one-pole ~20 ms (`SmoothedParam`); the buffers are zeroed only in `reset()` (e). Exception: a change of reverb_delay > 0.5 ms — a structural change of the geometry of the lines.
- `tail()` is honest (t60 over the maximal comb line); the graph relies on it (f).
- Send, not Sync — one thread owns the state (b).

## Tests (kt-engine/tests/fx.rs)
- 4: FFT purity of the warp (440→220 Hz, spurs < −60 dB outside ±16 bins) + consumption of the source exactly rate·duration (without accumulated drift).
- 5: a step of mix → no click (the jump ≤ 1.5× the natural slope of the sine).
- 6: the tail rings out beyond the clip boundary and decays −60 dB within tail(); rebuilding the graph in the middle of sounding gives a BYTE-IDENTICAL output (the state survived the rebuild).

## Subtleties
- The skirt threshold in test 4: the Hann window smears a non-bin-centred tone to ~−59 dB over ±7 bins — hence the exclusion of ±16 bins; the artifacts of the old engine (±47 Hz = ±64 bins) are still caught.
- The sinc resampler has a group delay of ~3k source frames — accounted for in the tolerance of the consumption test.
- rubato ratio = 1/rate (rate 0.5 → the output is twice the input, the pitch drops — the desired slowed aesthetic).

## New effect parameters (R.33 C5, item 228)

- **The default of a new parameter = the previous behaviour BYTE-FOR-BYTE.** All
  golden files (t26, GOLDEN_VIDEO, GOLDEN_AUDIO) rest exactly on it, and none of them
  was regenerated in C5. Practice: the kernel has an explicit `plain` branch (the defaults) — it repeats the previous code verbatim, including the
  AVX2 path; everything else is computed separately.
- **Integers instead of round() where there is a GPU mirror.** `round()` in
  WGSL rounds halves to EVEN, in Rust — AWAY FROM ZERO. The jitter of VHS
  lines drifted by a pixel because of this. The parameter is quantized once (×1024) and
  from then on is computed in integers on both paths.
- **A kernel that edits the frame IN PLACE does not touch all channels.** VHS does not write
  green outside the scan line — on a line shift it stayed from the old
  pixel, and the mirrors diverged. A new parameter MUST be checked on
  ALL channels, not only on those the kernel wrote before.
- The constructors `FxInstance::blur/edges/noise/vhs/glow/greenscreen/gain`
  set the defaults of the added fields — literals in the tests and in the calling
  code go through them, so the next parameter does not touch fifty places.

## Node audio chain (R.47, items 324-326)

Audio lives in `src/audio/{mod,nodes,primitives,warp}.rs`; the public surface is re-exported from lib.rs (the paths `kt_fx::…` did not change).
`ClipProcessor` holds a `Vec<(slot, AudioNode)>` — processing goes IN
THE ORDER OF THE SLOTS (the owner's decision of R.47; the former fixed pipeline
"the volume always after the reverb" is removed). The warp is NOT a node (a monopoly on
consumption of the source, `chain_source_*`); LayerMix is a marker node, the snapshot
of the dry side is taken by `produce()` before the first node with a slot >= from.

Rules of a node (see the doc in nodes.rs): the state survives `seam`; the targets
of Smooth parameters are set in `set_targets` once per chunk (the gate "only when
auto is non-empty" preserves the bytes of old chains — `apply` sets the bases);
the phase of the LFO is a FUNCTION OF POSITION (`lfo_phase` = the integral of the frequency curve,
`param_integral` in kt-model), not a counter; an effect with memory MUST live in
BOTH formulas of the tail — `AudioNode::tail()` and the static `chain_tail`
(their agreement is guarded by t322f); on zero input in the tail the node rings out.

Checklist "add an audio effect": an enum variant + serde defaults +
`params_for!` + a constructor + `class() -> Audio` (DO NOT forget — otherwise silently
Video2d) + `tag()` (a compile-time lock) + `FxInstance::all()` + a variant of
`AudioNode` (build/apply/set_targets/process/tail/reset) + a branch of
`chain_tail` for a tail + a branch of `chain_lookahead` for lookahead +
`fx_display_name` + the kt-app registry entry +
`param_label`/catalogues ×5 + for new units `unit_suffix`. The t322 frame
will run the newcomer automatically; a mismatch of tails/non-determinism is
its job — do not write a copy-paste for each effect.


## Chain lookahead (R.48, items 337-339)

An effect with an analysis window (a limiter, pitch) reads the source FORWARD, while its output is delivered aligned:
`produce` keeps two positions of the chunk — the output `q` and the
input `p = q + La` (the source, the warp and the tail gate live in the INPUT
domain); the node of slot k gets `pos = q + ahead[k]`, and each
lookahead node "consumes" its window with its own ring. At `La = 0`
everything is arithmetically identical to the PDC version — GOLDEN_AUDIO stands on this.

Rules of a lookahead node:
- **it lives in BOTH formulas** — `AudioNode::lookahead()` and the static
  `chain_lookahead`; their agreement is guarded by t322g. The formula is kept in ONE
  home (`limiter_la_frames` is called by both sides);
- **lookahead is not a tail**: it enters the pre-roll of `hard_seek` and the window
  `chain_source_window_ext`, but NOT `chain_tail` (otherwise the event sounds
  after its own end);
- **La < MIN_PREROLL_FRAMES (4800)** — otherwise the seek pre-roll would not be enough,
  and the first frames after a seek would come from an empty ring;
- **a change of La is structural**: `FxStateStore` checks
  `lookahead_frames()` against the static formula and REBUILDS the processor
  (the same for `ChainOpts`: the pitch engine is a project setting);
- **the neutral must be byte-for-byte**: zero parameters — an early
  exit without touching the buffer, and `La = 0` (t337);
- **the grids are absolute**: the grain/hop are computed from the clip position, not from
  the call counter, otherwise the slicing of the buffers changes the sound (t338; the
  OLA accumulator MUST hold the processor chunk of 1024 PLUS the full synthesis window).

## Video effects of R.47 (items 332-335): classes and checklist

Three classes of video effect; the class determines ALL the placement of the code:
- **Geo** (`geom::is_geo`): a reversible map "output pixel → input point" — a step of the merged pass, concatenated. Linear ones — affine/Proj;
  non-linear (R.47) — `GeomPrim::Warp` (lens/twirl/ripple/kaleidoscope/
  cylinder): the input and the output of the primitive are the NORMALIZED centred
  space (the pre/post-affines are built by the builder; magnitudes ~1 — this
  holds the f32 parity of the GPU); the precomputable part — f64 on the CPU into the uniform
  once; neutral parameters — a SKIP of the step (identity bytes);
  PerPixel supersampling (the condition `Proj|Warp`). The map is ONE function
  `warp_map` for both CPU loops; the WGSL mirror is in geom_fused.wgsl (both copies
  of the loop of the primitives!) is edited in THE SAME COMMIT.
- **Pointwise** (`geom::is_pointwise`): the colour of a pixel without neighbours.
  The formula is ONE tap function (`tap_grade`/`tap_duo`/`tap_poster`/
  `tap_vig`, f64): it is called by the CPU barrier (`pointwise_barrier`) and the CPU tap
  of the merged pass; there are TWO WGSL mirrors — its own pass (the barrier position) and
  the tap arm of geom_fused (kind 2..5, meta 4 bits). In total one f64 text and
  its two WGSL mirrors; a divergence is caught by the t329 cycles. Vignette measures the coordinate of ITS OWN STAGE
  (rec[ti]/stage_w/h — a precedent from Noise).
- **Barriers** (the remainder): they break the geo series; the K×-rule of item 313 applies to each on its own
  (Sharpen/DropShadow — mip ×K and 3 GPU passes; Pixelate — cell ×K,
  EXACT; Halftone — the pattern in LOGICAL coordinates l=p/K; RadialBlur —
  fractions of the frame + EXACTLY 16 taps as a constant; ChromaAb — fractions of the frame).
  DropShadow: an input without alpha — passthrough on both paths; the formula at
  alpha 255 degenerates into an identity (the early exit is an optimization).

Checklist "add a video effect": an enum variant + serde defaults +
`params_for!` + a constructor + `tag()` (a compile-time lock) + `all()` +
a class (an arm of `is_geo`/`is_pointwise` OR the arms of apply_barrier + FxOp/pass in
kt-gpu + chain_ops) + a line GOLDEN_ALL in t328 (the completeness lock will not let
you forget it) + a tolerance class in t329 + `fx_display_name` + the picker template +
`video_meta` (the t48 lock: a fallback category = an error) + `param_label` +
catalogues ×5 (+ subset-fonts for new glyphs). The semantic test — in
geo_fuse (t330/t332/t333 — samples by class).
