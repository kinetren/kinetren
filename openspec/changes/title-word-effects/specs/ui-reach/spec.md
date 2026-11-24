## ADDED Requirements

### Requirement: "Mask this text" from the title's own text field
Selecting characters in a title's text field and choosing "Mask this text" (or "… every occurrence") from its
right-click menu SHALL add an isolating text-range mask to the selected slot (or the event's picture) in one undo
step, named by the text, selected in the mask list, with its glyphs outlined on the preview while selected.
Why: the owner's TD1; Q1 (a).

#### Scenario: Blur one word of a crawl by mouse
- **WHEN** a Blur is added to a crawling title, "SALE" is selected in the text field, "Mask this text" is chosen, and the project plays
- **THEN** only "SALE" is blurred as it crawls; the mask list shows "text: “SALE”"; one Undo removes the mask
- **PATH** double-click the title → effects window → add "Blur" → the whole line blurs → in the title's text field select "SALE" → right click → "Mask this text" → only "SALE" stays blurred, its glyphs outlined on the preview → Space → the blurred word crawls with the line → Ctrl+Z → the whole line blurs again
- **TEST** t3945 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `word_blur`

#### Scenario: Add words one by one to the same mask
- **WHEN** after masking "SALE" the person Ctrl+selects "TODAY" and chooses "Add to this mask", then a third word
- **THEN** the mask list row reads "text: 3 pieces" and all three words are blurred as the line crawls; one Undo removes the last word only
- **PATH** in the title's text field Ctrl+select "TODAY" → right click → "Add to this mask" → "TODAY" blurs too → repeat for "NOW" → the mask row reads "text: 3 pieces" → Ctrl+Z → "NOW" is sharp again
- **TEST** t3945 (`src/apps/kt-app/src/reach_tests.rs`)
