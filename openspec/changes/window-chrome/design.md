# Design: window-chrome

> Revision 3 (2026-11-24): BUILT on orch `5986f08e`; what differs from the plan is said in each section. D4 dropped by
> the owner's B9.

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

- One layer, `under_windows()`: an egui `Area` of `Order::Background`, shown empty at the start of each frame. The panels
  are the root area of the same order, registered first, so the layer is painted and hit over every panel and under every
  window. Not `Order::Middle` (the plan): egui 0.36 paints a layer it does not know as an area LAST in its order — over
  every window — and has no `move_to_bottom`; an area there would also stand in every list of windows.
- Painted in it: the glow of the preview and of the meters, the project's title with its clickable crumbs, the resting
  status line.
- A LIVE status line (hover, a refusal, a message) stays `Foreground`: it answers the pointer or an action, which may be
  inside the very window that covers the strip; under it, the one place for hints (rule 10) would be silent there.
- The glow is NOT one mesh per thing (the plan): the meters' inner glow lies under their bars and the halo over the
  neighbouring panels; one mesh cannot be on both sides of the bars. His sentence asks why the halo shows and the body
  does not; now neither does.
- The guard is the closed list of `no_teaching_layer_over_the_ui` (t988): each row with its reason, and a row no source
  uses fails too. No separate t3935.

## D2. A maximized start (T5)

The mechanism (reproduced live under openbox): eframe saves at exit the state AND the size a maximized window has, the
screen; at the next start winit sends the maximized state before the window is mapped, and the window manager drops it
(X11). The window comes up the size of the screen and NOT maximized: a drag moves it at full size (his "в том же
размере"); maximized for real and dragged, it goes back to its "normal" size — the screen — and only the frame shrinks.
- `window_frame::note_normal` keeps the last size that was not maximized in egui's saved memory (written when it changes);
- `window_frame::adopt_start`, once, on the third frame: a window over the whole monitor that is not maximized gets its
  normal size (or 80 % of the monitor), centred, and then the maximized state, so the window manager knows both;
- `restore_window` sends the normal size when the window has not been normal in this run, and a drag places the window
  under the pointer;
- the bottom toolbar's strip had a drag of its own that sent only `StartDrag`: a maximized window moved at full size.
  It takes `window_drag_from` now. The restore button and the double click take the same `restore_window`.

## D3. The toolbar's wheel (U10)

Over the toolbar strip, a wheel delta (y, or x with Shift) moves a horizontal offset of the row by the delta,
clamped to [0, need − avail]; the transport and the timecode (the groups that never leave the row) stand, the groups
after them slide under, the clipped edges fade over 12 px; the "more" menu holds what is not wholly in view. The offset resets when the window grows past `need`. No scroll bar (rule 13).

## D4. (dropped)

Dragging a window by the preview picture is dropped (the owner, B9: "пропускаем это требование."). The detached
preview's title row is the shell's (change `detached-windows`).

## D5. Folds and buttons (T8, T7)

- The per-chain fold draws its body unindented (`CollapsingHeader::show_unindented`): no indent, no line.
- Copy/Paste the chain and the envelope become icon buttons (Phosphor `copy`, `clipboard`), each with its existing
  hint through `KtApp::hint`; their catalog names become the accessible names. Where `presets-everywhere`'s head
  menu lands, these buttons go into it.

## D6. Titles (W3)

`KtApp::event_name(clip)`: a title's first 24 characters of text (a counter's kind name: it does not draw its text); a
generator's kind name; a reel's name; a file's base name (40 characters). Effects: "<name> — Effects", a linked pair
"<name> + sound — Effects", a larger group "<name> and its linked tracks — Effects". A transition: "<a> / <b> —
Transition", at an edge "<name> — Edge transition" (it was "Transition 12", an id, against the plan's table). A morpher:
"Morpher M1, 00:00:03:00" — it has no name of its own: the row and the start are what the timeline shows. The number
("#2149") is weak and small: in the chain's fold head after "Video"/"Audio", and in the action row of the transition and
morpher windows.

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
| transition window title | `TITLE` with an id | — | D6: the events it joins |
| track window title | the track's label | fine | — |
| morpher window title | `TITLE` with an id | — | D6: the row and the start |
| track window, camera window | the track's label, the camera's name | fine | — |
| drag of a maximized window | header strip, top strip, window buttons, bottom toolbar | the toolbar's own drag did not restore | D2 |

## D8. Idle

Nothing new runs: the same meshes in another layer; the wheel offset changes only on a wheel event.

## Rejected

- **Hiding the glow while a window overlaps the preview** — a jump in the look; under-windows is the right place.
- **A horizontal scroll bar on the toolbar** — space (rule 13); the wheel and the "more" menu cover it.
