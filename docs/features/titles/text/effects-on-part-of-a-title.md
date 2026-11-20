---
id: F-TXT-010
title: Effects on a part of a title — a blur on one word of a crawl
status: candidate
priority: C
cost: M
category: titles/text
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-text, kt-fx, kt-engine, kt-app]
invariants: [7, 11, 12]
ui_rules: [17, 24]
round: null
point: null
tests: []
depends_on: [F-TXT-006, F-MSK-001]
checked: 2026-11-20
---

# F-TXT-010 — Effects on a part of a title — a blur on one word of a crawl

## Essence

A title is one event with one effect chain: a blur blurs all of it. The owner's example of "a complex system of
objects" is a running line where ONE word is blurred (or glows, or shakes) while the rest stays sharp, and the word
keeps its effect while the line crawls. The part of the text must be addressable like an object: an effect is put
on it, not on a rectangle of the screen that the word leaves a moment later.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26 | Text Animator with a Range Selector; animator properties include Blur, Fill, Scale, Position | an animator applies its properties to the characters its range selects (characters, words, lines; by index or by expression); the blur is per glyph group [NOT VERIFIED live] |
| Premiere | 26 | Essential Graphics text: styles per selected characters | colour, size, font per selection; no effect per range — an effect applies to the whole graphic layer [NOT VERIFIED live] |
| DaVinci Resolve | 21 | Fusion Text+ with Character Level Styling and Follower | per-character style and animation offsets; an effect on one word needs a separate Text+ or a mask in Fusion [NOT VERIFIED live] |
| Final Cut Pro | 12 | Motion text behaviours, per-glyph adjustments in Motion | glyph-level transforms in Motion, published into FCP [NOT VERIFIED live] |
| VEGAS Pro | 2026 | Titles & Text | style per selection; effects on the whole event [NOT VERIFIED live] |

## The trick behind it

**1. The range is a set of glyphs, not a place on the screen.** AE's selector picks characters by index (or by
word), and every frame the layout says where those glyphs are. A rectangle mask over the word slides off it as the
line crawls; a selection of glyphs cannot.

**2. An effect on a part must see only that part.** Blurring the whole title and keeping the blur inside the word's
area pulls the neighbours' pixels into the word. The part is lifted out (its glyphs on a layer of their own), the
effect runs on it, and it is put back over the rest — AE's per-character blur is exactly that.

## Pros

- One effect on one word with no second title event and no hand-made mask that drifts.
- The same mechanism gives "this word glows", "this word shakes", "the price in red with a glow".

## Cons

- The range must survive text edits (typing before the word shifts the indices).
- Each ranged effect adds a layer to the title's composite.

## What users say

The owner, `docs/ktprompts/v26.10.txt` line 159: "написать в генераторе текста некоторую бегущую строчку и на одно
слово натравить эффекты, ну например, блюр. это маленький пример относящийся к нашей системе".

NO QUOTES: no web access in this spec session (2026-11-20). The search would be the AE and Premiere forums for "blur
one word" and "effect on part of text"; the editor rows are [NOT VERIFIED] for the same reason.

## How it fits Kinotrain

### What already exists

- Titles are a generator kind with values and curves on the event (`src/libs/kt-model/src/genmedia.rs`), rasterized
  by `src/libs/kt-text` with a layout per frame (crawl offsets, animators of F-TXT-006).
- Masks on a slot (shape-masks, `src/libs/kt-model/src/mask.rs`): a slot's result is mixed with its dry input by the
  mask's coverage; the wand (change `magic-wand-mask`) adds a raster-made shape.

### What is missing

- A mask shape that is the coverage of a range of the title's own glyphs; a slot mask mode that lifts the inside
  out before the effect.

### What hinders

- The coverage must come from the same layout as the title's picture at that frame, or the word and its effect part.

### Which invariants it touches

- **7, 11** — the coverage is computed by the title's own layout in the one plan; the same bytes every run.
- **12** — nothing at rest.
- CAPABILITY: openspec/specs/generators/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | kt-text (range coverage), kt-model (the mask shape, the isolate mode), kt-fx/kt-gpu (the isolate pass), kt-app (pick a range in the text field) |
| Project format | yes, by content |
| Tests | a blurred word stays on the word through a crawl; the neighbours stay sharp; edits keep the range |
| Depends on | F-TXT-006 (animators), F-MSK-001 (masks) |
| Breaks compatibility | no |

## Verdict

Take it as two small pieces on existing machinery: a mask shape "text range" (the glyphs of a range of the event's
own title, laid out per frame) and a slot mask mode "isolate" (the effect sees only the inside, the result is put
back over the outside). An effect on one word is then an ordinary slot with that mask.

## Sources

- `docs/ktprompts/v26.10.txt` line 159 (TD1); registry `v26.10-index.md` TD1, TD2.
