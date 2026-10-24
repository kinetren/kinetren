---
id: F-MPH-005
title: Levels of applying video FX in VEGAS and the Video Bus Track
status: in-progress
priority: B
cost: XL
category: morpher/analogues
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-gpu, kt-otio, kt-app]
invariants: [7, 11]
ui_rules: [13, 17]
round: R.62
point: null
tests: [t735, t736, t737, t739]
depends_on: []
checked: 2026-09-07
---

# F-MPH-005 — Levels of applying video FX in VEGAS and the Video Bus Track

## Essence

In VEGAS the same plugin can be hung in four different places, and the choice of place determines WHAT it sees on input: the whole media file (Media FX),
one event (Event FX), the output of one track (Track FX), or the finished frame
of the whole project (Video Output FX). The last level lives on a separate row of the
timeline — the Video Bus Track — where, besides the chain of effects, there are three
envelopes over the whole mix. The order between the levels is rigid and not configurable:
the editor chooses not "when" but "where". The adjustment event is a fifth way, and it
does not replace any of the four.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Media FX · Event FX · Track FX · Video Output FX (Video Bus Track) | four levels in a rigid order; the input of each is the result of the previous one; the bus is a timeline row with a chain and three envelopes over the whole project |
| Adobe Premiere | 26.x | Adjustment Layer; Master Clip Effects; nested sequence | there are no track VIDEO FX at all: the "track" level is replaced by a layer-clip on top, the "master" level by nesting a sequence. Audio has everything: Track FX, Submix, Master |
| DaVinci Resolve | 20.3 | Clip · Group Pre-Clip · Group Post-Clip · Timeline (+ Adjustment Clip on the Edit page) | the same four levels, but the grade of the Timeline level is applied to EACH clip separately, not to the mixed frame; there are no track FX on the Edit page |
| Final Cut Pro | 12.3 | Adjustment Clip (built in since 11.1) | there are no tracks as a concept, so there is no track level; grouping is a compound clip; there is no "project output" level |
| After Effects | 26.x | Adjustment Layer, precomp | the "track" level is absent (a layer = a clip), the "output" level is replaced by nesting a composition into a composition |

## The trick behind it

**1. The Track FX button means two different things depending on the track flag.**
An ordinary track: the input of the chain is only its own events.
A track inserted via `Insert | Adjustment Track`: "Adjustment tracks
show the Track FX button in the track header by default. You can click this
button to access the Plug-In Chooser and add the video effects you want to
pass through to other tracks" — the same button, the same window, but at the input is already
the mixed picture of all the tracks BELOW. The scope can be limited by declaring the
lower tracks compositing children: "any tracks that are not children of
the adjustment tracks — even though they are below the adjustment track —
are no longer influenced by it". That is, the level is set not by where you put
the plugin, but by a property of the row, and by the look of the chain one case cannot be told
apart from the other.

