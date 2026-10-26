# Design

## Context

- **Where the keys live.** Global hotkeys are read in one place, `KtApp::handle_keys`
  (`src/apps/kt-app/src/app.rs:7071`), into a local `Keys` struct. Space is read from raw
  events, and the decision is made on release (`on_space_released`): a press shorter than
  `SPACE_HOLD_MS` = 220 ms toggles play, a longer one boosts the speed toward
  `SPACE_BOOST_MAX` = 4× over `SPACE_BOOST_SECS` = 5 s (an owner's limit, R.17). A second
  tap within `DOUBLE_PAUSE_MS` = 400 ms of a pause seeks to the region start, or to 0 when
  there is no region (`transport_toggle`). Local keys sit next to their fields: Enter/Esc
  in the bin rename (`bin_panel.rs:703`), Tab/Enter in the automation table
  (`automation.rs:1550`), Enter/Esc in published-parameter names (`pubparam.rs`),
  Enter/Esc in track and marker renames (`timeline.rs:5206–5316`), and the digit mask of
  the timecode (`toolbar.rs:656`) with its twin in `app.rs:6661`. Search reads ↑/↓/Enter/Esc
  (`app.rs:2346`). Mouse gestures with modifiers are in `timeline.rs`: Ctrl turns the
  magnet off and starts the rubber band; Ctrl+click adds to the selection; Shift and
  Ctrl+Shift change the wheel; double-clicks open the ruler, header, event, transition,
  reel and morpher actions. Elsewhere: the fader and speed strip (Shift = fine,
  double-click = reset, right-click or Ctrl+click = type the speed), the transform frame
  (Shift = 15° steps), the key-before/after buttons (Ctrl = every curve of the slot), the
  EQ graph (the wheel over the middle point sets Q), and parameter labels (double-click
  resets). Menu chords are fixed arguments of `Toolbar::menu_btn` / `widgets::menu_item`,
  never catalog text.
- **What the guide has today.** `shortcuts.md` has three tables (Transport, Editing,
  Mouse on the timeline), keys in bold. `timeline.md` repeats part of it in prose
  (lines 11–12, 20–23, 50, 54–55).
- **The site's renderer** (kt.9.site, change `remove-landing-keys`, D3) cuts out
  `<kbd>…</kbd>` pairs whose content has no `<`, `` ` `` or `*`, then fails on any other
  `<` + letter outside code. GitHub draws `<kbd>` as caps too.
- **The guard.** `src/scripts/readme-check.py` already walks `docs/guide/**/*.md` with
  code blocks skipped and code spans blanked (`lines_outside_code`). It reports
  `file:line: class: detail` and is run by t826/t827 and `finish.sh`.
- **The defect.** The split condition is `S && !ctrl` (`app.rs:7136`). `Alt+S` therefore
  fires `cut` and `track_solo` in the same frame. t742 presses `Alt+S` and asserts only
  the solo.

## Goals / Non-Goals

**Goals:**
- `shortcuts.md` lists every key and gesture the program handles, each checked against the
  code, and it looks right both on GitHub and on the site.
- Alt chords no longer fire the bare-letter action.
- The site's Markdown subset is enforced here, where the text is written, not discovered
  when the site's build fails.

**Non-Goals:**
- Changing any binding, hint text or catalog string. The landing rows are the thing being
  checked, not a spec to implement.
- Restyling key mentions in other guide pages to `<kbd>`. They are fixed only where they
  contradict the tables (see the open question).
- A machine check that the page is complete against the code.
- Touching kt.9.site.

## Decisions

### D1. The inventory comes from the code; the landing rows are only checked against it
The inventory is built first, before any prose, as a scratch table (key → handler `file:line`
→ condition → what it does). Every landing row is then looked up in it. Where they differ, the
page states the program's behaviour, and the difference goes into the report. Differences already
visible while planning (all to be confirmed by reading the handler):
- "hold Space — release slows down": on release the base speed returns at once
  (`on_space_released` → `set_playback_rate(base_rate)`). There is no slow-down ramp.
- "second tap — stop, to the region start": it seeks to the region start, or to the project
  start when there is no region, and stays paused.
- "double-click the volume rail": that rail was removed (R.28 item 176). Its place is the
  level line in the event's top strip, where a double-click gives 0 dB on audio and full
  opacity on video.
- "Esc — leave the reel": the cascade first closes the speed entry, search, the paste and
  preset dialogs and the top window, then cancels the drag, then clears the region and the
  selection, then collapses an expanded reel, and only then leaves the reel.
- Handled but missing from both the landing and the guide: a single click on the preview
  (play/pause), Ctrl+click on an event, the track-header double-click (rename), the header
  badges, the timecode mask keys, Shift fine adjustment and typed speed, the transform
  frame's Shift, the key-before/after Ctrl, the EQ wheel, label double-click reset.
- `Alt+S` also split (D3).

### D2. Groups by what a person looks up together; one row per action
Tables: Transport · Editing · Region and markers · Reels · View and zoom · Mouse on the
timeline · Windows and panels. The header is always `| Keys | Action |`. A group with fewer
than two rows is merged into its neighbour rather than kept alone. A key that acts in two
places gets a row in each place, worded for that place (the `compositions` rule: a chord is
promised only where it really does that). Local field keys (Enter applies, Esc cancels) get one
row per place where they do something non-obvious. The timecode mask fills digits from the
right, the speed field takes a minus for reverse, Tab in the automation table moves to the next
cell. A plain "Enter confirms a text field" is not repeated for every field.
Writing rules: `<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd>`, alternatives
`<kbd>Delete</kbd> / <kbd>Backspace</kbd>`, words for hold, double press, wheel and clicks
(`hold <kbd>Space</kbd>`, `<kbd>Ctrl</kbd>+wheel`, `double-click the timecode`). Actions
start lower-case and end with no full stop. `kinetren` is written in lower case.

### D3. Alt+S: the bare letter yields to its chords
`cut` gets `&& !i.modifiers.alt`, the same form as `marker`, `region_in/out`, `loop_toggle`
and `nav_*` already have. Shift stays allowed, as for every other letter (`Shift+A/D` relies
on it). t868 sets the playhead inside the selected event, presses `Alt+S` and asserts the solo
plus an unchanged event count. It fails on the current code, and that is checked before the
fix. The new requirement goes into `ui-timeline`, next to the t742 track-badge requirement,
and that requirement's text does not change: it already says `Alt+S` "does not take the cut".
That clause was never tested.

### D4. A `markup` class in readme-check, mirroring the site's renderer
Over `lines_outside_code` (code is skipped, as GitHub and the site skip it):
- HTML: balanced `<kbd>…</kbd>` pairs with content free of `<`, `` ` `` and `*` are cut out
  first (the site's own D3 rule). Any `<` followed by a letter, `/` or `!` that is left is a
  defect, and so is an unpaired `<kbd>` or `</kbd>`.
- A line starting with `>` (quote). A line made only of `=` or `-` (three or more), which is a
  setext underline or a thematic break; the site renders neither. A table separator
  starts with `|`, so it is not affected.
- `][` or a `[label]: url` definition line (reference links), and `[^` (footnotes).
- `_text_` with no word character on either side, so `snake_case` in prose is not hit.
- Any `\` not followed by `|`.
- A list marker nested two levels deep: deeper than the first indented level under a
  top-level item.
- `shortcuts.md` only: every table header row is `| Keys | Action |`, and no `**…**` span
  whose content is a key name or a chord (modifier names, Space/Esc/Enter/Tab/Home/End/
  Delete/Backspace, a single capital letter, or `+`-joined forms of these).
New negative cases go into a new test t869 next to t827, so t827's scenario text stays as
written.

### D5. Rejected and why
- **A completeness guard against the code** (parse `handle_keys`, diff it against the page).
  The key map is control flow inside one function, not data. Parsing it from Python would
  be a second, fragile reading of Rust, and making it data is a refactor of the hottest input
  path for the sake of a doc check. Completeness is the inventory task, and the report lists
  every row with its handler.
- **Bold keys on GitHub, caps only on the site** (rewriting `**Ctrl+S**` into `<kbd>`
  there). The site's design already rejected this: the same text would look different in
  two places.
- **Fixing the landing rows' wording into the page.** The prompt says not to copy them,
  and several are out of date (D1).

## Risks / Trade-offs

- The heuristic `_italic_` / bold-chord checks can flag a legitimate text → the fix is to
  rephrase or put the word in backticks; the guard reports the line, so this is cheap.
- The other guide pages keep bold keys, so the site shows two key styles across pages.
  **Open question for the owner:** convert `timeline.md`, `getting-started.md` and the rest to
  `<kbd>` as a follow-up?
- A future binding added to `handle_keys` without a page row is not caught mechanically
  (D5). The crate `CLAUDE.md` gets one line saying the page is the reference and must be
  updated with any binding.
