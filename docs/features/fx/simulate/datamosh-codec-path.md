---
id: F-FX-011
title: Datamosh: a motion-compensated codec broken on purpose
status: done
priority: B
cost: L
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, capcut-2026]
we_have: yes
crates: [kt-plugin-api, kt-engine]
invariants: [4, 7, 11]
ui_rules: []
round: kkt-26.11.01
point: D1
tests: [1235, 1236, 1237, 1238, 1239, 1240, 1241, 1242, 1243, 1244, 1245, 1246, 1247, 1248, 1249]
depends_on: [F-EXT-003, F-FX-007]
trackers: []
services: []
checked: 2026-11-01
---

# F-FX-011 — Datamosh: a motion-compensated codec broken on purpose

## Essence

A datamosh is the picture a VIDEO DECODER makes when it is fed a broken stream: the motion of
one shot drags the texture of another, a moving object "blooms" and smears, the image melts
until the next keyframe. The editor wants that look on purpose and wants to steer it: where it
starts, how long it lasts, how far the smear spreads, how much of the new picture leaks through.
It is not a filter over the picture — it is a property of how inter-frame compression works, and
it only looks right when the picture really went through motion compensation.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no | no codec-level effect; "glitch" means RGB shift and block displacement filters (third-party BCC/Sapphire packs) |
| Adobe Premiere | 26.x | no | the same: glitch looks are image filters or third-party transitions; a real mosh is made outside and imported |
| DaVinci Resolve | 21 | no (Fusion: no motion-vector node that re-predicts) | Resolve's optical flow computes vectors for retiming, but there is no "apply these vectors to that reference" node; the mosh is imported as a rendered file |
| Final Cut Pro | 12.3 | no | Motion templates imitate glitches by displacement; no prediction loop |
| After Effects | 26.x | no built-in; the third-party **Datamosh 2** (Plugin Play, aescripts) | the plugin runs its OWN moshing engines inside AE ("over 60 moshing algorithms", markers set where the reference breaks, "Mosh Maps" choose the region). It is the only line where the mosh happens inside the editor's render |
| CapCut | 2026 | "Glitch" effects tab `[NOT VERIFIED]` | named glitch looks are image operations (RGB split, block shift, scan lines); none re-predicts from a wrong reference |

**Outside the core.** The mechanism originates in codec tools, not editors; without them the
"trick" below cannot be stated.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Avidemux (classic method) | 2.8 line | delete keyframes in an Xvid/MPEG-4 AVI, copy mode | the user cuts I-frames out of the BITSTREAM with "copy" (no re-encode); the following P-frames are decoded against the previous shot. Duplicating a P-frame N times is "bloom". Works only with codecs and containers that tolerate it |
| FFglitch | 0.10.x (2024) | a "multimedia bitstream editor, based on FFmpeg" | exports the motion vectors and other syntax elements of an MPEG-2/MPEG-4 stream as JSON, lets a script edit them, and writes the bitstream back. "Motion transfer" moves the vectors of one clip into another of the same size |
| Datamosher-Pro, tomato.py, pymosh | 2021–2022 | GUIs and scripts over the two above | "Void" removes I-frames automatically at cuts, "Rise" removes a chosen range, "Fluid" averages motion |

## The trick behind it

**1. A mosh is a decoder fed the wrong reference, and every working method is a way to make the
decoder do that.** Inter-frame codecs send a few full pictures (I-frames) and, between them,
P-frames that carry only motion vectors per block plus a small residual correction relative to
the PREVIOUS decoded picture. Remove the I-frame that starts a new shot and the decoder keeps
applying the new shot's vectors and residuals to the old shot's pixels: the old texture moves
with the new motion, and the new picture leaks in only through the residual. Repeat one P-frame
and its motion is applied again and again: the moving region smears ("bloom"). Swap the vectors
of two clips and one clip moves like the other ("transplant"). None of these are image
operations; each is a change to what the decoder is told.

**2. Bitstream hacking is fragile because the decoder is not ours.** The avidemux method needs
an old codec (Xvid/MPEG-4 Part 2) in AVI, because H.264/HEVC decoders conceal errors, reorder
B-frames and refuse broken streams; the result also depends on the decoder version and on the
GOP the encoder happened to choose (x264's scene-cut detection puts an I-frame at every cut —
that is exactly what makes "delete the I-frame at the cut" work, and why a different encoder
setting gives a different mosh). FFglitch is precise but only for codecs whose syntax it
understands, needs equal frame sizes for motion transfer, and every change is a re-run of an
external tool over a file. For an editor this means: a mosh is a rendered asset, not an edit,
and it cannot be keyframed, previewed at a seek or exported in chunks.

**3. A codec model inside the effect turns the same mechanics into a deterministic function.**
If the effect contains its own small block codec (motion estimation, a prediction loop over a
reference buffer, a quantised DCT residual, a fixed GOP), then the "broken stream" is a set of
parameters, and the output at frame N is a pure function of the source frames from the last
intact I-frame up to N. Seeking, chunked export and determinism become properties of a pure
function with a bounded history, not of a file on disk. Datamosh 2 is the existence proof that
this is what an editor-side mosh has to be: it does not hack AE's codecs, it runs its own
engines.

