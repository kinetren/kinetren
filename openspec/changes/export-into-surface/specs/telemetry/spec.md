## ADDED Requirements

### Requirement: The export says which way its frames went to the encoder
An export's stage records SHALL count the frames handed to the encoder on the card and those uploaded, with the reason of
the latter; `kt-cli trace … export` and the render window's log SHALL print them.
Why: the gain and the refusals are known only on the owner's machine (C11; owner, 2026-11-09: telemetry explains).

#### Scenario: `trace export` names the way
- **WHEN** a run holds an export of sixty frames, fifty by surface and ten by upload with a reason
- **THEN** `trace … export` prints both counts and the reason, and no copy out, conversion or upload for the fifty
- **TEST** t4134 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: The log's line in a live render
- **WHEN** live `hwencode` renders on the fake driver
- **THEN** the render window's log holds the line with the frames' counts
- **PATH** File › Render… → the encoder h264_vaapi, Render → the progress window runs, the finished block comes; its log's last lines say "handed to the encoder on the card: N of N"
- **TEST** t4136 (`src/apps/kt-app/src/render_dialog.rs`), live `hwencode`
