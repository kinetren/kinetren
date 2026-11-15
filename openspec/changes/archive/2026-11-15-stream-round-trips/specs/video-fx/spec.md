## ADDED Requirements

### Requirement: A stream stage's walk keeps its state on the compute device
A history walk of a stream stage SHALL keep the decoder's picture and the clean encoder's state on the
compute device between its batches and leave them in the stream memo's live tier at its end; a later
walk SHALL continue from them with no upload. What comes back to the processor SHALL be only the frame
the host shows, the checkpoints and a region's clean picture, as 8-bit planes. A refresh I-frame the walk
encodes SHALL be kept beside it, and a region of interest at a held position SHALL be encoded in the
walk's own batch. The pictures SHALL be the per-position walk's.
Why: found-defects 401, 621, 402, 405, 135 — each batch read its state back and the next uploaded it,
and at 1080 lines the memo full of pictures evicted the encoder's state so every frame encoded its GOP
again (795 encodes for 196 positions, 104.6 MB uploaded a frame).

#### Scenario: A walk goes on from the device
- **WHEN** a cut mosh is played frame by frame from a memo that holds two pictures
- **THEN** the walk uploads nothing but its source frames, encodes each position once, reads back no more than the shown frame and the checkpoints as 8-bit planes, keeps buffers on the device, and every picture is the per-position walk's
- **TEST** t2700, t2706 (`src/plugins/fx-video-codec/tests/walk.rs`)

#### Scenario: A refresh is kept beside the walk
- **WHEN** a walk past an intra refresh is followed by a loss of the LRU and a re-walk to the refresh
- **THEN** the re-walk encodes nothing and gives the same picture
- **TEST** t2703 (`src/plugins/fx-video-codec/tests/walk.rs`)

#### Scenario: A region at a held position is one job
- **WHEN** a frame at a dropped I-frame is rendered with a region of interest after its predecessor
- **THEN** it takes one compute job and is the per-position walk's picture
- **TEST** t2704 (`src/plugins/fx-video-codec/tests/walk.rs`)

### Requirement: The shown stream frame costs one pass on the processor
The conversion of a stream stage's picture back to RGBA SHALL run row by row with the bytes of the
per-pixel conversion, and at the decode size the stage's RGBA SHALL be the layer itself: `fit_into` at a
frame's own size is a copy to the byte.
Why: found-defects 620 — 6–15 ms a 1080-line stage on the composite's thread for a conversion and a
same-size bilinear.

#### Scenario: The conversion back is the reference
- **WHEN** random planes are converted at full size and at several crops
- **THEN** the row-by-row and the per-pixel conversions give the same bytes
- **TEST** t2701 (`src/plugins/fx-video-codec/tests/walk.rs`)

#### Scenario: fit_into at its own size is the bilinear
- **WHEN** random frames from 1×1 to 1920×1080 are fitted to their own size
- **THEN** the result is the general bilinear's and the frame itself
- **TEST** t2702 (`src/libs/kt-fx/tests/fx_thumb.rs`)
