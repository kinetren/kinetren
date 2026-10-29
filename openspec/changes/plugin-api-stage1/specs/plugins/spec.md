# Spec Delta

## Purpose

The plugin API and registry: how an effect, a transition or a generator declares itself, which
declarations the host refuses at registration, how a plugin id lives in the project file, how
the built-in set is assembled from family crates, and what the build graph promises about
rebuild scope.

## ADDED Requirements

### Requirement: A plugin is a manifest plus pure kernels; the host refuses an incompatible declaration at registration
Every effect, transition and generator that is not a core-owned kind SHALL be a plugin: a
manifest and kernels behind one internal interface. The manifest SHALL declare the id, the
media (video effect, audio effect, transition, generator), the parameter passport (the same
passport type the model uses), and the declarations the host's invariants depend on: for a
video effect its planner class (geometry, point-wise or barrier), its alpha rule and its GPU
kernel; for an audio effect whether it keeps state, its tail and its look-ahead in frames as a
function of its parameters; for any plugin whether its kernel may be called from several
threads at once. A kernel SHALL be a pure function of (input, parameter values, the position
the host passes, the requested quality): no wall clock, no file or network I/O, no randomness
outside the host-provided position.

The registry SHALL refuse, at registration and before any frame or sample is produced: a
duplicate id; an id that is not lower `snake_case`; a video kernel that declares state between
frames, a tail or a look-ahead; any kernel that declares it must not be called concurrently; an
audio look-ahead that can reach `MIN_PREROLL_FRAMES` (4800); a built-in point-wise or barrier video plugin without a
GPU kernel; a geometry-class plugin that does not express its map in the host's closed
primitive set. A refusal SHALL name the plugin id and the declaration that failed.
Why: F-EXT-003 — the real content of a plugin API is what the host must promise and what the
plugin must confess; checking the confession at load makes refusal cheap and early. Video
kernels stay pure because post-FX memoization is built on frames without history (invariants 4,
11). Audio is different (Revision of F-EXT-003, see design D4): there is no post-FX audio cache
(prefetch fills only the pre-FX cache, invariant 2), and the host already owns per-clip FX state
on the render thread and diffs it by clip id (invariant 3), so a declared state, tail and
look-ahead are what the audio host needs, not a reason to refuse.

#### Scenario: Incompatible declarations are refused by name
- **WHEN** the registry is asked to register, one at a time: two manifests with the id
  `posterize`; an id `Posterize`; a video plugin declaring state; a video plugin declaring a
  tail; a video plugin declaring a look-ahead; a plugin declaring a single-threaded kernel; an
  audio plugin whose look-ahead reaches 4800 frames at the top of its parameter range; a
  built-in barrier plugin with no GPU kernel; a geometry plugin that supplies WGSL instead of
  primitives; and finally a well-formed audio plugin declaring state, a tail and a look-ahead
- **THEN** each of the first nine is refused with an error naming the id and the declaration,
  and no plugin from them is visible in the registry; the last one is registered, and its tail
  and look-ahead are read from its manifest by the same functions the audio processor uses
- **TEST** t972 (`src/libs/kt-plugin-api/tests/registry.rs`)

### Requirement: The plugin id is the file tag; a plugin this build lacks keeps its bytes
The id of a built-in plugin SHALL equal the tag the kind had as an enum variant (the serde
`kind` of today's files), and a plugin-backed instance SHALL be written to the `.kt` file with
the same text as before the move: the same `kind`, the same field names, the same field order,
and the same fields omitted at their defaults. A file that names an id this build does not
register SHALL still open: the slot SHALL keep its raw content and write it back unchanged on
save, the engine SHALL pass the frame or the sound through unchanged, and the program SHALL say
once per project which plugin id is missing and on which events. No project-format version
SHALL be raised by moving a kind into a plugin.
Why: the owner's files must not change because the code moved (R.70: `.kt` is line-oriented
for git); a missing plugin is the normal state of a shared project in the wasm stage, and
refusing to open such a file would make every project hostage to the slowest machine's plugin
set.

#### Scenario: Moving a kind into a plugin does not change a byte of the file
- **WHEN** a project holding every built-in kind moved so far, each with default and with
  non-default parameters, is written by the build after the move, and its text is compared with
  the text the frozen pre-move serializer (a verbatim copy inside the test) produces for the same
  model; then the file is read back and written again
- **THEN** both texts are byte-identical, and the second write equals the first
- **TEST** t973 (`src/libs/kt-model/tests/plugin_format.rs`)

