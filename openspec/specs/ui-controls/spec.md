# Layout & Controls styles

## Purpose

Which program the person's hands know: the table of keyboard commands that keys, menus and hints all read, the five styles with their keys, wheel and panels, the window that offers them at the first entry and from Preferences, and the settings code that carries the choice to another install.

## Requirements

### Requirement: Keys, menus and hints read one table, and our style is the old handler
Every key of `handle_keys` except Esc and Enter SHALL be a row of the command table in `controls.rs`. The menus and the hints that name a key SHALL print it from that table. The first tile's table SHALL give every press the meaning the old literals gave it, including the modifiers they ignored.
Why: the owner, L1: "Первой плиточкой идет наша"; his default was set key by key over the rounds. A label that is a second literal lies in any other style (rule 17).

#### Scenario: Our style keeps every key
- **WHEN** each chord of the old handler, its lax variants and the free keys are looked up in the default table
- **THEN** each gives the old command or none; the menu labels read as before; the default is Classic with the owner's wheel
- **PATH** start the program without choosing anything → the panels, the keys, the wheel and the menu labels are as they were
- **TEST** t1700 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: Every style is a clean table
- **WHEN** the five tables are read
- **THEN** no chord stands on two commands, every key can be carried by a code, Esc and Enter are nowhere, all five have Space on the transport, and the four other programs zoom on Alt
- **PATH** open each tile → the list has each key once; Space still plays, holds and reverses
- **TEST** t1701 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: No key is printed past the table
- **WHEN** the toolbar source and the key-naming hints of all five catalogs are read
- **THEN** no accelerator is a literal, each hint has a slot for the key
- **PATH** choose a style → hover the toolbar's buttons → each hint names that style's key
- **TEST** t1715 (`src/apps/kt-app/src/controls_tests.rs`)

### Requirement: A style moves the keys, the wheel and the panels at once
A tile SHALL change the keys, what the wheel does and where the panels stand, in the frame it is pressed, with no restart. Other programs' keys SHALL do the program's matching action; what that program has and we do not SHALL be named on the tile. All four arrangements SHALL keep every panel, the menu and the window buttons reachable; Classic SHALL be drawn as before.
Why: the owner, L1: "как в самых известных коммерческих программах". Reconnaissance F-UI-007: a keymap alone leaves the wheel behind, and a preset that lies is worse than none.

#### Scenario: The key and what prints it move together
- **WHEN** Premiere is chosen with an event under the playhead
- **THEN** S no longer splits, Ctrl+K does; the scissors' hint and the Edit menu's item say Ctrl+K
- **PATH** pick the Premiere tile → press S → nothing happens → press Ctrl+K → the event is split → hover the scissors → the status line says Ctrl+K → open Edit → the item says Ctrl+K
- **TEST** t1706 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: J, K and L run the transport
- **WHEN** a style has them: L, L, L, L, then J, then K; L, L and Space; separately in our style J and K
- **THEN** 1×, 2×, 4×, 4×; −1×; a pause with the base speed back; a pause by Space ends the run too; the speed widget shows the temporary speed; in our style the letters do nothing
- **PATH** pick the Premiere tile → press L → it plays → press L again → the speed strip shows 2× → press J → it plays backward → press K → it stops
- **TEST** t1707 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: The wheel follows the style
- **WHEN** the wheel turns over the tracks with Alt, Ctrl and no modifier, in our style, in Premiere's, and with a cell changed by hand
- **THEN** ours zooms on Ctrl only and scrolls bare; Premiere zooms on Alt, not Ctrl; the changed cell acts; the eased tail of Ctrl+wheel zooms only where Ctrl zooms
- **PATH** pick the Premiere tile → hold Alt and turn the wheel over the tracks → the timeline zooms around the mouse → hold Ctrl and turn it → it does not zoom
- **TEST** t1708 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: Each arrangement keeps every panel and the window controls
- **WHEN** each of the four arrangements is drawn at 1440×860
- **THEN** Classic has no header row and the old panel order; in the others the bin, the timeline and the meters stand where the arrangement says, and the menu and the close button stand at the window's top
- **PATH** pick the Premiere tile → the preview takes the top, the bin and the meters stand beside the timeline, a thin row carries the menu and the window buttons → press File → the menu opens
- **TEST** t1709 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: A style with Ctrl+= takes it from the interface zoom
- **WHEN** the Resolve style is chosen and Ctrl+= is pressed; our style is chosen again
- **THEN** the timeline zooms, the interface scale does not move; our style gives the interface zoom back
- **PATH** pick the Resolve tile → press Ctrl+= → the timeline zooms in, the buttons stay their size → pick our tile → Ctrl+= scales the interface as before
- **TEST** t1717 (`src/apps/kt-app/src/controls_tests.rs`)

