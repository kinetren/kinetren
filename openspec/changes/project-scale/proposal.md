# Proposal: project-scale

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` C2a and C2c (no: defect → project-format),
> round R.78. Tests reserved: t3924–t3931.

## Why

The owner, `v26.10` CORE item 2a: "проект на 10млн элементов не должен вешать систему если мы смотрим только
серединку проекта, да даже если отзумили весь проект"; item 2c: "если представим что я взял одно событие, навешал
100 эффектов и скопировал 1000 раз … chain-эффектов тоже можно сделать ссылочными". His answers of 2026-10-30:
question 3 — "сотни тысяч - миллион, нужен замер, чтобы поискать узкие места в программе … возможно breaking changes
сделать"; question 4 — option A, a reference only in the FILE ("если 10тыс раз поставлю событие с 100 одинаковыми
эффектами … это же все равно сколько места экономит и скорости чтения"). He asked for both to be measured together.

Today: `project_file::from_text` parses the whole file twice (a `serde_json::Value`, then the typed model from it);
`to_text` clones the project into its envelope; nothing has ever been measured above 41 357 events (rows 192, 641);
every copy of an event writes its whole chain with every curve.

## What Changes

- **A measurement, not a guess**: `kt-cli scale` builds synthetic projects of 10 k, 100 k and 1 M events in three
  shapes (flat, nested reels, 10 k copies of one event with 100 effects and curves) and times every path a person
  meets: open (read, validate, first engine build), save and autosave, an edit and its undo, the timeline at full
  zoom-out and in the middle, the bin, the OTIO export, and peak memory. The table goes into this change's design
  and every path over its budget or non-linear becomes a found-defects row with its number.
- **Breaking fixes found by reading, done in the same round**: a single typed parse of the file (the version is the
  first key, so it is checked before the body); `to_text` serializes the borrowed project (no clone).
- **Chain references in the file (C2c, option A)**: a `chains` table at the end of the `.kt` file holds every
  effect chain (slots, bypass, curves, the master switch) that appears at least twice, keyed by its content hash;
  each such event, track or patch writes `"chain": "<hash>"` instead. Reading expands every reference into an
  independent copy: in the program every copy stays its own. The file is still line-based and deterministic.

## Capabilities

### Modified Capabilities

- `project-format`: one parse; the chain table and references by content; growth guards at scale.

## Impact

- kt-model (`project_file.rs`): the typed single-pass read with the version peeked from the first key; a borrowing
  envelope; the chain table on write (canonical text → xxh3 → sorted table), expansion on read.
- kt-cli: `scale` (generator + timings, release build, the report as a table).
- Format: the chain table raises the `.kt` version by content (the next free number at apply time); a project with
  no repeated chain is written byte for byte as before. OTIO: no loss (the export writes each chain in full, as
  today).
- Interacts with change `shared-events` (in-memory sharing of events across versions): this change does not
  touch the model's memory; the measurement reports both before and after it.
- Idle: nothing runs at rest; the tool is a command.

## Questions to the owner

**Q1. What "does not hang" means at a million events.** Your words: "сотни тысяч - миллион … не должен вешать
систему". Today nobody has opened more than 41 357 events; an edit there costs 10.9 ms on the UI thread.
- (a) **Release build, 1 M events: opening ≤ 10 s with the window responsive meanwhile; any edit, scroll or zoom
  ≤ 16 ms on the UI thread; saving in the background** (*recommended*: what the measurement is judged against).
- (b) The same with opening ≤ 2 s, which likely needs a file read in parts (only the visible middle first).

**Q2. References for chains only, or for whole events too?** Your words: "chain-эффектов тоже можно сделать
ссылочными, или даже все скопированные события ссылать". A copied event differs from its source by its position,
and often by its source and in-point; its chain is what weighs (100 effects with curves against about ten lines of
its own).
- (a) **Chains only (with their curves), for events, tracks and morpher patches** (*recommended*: the weight is
  there; an event's own lines stay readable in `git diff`).
- (b) Also an "event template" table: the shared fields of copies written once, each copy writing only what differs.
