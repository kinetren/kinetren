# Audio render

## Purpose

The timeline's audio engine: assembling the mix from events (transition and overlap weights, gain, track summing, reverse and warp) into f32 48 kHz/2ch, owning the state of the FX chains, the price of the place, look-ahead, loudness measurement, and offline audio analysis (ducking, sync).

## Requirements

### Requirement: The path — f32 48 kHz/2ch from decode to output
Audio inside the engine SHALL be f32 48 kHz stereo interleaved right after decoding and up to the output itself; FX and the mix MUST NOT see the sample rate or channel count of the source file. Event boundaries live in ticks and SHALL be converted to 48 kHz frames with floor exactly once; after that the arithmetic is integer.
Why: the warp path, which computed at the source's frequency, produced periodic clicks at 44.1 kHz (defect a of the old engine); two roundings of the boundaries give a seam of one sample.

#### Scenario: The warp does not know the source frequency
- **WHEN** an event is slowed by half (rate 0.5) on a source of any frequency
- **THEN** the FFT peak moves 440 → 220 Hz, artifacts outside ±16 bins are below −60 dB, and exactly rate·duration of the source is consumed with no accumulated drift
- **TEST** t4 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Back-to-back events sound sample-exact
- **WHEN** two audio events stand back to back and each reads its own window of the source
- **THEN** the output is a continuous ramp, exact to f32, with no duplicates or gaps at the junction
- **TEST** t1 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: Determinism and the golden reference
- **WHEN** the project is rendered twice, with a cold start and after random jumps
- **THEN** the audio bytes are identical between runs and equal the committed `GOLDEN_AUDIO`; preview == export at the same Quality
- **TEST** t7 (`src/libs/kt-engine/tests/audio_engine.rs`), t11 (`src/libs/kt-engine/tests/golden_export.rs`)

### Requirement: FX state is owned only by the audio render thread
The state of the FX chains (`FxStateStore`) SHALL belong to the audio render thread; prefetch, meters, and UI MUST NOT touch it and MUST NOT place mutexes around it. Prefetch fills only the PRE-FX cache of the decoded source; the output of audio FX is not cached and cannot be — a chain is a streaming automaton, and the same piece on a second lap gives a different output. For the same reason, the sub-mix of a nested reel MUST NOT enter the `BurstCache`, and its state is keyed by the INSTANCE, not by the reel.
Why: four threads without locks over a shared delay line poisoned the reverb (defect b); a cache of the FX output would make the bytes depend on the read history.

#### Scenario: A warm cache gives the same sound as the cold reference
- **WHEN** a seek into a slowed event is performed from a cache warmed by prefetch
- **THEN** the output is byte-for-byte equal to the cold reference, and there are no cache misses — prefetch warmed the pre-FX source, not the result
- **TEST** t17 (`src/libs/kt-engine/tests/warp_prefetch.rs`)

