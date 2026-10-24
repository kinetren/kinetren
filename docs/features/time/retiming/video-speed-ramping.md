---
id: F-SPD-001
title: Event speed — constant and variable
status: candidate
priority: B
cost: M
category: time/retiming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-fx, kt-app]
invariants: [1, 2, 4, 5, 7, 8, 11]
ui_rules: [4, 9, 13, 17, 20]
round: R.37
point: 247
tests: [246, 247, 248, 249, 250]
depends_on: []
checked: 2026-09-07
---

# F-SPD-001 — Event speed — constant and variable

## Essence

The editor says "make this event twice as slow" and means one of two DIFFERENT things. The first: the
material is the same, the event became twice as long, and everything after it moved right. The second: the
event is the same length, twice less material passes through it, and the neighbours did not move. On top of
this a "variable" speed is layered: inside the event it travels along a curve (a ramp, a slowdown towards the
hit, a freeze frame). And to each variant three questions are attached, which the editor always asks: what
happened to the sound, what happened to its pitch, and where to get the frames that are not in the material.

## Who has it and how

The question for all is one: does changing the speed change the LENGTH of the event, do the neighbours move,
and what happens to the sound in the process.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Playback rate + Velocity envelope | TWO independent mechanisms. `Ctrl`+dragging the edge ("hold Ctrl while dragging the right or left edge of an event") stretches the event: the length changes, the material is the same; for an audio event the percentage after stretching is written in the lower right corner when Active Take Information is on. The Velocity envelope - only on VIDEO events, and it does not change the length: "Each video event in your project has a specific duration that is not changed by velocity envelopes". The presets - "from 100% reverse-speed playback to 0% playback (a freeze-frame effect) up to 300%", a negative value plays the event backwards. The top of the envelope was raised to 1000% back in VP14 build 161 (20.09.2016): "to increase the maximum possible overall velocity of video on the timeline to 40 times normal speed". Smoothing - a separate switch of the event, Resample (Force/Smart). The pitch of the sound is adjusted by its own pair of knobs in Event Properties (Time stretch/pitch shift, the élastique engine from zplane) |
| Adobe Premiere | 26.x | Speed/Duration, Rate Stretch tool | One dialog for both models: Speed and Duration are linked by a gang button ("To have the speed or duration adjusted together click the gang button so that it shows an unbroken link"), plus three checkboxes - Reverse Speed, Maintain Audio Pitch ("Check this to have the pitch remain the same as the duration is changed") and Ripple Edit, Shifting Trailing Clips ("Checking this will cause any adjacent clips to move if needed"). The same result by a gesture - the Rate Stretch tool on the edge of the clip. How to obtain an intermediate frame - a separate list Time Interpolation (Frame Sampling / Frame Blending / Optical Flow, see F-SPD-003). Variable speed - not here, but in Time Remapping (F-SPD-002) |
| DaVinci Resolve | 21 | Change Clip Speed, Retime Controls | The dialog gives the same thing in three ways - Speed %, Frames per second, Duration - plus Ripple Sequence, Pitch Correction, Reverse Speed, Freeze Frame. Ripple Sequence moves everything that is behind the clip to its new length; with the tick removed the clip stays in place. Variable speed - the retime strip and the retime curve (F-SPD-002), smoothing - Optical Flow: "building new frames so you can slow down and extend the action in a sequence without repeating or blending frames" |
| Final Cut Pro | 12.3 | Retiming (Retime Editor) | The length model is the main one and by default cross-cutting: "Constant speed changes alter the duration of a clip. By default, if a constant speed change causes the duration of a clip to become longer or shorter, all clips coming after it ripple forward or backward". In the Custom Speed window ripple is removed by unticking, and then "the clip remains at its current length but a shorter piece of the action is seen" - that is, exactly the VEGAS velocity model. The retime strip is coloured by direction (orange - slowdown, blue - speed-up). Pitch is preserved BY DEFAULT. Quality - Normal / Frame Blending / Optical Flow, on Apple silicon Smooth Slo-Mo (Best, Machine Learning) was added. Automatic Speed brings HFR material to the timeline |
| After Effects | 26.x | Time Stretch, Time Remapping, Timewarp | Not an event but a layer: Time Stretch sets a multiplier and changes the length of the layer; further "Time-stretching, time-remapping, and the Timewarp effect are all useful for creating slow motion, fast motion, freeze frame". There is nothing to move for neighbours - the layers lie in parallel, there is no concept of ripple. Smoothing - the Frame Blending switches of the layer (Frame Mix / Pixel Motion) plus the master toggle of the composition. There is NO pitch preservation at all: the official community answer is to stretch the sound in Audition (or in Premiere with Maintain Pitch) and bring it back as a finished file |

