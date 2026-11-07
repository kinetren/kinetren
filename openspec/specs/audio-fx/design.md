# Audio effects — why this way

## What hurt and for whom

There were exactly two audio effects (slowed+reverb and gain), the processor was a monolith
with fields for those two effects and a hard-wired chain "warp → reverb → gain". The chain's
strip in the UI already let slots be dragged with the mouse — that is, the interface promised an order
that the DSP did not have; a Gain placed before the reverb still sounded after
it. The owner demanded "10–20 more effects" and an honest order, as in video and in DAWs.

The slowed automation "counted time God knows how": the axis of the speed curve depended
on the curve itself, an early speed key shifted the trigger moments of the keys of other
parameters, the right part of the graph was unreachable. The requirement, verbatim: the axis is the
span cut out by the event, without accounting for slowdown.

The loop-region wrap-around was computed as a seek: a reverb reset, a resampler re-creation and
a pre-roll on every lap — "the computer is humming", underruns grow, the picture freezes.
The predelay ring of the reverb was not cleared on reset — old material leaked through the
seek (non-determinism). The limiter and pitch need an analysis window, and the chain had no concept of
delay — the only time property, the tail, extended the end of the event.
The most frequent live complaint from reconnaissance was "the music drowns out the voice".

## Decisions and why

- **A node chain in slot order.** `ClipProcessor` holds a `Vec<(slot, AudioNode)>`
  and processes in chain order; the warp stays a separate field (a monopoly on
  consuming the source, the prefetch window is computed from it alone); LayerMix is
  a marker node, the dry-side snapshot is taken by `produce` before the first node with a slot
  ≥ from, and "the layer's reverb only in the wet side" became a consequence of position, not
  a flag. The cost was accepted by the owner: projects with an automated Gain placed before
  slowed+reverb now sound different; the reference is intact, because it has one slot.
- **A `all()` registry + `tag()` instead of a constant list.** The variants carry a `Vec`
  (curves), so `const ALL` is impossible; `all()` via OnceLock and an exhaustive
  `match` in `tag()` — the only compile lock. The tests were moved from counting
  ("audio == 2", a manual list of 12 variants) to properties over the registry.
- **One parameterized frame (t322) instead of a test per effect.** Audibility,
  curves, reverse, seek, tail agreement and look-ahead are checked over
  `all().filter(is_audio)`; for a wave of 16 kinds not a single copy-paste
  check was written, and formula discrepancies are caught automatically for a newcomer.
- **A value is a function of position.** The LFO phase is the integral of the frequency curve over the clip position
  (like the noise seed in video), the hold grid of the crusher and the pitch grids — from the position.
  Consequence: in one piece == in pieces, seek == cold render, with no warm-up.
- **Two breaks: seek and seam.** A real seek — only after `reset()`, which the
  player does; any other break in continuous output is a seam: the source is swapped, the nodes and the sinc live on.
  The pre-roll at the loop wrap-around disappeared.
- **Look-ahead, not delay.** A node with an analysis window reads the source ahead and emits an
  aligned output; for the host the delay is zero (the master clock is the cpal callback, capability
  `transport`). The source window is extended forward in one function for render
  and prefetch. "Warming" and "sounding" are separated: look-ahead is part of the warm-up and the window, but not
  of the tail.
- **Neutral byte-for-byte.** Zero parameters without curves — an early exit without touching
  the buffer and zero look-ahead; the default of a new parameter is the previous behaviour.
  The audio reference was not regenerated even once over two waves.
