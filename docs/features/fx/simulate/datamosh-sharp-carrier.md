---
id: F-FX-015
title: Sharp Datamosh — the codec decides the break at its size, the picture is carried at the project's
status: accepted
priority: B
cost: M
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-plugin-api, kt-engine]
invariants: [4, 7, 11]
ui_rules: []
round: kkt-26.11.18b
point: U1
tests: []
depends_on: [F-FX-011, F-FX-013]
trackers: []
services: []
checked: 2026-11-18
---

# F-FX-015 — Sharp Datamosh — the codec decides the break at its size, the picture is carried at the project's

## Essence

The owner (kkt-26.11.18b item 1): at 144 lines Datamosh breaks beautifully, but the picture is
soft — a 256×144 picture stretched over the project's frame. He does not want pixelation; he wants
"the computation to be clever enough that it turns into the project's resolution if I want".
The break of a low-resolution codec — its coarse block grid, its big blocks moving as one, the
keyframes it drops, which blocks it bothers to code — is what makes the look. The softness is
only the decoder's picture being 144 lines high. The two can be separated.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no | no codec-level effect (F-FX-011) |
| Adobe Premiere | 26.x | no | a mosh is made outside and imported at the size it was encoded |
| DaVinci Resolve | 21 | no | the same; Resolve's optical flow serves retiming, not re-prediction |
| Final Cut Pro | 12.3 | no | displacement templates only |
| After Effects | 26.x | third-party **Datamosh 2** | its engines run inside the comp at the layer's size; the product page names no setting that separates the motion's scale from the picture's `[NOT VERIFIED: the page refused this environment]` |

**Outside the core.**

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Avidemux / ffmpeg I-frame removal | 2.8 / 7.x | delete keyframes in copy mode | the mosh IS the decoded file: its resolution is the encode's. A sharp mosh needs a full-size encode, whose block grid is then fine-grained — the "big block" look of a low bitrate is gone |
| FFglitch | 0.10.x | bitstream editor | vectors are edited in the encode's own syntax; a "motion transfer" needs equal frame sizes |
| Notch | 2026.1 | Motion Datamosh node | warps the held image by the scene's motion vectors or an optical-flow velocity image, at the render's size — the motion is a continuous field, there is no block grid and no codec to have a size |
| Real-time shaders (Three.js write-ups, shaders.com) | 2026 | datamosh shader | the previous output warped by a screen velocity buffer plus a blend of the new frame; the velocity buffer may be lower resolution than the screen, as in temporal upscalers |

## The trick behind it

**1. In a mosh the codec contributes DECISIONS, the decoder contributes PIXELS.** A P-frame is a
list of per-block decisions: this block moves by this vector, this block is coded (gets a residual),
this one is skipped; the GOP says where references start. The decoder then executes them on a
reference picture. Every tool above ties the two to one size: the file is decoded at the size it
was encoded, so a coarse grid (the look) and a soft picture (the defect) come together, and a sharp
picture (a full-size encode) brings a fine grid with it.

**2. The decisions can be executed on a larger picture exactly.** A block of 16 pels at 144 lines
is a block of 120 pels at 1080 lines; its vector, scaled by the same ratio, moves the same part of
the picture. Executing the 144-line decisions on a 1080-line reference gives the same break — the
same blocks sliding as one, the same old texture dragged by the same new motion — in 1080-line
detail. This is what a temporal upscaler does with a low-resolution velocity buffer; here the
"velocity" is the codec's block field, so the look stays a codec's (square blocks, integer pels of
the coarse grid), not an optical-flow smear.

**3. The residual must come from the full-size source, gated by the codec.** The part of a P-frame
that lets the new picture leak in is its residual. Upscaling the 144-line residual would put the
soft picture back. The full-size residual of the same prediction — the new source frame minus the
previous source frame moved by the block's own vector — is what an encoder at full size with the
same decisions would code with no loss. Which blocks get it is the small codec's decision (a block
whose residual its quantiser left empty is skipped), so the quantiser still shapes the break: at a
high QP more blocks are skipped and the old texture persists longer, as at 144 lines. With an intact
stream this carrier reproduces the source exactly; every difference from the source is a break.

**4. The break and the stream's sound stay one event.** The carrier changes no node of the stream:
the dropped keyframes, the repeated packet, the frozen reference and the lost blocks are the
schedule's, at the same positions, so the paired sound (F-FX-013) breaks at the same moments with no
change.

## Pros

- The coarse look of a low-bitrate codec in the project's detail — neither upscaled soft nor
  pixelated.
- No motion search at the large size: the expensive part of the codec stays at `lines`; the carrier
  is a per-pixel copy plus a difference.
- The same decisions at any carrier size: the preview carries at its own size, the export at the
  project's, and both show the same break.

## Cons

- The carrier needs the source at the carrier's size: the effects in front of the stream run at that
  size (in the preview the preview's, in the export the project's), not at `lines`.
- The skip decision is per block of the small grid: a block the small codec skipped keeps the large
  picture's previous content, so a slow change of fine detail inside a skipped block is not shown
  until the block is coded — a real codec's skip artefact, in fine detail.

## What users say

> This node is frame rate dependent, so changes in frame rate can heavily impact the results.
> — Notch manual, Motion Datamosh, https://manual.notch.one/2026.1/en/docs/reference/nodes/post-fx/stylisation/motion-datamosh/ (accessed 2026-11-18)

How to read this. A mosh driven by a velocity field at render time is a function of the render's
parameters; ours is a function of the codec's own parameters (`lines`, `block`, `qp`), so the
carried size must not change the decisions — that is the rule this note takes.

NO QUOTES: user complaints about soft low-resolution moshes were searched for Datamosh 2
(aescripts product page and forum) and real-time shader write-ups (codrops 2026-09-02); the pages
refused this environment (HTTP 403), and search snippets carried no verbatim user sentence about
resolution.

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-fx/
Accepted (owner kkt-26.11.18b U1, change `mosh-sharp`): an opt-in row of Datamosh; the codec runs at `lines`, a carrier decoder executes its decisions at the layer's size. Paths: `src/plugins/fx-video-codec/src/{mosh,walk,codec}.rs`, `src/libs/kt-engine/src/video_stream.rs` (`stream_quality`); invariants 4, 7, 11.

## Estimate

| What | Answer |
|---|---|
| Scope | M — a carrier step in the codec's walk, a resampling convert, the host's stream size rule |
| Where the main work is | the codec family crate (walk, shaders) |
| Project format | a new row of the plugin (omitted at its default) |
| Tests that will have to be written | carrier = source on an intact stream, the same break as the soft one, walk = reference, the sound unchanged |
| Depends on | F-FX-011, F-FX-013 |
| Breaks compatibility | no (default off) |

## Verdict

Take, as part of the SIMULATION: the carrier executes the codec's decisions; it is not a sharpening
filter over the soft picture and not an upscaler.

## Sources

- https://manual.notch.one/2026.1/en/docs/reference/nodes/post-fx/stylisation/motion-datamosh/ (access date 2026-11-18)
- https://aescripts.com/datamosh (access date 2026-11-18, refused)
- https://tympanus.net/codrops/2026/09/02/breaking-the-frame-building-a-real-time-datamosh-effect-with-three-js/ (access date 2026-11-18, refused)
- https://ffglitch.org/ (access date 2026-11-01)
