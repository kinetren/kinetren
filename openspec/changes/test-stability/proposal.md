# Proposal: test-stability

## Why

The owner's stage "build and CI" (`docs/user-feedback.md`, 2026-11-03: "все что больше 10мин+ это
ужас"). Every full run under load gave random reds, so the orchestrator rechecked every merge by hand,
and a real defect is easy to take for noise (`docs/found-defects.md`, rows 24–33, 47, 57–60, 79, 82).
The live scenarios are most of `finish.sh` (row 70): `readme_shots` alone took 593 s.

Reading the reds found three causes, none of them random:

1. **A wall-clock bound that was not the check.** A test waited for something that had to happen, and
   gave up after a number chosen on a quiet machine. On the working disks one fsync took 44 s while a
   build ran, so "the download did not end" (20 s), "probe not published" (45 s), "loudness not
   computed" (60 s) and "writing the restore record took 44 s" (t815, 2 s) fired on correct code.
2. **A race in the test's own scene.** Fixture generation shared one temp name between runs; a fired
   deferred click was toggled back off (t1282); an assertion read before the sender thread had spoken
   (t836); a lock file's descriptor copied into another test's child (t362).
3. **A real-time measurement read as an absolute number** (t1524): a scene that has to keep up with the
   wall clock can only be judged against what the machine gave at that moment.

## What Changes

- `kt_time::test_deadline()` is the one bound of a test's wait, ten minutes: it catches a hang, not a
  slow machine. A limit that IS the check stays, and a guard (t1671) names each such limit and refuses
  any other.
- `fixtures/gen.sh` is safe when runs meet (t1670).
- t815 reads the thread's processor clock: the program's own work, not the disk. t1524 counts against a
  control of one pass playing beside it. t1538 proves "a dead source stops no start" with a source
  that never answers, not with a clock.
- The live framework's waits follow the program's frame time, and the scenarios that bounded a gesture
  by the clock wait for its consequence.
- `readme_shots` and the live run take less time (row 70): measurements in design.md.
- No test is weakened, removed or retried: each keeps its observable consequence. The owner's existing
  "red is rerun alone" of `live-check.sh` stays and still prints what passed on the second try.

## Capabilities

### Modified Capabilities

- `distribution`: how a test waits, and what the full run promises under load.
- `durability`: the t815 scenario names the program's own work, not the wall clock.
- `media-index`: the t1524 scenario counts against its control.
