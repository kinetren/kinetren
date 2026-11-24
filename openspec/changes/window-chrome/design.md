# Design: window-chrome

> DESIGN ONLY, revision 2 (2026-11-22): D4 dropped by the owner's B9. Revision 1 on `e56a543c`.

## Context

- `app.rs`: the "ambient-overlay" glow is a `layer_painter(LayerId::new(Order::Foreground, …))` drawn at the end of
  the frame — above every `Window` (egui's windows are `Order::Middle`). The meters' bars draw in their panel
  (`Order::Background`), their halo partly in the overlay: through a window one sees the halo, not the body (W1).
  The status hint is `Order::Foreground` too ("status-hint"); the toolbar's title text is painted by the toolbar
  row.
- `toolbar.rs::window_drag_from`: a drag on the strip's empty space restores a maximized window and starts the
  drag the next frame (`pending_unmax_drag`). It asks `viewport().maximized`, which winit reports `false` for a
  window the WM maximized at start, so the drag starts at full size (T5); the restored size is the size the window
  had when it was first mapped — the maximized one — so the "restore" only shrinks by the frame.
- Toolbar overflow: groups that do not fit move into a "more" menu (windows-fill-their-area).
- `fx_editor.rs`: per-chain `CollapsingHeader` (egui indents its body and draws the indent line: T8); the title
  `k::fx::TITLE` / `TITLE_GROUP` with the clip number (W3); text buttons Copy/Paste the chain (`node_editor.rs`) and
  the envelope (`fx_editor.rs`) (T7).
- The detached preview (`show_viewport_immediate`), the drag of the main window from the toolbar strip; masks,
  trackers and the R.64 frame take drags on the picture.

## D1. The layer order: nothing of the main window over a tool window

- One layer `"under-windows"` in `Order::Middle`, created before any window each frame and never brought to the top
  (`Context::move_to_bottom` of its `LayerId` once a frame), so it lies above the panels and below every window.
- The glow: per glowing thing (the preview, the meters column) ONE mesh — the halo and the body's glow in the same
  vertices — drawn in this layer. The meters' bars stay in their panel; their glow is no longer split between the
  panel and an overlay (the owner's "на одном объекте").
- The status hint and the toolbar's title text are painted in this layer too (they belong to the main window).
- A guard (t3935) scans kt-app for `Order::Foreground` and `Order::Tooltip` and compares with a list of allowed
  uses with reasons: tooltips and the hint popup, drag ghosts of the timeline and the bin, the lesson outline,
  combo/menu popups, the preview's on-picture handles in the detached viewport. A new use without a row fails.

## D2. A maximized start (T5)

- At start, `window_frame` reads the WM's state (X11 `_NET_WM_STATE_MAXIMIZED_*`, Windows `IsZoomed`; Wayland: the
  configure state winit passes) and sets `KtApp.maximized_at_start`.
- `window_drag_from`: maximized = `viewport().maximized` OR (`maximized_at_start` and the window's outer rect equals
  the monitor's work area within 2 px). Restoring a window with no recorded normal size sends `InnerSize(80 % of
  the work area)` and `OuterPosition(centred under the pointer)` before `Maximized(false)`, so the drag starts at
  the normal size; the normal size is then remembered (eframe's window state).

## D3. The toolbar's wheel (U10)

Over the toolbar strip, a wheel delta (y, or x with Shift) moves a horizontal offset of the row by the delta,
clamped to [0, need − avail]; the groups are laid out from the offset, the clipped edges fade over 12 px; the "more"
menu holds what is still outside. The offset resets when the window grows past `need`. No scroll bar (rule 13).

## D4. (dropped)

Dragging a window by the preview picture is dropped (the owner, B9: "пропускаем это требование."). The detached
preview's title row is the shell's (change `detached-windows`).

## D5. Folds and buttons (T8, T7)

- The per-chain fold draws its body unindented (`CollapsingHeader::show_unindented`): no indent, no line.
- Copy/Paste the chain and the envelope become icon buttons (Phosphor `copy`, `clipboard`), each with its existing
  hint through `KtApp::hint`; their catalog names become the accessible names. Where `presets-everywhere`'s head
  menu lands, these buttons go into it.

## D6. Titles (W3)

`event_title(clip)`: a file's base name; a generator's kind name, a title's first 24 characters of text; a reel's
name; then " — " and the window's kind (Effects, Transition, Generator). A pair: "<video name> + sound". The event
number ("#2149") is drawn in the chain head row, right-aligned, weak text, as part of the row (no new row, rule 13);
its accessible name stays. Long names are elided by `widgets::elided_name` (rule 7).

## D7. Class walk — "the main window's drawing over a tool window" and "a title that names an id"

| Place | Order | Over windows today? | Here |
|---|---|---|---|
| preview glow | Foreground overlay | yes | D1 |
| meters glow | Foreground overlay (halo) + panel (body) | halo yes | D1, one mesh |
| status hint | Foreground | yes (the bottom strip) | D1 |
| toolbar title text | toolbar row | yes (T9) | D1 |
| timeline drag ghosts, insert marks | Foreground areas (`timeline.rs`) | during a drag only — correct | listed in the guard |
| lesson outline | Foreground | intended | listed |
| hint popup / tooltips | Tooltip | intended | listed |
| event window titles | `TITLE` with `#N` | — | D6 |
| transition window title | kind name | names the kind, fine | D6 adds the pair's names |
| track window title | the track's label | fine | — |
| morpher window title | the layer number | an id | D6: the layer's name |

## D8. Idle

Nothing new runs: the same meshes in another layer; the wheel offset changes only on a wheel event.

## Rejected

- **Hiding the glow while a window overlaps the preview** — a jump in the look; under-windows is the right place.
- **A horizontal scroll bar on the toolbar** — space (rule 13); the wheel and the "more" menu cover it.
