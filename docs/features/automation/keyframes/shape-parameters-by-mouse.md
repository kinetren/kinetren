---
id: F-AUT-011
title: Segment shape is visible on the graph, its parameters are under the mouse
status: done
priority: B
cost: S
category: automation/keyframes
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: yes
crates: [kt-app, kt-model]
invariants: [12]
ui_rules: [1, 3, 9, 10]
round: R.67
point: "change r67-curves-stills-import (chronicle frozen at §470)"
tests: [t784b, t785, t785b, t790]
depends_on: [F-AUT-009, F-AUT-001]
trackers: []
services: []
checked: 2026-09-26
---

# F-AUT-011 — Segment shape is visible on the graph, its parameters are under the mouse

## Essence

The editor sees on the graph THAT line that sounds: a zigzag as a zigzag, a step as a
step, a formula as its own curve, not a straight line between the diamonds of keys.
He points at the middle of a stretch and a handle appears; he pulls up and the zigzag
gets higher (or the third-order curve sharper), with Shift to the right there are more
periods; the number is visible next to it while he drags. The numbers in the fields stay,
but aiming at them is no longer required.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Envelope on an event/track | the envelope line is drawn BY ITS SHAPE (Fast/Slow/Smooth are visible by their curvature); the shape has no parameter, so there is nothing to change; the point is dragged with the mouse |
| Adobe Premiere | 26 | Effect Controls → Velocity/Value graph | Bezier handles on a key are dragged with the mouse; the value graph is separate from the speed graph; the shape is visible in both |
| DaVinci Resolve | 21 | Curve editor (Edit page) / Spline (Fusion) | Bezier handles; in Fusion on a spline there are "Smooth/Linear/Step" and handles; parameters of modifiers (Shake, Perturb) — only as fields |
| Final Cut Pro | 12.3 | Video Animation editor | the line is visible by its shape (Linear/Smooth), handles only on Smooth, the parameter is not editable |
| After Effects | 26 | Graph Editor | Bezier handles are dragged, "Easy Ease" is a command; expression parameters (`wiggle(freq, amp)`) — only as text |

**Outside the core.** Blender is the only one where the parameter of a procedural shape
is visible and edited in the same place as the line.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 | F-Curve Modifier Noise (Scale/Strength) | the line is redrawn at once by the modifier; the parameters are in a panel next to the graph, but not as a handle on the line itself |

## The trick behind it

1. **All five editors draw the line BY ITS SHAPE — none of them draws a straight line
   between keys.** This is not a feature but a presumption: a graph that lies about the
   shape does not occur anywhere, so the complaint "you can't visually see what the curve
   is" is not a matter of taste but a lag behind the base level. Our t698 asserted the
   model and was green while the screen showed straight lines.
2. **The Bezier handles of AE/Premiere/Resolve are the only "parameter under the mouse",
   and it is geometric.** For a procedural shape (zigzag, noise) the parameter is everywhere
   a number in a field (`wiggle(5, 20)` in AE, Scale/Strength in Blender). A handle in the
   MIDDLE of a stretch, pulling the amplitude/tension, is our move: our shapes are
   parametric, not Bezier, and the middle of a stretch is the only place that is not taken
   by keys and does not conflict with moving a key.
3. **Vegas shows that a shape WITHOUT a parameter does not need a handle** — there the line
   itself is the only control. Hence the rule: a straight line and a step have no handle
   at all (rule 9 — nothing superfluous).

## Pros

- The line is a sampling of the same `curve_value` that the render computes; there is no
  second drawing formula, and the test compares the pixels of the line with the evaluator,
  not the model with the model.
- A handle in the middle of a stretch does not conflict with the keys (diamonds at the
  ends) or with the right click (shape) — one gesture for one meaning.
- The number next to it while dragging is the same trick as the speed bar's (an overlay,
  not a popup).

## Cons

- A curve with a formula layer has two lines; they must be told apart (solid `k`, dashed =
  result).
- Sampling by pixels means allocating a buffer of points; one reusable buffer is kept in
  the state of the widget (invariant 12).

## What users say

NO QUOTES: the reconnaissance was done from the help of the five core editors and Blender (Graph Editor, Curve editor, Video Animation editor, F-Curve Modifiers); no search of forums was carried out — the requirement came from the owner verbatim (`docs/user-feedback.md`, R.67: "visually for the user you can't see at all what the curve is, and there isn't even a way to adjust it with the mouse instead of typing numbers into fields").

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-timeline/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
The graph draws a sampling of `curve_value` by pixels; a stretch with a parameter gets a handle in the middle (the vertical is the main parameter, Shift+horizontal is the periods), a number next to it while dragging; a straight line and a step have no handle.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage: drawing by sampling, the handle and its test |
| Where the main work is | kt-app `automation.rs` (`graph`), `widgets.rs` (point buffer in `AutoView`) |
| Project format | does not touch it |
| Tests to write | line pixels against `curve_value` on a zigzag/step/formula; dragging the handle changes the parameter in one history step; the field width does not depend on the window |
| Depends on | F-AUT-009 (shapes with parameters) |
| Breaks compatibility | no |

## Verdict

**Accepted (R.67).** A line drawn by shape is the base level of the industry, not a feature; a parameter handle in the middle of a stretch is our move for parametric shapes, which Bezier editors do not have and do not need. Bezier handles on keys (F-AUT-001) remain a candidate and are decided separately: our shapes are parametric, and a second geometric control would conflict with the stretch's handle.

## Sources

- Adobe After Effects Help: "Graph Editor", "wiggle()".
- Adobe Premiere Help: "Keyframe interpolation and the Value/Velocity graphs".
- Blackmagic DaVinci Resolve 21 Reference Manual: "Curve Editor", Fusion "Spline Editor".
- Apple Final Cut Pro User Guide: "Video Animation editor".
- VEGAS Pro 2026 Help: "Adjusting envelopes".
- Blender 5.2 Manual: "F-Curve Modifiers".
