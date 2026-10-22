---
id: F-CMP-002
title: Transition vocabulary: inversion, patterns, volume
status: done
priority: B
cost: L
category: compositing/transitions
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12]
we_have: yes
crates: [kt-model, kt-engine, kt-gpu, kt-otio, kt-app]
invariants: [7, 11]
ui_rules: [16, 20]
round: R.51
point: 396
tests: [459, 460, 461, 462, 463, 464, 465, 466, 467, 468]
depends_on: []
checked: 2026-09-16
---

# F-CMP-002 — Transition vocabulary: inversion, patterns, volume

## Essence

A Vegas editor has in his head a vocabulary of ~25 transitions: besides dissolves and wipes — venetian blinds, checkerboards, page peel, cube, split/squeeze — and each
has an "reverse". Ours is 16 kinds without a common "reverse": three partial flags (`Clock.reverse`, `Luma.invert`, `Barn.vertical`) do not add up to a rule, and there are no volumetric kinds at all. The claim of R.51 verbatim: "you have no transitions with the symbiosis of two events, no inversions of a transition, etc." — even though 7 of our 16 kinds do move both frames. The impression was created by the broken Push/Whip (a smeared edge entered instead of frame B) and by the absence of a common word "inversion".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Transitions window, ~25 families with presets; Transition Progress envelope | each transition has a plugin parameter panel (direction, angles, centre); "reverse" is done NOT by a checkbox but by an envelope of Transition Progress on the transition itself: the progress curve can stop, go backwards, play back in reverse |
| Adobe Premiere | 26.x | Effect Controls of the transition: direction arrows + Reverse checkbox | the direction is arrows at the edges of the thumbnail, and Reverse is a separate flag "play the transition backwards": a wipe goes right to left, an iris closes instead of opening. Two different axes in one panel |
| DaVinci Resolve | 21 | Inspector of the transition: Transition Curve, per-kind Direction/Angle | for some kinds direction/angle is a parameter; ease is set by the transition curve in the inspector; there is no general "reverse", each kind has its own handle |
| Final Cut Pro | 12.3 | Transition inspector + on-screen controls | direction and centre are edited with the mouse right in the viewer (arrows/points on the frame); a dictionary of ~100 presets, but "reverse" as a general flag does not exist |

## The trick behind it

**1. "Inverting a transition" in the industry is THREE different axes hidden under
one word.** (a) Direction of the geometry: a wipe left-to-right versus right-to-left — in Premiere these are the arrows, for us `Dir`/`angle`. (b) Swapping the roles of the sides: an iris OPENS the incoming against CLOSING over
the outgoing — in Premiere this is the very Reverse checkbox ("film played backwards"). (c) Reversing the progress in time: Vegas does this with the Transition Progress envelope, that is, a curve, not a flag. Editors mix the axes
in one panel, and the user calls all of this "inversion". The owner chose axis (b) as a checkbox — and it is implemented by ONE substitution `kernel(b, a, 1−k)`
(swap the sides, reverse the progress) before the kernel, without a single edit to the formulas:
for symmetric kinds (dissolve, blur, pixelate) this is an identity, and the checkbox is not shown there. Axis (a) remains as parameters of the kinds; axis (c) the owner rejected explicitly (it was proposed — "a progress curve as in Vegas").

**2. The whole industry of open transitions has converged on our model: one fragment pass, two frames, progress.** The GL Transitions signature —
`transition(uv)` over `getFromColor`/`getToColor` and the `progress` uniform — is exactly our `TransMix` (mode + numbers) over T1/T2. An important consequence for "volumetric" ones: page peel and cube in these libraries are not a scene with a depth buffer,
but an ANALYTIC inverse mapping in the same fragment shader (the validity of the coordinate resolves occlusion). So the 3D family requires neither a new pass nor depth — only precomputed scalars of the frame.

**3. Push differs from a wipe exactly in that it moves BOTH frames — and a broken push is worse than a missing one.** In a wipe the content stands still and the mask moves; in a push the outgoing one leaves and the incoming one must follow RIGHT BEHIND it. Our push moved the outgoing frame correctly, but the incoming one sampled with an offset in the opposite direction — the coordinate went off the frame and was clamped to the edge, and a "smeared strip" entered. No test saw this: the render test painted the frames in solid colours (any coordinate gives the same colour), and conformance compared CPU with GPU — the mirrors were broken in the same way. Hence the rule of the round: directional kernels require a closed-form test on gradient frames, where the expectation is computed by a formula, not by a second instance of the same formula.

## Pros

- One checkbox instead of 33 partial flags: inversion is a property of the transition EVENT, executed in the plan (`build_plan`); the kernels and the WGSL mirror do not change; the cache keys change themselves (the order of sides and k are already in the key).
- The vocabulary grows from 16 to 33 kinds, and all the new kernels live in the existing composite pass: patterns — through the same `front_mask`, volumetric ones — through the free
  16 floats of the `m` uniform (not read by any pass since R.44).
- Symbiosis where it is visible: split (the halves of the outgoing one separate), squeeze (the outgoing one compresses, the incoming one unfolds), swap (the frames exchange places along an arc with a switch of order at the midpoint).

