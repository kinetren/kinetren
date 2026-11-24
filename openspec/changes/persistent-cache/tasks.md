# Tasks: persistent-cache

> Revision 4 (D1, 2026-11-23, spec-features on orch `e7b19346`): the final preview frame only — revision 3's layer
> results are gone (no `results/…/layers/`). No owner question is open. Everything can start once
> `project-cache-folder` sections 1 and 3 are merged (the folder, the entry header, `ask_room`, the frames in
> `results/…/frames/`, the key without the build and `COMPOSITE_PATH_VERSION`). The layout is
> `project-cache-folder`'s (layout B, design D1, D1b): this change does not restate it. Tests reserved: t3440–t3459
> (used t3440–t3453). Build-queue areas: CACHE (the frames' quality rule, the copies), PLAYER, DECODE (the copy
> source), EXPORT (`MuxConfig`). Commit per section; target-prune after each.

## 0. Notes first

- [x] 0.1 F-MED-002 renamed "Persistent cache" and refreshed with A10, B1 and D1
- [x] 0.2 proposal, design (D1–D7, class walk), deltas `video-plan`, `media-index`, `export`, `ui-reach`
- [x] 0.3 coordination with `project-cache-folder` (layout B; the key without the build; one value order)
- [x] 0.4 the owner's answer to Q55 folded in (D1: frames only; the stage codec question fell with the stages)

## 1. The value measure and the frames (kt-engine)

- [x] 1.1 value per byte for every store entry: the measured cost saved per read (the plan's device time from
      `Player::note_cost`, `player.rs:1457`, and the pass timer's `predict`, `kt-gpu/src/pass_time.rs:175`)
      × the reads in this project ÷ the bytes, kept in the folder's `usage.tsv` beside the last-use stamp;
      `project-cache-folder`'s `ask_room` ranks by it; t3453
- [x] 1.2 the export never reads `results/` (a counter that does not move during an export); t3445

## 2. Frames only, at the preview's quality

- [x] 2.1 the store writes and reads two kinds only (`Entry::{Frame, Copy}`); nothing in the plan or the producer
      writes a picture at a point inside a plan (no read-back of a layer's or a stage's texture for the store);
      t3451
- [x] 2.2 the frames' quality rule (design D8): the key and the prerender's state already carry the quality
      (`render_cache.rs:717`, `:1141`); fix heaviness by size: `PreviewCosts::snapshot` (`preview_cost.rs:66`)
      returns `(ns, w, h)` or a per-size view, and `candidates` (`render_cache.rs:1164`) judges a moment by its time
      at the current size, else the nearest size scaled by pixels; an edit misses exactly the moments whose plan
      changed; t3452, t4106
- [x] 2.3 `cargo test -p kt-engine` (`tests/persistent_cache.rs`); clippy; the idle check on the heavy showcase with
      the store full; commit "persistent-cache 2: final frames only, at the preview's quality"

## 3. Source copies (kt-ffmpeg, kt-media)

- [x] 3.1 the muxer already keeps a source's times (`MuxConfig.time_base` `kt-ffmpeg/src/lib.rs:213`,
      `write_video_frame_at` `:838`, t3601); add an all-intra key interval (QuickTime and no sound: the design's "Found while building")
      (`format.rs` `keyint_tenths` `:108` gives frames from tenths: an all-intra mode is a keyint of 1 frame);
      t3440
- [ ] 3.2 the copy job for heavy sources (long-GOP above 1920×1080, or decoded slower than real time) at the
      preview's size, a better copy when a higher quality is chosen, "Use a camera's copy…"; copies written to
      `media/<content id>/copy-<w>x<h>.mov` through `ask_room`; a background lane job paused by playback and
      export; t3441, t3442, t3449
- [x] 3.3 the copy source reporting the original's natives (`kt-media`), swapped in through
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
