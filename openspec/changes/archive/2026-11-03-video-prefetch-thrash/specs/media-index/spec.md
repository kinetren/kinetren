## ADDED Requirements

### Requirement: The video look-ahead fits the cache until it is read
One video prefetch pass SHALL ask for the look-ahead window measured by what it asks for: every
layer of every frame at its own decode size, nearest first, cut at a whole frame within half the
video budget and at most 2 s; the playhead's own frame SHALL always be asked. Each pass SHALL
mark the window's cached entries recent from the furthest frame to the nearest, without counting
a read, so the eviction takes what was shown or is no longer wanted first and, among the window,
the frame furthest ahead. Look-ahead requests the window no longer asks for SHALL be withdrawn
from the decoder's queue; the window SHALL start at the frame on screen (the playhead minus the
output latency), not at the audio playhead. A Playback video request SHALL mean the frame on
screen: composing the preview withdraws the Playback requests that are not a layer of the frame
it composes. An entry dropped before its first read
SHALL count as prefetch thrash only while the latest prefetch pass still wanted it (or was the pass
current at its insertion); one the prefetch had stopped asking for SHALL be counted apart as
abandoned. Audio and a cache nobody refreshes keep the old count. `kt-cli probe` SHALL report the
window against the cache and the entries a playback of the range drops before the first read,
thrash and abandoned.
Why: the window was half the budget divided by ONE frame of the largest decode quality, while a
stack of layers asks for several frames per tick — on the README showcase one pass wanted 604 MB
of a 512 MB cache, and the Diagnostics panel reported prefetch thrash on three stretches; LRU
ranked a prefetched frame by its insertion, so the window's nearest frames went first; a request
the playhead had passed was still decoded into an entry nobody read — and the preview queued one
Playback request per frame shown, which outranks the look-ahead, so a lagging decoder decoded the
frames the picture had left before the window ahead; and the panel counted as thrash the window a
seek left behind and the frames the picture skipped, none of which was still wanted.

#### Scenario: A stack of layers plays without dropping unread entries
- **WHEN** three video layers of 1080p frames play for 4 s against a 128 MB cache with an infinitely fast decoder
- **THEN** one pass asks for at most 64 MB, at whole frames, nearest first and past the playhead's frame; the cache evicts, yet no entry is dropped before its first read and every read hits
- **TEST** t1416 (`src/libs/kt-engine/tests/prefetch_budget.rs`)

#### Scenario: A frame larger than the half budget is still asked at the playhead
- **WHEN** the playhead's frame alone is larger than half the video budget
- **THEN** the pass asks for exactly that frame
- **TEST** t1416b (`src/libs/kt-engine/tests/prefetch_budget.rs`)

#### Scenario: The refreshed window outlives what was read
- **WHEN** a window of three frames is decoded, the nearest is read, the next pass refreshes the rest from far to near, and a new frame arrives in a full cache
- **THEN** the frame already read is evicted, not the nearest unread one (plain LRU evicts that one); among the window the furthest goes first; a refresh moves no epoch, counts no hit and does not mark the entry read
- **TEST** t1417 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: Passed look-ahead is not decoded
- **WHEN** the decoder is busy while twenty look-ahead requests and one Playback request are queued, and the look-ahead before the playhead is withdrawn
- **THEN** the decoder serves the Playback frame and the ten requests still ahead, nothing behind
- **TEST** t1418 (`src/libs/kt-engine/tests/prefetch_budget.rs`)

#### Scenario: The preview withdraws the frames it left
- **WHEN** the paused preview pages over ten frames of a file whose every frame takes 60 ms to decode
- **THEN** the decoder decodes the frame it was on and the frame now shown; of the eight in between, at most one
- **TEST** t1418b (`src/libs/kt-engine/tests/prefetch_budget.rs`)

#### Scenario: An abandoned entry is not thrash
- **WHEN** a warmed window is left by a seek and evicted unread; then an entry the latest pass still wants is evicted unread; then an audio burst is evicted unread
- **THEN** the first are counted as abandoned, not as thrash; the second and the audio burst are thrash
- **TEST** t1417b (`src/libs/kt-cache/src/lib.rs`)
