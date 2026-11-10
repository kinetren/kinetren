## ADDED Requirements

### Requirement: The sound device is open only while there is sound to play
The output device SHALL be opened by a thread of its own (`audio-output`) when the transport plays, and closed once the transport has stood for `CLOSE_AFTER_REST` (1 s); with no stream open nothing of the sound output wakes. The thread SHALL park on the player's wake with no clock of its own; the UI thread MUST NOT open a device. The ring SHALL survive a close: the callback owns its consumer while the stream is open and hands it back when the stream drops it. A seek while the device is closed SHALL be flushed by the output thread, so the first callback of the next open hands out the new start's frames from the first. The callback stays the master clock: with no device open time stands (invariant 6's "before the first frame"), and the callback allocates nothing and takes no lock. An open that fails at Play SHALL reach the bell, and the next Play SHALL try again. The open's cost and the time to its first callback SHALL be in the trace (`play` aux `device_open`, `device_close`, `device_error`).
Why: found-defects 274 — on a pause cpal's ALSA worker woke ~24 times a second and PipeWire's data loop ~48, handing out silence ("the program at rest costs nothing"); the owner decided (kkt-26.11.10, "да звук открывать при Играть"). `cpal::Stream` is not `Send` and its open blocks, which is why a thread of its own owns it.

#### Scenario: Play opens, a pause closes after the rest, the thread parks
- **PATH** the editor stands paused: no sound device is open → Play: the device opens and the playhead runs → Pause: a second later the device has closed
- **WHEN** a paused player stands, then plays, then pauses
- **THEN** no device is open and the output thread makes no pass before Play; Play opens one and the clock runs; the pause closes it no sooner than the rest, and the thread stands again
- **TEST** t2380 (`src/apps/kt-app/src/audio_out.rs`)

#### Scenario: The ring survives a close
- **PATH** Play, Pause, wait a second; three times → each Play sounds at once, from where the playhead stood
- **WHEN** Play and pause alternate three times, each pause closing the device
- **THEN** each Play opens it again and the clock runs; what it hands out sounds
- **TEST** t2381 (`src/apps/kt-app/src/audio_out.rs`)

#### Scenario: A seek while closed is flushed
- **PATH** paused for a second, click on the ruler elsewhere → the playhead moves there; nothing of the old place is in the sound waiting for the next Play
- **WHEN** the device has closed at rest and a seek comes
- **THEN** the output thread clears the flush without opening the device
- **TEST** t2382 (`src/apps/kt-app/src/audio_out.rs`)

#### Scenario: An open error at Play
- **PATH** Play with the sound device unavailable → the bell says "Sound device: …", the playhead stands → Pause, Play again once it is back → it sounds
- **WHEN** the device does not open at Play
- **THEN** the error is handed to the bell, the clock stands, and the next Play opens it with the ring intact
- **TEST** t2383 (`src/apps/kt-app/src/audio_out.rs`)

#### Scenario: The first sound after the open
- **PATH** click on the ruler, Play → the first sound heard is the material at the click, nothing from before it
- **WHEN** a seek, then Play opens the device
- **THEN** before the first frame of the start only silence is handed out, from it the engine's frames byte for byte, and the callback allocates nothing
- **TEST** t2384 (`src/apps/kt-app/src/audio_out.rs`)

#### Scenario: A pause after a play wakes nothing of the sound
- **PATH** the editor is open on the heavy showcase → Space: the playhead runs a second → Space: it stops → two seconds later the sound device has closed
- **WHEN** the live `idle` scenario counts the context switches of every thread over 5 s of that pause
- **THEN** neither `audio-output` nor `cpal_alsa_out`, `pw-data-loop`, `alsa-pipewire` (nor any of the engine's threads) woke
- **TEST** live `idle` (`src/scripts/live/idle.py`)
