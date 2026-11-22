# Proposal: download-library

> DESIGN (spec session, 2026-11-20; revised 2026-11-22). The owner asked for it (kkt-26.11.20 A11) and
> answered its questions (kkt-26.11.20b B3: the list from our server for everything, the library now with
> plugins at the wasm host, a missing font pack as a placeholder with Download). Code starts on the
> orchestrator's go. Feature note: F-EXT-006
> (`docs/features/plugins/hosts/download-library.md`). Tests reserved: t3850–t3869. Base: `orch-26-11-01` at
> `5e7a8ea9`.

## Why

The owner, answering Q14–Q16 (kkt-26.11.20, line 16): "на будущее - все что у нас качается - надо
переносить и делать архитектурно в виде некоторой библиотеки, типо как плагины чтоли их же тоже можно
будет качать. вот и сделат библиотеку плагинов."

What is downloaded today and what is about to be:

| Item | State | How |
|---|---|---|
| FFmpeg 9.0.2 shared build (Windows) | built | `src/apps/kt-app/src/ffmpeg_setup.rs`, which handles: a pinned sha256 and size; two mirrors; a progress bar and Cancel; unpacking `bin/*.dll` and `LICENSE*`; installing by a rename; cleaning a killed run's leftovers. The folder is `<local>/ffmpeg/<version>` |
| whisper.cpp library + ggml model | designed (`subtitles-and-text-editing`) | "pinned URL + sha256, a card, a progress bar", copying `ffmpeg_setup.rs` |
| Noto CJK / Naskh Arabic font packs | designed (same change, Q3) | "downloadable font packs on a click, pinned by sha256" |
| plugins | nothing | the wasm host is stage 2 of `plugins/design.md`; there is no catalogue |

Each new item was about to grow its own window, mirror list, folder and cleanup. Kdenlive's tracker
shows the result of that path: a model downloaded to one folder and looked for in another, and a runtime
update that silently made the models unusable (F-EXT-006).

## What Changes

- **One catalogue of items, from our signed server for every kind** (the owner's Q1 b). The release
  carries the signed list as of its build, so a machine that was never online still has one. Each item
  gives:
  - its id, kind, version, licence, what needs it, and its dependencies;
  - per platform: mirrors, the sha256, the size and the unpack rule.
  - Kinds: `ffmpeg`, `speech-runtime`, `speech-model`, `font-pack`, and `plugin` (empty until the wasm
    host).
- **One folder and one resolver.** Items are installed under `<local>/library/<kind>/<id>/<version>/`
  with an `installed.toml` (the catalogue entry, the verified hash, the date). The one function
  `library::resolve(id)` answers "where is X" for every user of an item: the FFmpeg search plan, the
  speech loader, `kt-text`'s font fallback, and the plugin registry. Nobody else builds the path.
- **One download engine.** `ffmpeg_setup.rs`'s machinery becomes generic: `fetch_verified`, the mirrors,
  the size guards, `.partial-<pid>` + rename, `remove_stale`, the progress phases and Cancel. Unpacking is
  by the item's rule (7z, zip, a single file).
- **One window, "Library"** (View → Library…). It lists every item with:
  - its size, licence and state (not installed / downloading N % / installed vX / needs vY / broken);
  - what in the program needs it;
  - Download, Cancel, Remove, Open folder, and "Install from file…".
  - The FFmpeg window becomes its FFmpeg row, keeping "Locate folder…" and the search plan's report.
  - A feature that lacks an item shows one line with a button that opens the library on that row.
- **A check at load names a mismatch.** A loader is given:
  - the installed version;
  - the versions its catalogue entry accepts (a speech model's format against the runtime's).

  A mismatch is a row state ("needs vY") and a notice, never a silent failure.
- **Offline.** "Install from file…" accepts the archive or file a person brings and checks it against
  the pinned hash. kt-cli `library list|install <id> [--from FILE]|verify|remove` does the same headless.
- **The network only on the person's act.** A download is an explicit act each time. The list is fetched
  when the Library window is opened or Download is pressed, and daily under the update check's consent.
- **The plugin library** (the owner's Q2 a) is the same rows with the kind `plugin`, filled when the wasm host exists:
  - each plugin package carries its F-EXT-003 manifest: the parameter passport, the declared
    capabilities, and no I/O;
  - a project that uses a plugin names its id and version, and a missing one is the normal "plugin
    missing" state of `plugins/spec.md`.
- **Fonts and determinism (the owner's Q3 a).** A title that uses a font pack names the pack and its version in the
  project (invariant 11: the title's bytes depend on it). Without it, the title draws a marked
  placeholder and its row in the library.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `media-index`: the FFmpeg search plan reads the library's folder; the download moves into the library.
- `plugins`: the plugin library, the catalogue kind `plugin`, a package's pinned identity.
- `ui-reach`: the Library window; a feature's missing-item line opens its row.
- `durability`: an install or a removal never leaves a half item; a broken item is named, not loaded.

## Impact

- **kt-app:**
  - `library/` (catalogue, jobs, window) replaces `ffmpeg_setup.rs`'s window and job;
  - `subtitles-and-text-editing` uses it instead of a copy.
- **kt-dirs:** `library_root()` (`<local>/library`), plus the old `ffmpeg_download_root()` still read.
- **kt-ffmpeg:** the search plan gains the library's FFmpeg folder, after the user's folder and before
  the old download root.
- **kt-text:** font packs are registered from `library::resolve`.
- **kt-plugin-api:** later, the package manifest and the registry door.
- **kt-cli:** `library`.
- **Invariants:**
  - 9 and 10: FFmpeg stays in-process, loaded by the search plan;
  - 11: a font pack or a plugin version is part of a frame's identity and is named in the project.
- **Project format:** a title's font pack reference (absent means none: no version bump while no title
  uses one).

## The owner's answers (kkt-26.11.20b, lines 5–7, row B3)

- **Q1 b — "Всё с сервера".** Every kind's entries come from our signed list (design D1). The release
  carries the list as of its build as the offline floor, so "Install from file…" and an offline first start
  work. A newer list replaces it only when its signature verifies and its sequence number is higher.
- **Q2 a — "Библиотека сейчас, плагины с wasm (Recommended)".** The library ships with FFmpeg, speech and
  font packs. The plugin rows come with the wasm host (change `wasm-plugin-host`, the owner's B11:
  wasmtime + WGSL).
- **Q3 a — "Заглушка + «Скачать» (Recommended)".** A title whose pack is missing draws a marked placeholder
  with a Download button, and the export refuses until the pack is installed or the person chooses the
  fallback for that render (design D7).
