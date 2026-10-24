---
id: F-AUT-002
title: Key domain — source media or timeline time
status: already-have
priority: B
cost: M
category: automation/keyframes
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-engine, kt-fx, kt-otio, kt-app]
invariants: [3, 7, 11]
ui_rules: [20]
round: R.33
point: 212
tests: [190, 212, 213, 215, 221]
depends_on: []
checked: 2026-09-07
---

# F-AUT-002 — Key domain — source media or timeline time

## Essence

The editor put an animation on a clip: a blur ramps up toward the middle and falls off
toward the end. Then he trimmed the start of the clip, moved it along the timeline, cut it in
half and reversed it backwards. The question is exactly one: where is this animation now? The answer depends on what the key is tied to — to the SOURCE FRAME,
to the body of the event, or to a minute on the timeline. The user does not make this choice and usually does not even know that it exists — but feels it with every
trim and every ripple insert.

## Who has it and how

The question for each one is the same: what is the effect KEY on a clip tied to, and what do trimming the start, shifting, changing speed and reversing do to it.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Premiere | 26.x | Effect Controls keyframes ↔ Show Track Keyframes | an effect key is held by the MATERIAL of the clip: the Effect Controls ruler is marked by the clip, a shift carries the animation with it, and slip (sliding the material under a fixed window) DRAGS the keys along with the frames — there is an open request on this, "allow slip without moving keys". Trimming does not delete keys, it moves them outside the visible range. Track keys are a separate rubber band in SEQUENCE time (the switch Show Clip Keyframes ↔ Show Track Keyframes in the track header), and Adobe presents their independence from clips as an advantage. Key insertion has a "Pin To Clip" option: with it, keys beyond the target's out point are inserted but not shown |
| After Effects | 26.x | keys of the LAYER's property | the domain is the time of the LAYER, not of the material, and this is NOT the same as in Premiere. Trimming in/out does not move keys (they stay at their times and simply do not play); slip with the Pan Behind tool moves the frames and leaves the keys in place — unless they were selected beforehand. Time Remapping is then computed in source time, which is why for a trimmed layer the first remap key lands at the start of the WHOLE source. There are no tracks — a track domain does not exist |
| VEGAS Pro | 2026.0 | Event FX keyframes ↔ track envelopes | the keys of the chain belong to the EVENT, and the help specifically notes that event keys are not visible on the timeline (unlike track keys). Envelopes and track keys live in project time and are computed BEFORE event ones. Whether they follow the event is decided by the MENU item `Options → Lock Envelopes to Events`; the wording of the help covers exactly moving along the timeline — about trimming, ripple and moving to another timeline it is silent |
| DaVinci Resolve | 20.3.2 | Inspector/Color keyframes; Fusion — its own scale | the grade and its keys belong to the clip and are addressed by the SOURCE TIMECODE. Ordinary copying of a grade from clip to clip does not carry the keys at all; to move them, one goes through Gallery, where the domain is chosen aloud: Keyframes Aligning **Source Timecode** (the same material or its duplicate) / Aligning **Start Frames** (a foreign frame) / **No Keyframes**. A Fusion composition is a separate carrier with its own scale: its frame does not coincide with the frame of the trimmed clip on the Edit page |
| Final Cut Pro | 12.3 | keys in the clip inspector | there are no tracks as objects, so the track domain does not exist at all: both the level and its keys belong to the clip — the help says "at specific points in a clip". What retiming and reversing do to keys, Apple does not document; the domain surfaces only when saving a preset with keys — the switch Stretch to Fit (default) / Maintain Timing |

Separately about **speed change and reversing**: none of the five editors documents
what happens to keys in this case. Premiere has only forum bug reports ("keys disappear from view at speed ≠ 100 %, they do not trigger during a speed ramp"); VEGAS's help describes the velocity envelope
and reversing but is silent about event-FX keys; Apple is silent too. That is, the most
frequent question of an editor is the one the industry answers only with bug reports.

## The trick behind it

**1. The domain is visible on the graph's ruler, and it shows NOT the time the timeline shows.** For us the automation window's axis is the material of the event:
`clip_ctx` (`src/apps/kt-app/src/automation.rs`) sets
`range = (source_in, source_in + duration)`, and the labels are drawn by the same
`format_ruler_time` as the project ruler. For a clip taken from the middle of a
file, the graph starts not at zero and not at a minute of the edit, but at 0:00.5 of
the SOURCE. The second column in table mode is the 0..1 fraction — by the direct
request of the user to show "both one and the other". The reverse side: a key hidden by trimming, outside this range, is cut off by `painter_at` — it is absent from the graph, but present in the table, because the table builds rows from all `keys` without regard to `range`.

