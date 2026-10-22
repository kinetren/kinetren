---
id: F-HLP-010
title: Impossible action: how the editor explains why a gesture did not work
status: already-have
priority: B
cost: S
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app, kt-model]
invariants: []
ui_rules: [9, 16, 18, 19]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-HLP-010 — Impossible action: how the editor explains why a gesture did not work

## Essence

The editor drags an event — and it stops moving. The question he has is always exactly one
and the same: "why?". A good answer has three parts: WHAT is in the way (this object), WHY it
cannot move (in words), and WHERE TO LOOK if the obstacle lies outside the screen. The third
part is the one the user actually wrote the requirement for: the obstacle may be two minutes
to the right, and without a pointer one has to search for it by hand.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none: there is no stop at all | Butting into an event is LEGAL and turns into a crossfade ("a crossfade is inserted"); trimming past the end of the material gives a repeat or silence, depending on the Loop switch. There is nothing to explain, because there is no refusal. VEGAS can show a red marker on the guilty object — but for something else: `Show Unquantized Event Edges` draws a red line on an unquantized edge |
| Adobe Premiere | 26.x | Events panel (a log, not about the gesture) | Overlap is allowed, "the clip on the higher track will be visible"; a conflict = overwrite. The reason for the refused gesture is not reported; a locked track is marked IN ADVANCE with hatching, and a locked track simply stops being the target. The Events panel is a color-coded log (Errors / Alerts / Info) with a Details button, but it is about the format of the material and the system, not about the geometry of the edit |
| DaVinci Resolve | 21 | "Not Enough Handles" (a dialog, only for transitions) | Dragging over a neighbour SILENTLY OVERWRITES it. During the gesture a tooltip at the cursor shows the number of shifted frames, and when trimming an **outline of the permissible area** is drawn — "outlines show you how much media is available". The only real refusal with an explanation: there are not enough handles for a transition — the dialog names the reason and gives three outcomes (Trim Clips / Skip Clips / Cancel) |
| Final Cut Pro | 12.3 | a red clip edge; Alert icons | The magnetic timeline pushes the neighbours aside, so there is no stop at a neighbour at all. The limit of the material is shown by a MARKER WITHOUT TEXT: "the clip edge turns red"; in the precision editor the available reserve is drawn permanently as muted areas. The catalog of badges with reasons in words (Missing Clip / Missing File / Missing Plug-in…) concerns missing material, not a refused gesture |
| After Effects | 26.x | the layer blinks | There are no collisions by construction: layers are rows, and overlap in time is legal. The only refusal in the whole timeline is delivered by **the object itself**: "If you try to select or modify a locked layer, the layer flashes in the Timeline panel". The numbers of the gesture (name, duration, shift) go to a fixed Info panel |

From these five rows a single explanation emerges for why we are alone here. **A refusal is
needed only by someone who does not silently destroy someone else's edit.** VEGAS and AE make
the conflict legal, Resolve and Premiere let it through by overwriting, FCP pushes the
neighbours away; none of the five has to say "you can't", because for them everything is
possible. For us the stop appeared as a direct decision of R.24 item 10 and R.28 item 6
("NOTHING ANYWHERE MUST GROW BACK OR BE CUT OFF"), and together with it came the duty to explain.

Separately, a third question was checked — whether the editor reports WHERE the obstacle is
if it is outside the view. **None of the five: neither arrows at the edge, nor distances, nor
sides, nor auto-scrolling toward the obstacle.** This is a confirmed gap of the industry, not a
gap of the search (the list of pages reviewed is in "Sources"). Moreover, Blackmagic has solved
exactly this task — but in another window: the Fusion node editor has an Auto Navigator, which
"only appears when one or more nodes is outside the visible area of the Node Editor". On the
same product's editing timeline — nothing.