## The trick behind it

**1. Two speeds MULTIPLY, and their clamps are different.** In Vegas the property of the event
(Playback rate, up to 4×) and the velocity envelope are different mechanisms with different
consequences: the first changes the LENGTH of the event, the second does not touch the length at all.
They work simultaneously and multiply: since VP14 the envelope goes up to 1000%,
and in the release notes this is recorded precisely as "maximum overall speed on the
timeline - 40×". At the same time the envelope's preset menu ends at 300%,
so on the Boris FX forum in August 2026 people count aloud "4× as a property × 3×
of the envelope = 12×" and ask to raise it to 100×, while an old-timer replies that the ceiling
was broken "long ago". The result: by the look of the event one cannot tell its speed -
one must open the properties AND look at the envelope.

**2. Ripple is not a convenience but a switch of the MODEL, and it is hidden in a tick.**
Apple states it directly: with ripple removed "the clip remains at its current
length but a shorter piece of the action is seen". That is, one and the same tick
turns the "Premiere" model (speed sets the length, the tail of the edit moves) into the
"VEGAS" velocity model (the length is fixed, a piece of the action changes) - and
this is the only thing that separates them. In Resolve this same tick is called Ripple
Sequence, in Premiere - Ripple Edit, Shifting Trailing Clips. None of the three
calls it a change of model, although behind it stands the question "is the change local or
global": with ripple, changing the speed of one event rewrites the timing of everything
after it, including the neighbouring tracks, if they were not locked.

**3. "Preserve pitch" is a separate tick, because it is a different algorithm.** Resampling (speed and
pitch move together, "a record at a different speed") and time-stretching with pitch preservation (a phase
vocoder, in Vegas - élastique from zplane) are two different DSPs, not a mode of one. Therefore in FCP the
pitch is preserved BY DEFAULT and is switched off by a menu item; in Premiere it is a checkbox in the same dialog;
in Resolve - Pitch Correction, which "becomes less transparent as the speed change grows"; in AE it is absent
altogether; and in Vegas there is no sound on the velocity path at all - velocity lives only on the video event,
and the paired audio clip has to be stretched by hand. Hence the whole class of complaints "after slowing down the
sound drifted away".

**4. The axis of the speed curve cannot be in the consumed material - this is an equation on itself.** A detail
that is in none of the help pages, and we found it in our own code (R.37, item 246): if the key of the `rate` curve
is indexed by the fraction of CONSUMED material, then the position of the key depends on the values of the curve
itself - move the value, and the point has moved across the screen; the right part of the graph is not reachable at all,
because one can only get as far as `duration · avg_rate`, and the auto-key at the end of the curve never fires. Therefore the
axis of the speed curve must be in the OUTPUT time of the event - otherwise the curve editor lies.

## Pros

- The model "speed sets the length": the speed is visible in the geometry - the event became
  longer, so it is slowed; no separate number needs to be looked for. As a side effect comes the
  trick "enter the duration, and let the editor decide the percentage" - fitting a piece to a musical
  phrase is done by typing the timecode, not by picking.
- The model "the length is fixed" (velocity): changing the speed moves NOTHING after the event - neither
  neighbours, nor transitions, nor markers; a ramp inside a piece locked by length (an advertisement, a
  timing) is done without re-editing the tail.
- The split of "speed" and "how to obtain an intermediate frame" into two independent controls (Vegas
  Resample, FCP Video Quality, Premiere Time Interpolation) allows paying for expensive smoothing only where
  someone looks at it.
