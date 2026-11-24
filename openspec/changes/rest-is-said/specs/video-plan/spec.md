## ADDED Requirements

### Requirement: The prerender waits for a slow source and gives up on one that answered
The disk prerender SHALL ask a moment's frames once and wait for them for as long as a request of them is queued or being decoded, parked between insertions, ending at any input,
Play, an export or the close. It SHALL count a moment incomplete only when every request of it was answered and a frame is still missing, asked once more. No clock SHALL decide that a source will not answer.
Why: found-defects 839 — three seconds without a decoded frame, three times, left a moment unprepared until the next edit: on a loaded machine, a NAS or a waking disk the sources answer late, not never (rest-is-said D9).

#### Scenario: A source that answers late is prepared
- **WHEN** a heavy moment's source is held behind a gate for longer than any former limit and then released, and another moment's source answers with an error
- **THEN** the first moment is written to disk with no abandon record and the waiting made no more than a pass per insertion and four a second; the second is asked twice and left, with its record
- **TEST** t4144 (`src/libs/kt-engine/tests/render_cache.rs`)
