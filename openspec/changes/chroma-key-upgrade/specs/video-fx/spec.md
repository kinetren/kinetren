## ADDED Requirements

### Requirement: The greenscreen keys by colour difference with a screen balance
Every greenscreen SHALL key by the excess of the key colour's leading channel over the balance-weighted mix of the
other two, relative to the key colour's own excess; `tolerance` and `softness` SHALL be measured in code values along
that excess, so that a pixel differing from the key colour in the leading channel alone keys as `tolerance` and
`softness` say. A key colour with no excess SHALL key by closeness in luma and chroma under the same numbers. The
formula SHALL be one text in the fused geometry pass and at a barrier position; the RGB distance SHALL NOT exist.
Balance SHALL be 0.95 for a blue screen and 0.5 for a green one unless a person sets it.
Why: F-MSK-004; the owner, kkt-26.11.20 A22 — "Все ключи на новую модель".

#### Scenario: Green and blue screens key equally
- **WHEN** a synthetic subject (skin, white shirt, light hair strands) over a green and over a blue screen with noise is keyed with the automatic balance
- **THEN** both mattes are within 2 % of the known alpha over the subject and the screen, and neither is dirtier than the other by more than 1 %
- **TEST** t3558 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Old numbers keep their cut on the key axis
- **WHEN** a ramp from the key colour down the leading channel is keyed with tolerance 70 and softness 25, inside a fused geometry series and at a barrier position; a key colour of mid grey is used on a grey wall
- **THEN** alpha is 0 up to 45 code values from the key colour, 1 from 70 on and linear between, the same in both positions within the class's tolerance; the grey wall keys and the coloured subject stays
- **TEST** t3567 (`src/libs/kt-gpu/tests/parity.rs`)

### Requirement: The despill keeps the luma and runs on the whole frame
The despill SHALL take the excess by its strength from the leading channel of every pixel and, by its mode, restore the
luma that went, add a chosen colour for it, or neither; restoring SHALL be the default.
Why: F-MSK-004 — a despill by subtraction greys light hair.

#### Scenario: Restored despill keeps light hair light
- **WHEN** light hair strands lit by green spill are despilled in the subtract and in the restore mode, with the matte choked by −3 px
- **THEN** with restore their luma equals the original's within one code value and no green excess is left, in the grown edge too; with subtract they are darker by the removed green's luma
- **TEST** t3559 (`src/libs/kt-gpu/tests/parity.rs`)

### Requirement: The matte is choked and softened in pixels of the picture
`choke` SHALL move the matte's edge inward (positive) or outward by its pixels and `soften` SHALL blur the alpha alone by
its radius, both scaled with the resolution step; a key with either SHALL leave a fused geometry series and run as a
pass of its own, and a key with neither SHALL stay a tap inside it.
Why: one plan (invariant 7); a neighbourhood is not point-wise.

#### Scenario: Choke and soften at K 1 and K 2
- **WHEN** a keyed disc is choked by 5 and by −5 px and softened by 4 px, at K 1 and K 2
- **THEN** the half-alpha radius moves by 5 px inward and outward within half a pixel at both K, the colour channels equal the unsoftened frame's, and the two K agree within the class's tolerance
- **TEST** t3560 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: A key inside a geometry series
- **WHEN** [event grid, greenscreen, zoom] is planned with choke 0 and soften 0, then with choke 2
- **THEN** the first plan is one fused run, the second is run, barrier, run; t305 is unchanged
- **TEST** t3565 (`src/libs/kt-fx/tests/geo_fuse.rs`)

### Requirement: Analyze sets the key from the picture in one step
Analyze SHALL compose the key's input at five moments across its owner's span, find the screen as the fullest saturated
green or blue cluster, and set the key colour, tolerance, softness, balance and despill in one undo step, saying what it
set; a row with a curve SHALL keep its curve and be named. When no screen is found, or the project changed while it
ran, nothing SHALL change and the reason SHALL be said.
Why: the owner's B3 — "кнопку автоанализа … чтобы не подбирать всё руками".

#### Scenario: A plausible key from one click
- **WHEN** the analysis runs on frames of a subject over an uneven green screen, over a blue one, and on frames with no screen
- **THEN** the key colour lies within 6 code values of the screen's mean, the matte made from the numbers is within 3 % of the known alpha; the last answers "no screen" and gives no numbers
- **TEST** t3562 (`src/libs/kt-fx/tests/key_analyze.rs`)
