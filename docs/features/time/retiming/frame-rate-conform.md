---
id: F-SPD-005
title: Bringing source frame rate to the project grid
status: researched
priority: B
cost: L
category: time/retiming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: partial
crates: [kt-engine, kt-media, kt-model, kt-app]
invariants: [4, 7, 11]
ui_rules: [4, 16, 19]
round: null
point: null
tests: []
depends_on: [F-SPD-003, F-MED-004]
trackers: []
services: []
checked: 2026-09-23
---

# F-SPD-005 — Bringing source frame rate to the project grid

## Essence

A 30p clip, a 60p clip and a 23.976 clip get into a 25 fps project. On every frame of the project one must decide
which frame of the source to show: the nearest one, the previous one (floor), a blend of two neighbouring frames, or one synthesised from motion. To the editor's eye this is either a slight judder every few
frames, or soft doubling, or "as if native", at the cost of rendering. The question of the note is where
this rule lives and what happens when the frame rates coincide.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Resample mode of the EVENT (Smart / Force / Disable) and the default in the project properties | the rule is on the event with the project default; Smart blends neighbouring frames only when the rates differ, Disable — the nearest frame [NOT VERIFIED against the primary source: Vegas help was not opened from this environment; known from the interface 19–22] |
| Adobe Premiere | 26 | Interpret Footage → Assume this frame rate; Time Interpolation (Frame Sampling / Frame Blending / Optical Flow) on the clip | the file's rate is redefined by INTERPRETATION (it changes the clip's duration), the method of conforming is a property of the clip [NOT VERIFIED against the primary source] |
| DaVinci Resolve | 21 | Project Settings → Mixed Frame Rate Format; Retime Process (Nearest / Frame Blend / Optical Flow) on the project and on the clip | the rule is on the PROJECT: "DaVinci will create extra frames for the 23.97 fps clips using the frame interpolation settings of the retiming process (nearest, frame blend, or optical flow, as set in the project settings), and it will drop every second frame of the 60 fps clips" |
| Final Cut Pro | 12.3 | Rate Conform (Video inspector): Fast (Floor) / Fast (Nearest Frame) / Good (Frame Blending) / Better (Optical Flow) / Best (Machine Learning) | the rule is on the CLIP; "If the selected clip has the same frame rate as the project, the Rate Conform section doesn't appear in the Video inspector"; "Fast (Floor) is the default setting. Final Cut Pro duplicates frames or rounds down to the nearest whole frame to match the clip's frame rate to the project's frame rate" |
| After Effects | 26 | Interpret Footage → Conform to frame rate; the Frame Blending switch on the layer (Frame Mix / Pixel Motion) | the rate is an INTERPRETATION of the FOOTAGE, the blending is a layer switch; the composition computes at its own fps [NOT VERIFIED against the primary source] |
| Kdenlive | 26.08 | the MLT project profile | the clip is conformed to the profile by the nearest frame; the clip has no rule of its own; hence the requests to "change the project to fit the clip" — the same as for F-MED-004 |

## The trick behind it

**1. In FCP the section appears only on a mismatch — and this is the same trick as with Spatial Conform.** The control takes no space and at the same time REPORTS a fact: when the section is visible, the rates diverge. In Resolve, on the contrary, the rule is on the project, and the person learns of the mismatch from the judder.

**2. In Resolve the project frame rate is LOCKED by the first clip.** While the timeline is empty the fps can be changed; after the first clip the field is grey, and the only way out is a new project. This is the answer to "the first clip sets the project", carried to the extreme: not a hint, but a lock.

**3. "Floor" and "nearest" are different answers, and FCP's default is floor.** Floor duplicates the frame backwards and never shows a frame from the future; Nearest rounds in both directions and in places runs half a frame ahead of the sound. Our `clip_frame_keys` sets floor on the file's grid — exactly FCP's default, without a name and without a choice.

## Pros

- A rule on the clip (FCP) — 60p footage going into slow motion gets Optical Flow, while a 25→30 interview gets floor; one project, different answers.
- A rule on the project (Resolve) — switching after the fact for the whole edit.
- A section-by-fact (FCP) — the control takes no space while there is nothing to decide.

## Cons

