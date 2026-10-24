---
id: F-AUT-003
title: Envelope on a track
status: researched
priority: B
cost: XL
category: automation/keyframes
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-engine, kt-otio, kt-app]
invariants: [2, 3, 6, 7, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-AUT-002]
checked: 2026-09-07
---

# F-AUT-003 — Envelope on a track

## Essence

The editor opens a separate line on a track — volume, pan, opacity, an effect parameter — and it runs THROUGH the whole track, over the clips and across the joins. Points are placed by double-clicking, segments are dragged with the mouse, and each segment has its own curve shape. Clips under the line can be cut, replaced with duplicates and rearranged — the line stays, because it belongs to the track, not to the event. Separately: the line need not be drawn; it can be RECORDED by moving a fader during playback.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Track envelopes / Automation | audio: Volume, Pan, Mute, Assignable FX Send, Bus Send, Track FX parameters; video: Composite Level, Fade to Color, Mute (on the video bus also Motion Blur Amount and Supersampling). A point has one of six shapes (Linear/Fast/Slow/Smooth/Sharp/Hold); display is "Show Envelopes" one at a time, editing is a separate Envelope Edit Tool. Recording: Off / Read / Write (Touch) / Write (Latch) |
| Adobe Premiere | 26.x | Show Track Keyframes | a toggle in the track header "Show Clip Keyframes ↔ Show Track Keyframes"; rubber band draws Track: Volume / Pan / Mute and track effect parameters. Written from the Audio Track Mixer in Off / Read / Write / Latch / Touch modes; individual parameters are protected with "Safe During Write" |
| DaVinci Resolve | 20.3.2 | Fairlight Automation | the parameter curve is drawn right on the track (and on the bus); modes Read / Write / Latch / Touch / Trim, and from version 20 they are assigned INDEPENDENTLY per channel. Automation menu: Follows Edit, Active Range, Write To Start/End/All, Fill Range, Glide Range, punch commands. Clip Gain Line is a separate mechanism, not this one |
| Final Cut Pro | 12.3 | none | there are no tracks as objects at all: the level and its keys live on the CLIP, roles are gathered into audio lanes, a "bus" is imitated by a compound clip per role. There is no mixer with faders and no recording modes |
| After Effects | 26.x | none | there are no tracks: everything is properties of a LAYER with keys and the Graph Editor. The closest in meaning (precomp, adjustment layer) is a layer, not a line along a track; there is no recording by moving a controller |

## The trick behind it

**1. The recording modes differ in exactly one thing — what happens when you RELEASE the fader — and the whole job rests on that difference.** VEGAS help on Write (Touch): "Envelope points or keyframes are created only while a control is being adjusted. When you stop adjusting the control, automation recording stops and the existing envelope points/keyframes are unaffected". About Write (Latch): "recording continues until you stop playback". So Touch is a point-wise correction ON TOP of a finished mix (release — it returns as it was), Latch is an overwrite of the tail from the place of touch until stop, Write is an overwrite from the very start, and Trim (Resolve 20) is not absolute values but a SHIFT of an existing curve. None of these four scenarios is reproduced by setting keys with the mouse: a person listens and reacts, rather than counting. And the modes exist only where there is a permanently living control of the track — at FCP and AE the level belongs to the clip, and they have no recording modes at all.

**2. A recorded envelope is not five points, it is a STREAM, and every editor with the feature ships a separate thinning stage.** In VEGAS this is the checkbox "Smooth and thin automation data after recording" on the External Control & Automation tab, plus the manual "Thin All Points" and "Thin Selected Points" in the envelope's context menu. In Premiere there are two settings at once: Linear keyframe thinning and Minimum time interval thinning, and the recommendation is to raise the interval if the curve will then be edited by hand. This is not in any feature list: the feature "moved the fader — got a line" in fact consists of recording, thinning and smoothing, and without the second half the result cannot be physically edited, while with it the result stops being what the person played.

**3. The track envelope and the event level are not alternatives, but two multipliers with a strictly fixed order.** The VEGAS help states it directly: "A track envelope will always affect all events on the track. This means that any event envelopes will be calculated after the track envelope". Hence the non-obvious consequences: −6 dB on the track and −6 dB on the event give −12 dB, not "the last one wins"; "make it quieter" stops being one action, because first one must decide in which of the two layers; and the search for "why is it quiet here" becomes a walk through four places — the track fader, the track envelope, the event level, the event envelope. Envelopes are shown one at a time ("Show Envelopes" → submenu), and a HIDDEN one keeps acting.

## Pros

