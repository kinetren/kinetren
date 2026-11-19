## ADDED Requirements

### Requirement: The selected keys of a curve move together
The automation graph SHALL treat the ends of every selected segment as selected keys, together with
the keys taken in by Shift+click on a key; Shift+click on a selected key SHALL take it out. Dragging
any selected key SHALL move every selected key by the same time shift — the grabbed key on the frame
grid, the others keeping their distances — and by the same shift on the graph's value scale, each
value held inside its parameter's range. The moved block SHALL NOT pass an unselected key and SHALL NOT
leave the visible axis. The drag SHALL be one history step and the selection SHALL follow its keys. A
sweep over empty space with Shift (as with Ctrl) SHALL add to the selection; a plain press on a key
outside the selection SHALL drop it. A key drag SHALL keep the target and the zone of its press point.
Why: `v26.10` A1 ("выделения ключей и перемещения их") and his answer U9 of kkt-26.11.18b: the keys
of the selected segments are the key selection, "а еще можно шифтом прокликать ключи нужные или шифтом
выделить еще несколько участков".

#### Scenario: Two segments' keys move as one block and stop before a neighbour
- **WHEN** a curve of five keys has its second segment selected by Ctrl+click; its left key is dragged right and up past the plot's top; then dragged far right
- **THEN** both keys of the segment move by the same time (the grabbed one on the frame grid) and both values rise; the other three keys are untouched; the drag is one undo step and the segment stays selected; the far drag stops the block before the next key with its inner distance kept
- **PATH** FX window → the curve graph → Ctrl+click the second segment, it tints → press its left key and pull right and up → both its keys slide together, white → pull far right → the block stops before the fourth key
- **TEST** t3114 (`src/apps/kt-app/src/segfield_tests.rs`)

#### Scenario: Shift takes keys and segments in and out
- **WHEN** one segment is selected; Shift+click on the key after it, then Shift+click on the segment's right key; a key is dragged; a plain sweep over the first segment, a Shift sweep over the last; a plain click on a key outside
- **THEN** the keys selected are 1, 2, 3, then 1, 3; the drag moves 1 and 3 together and leaves 2; the sweeps leave the first segment, then the first and the last; the plain click leaves nothing selected
- **PATH** Ctrl+click a segment → Shift+click the next key, it turns white → Shift+click the segment's right key, it turns back to colour → pull a white key, both whites move → sweep empty space over the first segment → Shift+sweep over the last, both tinted → click another key, the tint goes off
- **TEST** t3115 (`src/apps/kt-app/src/segfield_tests.rs`), live scenario `src/scripts/live/curve_keys.py` (`./scripts/live-check.sh`)
