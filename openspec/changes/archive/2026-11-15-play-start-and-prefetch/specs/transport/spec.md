## MODIFIED Requirements

### Requirement: The sound device is open only while there is sound to play
The output device SHALL be opened by a thread of its own (`audio-output`) on the first Play, and once the transport has stood for `CLOSE_AFTER_REST` (1 s) its stream SHALL be paused and kept, so the next Play resumes it instead of opening the device again; with the stream paused or closed nothing of the sound output wakes. A stream whose pause fails, or whose callbacks keep coming for `PAUSE_CHECK` (300 ms) after a pause, SHALL be closed instead, and a stream that does not resume SHALL be closed and opened again. The thread SHALL park on the player's wake with no clock of its own; the UI thread MUST NOT open a device. The ring SHALL survive a close: the callback owns its consumer while the stream exists and hands it back when the stream drops it. A seek while the device is closed SHALL be flushed by the output thread, and a seek while it is paused by a short resume during which the callback drains it, so the first callback of the next Play hands out the new start's frames from the first. The callback stays the master clock: with no device calling back time stands (invariant 6's "before the first frame"), and the callback allocates nothing and takes no lock. An open that fails at Play SHALL reach the bell, and the next Play SHALL try again. The open's and the resume's cost and the time to their first callback SHALL be in the trace (`play` aux `device_open`, `device_resume`, `device_pause`, `device_close`, `device_error`).
Why: found-defects 274 — on a pause cpal's ALSA worker woke ~24 times a second and PipeWire's data loop ~48, handing out silence ("the program at rest costs nothing"); the owner decided (kkt-26.11.10, "да звук открывать при Играть"). `cpal::Stream` is not `Send` and its open blocks, which is why a thread of its own owns it. Found-defects 516 (play-start-and-prefetch): every Play then paid an open, 28–112 ms with its first callback beside ~100 busy loops and seconds in a loaded live run (row 425); a paused cpal/PipeWire stream wakes nothing (0 callbacks, the context switches of a closed one) and resumes in 20 µs.

#### Scenario: Play opens, a pause closes after the rest, the thread parks
- **PATH** the editor stands paused: no sound device is open → Play: the device opens and the playhead runs → Pause: a second later the device has closed
- **WHEN** a paused player stands, then plays, then pauses, on a device that cannot pause
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

#### Scenario: At rest the device pauses, and Play resumes it
- **PATH** Play a moment, Pause; a second later the sound device is quiet → Play: it sounds at once, no new open → Pause, Play again: the same
- **WHEN** a device that can pause stands a rest after a play, then Play comes twice
- **THEN** the stream is paused, not closed, the output thread stands and the device calls back no more; each Play resumes it, the clock runs, and the device was opened once
- **TEST** t2642 (`src/apps/kt-app/src/audio_out.rs`)

#### Scenario: A pause that keeps calling back is a close
- **WHEN** the device says it paused but its callbacks go on, or it cannot pause at all
- **THEN** the stream is closed (after `PAUSE_CHECK`, or at once), the thread stands, and the next Play opens the device again
- **TEST** t2643 (`src/apps/kt-app/src/audio_out.rs`)

#### Scenario: A seek while paused
- **PATH** Play, Pause, a second later click on the ruler elsewhere → nothing is heard → Play: the first sound is the material at the click
- **WHEN** the device is paused at rest and a seek comes, then again with the seek inside the pause's check
- **THEN** the flush is drained without Play and the device is paused again; the next Play's first sound is the start's first frame, byte for byte, and the device was opened once
- **TEST** t2644 (`src/apps/kt-app/src/audio_out.rs`)

#### Scenario: A pause after a play wakes nothing of the sound
- **PATH** the editor is open on the heavy showcase → Space: the playhead runs a second → Space: it stops → two seconds later the sound device is paused
- **WHEN** the live `idle` scenario counts the context switches of every thread over 5 s of that pause
- **THEN** neither `audio-output` nor `cpal_alsa_out`, `pw-data-loop`, `alsa-pipewire` (nor any of the engine's threads) woke
- **TEST** live `idle` (`src/scripts/live/idle.py`)

## ADDED Requirements

### Requirement: A start's pass waits for the drain of its flush
`Player::seek` SHALL store the flush before it moves the generation, and while a start is pending with a flush its consumer has not drained, the render thread SHALL write nothing into the ring (`play` aux `start_hold`, `b` 3) when the consumer drains flushes itself (`AudioShared::drains_flush`: the app's output thread, the bench's device). Time stands before the first frame, so the hold costs no sync (invariant 6).
Why: play-start-and-prefetch D2–D3. The consumer drains the flag when it next runs; a render thread that had taken the new generation wrote the start's whole pass first, and the drain dropped it — the start lost its first ring of sound and played ahead of the clock by it. The seek stored the flag after the generation, so the render thread could see one without the other.

#### Scenario: A start before the drain
- **WHEN** a paused player is sought and played with a consumer that drains flushes but has not yet
- **THEN** nothing goes into the ring for 300 ms; once the consumer drains, the start's pass comes
- **TEST** t2641 (`src/libs/kt-engine/tests/play_start.rs`)

### Requirement: Play's first sound is measured
The render thread SHALL record the time from Play to the first frames a callback handed out after it: `AudioShared::first_sound_us` and the trace's `play` aux `first_sound` (`b` µs). `preview_bench` SHALL model a device that opens `device_open` after Play and report the first sound; `kt-cli preview --device-open-ms N` and `kt-cli trace summary` SHALL print it.
Why: found-defects 516 — the bench modelled an always-open device, so `kt-cli preview` never saw what kept-mix-settle's open on Play added.

#### Scenario: A device that opens late
- **WHEN** the bench plays with a device that opens 150 ms after Play
- **THEN** the first sound it reports is at least 150 ms, and the trace holds one `first_sound` record, at least that late
- **TEST** t2645 (`src/libs/kt-engine/tests/play_start.rs`)

### Requirement: A stop or a seek handed to a parked thread is never lost
Every state a parked thread waits on SHALL be changed, and its waiter notified, under the lock the waiter looks at it with, or the waiter SHALL take the notify count before it looks and wait for it to move (`Wake`). A lock-free park handshake (the trace writer) SHALL fence both sides `SeqCst`. A new thread that parks on another's news follows one of the two forms; the list of hand-offs checked is in play-start-and-prefetch's design (D2).
Why: found-defects 525 — t2335's hang was the test's own race (fixed by ec6be337), but the walk of every hand-off found the compose thread's stop notified without its lock (the thread, not joined, parked for good holding the player's preview parts) and the trace writer's handshake without the producer's fence.

#### Scenario: A stop between the look and the wait
- **WHEN** the compose thread has looked at the stop flag and not yet waited, and the player stops it
- **THEN** its wait ends; with the stop unlocked it waited for good
- **TEST** t2640 (`src/libs/kt-engine/src/compose_queue.rs`)

#### Scenario: A paused seek owes one prefetch pass, under load
- **WHEN** t2335 counts passes from before each seek, beside 48 and 96 busy loops on 32 threads
- **THEN** every seek gets its pass (42 of 42 runs); counted after the seek, 8 of 12 runs waited for a pass nothing would ask
- **TEST** t2335 (`src/libs/kt-engine/tests/loop_fill.rs`)
