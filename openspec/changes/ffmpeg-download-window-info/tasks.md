# Tasks

Test numbers t948–t953 are reserved by this change. Every stage ends with a commit: the message is in
English and its first line names the stage and what was done. Depends on the archived
`ffmpeg-9` (2026-10-27, already in this branch). Its t867 and t850 must stay green throughout.

## 0. Records before code

- [x] 0.1 Proposal recorded: the owner's complaint (`kkt.txt` 2026-10-27, the second item 3) is in `docs/user-feedback.md` (round of 2026-10-27), and the change's artifacts are written. F-MED-007 is `done` and stays frozen. Verify `openspec validate ffmpeg-download-window-info --strict` is green

## 1. The downloader tells everything

- [x] 1.1 `DownloadProgress` snapshot (design D1), `DownloadSource.size`/`archive` (D3), the size check before the body, per-mirror `attempts`, the unpacking count (D4), and the result carries the folder and the verified hash. Move t848's step matching to the snapshot. Verify t848 is green unchanged in its assertions
  - `DownloadProgress { phase, mirror, mirrors, url, done, announced, expected, unpacked, to_unpack,
    attempts }` (no `receiving_since`: the window keeps its own samples, D2); `DownloadSource.size` and
    `.archive` (`KT_FFMPEG_DOWNLOAD_SIZE` in debug); the size check returns "announced A bytes,
    expected B" before the file is created; `extract_dlls` counts the wanted entries from the 7z
    header and reports after each file; the result is `Installed { folder, sha256, archive }`. t848's
    step matching moved to the snapshot (phases and `announced`), its assertions otherwise unchanged;
    green, and `pinned_archive_extracts_like_7zip`.
- [x] 1.2 Write t949 in `src/apps/kt-app/src/ffmpeg_setup.rs` (two local mirrors: size mismatch with no body read, 404 + break, cancel during unpacking). Verify it is green
  - t949: (1) a wrong-size first mirror is skipped with no `Receiving` snapshot for it, the second
    installs, the last snapshot names mirror 2 of 2 and lists the first with its exact reason; the
    unpacking goes 0/3 → 3/3; (2) a 404 and a broken connection are both listed, the error has both
    lines, nothing is left; (3) a cancel during unpacking leaves nothing. RED without the size check:
    "the wrong-size mirror's body was read".
- [x] 1.3 The pure `rate`/`eta` functions and the sample ring (D2). Write t948. Verify it is green
  - `Samples` (a fixed ring of 16 `(Instant, bytes)`, reset on a new mirror or fewer bytes) with
    `rate(now)` (≤ 5 s, `None` under two samples or after 2 s without bytes) and `eta(done, total,
    rate)`. t948 drives them with synthetic instants: steady 1 MB/s, a stall, a new mirror, the time
    left.
- [x] 1.4 Commit stage 1
  - Committed with stage 2 and 3.1 (one commit): the window had to change with the step type.

## 2. The window

- [x] 2.1 `FfmpegUi`: keep the result folder, the `os` override (D6), `test_open` (D5), and the sample ring fed in `poll`. `util::open_in_file_manager`. Verify `cargo build -p kt-app` and t867 are green
  - `FfmpegUi`: `installed`, `last` (the snapshot kept after the job, so a failure still lists the
    mirrors), `samples` fed in `poll`, `os` (native; `KT_FFMPEG_OS=windows` in debug builds for the live
    run), `test_open`, and `source` (computed once — the window reads it every frame; tests point it at
    their servers). `folder_to_open` (installed → a loaded folder → the download root on Windows rules;
    none for a system SONAME) and `open_folder`; `util::open_in_file_manager` (explorer.exe / xdg-open,
    detached). t867 green.
- [x] 2.2 `draw_ffmpeg_window`: the "what happens" block in the body, the progress line with host, mirror, MB, speed and time left, the "Tried:" list, the success and failure blocks, and **Open folder** in the actions row (D7). Verify t850 and t779 are green
  - The body: the "what happens" block in two catalog lines (the 160-character limit split it):
    archive, MB, host, mirror, the first 16 hex digits (the full hash in the hover), then the folder and
    how to remove it; "Tried:" with host — reason; the success block (installed into, SHA-256 …
    matches). The actions: the mirror line, the bar text by phase (`progress_text`: bytes · speed or
    "—" · time left; connecting; checking; unpacking n of m), **Open folder** (probe
    `ffmpeg.open_folder`). Probes `ffmpeg.what/.download/.progress/.tried`, `menu.view`, `view.ffmpeg`
    for the live run. The old `extracting` key is gone, `verifying` reads "Checking SHA-256…". t850,
    t779 green.
