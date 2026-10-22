---
id: F-UI-002
title: Customizable keys and other editors' keyboard layouts
status: candidate
priority: B
cost: L
category: ui/input
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-app]
invariants: []
ui_rules: [13, 14, 16, 17, 19, 20]
round: null
point: null
tests: []
depends_on: [F-TL-003]
checked: 2026-09-07
---

# F-UI-002 — Customizable keys and other editors' keyboard layouts

## Essence

A window where every command of the program shows its assigned chord and it can be changed, and at the top — a drop-down list of READY layouts: "like Premiere", "like Final Cut", "like Avid". The point is not that someone finds `Q` more convenient: the editor arrives with someone else's muscle memory, built up over years, and will not retrain their fingers — they will switch the layout. That is why competitors' layouts are shipped AS PART OF the package, and this is written about in the advertising. The layout itself is a FILE: it is exported, passed on, and someone else's is downloaded.

We have around twenty keys; they are hard-wired as `egui::Key::…` literals in one function and are not configurable in any way.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Customize Keyboard | `Options → Customize Keyboard`; tabs by area, a field "Show commands containing", a field "Shortcut keys"; `Save as` writes the layout to an **.ini** in `AppData\Roaming\Vegas\VEGAS Pro\`. The layout list contains only `[Default]`; no foreign layouts are shipped, "import" = put the .ini into that folder by hand |
| Adobe Premiere | 26 | Keyboard Shortcuts | `Edit → Keyboard Shortcuts`, a drawn keyboard; `Keyboard Layout Preset` contains ready-made competitors' layouts ("Avid Media Composer 5", "Final Cut Pro 7"); a saved layout is an **.kys** file in the user profile. There are TWO levels: application and panel, and a panel chord overrides the general one while the panel is in focus |
| DaVinci Resolve | 21 | Keyboard Customization | `DaVinci Resolve → Keyboard Customization` (⌥⌘K); one can start from the Avid Media Composer / Final Cut Pro / Adobe Premiere layouts; the three-dot menu — import/export of a preset (**.txt**); one key can mean different things on different pages (Edit vs Fairlight) |
| Final Cut Pro | 12 | Command Editor | ⌥⌘K; command sets (Command Sets), `Export`/`Import` → a **.commandset** file; the keyboard is colored by command groups; search by command name, by description keywords AND BY KEY NAME; the stock set is edited only through a duplicate; Apple does not ship foreign layouts |
| After Effects | 26 | Keyboard Shortcuts | `Edit → Keyboard Shortcuts`, a visual editor since October 2017; the sets are text files (`After Effects Default.txt` and copies) — a detail from a secondary source, `helpx` is unavailable from this environment |

## The trick behind it

**1. A "conflict" is not a general notion but three different policies, and they speak of different designs.** Final Cut, when a key is taken, shows the current assignment and asks for confirmation — the command TAKES the key away, and a gray dot remains on the new key. Premiere does not count a coincidence as an error at all, because there are two levels: on the drawn keyboard "Keys shaded in purple are
application-wide shortcuts. Keys shaded in green are panel-specific shortcuts", and the purple-green ones are where a panel command overrides the general one, and
"the application shortcut does not function when that panel has focus". Resolve
allows a duplicate assignment to be SAVED, and it simply does not work until the competitor is removed by hand. Three policies — three answers to the question of whether a key has a scope of visibility; a flat table "key → command" exists only in the one that has no scopes.

**2. Search by KEY, not only by command.** In Final Cut's Command Editor the search field takes "a command name, description keywords, or a key name", and one can simply tap a key on the keyboard and see what it does; the keys are colored by command groups ("editing commands, such as Overwrite (D), are
blue"). This is the reverse question — "what does this key do?" — and it is not answered by a list of commands. Vegas has no reverse lookup: there the filter is "Show commands
containing", that is, only a search of the command by name.

**3. The layout is a file with its own format for each, and the way to install it is also different for each.** FCP's `.commandset` with Export/Import items right in the menu; Premiere's `.kys` in the user profile; Resolve's `.txt` preset through the three-dot menu; Vegas's `.ini` — where there is NO import at all: the file is put into
`AppData\Roaming\Vegas\VEGAS Pro\` by hand and the program is restarted. Hence the market for foreign layouts: they are posted and sold, and a forum from 2007 already discussed that "someone could make such files for other programs".

## Pros

- A ready preset turns migration into one choice from a list instead of forty manual reassignments; a person starts working on the very first day, not a week later after retraining.
- Two levels of visibility (Premiere: application/panel; Resolve: the Edit and Fairlight pages) allow one letter to mean different things in different places — otherwise 26 letters have to be divided among several hundred commands, and they run out.
- A layout-file survives reinstallation and version change and is passed to the team: one person makes the layout — the whole studio installs it.
- Search by key name and coloring the keyboard by groups turn the key editor into a reference as well: the list of assignments can be read in both directions.
- Editing through a DUPLICATE of the stock set (FCP) means that "return to how it was" is choosing another set in the list, not rolling back edits one by one.

## Cons

- Context breaks the promise of the list: the line "Q — switch focus" is true only when focus is in the right panel, and the user searches the forum for where the key that "is in the table" went. Premiere is forced to explain the overlap rule and to paint the keys in two colors.
- A silently saved conflict (Resolve) sends the user to hunt for a defect in the program, although the cause is in their own layout.
- Having a key editor ≠ the ability to migrate: Vegas has had an editor since time immemorial, but no foreign layouts come with it, and installing someone else's is manual copying of a file into a system folder.
- A layout lives outside the project and outside the version: a new version brings commands that are absent from the old file, and a foreign layout does not assign them. A side effect for support — bug reports stop reproducing: "on my machine S does something else".
- A competitor's preset is approximate by construction: the sets of commands in the editors differ, and some of the familiar keys in the preset simply have nowhere to go.

## What users say

Advice to an editor moving from Final Cut Pro 7 to Premiere:

> Second: change the Keyboard Shortcuts to Final Cut Pro so you can continue to
> use your old muscle memory from FCP.
> — Walter Biscardi, Creative COW, 01.04.2014, https://creativecow.net/forums/thread/advice-for-an-editor-switching-from-fcp7-to-premie/

The same person, in the same thread, on the fact that this is not a temporary crutch for the first project:

> 2 years later I'm still doing that because I think FCP's keyboard shortcuts
> make a lot more sense than Adobe's arrangement.
> — Walter Biscardi, Creative COW, 01.04.2014, https://creativecow.net/forums/thread/advice-for-an-editor-switching-from-fcp7-to-premie/

The price of assembling a layout by hand, before they began to be shipped ready-made:

> Although creating and saving a new keyboard layout is pretty easy, it is
> monotonous.
> — Tom Meegan, Creative COW, 06.05.2007, https://creativecow.net/forums/thread/premiere-keyboard-shortcuts/

And a characteristic question about a CONTEXTUAL key, which a person searches for for years after switching:

> Is there any way to use a keyboard shortcut to switch focus between the
> Source Monitor and Timeline? In Final Cut Pro 7 you could press Q to do this
> and I found it very handy.
> — Ben Summers, Creative COW, 24.11.2021, https://creativecow.net/forums/thread/keyboard-shortcut-for-switching-between-source-monitor-and-timeline/

## How it fits Kinotrain

### What already exists

**One global key dispatch** — `KtApp::handle_keys`
(`src/apps/kt-app/src/app.rs:3750`). Inside is a local struct `Keys` of 18
fields, and each field is a literal:
`i.key_pressed(egui::Key::S) && !i.modifiers.ctrl`.
The gate on input is `ctx.egui_wants_keyboard_input()`. The full list of what is hard-wired today:

- **Space** — is read by EVENTS (`Event::Key{pressed, repeat}`), not by `key_pressed`: autorepeat sent a series of play/pause (item 3 of R.16). The decision is made on RELEASE: a short press — play/pause
  (`on_transport_toggle`, app.rs:1576), a repeated press shortly after a pause — a return to the start (`DOUBLE_PAUSE_MS`), holding — ramp up to 4× over 5 s
  (`on_space_released`, app.rs:1565; items 120/124, test 120).
- **S** — cut (`cut_action`, app.rs:2460); **Delete/Backspace** — delete.
- **Ctrl+Z**, **Ctrl+Shift+Z**, **Ctrl+Y** — undo/redo; **Ctrl+S** — save.
- **M** — marker at the cursor, CONTEXTUALLY: if the region and the cursor are inside it → a range marker, otherwise a point one (item 142).
- **I** / **O** — left/right edge of the region; **L** — loop.
- **Esc** — the `escape_action` cascade (app.rs:1665): search → the insert-prefix window → the topmost of our windows → cancel of a drag → clearing of the region and the selection.
- **Ctrl+C / Ctrl+V** — plus `Event::Copy`/`Event::Paste`, because egui-winit intercepts them and does not pass them as `key_pressed` (item 21).
- **Home/End** and **A/D** — stop points (item 82); **Shift+Home/End** — to the start/end of the project; **Shift+A/D** — a jump DRAGS the edge of the region (item 119, test 119).
- **Ctrl+L** — the timecode input mask (item 98); **Ctrl+F** — project search (item 92).

**Keys outside `handle_keys`** (their reassignment is a separate conversation):
arrows/Enter/Esc in the search palette (app.rs:1319), Esc in the insert-prefix window
(app.rs:2369), Esc in the detached preview window — deliberately local, does not trigger the common cascade (app.rs:964), Enter/Esc in inline renaming
(`src/apps/kt-app/src/timeline.rs:3521` and `:3572`,
`src/apps/kt-app/src/bin_panel.rs:577`), digits/Backspace/Enter/Esc that the timecode mask CONSUMES from the event queue
(`src/apps/kt-app/src/toolbar.rs:489`).

**Modifiers belong to GESTURES, not to commands**: Ctrl releases the magnet, and the same Ctrl, pressed before grabbing empty space, starts the selection frame
(`src/apps/kt-app/src/timeline.rs:1715`); Shift — vertical scroll, which does not cancel follow (timeline.rs:1114).

**Labels in the menu are a second, independent source of truth.** `Toolbar::menu_btn`
(`src/apps/kt-app/src/toolbar.rs:239`) glues the caption to the literal:
`"Ctrl+S"`, `"Ctrl+Z"`, `"Ctrl+Shift+Z"`, `"Ctrl+C"`, `"Ctrl+V"`, `"Del"`,
`"S"`. The handler in `handle_keys` and the caption in the menu are connected by nothing except attentiveness; the comment on the function honestly explains why the label is not in the translation catalog (a translator must not break the accelerator, and Latin inside an Arabic string would move during reordering).

**A ready sample of a registry** — `fx_registry::registry()`
(`src/apps/kt-app/src/fx_registry.rs:47`): `&'static [FxEntry]` behind a `OnceLock`; each entry has a name, a category and keywords for search, and next to it a comment "Full paths deliberately: test 125 looks up the catalog key by the path `k::…`". A registry of COMMANDS MUST look exactly the same. Ranked search also exists — `search_hits` (app.rs:4414).

