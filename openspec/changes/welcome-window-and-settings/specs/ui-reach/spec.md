## MODIFIED Requirements

### Requirement: A welcome window at start; one primary tile lands on the most likely action
When the program starts without a project on the command line, without a recovery offer, and with
"Open the welcome window at start" on (the default), it SHALL open the welcome window over an empty
editor (when FFmpeg is not found, the FFmpeg window comes first and the welcome window opens when it
is closed). The window SHALL be a sized, resizable window that never opens larger than the screen, and
its body SHALL be three panes side by side, each with its own scroll that reaches the window's bottom:
Start — the primary tile, "New from template" (the built-in templates 1080p25, 1080p30, 4K30, 9:16
vertical 1080×1920 30, 1:1 square 1080×1080 30, each drawn with its frame's shape, followed by the
person's own templates), the tour tile ("Start the tour", or "Continue the tour — lesson N, step M"
once started) and the five lessons with their progress, each opening its lesson; Recent — the recent
projects, newest first, as poster cards (a placeholder until the poster exists) with name and folder,
as many per row as the pane's width holds; What's new — the notes of `docs/guide/whats-new.md`
compiled into the binary, from the running version's section (`ktYY.MM`, or the newest section with
its label when the running one has none) down to the oldest, drawn by the Guide's renderer. Scrolling
one pane SHALL NOT move another. Enlarging the window SHALL give the room to the panes. Exactly ONE
tile SHALL be primary — drawn larger and focused, so Enter takes it — and it SHALL be chosen by a pure
function of (is this the first start, the tour progress, the recent list with each file's existence):
the tour on the first start; the previous project when it exists; "Continue the tour" when a tour was
left unfinished and no project was opened since; a new project from the last-used template when the
previous project is missing (the missing row stays in the list, says where the file was, and offers to
locate it — it is not silently dropped). A click on a template SHALL create an unnamed project with
that frame size and rate in the same window, without a question unless there is unsaved work. The
What's new pane SHALL be headed by one calm line "Kinetren <version> is out — Releases page" when the
update-check state already on disk says a newer release exists; the window MUST NOT make a request of
its own. The window's action row SHALL hold the Guide, Preferences… and Close; the window SHALL NOT
carry a switch of its own: whether it opens at start is set in File › Preferences…, and Help › Welcome
window… reopens it at any time. Nothing in the window SHALL animate: it repaints on input only.
Why: the owner (2026-10-28) asked for an IDE-style intro window with templates, recent projects with
thumbnails, a tour tile, a primary tile for the most likely action, and What's new with an update
hint. F-HLP-027 had found that the only first-run instruction dies in the status line after 6 s; its
verdict "a start screen as a separate window — no" is revised by that request. kkt-26.11.01 item 1:
"Стартовое окно уродское … не ресайзится и в нем whats new суженный, и при скроле whatsnew скролится
всё подряд"; item 13: the start checkbox sat in the window AND in View (design D1, D6).

#### Scenario: The primary tile for each case
- **WHEN** the choice function is asked for: a first start (empty store); a start after the tour was
  finished with a recent project that exists; the same with the project file deleted; a tour stopped
  at lesson 2 with no project opened since; a tour stopped at lesson 2 and a project opened since; a
  project on the command line; a pending recovery offer; FFmpeg not found at start; "Open the welcome
  window at start" off
- **THEN** in order: the tour; the previous project; a new project from the last-used template (and
  the missing row is flagged, not removed); "Continue the tour — lesson 2"; the previous project; no
  welcome window; no welcome window (recovery first); the FFmpeg window first and the welcome window when
  it closes; no welcome window and the last project opens as before
- **PATH** there is no gesture: the person starts Kinetren and sees the window with the primary tile
  already focused — on the first start "Start the tour", later the previous project's poster tile
- **TEST** t984 (`src/apps/kt-app/src/welcome.rs`)

#### Scenario: The welcome window — templates, recent, What's new, the switch
- **WHEN** the harness starts with a user store holding two recent projects (one with a poster, one
  without) and one user template "Shorts 60"; then clicks the "9:16" tile; then reopens the window from
  Help and clicks the recent project; then turns "Open the welcome window at start" off in File ›
  Preferences… and restarts
- **THEN** the window shows the five built-in tiles, then "Shorts 60", the two recent cards (one
  poster, one placeholder) with names and folders, the tour tile, the Guide and Preferences… buttons and
  What's new for the running version, and no "show at start" checkbox; it lies inside the screen; the
  9:16 click gives an unnamed 1080×1920 30 fps project; the recent click opens that project; after the
  restart no window opens and the last project is open; no frame is drawn while the mouse is still
- **PATH** start Kinetren → the welcome window over an empty editor: on the left "Start" with the
  primary tile and the template rows "1080p25 · 1080p30 · 4K30 · 9:16 · 1:1 · Shorts 60", in the middle
  "Recent" with two cards (a frame of the project and a grey placeholder), on the right "What's new in
  kt26.11" → click "9:16" → the window closes, the preview is tall and narrow, the title says
  "Untitled" → Help › Welcome window… → click the first recent card → that project opens → File ›
  Preferences… → uncheck "Open the welcome window at start" → quit and start again → the last project
  opens directly, no window
- **TEST** t986 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Three panes, each scrolls alone, the window fits and grows
- **WHEN** the harness (1440×860) opens the welcome window over a store with twelve recent projects;
  the wheel turns over the What's new pane, then over the Recent pane; the window's corner is dragged
  toward 1900×1150 on a 1920×1200 screen
- **THEN** the window at its default lies inside the screen and its panes are side by side in the
  order Start, Recent, What's new, each reaching the action row; the wheel over What's new moves only
  What's new (the Start and Recent contents stay where they were), the wheel over Recent moves only
  Recent; after the drag the three panes are wider and taller, What's new is at least 300 points wide,
  and Recent holds more cards per row than before
- **PATH** start Kinetren → the welcome window: "Start" on the left, "Recent" with poster cards in the
  middle, "What's new" on the right → turn the wheel over What's new → only the notes move → turn it
  over Recent → only the cards move → drag the window's corner to the screen's corner → the cards take
  the width in more columns, the notes get a wider and taller column
- **TEST** t1290 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

#### Scenario: What's new is the page, drawn by the Guide's renderer
- **WHEN** the welcome window is opened from Help › What's new with an update-check state that names a
  newer release
- **THEN** the What's new pane paints the running version's notes with their headings and items as the
  Guide draws them (a `<kbd>` is a keycap, not angle brackets), older sections follow under their own
  headings, the pane is scrolled to its top, and its first line names the newer release and the
  Releases page
- **PATH** Help › What's new → the welcome window opens with the What's new pane at its top: "Kinetren
  kt26.12 is out — Releases page", then "What's new in kt26.11" and its items → scroll the pane down →
  the "kt26.10" notes follow
- **TEST** t1291 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

#### Scenario: Lessons and templates in the Start pane
- **WHEN** the tour has lesson 1 done and lesson 2 started; the welcome window is open
- **THEN** the Start pane lists five lessons with their titles, lesson 1 marked done and lesson 2
  showing its done steps out of its steps; a click on lesson 3 opens lesson 3's sandbox and its panel;
  each template row paints a frame whose width-to-height ratio is the template's
- **PATH** Help › Welcome window… → in "Start" the lessons "1 · The first edit ✓", "2 · Effects and
  automation 1/3", … → click "3 · Transitions" → the window closes, the lesson project opens and the
  lessons panel reads "Lesson 3 of 5 · Transitions"
- **TEST** t1292 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

#### Scenario: The project name answers where it is drawn, past the left column too
- **WHEN** inside a reel the header's project name (the root crumb) runs past the left column over
  the preview — the menu row now has four items — and the person clicks the part past the edge
- **THEN** the editor leaves to the project's root: the crumbs take their clicks on the title's own
  layer with the screen as the clip, so a click is not clipped to the column (found by the live
  scenario `nestdead` after Help joined the menu row)
- **PATH** enter a reel → the header reads "<project> › Reel", the project name running past the
  left column → click the part of the name past the column's edge → the editor is at the root
- **TEST** t984b (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: The FFmpeg window — state, Download, Locate, Rescan
*File › Preferences… › FFmpeg…* SHALL open a window that shows the search report (capability
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
once per session, and the window stays reachable from the Preferences window at any time. The window follows
the window-form contract (scrolling body, action buttons outside it).
Why: the owner's decision to find the user's install and offer the gyan.dev download
(2026-10-22) needs a place where a person sees why nothing plays and fixes it with the
mouse. A notice without buttons would be a dead end ("create, but can't manage", rounds
33–40).

#### Scenario: From the notice to playing media
- **WHEN** the app starts with the FFmpeg search limited to an empty folder and a project with one video clip; the notice's **Locate folder…** gets a folder without DLLs, then a folder with a complete set; then *File › Preferences… › FFmpeg…* is opened. On Windows the complete set is the pinned gyan.dev 9.0.2 build. On Linux the test's search seam reports that folder as complete and loads the system FFmpeg behind it, so the same UI path runs in CI on both OSes
- **THEN** before: the notice is shown once, the clip is a placeholder, the window lists the empty folder as rejected; the folder without DLLs is refused with "no avcodec-*.dll" and the choice is unchanged; after the complete folder: the clip shows frames without a restart, the window shows the folder and the loaded version, and the choice is in the saved settings
- **PATH** start → the FFmpeg window over the timeline: "FFmpeg not found — media shows placeholders", the empty folder under "Not used", the clip drawn as a placeholder → **Locate folder…** → folder picker → a folder without DLLs → red line "… cannot be used: no avcodec-*.dll" → **Locate folder…** → the folder with DLLs → the window's first line becomes "FFmpeg <major> (avcodec <x.y.z>) — <folder>" (on Windows "FFmpeg 9 (avcodec 63.x)"), the clip's preview shows frames → close the window → File › Preferences… → FFmpeg… → the same window with the loaded folder
- **TEST** t850 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Download stays offered over an earlier download
- **WHEN** the window's offer is computed (Windows rules) for: nothing loaded; FFmpeg 8 loaded from the download root's `8.1.2\` with no `9.0.2\`; FFmpeg 9 loaded from `9.0.2\`; FFmpeg 9 loaded from a chosen folder with no `9.0.2\`
- **THEN** **Download FFmpeg 9.0.2** is offered in the first, second and fourth case and not in the third; in the second the window's first line names the `8.1.2` folder; the computation is pure and runs on any OS
- **PATH** (Windows, an 8.1.2 download from an earlier version of Kinetren) start → media plays as before, no notice → File › Preferences… → FFmpeg… → the window's first line "FFmpeg 8 (avcodec 62.x) — …\kinetren\ffmpeg\8.1.2", below it **Download FFmpeg 9.0.2 (gyan.dev, GPLv3)**, **Locate folder…**, **Rescan** → **Download** → progress bar with size and **Cancel** → after it finishes the first line becomes "FFmpeg 9 (avcodec 63.x) — …\kinetren\ffmpeg\9.0.2" only after a restart (a loaded FFmpeg stays for the process), and the Download button is gone
- **TEST** t867 (`src/apps/kt-app/src/ffmpeg_setup.rs`)

### Requirement: UI language on Windows comes from the user's locale
On first start, when no language has been chosen, the UI language SHALL come from
`LC_ALL` → `LC_MESSAGES` → `LANG` when one of them is set and not empty (on any OS), and
otherwise on Windows from the user's default locale name (`ru-RU`, `zh-Hans-CN`, `es-MX`, …)
with the same mapping. The user's choice always wins.
Why: these variables are unset on Windows, so every Windows user started in English.

#### Scenario: Locale name of the OS when no variable is set
- **WHEN** detection runs with injected sources: no variables and OS locale `ru-RU`; no variables and `zh-Hans-CN`; `LANG=es_ES.UTF-8` and OS locale `ru-RU`; no variables and `fr-FR`
- **THEN** the languages are Russian, Chinese, Spanish (the variable wins) and none (English is used)
- **PATH** there is no gesture: the first start of `kinetren.exe` on a Windows account whose display language is Russian (no LANG set) shows the menu bar, toolbar hints and dialogs in Russian at once; after File › Preferences… → Language → English the choice wins on every next start
- **TEST** t861 (`src/apps/kt-app/src/i18n_tests.rs`)

## ADDED Requirements

### Requirement: File › Preferences… holds the program's preferences; View keeps what is viewed
File › Preferences… SHALL open one registered window with the program's preferences — per machine,
outside any project, reachable with no project open — in sections: Start ("Open the welcome window at
start"), Interface (Language: Auto (system) and the five languages by their own names; Economy UI),
Privacy and updates ("Send anonymous usage statistics", then "Check for new versions daily", each
disabled with its reason in the hover text when the build or the environment blocks it), FFmpeg (the
loaded version or "not found", and FFmpeg… opening the FFmpeg window). Each control SHALL act at once
(no Apply), with its hover text saying what it does. The View menu SHALL hold only what changes what is
viewed: Detach/Dock preview, Diagnostics…, Notices. The window follows the shared window rule
(resizable, its body fills it, Close in the action row).
Why: kkt-26.11.01 item 13 — "пора бы формировать раздел настроек в file, а то у нас в view уже срач из
разного"; answer 3 (2026-11-01): yes. Named by industry (rule 21): Resolve, Premiere, Vegas, Blender say
Preferences; "Settings…" beside "Project settings…" reads as one window twice (design D5).

#### Scenario: The Preferences window holds every preference and each acts
- **WHEN** the harness opens File › Preferences…, switches Economy UI on, picks Русский in Language,
  unchecks "Open the welcome window at start", unchecks the statistics and the daily check, and presses
  FFmpeg…
- **THEN** the window is `kt_prefs` from the registry with the four section titles; Economy UI caps the
  playing repaint at 30 fps; the menu bar reads in Russian on the next frame; the start switch is off in
  the store; both consents are off and stored; the FFmpeg window opens
- **PATH** File › Preferences… → a window "Preferences" with Start, Interface, Privacy and updates,
  FFmpeg → tick "Economy UI" → pick "Русский" → the menus read "Файл / Правка / Вид" → untick "Open the
  welcome window at start" → untick both privacy boxes → press "FFmpeg…" → the FFmpeg window opens over it
- **TEST** t1293 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

#### Scenario: View holds only what is viewed
- **WHEN** the View menu is opened in each of the five languages
- **THEN** it lists exactly Detach preview (or Dock preview), Diagnostics…, Notices — no language, no
  Economy UI, no statistics or update switch, no FFmpeg…, no welcome switch
- **PATH** open View → three items: "Detach preview", "Diagnostics…", "Notices"
- **TEST** t1296 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

### Requirement: One switch, one place
A preference SHALL be changed from exactly one place in the interface. A second checkbox, menu item or
field for the same value elsewhere (a window, a menu, a panel) is a defect, also when its label
differs. The texts that tell where a switch is (the statistics entry, the update hint, the FFmpeg
notice) SHALL name that one place.
Why: kkt-26.11.01 item 13 — "нафига мне галочка welcome window at start И в самом window и еще
продублирована в View?!?!"; the diagnostics change found the same class with Economy UI (a copy in the
perf box). The two start checkboxes had different labels, so the guard watches the writers (design D9).

#### Scenario: Each preference has one writer and the old places hold none
- **WHEN** the kt-app sources outside tests are scanned for the preference writers (the start switch,
  Economy UI, the language, the statistics consent, the update consent), and the harness opens the View
  menu and the welcome window
- **THEN** each writer is called from exactly one site, in the Preferences window's source; neither the
  View menu nor the welcome window paints any preference label; in every catalog the statistics entry,
  the update hint and the FFmpeg notice name File and Preferences…, not View
- **PATH** open View → no switch; Help › Welcome window… → no checkbox, a "Preferences…" button; File ›
  Preferences… → every switch, once
- **TEST** t1294 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

### Requirement: Preferences survive opening another project and a restart
The language choice and Economy UI SHALL be kept with the program's preferences in the user store, not
in the open project's app state: File → New, File → Open, a template, a recent card and a lesson SHALL
keep them, and the next start SHALL apply them. A language chosen by an earlier build (kept in eframe
storage) SHALL be taken once when the store has none.
Why: found by this change — `replace_with` rebuilt the app without the two fields, so after File → New
the stored language became "auto" and the next start came up in the system language.

#### Scenario: File → New keeps the language and Economy UI; the restart applies them
- **WHEN** Russian and Economy UI are chosen in Preferences; File → New; the store is read again as on a
  restart; a store without the language and an eframe value `es` are given to the start
- **THEN** after File → New the language choice is Russian and Economy UI is on; the reread store holds
  both; the start without a stored language resolves Spanish from the former value
- **PATH** File › Preferences… → "Русский", tick "Economy UI" → File › New → the menus stay Russian,
  Preferences still shows both → quit and start → Russian, Economy UI on
- **TEST** t1295 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)
