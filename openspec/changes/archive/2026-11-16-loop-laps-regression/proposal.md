# Proposal: loop-laps-regression

## Why

kkt-26.11.16: a loop over the owner's problem region plays through without Loading only from about the
25th lap in kt26.11.16, from the 11th in kt26.11.10, and a piece without Datamosh plays no faster. His
two trace pairs show where the time went: on a display that fills its period the ring admitted the
look-ahead, its stream walk and the loop fill only a quarter of the bound per period (gpu-leftovers D4,
found-defects 491), in pieces of at most half the bound (gpu-throughput D1), so the device stood idle 25 s
with nothing queued while near work waited 486 % of its wall. `trace loop` also read an incomplete
composite of an earlier lap as "the frame's key changed", which looked like key churn.

## What Changes

- The ring's room for lower classes: `near` keeps the bound whatever the display costs, `visible` and
  `background` at least a quarter of it, an export (`offline`) only the display's slack. No piece is
  longer than the bound; the display's present waits behind at most `AHEAD_PIECES` lower pieces, each
  at most half the bound (kt26.11.10 put whole jobs of 380 ms in front of it).
- `trace loop` names a moment's key by its complete composites only.

## Capabilities

### Modified Capabilities
- `video-plan`: the room a saturating display leaves lower classes.
- `telemetry`: `trace loop`'s reasons.

## Impact

kt-gpu (`arbiter.rs`), kt-cli (`trace_query.rs`). No format change; OTIO unaffected.
