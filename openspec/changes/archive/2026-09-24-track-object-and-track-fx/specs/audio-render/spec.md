# Spec Delta

## ADDED Requirements

### Requirement: Track stage — sum, chain, level, mute/solo; without need, the path stays the same
An audio track with a non-empty chain, a level ≠ 0 dB, or silencing (mute / not in the solo set) SHALL have a stage: the sum of its events with the same weights (fade × overlap × event gain) →
the track chain (a streaming processor without the clip shape: no source, no length, no warp) →
× level → × 0 when silenced → into the output. A track without a stage SHALL be summed into the output
in the former order of addition — byte for byte. Silencing is a multiplier, not removal from the graph:
the FX state of the events of a muted track stays alive. Solo SHALL be a set of tracks within a
kind; clearing solo returns the former mutes. The FX state key SHALL be composite:
an event or a track; insertion, removal and reordering of tracks MUST NOT touch the state of neighbours.
The track chain SHALL receive lookahead like the event chain (the summing window
is extended forward, the output is aligned with the input) and ring out as a tail past the last event
(the input is silence). Mute/solo are project content and affect the export.
Why: reordering f32 additions would shift GOLDEN_AUDIO; removal from the graph would wipe the
state (invariant 3, defects b/f of the old engine); the key "(kind, number)" would die on
inserting a track; the owner's decision in R.62 — mute and solo in the project.

#### Scenario: Without a stage — the bytes are as before; level, mute, and solo are exact
- **WHEN** a project without track properties is rendered; then a level of −6 dB on a track; then mute on a track with a reverb on an event and clearing mute in the middle of the tail; then solo of two of the three tracks
- **THEN** t2/t3/`GOLDEN_AUDIO` byte for byte; −6 dB is applied exactly after the weights and the event gain; under mute — the track is silent, after clearing the reverb tail continues without a cold start; in solo exactly two sound, clearing solo returns the former mutes
- **TEST** t732 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: A track's chain on the sum, look-ahead, and tail
- **WHEN** a reverb is on a track of ten lines; a limiter with lookahead is on the track; the last event has ended
- **THEN** the tail extends across the seams (as t449), the output is aligned with the input (shift 0 relative to a track without a chain), the tail rings out past the last event; the cost of the window is measured and does not grow with the size of the project
- **TEST** t733 (`src/libs/kt-engine/tests/track_fx.rs`)

#### Scenario: The track's state key survives reorderings
- **WHEN** in the middle of the tail of a track chain a track is inserted above, then the tracks are reordered, then a track is removed
- **THEN** the state of the chain below is untouched (the output is continuous); after the removal the state is discarded
- **TEST** t734 (`src/libs/kt-engine/tests/track_fx.rs`)

## MODIFIED Requirements

### Requirement: A model edit does not destroy FX state
On any rebuild of the audio renderer the state of the chains SHALL be reconciled by the composite key
(clip id or track id) and the fingerprint of the chain STRUCTURE (the kinds of slots): the same structure → the
same processor, the parameters arrive smoothly (one-pole ~20 ms), curves and the GEOMETRY (entry point, length, handles) also arrive; buffers are zeroed only on a real seek (`reset`).
Structural changes (the lookahead window, the pitch engine, the geometry of the reverb lines) SHALL
rebuild the processor without touching the fingerprint. Silencing a track (mute, not-solo) MUST NOT
remove its chains from the graph.
Why: any twitch of the UI was recreating the FX nodes and tearing reverb tails (defect f); the fingerprint
knows only the kinds of slots, and without carrying the geometry a trim left the processor with the old anchor —
"sound from another event"; a key by track number would die when a neighbour is inserted.

