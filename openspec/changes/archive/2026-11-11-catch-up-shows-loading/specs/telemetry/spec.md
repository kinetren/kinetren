## ADDED Requirements

### Requirement: The Loading record names a stream catch-up
The trace's `loading` record SHALL carry, when the scrim comes, the positions a stream stage of the frame
on screen still lacks (zero when the frame waits for something else), and `kt-cli trace loading` SHALL
say for such a stretch that the stream history was catching up and by how many positions. Records of
older builds that carry the held flags SHALL still read.
Why: kkt-26.11.10b K1/K2 — Loading now stands where a held picture was; his next trace must tell a
catch-up from a missing decode without reading the stream records by hand.

#### Scenario: A Loading stretch over a catch-up says so
- **WHEN** a trace holds a `loading` record that came with 37 positions short, then one that came with none, each closed by its going
- **THEN** `trace loading` reports the first stretch as the stream history catching up 37 positions and the second without it
- **TEST** t2417 (`src/apps/kt-cli/src/trace_query/tests.rs`)
