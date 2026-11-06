---
id: F-UI-007
title: A control style chosen at the first entry, and settings as a typed code
status: done
priority: B
cost: L
category: ui/input
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: yes
crates: [kt-app]
invariants: []
ui_rules: [13, 14, 16, 17, 19, 20, 22, 23]
round: null
point: change control-layouts
tests: [t1700, t1701, t1702, t1703, t1704, t1705, t1706, t1707, t1708, t1709, t1710, t1711, t1712, t1713, t1714]
depends_on: [F-UI-002, F-UI-003]
checked: 2026-11-06
---

# F-UI-007 — A control style chosen at the first entry, and settings as a typed code

## Essence

The person meets one question before their first edit: which program do your hands know? The answer moves the keys, the wheel and the panels at once. Everything chosen and changed afterwards reduces to a short code, which they write down and type after a reinstall, as in an old console game's password. F-UI-002 covers the key editor and F-UI-003 the panel layouts; this note is about the two things neither of them answers: WHEN the question is asked, and HOW the answer leaves the machine without a file.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | no first-entry question | Keyboard maps are `.ini` files: drop one in the folder and it appears on the next start. No other editor's map ships. A plain wheel zooms, unlike the other four |
| Adobe Premiere | 26 | no first-entry question | The Keyboard Layout Preset menu has Avid and Final Cut 7 layouts; `.kys` files are copied by hand. **[NOT VERIFIED]**: helpx.adobe.com answers 403, read through search excerpts and secondary sites |
| DaVinci Resolve | 21 | Quick Setup, keyboard layout step | The sixth step of the first-launch wizard, default "DaVinci Resolve". The guide tells readers to keep the default, since its own shortcuts assume it. Presets for Premiere and Final Cut X are confirmed; the full list **[NOT VERIFIED]** |
| Final Cut Pro | 12 | no first-entry question | Command sets export to a file; no other editor's set ships |
| After Effects | 26 | no first-entry question | Text shortcut files; nothing shipped from other programs **[NOT VERIFIED]** |

**Outside the core.** The only program besides Resolve that asks at the first launch, and the one with a public record of what the choice costs.

| Program | Version | What it is called | How it is built |
|---|---|---|---|
| Blender | 5.2 | Quick Setup | Language, keymap (Blender or "Industry Compatible"), select mouse button, Space action, theme. The steps are read from the 2.80–3.6 manuals; the 5.x wording **[NOT VERIFIED]** |

## The trick behind it

**1. A style moves the wheel, not only the keys.** Alt+wheel zooms in Premiere, Resolve, Final Cut and After Effects; VEGAS alone zooms on the plain wheel. Every keymap discussion we read stops at keys, yet the wheel is the gesture a person makes a hundred times an hour, and a keymap preset leaves it behind. A style that changes `S` and not the wheel feels half-done.

**2. A wrong preset is worse than none, because it breaks trust in the list.** A Blackmagic forum user in May 2019 estimated that Resolve's Premiere preset gets over half its keys wrong, and a product manager asked for a better list. A preset is a promise. So what the program does not have (ripple trim, the blade tool) must be said on the tile, in words, instead of leaving the person to find a dead key.

**3. A style breaks the tutorials.** Polycount users say Blender's Industry Compatible keymap makes tutorials hard to follow, because tutorials name default keys. Resolve's guide tells its readers to keep the default for the same reason. So hints inside the program must print the live key, and the reference page must say which style it describes.

**4. No creative app has a typed settings code.** VEGAS, Resolve, Final Cut and Premiere all move settings by file. The codes found elsewhere fall into two families: a short key into a server's database (Super Mario Maker 2, Overwatch Workshop), or a long string that IS the data and can only be pasted (Factorio blueprints, WeakAuras, Path of Building). Only old console passwords carried the data at a typeable length (Metroid: 24 characters, an 8-bit sum), and they fit because their state was small. Ours fits for the same reason: a style is one byte, and only what the person changed adds to the length. The fields worth taking: an alphabet without look-alikes (Crockford Base32, Mario Maker), a version up front (Factorio, WeakAuras), a real check (Mario Maker's checksum field; Metroid's sum is weak).

## Pros

- The migrant works on the first day; the first entry is the one moment they have no habits in this program to break.
- A typed code survives everything a file does not: a new machine with no shared folder, a note on paper, a chat message.
- The code needs no server, no account and no network; `start.json` keeps the same string, so one format holds the setting at rest and in transit.