- Preserving the pitch by default (FCP) means a slowed interview stays intelligible without a single setting,
  and switching the tick off becomes a conscious artistic device.
- Negative and zero speed on the same envelope (Vegas: −100%…0%…1000%) gives a freeze frame and "rewinding
  back" without a separate object on the timeline: reversing a piece is a point of the curve, not a property of the event.

## Cons

- Two multiplying mechanisms (Vegas) mean that the "speed of the event" is not a number, but the product of two
  numbers from different places of the interface; on the forum this regularly surfaces as the question "why does more
  than 12× not give anything".
- The fixed-length model silently loses material: slowed by half - the second half of the action simply did not
  fit and is not shown, and there is no warning.
- The ripple model just as silently breaks synchronisation with other tracks: everything on THIS track moved, while
  parallel music/titles stayed.
- Time-stretching with pitch preservation spoils the sound the more noticeably, the larger the coefficient; at strong
  slowdowns the choice becomes "an unintelligibly low voice" or "a metallic artifact".
- Speed and frame processing are orthogonal settings, so "just slow down" by default gives repeated frames and a jerk:
  from the user's point of view the feature "works", but looks broken.
- Editing the velocity envelope in Vegas 2026 still misses the event (the bug was filed in April 2026, not closed by September) -
  the price of the envelope being drawn over the event and competing with its own grab zones.

## What users say

> I wanted to ask if there is a way to watch playback at a faster speed while
> keeping the audio at the same pitch
> — Yazura, 09.07.2026,
> https://forum.borisfx.com/t/playback-audio-when-playing-at-faster-speed/24562

> ExpressFX Time Stretch changes the audio playback rate while attempting to
> not alter the pitch. If you double the playback speed of a video clip that
> contains an embedded audio stream, try a 50% time stretch on the audio
> portion.
> — Howard-Vigorita, reply in the same thread, 10.07.2026,
> https://forum.borisfx.com/t/playback-audio-when-playing-at-faster-speed/24562

> I speed up the video to 4x and then add a velocity envelope to speed it up
> even further.
> — galKINO, 25.04.2026,
> https://forum.borisfx.com/t/velocity-envelopes-and-legacy-plugins-in-vegas-2026/22851

> the footage comes out all choppy, and when I scrub through it, every other
> frame is the same as the previous one.
> — James Strecker, 20.09.2016,
> https://creativecow.net/forums/thread/60fps-slow-motion-not-workingae/

> I don't have a clear repro yet, but this is a bug.
> — VegasFlorian (developer) about the cursor misses when editing velocity envelopes,
> 02.04.2026,
> https://forum.borisfx.com/t/timeline-jumping-when-editing-with-velocity-envelopes/21603

> Is there a way to time-stretch a comp, but keep audio clip pitch and
> duration?
> — hhhhhhhhhhhhhh5, 07.10.2020; accepted answer by Rick Gerard: "Stretch your
> audio in Audition.",
> https://community.adobe.com/questions-529/time-stretch-but-keep-audio-pitch-33246

## How it fits Kinotrain

### What already exists

R.37 (items 246–250) closed most of the mechanics, so the note must be read not as a "hole", but as
"one of the two models is done".

- The speed is the `rate` parameter of the `SlowedReverb` slot (`src/libs/kt-model/src/`
  `project.rs`, declaration `params_for!`: 0.25…32, step 0.01), automated by an ordinary
  curve; `FxInstance::warps_time()` marks such a slot.
- The video follows the integral of the speed: `clip_material_pos` and `clip_video_warp` in
  `src/libs/kt-engine/src/video.rs`, the map - `kt_fx::warp_material_ticks`
  (`src/libs/kt-fx/src/lib.rs`, `RATE_CLAMP = (0.25, 32.0)`). The edit is ONE, from it
  follow `layers_at`, `wanted_frames`, prefetch and export - invariant No. 7 is observed by construction.
