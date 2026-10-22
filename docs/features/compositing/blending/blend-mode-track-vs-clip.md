---
id: F-CMP-003
title: Blend mode — on the track or on the clip
status: in-progress
priority: B
cost: M
category: compositing/blending
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-gpu, kt-app, kt-otio]
invariants: [7, 11]
ui_rules: [9, 13, 17]
round: R.62
point: null
tests: [t737]
depends_on: [F-MED-004, F-MPH-007]
trackers: []
services: []
checked: 2026-09-23
---

# F-CMP-003 — Blend mode — on the track or on the clip

## Essence

The upper layer does not simply cover the lower one but multiplies, adds, lightens
or subtracts it: smoke in "Screen" over a frame, a logo in "Multiply" on a white
background, a flash in "Add". For the editor this is one list of a dozen words next to
opacity. The question of the note is not "which formulas" but **whose choice it is:
the track's or the clip's**, and what happens where the upper layer ends.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Compositing Mode — a button in the TRACK header (Source Alpha, Multiply (Mask), Add, Subtract, Screen, Overlay, …, 3D Source Alpha) | the mode belongs to the track and acts on ONE track below; to cover several — "Make Compositing Child", and then the parent gets a second mode, Parent Composite Mode. "Since lower tracks show through higher tracks, it is the compositing mode of the higher track that determines how much of the lower track shows though" |
| Adobe Premiere | 26 | Opacity → Blend Mode (Effect Controls inspector) | a property of the CLIP inside the built-in Opacity effect; acts on everything below; keyframed like any parameter |
| DaVinci Resolve | 21 | Composite Mode (Video Inspector → Composite) | a property of the CLIP; on the Color page the same modes live in the Layer Mixer and, since 18.5, on every node — three places for one word |
| Final Cut Pro | 12.3 | Blend Mode (Video inspector → Compositing) | a property of the CLIP; "the pixels of an image with a selected blend mode are combined with the pixels of any clip immediately below it in the timeline"; a separate Premultiplied Mix mode fixes the fringe of premultiplied graphics |
| After Effects | 26 | Blending Mode (the Mode column in the Layers panel) | a property of the LAYER; a layer is the track — one entity, one address |

## The trick behind it

**1. In Vegas the mode is a property of a TRACK and acts on exactly one track below,
so where there is no upper clip, the lower tracks can simply disappear.**
Multiply on track 1 multiplies track 2 by track 1; in the gap between clips
track 1 is empty — and the product is black. A 2013 thread about "layers below
(4 – 10 layers) are sometimes blacked out" is cured not by a mode but by kinship:
make track 2 a child, so that the mode is applied to the composite of the children. That is, a track mode has a cost
that a clip mode does not have: the EMPTINESS of the track is not
"nothing" but an operand. The clip mode (Premiere, Resolve, FCP) does not have this trap:
no clip — no operation.

