---
id: F-SPD-002
title: Retime curve — time as an animatable parameter
status: candidate
priority: B
cost: L
category: time/retiming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-fx, kt-engine, kt-app, kt-otio, kt-cache]
invariants: [2, 4, 5, 7, 8, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-SPD-001, F-SPD-003, F-AUT-001, F-AUT-002]
checked: 2026-09-07
---

# F-SPD-002 — Retime curve — time as an animatable parameter

## Essence

Not "speed 50 %", but a GRAPH: for every moment of the event it is known which
frame of the material to show on it. Along such a graph one clip inside ONE
event manages to slow down, speed up, freeze and go backwards - without
cutting it into pieces. The graph is drawn in two ways: vertically either the
source time (the POSITION curve), or the speed percentage (the SPEED curve,
which the editor integrates). It is one thing, but they feel opposite, and what
can be drawn in one cannot be drawn in the other.

The constant speed of an event and the ripple model are covered in F-SPD-001,
building intermediate frames - in F-SPD-003; here only the curve itself.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 (help 23) | **Velocity envelope** - an envelope of the EVENT (not of the track) | a SPEED curve, range **−100 % … 0 % … +300 %**: 0 = freeze frame, minus = motion backwards, all on the same line, without separate commands. The event length does not change - the material ends earlier, and the remainder "is filled either by a freeze of the last frame or by additional material from the file". No Bezier: 6 interpolation presets per SEGMENT (Linear/Fast/Slow/Smooth/Sharp/Hold) plus freehand drawing with a brush. The audio does not follow the envelope at all |
| Adobe Premiere | 26 | **Time Remapping → Speed** - a fixed clip effect + a "rubber band" on the event | a SPEED curve. The keyframe is **split**: "When the speed keyframe is dragged apart and spread out beyond one frame, the halves form a speed change transition"; between the halves a grey zone and "a blue curve control" with Bezier handles. Reverse - Ctrl/Cmd-drag of a keyframe, officially **palindrome reverse**, in the speed track "left-pointing angle brackets" appear. Freeze frame - Ctrl+Alt-drag, "hold keyframes" with vertical tick marks. Downward the curve reaches only 1 %, not 0 (according to user testimony, see quotes; the help has no limit). The numbers in Effect Controls are "for reference only. You cannot edit these values directly there". The clip length changes; the audio "remains at 100 % speed" and drifts away |
| DaVinci Resolve | 21 (checked against 20) | **Retime Controls** (Cmd+R) · **Retime Curve**: the **Retime Frame** and **Retime Speed** curves · Change Clip Speed | THREE interfaces over ONE model: "the control points of each of the speed curves have a 1:1 correspondence to the speed points that are exposed in the Retime controls", edits are mirrored live, both curves can be shown at once. Retime Frame - position (vertical - source frame, horizontal - timeline frame); Retime Speed - percentages, and there you pull the SEGMENT itself, not the points. Bezier easing of retime keyframes is shared with the other parameters (21). Freeze - two speed points and red bars; Reverse Segment - a menu item |
| Final Cut Pro | 12.3 | **Retime Editor** (Cmd+R) - a coloured bar above the clip | THERE IS NO CURVE AT ALL: only segments (orange - slower, blue - faster, red - hold), the speed is pulled with a handle at the right edge of the segment. Smoothness is a separate object, **Speed Transition**: "a shaded bar on either side of the border between speed segments", trimmed by its edges, removed with a tick at a specific joint. Hold - Shift-H, 2 s by default. Backwards - only as a whole clip or with the Rewind 1×/2×/4× preset. Speed is not keyframed and there is none in Video Animation |
| After Effects | 26 | **Time Remap** (Layer → Time → Enable Time Remapping, Ctrl+Alt+T) | a POSITION curve: the value of the property is the moment of the source, "an upward angle indicates forward playback, while a downward angle indicates reverse playback". Enabling immediately sets two keys - at the start and at the end of the layer, "with vertical time values equal to their horizontal position on the timeline". Freeze - just a flat section. The same Graph Editor shows both the Value Graph and the Speed Graph of one property; the Info panel writes the instantaneous speed in units per second. The layer can be pulled beyond the length of the source. The audio is retimed TOGETHER with the video, with a drifting pitch |

