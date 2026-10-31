# Spec Delta

## MODIFIED Requirements

### Requirement: One transition vocabulary, grows only through the registry
The transition kinds SHALL be enumerated by a single registry — the built-in transition plugins
listed by the assembler (capability `plugins`), 33 kinds: crossfade, wipes, patterns, shifts,
stylised, volumetric — with each id equal to today's file tag; the parameter specs of each kind
SHALL be built by the same passport as for effects, and the UI of transition fields MUST NOT
have its own form. The default splice is a crossfade; new kinds are chosen explicitly.
The sound SHALL crossfade under any picture kind (there are no stylised audio transitions — the
owner's decision), and the audio curve is chosen from four (equal-power by default, linear, S-curve,
exponential). A new kind that is not in the registry, or that has not declared whether it is
symmetric, MUST NOT build or register. A transition plugin SHALL carry its translation into the
kernel numbers, its CPU kernel and its WGSL piece in one family crate; the GPU composite SHALL be
assembled from the registry, and what goes into a cache key SHALL name the kind by id, not by the
executor's dispatch number.
Why: a single registry makes it impossible for "what can be applied" and "what is drawn" to drift
apart — the owner complained about exactly this mismatch. Until now the `MODE_*` numbers were a
contract between `transitions.rs` and `composite.wgsl` that allowed insertion only at the end; with
an assembled composite the number is an executor detail, and the contract becomes the id.

#### Scenario: Completeness of the specs and audio curves
- **WHEN** all kinds of the registry are walked
- **THEN** there are exactly 33 of them, the ids are unique and equal the former tags, the defaults of the specs equal the constructor's defaults, the values are clamped; at the middle of a joint equal-power keeps the sum of powers, linear gives the half-sum, exponential — a deliberate dip
- **TEST** t223 (`src/libs/kt-model/tests/transitions.rs`)

#### Scenario: The kind and the curve live in the file and raise the version by content
- **WHEN** a project with a non-crossfade, a non-default audio curve or an inversion is saved and read
- **THEN** the kind, the curve and the inversion survive the round trip; a meaningful `transition_type` goes outward, a foreign type is not substituted; a file with a single crossfade is byte-stable and stays on the prior version; when returning to a crossfade our envelope is cleared; the key `invert` is written only when true and raises the file to v7; an unreadable kind is a remark, not silence
- **TEST** t222 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A moved transition keeps its file text and its frames
- **WHEN** the transition families have moved into plugin crates (stage 4); a project with every kind is written by the frozen pre-move serializer and by the new build, and the conformance list runs
- **THEN** the texts are byte-identical, the CPU references of every kind are unchanged, the GPU results stay within the existing tolerances (t224), and the assembled composite text is identical between two executor creations
- **TEST** t973 (`src/libs/kt-model/tests/plugin_format.rs`), t224 (`src/libs/kt-gpu/tests/parity.rs`)