- The line belongs to the track, so it survives a change of material: a clip was cut, replaced with a duplicate, dragged — "quieter here" stays in place.
- The line does not break at a join. A volume dip across an edit is one pair of points, not two halves on two clips whose values at the join have to be matched by hand.
- The curve exists where there are no clips and where their boundaries do not coincide: ducking of music under speech is set by the time of the speech, even though the music is one long clip and there are ten short speech lines.
- Two level layers separate two different tasks: "even out the takes against each other" lives on the events, "balance the scene" lives on the track, and they do not fight over the same number and roll back separately.
- A mute envelope is not volume set to zero but a switch without interpolation, so "turn the track off for this piece" does not produce a fade artifact.
- Recording with a fader gives a shape that is not drawn by hand: a person's reaction by ear instead of placing points by numbers.

## Cons

- The envelope lives in PROJECT time, while editing happens in clips, so any ripple insert desynchronizes them. Everyone fixes this differently and all of them partially: VEGAS — the "Lock Envelopes to Events" checkbox in the Options menu (it carries points when events are moved, but users regularly hit "it did not work" with Insert Time and selected events); Resolve — Automation → Follows Edit, which works for track automation and does NOT work for bus automation on ripple delete; Premiere does not fix it at all — it is an open feature request.
- The line and the clips share the same pixels, and both large editors diverged by MODALITY. VEGAS: "With the Envelope tool selected, you can add, delete, select, and move envelope points, but events cannot be moved or edited". Premiere: in Show Track Keyframes mode audio clips are not editable. That is, the envelope is paid for with a mode in which one cannot edit, and switching back and forth has to be done constantly.
- The multiplication of two layers means that neither of them alone answers the question "what is the level now"; the meter shows the result, while the cause is scattered across four places.
- A recorded curve without thinning is not editable, and the thinning threshold lives in the application's global preferences, not in the project: the same project written on another machine gets a different point density.
- An effect parameter's envelope is tied to the effect slot: delete the effect — the curve is orphaned; change the order in the chain — the curve moves to the neighbor.
- There are many envelopes on a track (VEGAS has six kinds on an audio track), and there is room on the row for only one, so showing one at a time is mandatory, and an invisible envelope is a constant source of "the sound is not right, and it is not visible why".

## What users say

> Currently, when I create keyframes on an audio track in Adobe Premiere
> and later make timeline edits (such as trimming or rearranging clips),
> the keyframes remain fixed at their original positions.
> — Maaarit, 19.11.2025, feature request "Option to Ripple Audio Track
> Keyframes When Editing",
> https://community.adobe.com/t5/idea-premiere-pro/feature-request:-option-to-ripple-audio-track-keyframes-when-editing/idi-p/15598894

> The lack of this feature is the main reason I prefer Pro Tools.
> — 95allenp on "Automation follows clips", 30.01.2025 (Adobe Audition —
> the same architecture "envelope in project time"),
> https://community.adobe.com/t5/audition-ideas/automation-follows-clips/idc-p/15122645

> In my eyes, this is not a feature request. This is an outstanding bug.
> — Sage G.C., 21.02.2023, same thread,
> https://community.adobe.com/t5/audition-ideas/automation-follows-clips/idc-p/15122645

The negative feedback here is entirely about one thing: the line itself is liked by everyone, but people leave because it does not move together with the edit. The positive feedback is exactly the mirror image — in VEGAS the question "how to move the envelope together with the events" is closed by a single hint, "Lock envelopes to events", and the thread ends there.

## How it fits Kinotrain

### What already exists

The level lives on the EVENT and only there: `ClipEvent.gain_db`
(`src/libs/kt-model/src/project.rs`, line 1496) and `ClipEvent.opacity`
(same file, line 1512) are drawn as ONE line along the top of the event —
`clip_level` / `gain_to_level` / `level_to_gain` in
`src/apps/kt-app/src/timeline.rs` (lines 113–133), the grab `ClipZone::Level`
in the top strip `top_strip_h` (35 % of the row height, at most 12 px),
the range `RAIL_DB_SPAN` = ±6.02 dB, the history entry — `SetGain` /
`SetOpacity` (`src/libs/kt-model/src/model.rs`, lines 786–788).

The curve mechanics are ready and proven: `AutoKey` / `ParamCurve` /
`curve_value` (`src/libs/kt-model/src/project.rs`, lines 1587–1628),
the single widget for graph and table `src/apps/kt-app/src/automation.rs`,
the master switch `ClipEvent.fx_off`, curve copy/paste
`ops::copy_envelope` / `paste_envelope`
(`src/libs/kt-model/src/ops.rs`, lines 935 and 993). Curves already hang on
two carriers: `ClipEvent.fx_auto` (line 1520) and `CameraEvent.auto`
(line 2492).

