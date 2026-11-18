# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 156, registry MW1): "в медиа-виджете конечно прикольно
что скролится предпросмотр, но можно таки отображать там внутри таймкод курсора". Scrubbing a bin
card pages through the file, but the person sees no number: on a long file a pixel is seconds, and
"the take at about 1:40" cannot be found by the bin alone. Final Cut Pro's Skimmer Info writes the
source timecode while skimming; Premiere's hover scrub writes none (feature note F-MED-008).

## What Changes

- While the pointer scrubs a file card, the moment of the file under the cursor is written over the
  bottom of the thumbnail, on a dark backing, in the program's timecode form (`format_timecode`,
  the project's fps). It comes from the SAME hover-x that picks the thumbnail.
- A narrow card shortens the label from the left and then drops the frame and the milliseconds,
  never covering more than the thumbnail.
- The label is laid out once per moment and replayed on other hover frames (no allocation per
  frame); it asks for no decode (invariant 8) and no repaint of its own.
- Not in this change: a drag that places the file from the scrubbed moment (the second half of his
  line, "мб его порезать захотят или нет") — a question to him.

## Capabilities

- `ui-reach`: ADDED requirement "The bin's scrub writes the moment under the cursor".

## Impact

`src/apps/kt-app/src/bin_panel.rs`, `app.rs` (one field). Tests t3100, t3101; live scenario
`bin_scrub_tc`. No format change, no text in the catalogs (a number only).
