# Tasks: download-library

> The owner answered (kkt-26.11.20b B3): Q1 b, Q2 a, Q3 a. Code starts on the orchestrator's go. Feature note
> F-EXT-006. Tests reserved: t3850–t3869. Base: orch `5e7a8ea9`.

## 0. Notes first

- [x] 0.1 feature note F-EXT-006 (`docs/features/plugins/hosts/download-library.md`); `features-index.py --check`
- [x] 0.2 proposal, design (D1–D9, class walk D8), deltas `media-index`, `plugins`, `ui-reach`, `durability`;
      `openspec validate download-library --strict`
- [x] 0.3 his answers (kkt-26.11.20b lines 5–7);
      `kkt-26.11.20-index.md` A11 points here; `subtitles-and-text-editing` tasks 2.2 and 2.6 point here

## 1. The core and FFmpeg on it

- [ ] 1.1 kt-dirs `library_root()`; `library/catalogue.rs` (the signed list's reader, the packed list, the
      sequence rule), `library::resolve` and the grep guard; t3850, t3851
- [ ] 1.2 `library/fetch.rs` from `ffmpeg_setup.rs` (mirrors, length, cap, hash, Cancel, partial + rename,
      stale cleanup, phases); the FFmpeg download's existing tests green; t3852, t3853
- [ ] 1.3 the search plan reads `library/ffmpeg/` before the old root; t3854
- [ ] 1.4 "Install from file…" and kt-cli `library list|install|verify|remove`; t3855, t3856

## 2. The window

- [ ] 2.1 View → Library…, rows, states, buttons, the header total; the FFmpeg window's item opens its row;
      a feature's missing-item line; i18n in every catalog; `reach_tests`, `layout_tests`, font tests;
      t3857–t3859; planned live scenario `library_window` with a screenshot per step

## 3. Speech

- [ ] 3.1 `speech-runtime` and `speech-model` entries, the `depends` check at load, the row's "needs" state;
      t3860–t3862 (with `subtitles-and-text-editing`)

## 4. Fonts

- [ ] 4.1 `font-pack` entries; kt-text registers faces from `resolve`; the title's pack reference in `.kt`;
      the placeholder with Download and the export's refusal (Q3 a); t3863–t3865; a "What OTIO export loses" row if the
      reference does not survive the door

## 5. The server list and plugins

- [ ] 5.1 the list's fetch (Library opened, Download pressed, daily under consent), the sequence and rollback
      refusal, "no longer offered" rows; the release build packs the signed list; t3866, t3867
- [ ] 5.2 with the wasm host: `.ktplugin` rows, registration, a missing plugin's line; t3868

## 6. Rest

- [ ] 6.1 the idle check with the Library window open; t3869; What's new line ("everything the program
      downloads is in one Library window: what it is, its size, what needs it, one click to get or
      remove it"); media-index and plugins `design.md`; kt-app `CLAUDE.md` (only `resolve` builds an
      item's path); commit
