## ADDED Requirements

### Requirement: A wasm plugin is registered through the same door and runs the same kernels
A plugin package (a manifest, a WebAssembly module, WGSL files) SHALL be checked by the same manifest check as a
built-in plugin and registered in the same registry by family; its audio, generator and transition kernels SHALL run
in the module, its video passes SHALL run as WGSL on the one GPU path, and a project SHALL use its kinds by id exactly
as built-in kinds; the same inputs SHALL give the same bytes on every machine.
Why: the owner, v26.10 P5 — "rust+wasm свой плагин собрать … собирать и запускать(в тот же wasm)"; invariants 7, 11.

#### Scenario: The template plugin plays and renders like a built-in
- **WHEN** the template package (a gain and a solid-colour generator) built for wasm32 is added, used on an event, played and exported twice on two backends
- **THEN** the gain's output equals the built-in gain with the same value byte for byte; the generator's frames equal its CPU reference; the two exports are byte-equal
- **TEST** t3957 (`src/libs/kt-plugin-wasm/tests/host.rs`)

### Requirement: A wasm plugin is sandboxed by its imports, and a fault switches only it off
A module SHALL import only the host's log, the frozen math and bounded memory; a module importing anything else SHALL
be refused at registration naming the import; a trap or an exhausted fuel budget SHALL return a neutral result, switch
that kind off for the session with one notice naming the plugin, and leave the rest of the project playing.
Why: F-EXT-003 — "a panic can be caught, a segfault cannot"; a plugin never ends the program.

#### Scenario: A faulting plugin and a greedy one
- **WHEN** a module importing a clock is added; a module that traps on its tenth block and one that loops forever are used on events while the project plays
- **THEN** the first is refused naming the import; the second's slot goes silent from the tenth block and a notice names it; the third's block is stopped by fuel the same way; playback and the other events never stop
- **TEST** t3958 (`src/libs/kt-plugin-wasm/tests/sandbox.rs`)

#### Scenario: A missing plugin keeps the project's bytes
- **WHEN** a project using a wasm plugin's kind is opened on a machine without it, saved, then the plugin is added
- **THEN** the slot reads as Missing, the saved file is byte-equal to the original; after adding, the slot plays with its values
- **TEST** t3959 (`src/libs/kt-plugin-wasm/tests/host.rs`)
