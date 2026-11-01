# Design

## Context

The perf box is drawn by `KtApp::draw_perf_box` (`src/apps/kt-app/src/app.rs`) inside the left
column's `ScrollArea`, under the media bin: five two-column rows (`ui.columns(2)` + `Truncate`, the
fix of item 3 of R.7.1 — a `Grid` with `Extend` inflated the column and ate the preview), the GPU,
FX backend and compose lines, the "Economy UI" checkbox, the flight-journal button and the
cumulative FFmpeg line, then two `RollingGraph`s (`perf.quality`, `perf.decode`). The Diagnostics
window (`draw_diagnostics`, id `kt_diag`, a `Tool` shell window 560×460, min 420×300) holds the
video cache occupancy line and bar, the third graph (`perf.churn`), the log buttons and the
problem log. Both are fed by one sampler, `sample_perf`, every 500 ms of frames.

`RollingGraph::draw` (`src/apps/kt-app/src/graphs.rs`) rebuilt every series' point vector, the
pinned legends and every label string on every frame it was drawn, and stepped its auto-scale
(attack 0.35 / decay 0.08) per DRAW, so the scale's speed depended on the UI frame rate.

## Goals / Non-Goals

**Goals:** one place for diagnosis (the owner's three graphs in one window); the bin gets the
whole column; nothing the box showed is lost; the window fills its area; zero rebuild cost on a
frame without a new sample.

**Non-Goals:** new metrics; a dropped-frame light in the preview (F-UI-006 records the industry's
always-visible layer, ours is the status line and stays as is); the Settings window of K13; the
general "windows fill their area" work of K12 (`widgets::window_body`, the other windows).

## Decisions

### D1. The three graphs are the three that exist, one per CAUSE

The owner's sentence does not name them. It counts them: the box had two graphs (Quality,
FFmpeg activity), the window one (Cache churn) — moving the box into the window makes three.
They are also the right three for a person, because each answers a different cause of "it does
not play smoothly" (F-UI-006, "The trick behind it"): **Quality** — did the display keep up (FPS of
the UI, V-fps of new video frames, the audio and video cache hit rates); **FFmpeg activity** — did
the decoder and the disk keep up (opens, packet reads, video and audio decodes per second);
**Cache churn** — did the cache throw away what was just made (evictions, misses, the no-read drops
that are prefetch thrash). Rejected: a CPU/GPU load graph — it is the answer of Resolve's guides
that sends people to the wrong fix, and we have no honest per-process GPU counter; merging two of
ours into one to make room — the owner asked for three.

### D2. The window layout: rows, occupancy, graphs sharing the height, log

Top to bottom: the "Performance" heading and the five two-column rows (the same `ui.columns(2)` +
`Truncate`; the 190 px row budget of t128 still holds — at the window's minimum width a column is
wider than that), the GPU/FX/compose/FFmpeg lines, the journal button with the stall count; the
occupancy line and bar; the three graphs; the log heading with Clear / Reset the graphs; the log.
The body is ONE vertical scroll (the shell's rule: no window-level scroll, the body scrolls) as
tall as the window gives; the graphs take `(viewport − what the rows used − the log's share) / 3`
each, with a floor of 64 points, and the log keeps a quarter of the rest (at least 80 points, its
own `stick_to_bottom` scroll). A larger window gives larger graphs; a screen lower than the floors
scrolls the body instead of pushing the window larger than the person made it (the shell's rule
"no window grows by itself", t921–t923). Default size 640 × min(720, screen − 60), minimum
460×320: a fixed 720 put the title and its cross above a 420-point screen's top edge (t779 — the
first run of this change was red there).

Rejected: the graphs side by side — the pinned legends need 86 points on the left of each, and
three of them in a 640-point window would leave the lines a third of the width; collapsible
sections — the owner wants to see the three together.

### D3. Revision: the perf box leaves the left column — owner, kkt-26.11.01 item 14

The box lived in the column since the I1 port of the v1 panel; rounds 6–7 fitted it there
(item 18 of R.6 and item 15 of R.7: truncation, two columns). Those were rules for drawing it in a
narrow column, not a decision that it must be there; the owner's new request moves it. The column
keeps its limits (`max_size`, `min_size`, the 232 default — set at the owner's request, untouched)
and now holds only the bin, whose `ScrollArea` stays. The bin's frame reaches the column's bottom
(`set_min_height` from the column's viewport, `clip_rect` — R.16): the freed space is the bin's
background — a file drops there, a right click there opens the bin's menu — not a dead strip under
a short frame (the owner's item 12 class: an area that does not fill its place).

### D4. "Economy UI" is not carried over

The box's checkbox and View → "Economy UI (30 fps while playing)" are one switch written twice
(the same `ui_economy`, the same text under two keys). The owner's item 13 is this exact class:
"why on earth is the checkbox in the window AND duplicated in View". The window does not get it;
the View item takes the box's hint (`hint::ECONOMY`), `perf.economy` leaves the catalogs, and t128
measures the View item's text against the same 380 px budget. Where View's settings end up is K13's
change (a Settings window); this change only refuses to add a second copy.

### D5. A graph is built once per sample (invariant 12)

`RollingGraph` keeps a draw cache keyed by `(rect, generation)`, where `generation` grows on
`push`/`clear`: the series lines tessellated into ONE `Arc<Mesh>` (`epaint::tessellator::Path::
stroke_open`, the same feathering as `Shape::line`), the legend/value/scale labels as laid-out
galleys. A frame without a new sample and with the same rectangle draws by cloning `Arc`s — no
vector, no string, no layout. The auto-scale steps in `push` (once per sample) instead of per draw:
it was frame-rate dependent (60 fps decayed four times faster than the 250 ms idle tick) and it made
the picture a function of the frame count, which a cache cannot key. Rejected: keep the per-frame
rebuild because "the window is closed most of the time" — during playback with the window open it
is 12 vectors and ~40 strings at 60 Hz, exactly the draw-path allocation invariant 12 forbids.

### D6. Repaint is unchanged

The window adds no `request_repaint`: while idle the program keeps its 250 ms tick
(`repaint_delay(false)`), the sampler runs on the frames that come, and the graphs move during
playback, which repaints anyway. t1148 holds this with the window open.

## Risks / Trade-offs

- [The README screenshot of the Diagnostics window changes shape] → `readme_shots.py` opens it by
  `tb.diag` and crops by `win.diag`, as before; the shot is regenerated by its own script.
- [The tessellated mesh looks slightly different from `Shape::line`] → the same `PathStroke` and
  feathering (`1 / pixels_per_point`) as egui's own tessellator; the live scenario's screenshot
  is looked at.
- [A parallel executor (windows-fill) edits `widgets::window_body`] → the Diagnostics window does
  not use it (its body is the graphs and its own log scroll), so the two changes do not meet.