- The frame-extraction mode is `FrameMode::{Nearest, Blend}`; the blending is computed in the COMMON code of the plan
  (`clip_frame_keys` → `blend_frames`), not in the backends, and both frames of the pair get into prefetch
  (tests 250/250b). The continuation is F-SPD-003.
- The A/V pair keeps the speed shared: `sync_warp_to_link_group` in the mutations
  `SetFx`/`SetFxAuto`/`SetFxBypass` (`src/libs/kt-model/src/ops.rs`); the slot
  is found by kind, not by index (test 248).
- The preview of the event computes the stretch with the same map: `ClipWarp::material_off`
  (`src/libs/kt-engine/src/lib.rs`), the cache of maps by model revision in
  `src/apps/kt-app/src/app.rs` (`warp_maps()`), test 249. Invariant No. 8 is intact:
  only "pixel → key" changes, there is no new decoding.
- The reverse of the event is a separate boolean `ClipEvent.reversed`, composed with the warp
  as "warp, THEN mirror" (test 247c).
- Tests: `src/libs/kt-engine/tests/video_warp.rs` (247, 247b–e, 250),
  `src/libs/kt-engine/tests/fx.rs` (246, 246b), `src/libs/kt-model/tests/`
  `fx_auto.rs` (248), `src/apps/kt-app/src/layout_tests.rs` (248-video,
  249), live scenario `scripts/live/slowed.py`.

### What is missing

1. **The second model entirely.** In `Command` (`src/libs/kt-model/src/model.rs`)
   there is not a single operation "speed ↔ length": there are `TrimStart`/`TrimEnd`/
   `Split`/`MoveClip`, and speed arrives only through `SetFx`. To slow a piece and show it
   in full, the user must manually pull the edge of the event to the needed length - that is, do the
   arithmetic `duration ÷ rate` for the editor.
2. **Ripple is absent as a concept.** The only mention in the code is the comment "English: "No ripple.""
   in `src/libs/kt-model/src/ops.rs`.
3. **Preserving the pitch cannot be done by a "tick":** our warp is `rubato::SincFixedOut` with the ratio `1/rate`
   (varispeed), and the pitch follows the speed by construction. Moreover, for an effect named
   Slowed+Reverb this is not a defect, but a genre. Time-stretching is a second algorithm.
4. **There is no zero and no negative speed.** `RATE_CLAMP` = 0.25…32: there is no freeze frame
   (0%) and no "rewinding inside the event" (−100%); reverse is a property of the whole
   event. Vegas has one envelope from −100% to 1000%.
5. **The speed is not visible and is not called that.** The number lives inside the slot
   `"Slowed+Reverb"` (`src/apps/kt-app/src/fx_editor.rs`), nothing is written on the event itself;
   an editor looking for "speed" will find it only through the FX picker. Vegas writes the stretch percentage
   right in the corner of the audio event.
6. **`Split`/`TrimStart`/`punch_out_region` count `source_in` linearly** and lie on a warped clip - recorded in the
   decisions of R.37 (`docs/user-feedback.md`) as a known defect outside the scope of the round. For now it is treated
   by a veto: the morpher layer does not cut such a clip at all (test 247e).

### What gets in the way

- **Monotonicity of the map is a precondition of the decoder, not a style.** `read_video_span`
  (`src/libs/kt-media/src/lib.rs`) requires that the keys be "aligned to the grid, strictly increasing", and on this stands
  "the GOP is decoded once per span". A negative speed inside the event breaks this precondition rather than merely
  extending the clamp; besides, it makes `wanted_frames` non-monotonic
  (`src/libs/kt-engine/src/video.rs`), and the equality "prefetch == what was read" is backed by invariant No. 4 and test 247d.
- **A precedent of reverse transport.** `render_frames_raw`
  (`src/libs/kt-engine/src/audio.rs`) deliberately plays clips WITHOUT FX: when material is fed backwards the stream
  states (reverb, resampler) are meaningless - "the tail of the reverb would sound from the future", and it is also written there that
  Vegas behaves the same way in reverse scrubbing. A negative speed inside the event is the same case, only now it would get into the
  ORDINARY playback and into the export, where FX cannot be switched off (invariants No. 2 and No. 7).
