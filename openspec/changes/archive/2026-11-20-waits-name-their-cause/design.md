# Design: waits-name-their-cause

## D1. Why no window (row 824)

The X server can be asked which processes are its clients (XRes `QueryClientIds` with
`LocalClientPIDMask`); the supervisor's editor child is `/proc/<pid>/task/*/children`. With these a
wait tells apart:

| What it sees | What it says | Ends the wait |
|---|---|---|
| the process ended | "the program ended (code N)" + the first log line | at once |
| the editor child ended, the supervisor stands | "the editor process ended … (the crash window?)" | at once — the crash window is not the program's |
| neither pid is a client of the display | "not connected to its display" | no (it may still connect) |
| a client, no window | "connected …, still starting" | no |

The first log line is read after the offset the stand's appended log had at the start, so an earlier
run's lines are not taken for this one's.

## D2. The class in kt-app tests (rows 760, 781–783)

The mechanism: a test asserts a state the code reaches after a DESIGNED last step, and starts its
measurement at a fixed time or at a flag the code sets before that step. Free, the step is quick and
the test passes; under load the step lands inside the measurement. The rule written into the
`distribution` spec: wait for the consequence (the step's own observable — a counter that stops
moving, the code's own record, the state asserted), bounded by `kt_time::test_deadline()`, then assert.

Walked for the same mechanism: every `parked(` in `audio_out.rs` tests (t2380 had both forms; the
others wait for a flag that is the consequence — `opens == round`, `!open` before a seek, `flush`
drained), every `strong_count` assertion after a drop in kt-app (t1644 only), every fixed
`thread::sleep` before an `is_playing` assertion in `layout_tests.rs` (t1621's two).
