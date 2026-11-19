# Proposal

## Why

The owner (`docs/ktprompts/kkt-26.11.01b.txt` line 11, registry B10): copying the part of an event
under the region copies the whole event when the event is highlighted — the rule of `v26.10.18` item
4 ("one selected event → copy takes it, not the region"). Taken on 2026-11-01 (`docs/user-feedback.md`,
"the second list"): both hold if the region CROPS the selected event — a region over the selected event
copies that event's part under the region and nothing of the other tracks; a region that does not
touch it leaves the whole event. The earlier complaint was the region taking every track, not the
cropping.

## What Changes

- Copy with exactly one selected event (link partners count as one) and a region that crosses it
  without covering it: the clipboard holds that event and its partners CROPPED to the region, rebased
  to the crop's start, and nothing of the other events under the region. The status line says "Copied
  the selected event's part under the region: N".
- A region that covers the event whole, or does not touch it: the whole event, as before (t924).
- Several selected or none: unchanged (t925).
- t926's one-event case (a range marker inside one event, double-clicked) now copies the part under
  the marker — the owner's B10 rule; its scenario is modified here.

## Capabilities

- `ui-timeline`: MODIFIED requirement "Multiple selection, marquee, moving a batch, double clicks" (the
  t926 scenario; a new scenario t3105).

## Impact

`src/libs/kt-model/src/ops.rs` (`copy_clips_in`), `src/apps/kt-app/src/app.rs` (`copy_selection`),
five catalogs (one key). Tests t3105, t926 (expectation changed by his decision); live scenario
`region_crops_copy`.
