# Proposal: edit-analysis

> DESIGN ONLY (spec session, 2026-11-24). Registry `v26.10-index.md` N5. Note F-MED-011. Tests: t4211–t4228. Takes
> candidate 3 of `kkt-26.11.01-candidates.md` (the cut detector) with it.

## Why

The owner, `docs/ktprompts/v26.10.txt` line 90, whole: "функции динамического анализа аудио/видео - например, темпа/бита,
чтобы делать динамический монтаж, ведь смотреть на то же статическое лицо человека не интересно, можно определять что
где-то есть более выразительное лицо, или движение или т.д. даже тот же md sad уже полезен для построения более сложного
анализа."

**Reading, checked.** Tempo and beat, an expressive face, motion, MD/SAD are EXAMPLES ("например", "или т.д."). What he
asks for is a layer: functions that analyse the material's sound and picture, so that a person can make a DYNAMIC edit —
know where the edit is dull (a face that does not move), where the footage has something livelier, and where the music
wants a cut. "Для построения более сложного анализа" says the layer must be built upon: a simple measure (SAD) is an
input of the next one. So the design is a set of named measures per file that anything can read, and the first things
that read it; a "beat detector" button alone would answer an example. The row's state was "future" by the registry's
hand: the line stands among the NLE items, and "на будущее" is said of other lines, not of this one.

Today (`286c406c`): per-file measures exist one by one, each in the hands of its first consumer — peaks (the waveform),
loudness (a hint), the audio series (reactive generators), sync offsets. Nothing looks at the picture. A beat, a cut or a
motion cannot be seen, snapped to, marked or linked.

## What Changes

- **Measures of a file** (`material-analysis`): named time series in material time, computed on request by an offline job,
  kept in the cache folder, the same bytes on every machine:
  - sound: level and bands (exist), **onsets**, **beats and tempo** (a grid that follows a changing tempo), bars;
  - picture: **motion** of what moves in the frame with the camera's own motion taken out (block SAD), the **camera's
    motion**, **cuts** (flashes are not cuts), brightness, sharpness.
  A measure may be computed from other measures, and is declared with what it reads: that is the "more complex analysis".
- **Seen on the timeline**: a switch on the track header draws beats on the waveform and cuts with a motion curve on the
  filmstrip. Shown beats and cuts are snap targets.
- **Corrections**: a wrong tempo (double, half, shifted) is corrected on the spot — ×2, ÷2, "the beat is here" — and kept
  in the project.
- **What one does with them**, each shown before it acts and one undo step:
  - Mark beats; Mark cuts; **Split at cuts** (a delivered film or a stream recording);
  - **Cut on the beat**: the joints of chosen butt-joined events roll to the nearest beat; or one event is split on every
    N-th beat;
  - **Still stretches**: the places of the EDIT where the picture does not move for N seconds (his "то же статическое
    лицо");
  - **Find moments**: a list of the liveliest spans of the chosen sources by motion, sound and speech, with "Instead of
    this" for a selected dull event; a row is scrubbed in the bin and dragged to the timeline.
- **Linked**: any property may follow a measure of an event's material — a zoom on the beat, a blur by motion — through
  the one link system (R.61). A render computes what its links read before its first frame.
- **Faces in a second stage**: "более выразительное лицо" needs a face detector, which is a neural component; it comes
  through the library of downloads after whisper's door exists (stage B). Expressiveness itself is our own measure:
  the motion left inside the face's box after the box is held still.

## Capabilities

### New Capabilities

- `material-analysis` (with `pause-detection`): measures, their store, their dependencies, beats, motion, cuts.

### Modified Capabilities

- `automation`: a link may read a measure of an event's material.
- `ui-timeline`: the measures drawn on tracks, snapping to them.
- `ui-reach`: the commands, the correction of a tempo, Find moments.
- `project-format`: tempo corrections and measure links by content.

## Impact

- kt-plugin-api: `measure.rs` — the passport of a measure (id, version, inputs, cadence) and the kernels; `audio_series`
  becomes the first of them, its bytes unchanged.
- kt-media: the analysis pass over a short-lived source (luma at 160 wide), `Measures` in the cache folder as kinds of
  their own.
- kt-model: `MediaRef.beats: Option<BeatFix>`; `ParamOwner::Material`; batches for the commands.
- kt-engine: the registry of `listen.rs` holds any measure; `ensure` before an export computes what links read.
- kt-app: the track switch, overlays, the commands, the Find moments panel (a tab beside the bin); i18n ×5.
- Format: the next free `.kt` version by content. OTIO: a planned row (a measure link is its static value).
- Idle: no measure is computed without a request or a link that reads it; overlays are meshes per (measure, zoom).

## Non-Goals

- **Assembling an edit by itself.** The layer answers "where"; the person cuts (rule 20). A draft assembly could be built
  on it and would be his own later word.
- Scene description, object search (his FUT list, "objectdetection/scene-description").
- The mix after effects as the source of beats (owner check C12 asks the same of the generators; one answer for both).

## Questions to the owner

None. The order (motion, beats, cuts first; faces after the downloads' door) is an engineering matter: both halves of his
line are taken.
