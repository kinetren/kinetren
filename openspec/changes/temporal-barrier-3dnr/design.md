# Design: temporal-barrier-3dnr

> DESIGN ONLY, revision 2 (2026-11-25), base `fc60a5d9`. Q1 a, Q2 a decided (A21). The physics first (the owner, 2026-11-01). Implementation-ready
> for events; tracks and compositions come with `track-stream-stage`.

## D0. What revision 1 said and the code contradicts

1. **A new class and contract, `TemporalKernel { pre, step, post: PassGraph }`, hosted by kt-engine over fragment passes.** The stream class has
   everything a recursion needs (memo, checkpoints, live device tier, walks, pacing, Loading, kept composites by key, prefetch, preroll, its
   trace): a second host would copy it. The history's kernel runs as COMPUTE jobs on the host device, not as fragment passes. So the camera with a
   recursion IS a stream slot (D3): three parts of one slot, no new contract but two small hooks.
2. **The recursion "in linear YCbCr, the ISP's own s09 matrix moved before it".** s07 ends in RGB8 after the colour matrix and the tone curve
   (`s07_demosaic.wgsl:2`); nothing after it is linear or deeper than 8 bits. A phone's 3DNR works there, on the gamma YUV after the tone curve: the block stands between
   s08 and s09 and its INPUT is 8-bit; only its STATE is 16-bit (D1).
3. **Positions in MATERIAL frames, "playback speed, reverse and trims do not change what was recorded".** The camera stands in the chain, after the event's
   time: its noise is seeded by the output frame (`burst_index`), the frame rate it films at is the project's. A material history would filter noise that
   no shown frame had, and a frame shown twice by a slowed event would be recursed once. Positions are the project's frames of the event, as the stream
   host counts them.
4. **A cold start at every event.** A split in the middle of a take would start the filter again: the noise jumps at a cut that changes nothing. The host
   reaches into the event before when it is the same take (D2).
