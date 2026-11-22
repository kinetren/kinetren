## ADDED Requirements

### Requirement: The persistent cache is switched beside the quality and shown in the Diagnostics
A switch beside the preview quality SHALL read «Постоянный кэш» / "Persistent cache" with "As in the preview" (the
default), "1/2", "1/4" and "Off"; "Off" SHALL stop reading and writing the store; a bin card SHALL show its source copy's
state; the Diagnostics window SHALL list the store's three kinds with bytes, reads and writes; no text of the interface
SHALL say "proxy" in any language.
Why: the owner, A10 and B1.

#### Scenario: Heavy work survives an edit, and the store says so
- **WHEN** a heavy event's range is prerendered, a title above it is moved, and the Diagnostics is opened
- **THEN** the range stays bright on the coverage bar after the edit; the Diagnostics lists "effect results", "frames", "source copies" with their sizes and the reads that the edit used
- **PATH** play the heavy range, pause 2 s → the coverage bar turns bright → drag the title event 1 s right → the bar stays bright over the heavy event → open Diagnostics → the Persistent cache section lists "effect results 420 MB, read 48 just now"
- **TEST** t3447 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `persistent_cache` (t3448)

#### Scenario: No "proxy" anywhere in the interface
- **WHEN** the five catalogs are scanned
- **THEN** no entry contains "proxy" or "прокси" (or the word in the other three languages' catalogs)
- **PATH** switch the language to Russian → open the switch beside the quality and a clip card's menu → every item reads «Постоянный кэш», none "прокси"
- **TEST** t3450 (`src/apps/kt-app/src/i18n_tests.rs`)
