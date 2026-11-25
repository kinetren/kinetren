## ADDED Requirements

### Requirement: A range of a title's text is a mask that moves with the text
A mask of the text-range shape on a title event SHALL cover exactly the pixels whose ink is of the glyphs of its character range (or of every
occurrence of its text) as the title lays them out at that frame — the crawl, the reveal and a shown value included — by the title's own layout, a pixel
belonging whole to the glyph with the most ink in it; edits of the title's text SHALL move the range with the edit, and a fully deleted range
SHALL leave an empty mask named as deleted.
Why: the owner, v26.10 TD1 — "на одно слово натравить эффекты".

#### Scenario: The word's coverage rides the crawl
- **WHEN** a crawling title "BIG SALE TODAY" has a text-range mask on "SALE" and is rendered at 50 frames; then "VERY " is typed before "BIG"
- **THEN** at every frame the coverage is 1 on every pixel with ink of "SALE" alone and 0 on every pixel without it, and no edge pixel is shared between it and its neighbours; after the edit the range still covers "SALE"
- **TEST** t3942 (`src/libs/kt-text/tests/range_coverage.rs`), t3943 (`src/libs/kt-model/tests/text_range.rs`)

### Requirement: Many words in one mask, one rasterization for all of a title's masks
A text-range mask SHALL hold a list of spans; adding a word selected later SHALL add a span to the same mask; every
text-range mask of a title SHALL be computed from one layout of the title per frame (an id map of mask groups), so the
cost of a mask does not grow with its number of spans and the cost of several masks is one layout plus a select each.
Why: the owner, kkt-26.11.20b B8 — "что будет если выделено по отдельности 20 слов например".

#### Scenario: Twenty words picked one by one
- **WHEN** twenty words of a crawl are added to one isolating Blur mask one at a time, and another three words to a second slot's mask (a Glow)
- **THEN** the Blur mask lists twenty spans and its coverage equals the union of the twenty words' glyphs at every frame; the title is laid out once per frame for both masks (the trace counts one layout); the layout is made once and the cost with twenty spans is within 1.2× of one span
- **TEST** t4246 (`src/libs/kt-text/tests/range_coverage.rs`)
