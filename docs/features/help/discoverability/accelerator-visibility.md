---
id: F-HLP-017
title: Visibility of hotkeys — accelerator next to the command
status: done
priority: B
cost: S
category: help/discoverability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: []
ui_rules: [9, 13, 17, 18]
round: "R.49"
point: 362
tests: [392, 393]
depends_on: [F-UI-002]
trackers: [losslesscut, kde-bugzilla, shotcut, kde-invent, openshot]
checked: 2026-09-15
---

# F-HLP-017 — Visibility of hotkeys — accelerator next to the command

## Essence

To the right of a menu item, in a separate column, stands the chord by which the same item is invoked from the keyboard. The editor did not ask about keys: he came to the menu for a command and left with it — and, in passing, took away the knowledge that next time he can skip the menu. This is the only channel for learning a key that asks nothing, overlaps nothing, and takes up no pixel while the menu is closed. Its price is coverage: it teaches exactly those commands that are in the menu, and says nothing about everything that lives in a gesture on the timeline.

## Who has it and how

We look not at WHERE the key is reassigned (that is F-UI-002), but at where it is VISIBLE to someone who was not looking for it.

| Editor | Version | What it is called | How it is organized |
|---|---|---|---|
| VEGAS Pro | 2026 | accelerator in the menu; Customize Keyboard as a reference | Standard Windows menus: the chord in the right column next to the item. The full list is only in `Options → Customize Keyboard`, which one comes to on purpose: the field "Show commands containing" searches by command name, the field "Shortcut keys" shows the assigned key (F-UI-002). The program has no "cheat sheet" of its own |
| Adobe Premiere | 26 | accelerator in the menu; Keyboard Shortcuts as a map | Menus carry the chord; `Edit → Keyboard Shortcuts` is a DRAWN keyboard, that is, the reverse mapping "key → command" plus a view of which keys are free. Switching the `Keyboard Layout Preset` (included layouts of Avid and Final Cut Pro 7) rewrites both the menus and the map at once (F-UI-002) |
| DaVinci Resolve | 21 | accelerator in the menu; Keyboard Customization | Menus carry the chord; `DaVinci Resolve → Keyboard Customization` (⌥⌘K) — a list of commands with the assigned chord. A subtlety of the page model: one key means different things on Edit and on Fairlight, so the "truth" about a key depends on where you stand (F-UI-002) |
| Final Cut Pro | 12.3 | accelerator in the menu; Command Editor | Menus carry the chord; the Command Editor (⌥⌘K) is a keyboard PAINTED by groups of commands, and a search that accepts not only the command name and description words, but also the KEY NAME (F-UI-002). That is, the only one of the five where the reverse question "what does `Q` do" is asked by text, not by looking around |
| After Effects | 26 | accelerator in the menu; key in the tool label | Menus carry the chord; for the panel tools the chord stands right in the hint label, in parentheses ("Selection Tool (V)") — that is, the same device as our `[tb]` [NOT VERIFIED: `helpx.adobe.com` did not open from this environment; the device is known from Adobe's long-standing practice, not confirmed by a separate source]. The full map is `Edit → Keyboard Shortcuts`, a visual editor since 2017 (F-UI-002) |

**Outside the core.** A third channel of learning — "next time press X" after an action with the mouse — does not occur in video editing at all, and its price cannot be shown with five editors; and the surface "search by commands, where the chord stands next to the found line" comes from code editors, not from NLEs.

| Program | Version | What it is called | How it is organized |
|---|---|---|---|
| Visual Studio Code | 1.136 | Command Palette + Keyboard Shortcuts editor + printed cheat sheet | The chord sits in THREE places: next to the menu item, on the right in the Command Palette line (`Ctrl+Shift+P`), and in the editable table `Ctrl+K Ctrl+S`, where "All the keyboard shortcuts are rendered in the UI using the current system's keyboard layout". Separately — `Help → Keyboard Shortcut Reference`: "a condensed PDF version suitable for printing as an easy reference". A right click on a UI element gives `Configure Keybinding` — that is, a path from the widget to its key [NOT VERIFIED: no direct phrase about right-aligning the chord in the palette was found in the documentation, although the behavior is common knowledge] |
| IntelliJ IDEA | 2026.2 | Find Action / Search Everywhere; Key Promoter X | The chord is shown next to the found command in the search by actions — this is the same device as the VS Code palette. And the channel "after an action with the mouse" is NOT shipped by the VENDOR: the page "Mastering keyboard shortcuts" recommends a third-party plugin — "Key Promoter X is a plugin that shows a popup notification with the corresponding keyboard shortcut whenever a command is executed using the mouse". The most effective channel of the last thirty years did not become built-in even where keyboard culture is at its highest |

**Three channels of learning a key — and they are not interchangeable.** The difference between them is not in the presentation, but in WHAT has to happen for the channel to work.

1. **Accelerator in the menu.** Works when the editor has already gone to the menu. The cost is zero: no pixel of permanent screen, no delay, no overlap. Coverage — only menu commands. Direction: pull by form, push by effect (the person came for a command and took away a key as well). This is the "default channel" of all five core editors.
2. **Key in the hint.** Works when one hovers over an object. Coverage is wider — hints exist on buttons and timeline zones, that is, on gestures that are not in the menu. We have this channel and it works (F-HLP-001), but the chord is written into the TRANSLATION STRING by hand: about 30 values per language, about 150 across five, and none of them is checked by anything. The cost is five catalogs and the 160-character limit (F-HLP-033).
3. **"Next time press X" after an action with the mouse.** Works exactly at the moment when the knowledge is applicable and the person has just paid for not knowing with extra movements — that is why it is the most effective of the three. And the most expensive: to say "next time" one must (a) know that the command was executed with the mouse, not the key, (b) interrupt the person right after a successful action, (c) cover part of the screen with a pop-up window, and (d) keep a counter to fall silent. Items (b) and (c) are exactly items 1-2 of the owner's decisions (item 14 of R.8), and the fade timer is a conversation with invariant 12 ("repaint only on MOTION"). In video editing this channel is absent in each of the five; in the IDE it is held up not by the vendor but by a plugin. The analysis of the price ends here: **this channel is not offered by us.**

This note takes the first channel and fixes the second; the third is named so that it is not returned to as "obviously the best".

## The trick behind it

The load of this mechanism is not the chord, but the **column**: a dozen chords aligned along one right edge and read with one glance from top to bottom. A status-bar line shows exactly one hint at a time and therefore can never place two keys side by side under any conditions — it is impossible in principle to learn from a sequence of single lines that `I` and `O` are neighbors in meaning, or that `Ctrl+Z` and `Ctrl+Shift+Z` are a pair. And here is where the load goes with us: **we have no column — we have only text**. `Toolbar::menu_btn` glues together `"{label}\t{accel}"`, and in epaint 0.36.1 `\t` is not a tab stop but a fixed width of four spaces (`text_layout.rs:264`, `tab_size = 4.0`). The chords in our menu stand in a ragged staircase — each starts where ITS label ended, that is, on its own horizontal line, and differently in each of the five languages — that is, the mechanism is copied letter for letter and lost in essence.

Three details that nobody has in a list of features.

1. **An empty cell is an assertion.** The column reports not only "command X is invoked by key Y", but also, for free, "command Z has no key" — because an empty place among the filled ones is read as an answer, not as a gap. No other surface is able to state a negative fact: a cheat sheet enumerates what exists, command search is silent about what it did not find, a hint about a nonexistent key will say nothing at all. The practical consequence is visible only once one has worked with it: in an editor with a column, the editor stops looking for a key after two seconds, not after ten minutes of a forum.

2. **The menu and the drawn keyboard answer DIFFERENT questions, and therefore live together.** The menu answers "how to invoke this command" (command → key); the keyboard in Premiere's layout window and in Final Cut's Command Editor answers "what does this key do" and "which keys are free" (key → command). The second question arises exactly for the one who assigns his own, the first for everyone else. Neither surface replaces the other, and in none of the five core editors has one cancelled the other in thirty years.

3. **A chord written into a label by hand is four sources of truth for one piece of knowledge** (the handler, the menu literal, the translated phrase of the catalog, the site's cheat sheet), and they diverge not hypothetically. Our Spanish catalog already calls `Delete` by the word `Supr`, while the menu on the same screen says `Del`; `Shift` in the hint became `Mayús`, while the menu kept `Shift`. None of the roughly 150 hand-written keys in the five catalogs is checked by anything, whereas the only chord taken from the code is checked by test t129.

4. **The "menu + tooltip" channel is incomplete for the one who implemented it best — and the vendor admits this in print.** Adobe is the only one that documents the display of the chord in a tooltip verbatim ("*Hover your pointer over the tool or button. A tooltip will appear, and if a shortcut exists, it will appear after the tool description*") — and on the same page it states the gap itself: "*Check the tables for frequently used shortcuts that don't appear in tooltips or menus*". That is, even a full implementation of both channels does not cover the FREQUENTLY USED commands, and the vendor refers to a printed table. For us this removes the temptation to consider "delivering the accelerator to all 20 menu items" as the completion of the task: Adobe has delivered it, and a third surface is still needed (F-HLP-024).

5. **A layout editor may fail to cover exactly those keys that are used most often.** After Effects documents a section "Exceptions": the following cannot be reassigned — `A, AA, E, EE, F, FF, L, LL, M, MM, P, PP, R, RR, S, SS, T, TT, U, UU` — that is, the entire set of property-expansion keys, the product's most frequently used keys, lives OUTSIDE the shortcut editor. The mechanism exists and has a documented hole in its own center. The direct consequence for us: the list of commands and the list of REASSIGNABLE commands are not the same thing, and this difference must be resolved in the registry (F-UI-002), not discovered later.

## Pros

- The channel fires at the moment when the editor is ALREADY going for a command with the mouse: learning costs him not a second and not a single extra movement, because it piggybacks on an action he would perform anyway.
- While the menu is closed, the channel takes zero pixels and zero frames. A permanent panel with a list of keys pays with screen every second; the column — only in the fractions of a second when the menu is open.
- Assignment and display are read from one place, so a change of layout is visible in full and instantly: having chosen in Premiere the preset "Avid Media Composer 5", you see the new chords in the same place where you were used to seeing the old ones, and nothing needs to be relearned or reread separately [NOT VERIFIED live: the presets are confirmed by F-UI-002, the synchronous redrawing of the menu columns is a consequence of the single source].
- The absence of a key is also a message: an empty cell among filled ones says "this command has no key", and there is nowhere else to say it.
- It requires neither delay nor overlap: the column is drawn inside an already open item, nothing pops up and nothing covers anything. It is compatible with items 1-2 of the owner's decisions (item 14 of R.8) word for word, without a single caveat.

## Cons

- Coverage equals the menu. Everything that is invoked by a gesture — the wheel, a drag with a modifier, holding a key — is not expressed by a menu item and therefore is never learned by this channel. Our measure is precise: of the 30 lines of our own cheat sheet (`site/content/structure.toml`, `[keys.*]`), 7 are expressed by a menu item, the other 23 — transport, navigation and gestures — are expressed by nothing.
- The column is paid for by the whole program, while one item benefits: the width of the menu is set by the longest chord, and `Ctrl+Shift+Alt+…` stretches a list whose labels are three times shorter (rule 13 of the checklist).
- The channel does not reach the one who does not go to the menu. An editor who lives in the context menu and the toolbar — and with us that is the main scenario: the three root menus carry 18 commands, whereas `Command` in `src/libs/kt-model/src/model.rs:98` has 45 variants, and that is without transport, navigation and view — will never see the column even once.
- One item — one chord. A second binding of the same command is not expressed by a column: with us `Backspace` and `Ctrl+Y` are inexpressible structurally, not by oversight.
- While the chord is TEXT, not data, it lies silently, and it lies in one language. The error is visible only to someone who looks at precisely this translation, and only with his eyes.
- It teaches the key, but not the MEANING: the column says "this is `S`", and says nothing about `S` cutting at the cursor rather than at the selection. A paragraph "why" does not fit on this surface and moves to the hint and to the site.
- **Display generates its own class of complaints — about inconsistency, and the complaint arrives as a BUG.** Shotcut #1821 was filed not because of the absence of keys, but because the chord stands at "Lift (Z)" and does not stand at Properties, Files, Mute and the track lock. As long as it is shown nowhere, the price is zero; as soon as the column appears on one surface, every surface without it becomes a defect — and with us there are more such surfaces than menus: the toolbar, track labels, timeline zones, transition handles. This is an obligation to keep ONE form on all of them, and it is open-ended.
- **Display immediately creates demand for reassignment.** In the same trackers the request "make the keys customizable" is an independent genre (flowblade #364, losslesscut #492, pitivi #1824, olive #2158): having seen a chord, a person immediately asks whether another can be set. A column without a registry (F-UI-002) answers "no" with silence, and the price of this answer comes not to this note, but to the next one.

## What users say

SIGNAL: "give a key to this action" — 11 tasks in losslesscut, `(add|allow|provide|please|need|request|suggest|make|missing|no)[^.]{0,25}(keyboard short|hot ?key|key ?bind|shortcut key)|(keyboard short ?cut|hot ?key)s? (for|to) \w`; share 0.6 % against 0.1 % in kde-bugzilla

SIGNAL: "show the key that already exists" — 4 tasks in losslesscut, `(show|display|indicate|reveal)[^.]{0,25}(keyboard )?(short ?cut|hot ?key)|list of[^.]{0,15}(short ?cut|hot ?key)|(short ?cut|hot ?key)s?[^.]{0,40}(hover text|hover description|tooltip|menu entr|in parenthes)`; share 0.2 % against 0.0 % (1 out of 6346) in kde-bugzilla

**The hypothesis is tested by the ratio of these two counts, not by each one separately.** Requests to GIVE a key — 42 across eight trackers (flowblade 3, kde-bugzilla 7, kde-invent 4, losslesscut 11, olive 4, openshot 3, pitivi 8, shotcut 2); requests to SHOW an existing one — 6. A seven-fold skew by itself proves nothing: some requests are honestly about absent keys. Proof is given by a pair of tasks in one tracker with a two-month difference.

On 02.07.2019 in Kdenlive's Bugzilla someone files "Need hot key for Enable / Disable clip toggle" (#409409) — and the task is closed as **INVALID**. On 31.08.2019 another person files "Hotkey for enable / disable clip works only for audio track" (#411466) — and it is closed as **FIXED**. The second task proves the first: to complain that the key WORKS ONLY FOR AUDIO, the key must exist — and therefore it existed two months earlier, when it was being requested to be created. The difference between the two people is only knowledge, and it had nowhere to come from in the interface. Next to it — "[Feature Request] Shortcuts for multicam editing" (#403532, 23.01.2019), closed **WORKSFORME**: the same answer, the same class.

The maintainer who brought this to its form also confirms the price of the class. The LosslessCut task template opens with a checkbox item:

> Check out all the menus and keyboard shortcuts. Maybe the feature already
> exists?
> — template of a LosslessCut issue, visible in the body of each of #2032 (12.06.2024), #2154, #2407 and #2744, https://github.com/mifi/lossless-cut/issues/2032

The item stands first in a list of four and is preceded by a direct explanation ("The fewer issues I have to read, the more new features I will have time to implement"). That is, requests for what already exists arrive often enough for the vendor to pay for them with a line in the template; this is his own estimate of the size of the class, not ours. And the checkbox does not help: in all four tasks it is ticked, and the tasks were filed — because one can check "is there already a key" only where the key is written next to the command.

The most precise document on the topic is a thread in which a user asks for a list of keys, and the developer answers that there is no list and none is needed, because the list is the menu:

> There is not. But there are not many keyboard shortcuts in Builder. You can
> find them all from the menus I think.
> — Alan_Lorence (Boris FX), 20.01.2021, thread "Sapphire Effect Builder Keyboad
> shortcuts list", https://forum.borisfx.com/t/sapphire-effect-builder-keyboad-shortcuts-list/10278

The asker, two days later, replies "Ah ok I see that now. Thanks!" — that is, the menu really turned out to be the reference, and no other was needed. This is exactly the case for which the mechanism exists: there is no cheat sheet, and the knowledge got through.

The reverse side — what happens when the accelerator is not shown in the menu and the tool is invoked not from the menu:

> it's 2023 and theres no shortcut custom window, tooltips, or anything… I dont
> even know how use the erase tool for masking without manually clicking it, it
> doesnt tell you in manual
> — tigerkingstream3, 20.01.2023, thread "Keyboard shortcuts?",
> https://forum.borisfx.com/t/keyboard-shortcuts/15702

The vendor's answer on the same day lists exactly two channels, both pull: "Every icon has a tool tip. You can see the tooltip by hovering over the icon. A list of keyboard shortcuts is provided in a chapter within the user guide." The complaint, however, was not about the absence of keys, but about the program not telling about them.

A separate artifact: "show the key in the menu" appears in changelogs as an independent feature, not as decoration — "Insert point mode hotkey (i or Insert) now shown in Tools menu" (Imagineer Motor 1.1.0b4, 01.09.2006, https://forum.borisfx.com/t/01-sep-06-motor-v1-1-0b4/1372). The line about the label now showing the key stands in the list of novelties next to render fixes.

The quotes that are precisely about the VISIBILITY of the accelerator were provided by trackers that were not accessible at the first writing of the note. None of the five core editors has a public tracker (`_trackers.md`, section "Whom we do NOT take"), therefore the count and the quotes come from open video editors; the mechanism there is the very same.

The first one describes our own defect in someone else's words — the chord is shown on some elements and not shown on others — and was filed as a BUG, not as a wish:

> Unexpectedly, no keyboard shortcuts are displayed for these controls,
> whereas the keyboard shortcuts *are* displayed for other controls and
> buttons such as "Lift (**Z**)" in their hover text.
> — issue #1821, Shotcut, 30.04.2026, closed as completed,
> https://github.com/mltframework/shotcut/issues/1821

The second asks for exactly our `[tb]` idiom — the chord in parentheses inside the label — and names the price of not knowing in time. It is closed as a DUPLICATE, that is, it was requested even before it:

> Would be nice if keyboard shortcuts would be added in parentheses - "One
> frame forward (Shortcut)", otherwise it takes more time to learn the
> specific shortcuts.
> — issue #2407, LosslessCut, 07.04.2025,
> https://github.com/mifi/lossless-cut/issues/2407

The third is the reverse side of the same: a command that has a key but has no menu item is not found at all.

> The step (b) I only found by accident because it seems that it is only
> available as a keyboard short cut. Surely it is so useful and important that
> it should be in the File or Edit menu?!
> — issue #876, LosslessCut, 02.10.2021,
> https://github.com/mifi/lossless-cut/issues/876

This is exactly our case with `Ctrl+Y` and `Backspace`: the handler knows the key, but neither the menu nor any of the five catalogs names it, and it can be found only by accident — like the "by accident" of the author of #876.

Separately, consider the age of the request: "show the chord in the hint" was asked of Kdenlive as far back as 2015 ("SUGGESTION - 15.08 Show keyboard shortcuts in tooltip pop-ups when mouse hovers over", #351721, 24.08.2015, RESOLVED FIXED), and the same request came to Shotcut in 2026. Eleven years is a sufficient term to consider the invisibility of accelerators not a shortcoming of one product.

For the five core editors the former gap remains: `helpx.adobe.com` did not open either directly or through `r.jina.ai` (HTTP 422 and "Stream error in the HTTP/2 framing layer"), `documents.blackmagicdesign.com` broke off the manual download on timeout, `www.vegascreativesoftware.info` redirects to forum.borisfx.com. Through the Discourse API of the Boris FX forum (`/search.json`) the queries "Customize Keyboard Vegas", "shortcut not listed in menu", "learn keyboard shortcuts Vegas", "shortcut keys tooltips", "didn't know there was a shortcut" were run — the threads above were found there, but all of them are about Sapphire/Silhouette/Motor, not about VEGAS Pro.

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-reach/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
The accelerator is visible in the menu item and in the hint from one source (the i18n catalog); t83 keeps the ban on pop-up hints, the hint is the status line.
ADDRESS: src/apps/kt-app/src/toolbar.rs:239 — the class from shotcut #1821 ("the chord is shown on some elements and not on others") is POSSIBLE with us and is caught by nothing. `menu_btn` glues `"{label}\t{accel}"` for the seven items of the main menu, and the timeline's context menu is drawn with a bare `ui.button(tr(k::tl::menu::COPY))` (`src/apps/kt-app/src/timeline.rs:4750`), where the chord stands inside the translated label, in parentheses: two different forms of one piece of knowledge on one screen — exactly what #1821 was filed for. This surface is guarded by one check, t129 (`src/apps/kt-app/src/i18n_tests.rs:134`), and it looks only at whether the tab has not moved inside an Arabic phrase. The class from losslesscut #876 ("the command is available only under a key") is present with us in pure form: `Ctrl+Y` and `Backspace` (`src/apps/kt-app/src/app.rs:4045` and `:4041`) occur in none of the values of any of the five catalogs (checked by grep on 08.09.2026; the catalog has 529 values today).

## Estimate

| What | Answer |
|---|---|
| Volume | S — one stage of a round. Key point: nothing needs to be invented at either end — the chord type, its formatting and the column exist in egui 0.36 (`KeyboardShortcut`, `format_shortcut`, `Button::shortcut_text`), and the list of chords with translations exists in `site/content/structure.toml` |
| Where the main work is | `kt-app`: `toolbar.rs` (one table of chords instead of seven literals), the context menus of `timeline.rs`, the catalogs `i18n/*.toml` (about 30 values per language lose the bracket). The data already exists — `site/content/structure.toml`, `[keys.*]`, 30 lines; where it should live so that both the program and the site see it is a question of the round, not of the note |
| Project format | does not touch it |
| Tests that must be written | `reach_tests::shortcut_text_has_one_source` — a check "handler ↔ display" and a ban on writing a chord into a catalog value; t129 need not be edited, t100/t125/t128 only benefit from shortening the lines |
| Depends on | F-UI-002 (the registry of commands — the source; without it the table of chords lives in `kt-app` as a temporary source and moves into the registry without changing the display), F-HLP-001 (the status line), F-HLP-004 |
| Breaks compatibility | no |

The cost of the catalog lines is not recalculated here — see F-HLP-033; separately it is worth noting that this note does not GROW the catalog but shrinks it: the chord moves from the value into the code, and of 522 values about 30 per language become shorter.

## Verdict

We take it — but not as "add the accelerators to the menu" (there is nothing to add there: all seven menu commands that have a key already show the chord — true, one at a time, although "Delete" and "Undo" have two bindings each), but as "make the chord data and show it from one place". Today one piece of knowledge is written four times — in the handler, in the menu literal, in the translated phrase and in the site's cheat sheet — and the entries have already diverged. As long as it is text, any reassignment of keys (F-UI-002) will immediately make five catalogs liars at once. The second thing the reconnaissance changed in the statement: the main property of the mechanism — the COLUMN — we do not have at all, because `\t` in epaint is not a tab stop but four spaces; the chords stand in a staircase and differently in each language. It is cheaper than it seems: the data in the form of a table is already written and translated (`site/content/structure.toml`), and the column, the muted color and the chord type are already in egui and have simply not been taken.

TEST: reach_tests::shortcut_text_has_one_source — today it fails twice, and both failures are visible without running the program, by reading the files alone. `tl.menu.delete` in `src/apps/kt-app/i18n/es.toml:168` names the key "Supr", while the same Delete in the main menu comes as the Latin literal "Del" from `toolbar.rs:297` (the same with "Ctrl+Mayús+Z" in `es.toml:85` against "Ctrl+Shift+Z" in `toolbar.rs:284`) — rule 17 "paired ways are the same" is broken inside one language; and `Ctrl+Y` and `Backspace` (`app.rs:3933`, `:3929`) occur in none of the 522 values of any catalog, that is, the handler knows a key that does not exist in the program.

ONE LINE: overlap — does not touch | delay — does not touch | capacity — in the status line it DECREASES (the chord leaves about 30 values of each catalog for the code), and it appears in the SECOND COLUMN of an already existing menu item — the very one that `Toolbar::menu_btn` has been drawing through `\t` since 2026 — and in the same kind of column of the timeline's context menu; no new surface is created.

## Sources

**None of the seven programs was checked live** — there is no access to them from this environment; for lines outside the core a live check is not planned either (`_editors.md`, section "Software outside the core"). The mechanics of the layout windows of the five core editors were NOT re-examined: they are verified and recorded in F-UI-002, and a reference to it is given here. The fact that the menus of all five carry the chord in the right column is the standard behavior of their window sets; it was not confirmed by a separate source.

- `docs/features/ui/input/keymap-presets-and-editor.md` (F-UI-002) — verified information on the layout windows of VEGAS / Premiere / Resolve / Final Cut / After Effects, including search BY KEY NAME in the Command Editor
- https://support.apple.com/guide/final-cut-pro/keyboard-shortcuts-ver90ba5929/mac (accessed 2026-09-07) — Final Cut Pro 12.3, `Option-Command-K`, sections "Intro to the Command Editor", "View keyboard shortcuts"
- https://code.visualstudio.com/docs/configure/keybindings (accessed 2026-09-07) — the key editor `Ctrl+K Ctrl+S`, "Configure Keybinding" on right click, `Help → Keyboard Shortcut Reference` ("a condensed PDF version suitable for printing as an easy reference"), rendering of chords in the system layout
- https://www.jetbrains.com/help/idea/mastering-keyboard-shortcuts.html (accessed 2026-09-07) — Key Promoter X as a PLUGIN, not a built-in mechanism
- https://forum.borisfx.com/t/sapphire-effect-builder-keyboad-shortcuts-list/10278 (accessed 2026-09-07) — "You can find them all from the menus I think"
- https://forum.borisfx.com/t/keyboard-shortcuts/15702 (accessed 2026-09-07) — the complaint about "no shortcut custom window, tooltips, or anything" and the vendor's reply
- https://forum.borisfx.com/t/01-sep-06-motor-v1-1-0b4/1372 (accessed 2026-09-07) — "hotkey … now shown in Tools menu" as a changelog line

Trackers (R.41; the dumps live in `~/.cache/kinotrain/trackers/` and do not enter the repository — `_trackers.md`, section "Licenses"). They closed the honest note that stood here about a quote not found: quotes precisely about the VISIBILITY of the accelerator were found, only not at the five core editors — none of them has a public tracker.

- https://github.com/mltframework/shotcut/issues/1821 — "Toolbar entries and track controls unexpectedly do not show keyboard shortcuts in their hover texts", 30.04.2026, closed/completed (accessed 2026-09-08)
- https://github.com/mifi/lossless-cut/issues/2407 — "Add shortcut in hover descriptions", 07.04.2025, closed as a duplicate (accessed 2026-09-08)
- https://github.com/mifi/lossless-cut/issues/876 — "Shortcut(s) don't have menu entries", 02.10.2021 (accessed 2026-09-08)
- https://github.com/mifi/lossless-cut/issues/2032 — the body contains the LosslessCut issue template with the first item "Check out all the menus and keyboard shortcuts. Maybe the feature already exists?", 12.06.2024; the same template is visible in #2154, #2407 and #2744 (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=409409 — "Need hot key for Enable / Disable clip toggle", 02.07.2019, RESOLVED **INVALID**, paired with https://bugs.kde.org/show_bug.cgi?id=411466 — "Hotkey for enable / disable clip works only for audio track", 31.08.2019, RESOLVED FIXED: the same key, two months, two directly opposite conclusions (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=403532 — "[Feature Request] Shortcuts for multicam editing", 23.01.2019, RESOLVED WORKSFORME (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=351721 — "SUGGESTION - 15.08 Show keyboard shortcuts in tooltip pop-ups when mouse hovers over", 24.08.2015, RESOLVED FIXED (accessed 2026-09-08)

Both `SIGNAL:` lines and the breakdown "42 requests across eight trackers" are reproducible with the command `python3 src/scripts/tracker-rank.py --query '<filter from the backticks>'` on the dump of 08.09.2026: 12 dumps, 28,537 records. `blender-vse` and `ffmpeg-trac` were still downloading at that time (the files are empty) and do not participate in the percentage comparisons. The states of tasks (INVALID/WORKSFORME/duplicate) were taken from the dump's fields, not from reading the threads.

Internal sources (checked by grep and reading on 2026-09-07; the references to `toolbar.rs`, `timeline.rs`, `app.rs` and the catalogs were re-checked on 2026-09-08): `src/apps/kt-app/src/toolbar.rs:239-345`, `src/apps/kt-app/src/app.rs:3890-4020` and `:4101`, `src/apps/kt-app/src/timeline.rs:1258-1320` and `:4341-4732`, `src/apps/kt-app/src/i18n_tests.rs:134`, `src/apps/kt-app/i18n/*.toml` (522 values in each), `site/content/structure.toml` (`[keys.*]`), `site/content/*.toml` (`[keys]`), `site/build.py:665-680`, `src/libs/kt-model/src/model.rs:98`. The behavior of egui was checked against the sources of the installed versions: `egui-0.36.1/src/widgets/button.rs:217-237`, `egui-0.36.1/src/context.rs:1731`, `egui-0.36.1/src/data/input/keyboard_shortcut.rs:11`, `epaint-0.36.1/src/text/text_layout.rs:262-270`.

**Implemented in R.49** — item 362 (tests t392, t393).
