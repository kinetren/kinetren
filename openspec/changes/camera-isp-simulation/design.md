# Design

## Context

The owner wants a camera's sensor and image processor SIMULATED, not imitated (root `CLAUDE.md`,
2026-11-01): the look must come out of the physical chain, the parameters must be physical or ISP
quantities, and wrong settings must produce the artifacts a wrong camera produces. The
reconnaissance is F-FX-010 (`docs/features/fx/simulate/camera-sensor-and-isp.md`): no editor
simulates a camera; the reference chains are ISETCam (scene → optics → sensor → IP) and the
libcamera/Raspberry Pi ISP tuning blocks.

The contract it lands in: a barrier plugin (`kt_plugin_api::barrier`, plugin-api-barriers) — a
CPU kernel as the byte reference, an optional AVX2 twin behind the host's one switch, a GPU pass
graph over Rgba8Uint textures (kt-gpu: no float textures, no samplers, fragment passes only).

## Goals / Non-Goals

**Goals:** every stage of the chain below as a real step; each stage bypassable and mis-tunable;
deterministic noise (invariant 11); CPU and GPU within `FrameTol` (invariant 7); presets that are
only physical settings.

**Non-Goals:** a spectral scene (the frame is RGB); temporal ISP stages (3DNR, AE/AWB loops,
rolling shutter); a raw-file developer (that is Resolve's Camera Raw, F-FX-010); a device-like
"camera body" skin (the panel is shaped like the pipeline; the owner put physics first, p.4).

## The chain (each line is one stage; parameters in brackets)

SCENE
1. Linearisation: the frame's straight RGBA8 is sRGB-decoded to linear light (the scene radiance,
   relative: 1.0 = diffuse white). Alpha is carried from the source to the output untouched.
2. Near infrared: a video frame has no IR band, so the NIR radiance is ESTIMATED:
   `IR = scene_ir · (0.5 R + 0.35 G + 0.15 B)` — foliage and warm light reflect/emit NIR in
   proportion to their red-weighted luminance. [scene_ir]

OPTICS [optics bypass]
3. Natural vignetting, the cos⁴ law: at normalised radius r (1 = the half-diagonal),
   `E(r) = 1/(1 + t²r²)²` with `t² = 2^(ev/2) − 1`, so the corner loses `ev` stops. [vignetting]
4. Lateral chromatic aberration: R is sampled at radius ×(1+c), B at ×(1−c), c = px at the corner
   / half-diagonal; G is the reference. [lateral_ca]
5. The OLPF (a birefringent 4-spot filter): the average of four bilinear samples at (±d/2, ±d/2).
   [olpf]

COLOUR FILTER ARRAY
6. The layout table (codes R, G, B, W, IR) with its period: RGGB/BGGR/GRBG/GBRG (2), Quad Bayer
   (4), RGBW (4, the Kodak CFA2.0 checkerboard), RGB-IR (4, the OmniVision 4×4: IR at odd/odd), X-Trans
   (6, the Fujifilm 6×6 of dcraw). [cfa]
7. Spectral crosstalk (dye overlap + electrical): a colour pixel sees `M · scene` with
   `M_ii = 1 − x`, `M_ij = x/2` (rows sum to 1, so white stays white). [crosstalk]
8. The IR-cut filter: in, no NIR reaches any pixel; out, every pixel adds `qe_ir · T_c · IR`,
   with the dyes' NIR transmissions `T = (R 1.0, G 0.9, B 0.95, W 1.0, IR 1.0)` (the dyes are
   transparent above ~700 nm — F-FX-010 trick 2). An IR pixel sees visible light only through a
   3 % leak. [ir_cut, qe_ir]

SENSOR (in electrons, then DN)
9. Exposure: photons at scene white per pixel `= FW / qe_g · 2^ev` (ev 0: a white G pixel just
   fills the well). Mean signal `μ_c = photons · qe_c · (M·scene)_c` (W: the sum of the three
   bands). [exposure_ev, full_well, qe_r, qe_g, qe_b]
10. Fixed pattern (seeded by the sensor seed, NOT by the frame): PRNU — a gain `1 + prnu·z`;
    DSNU — dark current `max(0, dark + dsnu·z)`; hot pixels (a rate per million) add
    0.2…1.0 FW of dark charge; dead pixels respond with nothing. [prnu, dark_current, dsnu,
    hot_pixels, dead_pixels, sensor_seed]
