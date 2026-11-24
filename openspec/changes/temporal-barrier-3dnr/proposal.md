# Proposal: temporal-barrier-3dnr

> DESIGN, revision 2 (2026-11-25), base `fc60a5d9`: implementation-ready. The owner answered Q1–Q2 (A21). Code starts on the
> orchestrator's go. Tests: t3545–t3551, t4253.

## Why

The owner, reviewing `camera-isp-simulation` (2026-11-01, `docs/user-feedback.md`, request I1): "камера
может иметь шлейфы от шумодава? … шумодав может быть временной, пространственный … хромо-шлейфы … эффект
не доделан". Taken then: "The denoiser is a full block, spatial AND temporal (3DNR): a recursive temporal
filter with its motion detector, whose mis-tuning leaves the classic ghost trails behind moving objects,
luma and chroma separately (chroma trails)"; the time-dependent defects (3DNR trails, rolling shutter, mains
flicker) were moved here. The owner chose the real recursive filter and asked for the GPU. Registry
`kkt-26.11.01-index.md` I1: "Left: the temporal 3DNR with its trails … its own change
`temporal-barrier-3dnr` (a temporal-barrier contract)". Note F-FX-017.

## What Changes

- **The camera ISP's 3DNR**: between the lens corrections and the YUV stages, luma and chroma recursions over the previous OUTPUT with their own motion
  thresholds and a detector blur; a 16-bit state on the device. Its mis-tuning gives the trails he named.
- **Rolling shutter** by readout time, **mains flicker** by frequency and depth, in the same change (Q2 a).
- **A slot that keeps a history is a stream stage**: the camera declares a stream kernel that is active when a temporal row is on, and is cut
  into a front (run on every history frame), the recursion and a back. The machinery of Datamosh's history hosts it; with the rows off the camera is the barrier it was.
- **A split of one take does not restart the filter**: the history reaches into the event it continues.
- The trail is at most 64 frames (Q1 a).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `plugins`: a stream kernel may be active by its values and may cut its slot in three.
- `video-plan`: a temporal slot is a history stage inside the chain, hosted like a stream stage; anchors on
  the material grid; preview and export identical.
- `video-fx`: the camera ISP's 3DNR, rolling shutter and flicker; their rows and view.

## Impact

- kt-plugin-api: `StreamKernel::{active, split}`, `StreamLayout::continues`, `HeldStreamOut::Rgba`. kt-model: `is_stream`. kt-engine: `StreamStage`'s split.
  kt-gpu: the RGBA form of a held picture.
- `src/plugins/fx-video-camera`: three internal plugins by pass ranges, the recursion, rows, presets, the panel's blocks, i18n ×5.
- Format: the next free version by content. Sound: none of its own (the microphone half exists). Idle: nothing walks at rest. Note: F-FX-017.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20.txt` line 32, A21): "Q36/Q37 3DNR: «64 кадра + rolling shutter и мерцание (Recommended)»" — Q1 (a), the trail capped at 64
frames; Q2 (a), rolling shutter and flicker in the same change. Read again on 2026-11-25 with I1 ("шумодав может быть временной, пространственный … хромо-шлейфы"):
right. No open question.

The questions as asked:

**Q1. How long may a trail be?** The recursion's memory decays as α^k; the warm-up `W` is the frames after
which a frame's influence is under half a code value, and a seek walks up to `W + G` frames before the
picture appears. With strength α = 0.9, W ≈ 60; with α = 0.97, W ≈ 205.
- (a) **Cap W at 64 frames (≈ 2.5 s at 25 fps): stronger settings are allowed, and their trail is cut after
  64 frames; a seek shows "Loading" for at most ~0.1–0.3 s on a card** (*recommended*: real camera trails
  are a few to tens of frames).
- (b) No cap: exact for any α; a seek on a very strong setting may walk hundreds of frames.

**Q2. Rolling shutter and mains flicker in the same change?** They were moved here with 3DNR (2026-11-01).
- (a) **Yes** (*recommended*: the same contract carries rolling shutter; flicker needs only the frame time).
- (b) 3DNR first, the other two after.