5. **Anchors on the media's zero.** The host's layout is `[first, cut, end]` in project frames: anchors are `cut + k·G`.
6. **The state in `R32Uint` textures, "ledger rows".** Compute buffers in the memo's device tier (`StreamMemo::put_device`), counted there.
7. **`HeldStreamOut` is three 4:2:0 planes** (the codec's): the filter's picture would lose half its colour. It gets an RGBA form.
8. t3545's "registry refuses a Temporal class" has no class to refuse; it tests the two hooks.

## Context

`plugins/fx-video-camera` (twelve fragment passes in `gpu_values`, `Plan::new(p, k, burst_index)`, passes s00–s12, 8-bit RGB between s07/s08/s09), `stream.rs`
(`StreamKernel`, `StreamLayout`, `StreamMemo` with checkpoints and the device tier), `video_stream.rs` (`StreamStage::front`, `ClipStream::{on_device, rows_at}`,
`after_chain`, `link_stages`), `FxInstance::is_stream` (by the manifest's class), `front_exec`, `HeldStreamOut`, the compute host. Change `track-stream-stage` gives
tracks the same host (`HostStream`).

## Goals / Non-Goals

**Goals:** the recursive filter with its motion detector, luma and chroma apart; trails that come from its parameters; rolling shutter by readout time;
flicker by the mains; bytes independent of the way to a frame; the GPU.
**Non-Goals:** motion-compensated noise reduction; temporal exposure loops; another effect on the class.

## D1. The physics of the block

After s08 (lens corrected, RGB8), in the ISP's own YCbCr (Kr, Kb of its rows), per frame N:
```text
d_y = box_r(|Y_N − Ŷ_{N−1}|)                  // luma detector, r = tnr_detect_blur
m_y = smoothstep(th_y, 2·th_y, d_y)
Ŷ_N = Y_N + α·(1 − m_y)·(Ŷ_{N−1} − Y_N)       // α = tnr_strength
d_c = box_r(|Cb − Ĉb| + |Cr − Ĉr|), m_c with th_c, Ĉb, Ĉr likewise
```
The state is 16-bit fixed point (8.8): Y | pad, Cb | Cr in two words; the output is Ŷ, Ĉ back through the matrix, rounded to RGB8 for s09. An 8-bit state has a dead band
(`(1−α)·|Y − Ŷ| < ½` returns Ŷ for ever), which would be a fault of arithmetic, not of a camera.
Trails: where an object leaves a background that differs by less than the threshold, `m ≈ 0` and its ghost decays as α^k; a chroma threshold above the luma's gives the
coloured ghost he named ("хромо-шлейфы"). Static noise variance falls by (1−α)/(1+α).

## D2. Positions, anchors, warm-up

Position = project frame of the event. G = 32, anchors `A = cut + k·G`. State at A = a cold recursion (Ŷ := Y) over `[max(first, A − W), A]`; frames after it go on from it.
`W = clamp(⌈ln(1/510)/ln α⌉, 1, 64)` (Q1 a). Beyond W an input weighs under half a code value, except past the cap: at α 0.97 the trail is cut at 64 frames (14 % of
it is left), as he chose.
`first = cut − W` when the event before on the lane is the same take going on: same media, its material ends where this begins, no transition, the same
camera slot (rows equal, curves allowed), the same slots before it (`front_key`). The host answers by one new fact, `StreamLayout::continues() -> bool` (false by default),
computed in `link_stages`; otherwise `first = cut`. A seek walks at most `W + G` frames; steady play, one.

## D3. The slot is a stream when it keeps a history

`camera_isp` declares a `StreamKernel` beside its barrier, and one hook says when: `StreamKernel::active(&self, p: &[f64]) -> bool { true }` — here `tnr > 0 || rs_readout_ms > 0`.
`FxInstance::is_stream` asks it. Off: today's barrier, byte for byte.

When active, the host cuts the slot in three by a second hook, `StreamKernel::split(&self, slot: &PluginSlot) -> Option<(PluginSlot, PluginSlot)>` (front, back); `None` — as
today:
- **front** = internal plugin `camera_isp_front` (a barrier of s00–s08 with the slot's rows at the position): joins `StreamStage::front`, so every history frame is
  made by the existing front executor on the compute device and memoized by it. Its noise seed is the position;
- **the kernel**: per needed frame one compute job: input the front's held RGBA, previous state buffer → state, output RGBA, optionally the motion map; checkpoints at anchors
  (two words per pixel: 16 MB at 1080p each), the live tier holds the last state (gpu-round-trips' rule);
- **back** = `camera_isp_back` (s09–s12, its view): takes the slot's place in `after_chain`.
Both internal plugins are registered and hidden from the picker (as `matte_view` of `chroma-key-upgrade`); they never reach a file.
`render_held` returns `HeldStreamOut::Rgba(Held)` (new form; `held_planes.wgsl` gets its trivial twin), `render` the bytes.
`key(n)` = hash(anchor, rows at each frame of `[start(A), n]`, idents), kept in the plan cell.
With no compute device the stage has no picture, as Datamosh has none (said by the same status).

Masks of the slot, its strength and mute: a masked stream slot is mixed after the stage by the existing rule; `back` carries the mask.

## D4. Rolling shutter and flicker

- **Rolling shutter** (`rs_readout_ms`, 0 = off): the front alone needs the scene of N−1 and N: row y of N is their linear blend at `t = 1 − readout·(1 − y/H)/T`. It is a compute job on the fronts of the
  slots BEFORE the camera, so with it the front is split once more: `camera_isp_scene` (s00–s01) → blend → `camera_isp_front` (s02–s08) as a compute-side
  front plan. Order: scene(N−1), scene(N) → blend → front → TNR. `needs(n)` gains one frame before.
- **Flicker** (`flicker_hz` 0/50/60, `flicker_depth`): light at row y `1 − depth·(1 − |cos(2π·f·t_y)|)`, `t_y = N/fps + readout·y/H`, a factor in s02 from a uniform (the frame's time is `N/fps`, exact rational
  arithmetic, `FrozenMath`). No history: with it alone the slot stays a barrier.

## D5. Rows, window, sound

`tnr` (flag), `tnr_strength` 0–0.98, `tnr_motion_y`, `tnr_motion_c` (code values), `tnr_detect_blur` (px of the sensor), `rs_readout_ms` 0–33, `flicker_hz`, `flicker_depth`; view "3DNR motion map". Presets "Phone, low light,
trails" (α 0.9, th_y 6, th_c 14), "Action camera, jello" (readout 25 ms). In `isp_panel.rs` the TNR block stands between "Lens corrections" and "YUV" in the pipeline, rolling shutter and flicker in the
sensor's. The sound half (`camera_isp_sound`) has nothing temporal of this: named in the note (a video denoiser and a rolling shutter have no sound; mains flicker's hum is the microphone row it already has).

## D6. Format

New rows, omitted at default, raise the file by content. OTIO: `metadata["kt"]` as the camera's other rows; no new row.

## D7. Class walk: "a picture that depends on earlier pictures"

| Place | Here |
|---|---|
| Datamosh | unchanged; `active`/`split` default |
| an event's read in a branch (`streams_of`) | the same stage: a TNR camera in an arm is a read |
| a track's, a composition's camera | `track-stream-stage`'s host asks the same kernel; until it is in, the rows on a track are disabled with the reason, as Datamosh is |
| a morpher layer's camera | the layer's windows are events' chains: the stage starts where the window does |
| a generator under the camera | a source like any |
| frame blending, slowed, reverse | before the chain: the camera films the result (D0.3) |
| Echo and other track stages | after |
| VHS drift, film weave (`fx-lens-and-camera`) | per-frame functions; could use `split` later |
| the strip, poster, cells | `stream_preroll` as for Datamosh: a cell of a TNR camera costs a history; its cells are the barrier's with TNR off (the variant is said by its preset name) |
| the prerender to disk, export, reverse play | the one host |

## D8. Cost and idle

Per history frame: the front (nine fragment passes of the ISP) plus one job. On a card about 1 ms + 0.3 ms at 1080p: a seek of 96 frames ≈ 0.15 s under Loading. On llvmpipe the ISP front is about 100 ms: a seek
≈ 10 s of Loading; the status line names the history. Steady play adds one job. At rest nothing walks (t3551).

## Rejected

- A host of its own for fragment recursions (D0.1); a material history (D0.3); recursion from the event's first frame (a seek at minute 10 walks 15 000 frames); an 8-bit state.