- **The length of the event and the axis of the curve are linked.** The keys are stored in material ticks, and the share along the event is computed as
  `(k.t − source_in)/duration` (`AutoProfiles::from_curves`). The operation "speed changes the length" changes
  `duration` - and thereby re-lays out ALL curves of the event. This must be decided explicitly (recomputing the keys in the same mutation), otherwise the ramp will drift.
- **Ripple against UI rule No. 20** ("nothing grows or switches off by itself", "smart logic that re-arranges the neighbours for the user is a defect"). This is
  not a prohibition but a requirement: ripple may exist only as an EXPLICITLY selected mode/tick in the dialog, not as automatic behaviour of a speed
  change, and it must fall into the same undo step, as is already done for `reconcile_transitions` and `sync_warp_to_link_group`.
- **Time-stretching runs into determinism.** `GOLDEN_AUDIO` was regenerated "the second and last time" by a direct decision of the user (item 246), so a
  new DSP must by default be switched off and change not a single byte in existing projects (the rule of the kt-fx crate: the default of a new parameter =
  the previous behaviour byte-for-byte). Invariants No. 1 and No. 11.

### Which invariants it touches

No. 1 (audio f32/48k - time-stretching lives in the same path), No. 2 (the state of the time-stretcher - only in the audio-render thread), No. 4 (a change of length/speed changes the set of
needed frames - coverage is counted by the cache), No. 5 (the MediaWorker queue and the precondition of the span reader), No. 7 (the map "tick → source frame" is one for preview and
export - the edit only in `clip_material_pos`), No. 8 (previews of events by the map, but
without new decoding), No. 11 (GOLDEN_AUDIO/GOLDEN_VIDEO). UI rules: No. 4 (a useful
default - the "nearest" frame and ripple switched off), No. 9 (no second switches of the same meaning - the slot's tick already exists), No. 13 (the speed percentage
is to be written in the ALREADY occupied place of the event, like a marker/level, not as a new strip),
No. 17 (the `Ctrl`-drag gesture on the edge and the menu item must give the same thing), No. 20.

## Estimate

| What | Answer |
|---|---|
| Scope | M - 2–3 stages: the operation "speed ↔ length" + ripple as an explicit mode + showing the percentage on the event. "Preserving the pitch" is NOT included here: it is a separate DSP and a separate note (L) |
| Where the main work is | kt-model (a new command, recomputation of curves, a rule in `mutate_on` next to `sync_warp_to_link_group`), kt-app (the `Ctrl`+edge gesture, the percentage display, the dialog), kt-engine - almost untouched: the warp map already exists |
| Project format | does not touch: the speed is already serialised by the slot parameter, the length is an ordinary `duration` |
| Tests to be written | the command changes the length and does not spoil the curves (model, undo in one step); ripple moves exactly what is after the event and nothing on other tracks; the A/V pair changes length synchronously; `Split`/`Trim` on a warped clip count `source_in` by the map (closes the known defect of R.37); a harness test of the gesture and the percentage display |
| Depends on | F-SPD-002 (the retime curve - the UI of variable speed), F-SPD-003 (generation of intermediate frames), F-AUT-002 (key domain) |
| Breaks compatibility | no |

## Verdict

Take it - but precisely the MISSING half: the operation "the speed sets the length of the
event" with an explicit ripple and the percentage shown on the event. The expensive part (the warp
map, prefetch, preview, A/V pair synchrony) is already done in R.37, so the second model is an operation of the
model plus a gesture, not a round of the engine.

Negative and zero speed inside the event is NOT taken: it breaks the precondition "keys strictly increase" of the span
reader and repeats the problem of reverse transport (FX are meaningless under reverse feeding), while the freeze frame and reverse
for us are already expressed differently.

