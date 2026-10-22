---
id: F-HLP-001
title: Our model — one hint line at the bottom right of the status bar
status: already-have
priority: B
cost: S
category: help/hints
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, ableton-live-12]
we_have: yes
crates: [kt-app]
invariants: [12]
ui_rules: [10, 13, 16]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-HLP-001 — Our model — one hint line at the bottom right of the status bar

## Essence

The editor moves the mouse over the program, and in the bottom right corner of the window — in the strip that is there anyway — one line silently changes: what this object under the cursor is and what can be done with it. Nothing pops up, nothing covers the frame, there is nothing to wait for: the text changes in the same frame in which the cursor has moved. The same line is the only place where the program shouts about a gesture error (in red), reports a completed action and, when there is nothing to do, shows a standby phrase.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Status Bar | a strip at the bottom of the window: help text when hovering over a MENU item, the available recording time, progress meters of long operations. Buttons have ordinary popup tooltips over the interface. The mechanism is described in the manuals of the 14–17 line; for 2026 [NOT CHECKED] |
| Adobe Premiere | 26.x | Tool Tips + clip data popup | there is no fixed hint line. Tooltips are switched off in the settings, while the popup with clip data on hovering over an event of the timeline — no, "by design" (the answer of an Adobe Community Expert, 02.10.2025) |
| DaVinci Resolve | 21 | Tool tips | there is no fixed line, the hint pops up at the element; control of the delay/switching off is the subject of requests on the BMD forum, not a setting [NOT CHECKED live] |
| Final Cut Pro | 12.3 | Tool tips | there is no line: the hint pops up at the button, the hotkey is written into its text [NOT CHECKED live] |
| After Effects | 26.x | Tool Tips / Info panel | there is no line. The Info panel shows pixels, coordinates and time — the state of DATA, not the purpose of an interface element [NOT CHECKED live] |

Five out of five: a fixed description area under the cursor belongs to no one except VEGAS, and there it serves only the MENU. Therefore the second table is mandatory — the mechanism does not come from video editing, and without it our line has nothing to be compared with.

**Outside the core.** Without Ableton Live this feature cannot be described: our line is a direct descendant of its Info View, not a truncated tooltip, and to argue about it without naming the ancestor means arguing about taste.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Ableton Live | 12.4.5 | Info View | a fixed area in the corner of the window: writes what the thing under the cursor is — without covering and without delaying. The detailed analysis — **F-HLP-002**, here only the origin |

## The trick behind it

**What the mechanism carries that one line cannot hold.** Our line carries exactly what fits in 160 characters about the object UNDER THE CURSOR. It cannot hold an ADDRESS — where to look for an object that nobody has pointed at. And this load does not disappear for us anywhere: it lies on seven of the nineteen complaints of R.38.

The line is an ideal **pull** channel: you ask (point) — you get an answer. And it is not a **push** channel at all: you cannot point the mouse at what you have not found. Below is the proof over all seven R.38 complaints of the form "I don't understand how" (`docs/user-feedback.md`, section "Round R.38"). None of them is cured either by the capacity of the line or by its wording.

| R.38 | Complaint (verbatim) | What had to be pointed at | Why the line does not cure it |
|---|---|---|---|
| item 1 | "I don't understand how to delete a saved area region, I saved it and it now hangs there, I don't understand how to rename it — RMB gives nothing" | the area on the timeline — he pointed and pressed RMB | `RemoveArea`/`RenameArea`/`SetAreaRange`/`SetAreaLanes` were orphaned commands: covered by unit tests, not a single call in the whole `apps/`. The line describes what is reachable; it does not create a verb |
| item 2 | "I cannot add a morpher event to a morpher track in any way" | the `tb.add_morpher` button in the toolbar — and he was looking at the empty morph track | above the track the line answered `tl.hint.empty` ("LMB — cursor, drag — region…") — honest about the place, but not about WHERE the command that creates the object lies |
| item 3 | "add morpher layer also works strangely" | nothing to point at: the rule "there is a region — by region, there is none — from the cursor" belongs to the command | while the hand is on the button, there is no object yet; when the object has appeared — the cursor is above the result. There is nothing to point at either before or after |
| item 5 | "I don't understand how the camera track works" | the camera block | `tl.hint.camera` lists the gestures ("drag to move, edges resize, RMB — menu, double click — properties") and is silent about WHY the camera exists and what it acts on. "Why" is a paragraph, and the value limit is 160 characters, and it has to fit five languages |
| item 9 | "I don't understand how to change the length of a morpher event" | the NARROW edge of the block | the answer exists and it is in the line (`tl.hint.morph_edge`), but it is shown only when the cursor is already on the edge. Knowledge of the gesture is hidden behind the gesture itself — a pure pull dead end |
| item 14 | "in the automation timeline I don't understand how to zoom" | nothing to point at: zoom was a gesture without an object | the BUTTON cured it (the link lock with the timeline, the decision of item 4 of R.38), and the hint `fx.axis_link_hint` appeared only because a visible control appeared. The text came second, not first |
| item 16 | "the feeling that the effects have some separate 'default' state of values" | nothing: this is a wrong mental model | the field edits the BASE, and shows the value of the curve. Two sentences are needed AND at the moment when the person sees the discrepancy, not when he hovers the mouse over the field |

