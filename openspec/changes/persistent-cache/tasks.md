# Tasks: persistent-cache

> Revision 3 (B1), anchored to the tree by spec-engine on 2026-11-22 (orch `873934f7`; spec-features' session
> stopped). One owner question is open: Q1, how exact a layer result is (lossy hardware codec, or LZ4
> lossless). Only task 2.3's codec waits for it; everything else can start once `project-cache-folder`
> sections 1 and 3 are merged (the folder, the entry header, `ask_room`, the frames in `results/…/frames/`, the
> key without the build and `COMPOSITE_PATH_VERSION`). The layout is `project-cache-folder`'s (layout B, design
> D1, D1b): this change does not restate it. Tests reserved: t3440–t3459 (used t3440–t3453). Build-queue areas:
> CACHE (`results/…/layers/`, the copies), the plan's layer input (`video_plan.rs`, `video.rs`), PLAYER,
> DECODE (the copy source), EXPORT (`MuxConfig`). Commit per section; target-prune after each.

## 0. Notes first

- [x] 0.1 F-MED-002 renamed "Persistent cache" and refreshed with A10 and B1
- [x] 0.2 proposal, design (D1–D9, class walk), deltas `video-plan`, `media-index`, `export`, `ui-reach`
- [x] 0.3 coordination with `project-cache-folder` (layout B; the key without the build; one value order)
- [ ] 0.4 the owner's answer to Q1 folded in (gates task 2.3's codec only)

## 1. The value measure and the frames (kt-engine)

- [ ] 1.1 value per byte for every store entry: the measured cost saved per read (the plan's device time from
      `Player::note_cost`, `player.rs:1457`, and the pass timer's `predict`, `kt-gpu/src/pass_time.rs:175`)
      × the reads in this project ÷ the bytes, kept in the folder's `usage.tsv` beside the last-use stamp;
      `project-cache-folder`'s `ask_room` ranks by it; t3453
- [ ] 1.2 the export never reads `results/` (a counter that does not move during an export); t3445

## 2. Layer results

- [ ] 2.1 the cut points (D2): after a barrier node or a stream stage of an event's chain, after a track
      stage, the output of a reel instance's sub-plan; heavy = measured device time at this size ≥ half a
      frame period over the last three measurements
- [ ] 2.2 the layer key: `durable_hash` over the layer's sub-plan up to the cut, plus the decoder major, the
      prefix's kernels' versions and WGSL hashes, and `COMPOSITE_PATH_VERSION` (the guard is
      `project-cache-folder`'s t3808); t3452
- [ ] 2.3 writing: the producer reads back the cut's texture once and writes a stage segment (a run of
      consecutive moments of one cut) through `ask_room`, at rest and while playing when the device has spare
      time; the codec per Q1 (the prerender's hardware segment writer `kt-media/src/segment.rs`, or LZ4)
- [ ] 2.4 reading: `frame_plan` (`video.rs:2360`) asks the store for each layer's deepest held cut; found, the
      layer's plan begins at the cut with the decoded picture as `LayerInput::Frame` (or `LayerInput::Held`
      on the composing device, `video_plan.rs:128–150`, stream-picture-stays' form); the coverage bars draw a
      moment bright only when its frame or every heavy layer's cut is held; t3451
- [ ] 2.5 `cargo test -p kt-engine` (`tests/persistent_cache.rs`); clippy; the idle check on the heavy
      showcase with the store full; commit "persistent-cache 2: heavy layers' results survive on disk"

## 3. Source copies (kt-ffmpeg, kt-media)

- [ ] 3.1 the muxer already keeps a source's times (`MuxConfig.time_base` `kt-ffmpeg/src/lib.rs:213`,
      `write_video_frame_at` `:838`, t3601); add MKV with audio stream copy and an all-intra key interval
      (`format.rs` `keyint_tenths` `:108` gives frames from tenths: an all-intra mode is a keyint of 1 frame);
      t3440
- [ ] 3.2 the copy job for heavy sources (long-GOP above 1920×1080, or decoded slower than real time) at the
      preview's size, a better copy when a higher quality is chosen, "Use a camera's copy…"; copies written to
      `media/<content id>/copy-<quality>.mkv` through `ask_room`; a background lane job paused by playback and
      export; t3441, t3442, t3449
- [ ] 3.3 the copy source reporting the original's natives (`kt-media`), swapped in through
      `Player::replace_media` (`player.rs:885`) and opened through `device-decode-sources`'
      `factory::open_source` when that change has landed; t3443, t3444, t3446
- [ ] 3.4 `cargo test -p kt-ffmpeg -p kt-media -p kt-engine`; clippy; commit "persistent-cache 3: heavy sources'
      copies"

## 4. Interface (kt-app)

- [ ] 4.1 the switch "as in the preview", the card badges, the Diagnostics section; i18n in every catalog;
      t3447, t3450; planned live scenario `persistent_cache` (t3448) with a screenshot per step
- [ ] 4.2 the idle check on a heavy project with the store full

## 5. Close

- [ ] 5.1 What's new; F-MED-002 verdict; registry B1 moved; the report's "Найдено, не исправлено"
