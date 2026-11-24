# Tasks: cached-final-render

> IMPLEMENTATION-READY (revision 3, 2026-11-24, orch `229c694b`). The owner answered (kkt-26.11.20 A16: Q20 b, Q21 a, Q22 b). Tests: t3322–t3335, t3870–t3876, t4138–t4141.
> Areas: EXPORT, kt-ffmpeg (`muxer_impl.rs`, new `nal.rs`, `gop.rs`), kt-plugin-api + plugins (two facts), CACHE (`Tenant::Render`), DIALOG. One at a time with
> `loudness-norms` and `export-into-surface` 4 (EXPORT, DIALOG). 1.1 may go alone, at once. Read: design D1–D8; kt-ffmpeg and kt-engine `CLAUDE.md` (the export part). Commit per section; target-prune after each.

## 0. Notes first

- [x] 0.1 F-EXP-003 gains the owner's later word (X1, U5)
- [x] 0.2 proposal, design (D1–D13), deltas `export`, `video-plan`; `--strict`
- [x] 0.3 his answers folded in

## 1. Names

- [ ] 1.1 `adopt_sources` in every offline cache (D1); t4138 (red today: two exports of two projects with one media id and Posterize before Datamosh in one process); row 1068 closed; commit alone
- [ ] 1.2 `Plugin::{reads_time, reads_progress}`, the `false` answers; `picture_key`, `shape`; t3335, t4139; `cargo test -p kt-plugin-api -p kt-engine` and the plugin crates; commit

## 2. The file

- [ ] 2.1 `kt-ffmpeg/src/nal.rs`, `gop.rs` (SEI, `GopWriter`, hash); t4140; `Muxer::set_gops`, forced IDR per encoder, `open_encoder` taken out, `avc3`/`hev1`
- [ ] 2.2 `gop_plan` (kt-engine `cached_render.rs`), the fingerprint; t3322, t3323, t3325, t3326, t3324
- [ ] 2.3 `Tenant::Render`, `renders/`; the reader of a table and of SEIs; the strip; t3870, t4141; commit

## 3. The copy

- [ ] 3.1 `reuse`, the verification; `Muxer::copy_gop`, flush and reopen; t3328, t3329
- [ ] 3.2 spans in `export_pipeline` and the serial export, the pre-roll per span, the counts; t3327, t3330, t3331; t1451, t2805, t3998 green; commit

## 4. The window and kt-cli

- [ ] 4.1 the section, the strip, the summary, "Remove the render index"; i18n ×5; reach, layout, font tests; live `cached_render` (a screenshot per step); t3332, t3334, t3871
- [ ] 4.2 `kt-cli export --cached [--look]`, `strip-index`; t3333; `trace … export` prints the spans
- [ ] 4.3 What's new ("a second render of a project copies what did not change"); F-EXP-003; registry X1, U5; the idle check with the section open; commit

## 5. The draft

- [ ] 5.1 `DraftSource`, the mode, the window's option, `kt-cli export --draft`; i18n ×5; t3872–t3876; live `draft_render`; found-defects 773 closed by the two modes; commit
