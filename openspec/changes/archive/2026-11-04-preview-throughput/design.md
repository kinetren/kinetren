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

## D4. The CPU path composes ahead of the display

One thread cannot compose a frame in less than the frame's own cost: p95 381 ms at 1080p, on the UI
thread, 8–9 V-fps. `compose_ahead.rs` runs workers (half the cores, at most 16, bounded by the memory
of their scratches) over a shared renderer with `compose_export_with`, the export's own call: a
frame is a function of (cache, t, quality), so which worker made it does not show in its bytes
(t1530). The UI takes a finished frame by a swap.

- Workers read the cache and nothing else, and a frame whose layers are not all there is left for
  later, so they never decode and never compose incomplete (invariant 5).
- The key is (project version, content version, quality): a frame of another state of the project
  cannot reach the screen (t1532).
- They sleep on a condvar while the transport stands (invariant 12, t1531), and start on the first
  frame played on the CPU path, so a GPU session has no such threads.
- A stream stage keeps the synchronous path, as in the export (t1533).

Three things decided whether this worked, each found by measuring:

1. **A miss holds the picture, within a limit.** The first version composed a missed frame on the UI
   thread as well: a 500 ms stall that also threw the display past frames on their way, 30 % of them
   wasted. While the workers serve, a miss returns incomplete and the UI asks again next frame, the
   path a decode miss already takes. The hold is a promise, and the first version of it was not
   bounded: the workers serve forward play alone, so in reverse, at rate 0 or on a clock that stands
   nobody would bring the frame and the picture froze with the transport running. t947 caught it
   only as noise (2 failures of 4 runs of the suite). `may_hold` now answers yes for a frame in a
   worker's hands, or for three frames on one tick at a forward rate; past that the caller composes
   (t1536, red on an endless hold).
2. **Workers start past what cannot arrive in time.** A composite begun for a frame nearer than the
   composite's own smoothed cost ends after the display has left.
3. **The decode window starts at the workers' frontier.** Measured from the display, its 0.64 s left
   0.24 s past a 0.4 s composite, and the workers stood waiting for frames at 16 fps (`lead_ticks`,
   `shifted_head`).

## D5. The display orders no decode while the workers serve

With the window moved, 26–57 s came out with 38,026 frames of audio underrun where the old path had
none, all at 50–53 s: a 4K file with chromatic aberration and glow, its audio on the same worker.
Moving audio out from behind long spans did not cure it (34,698 and 5,573 after that change). The
evidence was the decoder: 3,165 pictures and 27 reopens against the GPU path's 2,230 and 17 on the
same range. On a miss the UI still submitted a Playback request for the frame on screen, which now
stands BEHIND the window, so the decoder seeked back through a 4K GOP for a frame about to be
skipped, on the worker that also reads that file's audio. The hold now comes before the requests:
0 underruns in three runs, 2,540 pictures, 20 reopens (t1534).

The first t1534 stayed green with the bug put back: the frame on screen was still in the cache, so
no request went out. It was rewritten without timing: the clock stands, and the frame asked lies past
the look-ahead, so only a Playback request can decode it.

Separately, a video span no longer holds audio of its own priority: the burst is read between
chunks and the span goes on, without moving the video cursor (t1529). This was not the cure for the
underruns above; it stands on its own, as a span of 64 keys could otherwise hold a burst for seconds.

## The hit rate counted a memo as a frame

A lookup of an empty post-FX memo counted as a video miss, though no frame was missing: on the CPU
path the rate read 41 % before this change and 57 % with the workers, with every decoded frame
there, and the panel's warning fires below 80 %. Only a frame now counts as a miss; a memo hit
still counts as a hit, which t-postfx and the mask tests read (t1535).

## The software adapter stays on the GPU path

The export refuses a software rasterizer in favour of CPU workers. The preview does not follow it:
in the real program at 1080p llvmpipe gives 19–20 V-fps and the CPU workers 18, both with no underrun.
The numbers do not justify a second rule, and `gpufx` keeps running the path real cards use.

## What a real window costs on a software rasterizer

