## ADDED Requirements

### Requirement: An edit that changes nothing reaches no engine
A command or a batch whose result equals the canon SHALL NOT be an edit: the model pushes no undo
step, starts no coalesced gesture and keeps its revision (`TimelineModel::revision`), and the app's edit
doors (`apply`, `apply_batch`, `apply_coalesced`) then bump no revision, hand the player no project and
mark nothing dirty. A real change SHALL still be exactly one edit.
Why: every edit door handed the player the whole project whether or not anything changed, and a swap
rebuilds the audio renderer on the audio render thread and both renderers on the prefetch thread. A
window that re-sent what the model held each frame turned every mouse move into that rebuild
(kkt-26.11.07b G5/G7).

#### Scenario: The model takes an equal value as no edit
- **WHEN** a gain equal to the held one is sent through `apply_coalesced`, `apply` and `apply_batch`, then a different gain under the same gesture token, then an undo
- **THEN** the equal value moves neither the revision nor the undo depth, the real change is one step and one revision, and the undo is a revision
- **TEST** t1873 (`src/libs/kt-model/tests/parity_ops.rs`)

#### Scenario: The app's doors take an equal value as no edit
- **WHEN** an event's chain is sent back unchanged through the three doors, then with one value changed
- **THEN** the unchanged chain moves no revision, undo step or player project and marks nothing dirty; the changed one does each once
- **TEST** t1874 (`src/apps/kt-app/src/input_work_tests.rs`)

### Requirement: The UI's own audio renderer is built once per project version
The renderer the UI thread uses (the levels under the cursor on a pause, the pinned head of the loop
region) SHALL be built once per project version and kept until the next.
Why: each change of the loop region built a whole audio renderer on the UI thread, 447 ms on the owner's
1974 events, and the first meter reading after an edit built another.

#### Scenario: Region changes and meter readings on one version
- **WHEN** the loop region changes five times and the level is read five times on one project version, then the project is swapped and the region changes twice
- **THEN** one renderer is built for the first five and one for the next two
- **TEST** t1875 (`src/libs/kt-engine/tests/probe_renderer.rs`)
