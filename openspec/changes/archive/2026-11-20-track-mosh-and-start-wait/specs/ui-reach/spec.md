## ADDED Requirements

### Requirement: Datamosh is refused on a track with its reason
The track window SHALL refuse a stream device (Datamosh) put on a video track's chain — from the
picker, a paste or a preset — leaving the chain as it was and saying in the status line, as an error,
that Datamosh breaks an event's own stream and a track has none. A stream slot an older project holds
on a track SHALL stay.
Why: the engine skips a stream slot on a track stage (`video-fx`); offering it was a dead control
(kt-app rule 9) and the device had no sound there (found-defects 820). Making it work on a track needs a
track stream stage (design D1).

#### Scenario: Datamosh picked in a track's window
- **PATH** right click Video 1's header → Track properties → open the graph's picker → type "mosh" → click Datamosh → the status line says Datamosh breaks an event's own stream and a track has none, the graph holds no Datamosh node → pick Blur → it stands on the track
- **WHEN** the person picks Datamosh, then Blur, in a video track's window
- **THEN** the track's chain holds no Datamosh and the refusal names its reason; Blur lands
- **TEST** t3237 (`src/apps/kt-app/src/node_editor_tests.rs`)

### Requirement: The live rig waits for the program's window up to the test's guard
Every lookup of the program's X window in the live rig SHALL wait for it as a consequence — asking
again while the program lives, failing at once if it ended — up to the test's guard (`TEST_DEADLINE`,
the ten minutes of `kt_time::test_deadline()`), never by a fixed number of seconds; the start of a
scenario's own process and of a worker's stand SHALL wait by the same guard.
Why: at a load average of 28 two scenarios went red with "the kt-app window did not appear" and passed
alone (found-defects 822); a clock limit is a claim about the machine's load.

#### Scenario: No bare window lookup
- **WHEN** the live rig's scripts are read
- **THEN** every `probe.window()` call is inside `kt_live.the_window` or a `try` that catches its exit, and the start waits of `kt_live` and the stand are bounded by `TEST_DEADLINE`
- **TEST** t3238 (`src/apps/kt-cli/tests/dist_guard.rs`)
