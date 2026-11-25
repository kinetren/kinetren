# Design

## Context

`Style` numbers are stored, "frozen, append only" (t1702). The tile order is `Style::ALL`, not the number.

## Decisions

### D1. `Style::Vegas = 5` is appended; `Own` keeps 0 and everything else
`ALL = [Own, Vegas, Premiere, Resolve, FinalCut, AfterEffects]`. A person who has `Own` keeps `Own`: no migration,
nothing moves under their hands. `product()` of `Own` is "Kinetren"; the window omits "like …" where the product is
ours (one branch on `Own`, not on the text). The trademark line names VEGAS as it names the others.

### D2. The VEGAS table, from F-HLP-028's table and VEGAS's help
Wheel: plain Zoom, Ctrl Tracks, Shift Time, Alt Zoom (D5 of control-layouts: Alt zooms in every style). Keys over
`OWN`: J/K/L shuttle, S split, M marker, R region from selection, T trim start/end are `[`/`]`, G group as ours. A
key VEGAS has and we cannot give (Ctrl+drag stretch is ours already; crossfade commands) is in `missing.vegas`, said
on the tile. Panels: Classic.

Rejected: changing `Own` towards VEGAS. He asked for two tiles; each key of `Own` was set at his request.

### D3. The code, version 2
Version 1: `[ver, style, layout, wheel, (cmd, chords)…, crc]`. Version 2 puts three bytes after the wheel: the language
(index of the frozen list `auto, ru, en, es, zh, ar`; 255 = not chosen), flags (bit 0 economy UI, bit 1 update check;
bits 2–3 "is stated"), and a reserved 0. A v1 code opens into `Choice` with the three `None`: entering it changes none.
A v2 code in an older build answers `Newer`, as it does today. The applier calls the same setters as Preferences
(`set_economy_ui`, the language choice, the update check), each written once.

`Choice` gains `prefs: Option<CodePrefs>`; `to_code` writes it from the store when the window asks for a code, so a
code given in the window always carries them; `start.json`'s own string stays without (it keeps them in their rows).

### D4. The scale's keys
`Cmd::ScaleUp/ScaleDown/ScaleReset` are appended. In a style that binds none of the three, they are egui's own Ctrl+=,
Ctrl+−, Ctrl+0 and the list shows them as such; in one that does, they are Ctrl+Alt+=, −, 0 and `handle_keys` sets the
zoom factor by egui's own steps (0.1, 0.2…5.0 clamp). The existing rule "the style turns the option off and takes all
three" (t1717) keeps its first half. Walked (`controls.rs`): Resolve and Final Cut bind Ctrl+=;
Premiere and After Effects bind plain =, so they keep egui's keys. The same table drives all.

### D5. Class walk
Every place that names "Vegas" of ours: `product()`, `missing.own`, the welcome window's tile, the guide's page
"Layout & Controls", the README. The hint of every chord comes from `Controls::label`, so nothing else prints a key.

## Risks / Trade-offs

- [A person types a v2 code on an older build] → it answers "made by a newer Kinetren", as designed.