- **Pitch: two engines, the engine is a project setting** ("both item 1 and item 2 with a global
  switch"). WSOLA is cheap and good on voice, the vocoder is smoother on polyphony;
  the choice is by the material for the whole edit, by the same command as fps (undo for free), the default
  is not written to the file.
- **Ducking — a snapshot.** The envelope of the source is computed by an honest render
  of the source track and placed as a curve on Gain: visible, editable, one undo. The music changed —
  call it again; it does not "grow back" by itself.
- **Delay: the ring at maximum, the time is not structural.** A time change is a fractional read
  with a tape glide of ~100 ms, not re-creation of the ring; LP 4 kHz in the loop (the tail darkens).
- **EQ — three bands in one effect** (shelf+peak+shelf); the frequency-response graph is a second view of the same
  numbers, the curve is computed from the kernel's coefficients, not from a second copy of the formulas.
- **Automation in the event's time domain**, the key — in the material tick (trimming/Split/
  reverse work by themselves); the warp map `S(tau)=∫rate` is piecewise quadratic, the inverse is
  a root in the citardauq form without `ln`/`expm1` (bit-portable). The formula lives in
  kt-model (`warp.rs`), because the clip's chunk rule must call it; kt-fx merely
  re-exports it (capability `automation`).

## Rejected and why

- **Live sidechain** — a second input needs addressing, buses and an ordering rule for
  evaluation: an XL edit of the model and the format, while the practical benefit is covered by the snapshot.
- **Master limiter** — the master fader exists only in the preview, export does not know it; there is
  nowhere to put a limiter "on the master" so that preview and export match (the cost is not
  in the DSP).
- **Output delay as the look-ahead implementation** — the output would drift against the master clock
  and the neighbouring tracks; the clock and determinism invariants make this a defect, not a matter of taste.
- **Linear-phase EQ** — requires delay compensation on the whole path; live preview and
  frame-accurate sync with the picture do not tolerate it.
- **Tempo-sync delay** — the project has no concept of tempo.
- **Frequency split of stereo width ("mono bass")** — one knob covers the edit.
- **LFO shapes other than sine, oversampling of distortion** — out of scope until it gets in
  the way in practice.
- **A constant `ALL` of the registry** — the variants carry a `Vec`.
- **`ease` on the speed curve** — the integral of a fourth-power smoothstep, no gain;
  the parameter spec fixes it.
- **Reverse in the DSP** — the mirror lives in the normalisation of the key; the processor knows nothing
  about reverse and should not.
- **A cost threshold in an assert** — in debug the numbers lie nine times over; speed is judged by the release build.

## Non-obvious details

- `Freeverb::reset()` must also zero the predelay ring (`src/libs/kt-fx/src/audio/nodes.rs`):
  without this up to 200 ms of old material leaked through the seek when predelay > 0.
- A change of `reverb_delay` > 0.5 ms is a structural change of the line geometry (the only exception to "`apply_params` does not reset buffers"), `src/libs/kt-fx/src/audio/nodes.rs`.
- The targets of Smooth parameters are set in `AudioNode::set_targets` once per chunk, and ONLY when
  the automation is non-empty — the bases are set by `apply`; the gate preserves the bytes of old chains
  (`src/libs/kt-fx/src/audio/nodes.rs`).
- The LFO node keeps a COPY of the points of the `rate_hz` curve (bound at apply/set_auto/set_clip_len):
  `process` does not read the model — only the render thread touches the state
  (`src/libs/kt-fx/src/audio/nodes.rs`, `LfoDrive`); the integral is `kt_model::param_integral`
  (`src/libs/kt-model/src/warp.rs`), a trapezoid WITHOUT clamping the values.
- The tail lives in TWO formulas — `AudioNode::tail()` and the static `chain_tail`
  (`src/libs/kt-fx/src/audio/mod.rs`); the look-ahead — in `AudioNode::lookahead()` and
  `chain_lookahead` there; formulas of this kind are kept in one house (`nodes::limiter_la_frames`
  is called by both sides). Agreement is guarded by t322 (f)/(g); the tail tolerance of 2 % is for the integer truncation of line lengths.
- The filter recomputes its coefficients in sub-blocks of 128 frames from the smoothed parameters
  (`src/libs/kt-fx/src/audio/nodes.rs`, `FilterNode`): a chunk of 1024 = 21 ms is audible on sweeps
  as zipper noise; changing the cut kind does NOT re-create the state; the tail is the ringing of the resonance,
  t60 = 6.9·Q/(π·f0).