### Requirement: The style window, ours first, the marks credited
The window `kt_controls` SHALL show five tiles with a schema of the panels and no product's mark, ours first with its name over "like Vegas". The trademarks SHALL be credited to their owners, the program SHALL say it is not connected with them. The window SHALL say where the choice changes later. Preferences SHALL have one row with a door to it. Writers of the choice SHALL stay in `prefs_window.rs`.
Why: the owner, L1: "Первой плиточкой идет наша и подпись "like Vegas". Про трейдмарки и права не забудь подписать что они принадлежат тем кому пренадлежат."

#### Scenario: Tiles, marks and the door
- **WHEN** the window opens; a tile is pressed; another is pressed; Preferences' row is pressed
- **THEN** ours is first with its caption; the marks line, the not-connected line and the where-to-change line are shown; a press changes the style and shows what it lacks; the window stays; Preferences opens it
- **PATH** File › Preferences… → the Layout & Controls style row → Choose… → five tiles, ours first, "like Vegas" under it, the trademarks under the tiles → press a tile → the panels move behind the window
- **TEST** t1710 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: File → New keeps the style
- **WHEN** a style is chosen and a new project is made
- **THEN** the style and its keys stay
- **PATH** pick a tile → File › New → the keys and the panels are the same
- **TEST** t1716 (`src/apps/kt-app/src/controls_tests.rs`)

### Requirement: The first entry asks once
At the first start that has the welcome window, the style question SHALL open before it, and the welcome window SHALL wait for its answer. A project passed by hand SHALL not spend the question. Esc SHALL keep the default. A record written by an older build SHALL count as asked.
Why: the owner, L1: "При первом входе в программу или в настройках - предлагать выбрать". The live run saw the welcome window drawn over the tiles, in a harness that was green.

#### Scenario: Asked once
- **WHEN** the first start with a project on the command line; the first start without one; Esc; the next start; a store of an older build
- **THEN** no question; the question, with the welcome window held back; the default stays; no question; no question
- **PATH** start the program for the first time → the style window stands alone, no welcome window over it → press Close → the welcome window opens → start again → no question
- **TEST** t1714 (`src/apps/kt-app/src/controls_tests.rs`)

### Requirement: Any key can be changed, a taken key is asked
Customize SHALL offer the panels, the four wheel cells and every command, searchable by name and by key. A press SHALL give the key to the command; a key that is taken SHALL name its holder in the row outside the scroll, with Take it and Leave it; nothing SHALL move until the answer. Esc and Enter SHALL be refused with the reason, and Esc SHALL only let go of the capture. Play SHALL take one bare key. While listening, the program's keys SHALL sleep. It SHALL work again after the first time.
Why: the owner, L2: "можно кастомизировать всё". Rules 14, 16, 20.

#### Scenario: A key is assigned twice in a row
- **WHEN** Split gets Q, then W; Esc while listening; Enter while listening; a chord on Play; the window closes
- **THEN** each press assigns and does not act; Esc lets go, the window stays; both refusals show their reason; S no longer splits, W does
- **PATH** Customize → type "split" in the search → press the cap → it says "press a key…" → press Q → the cap says Q, the reset arrow appears → press the cap → press W → the cap says W
- **TEST** t1711 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: Every key name has its glyphs
- **WHEN** the printed name of every key a code can carry is checked against our fonts
- **THEN** none draws a square: the arrows print as words, their symbols are not in the fonts
- **PATH** Customize → press a cap → press the left arrow → the cap says Left
- **TEST** t1718 (`src/apps/kt-app/src/font_tests.rs`)

#### Scenario: A taken key is asked
- **WHEN** Split asks for M, the answer is Leave it; again, the answer is Take it; the command, then the style, are reset
- **THEN** the row names the marker command; Leave changes nothing; Take moves the key and the marker has none, its hint printing a dash; both resets return the table
- **PATH** press Split's cap → press M → under the list "M is already on: Add or remove a marker" with Take it from there and Leave it there → press Leave it there → nothing moved
- **TEST** t1712 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: Changes stay on top of the style
- **WHEN** a key is bound, bound back, unbound, reset; one of a command's two keys is given up
- **THEN** a change equal to the style is not stored; "no key" is carried by the code; the other key stays
- **PATH** Customize → change a key → the arrow appears → change it back → the arrow goes
- **TEST** t1705 (`src/apps/kt-app/src/controls_tests.rs`)

### Requirement: The settings code carries the choice
The code SHALL carry the style, the arrangement, the wheel and every changed key, as Crockford Base32 in groups of five with a version and a CRC-16. A factory style SHALL take at most 10 characters. It SHALL read lower case, O for 0, I and L for 1, spaces and dashes. A wrong code SHALL be refused with its reason and SHALL change nothing. Its numbers SHALL be frozen. The store SHALL keep the same string, and a tile SHALL keep its last changes.
Why: the owner, L2: "получить "код сохранения" настроек, чтобы можно было при переустановке его ввести (как в старых играх сохранения)". Reconnaissance F-UI-007: a short code holds the data only when the state is small, and only with an alphabet and a check made for hands.