**Outside the core.** Without Blender the second half of the topic — "why is the widget grey" —
cannot be described: it is the only program found where the interface engine OBLIGES the author
of an action to put the reason for the refusal into the widget itself, not into the manual.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | "Disabled (red)" in the tooltip; API — `Operator.poll_message_set()` | The manual lists this as a standard element of the hint: "Disabled (red) — The reason why the value is not editable". The reason comes from two sources: the message that the operator sets next to the refusal of its `poll()`, and the `disabled_info` field of the button; it is drawn in the `TIP_LC_ALERT` style and passes through `TIP_()`, that is, it IS TRANSLATED. It appeared in 3.0. There is no fixed prefix "Disabled: " in the code — the red colour is the marker |
| VS Code | 1.136 | `enablement` versus `when` | Two different levers: `when` removes the item, `enablement` greys it out. The command palette "doesn't show icons nor disabled commands", the editor context menu shows them grey. A channel for "why" is not provided in the extension contract at all |
| IntelliJ IDEA | 2026.2 | — | The guidelines say WHEN to disable a button (its control is off; required fields are not filled in), and they prescribe that a tooltip carries the action's label and hotkey. The page about tooltips does not contain the word "disabled" even once |
| Ableton Live | 12.4.5 | Info View + Status Bar | The Info View "displays the name and function of whatever element of the UI that you hover over": a direct ancestor of our line in form. The status bar separately "displays useful information like error messages". Whether the Info View describes UNAVAILABLE elements and names the reason — not found |
| Photoshop | 27.x | Rich Tool Tips | The most expensive end of the scale: a description plus a short video of the action, switched off in the settings. The behaviour on inactive elements is not documented — not found |

## The trick behind it

**The payload that one status-bar line cannot hold is the COORDINATE: a pointer to a specific
object which, at the moment of the refusal, may lie outside the screen.** Text can name it but
cannot show it. We did not fit this payload into the line, and did not hand it to a popup
either — we split it into three surfaces, each carrying its own third: the object takes the
colour, the line takes the side and the number, the edge of the track's lane takes the arrow.
The note has the status `already-have` precisely for this reason: the split is already done, and
three of its details are non-obvious.

1. **The obstacle is chosen by the DIRECTION of the gesture, not by proximity to the cursor.**
   `plan_move()` takes `min_by_key` on the start when moving right and `max_by_key` on the end
   when moving left (`timeline.rs:267-272`). The difference shows when you drag a clip over three
   neighbours: the highlighted one is not the one nearest to the mouse, but the one you STOPPED
   AT — and the highlight jumps to another neighbour if you move the mouse back. This is the only
   choice under which the highlighted object is itself the explanation of the stop.
2. **The number is "how far to scroll", not "where it starts".** `block_view` measures to the
   NEAREST edge of the obstacle: `view.0 - span.1` to the left, `span.0 - view.1` to the right
   (`timeline.rs:166-174`). The user gets not a timecode to subtract from the current one, but a
   ready scroll distance.
3. **The arrow carries a second coordinate which the text does not have at all.** It is drawn not
   in a corner of the screen, but at the edge of THAT lane of the track where the obstacle lies
   (`lane_top(b.kind, b.lane)`, `timeline.rs:4230`). The text says "12.4 s to the right", while the
   arrow silently adds "on the second video track". Both answers are needed, and neither takes
   space from the other.

The reverse side of the split, also non-obvious: the highlight is clipped by the edges of the view
(`.max(rect.left())` / `.min(rect.right())`), so a partly visible obstacle shows a red rectangle
running off the edge — and this by itself reads as "it continues there".

The third part — "where to look" — is carried by **none** of the five core editors (checked, see
"Who has it and how"). So in this note the "trick behind the trick" works the other way round: the
finding is not in someone else's product, but in the fact that there is no other product with such
a payload.

## Pros

- **The decision "visible or not" is a pure function of `(span, view)`**, knowing nothing about
  pixels. Therefore "where to look" is checked by a table of values without a window (part 2 of
  t172) and cannot fall behind a change of the layout: it has no geometry and no state to fall
  behind with.
- **The obstacle is described in TICKS, not by `ClipId`.** `Blocked.span: (i64, i64)` — so the
  highlight works equally for the foreign pieces of `Project::foreign` (placeholders of imported
  OTIO, which have no `ClipId` at all) and does not break if the model has been rebuilt during the
  gesture.
