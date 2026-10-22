---
id: F-HLP-020
title: Empty states that teach, and grey widgets that stay silent
status: candidate
priority: B
cost: M
category: help/discoverability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2]
we_have: partial
crates: [kt-app, kt-media]
invariants: [8, 12]
ui_rules: [9, 10, 13, 18, 20]
round: null
point: null
tests: []
depends_on: [F-HLP-001, F-HLP-010, F-HLP-033, F-UI-002]
checked: 2026-09-07
---

# F-HLP-020 — Empty states that teach, and grey widgets that stay silent

## Essence

A container in which nothing is yet is an area of screen for which no one pays: it is already empty there. An empty timeline, an empty bin, an empty effect chain, an empty automation graph — these are the only moments when the editor has no object to look at and has attention for one phrase. The reverse side of the same place is the grey widget: it occupies area, promises a function and does not name the condition under which it will switch on. The note treats these two states as ONE decision: what to occupy the empty place with, and what to show where today there is grey.

## Who has it and how

The question of the row: what stands IN the empty container itself (not in the manual, not on the welcome screen) and what stands instead of an unavailable item.

| Editor | Version | What it is called | How it is organized |
|---|---|---|---|
| VEGAS Pro | 2026.0 b66 | "Building peaks…" | The only thing checked live against a first-hand source, the forum: the "not ready yet" state of the waveforms is NAMED and visible — so noticeable that threads are opened about "how to turn it off". An empty timeline carries no instructional text, according to all the found thread screenshots; unavailable items fade without a reason **[NOT VERIFIED live]** |
| Adobe Premiere | 26.x | Home screen · Import mode · Learn | An empty project is solved not by text in a panel, but by a separate MODE of the window: the tabs Import / Edit / Export at the top, and the empty state is a whole import screen instead of empty panels. Plus the Learn workspace with interactive lessons **[NOT VERIFIED: `helpx.adobe.com` does not open from this environment, see "Sources"]** |
| DaVinci Resolve | 21 | Project Manager | The empty state is raised to the level of the application: the program starts not in an empty edit, but in the project manager; an empty Media Pool shows its own toolbar and nothing else **[NOT VERIFIED live]** |
| Final Cut Pro | 12.3 | Create Library → Import Media | Checked against the guide: "The first time you open Final Cut Pro, it contains no media", and further the empty state of the MAIN window is two buttons in a row, `Create Library`, then `Import Media`. Separately, `Final Cut Pro > Download Demo Project` — a ready edit right from the menu. The reason for greying ("is dimmed if …") is written in the guide: the word `dimmed` occurs 20 times, in the program — never |
| After Effects | 26.x | Home screen · empty Composition | The empty composition panel offers to create a composition by a link in the panel itself, not only by a menu item **[NOT VERIFIED: the same `helpx.adobe.com` barrier]** |

There is not a single "no" row here: all five have an empty state — the only question is whether it TEACHES or merely exists. It is impossible in principle to verify this from documentation: the vendor describes functions, not what is visible when the functions have not yet been applied. Hence the three `[NOT VERIFIED]` marks, and hence the second table — there the mechanism is not spied on, but is recorded by a specification.

**Outside the core.** Without these two programs the feature cannot be described: in one the empty state is formalized by a written guideline with a mandatory composition, in the other it is implemented so that it reads the live keyboard layout and changes the composition depending on the state of the window. The five NLEs provide neither — there an empty state exists, but no one has published its rules.

| Program | Version | What it is called | How it is organized |
|---|---|---|---|
| IntelliJ IDEA | 2026.2 | Empty State (guideline + `ComponentWithEmptyText`) | A mandatory composition of three parts: "A reason why the container is empty", "Actions to fill in the area", "A link to the corresponding help topic". One or two actions, no more. A separate rule: if there is no action at all, still "add a text describing all the required steps" |
| Visual Studio Code | 1.136 | Editor watermark (`workbench.tips.enabled`) | The empty area of the editor is a list of commands, next to each of which is its LIVE hotkey: `editorGroupWatermark.ts` asks `IKeybindingService.lookupKeybinding(id)` for each entry and is redrawn on `onDidUpdateKeybindings`. The composition depends on the state: an empty window has its own set, an open folder has another |

