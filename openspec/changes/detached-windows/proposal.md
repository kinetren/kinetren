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

**Q1. Which windows detach?** Your words: "окна fx и прочие окна".
- (a) **Every tool window of the registry (effects, morpher, transition, camera, track, Diagnostics, Guide,
  settings); dialogs and progress windows stay inside** (*recommended*: a dialog asks and closes; detaching it
  helps nobody).
- (b) Every window, dialogs included.

**Q2. On Wayland a program cannot place its windows.** The system decides where a new window appears.
- (a) **Keep the detached state and size; the position is the system's** (*recommended*).
- (b) Offer detaching only on X11 and Windows.
