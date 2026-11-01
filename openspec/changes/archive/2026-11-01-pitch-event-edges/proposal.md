# Proposal

## Why

kkt-26.11.01 item 15 (`docs/ktprompts/kkt-26.11.01.txt`, line 25; `docs/user-feedback.md`, round of
2026-11-01): "pitch still sags: if you put, say, 15 short events in a row, the sound WITHOUT pitch is
right, and with pitch it tears". Measured on this branch before any fix (15 events of 100 ms cut from
one 440 Hz sine, pitch ±7 st, against the same material as one uncut event):

- WSOLA +7: about 320 frames (6.7 ms) of TOTAL silence right BEFORE every join; −7: about 250 frames
  of silence right AFTER every join. The largest sample step at a join is 0.46 against 0.004 inside
  the uncut event (≈ 110×).
- Vocoder: the level drops to 0.42 of the uncut level at the joins and jumps between events
  (0.6 → 0.2 → 0.67 around one join); the step is 0.68 against 0.006.

The cause is in the code: every event's pitch node starts COLD at its own edges. The hard-seek pre-roll
is clamped to the event's material (`ClipProcessor::hard_seek`: `.min(local + head_ext)`), so the ring
before the in-point is zeros; the input gate cuts the source at the out-point, so the grains near the
end read zeros past it (an upshift grain reads 1.5× ahead, a downshift grain behind); and the next
event begins a NEW stream — its own grain grid, WSOLA offsets and vocoder phases — so even with full
rings the waveform jumps at the cut. Without pitch the cut in continuous material is seamless because
nothing has a window. `pitch-loop-silence` (archived 2026-11-01) fixed the loop seam of ONE event; it
did not touch event edges.

## What Changes

- A kind may declare that its look-ahead window READS ACROSS THE EVENT'S EDGES (pitch does; the
  limiter does not — it limits what the event outputs, which is cut). For such a chain the input
  around an event edge is the event's own source: the pre-roll reaches before the in-point and the
  input continues `La` past the out-point. A content gate right after the last such node cuts the
  result back to the event, so nothing outside the event is heard and a reverb after pitch rings only
  on the event.
- A run of events cut from one material (same track, abutting, the next continues the previous one's
  source, same chain structure, no transition at the join, no tail) is ONE stream: when the next event
  starts while the previous one has just played to its end, the processor is handed on instead of
  starting cold. A split of a pitched event sounds byte for byte as the uncut event.
- An event whose chain reads across edges and has no tail, entered again at its first frame after it
  stopped (a loop lap over several events), starts fresh with the context pre-roll instead of reusing a
  stale look-ahead ring.
- Prefetch and the render ask the same extended window (one function).
- Unchanged byte for byte: chains without such a node (the limiter alone, every non-look-ahead chain),
  neutral pitch, `GOLDEN_AUDIO`, the single-event loop of `pitch-loop-silence`.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `audio-fx`: a new requirement "A look-ahead window that reads across the event's edges".
- `audio-render`: a new requirement "A split of a pitched event is one stream".

## Impact

- Code: `src/libs/kt-plugin-api` (`Decl.reads_across_edges`), `src/plugins/fx-audio-modulation`
  (pitch declares it), `src/libs/kt-fx/src/audio/{mod,nodes}.rs` (`chain_context`, the extended
  material in the window functions and in `hard_seek`/`produce`, the content gate, `hand_on`),
  `src/libs/kt-engine/src/{audio,fxstore}.rs` (the run predecessor, the hand-off).
- Docs: `src/libs/kt-fx/CLAUDE.md` (look-ahead node rules), `docs/guide/whats-new.md`.
- Tests: t1105–t1114 (`src/libs/kt-engine/tests/pitch_edges.rs`).
- No format change, no new dependency. Invariants 2, 3 and 11 hold: the hand-off is a move between
  two processors on the render thread, decided by the model alone; an edit does not destroy state;
  audio is byte-deterministic and independent of slicing.