- EQ: the band constants `EQ_SHELF_Q`/`EQ_MID_Q_FLOOR` are shared by the sound and the graph; `Biquad::mag_db`
  expands |H|² from the current coefficients, `kt_fx::eq_mag_db` configures the same bands with the
  same `tune` (`src/libs/kt-fx/src/audio/primitives.rs`, `nodes.rs`, `EqNode`).
- StereoWidth at width == 1.0 is an explicit skip: the m/s repacking is not byte-identical
  (`src/libs/kt-fx/src/audio/primitives.rs`, `apply_width`).
- Bitcrusher: the hold on the grid of the clip POSITION `floor(pos·rate/sr)`; the edge `rate_hz = 48000`
  is transparent (the grid at every frame) — hence the rule of the frame's probe "at least one of the
  two ends of the range is audible" (`src/libs/kt-engine/tests/audio_fx_registry.rs`, `probe_values`).
- The frame's stimuli — `Clicks` (wideband stereo clicks of 4 frames: filters have HF,
  stereo parameters have an s-component; Ramp was almost DC) and `ClicksBed` (clicks + a background at −34 dB:
  on a zero background the hold/release of the gate is inaudible) plus enablers (the EQ band frequency is probed
  with the gain on, the compressor — with an attack of 0.1 ms) — a table with a comment in
  `src/libs/kt-engine/tests/audio_fx_registry.rs`.
- Chorus: 1–3 voices (Structural), phases `+k·2π/voices`, right channel +90°; Flanger: the sign of
  feedback = phase inversion; Phaser — its own cascade of 1-pole allpass filters, not Freeverb's with a
  fixed 0.5; Vibrato — 100 % wet deliberately (with a dry signal it is a chorus)
  (`src/libs/kt-fx/src/audio/nodes.rs`).
- Delay: ping-pong — cross feedback; the tail t60 over (time, feedback) with a gate by
  mix — one formula in the node and in `chain_tail`. The ping-pong probe in t326 requires clicks
  alternating between channels: a stereo impulse is unsuitable for it
  (`src/libs/kt-engine/tests/audio_fx_registry.rs`).
- Compressor: a peak envelope `EnvFollower` with stereo link `max(|l|,|r|)`, the law
  `gr = over·(1−1/ratio)`; Gate: a detector of ~0.5 ms + hysteresis −6 dB + a hold counter in
  frames, which survives a seam (`src/libs/kt-fx/src/audio/nodes.rs`).
- Limiter: a sliding maximum over the window `[t, t+La]` with a monotonic queue; the gain
  `g = min(1, thr/wmax)·(ceil/thr)` — the ceiling is guaranteed algebraically, because the window
  includes the output frame itself; down instantly, up by a one-pole on release. There is deliberately
  no tail or branch in `chain_tail`. `lookahead_ms` — Structural: the length of the ring is part of the
  node's device (`src/libs/kt-fx/src/audio/nodes.rs`, `LimiterNode`).
- The PDC mechanics in `ClipProcessor::produce` (`src/libs/kt-fx/src/audio/mod.rs`): two positions
  of the chunk — the output `q` and the input `p = q + La`; the source, the warp and the tail gate live in the input
  domain, the node of slot k gets `pos = q + ahead[k]` (the sum of the look-aheads of the nodes not earlier than k; zero at
  the end of the chain). At `La = 0` everything is arithmetically identical — the reference stands on this.
- `hard_seek` (`src/libs/kt-fx/src/audio/mod.rs`): the formula of the source position did not change,
  the warm-up loop starts at `start − La` and lasts `preroll + La` — by the first output
  frame the rings of the look-ahead nodes are filled with real material. Therefore any La must
  be less than `MIN_PREROLL_FRAMES` = 4800 (`src/libs/kt-fx/src/lib.rs`).
- LayerMix over a look-ahead node: the dry side is delayed by a ring of
  `ahead[from] − ahead[mix]`, otherwise the blend would mix different moments of the material; with a zero
  delta the ring is not created (`src/libs/kt-fx/src/audio/mod.rs`).
- The accepted cost of PDC: with a live speed curve the warp map is sampled by the input position,
  the curve's phase is shifted by La (≤ 20 ms) — otherwise the consumption would diverge from `source_offset` and
  `chain_source_window_ext` (`src/libs/kt-fx/src/audio/mod.rs`).
