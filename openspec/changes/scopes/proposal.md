# Proposal: scopes

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3400–t3414.

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

- **Five instruments of one frame**: luma waveform, RGB parade, vectorscope (with the skin-tone
  line at 123°, as Vegas draws it), histogram (luma and R/G/B overlaid) and false colour (the frame
  painted by exposure zones over the preview). The scale is the honest one for our 8-bit path:
  0–255 and 0–100 %; no 10-bit or mV scale is offered (the frame is RGBA8 after decode).
- **They measure what the preview presented, through no second composition.** The executor's
  `present` gains a *scope tap*: when asked, after the composite it runs one downscale pass into a
  small analysis texture (at most 512 px wide, aspect kept) and starts an asynchronous readback of it,
  never waited for in the frame — the same way it already refreshes the 32×18 ambient glow. A scope
  thread turns the bytes into counts and an image; the window draws a texture. Invariant 7 holds:
  the tap reads the finished composite of the one `frame_plan`.
- **On a pause the scope can measure the export frame** (owner question Q2): once per paused moment
  the full-resolution composite of the same plan is made on the offline route, measured, and the
  scope says "1/1" next to its title; while playing it says the preview's quality ("1/4").
- **Clipping is never hidden**: no smoothing of the trace is offered, and every instrument marks
  the real excursions (the top and bottom rows of the waveform and parade light where any pixel
  reached 0 or 255) — the lesson of Resolve's Low Pass Filter (note F-CLR-005, trick 2).
- **A Scopes window** from View (owner question Q1): one to four instruments side by side, the
  choice and layout kept in `UiState` (outside undo, the default not written).
- **At rest it costs nothing**: the tap is asked only while the window is open AND the presented
  frame key changed; a pause with the mouse moving over the scope makes no pass, no readback and no
  scope work (`kt_time::work` rows, live `idle`).

## Capabilities

### New Capabilities

- `scopes`: the instruments of one frame — what they measure, the tap on the presented composite,
  the scales, the window, the cost at rest.

### Modified Capabilities

- `ui-reach`: the Scopes window is a row of the `WINDOWS` registry with its reason and is reached
  from View and from the preview's right-click menu.

## Impact

- kt-engine: `PresentOptions.scope: Option<ScopeTap>`; `Player::measure_paused` (one offline
  composite of the paused moment).
- kt-gpu: the tap pass (a box downscale, integer, `Rgba8Uint`) and a two-deep readback ring of the
  analysis texture; no atomics, no compute (works on the GL device and llvmpipe).
- kt-app: `scopes.rs` (counts → image, on its own thread), the window, View item, i18n ×5,
  `UiState.scopes` (serde default, not written at default — the `.kt` file does not move).
- Project format: unchanged for a project without the field; the field is UI state, so the file
  version does not rise (`project_file::version_for` unchanged). OTIO: `UiState` already travels in
  `metadata["kt"].ui`; nothing is lost that a foreign editor could show.
- Feature note: F-CLR-005 updated with the current code (the preview no longer reads back).

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20` A8, his Q6–Q7): "Окно, кадр превью" — Q1 (a), a Scopes window; Q2 (a), the
preview frame while playing, the full-size frame once on a pause.

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