## Cons

- A question at the first entry delays the first edit. Resolve lets a person skip the whole wizard for this reason; Esc does the same here.
- A code grows with what was changed: about 14 extra characters for two changed commands. Past a few dozen changes it is pasted, not typed.
- Tutorials and our own Guide pages name default keys (see trick 3).

## What users say

The price of relearning, in a thread on which layout to learn:

> "It's a massive pain in the ass trying to force new muscle memory."
> — u/i_sell_you_lies, thread "Which Keyboard Layout should I learn?", about January 2026 (a Reddit mirror, accessed 2026-11-06), https://reddit.sentinel-team.org/posts/1u8o9q4/snapshots/2026-06-18T16%3A43%3A33.523868Z

NO QUOTES: no user comment for or against a first-launch keyboard question was found; search on Reddit mirrors, the Blackmagic forum and Polycount on 2026-11-06.

## How it fits Kinotrain

CAPABILITY: openspec/specs/ui-controls/ — the table of commands, the styles, the code, the window.
Keys, wheel and panels of every style live in `src/apps/kt-app/src/controls.rs`; the window is in `prefs_window.rs`.
The owner's default is the first tile, untouched; its caption "like Vegas" is his.

## Estimate

| What | Answer |
|---|---|
| Scope | L — a whole round: the command table, the panels, the window |
| Where the main work is | kt-app |
| Project format | does not touch |
| Tests that will have to be written | our style key for key against the old handler; a frozen code; each tile from its window; a key assigned twice; a live scenario with a shot per step |
| Depends on | F-UI-002, F-UI-003 |
| Breaks compatibility | no |

## Verdict

Taken (L1, L2 of `docs/ktprompts/v26.10.txt`). Five tiles, ours first; keys, wheel and panels per style; any key can be changed on top, with Esc and Enter reserved; a code in Crockford Base32 with a version and a CRC-16. Not taken: settings files, because the owner asked for a code; product logos, a tile is a plain schema of the panels; the language and other preferences in the code, left as a question to the owner.

## Sources

Accessed 2026-11-06.

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/13-appendix/shortcutkeys.htm — VEGAS 2026 shortcuts and wheel
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/2-window/customizing_keyboard_shortcuts.htm — VEGAS keyboard maps
- https://support.apple.com/guide/final-cut-pro/keyboard-shortcuts-ver90ba5929/mac — Final Cut Pro 12.4 shortcuts
- https://documents.blackmagicdesign.com/UserManuals/DaVinci-Resolve-20-Fusion-Visual-Effects.pdf — Resolve Quick Setup, pp. xii–xvi
- https://documents.blackmagicdesign.com/UserManuals/DaVinci-Resolve-15-Keyboard-Shortcuts.pdf — Resolve shortcuts (old; cross-checked)
- https://www.editorskeys.com/blogs/news/davinci-resolve-keyboard-shortcuts-pdf-cheat-sheet — Resolve 21 shortcuts, secondary
- https://defkey.com/adobe-premiere-pro-windows-shortcuts — Premiere shortcuts, secondary; helpx.adobe.com answers 403
- https://community.adobe.com/t5/premiere-pro-discussions/timeline-zooming-fully-in-or-out-when-using-alt-scroll/m-p/15015818 — Premiere Alt+wheel
- https://blog.fmctraining.com/blog/after-effects-keyboard-shortcuts-cheatsheet — After Effects shortcuts, secondary
- https://www.manualsdir.com/manuals/753844/adobe-after-effects.html?page=14 — After Effects wheel (CC 2014/2015 mirror)
- https://docs.blender.org/manual/en/3.6/getting_started/configuration/defaults.html — Blender Quick Setup (search excerpt)
- https://forum.blackmagicdesign.com/viewtopic.php?p=510627 — the Premiere preset's accuracy (search excerpt)
- https://polycount.com/discussion/comment/2722119 — tutorials and Industry Compatible
- https://nintendo-wiki.pretendo.network/docs/misc/datastore-codes — Super Mario Maker 2 codes
- https://www.ietf.org/archive/id/draft-crockford-davis-base32-for-humans-01.html — Crockford Base32 (Internet-Draft)
- https://minimaxir.com/2013/05/its-a-livin/ — Metroid password
- https://wiki.factorio.com/Blueprint_string_format — Factorio blueprint strings
- https://docs.rs/crate/weakauras-codec/latest/source/src/lib.rs — WeakAuras strings
