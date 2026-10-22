---
id: F-AUT-009
title: Curve segment shape: not only linear and smooth
status: done
priority: B
cost: M
category: automation/keyframes
editors: [vegas-pro-2026, after-effects-26, premiere-26, resolve-21, fcp-12]
we_have: partial
crates: [kt-model, kt-app, kt-otio, kt-engine]
invariants: [7, 11]
ui_rules: [1, 16, 20]
round: R.61
point: 461
tests: [t695, t695b, t696, t697, t698, t698b, t698c]
depends_on: [F-AUT-001]
checked: 2026-09-22
---

# F-AUT-009 — Curve segment shape: not only linear and smooth

## Essence

Between two keyframes the value is connected by something, and this "something" is
an opinion the editor has. He wants THIS stretch to go straight, THAT one to start
slowly, THIS one to hold the value and then jump, and that piece over there to go
in a zigzag. The question is not aesthetics: a long smooth push and a six-frame
tremor are two different shapes on the same curve, and one checkbox for the whole
curve cannot express them.

## Who has it and how

The key question for each: WHO owns the shape, the segment between the keys or the
sides of the key itself.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Fade Types of an envelope segment | **a line of origin.** The shape belongs to the SEGMENT: right-click a part of the envelope between two points → a menu of types, and that part changes shape without touching its neighbours. The set is closed: Linear, Fast, Slow, Smooth, Sharp, Hold (in the service settings they are stored as numbers: 2 = fast log, 1 = linear, −2 = slow log, 4 = smooth cubic, −4 = sharp cubic). There are no handles and nothing to drag; the default type of a segment is set not in the project but by an internal setting of the program (`eDefEnvAudioASRType`). This is exactly what the owner asks for: "turn on third-order curves for everyone, zigzags, straight lines" — only without zigzags and without one's own formula |
| After Effects | 26.x | Keyframe Interpolation + Easy Ease | the shape belongs to the KEY, and a key has two: an incoming and an outgoing side. Linear / Bezier / Continuous Bezier / Auto Bezier / Hold, plus Easy Ease (F9). The handle is described by a number: the Keyframe Velocity dialog gives speed and influence 0.1…100 %. A segment between two keys has no shape of its own: it is the result of the outgoing side of the left key and the incoming side of the right key |
| Adobe Premiere | 26.x | Temporal Interpolation (right-click a key) | the same model as AE, but poorer: Linear / Bezier / Auto Bezier / Continuous Bezier / Hold; Ease In and Ease Out are set separately, there is no single "Easy Ease" button, and this hangs as a separate request. The default is Linear for the temporal side; you cannot set your own default |
| DaVinci Resolve | 21 | Ease In / Ease Out on the diamond in Inspector + Curve Editor | the shape belongs to the KEY: right-click the orange diamond gives Linear / Ease In / Ease Out / Ease In and Out; handles exist only in the Curve Editor. The ease itself has no parameters: its stiffness is always the same, and the Blackmagic forum has a request to make it quadratic/cubic, because the built-in one is too harsh. In 20.1 the mechanism was fixed, and projects created before 20 are read with the old (wrong) meaning |
| Final Cut Pro | 12.3 | key interpolation in the Video Animation editor | the shape belongs to the KEY: right-click → Linear / Ease / Ease In / Ease Out; there are no separate handles, the curvature of a stretch is set by `Command`-dragging the line itself between the keys — that is a gesture ON THE SEGMENT, but it still changes the sides of the neighbouring keys |

## The trick behind it

**1. "Shape of the SEGMENT" and "shape of the SIDES OF A KEY" are two different
mechanisms, not two interfaces to one. A program that has only one of them is
doomed to get requests for the other for the rest of its life.** The difference is
visible in a single phrase. "Make this piece zigzag" is a statement about the
segment: the stretch has its own character, the neighbouring stretches are not
touched, no keys are added. "Ease out of this pose more softly" is a statement about
the key: a side changes, and it changes BOTH adjoining segments at once. Vegas has
the first and not the second, so there it is impossible to have "a soft exit and a
hard entry" at one point; AE/Premiere/Resolve/FCP have the second and not the first,
so there "this stretch holds and the next one is linear" is expressed only by the
Hold type, and everything else has to be assembled from the two halves of the
neighbouring keys. Tellingly, both camps ask for exactly the missing half. Adobe
users ask for "automatic presets where you could define the default interpolation
for first, last and middle keyframes" (morphinapg, 28.02.2025), that is, a shape
assigned not by a gesture on a key but by a rule. Vegas users want a way to change
the default type of a segment, which was found only in the program's internal
settings.

