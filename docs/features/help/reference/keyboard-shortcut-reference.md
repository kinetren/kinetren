---
id: F-HLP-024
title: Keyboard cheat sheet inside the program
status: candidate
priority: B
cost: S
category: help/reference
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [9, 13, 14, 17, 18]
round: null
point: null
tests: []
depends_on: [F-UI-002]
trackers: [losslesscut, openshot, flowblade]
checked: 2026-09-07
---

# F-HLP-024 — Keyboard cheat sheet inside the program

## Essence

One place people come to DELIBERATELY to see all the keys at once: a table "chord — what it does", divided into semantic groups. People come here not for a command (for a command they go to the menu), but at the moment they have decided to stop moving the mouse around. A cheat sheet answers a question that the hint line does not hear at all: "what is there on the keys in general", because one cannot hover the mouse over a key. We have such a table already written, translated and published — on the landing page, and there is not a single link to it from the program.

## Who has it and how

The question of the row is one: is there a place IN THE PROGRAM where ALL keys are visible at once and which at the same time is NOT a layout editor (the editor is F-UI-002)?

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | no separate one | The list of assignments lives in `Options → Customize Keyboard` — that is, in the layout editor: tabs by area, the filter "Show commands containing", the field "Shortcut keys" (F-UI-002). There is no readable list "everything at once"; to a direct question of a user the vendor refers to a chapter of the manual [NOT VERIFIED: the chapter in the VEGAS 2026 manual could not be opened, mirrors of the help exist, see "Sources"] |
| Adobe Premiere | 26 | no separate one | `Edit → Keyboard Shortcuts` — a drawn keyboard: this is the form "interactive keyboard", but it is also the editor. Keys are colored by scope of visibility (purple — general, green — panel), pressing a modifier redraws the layer (F-UI-002, F-HLP-005) |
| DaVinci Resolve | 21 | no separate one | `Keyboard Customization` (⌥⌘K) — the same virtual keyboard plus a list; reading and editing in one window, the modifier layer is chosen by clicking the drawn key (F-UI-002) |
| Final Cut Pro | 12.3 | not in the program; a printed page in the guide | The only one of the five whose cheat sheet is separated from the editor and is a DOCUMENT: the page "Keyboard shortcuts in Final Cut Pro for Mac" — 10 groups (Application, Editing, Effects, General, Marking, Organization, Playback and navigation, Share and tools, View, Windows) and about 380 rows in three columns **Action / Command / Shortcut**. The full set is in the Command Editor; the page honestly says it is a subset |
| After Effects | 26 | no separate one | `Edit → Keyboard Shortcuts` — a visual editor since 2017; a readable list is a help page on the site, not a window of the program [NOT VERIFIED: `helpx.adobe.com` from this environment did not open either directly or through `r.jina.ai`, the same HTTP/2 error as in F-HLP-017] |

**Summary for the core, and it is unexpected: none of the five has a light cheat sheet inside the program.** The list of keys is either MERGED with the layout editor (four of them) or moved OUTSIDE into a document (Final Cut). A split of the two surfaces — a read-only cheat sheet separately, an editor separately — does not occur in video editing even once, and its price cannot be estimated by five lines of the core.

**Outside the core.** Both programs below make this split DELIBERATELY and explain why; without them the form "printed cheat sheet" cannot be described, and the "trick behind the trick" of this note is taken directly from them.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Visual Studio Code | 1.136 | Keyboard Shortcut Reference + Keyboard Shortcuts editor | Two different surfaces and two different items. `Help → Keyboard Shortcut Reference`: "We have a printable version of the default keyboard shortcuts… a condensed PDF version suitable for printing as an easy reference" — three PDFs by platform. Editing is separate: `Ctrl+K Ctrl+S`, an editable table, opened "in a modal overlay on top of the editor area" |
| IntelliJ IDEA | 2026.2 | Help → Keyboard Shortcuts PDF | A printed reference card of the layout, and the vendor explicitly limits its scope: "If you are using one of the predefined keymaps, you can print the default keymap reference card". For your OWN layout there is no standard printing — the help refers to a third-party plugin, Keymap exporter |

**Four forms of a cheat sheet and the price of each.** The difference between them is not in design but in what each can do and what it pays for doing it.

