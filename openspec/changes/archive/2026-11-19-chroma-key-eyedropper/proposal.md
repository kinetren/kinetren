# Proposal

## Why

The owner (`docs/ktprompts/kkt-26.11.01b.txt` line 3, row B2): "greenscreen не имеет пипетки, чтобы
ткнуть в тот же кадр превью или еще где-либо взять цвет с экрана". The key colour was the system
colour picker and three numbers; the screen's green had to be guessed. This is the eyedropper slice of
the planned `chroma-key-upgrade` (B2); the key's quality and automatic analysis (B3) stay there.

## What Changes

- A pipette beside every colour button of the program (the chroma key first; the effects' tints and
  lines, duotone, a transition's colour, a generator's and a title's colours, a composition's colour
  parameter): a click arms it, the cursor is a crosshair and the status bar says to click a colour.
- While armed, the next click anywhere in the program window is taken by one transparent layer: it
  acts on nothing under it (a click on the preview does not toggle playback), reads the screen's
  pixel under the pointer and writes it into the row that armed it, as one history step.
- Esc, or the pipette again, puts it down. Where the screen cannot be read (Wayland) the bell says so.

## Capabilities

- `ui-controls`: ADDED requirement "A colour is taken from the screen with the eyedropper".

## Impact

`kt-app`: new `eyedropper.rs`; `widgets.rs` (`rgb_button`, `color_row`, `color_row_rgba`),
`fx_editor.rs`, `transition_editor.rs`, `pubparam.rs`, `app.rs` (the layer, Esc, the resize zone),
`icons.rs`, five catalogs, `reach_tests.rs` (the layer in the closed list). Tests t3120–t3122; live
`eyedropper`. No model or format change.