**2. A closed set of shapes is cheaper than handles not because there are fewer of
them, but because the shape becomes a NUMBER in the file, not geometry.** In Vegas
the type of a segment is serialized as a small integer (1, 2, −2, 4, −4), and three
things follow that bezier handles do not have: the shape is visible in the diff of
the project file; it is transferred between segments with one command; and it cannot
be "almost the same" — it cannot be caught by the mouse and nudged by half a pixel.
The other side is named there too: since the shape is a closed list, adding a sixth
type is a change to the format, not a gesture of the user, and a "custom curve" is
inexpressible in principle. The owner's request runs exactly along this divide: a
closed set ("third-order", "straight", "zigzags") is Vegas, while a "formula that
generates from time" is no longer a shape of a segment but a source of value, that is
F-AUT-007.

**3. The stiffness of an ease is almost nowhere a parameter, and this is a separate
class of complaints that is not cured by adding one more type.** In AE the stiffness
is a number (influence 0.1…100 %, Easy Ease sets 33 % on both sides), so there users
ask not for "another ease" but for a button. In Resolve the stiffness is fixed, and
the Blackmagic forum has a request to replace it with a quadratic or cubic one, that
is, people argue not about the presence of smoothing but about its DEGREE. The
conclusion for anyone who introduces a closed set: almost every type except linear
and hold quickly acquires a request for one numeric parameter, and the place for it
should be provided at the very beginning, otherwise the sixth type will be called
"Smooth 2".

## Pros

- The shape of a segment is exactly what a person means when he says "this piece":
  the edit is local, neighbouring stretches do not move, no keys are added.
- A closed set is safe: a handle cannot be caught by accident, the curve does not
  become non-monotonic by itself, and the curve type is the same for all users —
  the same argument by which F-AUT-001 protects the Vegas/FCP variant.
- A shape as a small number is serialized, diffed, copied and printed; it can be
  applied to all segments at once with one command — exactly what the Adobe request
  asks for.
- Hold stops being a crutch of "duplicate the key one frame before": stepped
  animation does not double the number of keys.
- The computation stays a polynomial in the normalized position within the segment:
  no iterations, no solving of equations — so determinism (invariant 11) does not
  suffer, unlike x-monotonic beziers (F-AUT-001, item 2 "What gets in the way").

## Cons

- The shape of a segment cannot express different character of entry and exit at one
  point: "arrived softly and started abruptly" is inexpressible in the language of
  segments.
- A closed set is a format: a sixth type is added by a version of the envelope, not by
  the user, and a "custom curve" stays unreachable.
- The default becomes a setting: in Vegas the default type of a segment lives in the
  program's internal settings, not in the project — so the same file behaves
  differently for two editors on new points. The same class as `Lock Envelopes to
  Events` in F-AUT-002.
- The shape on a segment and the shape on a key do not convert into each other without
  loss, so importing foreign projects (and exporting ours) must choose what is lost.
- Each type except linear and hold quickly asks for a numeric stiffness parameter — and
  without it the dispute "too harsh / too soft" is not settled at all (Resolve).
- Shapes on segments double the amount of operations on a curve: reverse must mirror
  not only the keys but also the shapes (and swap asymmetric Fast/Slow), Split must cut
  the list of shapes, trimming must not lose the shape of a hidden segment.

## What users say

On the "shape of a key" side, users ask not for a new type but for a RULE: assign the
shape not by a gesture but by default.

> it would be nice to have automatic presets where we could define the
> default keyframe interpolation for first, last, and middle keyframes,
> rather than just always defaulting to linear.
> — morphinapg, Adobe Premiere Pro feature request, 28.02.2025,
> https://community.adobe.com/feature-requests-730/easy-ease-keyframe-interpolation-1328338

On the "shape of a segment" side users ask for the exact mirror — set the default
type — and it is found in the service settings, not in the interface:

> One would think that Sony would have the forethought to be able to
> specify a fade type.
> — Wayne Grauel, Creative COW (VEGAS Pro), 13.08.2010,
> https://creativecow.net/forums/thread/setting-default-fade-types-did-i-read-correctly/

> In the Internal Prefs panel, look for eDefEnvAudioASRType...Change the
> value to your preference.
> — Matt Crowley, same thread, 08.09.2010

And this one is about stiffness, which has no parameter: the dispute is not about
whether there is smoothing, but about its degree.

> Easing Curves in Keyframe Editor are too extreme.
> — thread title on the Blackmagic Design forum (DaVinci Resolve),
> https://forum.blackmagicdesign.com/viewtopic.php?t=218938
> — the page returns 403 from this environment; the title and the substance of the
> request (replace with quadratic/cubic) are taken from the search snippet; the
> wording of the message body is **NOT CONFIRMED** (2026-09-21)

## How it fits Kinotrain
CAPABILITY: openspec/specs/automation/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
The shape of a segment lives on the KEY; each type has its own parameters. The zigzag is recognized as a shape of a segment (revision of item 3), its amplitude in units of the parameter; for the speed of a warp only the straight line is allowed.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages: the shape in the model and in `curve_value`; carrying shapes through all operations on a curve; right-click on a segment and the command "all at once" |
| Where the main work is | kt-model (list of shapes on `ParamCurve`, `curve_value`, mirroring on reverse), kt-app (`automation.rs` — hit on a segment, showing the type; the command for the whole curve), kt-otio (envelope) |
| Project format | touches it: a curve gets a list of shapes. The default (today's two shapes) is not serialized — the `AudioCurve` and `is_default_curve` technique in `src/libs/kt-otio/src/meta.rs` is already proven, and files without shapes stay byte-stable |
| Tests to write | value at the boundary of segments is continuous for any pair of shapes; reverse swaps asymmetric shapes; Split and trimming do not lose the shape of a hidden segment; a shape on `rate` is rejected EXPLICITLY; GOLDEN_* did not move; round-trip of the format next to t192/t222 |
| Depends on | F-AUT-001 (bezier handles — the same line from the other end, to be decided together), F-AUT-002 (key domain), F-AUT-007 (a shape "by formula" is no longer a shape of a segment, but a source of value) |
| Breaks compatibility | forward no, backward yes — an old build must explicitly reject a file with a non-empty list of shapes |

## Verdict

**Candidate — and it must be decided with ONE decision together with F-AUT-001, because both notes correct one and the same line from opposite ends.**
The reconnaissance broke the owner's request ("setting segment shapes… you can turn on
third-order curves for everyone, you can have zigzags, you can have straight lines, you
can change the parameters of the curves or set a formula that generates from time, or
configure a separate curve your own way") into three different mechanisms, and they
cannot be taken in one stage:

1. **A closed set of shapes ON THE SEGMENT** — this is Vegas, and it is our reference
   editor for UX: the shape by right-clicking a part, neighbours untouched, the shape
   serialized as a small number. Cheap, deterministic, fits into `curve_value`, and
   repeats the technique we have already proven with `AudioCurve`.
2. **"Turn it on for everyone at once"** — this is not a new shape but a command over
   the curve, and it also closes the Adobe request about the default interpolation.
   Keeping this as a setting of the MACHINE, as Vegas did (`eDefEnvAudioASRType`), is not
   acceptable: we would get exactly the class that F-AUT-002 found in `Lock Envelopes to
   Events` — one file, two behaviours.
3. **"A formula that generates from time" and "zigzag" are no longer a shape of a
   segment**, but another SOURCE of value, that is F-AUT-007. A zigzag specified by a
   number of periods cannot be expressed by keys at all, and trying to squeeze it in as a
   sixth type into the closed list is the direct road to "Smooth 2".

Two caveats that must be written down before work starts. The first: for every type
except linear and hold, a numeric stiffness parameter is almost immediately requested
(Resolve), and room for it should be provided at once, not as a sixth type. The second:
`rate` remains an exception forever — `src/libs/kt-model/src/warp.rs:575` already today
discards `ease` on speed, because the closed-form inverse map of the warp stands on a
piecewise-linear profile; a new shape must be discarded in the same place and shown to
the user as an error, not as silence.

### REVISION of item 3 (R.61, owner's decision)

**Item 3 above — "a formula that generates from time and a zigzag are no longer a shape of a segment, but another SOURCE of value" — is revised in the part about the ZIGZAG.** The owner decided: a zigzag is a shape on the segment between two adjacent keys, on a par with the straight line and third-order.

The reason for the decision: a zigzag between two keys has a beginning and an end, and they are set by the keys; a source, on the other hand, describes the curve AS A WHOLE and has no keys at all. Mixing them would mean demanding a formula where a person wants "jitter on this piece".

A consequence, which the note predicted correctly and which was accepted TOGETHER with the decision: a shape must have its own set of parameters, not a single stiffness number (caveat 1 above). Therefore a shape is an enumeration with data: a straight line without parameters, third order and ease with a tension, a zigzag with a number of periods, an amplitude and a profile. With such a shape, a sixth type breaks neither the file nor the screen, and "Smooth 2" does not arise.

Separately found during the analysis: **the zigzag amplitude is set in units of the parameter, not as a fraction of the difference between the keys** — the most common zigzag is jitter around a CONSTANT value, where the difference is zero; it is clamped by the target's passport.

Item 3 remains in force in the part about "a formula that generates from time": it is a source (F-AUT-007). Items 1 and 2 remain in force entirely, and the caveat about `rate` is even more so: speed accepts only the straight line and refuses loudly.

## Sources

- https://help.magix-hub.com/video/vegas/22/en/content/topics/11-autoscript/adjusting_envelopes.htm
  — "right-click an envelope segment and choose a command": the shape
  belongs to the segment (2026-09-21)
- https://creativecow.net/forums/thread/setting-default-fade-types-did-i-read-correctly/
  — the types of a segment as numbers (2 = fast log, 1 = linear, −2 = slow log,
  4 = smooth cubic, −4 = sharp cubic) and `eDefEnvAudioASRType` as the default in the
  service settings; quotes by Wayne Grauel and Matt Crowley (2026-09-21)
- https://community.adobe.com/feature-requests-730/easy-ease-keyframe-interpolation-1328338
  — the request about the default interpolation for first/last/middle keys,
  28.02.2025 (2026-09-21)
- https://forum.blackmagicdesign.com/viewtopic.php?t=218938 — "Easing
  Curves in Keyframe Editor are too extreme", the request about quadratic/cubic
  stiffness. The page returns 403; the content is from a search snippet, **[NOT VERIFIED]**
  verbatim (2026-09-21)
- https://helpx.adobe.com/after-effects/using/keyframe-interpolation.html
  and https://ae-scripting.docsforadobe.dev/other/keyframeease/ — interpolation types
  at the KEY and influence 0.1…100 % (from the analysis of F-AUT-001, not re-opened in
  this pass)
- https://support.apple.com/guide/final-cut-pro/adjust-effect-curves-ver8e3f1f04/mac
  — Linear / Ease / Ease In / Ease Out and `Command`-drag along the line (from the
  analysis of F-AUT-001)
- https://davinciresolveclub.com/davinci-resolve-keyframes-curve-editor/ — ease on the
  Inspector diamond and handles only in the Curve Editor (2026-09-21)
- Ours: `src/libs/kt-model/src/project.rs` (`ParamCurve`, `curve_value`,
  `AudioCurve:731`), `src/libs/kt-model/src/warp.rs:575`,
  `src/libs/kt-otio/src/meta.rs` (`is_default_curve`),
  `src/libs/kt-engine/src/video.rs:184` (`fx_chain_hash`),
  `src/apps/kt-app/src/automation.rs`,
  `src/apps/kt-app/src/fx_editor.rs`, `docs/user-feedback.md` section
  "Round R.59"

No editor was run live: Vegas from the MAGIX/Boris FX help and Creative COW, Adobe from
the help and the feature-request forum, Resolve from a third-party analysis and a search
snippet of the Blackmagic forum (the forum itself returns 403 from this environment, the
caveat of the first wave in `_editors.md` remains in force), FCP from Apple's manual.