**2. Copy-paste of an envelope knows about the domain, and therefore it has an "entry point to entry point".** The buffer block carries not only the curves but also the
`src_in` of the source event (`EnvelopeBlock`/`FxChainBlock` in
`src/libs/kt-model/src/ops.rs`), and insertion runs the keys through `rebase`:
`d = target.source_in − block.src_in`, with no scaling at all. The practical consequence visible at once: the animation lands at the same position FROM THE START OF THE PIECE, not at the same second of the timeline and not stretched to the length of the target. Insert into a shorter clip — the tail of the animation is hidden, as with trimming; insert into a clip taken from another point of the file — the animation still starts "the same distance from the start of the event". Scaling was deliberately rejected here (item 215): it was precisely that "rubber" from which we moved away in item 212. In Premiere's language we firmly chose the "Anchor to In Point" mode and do not offer "Scale" or "Anchor to Out Point" — three mutually exclusive modes of transferring animation and the price of each are analyzed in F-AUT-004.

**3. The litmus test of the domain is not trimming but SLIP, and on it Premiere and After Effects diverge, though they are usually considered the same.** Slip is sliding the material under a fixed event window: the picture changes, there is no boundary on the timeline. In Premiere the keys move ALONG WITH THE FRAMES, and this hurts enough that a request sits on the ideas forum, "allow slip without moving keys", proposing that ordinary slip leave keys fixed and that moving be bound to a modifier. A respondent in the same thread notes that this behavior would match what After Effects does with Pan Behind — that is, in AE everything is the opposite: the layer is dragged along, and the keys stay, unless they are specially selected. The reason is exactly the domain: in Premiere a key is addressed by the material of the clip, in AE by the time of the LAYER, and in AE a "layer" is not a piece of material but an independent carrier. For us, on this test, the behavior is Premiere's and results on its own: slip is an edit of `source_in`, and the keys lie in its coordinate system.

**4. Only one of the five names the domain aloud, and another hides it in the MACHINE's settings.** Resolve, when transferring a grade, forces a choice of "Apply Grades Using": by the source timecode (the same material or its duplicate) or by the start of the event (a foreign frame) — exactly our question, moved into a menu; and there it also turns out that ordinary copying of a grade from clip to clip does not carry the keys AT ALL. The price of honesty is visible too: the choice applies to everything applied afterwards, and a forgotten switch quietly spoils the next twenty clips. VEGAS took the opposite path: whether the track envelope follows the event is decided by the menu item `Options → Lock Envelopes to Events` — a property not of the envelope and not even of the project, but of the settings on the machine. The same `.veg` file behaves differently for two editors with the same gesture, and the project file contains no such difference. The help describes exactly moving along the timeline: trimming, ripple and moving to another timeline are not included, and on the forum this surfaces as "the link breaks if you drag not on the same timeline".

**4. Only one of the five names the domain aloud, and another hides it in the MACHINE's settings.** Resolve, when transferring a grade, forces a choice of "Apply Grades Using": by the source timecode (the same material or its duplicate) or by the start of the event (a foreign frame) — exactly our question, moved into a menu; and there it also turns out that ordinary copying of a grade from clip to clip does not carry the keys AT ALL. The price of honesty is visible too: the choice applies to everything applied afterwards, and a forgotten switch quietly spoils the next twenty clips. VEGAS took the opposite path: whether the track envelope follows the event is decided by the menu item `Options → Lock Envelopes to Events` — a property not of the envelope and not even of the project, but of the settings on the machine. The same `.veg` file behaves differently for two editors with the same gesture, and the project file contains no such difference. The help describes exactly moving along the timeline: trimming, ripple and moving to another timeline are not included, and on the forum this surfaces as "the link breaks if you drag not on the same timeline".

## Pros

The advantages are consequences of OUR design (`AutoKey.t` = the material tick).

- The geometric commands of curves do not touch them at all: `TrimStart`, `TrimEnd`,
  `Split`, `SetReversed`, `MoveClip` in `src/libs/kt-model/src/model.rs`
  contain not a single reference to `fx_auto`. The attachment is ensured by the REPRESENTATION, not by the command code, so it cannot be forgotten in a new command: any future operation on geometry gets the correct behavior for free.
- Trimming HIDES a key rather than moving or deleting it: `check_fx_auto`
  specifically allows a key outside the used piece. Stretch the event back — the animation returns the same. This is rule 20 of the checklist
  ("nothing grows back or switches off by itself") in pure form.
