---
id: F-FX-017
title: A camera's temporal noise reduction (3DNR) and its trails
status: candidate
priority: B
cost: L
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-plugin-api, kt-engine, kt-gpu]
invariants: [4, 5, 7, 11, 12]
ui_rules: [9]
round: null
point: null
tests: []
depends_on: [F-FX-010]
checked: 2026-11-20
---

# F-FX-017 — A camera's temporal noise reduction (3DNR) and its trails

## Essence

Every phone and camera ISP runs a recursive temporal denoiser: the new frame is blended with the
previous OUTPUT where a motion detector sees no motion. Mis-tuned, it leaves the classic ghost trails
behind moving objects — a hand dragging a smear, chroma trails lagging behind the luma. The owner asked
for this block inside the camera simulation (F-FX-010), real and on the GPU.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | none as a simulation | denoise plug-ins (Boris FX) clean noise; nobody simulates a camera's 3DNR [NOT VERIFIED live] |
| Adobe Premiere | 26 | none | no temporal camera simulation; denoisers only [NOT VERIFIED live] |
| DaVinci Resolve | 21 | Temporal NR (Studio) | a REPAIR tool: motion-estimated temporal NR with frame count 1–5 and a motion threshold; the opposite intent (remove noise without trails), the same mechanism [NOT VERIFIED live] |
| Final Cut Pro | 12 | Noise Reduction | spatial/temporal repair, not a simulation [NOT VERIFIED live] |
| After Effects | 26 | none built in | third-party "camera emulation" plug-ins add grain, not ISP trails [NOT VERIFIED live] |

## The trick behind it

**1. The trails are the filter's error, not an added look.** A recursive filter `out = α·out₋₁ + (1−α)·in`
gated by `|in − out₋₁| < threshold` leaves, where the detector misses motion (low contrast, a threshold set
high), the previous OUTPUT in place — which itself held the frame before. So the smear's length is set by
α and the threshold together, and the chroma, filtered with a higher threshold (chroma noise is stronger),
trails longer than the luma: the coloured ghost behind a moving hand is chroma recursion.

**2. A recursive filter's memory decays, and that is what makes it computable from any point.** With α < 1
the influence of a frame k steps back is α^k; after W frames it is below half a code value. So the output
at frame N is fixed by the last W inputs, and a renderer may start a cold recursion W frames before any
anchor — the reason Resolve's repair TNR asks for a frame COUNT, and the reason a simulation can seek.

## Pros

- Simulates what phones actually show in low light (trails, smeared texture on motion).
- On the GPU: one or two fragment passes per frame on the slot's own previous output.

## Cons

- A frame depends on earlier frames: the host must keep history and walk it after a seek.
- Long trails (α near 1) need a long warm-up; a cap truncates the very longest.

## What users say

NO QUOTES: no web access in this spec session (2026-11-20). The owner's own words are the source: "камера
может иметь шлейфы от шумодава?… шумодав может быть временной, пространственный… хромо-шлейфы"
(`docs/user-feedback.md`, request I1 reviewed while in work, 2026-11-01).

## How it fits Kinotrain

### What already exists

- The camera ISP simulation (`src/plugins/fx-video-camera/`, F-FX-010): the spatial NR on raw and on YUV,
  twelve GPU passes; its design moved the temporal stages to this change.
- The stream-stage history machinery (`src/libs/kt-plugin-api/src/stream.rs`, kt-engine `video_stream.rs`):
  a memo, checkpoints, walks after a seek, pacing, history on the compute device, the front slots feeding
  the stage (stream-input-order, stream-front-on-device).

### What is missing

- A kernel class whose output depends on its own previous output (not a codec's history), run on the
  device inside the chain.

### What hinders

- The stream class is a SOURCE stage keyed by codec anchors; a recursive filter has no anchors of its own —
  the grid of D2 in the change supplies them.

### Which invariants it touches

- **7, 11** — preview and export compute the same recursion from the same anchors; the bytes do not depend
  on where playback started.
- **4, 5, 12** — history frames go through the existing memo and the decode queue; at rest nothing walks.
- CAPABILITY: openspec/specs/video-fx/

## Estimate

| What | Answer |
|---|---|
| Scope | L — a plugin contract, the host's history for it, the ISP's 3DNR block, rolling shutter |
| Where | kt-plugin-api (`TemporalKernel`), kt-engine (hosting it in the stream machinery), kt-gpu, `fx-video-camera` |
| Project format | new rows of `camera_isp`; version by content |
| Tests | seek-independence, steady = walked bytes, trails geometry on a synthetic motion, cost at rest |
| Depends on | F-FX-010 (done) |
| Breaks compatibility | no |

## Verdict

Take it as the owner chose: the real recursive filter with its motion detector, luma and chroma apart, on
the GPU, hosted by the existing history machinery through a new temporal-kernel contract with fixed anchors
and a warm-up from the filter's own decay.

## Sources

- `openspec/changes/archive/2026-11-01-camera-isp-simulation/design.md` ("NOT HERE"), `docs/user-feedback.md`
  (I1, 2026-11-01).
