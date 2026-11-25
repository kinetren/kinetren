# Proposal: scopes

> DESIGN, revision 2 (2026-11-24), base `229c694b`: implementation-ready. The owner answered Q1–Q2 (A8). Code starts on
> the orchestrator's go. Tests: t3400–t3405, t3410–t3412.

## Why

The owner took candidate ★8 on 2026-11-01 (`docs/ktprompts/kkt-26.11.01-candidates.md`, registry
`kkt-26.11.01-index.md` P1; `docs/user-feedback.md`, "The owner's follow-up (2026-11-01)" (7)):
"Цвет кручу на глаз на некалиброванном мониторе. Нужны waveform, RGB-parade, вектороскоп,
гистограмма и false color по кадру превью или по событию." Today the program has no instrument at
all (the one `histogram` in `src/` is the test diagnostics of `kt_gpu::assert_frames_close`), so
every colour effect (ColorGrade, Duotone, the camera ISP's white balance) and every question "is
this source already clipping?" is answered by eye. Note F-CLR-005 (`docs/features/color/scopes/`)
did the reconnaissance; it names scopes the first item of the colour branch, because they change
nothing in the picture.

## What Changes

- **Five instruments of one frame**: luma waveform, RGB parade, vectorscope (with the skin-tone line at 123°), histogram
  and false colour. The scale is the honest one for our 8-bit path: 0–255 and 0–100 %.
- **They measure what the preview shows, with no second composition**: the presented composite is copied off the device
  without the frame waiting; where the preview shows bytes (a GL window, a slow chain) the bytes are measured.
- **On a pause the full frame is measured once**, on the offline route of the effect cells; the window says the size.
- **Every pixel is counted**, with no smoothing: a clipped pixel lights its mark.
- **False colour is painted by the display pass**, in step with the frame.
- **A Scopes window** from View and the preview's menu, its instruments remembered per person.
- **At rest it costs nothing** (`Work::ScopeTap`, `Work::ScopeCount`, live `idle`).

## Capabilities

### New Capabilities

- `scopes`: the instruments of one frame — what they measure, the tap on the presented composite,
  the scales, the window, the cost at rest.

### Modified Capabilities

- `ui-reach`: the Scopes window is a row of the `WINDOWS` registry with its reason and is reached
  from View and from the preview's right-click menu.

## Impact

- kt-engine: `PresentOptions.tap`/`false_colour`, `VideoGpuExec::take_tap`, the compose thread's hand-on.
- kt-gpu: two map buffers, the copy in the two presents, `display.wgsl` mode 1.
- kt-app: `scopes.rs`, `SceneCtx::frame` (shared with the picker's cells), the window, `Workspace.scopes`, i18n ×5.
- Project format, OTIO: nothing. Feature note: F-CLR-005.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20.txt` line 13, A8): "Q6/Q7 scopes: «Окно, кадр превью (Recommended)»" — Q1 (a), a
Scopes window; Q2 (a), the preview frame while playing, the full-size frame once on a pause. Read again on 2026-11-24
against the options as asked: both are option (a), nothing else is said. His ★8 also names "или по событию": the Viewer
(fx-graph, built) shows the picture after any node of an event, and the scope measures what is shown. No open question.

The questions as asked:

**Q1. Where do the scopes live?** Your words (candidate ★8): "Нужны waveform, RGB-parade,
вектороскоп, гистограмма и false color по кадру превью или по событию." Your rule R.63 (UI rule
22): "I am putting a ban on any agents producing any additional windows without an obvious purpose."
Today: there is no place; the meters column is ~70 pt wide, the preview area holds only the picture.
- (a) **A Scopes window** (Resolve's way): resizable, can sit on a second screen, 1–4 instruments.
  Costs one registry row; the picture keeps its size. *Recommended* — a vectorscope is square and
  needs room the existing places do not have.
- (b) **Inside the preview area** (FCP's way): the preview splits into picture + instruments; no new
  window, but the picture shrinks while they are shown.
- (c) **In the meters column**: one small instrument at a time under the meters; nothing new opens,
  but a waveform 70–200 pt wide hardly reads.

**Q2. Which frame do they measure?** Today the preview is composed at its quality (default 1/2,
640×360 on a 1280×720 project); the export composes at full size, so a fine clipped highlight can
exist in the export and not in the preview.
- (a) **The preview while playing, the full-size frame once on a pause** (*recommended*): one extra
  composite per paused moment, then nothing; the title says "1/4" or "1/1".
- (b) Always the preview frame: zero extra work, readings at preview resolution.
- (c) Always full size: a second composition every played frame — it halves the playable effect load.