There is a master fader — `Player::set_master_gain`
(`src/libs/kt-engine/src/player.rs`, line 375) and a slider in
`src/apps/kt-app/src/meters.rs` (line 822). It is a MONITOR: it is not written
into the project, it is absent from `src/libs/kt-engine/src/export.rs`, so there
is nothing to automate on it.

There is also a precedent for "own track for a non-clip" — `LaneKind::Morph` and
the thin row `THIN_ROW_DIV` in `src/apps/kt-app/src/timeline.rs`
(line 537).

### What is missing

- **The track itself as an object.** In `Project`
  (`src/libs/kt-model/src/project.rs`, line 450) there are no tracks: there are
  counters `video_lanes` / `audio_lanes` / `morph_lanes` /
  `camera_lanes`, the row order `lane_order` and the names `lane_names`
  (the latter are inside `UiState`, outside undo). A grep through `src/` finds
  not a single `track_gain`, `lane_gain`, `lane_mute`, `lane_solo`, `track_fx`.
  A track is a number; there is physically nothing to hang an envelope on.
- **A "track" stage of the mix** between the event and the output: the sum is computed per event; there is no bus or master in the model.
- **Track FX — none at all, neither video nor audio.** So "the envelope of a track effect's parameter" makes no sense in principle, and the order of work is strict: first the track as an object, then track FX, only then curves for their parameters. The reverse order yields a curve with nothing to attach to.
- **A fourth time domain.** Right now there are three domains: the MATERIAL tick of a clip (item 212, the single function `kt_model::clip_material_at`), the tick FROM THE START of the event for a camera (`CameraEvent.auto`), and the 0..1 progress of the legacy `CamMotion`. A track envelope by its nature lives in the PROJECT tick — a domain we do not have (see F-AUT-002). It is precisely from this difference that all the ripple pain of other editors grows: for us it will appear together with the domain.
- **What to record with.** There is no permanently living fader on a track, the monitor master is not written into the project, and we do not support MIDI/Mackie surfaces. Without these, the touch/latch/write modes have nothing to even start with.

### What gets in the way

It is not the volume of code, but the occupied place and a direct decision of the user. Round R.28 item 176
removed the vertical volume rail of v1
precisely because it stood at the left edge of the clip, that is, exactly in the
fade zone. In `docs/user-feedback.md` (section "After R.28 — level uniformity",
lines 508–515) this is recorded as follows:

> "item 3. fades should look the same for audio and video."
> The user's decision among the offered options — **a line along the top with the
> PREVIOUS range of ±6 dB** (not "0 dB on top")… The rail is removed. Port of item 176.

The rule for agents from the root `CLAUDE.md`: "A limit, mode or view set by his direct request is not changed without a new request."
Therefore a track envelope cannot "simply bring the rail back" and cannot
become a second line inside the clip row: the top strip `top_strip_h`
is already occupied by the fade corners and the event level line, and any new line
there would compete for pixels with the gesture the user personally protected.