**2. Within a level the order is free, between levels it is nailed down — and that is exactly why a hidden fifth level exists.**
In the event chain Pan/Crop is the same draggable node as the plugins: "you can choose
whether effects are applied to the event before or after any event panning
and cropping by arranging the Pan/Crop button in the plug-in chain at the
top of the Video Event FX window". But you cannot swap Event FX and Track
FX in any way: the only way to "rearrange" is to move the plugin to
another level. For this purpose Media FX is kept, which is reached
by Alt+click on the Event FX icon: it is the only place BEFORE the chain
of the event. The classic scenario ("I want black-and-white on a piece on top of a track
correction") is solved not by a flag but by moving the correction to the media level.

**3. The bus is a track, not just a chain, because of three envelopes, two of which are not filters at all.**
Besides fade-to-color, Video Bus Track also hosts
motion blur amount and video supersampling — they do not process the finished
frame, but change HOW MANY intermediate frames the compositor computes:
"supersampling cannot improve the appearance of existing video", it works only
on the motion that VEGAS itself generates (track motion, pan/crop,
transitions, effect keyframes). Such a parameter needs a line over time,
whereas a chain of effects does not; hence a separate row. And here the domain of
keys emerges: the bus has no events at all, so its keys are always in the time
of the PROJECT, while on ordinary tracks whether keys follow the event is decided by the
global switch `Options | Lock Envelopes to Events` — a property of the application,
not of the effect or the clip.

## Pros

- A plugin hung on the bus is computed once per frame; the same plugin at the event level is
  computed as many times as there are events in the frame.
  On a six-layer composite that is a difference of six kernel runs.
- The "project output" level sees the frame AFTER the composite, so the final
  things (the level limiter, grain, vignette, display LUT) are applied once
  and uniformly to everything, including the background and the edges, which no
  event reaches.
- The track level is inherited automatically: a new event dropped on the track is already
  processed by its chain — nothing needs to be copied. Exactly this
  is what Premiere lacks, where there are no track video FX and the stand-in layer
  has to be stretched by hand.
- The adjustment event, being an ORDINARY event ("just like any video event, you
  can add up to 32 effects in the chain of effects for the event"),
  gets everything event-related for free: trim, split, copy/paste, corner fades and
  the opacity slider. Hence both partial blending of an effect and
  a crossfade between two different corrections.
- A bypass exists at every level and on every plugin: a checkbox on an individual plugin in the chain window,
  "disable all / enable all" by right-clicking the Track FX icon, one "Bypass FX and Envelopes" button on the bus.

## Cons

- Four levels — four places to look for why the picture is not what it should be, and none of them is visible from the timeline: the Track FX icon looks the same with and without a chain, Media FX has no badge on the event at all, and
  Video Output FX lives in a row that is hidden by default (View | Video
  Bus Track).
- The order can be neither seen nor changed. The expectation "I put the effect on the event, so it is last" always breaks in the same way, and it is cured only by
  knowing the list of levels by heart.
- The result depends on the level not only by order but also by what is being mixed: on a crossfade between two events, a correction sitting on the track and the same correction sitting on the events give different pictures — caught by the black dropout on the RGB parade.
- There is no separate preview by level. The split-screen "FX Bypassed" switches everything off at once — "bypasses all video effects (track effects, event
  effects, media effects, and video output effects)"; one cannot look at "without the track level, but with the event level".
- There is exactly one bus: "a single bus track exists as a timeline representation
  of the main video output". No submixes, no sends, no parallel branches — even though AUDIO in the same VEGAS has buses and assignable FX.
  The stand-in for a video submix is an adjustment track with compositing children, that is, the same work is done by a completely different mechanism.
- The bus has no events, so there is no trimming either: a bus effect can be limited to a piece of the film only
  by keyframes (or a bypass by hand), and the keys there are in the time of the project — inserting something at the start of the timeline shifts all of them.

## What users say

> Vegas processes FX in this order: Media FX –> Event FX –> Track FX –>
> Master Video FX. This is why your Track FX and overriding your Event FX.
> — John Rofrano, Creative COW, 12.05.2011,
> https://creativecow.net/forums/thread/any-way-to-apply-event-fx-after-track-fx/

> Aha, "Media FX." I am new to Vegas, I had NO IDEAL that these existed.
> — David Hall (thread author), Creative COW, 12.05.2011,
> https://creativecow.net/forums/thread/any-way-to-apply-event-fx-after-track-fx/

> Right click the track fx icon for a given track and you should see a menu
> giving options to disable all and enable all fx for that track.
> — Norman Black, Creative COW, 06.06.2013,
> https://creativecow.net/forums/thread/vide-fx-bypass-ae/
> (the person asking replied that they had looked for it in the manual and not found it)

What is telling is not that people get confused, but WHO gets confused: the author of the first thread
had been editing in VEGAS for more than a day and simply did not know that the media level existed,
until the processing order hit him on the hands. Negative feedback about the bus is almost absent — it is simply not switched on: the row is hidden by default, and most scenarios are covered by the adjustment event.

## How it fits Kinotrain

### What already exists

We have exactly two levels of applying video FX, and both are event-based.

- The event chain: `ClipEvent.fx` / `fx_bypass` / `fx_auto` / `fx_off` in
  `src/libs/kt-model/src/project.rs` (around line 1502).
- The morpher layer: `MorpherEvent.patch.video_fx`, which in
  `MorphPatch::apply_to` (same file) literally does `c.fx.extend(add)` —
  it APPENDS its plugins to the chain of each target. The layout of targets —
  `Project::effective_clips()`, accumulation bottom to top by
  `Project.lane_order` / `LaneKind`, edited via `Command::SetLaneOrder`
  (`src/libs/kt-model/src/model.rs`).
- Execution: `LayerSrc.fx` in `src/libs/kt-engine/src/video_plan.rs` — the chain is PER-LAYER;
  `apply_video_chain` in `src/libs/kt-fx/src/video.rs` is called before the composite, and the composite (`compose_from_cache` in
  `src/libs/kt-engine/src/video.rs`) comes after.
- The clip level as a single line: `ClipEvent.opacity` / `gain_db`, corner
  fades — a transition with one live end.
- An audio master exists, but only as volume: `set_master_gain` in
  `src/libs/kt-engine/src/player.rs:375`, the handle is in
  `src/apps/kt-app/src/meters.rs:822`.
- Automation: `ParamCurve` / `AutoKey` (`project.rs`), the widget is
  `src/apps/kt-app/src/automation.rs`. The key domain is the MATERIAL TICK:
  `curve_value(keys, ease, at)` takes the material position, therefore trimming
  hides the keys rather than shifting them. This is exactly the opposite choice relative to
  the VEGAS bus (project time) and relative to `Lock Envelopes to Events`
  (a global toggle): we have one domain and no switch.

### What is missing

A grep over `src/` (`track_fx|lane_fx|bus_fx|master_fx|output_fx|submix`) gives
zero matches. There is no track, bus, or master level for
video; for audio there are no track FX and no submixes. The main thing, however, is that we have NOT ONE
level whose input is a mixed picture. The morpher looks like an adjustment layer, but by construction it is closer to Resolve's Timeline grade
(edited once, acts on each clip separately), not to the Video Bus
Track and not to VEGAS's adjustment event. From this also follows the absence of a strength slider on the morpher (item 245, R.36
of `docs/user-feedback.md`): there is nothing to "partially blend in" until there is a separate
result of the composite.

