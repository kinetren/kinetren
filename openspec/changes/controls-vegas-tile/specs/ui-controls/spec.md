# Spec Delta

## ADDED Requirements

### Requirement: Ours and VEGAS are two tiles
The Layout & Controls window SHALL offer six tiles, ours first under the program's own name with no "like …" line, then
VEGAS. Choosing ours SHALL give the keys, wheel and panels it gave before; choosing VEGAS SHALL give the plain wheel
that zooms the timeline and J, K, L on the transport. A project or a store of a person who had our tile SHALL keep it.
Why: the owner, 2026-11-07: "делаем для нас отдельную плитку, а для vegas - тоже отдельную под них".

#### Scenario: Two tiles, one unchanged
- **WHEN** the person opens Layout & Controls, sees the tiles, picks VEGAS, rolls the wheel over the timeline and presses L, then picks the program's own tile
- **THEN** six tiles stand with ours first and without "like"; under VEGAS the wheel zooms and L plays; under ours every key and the wheel are those of the frozen table
- **PATH** File › Preferences… → Layout & Controls… → six tiles show, the first says "Kinetren" → click "VEGAS Pro" → the tile is marked; close; roll the wheel over the timeline → the timeline zooms; press L → it plays; reopen → click "Kinetren" → the wheel scrolls in time as before
- **TEST** t4256 (`src/apps/kt-app/src/controls_tests.rs`)

### Requirement: The settings code carries the language, the economy UI and the update check
A code made by this build SHALL carry them; entering it SHALL apply them with the keys; entering a code of the former
version SHALL change none of the three.
Why: the owner, 2026-11-07, answer 2.

#### Scenario: Old code, new code
- **WHEN** a code is made with Russian, economy UI on and the update check off, and entered after a reset to the defaults; then a version-1 code is entered
- **THEN** the first sets all three; the second sets its keys and leaves the language and both switches
- **PATH** Layout & Controls… → Settings code → copy the code → Preferences… → set English, economy off → Layout & Controls… → paste the code → Enter → the menus are Russian and economy UI is on; paste a code of the former version → the keys change, the language stays
- **TEST** t4257 (`src/apps/kt-app/src/controls_tests.rs`)

### Requirement: The interface scale always has a key
A style that binds Ctrl+=, Ctrl+− or Ctrl+0 SHALL give the interface scale Ctrl+Alt+=, Ctrl+Alt+−, Ctrl+Alt+0; the key
list SHALL name whichever keys change the scale in the chosen style.
Why: the owner, 2026-11-07, answer 3; today Resolve's and Final Cut's styles leave the scale without a key.

#### Scenario: The scale under a style that takes Ctrl+=
- **WHEN** Resolve's style is chosen and Ctrl+Alt+= is pressed twice, then Ctrl+Alt+0
- **THEN** the interface grows two steps and returns; Ctrl+= zooms the timeline; the list shows Ctrl+Alt+= for the scale
- **PATH** Layout & Controls… → click the Resolve tile → close; press Ctrl+Alt+= twice → the interface grows two steps; press Ctrl+= → the timeline zooms; Ctrl+Alt+0 → the interface is back
- **TEST** t4258 (`src/apps/kt-app/src/controls_tests.rs`)
