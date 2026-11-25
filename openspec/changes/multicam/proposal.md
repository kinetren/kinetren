# Proposal: multicam

> DESIGN, revision 2 (2026-11-25), base `fc60a5d9`: implementation-ready after `shared-events`. The owner answered Q1–Q3 (A18).
> Code starts on the orchestrator's go. Tests: t3505–t3514, t4247–t4249.

## Why

The owner, `kkt-26.11.01` M1 (line 55; registry `kkt-26.11.01-index.md`): "Удобный multicam, т.е. так
чтобы не нужно было страдать при сведении камер, у нас по коду всё есть - тот же морфер - спокойно
меняет сурсы туда-сюда. Но это не значит что я прошу это делать через ui морфера "в тупую", нужно
делать удобно и не выкидывать фичи куда-попало как это обычно делают агенты - запрещаю." Earlier:
`v26.10` N1 "multicam".

Note F-TL-005 (2026-09-07) deferred it for two reasons that are gone now: there was no nesting (F-TL-004
is done — compositions, R.50–R.59) and no sync (F-SPD-004 is done — `kt_media::sync::offset_from_pyramids`,
R.49). It also found that N live decoders are already normal here (a worker per media), so the cost is
the cache and the interface, not invariant 5.

## What Changes

- **A multicam composition**: "Create multicam…" over selected clips builds a composition whose video tracks are the ANGLES (each with its
  linked audio), aligned by sound, by timecode (now read from the files) or by start. Per-angle effects live inside it.
- **An instance plays one angle and one sound**: two handles of the composition, set per piece as composition parameters are; hidden angles
  cost no decode.
- **Switching is an edit of the piece, not a morpher**: a click on a cell or a digit cuts and switches; with Shift it switches without a cut.
  It works while playing; one pass from Play to Stop is ONE undo step.
- **The sound does not jump**: one chosen sound through every cut by default, and the cut itself is not heard (the composition's sound stream goes
  on across it); "Audio follows the angle" per instance ("сделай опцию звук за ракурсом").
- **The angle grid** in the preview area, with each angle's own effects, never ahead of the program picture.
- **Expand** turns pieces into ordinary events.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `compositions`: a multicam composition (angles = tracks), an instance's active angle and audio choice,
  switching by split + angle, expand.
- `media-index`: the angle viewer's cells as a decode consumer at their own size and the lowest priority.
- `project-format`: multicam and angles raise the file by content; OTIO export keeps the nested stack
  with only the active angle's tracks enabled.
- `ui-reach`: create, switch, the angle viewer, audio mode, expand.

## Impact

- kt-model: `Sequence.multicam`, `ParamOwner::Angle`, reserved handles, `ops::{create_multicam, cut_switch}`, `SetAngle`, validation; the next file
  version by content.
- kt-ffmpeg, kt-media: the source timecode. kt-engine: the nest stream handed on at a split, `AngleCells`.
- kt-app: the dialog, the grid, digits in `controls.rs`, menus, i18n ×5. OTIO: the planned row. Idle: nothing without the grid.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20.txt` lines 28–29, A18): "Q28 multicam: «Композиция ракурсов (Recommended)»" — Q1 (a); "Q29/Q30
multicam sound and grid: «п.1 и сделай опцию звук за ракурсом»" — Q2 (a) with the option of the sound following the angle, Q3 (a). Read again on
2026-11-25: "п.1" answers the pair Q29/Q30, option (a) of each; the option he adds is the per-instance mode of Q2 (a). His M1 (line 55, whole above)
forbids the morpher's UI and features "куда-попало": the angle stands where a composition's parameters stand, the grid on the preview. No open
question.

The questions as asked:

**Q1. What is a multicam structurally?** Your words: "тот же морфер - спокойно меняет сурсы туда-сюда.
Но это не значит что … через ui морфера "в тупую"". Today a morpher layer swaps a source over a TIME
WINDOW for the events under it, cutting them at its edges.
- (a) **A composition of angles + an angle choice on each instance piece** (*recommended*: Premiere's
  and Resolve's model; per-angle grading inside, one place to sync; a switch is a split + a value, so
  undo, ripple and copy treat it as any edit; it reuses compositions, not the morpher's UI).
- (b) A morpher-like switch track over the angle tracks (each switch block a morpher patch with a source
  swap): reuses the morpher engine directly, but the cameras stay N tracks on the root timeline and every
  ripple/trim must keep the switch blocks aligned with them.
- (c) Vegas's takes: each event carries a list of alternative sources; no composition; per-angle effects
  are impossible (one chain per event).

**Q2. Does the sound follow the angle?** Note F-TL-005 trick 2: for speech, switching the microphone at
every cut changes timbre and room tone; for a concert each camera's sound may matter.
- (a) **No by default: the instance keeps one chosen sound (an angle's or the recorder's) through every
  cut; "audio follows video" is a per-instance mode** (*recommended*).
- (b) Follows by default, as Vegas.

**Q3. Where is the angle grid, and do cells show each angle's effects?** UI rule 22; Vegas shows raw takes
and bypasses effects to keep the frame rate.
- (a) **In the preview area, switched with a button: grid beside the program picture; cells composed with
  each angle's own effects at 1/8 size, dropping frames before the program frame does** (*recommended*:
  you choose by what the camera will look like).
- (b) In the detached preview window only.
- (c) Raw cells (no effects), cheaper.
