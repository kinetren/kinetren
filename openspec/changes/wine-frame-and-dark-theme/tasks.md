# Tasks

## 0. Records before code

- [ ] 0.1 Append the owner's words of 2026-10-26 (wine: the interface dark with white parts; the title row gone, replaced by the system bar, so the menus are gone) and his priority ("real Windows: borderless with no Windows bar; wine: somehow works") to `docs/user-feedback.md`, with the reproduction (wine 11 + Xvfb + openbox: default / `KT_DECORATIONS=1` / `Decorated=N`); verify the entry names `wine-frame-and-dark-theme`

## 1. The dark theme

- [ ] 1.1 `theme::apply`: `ctx.set_theme(egui::ThemePreference::Dark)`, our visuals via `style_mut_of(egui::Theme::Dark, …)`; verify t862 (`layout_tests.rs`: a light `system_theme` in the harness input for several frames, then dark — the theme stays Dark, `panel_fill`/`window_fill` are `theme::PANEL`/`theme::WINDOW`, the inactive widget fill is ours) and that the existing layout/font/reach tests stay green
- [ ] 1.2 Commit stage 1

## 2. One frame decision

- [ ] 2.1 `src/apps/kt-app/src/window_frame.rs`: pure `decide(KT_DECORATIONS value, under_wine)`, `under_wine()` (`ntdll` `wine_get_version` under `cfg(windows)`, `false` elsewhere; `windows-sys` feature `Win32_System_LibraryLoader`), `current()` computed once; verify t863's pure part (the four cases of the spec)
- [ ] 2.2 The window builder, `draw_window_buttons`, the menu row's no-resize zone and the resize edges ask `window_frame::current()` instead of `KT_DECORATIONS`; a test hook to set the decision in the harness; verify t863's harness part (system frame → no window-button rectangle, borderless → present), t101/t254-family layout tests and `grep -rn KT_DECORATIONS src/apps/kt-app/src` showing only `window_frame.rs`
- [ ] 2.3 `src/packaging/README-windows.txt`: the wine note (the system frame under wine; `Decorated=N` + `KT_DECORATIONS=0` for the Linux look); verify `dist-check.py --check`
- [ ] 2.4 Commit stage 2

## 3. The wine check and finish

- [ ] 3.1 `src/scripts/wine-shot.sh` (design D5): `make windows` output or a given exe, Xvfb + openbox + wine in the container, `WINEDLLOVERRIDES="mscoree,mshtml="`, screenshot to `target/wine-out/<name>.png`; run it on the new zip's `kinetren.exe` with the defaults and with `KT_DECORATIONS=0` + `Decorated=N`; verify by looking at the screenshots (the system bar with our row below it, no white strips; the second — the Linux look) and record them here
- [ ] 3.2 `cargo test --workspace` and `cargo clippy --all-targets` green; the Windows cross-check `cargo check --target x86_64-pc-windows-gnu`; `src/scripts/finish.sh`; version in the report
- [ ] 3.3 Manual check on real Windows (owner): no Windows title bar, our row with the window buttons, resize from the edges, maximize without covering the taskbar; record here (with windows-build's open 6.4)
