# Tasks: project-cache-folder

> IMPLEMENTATION-READY (2026-11-22). The owner answered (kkt-26.11.20b B2); layout B is the orchestrator's final
> decision (D1, D1b); `persistent-cache` (spec-features) builds on this change. Tests: t3824–t3843, t3808, and
> the sidecar's existing tests rewritten. Code references checked against orch `873934f7` (design D1c). Read
> first: design D1–D5, D1c; `src/libs/kt-media/CLAUDE.md`, `src/libs/kt-engine/CLAUDE.md`;
> `openspec show media-index --type spec`. Build-queue area: CACHE, plus `app.rs` (hot) and Preferences.
> Commit per section; target-prune after each.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D9, D1b, D1c, class walk D7), deltas `media-index`, `video-plan`, `telemetry`,
      `ui-reach`
- [x] 0.2 the owner's answers (kkt-26.11.20b B2) and the layout decision (B)

## 1. The folder (kt-dirs, kt-media)

- [ ] 1.1 kt-dirs: `project_caches_root()` (`<data>/project-caches`), `cache_folders_registry()`
      (`<data>/cache-folders.tsv`), beside `data_dir` (`lib.rs:152`)
- [ ] 1.2 kt-media `cache_folder.rs`: the place (`<project folder>/kinetren-cache/`), the write test, the
      fallback under `project_caches_root()` and its reason; `CACHEDIR.TAG`, `README.txt`; the registry;
      the Dropbox attribute; t3824, t3825, t3827, t3828 (`kt-media/tests/cache_folder.rs`)
- [ ] 1.3 the entry header (`KTC1`, kind, version, length, xxh3), the write to `.<name>.<pid>.tmp` + rename, a
      broken or foreign header is a miss and removed; `usage.tsv` with a per-entry last-use stamp; t3826
- [ ] 1.4 the room and the cycle (D3): the room = min(cap, free − reserve); `ask_room(bytes)` on the writer's
      thread drops by value per byte across the store and `mix/`, then reels, then media; the last-minute
      stamp protects entries; `ENOSPC`/`EDQUOT` recount and retry once; a skipped write is counted;
      t3835–t3837
- [ ] 1.5 `cargo test -p kt-dirs -p kt-media`; clippy; commit "project-cache-folder 1: the folder, the header,
      the cycle"

## 2. The media index and the reels without SQLite

- [ ] 2.1 `MediaStore` (the paths map, content-id folders; peaks, packets, thumbs, thumb pixels, posters,
      facts, loudness) and pairs, with the method set of `MediaIndexDb` (`index.rs:126–770`: `ensure_media`,
      `set_loudness`/`loudness`, `set_sync_offset`/`sync_offset`, `is_complete`/`mark_complete`,
      `build_audio_peaks(_until)`, `put_packet_index`/`packet_index`, `thumbnail`/`thumbnail_near`,
      `poster`/`put_poster`, `thumb_size`/`put_thumb_size`, `put_thumbnail`, `audio_peaks`), so callers change
      their type, not their logic; `PeakPyramid` and `PeakAcc` (`:822`, `:887`) move with it
- [ ] 2.2 `ReelStore` (`put_reel_frames`, `put_reel_peaks`, `reel_preview`, `prune_reels`; `ReelPreviewRow`
      `:808`)
- [ ] 2.3 rewrite over files: t16 ×3, t872, t401, t402, t401b, t639 (`tests/index_db.rs`), t34
      (`tests/ffmpeg_conformance.rs`); read and keep or rewrite t15 ×2, t70, t795b, t1171, t1741, t1742,
      t2894, t1460 (`tests/sidecar_stall.rs`); t3829
- [ ] 2.4 the lanes (design D1c): index, thumbs, media (`util::open_source_indexed`), analysis, fx-thumbs,
      reel-preview; `index_db_path()` (`app.rs:3270`) and its five inline duplicates become `cache_folder()`;
      the editors that take the path (`morpher_editor.rs:326`, `transition_editor.rs:136`,
      `fx_editor.rs:422`); kt-cli's five openers; an untitled project caches under the fallback; t3830
- [ ] 2.5 the import of an old sidecar on the index lane, three tries; t3831, t3832
- [ ] 2.6 t985 (`user_store.rs:1252–1284`): the project folder holds `edit.kt` and `kinetren-cache/`;
      `showcase.rs:40` and `dist_guard.rs:86` follow; kt-app's t1831, t2519, t476, t504, t640, t411, t1740 read
      and kept green
- [ ] 2.7 `rusqlite` out of `kt-media/Cargo.toml:23` and the workspace `Cargo.toml:96`;
      `cargo test -p kt-media -p kt-app -p kt-cli`; clippy; commit "project-cache-folder 2: the media index
      and the reels are files"

## 3. The frames in the store, the key without the build

- [ ] 3.1 `render_cache.rs`: the root from the cache folder (`results/v<FORMAT>/<device class>/frames/`, the
      device class = adapter name + backend + driver), the app's root at `app.rs:913–924` and kt-cli's
      `Config` (`main.rs:1149–1156`); `<data>/render-cache` deleted at the first start; writes through the
      folder's `ask_room`
- [ ] 3.2 the key without the build: `durable_hash` (`:639`) plus the decoder major, each kernel's version and
      WGSL hash, and `COMPOSITE_PATH_VERSION` with its recorded hash of the composite code outside the kernels;
      the build salt (`render_cache_build`, `app.rs:12053`, `main.rs:1097`) leaves the key; a stale entry is a
      miss removed when met; t3808, t3833, t3834; t2980 and t2981 rewritten to their new scenarios
- [ ] 3.3 `cargo test -p kt-engine` (`tests/render_cache.rs`, `render_cache_bytes.rs`); commit
      "project-cache-folder 3: the prerender's frames in the store, keyed without the build"

## 4. Moves and the untitled project

- [ ] 4.1 the untitled folder renamed or merged on the first save, copied across file systems; Save As copies
      media and reels; a renamed folder hits; t3838–t3840

## 5. Telemetry, kt-cli, Preferences, notices

- [ ] 5.1 tenant records in `kt-time/src/trace.rs` (bytes, hits, misses, drops with reasons); extend
      `kt-cli trace cache` (`trace_query.rs:130`); `kt-cli cache <project> size|prune|clear` and
      `--cache-dir`; t3841
- [ ] 5.2 `Notice.action` (`app.rs:77`) and the cycle's notice once per session per folder
- [ ] 5.3 Preferences section "Persistent cache" (`prefs_window.rs`, beside FFmpeg): the folders, sizes,
      Delete, the cap; `k::prefs` keys in every catalog; the section list in `welcome_prefs_tests.rs:290`;
      `reach_tests`, `layout_tests`, font tests; t3842; planned live scenario `cache_folder` with a screenshot
      per step
- [ ] 5.4 the idle check on the heavy showcase: at rest nothing scans or trims; t3843
- [ ] 5.5 media-index `design.md` (why no SQLite, the class walk), kt-media and kt-engine `CLAUDE.md`; What's
      new line ("a project's caches live in a `kinetren-cache` folder beside it; Preferences shows what they
      take"); commit; the report's "Найдено, не исправлено"

After section 3, `persistent-cache` (layers, copies, the value measure) and `durable-history-and-mix` (history,
mix) can start; they use `ask_room`, the header and the key of this change.