- **A one-frame lifetime rules out a whole class of defects.** The clearing happens at the start
  of the pass, before all the hit tests; "the red highlight got stuck" is impossible not because
  someone remembers to reset it, but because the reset is unconditional.
- **The error is first in `status_line` — hover physically cannot win.** While the gesture is
  stopped, mouse movement over buttons is not in a position to overwrite the explanation, and
  neither a timer nor a priority rule "who wrote last" is needed.
- **The clamp happens in the PREVIEW of the gesture, not on release** (`d.cur_start =
  start` inside the drag handler). Releasing makes no surprise: what is visible is what will be —
  this is the requirement of R.28 item 6, "NOTHING ANYWHERE MUST GROW BACK OR BE CUT OFF".

## Cons

- **In the project's normal mode the mechanism is barely shown.** Overlap layering
  (`Project.overlap_layers`) is enabled BY DEFAULT under the R.28 decision
  (`src/libs/kt-model/src/project.rs:3020`), and `plan_move()` with overlap layering on and no
  foreign pieces returns `Free`: putting clips on top of each other is legal, there is nothing to
  butt into. The most refined feedback mechanism in the program lives on two scenarios — foreign
  placeholders from an import, and projects where the user has switched overlap layering off in
  the context menu. t172 itself has to start with `p.overlap_layers = false` and the comment
  "blocking is possible only in this mode".
- **Zero trace.** `blocked` does not get into the `notices` ring (200 entries) — the only
  correctly made error in the program is, by the same token, the one that is not saved. Move the
  mouse away — and there is nowhere to re-read it.
- **The line names the kind, not the object.** "An event on a track is in the way" — but which one:
  neither the media name nor the timecode. If there are several obstacles, you learn about the first,
  scroll past it — you run into the next, and so on one by one. The ability to name an object
  individually exists in the program — the neighbouring messages about a missing source are written
  exactly so (`media_missing`: "{0}: source unavailable, placeholder shown") — it is simply not applied here.
- **The unit of measure in this line is unique in the whole program.** The distance is given in seconds
  with one decimal place, whereas the toolbar, the input mask (t98) and the markers live in timecode.
  The number cannot be matched against anything by eye.

## What users say

A caveat about the source of the quotes: the forums of the four core editors did not open in this
pass — `forum.blackmagicdesign.com` returns an AWS WAF bot challenge, `community.adobe.com` returns
403 from CloudFront, `reddit.com` returns 403, and the VEGAS forum has moved from
`vegascreativesoftware.com` to the Boris FX Discourse (`forum.borisfx.com/c/vegas-pro/55`), which
there was no time to reach. The quotes below come from the platforms that did open:
`video.stackexchange.com`, `blender.stackexchange.com`, `blenderartists.org` and the Shotcut forum.

A marker without words does not work — here is that very FCP mechanism "the edge turns red" through
the eyes of a person who saw it and did not understand it:

> "Now I wanted to add some more images to my last segment but then I can't
> drag the clip longer. It will show a red border at the end and wont make it
> longer."
> — Georg, video.stackexchange.com, 22.12.2017,
> https://video.stackexchange.com/questions/23095/cant-make-a-clip-in-finalcutpro-x-longer

The condition under which the item becomes available is known only to the program, and the forum
explains it:

> "I'm trying to merge these two cut parts. The "merge" option is still greyed
> out even though those cut parts of the video."
> — Danielle, Shotcut Forum, 16.10.2023,
> https://forum.shotcut.org/t/the-merge-option-is-greyed-out/41150

And the reverse case — an author who deliberately put the unmet condition into the hint of a disabled button:

> "Sorry, I made the bake button appear disabled while certain conditions
> required for it to work were not met. Hovering the mouse over the button
> tells you what error has to be dealt with."
> — AquaticNightmare, blenderartists.org, 19.09.2020,
> https://blenderartists.org/t/ez-baker/1245869/76