From this follows the conclusion that reverses the statement: the cry "**WHERE DID THE BUTTON HINTS GO?!?!?!**" (R.9 item 1) and "I don't understand how to delete a saved area region" (R.38 item 1) are not one task solved halfway. These are two different tasks. The first (pull) is solved and closed by the regression guard t83. The second (push) is not solved and in principle is not solved by this surface.

## Pros

- **The place is single and known in advance** (rule 10 of the checklist): the hint does not have to be searched with the eyes — it is not "somewhere at the cursor", but always in one corner. The user looks there out of habit, not by searching.
- **Zero delay.** `tooltip_grace_time = 0.0`, and the hint itself is assembled within the frame in `hover_hint` and is drawn as the LAST operation of the frame (`self.draw_status_hint(&ctx)`, `src/apps/kt-app/src/app.rs:1162`). t83 checks what is DRAWN (`status_line_shown`), not an internal field — earlier tests read the field after the frame and were falsely green.
- **Nothing is covered.** Layer `Order::Foreground`, `interactable(false)`, a rectangle — the already existing status-bar strip `exact_size(20.0)`. Neither the preview nor the timeline is ever covered: the complaint "description pops up in the way" is inexpressible with us as a class.
- **An error overrides hovering.** The order in the pure `status_line`: error → hover → mode → message → standby line; t172 requires `HintKind::Error` exactly at the moment of a gesture's stop and of the obstacle highlight (rule 16: an impossible action is an error, not a faded hint).
- **Zero use of space** (rule 13): the line lives on the strip set up for the bell and the counters, and the long hint deliberately ERASES the metrics on the left — a comment in the code: "it is more important". Not a single new strip has been set up for hints.
- **The hint is machine-checkable as the truth about reachability.** t253 breaks the build if the pointer stands ON an entity while the line says "empty place". A popup tooltip cannot have such a property: a popup has no single point from which a test reads "what the program said to the user", while we have one — `status_line_shown`.

## Cons

- **One physical line, one `layout_no_wrap`.** No wrapping, no second line. The ceiling of a catalog value is 160 characters (`src/apps/kt-app/build.rs:42`, `MAX_VALUE_CHARS`; a longer one BREAKS the build with "split it into two keys"). The longest live hint — `tl.hint.clip`: 107 characters (en) / 109 (ru), that is, the margin is already less than a third.
- **One at a time, and the mode loses to hovering.** `HintKind::Mode` stands below `Hover`, and the timeline sets `hover_hint` at EACH of its points (the last branch of the zone analysis — `tl.hint.empty`). So the state "you are now in mode X, exit — Esc" is inexpressible on this surface: over the largest area of the window the mode line is never visible [a conclusion from the code; the order is closed by the unit test of `status_line`, there is no live scene].
- **Transience.** Move the mouse away — there is no text. Nothing to re-read, nothing to copy, nothing to show to another person.
- **It requires hovering** — exactly what "the trick behind it" is written for.
- **In a narrow window the text is not truncated.** The plate is pinned to the left edge of the strip (`.max(strip.left())`), and the galley is drawn from it across its full natural width: at the minimum window of 640 px (t86) a hint of 107 characters (~600 px at a 12 px font) takes up the whole strip and runs past the right edge. Rule 7 is closed by a test for NAMES (t255), but not for the hint itself [an estimate from the code].
- **Two short allocations per frame** (`text.clone()` into the galley + a copy into `status_line_shown`) — against the letter of invariant 12. Tolerable exactly because the hint is one and short; a rich hint would make this item noticeable.
- **What physically does not fit:** a chord of several keys (the accelerator is written into the text by hand — "Play/Pause (Space)" in `[tb]`, there is no command registry, see F-UI-002); the explanation "why"; a picture; a list of options; the state of a mode.

