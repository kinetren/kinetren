---
id: F-FX-010
title: Camera sensor and ISP — a simulated mosaic, its noise and the processor that develops it
status: in-progress
priority: B
cost: L
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-plugin-api, kt-fx, kt-gpu, kt-app]
invariants: [7, 11, 12]
ui_rules: []
round: kkt-26.11.01
point: I1
tests: []
depends_on: [F-FX-007]
trackers: []
services: []
checked: 2026-11-01
---

# F-FX-010 — Camera sensor and ISP — a simulated mosaic, its noise and the processor that develops it

## Essence

The frame on the timeline is taken as the SCENE, and a camera films it: a lens, a colour filter
array over the pixels (Bayer RGGB and its phases, Quad Bayer, X-Trans, RGBW, RGB-IR with infrared
pixels), the pixel's photo-electrons with their shot noise, the read noise, the black level and the
ADC, then the camera's image processor develops the mosaic back into a picture: black-level
subtraction, defect-pixel correction, lens shading, white balance, demosaic, colour matrix, tone
curve, sharpening, YUV and chroma subsampling. Every stage can be bypassed or mis-tuned, and the
look of a phone at night, a cheap webcam or a security camera with its IR-cut filter out is the
CONSEQUENCE of those settings, not a filter painted to look like them (the owner's rule of
2026-11-01: an imitation is not a simulation).

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | no | "Add Noise", "Film Effects": noise and grain painted over the finished frame; no mosaic, no ISP |
| Premiere | 26 | no | "Noise", third-party grain (Film Impact, FilmConvert): the grain is an overlay with an intensity knob |
| DaVinci Resolve | 21 | Camera Raw panel; Film Grain | the Camera Raw panel DEVELOPS real raw files (ISO, white balance, highlight recovery) — it never builds a mosaic from an RGB frame; Film Grain is an overlay |
| Final Cut Pro | 12.3 | no | "Noise", "Film Grain"-like looks: overlays |
| After Effects | 26 | Add Grain / Match Grain | Match Grain measures the noise of a sample and paints a statistically similar grain; not signal-dependent per channel through a mosaic |

**Outside the core.** The mechanism exists only in imaging science and camera firmware; without
these programs neither the chain nor the meaning of its parameters can be described.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| ISETCam (Stanford, Wandell) | 2024 (MATLAB R2024a pages) | scene → optical image → sensor → image processor | a spectral scene; the sensor turns irradiance into electrons with photon (Poisson) noise, read noise, DSNU, PRNU, conversion gain, analog gain and quantization; the IP demosaics, balances and renders |
| libcamera (Raspberry Pi tuning) | 0.5 (2026) | IPA algorithms in a tuning JSON | `rpi.black_level`, `rpi.dpc`, `rpi.alsc` (lens shading), `rpi.awb`, `rpi.noise`/`rpi.sdn` (denoise), `rpi.ccm`, `rpi.contrast` (tone), `rpi.sharpen`; each is tuned per sensor model |
| RawTherapee / darktable | 5.11 / 5.2 | demosaic module | bilinear, VNG4, AMaZE, RCD, LMMSE for Bayer; Markesteijn 1/3-pass for X-Trans; the method is a choice because each leaves different artifacts |

## The trick behind it

1. **Noise is decided in ELECTRONS, before the gain, and is then shaped by the whole chain.** The
   shot noise variance equals the signal in electrons (Poisson), the read noise is a floor in
   electrons, the analog gain (ISO) amplifies both before the ADC clips. Then white balance
   multiplies R and B noise by its gains, demosaic correlates it into colour blotches, the tone
   curve lifts it in the shadows. That is why an "add grain" overlay never looks like a phone at
   night: real noise is coloured, signal-dependent and spatially structured by the CFA.
2. **The CFA dyes are transparent in the near infrared.** Without the IR-cut filter all three
   channels receive roughly the same IR signal; white balance, tuned for visible light, then
   multiplies R and B by their (larger) gains, so a grey wall goes magenta and foliage glows.
   RGB-IR sensors (a 4×4 pattern with IR pixels in place of half the blue/red) measure the IR and
   subtract it per channel — the subtraction is a calibrated coefficient, and a wrong one tints.
