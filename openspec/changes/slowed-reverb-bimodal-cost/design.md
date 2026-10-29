## Context

The measurement that found it (plugin-api-stage1 stage 3, 2026-10-29):

| tree | slowed_reverb alone, best of 5, ms / 10 s | chain of all 18 audio kinds |
|---|---|---|
| c53131f (before the audio move) | 104, 415, 103 | 627 |
| ed78f71 (after) | 415, 414, 104 | 628 |

These are the slot's number rows at 70 % of their ranges: rate 12.8, mix 0.7,
reverb_delay_ms 350.3, damp 0.693, decay 0.686, wet_gain 1.4, predelay_ms 140, width 0.7.

Facts that narrow it down:
- The input and the code are identical between runs. The cost class is fixed for a whole PROCESS:
  all five renders of one run agree, and only the next run may flip it. So the cause is something
  a process receives anew, not something the data does.
- The 18-kind chain, which also contains a Slowed+Reverb slot, did not move. Its reverb sees a
  signal already changed by 17 other kinds; whatever makes the lone slot slow depends on that
  slot's own state.
- `DelayLine::write` flushes subnormals ("feedback tails on x86 without FTZ cost"). The freeverb
  `Comb` and `Allpass` in `kt_plugin_api::dsp` do not. Their buffers are eight + four `Vec<f32>`
  per channel, allocated when the reverb is built.

## Goals / Non-Goals

**Goals:**
- Name the cause with a measurement that separates the hypotheses.
- Remove it with the sound byte for byte unchanged.
- Leave a test that catches its return, if a deterministic proxy exists.

**Non-Goals:**
- Speeding up the reverb beyond removing the bad mode. The 104 ms mode is the reference.
- Changing the freeverb's sound, tunings or tail formula: t6, t973c and `GOLDEN_AUDIO` stand.

## Decisions

### D1. Separate the hypotheses before touching code
Each one makes a different prediction. All measurements on release, the lone slot, 20 process runs
each.

| hypothesis | experiment | what it predicts |
|---|---|---|
| H1 placement: 4K aliasing or cache sets between the comb/allpass buffers | `setarch -R` (no ASLR); log each buffer's address mod 4096 per run; `perf stat -e ld_blocks_partial.address_alias,l1d.replacement` | no ASLR → one mode only; the slow runs show buffer addresses congruent mod 4096 and an alias-counter jump |
| H2 core type | pin the thread with `taskset` to a P-core, then to an E-core (`lscpu --extended` shows them) | the mode follows the pinned core; nothing flips under a fixed pin |
| H3 subnormals | count subnormal values in the comb/allpass state after the render; run once with FTZ/DAZ set in MXCSR (diagnostic only) | slow runs carry many subnormals; with FTZ the slow mode is gone |
| H4 the warp at rate 12.8 | the same slot at rate 1.0, and a warp-only chain (mix 0) | the mode survives at rate 1 → not the warp; it appears with mix 0 → the warp |

If two hypotheses hold, they are fixed and recorded separately.

### D2. Fixes that keep the bytes
- **H1:** allocate the comb and allpass lines of one reverb as ONE buffer with offsets chosen so
  that no two line heads fall into the same 4 KiB or cache-set alias. The arithmetic per sample
  does not change, so the sound does not change.
- **H2:** nothing to fix in code; the finding goes into the docs. The render thread is not pinned:
  that is the OS's decision and a person's machine.
- **H3:** flush subnormals on write in `Comb`/`Allpass`, as `DelayLine` does. Flushing can move
  bytes where a tail reaches subnormal range, so t973c and `GOLDEN_AUDIO` decide: if either
  moves, the flush is wrong for the reference and FTZ is not an option either (invariant 11: the
  CPU path is the byte reference).
- **H4:** the warp's buffer setup at high rates (rubato `SincFixedOut` with the extended window),
  under the same byte rule.

### D3. The regression test is a proxy, not a stopwatch
Timing in CI is flaky by nature: R.68 measured the I/O-pressure flakes of t815/t640. So the test
asserts the CAUSE, and the decision depends on what D1 finds:
- for H1, the buffer layout invariant (no two line heads congruent mod 4096 within the reverb);
- for H3, zero subnormal values in the reverb state after a long tail.

The test number is t991 (the next free above t990). The speed itself is printed by a `--ignored`
release measurement that the report quotes. It is not asserted.

## Risks / Trade-offs

- [The bimodality comes from the machine, not the code (H2)] → the change ends with a written
  finding and no code. The requirement then states what the engine does not control.
- [A subnormal flush moves bytes] → t973c and `GOLDEN_AUDIO` catch it in the same run; that branch
  is dropped and H1 or H4 is the fix.
- [One buffer for all lines changes cache behaviour of other kinds] → the 18-kind chain and t293's
  cost bound are re-measured.

## Open Questions

- Whether the owner's machine shows the same two modes. The finding is reported with the
  experiment that reproduces it, so it can be run there with `kt-cli probe … --time`.