Measured in the real release program under Xvfb, 1080p, llvmpipe: `queue.submit` takes about 30 ms
on both Vulkan and GL, against 0.5 ms headless. The first explanation, that the queue backs up behind
the previous frame, was tested and is WRONG: a headless run that never waits for the device stays
at 0.56 ms. What blocks is inside the software driver with a window attached; `perf` is not
available here, so the cause is not known and is not claimed. On this adapter the frame's cost is the
rasterizer's own work, whoever waits for it. A real card was not measured: none is available.

## Measured, before and after

Release builds, the showcase project (1080p, 25 fps, 104 s, 101 events), footage in /dev/shm, the
binaries alternating. **This machine has no real graphics card: every "GPU" row is llvmpipe on 32
cores, and no number here says what a real card does.** The machine carried other sessions' load
(5–20) throughout.

`kt-cli preview`, one frame's stages on the UI thread, ms per composite (34–46 s, 1080p):

| stage | before | D1 | D1–D3 |
|---|---|---|---|
| plan | 0.03 | 0.03 | 0.06 |
| upload | 2.0 | 3.5 | 3.5 |
| encode + submit | 0.2 | 0.25 | 0.3 |
| wait for the device | 11.5–13.2 | 18–23 | 0 |
| copy out | 3.5–4.1 | 4.5–5.0 | 0 |
| UI stall, p95 per UI frame | 43–57 | 40–55 | 7–11 |

At 640×360 the same frame was: plan 0.03, upload 1.4, encode 0.3, wait 5.5, copy 0.3 (the before
column, with 72 incomplete composites). "Before" composes fewer complete frames, so its per-composite
stages look cheaper; that is starvation, not speed. The device's work did not go away with D2: a
headless run waits for it at the end of the frame (15–19 ms), and in a window the swapchain does.

`kt-cli preview`, the whole edit, 0–104 s:

| path | V-fps | V-skipped | incomplete | underrun, frames | pictures | reopens | UI stall p95, ms |
|---|---|---|---|---|---|---|---|
| GPU 1080p, before | 11.3 | 374 | 2003 | 193,987 | 42,654 | 661 | 49.3 |
| GPU 1080p, after | 21.0 | 356 | 104 | 0 | 10,934 | 113 | 6.2 |
| CPU 1080p, before | 5.2 | 1588 | 766 | 160,426 | 53,479 | 842 | 227.5 |
| CPU 1080p, after | 15.0 | 0 | 2503 | 0 | 11,961 | 115 | 0.1 |
| GPU 640×360, before | 17.4 | 298 | 1119 | 109,205 | 55,296 | 852 | 20.1 |
| GPU 640×360, after | 22.0 | 282 | 70 | 0 | 10,814 | 113 | 2.7 |

On the CPU path "after", V-skipped and incomplete do not mean what they mean above: a held frame
counts as an incomplete composite per UI frame, and the ratchet moves past it. Read V-fps.

The real program, release, Xvfb, 1080p, the loop region 26–57 s, the panel's counters, 18 s each,
three rounds. A round counted only if the GPU "after" run reached 20 UI frames per second, a rule
set before the data; two earlier rounds with the machine swamped (under 6 UI fps) were discarded.

| path | V-fps | underrun, frames | UI frames/s |
|---|---|---|---|
| GPU, before | 8.7–10.3 | 75,983–82,767 | 21.7–26.8 |
| GPU, after | 17.0–19.7 | 0 | 24.3–29.7 |
| CPU, before | 4.4–4.7 | 63,695–79,695 | 14.4–15.7 |
| CPU, after | 17.6–18.4 | 0 | 48.4–52.4 |

"Before" here is the final tree with the old `worker.rs` and `prefetch.rs`, compose-ahead off and
`KT_PREVIEW_READBACK=1`: a temporary build, the base branch has neither the counters nor the size
variable. 25 V-fps is NOT reached in a window on this rasterizer: the composite and the UI's own
painting share the same CPU rasterizer (see the next section). Headless the GPU path reaches 24.7
on 34–46 s.

## Hardware decode: not needed

After D1 a picture costs about 2 ms out of the decoder plus 8–9 ms of RGBA conversion at about 2.7
Mpx, one worker per file, and every run's decoders kept up on 4K VP9 footage. The waste was the
decoding of pictures nobody asked for, not the speed of decoding. No architecture question arises.
The RGBA conversion is the larger half, on one thread per file: at 4K native size it is the next
limit to meet, but nothing measured here reaches it.
