# Proposal

## Why

kkt-26.11.01 item 15, third answer (`docs/user-feedback.md`, "The owner on the pitch question and the
pace", 2026-11-01). Since `pitch-context-whole-chain` the pitch reads the event's material across its
edges with the whole chain before it, so the echo of an effect standing BEFORE the pitch (the event's
own Slowed+Reverb under a morpher's pitch, in the owner's project) is cut at the event's end. Asked
about it, the owner answered: "the pitch question is unclear, what does reverb have to do with
pitch?!? well then make an option so it can be done this way and that way."

## What Changes

- A switch on the pitch effect, named in the owner's words: **"Echo of the effects before the
  pitch: cut at the event's end / rings on after the event"**. "Cut" is the default and the current
  sound byte for byte (GOLDEN_AUDIO, t1105–t1114 and t973c untouched).
- "Rings on": everything up to the event's end sounds as with "cut" (the pitch still reads the
  material across both edges, so the joins keep no dips); from the end on, the echo of the effects
  before the pitch rings on, pitched. It carries none of the material after the out-point and has no
  hole: at the out-point a second stream of the nodes up to the pitch is split off (forked from the
  running state) and fed silence; it takes over the output from the out-point on. A piece of a split
  whose material the next piece continues does not ring (the next piece's context carries the echo).
- The plugin contract gains an optional `fork` of an audio state (every built-in audio kind
  implements it) and a declaration of the row that chooses the echo for an edge-reading kind.
- The `.kt` file writes the switch only when it is "rings on", and such a file is format 3: an
  older build refuses it ("written by a newer Kinetren") instead of silently cutting the echo. The
  OTIO door carries it in the slot's `metadata["kt"]` form.
- The pitch panel shows the switch with a hint; five catalogs.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `audio-fx`: the edge-reading requirement gains the echo choice of the nodes before the edge-reader;
  the plugin contract gains `fork`.
- `audio-render`: the split rule for "rings on" (a piece the next piece continues does not ring; a
  ringing piece hands nothing on).
- `project-format`: format 3 by content for a slot whose echo rings on.
- `ui-reach`: the pitch panel's switch is reachable and says what each choice does.

## Impact

- `src/libs/kt-plugin-api` (`AudioState::fork`, `Decl.echo_row`), every audio plugin family
  (`src/plugins/fx-audio-*`: `Clone` + `fork`), `kt_plugin_api::dsp` primitives (`Clone`).
- `src/libs/kt-fx/src/audio/mod.rs` (`ClipProcessor`: the tail twin and its promotion at the out-point);
  `src/libs/kt-engine/src/audio.rs` (`split_continued`).
- `src/libs/kt-model` (`project_file::version_for`, a predicate over every slot of the document).
- `src/apps/kt-app` (`fx_editor.rs` pitch panel, i18n ×5, font subsets).
- Tests t1330–t1339; docs: feature note, What's new, K15 row.
