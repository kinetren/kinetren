## ADDED Requirements

### Requirement: An effect's preset replaces the effect with its curves
Applying an effect's preset to an existing slot SHALL replace every value and every curve of the slot with the
preset's (none for a variant, the preset's own re-hung on the event's entry point for a user preset), keeping the
slot's position, bypass, masks and identity, in one undo step that restores the former values and curves. A chain
preset's curves SHALL come with its inserted nodes.
Why: the owner, 2026-11-20 (A4) — "если это пресет конкретного эффекта - то заменяется".

#### Scenario: A curved row is replaced and Undo brings it back
- **WHEN** a Blur slot with a curve on its radius is switched to the variant "Soft", then to a user preset with its own radius curve, then Undo twice
- **THEN** after "Soft" the slot has no curves and Soft's values; after the user preset it carries that preset's curve re-hung on the entry point; two Undos restore the original curve and values
- **TEST** t3596 (`src/apps/kt-app/src/layout_tests.rs`)
