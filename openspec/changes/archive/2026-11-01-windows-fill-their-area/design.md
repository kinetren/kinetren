# Design: windows-fill-their-area

## Context

Every window is built by `window_shell::Shell` (shared-window-shell, t921), and most bodies go through
`widgets::window_body` (R.66/R.67 form contract, t779). That makes the defect fixable in ONE place:
the shell decides resizability, the body decides how much of the window it takes. What made the
needle's eye:

- `window_body` — `ScrollArea::vertical().max_height(min(0.62 × screen, window − ACTIONS_H))
  .auto_shrink([true, true])`: the body is as small as its content and never taller than 0.62 of the
  screen, whatever size the window has;
- `Shell::new` is `resizable(false)` unless the site calls `.size()`/`.resizable()` — 12 sites did not;
- fixed numbers in window code (Guide 260 / 0.55× screen, search 320, render log 160, picker
  520/450/160, media picker 260).

egui facts the design relies on (egui 0.36.1, `containers/resize.rs`):

- a resizable window's content rect is its `desired_size`; content larger than it raises it
  (`desired = max(desired, last_content)`), smaller content leaves blank room — so a body that
  FILLS (`auto_shrink(false)`, height = available) has a stable size equal to the window (no ratchet:
  content = desired), and a body that shrinks leaves the empty frame the owner sees;
- the resize state is `insert_persisted` under `<window id>.with("resize")`, and eframe persists egui
  memory (`persistence` feature, `app.ron`) — a single window already keeps its size across runs;
- at the start of a corner drag egui runs one invisible sizing pass at `min_size.x` to learn the
  content's minimum width.

## Goals / Non-Goals

Goals: every window resizable; the body fills; sizes remembered; the Guide's tables and index fixed;
the picker one growing gallery; the toolbar never clipped; a guard over all windows so the class does
not come back.

Non-goals: re-laying out window forms (R.66 decided which six are laid out); the welcome window's
layout (the next change, `welcome-window-and-settings`, builds on this rule — here it only inherits
the filling body); the Diagnostics window's content (the `diagnostics-three-graphs` executor owns it —
it inherits the shell rule only); popups other than the two pickers.

## Decisions

### D1. Resizable is the shell's default; a size is mandatory

`Shell::new` starts `resizable = true`. A site gives `.size(default, min)`; a site that gives none
gets the kind's default: `Tool` 520×420 / 360×240, `Dialog` and `Progress` a width with height 0 (the
window opens at its content's height — egui raises `desired` to the content — and the person can
still pull it taller or wider). `.resizable()` goes (it is now the default). `egui::Modal` (the
recovery offer, the question on close) cannot be resized by egui; both are questions that block
until answered, and they are listed in t1125 with that reason.

Rejected: a `max_size` per window (the owner asked for the opposite); keeping non-resizable dialogs
(the owner: "and so everywhere"; a name field that grows with its dialog costs nothing).

### D2. One body rule: `window_body` fills, `window_fill` for windows without actions

`window_body(ui, body)` — `ScrollArea::vertical().auto_shrink([false, false])
.max_height(available − ACTIONS_H).min_scrolled_height(that)` — the scroll spans the window down to
the action row and its full width. `window_fill(ui, body)` is the same without the action row (the
notice log, search results, the Guide's two lists). Both record the body's outer rect per layer
(`widgets::fill_probe`, a `Copy` value in egui temp memory, no per-frame allocation) — that is what
t1125 reads; kittest's accessibility tree cannot see a scroll area.

The 0.62× screen cap goes: it existed so that an AUTO-SIZED window kept its buttons on screen (R.65);
with D1 every window has a size bounded by the screen (egui clamps `default_size` to the screen and
constrains windows to it), and the body takes `window − actions`, so the buttons stay inside — t779
keeps proving that in a 420-point-high harness.

### D3. Per-family size memory

Windows opened per object have per-object ids (`("fx", clip)`, the morpher's, the transition's, the
camera's, the track's), so egui's persisted resize state only remembers one object's window. The
shell gets `.remember("fx")`: on show, a remembered `Vec2` for the family (persisted egui data,
`("kt_win_size", family)`) replaces the site's default size (egui uses `default_size` only when the
window has no state yet); the body records `ui.max_rect().size()` (= the resize state's
`desired_size`, which egui keeps private) back under the family when it differs. Remembering is egui
memory, so it follows the eframe persistence the program already has: no new store, no format.

Rejected: `user_store` (a workspace preference file for a value egui already persists — two truths
about one size); remembering the outer frame rect (default_size is the CONTENT size; the frame adds the
title bar and margins).

### D4. Guide tables: measured column widths, painted grid

`guide::table(ui, rows, avail_w)`: for each cell lay out its text unwrapped (natural width) and find
its longest word (minimum width). Column minimum = max of its cells' minimum words (+ padding);
column natural = max of cell naturals. If the naturals fit, use them, and spread the rest
proportionally; otherwise each column gets its minimum plus a share of the remaining width in
proportion to `natural − minimum` (columns with long text take the room, the keys column keeps its
caps unwrapped). Then the rows are laid with galleys wrapped to their column, the row height is the
tallest cell, and the painter draws the header fill, stripes and 1-px rules. The column count is the
widest row's (the old `num_columns(2)` lost columns of wider tables). Clicks on links inside cells
keep working (the cell galley gets a click sense like a paragraph).

