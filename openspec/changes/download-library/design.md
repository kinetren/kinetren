# Design: download-library

> DESIGN ONLY, revision 3 (2026-11-24), base `229c694b`. Q1 b, Q2 a, Q3 a decided (B3). Implementation-ready for stages
> 1–4; stage 5 waits for Q4 (where the signed list lives and who keeps its key).

## D0. What revision 2 said and the code contradicts

1. **View → Library…, View → FFmpeg….** The FFmpeg window opens from Preferences (`prefs_window.rs:573`) and at the start
   without FFmpeg (`welcome.rs:655`); View holds only what is viewed (welcome-window-and-settings). The Library is a row
   of Preferences in the FFmpeg row's place.
2. **`library::resolve` in kt-app.** kt-cli searches FFmpeg too (`main.rs:1605`), kt-text draws titles for kt-cli, and an
   export runs with no window. The resolver is a crate, `kt-library`.
3. **A signed list from the first stage.** The list the release packs is inside the binary: it needs no signature and no key.
   Only a fetched list is verified. So stages 1–4 have no server and no key.
4. **Fonts.** kt-text already draws installed system families (`system_face`) and answers `MissingFallback` with a notice
   (R.52, his decision "a miss = substitution by the vendored one + a visible notice"). A pack adds a family of OURS;
   the substitution rule of SYSTEM families stays as he decided it, the pack's is Q3 a.
5. **A title's cache key does not know its font file.** `GeneratorSource::identity` (`generator.rs:380`) is the values alone
   and says `durable: true`, against kt-text rule 1 ("whose path and sha256 enter the identity of the frame"). A frame
   drawn with the fallback is kept on disk under the key of the frame with the font. D7 puts the face into the identity for
   packs AND system families (found-defects row asked of the orchestrator).
6. **Folders.** `<local>` is `kt_dirs::local_dir()`; t1311 counts every direct use in kt-app and a test build answers the
   test's own folder; t3722 counts kt-cli's, and what kt-cli WRITES out of a cargo target goes to `tool_dir()`.
7. **t860** (`dist-check.py:351`) ties the program's FFmpeg pin, `fetch-ffmpeg-win.sh` and the CI job: the pin moves into the
   list and t860 reads it there.
8. **OTIO, `.kt`:** a title with a pack raises the file by content: the change is **v+**, which the build queue did not say.

## Context

`ffmpeg_setup.rs` (the pin :45–57, `download_and_install` :209, `fetch_verified` :245 with ureq/rustls and sha2,
`extract_dlls` :351 with `sevenz-rust2`, `remove_stale` :408, `FfmpegUi` :442, the window `kt_ffmpeg` :737; its tests t848,
t867, t850, t950–t952, live `ffmpeg_download`), `search.rs` (`plan` pure over `Env`, `download_root` → version folders,
t844, t864), `kt_dirs` (`local_dir`, `ffmpeg_download_root` :240, `tool_dir`), `update_check.rs` (consent, once a UTC day),
kt-text (`FaceStyle`, `resolve_family`, `system_face`), `gen_ui::font_warning`, `GeneratorKind::Text { family, bold, … }`.

## D1. The catalogue and `kt-library`

A new crate `src/libs/kt-library` (depends on kt-dirs, serde, `toml`, `sha2`; feature `net`: `ureq`, `sevenz-rust2`,
`ed25519-dalek` verify-only; kt-app and kt-cli take `net`, kt-text takes nothing: it is handed folders, D7).

```toml
seq = 3                                   # rises with every list
[[item]]
id = "ffmpeg"  kind = "ffmpeg"  version = "9.0.2"  licence = "GPL-3.0 (gyan.dev full build)"
needed_by = ["media on Windows"]
[item.on.windows-x86_64]
mirrors = ["https://www.gyan.dev/…", "https://github.com/GyanD/…"]
archive = "ffmpeg-9.0.2-full_build-shared.7z"  sha256 = "4d20…"  size = 59758994
unpack = { seven_z = ["*/bin/*.dll", "*/LICENSE*"] }      # or zip = [...], or file = "name"
[[item]]
id = "fonts-noto-cjk-sc"  kind = "font-pack"  families = ["Noto Sans SC"]  depends = []  …
```

- `assets/library/catalogue.toml` is in the repository and `include_str!`d: the FLOOR. `Catalogue::load(store_dir)` takes the
  fetched list (`<local>/library/catalogue.toml` + `.sig`) when its ed25519 signature verifies AND `seq` is higher; else the
  floor, with the reason in the trace (`Ev::Job` `LIBRARY`). A malformed entry is dropped alone, by its id.
