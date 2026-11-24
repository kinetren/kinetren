## ADDED Requirements

### Requirement: The disk prerender's writer converts on the device
The disk prerender SHALL hand its segment writer the encoder's planes made by the export's conversion pass
on the composing device (the CPU reference where there is no pass), not RGBA converted by the processor.
The durable key SHALL not change.
Why: export-host-copy D4 — the prerender runs at rest, where every core counts (owner: "the program at rest
costs nothing"); it read back RGBA and converted it by swscale per moment.

#### Scenario: A segment is written from planes
- **WHEN** the prerender writes three moments to a segment through the test codec
- **THEN** the writer receives planes of the encoder's layout, no processor conversion runs, each moment reads back within the codec's tolerance, and the keys are those of before
- **TEST** t3317 (`src/libs/kt-engine/tests/render_cache.rs`)