- `chain_fingerprint` knows only the KINDS of slots; the look-ahead and options are geometry, and `FxStateStore::sync_clip`
  checks them (`src/libs/kt-engine/src/fxstore.rs`): a discrepancy of
  `lookahead_frames()` with the static formula or of `opts` (the pitch engine) REBUILDS
  the processor — it is audible as a mini-seek, documented.
- Pitch: a live curve over a zero base is NOT neutral — therefore `chain_lookahead`
  takes `auto` (`src/libs/kt-fx/src/audio/mod.rs`). `PITCH_LOOKAHEAD_FRAMES` = 4608 is the
  worst case of WSOLA at r = 4, one constant for both engines
  (`src/libs/kt-fx/src/audio/nodes.rs`).
- WSOLA: the synthesis grid is absolute (step 512, grain 1024, Hann 50 % — the sum of windows ≡ 1), the
  read shift is cross-correlation ±256 in steps of 8, candidates in ascending |off|, on a tie the
  smaller shift (determinism independent of the traversal order). The OLA accumulator must
  hold a chunk of 1024 PLUS the full synthesis window of 2048 (total 4096): on a ring of 2048 a grain
  synthesised ahead overwrote the not-yet-delivered frames — the result depended on the slicing
  (t338) (`src/libs/kt-fx/src/audio/nodes.rs`, `PitchNode`).
- Phase vocoder: synthesis by the INVERSE mapping (bin j asks the source at j/r) — the direct
  transfer `k → round(k·r)` gave several sources into one bin, the pitch drifted (612 Hz
  instead of 659); the phase accumulators are separate per channel; DC and Nyquist are zeroed by the imaginary
  part — otherwise realfft refuses (`src/libs/kt-fx/src/audio/nodes.rs`).
- The value of the `rate` key is clamped at the input of the profile (`AutoProfiles::from_curves`,
  `src/libs/kt-model/src/warp.rs`): `segments` clamps BEFORE interpolation, `eff_rate` — AFTER,
  and the legacy migration copies the number as it is; a discrepancy of the map with the DSP means misses in the prefetch.
- The trimming of a hidden key is done in `segments`, not in `source_progress`
  (`src/libs/kt-model/src/warp.rs`): this way the forward and inverse functions remain mutually
  inverse for free, and beyond the edges the extrapolation is the speed at zero, not the hidden key.
- The MODULE of the speed goes into the resampler, the sign lives in the address; |rate| < `RATE_AUDIO_FLOOR` (0.01)
  — hold: silence without consumption, the nodes keep working (the reverb rings out on zeros);
  the decision is made per chunk, not per frame — the resampler's cursor would not survive it
  (`src/libs/kt-fx/src/audio/warp.rs`, `mod.rs`; capability `transport`).
- A refusal of rubato when the ratio changes IS CHECKED (`Warp::rejected`, `src/libs/kt-fx/src/audio/warp.rs`):
  it used to be swallowed, and a curve wider than the window ±32 silently stuck at the previous ratio; a curve
  below 0.25 builds the resampler in extended mode.
- Ducking (`kt_engine::ducking::duck_spans`, `src/libs/kt-engine/src/ducking.rs`): RMS with a window of
  960 / step of 480, merging of pauses < 300 ms and dropping of spans < 150 ms (otherwise the curve "hiccups" between
  words); the window is not broken at a chunk boundary. The analysis reads the sources with ITS OWN fetcher —
  the cache is warm only around the cursor. The command `duck_curve_commands`
  (`src/libs/kt-model/src/ops.rs`): keys via `clip_material_at`, the existing `db` curve
  is REPLACED, one `apply_batch`.
- Checklist "add an audio effect" (`src/libs/kt-fx/CLAUDE.md`): `class() -> Audio` must not be forgotten — the kind would silently become Video2d; a branch in `chain_tail` for the tail, `chain_lookahead` for the look-ahead, the kt-app registry entry and the catalogs in ×5 languages (capability `ui-timeline`).
- Chain presets and inserting a chain block with curves — capability `automation`
  (`src/apps/kt-app/src/user_store.rs`, `kt_model::append_chain_block`).
