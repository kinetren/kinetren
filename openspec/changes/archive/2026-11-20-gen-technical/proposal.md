# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 16, row G5, his answer of 2026-10-30 to question 6): very
few generators — all four groups, built as plugins; the technical group is "sync beep/flash, safe-area
grids, timer/counter, audio tone/noise". The tone and the white/pink/brown audio noise already exist.

## What Changes

- `syncflash` (Signal check): a white field for N frames at the start of every interval, a dark field
  with a bar filling over the interval between them; a beep gated on the same ticks (on by default),
  so a picture event and its sound on A1 come together.
- `safearea` (Signal check): a transparent overlay — action-safe 93 % and title-safe 90 % frames
  (SMPTE ST 2046-1), a centre cross, the rule of thirds, the 4:3 centre cut; sizes in fractions of the
  frame, the colour with the eyedropper.
- The timer/counter is NOT a kind here: by his G3 decision it is a source of the Text generator
  (next change `text-counter-source`).

## Capabilities

- `generators`: ADDED requirement "The technical generators: a sync flash and the safe areas".

## Impact

`src/plugins/generators-test`, `kt-plugins-builtin` (`BUILTIN_IDS`), kt-app catalogs ×5 and the
label tables; t1048 golden rows, t1163's count; tests t3700–t3702; live `gen_technical`.
