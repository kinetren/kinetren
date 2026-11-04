# Spec Delta

## ADDED Requirements

### Requirement: The FFmpeg window tells what the download does, how fast, and where it went
Before the click on **Download FFmpeg <pin>**, the FFmpeg window SHALL say what will happen: the
archive's name and size; the source and its mirror; the check against the SHA-256 built
into Kinetren (shortened, the full value in a hover); that only the DLLs and the licence
are unpacked, and into which folder; that nothing else on the computer changes; and that
deleting the folder removes it. During the download the window SHALL show the host in use with
"mirror k of n", the received and total size, the speed and the time left (a rate smoothed
over the last seconds; a dash while nothing arrives), the verifying step, and "unpacking: n
of m files". After a success the window SHALL name the installed folder and the verified hash. After a
failure it lists one line per mirror tried, with that mirror's reason. An **Open folder** button
SHALL open the installed folder, or the folder of the loaded FFmpeg, in the system file
manager. The texts come from the five catalogs. The window repaints only while the
download runs (invariant 12), and the speed and time left are computed by a pure function of the
samples.
Why: `kkt.txt` 2026-10-27: "the FFmpeg download window is not informative — what speed,
where it installs, etc." The person clicked Download and saw only "Connecting…" and
"X of Y MB". The source, the mirror, the folder and the reason of a failure stayed inside
the program.

#### Scenario: Speed and time left are honest numbers
- **WHEN** the rate function is fed byte samples: steady 4 MB/s for 5 s; a burst then a 3 s stall; no announced total (the pinned size is used); the first sample only
- **THEN** steady gives 4.0 MB/s and the time left from the remaining bytes; the stall gives a dash for the speed and no time left (not an infinite or frozen value); without a total the pinned size is the total; one sample gives a dash; the function allocates nothing per frame
- **PATH** View → FFmpeg… → **Download FFmpeg 9.0.2** → the bar reads "12 of 57 MB · 4.0 MB/s · about 11 s left" → the network stalls → the speed shows "—" and the time left disappears instead of growing → bytes arrive again → speed and time left come back
- **TEST** t948 (`src/apps/kt-app/src/ffmpeg_setup.rs`)

#### Scenario: Before the click the window says what will happen
- **WHEN** the harness opens View → FFmpeg… with the Windows rules forced and no pin folder, the download root pointed into the test's own folder
- **THEN** the window shows the archive name `ffmpeg-9.0.2-full_build-shared.7z`, its size in MB, the hosts `www.gyan.dev` and `github.com` (as the mirror), the first 16 hex digits of the pinned SHA-256 (the full value in the hover), the folder `<root>/9.0.2`, and the line about removal; the **Download** and **Open folder** buttons are there; nothing repaints while the window rests
- **PATH** View → FFmpeg… → the window shows the current state, then the block "Download gets ffmpeg-9.0.2-full_build-shared.7z (57 MB) from www.gyan.dev (mirror: github.com), checks SHA-256 4d2060a8b34a940a…, unpacks only the DLLs and the licence into …\kinetren\ffmpeg\9.0.2; nothing else changes; delete the folder to remove it" → hover the hash → the full 64 digits → the buttons **Download FFmpeg 9.0.2**, **Open folder**, **Locate folder…**, **Rescan** are in the row below
- **TEST** t950 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: During the download the window shows source, speed and time left
- **WHEN** the harness starts the download from a local test server pair (the first mirror answers 404, the second serves the t848 archive slowly)
- **THEN** while bytes arrive the window shows "127.0.0.1 — mirror 2 of 2", "X of Y MB · S MB/s · about T left", and under "Tried:" the first mirror with "404"; then "Checking SHA-256…" and "Unpacking: n of m files"; the bar and the text move; **Cancel** is shown instead of **Download**
- **PATH** View → FFmpeg… → **Download FFmpeg 9.0.2** → the bar appears with "127.0.0.1 — mirror 1 of 2, connecting…" → a line "Tried: 127.0.0.1 … — 404" appears and the bar shows "mirror 2 of 2" → "1 of 3 MB · 0.5 MB/s · about 4 s left" grows → "Checking SHA-256…" → "Unpacking: 2 of 3 files" → the bar is gone
- **TEST** t951 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: After the download the window says where it went, or why it did not
- **WHEN** the harness download from the test server succeeds; then a second harness run where both mirrors fail (404, then a hash mismatch); in both runs **Open folder** is pressed
- **THEN** after the success the window shows "Installed into <root>/9.0.2", "SHA-256 … matches", and (with FFmpeg already loaded) the existing "restart to use it" line of ffmpeg-9; **Open folder** asks the file-manager seam to open `<root>/9.0.2`; after the failure the window lists both mirrors, each with its own reason (404; "checksum mismatch: expected …, got …"), the folder has no traces, and **Open folder** opens the download root
- **PATH** (continuing the previous path) the bar is gone → the window shows "Installed into …\kinetren\ffmpeg\9.0.2" and "SHA-256 4d2060a8… matches" → **Open folder** → Explorer opens the folder with the DLLs and the licence → (another run, no network) **Download** → after a while red "FFmpeg was not installed:" with two lines "www.gyan.dev — …" and "github.com — …" → **Open folder** → the download folder, without leftovers
- **TEST** t952 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The new texts are in every catalog and every font
- **WHEN** the catalog and font guards run over the new `[ffmpeg]` keys
- **THEN** all five catalogs have every key with the same positional placeholders, no extra or unused keys, every non-ASCII glyph is in the loaded fonts, and t100, t125, t128 and t171 stay green
- **PATH** View → Language → Arabic (then Chinese, Russian, Spanish) → View → FFmpeg… → the "what happens" block, the progress line and **Open folder** are in that language with no tofu boxes and no English left over
- **TEST** t953 (`src/apps/kt-app/src/i18n_tests.rs`, `src/apps/kt-app/src/font_tests.rs`)