**2. Mode and opacity are different axes, and both come AFTER alpha.** FCP
says this directly: "Blend modes work in addition to a clip's alpha channel and
Opacity control." The formula is `out = lerp(below, blend(below, above), alpha ×
opacity)`, not `blend(below, above × opacity)`: at 0.5 opacity Multiply
gives half the path to the product, not the product with a semi-transparent
multiplier. An implementation that multiplies opacity into the source before blending
diverges from any of the five editors at the very first slider.

**3. In Resolve one word lives in three places, and this is the reverse side of flexibility.**
Composite Mode on the clip, the Layer Mixer in the node tree, and a mode on each
node since 18.5 — three levels with one list, and the order of their application cannot be
changed from the interface. Vegas pays for this differently: two modes on one
parent track (its own and Parent Composite Mode), and on the forum this exact question
— "A 'Parent' track can have two compositing modes… Why?".

## Pros

- A mode on a track (Vegas, AE): one decision for the whole track of smoke or logos,
  not two hundred clips; when a clip is moved to another track the mode stays with
  the track, rather than travelling with the clip.
- A mode on a clip (Premiere, Resolve, FCP): there is no "track emptiness" operand,
  it is keyframed together with the other properties of the clip, copy-paste carries it along.
- Separate axes "mode" and "opacity" give a "half" Multiply without a second layer.

## Cons

- A track mode acts on ONE track below, and "covering everything" requires a
  second entity (kinship) and a second mode on the parent — two places to look.
- A hole on a track with a mode is not transparency but a black (Multiply) or white
  (Screen) result: a bug that the user sees as "the layers disappeared".
- A clip mode at transitions and overlaps is ambiguous: a pair in a crossfade has two
  modes, and which one applies in the blending zone is decided differently by each editor
  (in Resolve — a thread about transitions and composite modes).
- Premultiplied graphics with any mode other than Normal produce a dark fringe, and this is
  fixed by a separate item of the list (FCP Premultiplied Mix), not automatically.

## What users say

> I have a video on top of another video, and have it blending nicely into the
> background with the composting mode set on multiply of that top channel. The
> problem is now layers below (4 – 10 layers) are sometimes blacked out (rather
> than showing through).
> — Thoreau Bakker, Creative COW, 20.06.2013, https://creativecow.net/forums/thread/compositing-model-multiply-mask-black-out/

> In the Track 1 'Track Header', make Track 1 a Parent to your Track 2. Now
> whenever there is a gap you will see the other tracks and not turn black.
> — Graham Bernard, there, 20.06.2013

> Since lower tracks show through higher tracks, it is the compositing mode of
> the higher track that determines how much of the lower track shows though.
> — VEGAS Pro help, "Compositing and masks", https://help.magix-hub.com/video/vegas/23/en/content/topics/8-design/compositingandmasks.htm (accessed 2026-09-23)

> Blend modes work in addition to a clip's alpha channel and Opacity control.
> — Final Cut Pro User Guide 12.3, "Intro to blend modes", https://support.apple.com/guide/final-cut-pro/intro-to-blend-modes-vera85320379/mac (accessed 2026-09-23)

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-plan/
A mode is a property of the TRACK on the composite of its events ("track = implicit reel"): a hole is transparency, not an operand; the axes "mode" and "level" are separate and stand after alpha. The owner's decision in R.62, change `track-object-and-track-fx`.

Done in R.62 B: `BlendMode` lives on the track (`src/libs/kt-model/src/project.rs`),
the formula `out = lerp(below, blend(below, above), α × level)` is written as a PAIR —
`blend_over` in `src/libs/kt-engine/src/video.rs` and its mirror in
`src/libs/kt-gpu/src/shaders/composite.wgsl` (they are edited in one commit, like the transition kernels). A track with a stage enters the plan as an
embedding (`src/libs/kt-engine/src/video.rs`, `track_stage_layer`), so a "hole" in an event
shows the lower track, not black. The choice of mode is the track window
(`src/apps/kt-app/src/track_window.rs`), the check against the analytic formula and GPU is
t737 (`src/libs/kt-engine/tests/track_blend.rs`, `src/libs/kt-gpu/tests/parity.rs`).
The clip blend mode is POSTPONED: an event already has a level and transitions, and mixing
two axes in one round would mean deciding for the owner.

## Estimate

| What | Answer |
|---|---|
| Size | M: the blend kernels as a CPU + WGSL pair with conformance, a field on the track, an item in the track properties |
| Where the main work is | kt-engine (compositing), kt-gpu (the composite shader), kt-model (the field), kt-app (track properties), kt-otio (the track envelope) |
| Project format | a field in the track envelope, the version of round R.62 |
| Tests that will have to be written | each mode against the analytic formula on CPU; GPU conformance; opacity 0.5 — half the path; a hole on a track with Multiply does not blacken the lower ones; Normal — byte-for-byte the previous composite |
| Depends on | F-MED-004 (layer geometry in the plan — the same stage), F-MPH-007 (track composite as an object) |
| Breaks compatibility | no: Normal by default gives the previous bytes |

## Verdict

**We take it, on the track, on the track's composite.** The owner asked for "our own blend options on tracks" (R.62);
the track level is chosen together with the track's FX chain and its level — one stage, three properties. The Vegas trap with emptiness as an
operand is avoided because the operand is the track's composite with honest alpha, not the "track frame";
the clip mode is not added until it is asked for: a clip has a level and fades, and two modes on one
pair in a crossfade are an ambiguity that none of the five has solved well.

## Sources

- VEGAS Pro help, "Compositing and masks" (modes, kinship, Parent Composite
  Mode): https://help.magix-hub.com/video/vegas/23/en/content/topics/8-design/compositingandmasks.htm
  (accessed 2026-09-23)
- Creative COW, "Compositing model 'Multiply' (Mask) – Black out" (2013):
  https://creativecow.net/forums/thread/compositing-model-multiply-mask-black-out/
- Creative COW, "A 'Parent' track can have two compositing modes… Why?":
  https://creativecow.net/forums/thread/a-parent-track-can-have-two-compositing-modes-w/
- Final Cut Pro User Guide, "Intro to blend modes" and "Types of blend modes"
  (Premultiplied Mix): https://support.apple.com/guide/final-cut-pro/intro-to-blend-modes-vera85320379/mac ,
  https://support.apple.com/guide/final-cut-pro/types-of-blend-modes-ver007271c06/mac
  (accessed 2026-09-23)
- DaVinci Resolve 18.6 manual, "Compositing and Transforms in the Timeline",
  "More About Composite Modes": https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1241.htm
  (accessed 2026-09-23); thread about transitions and composite modes:
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=220360
- Adobe Premiere: Blend Mode inside the Opacity effect — per the helpx.adobe.com documentation
  [NOT VERIFIED against the primary source from this environment; the wording is from memory of the 26.x interface]
