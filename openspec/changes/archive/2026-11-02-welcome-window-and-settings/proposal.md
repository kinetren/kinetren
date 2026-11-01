# Proposal: welcome-window-and-settings

## Why

The owner, kkt-26.11.01 items 1 and 13 (registry rows K1, K13): "Стартовое окно уродское. Агенты не
работали. А также — оно еще и не ресайзится и в нем whats new суженный, и при скроле whatsnew
скролится всё подряд" and "нафига мне галочка welcome window at start И в самом window и еще
продублирована в View?!?! Да и пора бы формировать раздел настроек в file, а то у нас в view уже срач
из разного." His answer 3 (2026-11-01): yes — a settings window in File, the start checkbox leaves View.

Today the welcome window is one scroll over two columns (What's new squeezed into a 220 px column, so
scrolling the notes scrolls the templates and the recent cards with them), and the View menu mixes
what is viewed (detach the preview, Diagnostics, Notices) with the program's preferences (language,
economy UI, telemetry, the update check, FFmpeg, the start switch — the last one also in the window).
Found while reading the code: File → New/Open rebuilds the app and drops the language choice and the
economy switch (`replace_with` does not carry them), so the next start forgets the chosen language.

## What Changes

- The welcome window becomes a designed start window: a sized, resizable window (shared fill rule)
  with three panes side by side — Start (the primary tile, templates with their frame shape, the five
  lessons with progress), Recent (poster cards that wrap and grow with the window), What's new (the
  Guide's renderer over `docs/guide/whats-new.md`, from the running version down) — each pane with its
  own scroll; an action row with the Guide, Preferences… and Close. It fits the screen.
- **BREAKING (UI path)**: a new window **File › Preferences…** (`kt_prefs`, in the `WINDOWS` registry)
  holds the program's preferences: the welcome window at start, Language, Economy UI, anonymous usage
  statistics, the daily update check, FFmpeg (state and the FFmpeg… window). They leave View; View keeps
  Detach/Dock preview, Diagnostics…, Notices. The welcome window loses its own checkbox: each switch is
  in ONE place. The texts that name the old path ("View / …" in the telemetry entry, the update hint,
  the FFmpeg notice) name the new one in five languages.
- The language choice and Economy UI are kept in the user store (with the start record) and survive
  File → New/Open and a restart; the former eframe `lang` value is read once as a fallback.
- Guards: one switch, one place (each preference is written from one site; neither View nor the
  welcome window holds a preference); the welcome panes scroll apart; the start window fits the screen
  at its default and grows.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ui-reach`: the welcome window requirement (layout, panes, separate scrolls, no checkbox of its own);
  the FFmpeg window and UI-language requirements (their path is Preferences now); new requirements for
  the Preferences window, "one switch, one place" and preferences surviving a project open.
- `ui-timeline`: the performance-panel requirement names Economy UI in Preferences, not View.
- `telemetry`: the switch lives in Preferences; the entry names that path.
- `distribution`: the update-check switch lives in Preferences next to telemetry; the notice names that path.

## Impact

- `src/apps/kt-app/src/welcome.rs` (the window), new `prefs_window.rs`, `toolbar.rs` (File/View menus),
  `user_store.rs` (start record: `economy_ui`, `lang`), `app.rs` (fields → store, `run`, `save`),
  `guide.rs` (What's new from a version down), i18n ×5, `reach_tests.rs` (registry, openers, t986, t840,
  t878, t850 paths), `diag_tests.rs` (t1146), a new `welcome_prefs_tests.rs` (t1290–t1299).
- Live: `src/scripts/live/welcome.py` (start window at default and large size, Preferences), the
  `view.*` probes of `update.py`, `diagnostics.py`, `ffmpeg_download.py`.
- Docs: `docs/guide/getting-started.md`, `docs/guide/shortcuts.md`, What's new, the kt-app `CLAUDE.md`.
