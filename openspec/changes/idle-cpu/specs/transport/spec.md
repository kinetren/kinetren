## ADDED Requirements

### Requirement: A paused frame waits for what it lacks
A frame that came out incomplete SHALL be composed again when the layers it lacked are in the cache,
told by an insertion (`BurstCache::on_next_insert`), or on a pause that doubles from 250 ms and ends
at 8 s. It is NOT composed again because a UI frame happened.
Why: `need` held `!was_complete`, and `last_compose_retry` was written and never read. Each move of
the mouse over a picture that lacked something paid the whole chain on the UI thread, and a chain of
a slow effect made the interface wait for it (E2, E8).

#### Scenario: A moving pointer over an incomplete frame
- **WHEN** the frame is incomplete, nothing arrives, and the pointer moves over 40 UI frames
- **THEN** at most one composite runs
- **TEST** t1832 (`src/apps/kt-app/src/idle_tests.rs`)

#### Scenario: The next insertion is heard once
- **WHEN** a waiter is armed and two bursts are inserted
- **THEN** it is called once
- **TEST** t1836 (`src/libs/kt-cache/src/lib.rs`)
