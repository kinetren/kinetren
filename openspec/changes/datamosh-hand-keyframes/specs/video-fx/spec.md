# Spec Delta

## ADDED Requirements

### Requirement: Datamosh takes keyframes placed by hand
Datamosh SHALL have a gate row whose rising edge makes that position a scheduled I-frame, restarting the period; with the
period 0 the placed ones SHALL be the only ones besides the start and the cut; a file without the row SHALL render byte for
byte as before. A gate row SHALL take a pulse from its lane in one undo step.
Why: the owner (kkt-26.11.10b K9): "раставлять кейфреймы руками, а не только как сейчас с определенной скважностью".

#### Scenario: Two keys and no period
- **WHEN** a 60-frame mosh with period 0 has the row's pulse at frames 20 and 40, then none, then `mosh` on across both
- **THEN** the first has I-frames at 0, 20, 40 and the cut; the second only at 0 and the cut; with `mosh` the two are dropped as any scheduled I-frame
- **TEST** t4292 (`src/plugins/fx-video-codec/tests/hand_keys.rs`)

#### Scenario: The pulse is one step and moves as one
- **WHEN** the pulse is added at the playhead, dragged by 10 frames and undone
- **THEN** one undo removes the add; the drag moves both keys and keeps the one-frame width; a pulse over another is refused with the reason
- **TEST** t4293 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: A person places a keyframe
- **WHEN** they stand the playhead at a beat and press "Key here" in the Datamosh lane
- **THEN** a mark stands on the lane and the mosh restarts there in the preview
- **PATH** open the effect of an event with Datamosh → the lane "Keyframes" → move the playhead → click "Key here" → a mark appears at the playhead; play across it → the picture is clean for a frame; drag the mark → it moves; Ctrl+Z → it returns
- **TEST** t4294 (`src/apps/kt-app/src/reach_tests.rs`)
