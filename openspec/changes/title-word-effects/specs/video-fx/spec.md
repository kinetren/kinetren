## ADDED Requirements

### Requirement: An isolating mask lets an effect see only the inside
A slot mask in the isolate mode SHALL give the slot only its input inside the coverage (transparent outside) and SHALL
put the slot's result over the input outside the coverage; preview and export SHALL compute it in the one plan.
Why: F-TXT-010 trick 2 — a masked blur that sees the whole frame pulls the neighbours in.

#### Scenario: A blurred word, sharp neighbours
- **WHEN** a Blur slot with an isolating text-range mask on "SALE" is rendered on a title, and the same with the ordinary mix mode
- **THEN** in isolate mode every pixel outside the word's glyph boxes equals the unblurred title, and the blurred word holds no colour of its neighbours' glyphs; in mix mode the word's area contains neighbour colour
- **TEST** t3944 (`src/libs/kt-gpu/tests/parity.rs`)
