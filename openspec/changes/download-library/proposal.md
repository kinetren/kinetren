# Proposal: download-library

> DESIGN ONLY (spec session, 2026-11-20). The owner asked for it (kkt-26.11.20 A11, line 16); code starts when
> the orchestrator gives the go after his answers to Q1–Q3. Feature note: F-EXT-006
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

- **One catalogue of items, compiled into the program.** Each item gives:
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
- **The network only on a click.** A download is an explicit act each time. A catalogue refresh from a
  server (Q1) happens only under the update check's consent.
- **The plugin library** (Q2) is the same rows with the kind `plugin`, filled when the wasm host exists:
  - each plugin package carries its F-EXT-003 manifest: the parameter passport, the declared
    capabilities, and no I/O;
  - a project that uses a plugin names its id and version, and a missing one is the normal "plugin
    missing" state of `plugins/spec.md`.
- **Fonts and determinism (Q3).** A title that uses a font pack names the pack and its version in the
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

## Questions to the owner

**Q1. Where does the list of what can be downloaded come from?**
Today the FFmpeg version and its checksum are written into the program, so a new FFmpeg comes with a new
release.
- **a) Written into the program for everything except plugins; plugins from a list on our server**,
  signed with our key, fetched only with the update check's consent. *(Recommended: FFmpeg, the speech
  model and the fonts change rarely and must be tested with the program; plugins appear between
  releases.)*
- **b) Everything from the signed server list.** A new model or font is available without a release, but
  nothing works offline until the list was fetched once.
- **c) Everything written into the program, plugins included.** No server at all; a new plugin needs a
  release.

**Q2. When does the plugin library come?**
You said "вот и сделат библиотеку плагинов". Today plugins are only built in. Running downloaded plugins
needs the wasm host (plugins stage 2), which is not built.
- **a) The library now** with FFmpeg, speech and fonts and the "plugin" kind ready but empty; **the
  plugin rows with the wasm host**. *(Recommended: the library is useful at once, and a plugin row
  without a host could not run.)*
- **b) The wasm host first, then the library with plugins in it.** FFmpeg's and the subtitles' downloads
  wait.

**Q3. A title in a font from a pack opened on a machine without the pack.**
Today the program carries Noto Sans for Latin and Cyrillic. A Chinese or Arabic title needs a pack.
- **a) A marked placeholder in the frame and a line "needs the CJK font pack" with a Download button.**
  The export refuses until it is installed or the person says to render with the fallback. *(Recommended:
  a silent substitution is worse than a visible gap, F-PRJ-003.)*
- **b) Draw with any font the system has**: the export runs, the title looks different.
