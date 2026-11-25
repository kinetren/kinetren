# Design

## Context

Read on 36d751a7: a barrier plugin is passes in WGSL over `common.wgsl` (`BarrierKernel::gpu`, `Node::Pass`, `Resample`,
`Premul`); a positioned tap gets its stage coordinate and `TapCtx.burst_index` (`tap_vignette`, `tap_noise` with the
counter hash `noise_hash`); `t328`/`t329` cycle the whole registry against stored frames and between executors; `t532`
requires a curve to change the picture. There is no CPU kernel to write. `glow` (threshold, radius, tint) lives in
`fx-video-blur`.

## Decisions

### D1. `glitch_blocks` (barrier, one pass)
Rows: `amount` 0–1 (share of frames with a glitch), `block` 4–256 px (a logical size, × `res_scale`), `shift` 0–0.25 of
the width, `rgb` 0–0.1 (channel offset), `hold` 1–30 frames, `seed`. A frame is glitched by `hash(burst / hold, seed) <
amount`; in it each horizontal band of `block` takes a displacement `hash(band, epoch) · shift` for a random half of the
bands, channels take ±`rgb`. Pure function of (burst index, seed, rows); outside a glitch frame the graph is EMPTY (a
passthrough, no pass), so a still look costs nothing. `amount` 0 = empty always. Reads clamp at the edge.

### D2. `film_damage` (barrier, one pass; `weave` uses the same pass's sampling)
Rows: `scratches` (0–8 per frame), `dust` (0–1), `flicker` (0–0.5 of the luma), `weave` (0–0.02 of the height), `jump`
(rate of a frame's vertical slip), `seed`. Scratch positions, brightness and per-frame life come from hashes of (burst /
6, i, seed); dust is hashed cells of the stage grid (K×: the logical cell); the weave is a sub-pixel offset of the
bilinear sample by hash(burst, seed) low-passed over 5 frames by the sum of hashes (no state). Alpha preserved. At all
zeros the graph is empty.

### D3. `lens_blur` (barrier, one pass of 64 taps)
A golden-angle spiral disc of radius `radius` 0–64 (× `res_scale`), `highlights` 0–4 weights a tap by its luma above
`threshold`, `shape` (0 disc, 1 hexagon by a cosine clamp of the radius) and `squeeze` for anamorphic. Premultiplied on
alpha. The 64 taps are fixed, so the cost is bounded (about 0.4 ms at 1080p by `radial_blur`'s 16-tap measure; to be
measured in the trace). Radius 0 — empty graph. Radius over 64 px of a 4K frame is a K× matter: the decode's scale step
handles it as for `blur`.

### D4. `light_leak` (positioned tap)
Three blobs whose centre, radius and colour are sines of the burst seconds with fixed phases from `seed`; added over the
picture (`rows`: `amount`, `warmth`, `speed`, `size`, `seed`). `positioned: true`, coordinate of its stage, `cook_at`
gets the burst. Identity at `amount` 0. Pure: same burst, same picture.

### D5. Modes and variants instead of kinds
- `radial_blur`: new row `directional` (Flag, default off, `omit_default`) and `angle` (Deg, default 0): with it on,
  the 16 taps go along `angle` by `amount · 0.1` of the short side. `t328`'s stored frames of the former two modes stand.
- `glow` variant "Halation" (threshold 190, tint 255/90/40, radius 12); `duotone` "Sepia"; `noise` "Movie grain"
  (amount 0.12, size 2) — after the Noise default change (found-defects 14); `color_grade` "Cinematic" (contrast 0.2, saturation
  0.85, warmth, a slight black lift). Variants are `UiVariant` rows, the distinguishability guard covers them.

### D6. Motion and idle cost
None of them keeps state; a kind that changes with time (`light_leak`, `film_damage`, active `glitch_blocks`) makes an
event's memoised frames distinct per frame, as VHS does; a pause costs nothing (no wake).

### D7. Class walk
The other looks of the wave list looked at: gate weave (inside `film_damage`), shutter halation (variant), sharpen glow
(exists), old TV (VHS exists), chromatic aberration (exists). Each `burst_index` reader goes through `noise_hash`, so
export and preview agree; a slowed event's `burst_index` is the timeline's, as for noise.

## Risks / Trade-offs

- [`lens_blur`'s 64 taps on a weak GPU] → the measure decides 48 or 64 before the stored frames are written.
- [Halation as a variant has no separate radius per channel] → a variant, its limits are glow's.
