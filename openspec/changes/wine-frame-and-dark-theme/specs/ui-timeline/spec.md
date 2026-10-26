# Spec Delta

## ADDED Requirements

### Requirement: A fixed dark theme and one decision about the window frame

The interface SHALL be drawn with our dark visuals whatever theme the system reports: the theme
preference is Dark, and our visuals are those of the dark style. A light system theme (wine
reports one; some Linux desktops do) MUST NOT bring egui's default light style into any part of
the interface.

The window frame SHALL be one decision, asked by everything that depends on it (the window
builder, the window buttons, the resize edges, the no-resize zone of the menus):
- borderless — our own title row with the logo, the menus and the window buttons — by default,
  on Linux and on real Windows;
- the system frame when the program runs under wine, or when `KT_DECORATIONS=1`;
- borderless when `KT_DECORATIONS=0`, also under wine.

With the system frame our window buttons are not drawn and the resize edges are the system's;
the logo and the menus stay in our row below the system bar.

Why: `set_visuals` changes only the current theme's style, so a light system theme switched
egui to its default light style and painted the standard parts white over our dark panels
(the owner, under wine, 2026-10-26). wine lets the window manager decorate every window, a
borderless one too; the frame covered the top 22 px of our window — the menus and the window
buttons — and cut the status bar. The owner's priority: real Windows stays borderless with no
Windows title bar; under wine it must simply work.

#### Scenario: A light system theme does not reach the interface
- **WHEN** the harness reports a light system theme to the program for several frames, and then a dark one
- **THEN** the context's theme stays Dark throughout, the panel and window fills are ours (`theme::PANEL`, `theme::WINDOW`) and the inactive widget fill is our dark one — the same values as with no system theme reported
- **PATH** there is no gesture: the system switches to a light theme while the program is open → the preview's settings strip, the buttons and the windows stay dark, nothing turns white
- **TEST** t862 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The frame decision per environment
- **WHEN** the decision is computed for: no variable, not under wine; no variable, under wine; `KT_DECORATIONS=1`, not under wine; `KT_DECORATIONS=0`, under wine
- **THEN** it is borderless, system frame, system frame, borderless; with the system frame the window buttons are not drawn (the harness finds no window-button rectangle), with borderless they are
- **PATH** there is no gesture: the program starts under wine → a system title bar, below it our row with the logo and the menus, no window buttons of ours; started with `KT_DECORATIONS=0` → our row with the window buttons at the top, no system bar
- **TEST** t863 (`src/apps/kt-app/src/window_frame.rs`, the harness part in `layout_tests.rs`)

#### Scenario: Under wine and on real Windows
- **WHEN** the Windows build runs under wine (default settings, a window manager) and on real Windows 10/11
- **THEN** under wine: a system title bar, below it our row with the logo and File/Edit/View, the status bar whole; on real Windows: no Windows title bar, our row at the top with the window buttons, resize from the edges and maximize work
- **PATH** wine: start `kinetren.exe` → the system title bar, below it the logo and File / Edit / View, the menus open on a click → no white strips anywhere; real Windows: start `kinetren.exe` → our row at the top with minimise/maximise/close on the right, no Windows bar → drag the right edge → the window resizes → double-click the row → maximised without covering the taskbar
- **TEST** the wine part — a scripted run in the build container (`src/scripts/wine-shot.sh`, screenshot checked by the agent and kept in `target/wine-out`); the real Windows part — a manual check recorded in `tasks.md`
