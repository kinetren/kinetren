## MODIFIED Requirements

### Requirement: A render converts a span's pictures on threads
A render's source SHALL convert the pictures of a multi-size span to RGBA on several threads and emit
them in key order, and the frames SHALL NOT differ from a conversion on the decoder's thread. A tight
render converts on one. A preview's source SHALL take the count a render of the project's frame size
takes by the memory measured when the project opens.
Why: the conversion took four times the decoding on the showcase. The preview's sources converted on one
thread although the split changes no byte (gpu-throughput, found-defects 485: work a person waits for —
Loading, a history's decode — ran on one core of sixteen).

#### Scenario: Threads change no frame
- **WHEN** a span of keys at two sizes with a jump in it is read with one thread and with four
- **THEN** the frames are byte for byte equal and in the same order
- **TEST** t1765 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: Opening a project sets the preview's conversion threads
- **WHEN** the conversion count is one and a project is opened in the editor with the memory measured not tight
- **THEN** a source opened now converts a span on the render's count (four)
- **TEST** t2440 (`src/apps/kt-app/src/diag_tests.rs`)
