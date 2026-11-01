# Proposal

## Why

The owner's request I1 of `kkt-26.11.01` (line 45 of `docs/ktprompts/kkt-26.11.01.txt`, translated):
"A real physical effect of a camera sensor's ISP — the whole path from RGB → Bayer → all the
calibrations, conversions, denoisers and the rest an ISP has → YUV. And do not forget RGGB and the
other input layouts, and even +IR pixels." His answer of 2026-11-01 (p.4, `docs/user-feedback.md`):
a "real" effect is first a PHYSICS problem — the design starts from the physical and signal chain,
the interface is secondary. The root rule of the same day: an imitation of a physical effect is
not a simulation of it. Nothing in the catalogue simulates a camera: `noise` paints grain over the
finished frame (F-FX-010).

## What Changes

- A new built-in video effect `camera_isp` in a new family crate `src/plugins/fx-video-camera`:
  the frame is the SCENE (sRGB-decoded to linear light), filmed by a simulated sensor and developed
  by a simulated ISP. Each stage is a real step with physical or ISP parameters and can be bypassed
  or mis-tuned:
  - optics: natural vignetting (cos⁴ law), lateral chromatic aberration, the 4-spot OLPF;
  - the colour filter array: RGGB/BGGR/GRBG/GBRG, Quad Bayer, X-Trans, RGBW, RGB-IR (4×4 with IR
    pixels); spectral crosstalk; the near-infrared estimated from the scene and the IR-cut filter;
  - the sensor in electrons: quantum efficiency per channel, full well, photon shot noise
    (Poisson), dark current with DSNU, PRNU, hot and dead pixels, read noise, analog gain, black
    pedestal, ADC bit depth and clipping;
  - the ISP: black-level subtraction (optical-black based, with a tuning error), defect-pixel
    correction, IR subtraction for RGB-IR, lens shading correction, white balance and digital gain,
    demosaic (bilinear, Malvar–He–Cutler, edge-directed) under an assumed CFA phase, the colour
    correction matrix, the tone curve, YUV denoise and sharpening, RGB→YUV (BT.601/709) with 4:4:4,
    4:2:2 or 4:2:0 chroma and a decoder matrix that may mismatch.
- A barrier plugin with the full contract: the CPU kernel (byte reference), its AVX2 twin, a
  six-pass GPU graph. The noise is a counter hash of the sensor pixel and the timeline position:
  deterministic, repeated by an export.
- `kt_plugin_api::barrier::BarrierKernel` gains defaulted `frame_values`/`gpu_values` — the
  barrier sees ALL its rows, not the eight cooked numbers; the host calls these forms.
- Presets (picker variants): phone at night, cheap webcam, security camera at night (IR),
  cinema raw, wrong CFA phase, no IR-cut filter.
- The effect window draws the kind as its pipeline: one block per stage with its bypass and
  rows, using the shared parameter widgets.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: the camera sensor and ISP simulation and its consequences (clean chain ≈ input,
  shot-noise variance ∝ signal, CFA phase swaps, IR tint, demosaic zipper, determinism, CPU↔GPU).
- `plugins`: a barrier's values door (`frame_values`/`gpu_values`).

## Impact

- Code: new `src/plugins/fx-video-camera`; `src/libs/kt-plugin-api/src/barrier.rs` (+2 defaulted
  methods) and its registry check; `src/libs/kt-fx/src/video.rs` and `src/libs/kt-gpu/src/engine.rs`
  call sites; `src/libs/kt-plugins-builtin` (`BUILTIN_IDS` +1); kt-app catalogs ×5, the effect
  window's pipeline panel; F-FX-007's video list (t978).
- Project format: none (a new plugin id and its rows).
- Tests reserved: t1210–t1234.
