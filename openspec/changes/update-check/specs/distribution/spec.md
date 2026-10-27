## ADDED Requirements

### Requirement: A release build asks once a day, in the background, whether a newer release is out
A release build of the GUI SHALL, after its first frame is drawn, check whether the latest
normal release of `github.com/kinetren/kinetren` is newer than the running build — at most once
per UTC calendar day per data directory. The check SHALL be one HTTPS `GET` of
`https://api.github.com/repos/kinetren/kinetren/releases/latest` made from a background thread,
carrying no query string, no cookie and no header that identifies the person or the machine:
only `Accept: application/vnd.github+json` and `User-Agent: kinetren/<kt_version::FULL>`. The UI
thread SHALL only start the thread and later read its result; it MUST NOT wait for it, and
`on_exit` MUST NOT wait for it either. The connect timeout SHALL be 5 s and the whole request
10 s; the body read SHALL stop at 1 MiB. A failed check (no network, timeout, HTTP error,
unparsable body) SHALL be silent — one `journal::log` line, no notice, no title change — and
SHALL NOT mark the day as checked, so the next start tries again. `kt-cli` SHALL never check.

The result of a successful check SHALL be stored in `<data_dir>/update-check` as plain text:
the UTC day of the check and the version of the latest normal release (or nothing, when the
release carries no recognisable version). A start on a day already recorded SHALL send no
request and use the stored release version; an unreadable or malformed file SHALL count as "not
checked today" and be rewritten after the next successful check.

"Newer" SHALL be decided only from the release's asset names, which carry the binary's own
version form (`kinetren-<YY.MM.DD>-<hash8>-<os>-<arch>.<ext>`, `release-build.sh`): the release is
newer if and only if its date is strictly later than the running build's commit date AND its
hash differs. A release marked `prerelease` or `draft`, a release with no asset of that form, an
older or same-day release, and a running build without a git version (`kt00.00.00-nogit`) SHALL
all count as "not newer"; the last one SHALL not even send the request.
Why: the owner asked for a reminder that the version is old, checked "if today we have not
checked yet", without bothering the person at start (2026-10-27). The tag is only the release's
name (distribution design: "the tag is only the release name"), and a tag can be dated later
than the commit it points to — comparing tags with the binary's commit date would call the very
release a person just downloaded "outdated". Being wrong in the "not newer" direction costs one
day of silence; being wrong in the other direction teaches the person to ignore the mark.

#### Scenario: "Newer" is decided from asset names, conservatively
- **WHEN** the decision is asked for the running build `kt26.10.20-aaaaaaaa` against: a release
  with asset `kinetren-26.11.01-bbbbbbbb-linux-x86_64.tar.gz`; the same asset in a release with
  `prerelease: true`; with `draft: true`; asset `kinetren-26.10.20-bbbbbbbb-…` (same day, other
  hash); `kinetren-26.11.01-aaaaaaaa-…` (same hash); `kinetren-26.10.01-cccccccc-…` (older); a
  release with tag `v26.12.01` and only a `notes.txt` asset; a body that is not JSON; and the
  running build `kt00.00.00-nogit` against the first release
- **THEN** only the first case is "newer" and it reports `kt26.11.01-bbbbbbbb`; every other
  case is "not newer"; the `nogit` build reports that no check is to be made at all; and
  `src/scripts/release-build.sh` still names its assets `kinetren-$VERSION-$TARGET-$ARCH`
  (the form the decision parses — a rename there would silently make every release "not newer")
- **TEST** t872 (`src/apps/kt-app/src/update_check.rs`)

#### Scenario: Once a day, the day's answer is reused, a failure does not count
- **WHEN** in an empty temp data directory the "is a check due" question is asked for UTC day
  2026-10-27; then a successful check with a newer release `kt26.11.01-bbbbbbbb` is recorded;
  then the question is asked again for 2026-10-27 and for 2026-10-28; then the file is replaced
  by `garbage`; then, in a fresh directory, a failed check is recorded for 2026-10-27
- **THEN** the first answer is "due"; after the record the same day is "not due" and returns
  the stored `kt26.11.01-bbbbbbbb`, the next day is "due"; after `garbage` the answer is "due"
  and nothing panics; after the failed check the same day is still "due" and the file does not
  exist
