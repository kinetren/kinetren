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

(Q70, Q71 of `docs/owner-checks.md`; re-read against `fc60a5d9`, 2026-11-25.)

**Your items whole** (`docs/ktprompts/v26.10.txt` lines 97 and 99): "a. как можно быстрее индексируемо парсером (условно проект на 10млн элементов не должен вешать систему если мы смотрим только серединку проекта, да даже если
отзумили весь проект - все должно достаточно хитро работать, вероятно - как random-stream, может быть я бред полный говорю)." and "c. как можно меньше копипасты внутри файла, надо может-быть механизм ссылок ввести
для вложенных композиций? чтобы поменьше оно места сжирало. Ну и идя дальше - если представим что я взял одно событие, навешал 100 эффектов и скопировал 1000 раз и не менял события - то chain-эффектов тоже можно
сделать ссылочными, или даже все скопированные события ссылать". Your answers of 2026-10-30 (`docs/user-feedback.md`): question 3 "сотни тысяч - миллион, нужен замер, чтобы поискать узкие места …"; question 4 "п.А. …
если я 10тыс раз поставлю событие с 100 одинаковыми эффектами …".

**Now**: the largest project measured has 41 357 events: an edit there cost 10.9 ms on the interface's thread (`shared-events`, being built, removes that). A file is read whole, twice over in memory. From the fixtures: an event's own
text is about 240 bytes (12 lines), an effect 106, a curve of four keys 525.

| Events | File without effects | One effect with a curve each |
|---|---|---|
| 100 000 | 24 MB | 87 MB |
| 1 000 000 | 240 MB | 870 MB |

**Q70 (Q1). What must a million events do, by which the measurement is judged?**
- (a) **Opening up to 10 s with the window alive; any edit, scroll or zoom, also zoomed all the way out, within 16 ms; saving behind** (*recommended*): reachable with the file as it is (240 MB is read in one pass).
- (b) The same with opening up to 2 s: 240 MB cannot be read and checked whole in that; the file needs a section index and the program a model that loads the middle first and the rest behind ("random-stream"). It is the
  same work you describe in "a.", about twice as large, and the file stays text but gets an index at its end that a hand edit invalidates (it is then rebuilt at the next opening).
- (c) Only measure now; set the numbers after seeing them. Nothing is promised until then.

**Q71 (Q2). What is written once in the file: chains, or whole copied events too?** Your example, 10 000 copies of an event with 100 effects (ten with curves), computed:
- today: 161 MB, 7.5 million lines;
- (a) **chains (with curves) once, each event its own 12 lines and a reference** (*recommended*): 2.9 MB, 130 000 lines; changing one copy's chain changes a few lines of `git diff`;
- (b) also the repeated fields of events once: about 1.0 MB, 45 000 lines; an event in the file is then "template 7 + start 1234": reading a diff, or merging two people's edits, needs the template beside it, and
  moving the template's field moves lines of everybody who uses it.
