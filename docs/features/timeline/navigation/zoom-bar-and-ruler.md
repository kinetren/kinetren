---
id: F-TL-009
title: Overview bar and timeline ruler
status: candidate
priority: B
cost: M
category: timeline/navigation
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: partial
crates: [kt-app, kt-model]
invariants: [4, 8, 12]
ui_rules: [1, 3, 6, 13]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-08
trackers: [kde-invent, kde-bugzilla, pitivi, losslesscut]
---

# F-TL-009 — Overview bar and timeline ruler

## Essence

Two things above and below the tracks, answering different questions. The **ruler** says
WHAT TIME it is now: the ticks, labels, the unit (frames / timecode / seconds) and what
lives on it — marks, the region, render zones. The **overview bar** says WHERE I AM: across the full width of the window lies the whole project, and inside it — a rectangle of the visible
piece; drag it — and you pan, drag its EDGE — and you zoom. Neither zoom nor scroll answers the question
"where am I": each stores one number, while "where am I" is a pair.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Time ruler + horizontal scrollbar | the ruler's unit is a PROPERTY OF THE PROJECT: Options → Ruler Format or the Ruler tab in Project Properties, 15 formats (Samples, Time, Seconds, Time & Frames, Absolute Frames, Measures and Beats, Feet and Frames 16/35 mm, SMPTE 23.976/24/25/29.97 DF and NDF/30, Audio CD 75 fps) |
| Adobe Premiere | 26.x | Zoom scroll bar (navigator) | a strip under the tracks; the zoom handles APPEAR ON HOVER over its ends, drag an end — the visible range of the ruler changes. The Work Area Bar — at the bottom of the ruler itself |
| DaVinci Resolve | 21 | Edit: zoom slider; Cut: dual timeline | on Edit there is no overview — the "+/−" slider at the top right responds only to "how much is visible". On Cut the overview is made by a SECOND timeline: the upper one shows the whole programme, the lower one — the zoom around the place of work, and the zoom of neither of them is given to the user |
| Final Cut Pro | 12.3 | Zoom slider in Clip Appearance | there is no overview strip; zoom is a slider in the Clip Appearance popover and Cmd +/−, the ruler is the project's (or source's) timecode in a separate floating window |
| After Effects | 26.x | Time Navigator | Start/End brackets above the ruler: drag a bracket — zoom, the middle — pan. Double-click on the navigator — zoom to individual frames, Shift+double — the whole composition, Shift+double once more — a RETURN to the previous brackets |

**Outside the core.** Without Kdenlive the mechanism cannot be described: it is the only programme where the turning of a scrollbar into a zoom strip went public, in the tracker, with a date and with a checklist "where else" — and from the same place one can see what it costs.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | Zoom bar (it is also the horizontal scrollbar) | "zoom in/out in the timeline by dragging the edges of the timeline scrollbar" — a separate zoom widget was never created; the scrollbar handle was taught to stretch from the edges. On the ruler itself — the timeline zone (a square in the centre drags it whole, the edges are the boundaries), the preview render zone coloured by state (red / yellow / green) and the markers; the unit is hh:mm:ss:ff or frames |

## The trick behind it

**1. The overview bar and the zoom bar are one object, and that is precisely why nobody asks for it by name.** The scrollbar handle carries THREE values at once: its position —
"where I am", its width — "how much is visible", the length of the groove — "how much there is in total". The zoom slider carries one, and there is nothing to make it fall out of step with the window's position: 50 %
zoom says nothing about which half of the project you are standing in. Hence an observation visible only from the count: the words "minimap", "bird's eye",
"navigator" in nine trackers give two tasks, and neither is about the timeline — but
people formulate the same need as "Zoom bar it all" (37 comments) and
"Add a scroll bar after zoom in timeline". **The user asks for an overview in
words about zoom**, because he has no separate name for "where I am".

