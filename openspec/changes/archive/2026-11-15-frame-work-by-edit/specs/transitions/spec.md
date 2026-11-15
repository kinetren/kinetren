## ADDED Requirements

### Requirement: A staircase transition's third event is asked of its lane
The check for a third event in an overlapped transition's window SHALL look only at the events of
its (kind, lane) that can meet the window (`LaneSpans`), in validation (`Indexed`) and in a batch's
reconcile, and SHALL name the same event the scan named (the first in the list's order).

#### Scenario: The check costs its window
- **WHEN** a staircase pair is checked 8 times on a lane with 16 and with 256 further events after it, and the busy event is asked of the index and of the scan
- **THEN** the events looked at grow by less than the lane, and both name the same first event
- **TEST** t2665 (`src/libs/kt-engine/tests/frame_cost.rs`)
