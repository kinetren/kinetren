---
id: F-MED-011
title: Measures of the material — beats, motion, cuts, and finding the lively moments
status: candidate
priority: B
cost: L
category: media/analysis
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-plugin-api, kt-media, kt-model, kt-engine, kt-app]
invariants: [5, 8, 11, 12]
ui_rules: [12, 16, 20]
round: null
point: change edit-analysis
tests: []
depends_on: [F-AUD-008, F-TL-001]
trackers: []
services: []
checked: 2026-11-24
---

# F-MED-011 — Measures of the material — beats, motion, cuts, and finding the lively moments

## Essence

The person cuts to music and looks for something livelier than a face that does not move. The program can know where the
beats are, where the picture moves and where a finished film was cut, show it on the timeline, let edges stick to it,
and list the liveliest places of the footage. The person still makes the cut.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Detect Scenes; beat-aligned grid of a Groove/Acid loop | scene detection on a file; beats only for loops with tempo data [NOT VERIFIED against the primary source] |
| Premiere | 26.x | Scene Edit Detection; Remix; beat detection in the iPhone app | cuts at detected edits; it also cuts the linked audio unless the person separates it first |
| Resolve | 21 | Scene Cut Detector, IntelliSearch | the cut list with a movable threshold line [NOT VERIFIED against the primary source]; 21 finds objects, faces and spoken keywords, results as whole clips |
| Final Cut Pro | 12 | Beat Detection (Clip → Enable Beat Detection) | a grid of song parts, bars and beats drawn on the clip; with snapping on, the playhead and edges snap to it; analysis of a three-minute song takes under five seconds (Larry Jordan) |
| After Effects | 26.x | Scene Edit Detection; Convert Audio to Keyframes | the amplitude as keys, no beats [NOT VERIFIED against the primary source] |

## The trick behind it

**1. The grid lives on the clip, and it is only a snap target.** Final Cut 12 writes no markers: the beats are drawn while
the clip's detection is on and edges snap to them. A list of markers must be deleted when the song moves; a grid on the
clip moves, trims and blades with it, because it is in the material's time.

**2. The song parts matter more than the beats.** Apple proposes shortening a song by cutting between song-part lines: that
works because the parts repeat. Beats alone give cuts that are on time and musically wrong.

**3. A flash is the frame whose neighbours agree.** A detector that compares a frame only with the one before calls every
flash two cuts. Comparing the frame after with the frame before removes them with one more difference.

**4. A pan is not motion.** The plain frame difference ranks every pan above a dancer. The camera's own shift must be taken
out first, and it is a measure of its own (shaky shots are what one wants to avoid).

## Pros

- A measure in the material's time survives trims, moves, reverse and speed: the representation keeps it right, nothing is
  recomputed by an edit.
- One measure has many readers: what is drawn is also what edges snap to, what is marked and what a property follows.

## Cons

- The tempo may be read an octave off; without a correction in the project the whole grid is useless for that song.
- Motion says "something moves", not "something interesting happens": a ranking is a proposal.
- The whole file must be decoded once, and a long file takes minutes.

## What users say

> Scene Edit Detection often makes cuts where there are not any camera changes
> — Adobe community, "Scene edit detection not accurate, v25 ongoing, needs sensitivity adj",
> https://community.adobe.com/t5/premiere-pro-ideas/scene-edit-detection-not-accurate-v25-ongoing-needs-sensitivity-adj/idi-p/14965735 (read through a search result, 2026-11-24)

The octave error, outside editing: Audacity #6425, a 200 BPM track read as 100 and imported at double speed.

## How it fits Kinotrain
CAPABILITY: openspec/specs/media-index/

### What already exists

`src/libs/kt-plugin-api/src/audio_series.rs` (levels and 16 bands per burst), its store and registry
(`src/libs/kt-media/src/listen.rs`, `src/libs/kt-engine/src/listen.rs`), onsets over the peak pyramid in
`src/libs/kt-media/src/sync.rs`, links (`src/libs/kt-model/src/links.rs`), the bin's scrub and drag.

### What is missing

Any measure of the picture, beats, a way for another reader to ask for a measure, a snap source that is not a `Vec`.

### What hinders

`snap_targets` (`src/apps/kt-app/src/timeline.rs`) builds a vector per call; `AnalysisKind` is locked to one kind.

### Which invariants it touches

5 (an offline source), 8 (nothing is asked by scroll or zoom), 11 (a render must not depend on a background job), 12
(thousands of ticks as meshes). UI rules 12, 16, 20.

## Estimate

| What | Answer |
|---|---|
| Scope | L — four stages, faces after |
| Where the main work is | kt-media (the kernels), kt-app (overlay, panel) |
| Project format | a new version by content for corrections and links |
| Tests that will have to be written | click tracks, drift, flashes, pans, links against an export |
| Depends on | F-AUD-008 (speech), F-TL-001 (roll) |
| Breaks compatibility | no |

## Verdict

Take it as a layer of measures with several readers, beats and motion first. Face expressiveness second, through a
downloaded detector, and only for ranking: a neural value must not drive a picture.

## Sources

- https://support.apple.com/guide/final-cut-pro/ver65b55a2b7/mac (search result, 2026-11-24)
- https://larryjordan.com/articles/the-new-beat-detection-in-final-cut-pro-12-is-simple-and-effective/ (search result, 2026-11-24)
- https://helpx.adobe.com/premiere-pro-next/edit-project/change-clip-sequence/detect-edit-points-using-scene-edit-detection.html (search result, 2026-11-24)
- https://documents.blackmagicdesign.com/SupportNotes/DaVinci_Resolve_21_New_Features_Guide.pdf (found, not read)
- https://www.github.com/audacity/audacity/issues/6425 (search result, 2026-11-24)