It is exactly this conflict that others stumbled on: VEGAS and Premiere split the
line and the clips by MODALITY — "events cannot be moved or edited" in the
Envelope Edit Tool, "clips are not edited" in Show Track Keyframes.
A separate strip for the line is closed to us by rule 13 of the checklist in
`src/apps/kt-app/CLAUDE.md` — "DO NOT WASTE SPACE": creating a strip for something
that appears rarely is named there as a defect directly, while all the reference
examples of the rule ("the label name on the time-scale row", "the group name in the
gap of the upper frame line", "the long track name as a label over the start of the
track") are about how to PLACE something on an ALREADY occupied place. A track
envelope has nothing to be placed on an occupied place with: by definition it runs
through the whole track and covers everything drawn on it. This is the main
constructive constraint, not the data model.

The engine adds three separate constraints. The value of the envelope at time t
must get into the audio render stream without mutexes (invariant 2) and
move in quanta of the cpal callback, not wall-clock (invariant 6); the composite
level of a video track must be computed in
`VideoRenderer::frame_plan` (`src/libs/kt-engine/src/video.rs`), not in the
CPU/GPU executor (invariant 7). The recording modes add the worst:
model edits dozens of times per second during playback — and for us every
edit from the UI goes through the wrappers `KtApp::apply*` and advances `model_rev`, on which
the background autosave and the frame caches hang (section "Writing the project
to disk" in `src/apps/kt-app/CLAUDE.md`, item 200). Plus invariant 3: the
diff of chains must not reset the FX state at every recorded point.

### Which invariants it touches

The root `CLAUDE.md`: 2 (FX state — only in the audio-render stream),
3 (editing the model does not kill the FX state — critical for the recording modes),
6 (the cpal callback is the master clock; a recorded point is dated by it),
7 (the composite level of a track — in `frame_plan`, one code for preview and
export), 11 (determinism: a neutral envelope must leave GOLDEN_AUDIO and GOLDEN_VIDEO
untouched; in offline paths no wall-clock). UI rules of `src/apps/kt-app/CLAUDE.md`: 13 (do not waste
space — central here), 17 (paired methods: a point is placed both by mouse and by number, and they must match), 20 (nothing grows by itself —
an envelope does not appear on a track just because it was touched).

## Estimate

| What | Answer |
|---|---|
| Size | XL — a track as an object + a fourth time domain + a mix stage + a format version; the recording modes with thinning alone are a separate L on top |
| Where the main work is | kt-model (a track as an object, envelopes, commands and undo), kt-engine (the track stage in the audio mix; the composite level in `frame_plan`), kt-app (where to draw the line without taking pixels from the fade corner and the event level line), kt-otio (tracks and curves in the file) |
| Project format | a new version is needed: a track stops being a number, and an old reader would silently lose the envelopes (rule R.29) |
| Tests that will have to be written | round-trip of track envelopes in the model and in .otio; "track × event = product, not replacement"; behavior on ripple (what exactly should happen — the user's decision, not ours); `golden_export` and GOLDEN_AUDIO untouched with a neutral envelope; a layout test "the track line does not take pixels from the fade corner and the event level line" (item 176); with recording modes — a test of point density after thinning |
| Depends on | F-AUT-002 (the time domain of keys); track FX — a separate, not yet opened note; the track as an object itself |
| Breaks compatibility | no, with a neutral default (no envelope → today's behavior) |

## Verdict

We are not taking it now, and the reason is not the volume of code. Three constraints in order:
we do not have a track as an object — there is nothing to hang the envelope on; the place in the clip row is occupied by the user's decision of item 176 (the volume rail was removed at his
direct request), and rule 13 forbids solving this with a new strip;
the envelope of a track-effect parameter is meaningless while there are no track effects at all. If the feature is taken, the order is mandatory:
track as an object (volume / pan / mute / solo) → track FX → envelopes in project time → recording modes. Recording modes must not be taken without
a thinning stage: for everyone who has built them, this is a separate subsystem, not a detail, and without it the recorded curve is not editable.

## Sources

- https://help.magix-hub.com/video/vegas/22/en/content/topics/11-autoscript/insertaudioenv.htm (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/11-autoscript/insertvidenv.htm (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/11-autoscript/automation_recording_envelopes_and_keyframes.htm (2026-09-07)
- https://help.magix-hub.com/video/vegas/21/en/content/topics/11-autoscript/adjusting_envelopes.htm (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/tool.htm (2026-09-07)
- https://helpx.adobe.com/premiere/desktop/add-audio-effects/apply-audio-effects/audio-track-mixer-automation-modes.html (2026-09-07)
- https://helpx.adobe.com/premiere/desktop/add-audio-effects/apply-audio-effects/specify-automated-keyframe-creation.html (2026-09-07)
- https://helpx.adobe.com/africa/premiere-pro/using/adjusting-volume-levels.html (2026-09-07)
- https://community.adobe.com/t5/idea-premiere-pro/feature-request:-option-to-ripple-audio-track-keyframes-when-editing/idi-p/15598894 (2026-09-07)
- https://community.adobe.com/t5/audition-ideas/automation-follows-clips/idc-p/15122645 (2026-09-07)
- https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=196318 (2026-09-07)
- https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part3736.htm (2026-09-07)
- https://www.production-expert.com/production-expert-1/davinci-resolve-20-brings-focused-audio-enhancements-to-fairlight-and-timeline-recording (2026-09-07)
- https://cutsio.com/blog/davinci-resolve-fairlight-automation-touch-latch-write (2026-09-07)
- https://videomontazher.com/en/fairlight/automation (2026-09-07)
- https://creativecow.net/forums/thread/moving-volume-envelope-adjustments-with-the-video/ (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/organize-the-timeline-with-audio-lanes-verb71cb913/mac (2026-09-07)
- https://www.production-expert.com/production-expert-1/if-you-are-serious-about-pro-audio-dont-edit-video-in-final-cut (2026-09-07)
- https://helpx.adobe.com/after-effects/desktop/work-with-layers/layer-properties/layer-properties.html (2026-09-07)

No editor was run live: the mechanics are assembled from the official help
(VEGAS 21/22, Adobe helpx, the Resolve manual), Adobe and Blackmagic forums, and
reviews. The FCP and After Effects rows are based on Apple/Adobe documentation and
reviews; there was no hands-on check.
