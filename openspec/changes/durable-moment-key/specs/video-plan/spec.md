## ADDED Requirements

### Requirement: A moment's key does not depend on what is decoded now
The key a finished frame is kept under SHALL be one function of the plan (`kt_engine::moment_key`), used by the
display's memo, the render-ahead producer, the kept-moment recheck and the disk prerender. It SHALL name a layer's
frame by the source, the source's identity (capability `media-index`, "A cached source carries the identity of what it
decodes"), the frame's index and the decode size; a stream stage by its kernel's key, which SHALL mix the
identities of the sources it reads; a reel and a track stage by their whole sub-plan. It SHALL NOT contain a content
serial, the presence or the size of a decoded frame where the identity is known, so a moment has one key before its
frames are decoded, while they are held, after they are evicted and after they are decoded again. Where the cache
knows no identity (a private cache of an export, a thumbnail, a strip) the content serial SHALL name the frame as
before. The identity SHALL be taken from the cache together with the frame, never from the renderer: a picture
made of frames of old values or of an old file is never kept under the new ones' key. A picture with a frame whose
identity does not outlive the process SHALL NOT be written to or read from disk. The memory's key MAY add what only this
process knows (the run forms' revision).
Why: kkt-26.11.22 D2, D7 and found-defects 965 — the memo's key held the serial of each decoded frame, 0 for one not
decoded, so a kept composite was found only while its sources were decoded too; leaving them out of the cache turned
21 frames a second into 2. Found-defects 1061–1063.

#### Scenario: One key before, during and after the decode
- **WHEN** a moment of a demo with a file, a generator, a blended pair, a reel and a Datamosh event is keyed by the probe plan over an empty cache, with its frames decoded, after other frames evicted them, and decoded again; then its file is adopted under another identity
- **THEN** the first four keys are equal and equal the full plan's, the display's memo answers "held" to each once the moment was presented, and the last key differs; two private caches with no identity, the same media id and different frames still give two keys
- **TEST** t4108 (`src/libs/kt-engine/tests/moment_key.rs`)

#### Scenario: An old picture is never kept under new values
- **WHEN** a title's text is edited while its worker is held before the retune, the new renderer's moment is composed from the old frame and kept, and then the worker applies the retune and decodes
- **THEN** the first composite is not answered for the new moment's key: the new frame is composed and shown, and an undo to the old text shows the kept one with nothing composed
- **TEST** t4109 (`src/libs/kt-engine/tests/moment_key.rs`)

#### Scenario: Memory and disk name a moment alike
- **WHEN** a moment with durable sources and one with a placeholder's frame are prerendered, the first is restored into an empty memo by a new player over the same store, and the file is then replaced by another under the same media id
- **THEN** the restored picture is held under the moment's own present key; the placeholder's moment was neither written nor asked of the store; after the replacement the old picture is not shown
- **TEST** t4115 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: A relinked file starts a new history
- **WHEN** a Datamosh moment is composed, its event's media is replaced by another file under the same media id with the same timing, and the moment is composed again, also through a slot in front of the stage and through a second input
- **THEN** each time the stage reads the new file: the picture equals a fresh renderer's over a fresh memo
- **TEST** t4116 (`src/libs/kt-engine/tests/moment_key.rs`)