In the same thread "hover and read the reason" has become part of the support protocol, and another
participant writes that he prefers exactly the grey button with a reason, because it is faster than
pressing it and seeing what broke. This is exactly the behaviour we do not have today — not because
of a dispute about design, but because of `hovered()`.

## How it fits Kinotrain

CAPABILITY: openspec/specs/ui-reach/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Already there: an impossible action is an error in the status line, not silence (t172); "there are no silent objects" (regression guard t83).

## Estimate

What is estimated is not "make the feature" — it is done — but **extending rule 16 to the other five
places** and locking it with a machine, as R.38 did for rules 1, 3, 7 and 17.

| What | Answer |
|---|---|
| Size | S — one stage of the round; the reason lines cost more than the code |
| Where the main work is | kt-app (`app.rs` — `hovered()` → `contains_pointer()` in two helpers; `timeline.rs` — `blocked` on trims; `app.rs::apply` — the model's refusal as `Error`), kt-model (open `check_on` outward, so that the reason can be asked about BEFORE the gesture), plus the i18n catalogs |
| Project format | does not touch it |
| Tests that will have to be written | `reach_tests::refusal_is_an_error_everywhere` — a scanner: every `add_enabled(cond, …)` has a hint whose text DEPENDS on `cond` (today one entry answers for all six places); a harness check that the hint of a disabled widget reaches `status_line_shown`; an extension of t172 to `DragMode::Trim*`; a check that a refused `Command` gives `HintKind::Error` and a localized text |
| Depends on | — (joins F-HLP-020 on "the trace that remains"; the price of the lines — F-HLP-033) |
| Breaks compatibility | no |

## Verdict

Our Move gesture is the reference, which this note has nothing to improve on: the three parts of the
answer are in place, and the third (side + distance + arrow) is missing in ALL five core editors —
that is why the note was opened with the status `already-have`. There is nothing to change there.

The reconnaissance changed not the decision but its BOUNDARY. Rule 16 is written in words about the
gesture on the timeline — and it works exactly that far: the trim is silent, the model's refusal is
yellow and English, a grey button is not explained, and the only correct attempt in the project to
explain GREYNESS (`mrph.timed_by_ref`) physically cannot be shown — although the same device on a
neighbouring, not disabled widget (`mrph.ref_dead`) works. This is not "a bit more to finish here":
it is proof that prose has once again dropped nothing — the same conclusion with which R.38 closed.

ALREADY-HAVE: reach_tests::refusal_is_an_error_everywhere — rule 16 covers not the class "impossibility of an action" but a single Move gesture: the reconnaissance narrowed the wording to it and named five addresses to which it has to be extended — the trim (`src/apps/kt-app/src/timeline.rs:2665`), the model's command refusal (`Severity` without a variant `Error`, `src/apps/kt-app/src/app.rs:17`), all six `add_enabled`, the privacy of `TimelineModel::check_on`, and the input mask. The hint of a disabled widget is moreover dead by the design of egui (`hovered()` is always `false`), so the rule has to be extended by a scanner, not by prose: R.38 has already proved that prose drops nothing.

ONE LINE: overlap — untouched | delay — untouched | capacity — the reason for a refusal (a trim, a grey
widget, a rejected command) travels to the EXISTING surface of the red status line, `HintKind::Error`,
through `contains_pointer()` instead of `hovered()`; not a single new layer and not a single popup.

## Sources

None of the five core editors was run live in this pass: everything is from official documentation and vendor
manuals; for software outside the core, from documentation and sources. The access date for all is 2026-09-07.

**Core.**

- DaVinci Resolve 21 Reference Manual (PDF, downloaded and read locally; references in the text are by chapter
  and manual page: Ch. 45 pp. 964-965 the move and the outline of the permissible area, Ch. 36 pp. 737/741-742/747
  overwrite and hatching, Ch. 55 p. 1197 "Adding Transitions When There's Not Enough Handles", Ch. 66 p. 1472
  Auto Navigator in the node editor) —
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
- VEGAS Pro 2026 Online Help (last modified 15.04.2026),
  `cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/`:
  `moving_events.htm`, `autocrossfades.htm`, `adjusting_eventslength.htm`,
  `quantize_frames.htm`, `post_edit_ripple.htm`, `shuffling_events.htm`
- Final Cut Pro User Guide for Mac 12.3, `support.apple.com/guide/final-cut-pro`:
  `arrange-clips-in-the-timeline`, `drag-clips-to-the-timeline`,
  `extend-or-shorten-clips`, `use-the-precision-editor`,
  `snap-to-items-in-the-timeline`, `alert-icons`,
  `final-cut-pro-interface`
- Adobe Premiere 26, `helpx.adobe.com/premiere/desktop/`:
  `edit-projects/change-clip-sequence/rearrange-clips-on-the-timeline.html`
  (07.01.2026), `.../different-ways-to-move-clips.html` (01.04.2026),
  `.../track-lock-to-prevent-changes.html`,
  `.../navigation-controls-in-the-timeline.html`,
  `edit-projects/trim-clips/about-trim-mode.html` (18.08.2026),
  `edit-projects/correct-mistakes/view-warnings-errors-and-information-events.html`,
  `get-started/preferences-and-settings/timeline-preferences.html` (17.06.2026)
- After Effects 26, `helpx.adobe.com/after-effects/desktop/`:
  `work-with-layers/manage-layers/layers.html` (the blinking of a locked layer),
  `work-with-layers/select-and-arrange-layers/selecting-arranging-layers.html`,
  `get-started/get-familiar-with-the-interface/general-user-interface-items.html`

**Outside the core.**

- Blender 5.2 Manual, Help System → Tooltips → Elements —
  https://docs.blender.org/manual/en/5.2/getting_started/help.html ;
  API `Operator.poll_message_set()` —
  https://docs.blender.org/api/current/bpy.types.Operator.html ;
  release notes 3.0 (the appearance of the mechanism) —
  https://developer.blender.org/docs/release_notes/3.0/python_api/ ;
  implementation — `source/blender/editors/interface/regions/interface_region_tooltip.cc`
  in https://github.com/blender/blender
- VS Code Extension API, Contribution Points (`when` versus `enablement`) —
  https://code.visualstudio.com/api/references/contribution-points
- IntelliJ Platform UI Guidelines: Button (when to disable) —
  https://plugins.jetbrains.com/docs/intellij/button.html ; Tooltip (three kinds,
  "disabled" not mentioned) — https://plugins.jetbrains.com/docs/intellij/tooltip.html
