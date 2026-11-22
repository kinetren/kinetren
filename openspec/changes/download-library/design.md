# Design: download-library

## Context

- **The FFmpeg download** (`src/apps/kt-app/src/ffmpeg_setup.rs`, F-MED-007, media-index):
  - **The pin:** `PINNED_VERSION` 9.0.2 (from `kt_dirs::FFMPEG_DOWNLOAD_VERSION`), `PINNED_ARCHIVE`,
    `PINNED_SHA256` committed in code (never read from the server's `.sha256`), `PINNED_SIZE`,
    `MAX_ARCHIVE_BYTES` 200 MiB, and two mirrors serving byte-identical files.
  - **The fetch:** `fetch_verified` uses ureq over rustls with a 10 s connect timeout; plain HTTP is
    allowed only for loopback, and Cancel is polled per chunk.
  - **The install:** `extract_dlls` keeps `bin/*.dll` and `LICENSE*`, writes into `.partial-<pid>` and
    renames to `<root>/<version>`; `remove_stale` cleans leftovers after 10 min.
  - **The window and job:** View → FFmpeg…, `BgJob` "ffmpeg-download", phases
    Connecting/Receiving/Verifying/Unpacking.
  - **The rest:** debug overrides `KT_FFMPEG_DOWNLOAD_*`; the search plan (`kt-ffmpeg/src/search.rs`)
    reads every version folder under the root, newest first.
- **Other network:** the update check (`update_check.rs`) and telemetry (`telemetry.rs`), both under
  consent; neither downloads code or data to run.
- **Planned:**
  - whisper.cpp + a ggml model (base ~140 MB, small ~470 MB, medium ~1.5 GB), loaded with `dlopen`
    (`subtitles-and-text-editing` design 51–53);
  - Noto CJK (~16–19 MB a weight) and Naskh Arabic packs (same change, Q3).
- **Plugins:** `kt-plugin-api` `Registry` and `install()`, built-in families only, no dynamic loading;
  the wasm host is stage 2 (`openspec/specs/plugins/design.md`); F-EXT-003 fixes what foreign code must
  declare (no I/O, the parameter passport, deterministic kernels).

## D1. The catalogue

```text
Item {
  id: "ffmpeg" | "whisper-runtime" | "whisper-model-small" | "fonts-noto-cjk" | "plugin:<id>" …,
  kind: Ffmpeg | SpeechRuntime | SpeechModel | FontPack | Plugin,
  version: "9.0.2",
  licence: "GPL-3.0 (gyan.dev full build)",
  needed_by: ["subtitles from speech"],         // shown on the row and by the feature's line
  depends: [("whisper-runtime", ">=1.8 <2")],   // checked at load (D4)
  per_platform: { "windows-x86_64": Artifact { mirrors, sha256, size, unpack: SevenZ { keep: [...] } } , … },
}
```

- **Every entry comes from our signed list** (the owner's Q1 b, kkt-26.11.20b). It is a TOML of the entries
  with a sequence number, signed with ed25519. Our public key is compiled in, and the release's build packs
  the list as of that day (`assets/library-catalogue.toml` + `.sig`), so a machine that was never online
  has a list.
- **Fetching:** when the Library window opens, when Download is pressed on a row older than a day, and
  daily under the update check's consent. The fetched list is kept in `<local>/library/catalogue.toml`.
- **Which list is used:** the one with the highest sequence number whose signature verifies. A list with
  a lower number than the one held is refused (no rollback to an older, vulnerable pin), and so is a list
  whose signature does not verify. The trace says why.
- **An installed item keeps the entry it came from** (`installed.toml`), so a later list that drops an item
  does not break what is installed. The row then reads "no longer offered", and Remove still works.
- `KT_LIBRARY_CATALOGUE=<file>` replaces the list for tests (as `KT_FFMPEG_DOWNLOAD_URL` does today).

## D2. The folder and the resolver

```text
<local>/library/
  catalogue.toml                   (the newest verified signed list)
  <kind>/<id>/<version>/           the unpacked item
  <kind>/<id>/<version>/installed.toml   the entry it was installed from, the verified sha256, the date
  .partial-<pid>-<id>/             a download in progress (removed if stale)
```

- `<local>` is `%LOCALAPPDATA%\kinetren` on Windows and the data folder on Linux (`kt_dirs`). Items are
  per machine, never roaming.
- **`library::resolve(id) -> Option<Installed>`** is the one place that turns an id into a folder. It
  picks the newest installed version the catalogue accepts. The FFmpeg search plan, the speech loader,
  the font registration and the plugin door call it. A grep guard (t3851) keeps every other path out.
- **The old FFmpeg root** `<local>/ffmpeg/<version>` stays readable: the search plan keeps its step, and
  the library row shows such a folder as "installed (old place)". Nothing is moved.

## D3. The download engine

`ffmpeg_setup.rs`'s machinery becomes `library/fetch.rs`, item-agnostic:
- the mirrors in order;
- a declared length checked before the body;
- the size cap (the item's size + 5 %);
- the hash while receiving;
- Cancel per chunk;
- `.partial-<pid>-<id>/` + rename;
- `remove_stale`;
- the progress phases with speed and ETA.

The unpack rules are 7z (`sevenz-rust2`, today's), zip, and a single file; each keeps only the listed
entries. One `BgJob` per download. Two downloads may run at once, each item at most once.

## D4. Loading and the mismatch check

- A loader asks `resolve(id)`. It then checks that the installed version satisfies its `depends` (a model
  against the runtime it was made for), and that the item's own check passes:
  - for FFmpeg: the five DLLs open and report the major;
  - for the runtime: `dlopen` and the version symbol;
  - for a model: the file header;
  - for a font: the face's table checksum.
- A failure becomes:
  - a row state, "needs whisper-runtime ≥ 1.8" or "broken: header";
  - a notice naming the item;
  - a trace record.
  - Never a silent fallback. This is the class of Kdenlive issue 1831 (F-EXT-006).
- **"Verify"** on a row re-hashes the folder against `installed.toml`. Plain loads check only the
  existence and the item's own check (a 1.5 GB model is not hashed at every start).

## D5. The window

View → Library… (the FFmpeg window's menu item opens it on the FFmpeg row).
- **Each row:** name, kind, version, size, licence, "needed by", state, and buttons Download / Cancel /
  Remove / Open folder / Verify. The FFmpeg row also has Locate folder… and the search plan's report.
- **The header:** the total size and "Install from file…".
- **A feature's line:** the subtitles card, the title's font placeholder and a missing plugin each show
  one line, "Needs <item> (<size>)", with a button that opens the library on that row.
- **The rules:** the window follows the form contract (`src/apps/kt-app/CLAUDE.md`, R.66: the list
  scrolls, the buttons stay outside it); every text is in every catalog; and it runs no job while
  open, apart from a download the person started.

## D6. The plugin library (the owner's Q2 a: with the wasm host)

- Rows of the kind `plugin` come from the same signed list, empty until the wasm host (`wasm-plugin-host`,
  the owner's B11) exists. Each package is a `.ktplugin`, a zip
  holding the F-EXT-003 manifest (id, version, API version, families, the parameter passport, the
  declared capabilities with no I/O), the wasm module, and its licence.
- It is installed like any item. It is registered through the registry door the wasm host adds. Until
  then, the kind is empty and the window shows no plugin rows.
- A project names the plugin's id and version. Opening it without the plugin is the spec's "missing
  plugin" state, with a line to its row.
- A plugin never reaches the goldens (`plugins/spec.md`: only built-in ids).

## D7. Fonts and determinism (the owner's Q3 a: a placeholder with Download)

A title using a pack's face writes `font_pack = "<id>@<version>"` beside its face name in the `.kt`
(absent = the vendored Noto). With the pack missing:
- the preview draws the title's box with a hatched marker and the pack's name;
- the export refuses with the pack named, unless the person chooses "render with the fallback" for
  this render.

An old title with no pack field is unchanged.

## D8. Class walk: everything that reaches the network or loads a file we did not build

| Place | Today | After |
|---|---|---|
| FFmpeg download (Windows) | its own window and job | the library's `ffmpeg` row; the search plan reads `library/ffmpeg/` then the old root |
| FFmpeg on Linux | system SONAMEs | unchanged; the row shows "system FFmpeg N found", read-only |
| whisper.cpp + model | designed as a copy | `speech-runtime` and `speech-model` rows, the model's `depends` on the runtime |
| CJK / Arabic font packs | designed as a copy | `font-pack` rows; the project names the pack (D7) |
| plugins | none | `plugin` rows with the wasm host (D6) |
| update check | daily, consent | unchanged; it also fetches the signed list under the same consent |
| telemetry | consent | unchanged; it never sends what is installed beyond counts by kind |
| LUTs, presets, title templates | the person's own files, `user_store` | unchanged: not downloads; a downloadable preset pack would be a future kind |
| `shader-cache/`, pass times | written by us | unchanged: our own output, not a download |
| the project cache folder | written by us | unchanged (`project-cache-folder`) |

## D9. Stages and tests

1. The catalogue, the folder, `resolve` and its guard; the generic fetch with FFmpeg moved onto it (the
   FFmpeg download's tests keep passing); "Install from file…"; kt-cli `library` (t3850–t3856).
2. The window and the FFmpeg row; a feature's missing-item line (t3857–t3859; planned live scenario
   `library_window`).
3. The speech runtime and model rows with the mismatch check, for `subtitles-and-text-editing`
   (t3860–t3862).
4. The font packs and the project's pack reference (t3863–t3865).
5. The signed list's fetch, sequence and rollback refusal (t3866, t3867); the plugin rows with the wasm host
   (t3868).
6. The idle check with the Library window open (t3869).

## Risks

- **A mirror changes a file under the same name:** the hash refuses it, and the row says "the download
  does not match its pin" with the mirror named.
- **Disk:** a medium speech model is 1.5 GB. The row shows the size before the click, and the header
  shows the total.
- **A signed list is a key to keep:** losing the key means a new release with a new public key; the lists
  packed in older releases and the installed items keep working.
