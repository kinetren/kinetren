# Proposal: audio-fx-cost

## Why

The owner, 2026-11-07, on build kt26.11.07-61414dba with his own project (`docs/ktprompts/kkt-26.11.07b.txt`,
item 5, registry row G8): "врубаю vhs tape на видео - терпимо, добавляю на аудио - всё, пиздец, ни кеш не
работает у нас как я понимаю ни код не оптимизирован." The cache half is `cached-range-plays` (a covered
range keeps its post-effect sound). This change is the other half: what the audio effects themselves cost,
since live playback, the first pass over a range, scrubbing and export all run them. His latest word:
"сейчас мы не трогаем vhs-и всякие фильтры по существу, мы сейчас работаем над максимальной
производительностью всего" — effects are touched where their COST is the problem, never their sound.

The mechanism found: the shipped build is baseline x86-64 (SSE2, R.32), and SSE2 has no instruction for
`floor`/`ceil`/`round`/`trunc`. Every one of them on a per-sample path — an LFO phase, a hash to a unit
number, a fractional delay read, a tape position, a `fract` — was a call into the C library through the
PLT, in ten of the 21 audio kernels (fifteen call sites in VHS Tape's sound alone), and no loop around
them could use wider registers. On top of it, VHS Tape's sound did per-sample work no one needed: a `pow`
where an exact square root does, a tape transfer curve for a linear track nobody listens to, a 33-cell
dropout walk per sample per track for the same answer once a millisecond, a 64-sample shift per sample.

## What Changes

- `kt_plugin_api::cpu`: an audio kernel's per-block body runs compiled a second time with AVX2 where the
  CPU has it (`fast!`), the baseline build stays for every other CPU; `KT_SIMD=0` forces the baseline.
  Every audio plugin's `process` goes through it. The bytes do not move (t1981, both paths).
- VHS Tape's sound: the Hi-Fi FM receiver's eight sub-samples run side by side (the same operations in
  the same order), `powf_neg_half` replaces the compander's `pow` to the bit (t1982, every `f32`), the
  linear track's transfer curve waits until someone reads that track, the dropout field's cells are kept
  while the cell stays, the loss filter reads a doubled ring, the passport is not rebuilt per chunk.
- `kt_fx::audio::cost` and `kt-cli fx-cost`: every audio kind's cost against real time, at its defaults
  and at heavy settings, in units of a reference kernel measured in the same run; `--project P --clip ID`
  measures a person's own chain; `--digest` prints the output digests t1981 holds.
- Tests t1980 (a cost bound per kind), t1981 (the bytes of every kind, both paths), t1982 (the exact
  `pow` replacement).

## Impact

- `kt-plugin-api` (new module `cpu`, `math::powf_neg_half`), every `fx-audio-*` family, the audio halves
  of `fx-video-tape`, `fx-video-camera`, `fx-video-codec`; `kt-fx` (`audio::cost`), `kt-cli` (`fx-cost`).
- No format change, no new dependency, no sound change: invariant 11 holds byte for byte (t973c and
  t1981 unchanged, on the AVX2 and the baseline path).