## The trick behind it

The empty state carries what one status-bar line will never fit — **the PLACE**. Not the length of the text: the place. The line stands at the bottom right and speaks about what the cursor is on NOW; the empty state stands exactly where the object WILL APPEAR, and is addressed to the state, not to the cursor. Therefore it comes without a request (push), while the line — only on hover (pull), and no capacity changes this: see the thesis of R.40 and F-HLP-001. The load that appears here will, with us, go into the bodies of the empty containers themselves — the body of the timeline, the area under the automation graph, the clip strip without a waveform — and will not add a single character to the status bar.

Three details that are absent from the lists of capabilities.

**1. An empty state is not an addition, but a SUBTRACTION.** The natural move ("let's add a hint and leave the toolbar") is directly forbidden by the IntelliJ guideline: "Hide the area's toolbar if it does not have the same action as in the empty state. Usually, all other toolbar actions are not relevant in this case". That is, the standard answer to the question "what to show instead of the grey widget in an empty container" is NOTHING: the button is removed, and its action moves into the empty state as a link. The grey widget and the empty state turn out to be not two topics, but two halves of one decision, and the second half cancels the first. The same place sets a vocabulary: "avoid words that describe physical actions like press or click", "avoid saying `add new`. Just use `add`", and a non-breaking space between the name of an action and its key — because in the empty state the key must stand next to the action and not wrap onto the next line.

**2. The VS Code empty state has no right to lie about a key.** The list of commands in an empty editor looks like a static picture, but `editorGroupWatermark.ts` assembles it with three filters in a row: the command exists in the registry, its `when` is satisfied, and — last — `.filter(entry => !!this.keybindingService.lookupKeybinding(entry.id))`. A command with no assigned key DISAPPEARS from the empty state, and the key itself is not written into the text, but is requested from the layout service and redrawn on `onDidUpdateKeybindings`. Plus two touches visible only in the code: extra entries are shuffled (`shuffle`) down to a minimum of three items so that the same five do not petrify, and the base set is taken different for `WorkbenchState.EMPTY` and for an open folder. The empty state there is a function of the state and of the layout, not a line in a catalog.

**3. A teaching empty state is the only hint that removes itself.** It disappears exactly at the moment it stops being needed: the first clip is placed — there is no text any more, and there never will be. The tip of the day, the tour, the banner have no such property: they have to be closed by hand, and therefore they irritate. This also removes the main argument against: rule 13 of the checklist ("DO NOT WASTE SPACE FOR NOTHING") is inapplicable here by construction: the space that the empty state occupies is already occupied by emptiness.

## Pros

- The place is free and exactly one: the text stands where the object will appear, so "where to do it" and "what to do" are answered with one glance, without moving the eyes to the status bar and without hovering the mouse.
- Self-removal: the hint lives exactly as long as the state lasts — it needs neither a "don't show again" button, nor a setting, nor a counter of showings, nor storage of "the user has already seen this" (and today we have nowhere to store it — conclusion no. 1 of the first wave about the absence of user storage outside the project, `docs/features/README.md`).
- Push without overlap and without delay: nothing pops up, nothing covers anything, nothing waits — items 1 and 2 of the R.40 thesis are not touched at all.
- An empty state is a natural carrier of a hotkey (the IntelliJ rule "It can also educate about the shortcut"), and the key is exactly what we nowhere describe: 13 of 20 menu items have no accelerator, and `Ctrl+Y` and `Backspace` are not named in any line of the catalog.
- Greyness replaced by an empty state removes the question "why is it off" together with the switched-off widget itself, rather than answering it.

## Cons

- The empty state is seen only by a newcomer — and exactly once. An experienced user pays nothing for it, but also gets nothing: the channel does not scale to "the second year of work", unlike the palette or the cheat sheet.
- It lies by construction if its composition is hard-coded: a key that moved on reassignment will remain in the text (in VS Code this is solved by `lookupKeybinding`, and the solution required the layout service — that is, a dependency on F-UI-002).
- It provokes accumulation: the IntelliJ guideline is forced to limit it — "Use one or two actions. Three or more actions would make it harder to choose what to start from" — because the natural move of the author "to add one more tip" turns the empty container into an advertising board.
- The empty state does NOT solve the case "the container is full, but the action is unavailable": there the greyness remains, and its explanation is the subject of F-HLP-010, not of this note.
- Text in an empty area conflicts with rule 9 ("nothing superfluous, do not make obvious labels"): the inscription "here is empty" over emptiness is exactly what the rule forbids. What separates them is only one thing — whether the next gesture is named.