11. Temporal noise (seeded by the frame's burst index): shot noise `N ~ Poisson(μ + dark)` —
    exact inverse-CDF below 12 e⁻, the normal approximation `round(λ + √λ·z)` above (variance λ
    either way); the well clips at FW; read noise `σ_read · z` e⁻. Off: `N = λ` exactly. [noise,
    read_noise]
12. Analog gain and ADC: `DN = round(black + N · g · (2^bits − 1 − black)/FW)`, clipped to
    `[0, 2^bits − 1]`. [iso, black_level, adc_bits]

ISP — raw front
13. Black-level subtraction: the ISP's black = the pedestal + the dark-current mean in DN (what
    the optical-black rows measure) + a tuning error; `v = (DN − b) / (2^bits − 1 − b)`. Bypass:
    `v = DN / (2^bits − 1)` (lifted, tinted blacks). [blc, black_error]
14. Defect-pixel correction: a pixel further than `thr` beyond the [min, max] of its same-colour
    neighbours (5×5, under the ASSUMED layout) is clamped into that range. [dpc, dpc_threshold]
15. IR subtraction (RGB-IR): the mean of the IR pixels in the 3×3 neighbourhood × `T_c/T_IR` ×
    strength is subtracted from each colour pixel. [ir_sub, ir_sub_strength]
16. Lens shading correction: `× (1 + t²r²)^(2s)` — s = 1 exactly undoes step 3. [lsc,
    lsc_strength]
17. White balance and digital gain: per CFA colour `qe_g/qe_c × error × 2^dg` (W: `qe_g/Σqe`); bypass
    — unity gains (the green cast of a raw). [wb, wb_r_error, wb_b_error, digital_gain_ev]

ISP — demosaic and colour
18. Demosaic under the ASSUMED layout: the sensor's table shifted by the CFA phase error (0/1 px in x
    and y). Bilinear (the mean of same-colour neighbours in 3×3, else 5×5); Malvar–He–Cutler (the
    exact 5×5 kernels on a 2×2 Bayer; on other layouts the same gradient correction generalised:
    `c = mean_c + ½·(v − mean_own)`); edge-directed (each same-colour neighbour weighted by
    `1/(ε + |v(p+s) − v(p−s)|)` along its direction s — Hamilton–Adams/PPG in spirit). IR sites
    and, for colour, W sites are holes. [isp_phase, demosaic]
19. Colour correction matrix: `I + s·(M⁻¹ − I)`, s = 1 the calibrated inverse of step 7, 0 none
    (desaturated), > 1 oversaturated. [ccm, ccm_strength]
20. Tone curve: clip to [0,1], then the sRGB, BT.709 or 2.2 OETF; bypass — linear. [tone, curve]

ISP — YUV
21. RGB → YUV with BT.601 or BT.709 coefficients (full range, 8-bit); luma NR (a 3×3 range-weighted
    mean, weight `1/(1+(ΔY/σ)²)`), chroma NR (a 5×5 luma-guided mean of U,V); sharpening (unsharp
    3×3 on Y). [denoise, luma_nr, chroma_nr, sharpen, sharpen_amount, yuv, matrix]
22. Chroma subsampling: U,V averaged over 2×1 (4:2:2) or 2×2 (4:2:0) sensor blocks (integer mean,
    rounded) and replicated; decoded back to RGB by the decoder's matrix, which may differ (the
    601/709 mismatch). [chroma, decode_matrix]

## Decisions

### D1. One barrier plugin, six passes; the quantization points are the GPU's texture points

GPU intermediates are Rgba8Uint, so a pass boundary is a quantization point. They are placed where a
real camera has a fixed-point bus, and the CPU kernel quantizes at exactly the same points:

| pass | input | output texel | why there |
|---|---|---|---|
| P0 optics | Src | mean signal μ/FW, 16-bit fixed (×16384, 0…4 FW) in R,G | per-pixel samples of the scene |
| P1 sensor | P0 | DN, 16-bit integer in R,G | the ADC's integer |
| P2 raw front | P1 (neighbours) | normalised linear v, 16-bit fixed (×16384, 0…4) | DPC/IR need neighbours |
| P3 demosaic | P2 (5×5), Src | gamma RGB8 + source alpha | demosaic needs neighbours; 8-bit after the OETF |
| P4 YUV | P3 (5×5), Src | Y,U,V 8-bit (or RGB8 when YUV is bypassed) | NR and sharpening need neighbours |
| P5 chroma | P4 (blocks), Src | RGB8 + alpha | the block mean needs the block |

P4 runs only when denoise, sharpening or YUV is on; P5 only with YUV. The 16-bit fixed formats keep
the raw at more than the ADC's 14 bits; the mean is quantized to 1/16384 FW (≤ 0.1 LSB of the
output after the sRGB slope).

