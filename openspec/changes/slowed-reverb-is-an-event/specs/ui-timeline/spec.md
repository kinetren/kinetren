## ADDED Requirements

### Requirement: An editing window refuses a kind of effect or an event only for a technical reason it names
The morpher window's effect picker SHALL offer every effect kind of the registry, video and audio
together, as its preset list already does; a picked audio effect SHALL land in the patch's audio chain
and act on the audio targets. An event carrying slowed reverb (a time warp, with or without a speed
curve, a shaped curve or a formula) SHALL be copied, pasted, cut to the clipboard and split at the cursor
by the same gestures and with the same result as any event: no refusal, the chain and the curves travel,
every piece's material is the integral of its speed. Where a window does refuse something, it SHALL say
the technical reason in the status line, and that reason SHALL hold in the current engine.
Why: ktprompts v26.10.18 items 1 and 3 — "the morpher does not let me put audio effects on", "slowed does
not let me copy-paste the event and cut it — childish excuses". The morpher's single strip (R.35 S2) passed
"video" to the picker by oversight; the "cannot cut a warped clip" answer outlived its reason (R.39).

#### Scenario: The morpher picker offers audio effects
- **WHEN** the harness app has a morpher over an audio and a video track; its window is opened; "+ effect…" is clicked; "pitch" is typed in the search; Pitch shift is picked; the window is closed and the project rendered
- **THEN** the picker lists the audio kinds (Reverb, Pitch shift, EQ, Compressor, …) next to the video kinds; Pitch shift appears in the morpher's single strip after the morph slot; the model holds it in the patch's audio chain; the audio target sounds shifted inside the block and unchanged outside it
- **PATH** right-click the morph track → "Add morpher" → the purple block appears → double-click it → the morpher window opens with one chain strip starting with the purple morph slot → click "+ effect…" → the gallery lists video effects and, under their own heading, audio effects → type "pitch" → Pitch shift remains → click it → the strip shows "Pitch shift" after the morph slot → close the window → press Space → inside the block the sound is higher
- **TEST** t912 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A slowed event is copied, pasted, cut and split like any event
- **WHEN** the harness app has an audio event with slowed reverb at 0.8× and a speed curve 0.5→1.5 (linked to its video event); the event is selected and Ctrl+C, then the cursor moved and Ctrl+V; S is pressed with the cursor in the middle of the original; one half is selected, copied (Ctrl+C), deleted (Delete) and pasted elsewhere (Ctrl+V) — the program has no Ctrl+X; a region over half the event is copied and pasted
- **THEN** no gesture is refused and no error note appears (the split was refused before this change: the right half's speed keys moved with the anchor to a negative tick, and validation rejected the whole batch — `BadFxAuto`); the pasted copy has the same chain, bypass and curves, and sounds byte-for-byte like the original over its length; the split pieces' entry points are the integral of the speed and their joint has no material gap; the link partner is split and pasted with it; the region copy holds the piece by the same integral
- **PATH** click the slowed event → it is outlined → Ctrl+C → the status line says "1 event copied" → click the ruler further right → Ctrl+V → a copy with the same "Slowed reverb" badge appears at the cursor with its video partner → click the middle of the original on the ruler → press S → the event and its partner split into two → click the right half → Ctrl+C → Delete → it disappears → click the ruler elsewhere → Ctrl+V → it reappears there, the filmstrip continuing where the left half ended
- **TEST** t917 (`src/apps/kt-app/src/reach_tests.rs`)
