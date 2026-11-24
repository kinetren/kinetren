# Proposal: title-word-effects

> DESIGN, revision 3 (2026-11-25), base `fc60a5d9`: implementation-ready; needs nothing of `magic-wand-mask` (design D8). Decided by B8. Note F-TXT-010.
> Tests: t3942–t3946, t4246.

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
- **Many words from the start** (the owner: "что будет если выделено по отдельности 20 слов"): one text-range mask holds
  a LIST of ranges; Ctrl+selecting another word and "Add to this mask" adds a range to the mask the slot has. Twenty
  words picked one by one are one mask, one coverage raster and one isolate pass per slot — the cost does not grow with
  the number of words. Words that need DIFFERENT effects are different slots, each with its own mask; all of a title's
  masks share one glyph rasterization per frame (one id map of up to 255 groups), so ten masks cost one layout.
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

Decided (2026-11-22, B8, `kkt-26.11.20b.txt` line 20): "п.1. но это надо сразу учесть что будет если выделено по
отдельности 20 слов например" — the word is selected in the title's text field and the range follows edits; many words
selected one by one are designed for from the start (above). No open question.
