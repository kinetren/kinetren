## RENAMED Requirements

- FROM: `### Requirement: The check is switched off in View; development builds never ask`
- TO: `### Requirement: The check is switched off in Preferences; development builds never ask`

## MODIFIED Requirements

### Requirement: A newer release is said in the bell and in the title, never in a pop-up
When the day's check (or the stored answer of today's check) says a newer release is out, the
program SHALL post exactly ONE info notice to the notices window — on the start whose check
found it, not on later starts the same day — naming the running version, the new version, the
address `github.com/kinetren/kinetren/releases` and the File › Preferences… item that switches the
check off. The window title SHALL end with the running version followed by a short translated word —
`kt26.10 (outdated)` in a release build — both in the OS window title and in the program's own
title row, where the word SHALL be painted in the warning colour right after the version, and
hovering it SHALL show in the status-bar hint the new version and the releases address. The
program MUST NOT open a window, a dialog or a modal for this, MUST NOT download anything and
MUST NOT change any installed file. When the check is switched off, neither the notice nor the
title word SHALL appear, even from a stored answer.
Why: the owner's words — "remind him in the bell notice that the version is old (but do not
bother him with pop-up windows at startup), and also write at the top in the title bar,
somehow briefly, that the version is outdated" (2026-10-27); he chose "kt26.10 (outdated)"
over showing the new number in the title. The kt-app checklist forbids pop-ups and new
windows (rules 10 and 22) and new strips for rare labels (rule 13): the title row is already
occupied by the version, so the mark costs no space.

#### Scenario: The newer release reaches the bell once and the title stays marked
- **WHEN** the harness app starts with the check on and an injected check result "newer:
  `kt26.11.01-bbbbbbbb`" for the running build `kt26.10.20-aaaaaaaa`; the frame is drawn; the
  pointer is moved over the title word; then the app is rebuilt with File → New; then a
  second start the same day reads the stored answer
- **THEN** after the first start the notices list holds exactly one update notice (info
  severity) with both versions, the releases address and the Preferences item name; the title ends
  with the version and the translated "outdated" word; no window from the `WINDOWS` registry
  and no `Ask` is open; hovering the word puts the new version and the address into the
  status-bar hint; after File → New the title is still marked and there is no second notice;
  after the second start the title is marked and the notices list holds no update notice
- **PATH** start Kinetren a day after a newer release came out → no window opens; the bell in
  the status bar shows 1 and the title row reads "… — kt26.10 (outdated)" with the word in the
  warning colour → hover the word → the status bar says which version is out and where →
  click the bell → the notice says "Kinetren kt26.11.01 is out (you have kt26.10.20)", the
  address and where to turn the check off
- **TEST** t875 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Every language has the four texts and their glyphs
- **WHEN** the five catalogs are read and the update texts (the checkbox, its hover text, the
  notice and the title word) are laid out in the program's fonts
- **THEN** every catalog has all four keys with the notice's `{}` placeholders intact, the
  notice in every language contains the checkbox name, the title word is at most 12 characters,
  and no glyph falls back to the placeholder box (CJK and Arabic subsets included)
- **TEST** t876 (`src/apps/kt-app/src/i18n_tests.rs`, `src/apps/kt-app/src/font_tests.rs`)

### Requirement: The check is switched off in Preferences; development builds never ask
The check SHALL be on unless any of these holds: the build is a debug build and `KT_UPDATE_URL`
is not set; the environment has `KINETREN_UPDATE_CHECK=0` (or `off`/`false`); the person turned
it off. The choice SHALL be stored in eframe storage under `update_check` (`on`/`off`). The File ›
Preferences… window SHALL carry a checkbox "Check for new versions daily" right after the telemetry checkbox;
its hover text SHALL say what is asked (the latest release, from GitHub, once a day), what is
sent (nothing but the program version) and that nothing is downloaded. Turning it off SHALL
remove the title word on the next frame; turning it on SHALL make the check due on the next start (not
in the middle of the session). When the environment or the build blocks the check, the
checkbox SHALL be shown disabled with the reason in its hover text. A debug build SHALL honour
`KT_UPDATE_URL` (the live scenario's local server) and check then regardless of the day; a
release build SHALL ignore `KT_UPDATE_URL`, so a shipped binary cannot be pointed elsewhere.
Why: the owner chose a View checkbox like the telemetry one (2026-10-27); a per-machine
preference lives next to the other ones (checklist rule 13). The owner's live scenarios and
tests start the real `kinetren` dozens of times a day — none of them may reach GitHub
(the same reason telemetry keys only release builds). On 2026-11-01 (kkt-26.11.01 item 13) the
program's preferences left View for File › Preferences…, the two checkboxes staying neighbours.

#### Scenario: Consent resolution
- **WHEN** the consent is resolved over the table: debug build, no `KT_UPDATE_URL`; debug +
  `KT_UPDATE_URL`; release, nothing stored; release + `KINETREN_UPDATE_CHECK=0`; release +
  stored `off`; release + stored `on` + `KINETREN_UPDATE_CHECK=1`; release + `KT_UPDATE_URL`
- **THEN** the results are: off (blocked by the build); on, URL from the variable, the day
  ignored; on; off (blocked by the environment); off; on; on with the GitHub address (the
  variable ignored) — and a checker resolved off never spawns its thread
- **TEST** t877 (`src/apps/kt-app/src/update_check.rs`)

#### Scenario: The checkbox is next to telemetry and its choice holds
- **WHEN** the harness app with the check on and a stored "newer" answer opens File › Preferences…, unchecks
  "Check for new versions daily", draws a frame, then restarts with the stored state
- **THEN** the Preferences window holds the checkbox right after the telemetry checkbox, checked, and its hover
  text names GitHub and says nothing is downloaded; after unchecking, the title has no
  "outdated" word on the next frame and the word is no longer painted, the stored value is `off`, and after the restart the
  checker spawns no thread and the title stays unmarked
- **PATH** File › Preferences… → "Check for new versions daily" is checked under "Send anonymous usage
  statistics" → hover it → the status bar says it asks GitHub once a day and downloads nothing
  → uncheck it → the "(outdated)" word leaves the title → restart → the checkbox is
  still unchecked and the title is plain
- **TEST** t878 (`src/apps/kt-app/src/reach_tests.rs`)
