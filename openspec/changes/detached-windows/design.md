# Design: detached-windows

> DESIGN ONLY, revision 1 (2026-11-20), base `ccae9d10`. Q1–Q2 in `proposal.md`; defaults recommended.

> Re-read 2026-11-25 against `fc60a5d9`: 28 windows in the registry (14 `Tool`, 12 `Dialog` with the two modals, 2 `Progress`); "detachable" is a flag of the registry row, not of `Kind` (project settings is a
> `Dialog` and the render's progress a `Progress`, both worth a second monitor). Q1 a/b/c change that flag's rows only. `ctx.show_viewport_immediate` is used by the preview (`app.rs:2068`); the
> windows are drawn after the panels in one list (`draw_fx_editors` …), so a detached body keeps its place in it.

## Context

- `window_shell::Shell` (`src/apps/kt-app/src/window_shell.rs`) is the only way to a window (t921); its `Kind`
  (`Tool`, `Dialog`, `Progress`) decides Esc, the cross, scroll and width; sizes per kind are remembered;
  `escape_action` closes the window on top by the previous frame's layer order.
- The detached preview: `ctx.show_viewport_immediate(ViewportId::from_hash_of("kt-preview-window"), …)` with its
  own close/Esc handling (`app.rs`).
- egui 0.36 viewports: an IMMEDIATE viewport runs its closure inside the parent's frame and can borrow the app;
  a DEFERRED viewport has its own repaint but its closure must be `'static + Send + Sync` (no borrow of
  `KtApp`). The effect windows borrow `&self.model`, so the immediate form is the one that fits.
- The rest rules (kt-app CLAUDE.md): no frame at rest; `wake::now()`; work counters.

## D1. The detached form of a shell window

`Shell::detachable(true)` for `Kind::Tool` windows (Q1 a). State `Detached { id: ViewportId, outer: Option<Rect> }`
per window kind in the app settings store. When detached, `Shell::show` runs `ctx.show_viewport_immediate(id,
ViewportBuilder::default().with_decorations(false).with_title(title).with_inner_size(size)
[.with_position(pos)], |child_ctx, _| …)` and draws, in a `CentralPanel` of the child, the same title row
(title, the dock button, the cross) and the same body the inner window draws — one body function, two hosts.
The title row's drag sends `StartDrag` to the child viewport, followed by the mandatory pointer release
(`release_pointer_after_wm_grab`); the edges resize by `BeginResize` with the same rule. Esc in the child:
the shell's `Closed` (the same action as the cross); `close_requested` from the OS → `Closed`.

## D2. Repaint per surface

An immediate viewport is laid out when the parent frame runs; it must not be PAINTED with every playing frame.
The shell keeps per detached window a stamp of what its body shows (the model revision it read, its own input
counter, a wake addressed to it); when the stamp is unchanged and the child has no input, the shell skips the
child's body and asks no repaint of its viewport (`ctx.request_repaint_of(id)` only on change). Acceptance is
measured (t3903): playing with a detached effects window open over a static chain, the child surface
presents no frames while the main window keeps its V-fps; a parameter drag in the child repaints only the
child and the preview.

## D3. Docking and the registry

The dock button and dragging the child's title over the main window's rect (on release) dock it: the state
returns to inside, the inner window opens at its remembered inner place. The `WINDOWS` registry gains a column
"detachable" (true for tools); the guards t779/t921/t1125 run for both hosts (a detached window's body fills
it, its buttons are visible, it has no window-level scroll).

## D4. Placement

On X11 and Windows the outer position and size are read from the child's viewport info when it moves/resizes
and stored; on Wayland (`WAYLAND_DISPLAY` and no position given back) only the size and the detached flag
(Q2 a). A stored position outside every monitor is dropped (the window opens at the system's place).

## D5. Class walk — "a second surface that costs while nothing in it moves"

| Surface | Today | Here |
|---|---|---|
| detached preview | immediate viewport, repaints with playback (it shows the playing picture — correct) | unchanged |
| detached tool windows | — | D2 gate |
| egui tooltips/popups in a child | drawn in the child | the same gate (they come with input) |
| the main window behind a detached window | its own rest rules | unchanged |

## Rejected

- **Deferred viewports** — the bodies borrow the model and the app; rewriting every window as `'static` state is
  far larger than the gate.
- **OS decorations** — "с полностью тем же стилем самого окна".
