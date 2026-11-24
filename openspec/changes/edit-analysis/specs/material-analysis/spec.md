## ADDED Requirements

### Requirement: A measure of a file is a named series anyone can read
A measure SHALL be declared with an id, a version, its cadence and the measures it reads, computed by an offline job on
request, kept in the cache folder under the file's content id, and SHALL be the same bytes on every machine for the same
decoded material. One pass SHALL serve every measure asked of it; a measure read from others SHALL cost no decode; a cycle
SHALL be refused at registration. Scroll, zoom and a closed switch SHALL ask for none.
Why: the owner, v26.10 line 90 — "даже тот же md sad уже полезен для построения более сложного анализа".

#### Scenario: One pass, many measures
- **WHEN** `onset`, `beats` and `audio` are asked of one file with an empty cache folder, then again, then the file's `beats` version is raised
- **THEN** the file is decoded once; the second ask reads the folder; the third recomputes `beats` from `onset` with no decode; a cycle among two declared measures is refused naming both
- **TEST** t4211 (`src/libs/kt-media/tests/measures.rs`)

#### Scenario: The audio series keeps its bytes
- **WHEN** a series stored by an earlier version is read as the measure `audio`, and a reactive generator is rendered
- **THEN** it is a hit and the generator's frame is byte for byte the former one
- **TEST** t4212 (`src/libs/kt-media/tests/gen_reactive.rs`)

### Requirement: Beats follow the music and refuse when there are none
The beat grid SHALL follow a tempo that drifts, place each beat on its onset, name bars, and SHALL be absent with the
reason when no steady beat is heard. A person's correction (half, double, a shift, beats per bar, an anchor) SHALL be kept
in the project and applied to every reader.
Why: "темпа/бита, чтобы делать динамический монтаж"; a tempo read an octave off is the common fault of every detector.

#### Scenario: Steady, drifting and without a beat
- **WHEN** a click track at 128 BPM under pink noise, one drifting from 90 to 110 BPM over a minute, and a speech fixture are measured
- **THEN** every beat of the first lies within 10 ms of its click and of the second within 20 ms; the third gives no grid
- **TEST** t4213 (`src/libs/kt-media/tests/beats.rs`)

#### Scenario: The same on every machine and in any pieces
- **WHEN** `onset` and `beats` are computed with the pass stopped and resumed at several bursts, and with the CPU's levels forced down
- **THEN** the bytes are equal
- **TEST** t4214 (`src/libs/kt-media/tests/beats.rs`)

#### Scenario: Corrections
- **WHEN** a 140 BPM grid read as 70 is doubled, shifted and given two anchors 30 ms off
- **THEN** the grid passes through the anchors, the beats between them are spread evenly, and clearing the correction gives the measured grid back
- **TEST** t4215 (`src/libs/kt-model/tests/beat_fix.rs`)

### Requirement: Motion is what moves in the frame, a cut is not a flash
The picture's measures SHALL separate the camera's motion from the motion inside the frame, and SHALL report a cut only
where the picture after stays different from the picture before; a dissolve SHALL be a span. Integer arithmetic only.
Why: "или движение"; detectors that cut on flicker are the first complaint against this feature (Adobe's community).

#### Scenario: A pan is not motion
- **WHEN** a textured frame pans by 3 px a frame, then stands with a square moving across it, then stands still with sensor noise
- **THEN** the camera's motion is read in the first and the object motion is near zero; the second gives object motion centred on the square; the third gives none
- **TEST** t4219 (`src/libs/kt-media/tests/motion.rs`)

#### Scenario: Cuts, flashes and dissolves
- **WHEN** a fixture holds five hard cuts, three single-frame flashes, a fast pan and a one-second dissolve
- **THEN** exactly the five cuts are reported on their frames, the dissolve as one span, nothing else
- **TEST** t4220 (`src/libs/kt-media/tests/motion.rs`)
