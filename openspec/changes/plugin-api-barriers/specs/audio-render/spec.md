# Spec Delta

## MODIFIED Requirements

### Requirement: A model edit does not destroy FX state
On any rebuild of the audio renderer, the chain state SHALL be matched by a composite key (clip id or track id) and a fingerprint of the chain's STRUCTURE (slot kinds — each slot's effect kind, a plugin slot by its id, never by the enum variant all plugin slots share): the same structure → the same processor; parameters arrive smoothed (one-pole ~20 ms); curves and GEOMETRY (entry point, length, handles) arrive as well; buffers are zeroed only on a real seek (`reset`). Structural changes (look-ahead window, pitch engine, geometry of reverb lines) SHALL rebuild the processor without touching the fingerprint. Muting a track (mute, not solo) MUST NOT remove its chains from the graph.
Why: any UI hiccup recreated the FX nodes and tore the reverb tails (defect f); the fingerprint knows only the slot kinds, and without conveying the geometry a trim left the processor with the old anchor — "sound from another event"; a key by track number would die when a neighbor was inserted.

#### Scenario: Rebuilding the graph in the middle of a tail
- **WHEN** during the sounding of a reverb tail another event is edited and the graph is rebuilt
- **THEN** the tail rings on past the event's boundary and decays by −60 dB within `tail()`, and the output after the rebuild is byte-identical to the continuous one
- **TEST** t6 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: A parameter step does not click
- **WHEN** a chain parameter changes in a jump during playback
- **THEN** the sample-to-sample jump does not exceed 1.5× the natural slope of a sine — the parameter arrived through smoothing, and the buffers were not zeroed
- **TEST** t5 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Changing the entry point with the same chain is audible
- **WHEN** the event's `source_in` is shifted (trim, cut, punch-out, re-anchoring of a morph piece) with the same id and the same chain structure
- **THEN** the sounding material shifts by exactly that amount; the anchor is reassigned by a SEAM, the output is continuous, and the reverb rings on
- **TEST** t283 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: A structural change rebuilds the processor
- **WHEN** the look-ahead window of a node changes, the pitch neutral becomes an asset, or the project's pitch engine is switched
- **THEN** the processor is rebuilt (audible as a mini-seek), and the static tail and look-ahead formulas agree with the node for each kind in the registry
- **TEST** t340 (`src/libs/kt-engine/tests/audio_pitch.rs`), t322 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: Mute does not kill the state
- **WHEN** a track with a reverb on an event is muted and unmuted in the middle of the tail
- **THEN** the chain is not recreated, and the tail continues from the same place
- **TEST** t732 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: Another plugin kind in the same slot is another structure
- **WHEN** an EQ in a slot is replaced by a filter, two plugin kinds are swapped in order, and separately a filter's cutoff is edited
- **THEN** the first two give another fingerprint (the processor is rebuilt with the new kind's node), the value edit keeps it (the state survives)
- **TEST** t1057 (`src/libs/kt-fx/tests/chain_kind.rs`)
