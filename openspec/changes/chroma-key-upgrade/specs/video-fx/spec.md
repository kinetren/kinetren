## ADDED Requirements

### Requirement: The greenscreen keys by colour difference with a screen balance
The greenscreen SHALL offer a colour-difference model: the screen-ness of a pixel is its key channel's excess
over the balance-weighted mix of the other two channels relative to the screen colour's own excess, remapped
by clip black and clip white into the alpha; then the alpha SHALL be choked and softened by the given pixels,
and the despill SHALL remove the excess by its bias and restore the pixel's luma (or replace it with a chosen
colour, or neither). Every greenscreen SHALL use this model: a slot written before it SHALL be read through a
fixed mapping of its key colour, tolerance, softness and spill, and the distance model SHALL NOT exist.
Why: F-MSK-004 — a distance means different things on green and blue; subtraction greys light hair; the owner,
2026-11-20 (A22) — "Все ключи на новую модель".

#### Scenario: Green and blue screens key equally
- **WHEN** a synthetic subject (skin, white shirt, light hair strands) over a green and over a blue screen with noise is keyed with balance 0.5 and 0.95
- **THEN** both mattes are within 2 % of the known alpha over the subject and the screen; the distance model at its best tolerance leaves a dirtier blue matte than green
- **TEST** t3558 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Restored despill keeps light hair light
- **WHEN** light hair strands lit by green spill are despilled with subtraction and with restore
- **THEN** with restore their luma equals the original's within one code value and the green excess is gone; with subtraction they are darker by the removed green
- **TEST** t3559 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Old keys open with the new model
- **WHEN** the golden projects with greenscreen slots written before this change are opened, composed and saved
- **THEN** every slot keys with the difference model through the mapping (its matte over the fixture's known alpha is no worse than the distance model's was); the files are written back with the same text; the re-stored reference frames match
- **TEST** t3560 (`src/libs/kt-gpu/tests/gpu_render.rs`), t3561 (`src/libs/kt-model/tests/plugin_format.rs`)

### Requirement: Analyze sets the key from the picture in one step
Analyze SHALL compose the key's input at five moments across the event, find the screen as the dominant
saturated green or blue cluster, and set the key colour, the balance, clip black and clip white and the
despill in one undo step, saying what it set; when no screen is found it SHALL change nothing and say why.
Why: the owner's B3 — "кнопку автоанализа … чтобы не подбирать всё руками".

#### Scenario: A plausible key from one click
- **WHEN** Analyze runs on a fixture of a subject over an uneven green screen, and on a frame with no screen
- **THEN** the key colour lies within 6 code values of the screen's mean, the resulting matte is within 3 % of the known alpha; the second run changes nothing and reports "no saturated green or blue area"
- **TEST** t3562 (`src/libs/kt-engine/tests/key_analyze.rs`)
