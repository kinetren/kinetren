---
id: F-HLP-007
title: Feedback during a gesture — trim delta, offset, new timecode
status: candidate
priority: B
cost: M
category: help/hints
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [9, 10, 13, 16, 20]
round: null
point: null
tests: []
depends_on: [F-HLP-001, F-HLP-005, F-HLP-010]
checked: 2026-09-07
---

# F-HLP-007 — Feedback during a gesture — trim delta, offset, new timecode

## Essence

The editor drags an event edge, and while the mouse button is held down the program answers him with numbers: by how much he has already moved the edge, what the duration has become, what the timecode under the cursor is, what the edge is currently snapped to. Let go — the numbers disappear. This is not a description of the object under the cursor and not a tooltip: its source is different (the state of the drag, not hovering) and its moment is different (from press to release). It is the only reference surface that people read WITHOUT tearing their eyes away from the place of action.

## Who has it and how

The question in this row is one: does the editor answer the gesture with a NUMBER — and WHERE is this number. "Where" matters more than "what": at the cursor it means it covers something, in a separate panel it means the eyes travel away from the edge (the complaint about Info View, F-HLP-002).

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no number at the gesture; Event Length in the event header | the 2026 help describes the trim feedback as a GRAPH: handles appearing when the mouse approaches the edge, snapping to the grid/markers, the cursor shape (slip/slide/stretch), a zigzag between frames when stretching. A number exists, but it is permanent and optional: the Event Length button in the event header (More → Edit Visible Button Set); the stretch percentage — in the corner of the event when Active Take Information is enabled |
| Adobe Premiere | 26.x | a small popup window at the edit point + Trim mode | "a little window pops up showing you how many frames you are moving your edit point" (Community Expert, a reply to the complaint "I can't see how many frames"). Separately Trim mode (Shift+T): the Program Monitor is split in two — the outgoing and incoming frames, with shift counters below them |
| DaVinci Resolve | 21 | a two-window trim viewer | the viewer shows the outgoing and incoming frames and their change in real time; the official Cut page describes the behavior as "the other side will be extended by the same number of frames". The exact look of the NUMERIC offset and its place on the screen could not be confirmed from the primary source — [NOT CHECKED] |
| Final Cut Pro | 12.3 | numeric fields ABOVE the edit point | "Numerical timecode fields indicate the duration of the clip and the amount of time you're moving the edit point" — TWO numbers at once (the result and the path), right at the edge being dragged |
| After Effects | 26.x | the Info panel | "When you move a layer by dragging it in the Composition panel, the Info panel shows the change in the Position property as you drag" — a fixed panel, a live delta of the parameter, but off to the side of the hands |

Three different answers to "where": **at the cursor** (Premiere), **at the place of action** (FCP), **in a fixed area off to the side** (AE Info, the Resolve viewer, the Vegas event header). The first is closed for us by the owner's decision (item 14 of round 8), the second is almost closed by the same decision — any plate at the edge covers the neighboring frame, the third we have already built and during the gesture it is EMPTY.

A second table is absent here, and this is a result in itself: by the rules of `_editors.md` it is mandatory when three or more core rows are "no", or when "the trick behind it" is taken from outside. This is the only note of the `hints/` group where the feature exists in all five core editors and where the dispute is not about presence but about place — software outside video editing adds nothing here to the five rows.

## The trick behind it

**What the mechanism carries that one status-bar line cannot hold.** Not the volume: the number is short (six characters), it fits into the 160-character limit with a margin of twenty times. A line cannot hold the DISTANCE — the proximity to the edge that makes the number useful. For us the load therefore splits in two: the digit moves to the step of the slot that is free during the gesture (`HintKind::Mode`), while the proximity moves nowhere — it stays either on the object itself (a permanent label on the event, like Event Length in Vegas), or is lost altogether.

Three details that are not in the list of features:

