---
id: F-MPH-001
title: Adjustment Events in VEGAS Pro
status: candidate
priority: B
cost: L
category: morpher/analogues
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-gpu, kt-otio, kt-app]
invariants: [4, 7, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MPH-001 — Adjustment Events in VEGAS Pro

## Essence

The editor places an empty "spacer" event on the top video track and hangs ordinary video effects on it.
Everything that lies BELOW on the tracks and falls under this event by time is shown already processed. The block
is moved, trimmed, cut, copied and switched off like any clip: where it covers, the grade is there;
where it ends, it is not. Pull its corner fade — the effect is not switched off with a jerk, but slides in and out. Two such blocks are placed
one after another with an overlap — and you get a transition between two grades.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 (mechanics since 21, 2023) | Adjustment Event | an event on a real video track; by default acts on the composite of ALL tracks below, narrowed only via Make Compositing Child; has opacity and corner fades |
| Adobe Premiere | 26.x | Adjustment Layer | a synthetic clip from the project panel; acts on everything below for the duration of its length; Opacity in Effect Controls = the strength of the effect |
| DaVinci Resolve | 20.3.2 | Adjustment Clip | "receives the current composite of the lower tracks, processes it and passes it on"; the Inspector gives transform/crop/opacity; it is not possible to selectively skip an intermediate layer |
| Final Cut Pro | 12.3 (appeared in 11.1, March 2025) | Adjustment Clip | a transparent clip on top; "effects are applied to all clips below it in the timeline"; its own role in roles, the clip length = the zone of action |
| After Effects | 26.x | Adjustment Layer | a checkbox switch on an ordinary layer; acts on the layers below in THE SAME composition and only for its own time span; the layer's mask and blend mode limit the effect by the area of the frame |

## The trick behind it

**1. Opacity works as wet/dry, not as darkening.** For an
ordinary event the background is what lies under it; for an adjustment event
the background is EXACTLY THE SAME frame that is its source —
the composite of the lower tracks. Therefore the same opacity code of the event
gives not "fading to black" but a mix of the processed picture with the
unprocessed one. The help confirms this not with a formula but with a consequence:
opacity and fades are needed to "easily bring effects in and out,
create sophisticated custom transitions". That is, the corner fade of a block is
a ramp of the effect's STRENGTH, and a crossfade of two adjustment events is
a ready-made transition between grades, and no "wet/dry" parameter inside
each plugin is required for this.

**2. The input of the effect is the mixed frame, not a list of clips.** The place
of the adjustment event in the pipeline is described by users as "roughly
post-compositing Track FX": by the time it works, the lower tracks have already been stacked together with their transitions, track motion and PiP layout.
Hence three non-obvious consequences. A mask or Cookie Cutter on
an adjustment event cuts the AREA OF THE FRAME, not a list of targets — "blur this corner of the
screen" becomes a one-click operation. Colour correction sees
the already mixed sources, so pulling different log curves of two
cameras with one adjustment event is impossible in principle. And an FX plugin on it
loses its own preview modes: the backing is always non-empty, and
"Composite over checkerboard" in Title Studio does nothing on an adjustment event,
although it works on an empty event.

**3. The scope is limited not by the choice of tracks, but by
compositing kinship.** There is no button "act on tracks 3 and 5":
the only way to narrow "everything below" is to make the needed tracks
compositing children of the carrier track. That is, limiting the zone of action
of an effect rebuilds the structure of the project's composition, and conversely —
rebuilding the composition silently changes the zone of action of the grade.

## Pros

- An adjustment event is an ordinary event: trim, split, cut/copy/paste,
  Mute, Lock, a name, up to 32 effects in the chain. There are no new gestures and no new
  window to learn; the zone of action is edited with the same mouse movements as the
  boundaries of a clip.
- Editing one chain rewrites the whole stretch of the edit: forty clips
  under the block change with one action and one undo step, and the comparison
  "before/after" is a Mute of one event.
- Since the input is the composite, the effect does not "leak" into the join: the transition between
  the lower clips is already computed, and the grade lands on the result, not twice on both halves of the join.
- The order of application is visible to the eye: several adjustment tracks and
  events add up cumulatively by the geometry of the tracks, not by a hidden order in the list of effects.
- The strength of the effect lives on the EVENT, not inside the plugin, so it ramps
  the same way for any chain — including those plugins that have no "strength" parameter of their own.

## Cons

- The event occupies a real video track, and during that time nothing else can be held on it: each independent grade is +1 track in the project.
- By default it hits ALL the lower tracks; it can be narrowed only by
  rebuilding the composition in parent/child, because of which a "local grade"
  turns into a global edit of the project's structure.
- Specific clips cannot be addressed: what is addressed is the geometry of the tracks ×
  the time window. If the needed clip has moved to another track, it has silently fallen out of
  the zone of action.
- "An ordinary event" is so only in words: the help honestly notes that
  reverse and the Velocity envelope do not act on adjustment events.
- The input of the effect is never empty (there is always a composite underneath), so
  plugins with their own preview on a transparent background break on it,
  and heavy FX spoil the preview — the cache is built on the full frame of the backing.
- Video only: the adjustment event does not touch the sound of the lower tracks at all;
  there is no audio analogue.
- In the 2026 release (the first under Boris FX) the mechanics were not touched — the whole budget
  went to the Continuum titles, Z-Depth 2.0 and Blackwell; in the new-features list and in the
  patch notes of build 105 adjustment events are not mentioned even once.

## What users say

> By the way, I prefer using Adjustment Event over Track Motion and Track
> FX. Adjustment Event is at approximately `Post-Compositing Track FX` in
> the rendering process, but it's far more flexible than Track FX.
> — zzzzzz9125, 24.08.2026,
> https://forum.borisfx.com/t/revisiting-an-old-bug-with-track-fx-vs-event-fx/25189

> Both has pros and cons, while an adjustment event can do the same as an
> Track FX (also partially for a track if needed) it needs always an EXTRA
> track and it changes ALL underlying tracks (when not using child tracks).
> — 3POINT (Theo Houben), 24.08.2026,
> https://forum.borisfx.com/t/revisiting-an-old-bug-with-track-fx-vs-event-fx/25189

> The setting has does not chnage anything. The video frames are still in
> the background and the preview is very slow.
> — harold.linke about "Composite over Checkboard" for Title Studio on an
> adjustment event, 17.05.2026,
> https://forum.borisfx.com/t/title-studio-composite-over-checkerboard-not-working-for-adjustment-events-and-fx-on-video-clip/23568

## How it fits Kinotrain

### What already exists

The direct analogue is the morpher: `MorpherEvent` in
`src/libs/kt-model/src/project.rs` (the structure at ~line 569,
`MorphTargets::Below` — "everything below the line"), applied in
`Project::effective_clips()` (same file, ~line 2948) before the plan is built,
cumulatively from bottom to top by `(Reverse(row), MorphId)`. The morpher is stronger than
Vegas in three places: it lives on ITS OWN purple track and does not take up a video track (`LaneKind::Morph` in
`src/apps/kt-app/src/timeline.rs`); it addresses not only "below" but also a
frozen list, mark ranges and named areas
(`MorphTargets::Clips/Marks/Areas`); it can replace the SOURCE itself and
shift `source_in` (`MorphPatch` in the same file, ~line 677), not
only hang FX — which Vegas does not have at all. The morpher also has an audio chain
(`MorphPatch::audio_fx`); Vegas has no analogue.

The "strength" mechanics we have are built, but not connected to the morpher:
the opacity of the event `ClipEvent::opacity` (R.28 item 12 — a line along the top of the
clip, dragged with the mouse), the corner fades and the layer's `alpha_k` in
`src/libs/kt-engine/src/video_plan.rs`, 16 kinds of transitions with curves in
`src/libs/kt-engine/src/transitions.rs`, the automation `ParamCurve` with the
common widget `src/apps/kt-app/src/automation.rs`.

### What is missing

- Blend strength. `MorpherEvent` has neither an opacity field nor
  a curve: `MorphPatch::apply_to()` simply appends the chain
  (`c.fx.extend(add)`), and the editor passes `auto: None` into `ChainEdit`
  (`src/apps/kt-app/src/morpher_editor.rs`, ~line 380). The blending
  is binary — exactly the discrepancy with which item 245 began.
- Fades at the edges of the block and trim. `Drag::Morph` in
  `src/apps/kt-app/src/timeline.rs` changes only `start`; there are no trim/split
  commands for the morpher in `src/libs/kt-model/src/model.rs`
  (`AddMorpher`/`SetMorpher`/`RemoveMorpher`/`BakeMorpher`), the length is edited only
  by a number in the window.
- Processing of the MIXED frame. For us the FX are computed per layer, and the
  composite (`alpha_over`/`dissolve`) comes after them — `PlanLayer` in
  `src/libs/kt-engine/src/video_plan.rs`. Therefore the morpher is not a "layer over the
  picture" but a mass edit of clips: a mask by the area of the frame and a grade
  over a PiP layout are inexpressible in this model.

### What gets in the way

It is not the amount of code but the place in the pipeline. The stage "FX over the finished composite"
must appear inside `VideoRenderer::frame_plan`
(`src/libs/kt-engine/src/video.rs`) and be executed by BOTH
executors — the CPU `compose_plan_cpu` and the GPU from `src/libs/kt-gpu/`;
otherwise it is a second rendering path. Further, it runs into the cache: the post-FX
memoization is addressed by `SourceId` + the layer's `fx_hash`
(`src/libs/kt-cache`), while the source's composite has none — a new kind of
key is needed, and it also determines the coverage bars. The strength slider of this
stage does not require it and runs only into the UI: the morph row is three times lower than the row of
clips (`THIN_ROW_DIV` in `src/apps/kt-app/src/timeline.rs`), and the level lines
with corner fades physically have nowhere to live in the same form as on a
clip.

### Which invariants it touches

Root `CLAUDE.md`: No. 7 (the composition code is one — the stage over the composite is
written in `frame_plan`, not in a backend; on one backend preview ==
export byte for byte), No. 4 (the cache is the only truth about coverage: a new
kind of post-FX key), No. 11 (determinism: GOLDEN_VIDEO must remain
untouched at strength 1.0, cross-backend — only via
`kt_gpu::assert_frames_close`). The audio side of the morpher additionally
touches No. 2 — the audio blend strength cannot be computed anywhere except
the audio-render thread. UI rules in `src/apps/kt-app/CLAUDE.md`: 13 (do not
waste space — the thin morph row), 17 (paired ways: the strength
must be settable both with the mouse on the block and with a number in the window), 20 (nothing
grows by itself — a fade does not appear on the morpher by itself).

## Estimate

| What | Answer |
|---|---|
| Size | L — full parity is a round; separately "blend strength + corner fades on the morph block" — M |
| Where the main work is | kt-model (the strength field and the curve of `MorpherEvent`), kt-engine (`video_plan` — blending the processed with the source; the stage over the composite), kt-gpu (the mirror of the blending), kt-app (the level line and corners on the thin morph row), kt-otio (writing the strength) |
| Project format | a new version of .otio is needed: an old reader would silently lose the strength (rule R.29); the serialisation of the default 1.0 is skipped — old files stay byte-stable |
| Tests that will have to be written | a round-trip of the model and .otio with the strength; `morpher_render` (t36) — bake == render at strength < 1; CPU↔GPU conformance via `kt_gpu::assert_frames_close`; a layout test of the level line and the corners on the morph block; `golden_export` unchanged at strength 1.0 |
| Depends on | nothing: `ParamCurve`, the layer levels and the corner fades already exist |
| Breaks compatibility | no — the default 1.0 reproduces the current behaviour |

## Verdict

We take it, but in two steps. The blend strength and the corner fades on the morph block are
a direct request from the user (item 245: "the morpher in Vegas lets you control the transparency of the effect blend with a slider, and we do not"), and the work is in the
model plus the UI, without a new rendering path; the trim of the block with the mouse goes here too,
otherwise the fades have nowhere to attach. Processing of the MIXED composite of the lower
tracks we do not take into this note: that is a stage in `frame_plan` and a new kind of
post-FX cache key, that is, invariants 7 and 4 in full — it needs a separate note in `morpher/gaps/`.
In everything else we already outpace Vegas: our own track instead of a stolen video track, addressing by
marks and areas, source replacement, and the audio chain.

## Sources

- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/adjustmentevents.htm (access date 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/adjustmenttracks.htm (2026-09-07)
- https://forum.borisfx.com/t/revisiting-an-old-bug-with-track-fx-vs-event-fx/25189 (2026-09-07)
- https://forum.borisfx.com/t/title-studio-composite-over-checkerboard-not-working-for-adjustment-events-and-fx-on-video-clip/23568 (2026-09-07)
- https://forum.borisfx.com/t/vegas-needs-a-better-lock-function/24403 (2026-09-07)
- https://forum.borisfx.com/t/patch-vegas-pro-2026-build-105/23462 (2026-09-07)
- https://www.vegascreativesoftware.com/vegas-pro/whats-new/ (2026-09-07)
- https://www.slashcam.com/news/single/New-Vegas-Pro-21-offers-adjustment-events--AI-Z-de-18090.html (2026-09-07)
- https://www.jetdv.com/2023/10/16/add-an-adjustment-event-to-selected-tracks-in-vegas-pro/ (2026-09-07)
- https://davinciresolveclub.com/adjustment-clips-davinci-resolve/ (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/add-adjustment-clips-verfda63436e/mac (2026-09-07)
- https://helpx.adobe.com/premiere/desktop/add-video-effects/apply-video-effects/create-adjustment-layers.html (2026-09-07)

VEGAS was not run live: the mechanics are compiled from the official help of
version 22 and the Boris FX forum (threads of 2026). The Premiere/Resolve/
FCP/AE rows are based on documentation and analyses, without checking at the keyboard.
