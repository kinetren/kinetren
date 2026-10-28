# Design

## Context

See proposal.md, section Why. These facts were read in the code on 2026-10-28:

- **The downloader** (`src/apps/kt-app/src/ffmpeg_setup.rs`). `DownloadSource { urls, sha256,
  version }` (~67-90): the pinned one, or a debug-only test server
  (`KT_FFMPEG_DOWNLOAD_URL`/`_SHA256`). `download_and_install` (~103-130) writes
  `.download-<pid>.7z`, extracts into `.partial-<pid>` and renames it to `root/<version>`.
  `fetch_verified` (~134-207) tries the URLs in order, sends
  `Connecting(url)` and then `Bytes{done,total}` every 256 KiB read, hashes as the bytes
  arrive, and collects `"{url}: {e}"` per failed mirror, joined only at the end.
  `extract_dlls` (~211-252) counts written files but reports nothing.
- **The job** (`src/apps/kt-app/src/bg.rs`): `BgJob::poll` drains the channel and keeps only
  the last step (`:115-125`). `started: Instant` exists (`:78`).
- **The window** (`draw_ffmpeg_window`, ~433-560): the loaded line or "not found", the
  rejected candidates, the OS hint, the last message, `DOWNLOAD_NOTE` with the size, the
  progress bar (`PROGRESS` = "X of Y MB", `Connecting…`, `Verifying`, `Extracting`), and
  the buttons Download/Cancel, Locate, Rescan. It repaints every 100 ms only while a job runs
  (invariant 12). `poll` drops the installed path (`Ok(_)`). Strings are `[ffmpeg]` in
  `src/apps/kt-app/i18n/en.toml:1298-1323`.
- **ffmpeg-9** (archived 2026-10-27, its commits already in this branch): task 3.2 added
  `download_offered(os, pin_present)`, the cached `pin_present`, the window naming the loaded
  folder, and the "restart to use it" message for a download over a loaded FFmpeg. This
  change edits the same function and keeps those behaviours. The brief's path
  `t640-sigbus:openspec/changes/ffmpeg-9/tasks.md` does not exist any more. The change is at
  `openspec/changes/archive/2026-10-27-ffmpeg-9/`, and no branch holds unmerged ffmpeg-9 work
  (`git log HEAD..ffmpeg-9` and `HEAD..t640-sigbus` are empty).
- **Specs**: media-index "Downloading the pinned FFmpeg build" (`spec.md` ~478-521) requires
  that the source and the licence are shown before the press, plus progress, cancellation and
  no traces. ui-reach "The FFmpeg window" (`spec.md:580-609`) requires the window form contract
  (scrolling body, actions outside). Neither requirement changes. This change adds to them.
- **No "open folder" helper exists in kt-app.** The only `Command::new` is a test's `bash`
  (`util.rs:442`).

## Goals / Non-Goals

**Goals:**
- Everything the downloader knows reaches the window: source, mirror, sizes, speed, time
  left, hash, folder, the unpacking count, and every mirror's error.
- The person knows before the click what will be downloaded, from where, and where it will go.
- Testable on Linux, with no network and no real file manager.

**Non-Goals:**
- Changing the pin, the URLs, the hash or the search order (dist-check t860 stays).
- Resuming a broken download, choosing a mirror by hand, or a download on Linux.
- A system notification when the download finishes (the window and the bell are enough).

## Decisions

### D1. The step is a snapshot, not an event
`DownloadStep` becomes `DownloadProgress { phase, mirror: usize, mirrors: usize, url,
done, announced: Option<u64>, expected: Option<u64>, unpacked: usize, to_unpack: usize,
attempts: Vec<Attempt>, receiving_since: Option<Instant> }`. The job keeps only the latest
value, and the latest value now holds everything, so `BgJob` stays as it is (it is shared
with export and analysis). `attempts` has at most two entries, so cloning it per step costs
nothing measurable at 256 KiB per step.
Alternative: event-style steps with an accumulating `BgJob`. Rejected: `poll` drops every
step except the last by design, and changing that touches every background task.

### D2. Speed and time left are computed by the window from its own samples
`FfmpegUi` keeps a fixed ring of 16 `(Instant, done)` samples, taken in `poll` when the
bytes change. `rate(&samples, now) -> Option<f64>` is a pure function: bytes over the last
≤5 s of samples, `None` with fewer than two samples or when nothing arrived for 2 s.
`eta(done, total, rate)` gives `None` without a rate. The total is the announced size, then
the pinned size. Pure and allocation-free, so t948 tests it without a clock. The ring resets when the
mirror changes, so a failed first mirror does not spoil the rate.
Alternative: compute the rate from `BgJob.started`. Rejected: it averages in the connect
time and the failed mirror, so the rate would read low exactly when a mirror switch happened.

### D3. A wrong announced size skips the mirror before its body is read
`DownloadSource` gains `size: Option<u64>` (`PINNED_SIZE` for the pin, `None` for the test
server unless `KT_FFMPEG_DOWNLOAD_SIZE` is set in a debug build) and `archive: String`.
When Content-Length is present and differs from `size`, the attempt fails with "announced A
bytes, expected B" before the body is read. A file of another length cannot have the pinned
hash, so this saves up to 57 MB of download and says why. Without Content-Length, the hash
stays the only test, as now.

### D4. Unpacking progress counts what will be written
Before `for_each_entries`, `extract_dlls` counts the wanted entries (`bin/*.dll` and
`LICENSE*`) from the archive's entry list, and sends `unpacked`/`to_unpack` after each
written file. The 7z header is read before the solid stream, so the count is free.

### D5. Open folder through one helper with a test seam
`util::open_in_file_manager(path)` spawns `explorer.exe <path>` on Windows and `xdg-open
<path>` elsewhere, detached, with errors turned into the window's message. It is never
waited on. `FfmpegUi.test_open: Option<Vec<PathBuf>>` records the request in tests,
mirroring `test_pick`. The button opens the installed folder after a success, otherwise the folder
of the loaded FFmpeg when it is a folder (not a SONAME), otherwise the download root (created
if missing). On Linux with a system FFmpeg the button is hidden, because a SONAME has no folder to
open.

### D6. Windows rules in tests on any OS
`FfmpegUi.os: kt_ffmpeg::search::Os` (default `Os::NATIVE`) feeds `download_offered`.
The harness tests set it to Windows and point `download_root` and the source at their own
temp folder and local server, like t848 and t850. Tests do not write into the tree.

### D7. The window form
The "what happens" block and the attempts list live in the scrolling body. The progress bar and
the buttons stay in `window_actions`, as the form contract (R.66, t779) requires. The hash
is shortened to 16 hex digits with the full value in the hover, so the body does not widen
the 560 px window.

## Risks / Trade-offs

- [`xdg-open`/`explorer.exe` missing or failing] → The spawn error becomes the window's
  message, and the folder path is shown as text anyway, so the person can copy it.
- [A mirror that announces no size] → The hash still decides (unchanged), and the time left is
  computed from the pinned size, labelled "about".
- [More text in a small window] → The body scrolls (form contract). t779 guards the form, and t950
  checks that the buttons stay visible.
- [Changing `DownloadStep` touches t848] → t848 keeps its assertions (traces, hash
  message, extracted set). Only its step matching moves to the new snapshot.

## Migration Plan

No data migration. Rollback is a revert of the kt-app commits.
