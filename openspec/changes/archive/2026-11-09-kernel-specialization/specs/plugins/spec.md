## ADDED Requirements

### Requirement: A smoother knows when it stands still, and every variant has a general reference
`dsp::SmoothedParam::still` SHALL be true exactly when a tick would leave the value's bits unchanged (a
fixed point of the tick, which then holds until the target moves; not `value == target`), and
`advance(n)` SHALL give the bits of `n` ticks, stopping at the first tick that changes nothing. A kernel's
"stands still" variant (a constant-parameter block, a skipped re-tune, a skipped computation whose answer
is known) SHALL give the bytes of its general path; `cpu::with_general` runs every such variant on the
calling thread on its general path, the reference the tests hold the variants to. No variant adds a
platform math call or an x86-only intrinsic without a fallback.
Why: the owner, 2026-11-09 (kkt-26.11.09 I9): "запускай edit-without-rebuild и kernel-specialization";
engine-audit §3, tier T2 — specialized Rust chosen at run time; invariant 11.

#### Scenario: Still is a fixed point; advance is n ticks
- **WHEN** smoothers glide up, down, through −0, to a target they stop short of, at 0.01–150 ms, and `advance(n)` runs on fresh glides on both paths
- **THEN** `still()` agrees with every tick, no smoother moves after standing still, and `advance(n)` and the tick after it equal `n` and `n+1` ticks to the bit
- **TEST** t2170 (`src/libs/kt-plugin-api/tests/stands_still.rs`)
