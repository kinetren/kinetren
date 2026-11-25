# Proposal: timecode-is-text

> DESIGN ONLY (spec session), base orch `5986f08e`. The rest of G3 (`v26.10-index.md`), left out of
> `text-counter-source` on purpose. Tests t3993–t3997.

## Why

The owner's item, whole, `docs/ktprompts/v26.10.txt` line 14: "3. У большинства генераторов очень мало параметров,
например, тот же таймкод является текстом, но я не могу управлять им так же гибко как текстом, что очень странно. Ну и
т.д." His answer (2026-10-30, question 5, `user-feedback.md`): "Timecode = a mode of the text": the separate kind goes,
the full text passport works at once, old Timecode events are converted by the import.

`text-counter-source` built the mode. Today both exist: Generators → Titles offers "Timecode", a pixel-font picture
with one parameter (`fps`), and the Text title with a timecode source, font, colours, outline, curves and animation. A
person who picks the first still has the complaint of line 14.

## What Changes

- The generator kind `timecode` is removed: plugin, id in the built-in list, menu row, note, catalog key.
- The menu keeps a row "Timecode": a Text title whose source is the timecode. Same place, same word, full passport.
- **BREAKING** A project that holds a `timecode` event is not opened by "Open"; the refusal names the file's format,
  the removed kind and "Import older Kinetren project…" (the owner's rule R.67: repairs live in the import).
- The import door takes such a `.kt`, `.otio` or recovery text and turns each event into a title with the same running
  digits: centred, light grey on a black plate, the same width of the frame, the same frame rate and its curve.
- The import dialog offers `.kt` files (today it lists only `.otio` and `.ktproj`, so a project of the former name, which
  the door accepts, cannot be picked in it).

## Capabilities

### Modified Capabilities
- `generators`: the closed list without `timecode`; the menu row "Timecode" is a title.
- `project-format`: a retired generator kind is refused by the reader and repaired by the import door.
- `ui-reach`: the import of such a project from the menu.

## Impact

- kt-plugins-builtin (`BUILTIN_IDS`), plugins/generators-test (`timecode`, its passport), kt-model (`genmedia.rs`
  `ORDER`, the menu row; `project_file.rs` `RETIRED_GENERATORS` and the refusal), kt-otio (`legacy_import.rs`
  `retired_timecode`; `lib.rs` `import_interchange` takes a `.kt` with retired kinds), kt-app (`gen_ui.rs:809`,
  the import dialog's filter, recovery), kt-cli `import`; i18n ×5.
- By the owner's decision: t1048's `timecode` row goes with the kind, and t493 reads the title instead of the kind (the same
  promise: the digits follow the material at the set rate).
- No format number: a converted title is format 7 by content already. No OTIO loss: a title's `.otio` form is unchanged.
- Idle: nothing.

## Questions to the owner

None: his answer 5 decides the kind, R.67 decides the door.