**Application-level storage** — almost empty: eframe Storage holds two lines, `lang` and `last_project` (app.rs:5044), and the program's only file is `autosave_path()` = `$XDG_DATA_HOME/kinotrain/autosave-project.otio` (app.rs:4770).

### What is missing

A list of commands as an OBJECT (id + translatable name + default chord +
invocation); parsing and printing a chord; the notion of scope; a layout file
and its version; the editor window; presets "like in Vegas/Premiere/Resolve/FCP".

### What gets in the way

**1. The main issue: rule 17 ("paired ways of doing one action behave the same")
turns into a project requirement.** A key must have a counterpart in the menu, and
a menu item must be able to be a key; therefore the key editor must be built FROM
THE SAME command list as the menus and buttons, not from its own. Today such a
list does not exist: commands are scattered across three places — `handle_keys`
(app.rs:3750), `Toolbar::menu_items` (toolbar.rs:245) and the timeline context
menus (the `[tl.menu]` section of the catalog, 34 items). A key editor built on top
of this would create a FOURTH source of truth and would violate rule 17 by
construction. The order of work follows directly: first the command registry, from
which menus, hints and accelerators are drawn, and only then the assignment window.
This is the main cost of the feature — not the window, but merging three places
into one.

**2. i18n — a large and easily overlooked layer.** Test 125
(`src/apps/kt-app/src/i18n_tests.rs:27`) requires that EVERY catalog key is
non-empty in all five languages AND is mentioned in the sources by its literal
path ("catalog keys nobody uses"). Command names partly exist already — the
`menu.*`, `tb`, `tl.menu`, `bin.menu` sections give about a hundred strings — but
everything that is today reachable only by a key or a gesture (cut, mark, region
edges, loop, jumps between points, speed-up while holding, releasing magnet, the
Esc-cascade steps) has no name at all: that is several dozen more keys × 5
languages. Plus the NAMES OF KEYS (Escape, Home, End, Delete, Backspace, Space,
arrows, modifiers) — these must either be translated or deliberately left in
Latin. Further, test 100 (`font_tests.rs:265`) will require glyphs for all new
non-ASCII text, that is, rebuilding the subsets with
`python3 src/scripts/subset-fonts.py`. And Arabic: the accelerator is today
deliberately glued together in code so that it does not move when items are
reordered, while in the key editor a Latin chord would stand on every line of the
Arabic list — this is `i18n::bidi`, and test 126 already checks `"Ctrl+S"` as an
atom (i18n_tests.rs:99).

