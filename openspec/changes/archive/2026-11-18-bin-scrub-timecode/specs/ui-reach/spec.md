## ADDED Requirements

### Requirement: The bin's scrub writes the moment under the cursor
While the pointer scrubs a file card of the media bin, the card SHALL write the moment of the file
under the cursor over the bottom of its thumbnail, in the program's timecode form at the project's
fps, taken from the same hover position that picks the thumbnail. The label SHALL stay inside the
thumbnail, shortening on a narrow card (without the hours, then the frame, then the milliseconds),
SHALL leave with the pointer, and SHALL be laid out once per moment, asking for no decode.
Why: the owner, `v26.10` line 156 — "можно таки отображать там внутри таймкод курсора"; a scrub
without a number says only "roughly here" (F-MED-008).

#### Scenario: The scrub names the moment and follows the cursor
- **WHEN** a 12 s video file is in the bin and the pointer hovers its card at half, then at a quarter of the thumbnail's width, then leaves the card
- **THEN** the thumbnail carries exactly the timecode of the hovered fraction of the file each time, inside the thumbnail, the two differ, and nothing is written after the pointer left
- **PATH** hover a file card in the bin → a dark chip at the bottom of the thumbnail reads "00:00:06.000 F00" → move left → the chip reads an earlier moment as the picture pages back → move off the card → the chip is gone
- **TEST** t3100 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A narrow card shortens the label; a still moment is not laid out again
- **WHEN** the label is laid out for a 120-point card and for the 56-point narrowest card; the pointer rests on one moment for two hover frames
- **THEN** the wide card keeps the full form, the narrow one a shortened part of it that fits; the second frame replays the same laid-out label
- **PATH** narrow the bin column to its least → hover a file card → the chip reads a shorter form ("01:40.500" instead of "00:01:40.500 F12") inside the thumbnail → keep the pointer still → nothing changes
- **TEST** t3101 (`src/apps/kt-app/src/layout_tests.rs`)
