## ADDED Requirements

### Requirement: A player's drop is bounded and its workers stop at their next check
`Player::drop` SHALL set the stop, close the render-ahead producer's lane and join its build, render and
prefetch threads and its media workers each with a bound (`kt_time::shutdown::WAIT`), leaving a thread
past it behind. A media worker SHALL stop its decoder between two chunks of a span once it is closed; a
history walk SHALL stop at its next position: the prefetch thread's stop check includes the player's
stop, and the render-ahead producer walks under its lane's `cancel_check` (`trace::stop::CLOSED`).
Why: close-while-busy (kkt-26.11.18 T1): the joins had no bound, and a decoder ran a whole span of up to
64 pictures after its worker closed.

#### Scenario: A closing decoder stops between chunks
- **WHEN** a worker decoding a span of 32 slow pictures is shut down
- **THEN** the shutdown returns within its bound, the decoder ended (it stopped at the next chunk), fewer than 32 pictures were decoded, and the wait is in the trace
- **TEST** t2952 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A job sees its lane close
- **WHEN** a running job's lane is closed
- **THEN** the job's `cancel_check` turns true
- **TEST** t2953 (`src/libs/kt-time/src/sched/tests.rs`)
