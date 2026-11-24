## ADDED Requirements

### Requirement: A property may follow a measure of an event's material
A link SHALL be able to read a channel of an event's material (level, bass, onset, beat, bar, motion and its place, cut,
brightness) at the event's material moment through its trim, reverse and warp; outside the event the channel SHALL be 0.
The compiled curve SHALL be keyed by the measure's content and the person's correction. An export and kt-cli SHALL compute
every measure their links read before the first frame, so that no byte depends on a background job.
Why: one system of links (R.61); invariant 11.

#### Scenario: A zoom on the beat
- **WHEN** a Geometry zoom of one event links to the `beat` of a music event that is trimmed, then reversed, then at half speed
- **THEN** the zoom peaks on the beats as they are heard in each case, within one frame
- **TEST** t4226 (`src/libs/kt-engine/tests/measure_links.rs`)

#### Scenario: A measure that arrives late and an export that does not wait
- **WHEN** the preview shows the moment before the measure is computed, the measure lands, and the same project is exported in a process with an empty cache folder
- **THEN** only the moments that read the link are composed again; the export's frames equal those of a process that had the measure
- **TEST** t4227 (`src/libs/kt-engine/tests/measure_links.rs`)
