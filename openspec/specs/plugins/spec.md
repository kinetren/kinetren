# Plugins

## Purpose
The plugin API and registry: how an effect, a transition or a generator declares itself, which
declarations the host refuses at registration, how a plugin id lives in the project file, how
the built-in set is assembled from family crates, and what the build graph promises about
rebuild scope.

## Requirements

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

#### Scenario: The picker keeps its arrangement and a moved kind's cells are its manifest's
- **WHEN** the FX picker is opened on a video clip after the effects moved into plugins
- **THEN** the video rows stand in the order the picker had before any kind moved (core templates and plugins interleaved as they were, a third-party plugin after them), every row of that order is present, each moved built-in shows the variants its manifest names with captions from the catalogs, and event_grid's template is the manifest's 2×2 grid
- **PATH** select a video event → open its FX chain window → click "+" → the gallery lists Geometry, Mirror, Blur, Greenscreen … in their former places under the same headings → the Mirror row shows its "Vertical" and "Both" cells next to the template → click the EventGrid template → the chain strip shows EventGrid with 2 columns and 2 rows
- **TEST** t1018 (`src/apps/kt-app/src/fx_registry.rs`)

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
LayerMix and the read-only CamMotion form. The barrier kinds are plugins too, with a GPU
pass graph.
Why: the owner (2026-10-30): "take ALL the effects out — a mechanism as flexible and extensible as
possible". A kind kept core for a reason is a reason a third-party effect could never have.

