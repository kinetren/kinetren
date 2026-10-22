# Spec Delta

## MODIFIED Requirements

### Requirement: Live playback goes through the MediaWorker priority queue
One thread SHALL exclusively own the decoder instance (`&mut self`). Playback, prefetch and thumbnails MUST NOT touch the decoder directly — only by requests into the queue with priorities
Playback > NearPrefetch > FarPrefetch > Thumbnail and generation-cancellation; at equal priority, requests are served FORWARD from the decoder's cursor, and backward when forward is empty; a key is served no more than once per enqueue; what is already in the cache is not decoded. Playback MUST NOT block on decoding: if there is no data by the deadline → silence/last frame + underrun flag.
Playback audio MUST NOT starve behind the video of the same worker: after a jump (seek) into a file of any length, the first audio bursts with Playback priority SHALL be served before the video requests (Playback video — not before the audio of the same deadline; video prefetch — only when there are no Playback/NearPrefetch audio requests), and a single video frame MUST NOT hold the thread for longer than one audio burst without checking the queue; after pre-roll the underruns SHALL be zero, while the source is read faster than real time.
Why: the old engine shared one decoder between prefetch and playback through a mutex and drove it with seeks (defect h), and a single puller thread pulled both audio and video — a slow video decode starved the sound (defect l). R.63: per the flight log of paintitblack, each jump into the 870 s file (H.264 1080p) gave 1–6 s of complete silence (u≈48 000 frames/s at a render cost of 2% of real time) — that is, defect l came back in different clothes: the sound was waiting for the video.

#### Scenario: Scheduler — priority, cancellation, deduplication, direction
- **WHEN** the queue is clogged by a far prefetch and a Playback request arrives; the generation is bumped; one key is queued twice; equal priorities lie on both sides of the cursor
- **THEN** Playback is served among the first three; the stale generation is dropped; the key is served once; requests forward of the cursor are served first; the prefetch pass is idempotent
- **TEST** t9 (`src/libs/kt-engine/tests/scheduler.rs`)

#### Scenario: Sequential grid without a decode storm
- **WHEN** 90 grid frames are ordered in a row, then again over the warm cache; audio and video alternate
- **THEN** zero backward reads; each frame is decoded at most once; the warm repeat — zero decodes; the audio and video cursors do not zigzag after one another
- **TEST** t21 (`src/libs/kt-engine/tests/video_span.rs`)

#### Scenario: The video cache does not stand under a stream of Playback requests
- **WHEN** "slow 4K" plus constant Playback singles of the current frame and NearPrefetch passes
- **THEN** span top-up goes ON TOP OF the priorities (the Playback key becomes the start of the span); coverage grows continuously to the end; each frame is decoded once; the key past EOF is decoded at most once (negative cache)
- **TEST** t27 (`src/libs/kt-engine/tests/video_stall.rs`)

#### Scenario: The worker inserts coverage as a strip along the grid step
- **WHEN** 30 frames are ordered on the grid
- **THEN** coverage is one continuous run [0, n·step)
- **TEST** t28 (`src/libs/kt-engine/tests/video_stall.rs`)

#### Scenario: Retuning a source is not a request
- **WHEN** the source is reconfigured in the middle of seeks (dragging a generator parameter)
- **THEN** the reconfiguration is not subject to generation-cancellation, is applied first, and the cache of its source is cleared by THE SAME worker thread — not a single frame with the old parameters remains
- **TEST** t501 (`src/libs/kt-engine/tests/scheduler.rs`)

#### Scenario: A jump into a long file — the sound does not wait for the video
- **WHEN** a source with slow video (a frame costlier than an audio burst) and length ≥ 300 s; the player jumps into its tail and into the middle several times in a row, with the window's video prefetch on
- **THEN** the first audio bursts after each jump are served before any video frame; the underruns after pre-roll are zero (on the old code — thousands of frames), while the video catches up without skipping grid frames; in the live window on `kt-untitled-review-870s.mp4`, after a jump to 817 s the flight log contains no SNAP with `u` > 0 after the first second
- **TEST** t758 (`src/libs/kt-engine/tests/video_stall.rs`), live scenario `src/scripts/live/seekjump.py`
