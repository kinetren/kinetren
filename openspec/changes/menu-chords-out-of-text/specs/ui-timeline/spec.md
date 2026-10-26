## ADDED Requirements

### Requirement: A menu label never carries its key; a chord is shown only where it holds
A menu item's translated label SHALL NOT contain its key or gesture. The key SHALL be drawn in the
chord column (`widgets::menu_item`), and a gesture there as a translated chord text (like
"Enter, 2×click"). An item SHALL show a chord only where pressing it in that context does exactly
what the item does. Paste from a context menu pastes at the click, so it shows no `Ctrl+V`. Delete
morpher shows `Del` only when that morpher is the whole selection.
Why: `i18n/README.md` rule 3; five `[tl.menu]` labels carried "(Ctrl+C)", "(Ctrl+V)", "(Del)" and
"(Shift+double click)" in the text, and two of those promises were false where they stood (owner,
2026-10-26: do it, and put the gesture in the chord column).

#### Scenario: No menu label carries a key, and the chords shown are true
- **WHEN** every `[tl.menu]` and `[menu.*]` value (except the `*_accel` chord texts) is read in each of the five languages; the event menu and the empty-space menu with a region are opened by a right click; the morpher Delete chord is asked for with that morpher alone in the selection and with a second morpher selected
- **THEN** no label contains `Ctrl+`, `Del`, `Supr`, `Shift+` or a key in parentheses; Copy shows `Ctrl+C`, Region over this event shows the translated "Shift+2×click", Delete clips in the region shows `Del`, both Paste items show no chord; Delete morpher shows `Del` only when it is alone in the selection
- **PATH** right-click a clip → the menu: "Copy" with Ctrl+C on the right, "Paste" with nothing on the right, "Region over this event" with Shift+2×click on the right → Esc → draw a region, right-click empty track space → "Delete clips in the region" with Del on the right
- **TEST** t871 (`src/apps/kt-app/src/reach_tests.rs`)
