## REMOVED Requirements

### Requirement: A saturating display leaves lower classes only its slack
**Reason**: it starved the look-ahead, its stream walk and the loop fill on the owner's iGPU
(kkt-26.11.16): the device idle 25 s with nothing queued while near work waited 486 % of its wall.
**Migration**: replaced by "A saturating display keeps near work at the bound".

## ADDED Requirements

### Requirement: A saturating display keeps near work at the bound
While the display presents, what a lower class may leave in front of the display's next present SHALL be
the bound for `near` (the look-ahead, its stream walk, the loop fill), at least a quarter of the bound for
`visible` and `background` (more where the display's own present leaves more of the period free), and for
an export (`offline`) only the part of the period the display's present leaves free. No piece SHALL be
longer than the bound, and at most `AHEAD_PIECES` lower pieces stand uncompleted in front of the display.
A display that stopped presenting leaves the bound.
Why: kkt-26.11.16 — under the slack rule (gpu-leftovers D4, found-defects 491) the near class kept a quarter
bound on a saturating display; on his iGPU the device stood idle 25 s with nothing queued while near work
waited 486 % of its wall, and a loop filled in 25 laps against kt26.11.10's 11. kt26.11.10 put whole jobs of
380 ms in front of the display; this rule keeps at most three half-bound pieces there.

#### Scenario: Room by the display's cost
- **WHEN** the display presents frames costing a quarter period, seven eighths of it, twice it, and then stops
- **THEN** a background piece has the bound, a quarter bound, a quarter bound and the bound again; the near class keeps the bound in front of the saturating display; an export has the bound, an eighth of the period, nothing and the bound again
- **TEST** t2517 (`src/libs/kt-gpu/src/arbiter.rs`)
