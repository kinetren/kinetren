# Design: camera-isp-edges

## Context

Found on the README footage run (2026-11-03). The cause was located with the effect's own `view`
row on an edge-ramp frame: with "Phone at night" the band appears first at "lens_corrected"
(P3b); "demosaic" before it is clean. With a barrel lens the band appears already at "optics" (P0).
Both stages sampled through `Ctx::at`/`at_frame`, which clamp to the edge texel; the clamped
sample is the same texel for every pixel of a band, so the band is that texel smeared along the
normal. The kernels with a few-pixel support (OLPF, defocus, DPC, NR, demosaic, YUV NR, sharpening)
also clamp or period-wrap (D7) at the border, but a 2–5 px neighbourhood with its own centre
sample never yields a band of copies; they are not the defect.

## D1. The lens's field is fitted to the recorded scene

What a real lens does at the edge: it maps field angles onto the image plane; its distortion
`r·(1 + k r²)` decides where each angle lands, and the world continues past the sensor's field in
every direction. The simulation's world is the frame: nothing past its edge was recorded. A
sensor ray that would land outside it has nothing to see — clamping invents a smear, black
invents a hood that the lens does not have, a mirror invents a world.

So the simulated camera is framed on the scene the way a camera re-filming a print is: its focal
length is the one whose field is the largest that lies inside the recorded frame. Over the
sensor's border r² runs from m² = (short half-side / half-diagonal)² (the long edges' midpoints) to
1 (the corners), so the widest reach of the map is

    fit = (1 + max(k·m², k)) · (1 + |ca|)

and the scene is sampled at `r·(1 + k r²)(1 + ca_ch) / fit`. A barrel lens (k > 0) is limited by
its corners, a pincushion (k < 0) by its long edges' midpoints, lateral CA by its widest channel.
An ideal lens has fit = 1: the frame is filmed as before, byte for byte (the defaults' t328 hash).

Rejected: a "diagonal field preserved" normalisation `/(1 + k)` (the common convention): a
pincushion then reaches 1.5–4.9 % past the long edges — the band again. Including the OLPF and
defocus radius in the fit: the framing would breathe with an animated blur; their taps reach at
most one sensor pixel / the defocus radius past the edge, which is the frame's own last sample
area (no band).

## D2. The ISP crops its lens correction to the sensor

What a real ISP does at the edge: its dewarp maps each output pixel to the captured position; where
that position is off the sensor there is no data, and the ISP crops — the output is scaled so that
it covers only the region the corrected sensor image fills (OpenCV's `getOptimalNewCameraMatrix`
with α = 0; a phone's video mode, whose dewarp and EIS use the margin and the rest is cropped).
The output pixel at radius r reads the captured radius x with `x(1 + k x²)(1 + ca_ch) = r·crop`;
over the border it stays inside while

    crop = (1 + min(k·m², k)) · (1 − |ca|)     (floored at 0.05 for a folded, over-strong model)

A pincushion correction (k < 0) crops in by its corners, a barrel correction (k > 0) may widen to
the long edges' midpoints; the CA correction crops by its widest channel. The ISP knows only ITS
model (`k_corr`, `ca_corr`): a mis-tuned correction still never leaves the sensor.

Rejected: a sensor larger than the output (a margin, as many phones have). The sensor has the
frame's grid (D6 of the camera change) and the barrier contract hands every pass a frame-sized
plane; a margin needs larger planes in both executors — and the margin's own rays would need scene
past the frame, which is D1's problem again.

## D3. Correcting distortion costs field — t1220's reference

With D1 and D2 a calibrated camera's output is a crop of the scene by crop/fit: "Phone at night"
0.954, a 0.2 barrel on t1220's 96×64 frame 0.885. That is the physics of a sensor no larger than
its output, and real phones show it (the corrected video is narrower than the raw). t1220 compared
the corrected picture with the ideal camera at the full field and passed only because its 6 px
margin hid the smear; it now compares each output with the ideal camera filming the same analytic
pattern at its own paraxial magnification — crop/fit with the block, 1/fit without it — so the
pair still measures what the block removes (the warp, the colour misregistration), not the
framing. The assertion (with < half of without) is unchanged; the measured pairs got cleaner
(distortion 1.18 against 11.61, lateral CA 1.13 against 7.05).

## D4. The test (t1430)

A frame whose brightness is a sawtooth along each axis (2 LSB/px from every edge, period 40 px):
any band along an edge changes along the normal however the camera frames it. The profile is the
mean along the middle third of each edge; a smear is flat. Window 4 px and 1.5 LSB, because the
camera's own response is legitimately stepped: Quad Bayer resolves a colour in 2×2 blocks and its
Gr/Gb imbalance repeats every 4 px, inside the frame as much as at its edge. Row noise and the
column FPN are off: an offset per row or column survives the average and is not a sampling defect.
Distortion is limited to |k| ≤ 0.3 in the extreme forms: below −1/3 the pincushion map folds
(`1 + 3k r² = 0` inside the frame) and a fold is flat by optics. On the old kernel the test is red
on "Phone at night" (10 px of equal profile at the left edge).
