# Proposal: frame-format-variants

> DESIGN, revision 2 (2026-11-24), base `229c694b`: implementation-ready after `shared-events`. The owner answered Q1–Q2
> (A17). Code starts on the orchestrator's go. Tests: t3485–t3492, t4235–t4238.

## Why

The owner took candidate ★15 on 2026-11-01 (`docs/ktprompts/kkt-26.11.01-candidates.md`, registry
`kkt-26.11.01-index.md` P1): "Сделал 16:9, нужен ещё 9:16 и квадрат. Хочу варианты кадра одного
таймлайна: у каждого своя поправка геометрии события поверх общей, safe-area сетки, в рендере «все
форматы сразу»."

Today a project has one raster (`ProjectSettings.width/height`); a vertical version is a second project
or a manual composition per format. Premiere's Auto Reframe and FCP's Smart Conform make a COPY, after
which the edits diverge (note F-PRJ-006, trick 1) — exactly what "варианты кадра одного таймлайна"
excludes.

## What Changes

- **Formats of the project**: name, size, the rule for events that follow the project, a safe-area preset. The cut, the sound,
  the effects and the tracks are shared.
- **Per-event corrections over the common geometry**: an offset, a zoom factor and a turn (a rule, a crop or a row's own curve
  where wanted), and "hidden in this format". The master's placement and its curves go on living under them.
- **A format is a derived project**: one function gives the player, the render and kt-cli the project of a format; the engine
  learns nothing new but a field of the key (invariant 7).
- **Titles reflow**: they keep their share of the short side and break lines at the narrow width.
- **A chooser on the preview**, a Formats section in the project settings, safe areas of the platforms.
- **Render all formats** from one press; kt-cli `--format`.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-plan`: a format as a derived project with corrections over the common geometry; keys by format.
- `export`: an export renders a chosen format; "all formats" queues one export per format.
- `project-format`: formats and corrections raise the file by content; OTIO export flattens to the master.
- `ui-reach`: the format switch, the corrections by gesture, reset, safe areas.
- `generators`: a title and height-sized rows in a format of another shape.

## Impact

- kt-model: `Project.formats`, `ClipEvent.framing`, `Track.format_masks`, five commands, `Project::in_format`; the next file
  version by content.
- kt-text `wrap_lines`, kt-media `TitleWrap`; kt-engine: the key's field only; kt-cli `--format`.
- kt-app: `push_project_to_player`, the chooser, `geomctl.rs`, the Geometry form's section, `settings_window.rs`, the render
  queue, i18n ×5.
- kt-otio: `metadata["kt"]`; the planned row in "What OTIO export loses". Idle: nothing runs at rest. Note: F-PRJ-006.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20.txt` lines 26–27, A17): "Q26 9:16: «Каждое по своей геометрии (Recommended)»" — Q1 (a);
"Q27 safe areas: «Пресеты платформ (Recommended)»" — Q2 (a). Read again on 2026-11-24: option (a) promises that "a title
reflows to the narrow width", which the code does not do (design D3 builds it), and his ★15 asks for a correction OVER the
common geometry, not a replacement (design D2). No open question.

The questions as asked:

**Q1. How does an event WITHOUT an override sit in a 9:16 format?** Your words: "у каждого своя поправка
геометрии события поверх общей". Today the conform rule fits/fills each event's SOURCE into the raster.
- (a) **Each event is re-conformed into the format's raster with its master geometry** (*recommended*: a
  full-frame shot fills the vertical by its own centre, a PiP keeps its corner, a title reflows to the
  narrow width; you fix only what looks wrong).
- (b) The format is a crop of the finished master frame (its centre); an override moves an event inside
  that crop. Simpler to reason about, but a title or lower third is cut by the crop.

**Q2. Which safe-area overlays?** Your words: "safe-area сетки". Platforms' UI covers the right column and
the bottom of a vertical, differently per platform.
- (a) **Presets for YouTube Shorts, Instagram Reels, TikTok and a plain 90 %/80 % title-safe, one chosen
  per format, drawn in the preview only** (*recommended*).
- (b) Only the generic title/action-safe rectangles.
