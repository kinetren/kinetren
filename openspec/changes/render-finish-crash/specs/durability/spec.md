# Spec Delta

## ADDED Requirements

### Requirement: A test never touches the person's data directory
A test process SHALL NOT read or write the person's data directory (the journal, the crash report,
recovery, project locks, the workspace, presets, recent projects and posters, the render log,
eframe's storage, FFmpeg downloads). In a test build the data directory SHALL be a folder of the
test process in the system temp directory — or the absolute `KINETREN_DATA_DIR` the test that
started the process gave it — and every path of the program's data SHALL be derived from that one
directory. No code outside the one resolver SHALL ask for the person's directories directly.
Why: until 2026-11-01 the tests wrote into the owner's real `~/.local/share/kinetren`: his journal
was full of test lines (`RENDER render log not written (/tmp/kt-t959-…)`) and rotated his own records
away at 14:09, next to 7934 lock files, a recovery file and a workspace the tests had written.

#### Scenario: A test's data directory is its own
- **WHEN** a kt-app test resolves the data directory and the paths derived from it, and writes a line
  into the journal
- **THEN** the directory is not, and does not contain or lie inside, the directory the resolver gives
  for the person; it lies in the temp directory; the journal, crash report, recovery, lock, FFmpeg
  download root, render log and user store paths lie inside it; the line is in the test's journal and
  not in the person's
- **TEST** t1310 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: Only the resolver asks for the person's directories
- **WHEN** the kt-app sources are scanned for direct requests of the person's directories (the
  `kt-dirs` data, local and download roots, eframe's storage directory, `HOME`, `APPDATA`,
  `XDG_DATA_HOME`, a home-directory lookup)
- **THEN** the only uses are the resolver itself, the test comparing it with eframe's own choice, and
  the FFmpeg download root outside a test build
- **TEST** t1311 (`src/apps/kt-app/src/journal.rs`)
