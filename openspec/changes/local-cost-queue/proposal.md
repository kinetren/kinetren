# Proposal: local-cost-queue

## Why

Row 11 of the order table of `docs/engine-audit.md` (mechanisms M7 "per-element recomputation of
per-frame / per-block constants" and M9's local part), run under the owner's standing go of 2026-11-09
("когда эти закончат - продолжай другие делать по таблице", "не надо меня ждать - запускай сам по мере
завершения все сам"). The rows of `docs/found-defects.md` it takes:

- 136: Camera ISP behind a K× barrier (K > 1) computes every sensor pixel K² times;
- 137: radial blur in spin mode computes 16 sin/cos per pixel where one set per frame suffices;
- 138: a bind group and a uniform write per pass on the composing thread;
- 118: the first loop pass composes every frame in full (72 ms a frame on llvmpipe) — only what is
  local to the composite path (memory-ledger owns keeping and eviction);
- 210: the limiter divides `ceil/thr` every sample;
- 211: the camera microphone's µ-law takes `log10(256)` (and `256^q`) per stored sample;
- 311: VHS Tape's sound re-derives its deck per chunk when it has curves: the linear track's FIR
  redesign takes ~8.5 k cosines that are the same for every design, and the dropout cache is dropped
  and walked back 33 cells whether or not its law changed;
- 312: Camera ISP's microphone builds its sqrt-Hann window (2 048 `fsin`) and two vectors every 256
  samples, and looks its rows up by name per chunk;
- 313: Pitch (WSOLA) recomputes its Hann window (1 024 `fcos`) and allocates its candidate list per grain;
- 314: the audio codec allocates scratch per frame (`dct4`/`mdct`/`imdct` buffers, predictor clones, the
  packet's bits cloned before decoding) — Datamosh's sound and Camera ISP's microphone use it.

Row 128 (Phaser) stays: it needs the owner's word. Row 119 is closed by memory-ledger.

The class is "a kernel recomputes, per element, something constant over a larger unit, or allocates per
unit". The change walks every audio and video kernel for it (`design.md` lists what was checked).

## What Changes

- **Every fix keeps the bytes.** Audio byte for byte (invariant 11): the tables and memos hold exactly the
  values the per-element code computed, in the same order of operations. Video byte for byte on the device
  between the old and the new form of each pass (a two-form switch per fix, compared EXACT in tests).
- Camera ISP at K > 1: the passes that work on the sensor's grid render at the sensor's size; the last pass
  of the graph expands to the frame. The pixel a pass computes is the same sensor pixel, read from the same
  texel value, so the frame is the same bytes; K² fewer fragments in every sensor pass.
- Radial blur, spin mode (row 137): measured, not the cost — a table on the device costs more than the
  trig it replaces, and a CPU table cannot keep the bytes; the pass stays (design D2).
- The GPU executor: a frame's uniforms written in one write per submitted piece instead of one per pass;
  bind groups reused where the same textures meet the same layout again (bounded, never keeping a
  released texture alive past the frame that released it).
- Audio: limiter ratio memo (210), µ-law constant and level table (211), FIR cosine table, a dropout cache
  keyed by its law and stepped cell by cell, and a deck kept when only rows the sound does not read move
  (311), the suppressor's window table and scratch (312), the WSOLA window table and a fixed candidate list
  (313), the codec's scratch on the stack or in the coder, no clones (314).
- 118: the first lap's composite is measured on the heavy showcase with the structured trace; what is local
  to the composite path is fixed here, the rest is written down.
- A measuring tool: `kt-cli pass-cost` (a chain's passes by the pass timer, two forms interleaved, minima,
  "same bytes" check).
- Tests t2310–t2329.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: Camera ISP at K > 1 renders its sensor passes at the sensor's size, with the bytes of the
  previous form.
- `video-plan`: the executor writes a frame's uniforms once per submitted piece and reuses bind groups;
  nothing created per frame after warm-up (t2023) still holds.
- `audio-fx`: the local costs of the limiter, Pitch, VHS Tape's sound and Camera ISP's microphone removed
  with the same bytes, at rest and with curves.
- `plugins`: the shared audio codec allocates nothing per frame.

## Impact

- `fx-video-camera` (lib.rs graph, `camera_common.wgsl` and the sensor passes, `mic.rs`), `fx-video-blur`
  (radial blur), `kt-gpu` (`engine.rs` pass sites), `fx-audio-dynamics`, `fx-audio-modulation`,
  `fx-video-tape` (`sound.rs`), `fx-video-codec` (`sound.rs`), `kt-plugin-api` (`audio_codec.rs`),
  `kt-cli` (`pass-cost`).
- No format change, no new dependency, no visible change (no What's new line): the picture and the sound
  are the same bytes.