> **REFUSAL LIFTED in R.57 (items 453-454)** at the owner's direct demand.
> The grounds for the refusal were wrong twice: the precondition "keys strictly increase" holds by
> construction (this was noticed back in F-SPD-002), and "FX are meaningless under reverse feeding" is about reverse
> TRANSPORT, where blocks are unrolled whole. For a clip the feeding stays FORWARD: backwards goes the read
> ADDRESS, and the chain sees an ordinary forward stream - exactly as with the "Reverse" flag, which has existed from the start.
> The real wall was one (the invertibility of the map), and it was removed by splitting the axes.

"Preserving the pitch" is a separate note and a separate round: it is a second DSP,
not a tick on the existing one, and it must by default change not a single byte of
the sound.

## Sources

- https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/adjusting_eventslength.htm — `Ctrl`+drag of the edge, percentage display, Time stretch/pitch shift (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/event_envelopes.htm — velocity envelope: the event length does not change, −100%…0%…300%, negative = reverse (2026-09-07)
- https://help.magix-hub.com/video/vegas/en/content/topics/faq/faq_slowmo.htm — three ways of slowing down and the Resample switch (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/adjustmentevents.htm — "Certain functions, such as reversing or adding a Velocity envelope, do not apply to adjustment events" (2026-09-07)
- https://forum.borisfx.com/t/vegas-pro-release-history/24284 — VP14 build 161: envelope ceiling 1000%, overall speed 40× (2026-09-07)
- https://forum.borisfx.com/t/suggestions-for-boris-regarding-vegas-pro-2026-very-long-post/24911 — the count "4× × 3× = 12×" and the reply by jetdv, 01.08.2026 (2026-09-07)
- https://forum.borisfx.com/t/timeline-jumping-when-editing-with-velocity-envelopes/21603 and https://forum.borisfx.com/t/patch-vegas-pro-2026-0-3-build-189/25206 — an open bug of editing envelopes, April–September 2026 (2026-09-07)
- https://forum.borisfx.com/t/playback-audio-when-playing-at-faster-speed/24562 — pitch at accelerated playback, 07.2026 (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/change-clip-speed-ver40b00150/mac — a constant speed changes the length and ripples by default; ripple removed = "shorter piece of the action" (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/preserve-pitch-in-retimed-clips-verb6aca45d/mac — the pitch is preserved by default (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/whatsnew — Resolve 21, Optical Flow speed change (2026-09-07)
- https://davinciresolveclub.com/change-clip-speed-davinci-resolve/ — the fields of the Change Clip Speed dialog, Ripple Sequence, Pitch Correction (2026-09-07)
- https://helpwiki.evergreen.edu/wiki/index.php/Speed_and_Duration_-_Premiere_Pro — the wording of Maintain Audio Pitch, Ripple Edit Shifting Trailing Clips, the gang button (2026-09-07)
- https://community.adobe.com/questions-529/time-stretch-but-keep-audio-pitch-33246 — AE has no pitch preservation; the answer "stretch in Audition" (2026-09-07)
- https://creativecow.net/forums/thread/60fps-slow-motion-not-workingae/ and https://creativecow.net/forums/thread/frame-blending-issue-when-changing-speed-in-premie/ — jerks from repeated frames, the behaviour of frame blending (2026-09-07)
- Our side: items 246–251, `docs/user-feedback.md`
  round R.37.

Caveats on reliability. None of the five editors was run live.
The Vegas lines are collected from the official help (versions 22/23 - the topic structure
of 2026 did not open at the same addresses) and the Boris FX forum of 2026; the
Premiere line relies on secondary sources, because `helpx.adobe.com` does not open from
this environment - the wording of the Speed/Duration dialog is taken from the Evergreen training wiki and therefore I mark it as **[NOT VERIFIED against the
primary source]**. The Resolve dialog fields are taken from a secondary analysis
(davinciresolveclub, 2026) - **[NOT VERIFIED against the Reference Manual]**; there the
pair "Maintain Timing / Stretch to Fit" for keys in retiming is also mentioned, which could not be confirmed by the primary source, so it has not been
moved into the body of the note. `reddit.com` and `web.archive.org` are unavailable, and the `WebSearch` budget
is exhausted - the search went through direct queries to Yahoo and to the built-in search of the Boris FX
Discourse forum.