## The trick behind it

**1. The position curve and the speed curve are not equal in rights: on the speed
curve neither zero nor a negative value can be drawn.** Mathematically one is the
derivative of the other, but different things can be authored in them, and this is
visible from three independent pieces of evidence. Resolve, where both curves lie over
one set of points ("1:1 correspondence") and both are editable, directly forbids it:
"it's impossible to create reverse motion using the Retime Speed curve; you need to use
either the Retime controls or the Retime Frame curve". Premiere, where the curve is only a
speed curve, goes down only to 1 % (user testimony, see quotes) - a freeze frame has to be
made with a separate gesture (Ctrl+Alt-drag, "hold keyframes"), and reverse with a third
(Ctrl-drag, "palindrome reverse"), and both draw their OWN icons on the rubber band, not
points of the curve. AE, where the curve is positional, has not a single special command:
freeze is "drag the current keyframe marker to a value equal to the previous keyframe value
so that the graph line is flat", reverse is simply a decreasing section.

The reason is geometric. The frame position is the integral of the speed; to get a stop, the
speed must be exactly zero, and the function "how much material is consumed" must stop
strictly increasing, and its inverse ("at which moment this frame plays") disappears. Everyone
who stores speed runs into this in the same way and decides the same way - by a clamp slightly
above zero plus a separate "hold" object. We have the same clamp for the same reason:
`RATE_CLAMP = (0.25, 32.0)`, and in `seg_dt` it is written "the denominator ≥
r0 ≥ RATE_CLAMP.0, never zero". Premiere's 1 % is empirical confirmation that this is not
our private matter.

Incidentally: in the Resolve 20 manual this very prohibition is printed with an error -
it is attributed to the "Retime **Position** curve" and refers to the "Retime **Speed**
curve", that is, the paragraph about the Speed curve refers to itself; fixed in 21. The typo
reveals the legacy name: the present **Retime Frame** is the former **Retime Position**, so
"position curve" in old threads and "Retime Frame" in new ones are the same thing.

**2. "Smooth acceleration" is three different objects in three editors, and none of them is
a keyframe.** In Premiere the speed keyframe is physically split:
"Speed keyframes can be split in half, acting as two keyframes for marking
the beginning and end of a speed-change transition", and further "A gray area
appears between the halves of the speed keyframe, indicating the length of
the speed transition… A blue curve control appears in the gray area" -
the handle belongs to the ZONE, not to a point, and the halves are created with a
one-frame difference. In Final Cut the acceleration is a separate entity altogether: "A transition
appears as a shaded bar on either side of the border between speed
segments" - the same two halves, but with their own duration, which is trimmed by the edges. In
Resolve 21 the same is solved by interpolation: "The Retime parameter controls in the Keyframe Editor have been enhanced to use the
same bezier easing tools as the other parameters". VEGAS has no Bezier at all - only the choice of one of six
interpolation presets per segment.

The consequence, for the sake of which this is worth recording: a SPEED CURVE does not have smoothness
for free. A piecewise-linear speed gives a continuous but non-smooth frame position, and the kink is visible
as a jerk - smoothness has to be obtained by a second entity (Premiere, FCP) or by Bezier on top (Resolve). On a
POSITION curve the ordinary keyframe handle IS the acceleration, nothing needs to be added.

**3. Percentages in AE and Premiere mean the opposite, and this is not a user's slip.** The same "50 %" in
After Effects' Time Stretch means TWICE as FAST (the layer is compressed to half), while in Premiere's
Speed/Duration it means TWICE as SLOW. Inside one Adobe package. Hence the second half of the confusion:
in AE the retime is called Time Remapping and its vertical axis is time; Premiere also calls it Time
Remapping - but its vertical axis is speed, and in the Adobe suggestions forum people explicitly ask to
rename Premiere's to "Speed Curve or whatever".

