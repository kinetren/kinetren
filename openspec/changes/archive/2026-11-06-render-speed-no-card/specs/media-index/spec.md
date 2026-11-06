## ADDED Requirements

### Requirement: The end of a file is met once
A decoder SHALL treat the keyframes the packet scan found as what the file holds, whatever its header
promises. It SHALL NOT seek when no keyframe lies between its cursor and the target, SHALL NOT seek
past the last keyframe the scan found, and SHALL NOT send the decoder a packet the file ended inside
of. The audio SHALL answer past the end of an ended stream without a seek. Every key past the end SHALL
be answered with the last picture the file holds.
Why: a file cut short sought and reopened for every frame past its end, a GOP each, and the audio
sought per burst: a 40 s event did not finish in 280 s.

#### Scenario: Keys past the end of a cut VP9 file
- **WHEN** a 12 s file cut to half its bytes is read in spans over 20 s
- **THEN** every key is answered, at most one reopening, under 300 pictures decoded
- **TEST** t1760 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: A seek past the cut of an MP4
- **WHEN** a file with its index at the head, cut in half, is read far past the cut
- **THEN** the read gives the last picture the file holds, at most a GOP decoded
- **TEST** t1761 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: The audio past the end
- **WHEN** the bursts of a cut file are read through its end
- **THEN** the bursts past the end answer with at most two seeks in all
- **TEST** t1762 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

### Requirement: A render converts a span's pictures on threads
A render's source SHALL convert the pictures of a multi-size span to RGBA on several threads and emit
them in key order, and the frames SHALL NOT differ from a conversion on the decoder's thread. A tight
render and a preview's source SHALL convert on one.
Why: the conversion took four times the decoding on the showcase.

#### Scenario: Threads change no frame
- **WHEN** a span of keys at two sizes with a jump in it is read with one thread and with four
- **THEN** the frames are byte for byte equal and in the same order
- **TEST** t1765 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)
