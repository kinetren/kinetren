# Proposal: title-word-effects

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` TD1 (partial, later). Note F-TXT-010.
> Tests reserved: t3942–t3947.

## Why

The owner, `v26.10` line 159: "хотелось бы НАПРИМЕР написать в генераторе текста некоторую бегущую строчку и на одно
слово натравить эффекты, ну например, блюр. это маленький пример относящийся к нашей системе." Today a title is one
event with one chain: a blur blurs the whole line; a rectangle mask over a word slides off it as the line crawls,
and a masked blur pulls the neighbours' pixels into the word.

## What Changes

- **A mask shape "text range"** on a title event's slots: the coverage of the glyphs of a range of the title's own
  text, laid out by the title's own layout at every frame — it moves with the crawl, the typewriter and every
  animator. The range is picked by selecting the word in the title's text field ("Mask this text" in the field's
  right-click menu); an option "every occurrence" takes every match of the selected text.
- **A slot mask mode "isolate"**: the effect sees only the inside (the input times the coverage, transparent
  elsewhere), and its result is put over the outside (the input times one minus the coverage). A blur on one word
  then blurs that word's glyphs and nothing of their neighbours.
- **The range survives edits**: it is stored as character offsets plus the selected text; typing elsewhere moves the
  offsets with the edit (the field knows the edit); an edit inside the range grows or shrinks it; when the range's
  text is deleted entirely the mask is kept empty and named as such in the slot's mask list.
- One effect on a word is then an ordinary slot with its curves, presets and copy — nothing new to learn.

## Capabilities

### Modified Capabilities

- `generators`: a title's text range coverage, laid out per frame.
- `video-fx`: the isolate mask mode.
- `ui-reach`: "Mask this text" from the title's text field; the range in the mask list.
- `project-format`: the text-range shape and the isolate mode by content.

## Impact

- kt-text: `range_coverage(layout, from, to) -> mask raster` from the same shaped glyph runs the title draws.
- kt-model: `MaskShape::TextRange { from, to, text, every }` (legal only on a title event's slots and picture masks);
  `MaskMix::Isolate` beside the existing mix; offsets moved by the title's text edit command.
- kt-engine / kt-gpu: the isolate op in the plan (`MaskOpKind::Isolate { from }`: premultiply the slot's input by the
  coverage before the slot, then `over` the outside after it); the coverage uploaded as the mask raster.
- kt-app: the field's right-click item, the mask list row ("text: “SALE”"), i18n ×5.
- Format: the next free `.kt` version by content. OTIO: in `metadata["kt"]` with the other masks (shape-masks row).
- Idle: the coverage is computed with the title's raster, once per frame shown; nothing at rest.

## Questions to the owner

**Q1. How is the word chosen?** Your words: "на одно слово натравить эффекты". A title's text changes while it is
edited; a range must not slide onto another word.
- (a) **Select the word in the title's text field → right click → "Mask this text"; the range follows your edits;
  "every occurrence" takes all matches** (*recommended*: you point at the word itself).
- (b) Type a word in the mask's own field; every occurrence of it is masked (no selection, but a renamed word loses
  its mask).