## What users say

A state "not ready yet" that is not named aloud reads as a breakdown. A person sees a solid fill instead of a waveform and cannot distinguish "being built" from "broken" — and goes outside to ask:

> "In Vegas Pro Plus 2026, I have no waveform on the audio track. As my example
> shows, when I import the .MP3 file, it shows up in the audio track as one
> solid color."
> — Debbie-Whitmer, forum.borisfx.com, 31.05.2026,
> https://forum.borisfx.com/t/no-waveform-on-audio-track-using-borisfx-vegas-pro-plus/23845

She found the answer not in the program and not on the forum, but from an outside chatbot, and it boiled down to one menu item and one key combination. The reverse side: when the same state IS named, it is noticed so much that people ask to switch it off — and this is the proof that "Building peaks…" is visible:

> "Is there any way to disable peak building for source video files when opening
> a project and only generate peaks manually when needed?"
> — McVik, forum.borisfx.com, 02.04.2026,
> https://forum.borisfx.com/t/building-peaks-how-to-disable-it-for-source-video-files/21642

And the grey widget through the eyes of the one it promised a function to:

> "The tutorials make reference to the Move up and Move down commands to
> determine layer order, however in every instance I can find those commands are
> greyed out and I don't know why."
> — 626key, forum.borisfx.com, 19.02.2024,
> https://forum.borisfx.com/t/particle-illusion-move-up-down-greyed-out/18124

The developer's answer in the same thread is more instructive than the complaint itself: the commands are not "temporarily unavailable", they should not be there at all — they were left over from the two-dimensional version. The greyness outlived the function and went on promising it for three years.

## How it fits Kinotrain

### What already exists

An inventory from the code: sixteen places where a container can be empty. They divide into three kinds, and among them the teaching ones are **five**.

**Teach — name the next gesture (5):**

| Key | Text (en) | Where |
|---|---|---|
| `bin.empty` | "empty — drop media files on the window" | `src/apps/kt-app/src/bin_panel.rs:412` |
| `fx.chain_empty` | "Chain is empty — add an effect" | `src/apps/kt-app/src/widgets.rs:604` |
| `mrph.no_areas` / `mrph.no_marks` | "…: select a band and save it" / "…: mark one with M and give it a name" | `src/apps/kt-app/src/morpher_editor.rs:754` |
| `tl.menu.pick_something` | "Select a region or a clip" | `src/apps/kt-app/src/timeline.rs:4750` |
| `mrph.source_drop` | "Drop a file here, or drag a card from the media bin" | `src/apps/kt-app/src/morpher_editor.rs:134` |

**State — report the fact of emptiness and nothing more (5):** `win.notices_empty` ("Nothing yet", `app.rs:1114`), `perf.log_empty` ("no problems noticed", `app.rs:5129`), `search.nothing` ("nothing found", `app.rs:1398`), `fxreg.nothing` ("nothing found", `fx_registry.rs:146`), `mrph.src_slot_none` / `mrph.keep_source` ("Source: unchanged" / "— keep the source —").

**Silent — there is no text at all (6):** the body of an empty timeline (`src/apps/kt-app/src/timeline.rs`, no branch); the preview of an empty project (`app.rs::draw_preview`); the waveform while the index is not built (`timeline.rs:3269` — the whole branch is skipped by `let Some(levels) = peaks.get(&c.media)`); the filmstrip while the thumbnails are not decoded (`timeline.rs:3235` — `None` leads to an empty place); the automation widget without curves (`automation.rs:247` — `return out;` AFTER the heading and the two view switches, that is, the caption and the controls above emptiness); the parameter block of an effect without automatable parameters (`fx_editor.rs:395` — `return (false, None);`, the block is simply absent).

