# Proposal: video-prefetch-thrash

## Why

Shooting the README images on the free footage (change `readme-shots-showcase`, release build),
the Diagnostics panel showed the video cache 99–100 % full (531 of 537 MB), a stream of
"N cache entries dropped before the first read (prefetch thrash)" in its problem log and a
video hit rate of 60–80 %, on three different stretches: the start, the 4K Charlotte night
drone shot, Lake Bled with a glow. `docs/images/perf-panel.webp` shows it. The owner wants
honest screenshots, so the fix belongs in the program, not the picture.

Measured without decoding (`kt-cli probe`, the new prefetch-window section, on the showcase
project at the player's 640×360 preview): at 44 s one prefetch pass asked for 140 frames =
604 MB of a 512 MB cache, and playing the 104 s edit dropped 22 025 entries before their first
read. The window's length was half the budget divided by ONE frame of the largest decode
quality (item 19b, R.46 item 318). The showcase stacks layers — a cut over its blurred
backdrop, a floating frame, a reel — so every tick asks for several frames at mixed decode
sizes, and the window overflowed the cache by itself. The LRU then ranked a prefetched frame
by its insertion: the window's nearest frames, decoded first, were the first evicted.

## What Changes

- The video look-ahead is measured by what it asks for: every layer of every frame at its
  own decode size, nearest first, cut at a whole frame within half the video budget (still at
  most 2 s). The playhead's own frame is always asked for.
- Each prefetch pass refreshes the window's cached entries from the furthest frame to the
  nearest (`BurstCache::refresh`, not a read): the eviction takes what was shown or is no
  longer wanted first, and among the window the frame furthest ahead.
- Look-ahead requests the playhead has passed are withdrawn from the decoder's queue
  (`MediaWorker::withdraw_video`): a decoder slower than the picture no longer decodes frames
  behind the playhead into entries nobody reads.
- `kt-cli probe` prints the window against the cache at three preview sizes and plays the
  range with an infinitely fast decoder (`simulate_video_playback`), reporting entries dropped
  before the first read and the video hit rate.
- The README diagnostics image is re-taken.

## Capabilities

### Modified Capabilities
- `media-index`: a new requirement — the video look-ahead fits the cache until it is read.

## Impact

`kt-cache` (`refresh`), `kt-engine` (`prefetch.rs`: `video_prefetch_wants`,
`video_prefetch_pass`, `simulate_video_playback`; `player.rs` prefetch loop; `worker.rs`
`withdraw_video`), `kt-cli` probe. No format, no UI text. Tests t1416, t1416b, t1417, t1418.
