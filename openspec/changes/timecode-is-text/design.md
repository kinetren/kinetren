# Design: timecode-is-text

## Context (orch `5986f08e`)

- The kind: `src/plugins/generators-test/src/lib.rs:64` (pixel font, `[235,235,235]` on opaque black, digits 4/5 of the
  frame's width), one row `fps`; `BUILTIN_IDS`; `ORDER` in `genmedia.rs:435`.
- The mode: `TextSource::Timecode`, `count_fps`, `count_from` (`genmedia.rs:130–145`), `title_text_at`.
- An unknown kind opens as a named placeholder (`MissingSlot`, t973b). Without a rule for RETIRED kinds the removal would
  silently turn his timecodes into placeholders: the case the import rule exists for.
- `import_interchange` refuses a current `.kt` (`Error::ProjectFile`, "already a project"), and the app's import dialog
  filters `["otio", "ktproj"]` (`app.rs:11921`).

## D1. The menu

`GeneratorKind::all()` yields, at the place `timecode` had in `ORDER`, a Text title with `source: Timecode`,
`text: ""`, `count_fps` 30 (creation sets the project's rate, as the kind's creation should have), the conversion's look
(D3). Two rows with the tag `text`: the tables already live with five rows of one tag; `address_token()` tells the probe
apart (`gen.new.text.timecode`). `default_for_tag("text")` takes the first, so the plain title stays before it in `ORDER`.
`kind_name` names it by its source.

## D2. Who refuses, who repairs

`project_file::RETIRED_GENERATORS: &[Retired { id: "timecode", since: "<the release>" }]` — one table, used by three:

- **The reader** (`from_text`): on the peeked JSON, before deserializing, a media path whose `gen:` tag is retired or a
  `gen` object whose `kind` is retired gives `ProjectFileError::Retired { version, id }`: "project of format N holds the
  generator `timecode`, removed in <since>; use File → Import older Kinetren project…". Walked anywhere in the tree: root
  events, reels, morpher patches. kt-cli `render`, `export`, `probe` say the same.
- **The import door**: `import_interchange` on a `.kt` with retired kinds repairs the JSON (D3) and reads it normally;
  on one without, "already a project" as today. An `.otio` gets the same repair before its read, whatever its envelope
  version: the kind is not tied to one. The fix is a `Fixup` row in the spirit of R.67, deleted when support goes.
- **Recovery**: a recovery text of an older build goes through the same repair. It is not a file a person could carry to
  the menu, and a refusal there would lose unsaved work.

The document after the import is unnamed (R.70). Where the source is a `.kt`, the hint is `<name>-imported.kt`, so that
one Ctrl+S and Enter do not write over the only old copy.

## D3. The conversion

| Old | New |
|---|---|
| media `gen:timecode[:fps=N]` | `gen:text`; the media id, name and bin place stay |
| event `gen.kind = timecode`, `fps` | Text, `source = timecode`, `text = ""`, `count_from = 0`, `count_fps = fps` |
| `gen_auto` row `fps` | row `count_fps`, the same keys, formula and link |
| links, published parameters and overrides `gen/fps` of that event | `gen/count_fps` |
| the look | fill `[235,235,235,255]`, plate black opaque, `plate_pad` 2 (its ceiling), centre anchor and position, regular face, `size = 0.8 · W / (5.38 · H)` of the project's frame, clamped to the row's 0.005–0.5 |

5.38 em is the width of `00:00:00:00` in the vendored Noto Sans (eight tabular digits and three colons); digits are
tabular, so the running text does not jump. Where the plate does not cover the frame (a portrait project), the import's
notice says that the black background is now a plate around the digits. The notice names the count: "generator
timecode → title with a timecode ×3".

## D4. Class walk — "a kind that is another kind with fewer controls"

| Kind | Is it text drawn apart from the text passport? | Here |
|---|---|---|
| `timecode` | yes, wholly | removed |
| `leader` | a picture with one digit on a sweeping ring, its 2-pop frame-exact | stays (`text-counter-source` D4) |
| `smpte`, `table` | a label inside a test picture | stays |
| `plate`, `progress` | shapes without text | stays |
| the missing-media placeholder | a label for a person, not an edit | stays |
| the bin's scrub timecode, the toolbar's timecode | interface | stays |
| audio `beep`/`tone` beside the SMPTE tone | one synthesizer already | stays |

Also walked: every other door a retired kind can enter by — clipboard (in-process, same build), effect and generator
presets (`presets-everywhere` stores kind ids: its reader asks the same table), the showcase and the tour lessons (no
`timecode` in them; checked), fixtures (none).

## Rejected

- **Keep the kind, hide the row** — the complaint stays in every old project and the code carries two timecodes.
- **Convert in the reader** — R.67: "the reader does not learn old forms".
- **A solid black event under the title** — an extra event and track per timecode; a plate is the title's own means.
