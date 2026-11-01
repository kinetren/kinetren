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

The owner's review of 2026-11-01 (`docs/user-feedback.md`) set two rules for this chain: the input
is a DEVELOPED picture, so it is unprocessed first ("ISP is about turning the picture into a raw and
running the ISP again"); and every defect of the lens and the sensor has the ISP block that removes
it ("the paired block … you had to put into the fx"). Each pair below is marked ↔.

SOURCE — the camera that developed the frame, undone (frame pixels)
1. The source's transfer undone: sRGB, BT.709, gamma 2.2 or linear. [src_curve]
2. The source's colour undone: its saturation matrix `S(s) = s·I + (1−s)·1·wᵀ` (w the BT.709 luma)
   inverted, then its white-balance gains divided out → the scene radiance, 0…2. [src_saturation,
   src_wb_r, src_wb_b]
3. Near infrared: a video frame has no IR band, so the NIR radiance is ESTIMATED:
   `IR = scene_ir · (0.5 R + 0.35 G + 0.15 B)`. [scene_ir]

LENS [optics bypass]
4. Defocus (the lens MTF): a 17-tap disc of radius b px. [lens_blur] ↔ sharpening (step 31)
5. Geometric distortion: the sensor point at radius r sees the scene at `r(1 + k r²)` (k > 0
   barrel). [distortion] ↔ dewarp (step 26)
6. Lateral chromatic aberration: R at ×(1+c), B at ×(1−c). [lateral_ca] ↔ CA correction (step 27)
7. The OLPF (a birefringent 4-spot filter). [olpf] ↔ sharpening (step 31)
8. Natural vignetting, cos⁴: `E(r) = 1/(1 + t²r²)²`, `t² = 2^(ev/2) − 1`. [vignetting] ↔ lens
   shading correction (step 22)
9. Colour shading (the micro-lenses' chief ray angle): R × (1 − cs r²), B × (1 + cs r²/2).
   [color_shading] ↔ colour lens shading correction (step 23)

COLOUR FILTER ARRAY AND PIXEL
10. The layout table (codes R, G, B, W, IR): RGGB/BGGR/GRBG/GBRG (2), Quad Bayer (4), RGBW (4, Kodak
    CFA2.0), RGB-IR (4, OmniVision: IR at odd/odd), X-Trans (6, dcraw's). [cfa]
11. Spectral crosstalk: a colour pixel sees `M · scene`, `M_ii = 1 − x`, `M_ij = x/2`. [crosstalk]
    ↔ the CCM (step 29)
12. The IR-cut filter: out, every pixel adds `qe_ir · T_c · IR`, `T = (R 1.0, G 0.9, B 0.95, W 1.0,
    IR 1.0)`; an IR pixel sees visible light through a 3 % leak. [ir_cut, qe_ir] ↔ RGB-IR
    subtraction (step 21)
13. Exposure: photons at scene white `= FW / qe_g · 2^ev`; mean signal `μ_c = photons · qe_c ·
    (M·scene)_c` (W: the sum). [exposure_ev, full_well, qe_r, qe_g, qe_b]
14. Gr/Gb imbalance: a green beside red × (1 + δ), beside blue × (1 − δ). [green_imbalance] ↔
    green equalisation (step 19)
15. The fixed pattern (the sensor seed): PRNU, dark current with DSNU, hot pixels (0.2…1.0 FW of
    dark charge), dead pixels (no charge). [prnu, dark_current, dsnu, hot_pixels, dead_pixels,
    sensor_seed] ↔ BLC (the dark mean, step 17), DPC (step 18)
16. Shot noise (the frame seed): `N ~ Poisson(λ)` — inverse CDF below 12 e⁻, `round(λ + √λ·z)`
    above. [noise] ↔ raw NR (step 20), YUV NR (step 30)

READOUT
16a. Blooming: the charge above the well spills, a share b, into the four neighbours; the well
    clips. [blooming] ↔ highlight handling (step 28), purple-fringe suppression (step 30)
16b. Row noise (temporal, per row) and the column fixed pattern (per column), electrons. [row_noise,
    column_fpn] ↔ the optical-black row/column correction (step 17)
16c. Read noise, analog gain, the ADC: `DN = round(black + N·g·(2^bits − 1 − black)/FW)`, clipped.
    [read_noise, analog_gain, black_level, adc_bits]

ISP — raw front
17. Black-level subtraction (the pedestal + the dark mean the optical-black rows measure + a tuning
    error) and the row/column correction (the OB estimate of each row's and column's offset × a
    strength). [blc, black_error, rowcol, rowcol_strength]
18. Defect-pixel correction: clamped into the [min, max] of its same-colour neighbours (5×5, the
    ASSUMED layout) when further than `thr`. [dpc, dpc_threshold]
19. Green equalisation: a green pixel moves toward the mean of its diagonal greens (the other green
    type) when they differ by less than 0.05. [geq, geq_strength]
20. Raw NR by the noise model: a same-colour 5×5 range-weighted mean whose range is the noise the
    sensor model predicts at the pixel's level, `σ² = cg·v/range + (cg·σ_read/range)²`. [raw_nr,
    raw_nr_strength]
21. IR subtraction (RGB-IR): the 3×3 IR mean × `T_c/T_IR` × strength; the IR pixel's own 3 %
    visible leak goes with it, so a grey stays NEUTRAL but a little darker. [ir_sub, ir_sub_strength]
22. Lens shading correction: `× (1 + t²r²)^(2s)`; the table is calibrated for the lens, so bypassing
    the optics with LSC on over-corrects the corners. [lsc, lsc_strength]
23. Colour lens shading correction: R × (1 − cs r²)^(−s), B × (1 + cs r²/2)^(−s). [color_lsc,
    color_lsc_strength]
24. White balance and digital gain. [wb, wb_r_error, wb_b_error, digital_gain_ev]

ISP — demosaic, lens correction, colour
25. Demosaic under the ASSUMED layout (the CFA phase error): bilinear; Malvar–He–Cutler (exact on a
    2×2 Bayer, the gradient correction generalised elsewhere); edge-directed — on a Bayer
    Hamilton–Adams (green by direction weights, red and blue by colour differences: t1215 238 → 4),
    elsewhere the direction weights over the wanted colour. [isp_phase, demosaic]
26. Dewarp: the captured radius x with `x(1 + k'x²) = r_o` by three Newton steps, k' = k × strength;
    a bilinear resample. [dewarp, dewarp_strength]
27. CA correction: the same solve per channel with `(1 + c'_ch)`. [ca_corr, ca_corr_strength]
28. Highlight handling: within the top 10 % below the level where the first channel clips, the
    channels are pulled to luma. [highlight, highlight_strength]
29. The CCM: `I + s·(M⁻¹ − I)`. [ccm, ccm_strength]
30. Tone curve: sRGB, BT.709 or 2.2. [tone, curve]

ISP — YUV
31. RGB → YUV (BT.601/709); luma NR (3×3 range-weighted); chroma NR (5×5 luma-guided);
    false-colour suppression (chroma × (1 − s·|∇Y|·4) at luma edges); purple-fringe suppression
    (blue-and-red above luma within 2 px of a highlight); sharpening (unsharp 3×3). [denoise,
    luma_nr, chroma_nr, fcs, purple, sharpen, …]
32. Chroma subsampling 4:2:2/4:2:0 (block mean) and the decoder (its matrix may differ). [yuv,
    matrix, chroma, decode_matrix]

VIEW
33. `view` shows the picture after any stage: the scene, the light through the lens and the pixel
    charge (in the sensor's filter colours), the raw, the raw after BLC/DPC, after raw NR, after
    LSC/WB (in the ISP's filter colours), the demosaic, the lens correction, luma, chroma.

NOT HERE — the temporal stages, moved to their own change `temporal-barrier-3dnr`. A temporal NR
(3DNR: the previous OUTPUT blended in where the motion detector sees no motion; its errors leave
the trails behind moving objects, luma and chroma apart), the rolling-shutter skew and mains flicker
need the frames before N. The owner chose the real recursive filter and asked for the GPU
("п.1 … почему нельзя ускорить через GPU хоть что-то", 2026-11-01). The stream class of
`datamosh-codec-path` does not carry it: it is CPU-only by a registry refusal and a SOURCE stage run
before the chain, while 3DNR sits inside the ISP after demosaic. The options were (A) the whole camera
as a CPU stream stage when 3DNR is on — faithful trails but ~3 fps at 1080p and no GPU; (B) a new
core contract, a TEMPORAL BARRIER: the host hands a barrier its own slot's previous OUTPUT (N−1) on
both executors, with a GPU texture memo and anchor replay on a fixed grid, the CPU the byte
reference; (C) A now, B later. The orchestrator decided B as its own change (A would be a CPU path
thrown away). A contracting filter (α < 1) lets the GPU run it, unlike a codec, whose differences
grow.

## Decisions

### D1. One barrier plugin, up to twelve passes; the quantization points are the GPU's texture points

GPU intermediates are Rgba8Uint, so a pass boundary is a quantization point. They are placed where a
real camera has a bus, and the CPU kernel quantizes at exactly the same points:

| pass | input | output texel | why there |
|---|---|---|---|
| U unprocess | Src (frame px) | the scene, R/G 11 bits, B 10, `√(v/2)` coded | the defocus disc needs the neighbours |
| P0 lens and photons | U (bilinear) | μ/FW, 16-bit fixed | distortion/CA/OLPF sample anywhere |
| P1a pixel | P0 | charge/FW, 16-bit fixed, unclipped | the readout spills the excess |
| P1b readout | P1a (4 neighbours) | DN, 16-bit | blooming needs the neighbours; the ADC's integer |
| P2a raw front | P1b (5×5) | normalised raw, 16-bit fixed | DPC, GEQ need neighbours |
| P2n raw NR (if on) | P2a (5×5) | normalised raw | a denoiser needs neighbours |
| P2b raw correct | P2a/P2n (3×3) | linear raw after LSC/WB | IR subtraction needs the IR neighbours |
| P3 demosaic | P2b (5×5), Src | gamma RGB8 + alpha | the demosaic needs neighbours; 8 bits after the OETF |
| P3b lens correct (if any) | P3 (bilinear) | RGB8 + alpha | a resample reads anywhere |
| P4 YUV (if any) | P3/P3b (5×5) | Y,U,V 8-bit (or RGB8) | NR, FCS, purple, sharpening need neighbours |
| P5 chroma (if YUV) | P4 (blocks) | RGB8 + alpha | the block mean needs the block |
| PV view (if a raw stage is viewed) | the stage | RGB8 + alpha | the stage's texel made a picture |

`Plan::passes` is the one list both executors run; `Plan::uniforms` the one packing.

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

Measured 2026-11-01 on this machine (32 threads, the load of four parallel executors around it,
so an upper bound): `cargo test --release -p fx-video-camera --test cost -- --ignored`, the CPU
kernel on 8 row threads — the defaults 386 ms scalar / 334 ms AVX2 twin; the presets 338–437 ms
with the twin, "cheap webcam" 786 ms (its defocus disc reads 17 taps per frame pixel). The GPU
graph (`cargo test -p kt-gpu --test camera_isp -- --ignored`): 135 ms per frame with the upload
and the readback, the defaults (twelve passes at most; a memoized preview frame is not re-run).
The GPU is the default executor; the CPU is the byte reference and the fallback — a
fallback at 2–3 frames a second, which is why the twin and the row threads exist. Not a real-time
effect on the CPU; on the GPU a preview at reduced quality plays.

## What is simplified, and why

- RGB scene, not spectral: three band responses (QE, crosstalk) and an estimated NIR; a
  spectral scene needs spectral input, which a video file is not.
- Poisson by the normal approximation above 12 e⁻ (variance exact, skew lost — invisible at
  that count); read noise Irwin–Hall(4).
- The temporal stages (3DNR, rolling shutter, mains flicker) wait for the stream stage (see "NOT
  HERE"); the AE/AWB control loops are not simulated (the exposure and the gains are rows), and
  highlight RECONSTRUCTION (inventing the clipped channel) is not done — the ISP desaturates.
- The unprocessing undoes a saturation matrix and white-balance gains, not a source camera's full
  CCM and local tone mapping: a picture's development is not invertible beyond what it kept.
- Raw NR and YUV NR are range-weighted means, not BM3D-class denoisers; their trade-off (smearing
  versus residual noise) is the one cheap ISPs make.
- The demosaic methods are the classic three; AMaZE/LMMSE/Markesteijn are not ported (their point
  — fewer artifacts — is the opposite of this effect's).
- 4:2:x chroma is replicated (nearest), the encoder's block mean; a bilinear chroma upsampler is a
  decoder choice we do not offer.

## Risks / Trade-offs

- A compressed video frame is already developed; the unprocessing returns an ESTIMATE of the scene
  (F-FX-010 Cons); the presets are tuned on our fixtures.
- ~85 rows: the window draws the chain as blocks of stages, each correcting stage naming the defect
  it removes; the presets carry the looks; `view` shows any stage.
