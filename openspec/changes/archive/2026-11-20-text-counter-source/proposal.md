# Proposal

## Why

The owner's technical group of G5 (`v26.10` line 16, his answer of 2026-10-30, question 6) names a
"timer/counter"; his G3 decision (line 14, question 5) is that the timecode is TEXT and is controlled as
text — a mode of the Text generator, with the whole text passport, not a separate pixel-font kind. A
counter is the same thing.

## What Changes

- The Text generator gets a SOURCE: the typed text (default), a timecode, a stopwatch or a countdown,
  running with the event's material time. The typed text becomes a template: `{}` stands for the value,
  without it the value is the whole text. Every text property (font, size, colours, outline, shadow,
  backing, animation) works on it.
- A stopwatch and a countdown have four formats (`1:05`, `1:05.3`, `0:01:05`, `65.3`) and a start in
  seconds; a timecode has its frame rate. A stopwatch shows what has passed, a countdown what is left
  (rounded up, zero only when the time is up, then it stays at zero).
- The title's window chooses the source in a row of buttons under the text; the start and the frame
  rate show only where they count.
- A file with such a title is format 7 (by content); a typed title writes the former bytes.
- Not here: removing the separate `timecode` kind and converting its events by the import (G3's own work).

## Capabilities

- `generators`: ADDED requirement "A title runs a timecode, a stopwatch or a countdown".
- `project-format`: ADDED requirement "A running title is written by content".

## Impact

`kt-model` (`genmedia.rs`: `TextSource`, `CountFormat`, four fields, `title_text_at`, path tokens;
`project.rs`: `has_text_sources`; `project_file.rs`: format 7), `kt-media` (`generator.rs`: the
shown text at `pts`, `depends_on_time`), `kt-app` (`gen_ui.rs`, `widgets.rs`, catalogs ×5). Tests
t3703–t3706; t1048's tuned title row; live `text_counter`.