### What gets in the way

`FramePlan` (`src/libs/kt-engine/src/video_plan.rs`) describes LAYERS, not a pipeline: it has `layers`, but no object "finished frame" to which
a chain could be attached. A bus-level appearance is a new stage AFTER the
composite, and it must be born in both executors at once: the CPU
(`compose_from_cache*`) and the GPU (kt-gpu, a second pass over the target). The
post-FX memoization key is currently per-layer (`LayerSrc.fx_hash`) — a bus chain
changes the whole frame, so a frame-level key is needed, otherwise the cache will start returning
someone else's. Plus: the project-level chain is a new model field, and therefore the format version
`KT_META_V5` in `src/libs/kt-otio/src/meta.rs` (currently
`KT_META_VERSION = 4`). And the bus row on the timeline is one more
`LaneKind`, which must get into `lane_order` and not break test 49.

### Which invariants it touches

- **No. 7 (the composition code is one).** The stage "effects on the mixed frame" must
  appear inside `VideoRenderer::frame_plan`, not as a separate branch
  in export — otherwise we get exactly the second path against which the invariant is written.
- **No. 11 (determinism).** The new stage after the composite passes through
  GOLDEN_VIDEO and through CPU↔GPU conformance (`kt_gpu::assert_frames_close`),
  and through "preview == export byte for byte" on one backend.
