# Spec Delta

## ADDED Requirements

### Requirement: The FFmpeg window — state, Download, Locate, Rescan
*View → FFmpeg…* SHALL open a window that shows the search report (capability
`media-index`): the loaded FFmpeg's folder (or SONAME) and version, or "not found", and
every rejected candidate with its reason. On Windows the window SHALL offer **Download
FFmpeg 8.1.2 (gyan.dev, GPLv3)**, **Locate folder…** and **Rescan**. On Linux it offers
**Rescan** and the distribution hint. **Locate folder…** opens a folder picker. A folder
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
- **WHEN** the app starts with the FFmpeg search limited to an empty folder and a project with one video clip; the notice's **Locate folder…** gets a folder without DLLs, then a folder with a complete set; then *View → FFmpeg…* is opened. On Windows the complete set is the pinned gyan.dev build. On Linux the test's search seam reports that folder as complete and loads the system FFmpeg behind it, so the same UI path runs in CI on both OSes
- **THEN** before: the notice is shown once, the clip is a placeholder, the window lists the empty folder as rejected; the folder without DLLs is refused with "no avcodec-*.dll" and the choice is unchanged; after the complete folder: the clip shows frames without a restart, the window shows the folder and the loaded version, and the choice is in the saved settings
- **PATH** start → notice "FFmpeg not found" over the timeline, clip drawn as a placeholder → **Locate folder…** → folder picker → pick the folder with DLLs → notice closes, the clip's filmstrip and the preview show frames → View → FFmpeg… → window: folder path, "FFmpeg 8.1.2 (avcodec 62.x)", no rejected list except the empty folder
- **TEST** t850 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: UI language on Windows comes from the user's locale
On first start, when no language has been chosen, the UI language SHALL come from
`LC_ALL` → `LC_MESSAGES` → `LANG` when one of them is set and not empty (on any OS), and
otherwise on Windows from the user's default locale name (`ru-RU`, `zh-Hans-CN`, `es-MX`, …)
with the same mapping. The user's choice always wins.
Why: these variables are unset on Windows, so every Windows user started in English.

#### Scenario: Locale name of the OS when no variable is set
- **WHEN** detection runs with injected sources: no variables and OS locale `ru-RU`; no variables and `zh-Hans-CN`; `LANG=es_ES.UTF-8` and OS locale `ru-RU`; no variables and `fr-FR`
- **THEN** the languages are Russian, Chinese, Spanish (the variable wins) and none (English is used)
- **PATH** there is no gesture: the first start of `kinetren.exe` on a Windows account whose display language is Russian (no LANG set) shows the menu bar, toolbar hints and dialogs in Russian at once; after View → Language → English the choice wins on every next start
- **TEST** t861 (`src/apps/kt-app/src/i18n_tests.rs`)

### Requirement: Windows window behaviour matches Linux where the user can see it
On Windows a dropped file SHALL land where the pointer is when it is released over the
timeline (lane and time), as on Linux, and the drop silhouette SHALL follow the pointer.
The release build MUST NOT open a console window next to the program. The executable SHALL
carry the Kinetren icon and version in its resources.
Why: winit does not report the pointer position during an OLE drag on Windows, and the
non-Linux fallback returned "no position", so every drop landed at the playhead. A console
window and a generic exe icon are the first things a Windows user sees.

#### Scenario: Drop position, console and icon on real Windows
- **WHEN** on Windows a video file is dragged from Explorer onto the second video lane at about 10 s; the release `kinetren.exe` is started from Explorer; the exe is viewed in Explorer
- **THEN** the event lands on the second lane at the pointer's time and the silhouette followed the pointer; no console window appears; Explorer shows the Kinetren icon and the version in Properties
- **TEST** manual check on Windows recorded in `tasks.md` (the live harness runs only under Xvfb)
