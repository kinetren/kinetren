# Proposal

## Why

The owner runs the Windows build under wine and sees two defects (2026-10-26): the interface is
dark with some parts white, and our title row — the logo, File/Edit/View and the window buttons —
is gone, covered by a system title bar. Both were reproduced in the build container (wine 11 +
Xvfb + openbox, screenshots in the change's tasks):

- **White parts**: `theme::apply` sets our dark visuals with `ctx.set_visuals`, which in egui 0.36
  changes only the style of the CURRENT theme. Nothing fixes the theme preference, so egui follows
  the system theme. wine reports a light one; egui then draws with its default LIGHT style.
  Our panels paint fixed dark colours, so the screen is mostly dark, while standard egui parts
  (the preview's settings strip, buttons, windows, menus) come out white. A Linux desktop that
  reports a light theme would show the same.
- **The title row**: the window is borderless (`with_decorations(false)`, our own title row since
  R.6). By default wine lets the window manager decorate every window, a borderless one too. The
  frame covers the top 22 px of our window, exactly where the logo/menu row and the window
  buttons are, and the bottom of the status bar is cut off. With wine's `Decorated=N` the same
  build looks exactly as on Linux; with the system frame requested (`KT_DECORATIONS=1`) the menus
  are visible below the system bar.

The owner's priorities: on **real Windows** the window stays borderless with no Windows title
bar; under **wine** it must simply work.

## What Changes

- **The theme is fixed to dark** on every OS: the theme preference is set to Dark and our visuals
  are applied to the dark style, so a light system theme no longer brings egui's light style in.
  The look itself does not change (owner's R.6 decisions).
- **The window frame is one decision** (`window_frame`): borderless by default; the system frame
  when the program runs under wine (detected by `ntdll`'s `wine_get_version`) or when
  `KT_DECORATIONS=1`; `KT_DECORATIONS=0` forces borderless (for wine users who set
  `Decorated=N`). Everything that reads `KT_DECORATIONS` today — the window builder, the window
  buttons, the resize edges, the menu's no-resize zone — asks this one decision instead. With the
  system frame our window buttons are hidden and the resize edges are the system's, as with
  `KT_DECORATIONS=1` today.
- **Real Windows stays borderless**: nothing changes there; its look (no Windows title bar, our
  row at the top, resize from the edges, maximize) is a manual check on real Windows, the same
  open item as windows-build's 6.4.
- `README-windows.txt` gets a short wine note: the system frame is used under wine; with
  `Decorated=N` and `KT_DECORATIONS=0` the borderless look of Linux comes back.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `ui-timeline`: a new requirement — the theme is fixed to dark regardless of the system theme,
  and the window frame is one decision per platform (borderless; the system frame under wine or
  on request).

## Impact

- Code: `src/apps/kt-app/src/theme.rs` (theme preference, dark style), `src/apps/kt-app/src/app.rs`
  (window builder, resize edges and the menu zone ask `window_frame`), `src/apps/kt-app/src/
  toolbar.rs` (window buttons), a small `src/apps/kt-app/src/window_frame.rs` (the decision and the
  wine detection); `windows-sys` features `Win32_System_LibraryLoader` in kt-app.
- Packaging: `src/packaging/README-windows.txt` (the wine note).
- Tests: two new — the theme stays dark under a light system theme (harness), the frame decision
  (pure); the existing layout/reach tests keep passing (they run borderless).
- No engine, model or format change. Not a feature (a defect of the Windows build), so no
  `docs/features/` note.