- **An installed item keeps its entry** (`installed.toml`): a list that drops it leaves it working, the row reads "no longer
  offered".
- `KT_LIBRARY_CATALOGUE=<file>` (debug builds) replaces the floor, `KT_FFMPEG_DOWNLOAD_*` keep replacing the FFmpeg entry's
  artifact, so t848/t950–t952 and live `ffmpeg_download` keep their servers.
- t860 reads the FFmpeg entry of the floor; `FFMPEG_DOWNLOAD_VERSION` and `ffmpeg_download_dir` go (the old ROOT stays).

## D2. The folder and the resolver

```text
kt_dirs::library_root() = <local>/library         (kt-app: data_dir()/library in a test build; t1311's list grows)
<root>/<kind>/<id>/<version>/…  + installed.toml { entry, sha256, date }
<root>/.partial-<pid>-<id>/ , .download-<pid>-<id> , .removed-<pid>-<id>/
```

`Library::open(root)`; `resolve(id) -> Option<Installed { dir, entry }>`: the newest version folder with a readable
`installed.toml` (a folder without one does not exist for it). `resolve` reads the disk once and after every install,
removal and rescan (a revision, `AtomicU64`): no draw path and no media open touches the disk. A grep guard (t3851): only
`kt-library` joins a path under the root. Install: download → verify → unpack into `.partial` → `installed.toml` →
rename. Remove: rename to `.removed`, then delete. Stale leftovers (10 min, another pid) go at open.
**Windows**: FFmpeg's DLLs in use cannot be renamed over: an install of a loaded version writes the new version folder
beside it (they differ by name); removing a loaded one is refused with "in use until the program closes".

## D3. Fetching, one engine

`kt-library::net::download_and_install(entry, root, cancel, step)` is `ffmpeg_setup.rs`'s body with: the size cap
`size + 5 %` instead of 200 MiB; the unpack rule instead of `wanted_entry`; the check after unpacking by kind (D4). The state
(`DownloadProgress`, `Samples`, `eta`, `progress_text`) moves with it. One `BgJob` per item, at most two at once, the
network never on a lane (`BgJob::spawn`, as today). A proxy, loopback and cancel rules unchanged.

## D4. The loaders

| Kind | Who asks | Item's own check | On a miss |
|---|---|---|---|
| `ffmpeg` (Windows) | `search::plan`: `Env::library_dirs()` pushed after `NextToExe` and before the old download root, `Source::Library` | the five DLLs, `judge` as today | the plan's report, the row |
| `ffmpeg` (Linux, Flatpak) | nobody: the system's, or the runtime's | — | the row is read-only: "system FFmpeg 8 found", in Flatpak the runtime's text (t970) |
| `speech-runtime`, `speech-model` | `subtitles-and-text-editing` 2.x | the version symbol; the model's header; `depends` at load | "needs whisper-runtime ≥ 1.8" |
| `font-pack` | kt-text (D7) | every listed family parses (`build_face`) | Q3 a |
| `plugin` | `wasm-plugin-host` | its manifest | the missing-plugin state |

A failure is a row state, one notice and a trace record; never another version silently. **Verify** re-hashes the files listed
in `installed.toml` (sha256 per file, written at the install) on the `media` lane.

## D5. The window

`kt_library` replaces `kt_ffmpeg` in `WINDOWS`: "everything the program downloads, per machine, outside any project, reachable
when nothing plays". `.size([720, 520], [460, 260])`, `Kind::Tool`. The list scrolls, the buttons of the selected row stand in
its detail under it; the header: the total on disk and "Install from file…".

- The FFmpeg row's detail IS the former window's body (`FfmpegUi` with its Locate…, Rescan, Open folder, report, messages),
  so t850, t867, t950–t952 change only how they open it; live `ffmpeg_download` presses the same probes.
