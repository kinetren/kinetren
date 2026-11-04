# Spec Delta

## ADDED Requirements

### Requirement: The FFmpeg downloader reports what it is doing, mirror by mirror
Every progress step of the pinned FFmpeg download SHALL be a complete snapshot of the
attempt, so that a reader who sees only the latest step loses nothing. A snapshot holds: the phase
(connecting, receiving, verifying, unpacking), the URL in use and its position among the
mirrors ("1 of 2"), the bytes received, the size the server announced, the size of the
pinned archive, the files unpacked so far and the files to unpack, and the list of earlier
mirror attempts with each one's own error. A server whose announced size differs from the
pinned archive's size SHALL be recorded as a failed attempt naming both sizes, and the next
mirror is tried before the body is read. The SHA-256 check stays the deciding test when no size is
announced. A successful download SHALL report the installed folder and the verified hash. A failed one
SHALL report every attempt separately. Cancellation, a break and a hash mismatch still leave the download folder
without traces (unchanged).
Why: `kkt.txt` 2026-10-27: "the FFmpeg download window is not informative — what speed, where it
installs, etc." The URL, the mirror fallback, the announced size, the install folder and the
per-mirror errors all existed in the downloader, but they were overwritten by the next step
or joined into one line at the end.

#### Scenario: Two mirrors, the first refused by size
- **WHEN** the downloader is given two local test mirrors: the first announces a Content-Length different from the expected size, the second serves the small test archive of t848; then the first returns HTTP 404 and the second cuts the connection; then both serve the right archive and the download is cancelled during unpacking
- **THEN** in the first run the steps name mirror 1 of 2 with its URL, then a failed attempt "announced A bytes, expected B" with no body read (the server saw no body sent), then mirror 2 of 2, received bytes rising to the expected size, verifying, unpacking n of m with m equal to the DLLs plus the licence of the archive, and the result names `<root>/<version>` and the verified hash; in the second run the error lists both mirrors with their own reasons (404, the break); in the third the folder is left without traces, as t848 requires
- **TEST** t949 (`src/apps/kt-app/src/ffmpeg_setup.rs`)