#### Scenario: A positioned tap measures its own stage
- **WHEN** vignette, moved into a plugin, runs alone, inside a fused geometry series that zooms before it, and as a barrier between two blurs, on both executors
- **THEN** the CPU bytes equal the pre-move goldens (its radius measured on its own stage), the GPU is within the point-wise tolerance, the series stays one pass, and the assembled text passes the sub-sample's coordinate to the tap and the stage size in `b.zw`. Noise and greenscreen, whose barrier position is a kernel of its own (integer, K×, an AVX2 twin), move with the barrier pass graph
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
- **WHEN** a compressor, moved into a plugin, keys on another track; the project is saved, reopened and rendered; a second plugin with a key input, unknown to the engine, keys on the same track; the key track is deleted and a new track added
- **THEN** the file holds the former `key` text (absent when none) and the reopened project sounds the same; the second plugin receives the key signal through the row's role with no engine code naming it; the deleted track's key is kept (rule 20, t772: no edit of another device on the user's behalf) and its id is never handed to the new track, so the key does not start listening to someone else's signal
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

### Requirement: A barrier kind is a plugin with a CPU kernel, an optional SIMD twin and a GPU pass graph
A video kind of the barrier class SHALL be a plugin that declares:
- its scalar CPU kernel over the frame, which is the byte reference;
- optionally, a SIMD twin that is byte-equal to the scalar kernel on every frame it accepts;
- its relation to alpha (produces, preserves or consumes);
- for a built-in, a GPU pass graph made of the host's resample and premultiplication nodes and of
  its own fragment passes, which read the source, the previous output or a saved output.

The host SHALL decide when the twin runs: one runtime switch for the whole program, with no
reading of the environment by a plugin. The host SHALL refuse at registration:
- a barrier without a CPU kernel;
- a built-in barrier without a pass graph;
- a graph that reads a saved output before saving it, ends on a save, or whose output is not at
  the frame size;
- a size that can exceed the intermediate-buffer ceiling.

The alpha relation SHALL default to "preserves", the relation that stays silent in the chain's
alpha fold, so a barrier that declares none behaves as one that passes alpha through.

A kind that holds both a positioned tap and a barrier kernel SHALL run as a tap inside a fused
geometry series and as the barrier kernel at a series edge, exactly where the core kind ran in
each.

The core SHALL keep no barrier kind. The effect enum holds only the Geometry lead node, the
read-only CamMotion form, the LayerMix strength marker, the plugin slot and the missing slot.

Why: the owner (2026-10-30): "take ALL the effects out — a mechanism as flexible and extensible
as possible". The barriers were the last kinds kept for a reason (multi-pass GPU, AVX2 twins, K×),
and each reason becomes a contract. The SIMD switch stays one, so that `KT_SIMD=0` still names the
whole scalar reference (R.32).

#### Scenario: The moved barriers keep their bytes
- **WHEN** each of the twelve barrier kinds runs on the CPU at its default and at each built-in variant, at K=1 and K=2, on a frame with and without alpha — first as the core variant, then as the moved plugin
- **THEN** the hashes are those frozen before the move, row for row, and the file text of every kind is unchanged
- **TEST** t1051 (`src/libs/kt-fx/tests/barrier_plugin.rs`), t973

#### Scenario: A pass graph runs on the GPU within the tolerance class
- **WHEN** every registered barrier runs on the GPU executor assembled from the registry: alone, between two geometry series and under a layer strength below one; and a test barrier plugin with a two-pass graph (a saved source, a half-size resample, a final pass reading both) that the host names nowhere runs the same chains
- **THEN** each result is within its kind's existing tolerance class of the CPU, the uniform arena is consumed exactly as predicted from the graph, and the test plugin needs no host code
- **TEST** t1052 (`src/libs/kt-gpu/tests/barrier_graph.rs`), t181, t329

#### Scenario: The SIMD twin is checked over the registry
- **WHEN** every registered barrier that declares a twin runs on the parity frames with the twin and with the twin switched off
- **THEN** the bytes are equal for every kind, the scalar goldens hold with the switch off, and a kind added with a twin is checked without editing the test
- **TEST** t1053 (`src/libs/kt-fx/tests/barrier_plugin.rs`), t198

#### Scenario: An inconsistent barrier is refused before any frame
- **WHEN** test plugins are registered with: no CPU kernel; no pass graph as a built-in; a graph reading a saved output before saving it; a graph ending at a half-size resample or on a save; a size beyond the buffer ceiling at the largest K; a pass naming a shader the kernel lacks; an empty graph on a frame with alpha
- **THEN** each registration is refused with the reason named, and the registry is unchanged
- **TEST** t1054 (`src/libs/kt-plugin-api/tests/barrier_registry.rs`), t972

#### Scenario: The core keeps no barrier
- **WHEN** the effect enum's variants and the barrier position of the CPU executor are listed after the move
- **THEN** the enum holds exactly the Geometry node, CamMotion, LayerMix, the plugin slot and the missing slot, and the executor names no effect kind; an edit of a barrier kernel recompiles no core library
- **TEST** t1056 (`src/libs/kt-model/tests/plugin_format.rs`), t977

### Requirement: A generator kind may synthesize its sound; there is one sine text
A generator kernel SHALL either declare no sound, a level with an optional gate played by the
host's sine (`tone`/`gate`), or a synthesizer (`Synth`) that fills one interleaved stereo burst
of `AUDIO_BURST_FRAMES` frames from the event's values and the GLOBAL sample number of its first
frame; a synthesizing kind has no picture. The registry SHALL refuse, by id, a kind that declares
both a synthesizer and a tone. The sine and the dBFS-to-amplitude conversion SHALL live once, in
`kt_plugin_api::generator` (`sine`, `db_amp`), called by the host's tone and by every kernel that
plays a sine, through the frozen math (`kt_plugin_api::math`). The built-in set SHALL include the
`generators-audio` family after `generators-test`.
Why: a level and an on/off cannot express a frequency, a noise colour or a sweep; two sine texts
would drift in phase and in bytes (the rule of R.53), and a platform `sin` would differ across
machines (t1066).

#### Scenario: Two answers to "what does it sound like" are refused
- **WHEN** a generator kind declaring both a synthesizer and a tone is registered; then a kind with a synthesizer only
- **THEN** the first is refused with an error naming its id and the conflict, the second is registered and its kernel reports the synthesizer
- **TEST** t1162 (`src/libs/kt-plugin-api/tests/registry.rs`)

#### Scenario: The host's tone and the tone kind share the sine
- **WHEN** the SMPTE tone (host `tone_1k`) and the `tone` kind at 1 kHz read the same bursts at the same level
- **THEN** the bytes are equal
- **TEST** t1155 (`src/libs/kt-media/tests/gen_sound.rs`)

### Requirement: A barrier pass may render at its own size, and a barrier may read the passport values
A barrier's GPU graph MAY contain a pass rendered at its own size (`Node::PassAt`, both sides in
1..=the intermediate-buffer ceiling); the graph's output SHALL still be at the frame size, and the
registry SHALL refuse a pass-at-size of zero or above the ceiling as it refuses such a resample. The
host SHALL count such a pass in the uniform arena and in the intermediate-size check like any pass.

A barrier kernel MAY read its passport values directly (`frame_values`, `gpu_values`) when its
parameters do not fit the eight cooked numbers. The default forms SHALL cook the values and call the
eight-number kernel, and the host SHALL call the value forms, so every existing barrier keeps its
bytes and its graph.
Why: the VHS tape simulation (kkt-26.11.01 V1) runs on a signal grid of the standard, independent of
the frame size (a 640-wide preview has fewer texels than the grid), and has a deck's worth of physical
quantities (~30).

#### Scenario: A pass at its own size is checked like a resample
- **WHEN** test barriers are registered with a graph containing a pass at 1024×576 that ends with a
  frame-size pass; a pass at 0×576; a pass above the ceiling; and a graph ending on the 1024×576 pass
- **THEN** the first is accepted; the second and third are refused naming the size; the last is
  refused for ending off the frame size
