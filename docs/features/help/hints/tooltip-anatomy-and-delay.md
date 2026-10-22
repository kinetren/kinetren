---
id: F-HLP-004
title: Anatomy of a tooltip: delay, content, rich tooltips
status: researched
priority: B
cost: M
category: help/hints
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, photoshop-27, vscode-1.136]
we_have: no
crates: [kt-app]
invariants: [12]
ui_rules: [7, 10, 17]
round: null
point: null
tests: []
depends_on: [F-UI-002, F-HLP-001, F-HLP-002]
checked: 2026-09-07
---

# F-HLP-004 — Anatomy of a tooltip: delay, content, rich tooltips

## Essence

The editor hovers the cursor over a button — and after a pause a panel pops up next to the cursor, over the interface: the element's name, the hotkey, a phrase about its purpose, and, in the most expensive variant, also a silent five-second clip showing the gesture. Move the cursor away — the panel disappears. It appears exactly where you are looking, and exactly there it covers what you are looking at; it has to wait, because without the pause it would flicker on every mouse sweep across the panel.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | ToolTip | pops up at the element. It carries what is not on screen: on the effect button in the channel strip — the FULL name of the plugin and the preset (the button is narrow); during a drag of the envelope — a "floating ToolTip" with the current value; when dragging the snap-offset — the size of the offset. No switch is described in the 2026 help (checked in `prefgeneraltab`, `prefdisplay`) |
| Adobe Premiere | 26.x | Tool Tips | text like "Selection Tool (V)" — the name plus the accelerator. The "Show Tool Tips" switch appeared in the General settings in CC (there was none in CS6); some pop-ups it does not turn off "by design" — clip data on the timeline, marker data, Start/End/Duration above the time bar. The exact name and location of the checkbox in 26.x [NOT VERIFIED]: `helpx.adobe.com` does not open from the environment |
| DaVinci Resolve | 21 | Tool tips | **two surfaces at once**: the name of a node's input/output appears in the status bar IMMEDIATELY, and "after a few moments" the same text is shown as a floating tooltip over the node. During a roll trim the tooltip shows the offset of the edit. A common switch was not found in the manual: "Show Tips" is a local checkbox of the Spline Editor |
| Final Cut Pro | 12.3 | help tag | Apple's pop-up. The classic burden is what the label HID: for a renamed effect the help tag shows the original name. Nearby lives a separate surface with the same purpose — the Skimmer Info window (Control-Y), which shows data under the skimmer and is turned off by a menu item |
| After Effects | 26.x | Tool Tips | the "Show Tool Tips" checkbox in Preferences > General. In its own section "user interface tips" Adobe advises not a tooltip but keeping the Info panel in view. The wording is copied from the CS5 manual; for 26.x [NOT VERIFIED] |

**Outside the core.** Without Photoshop the upper end of the scale cannot be described: rich tooltips were invented there, and there the vendor itself split them from the ordinary ones into two different checkboxes. VS Code is needed for another reason: it is the only one of the programs reviewed where the delay is not a constant in someone else's code but a user parameter, and it has three such parameters.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Photoshop | 27.x | Rich Tool Tips | the pop-up carries the tool's name, the hotkey, the description phrase, and a 5–6 second animation loop showing the tool in use. Two independent checkboxes on the Preferences > Tools page: "Show Tooltips" turns off all of them, "Use Rich Tooltips" switches the rich ones back to ordinary. Enabled by default since CC 2018 |
| Visual Studio Code | 1.136 | hover | `editor.hover.delay` = 300 ms ("delay in milliseconds after which the hover is shown"), `editor.hover.hidingDelay` = 300 ms, `editor.hover.sticky` = true ("remain visible when mouse is moved over it"), plus the mode `editor.hover.enabled: onKeyboardModifier` — the hint is not by time but by a held Ctrl/Cmd. The values are from `src/vs/editor/common/config/editorOptions.ts` |

## The trick behind it

**What the mechanism carries, which a single status bar line cannot hold.** Exactly two payloads, of different natures. The first is TIME: a rich tooltip shows not a text about the gesture but the gesture itself, in a 5–6 second loop; the text "drag while holding Alt" does not retell it, because here what is taught is movement, not a word. The second is LOCATION: an ordinary tooltip is glued to a pixel and therefore can carry a line that is not on screen at all — the full name of a plugin hidden by truncation, or a number that lives only while a drag is in progress. The first payload has nowhere else to go for us: it is forbidden by invariant 12 (repaint only on movement) and moves to the site, into `site/content/*.toml` and `site/screenshots.py`. The second will end up in the same single line — but as DATA, not as a catalog label, and today this is done in two places out of five.

