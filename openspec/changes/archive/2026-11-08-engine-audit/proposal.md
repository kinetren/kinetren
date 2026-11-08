# Proposal

## Why

The owner (kkt-26.11.08 H10, line 16): «может нам учитывая все огрехи которые насобирали за всю
историю - придумать новый движок? а этот выкинуть просто как breaking change?» The orchestrator's
answer was "no rewrite from zero; replace the architecturally wrong parts in place under the same
tests; decide on data". Then H11, line 18: «запускай разбор дефектов по механизмам ... по поводу
планировщика - возможно даже не только GPU но и CPU планировщик. И может быть задуматься JIT
математики и вычислений?»

The decision needs the data in one place. That means every performance, latency and load defect the
project has met, grouped by the mechanism that produced it, and each mechanism judged architectural
(the structure makes it again) or local (fixed once).

## What Changes

- `docs/engine-audit.md`:
  - §0 measurements made for the audit;
  - §1 eleven mechanisms with their defects, costs and open rows;
  - §2 what an incremental engine and one CPU+GPU scheduler would buy against targeted fixes, with
    the scheduler's design;
  - §3 JIT kinds against the owner's acceleration scope and our measured costs;
  - §4 the ordered plan and the explicit answer to "a new engine as a breaking change?".
- A reconnaissance note `docs/features/plugins/kernels/fused-kernels.md` (F-EXT-005) on
  effect-chain fusion, specialization and pipeline caches. Its verdict: prewarm and cache now, fused
  point-wise runs after a real card's numbers, no audio JIT and no CPU JIT.
- `docs/found-defects.md` row 151 (new): the whole audio-engine build is quadratic in the points of a
  shaped speed curve. `warp::sign_knots` takes ~450 ms of the 393–488 ms build on the relinked haski,
  against 1 ms on the heavy showcase.
- Registry `kkt-26.11.08-index.md`, rows H10 and H11, moved.
- No product code, no spec requirement. The plan's changes start on the owner's word.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

(none — an analysis; the changes it orders will carry their own spec deltas)

## Impact

Documentation only: `docs/engine-audit.md`, `docs/features/plugins/kernels/fused-kernels.md`,
`docs/features/README.md` (table), `docs/found-defects.md`, `docs/ktprompts/kkt-26.11.08-index.md`.
