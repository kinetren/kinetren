## ADDED Requirements

### Requirement: An audio effect's cost is measured against a reference and bounded
Every audio kind of the registry SHALL be measured one instance at a time, at its defaults and at the
heavy settings `kt_fx::audio::cost::heavy` names, as its node chain's share of real time on a stereo
48 kHz stimulus in the engine's blocks, and as a RATIO to a fixed reference kernel (eight cascaded
biquads per channel) measured in the same run, interleaved with it, best of several. Each kind SHALL
stay within its bound in units of that reference (about twice what it measured when the bound was set).
A cost is never judged by a wall-clock limit: a busy machine slows the kind and the reference alike.
`kt-cli fx-cost` prints the same numbers on a release build, and with `--project P --clip ID` the chain
of a person's own event.
Why: the owner, kkt-26.11.07b item 5: "врубаю vhs tape на видео - терпимо, добавляю на аудио - всё,
пиздец, ни кеш не работает у нас как я понимаю ни код не оптимизирован." VHS Tape's sound cost 52
reference kernels at its defaults (2.5 % of a core, release) and 78 at heavy settings; ten kernels
called the C library for every `floor`/`round` per sample.

#### Scenario: Every kind costs within its bound
- **WHEN** every audio kind runs one second of the stimulus at its defaults and at its heavy settings, three times, alternating with the reference kernel
- **THEN** its best time over the reference's best time is within its bound; a kind without a bound fails
- **TEST** t1980 (`src/libs/kt-engine/tests/audio_fx_cost.rs`)

### Requirement: Making an audio effect cheaper keeps its bytes
An optimisation of an audio kernel's cost SHALL keep its output byte for byte unless a change records
the moved bytes and the reason: the same operations on the same values in the same order (a loop split
into lanes adds its sums in the original order; a table holds the values the expression gave; a cache is
keyed by everything its answer depends on). The digests of every audio kind at its defaults and at its
heavy settings, and of VHS Tape's output switched mid-stream, are held as printed before audio-fx-cost,
on the CPU's path and on the baseline path.
Why: the owner, 2026-11-07: "сейчас мы не трогаем vhs-и всякие фильтры по существу, мы сейчас работаем
над максимальной производительностью всего"; invariant 11.

#### Scenario: The optimised kernels give the bytes they gave
- **WHEN** every audio kind renders two seconds of the stimulus at its defaults and at its heavy settings, and VHS Tape's sound switches Hi-Fi → linear, mix → Hi-Fi and Hi-Fi → stereo mix halfway, on the AVX2 path and inside `cpu::with_baseline`
- **THEN** every digest equals the one printed at db6df6eb, before the change
- **TEST** t1981 (`src/libs/kt-engine/tests/audio_fx_cost.rs`)