1. **A printed page or PDF.** Lives OUTSIDE the program: it lies next to the keyboard, is read while the program loads, is photographed and passed on. The only form that survives closing the program, and the only one that can be read without taking the screen away from editing. Price: it does not follow reassignment (JetBrains prints only the predefined layout) and gets stale silently — nobody rebuilds a printed sheet. We already have this form and it is already built by the pipeline (`site/build.py`).
2. **A window in the program with a list.** People come to it deliberately, therefore the surface overlaps nothing and blocks nothing; while the window is closed it costs zero pixels and zero frames (rule 13). Price: it needs an entry point (a menu item, a key, an icon) — otherwise the window exists and is unreachable; and it needs the return of focus on closing (rule 14). This is a form that no video editor has.
3. **An overlay on top of the interface, shown while a key is held.** The most interesting and the most controversial. The reference is PowerToys Shortcut Guide: "When you open Shortcut Guide, it automatically shows shortcuts for the app currently in the foreground", holding Windows by default for 900 ms, the duration is configurable from 100 to 5000 ms, there is a setting "Close Shortcut Guide when the Windows key is released". The mechanism solves the length of the list by CONTEXT: it shows not all keys but the keys of what is in front of you right now. Price: the overlap (see the quotes) and the hold delay — that is, exactly points 1–2 of the owner's decisions (item 14 of round 8); plus a hold timer that has to be polled (invariant 12). Telling: the vendor of this form **itself rolled it back**: in Shortcut Guide V2 the guide became a regular window ("Shortcut Guide opens as a regular window that you can read alongside the app you were using"), the hold was removed, users demanded its return, and it was returned as a setting. **We do not propose this form** — it contradicts two decisions of the owner at once, and the market has changed its mind about it twice.
4. **An interactive keyboard where one clicks a key.** It answers the reverse question "what does this particular key do" and shows which keys are FREE. It exists in Premiere, Resolve, Final Cut and After Effects — and everywhere it is part of the layout editor, because "is this key free" is interesting exactly to the one who assigns his own. Price: the drawing of the keyboard depends on the physical layout of the user, and this is a separate subsystem. The form belongs to F-UI-002, not to this note.

## The trick behind it

**The payload of the mechanism is THE WHOLE LIST AT ONCE, that is, the possibility of reading about what one did not ask about.** A status-bar line is addressed by the pointer: it physically cannot say anything about an object over which the cursor is not hovering, and a key cannot be hovered over at all — a key has no rectangle on the screen. A cheat sheet is not addressed by anything: it is read from top to bottom and one learns about commands whose existence one did not suspect. For us this payload has already moved on, and moved entirely: 30 lines in four groups lie on the landing page (`site/content/structure.toml:203-244`), and in the program there is no trace of them and no link.

Three details that are absent from the list of capabilities of everyone.

1. **A cheat sheet is the only surface capable of showing a SECOND binding of a command.** The menu column cannot do this structurally: one item — one chord (F-HLP-017 calls this its own limitation). The table can, and ours already does: `A`/`D` and `Home`/`End` stand as two rows with the SAME description ("Previous / next stop point"), because these are two bindings of one action. And precisely there it fails: `Backspace` (the second binding of deletion) and `Ctrl+Y` (the second binding of redo) are absent from the thirty rows. The mechanism can do what the menu cannot, and exactly this ability is broken for us.

2. **Incompleteness of a cheat sheet is its property, not a defect, and therefore it cannot be generated with one pass over the registry.** Apple writes on its page: "Shortcuts for common commands are listed in the table below" — and that is ~380 rows with the full set living in the Command Editor. Readability is given by CURATION, and curation is someone's decision about which command is "common". So a machine assembly from the registry removes the question "is it lying or not", but does not remove the question "what to show": the registry needs a sign, not only a record. This is the part of the price that is absent from every list of capabilities.

3. **The chord and the caption of the action are TWO DIFFERENT SOURCES, and our own landing page has understood this already, while the program has not.** In `site/content/structure.toml` above the tables there is a decision written as prose: "Chords are not prose: they are the same in any language. The catalog holds only the description of the action, the key `keys.<group>.<id>`". The chord is split into tokens (`{ id = "top", keys = ["Shift", "+", "Home"] }`) and drawn as separate `<kbd>` elements, and the caption is taken from the catalog. This is exactly the justification recorded in the comment of `Toolbar::menu_btn` ("the translator must not be able to break the accelerator"), — and exactly what is violated in the ~150 hand-written mentions of keys inside translated phrases (F-HLP-017). One command, two files in one repository, two opposite policies, and neither of them was ever accepted aloud.

## Pros

