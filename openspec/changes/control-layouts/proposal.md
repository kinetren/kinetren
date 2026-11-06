## Why

The owner, `docs/ktprompts/v26.10.txt` lines 74–75 (L1, L2): at the first entry or in the settings, offer a "Layout & Controls style" like the best-known commercial programs, our tile first with the caption "like Vegas", trademarks credited to their owners; a customize button that ends in a "save code" entered after a reinstall, like an old game's password.

Today the keys are literals in `handle_keys`, their labels are separate literals in the menus and the hints, and the panels are one fixed order. A person from Premiere finds nothing where their hands go.

## What Changes

- A closed table of 36 keyboard commands (`controls.rs`): every key of `handle_keys` moves there, with the same meaning; menus and hints read it, so a label cannot disagree with its key.
- Five styles: ours ("like Vegas", byte for byte today's behaviour), Premiere Pro, DaVinci Resolve, Final Cut Pro, After Effects. Each brings keys, what the wheel does, and where the panels stand.
- Four panel arrangements; the three new ones add a header row, Classic is untouched.
- New commands the styles need: J/K/L shuttle, keyboard zoom, zoom to fit, clear region, selection tool, save as, new, open, render. Unbound in our style.
- A window `kt_controls`, opened once at the first entry and from File › Preferences…: tiles, the trademark line, the code, Customize (panels, wheel, every key with search, conflicts asked, Reset).
- A settings code: Crockford Base32, version, CRC-16, frozen numbers; `start.json` keeps the same string.

## Capabilities

### New Capabilities

- `ui-controls`: the command table, the styles, the arrangements, the wheel, the code, the window.

### Modified Capabilities

None: `ui-reach` and `ui-timeline` keep their requirements; their guards extend to the new window by the existing registry.

## Impact

`kt-app` only: `controls.rs` (new), `prefs_window.rs`, `app.rs` (`handle_keys`, the panels of `ui`), `toolbar.rs`, `timeline.rs` (the wheel), `user_store.rs`, `welcome.rs`, five catalogs, the font subsets, `docs/guide/shortcuts.md`. No model, format or engine change; OTIO is not touched.