Three details that are absent from the lists of features:

1. **Resolve keeps both surfaces at once, and the only difference between them is the delay.** The manual for version 21 (chapter 63, page 1348) describes it literally: the name of the node's input appears in the status bar at once, and "if you wait a few more moments" — the same text pops up over the node. That is, a vendor who has a status bar still puts a pop-up — not as an alternative but as a deferred duplicate. This is the proof that the delay is a separate axis: the content is the same, there are two channels, and a timer distinguishes them.

2. **A rich tooltip is expensive not because of the delay but because of production.** Someone shot a clip for each tool, and with each change of icons they have to be re-shot. But it localizes better than an ordinary one: the clip has no voice-over and no captions ([NOT VERIFIED live], from descriptions — a screen recording of the gesture), only the name, the key and the phrase are subject to translation, while the mouse movement is the same in all languages. The price of this is that it can be turned off only entirely: there is one checkbox for all tools, and an experienced user turns off together with it those three clips he has not seen.

3. **A tooltip that carries a NUMBER, the vendor does not let you turn off.** In Premiere the "Show Tool Tips" checkbox, by Adobe's direct answer, does not turn off pop-ups with clip data, marker data, and Start/End/Duration above the time bar — "by design". Vegas, with the same floating tooltip, shows the envelope value and the size of the snap-offset during a drag. At that moment the pop-up stops being a reference and becomes an instrument: it is tied to the point being pulled, and it has nowhere to go, because at that moment the eye is not in the corner of the window.

## Pros

- Zero distance from the object to the explanation: the cursor is already there, there is no need to move the gaze. This is the only real advantage of the form — everything else can be reproduced in a permanent place.
- The pop-up takes no space in the layout, so ANY element can be explained without paying for it in pixels of someone else's panel (for a permanent place the payment is the reverse — rule 13 of the checklist).
- The capacity is not limited by the width of someone else's strip: a paragraph, two lines, an image, a video — the form will hold anything, because it is drawn in its own layer.
- The hint is tied to the widget, not to the window: for an unlabeled icon this is the only way to learn its purpose without pressing it.
- An accelerator inside the text ("Selection Tool (V)") teaches the hotkey at the moment the hand is already on the mouse — a learning channel that costs nothing.
- For the programmer it is free: one line next to the widget. In our v1 about forty such lines accumulated (`git show cpp-reference-final:mainwindow.cpp`).

## Cons

- It covers exactly what you are looking at: this is not an implementation defect but a property of the chosen slot.
- It requires waiting, and the delay setting is a compromise with no good value: zero gives flicker when the mouse sweeps across the panel, half a second — "you don't wait", a second — a newcomer does not learn that a hint exists at all.
- On a touch screen there is nothing to hover with: the channel is simply absent, and everything that lives only in it is unreachable there.
- WCAG 2.2, criterion 1.4.13 (level AA) requires three properties of popup content: dismissible, hoverable ("pointer can be moved over the additional content without the additional content disappearing") and persistent. The pop-up stops being text and becomes an object with its own behavior — which is exactly why VS Code keeps `sticky` and `hidingDelay` as separate settings.
- Waiting is a source of repaints. In egui 0.36.1 the waiting branch calls `request_repaint_after_secs(time_til_tooltip)`, that is, a frame is scheduled in order to show the panel half a second later. This is directly against invariant 12.
- The rich form cannot be turned off piece by piece, and it is enabled by default — hence all the negative feedback below: the user pays, with every mouse sweep, for a lesson he no longer needs.
- "Tooltips are off" is almost always untrue: some pop-ups remain, because they carry data. The user looks for a second checkbox and does not find it.

## What users say

> Every time I hover my mouse over a tool that I haven't used yet, a gigantic
> video tutorial popup pops up that shows me how to use the tool.
> — DexFX, Adobe Community, 12.11.2017, https://community.adobe.com/questions-712/how-do-i-disable-the-annoying-but-helpful-video-windows-that-pop-up-when-hovering-over-a-tool-1059128

The thread has been running for nine years, with 35 replies and 77 thousand views; the answer "Preferences > Tools, uncheck Enable Rich Tooltips" is marked as correct.

> This is my job. Has been for 15 years. I know how to use these applications so
> both the blue banners and animated pop-ups are a nuisance.
> — Russell Lord, ibid., 17.10.2018

The reverse side is the same feature through the eyes of a newcomer, and this is not a retelling but a live complaint about its DISAPPEARANCE:

