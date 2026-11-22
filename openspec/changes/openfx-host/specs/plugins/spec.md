## ADDED Requirements

### Requirement: OpenFX plugins run in a host process of their own and join the plan as foreign nodes
OFX plugins SHALL be loaded by a separate host process started when a project first uses one; a filter or general plugin
SHALL be a video effect node, a generator-context plugin a generator kind, a transition-context plugin a transition kind;
their parameters SHALL become passport rows animated by our curves; a plugin that declares state across frames or a
retimer context SHALL be refused at registration with the reason; an OFX node SHALL be a barrier that reads its input
back, renders in the host process and uploads the result in the one plan, marked foreign, its results cached by the
plugin binary's hash; a crash or a hang of the host SHALL bypass the node with a notice and restart the host on the next
request, never ending the program.
Why: the owner, v26.10 lines 130–133 and R.59 — "openfx в будущем вернём"; F-EXT-003's conditions.

#### Scenario: An open plugin set renders, and a crash costs only the node
- **WHEN** openfx-misc's blur is used on an event and exported twice on one machine; then a test plugin that crashes on its fifth frame is used while the project plays
- **THEN** the two exports are byte-equal and the blur equals openfx-misc's own reference output on the test frame within one code value; the crashing node passes its input through from the fifth frame with one notice, playback never stops, and the next request restarts the host
- **TEST** t3974 (`src/libs/kt-plugin-ofx/tests/host.rs`), t3975 (`src/libs/kt-plugin-ofx/tests/crash.rs`)

#### Scenario: Contexts map to families, state is refused
- **WHEN** an OFX generator, an OFX transition and a plugin declaring sequential render state are registered
- **THEN** the generator is listed among generator kinds and the transition among transition kinds; the stateful one is refused naming the reason
- **TEST** t3976 (`src/libs/kt-plugin-ofx/tests/host.rs`)