**3. Storage outside the project — the fourth time in the database, and this is
already a system.** The same question is noted in **F-AUT-004** (the FX preset
library), **F-TXT-002** (title styles) and **F-EXP-001** (export presets): a project
for us is ONE `.otio` file (`src/libs/kt-otio/`), while at the application level
there is only a directory with the autosave. Four independent notes run into the
same thing — so it must be decided once and systematically (directory, schema
version, behaviour on an unknown command id, transfer to another machine), rather
than inventing a fourth format on the fly. The layout is, at the same time, the
most harmless of the candidates: a lost layout does not spoil a project.

**4. Focus and the Esc-cascade — the most likely place for a defect.** Rule 14 of the
checklist grew out of item 8 of R.12: "Ctrl+L fired exactly once — the mask widget
is redrawn every frame with the same id, after Enter it stayed in egui focus, and
`handle_keys` is gated by `egui_wants_keyboard_input()`; until a mouse click
elsewhere all hot keys were dead". The key-editor window must CATCH arbitrary
chords, that is, deliberately live on the other side of this gate and swallow its
own events, and on closing it must hand focus back (`surrender_focus`); a mistake
here kills all keys at once right after the first assignment. Separately:
`escape_action` is a FIXED order in the code; if Esc becomes reassignable, the
cascade would have to be described by data, not by the position of a line in the
function.

