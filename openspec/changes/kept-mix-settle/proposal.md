# Proposal: kept-mix-settle

## Why

Stage 9, the owner's run (`docs/ktprompts/kkt-26.11.10.txt`, registry `kkt-26.11.10-index.md`):

- **J1** — "при проигрывании маленького отрезка (230ms) - опять появился косяк с пустотным звуком в
  начале. но это есть например в rendereg с эффектами но нет на зацикленность_18". His trace
  (`fails/26.11.10/trace`, +240.149): a seek to a 163 ms region, the first 7 chunks handed out from
  the kept final mix (recorded at +104 during a linear play), the 8th chunk 634 frames before the wrap
  rendered live after `fx.reset_all()` and KEPT, the next 8 live chunks recording the whole second
  lap under the lap context, every later lap replayed from that recording (found-defects 390). The
  reset pre-rolls each event from the timeline; the store then keeps a lap that never heard the lap
  before it, and replays it for ever. Chains without memory do not show it.
- **J6** — "да звук открывать при Играть" (found-defects 274): on a pause cpal's ALSA worker wakes
  ~24 times a second and PipeWire's data loop ~48, handing out silence. The output device opens on
  Play and closes at rest.

## What Changes

1. **A kept chunk is what a continuous play gives.** Leaving the store, the effects go on from where
   they stopped through the replayed frames when those are within the chains' settle length (exact),
   else a reset and a discarded play of the settle length before the chunk through the same path —
   a loop's wrap included. A chunk the effects could not warm for (a starved chunk or pre-roll, a
   track stage after a seek) is not kept until the settle length has run. Trace: `mix_chunk` aux
   `warm` and `unsettled`.
2. **The class walk** (design.md D4): a seek now resets the sidechain key branches; a reel's move
   pre-rolls its own track stages.
3. **The output device opens on Play and closes at rest** (kt-app `audio_out`): an output thread owns
   the stream, opens it when the transport plays and drops it a second after it stopped; the ring
   comes back to it when the stream is dropped. The open's cost and the first callback are in the
   trace; an error at Play reaches the bell. The callback stays the master clock (invariant 6).

## Capabilities

- `audio-render` — MODIFIED "A covered range keeps its final mix"; ADDED "A reset reaches every
  stream, and every stream it reaches warms".
- `transport` — ADDED "The output device is open only while there is sound to play".

## Impact

`kt-engine` (`mix.rs`, `audio.rs`, `fxstore.rs`, `player.rs`), `kt-time` (trace constants), `kt-cli`
(`trace loop`), `kt-app` (`audio_out.rs`, `app.rs`), live `idle.py`.
