# Proposal

## Why

The owner answered the archived `engine-audit` (kkt-26.11.08 H12, H13).
- On its JIT section: «про JIT я вообще-то имел ввиду не только шейдеры».
- On its plan: «а с какого перепуга ты от меня ждешь журнала для построения приоритетов на
  видеокарте? это же буквально ты весь софт под конкретную машину пытаешься оптимизировать?»

The audit had judged JIT only for shaders and today's audio cost. It had also made the GPU arbiter and
fusion wait for his journal.

## What Changes

- `docs/engine-audit.md` §3 is rewritten for every computation:
  - three kinds of compiling: compiled data, specialized Rust chosen at run time, run-time machine
    code;
  - the limits they keep: byte-exact audio, ARM, no invented language;
  - a per-area table with the code read at 8770e6b2, what costs (measured or read), the kind that
    fits, the expected gain, risk, size and verdict.
- §2b and §4 no longer wait for any machine's journal. Limits are general rules or are measured at
  run time on the running machine; each is proved on our benches.
- The plan gains `compiled-curves`, `kernel-specialization` and `stream-layout-per-build`, and records
  the owner's answers (H14–H16).
- F-EXT-005 is widened to CPU computation: Cmajor, Max gen~, SuperCollider/Pd, Cranelift; the trick of
  compiling data rather than code.
- Registry rows H12 and H13 are moved.
- No product code.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

(none — an analysis)

## Impact

Documentation only: `docs/engine-audit.md`, `docs/features/plugins/kernels/fused-kernels.md`,
`docs/features/README.md`, `docs/ktprompts/kkt-26.11.08-index.md`.
