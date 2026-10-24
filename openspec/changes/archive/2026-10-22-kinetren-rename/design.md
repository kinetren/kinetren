# Design

## Context

See proposal.md. The name appears in four kinds of places, and each one breaks
differently if renamed carelessly:

1. **Display strings** (title, menus, errors, i18n catalogs, docs). These are safe to
   rename.
2. **Identifiers other programs remember**: the X11/Wayland app id, the `.desktop` file,
   the MIME type and icon names. A desktop that knew the old ones keeps stale entries
   unless `install.sh` removes them.
3. **Per-user state on disk** (`$XDG_DATA_HOME/kinotrain`): recovery copies, journal,
   locks, presets, workspace. eframe's own persistence is keyed by the app id. If we
   simply change the name, this is silently lost.
4. **The project file marker** (`kinotrain` at the root of every `.kt`). This is a format
   change.

## Goals / Non-Goals

**Goals:** a program that never says Kinotrain; no user data lost; old projects open
through one documented door.

**Non-Goals:**
- Renaming crates and binaries (`kt-*`, `kt-app`, `kt-cli`): `kt` already reads as
  Kinetren.
- Renaming the repository directory.
- Rewriting history chronicles: the porting log (frozen), owner quotes, archived
  changes.
- The OTIO metadata namespace `kt`, which stays as it is.

## Decisions

### D1. The reader is not taught the former marker
Owner's rule (R.67): old forms are repaired in the import, the reader rejects them with
the door's name. The marker is the very first thing a reader checks, so the rule applies
cleanly. `FileKind::FormerName` joins `Otio` and `LegacyKtproj` in `looks_like`; the
import door (`kt_otio::import_older` path for `.kt`) renames the root key and calls the
normal reader.

Rejected: accepting both keys in the reader. It is friendlier for one release, but it
teaches the reader an old form forever — exactly what R.67 forbids — and a later
"remove the alias" would break files silently.

### D2. Data directory: move once, never merge
`journal::data_dir()` is the single resolver (the app's recovery path and `kt-cli` locks
use it or its formula). On first call it checks for `kinetren/`; if that is missing and
`kinotrain/` exists, it does `rename` (atomic on the same filesystem). If that fails, it
does a recursive copy and leaves the old directory. If both exist, the new one wins and
nothing is merged: a merge could resurrect deleted presets or stale locks. eframe
persistence follows the app id. We pass the new id; its storage lives under the same
data dir, so the move carries it too. This is verified at implementation; if eframe uses
another directory, the move covers that path as well.

### D3. Desktop integration
The app id, `.desktop`, MIME type (`application/x-kinetren`, glob `*.kt`) and icon names
are renamed. `install.sh` deletes the former files it installed earlier and refreshes
the desktop and MIME databases.

### D4. Strings
The display name lives in `en.toml` and the four translations; "Kinetren" is not
translated (catalog rule 4, like FFmpeg). Error strings in `kt-model`/`kt-otio`/`kt-cli`
are English literals and are renamed in place.

## Risks / Trade-offs

- [Every existing `.kt` needs one import] → the error text names the door, and the
  import result is identical (t828). The owner chose this option knowing the cost.
- [The data move happens while another (old) build runs] → `rename` of a directory with
  an open lock file keeps the descriptors valid; the old build keeps writing into the
  moved directory. Acceptable for a one-time event.
- [Missed occurrences] → a guard test greps the program sources and catalogs for
  `Kinotrain`/`kinotrain` outside an allow-list (the former-marker constant, the import
  repair and its tests) — t833.

## Migration Plan

The first start of the new build moves the data directory. Projects stay untouched until
the user imports them. Rollback: the previous build still reads files with the
`kinotrain` marker; files written by the new build would need their key renamed back by
hand.
