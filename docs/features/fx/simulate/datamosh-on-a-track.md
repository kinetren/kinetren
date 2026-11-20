---
id: F-FX-017
title: Datamosh over a whole track — the edit's own cuts broken, with what was composed there
status: candidate
priority: C
cost: L
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, after-effects-26]
we_have: partial
crates: [kt-engine, kt-plugin-api, kt-model, kt-app]
invariants: [4, 7, 11, 12]
ui_rules: [9, 17]
round: null
point: change track-stream-stage
tests: []
depends_on: [F-FX-011, F-FX-013, F-FX-016]
trackers: []
services: []
checked: 2026-11-20
---

# F-FX-017 — Datamosh over a whole track — the edit's own cuts broken, with what was composed there

## Essence

The look people call datamosh is the cut that never happens: the next shot's pixels are dragged by the
previous shot's motion until a keyframe arrives. Our Datamosh (F-FX-011) runs on ONE event, with "the
event before" as its history. A person who has cut a sequence wants the mosh at EVERY cut of the edit, over
whatever is on screen there: the titles, the transitions and the effects they already put on.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | nothing built in | the community exports and moshes the file in Avidemux, then imports it back |
| Adobe Premiere | 26 | nothing built in | the same round trip, or third-party glitch plugins that imitate it on an adjustment layer |
| DaVinci Resolve | 21 | nothing built in | the same round trip; Fusion has no codec to break |
| After Effects | 26 | Datamosh 2 (aescripts, third party) | moshes at markers by RENDERING the composition to a file, moshing the file and inserting the result back into the composition |

## The trick behind it

**1. Every working datamosh breaks a RENDER of the edit, not a clip.** Avidemux deletes keyframes in an
exported file, and Datamosh 2 renders the composition first. So the history the codec carries across a cut
is the COMPOSITE: the title over the shot, the dissolve, the grade. That is why the look "belongs" to the
edit. Our event-level mosh reads the event before on the same lane, which is the composite only when that
lane is the whole picture.

**2. The round trip is a baked file.** Every tool above re-renders on any edit before the cut, and the
moshed file in the timeline is stale until then. A stream stage on the track recomputes the history from
the composite itself, so an edit re-walks only from where it acts.

**3. The cut has to be defined, because a composite has no keyframes.** In a file the cuts are where the
encoder put I-frames. On a track they are where an event begins. Under a dissolve that is a choice: the
first new picture, or the joint (change track-stream-stage, Q1).

## Pros

- The mosh at every cut of a sequence with one slot, over titles and effects, live, with no export round
  trip.
- The same codec, the same rows, the same sound half: nothing new to learn.

## Cons

- A history position costs a whole track composite: on a dense edit a fresh walk is slow, and it shows as
  Loading.
- Two places for one look (the event's chain and the track's): the event-level stays for one event's mosh.

## What users say

NO QUOTES: searched aescripts' Datamosh 2 pages and forum threads, and the general "datamosh after effects /
premiere" tutorials (2026-11-20). The pages describe the marker workflow and the render-and-replace round
trip; no verbatim user line about moshing a whole edit's cuts could be confirmed within the search budget.

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-fx/

A second host of the same stream kernel, `TrackStream` in `src/libs/kt-engine/src/video_stream.rs`. It lays
the track out at its cuts and feeds the kernel the track's composite, built by `build_plan` as
`track_stage_layer` builds it, on the one executor (invariant 7). The order asks what that composite reads
(invariant 4), the identities have no serials (invariant 11), and nothing walks at rest beyond the existing
prerender (invariant 12). The sound half pairs on the track's sound track over the same layout (F-FX-016).
Today the track window refuses the slot (t3237, `src/apps/kt-app/src/track_window.rs`).

## Estimate

| What | Answer |
|---|---|
| Size | L: a host, the order and keys, the export pre-roll, the sound layout, the UI refusal removed |
| Where the main work is | kt-engine `video_stream.rs`, `video.rs`, `render_cache.rs`, export; the codec plugin's `sound.rs` |
| Project format | none: a stream slot on a track is already a valid file |
| Tests that will have to be written | one event on a track equals its own mosh; the history crosses a cut with the composite; request equals read; an edit keeps the history before it; a range export equals its part; the sound breaks at the picture's cuts |
| Depends on | F-FX-011 (the codec path), F-FX-013 (the sound), F-FX-016 (a track's device sounds on its sound track) |
| Breaks compatibility | no |

## Verdict

**Candidate, designed** (change `track-stream-stage`). It waits for the owner's three answers: where a cut
falls under a dissolve, whether a gap ends the stream, and whether an event's mosh may stand under a track's.

## Sources

- aescripts, Datamosh 2: https://aescripts.com/datamosh (2026-11-20)
- aescripts, "Unleash the power of Datamosh mosh maps":
  https://aescripts.com/learn/unleash-the-power-of-datamosh-mosh-maps-in-after-effects (2026-11-20)
- Epidemic Sound, "How to datamosh": https://www.epidemicsound.com/blog/how-to-datamosh/ (2026-11-20)
- Our refusal: `openspec/changes/archive/2026-11-20-track-mosh-and-start-wait/design.md` D1
