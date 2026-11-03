# Design: sim-audio-pairs

## Context

Three simulation video effects model devices that record sound. Each needs its audio half, a
simulation too, driven by the same device. Constraints: invariant 1 (audio f32 48 kHz stereo inside),
2 (FX state only on the audio render thread), 3 (a model edit keeps FX state), 11 (determinism; frozen
math), 5 (no decoder/codec opened by the render thread).

## Decisions

### D1. One device = one passport, declared by the video half
`Decl.pair: Option<&'static str>` on the VIDEO kind names the audio kind. The audio kind's passport
and row forms are IDENTICAL (the same function builds both); the registry refuses a pair whose
passports differ, a pair that is not an audio effect, an audio kind that declares a pair, and two
video kinds naming one audio kind (`pair_fits`, `Registry::register`, either registration order).
`Registry::counterpart(id)` answers both directions; `unpaired()` lists declared pairs whose audio half
is missing — the assembler asserts it is empty. Rejected: a separate "device" object referenced by
both slots (a new model entity, a new file form, a new undo path — the passport already IS the device
state); a pair declared by the audio side (the video effect is what a person adds first).

### D2. The model mirrors the pair across a link group, like the warp
`sync_pairs_to_link_group` runs where `sync_warp_to_link_group` runs (SetFx, SetFxAuto, SetFxBypass,
inside `mutate_on`, so the mirror is in the same undo step). The n-th slot of a paired kind on the
edited event matches the n-th slot of its counterpart on every partner of the other kind: values and
curves copied, a missing one appended, an extra one removed (presence is shared: one device). Bypass
and masks are each side's own — the audio slot's checkbox is "do not let the device's sound through".
Two VHS Tape slots (a dub of a dub) give two sound slots. A morpher patch hands the other half to the
other kind's events in `chain_for`/`auto_for` (`derived_pairs`, one text for both) unless the patch
already holds that slot itself.

### D3. The audio half derives the device state from rows + timeline time
The picture reads the absolute timeline tick (`burst_index`); an audio state only got its clip-local
frame. `AudioState::place(Placement { start, fps })` gives it the timeline frame of its clip's frame 0
and the project frame rate. kt-fx `ClipProcessor::set_origin` (no reset — a moved clip keeps its
nodes) and `ChainOpts.fps`; kt-engine `FxStateStore::sync_clip` passes the clip's start. Nothing is
shared between the threads: both halves call the same pure functions of (rows, time) — for VHS
`Deck::words`, `deck::row_geo`, `deck::lt_scalar`; for the datamosh the GOP grid and gates; for the
camera the looks' rows and the sensor seed.

### D4. The VHS sound (F-FX-012)
Two systems, chosen by the deck's audio output row (Hi-Fi / normal / mix), recorded from one input:
- Linear: record level and bias (a tanh transfer bent harder by under-bias; over-bias adds
  self-erasure as spacing), the tape's displacement by the capstan (the picture's 5.3 Hz flutter
  component, phase 0.37; 43.2 samples per µs of the jitter row, i.e. 0.15 % at the default) plus scrape
  flutter, oxide defects at the picture's rate scaled by the swept areas, `L = dropout_us·v_write` long,
  `L / v_tape` in time, as deep as they cover the track; hiss from the track width and the tape's C/N;
  the head's losses (gap sinc, Wallace spacing, coating thickness) at λ = v_tape / f against a fixed
  playback EQ designed for the nominal head at SP (+20 dB cap) — a 65-tap zero-phase FIR; Dolby B as a
  sliding high band with an exact per-sample decoder.
- Hi-Fi: emphasis and a 2:1 RMS compander around FM carriers (1.4/1.8 MHz PAL, 1.3/1.7 NTSC, ±150 kHz)
  simulated as complex baseband at 8× the audio rate; per picture LINE the carrier amplitude of the
  head that plays it (`row_geo`: the head, the switch, the tracking coverage) through the spacing and
  gap losses at the carrier's wavelength and the heads' outputs; the picture's dropout field cuts the
  carrier on its line; complex noise from the C/N row (8 dB below the video FM's); a discriminator
  (FM threshold clicks come by themselves); the switching pulse holds the sample of the head switch;
  with the DOC on the deck holds through a lost carrier. The Hi-Fi heads take the condition of the
  video heads they ride beside (one head chip per channel): a clogged head A loses field A in both.
- Delays: the state reads input `LA = 96` frames ahead (the declared look-ahead): Hi-Fi's
  flutter line is centred there, the linear track's at `LA − 32` before its 65-tap filter.
- Still: a stopped tape plays nothing and the deck mutes Hi-Fi in trick play.

### D5. Placement of the new rows
The deck's sound rows go at the END of the passport (row 32…): the picture's words never read them, so
the picture keeps its bytes; with `omit_default` a project without them writes the former text.

### D6. The stream's sound: an own codec, the picture's schedule (F-FX-013)
An own small codec, not the system FFmpeg's AAC: the audio render thread opens no codec (invariant 5,
the callback rules), its bytes are not frozen across FFmpeg versions (invariant 11), and a real decoder
cannot be told which packets to lose. `kt_plugin_api::audio_codec`: MDCT 2048/1024 through a 256-point
FFT, AAC-Main-like order-2 NLMS per line with resets, one global step on a 1.5 dB ladder chosen by a
rate control, Rice lines in bands of 16, a bit reservoir (a frame's data may begin in earlier slots),
concealment by the last spectrum at −3 dB with scrambled signs.
The schedule is the picture's own: `MoshSound::sched(n)` builds `mosh::Sched` over a `StreamLayout` of
the audio event (cut from the placement, `first = cut − before`, rows from the slot's curves) and asks
`sched_i`, `dropped`, `pkt_pos` for the positions falling in audio frame `n`. The clean encoder resets
at EVERY I-frame; only the decoder misses a deleted one — otherwise the mismatch, which is the mosh,
would not exist. The audio frames sit on the absolute timeline grid.
Found by t1472: a decoder on the wrong history ran away (error 56 000): the weights are held inside
the stability triangle, the lines and the PCM to the decoder's range. After that the picture's
behaviour came by itself: the sound stays broken until an intact I-frame.
An audio stream cannot carry over the sound of the event before (the state sees its own event's
input); `first = cut − before` assumes the event before is long enough.

## Risks / Trade-offs
- The Hi-Fi FM costs ~16 complex samples per audio frame; measured in tasks (release).
- An audio stream of the datamosh cannot carry over the sound of the event before (the audio state
  sees only its event's input) — written in the note.