> When i started, the rich tool tips videos have been helpful in explaining what
> certain tools does etc etc but recently I have realised that they no longer pop
> up on my screen.
> — LR_, Adobe Community, 05.08.2025, Photoshop 26.9, https://community.adobe.com/questions-712/turn-on-rich-tool-tips-1181429

About ordinary tooltips in NLEs — the same split. Irritation:

> I know, it's small thing, but they drive me nuts!
> — Steve Martin, Creative COW, thread "Tool Tips – anyone know how to turn off?", 14.04.2013, https://creativecow.net/forums/thread/tool-tips-anyone-know-how-to-turn-offae/

And the dependence on the accelerator in the text of the hint — a person noticed its disappearance and opened a thread:

> So when I open Premiere pro and hover mouse over tools they show the shortcut
> but if I open project file and hover over tool they just show the name and
> shortcut hotkey is not showing.
> — Mxch, Adobe Community, 17.12.2022, https://community.adobe.com/questions-729/premiere-pro-tool-tips-are-not-showing-shortcuts-while-hovering-mouse-1396829

## How it fits Kinotrain

### What already exists

The barrier and its guard:

- `src/apps/kt-app/src/theme.rs:95-96` — `tooltip_delay = 1.0e6`, `tooltip_grace_time = 0.0`, with the comment "item 14 of round 8: there are NO pop-up hints at all". The egui 0.36.1 defaults for comparison: `tooltip_delay = 0.5`, `tooltip_grace_time = 0.2`, `show_tooltips_only_when_still = true` — that is, the library in our hands has exactly the same anatomy of three parameters as VS Code, and we turned the first one up to 1 000 000 seconds.
- `src/apps/kt-app/src/app.rs:1505` `KtApp::hint`, `app.rs:4413` `hint_into`, `app.rs:1274` `draw_status_hint`, pure `status_line` — the model is analyzed in **F-HLP-001**, the strip's budget is measured by **F-HLP-002**; they are not reopened here.
- The accelerator is written into the hint text BY HAND, exactly in the Premiere format: `src/apps/kt-app/i18n/en.toml`, section `[tb]` — `play_pause = "Play/Pause (Space)"`, `undo = "Undo (Ctrl+Z)"`, `search = "Search the project (Ctrl+F)"`.

The payload "a line that is not on screen" already goes to the status line for us — in two places:

- `src/apps/kt-app/src/timeline.rs:2193-2196` — "the full name of the track — in the hint (the label may be truncated)": `tf!(k::tl::hint::LANE_NAMED, name)`, that is, a RUNTIME string is substituted into the catalog key with `{0}`.
- `src/apps/kt-app/src/timeline.rs:3547-3548` — the caption of a morph block that did not fit into the block goes into `hover_hint` instead of being drawn.

The v1 legacy (the UX reference): about forty `setToolTip` calls, of which eleven in `fxchaineditorwidget.cpp` explained the PARAMETERS of effects — 53–111 characters each, for example "Timeline vs source: below 1 slows (longer event), above 1 speeds up (shorter event). 1 = normal." and "When off, changing playback rate resizes the clip. When on, rate still affects playback inside the fixed event." Plus one of 300+ characters (FrontScaler in `mainwindow.cpp`). In v2 nothing from this has moved over: the `[fxp]` catalog contains only field labels ("Rate:", "Mix:", "Damp:"), while descriptions exist only at the level of the EFFECT and lie outside the program, in `site/content/*.toml`, section `[fxdesc]`.

### What is missing

Three live calls to `on_hover_text` that have not a single chance to fire:

- `src/apps/kt-app/src/widgets.rs:415-416` — the head of the chain strip, the condition `h.label.chars().count() > SLOT_NAME_CHARS` (22) and the comment "the full name — in the hint: the truncated one is visible, but there is no other way to learn which file was substituted";
- `src/apps/kt-app/src/widgets.rs:468-469` — the same for the FX slot;
- `src/apps/kt-app/src/morpher_editor.rs:114-116` — the source field of the morpher.

Under the barrier `tooltip_delay = 1.0e6` none of them is ever shown.
The code's promise "there is no other way to learn" has also been verified: the title of the FX window is built from the kind and the name of the CLIP (`src/apps/kt-app/src/fx_editor.rs:128-137`), and the file name is not in it. At the same time `reach_tests::t255` REQUIRES the name to be truncated — that is, we have a recorded rule "truncate" and there is no paired answer "read it in full — here", although for the track this answer exists. This is exactly the class of problem for which `reach_tests.rs` was created (violation of rule 17: paired ways do the same thing differently), and no test catches it.

