## ADDED Requirements

### Requirement: A frame's own GPU error is its frame's error
Every engine entry point that composes (`execute_measured`, `execute_into`, `execute_once`,
`execute_held`, `present`, `present_kept`, `submit`, `collect`) SHALL run inside thread-local
Validation and Internal error scopes of its device; an error raised there SHALL return
`GpuExecError::Validation` (poisoned until the end of the session in the preview, an error in an
export; `present_kept` shows nothing) and SHALL NOT reach the device's uncaptured-error handler.
Why: kkt-26.11.15 N1 — no frame ran inside a scope, so any wgpu error a frame raised ended the
program through wgpu's default handler instead of the poisoning contract the player has.

#### Scenario: A frame whose upload overruns returns its error
- **WHEN** a frame's layer says 64×64 and carries the bytes of 4×4
- **THEN** `execute_into` returns `GpuExecError::Validation` with the cause, the uncaptured count does not move, and the next frame of 16×8 composes (before: wgpu's default handler panicked inside the frame)
- **TEST** t2721 (`src/libs/kt-gpu/tests/gpu_errors.rs`)