**2. In AE the bar holds a third state that neither zoom nor scroll has.**
Double-click on the Time Navigator — frame zoom, Shift+double — the whole composition,
Shift+double once more — a return to THE SAME window from which one left. Zoom stores a number,
scroll stores a number, while the bar stores a pair AND remembers the previous one. This is exactly what
a person otherwise keeps in their head: "I was looking at this piece, got distracted by the general plan,
I'll go back."

**3. A ruler with its own coordinates lies — and lies unnoticed.** Pitivi
drew its ticks as relative offsets from the previous one, accumulating error;
the divergence from the real frame grid is not visible at any zoom except the extreme one, and it was discovered only by stepping through frames at 23.976 (#1260, the author's diagnosis quoted verbatim below). The conclusion is general, not about Pitivi: **the tick step must be a divisor of the frame grid and must be computed from the absolute tick**, otherwise the ruler is not a measuring instrument but an ornament. This cannot be checked by eye, only by instrument: a frame at 25 fps is 3600 ticks, and a miss by one tick looks like a hit.

## Pros

- The overview bar saves not movements but **memory**: without it the structure of the project
  ("interview block — titles — b-roll") lives in the editor's head, and every return from a zoom costs a re-reading. With it the structure lies on the screen permanently and requires not a single gesture to be read.
- Zoom and pan become ONE gesture with feedback: you drag the edge — and you immediately see what share of the project will remain in the window. Ctrl+wheel does not show this until you let go.
- The ruler's unit, moved into a project property (Vegas), makes editing to music and editing to timecode different MODES OF ONE ruler, rather than two different counting habits in the head.
- Zones on the ruler (the render zone in Kdenlive and Premiere, the work area in AE) put the render state exactly where one looks at it during navigation — without creating a separate panel for it.

## Cons

- The overview bar is **permanent**: it occupies a horizontal strip all the time, while it is needed at the moment "I got lost". This is a direct conflict with rule 13 (do not waste space), and it is real: in Kdenlive the item "Timeline vertical scroll bar" in checklist #651 has stayed unticked for five years, while three neighbouring items are closed. The horizontal strip was justified by space, the vertical one was not.
- An overview to which zoom has been given stops being an overview: in Resolve's Cut the two timelines are deliberately not zoomed by hand. That is, the bar has no "settings" — either it always shows everything, or it is a second timeline, and that too has to be found somewhere.
- Zoom handles that appear only on hover (Premiere) — a saving of space at the cost of discoverability: until you hover over the end of the bar, there is no zoom there.
- The ruler is the most expensive widget to maintain of all the "simple" ones: in Pitivi 26 tasks out of 2672 (1.0 %) — HiDPI, system font scaling, ticks disappearing at 20- and 30-second steps, a port to another toolkit, a cache of theme colours, synchronisation of scrolling with the canvas. In Kdenlive, where the ruler lives in the coordinates of the timeline itself, — 4 out of 6346 (0.1 %), and those are about WHAT lies on it, not about where it is drawn.

## What users say

> The latest addition of a zoom bar to the effects UI has brought an enormous
> usability improvement. Let us zoom bar everything:
> — kde-invent #651 "Zoom bar it all", 30.04.2020,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/651

Further in the body is a checklist of four places; three are ticked, "Timeline vertical scroll
bar" is not ticked, to this day. The task is closed, 37 comments.

> The reason is, among other things, that the ruler uses relative paint positions
> instead of using absolute positions, and adds some small offset errors
> everytime.
> — pitivi #1260, 27.08.2012,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1260

The author found this not by looking at the ruler but by stepping through frames at 23.976: the hover tooltip showed the correct time, while the ticks did not.

> Starting a draft to improve the timeline ruler and bars visuals.
> — kde-invent #990 "Improve timeline ruler and bars", 13.03.2021,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/990

Still open (last activity 02.11.2025, 28 comments): half of the checklist concerns SEPARATING onto their own strips the render zone, the effects zone,
the guides and the markers, which today lie on top of each other.

SIGNAL: ruler and zoom bar — 26 tasks in pitivi, `ruler|zoom.?bar`; share 1.0% versus 0.1% in kde-bugzilla

SIGNAL: overview by name — 2 tasks across 9 trackers, `minimap|mini.?map|bird.?s.?eye|navigator`; share 0.0% in kde-bugzilla versus 0.1% in olive, and neither of the two is about the timeline

The second tally is itself the finding: the need exists and is formulated ("Zoom bar it
all", "Add a scroll bar after zoom in timeline", kde-bugzilla #501912 "Storyboard
view (AKA nonlinear timeline zoom / overview)"), but it has no name, so a search for the word "overview" in other people's trackers gives zero and creates the false impression
that the feature is not being asked for.

## How it fits Kinotrain

ADDRESS: src/apps/kt-app/src/timeline.rs — for us this class is possible, and nothing catches it: `view_start = view_start.max(-(tpp * 40.0))` (line 1399) is the ONLY clamp of the window, and it is only the left one. To the right the window drifts into emptiness with no limit and no feedback, and there is nothing to show where it has drifted to: there is no horizontal bar on the timeline at all — the only scrollbar is vertical (`ui.id().with("tl-vscroll")`, line 3678).

### What already exists

- **Zoom and pan** — `timeline.rs:1254-1345`: Ctrl+wheel or pinch
  (`zoom_delta`) with the anchor under the cursor, the bare wheel (any axis) — pan,
  Shift+wheel — vertical scroll, Shift+Ctrl+wheel — row height 16..120 with
  the anchor on the row under the cursor. The limits are uniform and stand at the direct request of the
  owner: `TPP_MIN = 30`, `TPP_MAX = 2_000_000` (≈22 s/px, "the 30-minute project whole in the window", item 17 of R.6, item 60 of `docs/porting-notes.md`).
- **Ruler** — `RULER_H = STRIPES_H(11) + TIME_H(22)` = 33 px. The upper 11 px are the cache
  warmth strips (invariant 4), the amber "head" of the loop region and the band of the range mark; the lower 22 px — ticks, labels and the mark name ON TOP of the digits, on the same
  row (rule 13, item 3 of R.12 — a separate strip under the marks was introduced and removed).
- **Step ladder** — `RULER_LADDER = [1s, 10s, 1m, 10m, 1h]`, `ruler_steps()`
  takes the minimum step ≥7 px for a tick and ≥70 px for a label; the format is
  `format_ruler_time()`, mm:ss up to an hour and h:mm:ss beyond. Closed by the test
  `t60_zoom_out_support` (kt-app unit) — previously the loop ran through seconds and at zoom-out gave thousands of shapes per frame.
- **Scrubbing on the ruler** with jump heat-zones at the edges and the zone hint
  `k::tl::hint::RULER` (`timeline.rs:2190`), the follow mode and its automatic return
  (`follow_view` / `follow_rearm`, items 59 and 72 of `docs/porting-notes.md`): manual
  zoom/pan suspends following, and after 1.5 s of silence it returns.
- **Timecode with frames is already written** — `src/apps/kt-app/src/util.rs:217`
  `format_timecode()` gives `00:00:00.000 F00`, and there is also the reverse parse
  (`parse_timecode`, tests t63/t93/t98).

### What is missing

- **There is no overview bar in any form.** Checked by grep: the only timeline scrollbar is
  vertical. The share of the visible part of the whole is shown nowhere, and one cannot return "to the whole project" with one gesture.
- **The denominator for such a bar does not exist as a function.** The project length is
  computed by three independent expressions: `src/libs/kt-engine/src/export.rs:73`,
  `src/libs/kt-model/src/ops.rs:68` (private `block_len`) and
  `src/libs/kt-otio/src/convert.rs:1045`. The model has no public "end of content",
  while the overview bar needs it every frame.
- **The ruler is silent exactly where it is most needed.** The minimum step of the ladder is
  1 second; at `TPP_MIN = 30` this is 3000 px, while a frame at 25 fps (3600 ticks) is
  120 px. In the very editing range of zoom there is NOT ONE tick between seconds:
  we do not lie, as Pitivi does — we simply say nothing. At the same time the toolbar at this
  moment shows `F00`: two notations of time in one window.
- **The unit of the ruler is not selectable.** In Vegas this is a project property with 15
  values; for us it is hard-coded mm:ss in `format_ruler_time`.
- **Render/cache zones on the ruler exist only as coverage.** The strips show the cache
  warmth, but for us the notion "this piece is rendered into the preview" does not exist — unlike Kdenlive and Premiere, where it is a colour on the ruler itself.

### What gets in the way

- **Rule 13 head-on.** "A strip across the full width of the screen for the sake of a label that
  appears occasionally is a defect, even if it is neater that way." The precedent is recorded in
  the file itself: a separate 13 px strip was already introduced for the names of marks and then removed,
  the name being moved onto the time-label row. The overview bar must either pay for itself
  permanently (it answers a question that is always present), or sit in a place already occupied — for example, become a second use of the 11-pixel cache-strip band, which already stretches across the full width and already draws the whole project.
- **`RULER_H` is a load-bearing constant.** It underlies the hit test "mouse on the ruler" (six places of the form
  `pos.y > rect.top() + RULER_H`), the clip of the track area and
  `vertical_layout(avail_h - RULER_H - 6.0)`. Any growth of the ruler shrinks the tracks, and the layout limits are held by `t76`/`t127`, and they stand at direct requests.
- **The frame grid changes the signature.** `ruler_steps(tpp)` is a pure function of one argument, and t60 checks it with numbers; a step that is a multiple of `90_000/fps` requires passing the project's fps and rewriting the test.
- **Invariant 8 is the main danger and at the same time our advantage.** In others' editors zooming the timeline drags decode along with it, and in the trackers this reads as "hangs on zoom"; for us this class is closed (waveforms from the peak pyramid, thumbnails from `<project>.ktindex.db`, tests 13/15 `src/libs/kt-engine/tests/viewport.rs`), and in v1 it existed — item 61 of the table in `docs/porting-notes.md` verbatim: "timeline scroll/zoom → a storm of thumbnail/waveform decodes… astronomical numbers of calls to avcpp". The overview bar draws the WHOLE project at once, so this is the first widget that by construction asks for data about all the material. It must be built from the model and `BurstCache::coverage()`, and not one of its pixels has the right to request a frame — otherwise we ourselves will bring back exactly the defect for which everyone else pays.

### Which invariants it touches

- **8** (scroll/zoom does not generate decode requests) — directly and more strictly than usual: see
  above. The regression test must extend `viewport.rs`, not merely look at the picture.
- **4** (the cache is the only truth about coverage) — the overview bar takes the warmth from
  `BurstCache::coverage()` and does not keep its own bookkeeping.
- **12** (repaint only on movement) — the bar does not animate and does not redraw the timeline while the window is not being moved; its contents are cached by `cache.epoch`.
- UI checklist rules: **13** (space is the main constraint), **6** (everything dragged with the mouse has a min and a max — `TPP_MIN`/`TPP_MAX` already exist and are reused by the handle edges), **1** (the grab zone of the handle edge is wider than its drawing, otherwise "dragging the edge" is aiming), **3** (switching the overview does not change the layout).

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: (1) the overview bar + a public "end of content"; (2) the frame step of the ruler and the display unit |
| Where the main work is | kt-app (`timeline.rs`: the bar, the grabbing of the edges, `ruler_steps`), kt-model (a public end of content instead of three copies of the expression) |
| Project format | the bar — does not touch it (the window lives in `UiState`, this is `dirty_ui`, item 96); the CHOICE of ruler unit — does touch it: it is a project property, so a new schema version |
| Tests to be written | a harness test: the handle shows the share of the visible part and the position of the window, dragging the middle = pan, dragging the edge = zoom, and `view_start` does not drift at this point (rule 3); the right limit of the window exists; an extension of t13 `viewport.rs` — "drawing the overview bar produced not a single request to MediaWorker"; t60 for a step that is a multiple of `90_000/fps`, and for the match of the ruler label with `format_timecode` |
| Depends on | nothing; F-HLP-007 (feedback during a gesture) — an adjacent surface, does not block |
| Breaks compatibility | no (stage 1); stage 2 adds a field to the project |

## Verdict

We take it, but split in two and in the reverse order of obviousness. **The cheap and urgent
half is the ruler**: we have no right limit of the window and not a single tick finer than a
second at frame zoom, even though the frame timecode is already written and shown
two steps away, in the toolbar. This is not a feature, it is a discrepancy inside one window.
**The expensive half is the overview bar**: a real one, with zoom at the edges, exists in
two of the five core editors (Premiere, After Effects); in Vegas "where am I" is answered
by a proportional scrollbar without zoom, Resolve on Cut replaces it with a second
timeline, and only FCP answers with nothing. And it is also the only widget
that by construction asks for data about the whole project at once —
that is, the first real examination of invariant 8. It must be designed not as a new
strip but as a second use of the 11-pixel cache-strip band: that band already stretches
across the full width and already draws the whole project, and in this case rule 13 turns from a
prohibition into a hint about where exactly to put it.

## Sources

- kde-invent #651 "Zoom bar it all", 30.04.2020 (closed, 37 comments):
  https://invent.kde.org/multimedia/kdenlive/-/work_items/651
- kde-invent #990 "Improve timeline ruler and bars", 13.03.2021, open to this
  day: https://invent.kde.org/multimedia/kdenlive/-/work_items/990
- pitivi #1260 "Inaccurate calculations create desynch offsetting in the ruler
  with decimal framerates", 27.08.2012:
  https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1260
- pitivi #1228 "Smarter and configurable ruler timecode / timestamp style",
  04.05.2012: https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1228
- pitivi #1236 "Allow displaying SMPTE timecode representation in the ruler",
  30.05.2012, open: https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1236
- pitivi #2622 "Ticks in ruler disappears at 20 & 30 second intervals",
  04.06.2022: https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2622
- kde-bugzilla #501912 "Feature: Storyboard view (AKA nonlinear timeline zoom /
  overview)", 23.03.2025: https://bugs.kde.org/show_bug.cgi?id=501912
- losslesscut #532 "Add a scroll bar after zoom in timeline", 06.12.2020:
  https://github.com/mifi/lossless-cut/issues/532
- Kdenlive 26.08 Manual, Timeline (the ruler, zones, "zoom in/out by dragging the
  edges of the timeline scrollbar"):
  https://docs.kdenlive.org/en/user_interface/timeline.html (08.09.2026)
- VEGAS Pro help, Time ruler (the list of 15 formats and two places of change):
  https://help.magix-hub.com/video/vegas/21/en/content/topics/2-window/ruler.htm
  (08.09.2026)
- Adobe, Navigation controls in the Timeline (zoom scroll bar, handles at the ends):
  https://helpx.adobe.com/premiere/desktop/edit-projects/change-clip-sequence/navigation-controls-in-the-timeline.html
  (08.09.2026, via search results — direct access to `helpx.adobe.com` from
  this environment is still closed, see `_editors.md`)
- Blackmagic Design, DaVinci Resolve — Cut (dual timeline: the upper one shows the whole
  programme, the lower one — the zoom): https://www.blackmagicdesign.com/products/davinciresolve/cut
  (08.09.2026)
- Apple Support, Zoom in to and out of the timeline in Final Cut Pro for Mac
  (the slider in Clip Appearance):
  https://support.apple.com/guide/final-cut-pro/zoom-in-to-and-out-of-the-timeline-ver4e2edcc/mac
  (08.09.2026)

**Caveat on the completeness of the sample.** The count was taken across nine exported trackers
(flowblade, kde-bugzilla, kde-invent, losslesscut, mlt, olive, otio, pitivi,
shotcut); openshot, blender-vse and ffmpeg-trac were still downloading at the time of writing.
The figures are reproducible with `src/scripts/tracker-rank.py --query 'ruler|zoom.?bar'` and
with the same script `--query 'minimap|mini.?map|bird.?s.?eye|navigator'`; with the export
the denominators will change, the ratio of the pitivi ↔ kde-bugzilla shares — hardly.