3. **The ISP must know the mosaic's PHASE.** The same raw read with the pattern shifted by one
   pixel swaps the channels predictably: RGGB read as BGGR swaps red and blue, read as GRBG turns
   green into magenta. Demosaic artifacts — the zipper on an edge with bilinear, the maze of Quad
   Bayer through a Bayer demosaic, the X-Trans "worms" — are the method's fingerprint.

## Pros

- The looks are consequences of physical numbers: a phone at night is a small full well, high
  analog gain and strong denoise; a cheap webcam is a bilinear demosaic, a wrong CCM and 4:2:0.
- Mis-tuning teaches: a wrong black level lifts or crushes blacks with a colour cast, the wrong CFA
  phase swaps colours exactly as a real broken pipeline does.
- The noise is deterministic per frame (seeded by the timeline position), so an export repeats it.

## Cons

- The scene is an RGB frame, not a spectrum: the CFA responses are three numbers, and the near
  infrared is ESTIMATED from the RGB of the frame (there is no IR band in a video file).
- A real sensor works on linear light; a compressed video frame is already developed by another
  camera, so the effect develops a picture twice — the result is "this camera filming a screen".
- Many parameters; a person needs presets and a panel shaped like the pipeline.

## What users say

> "Unfortunately, the captured images are completely pink!"
> — notprime, NVIDIA Developer Forums, 2021-04-12, https://forums.developer.nvidia.com/t/noir-cameras-record-pink-images/174978

> "The problem was a bad tuning of the camera, I solved it by downloading the tuning profile"
> — notprime, same thread, 2021-04-15 (a NoIR sensor: the colour comes from the ISP's tuning, not from the lens)

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-fx/ — planned in `openspec/changes/camera-isp-simulation/`.

### What already exists

The barrier contract `src/libs/kt-plugin-api/src/barrier.rs` (a CPU kernel as the byte reference,
an optional AVX2 twin behind the host's switch, a GPU pass graph over Rgba8Uint textures) and the
frozen math `src/libs/kt-plugin-api/src/math.rs` for transcendental functions on frozen paths. The
counter hash `noise_hash` seeds the grain of `noise` and `vhs` (`src/plugins/fx-video-stylize`).

### What is missing

A kernel that holds a mosaic, a sensor model in electrons and an ISP chain; a barrier whose
parameters exceed the eight cooked numbers of `Cooked`.

### What hinders

GPU intermediates are Rgba8Uint (invariant 7's determinism rule): a 14-bit raw must be packed into
two bytes between passes, and the CPU must quantize at the same points to stay within `FrameTol`.

### Which invariants it touches

7 (one composition path, CPU reference and GPU within tolerance), 11 (determinism: the noise is a
counter hash of position and frame, transcendental functions from `kt_plugin_api::math`), 12 (no
allocation per frame: the kernel's work buffers are reused).

## Estimate

| What | Answer |
|---|---|
| Scope | L — a whole change: note, design, kernel ×2 (CPU/GPU), panel, tests |
| Where the main work is | a new family crate `src/plugins/fx-video-camera`, its WGSL passes |
| Project format | does not touch (a new plugin id, rows in the passport) |
| Tests that will have to be written | clean chain ≈ input; variance ∝ signal; CFA phase swap; IR tint; zipper; determinism; CPU↔GPU |
| Depends on | the barrier contract (plugin-api-barriers) |
| Breaks compatibility | no |

## Verdict

Take (owner's request I1 of kkt-26.11.01, 2026-11-01): a SIMULATION of the sensor and ISP chain,
physics first; looks come as presets of physical settings.

## Sources

- https://web.stanford.edu/~wandell/data/isetcam/introduction/t_introduction2ISET.html (access date 2026-11-01)
- https://web.stanford.edu/~wandell/data/isetcam/sensor/s_sensorSNR.html (access date 2026-11-01)
- https://web.stanford.edu/~wandell/data/isetcam/sensor/s_sensorSpatialNoiseDSNU.html (access date 2026-11-01)
- https://forums.developer.nvidia.com/t/noir-cameras-record-pink-images/174978 (access date 2026-11-01)
- https://www.adimec.com/implementing-a-visible-camera-for-both-daylight-and-lowlight-vision/ (access date 2026-11-01)
- https://git.libcamera.org/libcamera/libcamera.git/tree/src/ipa/rpi/controller (access date 2026-11-01)
- H. S. Malvar, L. He, R. Cutler, "High-quality linear interpolation for demosaicing of Bayer-patterned color images", ICASSP 2004
