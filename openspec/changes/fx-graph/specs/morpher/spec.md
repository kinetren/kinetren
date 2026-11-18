## MODIFIED Requirements

### Requirement: A patch carries both a source swap and the effect chains
One morpher event SHALL carry simultaneously a source replacement (with an absolute entry point or a delta; the absolute one takes priority) and effect chains of both kinds. The switch of the morph slot SHALL mute only the replacement. Deleting media referenced by a patch MUST be rejected by the list of the patch's references, not by the kind of morpher. The chain for video SHALL include the time warp from the patch's audio chain: the warp is a rule for choosing the frame, not a pixel filter, otherwise the layer would slow down the sound and not slow down the picture.
Why: two mutually exclusive kinds of morpher forced the user to create two events for one edit.

#### Scenario: A patch with a replacement and chains
- **WHEN** a patch contains media as well as video and audio effects, and the morph slot is switched off
- **THEN** the source is not replaced, the effects are applied; deleting the patch's media is rejected
- **TEST** t243 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: A warp in a patch slows down the picture too
- **WHEN** a patch carries a slowdown in the audio chain and lands on a video clip
- **THEN** the video clip receives the warp in its own chain and is not cut in any target mode
- **TEST** t247 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: One chain lane with a pinned head-replacement
- **PATH** double click a morpher block carrying a Glow and a Gain → the morpher window opens with the node editor framed: the purple Source-swap node first, Glow on the picture wire, Gain on the sound wire → drag Glow onto the wire before the Output → it moves there → drag a wire from Glow's output to the sound socket of Gain → it snaps back with a hint → drop a file on the Source-swap node → the source field beside the graph takes it
- **WHEN** the morpher window is open with effects of both kinds
- **THEN** one node editor shows them: the first node is the source replacement (it is not reordered, has no remove), picture effects are wired picture to picture and sound effects sound to sound, a wire never joins the two kinds; the source field accepts a file and can return the original source
- **TEST** t244, t218 (`src/apps/kt-app/src/layout_tests.rs`, rewritten from the strip by the owner's "Вместо полоски", kkt-26.11.18b line 21)

## ADDED Requirements

### Requirement: A patch holds both halves of a paired device, each switchable
When a paired kind's slot is added to, edited on or removed from one chain of a morpher patch, the model SHALL, in the same `SetMorpher` and the same undo step, give the patch's other chain the counterpart slot: the n-th paired slot matches the n-th counterpart, values and curves copied from the edited side, a missing one appended at the chain's end, an extra one removed. Presence SHALL be shared; each half's bypass SHALL be its own. The render SHALL apply exactly the patch's enabled slots — no half is derived. A patch read from a file written before this rule SHALL be given, on reading, exactly the halves the render used to derive, appended where the render put them; a half the patch held switched off SHALL count as held (it stays off). In the morpher window's chain a sound effect SHALL carry the speaker mark, since both halves of a device carry one name.
Why: kkt-26.11.18b item 3 — "нет возможности отключить морфинг эффекта для аудио": the derived sound half had no slot, so no switch, and a switched-off half added by hand was derived again.

#### Scenario: VHS Tape put by a layer, its sound switched off
- **WHEN** a patch gets VHS Tape on its picture chain with a tracking curve; the sound half is switched off; the picture slot's tracking is edited; VHS Tape is removed; each step undone
- **THEN** the patch's audio chain holds `vhs_tape_sound` with the curve; switched off, the audio events under the layer get no device slot and the video events still get `vhs_tape`; the edit reaches the sound half's values while it stays off; removal takes both; undo restores each state in one step
- **TEST** t2990 (`src/libs/kt-model/tests/sim_pairs.rs`)

#### Scenario: An old patch gets its halves on reading
- **WHEN** a project written before this rule, with a layer carrying Datamosh on the picture and a gain on the sound and a layer carrying VHS Tape with a tracking curve, is read; read again after a save; then the tape's sound is switched off, saved and read
- **THEN** the first layer's sound chain is `gain, datamosh_sound`, the second holds `vhs_tape_sound` with the curve — the chains the render derived; a second reading changes nothing; the switched-off sound stays the only one and stays off
- **TEST** t2991 (`src/libs/kt-model/tests/sim_pairs.rs`)

#### Scenario: Switching the sound half off leaves the picture untouched
- **WHEN** a layer carries Camera ISP over a linked pair; the frames under it are rendered with the microphone half on and off; then the sound with both halves on is compared to an event carrying the same device directly
- **THEN** the picture is byte for byte the same in both renders; with both on, the sound equals the event-carried device's sound
- **TEST** t2992 (`src/libs/kt-engine/tests/sim_pairs.rs`)

#### Scenario: The sound half's switch in the morpher window
- **PATH** double click a morpher block carrying VHS Tape → the morpher window shows "VHS Tape" and its sound half marked with the speaker (the node editor: one VHS Tape node with a picture lane and a sound lane) → click the sound half's switch → it is struck through (the lane dims) → the picture half is unchanged
- **WHEN** the person switches the sound half off in the morpher window
- **THEN** the patch's audio bypass for that slot is on in the MODEL, the video chain is unchanged, and one undo step brings it back
- **TEST** t2993 (`src/apps/kt-app/src/vhs_tape_tests.rs`; on the node editor in v1c), live `morph_pair` (`src/scripts/live/morph_pair.py`)