**The mechanics, step by step (what the model must contain):**

1. **Colour space and planes:** RGB → Y'CbCr, chroma subsampled 4:2:0 — the chroma moves with
   the luma's vectors at half resolution, which is where a mosh's colour bleed comes from.
2. **Macroblocks:** the luma plane cut into 16×16 (or 8×8) blocks; each block is predicted.
3. **Motion estimation (encoder):** for each block, a search in the reference within ±R pixels
   for the displacement with the least SAD; the vector field is the codec's main data.
4. **Prediction loop:** the decoder keeps a reference buffer (the previous decoded picture) and
   forms each P-frame as "the reference displaced by the vectors".
5. **Residual:** source − prediction, per 8×8 block a DCT, quantised by a step derived from the
   quantiser (QP); the decoder adds the de-quantised inverse DCT back.
6. **Closed loop:** the encoder predicts from its OWN reconstruction, not from the source, so an
   intact stream drifts by no more than one quantisation error per frame.
7. **GOP structure:** an I-frame (a full picture, intra-coded through the same DCT/quantiser) at
   the stream start, at a cut, and every N frames; P-frames between.

**Breakages, and which are codec quantities:**

| Breakage | Codec quantity it changes |
|---|---|
| Drop I-frames from time X (the reference survives the cut) | the GOP's keyframe list |
| Repeat a P-frame N times (bloom) | the packet sequence |
| Freeze the reference | the decoder's reference buffer update |
| Scale / rotate / swap the vectors | the motion-vector field |
| Transplant vectors from another event or track | the motion-vector field's source |
| Quantiser, residual loss | the residual's quantisation step and how much of it reaches the decoder |
| Macroblock corruption probability | per-block packet loss (vector and residual of a block lost) |
| GOP length, block size, search range, encoded height | the encoder's configuration |

## Pros

- The look follows the footage's real motion: a pan smears as a pan, a face moves as a face,
  because the vectors are measured, not drawn.
- Every breakage is a number in a passport, so it can be keyframed, previewed at any frame and
  exported, unlike a bitstream edit baked into a file.
- One model gives every classic variant (cut mosh, bloom, transplant, melt) as presets over the
  same parameters.

## Cons

- The output at frame N depends on the frames since the last intact I-frame: the first frame
  after a seek pays for the whole history up to it, and the history must be bounded.
- Motion search and a DCT per frame are expensive at full resolution; the model has to run at an
  encoded height of its own, which is also the look people expect (a low-bitrate web codec).
- A serial prediction loop does not map onto a per-pixel GPU pass; the codec is CPU work, and
  everything after it in the chain stays on whichever backend runs the chain.

## What users say

> FFglitch is a multimedia bitstream editor, based on the open-source project FFmpeg.
> — ffglitch.org, front page (accessed 2026-11-01), https://ffglitch.org/

> Motion Transfer a powerful ffglitch feature that can transfer the vector motion data from one video to another.
> — Datamosher-Pro README, https://git.terata.org/eessppeelllloo/Datamosher-Pro/src/branch/Datamosher-Pro-master/Readme.md (accessed 2026-11-01)

How to read this. Both quotes describe a tool that edits a FILE: the motion data lives in a
bitstream someone else's decoder will read. That is the fragility the trick section names; our
answer is to own the decoder.

NO QUOTES: user complaints about Datamosh 2 were searched on the aescripts product forum
(`aescripts.com/forums/discussion/2556/datamosh-2`) — the page did not load from this
environment (proxy refused the connection), and the search snippets carried no verbatim user
sentences about previews or determinism.

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-fx/
Done (owner 2026-11-01, request D1): an own block codec inside a stream-stage plugin, the breakages as its passport and curve-drawn gates; frame N is a pure function of a bounded history (`datamosh-codec-path`).

## Estimate

| What | Answer |
|---|---|
| Scope | L — one round: the codec model, a source-stage declaration, the history order and the export pre-roll |
| Where the main work is | the codec family crate; kt-engine's layer source and order |
| Project format | does not touch (a plugin slot; rows are its passport) |
| Tests that will have to be written | round trip, cut mosh, bloom, seek = play, determinism, chunked export = one go |
| Depends on | F-EXT-003 (plugin contract), F-FX-007 (the catalogue's "temporal family") |
| Breaks compatibility | no |

## Verdict

Take, as a SIMULATION: our own motion-compensated codec, broken by parameters — never a
block-shift imitation renamed "datamosh" (root `CLAUDE.md`, imitation ≠ simulation).

## Sources

- https://aescripts.com/datamosh (access date 2026-11-01)
- https://ffglitch.org/ (access date 2026-11-01)
- https://git.terata.org/eessppeelllloo/Datamosher-Pro/src/branch/Datamosher-Pro-master/Readme.md (access date 2026-11-01)
- https://www.toolfarm.com/?p=69970 — Datamosh 2 release notes summary (access date 2026-11-01)
