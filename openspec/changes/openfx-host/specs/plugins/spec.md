## ADDED Requirements

### Requirement: OpenFX plugins run in a host process of their own and join the plan as foreign nodes
OFX plugins SHALL be loaded by a separate host process started when a project first uses one; a filter or general plugin
SHALL be a video effect node, a generator-context plugin a generator kind, a transition-context plugin a transition kind;
their parameters SHALL become passport rows animated by our curves; a plugin that declares state across frames or a
retimer context SHALL be refused at registration with the reason; an OFX node SHALL be a barrier in the one plan, marked
foreign, composed off the window's device, whose frames cross to the host process only through shared memory (the requirement below); a crash or a hang of the host SHALL bypass the node with a notice and restart the host on the next
request, never ending the program.
Why: the owner, v26.10 lines 130–133 and R.59 — "openfx в будущем вернём"; F-EXT-003's conditions; kkt-26.11.22 D4 (Q2 a).

#### Scenario: An open plugin set renders, and a crash costs only the node
- **WHEN** openfx-misc's blur is used on an event and exported twice on one machine; then a test plugin that crashes on its fifth frame is used while the project plays
- **THEN** the two exports are byte-equal and the blur equals openfx-misc's own reference output on the test frame within one code value; the crashing node passes its input through from the fifth frame with one notice, playback never stops, and the next request restarts the host
- **TEST** t3974 (`src/libs/kt-plugin-ofx/tests/host.rs`), t3975 (`src/libs/kt-plugin-ofx/tests/crash.rs`)

#### Scenario: Other moments are asked before the frame
- **WHEN** a test plugin declares that it reads its input one frame before and after, and another asks for a frame it did not declare, both on an event over a reel
- **THEN** the order asks exactly the declared moments, the first plugin's picture equals the reference, the second's request is refused and journalled, and the frame of neither waits on an unasked decode
- **TEST** t4153 (`src/libs/kt-plugin-ofx/tests/host.rs`)

#### Scenario: Values, curves and a foreign frame's key
- **WHEN** a double, a point, a choice, a parametric and a button parameter are set, curved and pressed; the bundle is rebuilt with another content; and a plugin that answers differently twice is used in a prerendered moment
- **THEN** the child gets our curves' values at the frame's time; the rebuilt bundle renames its frames; the undeterministic plugin's moment is written to no disk and said so
- **TEST** t4154 (`src/libs/kt-plugin-ofx/tests/host.rs`)

#### Scenario: Contexts map to families, state is refused
- **WHEN** an OFX generator, an OFX transition and a plugin declaring sequential render state are registered
- **THEN** the generator is listed among generator kinds and the transition among transition kinds; the stateful one is refused naming the reason
- **TEST** t3976 (`src/libs/kt-plugin-ofx/tests/host.rs`)

### Requirement: OpenFX frames cross through shared memory without a copy
A plugin that renders on the GPU SHALL receive the node's input and write its output in images allocated by our device
as exportable memory and imported by the host process into the plugin's API on the same card and driver, each side seen finished before the other is told, with no
byte of the frame passing through RAM; a processor plugin SHALL read and write RAM shared with the host process into
which our device copies the input and from which it uploads the result directly, with no memcpy by either process where
the device imports host memory; a device without that import SHALL use one counted copy into the shared pages, named in
the trace. The shared objects SHALL be pooled at warm-up and owned by our process; handles SHALL cross the process
boundary once per pooled object, never per frame; the trace SHALL name each OFX node's path and the bytes it moved
through RAM.
Why: the owner, kkt-26.11.22 D4 — "а нахрена копировать кадр? почему shmem ram не использовать? или sh mem gpu???".

#### Scenario: A GPU plugin renders on our image
- **WHEN** a test OFX plugin declaring OpenGL render inverts a frame on a device with external memory, for 100 frames, and the host process is killed at frame 50
- **THEN** the trace names `gpu-shared` with 0 bytes through RAM, no GPU resource is created after the first frame, the result equals the processor reference within one code value; after the kill the node passes its input with one notice, the restarted host re-imports the pooled images and renders again
- **TEST** t3978 (`src/libs/kt-plugin-ofx/tests/shared_frames.rs`)

#### Scenario: A processor plugin reads the shared pages
- **WHEN** openfx-misc's blur (a processor plugin) renders 100 frames on a device with host-memory import, and again with the import disabled
- **THEN** with the import the trace names `ram-shared` and counts no memcpy, the bytes through RAM are one frame in and one out per frame; without it the trace names `ram-copy` and counts the copy; both results are byte-equal
- **TEST** t3979 (`src/libs/kt-plugin-ofx/tests/shared_frames.rs`)
