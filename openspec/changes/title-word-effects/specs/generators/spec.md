## ADDED Requirements

### Requirement: A range of a title's text is a mask that moves with the text
A mask of the text-range shape on a title event SHALL cover exactly the glyphs of its character range (or of every
occurrence of its text) as the title lays them out at that frame — crawl, animators and counters included — rasterized
by the title's own layout; edits of the title's text SHALL move the range with the edit, and a fully deleted range
SHALL leave an empty mask named as deleted.
Why: the owner, v26.10 TD1 — "на одно слово натравить эффекты".

#### Scenario: The word's coverage rides the crawl
- **WHEN** a crawling title "BIG SALE TODAY" has a text-range mask on "SALE" and is rendered at 50 frames; then "VERY " is typed before "BIG"
- **THEN** at every frame the coverage equals the alpha of the title rendered with only "SALE" visible (exactly); after the edit the range still covers "SALE"
- **TEST** t3942 (`src/libs/kt-text/tests/range_coverage.rs`), t3943 (`src/libs/kt-model/tests/text_range.rs`)
