---
id: F-HLP-015
title: Command palette — search within the program itself
status: candidate
priority: B
cost: S
category: help/discoverability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [13, 14, 16, 17, 18, 20]
round: null
point: null
tests: []
depends_on: [F-UI-002]
checked: 2026-09-07
---

# F-HLP-015 — Command palette — search within the program itself

## Essence

One key opens an input field, the person types the NAME OF WHAT HE WANTS TO DO ("delete area", "morpher", "reverse"), sees a list of suitable commands with a hotkey next to each, and presses Enter — the command is executed. What is searched is not the project's material, but the program itself: its actions, its settings, its windows. This is the only surface where one does not need to know IN WHICH MENU and ON WHICH OBJECT the needed thing lives; one needs to know only the word.

## Who has it and how

| Editor | Version | What it is called | How it is organized |
|---|---|---|---|
| VEGAS Pro | 2026 | none | there is exactly one search of commands in the program — the field "Show commands containing" in `Options → Customize Keyboard`, and it ASSIGNS a key, it does not execute the command (see F-UI-002). Windows-only: there is no system search of menus on this platform |
| Adobe Premiere | 26 | none | MEDIA (Project) and EFFECTS (Effects) are searched; actions — only in `Keyboard Shortcuts`, and also for the sake of assignment. On macOS the system search of menu items (Help) works, on Windows there is none |
| DaVinci Resolve | 21 | none | the same division: Media Pool and Effects Library are searched, actions — only in `Keyboard Customization`. System search of the menus on macOS **[NOT VERIFIED]** live, the mechanism is platform-specific |
| Final Cut Pro | 12 | no own one; the system one | the Command Editor searches a command by name, by keywords of the description and by key name — but in order to reassign it. To EXECUTE a command by name is given by macOS: the search field in the Help menu finds menu items, shows with an arrow where the item lies, and executes it on Enter |
| After Effects | 26 | none | effects and presets (`Effects & Presets`) are searched; there is no search by actions. On macOS — the same system loophole through Help |

The summary for the core: **no one has its own command palette**. All five have search by CONTENT (media, effects) and all five have a list of commands — but it is locked in the window for reassigning keys and can only assign. The only working "search of the program" in this list came from the operating system and is available to four out of five for free and bypassing the vendor.

**Outside the core.** Without these programs the feature cannot be described: it originates from there (VS Code), its upper boundary is shown there too (JetBrains: commands and SETTINGS in one list), and only Blender and Photoshop remove the objection "a palette is an idea for programmers, alien to the editor".

| Program | Version | What it is called | How it is organized |
|---|---|---|---|
| Visual Studio Code | 1.136 | Command Palette | Ctrl+Shift+P; "From here, you have access to all functionality within VS Code, including keyboard shortcuts for the most common operations". One field, modes by prefix: `@` — symbols, `:` — line, `?` — list of modes; the chord stands on the right of the name; a command has a CATEGORY prefix ("Group commands together in the same category") and a visibility condition |
| IntelliJ IDEA | 2026.2 | Search Everywhere / Find Action | double Shift; searches "files, actions, classes, symbols, settings, UI elements, and anything in Git from a single entry point". The chord in the result line ("the Push action … together with the `Ctrl+Shift+K` shortcut"); settings are not only found, but also TOGGLED right from the list |
| Blender | 5.2 LTS | Menu Search (F3) | "The Menu Search pop-up lets you search Blender's interface for a certain operator and execute it" — in a program, without a single line of code on the user's side. The wording is quoted from the page of the 4.0 manual: 5.2 did not open from this environment **[NOT VERIFIED]** |
| Photoshop | 27 | Discover / Search | Ctrl+F (Cmd+F) — the same key that we have taken for project search. Results in three groups: "items that you could find through the Photoshop interface; such as tools, layers, and panels", learning articles and Adobe Stock. That is, Adobe KNOWS how to make a palette and puts it into Photoshop, but does not put it into Premiere or After Effects |

## The trick behind it

The load of the palette is a **LIST**: N items that the user has NOT yet pointed the mouse at, ranked under the word he has typed. A status-bar line by its nature carries exactly one item and only the one that has already been pointed at; a list does not fit into it not by length, but by quantity. For us this load falls onto the already existing Ctrl+F window (`kt_search`), which by its nature is a list, as a second MODE of one field, not as a second window.

