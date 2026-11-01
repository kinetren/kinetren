# Proposal

## Why

kkt-26.11.01 item 15, second report (`docs/user-feedback.md`, 2026-11-01, after `pitch-event-edges`):
the owner's region `broken_silents_steps` still tears. His events are COPIES of one piece of a reel
(the same in-point, abutting), each with Slowed+Reverb on the event and a morpher's pitch +5 after
it — the effective chain is `[slowed_reverb, pitch_shift]`, its rate curve changes sign far outside
the event. `pitch-event-edges` made the context apply only with no time warp and no tail BEFORE the
pitch, so exactly this chain kept its cold edges. Measured on a 440 Hz sine with the owner's shape
(five copies of 34 762 frames, `zz_probe_owner`): the deepest 64-frame window at every join is
**0.034** of the steady level with `[slowed (rate 1, mix 0.1), pitch +5]`, against 0.88–0.90 for
the same copies without pitch, with pitch alone, or with Slowed alone. Inside the reel the two
clips carry pitch alone (context applies) — nothing to fix there.

## What Changes

- The context applies to every chain with an active edge-reading node, whatever stands before it:
  the nodes before the pitch process the MATERIAL too ("the chain up to the pitch is applied to the
  material, then the event cuts it"), and the content gate after the pitch cuts the result to the
  event. A tail of a node BEFORE the pitch therefore ends with the event; a tail after it still rings.
- A time warp before the pitch: the context applies when the speed at the event's start moves
  forward (the warp already extrapolates the speed before the start and continues the curve past the
  end; the source and consumption windows follow in the same function).
- The run hand-off (one stream across a split) stays for tail-less chains without a warp.
- The version stamp of a worktree build is outside this change (reported to the orchestrator).
