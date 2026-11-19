## ADDED Requirements

### Requirement: An effect's centre is dragged on the preview
When the selected slot of an open effects window has a centre pair (`center_x`/`center_y`), the paused
preview SHALL show a crosshair at that centre in the frame the slot works on (the source placed by the
Geometry node before it, the canvas after it). Dragging it SHALL write the pair in one history step —
the base, or the key at the playhead when the pair has a curve. There SHALL be no crosshair during
playback or for an effect without a centre. The parameters grid SHALL keep every `_x`/`_y` pair on
one row. The hover text SHALL exist in all five catalogs.
Why: `v26.10` line 52 (T10): "Radial Blur имеет параметры центра X Y, и эти параметр находятся на
разных строках. Мало того - я еще не могу как в том же Vegas указать центр по самой превью".

#### Scenario: The centre is dragged
- **WHEN** a Radial Blur slot is selected and its crosshair is dragged a quarter of the picture right and up
- **THEN** the crosshair started at the picture's middle; the centre is (0.5, −0.5) as one history step; the crosshair stands where it was dropped
- **PATH** double click a clip → + effect… → Radial Blur → a crosshair at the middle of the preview → drag it up and right → the rays spread from there, Center X/Y read 0.5 and −0.5
- **TEST** t3123 (`src/apps/kt-app/src/input_work_tests.rs`), live scenario `src/scripts/live/fx_center.py` (`./scripts/live-check.sh`)

#### Scenario: A curve gets its key
- **WHEN** the centre's X has a curve and the crosshair is dragged
- **THEN** a key at the playhead's material moment holds the new X; the base is unchanged
- **PATH** a centre with a curve → drag the crosshair → a key appears on the graph at the cursor
- **TEST** t3124 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: Only where a centre is
- **WHEN** a Blur slot is selected; then Radial Blur during playback; the catalogs are read
- **THEN** no crosshair for the blur, none during playback; the hover text exists in all five catalogs
- **PATH** select Blur → no crosshair → select Radial Blur, press Play → the crosshair hides until the pause
- **TEST** t3125 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: X and Y on one row
- **WHEN** Radial Blur's parameters are drawn
- **THEN** Center X and Center Y stand on one row, Amount on the row before
- **PATH** select Radial Blur → Parameters: Amount; Center X and Center Y side by side
- **TEST** t3126 (`src/apps/kt-app/src/input_work_tests.rs`)
