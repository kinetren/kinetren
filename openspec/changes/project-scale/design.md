# Design: project-scale

> DESIGN ONLY, revision 1 (2026-11-20), base `ea0aff6f`. Q1–Q2 in `proposal.md`; defaults recommended.

## Context

- `.kt` (R.70, `src/libs/kt-model/src/project_file.rs`): pretty JSON, line-based, version by content (1–7 today),
  marker `kinetren` first. `from_text` does `serde_json::from_str::<Value>` then `from_value::<Envelope>`: the whole
  file lives twice (a DOM, then the model), the DOM about 5–10× the text in memory. `to_text` builds
  `Envelope { project: project.clone() }`.
- Measured so far: rows 192 and 641 (`kt-cli edit-cost`, up to 41 357 events); the build-cost guard
  (`src/libs/kt-engine/tests/build_cost.rs`, t2062) counts walked elements and allocated bytes, not wall time.
- Change `shared-events` (design): the model's per-edit whole-project copies.
- The owner's answers of 2026-10-30, questions 3 and 4 (`docs/user-feedback.md`).

## D1. The measurement

`kt-cli scale <dir> --events N --shape flat|nested|copies [--release-only]` writes the synthetic project (media are
generators, so no files are needed) and prints one row per path with wall time on the release build, allocations
and peak resident memory:

| Path | What is timed |
|---|---|
| read | `from_text` |
| open | read + normalize + validate + the first audio and video builds |
| write / autosave | `to_text` + the atomic write |
| edit | one move: the batch, the undo snapshot, the player's handover, the incremental build |
| undo | one undo |
| view middle | `EventIndex::hits` for a 10 s window in the middle |
| view all | the timeline's events at full zoom-out (the draw path's per-event work) |
| bin | the bin list over the media |
| otio | the OTIO export |

The release table at 10 k / 100 k / 1 M per shape goes into this design (revision 2) with the commit it was taken
on. A path over the Q1 budget, or growing faster than linear, gets a found-defects row (block 1100–1149) with its
numbers; the ones in kt-model's file code are fixed here (D2), others go to their owners (shared-events for edit and
undo, ui-frame-cost for the view).

## D2. One parse, no clone

Read: the file's first line after `{` is the marker (the envelope's field order); a small scanner reads the version
from it, refuses a newer one, then one typed `serde_json::from_str::<Envelope>` builds the model straight from the
text. The import door keeps its `Value` path (it repairs shapes). Write: `Envelope<'a> { project: &'a Project }`.
Byte output unchanged (t-idempotence guards stay).

## D3. The chain table (C2c, option A)

- A chain = the slots, their bypass flags, their curves and the chain's master switch of one carrier (an event, a
  track, a morpher patch's two chains). Its canonical text is the serialization the file would write for it; its key
  is xxh3-64 of that text, written as 16 hex digits.
- On write: count chains by key; a chain seen at least twice goes into `"chains": { "<hash>": {…} }` at the END of
  the file (sorted by key); each carrier of it writes `"chain": "<hash>"` in place of its four fields. A chain seen
  once is written in place, as today. Deterministic: the same model gives the same bytes.
- On read: the table is read with the body (one parse); every reference is expanded into its own copy; an unknown
  hash is a named error with the carrier's address (the "broken section" rule). The model never sees the table: an
  edit of one copy changes only that copy (the owner's option A).
- Version by content: a file with a non-empty table has the next free version; a project without repeated chains is
  byte-equal to before.
- `git diff`: editing one copy of a referenced chain moves that carrier to its own inline chain (or to a new table
  entry if it now matches another), a few lines; the table entry stays while two copies use it.
- Expected effect for the owner's case (10 k copies × 100 effects with curves): the chain text once instead of
  10 k times — the measurement in D1 states the ratio and the read time.

## D4. Partial read (Q1 b) — not now

A file read in parts (a per-reel index, the visible middle first) is the answer only if D1 shows the one-pass read
over the budget at 1 M. It would need a section index in the file and a lazy model; recorded as the next step with
the measured number that would justify it.

## D5. Class walk — "a path that is linear in the project where it could be local"

D1's table IS the walk: every path a person meets, measured at three sizes. Already known: the edit's whole-project
copies (`shared-events`), the builds' resolve and per-event hash (row 192), the file's double parse and write clone
(D2 here).

## D6. Idle

Nothing at rest: the measurement is a command; the chain table is computed only while a file is written (on the
save lane), expanded only while one is read.

## Rejected

- **References kept in the model (a live link)** — the owner chose option A; live following is the composition's
  public parameters (R.59) and links (R.61).
- **A binary or compressed file** — forbidden by the owner (R.70).
- **Referencing chains seen once** — a table entry and a reference cost more lines than the chain they replace.
