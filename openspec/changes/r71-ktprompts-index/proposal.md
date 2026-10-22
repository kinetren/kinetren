# Proposal

## Why

The owner put the claims file `ktprompts_v26.10` (36 KB, ~75 numbered items across
twelve areas: generators, usability, timeline, render, windows, automation,
layouts, NLE, core/format, plugins, FX, 3D, OpenFX, render cache, memory, future)
and asked to "index it into the nearest plan of edits". His own rules in the file:
"split into stages, discuss and decide", "we work through each point separately, one by one",
"no backward compatibility needed", "on any points I may be wrong — ask questions, don't hold back",
"make notes for agents, tests, so as not to return to the same kinds of problems".

A cross-check against the code and specs (03.10–12.10.2026) showed why the index is needed BEFORE any edit:
**part of the items is already closed by rounds R.49–R.70** (generator automation — R.59,
generator and effects in one window — R.59, freeze frame — R.57, crash report — R.49, moving away from
OTIO and git-indexability — R.70), **part contradicts his own later decisions** (the format name `ktproj` and removing the import of old files against `.kt` and one import door
R.67/R.70), **part are defects that are already repeating** (resize and the jump zone steal
clicks — the theme of R.12/R.13; "obvious labels" — checklist item 17 of R.7, broken by "Add morpher
here"), the rest are features without a reconnaissance note, and the future. Without a registry the next round
would take from the file what is already done or would do the opposite of what was decided.

## What Changes

- **Claims registry** `docs/ktprompts/v26.10-index.md`: each claim of the file gets a row with a state from the closed set (`done` — round and test; `partial`; `no` —
  a defect or a feature with an address of a capability/note; `question` — the text of the question to the owner;
  `future`), and a stage. The verbatim source — `docs/ktprompts/v26.10.txt` (copied at
  the proposal, this is the primary source; it is also quoted in `docs/user-feedback.md`).
- **Completeness guard** `src/scripts/ktprompts-index.py --check` (modeled on
  `features-index.py` and `live-coverage.py`): an item without a row, a state outside the set,
  "done" without an existing test in the sources, a "question" without text — red; it is part of
  `finish.sh` and of test t823. This is the "notes for agents and tests, so as not to
  return" — in a form that fails the check, not one that asks for a checklist.
- **Questions to the owner** — eleven, collected in `design.md`; the answers go into
  `docs/user-feedback.md` verbatim and move the registry rows from `question` to a decision.
- **Nearest plan** — the order of the next rounds by the registry (a proposal, confirmed by the
  answer to question 11): R.72 generators (the first section of the file) → R.73 timeline and
  transport defects (cheap and many) → R.74 render (encoders, formats, loudness, crash) → R.75
  windows and glow → R.76 automation and presets; further — features by notes (timeline notes, region
  cut, layouts, NLE, plugins/OpenFX, FX, 3D, render cache, memory). Each round is its own change; this change does not
  alter the application code.
- **Rule for agents** (in the root CLAUDE.md, a task of stage 3): any change to the model
  that the OTIO exchange file cannot express is recorded in the "what export loses" section of the
  registry — the owner's requirement from the CORE section ("any agent adding a feature that
  breaks otio must record this in a separate list").

Not included: implementation of any claim (each — its own change after the answers), reconnaissance of
other editors for new features (opened as a note in the round that takes the feature;
the exception — items where a note already exists, they are named in the registry).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `durability`: a requirement is added, "The owner's claims are indexed by the registry;
  the guard keeps it complete and honest" — next to the requirement "The work ends with
  `finish.sh`", because it is the same class: a process that fails the check rather than
  relying on a checklist.

## Impact

- `docs/ktprompts/` (a new directory: the verbatim source and the registry), `docs/user-feedback.md`
  (section R.71 — written at the proposal), `src/scripts/ktprompts-index.py` (the new
  guard), `src/scripts/finish.sh` (the call of the guard), `src/apps/kt-app/src/reach_tests.rs`
  (t823), `CLAUDE.md` (the rule "what export loses"), `openspec/specs/durability`.
- Application code, kt-cli, model, format — do not change.