**4. Audio: the only one that drags it along the curve is not an NLE.** VEGAS writes in its help "the event's
audio will not play in sync with the video", Premiere - "the audio remains linked to the video, but remains at
100 % speed", Resolve carries three mutually exclusive statements in one pdf ("audio … will be muted" /
"can now play either pitch-corrected or un-pitch-corrected variable speed audio" / "always pitch corrected"),
FCP drags it, but only at constant speed. Exactly one retimes the audio along the curve - After Effects, and
it does so in the same way as we do: "You can remap audio files to gradually decrease or increase the pitch,
play audio backward, or create a warbled or scratchy sound", and the subsection is literally called
"Time-remap audio pitch" and advises keeping the speed below 200 %, otherwise it squeals. So a warp with a
drifting pitch is not our own amateur invention, but the AE model; the question of preserving pitch with a
separate DSP is covered in F-SPD-001.

## Pros

- Speed changes INSIDE the event, so "slowing down the middle" does not give two extra cuts that then have to
  be moved and synchronised together.
- The position curve addresses the FRAME, so the hit holds: a strike on frame 12 stays on frame 12, whatever is
  done to the curve earlier in time. The speed curve has the same hit - the result of the whole integral.
- On the position curve freeze and reverse are ordinary sections of the graph, not separate commands with separate
  icons: the interface vocabulary does not grow.
- The speed curve, on the other hand, shows the number that the editor calls out loud ("set 300 %") - on a positional
  one it has to be computed from the slope.
- One model under several interfaces (Resolve) means that the handles on the clip and the points on the curve are the
  same thing, without conversion and without the question "which is the main one".
- In FCP smoothness is an object with a duration, so it is trimmed and removed piece by piece, rather than picked by the
  shape of a keyframe.

## Cons

- Speed curve: editing an EARLY key shifts everything after it, because the frame position is an integral. Every
  "a bit faster at the start" cancels the placement of the hits further along the clip.
- And the same curve does not reach zero and does not go into negative values, so a freeze frame and reverse have to be
  entered with separate gestures and separate kinds of keys - in Premiere these are three different mouse modifiers on
  one rubber band.
- Position curve: the speed is nowhere written as a number, "exactly 50 %" turns into fitting a slope.
- Premiere's split keyframe: there are twice as many entities as visible points, "delete the key" stops being an
  unambiguous operation, and the numbers in Effect Controls are "for reference only" - they cannot be edited.
- The length model divides editors into two camps (see F-SPD-001), and Premiere has a third, its own: the clip does not
  ripple and does not overlap its neighbour, but "pushes remaining frames into the tail of the lengthened clip" - the frames
  do not disappear, but are not visible either until you make a gap and stretch the edge.
- Retime does not survive transfer between editors: OTIO, as an exchange format, knows only `LinearTimeWarp{time_scalar}`
  and `FreezeFrame`; there is no curve in the schema.

## What users say

Exactly about two models of one thing - a request in the Adobe suggestions tracker, the topic title "Make Time Remapping less confusing":

> In After Effects, Time Remapping is quite logical: set a keyframe in the clip, and tell what frame should show at that time - everything in between keyframes is interpolated accordingly. But, unfortunately, Premiere Pro messes with this principle: it kind of allows to set 'Speed' keyframes, which are not actually editable.
> — Jaydude-wb, Adobe Community, 06.08.2024, https://community.adobe.com/feature-requests-730/make-time-remapping-less-confusing-1327690

About the fact that the difference between the models is felt precisely through the length of the event:

> Unlike After Effects' Time Remapping, Premiere Pro's speed ramping constantly changes the clip length, making it frustrating to refine the timing of edits.
> — michak19395341, Adobe Community, 28.02.2025, https://community.adobe.com/feature-requests-730/improve-speed-ramping-to-behave-more-like-after-effects-time-remapping-1328339

About the fact that the speed rubber band has no full-fledged graph:

> Creating speed ramps requires manipulating small split keyframes and limited Bezier handles directly inside the timeline clip. There is no proper scalable speed or value graph comparable to After Effects.
> — Alex-Dawson, Adobe Community, 08.08.2026, https://community.adobe.com/feature-requests-730/improve-speed-ramping-to-behave-more-like-after-effects-time-remapping-1328339

