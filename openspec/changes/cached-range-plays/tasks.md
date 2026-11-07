# Tasks: cached-range-plays

## 1. Measure
- [x] 1.1 `kt-cli preview --loop S --wav out.wav`: a range played as a loop, what the device played
- [x] 1.2 His relinked project, before: V-fps, composites, audio renders, underruns, the clicks
- [x] 1.3 Work rows `mix_replays`, `composite_replays` (load report, Diagnostics)

## 2. The composite is kept (video)
- [x] 2.1 `PresentMemo` in kt-gpu, `present` replays a held key (t1934)
- [x] 2.2 The player counts replays and records the presented keys; `final_video_runs`

## 3. The final mix is kept (audio, invariant 2 reworded)
- [x] 3.1 `AudioClip.fp`, `AudioRenderer::scene_hash`
- [x] 3.2 `kt_engine::mix`: `MixStore`, `MixPlay`, contexts, publish (t1930–t1933)
- [x] 3.3 The render loop through `MixPlay`; seek/direction/flip reset; region drops other laps
- [x] 3.4 The look-ahead, the start and the background prefetch skip held windows
- [x] 3.5 The rebuild during playback on a builder thread (t1935)
- [x] 3.6 Found by t1935: a reel's placements built all their inner state at once; born at first read, released away (t1937)

## 4. The bars
- [x] 4.1 The bright "plays without work" layer on the top strips and the event's line (t1936)

## 5. Contracts
- [x] 5.1 Root CLAUDE.md invariants 2 and 4; kt-engine, kt-cache, kt-gpu CLAUDE.md
- [x] 5.2 Spec deltas: audio-render, video-plan, media-index, transport, compositions
- [x] 5.3 What's new line; user-feedback round note; registry rows G2–G4

## 6. Check
- [ ] 6.1 Tests of the specs, `cargo test` of what changed, clippy
- [ ] 6.2 Live scenario `cached_range.py` on the heavy showcase
- [ ] 6.3 Load at rest on the heavy project; his relinked project after
