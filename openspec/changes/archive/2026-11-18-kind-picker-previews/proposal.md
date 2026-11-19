# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 17, registry G6): "нет превью генераторов при выборе в
окне, у переходов так же нету … в половине программы нормальный интерфейс (как у эффектов …) а у
половины обрубки". The effects picker is a gallery of cells, each the frame after its click on the
person's own frame (R.54, chain-everywhere). The generator kind picker in the event window and the
transition kind picker in its window were bare lists of names (feature note F-AUT-012).

## What Changes

- The two kind pickers become galleries of cells under their family headings (the generator creation
  menu's groups, the transition families), as wide as the window they open from, as tall as the
  screen allows: a cell per kind with its name under it, the current kind outlined.
- A generator kind's cell is the event's frame at the cursor with that kind at its defaults; a
  transition kind's cell is the cut at the middle of the transition window with that kind. A sound
  generator's cell has no picture: a speaker and its name.
- The cells are rendered by the effect cells' service (`fx_thumbs`), one new candidate form
  (`Swap`): the kind put in place of the event's generator or the transition's kind. Its misclick
  guard, its lane, its cache and its pause while playing apply unchanged.

## Capabilities

- `ui-reach`: ADDED requirement "The generator and transition kind pickers are galleries of the frame
  after the click".

## Impact

`src/apps/kt-app/src/fx_thumbs.rs`, `gen_ui.rs`, `transition_editor.rs`, `fx_editor.rs`,
`morpher_editor.rs`, `fx_registry.rs` (a constant made crate-visible). No catalog key, no format
change. Tests t3108–t3110; live scenario `kind_picker_previews`.