## What users say

The owner — once for each of the two accepted decisions; both are reproduced verbatim from `docs/user-feedback.md`.

> "I want it to be there always, is that it, that it's gone when hovering, is that what happens?"
> — the owner, R.6 (delay), `docs/user-feedback.md`, 2026

> "**WHERE DID THE BUTTON HINTS GO?!?!?!** … you did not finish this item! Recheck all the hints that you edited"
> — the owner, R.9 item 1 (the pull channel broke — and this was noticed immediately), `docs/user-feedback.md`, 2026

Other users — about the price of the opposite choice (a popup):

> "the control descriptions hover there while I'm attempting to adjust a
> parameter so that I can not see what I am adjusting"
> — Brian Cooney, Creative COW, 21.10.2013,
> https://creativecow.net/forums/thread/tool-tips-anyone-know-how-to-turn-offae/

> "All Effects boxes show useless info popups when I'm trying to adjust a
> parameter"
> — Ken Bronowski, same thread, 23.07.2014,
> https://creativecow.net/forums/thread/tool-tips-anyone-know-how-to-turn-offae/

> "Its not a bug but by design. Alas there is no way to disable this feature."
> — Ann Bens (Community Expert) about the clip data popup in Premiere, Adobe
> Community, 02.10.2025,
> https://community.adobe.com/t5/premiere-pro-discussions/how-do-i-stop-premiere-pro-from-showing-a-clip-data-window-when-the-cursor-is-hovering-over-a-clip-i/td-p/15530239

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-reach/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Already there: one hint line at the bottom instead of tooltips — t83 holds the ban on popup windows (rule 10 of kt-app).

## Estimate

| What | Answer |
|---|---|
| Size | S — there is no work under this note: the mechanism has been in production since R.8 and is covered by t83/t172/t253 |
| Where the main work is | kt-app: `app.rs` (`hint`, `hint_into`, `status_line`, `draw_status_hint`), `theme.rs`, the catalogs `i18n/` |
| Project format | not touched |
| Tests to be written | under this note — none; if one wishes to close the "narrow window" — a harness test of hint truncation at 640 px |
| Depends on | nothing. The push channel is the subject of the other notes of the wave; the command registry and accelerators — F-UI-002 |
| Breaks compatibility | no |

## Verdict

The model remains exactly as it is: one line, zero popups, zero delay. All three disputed decisions of R.40 are closed in this note as follows — overlap and delay are not touched according to the owner's direct words, while the capacity of the line does not grow, because growing it cures nothing of the seven complaints.

ALREADY-HAVE: app::status_line in src/apps/kt-app/src/app.rs — the reconnaissance changed the wording of the decision: not the inaccurate "we have no hints, we have one line", but the accurate "we have solved the pull channel and not solved the push, and these are two different tasks, of which the second is not cured either by the capacity of the line or by its text".

ONE LINE: overlap — not touched | delay — not touched | capacity — does not grow by a single character (160 characters, one `layout_no_wrap`, one hint at a time); the whole push moves to the SECOND surfaces — the context menu, the label itself, the window where things come on purpose — and they are owned by the other notes of the wave.

## Sources

- `docs/user-feedback.md`, sections R.6, R.9 item 1, R.38, R.40 (accessed 2026-09-07) — both remarks of the owner and all nineteen complaints of R.38.
- Code: `src/apps/kt-app/src/app.rs`, `theme.rs`, `timeline.rs`, `layout_tests.rs` (t83, t86, t172), `reach_tests.rs` (t253), `build.rs`, `src/apps/kt-app/i18n/*.toml` (2026-09-07).
- https://creativecow.net/forums/thread/tool-tips-anyone-know-how-to-turn-offae/ (accessed 2026-09-07) — quotes about a popup that covers an adjusted parameter.
- https://community.adobe.com/t5/premiere-pro-discussions/how-do-i-stop-premiere-pro-from-showing-a-clip-data-window-when-the-cursor-is-hovering-over-a-clip-i/td-p/15530239 (accessed 2026-09-07) — the clip data popup cannot be switched off "by design".
- VEGAS Pro manuals of the 14–17 line (status bar: help text for menu items, recording time, progress meters), found by search 2026-09-07; for version 2026 not checked live.
- The Ableton Live line — from the analysis in F-HLP-002 and `_editors.md`; a live check for software outside the core is not assumed (the rule of the section "Software outside the core"). The Resolve, FCP and After Effects lines rely on documentation and forums, not checked live — marked in the table.
