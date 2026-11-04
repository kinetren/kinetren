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

## D2. The GPU preview's picture stays on the device

Every shown frame was composed on the device, waited for, copied to the CPU, premultiplied pixel by
pixel into a new allocation, and uploaded to the device again, all on the UI thread. `present` runs
the same resolve and the same passes as `execute_into` (invariant 7: one plan, one set of kernels),
then one display pass into an `Rgba8Unorm` texture that egui draws. `display.wgsl` reproduces
`Color32::from_rgba_unmultiplied` exactly: `round(value · alpha / 255)` in gamma space, so the
picture does not change (t1525, red without the premultiplication). An incomplete frame is not drawn
(t1526), which keeps "hold the last full frame". The ambient glow is the same pass at 32×18 with
the CPU loop's picks (t1527).

Readback stays where someone needs the bytes: export, tests, no engine, a refused plan.

`read_display` and `read_glow` exist for the tests. `kt-cli preview` calls `finish_present` once per
UI frame because a headless run has no swapchain to hold it back; the time is reported apart as
"device at the frame's end".

## D3. The first frame of a cut is asked early

After D1 and D2 all the remaining incomplete frames of 34–46 s stood at one cut (41.12–41.96):
52 composites waiting for a 4K file's first frame. The window is limited by bytes, about 0.7 s at
this size, and that decoder needed about 0.85 s to reach the frame, so the cut was late however fast
the rest was. A first frame costs one frame of cache and buys the whole lead-in. Past the bytes, up
to the same 2 s, `video_prefetch_wants` adds the layers that start a read (`jumps_to`), at most
`CUT_LEAD_KEYS`. The frames after them come in the window's turn (t1528).

A speed changes the stride of a read, so "the same read" is the same media and size within four
frames; a faster stride looks like a jump, and the key limit is what bounds it.

## What a real window costs on a software rasterizer

Measured in the real release program under Xvfb, 1080p, llvmpipe: `queue.submit` takes about 30 ms
on both Vulkan and GL, against 0.5 ms headless. The first explanation, that the queue backs up behind
the previous frame, was tested and is WRONG: a headless run that never waits for the device stays
at 0.56 ms. What blocks is inside the software driver with a window attached; `perf` is not
available here, so the cause is not known and is not claimed. On this adapter the frame's cost is the
rasterizer's own work, whoever waits for it. A real card was not measured: none is available.

## Hardware decode: not needed

After D1 a picture costs about 3 ms out of the decoder plus 8 ms of RGBA conversion at about 2.7 Mpx,
one worker per file. Software decode is not the limit, so no architecture question arises.
