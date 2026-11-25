# Design: pro-keyer

> DESIGN ONLY, revision 2 (2026-11-25), base `fc60a5d9`. Q1 a, Q2 a decided (A22, its Q46–Q47). Implementation-ready after `chroma-key-upgrade`
> (revision 3: its difference model, `matte_view`, `AnalysisKind::Key`).

## D0. What revision 1 said and the code contradicts

1. **`clip_frame_keys` "adds `(media, held_pts, quality)`".** It returns the frame and its blend partner; a held frame is of another moment and must not be blended. It is a second question,
   `held_keys(clip, quality)`, asked wherever the leaf's keys are asked (`leaf_keys`, `layers_at`, `wanted_frames`, `max_concurrent_layers`), and read in `layer_src` into `LayerSrc.held` (D7).
2. **"The input frame".** A barrier reads the output of the slots before it; the plate is a frame of the SOURCE. A crop or a geometry before the keyer would misplace it. The held frame goes through the slots
   before the keyer as the source does (D7), rows at the HELD moment.
3. **The key on a track, on a composition's instance, on a layer**: there is no one source to hold there. The held plate is refused with its reason, the generated plate works (D2).
4. **Word passes "where precision matters"**: a target is `R32Uint` per pass, one word (`word_passes`); three 16-bit linear channels do not fit. Packing is 10:11:11 bits in linear light with a dither-free rounding; the arithmetic is f32 in
   one pass from the 8-bit input (D1). Passes that need only alpha keep 8 bits.