- **TEST** t873 (`src/apps/kt-app/src/update_check.rs`)

#### Scenario: The request is anonymous and a dead server costs the UI nothing
- **WHEN** the checker points at a local `TcpListener` that accepts and never answers, the
  check is started and `on_exit` runs; then at a local listener that answers `200` with a
  release JSON
- **THEN** starting returns in under 1 ms, `on_exit` returns in under 50 ms, no notice is
  produced and the day is not recorded; against the answering listener exactly one `GET` with
  path `/repos/kinetren/kinetren/releases/latest` and no query string arrives, it carries
  `Accept` and `User-Agent: kinetren/<FULL>` and no header outside the allow-list `Host`,
  `Accept`, `User-Agent`, `Accept-Encoding`, `Connection`, and the text of the request holds neither the user name
  nor the host name nor the data directory path
- **TEST** t874 (`src/apps/kt-app/src/update_check.rs`)

### Requirement: A newer release is said in the bell and in the title, never in a pop-up
When the day's check (or the stored answer of today's check) says a newer release is out, the
program SHALL post exactly ONE info notice to the notices window — on the start whose check
found it, not on later starts the same day — naming the running version, the new version, the
address `github.com/kinetren/kinetren/releases` and the View menu item that switches the check
off. The window title SHALL end with the running version followed by a short translated word —
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
  severity) with both versions, the releases address and the View item name; the title ends
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

### Requirement: The check is switched off in View; development builds never ask
The check SHALL be on unless any of these holds: the build is a debug build and `KT_UPDATE_URL`
is not set; the environment has `KINETREN_UPDATE_CHECK=0` (or `off`/`false`); the person turned
it off. The choice SHALL be stored in eframe storage under `update_check` (`on`/`off`). The View
menu SHALL carry a checkbox "Check for new versions daily" right after the telemetry checkbox;
its hover text SHALL say what is asked (the latest release, from GitHub, once a day), what is
sent (nothing but the program version) and that nothing is downloaded. Turning it off SHALL
remove the title word at once; turning it on SHALL make the check due on the next start (not
in the middle of the session). When the environment or the build blocks the check, the
checkbox SHALL be shown disabled with the reason in its hover text. A debug build SHALL honour
`KT_UPDATE_URL` (the live scenario's local server) and check then regardless of the day; a
release build SHALL ignore `KT_UPDATE_URL`, so a shipped binary cannot be pointed elsewhere.
Why: the owner chose a View checkbox like the telemetry one (2026-10-27); a per-machine
preference lives next to the other ones (checklist rule 13). The owner's live scenarios and
tests start the real `kinetren` dozens of times a day — none of them may reach GitHub
(the same reason telemetry keys only release builds).

#### Scenario: Consent resolution
- **WHEN** the consent is resolved over the table: debug build, no `KT_UPDATE_URL`; debug +
  `KT_UPDATE_URL`; release, nothing stored; release + `KINETREN_UPDATE_CHECK=0`; release +
  stored `off`; release + stored `on` + `KINETREN_UPDATE_CHECK=1`; release + `KT_UPDATE_URL`
- **THEN** the results are: off (blocked by the build); on, URL from the variable, the day
  ignored; on; off (blocked by the environment); off; on; on with the GitHub address (the
  variable ignored) — and a checker resolved off never spawns its thread
- **TEST** t877 (`src/apps/kt-app/src/update_check.rs`)

#### Scenario: The checkbox is next to telemetry and its choice holds
- **WHEN** the harness app with the check on and a stored "newer" answer opens View, unchecks
  "Check for new versions daily", draws a frame, then restarts with the stored state
- **THEN** View holds the checkbox right after the telemetry checkbox, checked, and its hover
  text names GitHub and says nothing is downloaded; after unchecking, the title has no
  "outdated" word in the same frame, the stored value is `off`, and after the restart the
  checker spawns no thread and the title stays unmarked
- **PATH** open View → "Check for new versions daily" is checked under "Send anonymous usage
  statistics" → hover it → the status bar says it asks GitHub once a day and downloads nothing
  → uncheck it → the "(outdated)" word leaves the title at once → restart → the checkbox is
  still unchecked and the title is plain
- **TEST** t878 (`src/apps/kt-app/src/reach_tests.rs`)
