## ADDED Requirements

### Requirement: The technical generators: a sync flash and the safe areas
The Signal check group SHALL offer a sync flash — a white frame for its flash frames at the start of
every interval, dark between with a bar filling over the interval, and a beep gated on the same ticks,
so the picture and the sound change on one frame — and safe-area guides — a transparent frame with the
action-safe and title-safe outlines (93 % and 90 % by default), a centre cross, the rule of thirds and the
4:3 centre cut, sized in fractions of the frame. Their rows SHALL be captioned in every catalog.
Why: `v26.10` G5 — the owner's technical group: "sync beep/flash, safe-area grids, timer/counter, audio
tone/noise".

#### Scenario: The flash and its beep line up
- **WHEN** a sync flash with a 2 s interval is read at 0, 2 s and the last tick of the first frame, and at 1 s
- **THEN** the frame is white at the flash ticks and dark at 1 s with the bar over the left half; the beep sounds at 0 and 2 s and is silent at 1 s; with the beep off the kind has no sound
- **PATH** Edit → Create a generator… → Signal check → Sync flash → a white preview at 0 s, its beep on A1 → the playhead at 0.5 s → a dark field, the bar half full
- **TEST** t3700 (`src/libs/kt-media/tests/gen_technical.rs`), live scenario `src/scripts/live/gen_technical.py` (`./scripts/live-check.sh`)

#### Scenario: The guides over a transparent frame
- **WHEN** the safe areas are drawn at 1920×1080 and 640×360
- **THEN** the frame is transparent away from the guides; the outlines stand at 93 % and 90 % of the frame at both sizes; the centre cross is drawn
- **PATH** Edit → Create a generator… → Signal check → Safe areas → two thin frames and a cross over the picture under them
- **TEST** t3701 (`src/libs/kt-media/tests/gen_technical.rs`)

#### Scenario: Reached from Edit and captioned
- **WHEN** Sync flash and Safe areas are created from Edit › Create a generator › Signal check; the catalogs are read
- **THEN** the flash is a picture event and its beep on A1, the guides a picture event; every number and flag row has a caption in all five languages
- **PATH** Edit → Create a generator… → Signal check → Sync flash / Safe areas
- **TEST** t3702 (`src/apps/kt-app/src/gen_audio_tests.rs`)