**1. In an NLE the palette exists exactly where the operating system provided it, and it teaches not the key, but the PLACE.** macOS puts a search field into the Help menu of any application, and it indexes the menu items: "Hover over one of these with your mouse, and macOS will locate the action for you by pointing an arrow at it in the corresponding menu. Hit Enter, and the action will be performed for you" (MacRumors, 16.01.2019). Two consequences follow from this, which are absent from every list of capabilities. First: three of the five core editors get "search of the program" for free on one platform and do not have it on another — Vegas, which lives only under Windows, does not have it at all. Second and main: the system palette sees ONLY THE MENUS. Everything that lives in the context menu, in a panel button, in a gesture or in a single hotkey does not exist for it — and that is exactly the class because of which the user writes "I don't understand how".

**2. A palette is not a window with a search, but a dispatcher of several indexes, and the prefix is its switch.** In VS Code the same field under `Ctrl+P` searches files, with `>` — commands, with `@` — symbols, with `:` — a line number, and `?` shows the list of available modes. The practical consequence directly for us: "project search" and "program search" are not two windows and not two keys, but two modes of one field. We stand at this fork today: Ctrl+F is already open under the first index, and Photoshop opens the second on the same key.

**3. The chord to the right of the name is the cheapest keyboard teacher, because it collects the payment in advance.** The person came to EXECUTE a command with the mouse and leaves knowing its key: JetBrains shows "the Push action … together with the `Ctrl+Shift+K` shortcut" on the same line, VS Code — "Many of the commands in the Command Palette have default keyboard shortcuts associated with them". A cheat sheet requires the person to come to learn; the palette requires nothing. The reverse side of the same coin is the only trace of misunderstanding that the user leaves HIMSELF, without writing anything: "With an omnibox, each failed search is a point of information to you" (hrishi, HN, 2021). An empty result is not a dead end, but a message about which word the person used to name what he needed. The boundary: the visibility of accelerators in general (in the menu, in a hint, in a cheat sheet) is owned by F-HLP-017; here we are talking only about the chord column INSIDE THE PALETTE itself.

## Pros

- Search by NAME removes the need to know the PLACE. This is the only property that neither the menu, nor the context menu, nor the hover hint has: all three require first hitting the right point on the screen.
- A chord in the result line turns every call with the mouse into a keyboard lesson — a learning channel that does not require the person to come to learn.
- A category prefix in the name ("Git: Commit") keeps a FLAT list of hundreds of commands readable: the menu hierarchy is replaced by a word in the name, and the search does not break from this but improves.
- A visibility condition on a command (VS Code declares it declaratively) means that the list adapts to what is selected — without grayed-out items.
- One field with modes does not create a second surface: no new strip, no new key, no new window.
- A failed query is data. The word with which the user named what he needed is visible only here.

## Cons

- **The palette requires knowing the WORD.** It does not solve discovery as such: someone who does not know that what he needs is called "area" will not find it by any ranking. Our user called it "saved area region" — three words, of which only one is ours.
- It indexes COMMANDS and is therefore blind to gestures. "Change the length of a morpher event" is an edge drag; a drag has no name, neither with us, nor in VS Code, nor in Blender.
- An anti-pattern built into the very idea: the presence of a palette allows declaring a command reachable without giving it a place in either the menu or the context menu. "But it is in the palette" is a legitimate-looking argument, after which the menu stops growing and dead ends become invisible to checks.
- A flat list without a visibility condition gets filled with commands inapplicable to the current selection, and a click on them gives silent nothing.
- "Recent at the top" is a state that grows by itself: the same query from two people gives a different first item, and there is nothing to explain why.
- Localization hits the mechanism itself: the command is searched by name, and the name is translated. A person who learned the English names from others' lessons will not find them in his language — if he searches only by translation.

## What users say

On WHAT exactly the palette gives — verbatim, and it is not about speed:

> With vim I find I'm constantly googling how to do things. With sublime
> learning how to do something usually happens in the editor, thanks to the
> command bar.
> — ollysb, Hacker News, 21.03.2013, https://news.ycombinator.com/item?id=5414566

The negative side, from a person who built the product's entire interface on a palette:

> There is still a lack of discoverability with a command palette. Our solution
> has been to include commonly used commands, and most popular commands
> alongside, as well as introducing some new ones intermittently.
> — hrishi, Hacker News, 04.06.2021, https://news.ycombinator.com/item?id=27394559

The same person — on what is not written in a list of capabilities:

> With an omnibox, each failed search is a point of information to you.
> — hrishi, Hacker News, 04.06.2021, https://news.ycombinator.com/item?id=27394559

And a market signal right in our domain: the video editor Blick, released in 2026 (public beta, Dihedron Software GmbH), sells the palette as a feature, on a par with the waveforms, which it immediately compares with Premiere Pro and DaVinci Resolve:

> The command palette lets you search through every action in the program in
> one place, and run it.
> — blickeditor.com, accessed 2026-09-07, https://blickeditor.com

Separately noted as a gap: in the Vegas forum (Boris FX Discourse, search `/search.json?q="command palette"`) there are ZERO mentions. Users did not learn to ask for a palette in NLEs: there was never one there, and instead they ask "where is this command", one command at a time.

## How it fits Kinotrain

### What already exists

**Half of the palette is built and works** — this is the project search window (item 3 of R.10):

- `KtApp::draw_search` (`src/apps/kt-app/src/app.rs:1332`) — a window with a PERMANENT id `kt_search` (not derived from the title: t129, `layout_tests.rs:1775`), anchor `CENTER_CENTER`, an input field with `hint_text`, focus control via `search_focus`, arrows/Enter/Esc, parsed INSIDE the window — because while focus is in the field, `handle_keys` is not executed at all; a `ScrollArea` of 320 px, `selectable_label` rows, a "nothing found" row, execution of the selection.
- Two entries, already symmetric (rule 17): Ctrl+F (`k.find`, `app.rs:3975`) and the `icons::SEARCH` button in the toolbar (`toolbar.rs:661`), both set `search_open` + `search_focus`.
- `escape_action` (`app.rs:1713`) closes the search as the FIRST step of the cascade.
- `search_hits` (`app.rs:4543`) — a pure function with ranking and test t92 (`app.rs:5486`). Rank: 0 — exact match, 1 — starts with the query, 2 — substring. This is NOT fuzzy search.

**Exactly one piece is not ready — the data source.** `HitKind` has five kinds (`Marker`, `Media`, `Track`, `Clip`, `Group`), and all five are project content. There is not a single line in it about "what the program can do".

**At the same time we have many named commands, and they do not live in the menu.** The catalog (`src/apps/kt-app/i18n/*.toml`): `[tl.menu]` — 38 keys, `[tl.lane]` — 19, `[bin.menu]` — 9, that is, about sixty labels, almost all of them commands reachable ONLY by the right button on the correct object. The main menu (`menu.file` + `menu.edit` + `menu.view`) — about twenty items, and the chord is shown for seven of them, because `Toolbar::menu_btn` (`toolbar.rs:239`) glues it in by hand as a literal.

**A ready example of "registry + search by synonyms"** — `fx_registry::registry()` (`src/apps/kt-app/src/fx_registry.rs:47`): a `&'static [FxEntry]` behind a `OnceLock`; an entry has a name, a category and invisible keywords (`[fxreg.kw]`, 12 keys); the filter is a pure function with test t48. The same place records a decision directly relevant to the palette: "The effect name is NOT translated: it is an industry term, and it is searched by that name in any locale".

**R.38 closed seven "I don't understand how" cases with context menus** — the area (`src/apps/kt-app/src/timeline.rs:4585`, the comment "Before this round the area had NO menu AT ALL") and the morpher track (`timeline.rs:1836`, `tl.lane.add_morpher_here`). This is the same pull surface: to find a command, one must guess to right-click the needed object. The user guessed and wrote "PKM gives nothing" — that is, he performed the gesture CORRECTLY, and still was left with nothing.

**Checking the promise against the seven claims of R.38** (items 1, 2, 3, 5, 9, 13, 14 — the class "I don't understand how"; the rest of the nineteen are about hitting with the mouse, layout and behavior). Would the palette find the command:

| Claim | Would find it | Why |
|---|---|---|
| item 1 delete/rename an area | **yes** | `RemoveArea`/`RenameArea` are named commands; the query "area" answers without knowing that the target is the area's tab itself |
| item 2 morpher on an existing track | **yes** | `add_morpher_here` is a named command, today reachable only by RMB on a morph track |
| item 14 zoom in the automation timeline | **conditionally** | only if "fit to content" becomes a NAMED command; the decision of R.38 was a lock button, that is, a widget |
| item 3 "add morpher layer works strangely" | no | the command was found; the question is about behavior; the palette would have executed the same thing |
| item 5 "how the camera track works" | no | the request is to EXPLAIN; the load is a paragraph, and it is not in a list of commands |
| item 9 length of a morpher event | no | a gesture (edge drag); a gesture has no name for anyone |
| item 13 automation on a morpher | no | the feature did not exist; the palette does not create capabilities, it shows existing ones |

Two out of seven for certain, the third conditionally. This is an honest price of expectations, and it directly contradicts the temptation to regard the palette as a cure for dead ends.