Second: the program has no explanation of an effect parameter at all. The user sees "Time scale:" and a slider; where the two phrases of v1 went, nobody knows.

### What gets in the way

**t83 (`src/apps/kt-app/src/layout_tests.rs:234`) — analysis.** The test does two different things. The first: it moves the mouse to three points (a toolbar button, a window button, a bin card) and checks the RENDERED `status_line_shown` — that the text begins with the expected hint and that `HintKind::Hover`; this is the guard of item 1 of round 9 ("WHERE DID THE BUTTON HINTS GO?!?!?!"), and it is about delivery, not about pop-ups. The second — the last three lines: after the hover and two frames, `h.ctx.memory(|m| m.layer_ids().all(|l| l.order != egui::Order::Tooltip))`, with the failure message "a pop-up tooltip has come back". This is a class-wide guard: it forbids not a specific pop-up but ANY layer of order `Tooltip` in the application, with one line and forever.

To bring pop-up hints back anywhere at all, one would have to weaken exactly the second part — either remove the check, or narrow it to "except these ids". Then the guard stops being class-wide: it caught ANY future `on_hover_text`, including one accidentally coming from an egui widget, and after narrowing it would catch only the listed ones. This is exactly what has already happened with the prose of the rules, which Round 38 had to move into tests. Moreover, weakening t83 alone is useless: the barrier stands not in the test but in `theme.rs`, and both places would have to be touched at once.

The owner's words, which contradict any revision — verbatim:

> I want it to always be there — so, it is absent when hovering?
> — `docs/user-feedback.md`, R.6 (item 15 and the decisions on the topic)

and the requirement of item 14 of round 8, recorded in the code: "there are NO pop-up hints at all — the only place for hints is the bottom right of the status line". The first is about the delay, the second is about overlap. The real hole, however, lies in the third decision (capacity and direction), which nobody made, and it is fixed without a single pop-up.

The third obstacle is technical: the status bar line is laid out by `layout_no_wrap` and is not truncated (`app.rs:1313-1322`), so a long file name will run off the right edge. This is also what **F-HLP-002** measures — it answers the question "does it fit", this note answers "does it reach at all"; the order of work is: first the budget, then the route.

### Which invariants it touches

- Invariant **12** (repaint only on movement, no allocations or clones per frame on the draw paths): both the hover timer for the pop-up and any animation in it are sources of frames; a clip of the Photoshop class is forbidden entirely.
- Rule **10** (exactly one hint, in one place) — the subject of the dispute; we do not touch it.
- Rule **7** (the text fits) — truncation is already mandatory (t255), and it is exactly this that created the debt.
- Rule **17** (paired ways are identical) — violated today: the track name is read in full, the source name in the chain is not, although the problem is one.
- The cost of any NEW catalog line — F-HLP-033; here it almost does not arise: the runtime file name is not translated, one key with `{0}` of the same kind as the track already has is needed.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: (1) the route of truncated names into the line + a test; (2) parameter explanations as a separate payload |
| Where the main work is | kt-app: `widgets.rs`, `morpher_editor.rs`, `reach_tests.rs`, the `i18n/` catalogs |
| Project format | does not touch it |
| Tests that will have to be written | `reach_tests::elided_label_has_a_reachable_full_name` — for each place where a label is truncated there must be a path to read it in full without a pop-up; t83 remains green without a single edit |
| Depends on | F-HLP-001 (the model of the line), F-HLP-002 (the budget of the strip), F-UI-002 (the registry of commands), F-HLP-033 (the cost of a line) |
| Breaks compatibility | no |

About the accelerator: **beyond the command registry (F-UI-002)** showing the key in the hint adds exactly one thing — the accelerator stops being HAND-WRITTEN into five catalogs and starts matching the real binding. Today "Play/Pause (Space)" is not lying only because we have no key reassignment; on the day it appears, all seventy lines will lie at once. The registry is not designed here — it owns the source, this note is only about display.

## Verdict

We do not take the form: a pop-up pays with overlap and delay for a single advantage — zero distance to the object — and both of these points are closed by direct decisions of the owner, while the rich variety additionally runs into invariant 12 and into the production of clips that we will not manage. But "we are all right" is also untrue: three places in the code directly count on a pop-up that has not existed in the program since round 8, and the v1 parameter prose is lost entirely. The payload that does not fit into a line — movement (teaching a gesture by showing it) — goes to the site. The payload that fits but does not reach — the truncated name — goes into the same line.

