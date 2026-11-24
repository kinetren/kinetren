## ADDED Requirements

### Requirement: The camera's 3DNR is a recursive filter with a motion detector, luma and chroma apart
The camera ISP SHALL offer a temporal noise reduction between its lens corrections and its YUV stages: per pixel the luma and the chroma are
each blended with the previous OUTPUT by the strength, gated by their own motion confidence from the
blurred difference to the previous output and their own threshold; the state SHALL be kept at 16-bit
precision. Its mis-tuning SHALL produce trails behind moving objects, longer in chroma when the chroma
threshold is higher. The view SHALL offer the motion map. Rolling shutter SHALL read each row between
frames N−1 and N by its readout time, and mains flicker SHALL modulate each row by the light at its readout
time; both default off.
Why: the owner, 2026-11-01 (I1): "шумодав может быть временной … хромо-шлейфы"; a simulation, not an
imitation.

#### Scenario: Noise falls where nothing moves, trails follow where the detector misses
- **WHEN** a fixture of a grey square moving slowly over a slightly darker background with noise is processed with α 0.9, th_y 6, th_c 14
- **THEN** the noise variance in static areas falls to (1−α)/(1+α) of the input within 5 %; behind the square a luma trail decays as α^k frame by frame within one code value; the chroma trail is longer than the luma trail; with th_y 1 there is no trail
- **TEST** t3548 (`src/libs/kt-gpu/tests/camera_isp.rs`)

#### Scenario: No dead band
- **WHEN** a static frame with a step of 2 code values is recursed for 200 frames at α 0.9 from a state offset by 3
- **THEN** the output converges to the input within half a code value (an 8-bit state would freeze the offset)
- **TEST** t3549 (`src/libs/kt-gpu/tests/camera_isp.rs`)

#### Scenario: Rolling shutter skews verticals and flicker bands the frame
- **WHEN** a vertical bar moving horizontally is read with readout 30 ms, and a static frame with flicker 50 Hz at 24 fps
- **THEN** the bar's slant equals speed × readout; the flicker draws horizontal bands that drift by the beat of 50 Hz against the frame rate, frame to frame as predicted
- **TEST** t3550 (`src/libs/kt-gpu/tests/camera_isp.rs`)

#### Scenario: The camera with 3DNR costs nothing at rest
- **WHEN** the live `idle` scene pauses on a camera ISP with 3DNR, moves the mouse over the preview and opens the ISP window
- **THEN** after the first composite no history walk, no compute job and no composite is counted
- **TEST** t3551 (`src/scripts/live/idle.py`)
