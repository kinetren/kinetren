---
id: F-UI-002
title: Customizable keys and other editors' keyboard layouts
status: done
priority: B
cost: L
category: ui/input
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: []
ui_rules: [13, 14, 16, 17, 19, 20]
round: null
point: change control-layouts
tests: [t1700, t1701, t1705, t1706, t1711, t1712, t1715]
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

CAPABILITY: openspec/specs/ui-controls/

Done in `control-layouts`: the command table `controls.rs` is the one source for keys, menus and hints; the editor is the Customize part of `kt_controls`; a conflict is asked (FCP's policy, rule 16).
Not taken: panel scopes, gesture modifiers, Esc and Enter as commands; the reasons are in that capability's design.

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