- They come deliberately, so the surface competes neither for the screen nor for attention: while the window is closed, it does not cost a single pixel or a single frame. Unlike a permanent panel, which pays with space every second, and unlike an overlay, which pays with overlap at the moment of showing.
- The only surface with a REVERSE index: the column of chords answers "what does `Ctrl+L` do". The hint line never answers this question — one cannot hover the mouse over a key; the menu answers only in the other direction.
- Adjacency by MEANING: a group puts `I` and `O` next to each other, and `Ctrl+Z` and `Ctrl+Shift+Z` as a pair. A sequence of single hints cannot be read this way in principle, while the menu column gives adjacency only vertically within an item.
- It shows a second binding of one command — what the menu column does not express structurally.
- It covers both what is in the menu and what lives in a gesture. For us this is exactly the hole that F-HLP-017 calls its ceiling: 22 rows of the cheat sheet out of 30 carry keys, 8 — mouse gestures, and none of the eight is expressed by a menu item.
- A cheat sheet assembled from the registry cannot lie, while one written by hand always does, and the proof lies in our own repository (see "How it fits"): 30 rows, two missing bindings and one row about behavior that does not exist in the program.
- The printed form works with the program closed: a sheet next to the keyboard can be read while the project is opening. No surface inside the program can do this by definition.

## Cons

