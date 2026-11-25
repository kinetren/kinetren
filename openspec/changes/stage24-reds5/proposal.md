# Proposal: stage24-reds5

> Found-defects 845, 846, 918, 919, 921, the reds of the fourth stage-24 finish (orchestrator's order, 2026-11-25).
> Tests t3269–t3272.

## Why

845: `fade` read the brightness of 1.5 s and got the picture of 0.2 s. The composite was made in 7 ms and drawn in
the program's next frame, 48 ms after the click; the scenario read the screen before that frame. `gen_shapes` read
the corner the same way. The mechanism: **a scenario reads the screen when the program READ the gesture, not when
it DREW what the gesture asked for.**

846: the cache query printed "lap 0: 0 restored, lap 1: 25 restored", and both began at the same
instant. The render thread writes a seek of its own right behind Play, and the query cut a lap between the two;
whether the opening's seek before it made one more depended on which of two threads wrote first. The loop's first
lap was restored whole, 25 moments, no read past its bound. The mechanism: **a query counts records of the transport
as laps, a lap is what was played.**

918: `trace::flush` gives up after two seconds and said nothing; a test read a file the writer had not written.

919: a piece claims half the bound, so the look-ahead's admission follows the frame's at once, and the
admission's record ended after the ring's lock was released: a thread put off there ended later than the next one.

921, found by 845's fix: a generator that left the project and came back with the same values had no source, and the
preview stood in Loading. The mechanism: **what is known of a source outlives the source.**

## What Changes

- The geometry dump says when the picture drawn is the one asked for; every reader of the live stand's picture waits.
- A lap of the trace's loop queries is what was played.
- `flush` answers whether the tail is written; its callers say when it is not; tests wait for it.
- An admission's record ends under the ring's lock.

## Capabilities

- `telemetry`, `distribution`, `generators`
