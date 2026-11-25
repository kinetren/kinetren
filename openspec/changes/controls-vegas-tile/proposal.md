# Proposal: controls-vegas-tile

> DESIGN ONLY (spec session, 2026-11-25). Found by the registry audit: three answers of the owner of 2026-11-07
> (`docs/user-feedback.md`, "Round of answers", 1–3) no change covered. Tests reserved: t4256–t4258.

## Why

His answers, whole: "в.1. делаем для нас отдельную плитку, а для vegas - тоже отдельную под них"; "в.2. а" (the
settings code also carries the language, the economy UI and the update check); "в.3. а" (Resolve and Final Cut:
the interface scale gets other keys of its own). Read in the tree: `controls.rs` `Style` has five members, `Own`
answers `product() = "Vegas"`; `CODE_VERSION = 1` carries style, panels, wheel and keys only; a style that binds
Ctrl+= turns `zoom_with_keyboard` off and leaves the scale with no key (t1717 states it).

## What Changes

- Six tiles: ours named "Kinetren" with no "like …" line, its keys, panels and wheel untouched (t1700); a new
  VEGAS tile that behaves as VEGAS does: the plain wheel zooms, J/K/L shuttle, S splits, M marks.
- The settings code version 2 appends the language, the economy UI and the update check. A version-1 code opens and
  leaves those three as they are.
- Where a style takes Ctrl+=, Ctrl+− or Ctrl+0, the interface scale has Ctrl+Alt+=, Ctrl+Alt+−, Ctrl+Alt+0, listed
  with the other keys and said on the tile.

## Capabilities

### Modified Capabilities

- `ui-controls`: the tiles, the code, the scale's keys.

## Impact

- `kt-app` `controls.rs` (the only file of the tables), `app.rs` `handle_keys` (the scale), the Layout & Controls
  window, catalogs ×5, `docs/guide` shortcuts. No model, no project format, no OTIO.