- **TEST** t1187 (`src/libs/kt-plugin-api/tests/barrier_registry.rs`)

#### Scenario: The value forms keep every barrier's bytes
- **WHEN** every registered barrier with at most eight passport rows runs through the host's value form and through its cooked form at
  its defaults and its top
- **THEN** the frames are byte-equal and the graphs equal
- **TEST** t1187 (`src/libs/kt-fx/tests/barrier_plugin.rs`), t1051, t1052

### Requirement: A layer strength over a step that destroys alpha keeps each side's alpha on both executors
When a layer strength (a mix of the chain before a step with the chain after it) stands over a step
whose relation to alpha is "destroys", the GPU executor SHALL mix the dry side with its OWN alpha
(the chain's fold at the snapshot) and the wet side with its own, and the mixed frame SHALL have
alpha when either side has it — as the CPU's frames do. The end-of-chain flag SHALL NOT stand for
the dry side.
Why: found by the VHS tape simulation (vhs-tape-simulation), the first barrier that declares
"destroys": t1052 ran it under a strength of 0.5 on a frame with alpha, and the GPU forced the dry
side opaque (it passed the chain's final flag for both sides) and then declared the mixed layer
opaque, so the composite skipped the alpha-over — 86 % of the samples differed from the CPU.

#### Scenario: A destroying barrier under a strength on a frame with alpha
- **WHEN** every registered barrier runs under a layer strength of 0.5 on a frame with gradient alpha, on both executors
- **THEN** the GPU result is within the kind's tolerance class of the CPU, the alpha channel included
- **TEST** t1052 (`src/libs/kt-gpu/tests/barrier_graph.rs`)

### Requirement: A stream kind is a source stage with a declared history, not video state
A video plugin MAY declare the planner class `Stream` and provide a `StreamKernel`. Such a kernel
SHALL declare no state, tail or look-ahead; its output at frame N SHALL be a pure function of its
rows — read per position WITH their curves through the layout (`rows_at`) — and of the source frames
of the HISTORY it names for N through the stream's layout. The layout SHALL also carry a REGION OF
INTEREST per frame (`region_ident`, `with_region`: a coverage plane at the stream's size with an
identity), the bridge for masks and object detectors; no region means the whole frame. The host
SHALL own the memo the kernel keeps results in (`StreamMemo`, bounded, keyed by what determines
the result) and SHALL hand it to the kernel; the bytes MUST NOT depend on what the memo holds.
The registry SHALL refuse: a `Stream` class without a stream kernel, a stream kernel on another
class, and a stream plugin offering any GPU side. A built-in stream plugin is exempt from the GPU
default (the codec is a serial loop; its output reaches both executors as a pre-FX frame).

Why: F-FX-007 named the temporal family's way through the purity rule — read the PAST SOURCE,
do not keep state; `datamosh-codec-path` design D2–D4.

#### Scenario: The registry refuses a malformed stream plugin
- **WHEN** a plugin declares the `Stream` class without a stream kernel, or with a WGSL side, or a point-wise plugin offers a stream kernel
- **THEN** registration is refused naming the declaration, and a well-formed stream plugin registers
- **TEST** t1240 (`src/libs/kt-plugin-api/tests/registry.rs`)

#### Scenario: The datamosh keeps its file text
- **WHEN** a slot `datamosh` with default rows and with every row moved is written and read again
- **THEN** the text is the frozen one, the rows at their defaults are omitted where declared, and the golden table holds its row
- **TEST** t1247 (`src/libs/kt-model/tests/plugin_format.rs`), t328 (`src/libs/kt-fx/tests/video_fx.rs`)

### Requirement: A barrier sees all its rows through the values door
`BarrierKernel` SHALL offer `frame_values` and `gpu_values`, which receive the slot's values in
passport order; their defaults SHALL be `frame`/`gpu` over `cook`, so a barrier that implements only
the cooked forms keeps its bytes. The host SHALL call the values forms everywhere it runs or
plans a barrier: the CPU executor's barrier position, the GPU barrier op and the registry's graph
check on the defaults and the top of the passport.
Why: `Cooked` is eight numbers. A simulation (camera_isp, ~50 rows) cannot pack its passport into
them without quantizing every automated row into bit-fields; the values door keeps the rows as
the passport declares them.

#### Scenario: A row past the eighth reaches the kernel on both executors
- **WHEN** a test barrier with twelve rows, whose output depends only on row 11, is registered and
  run through the CPU executor and its GPU graph is built, with row 11 at two values; and a
  built-in barrier (blur) runs through the same doors
- **THEN** the two values of row 11 give two different outputs on the CPU and two different graphs,
  the registry checked the graph on row 11's top, and blur's bytes are its frozen golden
- **TEST** t1210 (`src/libs/kt-fx/tests/barrier_values.rs`), t1051
