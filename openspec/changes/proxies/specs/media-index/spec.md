## ADDED Requirements

### Requirement: A proxy is an intra-frame copy as long as its original
A proxy made by the program SHALL be written by an offline job with its own short-lived source as an
MKV whose video is H.264 with every frame a keyframe at a fraction of the native size and whose
pictures carry the original's own timestamps one for one, and whose audio is the original's audio
packets copied without re-encoding. The job SHALL run at background priority, pause while the transport
plays or an export runs, be cancellable at any moment leaving no partial file, and record the binding
in the sidecar index keyed by the original's identity. An attached proxy SHALL be accepted only when its
duration and frame count match the original within one frame and its audio length within one burst;
otherwise it SHALL be refused naming what differs.
Why: invariant 5 (offline sources), F-MED-002 — a grid-timed proxy drifts on fractional rates.

#### Scenario: A proxy of fractional-rate footage matches the original
- **WHEN** a proxy is made of a 23.976 fps, 10-bit HEVC fixture with a 5.1 audio stream, and of a VFR fixture
- **THEN** every picture's pts equals the original's, the duration equals the original's to the tick, every frame is a keyframe, the audio decoded from the proxy is byte-equal to the original's
- **TEST** t3440 (`src/libs/kt-media/tests/proxy.rs`)

#### Scenario: Cancel and pause leave no trace
- **WHEN** a proxy job is cancelled halfway, and another is running when Play is pressed
- **THEN** no `.part` or proxy file remains and no sidecar row is written; the running job makes no progress while playing and resumes on pause
- **TEST** t3441 (`src/libs/kt-media/tests/proxy.rs`)

#### Scenario: A mismatched attached proxy is refused with its reason
- **WHEN** a proxy one second shorter, and one with a different frame count, are attached
- **THEN** both are refused, the message names the duration and the frame count respectively; a matching foreign proxy is accepted and bound
- **TEST** t3442 (`src/libs/kt-media/tests/proxy.rs`)

### Requirement: Switching proxies changes sharpness, never framing or sound
A source opened on a proxy SHALL report the original's native size, pixel aspect, frame rate and
duration, and deliver pictures at the proxy's size as a lower decode step; conform, geometry, masks and
the frame plan SHALL be the same with and without proxies. Switching SHALL replace the sources through
the one swap path without moving the playhead or resetting the effects' state, and SHALL clear the
replaced sources' cache, kept composites and kept final mix by the source's epoch. A composite made from
a proxy SHALL never be stored under the key of a composite of the original, in memory or on disk.
Why: invariant 4 — one truth of coverage; invariant 7 — the plan is the same; a disk tier keyed by the
original file would replay proxy frames after a restart.

#### Scenario: The same plan, a softer picture
- **WHEN** a project with a fit-conformed 4K event, a mask and a corner-pin is planned with the original and with a 1/2 proxy at preview 1/1
- **THEN** the layer geometry, masks and the fx chain of the plans are equal; only the decode size differs; the composite is within `FrameTol::BILINEAR` of the original's upscaled 1/2 decode
- **TEST** t3443 (`src/libs/kt-engine/tests/proxy_swap.rs`)

#### Scenario: Toggling while paused and while playing
- **WHEN** proxies are switched on and off during playback of a slowed-reverb event and on a pause
- **THEN** the playhead does not jump, the audio has no reset (the effects' state objects are the same), the coverage bars of the swapped sources empty and refill, the other sources keep theirs
- **TEST** t3444 (`src/libs/kt-engine/tests/proxy_swap.rs`)

#### Scenario: The disk tier keeps proxy and original apart
- **WHEN** a slow moment is prerendered to the disk tier with proxies on, the player is restarted with proxies off
- **THEN** the moment is not restored from the proxy's entry; it composes from the original and keeps its own entry
- **TEST** t3446 (`src/libs/kt-engine/tests/proxy_swap.rs`)
