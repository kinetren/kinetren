# Proposal

## Why

Found by `pitch-tail-choice` (its design, Risks) and left as is since `pitch-event-edges` D4: when a
loop region starts at (or inside) the start of an event whose chain has an effect with a tail (an
echo, a reverb) BEFORE a Pitch, every loop pass begins with a dip. Measured on a 440 Hz tone, an echo of
100 ms before a +7 pitch, the region holding the event and 200 ms past it — the deepest 64-frame
window of the first 1 024 frames of every lap after the first, against playing from the region start:
"rings on" 0.000 (≈ 75 ms of silence), "cut" 0.42–0.49 with sound from past the out-point mixed in;
with a feedback echo still ringing at the wrap both choices dip (0.01–0.37).

The cause: D4 sends a chain with a tail through a SEAM at the wrap (R.28: a reverb rings through the
circle), so the pitch's look-ahead ring still holds the input of where the previous lap stopped — past
the out-point: silence with "rings on", the material past the out-point with "cut". A seek to the same
place reads the context pre-roll and fills the ring with what will sound.

## What Changes

- A new lap of an event whose chain reads across its edges and has a tail (the event entered again at
  its first frame, or after its out-point, not as the expected input-first wrap) starts as a seek
  there does: a reset and the context pre-roll.
- What the previous lap still rings past its out-point is not dropped: the processor that played it
  goes on as a stream of its own and is summed over the new lap (at the tail zone's weight), as two
  copies of the event one after another sound. At most four earlier laps ring at once (a region
  shorter than the tail drops the oldest, four laps into its decay); a lap that rang out becomes the
  spare the next lap starts with, so a steady loop allocates nothing. A seek drops them.
- With "cut" nothing of an earlier lap is heard past its out-point, so a lap is the play from its
  start byte for byte; with "rings on" it is that plus the earlier laps' echo.
- Projects without a loop: byte for byte unchanged (a lap entry needs the output to jump back into
  the event). Chains without a tail or without the context: unchanged.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `audio-render`: a requirement for the loop lap of a context chain with a tail.

## Impact

- `src/libs/kt-fx/src/audio/mod.rs` (`ClipProcessor::lap_entry`, `past_out_point`; `read` seeks on a
  lap entry).
- `src/libs/kt-engine/src/fxstore.rs` (`FxStateStore::start_lap`, `laps_mut`, `retire_laps`,
  `MAX_LAPS`), `src/libs/kt-engine/src/audio.rs` (`render_fx_clip` sums the earlier laps).
- Tests t1440, t1441 (`src/libs/kt-engine/tests/pitch_tail.rs`); a live loop scenario; What's new.
