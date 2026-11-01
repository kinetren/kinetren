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
- **An event grid computes one cell and copies it** (morpher-event-grid-cost, ktprompts 26.10.18
  item 8: "it is basically a simple pixel shader — so do we have extra computations?"). The grid was
  one pass, but with 4+ cells per axis every output pixel averages 4×4 sub-samples (item 309), and
  every cell repeated the same tile: 12×10 cost exactly what 12×30 did, 16× a 1×1 grid. The builder
  marks a series `GeomProgram::tile` when every cell provably holds the same bytes, and both
  executors compute the cell at the grid's origin once and copy it (CPU `exec_geom_cpu`; GPU the
  tile program into a cw×ch target + `grid_tile.wgsl`). The gate is EXACTNESS, not a heuristic: the
  grid must be the last step of the series (nothing acts on the output after the Fold), the output
  at 1×, and the taps 1, 2 or 4 per axis — then `ox − 0.5 + (i + 0.5)/n` is dyadic, `x − kx·cw`
  is the same number in every cell in f64 and in f32, and no edge ring is needed (every sub-sample
  of a pixel inside the grid lies in its own cell). With 3 taps the offsets are not dyadic and round
  per cell, so the full pass stays; the owner's automated grid passes through such frames (3 cells
  on an axis) and they keep their cost. t943 sweeps the exactness on the CPU, t944 holds the GPU tile
  against the GPU full pass byte for byte. Release, 960×540: 12×30 274 → 2.1 ms on the CPU, 12.0 →
  3.4 ms on llvmpipe.

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
- **A pre-shrunk (mip) source for the grid** (morpher-event-grid-cost, alternative (a)): cheaper
  still and without the aliasing that 4×4 taps leave at 30× minification — but it changes the bytes
  of every grid golden and the look of existing projects. Offered to the owner as a separate
  decision (the change's Open Question 2), not taken here. A "fast grid" switch was rejected by the
  R.44 "no switches" rule; caching the grid output across frames helps only a still frame.

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

## The catch-up waves and their rank (effects-transitions-catchup, archived 2026-10-30)

Moved here from the archived change's design (its task 3.2); `t979`/`t981`/`t982`
(`src/apps/kt-cli/tests/fx_catalogue.rs`) read these two sections.

### D2. The ranked waves
Classes: G = geo (closed primitives), P = point-wise (tap), B = barrier (own pass), V = variant or
preset over an existing kind (no new kernel), A = audio node. Families are `plugin-api-stage1` D6
crates.

**W1 — Lens and camera** (`src/plugins/fx-video-distort`, `src/plugins/fx-video-color`,
`src/plugins/fx-video-stylize`; capability `video-fx`; depends on F-FX-002, F-FX-001, F-FX-008;
opened as the change fx-lens-and-camera):
- `lens_housing` (P): the dark circular housing and soft corners of a fisheye, measured in the
  coordinates of its own stage like `vignette`; pairs with `lens_distort` in a factory preset
  "Fisheye lens".
- `lens_dirt` (B): highlights extracted with `glow`'s threshold, multiplied by a procedural dirt
  texture (seeded noise pinned to the stage grid, as `noise` pins grain), added back — invisible on
  dark frames, as real dirt is (F-FX-007, trick 3).
- `lens_drops` (G): refraction by drops is an output→input displacement, i.e. invertible by
  construction — a new `drops` MODE of the Warp primitive (a core edit of `geom.rs` +
  `geom_fused.wgsl`, the one core change of the wave), seeded drop positions and radii.
- `lens_flare` (B): ghosts along the line through a light position and the frame centre; the
  position is a parameter (animatable), not detected.
- Camera shake and zoom punch (V): factory chain presets on `geometry` curves with zigzag segments
  and eased keys ("Zoom Shake", "Handheld"; "Punch in" is already a factory preset).
- (The live re-check of 2026-10-30, F-FX-008, moved `lens_dirt` and `lens_flare` next to `glow` in
  `fx-video-stylize`: a family crate cannot borrow another family's threshold code.)

**W2 — Glitch, film and soft looks** (`src/plugins/fx-video-stylize`, `src/plugins/fx-video-blur`;
`video-fx`; depends on F-FX-003, F-CLR-007):
- `glitch_blocks` (B): block displacement and per-channel offset, a pure function of `burst_index`
  and a seed.
- `film_damage` (B): scratches and dust specks; `gate_weave` (G): a per-frame affine jitter from the
  burst index; `halation` (B): highlights blurred and tinted, the `glow` pattern.
- `light_leak` (P): moving procedural light blobs, a pure function of `u` — the effect form of the
  existing `light_leaks` transition.
- `lens_blur` (B): a disc (bokeh) kernel with highlight weighting, under the K× rule.
- A directional mode of `radial_blur`. Revision: a directional blur returns as a MODE of
  `radial_blur`, not as a kind — the video-fx "Deferred" line already names this form, and the
  rejection of a separate kind stands.
- Sepia, "Movie Grain", "Cinematic" (V): variants of `duotone`, `noise`, `color_grade`. Revision:
  sepia and invert stay variants of existing kinds, as the video-fx rejection of "sepia/invert as
  separate kinds" requires — no new kind is created for them.

**W5 — Transitions catch-up** (`src/plugins/transitions-basic`, `src/plugins/transitions-styled`;
capability `transitions`; depends on F-CMP-002):
- `luma` with a user image as the matte (the passport already has a media TYPE); more `iris` shapes
  (heart, star, hexagon — appended at the end of the shape list); warp transitions (`lens_warp`,
  `ripple_warp`, `twirl_warp`) on the same uniform road as the volume kinds; a motion-blur amount on
  `push`/`slide` whose default 0 gives the former bytes.

**W3 — Text animation library** (`src/plugins/generators-title`; capability `generators`; depends on
F-TXT-006, F-TL-012):
- In / Out / Loop as three preset slots on the title (curve presets over the title's passport, the
  R.53 "presets set everything" form); per-word targets for the owner's "blur on one word of a
  ticker". The first task of that change is an investigation: a per-span style inside the title
  versus a word-box mask (the second waits for F-MSK-001).

**W7 — Voice presets** (`src/plugins/fx-audio-modulation`; capability `audio-fx`; depends on
F-AUD-010, F-AUT-004):
- Presets over existing ids: telephone (`filter` band-pass + `distortion`), megaphone (`filter` +
  `distortion` + `eq`), radio (`filter` + `bitcrusher`), chipmunk / deep (`pitch_shift`), small hall
  (`delay` + `eq`).
- One new node: `ring_mod` (A, a carrier frequency and mix; no state beyond the carrier phase,
  which is a function of the clip position as the LFO rule requires) for "robot".

**W4 — Speed-ramp presets and beat sync** (no new kernel: the presets live in the factory preset
file, the beat analysis is a background job of the program; capabilities `automation` and
`audio-fx`; depends on F-SPD-002, F-AUT-008):
- Rate-curve presets for `slowed_reverb`: Montage, Hero, Bullet, Jump Cut, Flash In, Flash Out
  (signed rate and freeze, R.57, make all six expressible).
- Beat markers from an offline analysis job (the loudness pattern: background, cancellable, result in
  the sidecar), then "snap the ramp to the beats".
- Audio-reactive zoom: a link (F-AUT-008) from an analysis envelope to `geometry` scale.
- Jag edit: a rate preset plus frame interpolation (F-SPD-003).

**W6 — Temporal and relief** (`src/plugins/fx-video-stylize`, `src/plugins/fx-video-distort`;
capabilities `video-fx` and `plugins`; depends on F-FX-005, F-CMP-003):
- `echo` / `trails` and the owner's "buffer + blend levels": a NEW plugin declaration — a temporal
  window of N past INPUT frames (pre-FX frames at t−1 … t−N from the cache, invariant 4) — so the
  kernel stays a pure function and the post-FX memo stays honest. This extends `plugin-api-stage1`'s
  refusal table and is designed in that wave's change.
- `droste` (G): the homothety / recursive zoom as a log-polar `droste` MODE of the Warp primitive
  (core edit, like W1's drops).
- `relief` (B): normals from luminance and one light — bump mapping.

### D3. The rank
W1 first: it is the owner's own FX item 1, and four kernels plus presets cover the lens group that
we lack entirely. W2 next: the most CapCut tabs (Retro, Glitch, Cinematic, Blur) for six kernels. W5:
transitions are half of the complaint, and the gap is short because R.51 already took the trend set.
W3 is ranked after them only because it starts with an investigation; it is the owner's complaint F2
("the text generator is poor") and may be pulled forward on his word. W7 is cheap but audio was not
the complaint. W4 waits for the analysis subsystem; W6 for an API extension.

## The datamosh is a codec, not a look (datamosh-codec-path, archived 2026-11-01)

The owner asked for "a real motion-compression path we can break carefully with parameters" and
answered (p.4) that a real effect is first its physics. A datamosh is what a motion-compensated
DECODER makes of a broken stream, so `datamosh` (`src/plugins/fx-video-codec`) runs its own block
codec: Y'CbCr 4:2:0 planes at an encoded height of its own, 16×16/8×8 block matching (candidates,
a diamond, a ±1 refinement, total tie order), a CLOSED prediction loop (the encoder predicts from
its own reconstruction, so an intact stream errs by one quantisation step per frame, t1235), an
8×8 DCT residual with the H.264 step law, a GOP with I-frames at the stream start, the cut and every
`gop` frames (0 — only at the cut and on a reset). The breakages change what the decoder is told —
drop I-frames, repeat a packet (bloom), freeze the reference, transform or transplant vectors, lose
residual or whole blocks — and nothing else; they are GATES and quantities drawn by curves, so a
person turns the mosh on and off and resets it with a key on the beat (the owner on the first build:
"no curves … no way to reset … GOP limited to 60 for some reason"). Full text: the archived change's design.md D1–D8; the reconnaissance: F-FX-011.

- **Why an effect on the incoming event, not a transition:** a transition sees two pictures at ONE
  moment and needs an overlap; the classic mosh happens at a hard cut and needs both STREAMS. The
  stream may start in the event before (`before`), and the cut is an I-frame — what a real encoder's
  scene-cut detection puts there — so "drop the I-frame at the cut" is the Cut mosh preset.
- **Why the stage runs on the CPU in the plan's common code (D4):** the codec is a serial loop with
  data-dependent search, and a mosh AMPLIFIES a one-LSB difference frame after frame; a GPU twin
  held to a tolerance would drift visibly. Both executors receive the decoded bytes as the layer's
  pre-FX frame, like frame blending; t329 excludes the kind, t1241 holds the bytes.
- **Why a replay with a memo, not a continued state (D3):** a renderer is rebuilt on every edit and
  preview, prefetch and export each have their own; a continued decoder would make bytes depend on
  what was rendered before. Frame N is the loop from its anchor; the memo (keyed by the source
  ADDRESSES and the per-frame rows of the chain, bounded) only skips work. A sliding history window
  was rejected: every frame would start its own chain and nothing could be reused. The intra refresh
  is a fixed grid of 600 frames from the anchor for the same reason.
- **Why the gates read curves per position, and the GOP does not:** a gate decides what the decoder
  is told AT a frame, so its curve is sampled at that frame's material position (`rows_at`) and the
  output stays a pure function of the curves. The GOP, the block, the search range, the encoded
  height and the frames before the cut shape the stream's structure; a curve on them would make the
  keyframe of N depend on its value at every earlier frame. A reset adds a keyframe without moving the
  GOP grid for the same reason.
- **A region of interest is a door, not a feature (design D9):** the stream contract carries a
  per-frame coverage region; the datamosh shows its broken stream inside and the intact one outside.
  A mask or an object detector (the owner: "faces, whole people") implements `region_ident` and
  `with_region` on the engine's stream layout; nothing else changes.
- **Measured (release, 360 lines):** encode 7.6 ms, broken decode 0.5 ms per frame; a cold seek
  through a 300-frame chain ≈ 2.4 s (600 at most, the refresh).
- **Not built:** a GOP strip in the panel (it would need the schedule's text a second time in
  kt-app); the stage on a reel or a track stage (skipped); a region SOURCE (masks, a detector).

## The mask rasterizer (shape-masks, 2026-11-01)

`kt_fx::mask::rasterize`: the inside is the nonzero winding at sample centres (a scanline over
crossings), the signed distance to the flattened outline is computed only in a band, the coverage is
a ramp of the distance (one pixel without feather, the feather centred on the contour as in After
Effects, the expansion moving the edge). A wide band is sampled on a coarser grid (step ≈ band/8)
and the distance interpolated: the distance field is 1-Lipschitz, and the cost stays O(perimeter)
instead of O(perimeter × feather²). Modes are the soft Boolean forms (OR, AND NOT, AND, XOR); AE's
Lighten/Darken were not taken — on a hard mask they equal Add/Intersect (F-MSK-001).

**One mask px is a project pixel of the reference frame's height** (`px_of`): the interface draws
with the source view as its reference, the engine with the decode raster; a feather in raster or
native pixels would differ between them, and between the preview at ¼ and the export.

The bytes depend only on IEEE-exact operations and the frozen math (the rotation), frozen by t1269.
The memo is keyed by the full Debug text of what decides the bytes (the owner's cache rule: keyed by
content, bounded, invisible to the bytes).

## The camera simulation (camera-isp-simulation, 2026-11-01)

`camera_isp` (`src/plugins/fx-video-camera`, F-FX-010) is a SIMULATION, not an imitation (the root
rule of 2026-11-01): the frame is unprocessed into a scene, a simulated lens and sensor film it, a
simulated ISP develops it. The archived change's design holds the chain step by step; what binds the
next editor:
- **Every defect has its correcting block** (the owner's review: "the paired block … you had to put
  into the fx"): distortion ↔ dewarp, lateral CA ↔ CA correction, vignetting ↔ LSC, colour shading ↔
  colour LSC, crosstalk ↔ CCM, Gr/Gb ↔ GEQ, blooming/clipping ↔ highlight and purple-fringe
  suppression, row/column noise ↔ the optical-black correction, dark current ↔ BLC, hot/dead ↔ DPC,
  shot/read ↔ raw NR by the noise model and YUV NR, demosaic false colour ↔ FCS, IR leak ↔ IR-cut and
  RGB-IR subtraction. A new defect comes with its block, or is not added (t1220 measures each pair).
- **The quantization points are the GPU's texture points** (Rgba8Uint): up to twelve passes,
  `Plan::passes` the one list and `Plan::uniforms` the one packing; the CPU (`cpu.rs`, f32) and the
  WGSL (`sNN_*.wgsl`) are edited as a pair, held by t1217 within `FrameTol::THRESHOLD`. Constants
  are literals that parse to the same f32 on both sides; a division the WGSL would round loosely is
  a multiply by a reciprocal the CPU computed.
- **Noise is integer hashes of the SENSOR pixel** (Irwin–Hall(4), the Poisson inverse CDF below 12
  e⁻): the same bits on every device; the fixed pattern is seeded by the sensor seed, the temporal
  noise by the timeline position (t1216).
- **K× magnifies the sensor**: the sensor has the logical frame's grid; a sub-pixel sample (OLPF,
  distortion, CA) legitimately resolves a finer K× frame.
- **Not here — the temporal stages**: 3DNR with its trails, rolling shutter and flicker need the
  slot's previous OUTPUT; the stream class is a CPU-only source stage, so they are the change
  `temporal-barrier-3dnr` (a temporal-barrier contract, GPU and CPU).