And about the clamp at zero - a person expected that a freeze frame is simply 0 % on the curve:

> I've never actually played with Premiere's time remapping via keyframes before. I assumed I'd be able to ramp down to 0% speed for a freeze, but it seems you can only go to 1%…
> — Gareth R, Creative COW, Adobe Premiere Pro forum, 01.03.2024, https://creativecow.net/forums/thread/time-remapping-keyframes-only-go-down-to-1-not-0-is-that-right/

"I want Resolve's way" from an FCP user, who has no curves at all:

> I used davinci resolve the other day and absolutely love how the speed ramping works with retime curves. … I wish fcpx would have a similar implementation.
> — Darren Martin, Creative COW, Apple Final Cut Pro forum, 13.09.2018, https://creativecow.net/forums/thread/speed-ramping-question/

A counter-complaint - from AE, where the audio does follow the curve, and this gets in the way:

> The issue is now that whenever the video slows down or speeds up the audio also slows down and speeds up along with it. I want to know how to keep the audio speed default without the time remap affecting it.
> — Sachin Jadhav, Creative COW, Adobe After Effects forum, 06.04.2021, https://creativecow.net/forums/thread/audio-getting-affected-by-time-remapping/

And about the fixed window of the event in VEGAS (there were no answers in the thread):

> I know using Envelope won't shorten or lengthen the duration of the selected chunk. What I don't understand at all why there's NO ANY hotkey (pressing down CTRL or something) to make this happen?
> — Tobin Bell, Creative COW, VEGAS Pro forum, 26.10.2014, https://creativecow.net/forums/thread/velocity-envelope-mess/

## How it fits Kinotrain

### What already exists

**Variable video speed has been WORKING in Kinotrain since R.37** (items 246–251 of
`docs/porting-notes.md`). The design:

- The speed curve is an ordinary automation of the `rate` parameter of the `SlowedReverb` effect
  (`src/libs/kt-model/src/project.rs`, spec
  `rate: f64, 0.25, 32.0, 1.0, 0.01, Ratio, Smooth`). There is no separate "retime" entity in the
  model at all.
- The time map is the integral of this curve: `S(tau) = ∫rate` over the OUTPUT progress of the event,
  piecewise-quadratic; the inverse `T = S⁻¹` is the root of a quadratic equation in the citardauq form
  (`src/libs/kt-fx/src/lib.rs`: `source_progress`, `output_time_for_source`, `warp_material_ticks`).
- There is exactly one editing point for video - `clip_material_pos`
  (`src/libs/kt-engine/src/video.rs`); `layers_at`, `wanted_frames`, the player's prefetch, the preview
  warm-up and the export follow from it on their own (t247d compares as sets "what is warm" and "what is read").
- The speed is SYNCHRONOUS for the A/V link pair: `sync_warp_to_link_group` in the mutations
  `SetFx`/`SetFxAuto`/`SetFxBypass` (`src/libs/kt-model/src/model.rs`).
  The rule lives in the model, not in the FX window, because `SetFx` also arrives from inserting a chain
  from the buffer, from the morpher bake, from the format reader and from kt-cli. On this point we stand on the
  AE model, not on an NLE: the audio follows the curve and the pitch drifts.
- Length model - as in VEGAS and AE: the event holds its window on the timeline, only which material
  falls into it changes.
- Building the frames: `FrameMode::Nearest` and `Blend` - the subject of F-SPD-003.
- The preview of the event computes THE SAME map: the waveform and the filmstrip go through
  `kt_engine::ClipWarp::material_off` (`src/libs/kt-engine/src/lib.rs`), the maps are cached by `model_rev` in
  `warp_maps()` (`src/apps/kt-app/src/app.rs`).
- The key domain is the material tick, but the normalised axis is LINEAR and does not depend on the curve itself
  (R.37 C1, item 246): `(k.t − source_in) / duration`.
  Details of the domain - F-AUT-002.

### What is missing