## Cons

- 33 kinds in the menu — three levels of nesting (clip → transition → category → kind); if the live check shows inconvenience, the fallback option is a flat list with category separators.
- The inversion of directional kinds (push/slide/whip) duplicates changing `Dir` (inverted push to the left == push to the right) — the checkbox honestly works for them, but looks like a second way of doing the same thing.
- Volumetric kernels on the CPU fallback are more expensive than wipes (perspective division, cylindrical unwrap) — on the preview without a GPU the transition window will slow down more than other places; accepted as the price of the fallback (the level of whip/box9).

## What users say

> You can reverse the transition by clicking the Reverse checkbox.
> This plays the transition backward.
> — Adobe help "Adjusting transitions" (Premiere Elements, the same mechanism in Premiere), accessed 16.09.2026,
> https://helpx.adobe.com/premiere-elements/using/adjusting-transitions.html

The demand for the same handle from users is visible in live threads: "Changing the
direction of a video transition" (Adobe Community, 2020,
https://community.adobe.com/t5/premiere-pro-discussions/changing-the-direction-of-a-video-transition/td-p/11133992)
— the question is posed exactly as "how to reverse a transition", without distinguishing axes
(a)/(b). In the Vegas community the same request is solved with an envelope: the thread "Pausing
a Transition" (Creative COW) advises "Insert/Remove Envelope → Transition
Progress" so that the transition stops or goes backwards — that is, the reverse
there lives on the time axis, not in the kind's panel.

NO QUOTES: for volumetric kinds (page peel/cube) no live verbatim complaint was sought beyond snippets — the demand is obvious from the composition of all four core editors; the search budget of the wave went to exploring the inversion axes.

## How it fits Kinotrain
CAPABILITY: openspec/specs/transitions/ — the contract is in `spec.md`, the "why" and code paths are in `design.md`.
33 transition kinds; `transitions.rs` and `composite.wgsl` — as a pair, the closed-form t459 guards against the blindness of mirrors.

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round: defects, inversion, 17 kinds, the volumetric mechanism |
| Where the main work is | kt-engine/transitions.rs + composite.wgsl (kernels), kt-model (kinds/specs), kt-app (window/menu/i18n) |
| Project format | `invert` in the transition envelope + new kind tags; a bump of the file version (an old build would silently play a different kind) |
| Tests that will have to be written | closed-form ramps (push/whip, red until the fix); t223 → 33 kinds; t224 + cases of all axes; t225 analytic asserts of the new kernels; a regression of the kind-reset |
| Depends on | — |
| Breaks compatibility | yes, deliberately: the owner's decision in R.51 "no backward compatibility is needed" |

## Verdict

We take it entirely (the owner's decision, R.51): inversion — a checkbox for axis (b) with execution before the kernel; axis (c) — the progress curve — was rejected by the owner (it was proposed explicitly); the composition is the "modern mix" of 17 new kinds;
transitions on overlap are a separate topic, its decision is recorded in F-TL-010.
The existing partial flags of the kinds are not renamed and not collapsed:
they are axis (a), the direction of the geometry, and the common checkbox does not replace them.

DONE in R.51 (items 396–401): repair of push/whip and of the kind's reset points, inversion by the plan, 33 kinds (patterns/shift-symbioses/stylized/volumetric
through `trans_ext`), centres, transitions on overlap. Further life — in
`docs/porting-notes.md`.

## Sources

Documentation and communities (accessed 16.09.2026, opened via
search snippets — other editors were not run live):

- Adobe, "Adjusting transitions" (Reverse checkbox):
  https://helpx.adobe.com/premiere-elements/using/adjusting-transitions.html
- Adobe, "Modifying and customizing transitions":
  https://helpx.adobe.com/ca/premiere-pro/using/modifying-customizing-transitions.html
- Adobe Community, "Changing the direction of a video transition":
  https://community.adobe.com/t5/premiere-pro-discussions/changing-the-direction-of-a-video-transition/td-p/11133992
- Creative COW, "Pausing a Transition" (Transition Progress envelope):
  https://creativecow.net/forums/thread/pausing-a-transition/
- Creative COW, "Clock wipe transition in Vegas Pro 13":
  https://creativecow.net/forums/thread/clock-wipe-transition-in-vegas-pro-13/
- GL Transitions (an open library, the transition/progress signature):
  https://gl-transitions.com/

Ours:

- `src/libs/kt-engine/src/transitions.rs` (`TransMix::of`, `kernel`),
  `src/libs/kt-gpu/src/shaders/composite.wgsl` (`trans`),
  `src/libs/kt-model/src/project.rs` (`TransitionKind`,
  `TransitionEvent`), `src/apps/kt-app/src/transition_editor.rs`.
- `docs/user-feedback.md` R.33 (decisions about the 16 kinds and sound), R.51
  (the claim and the decisions of this round); `docs/porting-notes.md` items 222–227.
- Related note: `docs/features/timeline/structure/same-track-transitions.md`
  (F-TL-010) — overlap as a state and a transition on overlap.