#### Scenario: A missing plugin is kept, passed through and named
- **WHEN** a `.kt` file holds a chain `[blur, wobble_x, glow]` where `wobble_x` is not
  registered, with parameters and a curve on it; the file is opened, rendered and saved
- **THEN** it opens; the rendered frame equals the frame of `[blur, glow]`; the saved file holds
  the `wobble_x` slot with the same bytes; the missing id is reported once, naming `wobble_x` and
  the event
- **TEST** t973 (`src/libs/kt-model/tests/plugin_format.rs`) — the bytes, t973b (`src/libs/kt-engine/tests/plugin_missing.rs`) — the pass-through and the report

### Requirement: Family crates are leaves; the core never depends on a plugin
The built-in plugins SHALL live in family crates under `src/plugins/`, each depending only on
the plugin API crate (and, where declared, on a low crate from a closed list, e.g. the text
crate for the title). The plugin API crate SHALL depend only on the time crate and serde. The
core crates (model, fx, engine, gpu, media, cache, otio) MUST NOT depend on a family crate or on
the built-in assembler; the assembler SHALL be depended on only by the two programs and by test
targets, and the registry SHALL be handed to the core at run time. Editing the source of one
family crate SHALL recompile that crate, the assembler and the programs, and no core crate.
Why: the owner's reason for doing plugins first is build time (R.68: a kt-model edit cascades
for 61 s through the workspace); a graph rule that no test checks is broken by the first
convenient `use`.

#### Scenario: The crate graph keeps plugins at the leaves
- **WHEN** the workspace manifests are read (normal, build and dev dependencies separately)
- **THEN** the plugin API crate's normal dependencies are exactly the time crate and serde; no
  core crate has a normal or build dependency on a family crate or the assembler; every family
  crate depends on the API crate and only on crates from the declared closed list; the assembler
  appears as a normal dependency only of kt-app and kt-cli
- **TEST** t975 (`src/apps/kt-cli/tests/plugin_graph.rs`)

#### Scenario: Editing one effect rebuilds only the leaves
- **WHEN** on a warm tree one line of the pilot plugin's kernel source is touched and the
  workspace is built (`cargo build -p kt-app -p kt-cli`, then `cargo test --workspace --no-run`),
  with the crates cargo recompiles and the wall time recorded
- **THEN** the recompiled workspace crates are exactly the family crate, the assembler, kt-app,
  kt-cli and test targets that depend on them; the recorded times are reported next to the R.68
  baseline (61 s cascade, 43 s kt-app + kt-cli) and the stage-1 measurement of the same edit made
  before the move
- **TEST** t977 (`src/scripts/plugin-rebuild-check.py`)

### Requirement: The built-in set is closed and only it carries references
The built-in plugins SHALL be listed by one closed list of ids in the assembler, in a fixed
order. The golden and parameterized reference tests SHALL iterate exactly that list; a plugin
registered by any other door (a test door now, wasm later) MUST NOT appear in them, and SHALL
still appear in the pickers and the parameter forms like a built-in one. Pickers, the parameter
forms and the generator menu SHALL read the registry, not a list of their own; every catalog key
a built-in manifest names (category, keywords, variant names, parameter labels) SHALL exist in
all five translation catalogs.
Why: references are a property of the product, not of what the user installed (F-EXT-003); and a
picker list kept beside the registry is the "what can be applied vs. what is drawn" drift the
transitions vocabulary already closed once.

#### Scenario: A foreign plugin is in the picker, not in the references
- **WHEN** a test registers an extra video plugin `test_tint` through the test door next to the
  built-in set; the golden id list, the audio harness list and the transition list are taken;
  the FX picker is opened on a clip and searched for "tint"; the catalogs are scanned for every
  key the built-in manifests name
- **THEN** none of the reference lists holds `test_tint` and each equals the assembler's closed
  list; the picker shows "Test Tint" under its declared category, and a click puts it into the
  chain with its passport's defaults drawn by the common parameter row; no built-in key is
  missing from any of the five catalogs
- **PATH** select a video event → open its FX chain window → click "+" → the picker opens with
  the search row focused and the categories listed → type "tint" → one cell "Test Tint" under
  its declared category with its thumbnail → click it → the chain strip shows "Test Tint" as the last node and
  its parameters appear below with sliders and units
- **TEST** t976 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Every effect a person picks is a plugin; each reason it was core is a contract
Every user-facing effect SHALL be a plugin. What kept a kind in the core SHALL be a contract any
plugin can use:
- a POSITIONED tap: the stage's size, the sub-sample's coordinate and the burst index;
- a GEOMETRY program of the host's primitives with one open primitive, a plugin warp of the
  normalized centered space with one WGSL function;