1. **FCP has TWO numbers, and the second is not a delta.** "the duration of the clip AND the amount of time you're moving the edit point" — during one and the same gesture they show both the total ("it has become this much") and the path ("moved by this much"). One is not enough: during the trim the editor switches between these questions, and he cannot subtract one from the other in his head — both numbers change.
2. **Premiere pays for proximity exactly with what our owner forbade, and pays twice.** The window pops up at the edit point, that is, it covers the neighboring frame; and it has a "gone" mode — the complaint with which the thread begins sounds like "I now can't see how many frames on adjust to my clip". So a popup at the cursor is not "simply closer", it is also an extra setting that breaks.
3. **Vegas solved the same task WITHOUT a popup — with a permanent number ON the object.** The duration lives in the Event Length button in the event header and is enabled in the set of visible buttons; the stretch is a percentage in the corner of the event. This is the only one of the five constructions that does not cover, does not get delayed and does not disappear on release. The price — the space on the event and the fact that the number must first be enabled.

And a fourth, ours: **the cost of the absence of this help is measured in undo.** All other surfaces of the wave cost the user reading; this one costs him a REPEATED GESTURE. A gesture without a number is verified in only one way: let go, see what came out, and if it is not right, press Ctrl+Z and drag again. That is why it is the only one whose absence is visible in the undo counter, not in the letters to support.

## Pros

- **The slot during the gesture is FREE, and this is the only place of the wave where rule 10 ("exactly one hint") costs nothing.** The entire analysis of the zone under the cursor — all sixteen branches — stands under `if self.drag.is_none()` (`src/apps/kt-app/src/timeline.rs:2179`), that is, while `Drag::*` is running, the zone hints are not written at all. There is no competition between hovering and the gesture: hovering goes out of the way by itself.
- **A step in the priority ladder already exists, and it is exactly the one needed.** `status_line` (`src/apps/kt-app/src/app.rs:4426`) goes error → hover → **mode** → message → standby line, and `HintKind::Mode` has its own green color (`app.rs:1305`). The gesture's number is, in meaning, a "mode": it lives as long as the state lasts, and must stand above the aging message and the standby line, but below the error. Nothing needs to be invented.
- **The frame budget is already paid, and unconditionally.** `self.drag.is_some()` is part of `animating` (`src/apps/kt-app/src/app.rs:1243`), so during ANY drag the application is redrawn every 16 ms (33 ms in economy mode) regardless of whether the mouse moves. The dispute with invariant 12 is settled not by reasoning "a drag is movement", but by this line: the frame is drawn anyway.
- **The number hardly costs any translation.** The price of a new help line (F-HLP-033) — five languages, glyphs, subsets, Arabic bidi; for digits and colons the glyphs are already verified, only the frame around them is translated ("±%s frames." → "%s"), and some gestures need no words at all.
- **Verifiable by machine.** The number during the gesture reaches `status_line_shown`, and the harness can drive the whole gesture (`hover_at` → `drag_at` → `hover_at` → `drop_at`, `src/apps/kt-app/src/layout_tests.rs`, t172). A popup has no such single point — we have one.

## Cons

- **The distance is not cured.** The line stands in the BOTTOM RIGHT corner of the window, the edge — anywhere; with the window maximized this is more than a thousand pixels. This is exactly the objection that Ableton's users raise against Info View (F-HLP-002), and moving the number closer to the edge would mean a plate over the timeline, that is, a dispute with item 14 of round 8.
- **We have no quantization by frames, and "+12 frames" would be a lie.** `timeline.rs` has not a single division by the frame step: a clip snaps to an arbitrary tick, magnet targets the boundaries of neighbors, markers, the region, the playhead and zero (`snap_targets`, `timeline.rs:1183`), and never the grid of frames. The honest unit of delta for us is ticks or seconds; frames would have to be either rounded and said as "≈", or quantization introduced, and that is no longer a hint but the behavior of the edit.
- **Transience.** Let go — there is no number. Nothing to re-read, nothing to check against what came out; that is exactly why Vegas keeps a second, permanent form of the same number on the event.
- **The formula is different for every gesture.** There are seventeen `Drag` variants, and with the three `DragMode` modes — nineteen different gestures, and the "delta" in them differs: ticks, pixels of height, a share of the level, a row number, the number of events in a frame. There is no common number — these are nineteen small decisions, and each of them can be forgotten (the lesson of maintaining the Blender status bar, F-HLP-005).
- **The error must remain above.** Rule 16 and t172 require that the gesture's stop be shown in red and override everything; the number has no right to displace the message about an obstacle. The boundary with F-HLP-010 runs here: there — a refusal, here — a successful gesture.

