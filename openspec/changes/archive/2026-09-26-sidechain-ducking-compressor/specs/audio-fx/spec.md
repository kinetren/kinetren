# Spec Delta

## ADDED Requirements

### Requirement: Compressor with a KEY input — ducking with the same instrument
The compressor SHALL accept an optional KEY signal (sidechain): the detector computes the envelope from it, while the compression is applied to the node's own input. The compression law, units and parameter limits MUST NOT change — the key changes only the SOURCE OF THE DETECTOR. Without a key the node SHALL compute exactly what it did before this work, byte for byte (`GOLDEN_AUDIO`, t2/t3 without regeneration), and MUST NOT pay for the key branch with an allocation or an extra pass.
The key signal SHALL arrive at the node as a processing argument in the same domain (f32 48 kHz/2ch) and over the same window as its input; the key branch MUST NOT have its own state, its own fetcher or its own copy of the parameters — otherwise the device would have a second definition of what it does. A key signal BELOW the threshold SHALL leave the signal untouched (ducking does not "breathe" in the pauses), and a key signal ABOVE the threshold SHALL produce the same reduction that this same signal would produce on its own input (one formula, verifiable by comparing two runs).
A second kind of effect for ducking SHALL NOT exist: "ducking" is a compressor with an assigned key, and it is shown by the same parameter widget.
Why: the owner of R.63 — "everywhere, meaningless copy-pastes of the same code pile up, when one can… show that ducking works in the program, for example, through two compressors"; the industry precedent is the Track FX ducker in Resolve, which is fed by the signal of the dialogue tracks. A key as a STATE of the node (a setter living between calls) was rejected: it would create a hidden dependency on the call order, and invariant 2 requires that the state is touched only by the render thread and only on its own window.

#### Scenario: The key controls, the input is compressed
- **WHEN** a steady tone is fed to the compressor input, and a speech sample with pauses is fed as the key; then the same run without the key
- **THEN** with the key the output level falls exactly within the speech intervals and returns in the pauses (attack/release by its own parameters); outside the speech the output equals the input up to makeup; without the key the result is byte for byte equal to the previous code
- **TEST** t767 (`src/libs/kt-fx/tests/sidechain.rs`)

#### Scenario: Neutrality and cost
- **WHEN** the golden chains are run without a single key; then a chain with a key during silence in the key
- **THEN** `GOLDEN_AUDIO`, t2, t3, t322 skeleton are green without regeneration; during silence in the key the output equals the input (up to makeup), and there are no allocations in processing
- **TEST** t768 (`src/libs/kt-fx/tests/sidechain.rs`), t322 (`src/libs/kt-fx/tests/audio_registry.rs`)

## REMOVED Requirements

### Requirement: Auto-ducking — a snapshot of the curve, not a live sidechain
**Reason**: The owner's decision (R.63, recorded in `docs/user-feedback.md`): ducking becomes a live sidechain compressor, the snapshot "kept exactly until change `sidechain-ducking-compressor`". In a direct answer in R.65 the owner chose "remove entirely", knowing that the path of "exporting the envelope outward" would disappear.

**Migration**: The `Gain.db` curves already baked by the snapshot in existing projects are ordinary automation curves — they are read, play and are edited as before, the format of them does not change. Only the way to CREATE them with a button disappears: instead, a compressor with a key on the speech track is placed on the background track.
