## ADDED Requirements

### Requirement: A stream stage's history survives a restart without loss
A stream kernel MAY give an exact codec of its memo entries (`EntryCodec`: a version, encode, decode).
For such a kernel the host SHALL keep a durable tier behind its memo: an entry the memory lacks SHALL be
looked up there by `mix(entry key, salt)` — the salt naming the program build with the FFmpeg version,
the kernel and its codec version, and the identity (path, size, modification time) of every file the
stage reads — before the kernel walks for it, and the checkpoints and broken pictures the memo holds SHALL
be written there only at rest (no input for the prerender's quiet period, not playing, no export's vote).
Entries SHALL be stored without loss; the tier SHALL share the disk tier's folder rules and cap and SHALL
change no byte of any frame, in the preview or the export. Datamosh SHALL give its codec.
Why: found-defects 770 — after a restart every Datamosh moment the composite tier lacks walked its history
from the anchor again; durable-history-and-mix D3, D4. A lossy entry would change every later frame.

#### Scenario: A restart resumes the walk from disk
- **WHEN** a Datamosh event with no refresh is walked to its 120th position, the memo is written at rest, the process's memo is dropped and a fresh memo over the same folder asks position 121
- **THEN** the walk starts from the latest stored checkpoint (positions walked ≤ the checkpoint spacing), and frame 121 is byte-equal to the frame of an uninterrupted walk
- **TEST** t3338 (`src/libs/kt-engine/tests/stream_durable.rs`)

#### Scenario: The entry codec is exact
- **WHEN** every entry type Datamosh stores (pictures, reconstructions, at sharp and soft, 8 and odd sizes) is encoded and decoded
- **THEN** each decoded entry equals the original bit for bit, and a version mismatch is a miss
- **TEST** t3339 (`src/plugins/fx-video-codec/src/walk.rs`)

#### Scenario: What a process could assume re-keys the entries
- **WHEN** stored entries are asked after a source file's modification time changes, after the build salt changes, and after the codec version moves
- **THEN** each is a miss, and the walk recomputes from its anchor
- **TEST** t3340 (`src/libs/kt-engine/tests/stream_durable.rs`)

#### Scenario: Writes happen only at rest
- **WHEN** a loop over a Datamosh event plays for five seconds, then the input stops for the quiet period, then Play is pressed
- **THEN** nothing is written while playing, entries are written during the quiet, and the writing stops at Play
- **TEST** t3341 (`src/libs/kt-engine/tests/stream_durable.rs`)

#### Scenario: A kernel without a codec is never stored
- **WHEN** a test stream kernel without `EntryCodec` walks and the quiet period passes
- **THEN** the tier holds no entry of it and its frames are unchanged
- **TEST** t3342 (`src/libs/kt-engine/tests/stream_durable.rs`)

#### Scenario: The export is the same with and without the tier
- **WHEN** a project with Datamosh is exported over an empty tier, over a tier the preview filled, and with the tier off
- **THEN** the three video outputs are byte-equal, and the second run walks fewer positions
- **TEST** t3343 (`src/libs/kt-engine/tests/stream_durable.rs`)

#### Scenario: The tier shares the cap and goes first
- **WHEN** composites and stream entries of the same age fill the cap and a new composite arrives
- **THEN** stream entries are evicted before the composites, nothing this process used is evicted by the prerender, and another build's folder is removed at open
- **TEST** t3344 (`src/libs/kt-engine/src/render_cache.rs`)
