# Design

## Context

See proposal.md, Why. The state of the code:

- `src/apps/kt-app/src/theme.rs` `apply(ctx)` builds `egui::Visuals::dark()` with our colours and
  calls `ctx.set_visuals(v)`. In egui 0.36 that is `style_mut_of(self.theme(), …)`: only the
  CURRENT theme's style. eframe/egui-winit feed `system_theme` from winit (`egui-winit`
  `lib.rs` 188, 451); with the default `ThemePreference::System` egui switches to the light style
  when the system says light — egui's default light style, untouched by us.
- `KT_DECORATIONS` is read directly in four places: the window builder (`app.rs` `run`,
  `with_decorations`), the window buttons (`toolbar.rs` `draw_window_buttons`), the menu row's
  no-resize zone (`app.rs` ~1692) and the resize edges (`app.rs` ~2099).
- Reproduced in the `windows` image (wine 11, Xvfb, openbox; the dist `kinetren.exe`):
  default — a system frame over our window, our 22 px row hidden, the status bar cut, the
  preview's settings strip and "Save the playback journal" white; `KT_DECORATIONS=1` — the
  system bar plus our row with the menus, the status bar whole; wine's
  `HKCU\Software\Wine\X11 Driver\Decorated=N` — exactly the Linux look.

## Goals / Non-Goals

**Goals:** the dark look on every OS regardless of the system theme; a usable window under wine
with no wine configuration; real Windows unchanged (borderless).

**Non-Goals:** writing wine's registry for the user (the owner chose "real Windows borderless,
wine somehow"; a write into someone's wine prefix is a persistent change nobody asked for); a light
theme; any change of the look on Linux or real Windows.

## Decisions

### D1. Fix the theme preference to Dark and style the dark theme
`theme::apply` calls `ctx.set_theme(egui::ThemePreference::Dark)` and then sets our visuals with
`ctx.style_mut_of(egui::Theme::Dark, …)`. With the preference fixed, egui ignores `system_theme`,
so a later system-theme event cannot switch styles. Alternative: setting our visuals into BOTH
styles (light and dark). Rejected: the theme would still flip on a system event, and anything
egui derives from the theme (e.g. `ctx.theme()` checks in widgets) would answer "light" for a dark
screen. No fallback is needed for the light theme: the program has one look (R.6).

### D2. One frame decision, computed once at start
A small module `window_frame.rs`: a pure `decide(var: Option<&str>, under_wine: bool) -> Frame`
(`Frame::Borderless` / `Frame::System`) and `current()` that reads `KT_DECORATIONS` and the wine
check once (`OnceLock`). The four readers of `KT_DECORATIONS` call `window_frame::current()`.
`KT_DECORATIONS` keeps its meaning (`1` = system frame) and gains `0` = borderless even under wine.
Why at start and not per frame: the window is built with it, and switching decorations on the fly
is not wanted.

### D3. Wine detection: `wine_get_version` in `ntdll`
On Windows: `GetModuleHandleW("ntdll.dll")` + `GetProcAddress("wine_get_version")` — the standard
check (wine exports it, Windows does not). No process spawn, no registry read. On other OSes:
`false`. Alternative: checking `WINEPREFIX`/`WINELOADER` variables — not reliable (wine does not
always set them for the program).

### D4. Under wine: the system frame, not a registry write
The owner: real Windows borderless with no Windows bar, under wine "somehow works". The system
frame needs nothing from the user and nothing is written; the menus and the project name stay in
our row below the bar; our window buttons are hidden (the system ones do the job). Rejected: the
app writing `AppDefaults\kinetren.exe\X11 Driver\Decorated=N` (a persistent change in someone's
wine prefix), and "document only" (the default wine run would stay broken). The README-windows
note tells wine users how to get the Linux look (`Decorated=N` + `KT_DECORATIONS=0`).

### D5. The wine check is scripted, real Windows is manual
`src/scripts/wine-shot.sh`: builds the `windows` image if needed, starts Xvfb + openbox in the
container (installing them in the throwaway container, as done for the reproduction), runs the
given `kinetren.exe` with `WINEDLLOVERRIDES="mscoree,mshtml="` (no Mono/Gecko prompt), waits and
saves a screenshot to `target/wine-out/`. The agent looks at the screenshot (our row visible below
the system bar, no white parts) — a picture, not a pixel assertion: fonts and openbox's theme are
not ours to pin. Real Windows has no runner with a desktop session here; its borderless look,
resize and maximize stay a manual check (with windows-build's open 6.4).

## Risks / Trade-offs

- [A Linux user who liked egui following a light system theme] → there was never a light look:
  the panels were always dark, a light system theme only produced the broken mix.
- [The system frame under wine loses our window buttons] → the system ones replace them; the
  owner asked only that wine works.
- [Some wine setups already set `Decorated=N`] → they get a system frame anyway unless they set
  `KT_DECORATIONS=0`; the README note says so. Detecting the registry value would mean reading
  wine's registry — not worth it for this case.
- [winit's borderless window on real Windows has its own quirks (maximize over the taskbar, snap)]
  → not changed here; the manual check covers it.
