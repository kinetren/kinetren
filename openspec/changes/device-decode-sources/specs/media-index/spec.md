## ADDED Requirements

### Requirement: A picture decoded on the device is the processor's picture
A video picture decoded on the device (VAAPI, D3D11VA) SHALL be laid out, by an exact copy, in the pixel
format the processor's decoder gives for the same stream (NV12 → yuv420p or yuvj420p, P010 →
yuv420p10le) with its time and colour tags, before the one conversion every picture takes, so that the
colour rule, the conversion context's key and every RGBA byte are those of the processor's decode. No
cache key, durable key, pixel version or build salt SHALL name the decoder.
Why: device-decode-sources D1 — H.264, HEVC, VP9 and AV1 define their decoded samples exactly; the only
difference between the two decodes was our NV12 conversion path (mean 1.46, t3129), and a key that does
not name the decoder (invariant 4) must not hold two different frames.

#### Scenario: The device decode equals the processor's byte for byte
- **WHEN** a file the fake VA driver's encoder wrote (8-bit, and a 10-bit P010 variant where the driver writes one) is read out of order at full size, at a decode step and in a span at two sizes, once on the fake device and once on the processor, on every installed FFmpeg major
- **THEN** every RGBA frame is byte-for-byte equal between the two, the device read names `vaapi <node>` and the processor read names the processor
- **TEST** t3300 (`src/libs/kt-media/tests/segment.rs`)

#### Scenario: The layout copy is exact
- **WHEN** synthetic NV12 and P010 pictures with odd widths, a bottom crop and full-range tags are laid out as yuv420p, yuvj420p and yuv420p10le
- **THEN** every sample equals the source sample (P010 shifted right by six), the tags and the time are kept, and the line padding is never read
- **TEST** t3301 (`src/libs/kt-ffmpeg/tests/device_layout.rs`)