- **Look-ahead rings are stream-keyed; a seam keeps them** (pitch-loop-silence, 2026-10-29). The
  pitch node used to restart its ring, OLA accumulator and grain grid on every jump of the clip
  position, and synthesised its whole look-ahead (`La` = 4 608 frames, 96 ms) from an empty ring
  after every loop wrap; a loop shorter than about 5 100 frames was silent throughout (measured
  4 136 frames WSOLA / 2 839 vocoder, t906/t907). Now the stream position is set at a real seek
  and only advances; the curves are read at the grain's clip position through the
  `(stream, clip − stream)` list (`PitchNode`, `src/plugins/fx-audio-modulation/src/lib.rs`).
  Rejected: relabelling the ring by `−span` (laps collide modulo the ring for a short region), a
  warm-up at every seam (it would push material twice through the nodes before), a crossfade
  from the dry signal (it plays the wrong pitch for 96 ms). The vocoder's level depends on the
  phase its synthesis starts from — not fixed there (algorithm out of scope).
- **A window that reads around its moment reads across the event's edges** (pitch-event-edges,
  2026-11-01, kkt-26.11.01 item 15: fifteen short pitched events tore). Every pitched event started
  cold at its own edges: the seek pre-roll stopped at the in-point, the input gate cut at the
  out-point, so the grains at the edges were half silence (measured: WSOLA +7 — 320 frames of total
  silence before every join, −7 — 250 after it; the vocoder at 0.42 of its level). An event is a
  window into its material and pitch is a time-scale operation: the pitched event is "the material
  pitched, then cut", as other editors apply clip pitch to the source with handles. A kind declares
  `Decl.reads_across_edges`; `chain_context` (one home) decides; a content gate after the last such
  node cuts the result back. The limiter does not read across: it shapes what the event outputs and
  must not duck before a peak that is never heard. Rejected: a grid anchored to source time (the rings
  stay empty, two streams still differ), reflecting the event's tail (invents material, reverses the
  phase), a crossfade at every join (both sides silent at the cut; two equal tones out of phase
  cancel).
