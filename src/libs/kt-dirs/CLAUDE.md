# kt-dirs — per-user directories

Capability: `durability` (requirement "Per-user data lives in `kinetren`").

- One resolver for kt-app AND kt-cli: the data directory (recovery, journal, crash report,
  locks, presets, workspace, eframe's `app.ron`) and the local directory (large regenerable
  downloads — the FFmpeg build on Windows).
- Linux: `$XDG_DATA_HOME/kinetren` (a RELATIVE `XDG_DATA_HOME` is ignored, as eframe does),
  else `~/.local/share/kinetren`. Windows: `%APPDATA%\kinetren\data` (Roaming — the exact
  directory eframe uses for its storage) and `%LOCALAPPDATA%\kinetren` for downloads.
- NEVER the working directory: before windows-build the fallback was `"."`, so on Windows
  (no `HOME`) the data went wherever the process was started — `System32` from Explorer —
  and the migration of the former name renamed any former-name folder found there. Without a
  per-user root the answer is the system temp directory with `fallback == true`, and the
  caller says so once.
- The migration of the former name is NOT here: it lives in kt-app (`journal.rs`), Linux
  only, and runs only on a resolved (non-fallback) root.
- `resolve` is pure over `Env`, so the Windows rules are tested on Linux (t851).
