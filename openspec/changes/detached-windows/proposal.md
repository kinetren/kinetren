# Proposal: detached-windows

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` W2 (partial). Note F-UI-012. Tests reserved:
> t3901–t3903 (the block t3900–t4099).

## Why

The owner, `v26.10` line 65: "окна fx и прочие окна не должны ограничиваться основным окном приложения,
хочется их выдвигать и за пределы основного окна, без потери производительности, и с полностью тем же стилем
самого окна." Today only the preview detaches (`show_viewport_immediate`, `app.rs`); every registry window lives
inside the main window.

## What Changes

- **Detach is a property of the window shell**: a tool window of the registry shows a "detach" button in its
  title row; pressed, the window becomes an OS window of its own, drawn without the OS decorations with the
  shell's own title row (the same theme, cross, Esc behaviour and kinds as inside). The same button on the
  detached window docks it back; dragging its title onto the main window also docks it.
- **Placement is the machine's**: whether a window kind is detached, and its outer position and size, are kept
  in the app settings (not the project), so the effects window opens on the second monitor next launch. Where
  the platform gives no global positions (Wayland), only the size and the detached state are kept (Q2).
- **No loss of speed**: a detached window repaints only when its own content changes (its input, a model
  revision it shows, a wake for it) — never because the main window repaints for playback; dialogs and progress
  windows stay inside the main window.

## Capabilities

### Modified Capabilities

- `ui-reach`: detach/dock of tool windows, their chrome, Esc and the cross, remembered placement, the repaint rule.

## Impact

- kt-app: `window_shell.rs` (a detached form of `Shell::show` through a viewport id per window kind; the title
  row drawn by the shell; StartDrag + `release_pointer_after_wm_grab`), the settings store (`detached` and
  geometry per kind), the repaint gate per viewport.
- No model, engine or format change.
- Idle: a detached window at rest asks no frame (the rest rules per surface; the live `idle` scene adds a
  detached effects window).

## Questions to the owner

(Q66, Q67 of `docs/owner-checks.md`; re-read against `fc60a5d9`, 2026-11-25.)

**Your item whole** (`docs/ktprompts/v26.10.txt` line 65): "окна fx и прочие окна не должны ограничиваться основным окном приложения, хочется их выдвигать и за пределы основного окна, без потери
производительности, и с полностью тем же стилем самого окна."

**Now**: only the preview leaves the main window (View › Detach preview), with the system's frame. The other 28 windows live inside it and stop at its edge: 14 tools (effects, morpher layer, transition,
camera, track, program loudness, search, notices, diagnostics, guide, composition parameters, preferences, keys, FFmpeg), 12 dialogs (project settings, render settings, welcome, names, confirmations),
2 progress windows (render, analysis).

**Q66 (Q1). Which windows may stand outside?** "окна fx и прочие окна" can be read as "effects and the other editing windows" or as "all of them".
- (a) **The 14 tools, project settings and the render's progress; the questions stay inside** (*recommended*): you move effects, morpher, transitions, camera, the track, diagnostics and the running render to
  the second monitor. A question ("save?", a name) appears over the main window, as now.
- (b) All 28: a question may also open on another monitor, where it was last left; a "save before closing?" that is not seen stands in the way of closing.
- (c) The editing windows only (effects, morpher, transition, camera, track): five. Diagnostics and the guide stay inside.

**Q67 (Q2). On Wayland a program cannot place its own windows; the system decides.** On X11 and Windows the place is remembered to the pixel.
- (a) **Detach there too: the size and "outside" are remembered, the place is the system's each time** (*recommended*): on Wayland the effects window reopens outside, at a place of the system's choosing
  (usually the active monitor's centre); moved by hand each session.
- (b) No detaching on Wayland: the windows stay inside there.
Under XWayland (the program's X11 mode) the places are remembered as on X11 in both options.