- A cut does not split the curve physically: `clip_slice` copies it to both
  halves entirely and edits only `source_in`. Continuity at the join is arithmetic, not code: both halves ask the same material
  point (t212 checks the equality of values from both sides). Copying a region behaves the same way: `copy_region_block` in
  `src/libs/kt-model/src/ops.rs` trims the clip at the region's edges and edits only `source_in` — the curve moves whole and stays on its frames.
- Reversing mirrors the animation by itself: the mirror lives in `clip_material_at`,
  not in the commands. A slide-trim ("the material moved under the fixed window") also happens by itself: `source_in` moves, the curve stays on the frames.
- A piece of a morpher layer inherits a piece of the curve (`morph_slice`, t35):
  "a piece of the curve is the piece of a curve" — a direct dividend of the material domain;
  without it, cutting targets with a morpher would require recalculating keys.
- Speed is not a property of the event but a parameter of an effect, so its editing cannot at all "stretch the keys": the length of the event does not change. Since R.37 the automation axis does not depend on the curve `rate` itself either.
- The resolution of curves is ONE place for the whole product: `layer_src` in
  `src/libs/kt-engine/src/video.rs` computes the parameters once when building the `FramePlan`, and the CPU and GPU receive ready numbers
  (invariant No. 7). Audio receives `AutoProfiles` as numbers before the stream starts
  (invariants No. 2 and No. 6).

## Cons

The disadvantages are the price of the same design.

- **A hidden key is not visible.** The graph's axis is exactly the piece of the event's material (`clip_ctx`: `range = (source_in, source_in + duration)`), and `painter_at` cuts off everything beyond the edges. A key hidden by trimming exists, affects the extreme values, but is absent from the graph; it can be found only by switching to the table, which lists ALL moments. There is no indicator "there are more keys beyond the edge".
- **There are three domains, not one.** For `ClipEvent.fx_auto` the key is a material tick;
  for `TransitionEvent.auto` — a tick from the start of the transition window
  (`joint − in_offset`); for `CameraEvent.auto` — a tick from the start of the event. By PRINCIPLE there are two of them ("the carrier's material" and "the start of one's own axis" — that is how F-MPH-008 counts them), but for a transition its own axis does not coincide with any event: it is derived from the joint and two offsets, and therefore behaves differently from the camera's. The widget is one (`AutoCtx` with the pair
  `to_timeline`/`to_curve`), but the semantics of edge editing differ, and no one tells the user about it. Each domain has its own tests, which do not know about each other: t212 (clip, material),
  `t232_transition_params_animate` in
  `src/libs/kt-engine/tests/transitions_video.rs`,
  `t231_camera_curves_replace_keyframes` in
  `src/libs/kt-model/tests/cameras.rs`.
- **Asymmetry in the transition.** The left edge of the window (`in_offset`) is the origin of counting, so editing it DRAGS the whole animation along the timeline; the right one (`out_offset`) is just an edge — it hides and reveals keys. One and the same gesture at two different corners gives two different results.
- **In the transition the resolution formula is duplicated.**
  `TransitionEvent::kind_at` in `src/libs/kt-model/src/project.rs` is labeled "the single resolution point", but it has not a single call: the render computes the same thing itself — `TransAnim::of/at` in
  `src/libs/kt-engine/src/video.rs` keeps its own `w0` and calls
  `curve_value` directly. Two places where the window's domain lives — exactly the desynchronization from which the clips' material domain is protected by one function.
- **For a camera, trimming the start shifts the animation.** Keys are counted from
  `CameraEvent.start`; the camera has no separate trim, and the edit goes through the whole `SetCamera`, the keys are not recalculated. Moving the camera carries the animation with it (this is intended), but so does changing the start.
- **There is nothing to hang a key on in a morpher layer.** `MorphPatch` in
  `src/libs/kt-model/src/project.rs` carries `video_fx`/`audio_fx` and bypass flags, but has NO `fx_auto` field: `apply_to` appends the layer's effects to `c.fx` of the target, not touching its `fx_auto`. A layer has no material of its own, and the material of each target is its own — there is nothing to hang a curve on (see F-MPH-008).
- **The audio domain is not mirrored on reverse.** `AutoProfiles::from_curves`
  (`src/libs/kt-fx/src/lib.rs`) receives `src_in_frames` and the length, but does NOT receive `reversed`; the UI and the video mirror. A reversed audio clip with curves has its graph and sound diverge — a defect recorded in the R.37 reconnaissance
  (`docs/user-feedback.md`), to be fixed in its own stage.
- **A missing source gives "infinite" material.**
  `src/libs/kt-media/src/placeholder.rs` declares a duration of 1000 hours so as not to cut the edit, while the media-duration model does not know about it at all. Keys are placed on an axis with no picture behind it; when the file returns (t211) the coordinates will match — but if the returned file is shorter, some keys will remain beyond its end and will hold the extreme value.
