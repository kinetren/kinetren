## ADDED Requirements

### Requirement: A VAAPI encoder takes the composite on the card
Where the device that composes an export is the card whose VAAPI encoder encodes it, the export SHALL draw each frame's
planes into a surface of the encoder's own pool, imported as a dma-buf, and hand that surface to the encoder, with no
copy through host memory. The surface SHALL go to the encoder only after its submission finished. The samples SHALL equal
the conversion's reference, so the file does not depend on the way. A surface SHALL be imported once for the encoder's
life and released before its pool.
Why: the owner, kkt-26.11.20 A13 ("Сделать сейчас"); found-defects 741 — a frame left the card, was copied twice by the
processor and was uploaded to the same card (export-into-surface D1–D4).

#### Scenario: The two draws equal the reference
- **WHEN** composites of odd and even sizes are drawn by the luma and the chroma pass into integer targets on the test device, for BT.601 and BT.709, limited and full
- **THEN** the targets' samples equal `yuv::convert`'s NV12 planes, every one
- **TEST** t4125 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: The file is the upload path's file
- **WHEN** sixty frames are exported through the fake VA driver's exported surfaces (the import replaced by a copy into their memory), and again with `KT_EXPORT_SURFACE=off`, pipelined and serial
- **THEN** the four files' video packets are equal byte for byte and each result's written frames equal the announced; in the first the driver mapped each surface once, and no copy out, conversion or upload is counted
- **TEST** t4127 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The pool is waited for, and released
- **WHEN** an encoder keeps every surface but one until its next frame, over forty frames; then the export is cancelled midway
- **THEN** the export goes on at one frame in flight and ends whole; with an encoder that gives none back until the next frame, the frames it cannot give a surface go by upload and the export ends whole; after the cancel no imported texture and no surface is left
- **TEST** t4128 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: An incomplete frame is composed again into its surface
- **WHEN** a source fails its first read of one frame, so the composite is incomplete once
- **THEN** the frame is composed again into the same surface before the encoder gets it, and the file holds the whole picture
- **TEST** t4131 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: The card's way is proved before it is used, and left with a reason
The export SHALL use the encoder's surfaces only when the composing device imports dma-bufs, is the encoder's card, can
draw to the surface's layout, a throwaway process did so first, and a test picture drawn into a surface and read back
equals the reference. Otherwise, and from the frame where the way fails, the frames SHALL be uploaded as before, and the
log SHALL name the reason. The verdict SHALL not be kept from one export to the next.
Why: no card is on the build machine; a driver that faults takes the process with it (hw-encode-context D10); an image
of a layout the device does not support is undefined behaviour, not an error (export-into-surface D5–D7).

#### Scenario: What the driver exports decides
- **WHEN** descriptors of two one-plane layers, one two-plane layer, an unknown modifier, a third layer and no objects are read; and a device without the feature, on another PCI address, and on software workers are asked
- **THEN** the first is taken with its offsets and pitches, each other one is refused in its own words
- **TEST** t4126 (`src/libs/kt-ffmpeg/tests/surface_desc.rs`)

#### Scenario: A surface that does not hold what was drawn is not used
- **WHEN** the fake driver exports other memory than the surface's, and an export runs
- **THEN** the proof names the differing samples, every frame is uploaded, and the file equals the upload path's
- **TEST** t4129 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: A driver that faults in the export of a surface ends the throwaway process only
- **WHEN** kt-cli exports with the fake driver aborting in `vaExportSurfaceHandle`, and then refusing it
- **THEN** both renders complete by upload, and the log names the fault and the refusal
- **TEST** t4130 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: A failure midway
- **WHEN** the tenth acquire of a surface fails
- **THEN** the frames before it went by surface, it and the rest by upload, in order, and the log says from which frame and why
- **TEST** t4132 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: A real dma-buf is imported and drawn
- **WHEN** a device with the dma-buf feature imports a udmabuf of NV12 and the passes draw a picture (with `KT_EXPECT_DMABUF=1` the test fails where it cannot; without it, it says why it was skipped)
- **THEN** the buffer's bytes, read through its own mapping, are the reference's planes
- **TEST** t4135 (`src/libs/kt-gpu/tests/surface_import.rs`)
