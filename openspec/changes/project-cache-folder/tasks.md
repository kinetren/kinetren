# Tasks: project-cache-folder

> The owner answered (kkt-26.11.20b B2): one folder per project folder, cyclic writing with a notice. Code
> starts on the orchestrator's go. Tests
> reserved: t3824–t3843. Base: orch `5e7a8ea9`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D9, class walk D7), deltas `media-index`, `video-plan`, `telemetry`,
      `ui-reach`; `openspec validate project-cache-folder --strict`
- [x] 0.2 his answers (kkt-26.11.20b lines 3–4): Q1 a, the cyclic rule for Q2/Q3;
      `kkt-26.11.20-index.md` A14 points here

## 1. The folder

- [ ] 1.1 kt-dirs `project_caches_root()`, `cache_folders_registry()`; kt-media `cache_folder.rs`: the place,
      the write test, the fallback and its reason; `CACHEDIR.TAG`, `README.txt`; t3824, t3825
- [ ] 1.2 the entry header, the atomic write, a broken or foreign header is a miss and removed; t3826
- [ ] 1.3 the registry; a sync root's mark (Dropbox attribute); t3827, t3828

## 2. Media and reels without SQLite

- [ ] 2.1 `MediaStore` (paths map, content-id folders, peaks, packets, thumbs, posters, facts, loudness),
      pairs; t16, t34, t401, t401b, t872 rewritten over files
- [ ] 2.2 `ReelStore`; t639 rewritten; t3829
- [ ] 2.3 the lanes switched (index, thumbs, media, reel-preview); `index_db_path` → `cache_folder`; an
      untitled project caches too; t3830
- [ ] 2.4 the import of an old sidecar on the index lane, three tries; t3831, t3832
- [ ] 2.5 `rusqlite` removed from kt-media; `cargo test -p kt-media -p kt-app`; clippy; target-prune; commit

## 3. The prerender in the folder

- [ ] 3.1 `render_cache` root from the folder; `<data>/render-cache` deleted at the first start; t3833, t3834
- [ ] 3.2 `cargo test -p kt-engine`; commit

## 4. Cyclic writing

- [ ] 4.1 the room (cap, free space less the reserve), the cycle on the writer's thread, the tenants' order,
      `usage.tsv`, `ENOSPC`/`EDQUOT` as the same event, the notice once per session; t3835–t3837
- [ ] 4.2 the idle check on the heavy showcase: at rest nothing scans or trims; t3843

## 5. Moves

- [ ] 5.1 the untitled folder renamed or copied on the first save; Save As copies media and reels; a moved
      folder hits; t3838–t3840

## 6. Telemetry, kt-cli, Settings

- [ ] 6.1 tenant records; `kt-cli trace cache`; `kt-cli cache <project> size|prune|clear`, `--cache-dir`; t3841
- [ ] 6.2 Settings → "Caches on disk" (the list, sizes, Delete, the cap); i18n in every catalog;
      `reach_tests`, `layout_tests`, font tests; t3842; planned live scenario `cache_folder` with a
      screenshot per step
- [ ] 6.3 media-index `design.md` (why no SQLite, the class walk), kt-media and kt-engine `CLAUDE.md`;
      What's new line ("the caches of a project live in a `kinetren-cache` folder beside it, and Settings
      shows what they take"); commit