TEST: reach_tests::elided_label_has_a_reachable_full_name — today it would fail: label truncation is mandatory (t255, `SLOT_NAME_CHARS = 22`), and the label can be read in full only for the track (`timeline.rs:2196`); three calls of `on_hover_text` in `widgets.rs:416`, `widgets.rs:469` and `morpher_editor.rs:116` are silenced by the barrier `tooltip_delay = 1.0e6` from `theme.rs`, and the source name in the FX chain is unreachable by any means.

ONE LINE: overlap — does not touch | delay — does not touch | capacity — plus one runtime argument (the full name of the truncated label) into THE SAME status bar line via `hint()`/`hint_into()`, as is already done for the track name; the clip and the paragraph are not taken for this surface at all and stay on the site.

## Sources

- VEGAS Pro 2026, official help (Boris FX CDN, accessed 2026-09-07):
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/9-audio/spconsoleopt_audio_track_channel_strips.htm
  (ToolTip with the full name of the plugin and preset),
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/11-autoscript/adjusting_envelopes.htm
  (floating ToolTip with the envelope value),
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/enablesnapping.htm
  (ToolTip with the size of the snap-offset). `prefgeneraltab.htm` and
  `prefdisplay.htm` were checked — the checkbox for turning tooltips off is not described there.
- DaVinci Resolve 21 Reference Manual (PDF, accessed 2026-09-07): chapter 63
  "Exploring the Fusion Interface", page 1348 — the status bar at once, the tooltip "after a few moments"; chapter 45 — the tooltip with the offset during a roll trim; the section of Fusion settings — "Show Tips: Toggles whether tooltips are displayed".
- Final Cut Pro User Guide for Mac (PDF, 2026-09-07): the help tag with the original name of a renamed effect; the Skimmer Info window (Control-Y).
- Adobe After Effects, manual of the CS5 line (PDF, 2026-09-07): "To show tool tips, select the Show Tool Tips preference (Edit > Preferences > General)" and the advice to keep the Info panel in view. Not re-checked for 26.x: `helpx.adobe.com` does not open from this environment (see `_editors.md`).
- Adobe Community, thread on rich tooltips (2026-09-07):
  https://community.adobe.com/questions-712/how-do-i-disable-the-annoying-but-helpful-video-windows-that-pop-up-when-hovering-over-a-tool-1059128
- Adobe Community, "Turn on Rich Tool Tips" (2026-09-07):
  https://community.adobe.com/questions-712/turn-on-rich-tool-tips-1181429
- Adobe Community, "Premiere pro tool tips are not showing shortcuts…"
  (2026-09-07):
  https://community.adobe.com/questions-729/premiere-pro-tool-tips-are-not-showing-shortcuts-while-hovering-mouse-1396829
- Creative COW, "Tool Tips – anyone know how to turn off?" (2026-09-07), the answer of
  Peter Garaway (Adobe) about the pop-ups that remain "by design":
  https://creativecow.net/forums/thread/tool-tips-anyone-know-how-to-turn-offae/
- Ask Tim Grey, "Rich Tooltips", 03.05.2018 — both checkboxes of Preferences > Tools:
  https://asktimgrey.com/2018/05/03/rich-tooltips/
- Photoshop Essentials, "Rich Tool Tips in Photoshop CC 2018" — the composition of the pop-up and the duration of the animation: https://www.photoshopessentials.com/basics/rich-tool-tips-photoshop-cc-2018/
- W3C, Understanding SC 1.4.13 Content on Hover or Focus (WCAG 2.2, AA):
  https://www.w3.org/WAI/WCAG22/Understanding/content-on-hover-or-focus.html
- Visual Studio Code, source code (`src/vs/editor/common/config/editorOptions.ts`,
  2026-09-07): defaults `delay: 300`, `hidingDelay: 300`, `sticky: true`,
  mode `onKeyboardModifier`.
- GTK 3, `gtk-tooltip-timeout` — default 500 ms (the property is declared obsolete since 3.10): https://docs.gtk.org/gtk3/property.Settings.gtk-tooltip-timeout.html
- egui 0.36.1, local sources: `style.rs` (defaults 0.5 / 0.2 / `true`),
  `containers/tooltip.rs` (the waiting branch with `request_repaint_after_secs`).
- Our v1: `git show cpp-reference-final:mainwindow.cpp`,
  `git show cpp-reference-final:fxchaineditorwidget.cpp`.

Live checks for programs outside the core (Photoshop, VS Code) are not assumed —
their lines explain the mechanism, not carry a verdict. The lines about Premiere and After
Effects rely on secondary sources and manuals of past lines, which is marked in the table.
