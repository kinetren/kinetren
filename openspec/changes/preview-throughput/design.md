# Design

The measurements are `kt-cli preview <project> <from> <to> [--size WxH] [--cpu]`: the real `Player`
(workers, prefetch, audio render) against an output device made of the wall clock, and a 60 Hz
display loop doing what `draw_preview` does, with the same ratchet (`preview_key_step`) and the
same "shown" and "skipped" counts as the performance panel. Release build, the showcase project,
34–46 s, footage in /dev/shm (on the disk the runs differ by 215 incomplete composites), the binaries
alternating so load falls on each alike.

**This machine has no real graphics card.** Every "GPU" number is llvmpipe on 32 cores. A real card
changes the size of the wait, not its cause; the decisions below follow from where the time is
spent and who waits, not from these numbers.

## D1. The worker decodes a moment once

The export's D7 and D9 stayed in the export. In the preview, one file at two decode sizes went out
as two spans; the second stood behind the cursor, and a read behind the cursor rebuilds the decoder.
The worker now builds one span across sizes and calls `read_video_span_qualities`, which also brings
the second decoder for a file read at two places, on the same worker thread (invariant 5: no new
owner).

A chunk is counted in moments: cut between two sizes of a moment, the decoder would be asked for
the same picture twice. The span's budget counts each key at its own size; the former count used
the first key's size, so a thumbnail-sized first key could carry a span larger than the cache. Keys
past the budget now stay queued: they were removed and dropped, and only the prefetch's next pass
brought them back, which a one-off request never gets (t1522 reached 2 of 13 keys on the old code).

## Hardware decode: not needed

After D1 a picture costs about 3 ms out of the decoder plus 8 ms of RGBA conversion at about 2.7 Mpx,
one worker per file. Software decode is not the limit, so no architecture question arises.