- **UI rule 13 ("do not waste space").** A permanent bus row for the sake of one chain is
  exactly the case for which we get criticised: in VEGAS it is
  hidden by default, and that is an admission of the problem, not a solution.
- **UI rule 17 ("paired ways").** If the level appears, it must immediately have both a level bypass and an A/B view,
  otherwise we will repeat VEGAS's minus: the bypass exists only as "everything at once".

## Estimate

| What | Answer |
|---|---|
| Size | XL — a round plus a format version and a new render stage |
| Where the main work is | kt-engine (`video_plan`/`video`: the stage after the composite), kt-gpu (the second pass), kt-model (the project field + undo + `LaneKind`), kt-otio (v5), kt-app (the row and its FX editor) |
| Project format | a v5 version is needed (`KT_META_V5`); old files = an empty chain |
| Tests that will have to be written | a model round-trip with the bus chain; CPU↔GPU conformance of the new stage; preview == export (an extension of t67/t184); unchanged t36 (morpher == bake) and GOLDEN_VIDEO without the bus chain; the frame-level post-FX cache key |
| Depends on | — (but the "morpher strength slider" logically follows: both features run into the absence of an object "the mixed frame") |
| Breaks compatibility | no |

## Verdict

We take it into the plan, but not first: the value here is not in "one more place for
plugins", but in the fact that a level with the mixed frame on its input is the missing piece for the morpher's strength slider and for honest
final effects. The four levels should NOT be copied whole: the minus of "four places to look" is real, and VEGAS pays for it with a
row hidden by default. Our reasonable minimum is one project-output level plus, as a separate decision,
a switch "the morpher works on the composite below" — that is, what VEGAS does not have at all: there Track FX and the adjustment track are distinguished by a
row flag, not by a switch on the layer itself.

## Sources

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/bus_tracks-video.htm — Video bus track, VEGAS Pro 2026 (access date 2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/adjustmenttracks.htm — Adjustment tracks, 2026 (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/adjustmentevents.htm — Adjustment events, 2026 (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/8-design/addvideofx_media.htm — Adding video media effects, 2026 (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/1-before/whatsnew.htm — What's New in 2026: there are NO changes to FX levels; new items are Speech-to-Text/Text-to-Speech, Storyblocks, Z-Depth 2.0, Continuum Title Pack, Blackwell 4:2:2 (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/8-design/addvideofx.htm — Adding video effects, v22 (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/5-preview/split-screen_previews.htm — Split-screen previews / FX Bypassed (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/2-window/pancropwindow.htm — Pan/Crop as a node of the event chain (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/11-autoscript/insertvidenv.htm — Video track automation, Lock Envelopes to Events (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/videotrack.htm — the composition of the video track header (2026-09-07)
- https://creativecow.net/forums/thread/any-way-to-apply-event-fx-after-track-fx/ — the order of levels and Media FX (2026-09-07)
- https://creativecow.net/forums/thread/vide-fx-bypass-ae/ — bypass of the track chain by right-click (2026-09-07)
- https://creativecow.net/forums/thread/vegas-12-crossfades-with-color-correction/ — the divergence of the picture on a crossfade between levels (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/can-i-apply-a-video-effect-to-a-track/m-p/2793436 — in Premiere there are no track video FX (2026-09-07)
- https://mixinglight.com/color-grading-tutorials/revisiting-resolve-grouping-using-post-clip-as-a-timeline-level-grading-alternative/ — the grade levels of Resolve (2026-09-07)
- https://www.cined.com/apple-final-cut-pro-11-1-introduces-adjustment-clips-and-enhances-magnetic-masks/ — Adjustment Clips built into FCP since 11.1 (2026-09-07)

A note on availability: the forum vegascreativesoftware.info since 2026 answers
with a 301 redirect to the root https://forum.borisfx.com/ — direct links to the
old Vegas-forum threads are dead, so the feedback here was collected from Creative COW
and the Adobe/Blackmagic sites. Not checked live in VEGAS 2026: the note
is compiled from the official 2026 help and forum threads.
