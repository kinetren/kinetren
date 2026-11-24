## ADDED Requirements

### Requirement: The disk prerender's writer hands its frames over on the card
Where an export on the same device and encoder could hand its frames over by surface, the disk prerender's writer SHALL do
so. Its verdict MAY be kept for the process per device and render node. The frames' key SHALL not change.
Why: the writer opens an encoder per segment and runs at rest, where every core counts (export-into-surface D8).

#### Scenario: A segment by surface
- **WHEN** the prerender writes two segments through the fake VA driver's exported surfaces
- **THEN** no planes are read back, the driver's trial ran once, each moment reads back as the composite within the codec's tolerance, and the keys are those of before
- **TEST** t4133 (`src/libs/kt-engine/tests/render_cache.rs`)
