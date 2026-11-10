## ADDED Requirements

### Requirement: The shared audio codec allocates nothing per frame
`audio_codec`'s transforms (`dct4`, `mdct`, `imdct`) SHALL use stack scratch, the encoder and decoder
SHALL update their predictors without copying the prediction, and their callers SHALL decode a packet's
bits without cloning them unless they damage them (Datamosh's flips, into a buffer the caller keeps). The
coded and decoded samples SHALL be the same bytes.
Why: found-defects row 314: four vectors, a prediction clone and a payload clone per 1024-sample frame and
channel in Datamosh's sound and Camera ISP's microphone.

#### Scenario: A frame of the codec allocates nothing
- **WHEN** an encoder and a decoder, predicting and not, code 64 frames after a warm-up of 4, with a counting allocator on the thread
- **THEN** the 64 frames allocate nothing but each packet's payload, and the decoded samples equal the digest printed before the change
- **TEST** t2317 (`src/libs/kt-plugin-api/tests/codec_alloc.rs`)