### Requirement: A file decodes on the device only when its decode is exact and proven
A file's video SHALL decode on the device only when its codec defines exact decoding (H.264, HEVC, VP9,
AV1), its flavour is one the device takes (4:2:0, 8 or 10 bit, offered by the decoder's format list), it
has no top or left crop, and its first keyframe decoded on the device equals, laid out, the processor's
decode of it. The decision SHALL be made once per file identity and device per process, and SHALL be a
function of the file; any other file decodes on the processor and the source's report SHALL name why. A
device decode error after the decision SHALL reopen that decoder on the processor at the same keyframe,
mark the file processor-only for the process and name the fault; the frames SHALL not change.
Why: device-decode-sources D2, D6 — MPEG-2/MPEG-4 Part 2 allow IDCT mismatch, libavcodec cannot crop a
hardware frame at its top or left, and a driver can deviate from the standard.

#### Scenario: The eligibility table
- **WHEN** streams of H.264 4:2:0 8-bit, HEVC Main 10, H.264 4:2:2, MPEG-2, MPEG-4 Part 2, ProRes and an H.264 with a top crop are asked whether they may decode on the device
- **THEN** the first two may, each other one is refused with its reason (flavour, inexact codec, no device decode, crop)
- **TEST** t3302 (`src/libs/kt-ffmpeg/tests/device_layout.rs`)

#### Scenario: A device whose samples differ is not used
- **WHEN** the fake VA driver is told to change one sample of each picture it decodes, and a file is opened for the preview
- **THEN** the proof fails, every decoder of the file decodes on the processor, the report names "samples differ", and the frames equal the processor's
- **TEST** t3303 (`src/libs/kt-media/tests/segment.rs`)

#### Scenario: A device that faults mid-stream hands over to the processor
- **WHEN** the fake VA driver fails the tenth picture's decode of a file read forward
- **THEN** the read completes, every frame equals the processor's decode, the decoder reopened on the processor once, and the report names the fault
- **TEST** t3306 (`src/libs/kt-media/tests/segment.rs`)

### Requirement: One decode device per process; decoders and their surfaces stay with their owner
The process SHALL open its decode device once, under the per-node verdict, and every device decoder SHALL
hold a reference to it; each decoder's surface pool SHALL be created by and live with that decoder, owned
by the thread that owns the decoder (invariant 5). The preview's sources and the export's sources SHALL
open their decoders this way; thumbnails, filmstrips, bin scrubbing and stills SHALL decode on the
processor.
Why: device-decode-sources D3, D9 — a VA display per decoder (today's `open_hw`) multiplied by a worker
per source and up to four decoders per file; a single picture does not repay a device decoder's open.

#### Scenario: Four threads share one device
- **WHEN** four sources of fake-driver files are read at once on four threads, each with two decoders
- **THEN** the driver counts one display initialisation, every decoder decodes on it, and every frame equals the processor's decode
- **TEST** t3304 (`src/libs/kt-media/tests/segment.rs`)

#### Scenario: Who opens on the device
- **WHEN** the preview's worker opens a fake-driver file, an export opens it, and the thumbnail service, the filmstrip and a still open theirs
- **THEN** the worker's and the export's sources report the device, the thumbnail, filmstrip and still decodes report the processor; on a machine with no render node every one reports the processor and why
- **TEST** t3309 (`src/libs/kt-media/tests/segment.rs`), t3310 (`src/apps/kt-app/src/thumbs.rs`)

### Requirement: Device decoders live within the memory ledger
A device decoder's surface pool SHALL be counted in the ledger's fixed tenant `Surfaces` and SHALL fit, with the
pools already held, in a bound of at most 1 GiB and at most a quarter of the memory available when the decoder opens.
A decoder whose pool does not fit SHALL open on the processor, say so, and ask again at its next backward reopen; a
decoder on the device SHALL stay on it across a reopen. An export's claim SHALL not change the bound. A decoder opened
later for the same source SHALL take the settings the source was opened with. None of it SHALL change a byte of a frame.
Why: device-decode-sources D4, D6; found-defects 1064 — a 4K H.264 decoder holds 17+ surfaces of 12 MB; Vegas's NVDEC is
reported to take all video memory, Resolve uses the device only while memory is left (F-MED-010). A growing tenant is
granted nothing while an export lives, so the export would never decode on the device; and a render's second decoder of
a file took the preview's settings, being opened outside the render's scope.

#### Scenario: A pool that does not fit decodes on the processor with the same frames
- **WHEN** the bound takes the pools of two device decoders of a file and not a third; a backward seek reopens each; a claim is taken; another source's decoder is dropped and the third reopens
- **THEN** two decoders report the device and the third the processor with the room it lacked; the kinds survive the reopen and the claim; after the drop the third's reopen is on the device; every frame equals the processor's decode; after the source's drop the tenant holds zero bytes
- **TEST** t3305 (`src/libs/kt-media/tests/segment.rs`)

#### Scenario: A source's later decoder keeps the source's settings
- **WHEN** a source is opened under a render's settings (3 threads, the device off) and a span read on another thread, outside the scope, opens its second decoder
- **THEN** the second decoder has 3 threads and decodes on the processor
- **TEST** t3305 (`src/libs/kt-media/tests/segment.rs`)

### Requirement: Every source of a preview or an export opens through one factory call
The preview's, the render window's and kt-cli's sources SHALL open through `factory::open_source(path, index,
purpose)`, which decides the device (eligibility, the setting, the ledger) in one place. Thumbnails, the
filmstrip and stills SHALL keep their own software opens.
Why: the sources opened at four places (`util.rs` through the factory, `render_dialog.rs` `open_all`,
kt-cli's `open_render_sources` and `open_sources`); a rule decided at one of them would not reach the
others (device-decode-sources D6).

#### Scenario: The four openers ask the factory
- **WHEN** the preview, the render window's source list, kt-cli `export` and kt-cli `probe --time` open the same fake-driver file, and a facts query and the audio-reactive series open it too
- **THEN** each source names the same decoder kind and reason, the render window's source reads the packet index instead of scanning the file, and the last two open no device decoder
- **TEST** t4102 (`src/libs/kt-media/tests/segment.rs`)