#### Scenario: A code given today opens tomorrow
- **WHEN** the two frozen literals are decoded and the same choices are encoded
- **THEN** they match byte for byte: a renumbered table turns this red
- **PATH** write a code down → update the program → enter it → the same keys and panels
- **TEST** t1702 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: A code survives how a person writes it
- **WHEN** a code is entered in lower case, with spaces, with no separators, with O and I for 0 and 1
- **THEN** each gives the same choice; a factory style is at most 10 characters
- **PATH** copy the code by hand on paper → type it back in lower case → Apply → it is taken
- **TEST** t1703 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: A wrong code is refused
- **WHEN** every single letter of a code is changed in turn; garbage; a short string; an empty string
- **THEN** each is refused, none is applied
- **PATH** change one letter of the code → Apply → the error says there is a typo → the tile and the keys did not move
- **TEST** t1704 (`src/apps/kt-app/src/controls_tests.rs`)

#### Scenario: The code in the window and after a reinstall
- **WHEN** a typo is entered; the right code in lower case; the store is read by a new start; a tile is left and picked again
- **THEN** the refusal is shown and nothing changed; the choice is in force; the new start has it; the tile brings its changes back
- **PATH** under the tiles the Settings code with Copy → reinstall → enter the code in the field → Apply → the tile, the panels and the keys are back
- **TEST** t1713 (`src/apps/kt-app/src/controls_tests.rs`)

### Requirement: The region's interior is cut out by a key and the Edit menu
"Cut out the region interior" SHALL be a command of the key table, bound to `C` in our layout and
unbound in the borrowed layouts, and SHALL have an Edit menu item with its chord; the key, the Edit
item and the empty-space item SHALL run one body, which cuts out the interior of the region from
every event it crosses and leaves the rest in place, as one undo step. Without a region the key and
the Edit item SHALL say so and change nothing.
Why: the owner, `v26.10` line 35 — "нужна функция cut(ножницы) региона и чтобы еще с клавиатуры
можно было (кнопка "C" свободна?)"; the action lived only in the empty-space menu (F-TL-015).

#### Scenario: C and Edit › Cut out the region interior give the same cut
- **WHEN** a region 20 000–40 000 ticks lies over an event and `C` is pressed; the cut is undone and Edit › "Cut out the region interior" (chord `C`) is clicked
- **THEN** no material is left inside the region, the remainders end and start at its edges, one undo restores the events, and the Edit item gives exactly the cut `C` gave
- **PATH** draw a region over part of an event (I, O) → press C → the event's middle is gone, both remainders stay where they were → Ctrl+Z → the event is whole → Edit menu → "Cut out the region interior" with C on the right → click → the same gap
- **TEST** t3102 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: No region, other layouts
- **WHEN** there is no region and `C` is pressed; the key list of every borrowed layout is read
- **THEN** the project is unchanged and the notice says "Nothing to cut out: draw a region first"; our layout prints `C` for the command, every borrowed layout prints no key
- **PATH** clear the region → press C → nothing is cut, the status line says to draw a region first
- **TEST** t3103 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A colour is taken from the screen with the eyedropper
Every colour button of the program SHALL carry an eyedropper beside it. A click on it SHALL arm it:
the cursor becomes a crosshair and the status line says to click a colour. While armed, the next
click anywhere in the program window SHALL be taken by the eyedropper alone — nothing under it acts
— and SHALL write the screen's colour under the pointer into that row as one history step. Esc, or the
eyedropper again, SHALL put it down without writing. Where the screen cannot be read the bell SHALL
say so. The texts SHALL exist in all five catalogs.
Why: kkt-26.11.01b line 3 (B2): "greenscreen не имеет пипетки, чтобы ткнуть в тот же кадр превью или
еще где-либо взять цвет с экрана".

#### Scenario: The key colour is picked from the preview's frame
- **WHEN** a Greenscreen slot's pipette is clicked, then the preview's picture is clicked
- **THEN** the status line said to click a colour; the key colour is the screen's colour there, written as one history step; the preview did not start playing; the pipette is down
- **PATH** double click a clip → its effects window → + effect… → Greenscreen → the pipette beside Key → the cursor is a crosshair, the status line says to click a colour → click the preview's picture → Key shows the frame's colour, the background goes transparent
- **TEST** t3120 (`src/apps/kt-app/src/input_work_tests.rs`), live scenario `src/scripts/live/eyedropper.py` (`./scripts/live-check.sh`)

#### Scenario: Esc or the pipette again puts it down
- **WHEN** the pipette is armed and Esc is pressed; then armed and clicked again
- **THEN** each time it is down, the effects window is still open and no colour was written
- **PATH** the pipette → crosshair → Esc → the arrow is back, the window stays → the pipette twice → the arrow is back
- **TEST** t3121 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: Every colour button has it
- **WHEN** the kt-app sources are read for colour buttons; the catalogs are read
- **THEN** every colour button has the eyedropper call beside it; the three eyedropper texts exist in all five catalogs and the armed line names Esc
- **PATH** any colour row — a title's fill, a tint, a transition's colour — shows the pipette after its colour swatch
- **TEST** t3122 (`src/apps/kt-app/src/input_work_tests.rs`)
