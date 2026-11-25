## ADDED Requirements

### Requirement: A wasm plugin is registered through the same door and runs the same kernels
A plugin package (a manifest, a WebAssembly module, WGSL files) SHALL be checked by the same manifest check as a
built-in plugin and registered in the same registry by family; its audio, generator and transition kernels SHALL run
in the module, its video passes SHALL run as WGSL on the one GPU path, and a project SHALL use its kinds by id exactly
as built-in kinds; the same inputs SHALL give the same bytes on every machine.
Why: the owner, v26.10 P5 — "rust+wasm свой плагин собрать … собирать и запускать(в тот же wasm)"; invariants 7, 11.

#### Scenario: The template plugin plays and renders like a built-in
- **WHEN** the template package (a gain and a solid-colour generator) built for wasm32 is added, used on an event, played and exported twice on two backends
- **THEN** the gain's output equals the built-in gain with the same value byte for byte; the generator's frames equal the stored ones; a curve on a row moves the sound as the built-in's does; a warm render of it allocates nothing; the two exports are byte-equal
- **TEST** t3957 (`src/libs/kt-plugin-wasm/tests/host.rs`)

### Requirement: A wasm plugin is sandboxed by its imports, and a fault switches only it off
A module SHALL import only the host's log, the frozen math and bounded memory; a module importing anything else SHALL
be refused at registration naming the import; a trap or an exhausted fuel budget SHALL return a neutral result, switch
that kind off for the session with one notice naming the plugin, and leave the rest of the project playing; an export with a kind switched off SHALL not start, and a fault during one SHALL end it, both named.
Why: F-EXT-003 — "a panic can be caught, a segfault cannot"; a plugin never ends the program.

#### Scenario: A faulting plugin and a greedy one
- **WHEN** a module importing a clock is added; a module that traps on its tenth block and one that loops forever are used on events while the project plays
- **THEN** the first is refused naming the import; the second's slot goes silent from the tenth block and a notice names it; the third's block is stopped by fuel the same way, at the same instruction on every run; playback and the other events never stop; the crash report's signal handlers are the installed ones
- **TEST** t3958 (`src/libs/kt-plugin-wasm/tests/sandbox.rs`)

#### Scenario: A missing plugin keeps the project's bytes
- **WHEN** a project using a wasm plugin's kind is opened on a machine without it, saved, then the plugin is added
- **THEN** the slot reads as Missing, the saved file is byte-equal to the original; after adding, the slot plays with its values
- **TEST** t3959 (`src/libs/kt-plugin-wasm/tests/host.rs`)

#### Scenario: An export does not hide a fault
- **WHEN** a project whose effect's kind was switched off is exported, and another whose module traps in the middle of its export
- **THEN** the first does not start and names the plugin; the second ends with its name and leaves no file
- **TEST** t4151 (`src/libs/kt-plugin-wasm/tests/sandbox.rs`)

### Requirement: A plugin package meets a throwaway process before it is installed
Before a package is installed, and again when the GPU driver changes, our own binary SHALL be started to register it, run every kernel at the defaults and at the bottom and top of its rows and draw every pass;
a refusal, a death or more than its bound SHALL leave it not installed with the reason. Installed, updated and removed packages SHALL take effect at the next start, and the program SHALL say so.
Why: a shader that never ends takes the device of the process that draws it; hw-encode-context D10 met drivers the same way (wasm-plugin-host D5, D6).

#### Scenario: A package that would take the device is not installed
- **WHEN** packages with a shader of an endless loop, a kernel that traps at the top of a row, an id of a built-in effect and a path outside its folder are added, then the template package
- **THEN** each of the four is refused with its own reason while the program goes on, and nothing of them is left in the library; the template is installed, reads "works after a restart", and a project naming its kind keeps its bytes until then
- **TEST** t4152 (`src/apps/kt-cli/tests/plugin_trial.rs`), t4157, t4158 (`src/libs/kt-plugin-wasm/tests/package.rs`)
