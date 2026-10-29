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

## Finding (2026-10-30)

**The cause is not the reverb. It is the measurement's stimulus, `kt-media`'s `SyntheticSource`.**
`read_audio_burst` matched the pattern PER SAMPLE (`sample_at` in the inner loop), and the compiler
hoisted the two `sin()` calls of the `Sine` and `ClicksBed` arms out of the match: every sample of
every pattern computed `sin(2π·200·n/48000)` and `sin(2π·hz·n/48000)`, the results kept only by the
arm they belong to. For `Clicks` the `hz` it read is the payload of a dataless variant, which holds
whatever the constructing stack frame held. In the slow runs that was a stack address,
`6.77e-310` as an f64: a SUBNORMAL, and the hoisted `2π·hz·n` arithmetic and `sin` then paid
microcode assists on every sample. The bytes never moved (the arm's result is discarded); only the
cost did, and it depended on stack garbage, which is why it changed between process runs AND
between renders inside one process. Slowed+Reverb showed it 4× because at rate 12.8 it reads
12.8 times as much source as any other kind.

Measured (release, best of 5, machine: a KVM guest with 32 vCPUs on an i9-13900KF host; another
session's builds ran in parallel, so outliers above the cluster are load):

| experiment | result | verdict |
|---|---|---|
| the lone slot as measured before (source built in the test), 20 process runs | 103.1–111.8 ms, one mode | the stack held zero there |
| the stage-3 order (gain rendered first), ed78f71, 3 runs | slowed_reverb 405 / 429 / 328 ms | reproduced |
| `gain,slowed_reverb` in one process, renders 1–5 | 416, 103, 416, 103, 416 ms (and gain 5.4 / 43 ms) | the mode flips between renders |
| stage split of a slow and a fast render | reverb nodes 23.7–27.1 ms in BOTH; source fetch 370–397 against 58 ms; copy 1 ms | the cost is in the source read |
| fetcher-only micro (no FX), 6000 bursts per round | 52.4 ms and 346.6 ms at the SAME source and burst addresses; slow ⇔ the `hz` bytes ≠ 0 | the cause |
| H1 placement: `setarch -R`, 20 runs | 101.7–113.0 ms (one 243.9 under load) | rejected: equal addresses give both modes |
| H1: `perf stat` alias counters | unavailable: `perf_event_paranoid` = 4 and the guest exposes no hardware PMU | — |
| H2 core type | 40 unpinned runs on 20 different vCPUs: 101–111 ms; the mode flips between renders milliseconds apart | rejected |
| H3 subnormals in the reverb | 267,675 of 11,520,000 comb/allpass operations touch a subnormal (deterministic, the same every run), 0 left in the state after the render; FTZ+DAZ: 103 → 86–92 ms | a constant ~12 ms, not a mode; FTZ hid the slow mode because the slow mode's subnormals were the SOURCE's |
| H4 the warp at 12.8 | rate 1.0: 142 ms; mix 0: 104–114 ms; rate 1 + mix 0: 148–154 ms; the resampler itself ~23 ms in both modes | rejected: the warp only multiplies the source reads |

### The fix (D2, as done)
The pattern is matched ONCE per burst, and each arm fills the burst with a loop of its own
(`fill(data, base, arm)`, monomorphized per arm), so no arm's arithmetic can run for another arm.
The per-sample expressions are the same functions `sample_at` calls, so the bytes are unchanged:
t11, t973c, t6 green without a golden edit. The reverb's own subnormals stay (Non-Goals: no
speed-up beyond the bad mode; flushing them would move the tail bytes).

### The proxy (D3, as done)
t991 builds `AudioPattern::Clicks` whose unused bytes hold the observed subnormal and reads 2000
bursts against the same source with clean bytes, interleaved, best of 5: same bytes, cost below
2×. On the unfixed code: 126.9 against 18.6 ms in release, 154.2 against 28.0 ms in the dev
profile (red in both). There is no hardware counter in this machine to assert the assists
directly, so the assertion is a same-process ratio with a threshold far from both 1× and 6.5×,
like t293's. The `--ignored` twin prints the lone slot with a clean and a dirty payload: 109 and
421 ms before the fix, 54 and 53 ms after.

### What it means for earlier measurements
Every audio cost measured on a `synth:` source before this change carried the two hoisted `sin()`
calls per sample of every source frame read. After the fix, the same harness (10 s of clicks,
70 % rows, best of 5): gain 6.0 → 2.4 ms, slowed_reverb 104 → 52 ms, delay 92 → 51 ms, limiter
90 → 7 ms, compressor 49 → 7 ms, pitch_shift 156 → 74 ms, the 18-kind chain 628 → 266 ms. The
stage-3 "within 3 %" comparisons of plugin-api-stage1 compared two trees under the same inflated
stimulus; they hold as relative numbers, but the absolute costs were mostly the stimulus.