- The frame-rate lock (Resolve) — the mistake of the first clip costs a new project.
- Blending and optical flow cost a full second decode and are not deterministic between versions (see F-SPD-003).
- Footage interpretation (Premiere/AE) changes the clip's DURATION and desynchronises it from its own sound if the latter has not been reinterpreted.

## What users say

> DaVinci will create extra frames for the 23.97 fps clips using the frame
> interpolation settings of the retiming process (nearest, frame blend, or optical
> flow, as set in the project settings), and it will drop every second frame of the
> 60 fps clips.
> — Frame.io blog, "Mixing Frame Rates in DaVinci Resolve – Part 2", 07.10.2019,
> https://blog.frame.io/2019/10/07/mixed-rates-resolve-part-2/

> If the selected clip has the same frame rate as the project, the Rate Conform
> section doesn't appear in the Video inspector.
> — Final Cut Pro User Guide 12.3, "Conform frame sizes and rates",
> https://support.apple.com/guide/final-cut-pro/conform-frame-sizes-and-rates-ver3363b44e/mac
> (accessed 2026-09-23)

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-plan/

### What already exists

- Floor on the file's grid — `clip_frame_keys` (`src/libs/kt-engine/src/video.rs`), one
  function for both the order and the read (invariant 4); the procedural source is computed on
  the continuous tick (R.56).
- Frame blending on the event ("Frames: blending", R.37) — `blend_frames` there, the
  kernel `dissolve_into`; the pair is resolved by the same function.
- Synthetics with their own frame grid — `SyntheticSource::with_video_fps`
  (`src/libs/kt-media/src/synthetic.rs`): on it floor and nearest are distinguishable.
- The project fps — divisors of 90 000 (`TICK_HZ % fps == 0`); 29.97 cannot be expressed.

### What is missing

- The rule itself as a choice: the floor is hard-wired; "nearest" and "blending of neighbours by the fractional position" are not selectable; "optical flow" — F-SPD-003.
- The place of the rule: for a project with an exception on the event — the same pair as the frame-conform rule (F-MED-004, R.62).
- A message about the mismatch at placement — R.62 reports about proportions; the frame rate is the neighbouring line of the same hint.

### What gets in the way

- The frame key in the cache is the source's pts (invariant 4): "nearest" changes the tick→frame map, and the order must be computed by the same function as the read.
- Invariant 11: changing the default would shift GOLDEN_VIDEO on the 30p fixtures in a 25p project — the default must stay floor.

## Estimate

| What | Answer |
|---|---|
| Scope | L: the rule on the project and the event, a tick→frame map with three answers, a hint |
| Where the main work is | kt-engine (`clip_frame_keys`), kt-model (fields), kt-app (hint, project window) |
| Project format | a rule field on the project and the event, a round version |
| Tests | floor/nearest/blending on synthetics with a grid; GOLDEN does not move on the default |
| Depends on | F-MED-004 (the place of the rule), F-SPD-003 (optical flow) |
| Breaks compatibility | no, with the default "floor" |

## Verdict

**Deferred (R.62).** The owner named "different fps" in one complaint together with the aspect
ratio, but frame-rate conforming is a different stage of the pipeline (the tick→frame map, not geometry),
and the R.62 round decided not to mix them. The default "floor" in our case matches FCP; a name and a choice are missing. Take it after F-MED-004, in the same pair "project + exception on the event"
and with the same hint at placement.

## Sources

- Final Cut Pro User Guide 12.3, "Conform frame sizes and rates":
  https://support.apple.com/guide/final-cut-pro/conform-frame-sizes-and-rates-ver3363b44e/mac
  (accessed 2026-09-23)
- Frame.io, "Mixing Frame Rates in DaVinci Resolve" (parts 1–5, 2019):
  https://blog.frame.io/2019/09/30/mixed-rates-resolve-part-1/ ,
  https://blog.frame.io/2019/10/07/mixed-rates-resolve-part-2/
- DaVinci Resolve 18.6 manual, "Mixed Frame Rates" (the page did not open from this environment, 404): https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1321.htm
- The frame-rate lock after the first clip: https://beginnersapproach.com/davinci-resolve-frame-rate-issues-fixes/
  (accessed 2026-09-23)
- Vegas / Premiere / After Effects — from memory of the interface, primary sources not opened
  [NOT VERIFIED]