- **What stands before the pitch processes the material too** (pitch-context-whole-chain,
  2026-11-01, the owner's second report on item 15). The first fix excluded chains with a tail or a
  time warp before the pitch — exactly the owner's case: copies of a reel piece with Slowed+Reverb on
  the event and a morpher's pitch appended after it (dip to 0.034 at every join). The warp map is
  defined outside the event (the first speed extrapolates backwards, the curve runs on past the end),
  and every node before the pitch is causal, so feeding it the material changes its output only
  outside the event, where the content gate cuts. Cost: a reverb or echo BEFORE the pitch starts with
  the reverb of the material before the in-point and its tail ends with the event. Rejected: resetting
  those nodes at the in-point (the end still needs the material, the two edges would follow different
  rules); feeding the material only for the look-ahead and letting their tail ring (the pitched
  material is heard for ≈ 40 ms after the event, or the tail gets a 96 ms hole). A warp starting in
  hold or backwards keeps the cold edges: there is no "before" for it.
- **The echo before the pitch: cut or rings on** (pitch-tail-choice, 2026-11-03, the owner: "what does
  reverb have to do with pitch?!? well then make an option so it can be done this way and that way").
  A row of the pitch itself (`echo_before`, declared to the host as `Decl.echo_row`), not a project
  setting: the choice is about what stands before THIS pitch, and the row travels with the slot into
  morpher batches and presets. "Cut" is the context as above, byte for byte. "Rings on" keeps the body
  and, where the input reaches the out-point, FORKS the nodes up to the pitch into a twin fed silence
  that gives the output from the out-point on — the echo of the event's own sound, with nothing of the
  next material and no hole (the deepest window past the out-point 0.97 / 0.87 / 0.82 / 0.32 of the
  echo's level for WSOLA ±7 and the vocoder ±7, against the rejected alternative's 96 ms of silence).
  This needed `AudioState::fork` (every built-in kind, t1330); a cheaper second processor warmed by a
  seek pre-roll would have had its own WSOLA offsets and phases (a click) and cost a whole tail per
  event end. Rejected after measuring: restarting the ringing nodes at the in-point (no echo of the
  material before it) — a step in the pitch's input, the vocoder at 0.29 at every event start.

## Deferred

- Live sidechain and buses; a master limiter shared by preview and export.
- Oversampling of distortion/crusher; LFO shapes other than sine; tempo sync; frequency split of
  width.
- SIMD for audio nodes (the 1.3× rule from the video kernels was not applied).
- Found and not fixed: `warp_prefetch.rs` (t17) fills the legacy `automation` field when
  `fx_auto` is empty, and the migration is not called in the tests — it has no curve at all.

## Simulated devices: the sound half (sim-audio-pairs, 2026-11-04)

The owner, kkt-26.11.03b D1: a simulated device that records picture and sound simulates both. The full
decisions are in `openspec/changes/archive/2026-11-04-sim-audio-pairs/design.md`; what a later change
must not undo:

- **One device = one passport.** The audio half is built from the video half's passport; the registry
  refuses a difference. No second "device" entity: no new file form, no new undo path.
- **Mirrored in the model like the warp**, inside the same undo step; bypass and masks stay each
  side's own, so "the device's sound off" is the audio slot's checkbox.
- **Nothing crosses threads.** Each half derives the device from rows + timeline time through the
  same pure functions (`deck::row_geo`/`lt_scalar`, `mosh::Sched`). A clip-local `pos` cannot do it:
  the picture reads the absolute tick, hence `AudioState::place` (t1467 stands half a field off the grid).
- **The clean encoder resets at EVERY I-frame; only the decoder misses one.** Skipping the reset on the
  encoder side removes the mismatch, which IS the mosh.
- **A broken stream must stay bounded.** A predictor fed the wrong history ran to 56 000: its weights
  stay in the stability triangle, lines and PCM in the decoder's range. Then the sound stays broken
  until an intact I-frame, as the picture does, by itself (t1472).
- **Compander detectors attack fast and release slowly.** A symmetric 16 Hz detector pumped a lost
  field into the next one; with the carrier detector's mute the defect beats at the head's fields.
- **The head that matters is the one reading, not the field**: from the switch point (`h/2 −
  switch_lines`) the next head plays, so field B's tail carries head A's loss, in sound as in picture.
- **Own codec, not the system FFmpeg's**: the render thread opens no codec, FFmpeg's bytes are not
  frozen across versions, and a real decoder cannot be told which packets to lose.

## What an effect costs (audio-fx-cost, 2026-11-08)

The owner (kkt-26.11.07b item 5): "врубаю vhs tape на видео - терпимо, добавляю на аудио - всё". The
mechanism was per-sample work the sound did not need, in every kernel with the pattern: on the SSE2
build each `floor`/`round`/`trunc`/`fract` was a C-library call (ten of 21 kernels), and pure math of a
resting knob (`10^(dB/20)`) was redone per sample. The kernels now run at the CPU's level
(`kt_plugin_api::fast!`) and memoise such math (`dsp::Memo`); VHS Tape's sound lost its other
needless work (a `pow` that is a reciprocal square root, the curve of an unheard linear track, a
33-cell dropout walk per sample, a 64-sample shift per sample). Nothing moved a byte: t1981 holds every
kind's output, printed before the change, on both paths. A cost is read as a ratio to a reference
kernel measured in the same run (t1980, `kt-cli fx-cost`), never in milliseconds on a busy machine.
Release, in reference kernels: VHS Tape's sound 52.8 → 22.2 (heavy 77 → 30.5), gate 5.8 → 0.6,
limiter 6.0 → 1.2, distortion 4.0 → 0.35, bitcrusher 3.4 → 0.4. Left as they are, with reasons and
the full walk of the class: `changes/archive/2026-11-08-audio-fx-cost/design.md` — the phaser's
per-sample coefficients (a sound decision), the compressor's per-sample `log10`, the LFO's walk of
its rate keys, Camera ISP's and Datamosh's codec (the simulation itself).
