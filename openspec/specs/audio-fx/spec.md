# Audio effects

## Purpose

The registry of audio effects of an event and the node chain that runs them: processing in slot order, state as a function of the clip position, honest tails, look-ahead without delaying the output, and automation in the event's time domain.

## Requirements

### Requirement: Registry of audio kinds and a parameterized contract skeleton
Every audio effect kind SHALL be listed in the closed built-in set — the core-owned warp
(`SlowedReverb`) plus the built-in audio plugins listed by the assembler (capability `plugins`) —
with a unique id equal to its file tag, and the audio effect contract SHALL be checked by one
parameterized harness over all audio kinds of that set: the default sounds and is deterministic,
every smoothable parameter is audible at least at one end of its range, the parameter curve
works, reverse with a curve is deterministic, a repeated seek is byte-for-byte identical, and the
tail and look-ahead declared in the manifest agree with the processor's device. A new kind MUST
NOT build or register without an id and MUST fall under the harness without its own copy-pasted
checks. An audio plugin's state SHALL be created, reset on seek and kept across model edits by
the host (by clip id), on the audio render thread only; the plugin SHALL receive it only for the
duration of a process call. The tail and look-ahead formulas SHALL each have one home — the
plugin's manifest — read both by the static chain formulas and by the built processor.
Why: with 16+ kinds, hand-written tests "per effect" drift away from the code, and the
completeness lock by tags catches an effect that "compiles and silently does nothing". Moving
the formulas into the manifest removes the second home that `chain_tail` / `chain_lookahead`
were (t322 items (e)/(zh) guarded their agreement by comparison; now there is nothing to
disagree).

#### Scenario: A new audio kind falls under the harness automatically
- **WHEN** an audio kind (manifest, id, kernel) is added to the built-in set
- **THEN** the harness runs it through all points of the contract without editing the test, and the set of covered ids equals the set of audio ids in the built-in set; a plugin from the test door is not covered and not required
- **TEST** t322 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: The echo's tail follows its own mix curve
- **WHEN** a chain holds a Delay at mix 0 whose own mix curve rises above 0, and a chain holds a Slowed+Reverb with a mix curve next to a Delay at mix 0
- **THEN** the static tail of the first is the echo's t60 (its repeats ring past the clip's end), and the second's tail is the reverb's alone — the echo is gated by ITS mix and ITS curve, never by another slot's
- **TEST** t990 (`src/libs/kt-fx/tests/audio_nodes.rs`)