### D2. The barrier sees all its rows: `frame_values` / `gpu_values`

`Cooked` is eight numbers; the simulation has ~50 rows. Packing them into eight f64 bit-fields
would quantize automation and hide the meaning. Instead `BarrierKernel` gains two DEFAULTED methods
taking the slot's values (`&[f64]`, passport order); the defaults call `frame`/`gpu` over `cook`, so
every existing barrier is untouched byte for byte, and the host's three call sites (kt-fx
`apply_barrier`, the kt-gpu barrier op, the registry's graph check) call the values forms. The
camera's `frame`/`gpu` (Cooked) answer its defaults — the host never calls them.
Rejected: widening `Cooked` (every point-wise kernel and the GPU tap packing are built around eight
numbers); a parameter texture (a barrier cannot upload data).

### D3. GPU uniforms: per pass, the CFA table packed in the uniform

Each pass has 8 u32 + 8 f32. The CFA table (≤ 36 cells × 3 bits) is packed into four u32 by the
CPU, so WGSL needs no layout code and the ISP's assumed (phase-shifted) table is just another
packing. Coefficients are computed in f64 on the CPU and rounded to f32 once; when a pass needs
more than eight floats the rest travel as `bitcast<f32>` of u32 slots.

### D4. Noise: integer hashes, the same on every device

A random number is `lowbias32(x ^ lowbias32(y ^ S))` of the SENSOR pixel and a stream seed `S`
(the frame's `noise_seed(burst_index)` mixed with a stream constant for temporal noise; the
`sensor_seed` for the fixed pattern). A normal deviate is Irwin–Hall(4) of four 16-bit fields of
two hashes, `(Σ − 131070) × (1/37837.23)` — integer sums, one correctly rounded multiply, so CPU
and GPU agree bit for bit (tails truncated at ±3.46σ: documented, not visible). The Poisson
inverse-CDF below 12 e⁻ uses `exp` (frozen on the CPU, the device's on the GPU): a count may
differ by one electron at a CDF edge on a rare pixel — inside `FrameTol`.

### D5. Precision and math

f32 everywhere on both sides (the GPU's precision; the CPU is the reference of the SAME algorithm).
Transcendentals on the CPU through `kt_plugin_api::math` (`fpowf`, `fexp`; t1066 guards the
family). The sRGB decode on the CPU is a 256-entry table of the frozen `fpowf`, built once.

### D6. K× and the sensor grid

The sensor has the LOGICAL frame's pixel grid. At K > 1 every pass works on sensor pixels
`(x/K, y/K)`: neighbours are read at sensor offsets, and the output is the K=1 picture magnified
by K (a zoom after the sensor shows its pixels, as a real camera's file would). OLPF and CA
distances are in sensor pixels.

### D7. Borders keep the CFA colour

A neighbour outside the frame is shifted by the layout's period, which keeps its colour; the
demosaic never mixes in a wrong colour at the border. YUV passes clamp.

### D8. The SIMD twin is the same rows compiled for AVX2

Every pass is a row loop over a pure per-pixel function. The twin compiles the same functions
inside `#[target_feature(enable = "avx2")]` wrappers: Rust never contracts to FMA nor
re-associates floats, so the bytes are identical by construction; t1053 holds it over the registry
(and t1216 on the presets). The CPU kernel splits rows across scoped threads (each row is
independent: no effect on bytes).

### D9. Cost (measured on the release build, 1920×1080, this machine)

Recorded in tasks 6.x after the measurement: CPU scalar / AVX2 per frame, GPU per frame
(lavapipe and the device). The GPU is the default executor; the CPU is the reference and fallback.

## What is simplified, and why

- RGB scene, not spectral: three band responses (QE, crosstalk) and an estimated NIR; a
  spectral scene needs spectral input, which a video file is not.
- Poisson by the normal approximation above 12 e⁻ (variance exact, skew lost — invisible at
  that count); read noise Irwin–Hall(4).
- No temporal stages (3DNR, AE/AWB control loops), no rolling shutter, no row/column noise, no
  highlight reconstruction: each is a separate effect of time or of the readout, not of a frame.
- The demosaic methods are the classic three; AMaZE/LMMSE/Markesteijn are not ported (their point
  — fewer artifacts — is the opposite of this effect's).
- 4:2:x chroma is replicated (nearest), the encoder's block mean; a bilinear chroma upsampler is a
  decoder choice we do not offer.

## Risks / Trade-offs

- A compressed video frame is already developed; this camera films a developed picture — said in
  F-FX-010 Cons; the presets are tuned on our fixtures.
- ~50 rows: the panel groups them by stage; presets carry the looks.
