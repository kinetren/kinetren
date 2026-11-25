# Proposal: told-before-render

> DESIGN ONLY (spec session, 2026-11-25). Found by the registry audit: the owner's answer 6 of 2026-11-07 no change
> covered (found-defects 76, 95). Tests reserved: t4263–t4265.

## Why

The owner, 2026-11-07: "в.6. а (но вообще-то это надо говорить ПЕРЕД рендером а не в конце блядь... где
логика!??!?)". An event longer than its file holds the last frame and is silent with nothing said; a file shorter than
its own header says the same. `picture-check` (F-EXP-009) covers other things and waits for his two questions, so this
is apart.

## What Changes

- The render window, as it opens, names what will not be rendered as it looks: events that run past the end of their
  file (picture and sound apart) in the rendered range, and files found truncated. Each line has the event's number,
  its time and how much is missing; a click jumps to it. Nothing blocks Start.
- A truncated file found during the render is written to the log with its path and the last whole moment.

## Capabilities

### Modified Capabilities

- `export`: the warning before the start, the log line.
- `ui-reach`: the lines in the window and their jump.

## Impact

- `kt-model` a new pure `lint::past_end` (a module `picture-check` joins), `kt-app` `render_dialog.rs`, `kt-engine`
  export log; no format, no OTIO, no decode.
