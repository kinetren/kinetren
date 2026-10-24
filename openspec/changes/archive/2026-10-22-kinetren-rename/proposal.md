# Proposal

## Why

The product is published as **Kinetren** (github.com/kinetren/kinetren, LICENSE holder
Kinetren). The owner chose a **full rename** on 2026-10-21 ("kinetren, not kinotren";
answer: full rename, including the `.kt` root marker and internal names). The program
still calls itself Kinotrain in the window title, menus, the desktop entry, the MIME type,
icon file names, the per-user data directory, the release tarball and the project-file
marker. A half-renamed program is worse than either name.

## What Changes

- **Program name.** The window title, app id, `.desktop` entry, MIME type
  (`application/x-kinetren`), icon files, the release tarball and `install.sh` use
  Kinetren. Every UI string in the five catalogs that names the program says Kinetren.
  The menu item becomes "Import older Kinetren project…". Binary and crate names
  (`kt-app`, `kt-cli`, `kt-*`) stay: `kt` already stands for Kinetren.
- **BREAKING — project file marker.** New `.kt` files carry the root key `kinetren`.
  A `.kt` file with the former key `kinotrain` is refused by **Open**, with a message that
  names the former name and the import menu item. It opens through the import door, which
  renames the key (the owner's rule: old forms are repaired in the import, never taught to
  the reader). `kt-cli import` does the same.
- **Per-user data.** The data directory moves from `$XDG_DATA_HOME/kinotrain` to
  `$XDG_DATA_HOME/kinetren`, together with recovery, journal, project locks, presets and
  workspace. On the first start the old directory is moved over once, so nothing the user
  had is lost. `kt-cli` and the app agree on the lock directory.
- `install.sh` removes the old `kinotrain` desktop entry, icons and MIME file when it
  installs the new ones.
- The landing page (`site/`) and the live development documents (root `CLAUDE.md`, crate
  notes, current spec texts that quote UI strings) use the new name. Historical
  chronicles (owner quotes in `docs/user-feedback.md`, archived
  changes) are left as written.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `project-format`: the root marker is `kinetren`; a file with the former marker is
  refused by the reader and repaired by the import door.
- `ui-reach`: the import menu item is named "Import older Kinetren project…", and it also
  accepts `.kt` files with the former marker.
- `durability`: the per-user data directory is `kinetren`, and the former directory is
  migrated on first start.

## Impact

- `src/libs/kt-model/src/project_file.rs`: marker, error kinds.
- `src/libs/kt-otio` (import door).
- `src/apps/kt-app`: title, app id, data dir, file dialogs, i18n catalogs.
- `src/apps/kt-cli`: import, lock path, messages.
- `src/packaging/*`, `src/scripts/release-build.sh`, `site/`, docs.
- Every existing project must be imported once. Their files are not modified.
- Next free test number is t828.
