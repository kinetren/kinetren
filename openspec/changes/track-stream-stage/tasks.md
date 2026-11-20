# Tasks: track-stream-stage

> Owner Q1–Q3 first. Tests reserved: t3379–t3398. Base: orch `b2653fcc`.

## 0. Notes first

- [x] 0.1 feature note F-FX-018 (`docs/features/fx/simulate/datamosh-on-a-track.md`); proposal, design (D1–D8, class walk D7), deltas `video-fx`, `audio-fx`, `ui-reach`; `openspec validate track-stream-stage --strict`

## 1. Layout and host

- [ ] 1.1 `track_stream_layout` (D2); t3379
- [ ] 1.2 `TrackStream` (`StreamLayout`/`StreamInput`): the composite input by `build_plan` + the front, `execute_held`/`execute_once`, the front memo (D1, D3); `track_stage_layer` runs the stage; t3380, t3381
- [ ] 1.3 the order and the byte-paced window (D4); `layers_at`, `wanted_frames`, `video_decode_qualities`; t3382, t3383

## 2. Keys

- [ ] 2.1 `moment(k)` and `ident` (D4); the layer's content key; t3384
- [ ] 2.2 the probe key (`key_only` twin), `show_kept`; t3385
- [ ] 2.3 `probe_durable` leaves per segment; t3386

## 3. Front and second input

- [ ] 3.1 the track chain's front; `TrackAbove/Below` as the adjacent track's composite (D5); t3387, t3388
- [ ] 3.2 `stream_want`, `stream_advance_*`, the far walk count track stages (D7)

## 4. Export

- [ ] 4.1 `stream_preroll`, the pipeline's `Planned` route, `project_may_stream` (D5); t3389, t3390

## 5. Sound

- [ ] 5.1 kt-model `track_pairs` admits Datamosh; `sound.rs` takes the segmented layout; the audio track stage seats it (D6); t3391, t3392

## 6. UI and rest

- [ ] 6.1 the track window's refusal removed; t3237's test rewritten as t3393; live `track_mosh` (`src/scripts/live/track_mosh.py`)
- [ ] 6.2 rest measured with the track window open on the heavy showcase with a track mosh; t3394
- [ ] 6.3 found-defects 820 closed; What's new line; `cargo test` of the touched crates, clippy, target-prune; commit per stage
