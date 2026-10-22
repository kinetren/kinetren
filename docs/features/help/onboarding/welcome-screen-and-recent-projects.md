---
id: F-HLP-027
title: Start screen: recent projects, templates, entry to learning
status: candidate
priority: B
cost: S
category: help/onboarding
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, vscode-1.136, jetbrains-2026.2]
we_have: no
crates: [kt-app]
invariants: [12]
ui_rules: [9, 13, 18, 19, 20]
round: null
point: null
tests: []
depends_on: [F-HLP-009, F-HLP-012, F-HLP-020, F-HLP-033]
trackers: [kde-invent, kde-bugzilla, pitivi, olive, openshot]
checked: 2026-09-08
---

# F-HLP-027 — Start screen: recent projects, templates, entry to learning

## Essence

What the editor sees in the first second after launch, when there is no project yet: a list of what they worked on (file name and when), a way to start a new project from a template, a way to open someone else's, and one door into training. This is not a showcase of the product's news but an answer to the only question of the first second — "where is what I was doing yesterday". Everything else on this screen is an add-on, and precisely the add-on is what makes it something people turn off.

## Who has it and how

The question of the row: what stands in place of the empty window AT LAUNCH (not in the empty panel inside an already open project — that is F-HLP-020) and what the program does when it was launched without arguments.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 b66 | opening splash screen + "open last project" | Two independent answers to one question, and they fight. There is a start splash screen that can be switched off, and a separate checkbox as the first item of `Options → Preferences → General`, which opens the last saved project. Verified on the vendor's forum on build 143 (August 2026): with the splash screen ENABLED, the checkbox stops working. Separately there is `VEGAS Hub` — a panel of stock media (Storyblocks); it has nothing to do with startup, and with a perpetual license some of its functions are not shipped |
| Adobe Premiere | 26.x | Home screen | A separate window over an empty application: the list of recent projects, "new", "open" and a training tab; part of the area is given to Adobe materials. **[NOT VERIFIED: `helpx.adobe.com` returns 404/422 via the text proxy, `community.adobe.com` and `reddit.com` do not open from this environment — see "Sources"]** |
| DaVinci Resolve | 21 | Project Manager | There is never an empty edit at startup. According to the manual: "For most users, Project Manager is the first window you'll see when you open DaVinci Resolve", and "Unlike other applications that rely on your file manager for organizing projects, DaVinci Resolve requires you to do most project organization in the Project Manager" — the project lives in a library, not as a file, so the start screen here is not a greeting but a mandatory step. The way back is named: the Home button at the lower right or `File > Project Manager` (`Shift-1`). The conflict with "open last" is resolved by the setting `Reload last working project on startup`, but it is switched on only inside a preset of user settings |
| Final Cut Pro | 12.3 | none | There is NO start screen at all, and this is a conscious choice: according to the manual, "The first time you open Final Cut Pro, it creates a library file in the Movies folder" — the program creates the library itself, and afterwards the libraries stay open between launches. "Recent" items exist, but in a submenu: `File > Open Library`, "then choose a library from the submenu" |
| After Effects | 26.x | Home screen | The same Adobe mechanism as in Premiere: a common home screen with the list of recents and training materials. **[NOT VERIFIED: the same `helpx.adobe.com` barrier]** |

