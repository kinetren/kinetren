# Tasks

## 1. Preferences in the store, the Preferences window, View trimmed

- [x] 1.1 `user_store.rs`: `StartChoice` gains `economy_ui` and `lang` (read/write in `start.json`, setters
      `set_economy_ui`, `set_lang`); `app.rs`: the `ui_economy`/`lang_choice` fields go, readers use the store;
      `run` reads the store before resolving the language (KT_LANG → store → former eframe `lang` → OS);
      `save` stops writing `lang` — verify t1295 and t987
- [x] 1.2 new `prefs_window.rs` (`kt_prefs`, Tool, sections Start / Interface / Privacy and updates / FFmpeg,
      Close), File › Preferences… in `toolbar.rs`, View keeps Detach/Dock, Diagnostics…, Notices; registry
      `WINDOWS` + opener + action label — verify t757, t779, t1125, t1127, t1293, t1296
- [x] 1.3 catalogs ×5: `[prefs]` section, `menu.file.preferences`, moved keys removed from `[menu.view]` and
      `[welcome]`; the statistics entry, update hint and FFmpeg notice name File / Preferences… — verify t100,
      t125, i18n tests, t840, t876
- [x] 1.4 rewrite the guards that walked View (t840, t850 path, t878, t1146/diag tests, t986's switch) with
      their reasons; t1294 (one writer per preference, View and welcome hold none) — verify those tests

## 2. The start window

- [x] 2.1 `guide::whats_new_from` (the running section down to the oldest) and the update line — verify t1291
- [x] 2.2 `welcome.rs`: sized window `.size([1180,720],[820,460])` bounded by the screen, three panes each in
      its own `window_body` (Start: primary, templates with frame shape, tour tile, five lessons with progress;
      Recent: growing poster grid; What's new), action row Guide / Preferences… / Close; no checkbox —
      verify t986, t1290, t1292, t918, t1125, t1127
- [ ] 2.3 live scenario `src/scripts/live/welcome.py`: the start window at default and enlarged, File ›
      Preferences…; `update.py`, `diagnostics.py`, `ffmpeg_download.py` probes moved to `prefs.*` — run them
      with `KT_LIVE_DISPLAY=:285`, screenshots in `target/live-out`

## 3. Docs and records

- [x] 3.1 `docs/guide/getting-started.md` (Language, the welcome window), `docs/guide/shortcuts.md`, kt-app
      `CLAUDE.md` ("start, Guide and tour"), What's new lines (`## kt26.11`), `docs/user-feedback.md` "Taken as",
      registry rows K1/K13 closed — verify `python3 src/scripts/ktprompts-index.py --check`,
      `python3 src/scripts/specs-user-path.py --check`

## 4. Close

- [ ] 4.1 `git merge master`, `cargo test --workspace --no-fail-fast`, `cargo clippy --all-targets` (no new warnings)
- [ ] 4.2 `src/scripts/finish.sh` — version recorded here and in the report
- [ ] 4.3 archive: specs merged, the "why" appended to `openspec/specs/ui-reach/design.md`