- **We have no track envelopes at all**, so "track vs clip" from other editors is not yet a dispute for us but a gap: in `Project`
  (`src/libs/kt-model/src/project.rs`) a track has neither volume nor a switch — only a line number.

## What users say

The most valuable here is not the complaint "the keys are not where they should be", but the request to CHANGE the domain:
people understand perfectly well where a key lies, and they argue precisely with the fact that the choice was made for them. Premiere drags keys along with the material on slip, and people ask for this to be switched off:

> Allow clip to be slipped without moving its keyframes!
> — Pierre Louis B., Adobe Premiere Pro Ideas, 24.02.2023,
> https://community.adobe.com/t5/premiere-pro-ideas/allow-clip-to-be-slipped-without-moving-its-keyframes/idi-p/13605948

In the same thread the respondent notes that the requested behavior would match
what After Effects does with Pan Behind — that is, two products of one vendor answer the question about the domain DIFFERENTLY, and users see this. AE's own domain is also visible to the naked eye: you trim a layer,
turn on Time Remapping — and the first key lands at the start of the whole source, not of the trimmed piece.

> Time-remapping is based on the source time, not the comp time
> — Mylenium, Adobe Community (After Effects), 27.12.2020,
> https://community.adobe.com/t5/after-effects/time-remapping-leads-to-negative-keyframe-times-on-trimmed-clips/td-p/11708729

And here is the complaint for which our "Cons" has an item about invisible keys: the domain is correct, but the user loses the keys from VIEW.

> How often do I loose keyframes at start/end of clips when trimming a
> clip and the keyframe is then out of visible range.
> — Andreas Lukas, Adobe Community (request "sticky keyframes"), 24.01.2023,
> https://community.adobe.com/feature-requests-730/responsive-keyframes-ability-to-stick-keyframes-to-clip-s-start-and-end-1329869

The track half of the question ("the clips moved, the envelope stayed") is covered by quotes in F-AUT-003 — it is about another feature, not about the domain of clip keys.

## How it fits Kinotrain
CAPABILITY: openspec/specs/automation/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
The key domain is the time of the event's MATERIAL, not of the timeline: trim and source substitution carry the curves along with them by one delta.

## Estimate

| What | Answer |
|---|---|
| Size | M — the clip domain already exists; unification in 2–3 stages remains |
| Where the main work is | kt-fx (mirroring of reverse in the audio domain), kt-model (the domain of the transition/camera), kt-app (the edge of the graph, the domain label) |
| Project format | not touched (v3 is already about curves); changing the domain of the camera/transition would require conversion on read, like `material_from_progress` |
| Tests that will have to be written | a reversed audio clip with a curve sounds as the graph shows; editing `in_offset` of a transition and `start` of a camera — formulate the expectation and pin it; a hidden key is visible through an indicator |
| Depends on | nothing; on the contrary, F-AUT-003 (the track envelope) and F-MPH-008 (morpher-layer automation) depend on this decision |
| Breaks compatibility | no, as long as the transition/camera domains are not changed |

## Verdict