- [x] 2.3 `[ffmpeg]` keys in all five catalogs, then `python3 scripts/subset-fonts.py`. Write t953's checks in `i18n_tests.rs`/`font_tests.rs`. Verify t953, t100, t125, t128 and t171 are green
  - 13 keys in all five catalogs, `subset-fonts.py` rerun; t953 (`i18n_tests.rs`): every new key in
    four languages has the English placeholders and no English left over (glyphs: t100). t953, t100,
    t125, t128, t171 green.
- [x] 2.4 Write t950, t951 and t952 in `src/apps/kt-app/src/reach_tests.rs` (harness, Windows rules forced, local server, temp download root). Verify they are green and `src/scripts/specs-user-path.py --check` (t799) accepts their PATH lines
  - t950 (before: both "what happens" lines, the hash start, 57 MB, Download and Open folder; Open
    folder before any download asks for the root and creates it), t951 (404 then a slow archive: the
    mirror line "127.0.0.1 — mirror 2 of 2", "Tried:" with 404, the bytes line, Cancel; the phase
    texts and "12 of 57 MB · 4.0 MB/s · about 11 s left" / the stall's dash checked on
    `progress_text`, the phases pass faster than a frame here), t952 (success: installed into + SHA-256
    matches, Open folder → the folder; failure: 404 + checksum mismatch listed, no traces, Open folder →
    the root). RED with Open folder hidden (t950 "no Open folder", t952 at the click). The spec's
    "nothing repaints at rest" is NOT asserted: the program always schedules its 250 ms idle redraw
    (t20), so egui reports a requested repaint every frame; t950 asserts no job runs at rest. t799
    green.
- [x] 2.5 Run the UI guards (`reach_tests.rs` incl. t757, `layout_tests.rs`). Verify they are green
  - kt-app 409 tests: green except t640 (load flake). The live coverage guard: `explorer.exe` was read
    as a place — `exe` joins the collector's file extensions.
- [x] 2.6 Commit stage 2 (`cargo test --workspace --no-fail-fast`, `cargo clippy -p kt-app --all-targets` with no new warnings)
  - Committed with 3.1; clippy clean on the touched files.

## 3. Live and Windows

- [x] 3.1 Live scenario `src/scripts/live/ffmpeg_download.py` on Xvfb: a debug build with `KT_FFMPEG_DOWNLOAD_URL`/`_SHA256`/`_SIZE` pointing at a local server serving the t848-shaped archive, and the Windows offer forced by the test seam. The archive comes from the t848 builder; if exposing that builder to the script costs more than this stage, the live run covers only the before state and the failed state (404, then size mismatch), and the success state stays with t952. Take one `kt_live.step` screenshot per PATH step of t950–t952. Verify `./scripts/live-check.sh ffmpeg_download` is green
  - `scripts/live/ffmpeg_download.py` (`OWN_PROCESS`): `KT_FFMPEG_OS=windows`, two local mirrors in
    `KT_FFMPEG_DOWNLOAD_URL` (space-separated, debug only): a 404 that answers after 3 s (a slow network)
    and one announcing 100 bytes against `KT_FFMPEG_DOWNLOAD_SIZE=1000`. The fallback of this task:
    the success state stays with t952 (the 7z writer is Rust). View → FFmpeg… shows the block; Download
    → the bar "127.0.0.1 — mirror 1 of 2, connecting…" with Cancel → the red "not installed" with both
    reasons and "Tried:" → Open folder in the row (hovered; a click would launch a real file manager).
    Green. Screenshots: `target/live-out/ffmpeg_download/01-step-before.png`, `02-step-connecting.png`,
    `03-step-failed.png`, `04-step-open-folder.png`.
- [ ] 3.2 `./scripts/windows-test.sh`: the kt-app tests under wine, including t949–t952. Verify they are green. The real-Windows click on the pinned gyan.dev archive (Explorer opening the folder) stays with the owner: name it in the report
- [ ] 3.3 Commit stage 3

## 4. Records and finish

- [x] 4.1 Append the "why" to `openspec/specs/media-index/design.md` (snapshot step, size check before the body) and `openspec/specs/ui-reach/design.md` (the window's three states, Open folder). Verify `openspec validate --specs` is green
  - media-index design: the snapshot step and the size check before the body; ui-reach design: the
    window's three states and Open folder. `openspec validate --specs --strict` 20/20.
- [ ] 4.2 Run `./scripts/finish.sh` from `src/`. Verify it is green, and record the version `vYY.MM.DD-hash` and the live screenshots here
