# Layout & Controls styles — why it is this way

## Context

L1 and L2 ask for three things at once: a choice at the first entry, styles that move keys and panels, and a typed code. Reconnaissance: `docs/features/ui/input/control-style-at-first-entry.md` (F-UI-007), on top of F-UI-002 and F-UI-003, which say the command list must come first and that a preset in a fixed panel grid has a ceiling. Owner's decisions that bind this work: the default keys and panels (rounds 6, 8, 16, R.72, each commented in place), the Space machine, the jump zones, the bin column limit, one switch in one place (t1294), no window without a registry row (t757).

## Goals / Non-Goals

**Goals:** a person picks a style by name and finds its keys, wheel and panels; any key can be changed; a code brings it all back after a reinstall; the default does not move by a pixel or a key.

**Non-Goals:** ripple editing, a blade tool, a source monitor, docking panels by hand, importing other programs' keymap files, gesture modifiers (Ctrl releases the magnet in every style), the language or other preferences in the code.

## Decisions

### D1. One table, read by the keys, the menus and the hints

`handle_keys` asks `Controls::pressed`, which returns a bit set; the actions below it stay in the same order. Menus call `label(cmd)`, hints `hint_key`. The hint text is built only while hovered: an allocation per frame on 15 buttons would break invariant 12. Rejected: a second keymap beside the literals, the fourth source of truth F-UI-002 warned about.

### D2. The old handler's laxness is kept, by rule rather than by rows

`S` used to split with Shift held, `Home` jumped with Ctrl held. A default that does not change must keep that. `lookup` tries the exact chord, then drops an unbound Shift (not on arrows, which were `is_none()`), then drops any unbound modifier on Delete, Backspace, Home, End. Not kept: Ctrl+Alt+S doing save AND solo in one press, a defect on Windows, where AltGr is Ctrl+Alt.

### D3. Space stays one machine; its key is a one-key field

Tap, double tap, hold and Ctrl+hold (rounds 6, 16, R.72) are not chords. The style names the machine's key; the list refuses a modifier for it, and Ctrl+that key counts as held by it. All five styles use Space, so no style touches this behaviour. Esc and Enter are not commands: they answer in every window, and the capture itself needs Esc.

### D4. A conflict is asked, never taken

Flat table, one chord, one command: no panel scopes, because `handle_keys` has none. A taken chord shows who holds it in error style, with Take it / Leave it (rules 16, 20). FCP's policy; Resolve's "saved, but dead" is forbidden by rule 16.

### D5. The wheel is four cells, Ctrl+Shift+wheel and the pinch stay

Plain, Ctrl, Shift, Alt each map to time, tracks or zoom. Sources disagree on the rarer cells (F-UI-007): Alt zoom is confirmed in all four; Premiere's plain wheel along time, After Effects' plain wheel through layers and Shift along time are confirmed; Resolve's and Final Cut's plain wheel are from forum reports. These are the style's starting values and stay editable in Customize.

### D5a. Ctrl+= and egui's interface zoom

egui consumes Ctrl+=, Ctrl+-, Ctrl+0 before the frame while `zoom_with_keyboard` is on, and the owner works at zoom 1.1. A style that binds one of them turns that option off and takes all three; ours keeps it. A person in the Resolve or Final Cut style changes the interface scale only with View's items, if any: said in the report.

### D6. Four arrangements; the header row is theirs alone

The menu lives in the bin column and the window buttons in the meters column, both at the window's top in Classic. Any other arrangement has neither there, so it adds one 22 px row. Classic's panel order, ids and sizes do not change: rule 13 stands on the owner's default, and the three layouts pay the row. Final Cut and After Effects use Classic: their main window has the same shape. `TimelineOnTop` is offered in Customize only: real VEGAS sources conflict on where its timeline stands today, and the "like Vegas" tile is the owner's arrangement.

### D7. The window, and where the first entry lives

`kt_controls` is one window for the first entry and for Preferences. Rule 22's order: a section of Preferences cannot hold tiles and a 36-row list in 560 px, and a first entry should not open on the FFmpeg and privacy rows. Preferences keeps a row with the current style and a Choose button. Writers stay in `prefs_window.rs`, so t1294's one-place rule holds. Asked once, at the first start that has the welcome window; a project passed on the command line does not spend the question. An older `start.json` without the key counts as asked: that person's first entry is behind them.

### D8. The code

`[version][style][layout][wheel]`, then for each changed command `[command | count<<6]` and two bytes per chord, then CRC-16/CCITT; Crockford Base32 in groups of five. Every number comes from a frozen table of `controls.rs`, including the key table (egui's numbering moves). A factory style is 10 characters. A reader takes lower case, O for 0, I and L for 1, spaces, dashes. A larger version or an unknown number is "made by a newer Kinetren"; a wrong CRC is "a typo"; neither applies anything. Rejected: JSON in base64 (Factorio's family: pasted, never typed), a server key (needs a network).

`start.json` keeps the code itself: what the code does not carry cannot survive there either, so the two cannot drift. It also keeps the last code per style, so going back to a tile brings its changes back (rule 23), instead of asking.

### D9. J, K, L

`set_playback_rate` carries the signed speed (R.57). Same direction doubles, up to the Space hold's 4×; the other direction starts at 1×; K pauses and returns the person's base speed. A pause by any other door ends the run. The speed widget shows the temporary speed (item 120).

## Risks / Trade-offs

- [The presets are approximate: Adobe's help does not open from here] → each row's source is in F-UI-007, and each tile says what is missing.
- [The Guide and lessons name default keys] → `shortcuts.md` says which style it describes; the Customize list is the live reference. Lesson texts stay on Space, which no style moves.
- [`zoom_how` in the Guide index says Ctrl+wheel] → same note; found, not fixed.

## Open Questions

For the owner, in the report: whether "like Vegas" should move towards real VEGAS (plain wheel zoom, J/K/L); the order of the tiles after ours; whether the code should carry the language and the other preferences.
