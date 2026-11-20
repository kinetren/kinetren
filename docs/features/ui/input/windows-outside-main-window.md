---
id: F-UI-012
title: Tool windows outside the main window, in the program's own style
status: candidate
priority: B
cost: M
category: ui/input
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [13, 22]
round: null
point: null
tests: []
depends_on: []
checked: 2026-11-20
---

# F-UI-012 — Tool windows outside the main window, in the program's own style

## Essence

An effects window, the morpher window or the track window is pulled off the main window onto the second
monitor or beside it, keeps working at the same speed, looks like the program (its own title bar, the same
theme), and comes back with one gesture. Next launch it opens where it was left.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | floating/docked windows | every window (Video FX, Trimmer, Preview) can float as its own OS window or dock; layouts remember positions [NOT VERIFIED live] |
| Adobe Premiere | 26 | Undock Panel / Floating panels | any panel undocks into a floating window with the app's chrome; workspaces remember them [NOT VERIFIED live] |
| DaVinci Resolve | 21 | dual-screen layout | a second monitor layout; few panels float freely [NOT VERIFIED live] |
| Final Cut Pro | 12 | second display for viewer/browser | panels move to a second display as a whole area, not freely [NOT VERIFIED live] |
| After Effects | 26 | Undock Panel | every panel undocks into a floating window [NOT VERIFIED live] |

## The trick behind it

**1. A floating window is a second OS surface, and its cost is its repaint, not its existence.** An undocked
panel is free only if it repaints when ITS content changes; a toolkit that repaints every child window with the
main one doubles the cost of every playing frame on a software rasterizer.

**2. "The same style" means the program draws the title bar.** With the OS's decorations a floating panel looks
foreign; the editors draw their own chrome and ask the window manager only for the move and resize.

## Pros

- A second monitor holds the effects and the morpher while the main window keeps the timeline and preview.

## Cons

- Wayland gives no global positions: a floating window cannot be put back exactly where it was.
- Each extra surface is a presentation per frame when it repaints.

## What users say

The owner, `docs/ktprompts/v26.10.txt` line 65: "окна fx и прочие окна не должны ограничиваться основным окном
приложения, хочется их выдвигать и за пределы основного окна, без потери производительности, и с полностью тем же
стилем самого окна."

NO QUOTES: no web access in this spec session (2026-11-20). The search would be the editors' forums and r/premiere, r/davinciresolve, r/VEGASPro for the feature by its name in the table; the editor rows are [NOT VERIFIED] for the same reason, and the next wave verifies them live.

## How it fits Kinotrain

### What already exists

- The detached preview: `ctx.show_viewport_immediate` with `ViewportId::from_hash_of("kt-preview-window")`
  (`src/apps/kt-app/src/app.rs`), Esc returns it.
- The window shell (`src/apps/kt-app/src/window_shell.rs`): every window is made by it; its kind decides Esc, the
  cross and the size; sizes are remembered per kind.
- The main window's own title bar (StartDrag + `release_pointer_after_wm_grab`, kt-app CLAUDE.md).

### What is missing

- A detach for registry windows; their own chrome as an OS window; remembered placement.

### What hinders

- Immediate viewports are drawn in the parent's frame: a detached window must not repaint with every playing
  frame unless its content moves (invariant 12; a measured acceptance).

### Which invariants it touches

- **12** — repaint only on change, per surface. UI rules 13 and 22.
- CAPABILITY: openspec/specs/ui-reach/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | kt-app `window_shell.rs` (detach as a shell property), the app settings store |
| Project format | none (placement is the machine's) |
| Tests | detach/dock round trip; Esc and cross; repaint counts per surface while playing |
| Depends on | nothing |
| Breaks compatibility | no |

## Verdict

Take it as a property of the window shell: any tool window of the registry can be detached into an OS window
drawn with the program's own title bar and theme, docked back by a button or Esc, its placement remembered in
the machine's settings, and a detached window repaints only when its content changes.

## Sources

- `docs/ktprompts/v26.10.txt` line 65 (W2); registry `v26.10-index.md` W2.
