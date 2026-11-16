## ADDED Requirements

### Requirement: An export writes where its time went
Every export, serial or pipelined, SHALL keep its stage profile whether or not a caller asked for one, and
when it ends SHALL write it into the trace — an `ExportStage` record per stage (busy nanoseconds summed over
the stage's threads, the wall, the frames written, the frame workers) and one for the wall with the timeline
it covered — and SHALL return it as one line (`ExportStats::stages`) that the render window writes into the
telemetry journal. The single-device export SHALL report its upload, encode, device wait and copy-out as the
per-worker devices do.
Why: kkt-26.11.15 N3 — "the final render is slower than the preview in every version"; none of the owner's
traces held an export, and the profile existed only under `kt-cli export --time` on our machine.

#### Scenario: Serial and pipelined exports leave their stages
- **WHEN** a two-second project is exported serially and pipelined, with no profile asked
- **THEN** the trace holds a wall record with 60 frames and the two seconds covered (frame workers 0 only for the serial one), decode and encode records with busy time, the same wall and 60 frames, and the returned line names the frames, the size, the real-time factor and the busy stages
- **TEST** t2792 (`src/libs/kt-engine/tests/export_options.rs`)
