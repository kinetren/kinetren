## ADDED Requirements

### Requirement: Parts of a picture are element sets with operators
A producer of parts of a picture — a title's clusters, words or lines, a mask's vertices or primitives, a tracker's
points, an instance's child layers, a 3D group's planes, a transition's pieces, an emitter's particles — SHALL be able
to expose them as an element set: elements with a stable identity, a rest pose and a size, given by the same function
the producer draws with. An ordered list of operators SHALL turn rest poses into placed poses; an operator SHALL be
stateless (a closed form of the moment) or a simulation by a solver of the `sim` plugin family. A solver's passport
rows SHALL take curves and SHALL be addressable as properties: linked, published and driven like an effect's. An
element's placed pose SHALL be readable by other sets. A simulation SHALL run in its owner's reference frame, and its
result SHALL be the same at every preview quality and in an export.
Why: the owner, kkt-26.11.22 D3 — "как построить ТАКУЮ систему композитинга, которая позволит В ТОМ ЧИСЛЕ сделать
такого рода эффект"; R.59 item 3 — "so that one can control everything, including from plugins".

#### Scenario: Three producers, one contract
- **WHEN** a spring solver is put on a title's clusters, on a mask's vertices whose home is keyed, and on an emitter's particles, a link drives the first spring's stiffness, and the particles are pulled by the title's set
- **THEN** each set lists its elements with stable ids and the producer's own rest poses; the spring moves each kind's elements by the same law; the stiffness follows its link; the particles follow the simulated letters, not their homes
- **TEST** t3983 (`src/libs/kt-sim/tests/element_sets.rs`)

#### Scenario: The preview's size does not change the motion
- **WHEN** a crawl on springs is composed at 360 and at 1080 lines and exported, and its offset row moves by less than 1/64 px a step
- **THEN** every element's delta in fractions of the frame is equal bit for bit in the three, and at constant speed the deltas are zero
- **TEST** t3991 (`src/libs/kt-engine/tests/simulated_elements.rs`)

#### Scenario: A plugin solver is a family member with a version
- **WHEN** a test solver is registered in the `sim` family and its version is bumped
- **THEN** it is listed with its passport, a set can use it, and the frames' keys that used it change while others do not
- **TEST** t3984 (`src/libs/kt-plugin-api/tests/sim_family.rs`)

### Requirement: A simulation is a function of the edit
A simulation's state at a moment SHALL be integrated from the owner's origin on its instance path, from rest, with a
fixed step of 1/240 s of the owner's outer time, using only the frozen math; a frame SHALL read the state interpolated
between the two steps around its moment. The origin SHALL look through cuts: a split, a left trim and the window of an
instance SHALL NOT change the state at a moment; a held piece SHALL hold the state of the moment it holds. Its forces SHALL come only from what the edit determines: the outer time
map's speed, acceleration and speed change, the home's own acceleration, a linked property, a track's sound analysis,
a tracker's motion, and other sets' elements. Two instances of one composition at different speeds SHALL simulate
apart. Sound that is not analysed yet SHALL make the frame incomplete, never silent. Checkpoints SHALL be kept in memory only and SHALL never change a byte.
Why: R.59 — "a simulation from the start of the event with a fixed step", the state at a frame a pure function;
invariant 11; the owner's D1/D2 — nothing intermediate on disk.

#### Scenario: A seek equals continuous play, with or without checkpoints
- **WHEN** a crawling title on springs, braked by its Slowed&Reverb rate from 1 to 0.4 over 0.5 s and back, is rendered continuously, then by seeking into the middle, then with checkpoints cleared
- **THEN** at constant speed every glyph sits at its home; during the braking the glyphs run ahead and settle; the three renders are byte-equal; nothing of the simulation is written to the cache folder
- **TEST** t3968 (`src/libs/kt-sim/tests/function_of_edit.rs`), t3969 (`src/libs/kt-engine/tests/simulated_elements.rs`)

#### Scenario: A cut, a trim and a hold keep the letters where they were
- **WHEN** a swinging title is split in the middle of a swing, then its right piece is trimmed from the left by 0.2 s and extended back, then a frame is held in the swing, then the composition with it is split
- **THEN** after the split, the trim and the split of the composition every frame equals the uncut one; the held piece shows the held moment's letters for its whole length; undo restores the bytes of the file
- **TEST** t3990 (`src/libs/kt-engine/tests/simulated_elements.rs`)

#### Scenario: Sound not heard yet is not silence
- **WHEN** an emitter's burst is driven by a track whose sound is not analysed, the frame is asked, then the analysis arrives
- **THEN** the first answer is incomplete and is kept nowhere, the Loading line names the track; afterwards the frame equals an export's
- **TEST** t3992 (`src/libs/kt-engine/tests/simulated_elements.rs`)

#### Scenario: The motion does not depend on the frame rate
- **WHEN** the same edit is rendered in a 25 fps and a 30 fps project
- **THEN** at the same outer moments the poses are equal within the interpolation between two 1/240 s steps
- **TEST** t3970 (`src/libs/kt-sim/tests/function_of_edit.rs`)

#### Scenario: Two instances at two speeds, two looks
- **WHEN** a composition with a simulated title is placed twice, one instance at speed 1, the other with a speed curve that brakes over the same content range
- **THEN** the first instance's letters rest at home over the range, the second's run ahead and settle; each instance's frames are the same in a fresh process
- **TEST** t3971 (`src/libs/kt-engine/tests/simulated_elements.rs`)

### Requirement: Sets depend on each other in an order, without cycles
A set whose forces read another set's elements SHALL be evaluated after it at each step; sets that act on each other
SHALL be declared as one set with several sources; a cycle SHALL be refused at edit time naming the sets.
Why: a step order that depended on evaluation order would break determinism.

#### Scenario: Letters fall onto a mask's floor, a cycle is refused
- **WHEN** a title's letters fall with gravity onto a mask's edge as collider, then the mask is made to attract the letters' set
- **THEN** the letters land on the edge identically in two runs; the second edit is refused naming both sets
- **TEST** t3985 (`src/libs/kt-sim/tests/order.rs`)

### Requirement: A simulation costs only where it is used and never blocks the interface
An owner with no element set SHALL run no simulation code and keep its plan key; a simulated owner SHALL step on the
player's simulation thread, never on the UI thread, only for moments where it is visible; a frame whose simulation has not reached
its moment SHALL report itself incomplete with how far behind it is, and an export SHALL integrate forward from the
origin. A set SHALL declare a ceiling on its elements; past it, spawning SHALL stop with a notice.
Why: R.59 item 4 — "if physics was added to only one source - that is no reason to break and weigh down the work of
the whole project"; "no interface action waits for a picture".

#### Scenario: A deep cold seek shows Loading, then the frame
- **WHEN** an emitter of 20 000 particles running 5 minutes is opened and the playhead is clicked at 4:50, then a project without any set is planned
- **THEN** the click returns at once, the frame reports "simulation behind" and the preview shows Loading until the state reaches 4:50, then the frame equals an export's; the project without sets has the same plan keys and bytes as before this change
- **TEST** t3986 (`src/libs/kt-engine/tests/simulated_elements.rs`)
