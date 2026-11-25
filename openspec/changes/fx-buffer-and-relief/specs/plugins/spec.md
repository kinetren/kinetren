# Spec Delta

## ADDED Requirements

### Requirement: A plugin may declare a window of past input frames
`Decl.past` SHALL name two rows of the passport (a count and a step in frames) and a bound; the registry SHALL refuse
a name the passport lacks, a bound over 7, a count row that can exceed the bound, and any family but a video effect. The host SHALL
provide the declared frames to the kernel as reads of the same line.
Why: the only other windows are a stream's history and a warp; a kernel that wants the past must not become a state.

#### Scenario: The registry refuses a bad window
- **WHEN** plugins declare a missing row, a bound of 8, a count row whose maximum is 12 and an audio kind with a window
- **THEN** each is refused with its reason and the registry keeps the rest
- **TEST** t4289 (`src/libs/kt-plugin-api/tests/past.rs`)
