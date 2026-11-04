# Proposal

## Why

The owner (`kkt.txt`, 2026-10-27, the second item 3; recorded in `docs/user-feedback.md`, round of
2026-10-27): "The FFmpeg download window is not informative — what speed, where it installs,
etc." The window (`src/apps/kt-app/src/ffmpeg_setup.rs`, `draw_ffmpeg_window` ~433-560)
shows one grey line before the click: "Shared build by gyan.dev, GPLv3, N MB, from
www.gyan.dev…". During the download it shows a bar reading "X of Y MB", and at the end a
single sentence. Most of what a person would want to know already exists in the code and
is dropped on the way to the screen:

- The URL in use and the mirror fallback (gyan.dev → GitHub, `PINNED_URLS` ~47-50).
  `DownloadStep::Connecting(url)` is sent, but the window paints only "Connecting…", and
  `BgJob` keeps only the LAST step (`src/apps/kt-app/src/bg.rs:115-120`), so the URL is lost
  as soon as the first bytes arrive.
- Speed and time left. Both can be computed from the byte steps and the clock, but nothing computes them.
- The real Content-Length against `PINNED_SIZE`, the archive's name (`PINNED_ARCHIVE`),
  the pinned SHA-256 and the fact that the hash matched.
- The install folder `download_root()/<version>`. `poll` (~372-392) receives it in
  `Ok(path)` and drops it (`Ok(_) => …`).
- How the unpacking goes: `extract_dlls` counts the files it writes and shows nothing.
- Why a mirror failed: `fetch_verified` collects one error per URL and joins them into one
  red line at the very end, so the person never learns that gyan.dev failed while GitHub worked.

## What Changes

- **Before the click**: a short "what happens" block. Archive name and size; the source
  (gyan.dev, and GitHub as a mirror if it fails); the check against the SHA-256 built into
  Kinetren (the first 16 hex digits shown, the full value in a hover); only the DLLs and the
  licence are unpacked into `<folder>`; nothing else on the computer changes; to remove it,
  delete that folder.
- **During**: the host in use and "mirror 1 of 2"; "X of Y MB · S MB/s · about T left" (a
  smoothed rate over the last seconds; "—" when stalled or unknown); a server whose
  Content-Length differs from the pinned size is named and skipped before its body is read
  (its file cannot have the pinned hash); "Checking SHA-256…"; "Unpacking: n of m files".
- **After**: on success "Installed into <folder>" with the matching hash. On failure, a
  list of attempts, one line per mirror with its own error. **Open folder** opens the
  install folder (or the folder of the loaded FFmpeg) in the system file manager.
- The download step becomes a full snapshot of the attempt (mirror index, URL, bytes,
  expected size, attempts so far). A dropped intermediate step then loses nothing, and
  `BgJob` stays as it is.
- Translation catalogs (five languages) get the new strings.
- Builds on the archived change `ffmpeg-9` (2026-10-27, merged into this branch). Its task 3.2
  (`download_offered`, the window naming the loaded folder, "restart to use it" over a loaded
  FFmpeg) is the base this change edits. t867 and t850 must stay green.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `media-index`: the downloader reports what it does. Each progress step carries the source,
  the mirror, the expected and announced sizes and the attempts so far. A size mismatch skips
  the mirror before its body is read. The result names the installed folder and the verified hash.
- `ui-reach`: the FFmpeg window explains the download before the click; it shows source,
  speed and time left during it, and the folder, hash and attempts after it; it gains
  **Open folder**.

## Impact

- Code: `src/apps/kt-app/src/ffmpeg_setup.rs` (`DownloadStep` → snapshot, `DownloadSource`
  gains `size`/`archive`, `fetch_verified` size check and per-mirror attempts, `extract_dlls`
  progress, `FfmpegUi` keeps the result folder, a rate sampler and an `os` override for tests,
  `draw_ffmpeg_window`); a small `open_in_file_manager(path)` helper in
  `src/apps/kt-app/src/util.rs` (`explorer.exe` on Windows, `xdg-open` elsewhere, with a test
  seam); `src/apps/kt-app/i18n/*.toml` (`[ffmpeg]` section).
- No new dependency (process spawn from std). No engine, format or loader change. The pin,
  hash and URLs do not change, so dist-check t860 is untouched.
- Docs: none. The feature note F-MED-007 is `done` and frozen. This change answers a complaint
  about a finished window and adds no new feature, so no `docs/features/` note is needed.
