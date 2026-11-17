## ADDED Requirements

### Requirement: Every read of a file chooses its decoder per key
`FfmpegSource` SHALL read every video request — a frame, a span at one size, a span at several sizes — through
one rule: a run of keys goes to the decoder that reaches its first key by decoding on (the nearest), and goes
on while that decoder, standing at the run's last key, is the nearest that reaches the next key; every key
decides between decoding on and seeking. A fresh decoder SHALL open only when no open decoder is free to go —
each stands ahead of the key, has read a key of this request, or is the one a later key of this request
would be given — and while the file holds fewer than its picture allows (two for 4K, as many as fit two
4K pictures for a smaller file, at most four). Otherwise the free decoder standing longest unused goes, and
a decoder a later key of the request would be given is the last to be sent away. The choice is a function
of the requests, never of timing; a decoded frame does not depend on the decoder that read it.
Why: found-defects 712 (kkt-26.11.15 N2–N4): the heavy showcase's 20–30 s decoded 12,289 pictures for 1,924
used. Keys of two places of one file closer than 2 s were one run on one decoder, which walked from the
first to the second and sought back to the first in the next chunk; three places of a file thrashed two
decoders; the least recently used decoder sent away was often the next run's; a span at one size sought
only before its first key (decode-waste design D0–D4).

#### Scenario: Two near places of one file keep a decoder each
- **WHEN** thirty one-frame chunks each ask a 30 fps file with a keyframe a second at 1.0 s + n and 2.5 s + n
- **THEN** the decoders open at most three times and decode at most the first chunk's walk, two GOPs and the sixty frames (one decoder walking between them: thirty reopens, 1,845 pictures), and the frames equal a fresh source's
- **TEST** t2880 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: Decoders per file follow its picture
- **WHEN** thirty one-frame chunks each ask a 320×180 file at 1 s, 5 s and 9 s + n
- **THEN** three decoders serve them, at most three opens and three GOPs plus the ninety frames (two decoders: ninety reopens); a 4K, 8K or 3840×1608 file is allowed two decoders, a 2048×858 or 1080p file four
- **TEST** t2881 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: A decoder a later key of the request needs is not sent away
- **WHEN** four places of the file are asked in every chunk and every fifth chunk adds a key no decoder reaches between the second and third place
- **THEN** each stray key costs at most two opens (no cascade through the later places: sixty opens and 3,420 pictures with the least recently used rule)
- **TEST** t2882 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: A span at one size seeks per key
- **WHEN** one span at one size asks 1 s and 10 s of the file
- **THEN** at most two GOPs of pictures are decoded, not the 270 between (272 before), and both frames equal a fresh source's
- **TEST** t2883 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: A forward sweep keeps one decoder
- **WHEN** single frames at 0.2, 4.5, 9.0 and 11.5 s are read one request each, then one at 1 s
- **THEN** the sweep holds one decoder, and the key behind it opens a second
- **TEST** t2884 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)