**5. A flat table does not express what we already have.** Space is FOUR behaviours
on one key (a short press, a repeated press soon after a pause, holding, releasing),
M depends on the region, Ctrl is a modifier of a GESTURE (magnet/frame), not of a
command. Therefore a registry entry is not "key → function", but at least
"chord → command, with a phase (press/hold/release)", and gesture modifiers are not
taken into the first version at all, which must be stated out loud (rule 19: the
dialog must say where the decision switches).

**6. Live scenarios depend on the layout.** `src/scripts/live/*.py` send XTest keys
by name: `app.key("space")`, `app.key("Home")`, `app.key("ctrl+s")`
(`src/scripts/ui-probe.py:72`). As soon as the layout becomes a user file in
`$XDG_DATA_HOME`, the scenarios will start depending on someone else's state — they
need a forced default (an env variable such as `KT_KEYMAP=0`, like
`KT_SIMD=0`/`KT_VIDEO_GPU=0`).

### Which invariants it touches

Of engine invariants 1–12 — **none**: the feature lives entirely in `kt-app` and
touches neither the model, nor the project format, nor rendering, nor determinism.
This is a strong argument: "risk is low, the cost is volume".

UI checklist rules: **17** (paired ways — the source of the requirement for a single
command registry), **14** (repetition and return of focus), **16** (a conflict is shown
as an ERROR naming WHO took the key, not as a faded hint — Resolve's policy "saved, but
does not work" is forbidden for us by this rule), **13** (the window must not create a
new strip in the main interface), **19** (say where the layout switches back), **20**
(reassigning one key must not "fix up" its neighbours: if taken, say from whom).

## Estimate

| What | Answer |
|---|---|
| Volume | L — a whole round: command registry, rewiring of menus/keys onto it, window, storage, i18n |
| Where the main work is | `kt-app`: command registry, `handle_keys`, `Toolbar::menu_items`, the new window; catalogs `src/apps/kt-app/i18n/*.toml` + font subsets |
| Project format | not touched: the layout lives outside `.otio` (see item 3 "What gets in the way") |
| Tests that will have to be written | `layout_tests`: assignment window, REPEATED assignment in a row (rule 14), a conflict as an error showing the command that took the key, "a menu item and a key came from one registry entry" (rule 17); `i18n_tests` t125 and `font_tests` t100 for the new command and key names; a unit on parsing/printing a chord and on "a preset assigns only existing ids"; a live scenario with a forced default layout |
| Depends on | F-TL-003 (JKL: there `L` already conflicts with loop, and the verdict says outright "moving the binding needs the user's sanction"); the general question of user storage — F-AUT-004, F-TXT-002, F-EXP-001 |
| Breaks compatibility | no |