#### Scenario: Graph rebuild in the middle of a tail
- **WHEN** while a reverb tail is sounding, another event is edited and the graph is rebuilt
- **THEN** the tail rings out past the event boundary, decays −60 dB within `tail()`, and the output after the rebuild is byte-identical to a continuous one
- **TEST** t6 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: A parameter step does not click
- **WHEN** a chain parameter changes by a jump during playback
- **THEN** the sample-to-sample jump does not exceed 1.5× the natural slope of the sine — the parameter arrived through smoothing, the buffers were not zeroed
- **TEST** t5 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Changing the entry point with the same chain is audible
- **WHEN** the `source_in` of an event is shifted (trim, cut, punch-out, re-anchoring of a morph piece) with the same id and the same chain structure
- **THEN** the material that sounds is shifted by exactly that amount; the anchor is repositioned with a SEAM, the output is continuous, the reverb rings out
- **TEST** t283 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: A structural change rebuilds the processor
- **WHEN** the lookahead window of the node changes, the pitch neutral becomes an asset, or the project's pitch engine is switched
- **THEN** the processor is rebuilt (heard as a mini-seek), and the static tail and lookahead formulas agree with the node for each kind of registry
- **TEST** t340 (`src/libs/kt-engine/tests/audio_pitch.rs`), t322 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: Mute does not kill the state
- **WHEN** a track with a reverb on an event is silenced and unmuted in the middle of the tail
- **THEN** the chain is not recreated, the tail continues from the same place
- **TEST** t732 (`src/libs/kt-engine/tests/track_mix.rs`)

### Requirement: Mix — transition weights, fades, overlaps, gain, track sum
A transition at a seam SHALL give a crossfade over the window with the curve chosen by the user;
equal-power (`√(1−t)` / `√t`) SHALL remain the former formula, byte for byte. A fade is
a transition with one live end, with the same weights against emptiness. The overlap of events of one
track without a transition (`overlap_layers`) SHALL sound equal-power for two and 1/N for
three or more; an overlapping transition silences this weight and applies its own curve — a double
attenuation MUST NOT occur. Tracks without a stage are summed without weights in the former order; for
a track with a stage the sum of its events passes the chain, the level and the track's silencing, and then
is summed into the output. The gain of the event (dB → linear) is applied after the weights; the FX tail beyond the
material is not weighted. The "handles" of a transition read the source beyond the edges of the body; material beyond the
edges of the file — silence WITHOUT an underrun.
Why: `GOLDEN_AUDIO` rests on these formulas, and the conversion of old overlaps must
be byte-exact; the absence of a handle is a fact of the model, not starvation; the track stage appears
only where it is needed, so that the summation order of old projects does not move.

#### Scenario: A converted overlap sounds like the old one
- **WHEN** a legacy overlap has been brought to a seam with a transition of the same window
- **THEN** the render is byte-for-byte equal to the analytic formula of the old crossfade; the weights and the handle material are exact by frames; the FX path without warp reads the handles as plain; the reversed neighbour continues the mirrored formula; the prefetch warms the handles
- **TEST** t136 (`src/libs/kt-engine/tests/transitions_audio.rs`), t2 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: A handle longer than the material
- **WHEN** incoming starts at `source_in = 0`, and the transition window asks for material to the left of zero
- **THEN** there is silence there and `underruns == 0`
- **TEST** t136 (`src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: A fade at the edge
- **WHEN** an event has a transition with one live end
- **THEN** the sound rises from silence (decays into silence) by the same equal-power ramp, half at the middle of the window, silence at the edge
- **TEST** t156 (`src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: The crossfade curve reaches the samples
- **WHEN** a transition has Linear / SCurve / Exponential / EqualPower chosen (capability `transitions`)
- **THEN** each is computed by its own formula and is distinguishable by ear, and EqualPower is bit-for-bit equal to the former one
- **TEST** t226 (`src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: An overlapping transition applies its own curve
- **WHEN** two events in a "staircase" with an explicit transition on the overlap zone
- **THEN** Linear gives the half-sum in the middle against the √-law, equal-power equals the bare overlap byte for byte, without a transition the path is as before
- **TEST** t463 (`src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: An overlap without a transition
- **WHEN** events of one track overlap with `overlap_layers` enabled
- **THEN** two — equal-power over the overlap zone, three or more — 1/N; with the option disabled the overlap is rejected by validation
- **TEST** t162 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: Tracks and gain
- **WHEN** events lie on different tracks, one of them has a gain of ±6 dB
- **THEN** the tracks are summed without weights, the gain is applied exactly
- **TEST** t2, t3 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: Track level after the event's gain
- **WHEN** a track has a level of −6 dB, and an event on it has a gain of +6 dB
- **THEN** the result equals the original within f32 rounding; the track stage is enabled only for it
- **TEST** t732 (`src/libs/kt-engine/tests/track_mix.rs`)
