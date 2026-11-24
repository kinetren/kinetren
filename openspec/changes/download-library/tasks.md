# Tasks: download-library

> Decided (kkt-26.11.20b B3: Q1 b, Q2 a, Q3 a); revision 3 implementation-ready at `229c694b` (2026-11-24). Stage 5 waits for
> Q4. Tests: t3850–t3869.

## 0. Notes

- [x] 0.1 F-EXT-006; proposal, design (D0–D9), deltas `media-index`, `plugins`, `ui-reach`, `durability`;
      `openspec validate download-library --strict`
- [x] 0.2 his answers (kkt-26.11.20b lines 5–7); `kkt-26.11.20-index.md` A11 and `subtitles-and-text-editing` 2.2, 2.6 point here
- [x] 0.3 planned OTIO row (the font pack's reference travels in `metadata["kt"]` with the title's values)

## 1. The core (new `libs/kt-library`; kt-dirs; kt-ffmpeg `search.rs`; kt-cli `main.rs`)

- [ ] 1.1 kt-dirs `library_root()`, t1311's and t3722's lists; `Catalogue` with the packed floor (`assets/library/catalogue.toml`),
      `Library::resolve`, `installed.toml`, stale cleanup; t3850, t3851
- [ ] 1.2 the search plan's library step (`Env::library_dirs`, `Source::Library`), kt-app and kt-cli hand it the root; t844, t845,
      t864 green; t3854
- [ ] 1.3 `dist-check.py` t860 reads the floor; `FFMPEG_DOWNLOAD_VERSION` gone; the crate in the workspace map (root `CLAUDE.md`),
      its own `CLAUDE.md`

## 2. Downloads

- [ ] 2.1 `kt-library::net` from `ffmpeg_setup.rs` (mirrors, length, cap, hash, cancel, unpack rules, partial + rename), t848 and
      t867 on it; t3852, t3853, t3862
- [ ] 2.2 "Install from file…"; kt-cli `library list|install [--from FILE]|verify|remove`; t3855, t3856

## 3. The window (kt-app `library/`)

- [ ] 3.1 `kt_library` in place of `kt_ffmpeg`, the FFmpeg row's detail, Preferences › Library…, the start; `library_line`; i18n ×5;
      `reach_tests`, `layout_tests`, t850, t950–t952; t3857–t3859
- [ ] 3.2 live `library_window` (a screenshot per step), live `ffmpeg_download` green on the new window; t3869

## 4. Fonts (**v+**) and speech

- [ ] 4.1 `GeneratorKind::Text.font_pack`, the next free version by content, `docs/ktprompts/v26.10-index.md` OTIO row; kt-text
      `set_packs`, `resolve_family`; the face in `GeneratorSource::identity`; t3863, t3865
- [ ] 4.2 the placeholder in `render_at`, the family combo, the export's refusal and the substitute choice, kt-cli
      `--font-substitute`; t3864
- [ ] 4.3 the `speech-*` entries and the `depends` check, with `subtitles-and-text-editing`; t3860, t3861

## 5. The server list (after Q4) and plugins

- [ ] 5.1 the fetch, the signature, the sequence, "no longer offered"; `scripts/library-sign.py`; t3866, t3867
- [ ] 5.2 with the wasm host: `.ktplugin` rows; t3868

## 6. Close

- [ ] 6.1 What's new ("everything the program downloads is in one Library window: what it is, its size, what needs it, one
      click to get or remove it"); media-index and plugins `design.md`; kt-app and kt-text `CLAUDE.md`; A11, B3 moved;
      `finish.sh` (orchestrator)