- an exact LONE path for a series of length 1 without a scale change;
- a HOST-REFERENCE row: a track, with a role the host routes by;
- a TIME-WARP declaration: the slot whose rate consumes the source.
The core keeps only what is not a picked effect: the Geometry lead node, the strength marker
LayerMix and the read-only CamMotion form. The barrier kinds move in their own change with a GPU
pass graph.
Why: the owner (2026-10-30): "take ALL the effects out — a mechanism as flexible and extensible as
possible". A kind kept core for a reason is a reason a third-party effect could never have.

#### Scenario: A positioned tap measures its own stage
- **WHEN** vignette and noise, moved into plugins, run alone, inside a fused geometry series and as barriers, on both executors, at a decode scale step of 2
- **THEN** the CPU bytes equal the pre-move goldens (vignette's radius measured on its stage, noise's grain pinned to its stage's grid and to the burst index), the GPU is within the point-wise tolerance, and the series stays one pass
- **TEST** t992 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: A geometry program fuses and keeps its rules
- **WHEN** the five warp kinds, corner_pin, event_grid and pan3d, moved into plugins, run in series of any length on both executors
- **THEN** the CPU bytes equal the pre-move goldens, a series of geometry plugins is one pass, the GPU is within the geometry tolerance, event_grid's exact tile program still decides from the primitives, and pan3d's points behind the camera and outside the quad stay transparent
- **TEST** t993 (`src/libs/kt-gpu/tests/parity.rs`), t943, t944, t298

#### Scenario: The exact lone path keeps a permutation's bytes
- **WHEN** a moved mirror runs alone without a scale change, and then in a series and with a change of scale
- **THEN** alone it takes the plugin's exact path and gives t297's bytes; in a series or with a scale change it is a step of the fused pass
- **TEST** t994 (`src/libs/kt-fx/tests/geo_fuse.rs`), t297

#### Scenario: A host-reference row routes without engine code
- **WHEN** a compressor, moved into a plugin, keys on another track; the project is saved, reopened and rendered
- **THEN** the file holds the former `key` text (absent when none), the key signal reaches the compressor through the row's role, and a deleted track leaves the row empty rather than pointing elsewhere
- **TEST** t995 (`src/libs/kt-engine/tests/sidechain.rs`)

#### Scenario: A declared time warp drives the source the way the variant did
- **WHEN** Slowed+Reverb, moved into a plugin with a time-warp declaration, plays a curved signed rate on a linked A/V pair, is cut by a morpher layer and reversed
- **THEN** the source windows, the frame map, the link-group sync and `clip_slice`'s anchor equal those of the frozen variant, the per-kind sound table and `GOLDEN_AUDIO` do not move, and a second warp slot in one chain is refused
- **TEST** t996 (`src/libs/kt-engine/tests/plugin_sound.rs`), t973c, t11

### Requirement: A GPU kernel joins the one composition path, never beside it
A built-in video plugin's GPU kernel SHALL be assembled into the GPU executor from the registry
when the executor is created: a point-wise plugin supplies one WGSL tap function that is placed
both into its own pass and into the tap arm of the fused geometry pass; a barrier plugin
supplies a fragment pass over the shared prelude; a geometry plugin supplies no WGSL and is
mapped to the closed primitive set. The chain plan SHALL be built by the single planner for both
executors from the manifests' classes. Anything that is hashed into a cache key SHALL use the
plugin id, never the executor's dispatch number. The CPU kernel SHALL remain the byte
reference and the GPU kernel SHALL be held to the existing tolerance classes.
Why: invariant 7 — one composition path, CPU the byte reference, geometry series in one pass;
an assembled shader is a pure function of the registry, so GPU determinism between runs on one
device (invariant 11) holds; a dispatch number depends on what is registered and would silently
shift keys.

#### Scenario: The pilot plugin produces the former bytes on both executors
- **WHEN** Posterize, moved into its family crate, runs on the CPU alone, inside a fused series
  (`Mirror, Posterize, Vignette`), as a barrier between two blurs, and the same three chains run
  on the GPU executor assembled from the registry; separately a geometry plugin offering a WGSL
  string is registered
- **THEN** the CPU bytes equal the pre-move goldens (t328 does not move), the GPU frames are
  within the existing point-wise tolerance of the CPU, the fused series is still one pass, the
  assembled shader text is identical between two executor creations, and the geometry plugin is
  refused (t972)
- **TEST** t974 (`src/libs/kt-gpu/tests/parity.rs`)
