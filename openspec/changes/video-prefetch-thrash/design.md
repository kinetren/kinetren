# Design: video-prefetch-thrash

## Context

The player's prefetch thread runs a pass about every 100 ms: the look-ahead window from the
playhead (2 s, folded into the loop region) → `wanted_frames` → every key not in the cache is
submitted as a NearPrefetch request. The cache is one LRU over a 512 MB video budget. Item 19b
had already met this disease on single 4K frames and capped the window at "half the budget
divided by one frame"; R.46 (item 318) made that frame the largest DECODE quality. Neither
counted layers.

## Measurements (showcase project, release, no decoding)

`kt-cli probe target/thrash/showcase.kt 0 104`, the new "video prefetch window" section:

| preview | before: worst pass | dropped unread | video hit | after: worst pass | dropped | hit |
|---|---|---|---|---|---|---|
| 640×360 (the player's start) | 140 frames = 604 MB | 22 025 | 92 % | 58 frames = 255 MB | 0 | 100 % |
| 960×540 | 72 frames = 662 MB | 22 960 | 83 % | 56 frames = 255 MB | 0 | 100 % |
| 1920×1080 | 21 frames = 343 MB | 100 | 98 % | 24 frames = 255 MB | 0 | 100 % |

Each half of the fix alone: the byte-sized window alone already gives 0 dropped and 100 % in the
simulation; the refresh alone (the old 2 s window) cuts 640×360 from 22 025 to 3 175 but cannot
help 960×540 (79 662) or 1920×1080 (158 578): a window larger than the cache evicts itself
whatever the order.

## Decisions

### D1. The window is measured in bytes of what it asks for, at whole frames

`video_prefetch_wants` walks the window frame by frame, nearest first, takes `layers_at` of each
tick (a key already taken is not counted again — a 24 fps file on a 25 fps grid repeats frames,
a blended pair shares a side), and stops before the frame that would cross half the budget. A
partial frame is never asked: it would leave that frame incomplete anyway. The playhead's own
frame is asked whatever its size — a window cut to nothing would show nothing (t1416b).

Rejected: multiplying the old frame count by `max_concurrent_layers`. It is a bound over the
whole project, so one dense stretch would shorten the window everywhere, and it still assumes
one decode size per layer. The walk costs the same `layers_at` calls `wanted_frames` made.

Why half and not all: until a pass refreshes them, the frames just shown, the CPU composite
memo (`PostFxVideo`, which lives in the same budget) and the Playback requests of the frame on
screen need room too. Half is item 19b's number, kept.

### D2. The pass refreshes the window, from the furthest frame to the nearest

LRU ranks a prefetched frame by its insertion. The nearest frame of the window is the one
decoded first, so it was always the oldest unread entry, older than every frame playback had
just read: under any pressure the cache dropped exactly the frames needed next.
`BurstCache::refresh(keys)` marks present keys recent in the given order without counting a
read (`had_hit` stays false, no hit, no epoch). `video_prefetch_pass` refreshes the window in
reverse, so the nearest frame ends up the most recent; what was shown, and what a seek left
behind, goes first; among the window the furthest frame goes first.

Item 19 forbade a `contains` probe from touching the LRU: probes in ascending order made the
nearest frame the oldest. `refresh` is a separate, explicit call with the order that matters
written at the one call site; `contains` stays a pure probe (t27 unchanged).

Rejected: an eviction rule "read entries before unread ones" inside the cache. The cache does
not know the playhead: stale unread entries of an old seek would outlive everything read, and
the loop region's head (read every lap) would be evicted before them.

### D3. Passed look-ahead is withdrawn from the queue

A NearPrefetch video request stayed queued until a seek bumped the generation. When decoding
4K layers is slower than the picture, the worker reaches such a key after the playhead has
passed it, decodes it and inserts an entry nobody reads — the same "dropped before the first
read", and decode time taken from the frames ahead. Video NearPrefetch requests come from the
prefetch loop alone, so the loop owns them: each pass withdraws the ones its window no longer
asks for (`MediaWorker::withdraw_video`). Playback requests (the frame on screen) are not
touched; a span already taken by the decoder is not interrupted.

### D4b. The preview's Playback request is the frame on screen

The first re-shoot of the diagnostics image on the fixed build still showed 1–4 drops a
second and "video hit 75 % — decoding cannot keep up" (no-read drops 59 in all, against 543
before; V-fps 25.7 against 13.9). The rest came from the queue's other end: the preview asks
for the frame it shows at Playback priority, and Playback outranks the look-ahead whatever the
direction. Under a decoder slower than the picture (4K layers on llvmpipe in Xvfb), every frame
shown queued one more request; the old ones, behind the decoder's cursor, were still served
before the whole look-ahead window — entries nobody would read, while the window starved and
the picture asked for more. `compose_preview_measured` now withdraws the Playback video requests
that are not a layer of the frame it is composing (t1418b: ten frames paged on a slow file
decode two, not ten). The diagnostic `request_video` re-submits in its loop, so a composite
between two of its calls costs it one retry.

### D4c. The window starts at the frame on screen

The display shows the playhead minus the smoothed output latency (`video_display_frame`). A
window from the audio playhead left the frames between them out, and with D3 it would also
withdraw their requests — on a device with 40–100 ms of latency, the very frames about to be
shown. The prefetch loop now starts the forward window at `video_display_frame(head,
latency_smooth, region, 1, rate)`; the reverse window stays as it was (forward from the head).

### D4. One wiring for the player, the probe and the tests

`video_prefetch_pass` (refresh + missing keys) is what the player submits, and
`simulate_video_playback` drives the same pass with an infinitely fast decoder and reads through
`frame_plan`. `kt-cli probe` and t1416 call it, so the diagnostic cannot drift from the engine
(the rule of R.43: the diagnostics repeat the engine's wiring). The simulation has no timing: it
answers "does the window fit", not "does the decoder keep up" — the panel's live numbers answer
that.

## Risks

- A very dense stretch (many 4K layers) gets a short window (down to the playhead's frame).
  That is honest: a longer one would evict itself. The cure there is a larger cache or a lower
  preview size, and the probe shows which.
