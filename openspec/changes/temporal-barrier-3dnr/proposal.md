# Proposal: temporal-barrier-3dnr

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3545–t3557.

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

- **A temporal kernel contract** (`kt-plugin-api`): a video kernel whose output at material frame N is a
  recursion over its own previous state, declared with its warm-up `W` (from the filter's decay) and its
  anchor grid `G`; optional spatial passes before (on every history frame) and after (on the shown frame).
  The kernel is pure: the state at an anchor is a cold recursion over the W frames before it, so the bytes
  do not depend on where playback started.
- **Hosted by the existing history machinery**: the stream stage's memo, checkpoints, walks after a seek,
  pacing and on-device history carry a temporal kernel as they carry a codec; the slots in front of it feed
  it (stream-input-order). Steady play costs one recursion step per frame; a seek walks at most `W + G`
  frames.
- **The camera ISP's 3DNR**: after demosaic, before the YUV stages — luma and chroma recursions with their
  own motion thresholds, a detector blur, strength; the state kept at 16-bit precision on the device (an
  8-bit recursion sticks: its dead band freezes small differences forever). The "view" gains "3DNR motion
  map". Its mis-tuning gives the trails.
- **Rolling shutter** (a temporal kernel with W = 1 and no recursion: rows read between frames N−1 and N by
  readout time) and **mains flicker** (no history: a function of the frame's material time and the row's
  readout time) join the ISP's sensor block.
- **The registry**: a temporal plugin declares the class `Temporal`; a temporal kernel on another class, or
  without its warm-up, is refused.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `plugins`: the `Temporal` class and the temporal kernel contract.
- `video-plan`: a temporal slot is a history stage inside the chain, hosted like a stream stage; anchors on
  the material grid; preview and export identical.
- `video-fx`: the camera ISP's 3DNR, rolling shutter and flicker; their rows and view.

## Impact

- kt-plugin-api: `TemporalKernel { warm, grid, pre, step, post }` with its GPU pass graph (word passes for
  the 16-bit state), `VideoClass::Temporal`.
- kt-engine: `StreamStage` hosts `Codec` or `Temporal`; anchors and warm-up in `needs`; the front memo and
  checkpoints reused; keys include the anchor.
- kt-gpu: state textures (R32Uint words), the step and the motion-map passes, the walk on the compute host.
- `src/plugins/fx-video-camera`: the ISP split into pre / temporal / post when 3DNR or rolling shutter is
  on; new rows; presets ("phone low light" with trails).
- Format: new `camera_isp` rows raise the file by content (an older build would drop them on save).
- Idle: on a pause the shown frame's walk runs once and is kept; at rest nothing walks.
- Sound: the camera's microphone half exists (`camera_isp_sound`); a video denoiser has no sound of its own.
- Feature note: F-FX-017.

## Questions to the owner

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
