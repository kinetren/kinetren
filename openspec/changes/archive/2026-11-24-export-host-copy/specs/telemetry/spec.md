## ADDED Requirements

### Requirement: The export trace states every move of a frame
An export's stage profile SHALL count apart the copy out of the device, the conversion and the upload to
a hardware encoder's surface (no longer inside the encode), and `kt-cli trace … export` SHALL print each per
frame beside the device time and the time back.
Why: found-defects 741 and export-readback D4 — what binds a card export after the latency fix is to be read
from the owner's next render, not guessed (owner, 2026-11-09).

#### Scenario: `trace export` names copy, convert and upload
- **WHEN** a run holds an export of sixty frames with copy, convert, upload and encode stage records
- **THEN** `trace … export` prints the per-frame copy out, convert and upload, and the encode without the upload
- **TEST** t3316 (`src/apps/kt-cli/src/trace_query/tests.rs`)