We keep the material domain: it was chosen by the user through a direct answer ("like
Premiere/AE", in reply to his own question "why do we work in normalized time, it is not clear") and has paid off — five geometric commands got the correct behavior without a single line of code. The reconnaissance, however, refines the wording of the decision itself: we matched Premiere, but not After Effects — AE's domain is layer-based, and there slip does not move keys. What needs finishing is not the domain but its edges: mirroring of reverse in the audio profiles, the visibility of hidden keys, and a conscious decision about the domain of transitions and cameras. Three domains in one project is a debt, but a MARKED debt: as long as it is recorded here and in
`src/libs/kt-model/CLAUDE.md`, it does not turn into a surprise.

## Sources

Foreign editors (all — 07.09.2026):

- Premiere, slip drags keys with the material — the request and the discussion with the comparison to AE Pan Behind:
  https://community.adobe.com/t5/premiere-pro-ideas/allow-clip-to-be-slipped-without-moving-its-keyframes/idi-p/13605948
- Premiere, trimming moves keys beyond the visible range (the "sticky keyframes" request, modeled on Avid):
  https://community.adobe.com/feature-requests-730/responsive-keyframes-ability-to-stick-keyframes-to-clip-s-start-and-end-1329869
- Premiere, track keys versus clip keys and "Pin To Clip" on insertion:
  https://helpx.adobe.com/premiere-pro/how-to/adjust-audio-track-volume.html
  and https://helpx.adobe.com/premiere/desktop/add-video-effects/control-effects-and-transitions-using-keyframes/copy-and-paste-keyframes.html
- Premiere, track automation modes:
  https://helpx.adobe.com/premiere/desktop/add-audio-effects/apply-audio-effects/audio-track-mixer-automation-modes.html
- Premiere, three modes of transferring animation in a preset, and "Scale Attribute
  Times" in Paste Attributes:
  https://community.adobe.com/t5/premiere-pro-discussions/how-to-save-a-preset-with-both-anchor-to-in-point-and-anchor-to-out-point-selected/td-p/13370740
  and https://community.adobe.com/t5/premiere-pro-discussions/copy-and-paste-effect-relatively-to-clip-length/m-p/15079185/highlight/true
- After Effects, Time Remapping is computed in source time:
  https://community.adobe.com/t5/after-effects/time-remapping-leads-to-negative-keyframe-times-on-trimmed-clips/td-p/11708729
- After Effects, trimming and slip of a layer do not move keys (Pan Behind drags only the selected ones): Chris & Trish Meyer, CMG Hidden Gems, ch. 7,
  https://www.provideocoalition.com/cmg_hidden_gems_chapter_7_trimming_layers/
- After Effects, Time Stretch and keys — the sources CONTRADICT each other:
  https://flylib.com/books/en/2.104.1/manipulating_time_itself.html
  ("Stretching a layer does not change the timing of any applied
  keyframes") versus secondary retellings of the Adobe help
- VEGAS, event keys are not visible on the timeline:
  https://help.magix-hub.com/video/vegas/21/en/content/topics/keyframes.htm
- VEGAS, `Lock Envelopes to Events` and the order "track → event":
  https://help.magix-hub.com/video/vegas/22/en/content/topics/11-autoscript/insertvidenv.htm
  and .../insertaudioenv.htm
- VEGAS, the link breaks when moved to another timeline:
  https://creativecow.net/forums/thread/locking-envelope-to-events/
- Resolve, "Apply Grades Using" (Source Timecode / Start Frames / No
  Keyframes) and the fact that an ordinary grade copy does not take the keys:
  https://davinciresolveclub.com/copy-color-grade-davinci-resolve/
- Resolve, Fusion as a separate time scale: a discussion of the render range and the discrepancy of the composition's frame with the frame of the trimmed clip,
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=201890
  (the page returns 403 — the fact is taken from a search snippet, the wording is NOT confirmed) and https://vfxstudy.com/tutorials/change-the-length-of-a-fusion-animation/
- Final Cut Pro, "at specific points in a clip":
  https://support.apple.com/guide/final-cut-pro/add-video-effect-keyframes-ver8e3f20ea/mac
- Final Cut Pro, a preset with keys: Stretch to Fit / Maintain Timing:
  https://support.apple.com/guide/final-cut-pro/save-video-effects-presets-ver462c2078f/mac

Ours:

- `src/libs/kt-model/src/project.rs` (`AutoKey`, `ParamCurve`,
  `check_fx_auto`, `curve_value`, `clip_material_at`/`clip_timeline_at`,
  `TransitionEvent.auto`, `CameraEvent.auto`, `MorphPatch`),
  `src/libs/kt-model/src/model.rs` (geometric commands),
  `src/libs/kt-model/src/ops.rs` (`rebase`, buffer blocks,
  `copy_region_block`), `src/libs/kt-engine/src/video.rs` (`layer_src`,
  `fx_chain_hash`, `TransAnim`), `src/libs/kt-fx/src/lib.rs`
  (`AutoProfiles::from_curves`), `src/apps/kt-app/src/automation.rs` and
  `fx_editor.rs`, `src/libs/kt-media/src/placeholder.rs`.
- items 212–215; `docs/user-feedback.md` R.33
  (the decision on the domain) and R.37 (the axis stopped depending on speed, the list of known domain defects); `src/libs/kt-model/CLAUDE.md` section
  "Parameter automation of FX"; `src/apps/kt-app/CLAUDE.md` section
  "Automation in the UI".

A note on the method: no foreign editor was run live. The forum quotes above were read verbatim on community.adobe.com; the helpx.adobe.com pages were NOT always accessible in this pass (some of Adobe's facts are taken from search snippets), forum.blackmagicdesign.com returned 403, reddit was unavailable, and the search exhausted the session limit. Therefore the Resolve (Fusion) and After Effects (Time Stretch) rows are marked as unconfirmed directly in the text, rather than "smoothed over". Some links were re-checked in this same wave by the notes F-AUT-003, F-AUT-004 and
F-MPH-005/008.
