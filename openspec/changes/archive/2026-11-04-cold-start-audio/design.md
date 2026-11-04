# Design

## Measured

Real release program, Xvfb, `KT_AUTOPLAY`, 640×360, the showcase, counted from the first moment, with
and without the sidecar index:

| code | starts | underrun, frames | callback underruns |
|---|---|---|---|
| before `preview-throughput` | 4 | 98,511–126,903 | 1 |
| after it | 4 | 0, 0, 0, 32,803 | 1 |
| this change, footage in /dev/shm | 12 | 0 | 0 |
| this change, footage on disk | 6 | 0 | 0 |

The disk runs are not a cold disk: the OS page cache cannot be dropped here and the footage had been
read before. A start from a really cold spinning disk was not measured; past 500 ms its audio would
start with the former silence, not worse.

t1537 without the wait shows `(tick 0, 16384), (tick 0, 16384)`: the diagnosis, in the test's own
failure message.

## D1. Waiting is free before the first frame

Invariant 6 lets time stand before the first frame, and the output kernel already does. A wait there
moves nothing out of sync: the picture follows the same clock. The cost is a later start, paid only
when the audio is not there, and a person prefers a start 0.3 s late to one that begins with 0.3 s of
silence.

The first version of the wait covered exactly the ring, and left 768 frames at 43 ms: the first
refill reads just past it, and those bursts had not been asked for. The window is the ring plus the
running path's look-ahead.

## D2. The wait has a limit

A source that does not answer must not stop playback: `START_AUDIO_GRACE` is 500 ms, and past it the
render goes on with what there is (t1538). A window whose every burst is past the end of a file does
not wait for a decoder that answered: see the risk below.

## D3. An empty pass before the first frame is no underrun

`callback_underruns` counted each empty pass of the callback, so a wait would have raised it by one a
callback. `OutputEdge::started` closes that, and the counter reads 0 where it read 1 before.

## Risk

`prefetch_audio_pass` counts a burst the worker has marked as past the end of its file as missing,
so a start in such a place waits out the 500 ms. That is a late start, never silence, and only at a
position with no material under an event.