Of the sixteen, ten speak in text, five name the verb, and the KEY — exactly one: `mrph.no_marks`, "mark one with M and give it a name". One case out of the 522 catalog keys (`i18n/en.toml`, `N_KEYS = 522`) by chance fulfills the IntelliJ rule "It can also educate about the shortcut".

Separately, there are two standing surfaces that are not empty states but are substituted for them:

- `hint::IDLE` (`app.rs:1293`) — the last branch of the pure `status_line`, "drop a media file on the window — the clip lands under the mouse". It does not depend on ANYTHING: a project with two hundred clips shows exactly the same text as an empty one. This is the direct opposite of the VS Code structure, where the set is taken differently for `WorkbenchState.EMPTY` and for an open folder.
- `tl.hint::EMPTY` (`timeline.rs:2256`) — "LMB — playhead, drag — region…" — lives inside `if self.drag.is_none() && let Some(pos) = hover`, that is, it is shown ONLY ON HOVER. This is the best text in the program about an empty timeline, and it is delivered through the pull channel.

And the first thing that a person who launches the program for the first time sees: `KtApp::new_empty(Some(tr(k::msg::OPEN_A_PROJECT)))` (`app.rs:5287`) — "Open a project: kt-app &lt;file.otio&gt; or drop it on the window". This is `HintKind::Status`, which means `STATUS_TTL_MS = 6000` (`app.rs:4096`): the program's only address to the newcomer **self-destructs after six seconds** and, in half of its length, explains to him a command-line argument.

### What is missing

1. **The timeline has no empty state at all.** This is the main gap of the note, and it concerns the largest container of the window. `Project::new` gives one video and one audio track (`src/libs/kt-model/src/project.rs:3148`), the timeline honestly draws two labelled empty rows — and stays silent. Help appears only on hover (`tl.hint::EMPTY`), that is, exactly when the user has already guessed to move the mouse over the large dark rectangle. The empty timeline at the first launch is the moment when a push is needed more than anywhere else in the program, and it is the only place where the space for it is certainly free.
2. **The index being built is not named by a single word.** `spawn_index_worker` (`src/apps/kt-app/src/util.rs:89`) is a thread without feedback: it puts the finished pyramids into `Arc<Mutex<HashMap<MediaId, Peaks>>>` and that is all. There is not a single key with the word index, building or peaks in the catalogs (checked by grep over `apps/kt-app/i18n/en.toml`). While the index is being built, the audio clip is drawn as a solid fill — exactly the picture because of which Debbie-Whitmer decided the program was broken. Worse: the same fill means both "built, and there is silence" and "building failed". The key point here is that the state ALREADY EXISTS in the data and simply is not read as a state: the absence of a key in `peaks` is "not ready yet", and for the thumbnails the difference is even typed (`ThumbState::Failed`, `bin_panel.rs:655`) and still is shown to no one. This is the same diagnosis as F-HLP-011: counted and lost along the way.
3. **Invariant 8 promises what does not exist.** Its own text: "Waveforms — from the peak pyramid, thumbnails — from the strip in the sidecar index DB. **While there is no index — placeholders.**" There are no placeholders: `timeline.rs:3235` draws nothing when the texture is absent, `timeline.rs:3269` skips the waveform entirely. The promise is written in the root `CLAUDE.md` and is not fulfilled — exactly the failure mechanism by which R.38 was closed ("prose drops nothing").
4. **The standing line is blind to the state.** One surface that is ALWAYS visible is occupied by one and the same sentence for the whole lifetime of the project.
5. **Two grey widgets out of six are in fact empty states.** `timeline.rs:4599` ("Area from region" is grey when `has_region == false`) and `timeline.rs:1808` ("Delete track" is grey when `lanes_of_kind == 1`) — in both cases the condition is an assertion about EMPTINESS ("there is no region yet", "the track is the last one"), and by the IntelliJ rule the answer to them is not "explain the greyness", but "say about the emptiness where it lives". I stress the boundary: the mechanism of delivering the reason to a grey widget is F-HLP-010, where it is also analyzed why the hint of a disabled widget is dead in egui (`hovered()` for disabled is always `false`) and why this is cured by a single predicate. This note owns the previous question — whether the widget should be grey at all.

