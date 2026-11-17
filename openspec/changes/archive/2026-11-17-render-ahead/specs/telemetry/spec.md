## ADDED Requirements

### Requirement: The trace says how busy the device was while frames were missing
Every preview, offline or export frame whose pass timestamps come back SHALL write a `GpuFrame` trace
record (`a` its device nanoseconds — the sum of its passes, `b` its span from the first pass's start to the
last pass's end, `aux` its role) when the timer harvests it. `kt-cli trace loop` SHALL print for every lap
the device's busy share (the device nanoseconds of the frames harvested in the lap over the lap's wall),
and, over the laps that had a moment not kept, the share "while frames were missing"; `kt-cli trace gpu`
SHALL print the run's frames and their device time.
Why: `docs/architecture-review-26.11.17.md` §2 — a performance change was accepted when the mechanism found
in the owner's trace was gone, never by the property he notices: on his iGPU the card was 4–26 % busy while
frames were missing. render-ahead is accepted by that share.

#### Scenario: A lap reads its device share
- **WHEN** a trace holds two laps of a loop, the first with moments not kept and frames of known device time harvested in each
- **THEN** `trace loop` prints each lap's device share as the harvested device time over the lap's wall, and the share while frames were missing over the first lap alone
- **TEST** t2926 (`src/apps/kt-cli/src/trace_query.rs`)