5. **`cook` gives eight numbers**: the keyer has thirty rows; the graph is built by `gpu_values`.
6. **"A pyramid to ~8×8", fills and chokes by long loops**: a loop past 32 iterations costs ten times its body on llvmpipe (kt-gpu's rule). Chokes are the growing three-tap passes of `chroma-key-upgrade`
   D4; the pyramid is `PassAt` halves.
7. **Views by the Viewer's chips** are `matte_view` modes over the OUTPUT's alpha; Core, Edge, Plate, Shadow and Spill are inside the graph: the copy's slot carries an internal row `show` (D6).
8. Tests: t3595–t3599 belong to other changes now; the keyer's are t3585–t3590, t4254.

## Context

`barrier.rs` (`BarrierKernel::{gpu_values, word_passes}`, `Node::{Pass, PassAt, Resample, Save}`, `Slot`, `GpuCtx { res_scale, … }`), the executor's loop (`engine.rs:3968`), `LayerSrc`
(`video_plan.rs:57`), `clip_frame_keys` (`video.rs:784`), frame blending, `moment_key::put_src`, `isp_panel.rs`, `chroma-key-upgrade` D2–D7.

## Goals / Non-Goals

**Goals:** uneven screens, hair, shadows, spill and bounce handled by one effect; deterministic; GPU; tunable
by sight.

**Non-Goals:** neural matting (the owner's base is an algorithm); light wrap (Q2); temporal matte
stabilisation; 3D keying (Primatte's polyhedron) — the plate-relative difference model covers uneven screens
more simply.

## D1. Linear light

Every pass that mixes colours decodes 8 bits to linear by the sRGB curve (a closed f32 form by `pow`-free polynomial, the same text on the CPU oracle), works in f32 and either ends in 8 bits (the last pass, alpha passes) or packs linear RGB into one word
(10:11:11). The rounding of a pack is the stored frames' tolerance class (`FrameTol` measured at landing). Keying in linear light makes the shadow ratio a light ratio and the restore preserve light.
`hdr-sources-to-sdr` stage 5 gives these passes their 16-bit input later; nothing here assumes SDR white but the 8-bit decode.

## D2. The clean plate

- **Generated** (default): a provisional screen-ness s0 with the global screen colour (from Analyze or the
  key rows); weights w = s0⁴ (only clearly-screen pixels vote); a pyramid down of (Σ w·c, Σ w) by 2× box to
  ~8×8, then up: at each level, where the level's weight is low, the coarser level's colour fills in
  (push-pull). The result is a smooth screen colour everywhere, the subject erased.
- **Held frame**: the row `plate_at` (material tick, −1 = generated, the default; Q1 a); the plan fetches that frame of
  the event's own source, through the slots before the keyer at that moment, the plate = that frame lightly blurred (3×3).
  Only on an event with a file or a generator as its source; on a track, a layer or a composition's instance the row is disabled with its reason. A held frame from a moving camera gives a wrong plate: the panel says so (the plate view shows it).

## D3. Mattes

```text
e(P)   = P_k − (w·P_a + (1−w)·P_b)        e(C) likewise for the plate colour C at the pixel
s      = clamp(e(P)/max(e(C), ε), 0, 1)   // screen-ness against the plate
core   = fill_holes(choke(clip(1 − s, core_black, core_white), core_choke))
edge   = clip(1 − s, edge_black, edge_white)              // soft
band   = dilate(core, edge_band) − erode(core, edge_band)
alpha  = core outside the band; max(core, edge) inside it
```

Holes are filled by a coarse closing (dilate then erode at the band's scale) restricted to the core;
dilate/erode separable min/max by the growing three-tap passes (radii in thousandths of the frame's height × `res_scale`, so preview and export agree). Defaults from Analyze
(`AnalysisKind::Key` with the keyer's rows: colour, balance, clips, five frames, one undo step).

## D4. Shadows

Where s is high (screen), `ratio = Y(P)/Y(C)`; `shadow = clamp((1 − ratio)·shadow_gain, 0, 1)` softened by a
box blur; the output's alpha = max(alpha, shadow·shadow_amount) with colour black (a darkening layer over what
is below). `shadow_amount = 0` drops shadows entirely.

## D5. Spill and bounce light

On the subject (alpha > 0): `excess = max(0, e(P) − bias·e_ref)` removed from the key channel; luminance
restored to the original's (the upgrade's restore); the removed light replaced by `replace_mode`: neutral grey
of the same luma, a chosen colour, or `subject hue` — the mean hue of the pixel's neighbourhood where excess
is near zero (a 9×9 weighted mean at quarter size), so a green-lit shoulder returns to the shirt's colour.

## D6. Outputs and the Viewer

The Viewer's chips on a Pro Keyer node: Composite, Matte, Status (by `matte_view`) and Core, Edge, Plate, Shadow, Spill. The last five set, in the copy the player holds (`Viewer::cut_event`), the internal row `show` of the slot
(not in the passport a person sees, refused in a file): the graph ends at that stage. Never in the model, never rendered.

## D7. The held frame

```text
BarrierKernel::held(&self, values: &[f64]) -> Option<Tick>      // material tick; default None
Slot::Held                                                       // the held frame after the slots before this one
```
- The engine: `VideoClip.held: Option<HeldAt { tick, slot }>` from the first slot that asks (a second one is refused by the chain check, named). `held_keys` gives `(source, floor(tick), the layer's decode size)`; the order,
  the prefetch window, the coverage, the export chunk (`max_concurrent_layers` + 1) and the copies ask it; a missing one makes the frame incomplete.
- `LayerSrc.held: Option<Box<LayerSrc>>`: a layer of the same event over the slots before the keyer with rows at `tick` (the machinery of a graph read, `reads`): the executor resolves it as a read and gives the graph `Slot::Held`.
  A stream stage or a time kind in those slots refuses the held plate (a history of another moment), named.
- `put_src` hashes it (its leaf included); a chain without one keeps its key.
- In an arm of the graph the same; on a reel instance and a track: no `held` (D0.3).

## D8. The panel

The ISP-style pipeline (the simulation rule's device-like panel, though this is not a simulation): Plate →
Screen → Core → Edge → Shadow → Spill → Out, each stage a tile with its few rows and a "view" eye that sets the
Viewer; "Analyze" at the head; "Use this frame as the plate".

## D9. Cost

At 1080p ≈ 12 passes; the heaviest are the choke/fill (radius-bounded, separable) and the plate pyramid (log
size); comparable to the camera ISP. Measured per pass by the pass timer; the export uses the same graph.

## D10. Class walk — "a key that assumes an even screen"

| Place | Assumes one screen colour? | Here |
|---|---|---|
| greenscreen (distance) | yes | unchanged (old slots) |
| greenscreen (difference, upgrade) | yes, with balance | the simple key |
| Pro Keyer | no — per-pixel plate | this change |
| magic-wand mask colour range | seed colours | its own tool |

## Rejected

- **A neural matte as the base** — the owner's decision (2026-11-01); an assist later.
- **Primatte's 3D polyhedron** — hard to tune by sight; the plate-relative model divides the unevenness out.
- **Light wrap through a hidden read of the layer below** — breaks the single-input contract (Q2).