#### Scenario: Tail and look-ahead are one formula in two homes
- **WHEN** a kind has memory (tail) or an analysis window (look-ahead)
- **THEN** the static formula of the stateless chain matches the answer of the built processor: the tail within a 2 % tolerance for integer truncation of lines, look-ahead exactly in frames, with and without a curve; for a plugin both read the manifest's declaration
- **TEST** t322 items (e)/(zh) (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: A moved audio kind sounds byte for byte as before
- **WHEN** an audio family (stage 2: dynamics, modulation, filters, delay, pitch) has moved into plugin crates, and the audio references and the harness run
- **THEN** `GOLDEN_AUDIO` and every harness hash are unchanged — a per-kind table printed on the tree before any audio kind moved holds the output of each audio kind at its default, with every row at 70 % of its range, with a curve on its first Smooth row and reversed (pitch also on the vocoder engine), and an audio kind without a row fails; a model edit keeps the reverb tail and the limiter ring (the host-owned state is not recreated); a seek zeroes them
- **TEST** t322 (`src/libs/kt-engine/tests/audio_fx_registry.rs`), t973 (`src/libs/kt-model/tests/plugin_format.rs`), t973c (`src/libs/kt-engine/tests/plugin_sound.rs`)

### Requirement: Processing follows the order of the chain slots
Chain nodes SHALL execute strictly in slot order, as the chain strip in the UI shows them; no hard-wired pipeline "volume always after reverb" may exist. Single-slot chains SHALL sound byte-for-byte the same as before the node chain. Why: the interface allows dragging slots with the mouse and thereby promises an order; with nonlinear effects (distortion, compressor) the order is audible even on constants.

#### Scenario: Volume before reverb — an input fade
- **WHEN** an automated Gain stands BEFORE reverb and goes to zero by the end of the event
- **THEN** the reverb tail rings on after the input has gone to zero (the last quarter is not choked)
- **TEST** t320 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Order is audible on a nonlinearity
- **WHEN** the same pair [quiet Gain, Distortion] stands in two orders
- **THEN** "quiet → overdrive" is clean, "overdrive → quiet" is dirty: the RMS values differ by more than a factor of two
- **TEST** t323 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: One slot — the same bytes as before
- **WHEN** the chain consists of a single Gain of −6 dB
- **THEN** the output equals the source multiplied by 10^(−6/20), element by element; the committed audio reference is not regenerated
- **TEST** t321 (`src/libs/kt-engine/tests/fx.rs`), `t11_streams_match_committed_golden` (`src/libs/kt-engine/tests/golden_export.rs`)

### Requirement: Node value — a function of the clip position, not a counter of processed items
Everything that depends on time inside a node (LFO phase, the hold grid of a crusher, the grids of grains and hops of pitch shifting) SHALL be computed from the clip position, not from the number of processed frames or calls. Rendering in one piece MUST match rendering in chunks of arbitrary length, and a seek MUST match a cold render without warm-up.
Why: otherwise export and playback would sound different, and a seek would require a warm-up that an LFO does not have.

#### Scenario: LFO phase does not depend on buffer slicing
- **WHEN** a tremolo is rendered in one piece and in chunks, and also after a seek
- **THEN** the results are byte-for-byte equal; a frequency curve from 2 to 8 Hz speeds up the modulation toward the second half of the clip
- **TEST** t325 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: Pitch does not depend on slicing
- **WHEN** a pitch shift is rendered in one piece and in chunks on both engines
- **THEN** the results match byte for byte
- **TEST** t338 (`src/libs/kt-engine/tests/audio_pitch.rs`)

#### Scenario: A seek clears all state, including the reverb predelay
- **WHEN** a reverb with predelay > 0 is reset before a seek
- **THEN** on zero input after the reset the output is strictly zero — old material does not leak through
- **TEST** t319 (`src/libs/kt-fx/tests/audio_nodes.rs`)

### Requirement: Two position breaks — seek and splice — are told apart
The processor SHALL distinguish a real seek (after `reset()`, a warm-up `hard_seek` with a pre-roll of `max(tail, 100 ms)`) from a position jump during CONTINUOUS output (a loop-region wrap-around, a clip shifted with the mouse during playback). At such a jump, only the source is reassigned as a seam; the state of the nodes and the resampler lives on. Parameter edits MUST NOT reset the buffers: smoothed values glide via a one-pole filter of about 20 ms.
Why: a pre-roll on every lap of a loop warmed the processor and dropped the ring into underruns, while resetting state on any edit killed reverb tails.

#### Scenario: Loop wrap-around — a seam, not a seek
- **WHEN** an FX clip plays three laps over the loop region
- **THEN** no new warm-up occurs at all, and the reverb tail survives the wrap-around
- **TEST** t166 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: Graph rebuild in the middle of a tail
- **WHEN** the model is edited while a reverb tail is sounding
- **THEN** the chain with the same structural fingerprint reuses its state: the output is byte-identical to the continuous one
- **TEST** `t6_graph_rebuild_mid_tail_preserves_state` (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: A parameter step does not click
- **WHEN** the mix parameter changes in a jump
- **THEN** the output jump does not exceed 1.5× the natural slope of a sine
- **TEST** `t5_param_step_does_not_click` (`src/libs/kt-engine/tests/fx.rs`)

### Requirement: The tail is honest; "warming up" and "sounding" are kept apart
The chain tail SHALL be the maximum over the nodes and honest (t60 for the longest line); the extension of the event window relies on it. The tail rings past the clip boundary and decays to −60 dB within `tail()`. Look-ahead (the analysis window) MUST NOT enter the tail: it warms up a seek and widens the source window, but it does not extend the event.
Why: the tail and the warm-up used to be one number; a limiter with an analysis window does not ring after the clip, and an extra tail would be audible as the event sounding after its end.

#### Scenario: A reverb tail rings on and decays
- **WHEN** a clip with reverb ends
- **THEN** the output past the clip boundary is not zero and decays by −60 dB within `tail()`
- **TEST** `t6_reverb_tail_rings_past_clip_end_and_decays` (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: The limiter window does not grow the tail
- **WHEN** an event with a limiter ends
- **THEN** after the end of the event there is silence: the analysis window does not extend the sound
- **TEST** t335 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

### Requirement: Lookahead — input runs ahead of output, no delay for the host
An effect with an analysis window SHALL read the source ahead of the output position and deliver its output aligned: for the rest of the engine the delay is zero, the playhead and crossfades do not move. The source window SHALL be extended forward by the look-ahead in ONE function, which both the render and the prefetch ask. Changing the look-ahead window (and the pitch engine) is structural: the processor is rebuilt. The total look-ahead MUST be less than the minimum seek pre-roll (4800 frames).
Why: the output delay would drift against the master clock and neighboring tracks; a window extended only for the render would produce a silent cache miss — beyond the window the source would return silence without an underrun.

At a wrap of the loop region the look-ahead SHALL read the NEXT LAP: the chain's input crosses the
region end `La` frames before the output does and continues from the region start, so the frames a
look-ahead node holds at the moment the output wraps are the head of the region — the material that
will actually sound. A look-ahead node's rings and grids SHALL survive every seam (a loop wrap, a
clip shifted during playback, a re-anchored material): they are keyed by the continuous position of
the stream since the last real seek, while the node's curves are read at the clip position of the
grain. A loop wrap MUST NOT produce a silent run or material from beyond the region end, for any
region length, including a region shorter than the look-ahead window.
Why: the pitch node re-initialised its rings on every position jump and the source was re-seated only
when the output wrapped, so the first `La` = 4 608 frames (96 ms) after each wrap were synthesised from
an empty ring — "on a loop a piece of silence is always heard, and on a small loop total silence"
(ktprompts v26.10.18 item 7); the limiter's ring meanwhile emitted material from past the region end.

#### Scenario: Look-ahead does not shift the output
- **WHEN** a limiter with a 5 ms window sits in linear mode (threshold and ceiling at 0 dB)
- **THEN** the output equals the chain without the limiter, sample for sample
- **TEST** t334 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: The ceiling is guaranteed and the reduction starts before the peak
- **WHEN** a limiter with a −6 dB ceiling processes clicks
- **THEN** the peak is exactly at the ceiling, and the gain drops within the window before the peak
- **TEST** t335 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: Prefetch covers the look-ahead window
- **WHEN** a chain with a look-ahead node is rendered
- **THEN** everything read from the source enters the prefetch request (`wanted ⊇ read`)
- **TEST** t336 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: A loop with pitch has no silent run
- **WHEN** a clip of a 440 Hz sine carries a pitch shift of +7 semitones (WSOLA, then the vocoder), a loop region of 1 s that is aligned neither to a burst nor to the pitch hop plays four laps through the looped render in bursts of 1 024 frames
- **THEN** after the first output frame there is no run of 64 or more frames below −80 dBFS anywhere, in particular none after a wrap (both engines); with WSOLA the RMS of every lap is within 1 dB of the first lap (the vocoder's level depends on the phase its synthesis starts from — 0.56 or 0.25 RMS by cold-start position, with no loop at all — its own defect, left for its own change); the same render before this change showed a run of 4 136 (WSOLA) and 2 839 (vocoder) frames after each wrap (the red proof)
- **TEST** t906 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: A short loop with pitch still sounds
- **WHEN** the same pitched clip plays loop regions of 700 frames (shorter than a burst), 3 000 frames (shorter than the look-ahead) and 6 000 frames (just longer than it), twenty bursts each
- **THEN** every burst carries signal (RMS at least half the dry RMS of the same tone), no burst is all zeros; there are no repeated warm-ups and no underruns (as t167)
- **TEST** t907 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: The looped pitch is deterministic, sliced or not, and reads only what was ordered
- **WHEN** the four-lap looped render of t906 is run twice, once in bursts of 1 024 and once in pieces of 333 frames; and the source reads during it are compared with the prefetch request for the same looped window
- **THEN** both runs and both slicings are byte-for-byte equal; the first lap up to `region end − La` equals the plain (non-looped) render of the same range; everything read from the source is in the prefetch request (`wanted ⊇ read`)
- **TEST** t908 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: A real seek and a moved clip with pitch
- **WHEN** a pitched clip is sought cold to a position and rendered; the same position is reached by continuous playback; then the clip is shifted on the timeline during playback (a seam that is not a loop wrap); and a neutral pitch (zero semitones, no curve) plays a loop
- **THEN** the seek equals the cold render of that position; the shifted clip continues without a silent run of 64 frames or more; the neutral pitch leaves the looped buffer byte-for-byte equal to the dry loop and asks for no look-ahead
- **TEST** t910 (`src/libs/kt-engine/tests/audio_pitch.rs`)

### Requirement: A neutral effect — byte-for-byte identity
An effect with neutral parameters and no curves SHALL NOT touch the buffer and SHALL NOT require look-ahead: zero semitones for pitch, zero gains for EQ, width 1.0 for stereo width. The default of a new parameter SHALL be the previous behavior byte for byte; the committed audio reference MUST NOT be regenerated when a kind is added.
Why: an effect added "to try out" must not silently change the export.

#### Scenario: Neutral pitch — pass-through
- **WHEN** PitchShift with zero semitones and cents and no curves stands in a chain on any engine
- **THEN** the output equals the dry signal byte for byte, and the chain's look-ahead is zero
- **TEST** t337 (`src/libs/kt-engine/tests/audio_pitch.rs`)

#### Scenario: The reference holds
- **WHEN** a kind or a parameter is added to the registry
- **THEN** the golden export matches the committed hash
- **TEST** `t11_streams_match_committed_golden` (`src/libs/kt-engine/tests/golden_export.rs`)

### Requirement: Pitch — height without changing duration; the engine is a project setting
PitchShift SHALL work on the clip-local stream AFTER the warp, without consuming the source (the event duration does not change, the warp's monopoly stays intact). There are two engines: WSOLA and a phase vocoder. The choice SHALL be a project setting (not an effect parameter), change through an ordinary command with undo, and survive saving; the default is not written to the file.
Why: the engines cover different material, and the choice is part of the editor's work for the whole edit, not a hidden application setting.

#### Scenario: Both engines shift the pitch
- **WHEN** a 440 Hz sine is shifted by +7 and −5 semitones with each engine
- **THEN** the spectral peak is at 440·2^(st/12) (±0.5 %), the original frequency has lost more than 100× in energy, and the result is deterministic
- **TEST** t339 (`src/libs/kt-engine/tests/audio_pitch.rs`)

#### Scenario: Engine — a model edit
- **WHEN** the pitch engine is changed in the project settings
- **THEN** the sound changes, the choice survives a file round-trip, and the change is undone by undo
- **TEST** t340 (`src/libs/kt-engine/tests/audio_pitch.rs`)

### Requirement: Effects do what is written on their handles
The formulas of the kinds SHALL be verified by direct probes: a filter cuts, an EQ shelf does not touch another band, a crusher produces a harmonic comb, width 0 gives equal channels, a delay repeats exactly every time_ms in a decaying series, ping-pong crosses the channels, a compressor follows its ratio law, a gate silences quiet input and passes loud input. The frequency response graph of the equalizer SHALL be computed by the same code that produces the sound.
Why: the harness checks "audible" but not "correct"; a frequency response picture computed by a second copy of the formulas would diverge from the DSP at the first edit of the kernel.

#### Scenario: Spectral probes of the filter, EQ, crusher, and width
- **WHEN** sines pass through an LP filter, an EQ shelf at −12 dB, a 3-bit crusher, and width 0
- **THEN** the LP cutoff is ≥ 40 dB at 8 kHz, the shelf does not touch the high frequencies, the crusher's harmonic peak lies in its band, and the channels are equal at width 0
- **TEST** t324 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Echo and dynamics
- **WHEN** clicks pass through a Delay (including ping-pong), a Compressor at 4:1, and a Gate
- **THEN** the repeat occurs at time_ms, the series decays, the second repeat in ping-pong is in the opposite channel; −6 dB → −15 dB (±1.5 dB); a quiet sine is muted, a loud one passes
- **TEST** t326 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

#### Scenario: EQ frequency response from the kernel coefficients
- **WHEN** the EQ response graph is computed from the current biquad coefficients
- **THEN** the peak equals the gain of its band at f0 (±0.05 dB), zeros give 0 dB across the whole sweep, Q narrows the band, and the cascade is the sum in dB
- **TEST** t348 (`src/libs/kt-fx/tests/audio_nodes.rs`)

### Requirement: Audio automation lives in the event's time domain
The curve of any audio parameter SHALL be a function of the event's output progress (the "span cut out by the event, without accounting for slowdown"), while the keyframe is stored in the MATERIAL's tick. The source consumption under a speed curve SHALL be the integral `S(tau) = ∫rate` over output time; the warp profile SHALL start at zero even when the keyframe is hidden by trimming to the left of the input; for a mirrored clip the curve MUST NOT play backwards. A slot without a curve SHALL collapse into a single multiplier (the constant path stays byte-for-byte the same as before).
Why: a domain in material terms made the speed curve's axis depend on the curve itself, shifted the firing moments of other people's keyframes, and left the right side of the graph unreachable.

#### Scenario: An early speed bump does not shift the moments of later keyframes
- **WHEN** an early ×2 bump is added before the speed profile
- **THEN** later keyframes fire at the same output-time moments but on different material (the probe both on the plateau τ=0.55 and in the tail τ=0.90)
- **TEST** t31 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Volume by curve, and without a curve — untouched
- **WHEN** a triangular db curve is on a Gain, and the neighboring clip has no curve
- **THEN** the level rises toward the middle and falls toward the end, and at the peak it equals the original level; without a curve the signal is untouched; legacy slowed keyframes migrate into curves and continue to act
- **TEST** t213 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Consumption — the integral over output time
- **WHEN** the speed goes linearly from 1.0 to 0.5 over the event
- **THEN** by the half of the event 0.625 of the length has been consumed (a parabola), not 0.75
- **TEST** t246 (`src/libs/kt-engine/tests/fx.rs`), `clip_processor_follows_integral_map` (`src/libs/kt-fx/tests/integral_map.rs`)

#### Scenario: The curve of another parameter on a slowed clip
- **WHEN** the clip is slowed by half, and the volume curve is a triangle with its peak in the middle of the event's material
- **THEN** the peak falls in the MIDDLE of the event, not at its end
- **TEST** t246b (`t246_gain_curve_peaks_at_event_middle_under_warp`, `src/libs/kt-engine/tests/fx.rs`)

#### Scenario: The profile starts at zero when the keyframe is hidden
- **WHEN** the speed keyframe lies to the left of `source_in` (after a TrimStart or on the right half of a Split)
- **THEN** `S(0)` is exactly zero, the forward and inverse maps remain mutually inverse, and beyond the edges the extrapolation holds the speed at zero
- **TEST** t269 (`src/libs/kt-model/src/warp.rs`)

#### Scenario: A mirrored clip does not play its curve backwards
- **WHEN** on a clip with `reversed` a keyframe is placed at the start of the event
- **THEN** it fires at the start, not at the end: the mirror lives in the keyframe normalization, and the DSP knows nothing about reversal
- **TEST** t271 (`src/libs/kt-model/src/warp.rs`), t322 item (g) (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

### Requirement: Warp is not a node; it consumes the source exclusively and cleanly
The warp (resampler) SHALL remain a separate field of the processor, not a node: it alone consumes the source strictly sequentially through an internal cursor, without skips, duplicates, or seam stitching; the speed module feeds into the resampler. The chain's input and output are only f32 48 kHz/2ch; no node MUST see the file's sample rate.
Why: two consumers of the source would diverge from the prefetch window, and computing at the file's frequency produced periodic clicks.

#### Scenario: Halving the speed is clean and consumes exactly rate·duration
- **WHEN** a 440 Hz sine is slowed by half
- **THEN** the tone is 220 Hz, artifacts are below −60 dB outside ±16 bins; exactly rate·duration is consumed, with no drift
- **TEST** `t4_slowed_halves_pitch_without_artifacts`, `t4_warp_consumes_source_at_rate` (`src/libs/kt-engine/tests/fx.rs`)

### Requirement: The cost of a dense chain is measured, not asserted
The cost of audio rendering for dense chains SHALL be measured by a printout in the release build (`#[ignore]`), not by an assertion: speed is judged only on release builds.
Why: in debug the numbers are off by a factor of nine, and a threshold in an assertion would break on any CI machine.

#### Scenario: Printing the price of seven full chains
- **WHEN** seven events with a chain of six nodes are rendered in release
- **THEN** the milliseconds per second of sound are printed; the reference point is units to tens of percent of real time
- **TEST** t327 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)

### Requirement: Compressor with a KEY input — ducking with the same instrument
The compressor SHALL accept an optional KEY signal (sidechain): the detector computes the envelope from it, while the compression is applied to the node's own input. The compression law, units, and parameter limits MUST NOT change — the key changes only the SOURCE OF THE DETECTOR. Without a key the node SHALL compute exactly what it did before this work, byte for byte (`GOLDEN_AUDIO`, t2/t3 without regeneration), and MUST NOT pay for the key branch with an allocation or an extra pass.
The key signal SHALL arrive at the node as a processing argument in the same domain (f32 48 kHz/2ch) and over the same window as its input; the key branch MUST NOT have its own state, its own fetcher, or its own copy of the parameters — otherwise the device would have a second definition of what it does. A key BELOW the threshold SHALL leave the signal untouched (the ducking does not "breathe" in the pauses), and a key ABOVE the threshold SHALL produce the same reduction that the same signal would produce on its own input (one formula, verified by comparing two runs).
A second kind of effect for ducking SHALL NOT exist: "ducking" is a compressor with an assigned key, and it is shown by the same parameter widget.
Why: the owner of R.63 — "everywhere meaningless copy-pastes of the same code accumulate, when it is possible to… show that ducking works in the program, for example via two compressors"; the industry precedent is the Track FX ducker in Resolve, which is fed by the signals of the dialogue tracks. A key as STATE of the node (a setter that lives between calls) was rejected: it would create a hidden dependence on the order of calls, and invariant 2 requires that state be touched only by the render thread and only on its own window.

#### Scenario: The key controls, the input is compressed
- **WHEN** a steady tone is fed to the compressor's input and a speech sample with pauses is fed as the key; then the same run without the key
- **THEN** with the key the output level drops exactly during the speech intervals and returns during the pauses (attack/release by its own parameters); outside the speech the output equals the input up to makeup; without the key the result is byte-for-byte equal to the previous code
- **TEST** t767 (`src/libs/kt-fx/tests/sidechain.rs`)

#### Scenario: Neutrality and cost
- **WHEN** the golden chains are run without a single key; then a chain with a key during silence in the key
- **THEN** `GOLDEN_AUDIO`, t2, t3, and the t322 harness are green without regeneration; with silence in the key the output equals the input (up to makeup), and there are no allocations in the processing
- **TEST** t768 (`src/libs/kt-fx/tests/sidechain.rs`), t322 (`src/libs/kt-fx/tests/audio_registry.rs`)

### Requirement: Audio catch-up — presets over existing nodes before new nodes
The audio part of the catch-up (wave W7 of F-FX-007, "voice presets": telephone, megaphone, radio,
robot, chipmunk/deep) SHALL be delivered first as chain presets and built-in variants over the
existing audio kinds; a new audio node SHALL be added only where no chain of existing nodes
expresses the sound (today: a ring modulator for "robot"), as an audio plugin with declared state,
tail and look-ahead (capability `plugins`) that falls under the parameterized harness without a
test of its own. The note SHALL list every built-in audio id.
Why: CapCut's voice changers are presets of filters, pitch and distortion; we have every one of
those nodes and the preset library that carries curves (F-AUT-004). A node per preset would grow
the harness and the catalogs for sounds the chain already makes.

#### Scenario: The audio list matches and the wave declares what is new
- **WHEN** the guard compares the audio list of F-FX-007 with the audio kinds of the registry and
  reads wave W7 of the change design
- **THEN** the list equals the registry (18 ids today); W7 names its presets by the existing ids
  they chain and names exactly the nodes it adds, each with the family crate it lives in
- **TEST** t978 (`src/apps/kt-cli/tests/fx_catalogue.rs`), t982 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

### Requirement: A look-ahead window that reads across the event's edges
A kind whose output at a moment is built from the input around that moment (pitch: a grain, an FFT
frame) SHALL declare that its look-ahead window reads across the event's edges
(`Decl.reads_across_edges`); a kind that shapes what the event outputs (the limiter) SHALL NOT. When a
chain has such a node with a non-zero look-ahead and, if it warps time, its speed at the event's start
moves forward ("the context applies", one function `chain_context` for the processor and the window
functions), the chain's input around the event's edges SHALL be the event's own source: the seek
pre-roll reads before the in-point and the input continues past the out-point by the look-ahead. A
content gate right after the last such node SHALL cut the result back to the event (and the dry side
of a layer mix waiting for its marker with it), so nothing outside the event is heard and nodes after
it see the event as cut; nodes BEFORE it process the material too, and a tail of theirs ends with the
event. The render and the prefetch SHALL extend the source window in the same
function. A chain where the context does not apply SHALL be byte for byte as before.
Why: an event is a window into its material, and pitch is a time-scale operation — the pitched event
is "the material pitched, then cut", as clip pitch is applied to the source media in other editors.
Cut first and pitched after, its grains at the edges were half silence: fifteen short pitched events in
a row tore (kkt-26.11.01 item 15) — WSOLA +7 had 320 frames of total silence before every join, −7
250 frames after it, and the vocoder dropped to 0.42 of its level. A limiter that saw past the
out-point would duck before a peak that is never heard, so it does not read across. The owner's own
case (pitch-context-whole-chain) was a morpher's pitch AFTER the event's Slowed+Reverb on copies of
one piece of a reel: limited to chains with nothing before the pitch, the fix left it at 3 % of its
level at every join.

#### Scenario: Pitched events of other material have no dip at their edges
- **WHEN** fifteen 100 ms events, each from a different place of a 440 Hz sine (no event continues another), carry a pitch shift of ±7 semitones, with each engine
- **THEN** no 64-frame window within 1 024 frames of a join falls below half of the steady level of its event (before: total silence with WSOLA); the render is the same in one piece and in pieces of 1 024 and 333 frames
- **TEST** t1106 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: Nothing outside the event is heard; a reverb after pitch rings only on the event
- **WHEN** one pitched event (+7) stands in the middle of a timeline over a source that is loud before and after it; then the same event with an echo AFTER the pitch
- **THEN** the output before the event's start and (without a tail) after its end is exactly zero; with the echo after the pitch the tail equals the echo of the gated pitched event
- **TEST** t1107 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: The prefetch orders the context
- **WHEN** pitched events (forward and reversed) are rendered with a recording fetcher from a cold seek and through their edges
- **THEN** every source frame read is inside the prefetch request for the same range (`wanted ⊇ read`), including the pre-roll before the in-point and the look-ahead past the out-point
- **TEST** t1108 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: Who reads across edges
- **WHEN** the static `chain_context` and the processor's own decision are compared over chains: pitch alone, a limiter alone, neutral pitch, limiter + pitch, pitch + echo, echo + pitch, Slowed+Reverb + pitch
- **THEN** they agree; every chain with an active pitch reads across (an echo or Slowed+Reverb before it included), the limiter alone and neutral pitch do not; the limiter alone renders an event cut from a longer material byte for byte as the same event with silence around it
- **TEST** t1109 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: Copies of a reel piece with Slowed+Reverb and a morpher's pitch do not tear
- **WHEN** five abutting copies of the same 0.72 s piece (the same in-point) carry Slowed+Reverb (rate 1, mix 0.1, a rate curve that turns negative far past the events) and a pitch +5 — in the event's chain and appended by a morpher — over a 440 Hz sine and over a reel holding it, with each engine
- **THEN** no 64-frame window within 1 024 frames of a join falls below half of the steady level (before: 0.034); the render is the same in one piece and in pieces of 1 024 and 333 frames
- **TEST** t1113 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: A node before the pitch processes the material, and its tail ends with the event
- **WHEN** one event (+7) over a loud source carries an echo BEFORE the pitch, then Slowed+Reverb (rate 0.8) before it, forward and reversed, rendered with a recording fetcher
- **THEN** the output outside the event is exactly zero (the echo's tail ends with the event), the edges sound, and every source frame read is inside the prefetch request (`wanted ⊇ read`), the pre-roll before the in-point included
- **TEST** t1114 (`src/libs/kt-engine/tests/pitch_edges.rs`)
