## ADDED Requirements

### Requirement: The pitch's echo choice in the file — format 3 by content
The pitch's echo choice SHALL be written in the slot's file form only when it is "rings on"
(`"echo_before": "ring"`). The `.kt` file SHALL be written as format 3 when any effect slot of the
document (an event's, a track's, a morpher patch's, in every reel) rings on, and as format 1 or 2
otherwise byte for byte as before; the reader SHALL read formats 1–3, and a build that reads up to 2
refuses a format-3 file as "written by a newer Kinetren". The OTIO door SHALL carry the row in the
slot's `metadata["kt"]` form.
Why: an older build ignores a slot row its passport does not name — it would open the project and
silently cut the echo the owner chose to ring.

#### Scenario: Rings on is format 3; cut is the former file
- **WHEN** a project whose event, track and morpher patch carry a pitch with "rings on" is written and read; the same with "cut"; the text of format 3 is given to a reader that reads up to 2; the project goes through the OTIO door and back
- **THEN** the first file is format 3 with `"echo_before": "ring"` on each slot and reads back equal; the second is format 1 and has no `echo_before`; the older reader refuses with the version in its error; the OTIO round trip keeps "rings on"
- **TEST** t1336 (`src/libs/kt-model/tests/pitch_tail_format.rs`)