### What is missing

A source of "command" (the registry — F-UI-002); a mode of the input field so that Ctrl+F owns two indexes rather than one; a chord column on the right; a condition of the command's applicability to the current selection; a clear answer to an empty result.

### What gets in the way

**1. There is no command registry, and a palette on top of today's state will become a FOURTH source of truth.** The commands are smeared across `handle_keys` (`app.rs:3750`), `Toolbar::menu_items` (`toolbar.rs:245`) and the context menus of the timeline and the bin. F-UI-002 analyzes this in detail and requires the reverse order: first the registry, from which the menus and accelerators are built, then everything else. The palette is "everything else".

**2. A command has a TARGET, and the palette is invoked without pointing.** More than half of our named commands are contextual: "fade at the start of the clip", "reverse", "delete track" require an object. We already have an answer to this, and it is not invented: operations on the SELECTION (t205, multi-selection t202/t203) — the same thing on which Ctrl+C/Del live today. But then a command that lacks a selection must refuse HUMANLY (rule 16: the impossibility of an action is an error, not a faded hint), rather than silently do nothing.

**3. Invariant 12.** `search_hits` is computed AFRESH EVERY FRAME while the window is open (`app.rs:1339`), and allocates a `Vec<Hit>` with a `String` for each hit. Today this is tolerable — there are dozens of objects in a project. With a hundred commands, their categories and synonyms in five languages, a constant recomputation in the draw path becomes exactly what the invariant forbids; memoization by query is needed.

**4. Language is a fork, not a detail.** t125 requires every catalog key to be non-empty in all five languages, t131 — English literals in the sources. Command names are translated, and a person who knows them from others' lessons searches in English. We already have both ready solutions — "the name is not translated" (`fx_registry.rs`) and "invisible keywords" (`[fxreg.kw]`) — and one must choose between them consciously. The full price of a new line in five languages — F-HLP-033.

**5. Our ranking is substring-based, not fuzzy.** the query "rzrz" will not find "razrez, "cut"", and this is noticeable precisely on commands: command names are long, and they are typed abbreviated (ollysb: "you can actually figure out the 2 or 3 letters that bring up the command you want"). For CJK fuzzy makes no sense — a substring is the right answer there; so the ranking rule depends on the language.

**6. An anti-pattern against t252.** `reach_tests::t252_no_orphan_commands` catches model commands that the UI never invokes. The palette gives a cheap way to make such a test green without fixing anything: add a line to the registry — and formally the command is "reachable". So when the palette is introduced, t252 must explicitly NOT count the palette as reachability, otherwise R.38 will repeat with green tests.

### Which invariants it touches

Engine ones — **12** (repaint only on motion, no allocations or clones per frame in the draw paths): recomputing the list on every frame and on every key press. The feature touches neither the model, nor the project format, nor the rendering, nor determinism.

UI checklist rules: **13** (the palette does not create a new strip — it is a mode of an already existing window), **14** (the window takes focus in the input field and must give it back; the precedent is analyzed directly in `draw_search`), **16** (an inapplicable command refuses with an error, not with silence), **17** (the palette must call THE SAME function as the menu item — this is the requirement of the registry itself), **18** (an item says WHAT and WHERE: on macOS the answer to "where" is given by an arrow in the menu, with us — by a category in the name), **20** (nothing grows by itself: "recent at the top" is precisely a growing state, and it must be introduced consciously or not introduced at all).

## Estimate

| What | Answer |
|---|---|
| Volume | S — one stage of a round AFTER the registry; without the registry — zero, there is nothing to do |
| Where the main work is | `kt-app`: `draw_search`/`search_hits` (`src/apps/kt-app/src/app.rs`) — the mode of the field, a sixth kind of hit, the chord column, memoization; the data comes from the registry of F-UI-002 |
| Project format | does not touch it; "recent" — either not introduced, or eframe Storage, where two lines lie today (`app.rs:5044`) |
| Tests that must be written | `reach_tests`: each menu item is found by its own name set; "the palette and the menu call one function" (rule 17); refusal of an inapplicable command as an error (rule 16); a caveat in t252 "the palette is not counted as reachability". The command names and their glyphs (t125/t100) — the price of the registry, not of the palette |
| Depends on | F-UI-002 (the command registry — a hard condition); the boundary with F-MED-001: there is search by CONTENT, here by the program; with F-HLP-017: there is the visibility of the accelerator in general, here — the chord column in the palette |
| Breaks compatibility | no |

## Verdict

