# Design

## D1. A frame is a function of the material time

Each kind is `live`: the frame is drawn from its rows and `phase = material seconds × speed` (the
preamble's one float), so a backdrop under a cut repeats exactly and two moments draw the same bytes
(t3707). The pictures are WGSL `pixel` texts on the compute device, as the shapes, gradients and noise
are (gpu-render-hardening D7): f32 pixels, held to the software adapter's hashes by t1048.

## D2. The four looks

- **Moving gradient:** three Gaussian blobs on Lissajous orbits, colours blended by their weights — the
  "mesh gradient" look; `softness` is the blobs' width.
- **Plasma:** four travelling sines summed, walked round a palette of three colours with smoothstep, so
  no colour band is hard.
- **Particles:** a hashed field (`lowbias32` of seed and index) of discs drifting up at their own pace,
  wrapped so the field never empties; additive glow over the backdrop, capped.
- **Pattern:** a rotated cell lattice; stripes, dots, a grid or waves by a signed distance and a soft
  edge in cells, so the picture does not depend on the preview's size.

## D3. Styles are not here

The owner asked for styles per kind. A creation-menu row is named by a row's value (`GenDecl.names`),
and a "style" row only for a name would be a dead control (rule 9). Style presets need a generator
preset store, as the title presets and the effect presets have — the next step, named in the report.

## Class walk

The mechanism: a backdrop that moves only when keyframed, and looks of the 90s. Checked: the linear and
four-corner gradients (static; their motion is curves — kept), the noise field (moves, grain only), the
checkerboard (static). The new four move by themselves; the old ones are unchanged (t1048).
