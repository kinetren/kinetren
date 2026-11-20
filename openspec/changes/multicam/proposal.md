# Proposal: multicam

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3505–t3519.

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

- **A multicam composition**: "Create multicam" over selected clips builds a composition whose video
  tracks are the ANGLES (each with its linked audio), aligned by sound (the existing sync), by timecode or
  by start; an instance replaces the selection. Per-angle effects live inside it, on the angle's track
  (grade each camera once); effects on the instance apply to what plays.
- **An instance shows one angle**: an instance carries its active angle (video) and its audio choice; the
  engine composes only that angle's track, so inactive angles cost no decode in the edit.
- **Switching is an edit of the instance, not a morpher**: cut-and-switch splits the instance at the
  playhead and sets the right piece's angle; switch-only changes the angle of the piece under the
  playhead. It works while playing; one live pass (Play → switches → Stop) is ONE undo step.
- **The angle viewer**: while the playhead stands on a multicam instance, the preview area can show the
  angles as a grid (4 / 9 / 16) beside or instead of the program picture (Q3), the active one outlined;
  a click on a cell, a digit 1–9 or the menu does the same switch (UI rule 17); Alt switches without a
  cut. Cells compose at a coarse size through the one plan at a lower priority than the program frame.
- **Sound does not follow the video by default** (Q2): the instance's audio stays on the chosen angle's
  sound (or a separate recorder track) through every cut; a mode "audio follows video" exists per
  instance.
- **Flatten**: "Expand multicam" turns pieces into ordinary events of their active angles (the existing
  `expand_instance`, keeping only the active angle per piece).

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

- kt-model: `Sequence.multicam: Option<Multicam { angles: Vec<Angle { video: TrackId, audio:
  Option<TrackId>, name }> }>`, `ClipEvent.angle: Option<AngleSel { video: u16, audio: AudioSel }>`;
  `resolve_reel` applies the selection (other angle tracks drop out); commands `CreateMulticam`
  (an ops batch), `SetAngle`; ops `cut_switch`; validation.
- kt-engine: nothing new in the plan — a resolved reel already omits the dropped tracks; the grid cells
  are plans of the reel with each angle selected, composed by the compose thread at class `background`.
- kt-app: the angle viewer in the preview area, digits in `controls.rs`, the create dialog (sync method),
  menus, i18n ×5; live passes coalesced (`apply_coalesced`).
- Format: next free version by content; a row in "What OTIO export loses".
- Idle: on a pause the grid composes each cell once per moment and stops; at rest nothing runs.

## Questions to the owner

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