## Verdict

Take it, but not for the sake of key customisation — for the sake of the COMMAND
REGISTRY: today the list of commands does not exist as an object, so rule 17 holds
only through attentiveness, and the label in a menu and the key handler are two
independent literals in different files. The registry is useful on its own (menus,
hints, search palette, a future command palette), while the assignment window and
presets are an add-on on top of it; therefore the order "first the registry, then the
editor" is here not advice but a condition.

Priority **B**, and honestly: for our single user this is closer to C — he is not a
migrant from Avid and asks not for a setting but for the RIGHT default straight away
("Shift+A / Shift+D (as in pro programs)", item 2 of R.16 in `docs/user-feedback.md`;
"as in Vegas — the later one on top"; "as on YouTube — holding the space bar"). For any
second user this is A: all five competitors have configurable layouts, two of them
ship each other's layouts as a bundle, and a veteran editor changes the layout, not his
fingers. While the user is one, from the feature we should take the cheap half — the
command registry and the display of chords from it — and leave the window with presets
for a round in which a second user appears or JKL (F-TL-003) is required.

## Sources

- https://support.apple.com/guide/final-cut-pro/intro-to-the-command-editor-ver8a33d500d/mac (accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/view-keyboard-shortcuts-ver4df74f72/mac (2026-09-07) — keyboard colouring by group, search by key name
- https://support.apple.com/guide/final-cut-pro/modify-keyboard-shortcuts-ver761c9236/mac (2026-09-07) — duplicating a set, confirmation when a key is taken
- https://support.apple.com/guide/final-cut-pro/export-or-import-keyboard-shortcuts-ver761c9a56/mac (2026-09-07) — `.commandset`
- https://help.magix-hub.com/video/vegas/en/content/topics/customizing_keyboard_shortcuts.htm (2026-09-07) — `Options → Customize Keyboard`, `[Default]`, `.ini` in AppData, import by copying
- https://larryjordan.com/articles/more-keyboard-shortcut-secrets-in-adobe-premiere-pro-cc/ (24.06.2018, accessed 2026-09-07) — application/panel, purple/green, the overlap rule
- https://creativecow.net/forums/thread/advice-for-an-editor-switching-from-fcp7-to-premie/ (01.04.2014, accessed 2026-09-07)
- https://creativecow.net/forums/thread/premiere-keyboard-shortcuts/ (06.05.2007, accessed 2026-09-07)
- https://creativecow.net/forums/thread/keyboard-shortcut-for-switching-between-source-monitor-and-timeline/ (24.11.2021, accessed 2026-09-07)
- https://writedirect.co/how-to-create-custom-keyboard-mappings-in-davinci-resolve/ (2026-09-07) — secondary: presets of other NLEs, import/export via a menu in three places, "will save, but will not work", one key on different pages
- https://www.filmeditingpro.com/how-to-install-keyboard-shortcuts-premiere-resolve-avid-final-cut/ (2026-09-07) — secondary: Premiere's `.kys`, Resolve's `.txt` preset, file locations

**What could not be verified.** `helpx.adobe.com` (the official Premiere and
After Effects help) and `reddit.com` do not open from this environment, and the web
search budget is exhausted — the search went through direct queries to Yahoo and Bing.
Therefore: the Premiere and After Effects rows rely on larryjordan.com and secondary
analyses; the presence of the presets "Avid Media Composer 5" and "Final Cut Pro 7"
in version 26.x **[NOT VERIFIED]** — confirmed for the CS6 line and later from secondary
sources and the community; the location and format of After Effects' sets
(`After Effects Default.txt`) — **[NOT VERIFIED]**, from the documentation of a third-party
plugin. The Vegas forum has moved: `vegascreativesoftware.info/us/forum` now redirects
to `forum.borisfx.com` (Discourse), and old threads at the former links do not open —
this should be taken into account in the next wave of notes.