1. **There is no second kind of curve.** We have only the speed one, and "show this frame at this moment" is not
   expressed. Resolve gives both over the same points, AE - the positional one, and it is precisely on it that
   freeze and reverse are free.
2. **There is no stop and no reverse inside the event.** `RATE_CLAMP =
   (0.25, 32.0)`; there is no freeze frame in the project at all (`grep -i freeze` over `src/` gives one loop test),
   reverse - only for the whole event (`ClipEvent.reversed`). Premiere has the same wall (1 %), but it is bypassed by two
   separate gestures; VEGAS has no wall, because the audio does not follow the envelope there.
3. **There is no smooth acceleration.** The `rate` curve is piecewise LINEAR by construction:
   `rate_at` forcibly calls `param_at(…, false)`, and `AutoProfiles::from_curves` switches off `ease` precisely for
   `"rate"` - the integral of smoothstep would be of the fourth degree. There is no Bezier anywhere (F-AUT-001), a split
   key-pair is even less present; of the six VEGAS presets per segment we have one - Linear.
4. **The retime is not visible on the clip.** The curve is edited only in the FX window
   (`src/apps/kt-app/src/fx_editor.rs` + the widget
   `src/apps/kt-app/src/automation.rs`); on the event only the level (`opacity`/`gain_db`) is drawn as a line along
   the top. In all five the retime lives RIGHT ON THE CLIP, and in three of them that is the main way.
5. **The speed is shown as a ratio, not a percentage.** `ParamUnit::Ratio`
   is printed without a suffix (`src/apps/kt-app/src/widgets.rs`): one sees
   "0.50" where all five write "50 %". Given that even Adobe split percentages in two directions, the unit of measure here is no trifle.
6. **A foreign constant speed from `.otio` does not play.** Import understands
   `LinearTimeWarp` only with scalars −1 (this is our `reversed`) and 1; any other goes to passthrough with the notice "time warp x{} is not rendered
   (kept untouched)" (`src/libs/kt-otio/src/convert.rs`).

### What gets in the way

**The circle "a parameter sets the domain of its own keys".** The automation key in our code is addressed by the
tick of the MATERIAL (`AutoKey.t`, F-AUT-002), while the retime curve IS the mapping "timeline → material". A key of such a
curve, addressed by material, depends on the value of the curve itself: move the value - and the point has moved.
The circle has already fired: before R.37 the `rate` axis was computed through the warp, and the user caught exactly this (item 246).

**The POSITION curve is free of the circle by construction, and this, not convenience, explains why Resolve authors reverse
precisely on it.** Its domain is the timeline time, which does not depend on any curve; the material sits in the VALUE of the key.
We chose the opposite break: to store the speed, leave the key's domain material, and treat the normalised axis as the output
fraction of the event - `(k.t − source_in)/duration`, where the denominator is the nominal length, which knows nothing about the warp.
The "material" tick of a key for us is a disguised offset along the body of the event; with a linear map they coincide, and that is why
`AutoProfiles::from_curves` did not change by a single line. But this break has a price, and it is visible in three places:

