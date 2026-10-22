# Spec Delta

## ADDED Requirements

### Requirement: Import of an old-version project — its own door; "Open" rejects it with the address
The "File" menu SHALL have an item "Import older Kinotrain project…" (the industry name —
"Import project", as in Premiere/Resolve), which reads a container file of an EARLIER version,
applying repairs of old forms, and opens it as a document of the current form with the previous path
and the "has edits" flag raised (the next write goes in the new form). Ordinary "Open"
SHALL reject such a file with the file's version and with the name of the import item in the error text
(rule 19: the dialog says where to change the decision), not with "metadata.kt is unreadable".
The item SHALL be in the verb registry and reachable from the menu and from the open-error window.
Why: the owner's decision (R.67): "make an item in the File menu for importing old versions, and
fix precisely in the import so that the reader can read it normally".

#### Scenario: An old file through import, "Open" names the door
- **WHEN** a v5 file with `targets: null` on morphers (the form `untitled2_v26.09.26.otio`) is opened via "Open"; then via "Import older Kinotrain project…"
- **THEN** "Open" rejects it with a text that contains the file's version (5) and the name of the import item; the import opens the project: morphers with `null` act on "everything below", the number of events and morphers equals the file, the title carries an asterisk, saving writes the current form (the version — by content) and the next "Open" already reads the file itself
- **PATH** File → Open → old file → error window: "version 5; use File → Import older Kinotrain project…" → File → Import older… → the same file → the timeline with events and morphers, an asterisk in the title
- **TEST** t783 (`src/apps/kt-app/src/reach_tests.rs`), t783b (`src/libs/kt-otio/tests/convert.rs`), live scenario `src/scripts/live/import_old.py`

### Requirement: User path for a new UI scenario; live scenario screenshots in the report
Each UI capability scenario (`ui-timeline`, `ui-reach`) whose test lives in
`src/apps/kt-app/` AND has the number t783 or higher, SHALL carry a line `- **PATH**` —
the sequence of the person's actions (where to click, what to drag) and what they SEE after
each step; without it the spec check SHALL be red (the guard is a script run by the
test and by `finish.sh`). A scenario where there is no gesture at all (the spec guard, producing a
report) has, on the same line, a REASON in words — and it is measurable: a reason shorter than forty characters
is not accepted, otherwise the excuse would become cheaper than the description. The threshold is by number, not
"for everyone": the numbering is continuous and monotonic, and the rule applies from the round in which it is introduced —
rewriting two hundred earlier scenarios for the sake of the form is pointless.
Live scenarios SHALL save a screenshot for each step of `PATH` into `target/live-out/`, and
`finish.sh` SHALL print the list of these files after the version; the agent's report on UI work carries
these screenshots. The run goes on OUR fixture projects, not on the owner's projects.
Why: R.67, the owner: "HOW MANY TIMES DO I HAVE TO REPEAT SO THAT ALL AGENTS THINK ABOUT THE USER FIRST? … THIS IS CRUCIAL TO CHANGE!" — and his choice: both mechanisms, "but not on
my projects". A rule in prose (a checklist, R.40) held for seven rounds and did not work:
the check "what the person sees" must be part of the text of the contract and part of the report.

#### Scenario: The guard of the user path
- **WHEN** the specs are checked; temporarily the `PATH` line is removed from a new UI scenario; separately — from a scenario before t783
- **THEN** the check is green on the current specs; removal from a new scenario — red with the address of the file and the scenario; for an old scenario — not required (the numbering is monotonic, the threshold is measurable)
- **PATH** there is no gesture: the guard reads the spec texts and fails on a scenario without a described path — checked by running the script, not with the mouse
- **TEST** t799 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/specs-user-path.py --check`)

#### Scenario: Step screenshots in the report
- **WHEN** a live scenario with `PATH` steps is run; then `finish.sh`
- **THEN** in `target/live-out/<scenario>/` there is one file per step with the number and the name of the step; `finish.sh` prints their list after the version line; the list is non-empty for each scenario of the round
- **PATH** there is no gesture: checked by the fact that after the run the step files lie in `target/live-out/<scenario>/`, and `finish.sh` prints their list
- **TEST** live scenarios `src/scripts/live/curves.py`, `stills.py`, `import_old.py` (`./scripts/live-check.sh`), `src/scripts/finish.sh`
