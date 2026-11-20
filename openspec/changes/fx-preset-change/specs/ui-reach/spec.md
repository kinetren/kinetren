## ADDED Requirements

### Requirement: An added effect's preset is changed in place
Every effect slot's form SHALL begin with a Preset menu listing the kind's variants exactly as the picker
lists them and the user's single-slot presets of that kind; choosing one SHALL set the slot's values to what
the picker would have given, in one undo step, keeping the slot's position, bypass, masks and identity; the
menu SHALL show the preset the values currently equal, or "(custom)" after any edit.
Why: the owner, 2026-11-07: "невозможно после добавления эффекта - ПОМЕНЯТЬ ЕГО ПРЕСЕТ НЕ ПЕРЕСОЗДАВАЯ ЕГО".

#### Scenario: From Heavy to Soft without re-creating
- **WHEN** a Blur added as "Heavy" in the middle of a three-slot chain with a mask is switched to "Soft" from its Preset menu, a value is then nudged, then Undo twice
- **THEN** the slot stays second with its mask and bypass, its values equal the picker's "Soft"; the menu reads "Soft", then "(custom)" after the nudge; two Undos return to "Heavy"; an audio reverb switched the same way keeps ringing (its state object is the same)
- **PATH** double-click the event → effects window → select the Blur slot → the form head shows "Preset: Heavy" → open it → "Soft" → the picture softens, the slot stays in place → drag the radius a little → the head reads "(custom)" → Ctrl+Z twice → "Heavy"
- **TEST** t3595 (`src/apps/kt-app/src/reach_tests.rs`)
