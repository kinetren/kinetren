# Spec Delta

## ADDED Requirements

### Requirement: Between-projects records — recent projects, posters, templates, tour progress
The user store SHALL keep, outside every project and in the per-user data directory: the recent
projects list (path, name, last-opened time, the template it was made from), bounded to 12 entries,
newest first, with "Clear the list" in the welcome window; one poster image per recent entry, stored
under the data directory's own posters folder and keyed by the project's path, NEVER written next to
the project or into its sidecar index; the person's project templates (name, frame width and height,
frame rate), saved from Project settings with "Save as template…"; the tour progress (finished steps
per lesson); and the start choice ("Show the welcome window at start"). The poster SHALL be rendered
by the offline path — its own short-lived sources, a finite cancellable job, never during playback —
when a named project is saved or closed, from the frame at the playhead (or at a tenth of the
project's length when the playhead is at zero); the welcome window SHALL only read finished poster
files and show a placeholder until one exists. The former single `last_project` key SHALL seed the
recent list once. Each record SHALL follow the store's existing rules: a broken file is moved aside to
`.broken` and the record starts empty; a file of a future version is read but not overwritten; under
tests the store lives in memory or in the directory the test names.
Why: the welcome window (capability `ui-reach`) needs a list that F-HLP-027 found missing ("exactly
one path is stored, and it is invisible"), with a limit and a "clear" (UI rule 20: nothing grows by
itself); a poster in the project's folder or sidecar would make the program write files the person
did not ask for into his project tree, and the reel-preview disk tier lives in the sidecar precisely
because it is the project's content cache, not the program's (R.58). Templates were the fifth
between-projects record F-HLP-027 deferred to the shared store (first-wave conclusion 2).

#### Scenario: The recent list is bounded, keeps a missing file and never touches the project tree
- **WHEN** in a temporary data directory 14 projects are saved and closed in turn (each poster job
  run to completion), the 3rd project's file is then deleted, the store is reloaded, the list is
  cleared; separately the store starts with only a `last_project` value in the eframe storage;
  separately a broken `recent.json` and one of version 99 are read
- **THEN** the list holds the 12 newest, newest first; each has a poster file under the data
  directory's posters folder and no new file appears in any project folder or sidecar; the deleted
  project stays listed with its path and is reported missing; "Clear" empties the list and removes
  the posters it owned; the `last_project` start yields a list of one; the broken file is moved to
  `.broken` and the list starts empty; the version-99 file is read and left unwritten
- **TEST** t985 (`src/apps/kt-app/src/user_store.rs`)

#### Scenario: Templates, tour progress and the start choice survive a restart
- **WHEN** a user template "Shorts 60" (1080×1920, 60 fps) is saved from Project settings, two steps
  of lesson 1 are finished, "Show at start" is unchecked; the store is reloaded; then "Start over" is
  used on the tour
- **THEN** after the reload the template is listed after the built-in ones with its size and rate, the
  progress shows two finished steps, the start choice is off; "Start over" clears the progress and
  nothing else
- **TEST** t987 (`src/apps/kt-app/src/user_store.rs`)