- Length. The full list is unreadable: Final Cut has ~380 rows even in the SELECTED form. Curation is manual labor, and it goes stale first.
- A second source of truth by construction — if the cheat sheet is not assembled from the registry. Ours is the proof. For us this means directly: until the registry of commands exists (F-UI-002), a cheat sheet in the program becomes a FIFTH source alongside the handler, the menu literal, the translated phrase of the catalog and the site table.
- The printed form does not follow reassignment: JetBrains prints only the predefined layout, and for one's own the vendor refers to a third-party plugin. That is, form 1 and F-UI-002 are structurally hostile to each other.
- "They come deliberately" can be read also as "they do not come at all". A cheat sheet is pull, and pull of last resort: to open it one must know that it exists. It has no entry point of its own; it has to occupy one in the menu or on a key, and this is exactly the same task one level up.
- A flat table does not express context. For us `Esc` is a cascade of five steps (`escape_action`, `src/apps/kt-app/src/app.rs:1712`), `Space` is four behaviors on one key, while a table row is one; it inevitably describes one thing.
- Five languages in one table require two columns from two sources: the chord is not translated, the caption is. This is solvable (the landing page solved it), but it means that the Spanish policy "translate the NAMES of keys" (`Supr`, `Mayús`, `Inicio`, `Fin` — F-HLP-017) is incompatible with this table; one of the two policies will have to be buried deliberately.
- **A window ABOUT keys swallows keys — and this is not a hypothesis of rule 14 but the most frequent way to break exactly this feature.** The tracker showed the price of form 2 live: "I want to learn keyboard shortcuts, but with the Keyboard Shortcuts dialog open I can not apply any of these keyboard shortcuts, as the dialog seems to intercept all of them" (flowblade #620, 14.02.2019, https://github.com/jliljebl/flowblade/issues/620; closed by a fix). The same defect came to LosslessCut from the other side — there, because of it, people ask to move the help OUTSIDE, because the internal one "has to be dismissed in order to start using a key command" (losslesscut #726). That is, form 2 has a price that none of the other three has: while the cheat sheet is open, one cannot train on it with the keys.

## What users say

What happens when there is no cheat sheet: the user makes one himself and publishes it.

> My question is whether it would be possible to make an authoritative list of
> keyboard shortcuts/hotkeys? I have searched for a long time for a good
> reference doc but haven't come upon anything.
> — nateC, 20.01.2026, thread "Keyboard Shortcuts for SynthEyes?",
> https://forum.borisfx.com/t/keyboard-shortcuts-for-syntheyes/20970

The same day he answers himself, attaching a 91 KB PDF: "This is my list of SynthEyes Keyboard Shortcuts and Hotkeys as of now." The whole thread is the price of the absence of a cheat sheet, expressed in hours of someone else's labor: the keys, in his words, are "buried in the documentation or tutorial videos".

About form 3 (the overlay on hold) — both sides of the dispute live. For:

> In earlier versions of PowerToys, holding down the Windows key opened the
> Shortcut Guide… This feature was extremely useful and has been completely
> removed in current versions.
> — h3rbstk1nd, 22.06.2026,
> https://github.com/microsoft/PowerToys/issues/48789

The maintainer there (niels9001, 24.06.2026) confirms the rollback and the return: "taskbar indicators are already shipping in the current version, and we are working on adding long-press support". Against — the price of the overlap, verbatim:

> Opening the guide shows the guide as expected, but the rest of the screen is
> covered in a grey overlay.
> — jungletek, 17.08.2026,
> https://github.com/microsoft/PowerToys/issues/49975

In the bug trackers of open editors the same request lies in their own words. The first is exactly form 1 (the printed card) and exactly the price that JetBrains pays: the cheat sheet is needed AFTER reassignment, while only the predefined layout is printed.

> It helps to create a cheat sheet from the list and display it till muscle
> memory develops. Let the user export the "command vs hotkey" text from the
> **Preferences** dialog.
> — openshot #5447, 20.02.2024 (still open),
> https://github.com/OpenShot/openshot-qt/issues/5447

The second is a direct opposition of forms 1 and 2, and the justification is exactly what stands in the "Cons" of form 2:

> It would be helpful to have a keyboard shortcut reference sheet available
> outside of the app (rather than only as a help menu within the app that has to
> be dismissed in order to start using a key command).
> — losslesscut #726, 18.04.2021 (open, 6 comments),
> https://github.com/mifi/lossless-cut/issues/726

The third — about the entry point, which in this note costs more than the window itself, and the user names both paired ways at once (rule 17):

> It would be nice to be able to press <kbd>?</kbd> and have the list of
> shortcuts appear. It could also be an option in the top menu.
> — losslesscut #19, 04.01.2017, https://github.com/mifi/lossless-cut/issues/19

SIGNAL: the list of keys as a DOCUMENT — 2 tasks in losslesscut, `cheat ?sheet|(shortcut|hotkey|keybinding)s?[^.]{0,20}(\breference|cheat|chart|\bdocumentation|documented)|(print|export)\w*( the| your| current| all| custom){0,2}( keyboard| key)? (shortcut|hotkey|keybinding)|(list|overview) of[^.]{0,20}(shortcut|hotkey)|show shortcuts`; share 0.1% against 0.0% (1 task out of 4516) in openshot

SIGNAL: keys as a topic in general — 41 tasks in losslesscut, `(shortcut|hotkey|keybinding|keymap)`; share 2.1% against 0.4% in openshot

Two counts side by side give a find that is not in the documentation: the topic of keys is hot everywhere (1.0–2.1% in seven trackers out of eight), while "the list of keys as a document" is 0.1% at best. People ask not for a cheat sheet but for a layout EDITOR, and this is the same result as the survey of five core editors above, only expressed in counts. Moreover, both tasks of losslesscut came where a window with the list ALREADY exists (`HelpSheet`, the Help item and the key `H`): the cheat sheet is requested there not instead of the window but in addition to it — because the window has to be closed in order to press a key.

About the five core editors no quote exists and cannot exist: there is no public tracker for any of them (`_trackers.md`, "Whom we do NOT take"), and a vendor forum is not a tracker. Through the Discourse API of the Boris FX forum the queries "cheat sheet shortcuts", "printable keyboard shortcuts", "shortcut list pdf", "keyboard shortcuts chart", "list of hotkeys", "shortcuts reference document", "print shortcuts", "quick reference guide keyboard", "where is the list of shortcuts", "learn shortcuts faster", "memorize shortcuts" were worked through — the only thread on the substance is quoted above, and it is about SynthEyes, not about VEGAS.

## How it fits Kinotrain

**What the cheat sheet adds ON TOP of the command registry (F-UI-002).** The registry is the source: it knows which command has which chord, and answers the one who came to ask. The cheat sheet is the only surface that is read WITHOUT a question, from top to bottom, and the only one that answers in the reverse direction ("what does `Ctrl+Y` do") without being at the same time an editor of assignments. Plus, the printed form works where there is no registry at all — with the program closed. The answer "nothing" is impossible here: a registry without a cheat sheet shows NOTHING to anyone who did not open the assignments window.

**What the cheat sheet adds ON TOP of the accelerators in the menu (F-HLP-017).** The menu column teaches seven commands that have an item, one chord per item, and only to the one who went into the menu. The cheat sheet takes all 27 chord forms, including 20 that have no menu item and cannot have one (transport, navigation, the edges of a region, wheel gestures), and shows a second binding of one command, which the column does not express structurally. The boundary of F-HLP-017 was formulated by itself: "Coverage equals the menu… the other 23 — transport, navigation and gestures — are not expressed by anything". This is the subject here. The four diverged sources of a chord (handler, menu literal, catalog phrase, site table) and the Spanish `Supr`/`Mayús` are a find of F-HLP-017; it is cited, not reopened.

ADDRESS: src/apps/kt-app/src/app.rs (`handle_keys`, the gate `ctx.egui_wants_keyboard_input()`) — the class of flowblade #620 ("the window about keys intercepted the keys") is possible for us, and nothing catches it: the gate mutes all 27 chord forms at once while any widget holds focus, and the only guard of a surrendered focus — t98 in `src/apps/kt-app/src/layout_tests.rs` — checks exactly one surface, the timecode mask, and knows nothing about windows ("Notifications", "Diagnostics"). Half of the class of losslesscut #726 is, on the contrary, impossible for us: the printed form already lives outside the program (`site/build.py`), and closing it in order to press a key is not required.

### What is already there

**The cheat sheet is written, split into data, rendered and published — outside the program.** `site/content/structure.toml:203-244`: four sections `[keys.transport]`, `[keys.editing]`, `[keys.region]`, `[keys.view]`, 30 rows of the form `{ id = "top", keys = ["Shift", "+", "Home"] }`. The captions of actions are in `site/content/*.toml`, the section `[keys]`, 36 values per catalog (heading, subheading, 4 groups, 30 actions). It is rendered by `c_keys` (`site/build.py:382`), chords go as separate `<kbd>`, the caption as a separate `<span>`. The section is declared as a section of the landing page (`structure.toml:147-152`, `component = "keys"`, `timecode = "00:01:58"`).

**The subheading of the section formulates our philosophy of hints in one line — and formulates it on a page that cannot be reached from the program:**

> "There are no pop-up hints in Kinotrain anywhere. There is one hint line, always in the same place at the bottom of the window, and it says what the thing under the cursor does." (`site/content/ru.toml:185`)

**The chord as data is already separated from prose there, and the decision is recorded:** `structure.toml:200-202` — "Chords are not prose: they are the same in any language. The catalog holds only the description of the action, the key `keys.<group>.<id>`". This is exactly the model that the program lacks, and it already exists in the repository.

**There are no links to the site from the program.** Checked by grep: across the whole `src/apps/kt-app/src/` and across all five catalogs `src/apps/kt-app/i18n/*.toml` there are zero occurrences of `http://`/`https://`; neither `open_url` nor `Hyperlink` is used. `DEFAULT_BASE = "https://kinotrain.app"` (`site/build.py:54`) is known only to the site builder.

**Places for an entry point are free.** The key `F1` is not taken — across the whole `src/apps/kt-app/src/` there are zero mentions of `egui::Key::F` with any number. There are three root menus (`Toolbar::menu_items`, `src/apps/kt-app/src/toolbar.rs:244`) — File, Edit, View; there is no Help menu. There is no icon for help either: `icons.rs` has 39 constants, among them there is no "?", no book, no keyboard (a new codepoint means rebuilding the subsets, `src/scripts/subset-fonts.py`, and guard t100).

**A window with exactly the needed behavior has already been written twice.** "Notifications" (`src/apps/kt-app/src/app.rs:1103`) and "Diagnostics" (`:5069`) — `egui::Window` behind a `bool` flag, opened by a menu item in "View", closed by the cross and by a step of the cascade `escape_action` (`:1734`, `:1738`). The cheat sheet is the same widget with the same life cycle; there is nothing to invent.

### What is missing

**Checking the cheat sheet against the handler, and it fails.** `KtApp::handle_keys` (`src/apps/kt-app/src/app.rs:3879`) listens to 27 chord forms: `S`, `Delete`, `Backspace`, `Ctrl+Z`, `Ctrl+Shift+Z`, `Ctrl+Y`, `Ctrl+S`, `M`, `I`, `O`, `L`, `Esc`, `Ctrl+C`, `Ctrl+V`, `Home`, `End`, `A`, `D`, `Shift+Home`, `Shift+End`, `Shift+A`, `Shift+D`, `Ctrl+L`, `Ctrl+F` and three behaviors of `Space` (a short press, a repeated press within `DOUBLE_PAUSE_MS`, a hold longer than `SPACE_HOLD_MS`). The cheat sheet names 25 of the 27 in 22 rows; the remaining 8 rows are mouse gestures (`Ctrl` without magnet, four wheel gestures, three double clicks).

Not named anywhere in the cheat sheet:

- **`Backspace`** (`app.rs:3929`) — the second binding of deletion;
- **`Ctrl+Y`** (`app.rs:3933`) — the second binding of redo.

These are the same two keys that F-HLP-017 found absent in all 522 values of each of the five catalogs of the program. That is, they do not exist ANYWHERE — neither in the program nor on the site; the handler listens to keys that the product does not have.

**The reverse direction: the cheat sheet promises what is not in the program.** The row `view_reset` ("Speed bar, volume rail, fader: reset") names three surfaces, and the third is wrong. A double click on the speed bar resets the speed (`src/apps/kt-app/src/app.rs:3546`), a double click on the fader resets the level (`src/apps/kt-app/src/meters.rs:780`), and a double click on a clip — including the zone of the level line, `ClipZone::Level` — opens the FX window (`src/apps/kt-app/src/timeline.rs:2471`). The reset of gain lives as the context-menu item `tl.menu.reset_gain` (`timeline.rs:4401`). The error is written by hand, published, and lies in five files of the catalog.

**The most frequent double click of the program is absent from the cheat sheet.** Of at least thirteen places where the code distinguishes a double click, three are named (preview, timecode, reset). Not named: clip → FX window (`timeline.rs:2471`, the comment in the code: "exactly what the hint of the clip promises from R.18"), `Shift` + double click → region by event (there as well), track header → rename (`:1681`), morpher → its window (`:2406`), camera → its window (`:2379`), transition → its properties (`:2420`), range marker → selection of the region (`:2343`), a curve key → deletion (`src/apps/kt-app/src/automation.rs:547`), the window title background → maximization (`src/apps/kt-app/src/toolbar.rs:230`, `:388`). And separately: a SINGLE click on the preview is play/pause (`app.rs:3344`, `preview_click_step`), while the cheat sheet speaks about the preview only with respect to the double click.

**The row about `Esc` describes one step out of five.** `region_clear` — "Clear the region (the loop stays armed)": this is step 3 of the cascade `escape_action` (`app.rs:1712`), and in this part it is accurate (`clear_region_keep_loop`). About closing the search, the window of the insert prefix, the top window and cancelling a drag it is silent. The same failure that F-HLP-017 found in `tb.timecode_edit` ("Esc — cancel" in a private sense), only in the other direction.

**There is no translation, contrary to the general assumption of the wave.** The section `[keys]` exists in all five catalogs of the site, but it is translated into ONE language: `es.toml`, `zh-Hans.toml` and `ar.toml` contain the English text byte for byte (coverage 0.0%, automatic `noindex` by `translated_min = 0.9`). This was established by F-HLP-019 and F-HLP-033 for `[fxdesc]`, and the same count confirms it for `[keys]`: ru — 98.1%, the other three — 0%. The practical conclusion for this note: moving the cheat sheet into the program means 30 untranslated values × 3 languages, not "the translation is already paid for".

### What gets in the way

**A fifth source of truth.** Today the chord `Del` is recorded four times (handler, the `menu_btn` literal, the catalog phrase, the site table), and the records have already diverged (F-HLP-017). A cheat sheet in the program, typed by hand, becomes a fifth record — and it is worse than the other four, because it claims completeness: an incomplete column in the menu has an honestly empty cell, while in an incomplete cheat sheet the absence of a row reads as "there is no such key". Hence the strict order: first the chord as data (F-UI-002 and F-HLP-017), then the window. Otherwise the feature, done by hand, creates a defect that it itself will later have to catch.

**The catalog of the program is not adapted for the table.** 30 captions of actions are 30 new keys × 5 languages with the full price of F-HLP-033 (the limit of 160 characters in `src/apps/kt-app/build.rs:42`, the glyphs of t100, the subsets, t125 "not a single inscription outside the catalog", t131). The captions are short (the longest English one is "Suspend snapping while dragging", 32 characters), so the limit does not pinch, but three languages will have to be translated, not carried over.

**Arabic is solved by the form, not by a crutch.** A Latin chord inside an Arabic phrase is a forbidden situation, which the comment of `menu_btn` speaks about directly, and which `ar.toml` already violates 30 times (F-HLP-017). Two columns remove it: in its own cell the chord is a separate glyph atom, and `i18n::bidi` (`src/apps/kt-app/src/i18n/bidi.rs`) reorders atoms, not characters; the guard is t126, where `"Ctrl+S"` is already checked as an atom. That is, the table is not a convenient form but the ONLY correct one for RTL.

**The entry point costs more than the window itself.** A menu item requires a fourth root menu or a place in "View" (rule 13 against a new strip, rule 18 — the item must say WHAT and WHERE). `F1` is free, but a lone key without a menu item violates rule 17 (paired ways). An icon requires a new codepoint and rebuilding the subsets. None of the three variants is free, and the choice is the question of the round.

**The `Esc` cascade is a fixed order in the code.** A new window must enter `escape_action` by an explicit branch (`app.rs:1712-1741`) and return the focus on closing, otherwise the defect of item 8 of round 12 repeats, from which rule 14 grew: a widget held the egui focus, `handle_keys` is gated by `egui_wants_keyboard_input()`, and all hotkeys died until a click elsewhere. For a window that is ABOUT hotkeys this would be a particularly elegant defect.

### Which invariants it touches

**Invariant 12 — not touched by the chosen form and touched by the rejected one.** A window with a static list does not set up any timer, animation or decaying state: it is redrawn exactly when the frame is redrawn, and does not change the frequency (the idle tick of 250 ms, `repaint_delay(false)`, guard t20). The rejected form 3 (the overlay on hold) requires MEASURING the duration of the hold, that is, polling time in frames without input — this is a direct dispute with invariant 12 and with t20, and it is one of the three reasons why it is not proposed here.

Rules of the checklist `src/apps/kt-app/CLAUDE.md`:

- **13 "do not waste space"** — the main argument "for" precisely the window: a closed cheat sheet costs nothing, while a permanent panel pays every second, and an overlay pays with overlap at the moment of showing.
- **14 "works repeatedly, focus returns"** — the window must enter the `Esc` cascade and give the focus back; see "What gets in the way".
- **17 "paired ways are identical"** — central: the cheat sheet must name the same key as the handler, and with the same words as the menu column (F-HLP-017). Today it is violated in both directions.
- **18 "the menu item says WHAT and WHERE"** — the entry point must name itself without riddles; "Help" without a qualifier does not answer the rule.
- **9 "nothing superfluous"** — forbids duplicating in the cheat sheet what already stands in the menu column, IF both surfaces take the chord from different places; when the source is one, duplication stops being duplication.

The format of the project, the engine and invariants 1–11 are not touched: the feature lies entirely in `src/apps/kt-app` and `site/`.

## Estimate

| What | Answer |
|---|---|
| Volume | S — one stage of the round, IF by then the chord has become data. The window itself is a list from a ready table; no new subsystem, no new widget (the model — the "Notifications" window, `app.rs:1103`) |
| Where the main work is | `kt-app`: the window and its entry point, a branch in `escape_action`, 30 captions in `i18n/*.toml` (3 of 5 languages will have to be translated); `site/`: the table moves from `site/content/structure.toml` to a common source, so that the landing page and the program read one |
| Project format | does not touch it |
| Tests that will have to be written | `reach_tests::cheatsheet_and_handler_agree` — a check "chord in the handler ↔ row of the cheat sheet" in both directions; a harness test "the menu item opens the window, `Esc` closes it, the keys are alive after closing" (rule 14); t100/t125/t128 automatically for the new keys |
| Depends on | F-UI-002 (the command registry is the source; without it the cheat sheet becomes a fifth source of truth), F-HLP-017 (the chord as data and the column), F-HLP-033 (the price of a row in five languages), F-HLP-032 (the pipeline of the site — the printed form lives there too) |
| Breaks compatibility | no |

Separately: the printed form we ALREADY have works and costs nothing — it is built by `site/build.py` together with the landing page. The note does not propose to make it; it proposes to stop pretending that the program and the site do not exist for each other.

## Verdict

We take it, but not as "add a window with a list of keys" — the window is the cheapest part here — but as "make it so that the list cannot be written by hand". Three things that the reconnaissance changed in the statement. First: the cheat sheet already exists, is written and published, and the question is not "make" but "reach" — from the program to the site there is not a single link, `F1` is free, there is no Help menu, no icon. Second: the hand-written cheat sheet is already lying today, and this is verifiable without running the program — two bindings are omitted, one row describes behavior that does not exist, and the most frequent double click of the program is not mentioned in thirty rows. Third: the form "overlay on hold" is the most interesting of the four and the only one that solves the length of the list by context (a link to F-HLP-005: there the slice is chosen by a modifier in the same row, here by a modifier over the whole screen), — and it is rejected by three arguments at once: the overlap (item 1 of the owner's decisions), the hold delay (item 2) and the timer against invariant 12; the vendor of this form, Microsoft, itself replaced it with a regular window.

TEST: reach_tests::cheatsheet_and_handler_agree — today it fails three times, and all three failures are visible by one reading of the files, without running the program. (1) `Backspace` and `Ctrl+Y` (`src/apps/kt-app/src/app.rs:3929`, `:3933`) are heard by the handler and are absent from all 30 rows of `site/content/structure.toml` (`[keys.*]`) — the program listens to keys that the product, from the point of view of any user, does not have. (2) The row `view_reset` promises a reset by double click for the "volume rail", while a double click on a clip, including the zone `ClipZone::Level`, opens the FX window (`src/apps/kt-app/src/timeline.rs:2471`), and the gain reset lives as a context-menu item (`timeline.rs:4401`) — the cheat sheet promises a gesture that does not exist, and promises it in five catalog files. (3) A double click on a clip = the FX window, the most frequent double click of the program, is not named in any of the 30 rows, although the code calls it "exactly what the hint of the clip promises from R.18".

ONE LINE: overlap — does not touch | delay — does not touch | capacity — does not change by a single character in the status-bar line; the whole list appears on a THIRD surface, to which one comes deliberately — a separate window which, while closed, does not cost a single pixel or frame — plus the already existing printed page of the landing, to which from the program a link finally leads.

## Sources

**Not one of the seven programs was verified live** — there is no access to them from this environment; for rows outside the core a live check is not assumed (`_editors.md`, section "Software outside the core"). The mechanics of layout windows of the five core editors were NOT reopened: they are verified and recorded in F-UI-002, the visibility of the accelerator — in F-HLP-017; here references to them are given.

- https://support.apple.com/guide/final-cut-pro/keyboard-shortcuts-ver90ba5929/mac
  (accessed 2026-09-07) — the printed page of Final Cut Pro 12.3: "Shortcuts for common commands are listed in the table below", 10 groups, ~380 rows, columns Action / Command / Shortcut
- https://code.visualstudio.com/docs/configure/keybindings (accessed 2026-09-07) — `Help → Keyboard Shortcut Reference`, "a condensed PDF version suitable for printing as an easy reference"; separately the editor `Ctrl+K Ctrl+S`
- https://www.jetbrains.com/help/idea/mastering-keyboard-shortcuts.html (accessed 2026-09-07) — `Help | Keyboard Shortcuts PDF`, "If you are using one of the predefined keymaps, you can print the default keymap reference card"; for one's own layout — the third-party plugin Keymap exporter
- https://learn.microsoft.com/en-us/windows/powertoys/shortcut-guide (page updated 25.08.2026, accessed 2026-09-07) — form 3: contextuality ("shows shortcuts for the app currently in the foreground"), hold 900 ms (configurable 100–5000), "Close Shortcut Guide when the Windows key is released", and the transition to a regular window ("opens as a regular window that you can read alongside the app you were using"); the manifests of cheat sheets are YAML, among the built-in ones there are DaVinci Resolve and After Effects
- https://github.com/microsoft/PowerToys/issues/48789 (22.06.2026, accessed 2026-09-07) — the request to return the hold; the maintainer's answer about the return
- https://github.com/microsoft/PowerToys/issues/49975 (17.08.2026, accessed 2026-09-07) — the price of the overlap, verbatim
- https://forum.borisfx.com/t/keyboard-shortcuts-for-syntheyes/20970 (20.01.2026, accessed 2026-09-07) — a user assembles a cheat sheet himself and publishes a PDF
- https://help.magix-hub.com/video/vegas/en/content/topics/customizing_keyboard_shortcuts.htm (accessed 2026-09-07) — VEGAS: the list of assignments lives in Customize Keyboard; a separate page "keyboard shortcuts" was not found on this mirror
- https://developer.apple.com/documentation/uikit/uikeycommand (accessed 2026-09-07) — `discoverabilityTitle`, "An elaborated title that explains the purpose of the key command": a platform API for form 3 exists, but a description of the behavior "hold ⌘ → list" is absent from the current version of the page — [NOT VERIFIED], therefore this platform is not entered into the tables

Trackers (dumps in `~/.cache/kinotrain/trackers/`, they do not enter the repository — `_trackers.md`, the section on licenses; both numbers of the rows `SIGNAL:` are reproduced by `python3 src/scripts/tracker-rank.py --query '<filter>'` with the filters from the backticks, accessed 08.09.2026):

- `losslesscut` — https://github.com/mifi/lossless-cut/issues/726 (18.04.2021, open), `/19` (04.01.2017, closed by implementation), `/1996` (14.05.2024) and `/549` (13.12.2020) — from the last two it is visible that the window "Keyboard & Mouse Shortcuts" is called by the Help item and the key `H`, that is, form 2 was already there when form 1 was requested
- `openshot` — https://github.com/OpenShot/openshot-qt/issues/5447 (20.02.2024, open): export "command vs hotkey" from the Preferences dialog
- `flowblade` — https://github.com/jliljebl/flowblade/issues/620 (14.02.2019, closed by a fix): the layout dialog intercepts all input; and `/1199` (28.08.2025, open) — `Backspace` in the layout editor crashes Flowblade with a traceback: exactly that second binding of deletion which we have neither in the cheat sheet nor in the catalogs
- The five core editors are not here and cannot be: there is no public tracker for any of them (`_trackers.md`, "Whom we do NOT take — and why")

Internal sources (verified by grep and reading 2026-09-07):
`src/apps/kt-app/src/app.rs:1103-1127`, `:1712-1752`, `:3344`, `:3546`,
`:3879-4020`, `:4472-4490`, `:5069`; `src/apps/kt-app/src/timeline.rs:1275-1333`,
`:1681`, `:2343-2500`, `:2977`, `:4401`; `src/apps/kt-app/src/meters.rs:780`;
`src/apps/kt-app/src/automation.rs:547`; `src/apps/kt-app/src/toolbar.rs:230`,
`:239-345`, `:388`, `:447`; `src/apps/kt-app/src/icons.rs` (39 constants);
`src/apps/kt-app/src/i18n/bidi.rs`; `src/apps/kt-app/build.rs:42`;
`src/apps/kt-app/i18n/*.toml` (522 values in each);
`site/content/structure.toml:18-24`, `:147-152`, `:200-244`;
`site/content/*.toml` (section `[keys]`, 36 values in each);
`site/build.py:54`, `:120-162`, `:382-392`, `:665-680`, `:805`. The translation coverage of the landing page was recounted by the algorithm of `site/build.py:validate` itself:
ru — 98.1%, es / zh-Hans / ar — 0.0% (215 out of 215 values coincide with the English byte for byte).
