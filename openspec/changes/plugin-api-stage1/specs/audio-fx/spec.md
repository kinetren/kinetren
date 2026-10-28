# Spec Delta

## MODIFIED Requirements

### Requirement: Registry of audio kinds and a parameterized contract skeleton
Every audio effect kind SHALL be listed in the closed built-in set — the core-owned warp
(`SlowedReverb`) plus the built-in audio plugins listed by the assembler (capability `plugins`) —
with a unique id equal to its file tag, and the audio effect contract SHALL be checked by one
parameterized harness over all audio kinds of that set: the default sounds and is deterministic,
every smoothable parameter is audible at least at one end of its range, the parameter curve
works, reverse with a curve is deterministic, a repeated seek is byte-for-byte identical, and the
tail and look-ahead declared in the manifest agree with the processor's device. A new kind MUST
NOT build or register without an id and MUST fall under the harness without its own copy-pasted
checks. An audio plugin's state SHALL be created, reset on seek and kept across model edits by
the host (by clip id), on the audio render thread only; the plugin SHALL receive it only for the
duration of a process call. The tail and look-ahead formulas SHALL each have one home — the
plugin's manifest — read both by the static chain formulas and by the built processor.
Why: with 16+ kinds, hand-written tests "per effect" drift away from the code, and the
completeness lock by tags catches an effect that "compiles and silently does nothing". Moving
the formulas into the manifest removes the second home that `chain_tail` / `chain_lookahead`
were (t322 items (e)/(zh) guarded their agreement by comparison; now there is nothing to
disagree).

#### Scenario: A new audio kind falls under the harness automatically
- **WHEN** an audio kind (manifest, id, kernel) is added to the built-in set
- **THEN** the harness runs it through all points of the contract without editing the test, and the set of covered ids equals the set of audio ids in the built-in set; a plugin from the test door is not covered and not required
- **TEST** t322 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: Tail and look-ahead are one formula in two homes
- **WHEN** a kind has memory (tail) or an analysis window (look-ahead)
- **THEN** the static formula of the stateless chain matches the answer of the built processor: the tail within a 2 % tolerance for integer truncation of lines, look-ahead exactly in frames, with and without a curve; for a plugin both read the manifest's declaration
- **TEST** t322 items (e)/(zh) (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: A moved audio kind sounds byte for byte as before
- **WHEN** an audio family (stage 2: dynamics, modulation, filters, delay, pitch) has moved into plugin crates, and the audio references and the harness run
- **THEN** `GOLDEN_AUDIO` and every harness hash are unchanged; a model edit keeps the reverb tail and the limiter ring (the host-owned state is not recreated); a seek zeroes them
- **TEST** t322 (`src/libs/kt-engine/tests/audio_fx_registry.rs`), t973 (`src/libs/kt-model/tests/plugin_format.rs`)
