## ADDED Requirements

### Requirement: The decoder reads what it is asked, once, without corrupting memory
The RGBA conversion of a decoded picture SHALL write into a frame buffer with
`kt_ffmpeg::RGBA_TAIL_PAD` spare bytes after the frame, cut off after the conversion — libswscale's
x86 kernels write past the last row when the width is not a multiple of their block. A read whose
target is past the decoder's deferred frame SHALL decide between decoding on and seeking by the
forward bound, never decode every picture up to it; every key of a span SHALL be decided so.
A span asked at several decode qualities SHALL decode each picture once and convert it to every
quality asked at it. The export's own source of a file read at two places at once SHALL give each
place its own decoder (at most two per file), chosen by the requests alone. The binaries SHALL fix
the allocator's mmap threshold at 1 MiB before anything else, so a freed frame-sized buffer goes
back to the system instead of a thread's arena.
Why: the showcase export (export-throughput, 2026-11-03) died within three seconds with
`malloc(): corrupted top size` — a 1080×1080 source's conversion wrote 32 bytes into the next
heap chunk's header. Once it ran, its decoders produced 101,009 pictures to deliver 5,644: a
jump of two minutes forward was decoded linearly (3,733 4K pictures for one event), and a
picture-in-picture of the same footage 7 s earlier made one decoder seek back and decode a GOP
for every frame. Pipelined, glibc's arenas then held 9 GB of freed frames: 16.2 GB peak.

#### Scenario: The conversion's overrun fits the pad
- **WHEN** YUV420P pictures of the owner's 1080×1080 and of every even width up to 130 are
  converted to RGBA at their size and from three times their width, through the system FFmpeg
- **THEN** no conversion writes more than `RGBA_TAIL_PAD` bytes past the frame (the worst
  measured is 56 bytes)
- **TEST** t1450 (`src/libs/kt-ffmpeg/tests/rgba_tail.rs`)

#### Scenario: A far forward span seeks
- **WHEN** a span is read at 0 s and the next span at 10 s of a 30 fps file with a keyframe a
  second
- **THEN** the second span decodes at most a GOP and its frames (45 pictures, not 300), and its
  frames equal a fresh source's
- **TEST** t1453 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: A span at two qualities decodes once
- **WHEN** forty keys on a 25 fps grid are asked at 160×90 and 96×54 in one span of a 30 fps file
- **THEN** the decoder opens once, decodes at most a GOP plus the keys, and each quality's frames
  equal a span at that quality alone
- **TEST** t1454 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: Two places of one file get two decoders
- **WHEN** thirty one-frame chunks each ask the file at 1 s + n and at 8 s + n
- **THEN** the decoders open at most twice and decode at most two GOPs and the sixty frames (one
  decoder: sixty reopens), and the frames equal a fresh source's
- **TEST** t1455 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: Frame-sized buffers return to the system
- **WHEN** the process starts (kt-cli, kt-app)
- **THEN** the first statement of each entry point fixes the threshold, and on glibc the allocator
  takes it (the showcase's pipelined export peaks at 7.3 GB, not 16.2 GB)
- **TEST** t1458 (`src/libs/kt-media/src/frame_alloc.rs`)