### What gets in the way

**Rule 9, not rule 13.** Rule 13 ("DO NOT WASTE SPACE FOR NOTHING") is not an opponent here: the space occupied by the empty state is empty without it, and the text disappears together with the emptiness. The real obstacle is rule 9 ("nothing superfluous, do not make obvious labels"): "Chain is empty" above an empty chain is a candidate for an obvious label. What separates them is exactly one sign, and it is checkable: whether a VERB or a KEY is named that is not visible in this area. "Chain is empty — add an effect" passes, "Nothing yet" does not. This is the criterion of the future test, not a matter of taste.

**Rule 20 ("nothing grows by itself") and invariant 12.** The empty state must be drawn ONLY while the container is empty, as static text, without animation and without its own `request_repaint`. Then the cost is zero: `tr()` returns `&'static str` without allocations (`src/apps/kt-app/src/i18n/mod.rs`), and the branch itself executes only in the empty state. Everything that "fades", "pulses" or lives on a timer is forbidden by invariant 12, and this also cuts off tours and coach marks.

**t83 is not touched.** The regression guard `layout_tests::t83_hints_reach_status_line` (`src/apps/kt-app/src/layout_tests.rs:234`) holds two promises at once: the hints reach `status_line_shown`, and there is not a single layer `egui::Order::Tooltip` in the frame. The empty state is drawn as ordinary content of the container (`ui.weak` / `painter.galley`) in the standard layer, creates no pop-up and does not touch `hover` — therefore neither `tooltip_delay = 1.0e6` from `src/apps/kt-app/src/theme.rs`, nor t83 will have to be weakened, and both owner's decisions (overlap, delay) remain untouched. t83 would have to be weakened only in one scenario, which this note does NOT propose: if it were decided to give the explanation of greyness through the standard `Response::on_disabled_hover_text`.

**The real price is words, not code.** Five languages, the ceiling of 160 characters (`src/apps/kt-app/build.rs`), the glyphs of t100, the subsets, Arabic bidi, t125 and t131 — all of this is counted in F-HLP-033 and is not recalculated here. The order of magnitude: eight to ten new keys.

### Which invariants it touches

- **Invariant 8** — touched in its favor: its text already requires placeholders that do not exist. The note does not change the invariant; it closes the gap between the invariant and the code.
- **Invariant 12** — constrains the form: static text only, only in the empty state, no animation and no repaint on a timer.
- **Rule 9** — the main opponent, lifted by the criterion "a verb or a key is named".
- **Rule 10** ("exactly one hint, in one place") — not violated: the empty state is not a hint about the widget under the cursor; it is the content of a container. It does not compete with the status-bar line, because that line is tied to the cursor, while the empty state is tied to the state.
- **Rule 13** — lifted by construction (the place is already empty).
- **Rule 18** ("a menu item says WHAT and WHERE") and **rule 20** — hold the form: one or two actions, disappears without a trace.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages of the round |
| Where the main work is | kt-app: `timeline.rs` (empty timeline; "not ready yet" for the waveform and the filmstrip), `automation.rs` and `fx_editor.rs` (silent early returns), `app.rs` (a stock status line that depends on the project state); catalogs `apps/kt-app/i18n/*.toml`. kt-media: distinguish "building" from "failed" in `spawn_index_worker`/`ThumbState` so that the UI can ask about it |
| Project format | does not touch it |
| Tests that will have to be written | `reach_tests::empty_container_names_the_next_step` — a scanner over the draw paths: every `is_empty()` branch has a text, and the text names a verb or a key; a harness check that an empty project shows a line IN THE BODY of the timeline and that it disappears after the first clip; a check that "not ready yet" is distinguishable from "ready and empty"; t100/t125/t128 for the new keys |
| Depends on | F-HLP-010 (delivering the reason to a grayed-out widget), F-HLP-001 (the status-bar line as a pull channel), F-HLP-033 (the price of a word), F-UI-002 (if the empty state shows a live key rather than a literal) |
| Breaks compatibility | no |