Rejected: `egui_extras::TableBuilder` (a new dependency for one widget, and its column widths are
still declared, not measured).

### D5. Index cards

Each entry is a `Frame` with a 1-px outline and a small fill; inside, the "what" (strong) and on its
own line the "where" (weak, `→` from Phosphor kept as a leading arrow), both wrapping. The frame's rect
gets `Sense::click` (the whole card is the target — rule 1), hover lightens the fill. A 6-point gap
between cards. The headings found by the search are the same card shape with one line. The left side
is `clamp(0.3 × width, 240, 420)`; both scrolls are `window_fill`.

### D6. The picker gallery

The popup is a `MenuButton` popup (R.54 `CloseOnClickOutside` stays). Its width = the HOST window's
body width (the calling scroll's viewport, `ui.clip_rect()`, minus the popup's own margins), never
under `GALLERY_W`; its height = the screen below the popup's top minus a margin, stated with
`min_scrolled_height` (egui bounds a scroll by the popup area's own room). One `ScrollArea` holds:
the head, the presets as a wrapped row of cells (a cell with its caption under it, the same
`preview_cell`), then per category one heading and the effects as CARDS — the name row and its
variant strip, `EFFECT_CARD_W` = three cells wide — that flow, top-aligned, into as many columns as
the popup is wide. A card is allocated with its width up front: a wrapped row wraps a child only by
the size it asks for, and a bare `vertical` ran on past the edge and widened the popup to the
screen. The search field takes the popup's width. The width depends only on the host window, so
typing does not move it (the R.54 rule). The audio picker (no preview) flows its names the same way
(`EFFECT_NAME_W`). The media picker list takes the same screen-bounded height
(`widgets::popup_list_height`).

### D7. Toolbar overflow

The row is split into groups (transport, timecode+speed, tracks, edit, zoom, region, undo/redo,
output). Each group's width is measured when drawn in the row and kept in `KtApp` (`tb_group_w`, a
fixed-size array — no allocation). Next frame: the groups are drawn from the left while
`x + group_w + more_w ≤ right`; the rest are drawn inside a `⋯` `menu_button` at the row's end, by the
same group function (one body — rule 17). The first two groups never move. The first frame has no
widths yet and draws all groups (clipped), which measures them. A group in the menu keeps its last
measured width, so the decision is stable (no flicker: widths do not change with the menu).

## Risks / Trade-offs

- **A filling body in a resizable window could ratchet** (t918/t923 history). The body equals the
  available size exactly, so `last_content = desired`; t918 runs every window 300 frames at five
  scales and t923 the owner's morpher scene. Both stay as they are.
- **Default sizes now visible**: a tool window whose content is short opens with blank room below
  the content, inside the scroll. Acceptable — the person sizes it once and it is remembered (D3).
- **Coordinates in older tests**: harness tests that click inside windows by layout read rects from
  probes/accessibility, not constants; the fill only adds room at the bottom.
- **The diagnostics executor edits `kt_diag`**: this change touches only its shell line, if at all.

## Found while applying

- **egui sizes a `Window` by its content** (`Resize::end`: a window's frame is `last_content_size`;
  `desired_size` is only the room offered). A resizable window whose content does not fill cannot
  be made taller at all — the paste-prefix and preset-name dialogs grew in width only (t1125
  found it). So the filling body is not a look, it is what makes a window resizable; the short
  dialogs got it too (the blank room sits above their buttons).
- **A `ScrollArea`'s `inner_rect` can be taller than what it took** (its `min_scrolled_size`
  default is 64): the body's bottom was measured below the action row and the tail read zero. The
  body rect is the space the scroll allocated (`ui.min_rect().bottom()` after it).
- **A scroll is bounded by its parent's current room.** A fresh window starts at its minimum and a
  popup area at egui's own guess, so a fit body and the picker gallery could not grow past them
  (the gallery stopped at 400 points); `min_scrolled_height` states the height they may take.
- **Fit and fill must agree to the point**, or the window moves by itself right after opening
  (t918): the body subtracts the action row height MEASURED last frame (not a constant), the fit is
  not padded to the window's minimum, a fitted window's minimum height stays under its content, and
  an empty body has no floor. egui's sizing pass runs a window twice in its first frame, so the fit
  is counted in frames, not passes.
- **Two windows of one family open at once** (creating a generator opens its effects window) wrote
  the family size every frame, each its own: a window writes the family only when ITS size
  changed, and never in its first two frames (the sizing pass).
- **A resizable window's edge catches the pointer a few points outside its frame**: the message log,
  auto-placed in the top-right corner, covered the program's own window buttons (t129) — it opens
  below the title row.
- **The render window's finished actions stood where Cancel was** (t957, trick 3). The old window
  grew at the finish and moved the row down, which hid it; a window that keeps its size shows it.
  The finished actions stand at the far right.

## Migration Plan

None: no format, no setting. Sizes remembered by egui before this change keep working; family
memory starts empty.

## Open Questions

None — the scope was discussed with the owner (kkt-26.11.01 answers, 2026-11-01).
