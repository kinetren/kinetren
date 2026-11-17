## MODIFIED Requirements

### Requirement: A stop or a seek handed to a parked thread is never lost
Every state a parked thread waits on SHALL be changed, and its waiter notified, under the lock the waiter looks at it with, or the waiter SHALL take the notify count before it looks and wait for it to move (`Wake`). A lock-free park handshake (the trace writer) SHALL fence both sides `SeqCst`. A new thread that parks on another's news follows one of the two forms; the list of hand-offs checked is in play-start-and-prefetch's design (D2).
Why: found-defects 525 — t2335's hang was the test's own race (fixed by ec6be337), but the walk of every hand-off found the compose thread's stop notified without its lock (the thread, not joined, parked for good holding the player's preview parts) and the trace writer's handshake without the producer's fence.

#### Scenario: A stop between the look and the wait
- **WHEN** the compose thread has looked at the stop flag and not yet waited, and the player stops it
- **THEN** its wait ends; with the stop unlocked it waited for good
- **TEST** t2640 (`src/libs/kt-engine/src/compose_queue.rs`)

#### Scenario: A paused seek owes one prefetch pass, under load
- **WHEN** t2335 counts passes from before each seek, beside 48 and 96 busy loops on 32 threads
- **THEN** every seek gets its pass (42 of 42 runs); counted after the seek, 8 of 12 runs waited for a pass nothing would ask
- **TEST** t2335 (`src/libs/kt-engine/tests/render_ahead.rs`)
