# Design: temporal-barrier-3dnr

> DESIGN ONLY, revision 1 (2026-11-20), base `19be4176`. Q1–Q2 in `proposal.md`; the defaults follow the
> recommended options. The physics first (the owner, 2026-11-01 (4): "a real effect is first a PHYSICS
> problem").

## Context

Read on `19be4176`:

- **The camera ISP** (`src/plugins/fx-video-camera/`, F-FX-010): s00 unprocess → s01 optics → s02 pixel
  (photons → electrons, shot noise) → s03 readout → s04 raw front → s05 raw NR → s06 raw correct → s07
  demosaic → s08 lens correct → s09 YUV → s10 chroma → s11 view → s12 curve; CPU + AVX2 twin + twelve GPU
  passes. The design's "NOT HERE" moved 3DNR, rolling shutter and flicker to this change, option B "a
  temporal barrier".
- **The stream class** (`src/libs/kt-plugin-api/src/stream.rs`, plugins spec "A stream kind is a source stage
  with a declared history"): `needs(n, memo)`, `checkpoints`, `key`, `advance`, `render`; the host owns the
  memo; the slots in front of the stage feed it (`StreamStage::front`, stream-input-order) on the compute
  device (stream-front-on-device); history on the device (stream-history-on-device); pacing and Loading
  (catch-up-shows-loading); kept composites by `key` (structured-telemetry).
- **Word passes** (`R32Uint`, render-speed-no-card D3) exist for barriers.

## Goals / Non-Goals

**Goals:** the physical recursive 3DNR with its motion detector, luma and chroma apart; its trails appear
from its parameters, not as a painted effect; rolling shutter by readout time; flicker by mains frequency;
bytes independent of the playback path; GPU.

**Non-Goals:** motion-compensated TNR (a camera ISP's 3DNR is usually not motion-compensated; a
compensated mode would be the repair tool, a different intent); temporal AE/AWB loops (separate); a 3DNR
for other effects (the contract allows it, nothing else adopts it now).

## D1. The physics of the block

Per frame N, after demosaic, in linear YCbCr (the ISP's own s09 matrix moved before it):

```text
d_y  = blur_r(|Y_N − Ŷ_{N−1}|)                 // detector on luma, box blur radius r (detector_blur)
m_y  = smoothstep(th_y, 2·th_y, d_y)           // motion confidence 0..1 (th_y: tnr_motion_y)
Ŷ_N  = Y_N + α·(1 − m_y)·(Ŷ_{N−1} − Y_N)       // α: tnr_strength
d_c, m_c, Ĉ_N likewise with th_c (tnr_motion_c), on Cb and Cr together
```

Trails: where an object moves over a similar background, `d` stays below `th`, `m ≈ 0`, and the previous
output (which already held the object) remains — a ghost decaying as α^k. A chroma threshold above the luma
threshold (the usual tuning: chroma noise is stronger) makes chroma trails outlive luma trails — the
coloured ghost the owner named. Noise reduction: in static areas the noise variance falls by
(1−α)/(1+α).

Precision: the state Ŷ, Ĉ is kept as 16-bit fixed point per component in `R32Uint` words (two words per
pixel). An 8-bit recursion has a dead band: when `(1−α)·|Y − Ŷ| < 0.5` the rounding returns Ŷ unchanged, so
small differences freeze forever (a permanent "ghost" that is a quantization artefact, not the physics);
real ISPs run their TNR at 10–12 bits for this reason.

## D2. Anchors and warm-up — computable from any frame

The recursion runs over MATERIAL frame indices of the event's media (the camera recorded in that order:
playback speed, reverse and trims do not change what was recorded). Anchors lie on a fixed material grid
`A_k = k·G` (G = 32). The state at anchor A is defined as a cold recursion over `[max(0, A − W), A]`
(cold: Ŷ := Y at the first frame). Frames `N ∈ [A, A + G)` continue the recursion from the state at A.

`W = clamp(ceil(ln(1/510) / ln(α)), 1, W_MAX)`, W_MAX = 64 (Q1 a): beyond W the earlier input's weight is
below half a code value, so the state at A differs from the infinite recursion by less than half a code
value; at the anchor change the picture moves by at most that — invisible, and the definition is exact and
playback-independent. A seek walks at most `W + G` frames (fronts + steps); steady play one step.

## D3. The contract

```text
VideoClass::Temporal
TemporalKernel {
    fn warm(&self, p) -> u32;                 // W
    fn grid(&self, p) -> u32;                 // G
    fn pre(&self) -> PassGraph;               // spatial passes on every history frame (the ISP's s00–s08)
    fn step(&self) -> PassGraph;              // In, Prev(state) → State, Out
    fn post(&self) -> PassGraph;              // spatial passes on the shown frame (s09'–s12)
    fn key(&self, p, n) -> u64;               // the rows at the anchor's range + n + anchor
}
```

Pure: State(N) = f(rows, inputs over [anchor-W, N]). The registry refuses a temporal kernel on another class,
a `Temporal` class without one, and `warm == 0` with a non-empty state.

## D4. Hosting

`StreamStage` gains a second form, `Temporal`: its `needs(n)` returns the material frames from the nearest
memo checkpoint (or the warm-up start of `anchor(n)`) to n; checkpoints are the anchor states (kept in the
memo's pinned tier, about two pictures each — the stream-memo-budget sizing); the front (slots before the
camera slot) and the kernel's `pre` run on each needed frame through the front memo on the compute device;
`step` advances the state; `post` runs on the shown frame. The preview's pacing, Loading, kept composites
by `key`, the prefetch's `stream_advance_ahead` and the export's `stream_preroll` apply unchanged — one
machinery (invariant 7). The plan places the temporal slot as a stage inside the chain: slots after it run
in the chain as after a stream stage (`after_chain`).

## D5. Rolling shutter and flicker (Q2 a)

- **Rolling shutter** (`rs_readout_ms`): a temporal kernel with W = 1, G = 1 and no recursion: row y of
  frame N is read at `t = t_N − T_frame + y/H·readout`; the picture of row y is the linear blend of the
  pre-sensor scene at N−1 and N by that time (a skew of moving verticals, the jello on shake). It sits in
  the sensor block (after s01 optics) — the kernel's `pre` is s00–s01 and its `post` the rest.
- **Mains flicker** (`flicker_hz` 50/60, `flicker_depth`): the light at row y is `1 − depth·(1 − |cos(2π·f·t_y)|)`
  with `t_y` the row's readout time from the frame's material time; no history — a spatial factor in s02,
  a function of the frame's time passed as a uniform.

## D6. Rows, presets, view

`camera_isp` rows: `tnr` (flag), `tnr_strength` (0–0.98), `tnr_motion_y`, `tnr_motion_c` (code values),
`tnr_detect_blur` (px), `rs_readout_ms` (0 = global shutter), `flicker_hz`, `flicker_depth`. View: "3DNR
motion map" (m_y as grey, m_c as colour). Preset "Phone, low light, trails": α 0.9, th_y 6, th_c 14. The
ISP window (`src/apps/kt-app/src/isp_panel.rs`) gets the TNR block in its pipeline with the motion map
toggle — the device-like panel, not a slider column (the simulation rule).

## D7. Format

New rows are written only when not default; a file with any of them is written with the next free project
file version by content (an older build would drop the rows on save: the sim-audio-pairs rule). OTIO: rows
travel in the slot's metadata as every effect row; no new row in "What OTIO export loses" beyond the
camera's existing metadata-only status.

## D8. Class walk — "a picture that depends on earlier pictures"

| Place | Mechanism | Here |
|---|---|---|
| Datamosh (stream) | codec history from GOP anchors | same host, codec form |
| 3DNR | recursion from grid anchors | temporal form |
| rolling shutter | two frames | temporal form, W = 1 |
| frame blending (slowed) | two source frames in common code | unchanged |
| VHS tape (tracking, head switching) | per-frame physics with time | no history today; a tape's drift memory would be a temporal kernel — not in scope |
| render-ahead / kept composites | key per moment | the temporal `key` includes the anchor |
| reverse play | material order | the recursion stays in material order (D2) |
| export | `stream_preroll` | walks the same anchors |

## D9. Idle

On a pause the shown frame's walk runs once (≤ W + G steps) and the composite is kept; hovering does
nothing; the live `idle` scene adds a camera ISP with 3DNR on a paused frame (the owner's idle-load rule).

## Rejected

- **The whole camera as a CPU stream stage** (option A of the ISP design) — ~3 fps at 1080p, no GPU, thrown
  away later.
- **Recursion from the event's first frame** — exact for the infinite filter but a seek at minute 10 walks
  15 000 frames.
- **An 8-bit state** — the dead band freezes residues (D1).