- **There are TWO "material positions", and they diverge.** The nominal one (`kt_model::clip_material_at`, `src/libs/kt-model/src/project.rs`) -
  the automation domain, the axis of the FX window, the trim arithmetic; the actual one (`clip_material_pos` with the warp, `src/libs/kt-engine/src/video.rs`) -
  which frame to decode. On a warped clip `raw_trim_start`
  (`src/libs/kt-model/src/ops.rs`) moves `source_in` by Δ, whereas what is actually consumed is `S(Δ)` - the trim takes the wrong frame. Recorded in R.37 as
  a known defect outside the scope ("`Split`/`TrimStart`/`punch_out_region`
  count `source_in` linearly and lie on a warped clip").
- **The model ALREADY VETOES a cut because of the circle.** The predicate
  `FxInstance::warps_time` stands FIRST in `windows_on`
  (`src/libs/kt-model/src/project.rs`): the morpher layer does not cut a clip with a warp, "the linear `source_in` of the piece would point to the wrong place"
  (`src/libs/kt-model/CLAUDE.md`). A full-fledged retime curve will extend this zone of prohibition from the layer to all geometric operations - or will force them to learn the map.
- **The coverage bar on a warped clip lies.** `coverage_runs`
  (`src/apps/kt-app/src/app.rs`) asks the cache about the range
  `source_in .. source_in + duration`, while the run is drawn linearly:
  `t0 = c.start + run.start − c.source_in`
  (`src/apps/kt-app/src/timeline.rs`). On a 0.5× clip half of the requested material is not read by the clip at all, and the warm section
  is projected into the first half of the event instead of all of it. R.37 C4 moved the waveform and the filmstrip to the warp map, but coverage - not. Invariant No. 4
  is not contradicted by this (the truth about coverage is still one, from
  `BurstCache::coverage()`), but "pixel → burst index" is computed by a second, linear
  formula. **Found while writing this note; NOT recorded in `docs/porting-notes.md`.**

**Zero and minus rest not on a precondition of the span reader, but on something else.** This is important to clarify, because the verdict of
F-SPD-001 names the reason as "keys strictly increase". In fact this precondition holds by construction:
`wanted_frames` (`src/libs/kt-engine/src/video.rs`) does not solve the integral in the reverse direction, but enumerates the OUTPUT frames on the grid, passes each through
the same map and at the end performs `sort()` + `dedup()` - the output is always a strictly increasing list, whatever the map does. There are three real walls:

1. **Invertibility.** `seg_dt` divides by `r0 + √(r0² + 2·b·ds)`, "the denominator
   ≥ r0 ≥ RATE_CLAMP.0, never zero"; at `rate = 0` the function `S`
   ceases to be strictly increasing and `T = S⁻¹` is undefined, while the calculation of the source window for audio (`chain_source_window_ext`) rests on it.
2. **The sequential cursor.** `ClipProcessor` consumes the source STRICTLY
   SEQUENTIALLY through the internal cursor of rubato - "no skips, no
   duplicates" (`src/libs/kt-fx/CLAUDE.md`); reverse means the cursor going backwards,
   which the resampler does not have, and invariant No. 2 forbids touching the FX state
   from anywhere except the audio-render thread.
3. **The mirror is a property of the WHOLE window, not of a piece.** The reverse of an event is done as a
   virtual forward stream: `fill_source`
   (`src/libs/kt-engine/src/audio.rs`) reads the real source mirrored
   inside the window `[source_in, window_end)`, and the FX chain sees an ordinary forward stream and knows nothing about the reverse; therefore reverse and warp
   compose as "WARP, then MIRROR" anchored on the consumed `used`. A piecewise reverse is not expressed by this technique - the audio would have to be cut into segments, each with its
   own window, that is, a second mechanism would have to be introduced alongside `reversed`. Transport backwards (`playback_rate < 0`, `reverse_frames`,
   test 42) is also about the whole timeline, not about a piece of the clip.

**What really hurts for prefetch is not non-monotonicity, but acceleration.**
At 32× one output frame costs 32 source frames, and the decoder goes into a seek on
every frame - the class on which we have already been burned (item 14 of R.14: "when
the project fps > the source fps it went into a seek + decoder re-creation on every
second frame"). During live playback this goes through ONE MediaWorker per source
(invariant No. 5) with a deadline: if it does not make it - silence and the last frame.
On slow motion, on the contrary, `dedup` works in our favour (t247d:
60 frames instead of 120).

### Which invariants it touches

- **No. 7** (one composition code): the blending of frames is done in the common plan code,
  not separately in the CPU kernel and in WGSL, and a second kind of curve must remain a
  recomputation of the same plan, not a second path.
- **No. 5** (one MediaWorker per source) and **No. 4** (the cache is the single source of truth
  about coverage): acceleration hits the queue with a deadline, and the warmth bar on a warp is
  already computed by a linear formula.
- **No. 2** (FX state only in the audio stream) and **No. 11** (audio byte-for-byte) - the
  obstacle for zero and negative speed.
- **No. 8** (scrolling/zooming does not generate decoding) - observed: the retime changes only the
  display "pixel → thumbnail key".
- **UI rule 17** ("paired ways of one action behave identically") - Resolve shows the price of three
  inputs into one model: three different answers to "will the neighbours move?". If we get both a
  strip on the clip and the FX window, it must be one curve with one behaviour.
  **13** ("do not waste space") - the retime strip on the event competes with the level line that
  already lives there. **20** ("nothing grows by itself") - about the length model: the event must not
  change its length by itself.

## Estimate

| What | Answer |
|---|---|
| Scope | L - a whole round; half (the speed curve with an honest integral) is already done in R.37 |
| Where the main work is | kt-fx (the map: clamp, hold, the backward branch), kt-model (geometry: trim/split/cut by the map), kt-app (the strip on the event + a second kind of curve in the shared widget) |
| Project format | extending the range of `rate` does not require a version bump; a separate POSITION curve as a second store - yes, `KT_META_VERSION` 4 → 5 (`src/libs/kt-otio/src/meta.rs`). The curve has no exchange representation: OTIO knows only `LinearTimeWarp{time_scalar}` and `FreezeFrame` |
| Tests to be written | trimming a warped clip takes the right frame (kt-model); the coverage strip by the warp map (kt-app `layout_tests.rs`); a second kind of curve = the derivative of the first, an edit in either form gives the same keys (kt-app); hold does not break `T = S⁻¹` (kt-fx `integral_map.rs`); a reverse segment ↔ audio (kt-engine `reverse` + `fx`); prefetch == what was read on a jerky curve (extension of t247d) |
| Depends on | F-SPD-001 (constant speed and the length model), F-SPD-003 (building frames), F-AUT-001 (Bezier), F-AUT-002 (key domain) |
| Breaks compatibility | no, as long as a second curve is not introduced as a separate store |

## Verdict

Take it, but not entirely and not in one piece. Half is already standing since R.37 - the speed
curve with an honest integral, shared by the A/V pair; the nearest work is not a new feature but the
DEBTS of this half: the trim, split and cut by the layer, which count `source_in` linearly, and the
coverage strip, which draws the warm section in the wrong place.

Do not introduce the second curve as a separate entity: we have one automation widget and one key
domain, and a second curve of the same meaning is a direct violation of item 17 of the checklist. The right
goal is the Resolve path without its problem: ONE set of keys and a second KIND (frame instead of percent) as a
recomputation. Then the question "are the keys recomputed when switching?" does not arise, and the asymmetry of rights
("zero and minus are drawn only in the form of position") ceases to be architecture and becomes a consequence of the
clamp, which can be removed deliberately and in parts.

Hold and a reverse section are a separate round with an honest price: they break the invertibility of the map, the
sequential cursor of the resampler and the window-wise mirroring of the reverse, rather than "require a button". The
reference of F-SPD-001 to the precondition "keys strictly increase" as the reason for refusal is wrong: it holds by
construction (`wanted_frames` sorts and deduplicates), and the decision must rest on the three walls above.

**The round took place - R.57 (items 453-454), at the owner's direct demand.** All three walls were removed by one decision: to
SPLIT TWO AXES - the signed position P(τ) = ∫rate (the read address) and the accumulated consumption A(τ) = ∫|rate|
(the resampler's cursor, monotonic by construction).

1. *Invertibility* - it was the WRONG thing that had to be inverted. The inverse is needed in two places (trims of a stretched clip), and there
   they ask "how far can the edge be pulled", that is A, not P. `output_time_for_consumption` is always invertible, except for
   a zero-speed plateau, where the rule is named: the left edge of the plateau.
2. *The sequential cursor* - it remained sequential: the rubato eats a virtual stream at speed |rate|, and the direction lives in the ADDRESS
   (runs of constant sign, within a run ±1 per frame).
3. *The window-wise mirror* - "Reverse" remained a property of the whole event and is applied ON TOP OF the map, around the SPAN of the position (min P, max P).

Zero became a freeze frame (the video holds the frame, the audio is silent, the reverb rings out); the range is −32…32. The byte-for-byte
invariance of old projects is proven by a frozen oracle in a test, not by hope. Left outside the scope: the second KIND of curve (frame instead of
percent) - it was the main idea of this note and does not depend on the sign.

## Sources

- Adobe After Effects CS5 Help (archived official PDF, sections
  "Time-remapping", "Time-remap audio pitch", "In/Out points"):
  https://help.adobe.com/archive/en_US/aftereffects/cs5/after_effects_cs5_help.pdf
  — two starting keys "with vertical time values equal to their
  horizontal position on the timeline", "an upward angle indicates forward
  playback, while a downward angle indicates reverse playback", freeze
  as a flat section, retiming of audio with a drifting pitch (accessed
  2026-09-07)
- Adobe Premiere Pro CS5 Help (archived official PDF, sections on Time
  Remapping):
  https://help.adobe.com/archive/en_US/premierepro/cs5/premierepro_cs5_help.pdf
  — splitting of the speed keyframe and "blue curve control" in the grey zone,
  "palindrome reverse" via Ctrl-drag, "hold keyframes" via
  Ctrl+Alt-drag, "for reference only. You cannot edit these values directly
  there", "pushes remaining frames into the tail of the lengthened clip",
  "the audio … remains at 100 % speed" (accessed 2026-09-07)
- DaVinci Resolve 20 Reference Manual (ch. 51, pp. 1071–1082) and DaVinci
  Resolve 21 Reference Manual (ch. 58, pp. 1254–1264; ch. 60, p. 1305):
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf
  and https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
  — "1:1 correspondence", Retime Frame as the position curve, the prohibition of reverse
  on the Speed curve (printed with an error in 20, corrected in 21), Bezier easing of
  retime keyframes (accessed 2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/edit — "graphical
  curve editor for both frame position and playback speed" (accessed
  2026-09-07)
- https://support.apple.com/guide/final-cut-pro/retime-clips-verf4d4b1b46/mac
  and https://support.apple.com/guide/final-cut-pro/add-speed-transitions-ver41a2cd3ae/mac
  — the composition of the FCP retiming section (not a single article about a speed curve or speed keyframes) and "a shaded bar on either side of the border between speed
  segments" (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/event_envelopes.htm
  and https://help.magix-hub.com/video/vegas/23/en/content/topics/3-projects/hfr-clips.htm
  — velocity as an envelope of the EVENT, −100…+300 %, "duration that is not
  changed by velocity envelopes", "the event's audio will not play in sync
  with the video" (accessed 2026-09-07)
- Adobe Community and Creative COW threads listed in the section "What users say"
  (dates there); on the inverted percentages of AE and
  Premiere - Navarro Parker, Creative COW, 25.10.2013,
  https://creativecow.net/forums/thread/converting-between-ae-premiere-speed-ramps/
- https://opentimelineio.readthedocs.io/en/latest/tutorials/otio-serialized-schema.html
  — `LinearTimeWarp.time_scalar`, `FreezeFrame`; there is no retime curve in the
  schema (accessed 2026-09-07)

**Caveats.** `helpx.adobe.com` (the current Premiere/AE help) does not open at all from this
environment, so Adobe's official wording is taken from the ARCHIVED PDFs of the CS5/CS5.5 editions on `help.adobe.com`. The mechanics
(splitting of the key, palindrome reverse, the two Time Remap keys, value/speed
graph) are confirmed by the community threads of 2024–2026 quoted
above, but the names of menu items and mouse modifiers in version 26 may have
changed - entry into Premiere 26 today goes through the FX badge of the clip
[NOT VERIFIED against the v26 primary source]. The behaviour of neighbouring clips in v26
is also not confirmed: the CS5 help says "trims the tail", while a user in April 2026 writes
"pushes everything on your timeline around".
`forum.blackmagicdesign.com` is wholly blocked by a WAF challenge, so there is no live feedback on
Resolve - only the primary source. The VEGAS help is taken from
version 23 (the latest online; the `/2026/` catalogues return 404), the numeric
limits for 2026 may have shifted.