- Ableton Live 12 Manual, "First Steps" (Info View) and "Live Concepts"
  (Status Bar) — https://www.ableton.com/en/live-manual/12/first-steps/
- Photoshop, Rich Tool Tips —
  https://helpx.adobe.com/photoshop/desktop/get-started/set-up-toolbars-panels/show-or-hide-tool-tips.html
  (23.02.2026)

**What was searched for and NOT found** (so that the next wave does not search again): indications of an
obstacle outside the view in any of the five — a full-text search of the Resolve manual (`off screen`,
`out of view`, `outside the visible`, `scrolls the Timeline to`), the Premiere pages
`rearrange-clips-on-the-timeline`, `different-ways-to-move-clips`, `snap-clips`,
`navigation-controls-in-the-timeline`, `timeline-preferences` (auto-scroll / edge / arrow / distance), the
FCP pages `scroll-in-the-timeline`, `snap-to-items`, `arrange-clips`, `extend-or-shorten`, the VEGAS pages
`moving_events`, `adjusting_eventslength`, `quantize_frames`, `post_edit_ripple`. Also not found: highlighting
of the obstacle clip during a gesture (in none of them); the behaviour of JetBrains Search Everywhere with
unavailable actions; whether Ableton's Info View describes unavailable elements; whether Photoshop's Rich
Tool Tips appear on inactive elements; a "Studio only" message inside the Resolve interface (the manual marks
such features in the headings, the interface simply greys them out).
`[NOT VERIFIED]` — the Premiere line "Trim media limit reached on Footage 1" and the white triangle at a clip
edge: they are absent from Adobe's official documentation; confirmation exists only on forums.
