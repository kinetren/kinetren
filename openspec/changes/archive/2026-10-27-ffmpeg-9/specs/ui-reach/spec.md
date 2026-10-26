# Spec Delta

## MODIFIED Requirements

### Requirement: The FFmpeg window — state, Download, Locate, Rescan
*View → FFmpeg…* SHALL open a window that shows the search report (capability
`media-index`): the loaded FFmpeg's folder (or SONAME) and version, or "not found", and
every rejected candidate with its reason. On Windows the window SHALL offer **Download
FFmpeg 9.0.2 (gyan.dev, GPLv3)**, **Locate folder…** and **Rescan**. On Linux it offers
**Rescan** and the distribution hint. **Download** stays offered while the current pin's folder is absent, also
when an earlier download (`8.1.2\`) is the loaded set; the window then names that folder. **Locate folder…** opens a folder picker. A folder
holding a complete supported set is saved as the user's choice and loaded at once; any
other folder is refused with the reason, and the previous choice stays. The download shows
its progress, size and **Cancel** in the same window, and after it finishes the window shows
the new state without a restart. When FFmpeg is not loaded at start, the program SHALL show
one notice ("FFmpeg not found — media shows placeholders") with the same buttons, at most
once per session, and the window stays reachable from the View menu at any time. The window follows
the window-form contract (scrolling body, action buttons outside it).
Why: the owner's decision to find the user's install and offer the gyan.dev download
(2026-10-22) needs a place where a person sees why nothing plays and fixes it with the
mouse. A notice without buttons would be a dead end ("create, but can't manage", rounds
33–40).

#### Scenario: From the notice to playing media
- **WHEN** the app starts with the FFmpeg search limited to an empty folder and a project with one video clip; the notice's **Locate folder…** gets a folder without DLLs, then a folder with a complete set; then *View → FFmpeg…* is opened. On Windows the complete set is the pinned gyan.dev 9.0.2 build. On Linux the test's search seam reports that folder as complete and loads the system FFmpeg behind it, so the same UI path runs in CI on both OSes
- **THEN** before: the notice is shown once, the clip is a placeholder, the window lists the empty folder as rejected; the folder without DLLs is refused with "no avcodec-*.dll" and the choice is unchanged; after the complete folder: the clip shows frames without a restart, the window shows the folder and the loaded version, and the choice is in the saved settings
- **PATH** start → the FFmpeg window over the timeline: "FFmpeg not found — media shows placeholders", the empty folder under "Not used", the clip drawn as a placeholder → **Locate folder…** → folder picker → a folder without DLLs → red line "… cannot be used: no avcodec-*.dll" → **Locate folder…** → the folder with DLLs → the window's first line becomes "FFmpeg <major> (avcodec <x.y.z>) — <folder>" (on Windows "FFmpeg 9 (avcodec 63.x)"), the clip's preview shows frames → close the window → View → FFmpeg… → the same window with the loaded folder
- **TEST** t850 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Download stays offered over an earlier download
- **WHEN** the window's offer is computed (Windows rules) for: nothing loaded; FFmpeg 8 loaded from the download root's `8.1.2\` with no `9.0.2\`; FFmpeg 9 loaded from `9.0.2\`; FFmpeg 9 loaded from a chosen folder with no `9.0.2\`
- **THEN** **Download FFmpeg 9.0.2** is offered in the first, second and fourth case and not in the third; in the second the window's first line names the `8.1.2` folder; the computation is pure and runs on any OS
- **PATH** (Windows, an 8.1.2 download from an earlier version of Kinetren) start → media plays as before, no notice → View → FFmpeg… → the window's first line "FFmpeg 8 (avcodec 62.x) — …\kinetren\ffmpeg\8.1.2", below it **Download FFmpeg 9.0.2 (gyan.dev, GPLv3)**, **Locate folder…**, **Rescan** → **Download** → progress bar with size and **Cancel** → after it finishes the first line becomes "FFmpeg 9 (avcodec 63.x) — …\kinetren\ffmpeg\9.0.2" only after a restart (a loaded FFmpeg stays for the process), and the Download button is gone
- **TEST** t867 (`src/apps/kt-app/src/ffmpeg_setup.rs`)
