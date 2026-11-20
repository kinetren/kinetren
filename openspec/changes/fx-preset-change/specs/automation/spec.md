## ADDED Requirements

### Requirement: A preset applied to a slot keeps its curves
Applying a preset to an existing slot SHALL set only the rows that carry no curve; a row with a curve SHALL
keep it, and the menu SHALL say how many rows kept their curves.
Why: rule 20 — animation work is not lost by a menu pick.

#### Scenario: A curved row survives a preset change
- **WHEN** a Blur slot with a curve on its radius is switched to another preset
- **THEN** the radius curve is unchanged, the other rows take the preset's values, the menu notes "1 row keeps its curve"
- **TEST** t3596 (`src/apps/kt-app/src/layout_tests.rs`)
