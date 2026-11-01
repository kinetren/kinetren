# Spec Delta

## ADDED Requirements

### Requirement: The camera effect simulates a sensor and an ISP, stage by stage
The built-in barrier plugin `camera_isp` (family `src/plugins/fx-video-camera`) SHALL take the
frame as a picture another camera DEVELOPED, SHALL unprocess it into a scene in linear light (the
source's transfer curve, saturation matrix and white balance undone), and SHALL produce the picture
a simulated camera records of that scene: the lens (defocus, distortion, lateral CA, cos⁴
vignetting, colour shading, the 4-spot OLPF); a colour filter array of the layouts RGGB, BGGR,
GRBG, GBRG, Quad Bayer, X-Trans, RGBW and RGB-IR, with spectral crosstalk, Gr/Gb imbalance and an
IR-cut filter whose absence lets the estimated near-infrared into every pixel through the dyes' NIR
transmission; a sensor in electrons (quantum efficiency per channel, full well, Poisson shot noise,
dark current with DSNU, PRNU, hot and dead pixels, blooming into the neighbours, row noise and a
column fixed pattern, read noise, analog gain, black pedestal, ADC depth and clipping); and an ISP.
EVERY defect of the lens and the sensor SHALL have the ISP block that real cameras use to remove
it, and each block SHALL be bypassable and mis-tunable: black-level subtraction (the pedestal and
the dark mean, with a tuning error), the optical-black row/column correction, defect-pixel
correction, green equalisation, raw noise reduction by the sensor's noise model, RGB-IR
subtraction, lens shading and colour lens shading correction, white balance and digital gain,
demosaic (bilinear, Malvar–He–Cutler or edge-directed) under an assumed CFA phase, dewarp and
lateral-CA correction, highlight handling, the colour correction matrix, a tone curve, YUV denoise,
false-colour and purple-fringe suppression, sharpening, BT.601/709 YUV with 4:4:4/4:2:2/4:2:0
chroma and a decoder matrix. A `view` row SHALL show the picture after any stage (the scene, the
light through the lens, the pixel charge, the raw, the raw after each correction, the demosaic,
the lens correction, luma, chroma). The picker SHALL offer the presets phone at night, cheap
webcam, security camera at night (IR), cinema raw, wrong CFA phase and no IR-cut filter, each only
a set of the passport's rows.
Why: the owner's request I1 of kkt-26.11.01 and his rule of 2026-11-01 — an imitation of a
physical effect is not a simulation of it; and his review of the same day: "you added the noise but
not the denoiser … the PAIRED block of optics compensation simply does not exist … what is hidden
in real ISPs, the distortions and the ways to remove them, you had to put into the fx" (F-FX-010).
The temporal stages (3DNR with its trails, rolling shutter, mains flicker) need the frame history
of a stream stage and are not part of this requirement.

#### Scenario: A clean chain reproduces the input
- **WHEN** a smooth colour frame passes `camera_isp` on every layout and demosaic, with no temporal
  noise, no fixed pattern, no defects, optics flat, exposure below clipping, the calibrated WB and
  CCM, the sRGB curve, no denoise or sharpening and 4:4:4
- **THEN** away from the frame border a 2×2 Bayer gives the input within 1 LSB on average and 3 on
  every pixel, the other mosaics (a generic interpolation) within 2 on average and 12 on every
  pixel; flat patches are within 1 LSB on every layout
- **TEST** t1211 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: Shot noise variance grows with the signal
- **WHEN** flat grey frames of four levels are filmed with read noise 0 and no fixed pattern, tone,
  WB and CCM bypassed, and the raw DN of the G pixels is read
- **THEN** the DN variance divided by the mean signal in DN equals the conversion gain within 10 %
  at every level (variance ∝ signal), and with read noise σ the variance at a dark level is the
  shot variance plus σ²·g²
- **TEST** t1212 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: A wrong CFA phase swaps colours as predicted
- **WHEN** a red, a green and a blue patch are filmed by an RGGB sensor without noise, and the ISP
  assumes the phase shifted by one pixel in x and y (BGGR), then in x only (GRBG)
- **THEN** with the x-and-y shift red comes out blue and blue red while green stays green; with
  the x shift green comes out magenta (R and B high, G low) and red and blue come out green
- **TEST** t1213 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: An infrared leak tints as predicted
- **WHEN** a mid-grey frame is filmed with scene NIR, the IR-cut filter out and the visible WB
  calibration, on an RGGB sensor; then on an RGB-IR sensor with the ISP's IR subtraction on
- **THEN** the RGGB output's linear R, G and B are grey plus `qe_ir·T_c·IR/qe_c` each — R and B
  above G, the magenta cast — within 3 %; with the IR-cut filter in it is grey; the RGB-IR output
  with subtraction is neutral (its channels within 3 LSB of each other)
- **TEST** t1214 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The demosaic zipper on a known edge
- **WHEN** a vertical black/white edge is filmed by an RGGB sensor without noise and developed by
  the bilinear, Malvar–He–Cutler and edge-directed demosaics
- **THEN** the bilinear output alternates colour along the edge with a two-row period (the zipper)
  by more than 20 LSB between neighbouring rows, and the edge-directed output (green by gradients,
  red and blue by colour differences) alternates at most half as much
- **TEST** t1215 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The noise is deterministic, the fixed pattern is fixed
- **WHEN** a preset is rendered twice at the same timeline position, once at another, with and
  without the SIMD twin, and once at K=2
- **THEN** the same position gives the same bytes on every path; another position changes the
  temporal noise but not the hot pixels; the K=2 frame is the K=1 frame magnified by 2
- **TEST** t1216 (`src/plugins/fx-video-camera/tests/consequences.rs`), t1053

#### Scenario: The GPU graph agrees with the CPU kernel
- **WHEN** every preset and the defaults run on the GPU executor and on the CPU, on a frame with
  and without alpha
- **THEN** each pair is within `FrameTol::THRESHOLD` and the alpha is the source's
- **TEST** t1217 (`src/libs/kt-gpu/tests/camera_isp.rs`), t1052

#### Scenario: Each layout is the camera's mosaic
- **WHEN** the layout tables are read
- **THEN** each has its period and colour fractions (Bayer ½ G, ¼ R, ¼ B; Quad Bayer the same in
  2×2 blocks; X-Trans 20/36 G, 8/36 R, 8/36 B; RGBW ½ W; RGB-IR ¼ IR, ½ G), and every preset sets
  only passport rows within their ranges
- **TEST** t1218 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The effect window is the pipeline
- **WHEN** the person adds the camera effect to a clip and opens its window
- **THEN** the window shows the blocks in the order of the chain, each stage with its bypass and
  its rows; every passport row has its place; switching a stage's bypass reaches the model and
  joins the window's one undo step
- **PATH** select a clip → the effect picker "+ effect…" → type "camera" → click "Camera ISP" (the
  window shows the blocks Source, Lens, Sensor, ISP: raw, ISP: demosaic, ISP: lens correction,
  ISP: colour, ISP: YUV; each correcting stage says which defect it removes) → turn the wheel down
  over the window to the ISP: raw block → untick "White balance" (the preview turns green)
- **TEST** t1219 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Every defect has its correcting block
- **WHEN** a smooth pattern is filmed by a camera with an ideal lens, then with one defect at a time
  — distortion, lateral CA, vignetting, colour shading, Gr/Gb imbalance, a column fixed pattern, hot
  pixels — once with its ISP block calibrated and once with the block off
- **THEN** without the block the picture differs visibly from the ideal camera's; with the block the
  difference falls below half (for hot pixels: at most a quarter of the lit pixels remain)
- **TEST** t1220 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The view shows any stage
- **WHEN** a grey frame is shown at every value of `view`
- **THEN** the scene of an sRGB source is the input within 1 LSB; the raw is a mosaic in its filter
  colours (a red site red only, a green green only, a blue blue only); every raw-domain view
  differs from the output
- **TEST** t1221 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The frame is unprocessed into the scene
- **WHEN** a frame developed with a saturation of 1.4 and a warm white balance is unprocessed with
  exactly those settings, and once without unprocessing
- **THEN** the unprocessed scene is the plain frame's scene within 2.5 LSB on average; without
  unprocessing it keeps the source's look (at least three times further)
- **TEST** t1222 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: Raw NR follows the noise model; blooming spills
- **WHEN** a noisy two-level frame is developed with raw NR off and on; and one overexposed spot is
  filmed with blooming 0 and 1
- **THEN** raw NR at least halves the noise variance of the flat part and keeps the edge step within
  12 LSB; with blooming the raw of the spot's neighbour rises by more than 200 DN
- **TEST** t1223 (`src/plugins/fx-video-camera/tests/consequences.rs`)