## What users say

> I now can't see how many frames on adjust to my clip on the timeline while
> shortening or lengthening it.
> — member of the Adobe Community, 19.09.2018,
> https://community.adobe.com/t5/premiere-pro-discussions/timeline-can-t-see-how-many-frames-secs-i-changed-to-clip-pic-included/m-p/10078705

> If you perform an edit a little window pops up showing you how many frames you
> are moving your edit point.
> — Ann Bens (Community Expert), same thread, 19.09.2018

Two messages from one thread — a ready-made formulation of our task: the number during the gesture is so essential that its disappearance is noticed and complained about, and so habitually lives in a popup that an expert explains its existence in one phrase.

The owner — about a neighboring case of the same gesture (a push into someone else's event), and this is the only remark where he speaks about feedback DURING the move:

> "the attempt to cross several events is also done stupidly — I haven't scrolled
> there myself yet — didn't understand that there turns out to be another event at the end
> that gets in the way"
> — the owner, R.28 item 5, `docs/user-feedback.md`, 2026

## How it fits Kinotrain

### What already exists

**The slot and its ladder** — F-HLP-001: `status_line`
(`src/apps/kt-app/src/app.rs:4426`), `HintKind` (`app.rs:4408`), rendering `draw_status_hint` (`app.rs:1274`), the result in `status_line_shown`.

**What happens to the slot during the gesture — the exact answer** (the brief assumed competition with hovering; there is none):

1. The whole zone analysis is switched off: `if self.drag.is_none() && let Some(pos) = hover`
   (`timeline.rs:2179`). While a drag is running, `hover_hint` is not written from the timeline.
2. Foreign panels are silent too: `hint()`/`hint_into()` set a hint by `r.hovered()`, and in egui 0.36.1 "`hovered` is always false when another widget is being dragged" (`response.rs:517`). The toolbar, the bin and the windows are unreachable for hovering by the device's library during a drag.
3. So the slot receives: a **red error** of the stop (`self.blocked`, `timeline.rs:2624` and `:2657` — only for `Drag::Clip`/`Move`); otherwise one of the **two leaks** (below); otherwise a still-alive **message** `note()` (TTL 6 s); otherwise the **standby line** — "drop a media file on the window". That is, the editor who drags an event reads in the status bar an invitation to drop a file.
4. **`HintKind::Mode` in the program has exactly one producer** — `if self.dnd_hover.is_some()` (`app.rs:1279`), the text `hint.drop_here` ("Release — the clip lands under the mouse"). This is a hint DURING the gesture, and the only one. The irony is pointed: the only gesture the program talks to is the one that starts OUTSIDE its window (a file drop) or in the bin.

**Two leaks into the slot** (neither is tied to `drag`, both are conclusions from the code, there is no live scene):

- `timeline.rs:1703` — the hint of the lower edge of the row (`tl.hint.row_height`) stands under the geometric check, not under `drag.is_none()`; during `Drag::LaneHeight` the pointer stays at the edge, and the instruction "Track bottom edge: drag to set this track height" hangs in the slot — that is, how to START a gesture that is already running.
- `timeline.rs:3548` — the caption of a morph block ("Layer" / "Events: N") goes into `hover_hint` when it does not fit into the block; the condition is `pointer_hover_pos()` inside the rectangle, and during `Drag::Morph` the block moves under the pointer.

**What is visible during each gesture now** (nineteen gestures, seventeen `Drag` variants, `timeline.rs:408`):

| Gesture | What the user sees | Number |
|---|---|---|
| `Clip`/`Move` | the ghost of the event moves, fill ×0.75, a yellow snap line, obstacle highlight + red line when stopped | no |
| `Clip`/`TrimStart` | the edge moves, the waveform and filmstrip scroll along with it (`preview_src_shift`, `timeline.rs:182`, t169), a snap line | no |
| `Clip`/`TrimEnd` | the edge moves, a snap line | no |
| `Fade` | a preview of the fade triangle and the handle (`timeline.rs:3821`) | no |
| `Level` | the level line moves (rail ±6.02 dB for audio, opacity for video) | no |
| `LaneHeight` | **nothing**: the height is applied only on release (`set_row_height` in `drag_stopped`, `timeline.rs:1730`) | no |
| `LaneRow` | a light blue semi-transparent band on the target row | no |
| `Scrub` | the cursor moves; the toolbar timecode is updated (`format_timecode`, `src/apps/kt-app/src/toolbar.rs:434`) — the only live number in the program, and it is in the opposite corner of the window | YES |
| `Region` | the rectangle of the region grows | no |
| `Rubber` | the selection frame | no |
| `RegionEdge` | the edge of the region moves | no |
| `Marker`, `MarkerEdge` | the flag/band of the marker moves | no |
| `TransitionEdge` | the transition window changes live (`SetTransition` with coalescing) | no |
| `TransitionBody` | the line of the future joint | no |
| `Morph`, `MorphEdge` | the block moves/changes; the caption may leak into the slot | no |
| `Camera`, `CameraEdge` | the same (the live preview was set up in R.38: "before R.38 the camera stood still during the drag until release — the gesture looked dead", `timeline.rs:3553`) | no |
| file drop / bin card drop | the track highlight, a ghost for ~3 s, a vertical line (`timeline.rs:3605`) **and the `HintKind::Mode` line** | no |

Outside the timeline there is exactly one number during a gesture: in the automation editor the panel header shows the time of the selected key and its share of the event (`fx.key_at` = "key: {0} s of the source ({1}% of the event)", `src/apps/kt-app/src/automation.rs:241`), and it follows the key during the drag, because `view.sel` is reassigned to the key being dragged. The VALUE of the key is not shown at that time — only the scale bounds on the left (`automation.rs:530`).

### What is missing

- **Not a single delta, not a single new duration, not a single timecode under the cursor** — across all nineteen timeline gestures.
- **The magnet does not name its target, and today there is nothing to name it with.** `snap_targets()` (`timeline.rs:1183`) collects a `Vec<i64>`: zero, the playhead, the starts and ends of all other clips, the points and ends of markers, the boundaries of the region — and **the type of the target is lost at the point of computation**, before any drawing. `snap_point`/`snap_move` (`timeline.rs:749`, `:767`) return a tick, then a yellow line is drawn (`timeline.rs:4272`) — and that is all. The user sees that the event jumped, but does not know to what: to a marker, to a neighbor, to the cursor or to zero. To name the target, one will have to change the signatures of three pure functions and their tests (t60, t95) — this, and not the text, is the main work of the note.
- **The magnet can, in addition, pull past the END.** `snap_move` magnets the start OR the end, the smaller offset wins (`timeline.rs:767`), so the yellow line appears on the opposite edge of the event — where the hand is not. Without a target name this looks simply like a jerk.
- **The crossfade from an overlap is decided ONLY on release.** `MovePlan::Crossfade` is not asked about at all during the drag: only the single branch `MovePlan::Clamped` is checked in the gesture (`timeline.rs:2630`), while the full plan is read once on release (`timeline.rs:2864`) and executes `TrimEnd` on the neighbor to the left, `TrimStart` on the neighbor to the right and `AddTransition`. So by the outcome of the gesture TWO events are trimmed that the user did not drag, and during the gesture this is not said a word. This is a direct clash with rule 20 and with the wording of R.28 item 6 ("as much was there and must remain… NOTHING ANYWHERE SHOULD GROW BACK/BE CUT OFF"): the gesture is lawful (the decision of R.24), but its silence is not.
- **`Drag::LaneHeight` has no preview at all** — a gesture in which during execution not a single pixel changes, except the leaked hint. This is the same "dead gesture" that was fixed for the camera in R.38.

### What gets in the way

- **The distance to the corner of the window** — structural; it is cured only by a second surface on the event itself, and there is only room for that in the clip header.
- **The absence of a frame grid** (see "Cons"): the number will have to be given in seconds/ticks or rounded, and this decision is taken by the round, not by the note.
- **The order of the steps must not be touched**: the error must stay above the number (rule 16, t172), the message and the standby line — below. The only safe insertion point is `mode` in the call to `status_line` (`src/apps/kt-app/src/app.rs:1287`), where today only the file drop stands.
- **The two leaks will have to be closed before the insertion**, otherwise they will compete with the number for the same step: `row_height` — pin to `drag.is_none()`, the morpher caption — to the same condition.
- **The ban on popup hints and its guard t83 are not affected by anything**: no new layers appear, the drawing order does not change, the delay stays zero. The note does not argue with either of the owner's two decisions.

### Which invariants it touches

- **Invariant 12 — not violated, and the argument is stronger than the general reasoning.** The dispute "the hint must update every frame" is removed by the fact that the frame during a drag is drawn anyway: `self.drag.is_some()` is already part of `animating` (`src/apps/kt-app/src/app.rs:1243`), interval 16 ms (33 ms with `ui_economy`), and this does not depend on whether the mouse moves. The number adds not a single repaint. What remains is the letter "no allocations in draw paths": the number format is a `format!` per frame, exactly as `blocked_text()` (`app.rs:1930`) already does during a stop. The implementation constraint from this is strict: format only while a drag is running, and do not assemble the string in frames without a drag.
- **Rule 10** (exactly one hint) — observed by construction: the slot is the same, the line is the same, at the same time there is still only one on the screen.
- **Rule 13** (do not waste space) — the main argument "for": during the gesture the status bar strip is occupied by the standby phrase about file drops, that is, right now the space is literally wasted.
- **Rule 16** — the boundary with F-HLP-010: the red error is above the number, always.
- **Rule 20** — the point about the crossfade above: the rule requires that nothing grows back by itself; since the gesture that cuts off neighbors is kept consciously, it must say so before release.
- **Rule 9** (nothing superfluous) — forbids writing a number where it is already visible: for `Rubber` and `LaneRow` there is nothing to show, and inventing a figure for them for the sake of symmetry is not allowed.
- The project format, the engine and invariants 1–11 are not touched: everything is inside `src/apps/kt-app`.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: (1) the number into the slot by gestures, (2) the name of the magnet target and a warning about the crossfade |
| Where the main work is | kt-app: `timeline.rs` (the delta formula for each `Drag::*`, the type of the magnet target in `snap_targets`/`snap_point`/`snap_move`, closing the two leaks), `app.rs` (insertion into the `mode` argument of `status_line`), `i18n/*.toml` (frames around the numbers) |
| Project format | not touched |
| Tests to be written | `reach_tests::every_gesture_shows_its_delta` (an exhaustive `match` over `Drag` — the compiler breaks the build on a new variant, as with `TlEntity` in t253; the harness drives each gesture and requires a non-empty number in `status_line_shown` with `HintKind::Mode`); a harness test "the magnet named the target"; a harness test "a push into a neighbor warns about the crossfade BEFORE release"; t100/t125/t128 automatically for the new keys |
| Depends on | F-HLP-001 (the slot and the ladder), F-HLP-010 (the error stays above), F-HLP-005 (the same slot during the gesture for the modifier cut — separated by steps, not by slots) |
| Breaks compatibility | no |

Priority B, not A, deliberately: without numbers editing is possible, but it is verified by undos. Raising it to A makes sense together with quantization by frames — then "by how many frames" becomes a question to which we have an exact answer at all.

**The boundary with F-HLP-005.** That note owns the MODIFIERS — what changes if Shift/Ctrl is held; this one owns what is shown WHILE YOU DRAG. Both aim at one step, `HintKind::Mode`, and this is not a conflict but an order: while the drag has not started, the step is occupied by the layout cut; as soon as `self.drag` becomes `Some`, it is occupied by the gesture's number, and the modifier inside the gesture (for us exactly `snap_off`, `timeline.rs:1996`) becomes part of the same number — "snap off" is appended to the delta, not put on a separate line.

## Verdict

We take it. This is the cheapest position of the wave in terms of "benefit/cost": the slot during the gesture is EMPTY (not occupied by hovering, as was assumed); the step for it already exists and is already in use — but by exactly one gesture, coming from outside the window — and the frame budget of a drag is paid unconditionally, so invariant 12 does not dispute it. What is expensive here is not the number but two things around it: the name of the magnet target (the type is lost in `snap_targets`; three pure functions and their tests will have to be changed) and an honest unit of delta (we have no frame grid). The crossfade from an overlap, which on release silently trims two neighbors, is not decoration but a debt under rule 20.

TEST: reach_tests::every_gesture_shows_its_delta — today fails on all nineteen timeline gestures: `if self.drag.is_none()` in `src/apps/kt-app/src/timeline.rs:2179` switches the hints off for the duration of a drag, the only producer of `HintKind::Mode` is the file drop (`app.rs:1279`), and therefore while an event is being moved the status bar shows the standby "drop a media file on the window".

ONE LINE: overlap — not touched | delay — not touched | capacity — does not grow by a single character: the gesture's number takes the step `HintKind::Mode` of the same single line, which is FREE during a drag; the permanent event duration goes to a second surface, the label itself in the event header, following the Event Length example in VEGAS.

## Sources

- https://support.apple.com/guide/final-cut-pro/extend-or-shorten-clips-ver9847ec25/mac (Final Cut Pro 12: "Numerical timecode fields indicate the duration of the clip and the amount of time you're moving the edit point"; accessed 2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/timeline-can-t-see-how-many-frames-secs-i-changed-to-clip-pic-included/m-p/10078705 (both quotes of the thread, 19.09.2018; accessed 2026-09-07)
- https://helpx.adobe.com/premiere/desktop/edit-projects/trim-clips/about-trim-mode.html (Premiere 26: Trim mode via Shift+T, split-screen in the Program Monitor, the captions "trim indicators, shift counters"; accessed 2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/adjusting_eventslength.htm (VEGAS Pro 2026 Online Help, "Adjusting an event's length": handles at the edges, snapping, Shift as a temporary disabling of snapping, the zigzag and the percentage when stretching; accessed 2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/eventlengthdisplay.htm (VEGAS Pro 2026, "Event Length Display": "You can display the length of an event in the event header"; accessed 2026-09-07)
- http://vegaspro.helpmax.net/en/using-vegas-software/editing-events-on-the-timeline/slipping-and-sliding-events/ (a mirror of the VEGAS help: slip/slide feedback — the cursor shape only; accessed 2026-09-07)
- https://helpx.adobe.com/after-effects/desktop/work-with-layers/select-and-arrange-layers/selecting-arranging-layers.html (After Effects: "the Info panel shows the change in the Position property as you drag"; accessed 2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/cut (Resolve: "the other side will be extended by the same number of frames"; accessed 2026-09-07)
- Code and directories: `src/apps/kt-app/src/timeline.rs`, `app.rs`, `toolbar.rs`, `automation.rs`, `layout_tests.rs` (t172, t173), `src/apps/kt-app/i18n/en.toml`, `ru.toml`; `~/.cargo/registry/…/egui-0.36.1/src/response.rs` (the semantics of `hovered` during a drag) — read 2026-09-07.
- `docs/user-feedback.md`, sections R.24, R.28 (items 5, 6), R.38, R.40; `src/apps/kt-app/CLAUDE.md`, rules 9, 10, 13, 16, 20.

Caveats on trust:

- `helpx.adobe.com`, `docs.blender.org`, `reddit.com` and `community.borisfx.com` were not opened by direct request from this environment; the Adobe and Apple pages were taken via the text proxy `r.jina.ai`, the VEGAS 2026 documentation by direct `curl` from `cdn.borisfx.com`. The session's `WebSearch` budget was exhausted before the work started; the search went through `r.jina.ai` over DuckDuckGo.
- The Resolve line is the weakest: the official wording on the numeric display of the offset during a trim could not be confirmed; the mirror of the reference manual (`steakunderwater.com`) returns 404, the manual's PDF is 209 MB. Marked `[NOT CHECKED]` in the line itself.
- No live check was made for any of the five editors; everything said about Kinotrain is checked against the sources and given with line numbers.