#### Scenario: Two instances of one reel do not share the reverb
- **WHEN** one nested reel with a reverb stands in two places and sounds at the same time (capability `compositions`)
- **THEN** each instance has its own sub-mix stream and its own chains; the output does not depend on which one read first
- **TEST** t564 (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: A model edit does not destroy FX state
On any rebuild of the audio renderer, the chain state SHALL be matched by a composite key (clip id or track id) and a fingerprint of the chain's STRUCTURE (slot kinds): the same structure → the same processor; parameters arrive smoothed (one-pole ~20 ms); curves and GEOMETRY (entry point, length, handles) arrive as well; buffers are zeroed only on a real seek (`reset`). Structural changes (look-ahead window, pitch engine, geometry of reverb lines) SHALL rebuild the processor without touching the fingerprint. Muting a track (mute, not solo) MUST NOT remove its chains from the graph.
Why: any UI hiccup recreated the FX nodes and tore the reverb tails (defect f); the fingerprint knows only the slot kinds, and without conveying the geometry a trim left the processor with the old anchor — "sound from another event"; a key by track number would die when a neighbor was inserted.

#### Scenario: Rebuilding the graph in the middle of a tail
- **WHEN** during the sounding of a reverb tail another event is edited and the graph is rebuilt
- **THEN** the tail rings on past the event's boundary and decays by −60 dB within `tail()`, and the output after the rebuild is byte-identical to the continuous one
- **TEST** t6 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: A parameter step does not click
- **WHEN** a chain parameter changes in a jump during playback
- **THEN** the sample-to-sample jump does not exceed 1.5× the natural slope of a sine — the parameter arrived through smoothing, and the buffers were not zeroed
- **TEST** t5 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Changing the entry point with the same chain is audible
- **WHEN** the event's `source_in` is shifted (trim, cut, punch-out, re-anchoring of a morph piece) with the same id and the same chain structure
- **THEN** the sounding material shifts by exactly that amount; the anchor is reassigned by a SEAM, the output is continuous, and the reverb rings on
- **TEST** t283 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: A structural change rebuilds the processor
- **WHEN** the look-ahead window of a node changes, the pitch neutral becomes an asset, or the project's pitch engine is switched
- **THEN** the processor is rebuilt (audible as a mini-seek), and the static tail and look-ahead formulas agree with the node for each kind in the registry
- **TEST** t340 (`src/libs/kt-engine/tests/audio_pitch.rs`), t322 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: Mute does not kill the state
- **WHEN** a track with a reverb on an event is muted and unmuted in the middle of the tail
- **THEN** the chain is not recreated, and the tail continues from the same place
- **TEST** t732 (`src/libs/kt-engine/tests/track_mix.rs`)

### Requirement: There are two breaks — seek and splice — and they must not be confused
A real seek (a change of generation, a sign flip of rate, a loop flip) SHALL go through `reset` and `hard_seek` with a pre-roll of `max(tail, 100 ms)` into the void; a break of the clip-local position during CONTINUOUS output (a loop-region wrap-around, an event shifted with the mouse during playback) SHALL be a SEAM: the source is reassigned, and the reverb and the resampler live on. Misses of the pre-roll SHALL be counted separately (`warmup_misses`) and MUST NOT enter the underruns.
Why: a wrap-around taken for a seek warmed the processor ≈1.25 s on every lap — "the computer is noisy", the underruns grow, the picture freezes.

#### Scenario: A loop-region wrap-around — a seam
- **WHEN** an FX event is played in a loop three times
- **THEN** not a single new warm-up after the first, the reverb tail survives the wrap-around, and warm-up misses are not in `underruns`
- **TEST** t166 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: A region shorter than a burst
- **WHEN** a loop region of 700 frames is played through several wrap-arounds within one burst
- **THEN** there are no repeated warm-ups and no avalanche of source reads, and the output does not degenerate into silence
- **TEST** t167 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: The wrap-around is sample-continuous and consistent with the transport
- **WHEN** the transport plays the region in a loop (capability `transport`)
- **THEN** the player's stream equals the looped offline reference, and the head of the region is pinned in the cache
- **TEST** t14 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: reset zeroes everything, including the predelay
- **WHEN** a processor with a reverb and a predelay receives `reset`
- **THEN** the buffers, including the predelay line, are empty — old material does not ring on after the seek
- **TEST** t319 (`src/libs/kt-fx/tests/audio_nodes.rs`)

### Requirement: Underruns are counted in 48 kHz FRAMES on both paths
The starvation counter SHALL have a single unit — the output frames replaced by silence — and it SHALL apply on both the path without FX and the path with FX. A render with a source that is certainly available MUST NOT produce underruns: any underrun in such a pairing means that the render asked for material outside its own window.
Why: the path without FX counted frames, the path with FX counted burst misses; a single starvation gave numbers differing by a factor of 1024, and the user correctly read the counter as broken.

#### Scenario: One starvation — numbers of one order
- **WHEN** two identical events, one with FX and one without, are played on a completely cold source
- **THEN** both counters are in frames: comparable to the length of the body and differing by a few times, not by a thousand
- **TEST** t280 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: A complex edit without a single underrun
- **WHEN** 12 events on two tracks with overlaps, FX, reverse, fades, and different gains are played linearly and in a three-lap loop across the junction with the source available
- **THEN** `underruns == 0`
- **TEST** t168 (`src/libs/kt-engine/tests/loop_fx.rs`)

### Requirement: Mix — transition weights, fades, overlaps, gain, track sum
A transition at a junction SHALL give a crossfade over the window with a curve chosen by the user; equal-power (`√(1−t)` / `√t`) SHALL remain the previous formula byte for byte. A fade is a transition with one live end, using the same weights against emptiness. The overlap of events on one track without a transition (`overlap_layers`) SHALL sound equal-power for two and 1/N for three or more; an overlapping transition mutes this weight and applies its own curve — there MUST NOT be double fading. Tracks without a stage are summed without weights in the previous order; for a track with a stage, the sum of its events passes the chain, the track's level and mute, and then is added to the output. The gain of an event (dB → linear) is applied after the weights; the FX tail beyond the material is not weighted. The transition "handles" read the source beyond the body's edges; material beyond the file's edges is silence WITHOUT an underrun.
Why: `GOLDEN_AUDIO` rests on these formulas, and the conversion of old overlaps MUST be byte-exact; the absence of a handle is a fact of the model, not starvation; a track's stage appears only where it is needed, so that the summation order of older projects does not move.

#### Scenario: A converted overlap sounds like the old one
- **WHEN** a legacy overlap is brought to a junction with a transition of the same window
- **THEN** the render is byte-for-byte equal to the analytical formula of the old crossfade; the weights and the handle material are exact by frame; the FX path without warp reads the handles as plain; the reverse neighbor continues the mirror formula; prefetch warms the handles
- **TEST** t136 (`src/libs/kt-engine/tests/transitions_audio.rs`), t2 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: A handle longer than the material
- **WHEN** the incoming event starts with `source_in = 0`, and the transition window asks for material to the left of zero
- **THEN** there is silence there, and `underruns == 0`
- **TEST** t136 (`src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: A fade at the edge
- **WHEN** an event has a transition with one live end
- **THEN** the sound rises from silence (or fades into silence) along the same equal-power ramp, half of it at the middle of the window, and silence at the edge
- **TEST** t156 (`src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: The crossfade curve reaches the samples
- **WHEN** the transition has Linear / SCurve / Exponential / EqualPower selected (capability `transitions`)
- **THEN** each is computed by its own formula and is audibly distinct, and EqualPower is bit-for-bit equal to the previous one
- **TEST** t226 (`src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: An overlapping transition applies its own curve
- **WHEN** two events in a "staircase" with an explicit transition over the overlap zone
- **THEN** Linear gives the half-sum at the middle versus the √ law, equal-power equals the bare overlap byte for byte, and without a transition the path is as before
- **TEST** t463 (`src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: An overlap without a transition
- **WHEN** events of one track overlap while `overlap_layers` is enabled
- **THEN** two — equal-power over the zone of overlap, three or more — 1/N; with the option disabled, the overlap is rejected by validation
- **TEST** t162 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: Tracks and gain
- **WHEN** events lie on different tracks, and one has a gain of ±6 dB
- **THEN** the tracks are summed without weights, and the gain is applied exactly
- **TEST** t2, t3 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: Track level after the event's gain
- **WHEN** a track has a level of −6 dB, and an event on it has a gain of +6 dB
- **THEN** the result equals the original within f32 rounding, and the track's stage is enabled only for that track
- **TEST** t732 (`src/libs/kt-engine/tests/track_mix.rs`)

### Requirement: Reverse — a mirror AFTER the warp; the axis is the consumed amount
A reversed event without FX SHALL sound as an exact mirror slice of the source; trims and cuts MUST NOT shift the surviving content. With warp, the order is "warp, then mirror": the mirror axis SHALL equal the material actually consumed (`chain_source_used`), not the end of the source window with the resampler's margin; the same axis MUST stand in the mirror branch of the burst order. The chain sees an ordinary forward stream and knows nothing about direction.
Why: an axis with a margin of 8192 frames shifted the material by 170.7 ms (63 % of the user's event length), while the video was anchored without a margin — A and V showed different things.

#### Scenario: Reverse without FX — an exact mirror
- **WHEN** an event is reversed
- **THEN** the audio equals the mirror slice of the source exactly; with warp — deterministic and not silence
- **TEST** t24 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: A trim of a reversed event does not move the content
- **WHEN** the right/left edge of a reversed event is edited, or it is cut
- **THEN** the surviving frames and samples sound with the same material; trims are applied to the whole link group
- **TEST** t29 (`src/libs/kt-engine/tests/reverse.rs`, `src/libs/kt-model/tests/parity_ops.rs`)

#### Scenario: Reversed + warp reads its own material
- **WHEN** an event with a reverb-warp is reversed
- **THEN** the first frame of the output reads the last frame of the CONSUMED span, not one 8192 frames further
- **TEST** t282 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: Speed −1 — the same mirror with a shifted entry point
- **WHEN** a constant speed of −1 and the "Reverse" flag at +1 with an entry point shifted by the event's length (capability `transport`)
- **THEN** the outputs are equal byte for byte (a constant −1 stays on the flat path without the resampler); the reverb meanwhile sounds forward in its own time and does not equal the mirror of the finished render; around the reversal point the output is a palindrome with no skipped or doubled frame
- **TEST** t616, t617, t618 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: Zero speed — silence with a tail
- **WHEN** |rate| is below 0.01 (the hold)
- **THEN** the source cursor stands still, the output is zero, and the nodes keep working — the reverb rings on over zeros
- **TEST** t619 (`src/libs/kt-engine/tests/reverse.rs`)

### Requirement: Warp and automation domain — event time
The consumption of the source SHALL equal the integral of the speed curve over the event's OUTPUT time; the curves of all parameters live in the same time, and an early speed bump does not shift the firing moment of later keyframes, but it does shift the material under them. The warp map SHALL be a single one for audio, video, and preview (kt-model); the modulus of the speed goes into the resampler, the sign lives in the address map; the decision to hold is made per chunk.
Why: indexing the curves by the consumed material desynchronized the parameters from the picture; three independent clamps of the speed silently turned 0.05 into 0.25.

#### Scenario: Consumption — the integral
- **WHEN** the speed grows linearly from 1 → 2 over the event
- **THEN** by the half of the event 0.625 of the length has been consumed (a parabola); the output reads the material by the integral; a volume curve with its peak in the middle of the event fires in the middle and under the slowdown
- **TEST** t246, t246b (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Keyframes are anchored to the event's time
- **WHEN** an early speed bump is inserted before the later speed keyframes
- **THEN** the firing moment of the later keyframes is not shifted, while the material under them is shifted (the probe in the tail τ=0.90); the volume follows its own curve
- **TEST** t31, t213 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: The warp map with a sign
- **WHEN** the speed curve goes negative, stands at zero, or is reversed
- **THEN** the position walks backward and returns, the consumption only grows, and the mirror axis is the span; positive curves are bit-identical to the frozen oracle of the old map; reverse does not drive the curve backwards; the profile starts at zero
- **TEST** t603a, t603d, t603g, t269, t271 (`src/libs/kt-model/src/warp.rs`)

### Requirement: Burst order == read — one function of the source window
The source window of an event SHALL be computed by ONE function for the render and for the prefetch (`chain_source_window_ext` and its relatives): an extension only for the render MUST NOT be permitted — beyond the window the source returns silence without an underrun, and the miss would be permanent and silent. The raw reverse-transport path plays all events linearly and without warp, and SHALL have ITS OWN burst order.
Why: the prefetch, which warmed "the wrong" bursts, produced endless growth of underruns on reverses.

#### Scenario: The order covers the consumption under random jumps
- **WHEN** a slowed event is played with seeks to random places
- **THEN** wanted ⊇ consumed, and the end of the event does not drop into silence
- **TEST** t17 (`src/libs/kt-engine/tests/warp_prefetch.rs`)

#### Scenario: The raw reverse path warms what it reads
- **WHEN** the transport goes backward over a slowed event
- **THEN** every burst read by the raw render enters `wanted_audio_bursts_raw`, and the warp window does not cover it — otherwise the test guards nothing
- **TEST** t285 (`src/libs/kt-engine/tests/warp_prefetch.rs`)

#### Scenario: Look-ahead extends the window forward for both
- **WHEN** the chain has a node with an analysis window
- **THEN** the source window is extended forward by La both for the render and for the prefetch; there are no misses
- **TEST** t336 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

### Requirement: Chain lookahead (PDC) — input runs ahead of output
An effect with an analysis window SHALL read the source FORWARD of the output position and deliver the output aligned — for the rest of the engine there is no delay, the playhead and crossfades do not move. The look-ahead is included in the seek warm-up and in the source window, but MUST NOT enter the tail (`chain_tail`) — otherwise the event would sound after its end. The look-ahead formula of a kind lives in one home and matches between the node and the static function; La < 4800 frames.
When La = 0 the arithmetic SHALL be identical to the path without look-ahead (`GOLDEN_AUDIO`).
Why: an output delay would shift the sound away from the picture, and a tail that included the analysis window would extend the event.

#### Scenario: Look-ahead does not shift the output
- **WHEN** a limiter in linear mode (threshold and ceiling at 0 dB, window 5 ms) stands in the chain
- **THEN** the output equals the chain without it, sample for sample
- **TEST** t334 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: The window works, the event does not sound longer
- **WHEN** a limiter with a threshold of −6 dB
- **THEN** the peak is exactly at the ceiling, the reduction completes before the peak, and `ext_end` has not grown
- **TEST** t335 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: Neutral is byte-for-byte and independent of slicing
- **WHEN** a pitch shifter at zero parameters without curves; a pitch shifter that is active, with the render requested in one piece and in pieces
- **THEN** the neutral does not touch the buffer and requires no look-ahead; one piece == pieces for both engines (the grids are absolute from the event position); the pitch shift is audible on both engines
- **TEST** t337, t338, t339 (`src/libs/kt-engine/tests/audio_pitch.rs`)

### Requirement: The chain is processed in slot order; the tail is honest
The chain nodes SHALL execute in slot order; the warp is not a node (it holds the monopoly on consuming the source). An effect with memory MUST live in both tail formulas — the node's and the static one; the tail extends the event's audio window, and the reverb rings on under its neighbors with a weight of 1.0.
Why: the fixed pipeline "volume always after reverb" choked the tail with the same fade that muted the input.

#### Scenario: Gain before the reverb — an input fade
- **WHEN** an automated Gain stands BEFORE the reverb and goes to zero
- **THEN** the reverb tail rings on after the input has gone to zero; on a nonlinearity ("quiet → overdrive" versus "overdrive → quiet") the order is audible on constants
- **TEST** t320, t323 (`src/libs/kt-engine/tests/fx.rs`)

### Requirement: The cost of space does not grow with project size and does not read the source in the tail
Nothing on the hot path MUST be O(the whole edit): the set of covering events is computed once by a sweep of the track, and per frame only a binary search remains; the index order in the set is ascending (f32 addition is not associative). Beyond the end of the material the dry signal is zeroed by the gate, so a chunk lying entirely beyond `gate_end` SHALL be skipped without the resampler and without reading the source — byte-identical by construction.
Why: iterating over all events for every frame gave 92 ms/s with 2000 distant events against 3.6 with seven; in the tail the warp honestly computed frames that were then overwritten with zeros.

#### Scenario: Distant events do not change the sound
- **WHEN** 2000 events are added to seven events from a minute away
- **THEN** the output is the same byte for byte
- **TEST** t292 (`src/libs/kt-engine/tests/cost.rs`)

#### Scenario: The cost does not grow with the size of the project
- **WHEN** the same two projects are rendered with measurement
- **THEN** the large one costs no more than three times the small one (on the old code — 25 times); the cost of a dense chain is printed for the measurement
- **TEST** t293 (`src/libs/kt-engine/tests/cost.rs`), t327 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: The tail does not read the source
- **WHEN** the render continuously runs through the reverb tail beyond the end of the body (after the legitimate pre-roll)
- **THEN** there are zero accesses to the source, and the tail sounds
- **TEST** t294 (`src/libs/kt-engine/tests/cost.rs`)

#### Scenario: A region entirely in tails on repeat
- **WHEN** a loop region contains no body at all — only the tails of seven chains
- **THEN** the laps are deterministic, the source is not read, and there are no underruns
- **TEST** t295 (`src/libs/kt-engine/tests/cost.rs`)

### Requirement: Submix of a nested reel — a virtual media under the fetcher
A nested reel SHALL be a virtual source beneath the fetcher, not a branch in the mixer: the weights, the windows, and the f32 summation order know nothing about reels. The sub-mix is rendered ONLY FORWARD along the reel's time; reverse and stretching of the instance act on the samples of the finished mix.
The pre-roll of a move is a function of the project (the tail of the longest-tailed chain), the block = 4× the pre-roll; the value of a burst is a pure function of its index. The order of bursts goes down to the real sources, and the raw path over an embedding MUST sound.
Why: playing a reel backward means computing a third result that belongs to no one; a ring size dependent on the memory budget would make the bytes depend on the budget (capability `compositions`).

#### Scenario: Law E for audio
- **WHEN** a reel with an empty chain is nested in place of its expanded content
- **THEN** the sound is the same byte for byte; the burst order contains no address of the reel, only the addresses of its content; one reverb on a reel of ten lines — not a chain on each one
- **TEST** t447, t449 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The sub-mix in one piece == in pieces, and under reverse too
- **WHEN** an instance of a reel is rendered entirely and in arbitrary pieces, forward and reversed
- **THEN** the bytes are equal; the raw reverse transport over an embedding is not silence
- **TEST** t448, t563, t565 (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: Loudness — BS.1770 / EBU R 128 in f64, in a pass, without master gain
The measurement SHALL give integrated loudness (LUFS), LRA, and true peak (dBTP) per BS.1770-4: a K-filter with the coefficients for 48 kHz as printed, 400 ms blocks with 75 % overlap, gates at −70 LUFS and −10 LU, ×4 oversampling for the peak. The accumulators SHALL be f64. Silence means "no number", not −70. The meter is fed the finished buffer wherever a pass already runs (export, building the peak pyramid); the player's master gain MUST NOT enter the measurement. The result does not depend on the slicing of the buffers.
Why: accumulating squares in f32 over ten minutes drifts by 0.37 LU against a tolerance of 0.1; a second pass for LUFS is a second full decode; the master gain is the loudness of the control, and the preview's number MUST match the file's number by construction.

#### Scenario: Calibration of the scale
- **WHEN** a full-scale 1 kHz sine is fed into both channels; then −20 dBFS and another 10 dB quieter
- **THEN** 0.0 LUFS ±0.1; −20.0 ±0.1; the difference is exactly 10 LU; the LRA of a steady tone ≈ 0; silence → `None`
- **TEST** t403 (`src/libs/kt-fx/tests/loudness.rs`)

#### Scenario: The reason for f64 — sums, not coefficients
- **WHEN** the squares of ten minutes of tone are accumulated in f32 and in f64
- **THEN** the f32 drift exceeds the EBU tolerance of 0.1 LU, the f64 meter is exact — moving the accumulators to f32 "for the sake of speed" is a regression
- **TEST** t403b (`src/libs/kt-fx/tests/loudness.rs`)

#### Scenario: Inter-sample overshoot and slicing
- **WHEN** a sine at a quarter of the sampling frequency with a phase of 45° (samples at −3 dBFS, the wave at 0); material with pauses is fed whole and in pieces of 1/1024/4096/7919 frames
- **THEN** the true peak sees the overshoot (> −1.5 dBTP, ≤ 0.5); LUFS/LRA/dBTP do not depend on the slicing; the gates did not pull the pauses down
- **TEST** t404, t405 (`src/libs/kt-fx/tests/loudness.rs`)

#### Scenario: Loudness on the export pass
- **WHEN** the project is exported (capability `export`)
- **THEN** the summary contains the loudness of the mix: an event 10 dB quieter gives 10 LU less, and dBTP lies at the source's amplitude; the master gain is neither in the project nor in the measurement, and a repeat gives the same number
- **TEST** t406, t407 (`src/libs/kt-engine/tests/export_loudness.rs`)

#### Scenario: The source's loudness on the pyramid pass
- **WHEN** the index service builds the peak pyramid of a file; the bin's card shows a hint
- **THEN** the LUFS/dBTP of the source are computed on the same pass and lie in the sidecar (the second run — without decoding); the display does not place a request, "not measured" is said in words, not shown as zero
- **TEST** t411 (`src/apps/kt-app/src/util.rs`), t412 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Offline analysis does not take the disk away from playback
- **WHEN** several analysis tasks are queued in a row; one is cancelled
- **THEN** one task runs at a time, the queue is deduplicated, cancellation clears everything, and "running" means a live task
- **TEST** t408, t410 (`src/apps/kt-app/src/analysis.rs`)

### Requirement: Sync by sound — by peak pyramid, with an honest refusal
The offset between two recordings SHALL be computed on the peak pyramid (without decoding) over the positive part of the first difference of the envelope — the transients are shared between different microphones, the shapes of the envelopes are not. Sign: a positive lag = "B is later". Below a confidence of 0.30 there is no answer — a refusal instead of a guess. The command SHALL only move (without trim and link), convert "material → timeline" by the single rule of the chunk, and MUST NOT pin the event to zero.
Why: a shift "by eye" is worse than a refusal — the editor will hear a flanger and look for the cause anywhere; a value pinned to zero looks synchronized while the materials are out of step.

#### Scenario: Different microphones, sign, refusal
- **WHEN** two takes of one scene with different frequency response and noise; the arguments are swapped; two unrelated recordings
- **THEN** the offset is found to within 480 ticks; swapping changes the sign; the unrelated ones are refused, not given zero
- **TEST** t413, t414, t415 (`src/libs/kt-media/tests/sync.rs`)

#### Scenario: The command aligns the material, not the edges
- **WHEN** the beginnings of events are trimmed, and one of them is reversed; the alignment would take it beyond the start of the project
- **THEN** the offset accounts for the entry point and the reverse, and only moves; beyond the start — `OutOfRange`, not a clamp
- **TEST** t416, t417, t417b (`src/libs/kt-model/tests/sync_ops.rs`), t418 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Track stage — sum, chain, level, mute/solo; without need, the path stays the same
An audio track with a non-empty chain, a level ≠ 0 dB, or a mute (mute / not in the solo set) SHALL have a stage: the sum of its events with the same weights (fade × overlap × event gain) → the track's chain (a streaming processor without a clip shape: no source, no length, no warp) → × level → × 0 when muted → into the output. A track without a stage SHALL be summed into the output in the previous summation order — byte for byte. Muting is a multiplier, not a removal from the graph: the FX state of the events on a muted track stays alive. Solo SHALL be a set of tracks within a kind; removing solo restores the previous mutes. The key of the FX state SHALL be composite: an event or a track; inserting, deleting, and reordering tracks MUST NOT touch the state of the neighbors.
A track's chain SHALL receive look-ahead like an event's chain (the summation window is extended forward, the output is aligned with the input) and ring on as a tail past the last event (the input is silence). Mute/solo are project content and apply to the export.
A stage SHALL be allowed to depend on ANOTHER stage — through a sidechain key: then the order of computation is set by the dependency, not only by the row rank, and the key branch has its own state.
Stages have no dependencies other than the sidechain key: a track does not read a neighbor's output in any other way.
Why: reordering the f32 summations would shift GOLDEN_AUDIO; removing from the graph would wipe the state (invariant 3, defects b/f of the old engine); a key "(kind, number)" would die when a track was inserted; the owner's decision R.62 — mute and solo in the project; R.65 — a live sidechain, whose detector source is a neighboring stage.

#### Scenario: Without a stage — the bytes are as before; level, mute, and solo are exact
- **WHEN** a project without track properties is rendered; then a track's level is −6 dB; then a track with a reverb on an event is muted and unmuted in the middle of the tail; then solo on two of three tracks
- **THEN** t2/t3/`GOLDEN_AUDIO` are byte-for-byte equal; −6 dB is applied exactly after the weights and the event gain; under mute the track is silent, and after unmuting the reverb tail continues without a cold start; in solo exactly two sound, and removing solo restores the previous mutes
- **TEST** t732 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: A track's chain on the sum, look-ahead, and tail
- **WHEN** a reverb stands on a track of ten lines; a limiter with look-ahead is on the track; the last event has ended
- **THEN** the tail extends across the seams (as t449), the output is aligned with the input (shift 0 relative to a track without a chain), and the tail rings on past the last event; the price of the window is measured and does not grow with the size of the project
- **TEST** t733 (`src/libs/kt-engine/tests/track_fx.rs`)

#### Scenario: The track's state key survives reorderings
- **WHEN** in the middle of a track chain's tail a track is inserted above it, then the tracks are reordered, then a track is deleted
- **THEN** the state of the chain below is untouched (the output is continuous); after the deletion the state is discarded
- **TEST** t734 (`src/libs/kt-engine/tests/track_fx.rs`)

#### Scenario: A stage depends on a stage
- **WHEN** the target track has a compressor with a key on the neighboring track, and both have their own chains
- **THEN** the key stage is computed earlier, its own output in the mix is not doubled, and the state of the key branch does not interfere with the state of its output stage (the tails of both are continuous)
- **TEST** t770 (`src/libs/kt-engine/tests/sidechain.rs`)

### Requirement: Live sidechain — stage order by key, key window, no cycles allowed
A compressor in an audio track's chain SHALL accept a KEY — another audio track of the same reel. The key signal SHALL be that track AS IT SOUNDS: the sum of its events with their weights, FX and gains, its own chain, its level, and its muting (mute / not in solo). A muted key SHALL give silence in the detector, that is, there will be no ducking — and this is exactly what a person hears.
The key SHALL be computed AT THE POSITION OF THE SAME SAMPLES that the device's slot processes: nodes with an analysis window standing BEFORE it deliver their output through their rings, and the signal at the slot is older than the chain's input by exactly their sum. The detector and the compressed signal MUST be aligned in time, otherwise the ducking fires earlier or later than the speech (on a chain of "limiter + compressor with a key" — 5 ms earlier). The key branch SHALL have ITS OWN state (its own stream through the key's chain), separate from the output stage of this same track: one and the same processor cannot be run twice per frame without losing continuity (invariant 3).
The order of stages SHALL be determined by the key dependency: the key track is computed before the target track; with equal dependencies the previous order (row rank) applies, so that the summation order of the f32 does not change. A CYCLE (A listens to B, B listens to A, of any length) MUST NOT exist: the model rejects a write that creates a cycle with a named error; the render additionally breaks the recursion — inside the key branch a compressor with a key works as an ordinary one (the detector on its own input). A key pointing to a vanished or non-audio track SHALL be considered absent (the device works as an ordinary compressor), not as a render error.
A track without keys SHALL render in the previous order with the previous code — byte for byte.
The sidechain MUST NOT add look-ahead: the ducking has no output delay, and `chain_lookahead` does not depend on the key.
Why: the owner's decision R.63/R.65 — a live device instead of a snapshot; alignment to the window of the target is the only way not to drift from PDC (R.48); the ban on cycles — because otherwise the frame would be undefined, and "fixing" a cycle silently would mean passing silence off as music.

#### Scenario: Music dips under speech live
- **WHEN** a speech track has two lines, a music track has a steady tone with a compressor whose key points to the speech track; a window covering both lines is rendered
- **THEN** the music is quieter exactly in the intervals of speech (taking attack and release into account) and returns to its previous level between them; a project without keys in the same run is byte-for-byte equal to the previous one
- **TEST** t769 (`src/libs/kt-engine/tests/sidechain.rs`)

#### Scenario: Stage order, alignment, and determinism
- **WHEN** the key track stands BELOW the target by rank; the target's chain has a node with look-ahead; the same render is run twice, and the second time in pieces with different chunk boundaries
- **THEN** the key is computed before the target regardless of rank, the ducking coincides in time with the speech (there is no shift for the look-ahead), and both runs are byte-for-byte equal
- **TEST** t770 (`src/libs/kt-engine/tests/sidechain.rs`)

#### Scenario: A cycle, a dead key, muting the key
- **WHEN** an attempt is made to assign keys in a circle; then the key track is deleted; then the key is muted
- **THEN** the cycle command is rejected with a named error and the model is untouched; with a dead key the device sounds like an ordinary compressor (no panic and no silence); with a muted key there is no ducking
- **TEST** t771 (`src/libs/kt-engine/tests/sidechain.rs`), t772 (`src/libs/kt-model/tests/model.rs`)
