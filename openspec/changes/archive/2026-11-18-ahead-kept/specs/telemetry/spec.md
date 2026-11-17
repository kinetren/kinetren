## ADDED Requirements

### Requirement: A loop's report says why moments ahead were not kept and how long work waited for its ring
Every render-ahead job record SHALL say what came of its moment (`b`: 0 kept, 1 incomplete, 3 refused by the
display's memo, 4 short of its stream history). `kt-cli trace loop` SHALL print for every lap, beside the
producer's kept and composed counts, how many were short of the history, incomplete otherwise and refused,
and the sum of the waits of the ring admissions in the lap with their count.
Why: kkt-26.11.18 T2 — "6 kept of 68 composed" did not say why; the dump had to be read record by record to
find that every one was a stream stage short of its history, and that the device stood idle while each
history piece waited for an empty ring.

#### Scenario: A lap says why moments ahead were not kept
- **WHEN** a lap holds five render-ahead jobs (one kept, two short of the history, one incomplete, one refused) and two admissions that waited 0.3 and 0.4 s
- **THEN** `trace loop` prints "1 kept of 5 composed" with "2 short of the history, 1 incomplete, 1 refused by the memo" and "ring waits 0.7 s over 2"
- **TEST** t2932 (`src/apps/kt-cli/src/trace_query/tests.rs`)
