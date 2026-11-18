# Tasks

Test t3105 (the `features-slow` block). Branch `region-crops-copy` from `orch-26-11-01`.

## 1. The rule

- [x] 1.1 `ops::copy_clips_in` (kt-model); `copy_selection` takes it when one selected unit is cropped by the region; `msg.copied_event_part` in five catalogs
- [x] 1.2 t3105 green, red on the old code (the whole event was copied); t926's one-event case expects the part (his B10 decision); t924, t925 unchanged and green
- [x] 1.3 F-TL-013's Verdict gets the B10 line (the note is done; its body is frozen)

## 2. Live, docs, close

- [x] 2.1 Live scenario `scripts/live/region_crops_copy.py`, three screenshots (paste through Edit › Paste: Ctrl+V reads the X clipboard, which the virtual screen does not hold)
- [x] 2.2 kt-model 233/233; i18n/font/reach/layout tests
- [x] 2.3 What's new; registry B10 → done; archive