We take it, but strictly as the second step. **What the palette adds OVER the registry** — three things, and none of them follows from the registry: (1) searching a command by NAME instead of knowing its PLACE — today about sixty named commands live in context menus and are found only by guessing which object to right-click; (2) the chord in the result line, that is, learning the key at the moment of a mouse invocation — the registry knows the chord but shows it to no one; (3) a mode of the already existing Ctrl+F field, because of which no second surface is created at all. A registry without a palette changes exactly nothing for the user: the menu remains a menu.

The honest price of expectations was named above in a table: of the seven R.38 claims "I don't understand how", the palette would remove two for certain and a third conditionally; the rest are gestures, behavior and requests to explain. The palette is not a cure for dead ends; it has a narrow and precise target: a COMMAND THAT HAS A NAME AND NO PLACE. Separately, and this is not an ornament of the verdict: none of the five core editors has its own palette, and a working search of the program, where it exists, was provided by the operating system and sees only the menu.

TEST: reach_tests::palette_finds_the_program_not_only_the_project — today it fails: `search_hits` (`src/apps/kt-app/src/app.rs:4543`) returns five kinds of hits, and all five are project data; none of the ~20 items of the main menu and none of the ~60 labels of `[tl.menu]`/`[tl.lane]`/`[bin.menu]` is found by its own name in the window that the user opens with Ctrl+F and which the caption promises as "Search the project".

DEFERRED: until F-UI-002 — the data source of the palette is the command registry, and creating it separately for the sake of search means creating a fourth source of truth next to `handle_keys`, `Toolbar::menu_items` and the context menus; the order "first the registry, then the display" is not advice here, but a condition.

The palette covers the screen — but by the user's call, not by hover, and therefore it is not a pop-up in the sense of R.40: it is a place to which one comes deliberately. It touches neither the delay nor the capacity of the status line.

## Sources

- https://code.visualstudio.com/docs/getstarted/userinterface (2026-09-07) — "access to all functionality within VS Code, including keyboard shortcuts", `?` as the list of modes
- https://code.visualstudio.com/docs/getstarted/tips-and-tricks (2026-09-07) — the modes `@`, `:`, `?`; chords next to command names
- https://code.visualstudio.com/api/ux-guidelines/command-palette (2026-09-07) — "where all Commands are found", the requirement of categories for commands, the visibility condition
- https://www.jetbrains.com/help/idea/searching-everywhere.html (2026-09-07) — a single entry for files/actions/settings, the chord in the result line, toggling settings right from the list
- https://docs.blender.org/manual/en/4.0/interface/controls/templates/operator_search.html (2026-09-07) — "lets you search Blender's interface for a certain operator and execute it". A direct request to the page of version 5.2 from this environment returned 403/404, so the wording is taken from the 4.0 page
- https://www.macrumors.com/how-to/use-the-macos-help-menu/ (16.01.2019, accessed 2026-09-07) — system search of menu items, the pointer arrow, execution by Enter
- https://helpx.adobe.com/photoshop/desktop/get-started/learn-the-basics/access-discover-panel.html (2026-09-07) — Discover via Cmd+F/Ctrl+F; the page did not render through the text proxy, the key fact is taken from search engine results
- https://photoshoptrainingchannel.com/tips/photoshop-search/ (2026-09-07) — secondary: three groups of results, "tools, layers, and panels"
- https://news.ycombinator.com/item?id=5414566 (21.03.2013, accessed 2026-09-07)
- https://news.ycombinator.com/item?id=27394559 (04.06.2021, accessed 2026-09-07)
- https://blickeditor.com (accessed 2026-09-07) — a 2026 NLE that sells a command palette as a feature
- https://forum.borisfx.com/search.json?q=%22command%20palette%22 (2026-09-07) — zero results; the forum has moved to Discourse, the old Vegas threads by the former links are unavailable, therefore the conclusion "they do not ask" is a weak signal, not proof
- The Vegas and Final Cut lines about searching commands in the reassignment window — from the sources of F-UI-002 (help.magix-hub.com, support.apple.com/guide/final-cut-pro)

**What could not be verified.** No program was checked live: for software outside the core this is stated in `_editors.md` as standard practice; for the NLEs the session's search budget is exhausted (WebSearch unavailable, direct requests and the text proxy were used). The negative rows of the core table ("no palette") rely on the absence of the feature in documentation and in the hotkey reference — absence cannot be proven by documentation, and this limitation is deliberate. The presence of the macOS system menu search in DaVinci Resolve **[NOT VERIFIED]**: the mechanism is platform-specific, but Resolve draws its interface on Qt.