- Openers: Preferences › "Library…" (where FFmpeg… stood, with the loaded FFmpeg's line kept above it); the start without
  FFmpeg opens it on the FFmpeg row (`welcome.rs:655`, `prefs_window.rs:134`); a feature's line opens it on its row.
- **A feature's line** (`library_line(ui, id)`): "Needs the CJK font pack (19 MB)" + "Library…". Places: the title's form
  (under the family), the preview badge of a placeholder title, the render window's refusal, the subtitles card, a missing
  plugin's node.

## D6. The plugin rows

As revision 2: rows of the kind `plugin` in the same list, empty until `wasm-plugin-host`; a `.ktplugin` zip holds the
F-EXT-003 manifest, the module and its licence; a downloaded plugin never reaches the goldens.

## D7. Fonts

- `GeneratorKind::Text.font_pack: Option<String>` (`"fonts-noto-cjk-sc@2.004"`, not written when absent → `.kt` unchanged; a
  file with one is the next free version by content). The family combo lists the catalogue's families of installed and not
  installed packs after the system's, with a download mark; choosing one sets `family` and `font_pack` in one step. A
  family of a pack typed by an old title (no `font_pack`) keeps today's substitution and notice.
- kt-text: `set_packs(&[(pack, dir, version)])` (kt-app and kt-cli call it at the start and after an install);
  `resolve_family(family, bold, pack) -> Vendored | System(path, sha256) | Pack(path, sha256) | MissingFallback |
  MissingPack(pack)`. The sha256 is taken once per file, on first use.
- **The face enters the identity**: `GeneratorSource::identity` hashes the resolution (its kind, the file's sha256) beside
  `gen_rev`; `MissingPack` is its own value. An install bumps kt-library's revision → kt-app calls
  `Player::content` refresh by retuning the events with that pack (`update_gen` of the same values makes a new identity),
  so no frame of one face is shown for the other (t3865).
- **The placeholder** is drawn by kt-media `render_at` (one rule for preview, cells and poster): the title's block with the
  vendored font's layout, hatched, the pack's name in it. The export asks first (`Project::missing_packs(&Library)`): it
  refuses naming packs and titles; "Render with the substitute font" in the render window (and `--font-substitute` in
  kt-cli) renders them with the vendored face and names it in the result, never with the hatched box.

## D8. Class walk: everything that reaches the network or loads a file we did not build

| Place | Today | After |
|---|---|---|
| FFmpeg (Windows) | its own window and job | the `ffmpeg` row; the old root still searched |
| FFmpeg (Linux, Flatpak) | system / runtime | unchanged, a read-only row |
| whisper + model | designed as a copy | rows; `subtitles-and-text-editing` 2.2, 2.6 |
| Noto CJK / Arabic for TITLES | designed as a copy | `font-pack` rows |
| the interface's fonts (subsets) | vendored | unchanged: built from the catalogs |
| a system font family | drawn by path, fallback with a notice | unchanged; its file now in the identity (D0.5) |
| libva drivers, GPU drivers | the system's | not ours; named in Diagnostics |
| update check, telemetry | consent | unchanged; stage 5 fetches the list under the update check's consent |
| LUTs, presets, templates | the person's files | not downloads |
| caches, shader cache | ours | unchanged |
| `flatpak-flathub` | — | `<local>` is inside the sandbox's data; downloads work there; the manifest needs no new permission |
| `wasm-plugin-host`, `openfx-host` | designed | the first through the `plugin` kind; OpenFX bundles are the person's, found by their own search, not downloaded by us |

## D9. Stages and tests

1. `kt-library` without `net` + the floor, `resolve`, the guard, the search plan, t860 (t3850, t3851, t3854).
2. `net` from `ffmpeg_setup.rs`; install from file; kt-cli `library list|install|verify|remove` (t3852, t3853, t3855, t3856, t3862).
3. The window, the FFmpeg row, the feature's line (t3857–t3859, t3869, live `library_window`, live `ffmpeg_download`).
4. Fonts: the model field, kt-text, the identity, the placeholder, the export (t3863–t3865). `speech-*` rows with
   `subtitles-and-text-editing` (t3860, t3861).
5. After Q4: the fetched list (t3866, t3867); the plugin rows with the wasm host (t3868).

kt-cli's `library install|remove` writes into the person's library only when installed; out of a cargo target it uses
`<tool_dir>/library` (own-data-dir) and says so.

## Idle

`resolve` is memory; the window with no job asks no frame (t3869). A hash of a font file is once per file per process.

## Risks

- **A mirror changes a file under the same name**: the hash refuses it, the row names the mirror.
- **A font pack is 16–19 MB a weight, a medium model 1.5 GB**: the size stands before the click, the total in the header.
- **ed25519-dalek**: verify only, no randomness; it enters with stage 5 alone.

## Rejected

- **Signing the packed list** (revision 2) — trust in a file inside the binary is trust in the binary.
- **kt-text depending on kt-library** — kt-text has no dependency on our crates (its CLAUDE.md).
- **Refusing system-font misses too** — his R.52 decision stands; only a pack has a button that repairs the miss.
