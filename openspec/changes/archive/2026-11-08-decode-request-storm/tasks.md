# Tasks: decode-request-storm

- [x] 1.1 Reproduce on his relinked project (release, /dev/shm): `kt-cli pause-scan --from/--to` with request/decoded counts; loops via `kt-cli preview --loop`, with the Datamosh at 480 and 1080 lines
- [x] 1.2 Trace the worker's spans and the stage's progress: the history ordered whole, its front evicted unread, the backward walk one key per seek
- [x] 2.1 `StreamPacing`: the windowed order (`ClipStream::window`), the composite's two-step rule, the late frame while playing (`StreamKernel::held`, `stream_held`)
- [x] 2.2 The prefetch thread advances the stages at the frame on screen and ahead (`stream_advance_ahead`, `stream_ahead_frames`), a pending stage keeps the passes running
- [x] 2.3 The worker: the run's start behind the cursor (`video_run_start`), `in_flight`, `decode_requests` counts only new queue entries
- [x] 2.4 Reel strip and poster: `stream_preroll` (public, half the given cache)
- [x] 3.1 t2040–t2044 (red on 7c6ffe06), the live scenario t2045
- [x] 3.2 The class walked and written down (design.md); measurements before/after
- [x] 3.3 kt-engine CLAUDE.md, What's new, the registry rows H6/H8