What this adds BEYOND the command registry (F-UI-002): the registry answers the question "which key does the command have", the empty state answers the question "which of the commands is appropriate to name RIGHT NOW, in this container, in this state". The first is a table, the second is a choice from it based on context; VS Code shows that the second cannot be built without the first (`lookupKeybinding` on every entry), while the first without the second remains a reference one has to come and look up.

## Verdict

We take it. This is the cheapest push channel of all those the wave found: it does not overlap, does not delay, does not require storage outside the project, does not require a popup, and costs zero pixels, because it occupies a place that is already empty.
We should start not with new texts but with two holes where there is no text at all: the body of the empty timeline and "not ready yet" for the waveforms — and the second of these closes an unfulfilled promise of invariant 8, rather than adding something new.

TEST: reach_tests::empty_container_names_the_next_step — today it fails on the very first container, and the reconnaissance can name the count: of the sixteen places where we have "there is nothing here", the text is produced by ten, a verb is named by five, a key by exactly one (`mrph.no_marks`, "mark one with M"), and the largest container of the window — the body of the timeline — has no empty-state branch at all: `src/apps/kt-app/src/timeline.rs` draws two labeled empty tracks and stays silent until the mouse is placed on them, and the only address to a newcomer (`msg.open_a_project`) disappears after `STATUS_TTL_MS = 6000` ms.

ONE LINE: overlap — not touched | delay — not touched | capacity — zero new characters in the status-bar line; the whole load falls on the BODIES of the empty containers (the timeline body, the area under the automation graph, the clip strip without a waveform), plus the stock line stops being a constant and begins to depend on the project state — no new layer, no popup, t83 remains green without edits.

## Sources

- The "Empty State" guideline, IntelliJ Platform UI Guidelines (updated 04.07.2025) — the mandatory composition of an empty state, the restriction "one or two actions", the rule "Hide the area's toolbar", the vocabulary and the non-breaking space before a key — https://plugins.jetbrains.com/docs/intellij/empty-state.html (accessed 2026-09-07)
- Source code of VS Code, `src/vs/workbench/browser/parts/editor/editorGroupWatermark.ts` (branch `main`) — the composition of the watermark, `SETTINGS_KEY = 'workbench.tips.enabled'`, `MINIMUM_ENTRIES = 3`, the triple filter of entries, `lookupKeybinding`, `shuffle`, different sets for `WorkbenchState.EMPTY` and an open folder — https://raw.githubusercontent.com/microsoft/vscode/main/src/vs/workbench/browser/parts/editor/editorGroupWatermark.ts (accessed 2026-09-07)
- Final Cut Pro User Guide for Mac (PDF, version 12.3), sections "If it's your first import" ("The first time you open Final Cut Pro, it contains no media"), "Download a demo project", 20 occurrences of the word `dimmed` with conditions — https://help.apple.com/pdf/final-cut-pro/en_US/final-cut-pro-user-guide.pdf (accessed 2026-09-07)
- Boris FX forum (Discourse, read via `/t/<id>.json` and `/search.json`): topics 23845 (no waveform — "one solid color"), 21642 ("Building peaks…"), 18124 (grayed-out Move up/down) — https://forum.borisfx.com/ (accessed 2026-09-07)
- Our code and catalogs: `src/apps/kt-app/src/{app,timeline,bin_panel,widgets,automation,fx_editor,fx_registry,morpher_editor,util}.rs`, `src/apps/kt-app/i18n/en.toml`, the root `CLAUDE.md` (invariants 8 and 12), `src/apps/kt-app/CLAUDE.md` (rules 9, 10, 13, 18, 20)

**What could not be verified.** There was no live check for any of the programs: for software outside the core it is not assumed at all (`_editors.md`), and for the NLE core the empty state is not documented by vendors in principle. From this environment `helpx.adobe.com` does not open either directly (the connection drops) or through the text proxy `r.jina.ai` (only the navigation comes back), and Apple's help returns over HTTP only the table of contents — therefore FCP was taken from the PDF guide, and the Premiere, Resolve and After Effects rows are marked `[NOT VERIFIED]`. The session's `WebSearch` budget (200 of 200) was exhausted before this note was started, so all sources were obtained by direct `curl`/`WebFetch` requests to known addresses.