**Outside the core.** Without these three programs the feature cannot be described: the first one's start screen is the benchmark for the composition (templates + recents + session restore in ONE place) and the benchmark for the price of closing it; the second one's decision is designed not as a checkbox but as an enumeration of "what to fill the empty slot with", which answers rule 13 exactly; the third one's list of recents has grown into its own object with groups and icons. Of the five core NLEs only Resolve reaches the third item (folders in the Project Manager), while the first two are held by nobody: neither an explicit session-restore button next to the recents, nor a decision on "what occupies the empty slot" instead of a checkbox.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Splash Screen | The lower half of the splash is an "interactive area": `New File` (templates), `Recent Files`, `Open`, `Recover Last Session`, `What's New`, `Donate to Blender`. It is closed by clicking anywhere in the window outside it or with `Esc` — there is no "OK" button at all; it is brought back via the Blender icon in the Topbar → `Splash Screen` |
| Visual Studio Code | 1.136 | Welcome page | Not a checkbox, but an enumeration of seven values of `workbench.startupEditor`: `none`, `welcomePage`, `readme`, `newUntitledFile`, `welcomePageInEmptyWorkbench`, `terminal`, `agentSessionsWelcomePage`. According to the description — "Controls which editor is shown at startup, **if none are restored from the previous session**": the screen occupies a slot that would otherwise be empty |
| IntelliJ IDEA | 2026.2 | Welcome screen | The only window when no project is open. The list of recent projects is not an MRU tail but an object: right click → `New Project Group`, arbitrary grouping, and its own SVG icon per project ("Set Custom Project Icon") |

## The trick behind it

**What the mechanism carries that one status-bar line cannot hold:** a list of OTHER PEOPLE'S NAMES. A line carries one sentence of ours; the start screen carries N lines, each written by the user (the name of his file), clickable, and carrying a date. Neither the capacity of a line nor its text can replace this in principle: to hover the mouse over "yesterday's project", it has to be drawn somewhere.

Three details that are absent from the lists of features.

1. **The list of recent items is the only reference surface whose text we do not write.** File names are not translated, are not counted against the 160-character limit of `src/apps/kt-app/build.rs`, do not require new glyphs, and do not pass through t125/t131. The whole localization cost from F-HLP-033 barely applies to it: one has to translate the heading and two verbs, not the content. This makes it the cheapest "help" in the project — and the only one that the user WAITS to see, rather than endures.
2. **Blender makes session recovery a CHOICE, not a behavior.** On the splash, `Recover Last Session` stands as a separate line next to `Recent Files` — that is, the emergency autosave is visible and is clicked by hand. Ours is the same autosave (`src/apps/kt-app/src/app.rs:4908`), which is substituted silently and only in third priority; the user is told about it by a yellow line that lives for six seconds.
3. **The start screen and "open the last project" are two answers to one question, and they always fight.** In VEGAS 2026 build 143 this is a bug: an enabled splash breaks the checkbox. In Resolve 21 this is an acknowledged setting `Reload last working project on startup`, but it can be enabled only "when editing a preset configuration in the Presets panel" — that is, the decision about the first screen is hidden a floor below the settings. We have no fight precisely because there is no second answer: we always silently open the last one.

## Pros

- The list of recent items answers "where is yesterday's work" without a file dialog: the program remembers the path, the user remembers only the name. This is the only way to return to work without knowing where you put it.
- The start screen is the only moment when the program has the user's attention and there is NO work that can be spoiled: there is nothing to overlap, nothing to disrupt with a gesture, so what is forbidden in the working window fits here.
- It teaches without a tour: the user comes to this screen DELIBERATELY (for his file) — so everything lying next to it is read along the way, for free.
- A project template = a set of settings (fps, size, set of tracks) in one click. For us the settings of a new project are hard-coded as the constant 1280×720@30 (`src/apps/kt-app/src/app.rs:333`), and there is nowhere to change them at creation.
- A separate "restore session" line makes the emergency autosave visible: the file on disk stops being the program's secret.

## Cons

- A screen that must be CLOSED before work takes a second on every launch, and this is the only reason all vendors give a way to turn it off. Blender pays the least: the splash closes by a click anywhere, that is, by the very movement with which the user starts working anyway.
- Being switchable creates a debt under rule 19: having switched it off, one has to say where to turn it back on (F-HLP-009). The way back is written in the manuals by two: Blender (the icon in the Topbar → `Splash Screen`) and Resolve (the Home button or `File > Project Manager`).
- **The start screen is the most expensive place in a program for marketing, and for some vendors it has become exactly that.** This is visible not from complaints but from the sources: the value `agentSessionsWelcomePage` replaces the start page of VS Code with a promo page for a new product, applies only while the user has not hidden the corresponding panel (`chatEntitlementService.sentiment.hidden`), and by its own description "Will override the workbench secondary side bar visibility settings" — that is, it overrides the layout settings. The default value itself is marked `'experiment': { mode: 'auto' }`: what your first screen is occupied with is decided by the vendor's experiment. Blender has a soft version of the same thing — the line `Donate to Blender` on the splash.
- The list of recent items grows by itself (rule 20 literally): without a limit, an order, and a way to clear it, it turns into garbage, and on a shared machine into a list of other people's projects.
- The list shows what no longer exists: the line stays, the file was deleted or lies on a disconnected disk.
- **A separate window brings a class of defects that an editor without one does not have, and the trackers name it explicitly.** Not "the edit does not assemble", but: the window does not rise above other applications and is not visible in `Alt+Tab` (kde-invent #2124, open since 10.01.2026), the splash is not of fixed size and breaks tiling WMs (kde-bugzilla #513581), a crash when idle (kde-invent #2112), focus and modality (pitivi #1066, #1672), "the viewer closes the assistant" (pitivi #1575). None of these tasks is about editing — this is the price of a second window as such, and it is paid on other people's window managers, where we are not present.
- **A list of recent items on a shared machine is a list of other people's projects, and this is not speculative.** In an OpenShot build of January 2016 a user found the MRU tail of the developer himself (openshot #22): the program contained a list of someone else's personal files, and not one line reported an error when the file did not exist.
- **A list row is also a path to a crash.** For kdenlive, half of the tasks about "recent" are crashes exactly when a list row is opened (#430183, #455595, #455834, #519313); the list does in one click what used to require a file dialog, so broken projects get opened more often and without a warning.

## What users say

> "I have found that if the opening splash screen is turned off the previous
> file will open. If the splash screen is set to on the file will not open."
> — lqbbusse, Boris FX forum / VEGAS Pro, 23.08.2026,
> https://forum.borisfx.com/t/open-last-saved-project-doesnt-open-last-saved-project/25165

The thread began with "My preference is set to always open last saved project, but Vegas always just opens a new project" (21.08.2026, VEGAS Pro 2026 build 143); a second participant asked to confirm that the "first item" in `Options/Preferences/General` was checked, and could not reproduce it — the cause was found two days later, and it turned out to be the start screen.

About the vanished rows of the list — from the same ecosystem (Silhouette, Boris FX):

> "So I tried some projects in my 'recent projects' lists, and some would load
> (allowing a media drop) and some would not (because they would fail to load
> for an unknown reason)."
> — sam.marrocco, 01.01.2023,
> https://forum.borisfx.com/t/feature-suggestion-no-project-open/15592

Further in the same message: the projects had been deleted from the disk, and the only sign of failure was the absence of a name in the window title. This is exactly the failure that a list of recent items creates if it does not respond to a missing file.

The thesis "the start screen is the most expensive place for marketing" no longer rests only on the sources of VS Code. Kdenlive built its own start screen: the request has been open since November 2023, with 56 comments; the implementation arrived in the dev build 25.11.70 and shipped in the 25.12 release, and the task is still not closed — and its description names exactly what occupies the bottom strip of the screen:

> "When 3 or more projects appear in the list of recent files, a bottom bar
> displays a button to get involved and a button redirecting to our fund page."
> — kde-invent #1787 "Feature : Add a welcome screen", task description (filed
> 12.11.2023, description updated 03.04.2026),
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1787

The threshold for showing the fundraising panel is "three projects in the list": the first second's area is monetized even where there is nothing to sell. The same task names what rule 20 requires — the list must be able to be cleaned:

> "Both list of recent elements (files / profiles) allow removing single
> elements from the list or entirely clearing history."
> — the same task, the same URL.

What happens when a list is neither cleaned nor checked is visible from the OpenShot build of January 2016, in which a user found the developer's own MRU tail:

> "It does not bother me the least but I'd just like for you to know about this.
> […] And BTW, there's no error message when I try to load any of the
> non-existing project files."
> — openshot #22 "Menu: File --> Recent Projects contains a list of your
> (Jonathan) recent projects", 12.01.2016,
> https://github.com/OpenShot/openshot-qt/issues/22

SIGNAL: the start screen as a SEPARATE WINDOW — 14 tasks in pitivi, `welcome (screen|dialog|page|window)|start.?up (wizard|assistant)`; the share is 0.5% against 0.2% in kde-invent (4 of 2217)

What is counted is not "how many times it was asked for" but what the ownership cost. Fourteen pitivi tasks are the biography of one window: focus (#1066), button width (#1261), modality (#1672), "make it non-dismissable" (#1382), the viewer closing the assistant (#1575), a core dump when creating a project (#1803) — and the outcome #1302 "Integrate the welcome / startup assistant dialog in the main window", that is, an editor that built its start screen as a separate window ended up pulling it into the main one. kdenlive's own four came in during the first half-year of the screen's life, from the dev build 25.11.70 to June 2026: a click on a recent row opened the wrong project (#2092, 10.11.2025), the window does not rise above other applications and does not appear in `Alt+Tab` (#2124, 10.01.2026, open), the splash crashes when idle (#2112, 18.12.2025), the profile name sits under the scrollbar (#2202, 05.06.2026).

SIGNAL: the list of recent items as a subject of tasks — 8 tasks in kde-bugzilla, `recent(ly)? (used |opened )?(project|file|document)s?|open recent|clear.{0,12}(recent|history)`; the share is 0.1% against 0.2% in olive (4 of 1853)

Half of the eight are crashes exactly at the moment of opening a list row (#430183, #455595, #455834, #519313), and none of the four is closed as FIXED: INVALID, UPSTREAM, WORKSFORME or UNCONFIRMED (see the rule "closed ≠ fixed", `_trackers.md`). Separately, #499544 "Clear Recent History not working" — the clearing that rule 20 requires — is also broken. Of olive's four tasks, one is "[UI] Open Recent doesn't work" (#1243), and the second is a refusal to open a recent project when the path contains diacritics (#954); the third (#180, "Compilation broken due to recent Project UI removal") is a false positive on the word "recent", and the 0.2% share includes it: a filter by titles is never clean, which is why it is quoted verbatim.

About the price of the second form of answer — not as a file but as a window. Also, a live review of the Adobe start screen as an advertising platform is still absent: `helpx.adobe.com` returns 404 and an HTTP/2 error through the text proxy, `community.adobe.com` and `old.reddit.com` answer with 403. For the thesis this is no longer critical — it was closed by the kdenlive tracker.

## How it fits Kinotrain

### What already exists

The startup restore order is at `src/apps/kt-app/src/app.rs:5270-5287`: a command-line argument → `storage.get_string("last_project")` → `.filter(|p| p.exists())` → the autosave file. The storage is eframe with the `persistence` feature (`src/Cargo.toml:42`), physically `~/.local/share/kinotrain/app.ron`: a flat RON map of strings, in which today exactly two of our keys live, `lang` and `last_project` (written at `app.rs:5182-5185`), plus three service keys of egui. **There is no list of recent items: exactly one path is stored, and it is invisible** — it cannot be viewed or chosen, it is only silently opened.

If nothing is found, `KtApp::new_empty(Some(tr(k::msg::OPEN_A_PROJECT)))` is built (`app.rs:5287`). The argument goes into the field `status` (`app.rs:481`), and `status_line` (`app.rs:4426`) gives it as `HintKind::Status` — in yellow, `Color32::from_rgb(255, 210, 120)`, with exactly `STATUS_TTL_MS` = 6000 ms (`app.rs:4096`).

**To put it plainly: the only instruction of the first launch destroys itself after six seconds — and that is the best case.** In `status_line` the order is "error → hover → mode → message → idle line", so `hover` stands ABOVE `status` (`app.rs:4439`): the first movement of the mouse over any button erases the invitation from the screen before its time. And after six seconds, in its place stands `hint::IDLE` — "drop a media file on the window — the clip lands under the mouse", that is, text about adding MEDIA, not about how to open a project. A user who has stepped away for tea will not see the instruction at all.

Other state, checked by the code:

- An empty project is not an empty window. `empty_project()` (`app.rs:333`) gives 1280×720@30, one video and one audio track; the editor is drawn whole. The place at startup is NOT empty for us — this is the key fact for rule 13.
- `File → New` (`app.rs:2717`) is `*self = Self::new_empty(None)`: there is no prompt at all, and no question about unsaved changes either.
- `last_project` is written only when `project_path.is_some()` (`app.rs:5183`), so after `File → New` the PREVIOUS path remains in the storage, and the next launch will open not what the user finished with.
- `autosave_path()` (`app.rs:4908`) — `~/.local/share/kinotrain/autosave-project.otio`, our analog of `Recover Last Session`. It is substituted ONLY if `last_project` is empty or the file has disappeared; with a living `last_project`, an unnamed autosave is never offered and lies on disk invisibly.
- The `File` menu (`src/apps/kt-app/src/toolbar.rs:246-277`): New · Open… · Import .ktproj… · Save · Save As… · Render to MP4… · Quit. There is no "Recent" item.
- The machinery of "the file went missing / the file came back" is already written for media: `src/libs/kt-media/src/placeholder.rs`, three levels of the message and the return of the source (F-HLP-012).

### What is missing

A list instead of one path; a visible place where it lies, and a way to clear it; a first-second surface that does not die on a timer; templates for a new project; an explicit "restore session" instead of a silent third fallback.

### What gets in the way

**The storage format gets in the way not at all.** `app.ron` is a flat map of strings; a key `recent` with N paths costs one line in `save()` and one in `run()`. The project format (`kt-otio`) is not touched at all: no new version of the kt section is needed, there is no migration, and old `.otio` files are read as they were read before.

What gets in the way is the behavior when a file goes missing. Today `.filter(|p| p.exists())` (`app.rs:5274`) silently drops a vanished `last_project` into the autosave, and the person gets "Restored the autosave of an unnamed project" instead of "your project was not found there". A list of recent items makes this silence visible: the row is there, the file is not — and the row cannot be struck out, because the disk may simply be disconnected. The answer must be the same one we give for a missing source (F-HLP-012, `src/libs/kt-media/src/placeholder.rs`): name the path and offer to return the file.

What gets in the way is `File → New`, which replaces the whole application without asking (`app.rs:2717`). The start screen would place the "new" button right next to the list of recent items, and the cost of a miss there is higher than in the menu.

It also constrains invariant 12: repaint only on movement, in draw paths no allocations and no clones. The start screen must be a static picture for us — no carousels, no animated tours, no "what's new" with a network request.

### Which invariants it affects

Invariant 12 (see above) and the checklist rules 9, 13, 18, 19, 20.

- **13 — do not waste space.** The argument "the start screen does not waste space, it stands in place of an empty window" does NOT work for us: at launch the place is not empty; there is a full editor with two tracks (`app.rs:333`). A modal window over it is pure waste. And the submenu "Recent" in the `File` menu and a long-lived line in the empty timeline waste nothing.
- **19 — say where to get it back.** Everything that can be switched off must name the way back; for us this rule is not met by a single line (F-HLP-009). Introducing a switchable screen without closing F-HLP-009 means introducing a new debt.
- **20 — nothing grows by itself.** A list of recent items grows by definition, so it must have a limit, an order, and "clear", otherwise the rule is violated literally on the first day.
- **18 — the menu item says WHAT and WHERE.** "Recent" without a path does not do: the submenu carries the file name, and the full path goes into our hint line on hover (F-HLP-001), that is, via a pull channel, without a popup.
- **9 — nothing superfluous.** Everything on the start screen that is not the list of recent items and not "open" must prove its right to the area; with vendors it is exactly here that advertising takes hold.

ADDRESS: `src/apps/kt-app/src/app.rs` — the class "a row leads nowhere, and the program stays silent" (openshot #293 "Open recently silently fails if path/file don't exist", pitivi #2349) is POSSIBLE for us and is not caught by anything: the startup restore order contains `.filter(|p| p.exists())`, and a vanished `last_project` silently falls through to `autosave_path()` — the user gets "Restored the autosave of an unnamed project" instead of the name of the missing file. No name, no path, no error. Tests t194 and t199 (`src/apps/kt-app/src/layout_tests.rs`) guard the background nature of the autosave and the non-return of the revision, but no test looks at the choice of source at startup: today one branch is silent; with a list of recent items, N will be silent.

The second class — "a separate window" (kde-invent #2124, pitivi #1672, #1575) — is POSSIBLE for us, and this must be named directly: a top-level window above the main one already exists in `kt-app` exactly once — the detached preview (`src/apps/kt-app/src/app.rs:1014`, `ctx.show_viewport_immediate` on `ViewportId::from_hash_of("kt-preview-window")`), and the user opens it himself by his own command. A start screen would become a second such window, appearing WITHOUT a request, at every launch and before the main one — that is, the whole account of #2124 (does not rise above other applications, not visible in `Alt+Tab`) and
#513581 (tiling WMs) would move to us in full. This is an argument for a submenu and a long-lived line inside the single window, not for a modal over it.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage: a list of recent items + a first line that does not die. A full start screen with templates is separate and more expensive (M) |
| Where the main work is | kt-app: `app.rs` (`run()`, `save()`, the state "no project"), `toolbar.rs` (the File menu), the catalogs `apps/kt-app/i18n/*.toml` |
| Project format | not touched: the storage lives outside the project, in `app.ron` |
| Tests to write | `reach_tests::first_run_instruction_outlives_the_status_line`; a harness test of the "Recent" submenu (the items are reachable, a missing file gives an error, not silence); t125/t100 on the new catalog keys |
| Depends on | F-HLP-009 (the way back), F-HLP-012 (a missing file), F-HLP-020 (the empty container), F-HLP-033 (the cost of a line) |
| Breaks compatibility | no |

## Verdict

We take the smaller half: a list of recent items (N instead of one invisible path) and a first line that does not die on a timer. A start screen as a separate window — no: at launch we do not have an empty window but a drawn editor, and a modal over it violates rule 13 in the same direction as vendors' advertising does. The list of recent items, meanwhile, is the cheapest reference surface of all thirty-three notes of the wave, because its text is written by the user.

TEST: reach_tests::first_run_instruction_outlives_the_status_line — fails today, because the only instruction of the first launch (`app.rs:5287`, `msg::OPEN_A_PROJECT`) arrives as `HintKind::Status` and after `STATUS_TTL_MS` = 6000 ms is replaced by `hint::IDLE` about dragging media, and any mouse hover erases it even earlier (`app.rs:4439`).

DEFERRED: until F-EXP-001 — the templates of a new project are the fifth resident of the same "user storage outside the project" as the export presets, FX presets, title templates and key layouts (the conclusion no. 2 of the first wave, `docs/features/README.md`). The list of recent items enters `app.ron` without this subsystem; the templates do not.

## Sources

- https://docs.blender.org/manual/en/latest/interface/window_system/splash.html
  (access date 2026-09-07) — the composition of the splash and the way to bring it back.
- https://github.com/microsoft/vscode — `src/vs/workbench/contrib/welcomeGettingStarted/browser/gettingStarted.contribution.ts`, the definition of `workbench.startupEditor` and the branch `agentSessionsWelcomePage` (access date 2026-09-07).
- https://www.jetbrains.com/help/idea/open-close-and-move-projects.html
  (access date 2026-09-07) — project groups and icons on the Welcome screen.
- https://forum.borisfx.com/t/open-last-saved-project-doesnt-open-last-saved-project/25165
  (21-23.08.2026) — the splash versus "open the last project".
- https://forum.borisfx.com/t/feature-suggestion-no-project-open/15592
  (01.01.2023) — deleted projects in the list of recent items.
- https://forum.borisfx.com/t/vegas-hub/22553 (18-25.04.2026) — what VEGAS Hub is and why it is not the start screen.
- https://support.apple.com/guide/final-cut-pro/welcome/mac — Final Cut Pro User Guide for Mac (Apple Inc., PDF 028-00843), the sections on libraries and `File > Open Library`.
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
  (access date 2026-09-07) — the chapters "The Project Manager", "Using the Project Manager", "UI Settings" (the setting `Reload last working project on startup`).
- Trackers (export of 08.09.2026; the dumps are in `~/.cache/kinotrain/trackers/` and do not go into the repository — `_trackers.md`, the section on licenses):
  `kde-invent` #1787 (the request and the description of the implementation), #2092, #2112, #2124, #2202;
  `kde-bugzilla` #361395, #430183, #455595, #455834, #499544, #504597, #513581, #519313;
  `pitivi` #820, #984, #985, #1066, #1261, #1302, #1382, #1575, #1672, #1803, #2183, #2349;
  `olive` #954, #1243, #2330; `openshot` #22, #293.
  Both `SIGNAL:` line numbers are reproduced by `python3 src/scripts/tracker-rank.py --query '<filter>'` with the same filters.
- **State of the export as of 08.09.2026:** `blender-vse` and `ffmpeg-trac` are empty; `openshot` has an incomplete dump (the state file shows a restart from the first page with 4516 tasks recorded out of about 4907), therefore openshot does not take part in the shares of the `SIGNAL:` lines — only `pitivi` 2672, `kde-invent` 2217, `kde-bugzilla` 6346 and `olive` 1853, whose exports are marked `done`. The openshot tasks #22 and #293 are quoted by name: their existence does not depend on the completeness of the dump, the share does.
- No live check was made for any program; for software outside the core it is not expected (`docs/features/_editors.md`, the section "Software outside the core").
- Unavailable from this environment: `helpx.adobe.com` (404 and an HTTP/2 error through `r.jina.ai`), `community.adobe.com` (403), `reddit.com`/`old.reddit.com` (403), the search engines yahoo/duckduckgo/mojeek (CAPTCHA); the session's `WebSearch` budget was used up before work on the note began. The Premiere and After Effects rows are therefore marked `[NOT VERIFIED]`.
