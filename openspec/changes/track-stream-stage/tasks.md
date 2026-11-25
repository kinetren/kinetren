# Tasks: track-stream-stage

> IMPLEMENTATION-READY (revision 2, 2026-11-24, orch `229c694b`). The owner answered Q1 a, Q2 a, Q3 a (kkt-26.11.20 A23). Tests: t3379–t3398, t4137. Areas: STREAM,
> `video.rs`, GPU (one op in `engine.rs`), AUDIO, EXPORT (`stream_preroll`'s predicate only), FX-UI (`track_window.rs`). After
> found-defects 1068's fix (one helper, anyone) and not beside `shared-events` sections that write `video.rs`/`audio.rs`, `durable-history-and-mix`
> (`audio.rs`) or `export-into-surface` 3–4 (`engine.rs`). Read: design D1–D9, kt-engine `CLAUDE.md` (the stream and fx-graph parts). Commit per section;
> target-prune after each.

## 0. Notes first

- [x] 0.1 F-FX-018; proposal, design (D1–D11, class walk D10), deltas `video-fx`, `audio-fx`, `ui-reach`; `--strict`
- [x] 0.2 the owner's answers folded in

## 1. The line

- [ ] 1.0 D1: `VideoTrackStage.fx_in`, its hash, `track_stage_layer`; t3396 (red on today's code: a Merge on a track renders the line); row 1067 closed;
      commit alone
- [ ] 1.1 `track_layout.rs` (D3), `handles` shared with `build_reel`; t3379
- [ ] 1.2 `Look` (D4) through `layer_src`, `track_stage_layer`, `build_plan`, `key_only`, `durable`; no byte and no key moves: t3398, t2145,
      t4108–t4116, GOLDEN_VIDEO; commit

## 2. The host

- [ ] 2.1 `TrackLine`, `link_track_lines`; `TrackStream` (D5); the trunk's layer (D6) with `LayerSrc.matte`; kt-gpu: the matte resolved as a sub-composite,
      `Pipe::Matte` + the mask-alpha pass; `moment_key`, `starved`, `durable`; t3380, t3381, t3395
- [ ] 2.2 `streams_at` and `HostStream`; the seven walkers (D8); `order_of`; the window by bytes; `has_stream_front`, `project_may_stream`;
      t3382, t3383, t3390; t1241–t1248, t2042–t2045, t3228 green as they are
- [ ] 2.3 `cargo test -p kt-engine -p kt-gpu`; clippy; commit

## 3. Keys

- [ ] 3.1 `ident` and its cells; t3384; the probe and kept moments; t3385; durable per segment; t3386

## 4. Arms, front, second

- [ ] 4.1 the front on the composite, arms as reads, the adjacent track (D7); t3387, t3388, t3397; commit

## 5. Export

- [ ] 5.1 t3389; t2707, t1451 green; commit

## 6. Sound

- [ ] 6.1 kt-model `track_pairs` goes; t3392; t3230's clause by the delta
- [ ] 6.2 `AudioState::segments`, `StreamSeg`, `TrackProcessor`; `MoshSound` per segment; `TrackStage.segs`, `scene_hash` (D9); t3391, t4137; t3232, t2370–t2373 green;
      `cargo test -p kt-engine -p kt-fx -p kt-model` and the plugin crate; commit

## 7. UI and rest

- [ ] 7.1 the refusal, its string (i18n ×5) and `has_device`'s exception go; t3237 rewritten as t3393; `kt-cli probe` prints the lines; live `track_mosh`
      (`src/scripts/live/track_mosh.py`, a screenshot per step, OUR fixtures: a dense edit with a title and a dissolve; Loading then picture; a PiP on Video 1 over Video 2 keeps
      Video 2 visible; the sound half off and on)
- [ ] 7.2 rest measured with the track window open on the heavy showcase + a track mosh (pause, mouse over a static place, the window open); t3394
- [ ] 7.3 found-defects 820 closed; registry A23; What's new ("Datamosh works on a whole track: it smears across every cut with the titles and effects that
      were on screen"); kt-engine `CLAUDE.md`: "what builds a plan takes a `Look`; what walks streams goes over `streams_at`"; `cargo test` of the touched crates, clippy, target-prune; commit
