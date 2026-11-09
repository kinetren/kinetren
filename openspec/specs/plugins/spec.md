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
- **WHEN** vignette, moved into a plugin, runs alone, inside a fused geometry series that zooms before it, and as a barrier between two blurs
- **THEN** the frames are within the point-wise tolerance of their stored frames (its radius measured on its own stage), the series stays one pass, and the assembled text passes the sub-sample's coordinate to the tap and the stage size in `b.zw`. Noise and greenscreen, whose barrier position is a kernel of its own (integer, K×), move with the barrier pass graph
- **TEST** t992 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: A geometry program fuses and keeps its rules
- **WHEN** the five warp kinds, corner_pin, event_grid and pan3d, moved into plugins, run in series of any length
- **THEN** the frames are within the geometry tolerance of their stored frames, a series of geometry plugins is one pass, event_grid's exact tile program still decides from the primitives, and pan3d's points behind the camera and outside the quad stay transparent
- **TEST** t993 (`src/libs/kt-gpu/tests/parity.rs`), t944, t298

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
mapped to the closed primitive set. The chain plan SHALL be built by the single planner from the
manifests' classes. Anything that is hashed into a cache key SHALL use the
plugin id, never the executor's dispatch number. The GPU kernel SHALL be held to the existing
tolerance classes of the stored frames.
Why: invariant 7 — one composition path, geometry series in one pass;
an assembled shader is a pure function of the registry, so GPU determinism between runs on one
device (invariant 11) holds; a dispatch number depends on what is registered and would silently
shift keys.

#### Scenario: The pilot plugin stays within its class of the stored frames
- **WHEN** Posterize, moved into its family crate, runs alone, inside a fused series
  (`Mirror, Posterize, Vignette`) and as a barrier between two blurs, on the GPU executor assembled
  from the registry; separately a geometry plugin offering a WGSL
  string is registered
- **THEN** the frames are within the existing point-wise tolerance of their stored frames, the fused series is still one pass, the
  assembled shader text is identical between two executor creations, and the geometry plugin is
  refused (t972)
- **TEST** t974 (`src/libs/kt-gpu/tests/parity.rs`)

### Requirement: A barrier kind is a plugin with a GPU pass graph
A video kind of the barrier class SHALL be a plugin that declares:
- its kernel (`BarrierKernel`), which cooks the values and builds the graph;
- its relation to alpha (produces, preserves or consumes);
- for a built-in, a GPU pass graph made of the host's resample and premultiplication nodes and of
  its own fragment passes, which read the source, the previous output or a saved output.

The host SHALL refuse at registration:
- a barrier without a kernel;
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
as possible". The barriers were the last kinds kept for a reason (multi-pass GPU, K×), and each
reason becomes a contract.

#### Scenario: The moved barriers keep their file text
- **WHEN** a project with each of the twelve barrier kinds is written, first as the core variant, then as the moved plugin
- **THEN** the file text of every kind is unchanged
- **TEST** t973 (`src/libs/kt-model/tests/plugin_format.rs`)

#### Scenario: A pass graph runs on the GPU within the tolerance class
- **WHEN** every registered barrier runs on the GPU executor assembled from the registry: alone, between two geometry series and under a layer strength below one; and a test barrier plugin with a two-pass graph (a saved source, a half-size resample, a final pass reading both) that the host names nowhere runs the same chains
- **THEN** each result is within its kind's existing tolerance class of the stored frames, the uniform arena is consumed exactly as predicted from the graph, and the test plugin needs no host code
- **TEST** t1052 (`src/libs/kt-gpu/tests/barrier_graph.rs`), t181, t329

#### Scenario: An inconsistent barrier is refused before any frame
- **WHEN** test plugins are registered with: no kernel; no pass graph as a built-in; a graph reading a saved output before saving it; a graph ending at a half-size resample or on a save; a size beyond the buffer ceiling at the largest K; a pass naming a shader the kernel lacks; an empty graph on a frame with alpha
- **THEN** each registration is refused with the reason named, and the registry is unchanged
- **TEST** t1054 (`src/libs/kt-plugin-api/tests/barrier_registry.rs`), t972

#### Scenario: The core keeps no barrier
- **WHEN** the effect enum's variants are listed after the move
- **THEN** the enum holds exactly the Geometry node, CamMotion, LayerMix, the plugin slot and the missing slot, and an edit of a barrier kernel recompiles no core library
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

A barrier kernel MAY read its passport values directly (`gpu_values`) when its parameters do not
fit the eight cooked numbers. The default form SHALL cook the values and call the eight-number
kernel, and the host SHALL call the value form, so every existing barrier keeps its graph.
Why: the VHS tape simulation (kkt-26.11.01 V1) runs on a signal grid of the standard, independent of
the frame size (a 640-wide preview has fewer texels than the grid), and has a deck's worth of physical
quantities (~30).

#### Scenario: A pass at its own size is checked like a resample
- **WHEN** test barriers are registered with a graph containing a pass at 1024×576 that ends with a
  frame-size pass; a pass at 0×576; a pass above the ceiling; and a graph ending on the 1024×576 pass
- **THEN** the first is accepted; the second and third are refused naming the size; the last is
  refused for ending off the frame size
- **TEST** t1187 (`src/libs/kt-plugin-api/tests/barrier_registry.rs`)

### Requirement: A layer strength over a step that destroys alpha keeps each side's alpha
When a layer strength (a mix of the chain before a step with the chain after it) stands over a step
whose relation to alpha is "destroys", the GPU executor SHALL mix the dry side with its OWN alpha
(the chain's fold at the snapshot) and the wet side with its own, and the mixed frame SHALL have
alpha when either side has it. The end-of-chain flag SHALL NOT stand for
the dry side.
Why: found by the VHS tape simulation (vhs-tape-simulation), the first barrier that declares
"destroys": t1052 ran it under a strength of 0.5 on a frame with alpha, and the GPU forced the dry
side opaque (it passed the chain's final flag for both sides) and then declared the mixed layer
opaque, so the composite skipped the alpha-over — 86 % of the samples differed.

#### Scenario: A destroying barrier under a strength on a frame with alpha
- **WHEN** every registered barrier runs under a layer strength of 0.5 on a frame with gradient alpha
- **THEN** the result is within the kind's tolerance class of the stored frames, the alpha channel included
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
default (its output reaches the executor as a pre-FX frame).

Why: F-FX-007 named the temporal family's way through the purity rule — read the PAST SOURCE,
do not keep state; `datamosh-codec-path` design D2–D4.

#### Scenario: The registry refuses a malformed stream plugin
- **WHEN** a plugin declares the `Stream` class without a stream kernel, or with a WGSL side, or a point-wise plugin offers a stream kernel
- **THEN** registration is refused naming the declaration, and a well-formed stream plugin registers
- **TEST** t1240 (`src/libs/kt-plugin-api/tests/registry.rs`)

#### Scenario: The datamosh keeps its file text
- **WHEN** a slot `datamosh` with default rows and with every row moved is written and read again
- **THEN** the text is the frozen one, the rows at their defaults are omitted where declared
- **TEST** t1247 (`src/libs/kt-model/tests/plugin_format.rs`)

### Requirement: A barrier sees all its rows through the values door
`BarrierKernel` SHALL offer `gpu_values`, which receives the slot's values in passport order; its
default SHALL be `gpu` over `cook`, so a barrier that implements only the cooked form keeps its
graph. The host SHALL call the values form everywhere it plans a barrier: the GPU barrier op and the
registry's graph check on the defaults and the top of the passport.
Why: `Cooked` is eight numbers. A simulation (camera_isp, ~50 rows) cannot pack its passport into
them without quantizing every automated row into bit-fields; the values door keeps the rows as
the passport declares them.

#### Scenario: Rows past the eighth reach the graph
- **WHEN** camera_isp, whose passport is far longer than eight rows, runs its defaults and every preset on the GPU executor
- **THEN** each frame is within `FrameTol::THRESHOLD` of its stored frame
- **TEST** t1217 (`src/libs/kt-gpu/tests/camera_isp.rs`)

### Requirement: A simulated device's two halves are one passport
A video kind MAY declare the audio kind that records the same device's sound (`Decl.pair`). The
two SHALL have the same passport and row forms; the registry SHALL refuse, by name and in either
registration order, a pair whose passports differ, a pair that is not an audio effect, an audio kind
that declares a pair, a pair declared by a kind that is not a video effect, and two video kinds naming
one audio kind. `Registry::counterpart` SHALL answer both directions, and the built-in assembler SHALL
refuse a declared pair whose audio half is not registered.
Why: the owner, kkt-26.11.03b D1 — a simulated device that records picture and sound simulates both,
and the two halves are one device; a row only one half had would be a parameter only one half of the
device obeys.

#### Scenario: The registry holds a pair to one passport
- **WHEN** a video kind naming `tape_sound` is registered, then `tape_sound`; then a second video kind
  naming it, an audio kind declaring a pair, a pair with a different default (both orders), a pair
  naming a video kind, an audio pair declaring a pair
- **THEN** the first two register and name each other as counterparts, `unpaired` empties when the
  sound arrives; every other one is refused naming its reason
- **TEST** t1484 (`src/libs/kt-plugin-api/tests/pairs.rs`)

### Requirement: An audio state knows where its clip sits on the timeline
The host SHALL tell every audio state the timeline frame of its clip's frame 0 and the project frame
rate (`AudioState::place`), when the state is built and whenever the clip is synced, without a reset
(a moved clip keeps its state). An audio half of a simulated device SHALL read the device state at the
TIMELINE position its picture reads, derived from the rows and that position alone — no state is
shared with the picture's executor.
Why: the picture of a tape or a stream reads the absolute timeline tick; an audio state saw only its
clip-local frame, so the sound could not lose the field the picture loses.

#### Scenario: A clogged head's field on an event off the field grid
- **WHEN** an audio event starting 3.01 s into the timeline (half a PAL field off the tape's grid)
  carries the VHS deck's sound with head A clogged, rendered by the engine; then the slot switched off;
  then rendered again
- **THEN** the sound is lost while head A reads on the TIMELINE's tape grid (eight times head B's
  error); switched off it gives the dry bytes; the two renders are byte-equal
- **TEST** t1467 (`src/libs/kt-engine/tests/sim_pairs.rs`)

### Requirement: A kernel runs compute shaders through the host's door
A kernel SHALL be able to name WGSL compute shaders and buffers as a job the host runs on its own
compute device (Vulkan, DX12 or Metal, whatever backend the interface uses). The host SHALL report a
shader it cannot compile as an error, without a panic. With no compute device `host()` SHALL give
nothing, and the kernel SHALL say so instead of drawing something else.
Why: the owner, C2 of kkt-26.11.03 — the math runs on Vulkan rather than on hand-written copies; a
plugin still sees nothing of wgpu.

#### Scenario: A job runs and reads back
- **WHEN** a job doubles the words of a buffer in a compute shader, on a machine with a compute device
- **THEN** the buffer read back holds the doubled words, and a second job with the same text reuses
  the compiled shader
- **TEST** t1502 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: A shader the device refuses is an error
- **WHEN** a job names a shader that does not compile
- **THEN** `run` returns an error naming it, and the next valid job on the same host runs
- **TEST** t1503 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The Datamosh encoder keeps its quality on the compute device
- **WHEN** a moving textured picture is encoded in a closed loop at QP 24
- **THEN** the reconstruction's luma PSNR holds above 40 dB on every P-frame, and most blocks of the
  moving area carry the true motion
- **TEST** t1504 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: Datamosh with no compute device leaves the frame and says why
- **WHEN** the stream stage runs with no compute host installed
- **THEN** the frame is the source frame, and `compute::unavailable` gives the panel its reason
- **TEST** t1505 (`src/plugins/fx-video-codec/tests/no_device.rs`)

### Requirement: The compute host reuses a job's buffers without showing it
The host SHALL hand a finished job's buffers to later jobs of the same size and usage, SHALL keep at
most 1 GiB of them, and a buffer a job asks for as zero SHALL be zero whatever job used it before.
Why: a kernel asks for the same set every frame; the pool must stay invisible to the bytes
(project rule on caches).

#### Scenario: A zero buffer stays zero after reuse
- **WHEN** a job fills a buffer, and the next job asks for a zero buffer of the same size
- **THEN** the buffer read back holds zeros
- **TEST** t1570 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The pool is bounded
- **WHEN** the same job runs twice, then jobs hold more than the bound
- **THEN** the second run does not grow the pool, and the pool never passes 1 GiB
- **TEST** t1571 (`src/libs/kt-gpu/tests/compute_host.rs`)

### Requirement: The Datamosh decoder is a compute shader
The block copy and the residual SHALL run as a compute job with no CPU copy: unrestricted vectors
clamped at the frame edge, the chroma vector halved and floored, the residual scaled by the gain and
floored, the result clamped to bytes.
Why: the owner, C2 of kkt-26.11.03; `predict` and `reconstruct` were the last single-thread stage of
a clean decode.

#### Scenario: The decoder's arithmetic
- **WHEN** a picture decodes with known vectors, a residual of both signs and the gain 128
- **THEN** each pixel is the copy at the clamped position plus the floored scaled residual
- **TEST** t1572 (`src/plugins/fx-video-codec/tests/codec.rs`)

### Requirement: A test model answers to its shaders
A simulation whose physics tests read an f32 model SHALL have a test running the model and the GPU side
by side at every preset, by the mean of the difference, so that a pass edited alone is red.
Why: tests that read a model say nothing of a shader that left it.

#### Scenario: The tape
- **WHEN** one frame runs through the model and through the shaders, at the defaults and each preset
- **THEN** the mean difference is under 0.02 (a clogged head: 0.9)
- **TEST** t1640 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The camera
- **WHEN** one frame runs through the model and through the shaders, at the defaults and each preset
- **THEN** the mean difference is under 0.002
- **TEST** t1641 (`src/plugins/fx-video-camera/tests/consequences.rs`)

### Requirement: Every shader compiles on D3D12 in CI
The Windows job SHALL compile every engine pipeline and every compute shader on the D3D12 adapter
through FXC, and SHALL run no frame on WARP.
Why: FXC refuses what naga and Vulkan pass, and no other job reaches it.

#### Scenario: The engine's pipelines
- **WHEN** the engine builds on a D3D12 adapter, WARP included, in compile-only mode
- **THEN** every shader compiles; with no D3D12 the test skips
- **TEST** t1650 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The compute shaders
- **WHEN** the codec's shaders compile on a D3D12 adapter
- **THEN** none is refused; with no D3D12 the test skips
- **TEST** t1651 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: The generators' shaders
- **WHEN** the generators' shaders compile on a D3D12 adapter
- **THEN** none is refused; with no D3D12 the test skips
- **TEST** t1652 (`src/libs/kt-gpu/tests/compute_host.rs`)

### Requirement: An audio kernel's block runs at the CPU's level, with the same bytes
An audio plugin state's `process` SHALL run its per-block body through `kt_plugin_api::fast!`: compiled
a second time with AVX2 enabled and taken where the CPU reports AVX2, the baseline x86-64 build
otherwise (and on every other architecture as built). The two SHALL give the same bytes: no FMA in the
feature set, no reordered float sums, the rounding functions exact by IEEE 754. `KT_SIMD=0` forces the
baseline path for the process; `cpu::with_baseline` does so for one thread (the reference of t1981). A
per-sample helper the body calls is `#[inline]`, or it keeps the baseline code.
Why: SSE2 has no instruction for `floor`/`ceil`/`round`/`trunc`: on the shipped build each one on a
per-sample path was a call into the C library through the PLT — ten of the 21 audio kernels, fifteen
call sites in VHS Tape's sound — and the loops around them could not use wider registers.

#### Scenario: Both paths give the same bytes
- **WHEN** every audio kind's digests are taken on the CPU's path and inside `cpu::with_baseline`
- **THEN** both equal the digests printed before the paths existed
- **TEST** t1981 (`src/libs/kt-engine/tests/audio_fx_cost.rs`)

### Requirement: A cheaper form of a frozen function is that function to the bit
A replacement of a frozen-math call on a per-sample path by a cheaper form SHALL return the same bits on
every input, proved exhaustively where the input is one `f32`. `math::powf_neg_half(x)` is
`powf(x, -0.5)`: `1 / sqrt(x)` in `f64`, and `powf` itself outside the positive finite numbers and where
the `f64` value lies within 2⁸ `f64` ulps of a boundary between two `f32` roundings.
Why: VHS Tape's Hi-Fi compander took a `pow` per sample per channel, 17 % of the effect's cost; kt-fx
CLAUDE.md: a rounding trick is closed by an exhaustive test.

#### Scenario: The replacement equals pow on every float
- **WHEN** `powf_neg_half` and `powf(x, -0.5)` are taken on every positive finite `f32`, zero, the infinities, NaN and a negative
- **THEN** their bits are equal on all of them
- **TEST** t1982 (`src/libs/kt-plugin-api/tests/math.rs`)

### Requirement: A kernel names its passes and its dispatches
A barrier kernel MAY name each of its `shaders()` with `BarrierKernel::pass_names` (same order, same
length); the host calls an unnamed pass `p<i>`. A compute `Step` SHALL carry a `name`. The names are what
the telemetry journal's GPU record shows (`vhs_tape:s5-head-read`, `datamosh:search`,
`generator:noise`); a host node inside a barrier's graph is named after the barrier (`vhs_tape:resample`).
The multi-pass simulations (VHS Tape, Camera ISP) name every pass.
Why: gpu-pass-cost — a record that says "p6" tells the owner nothing.

#### Scenario: Every tape pass has its name
- **WHEN** a frame with VHS Tape is timed
- **THEN** no pass of the tape is called `p<i>`, and its resample to the grid is `vhs_tape:resample`
- **TEST** t2020 (`src/libs/kt-gpu/tests/pass_time.rs`)

### Requirement: A plugin names its compute shaders; the host prepares them before the first frame
A plugin whose kernels run shaders on the compute host SHALL name every one of them in
`Plugin::compute_shaders` (the text and how many buffers a step reads and writes); a generator kind
drawn with `shader_frame` names its `pixel` text with `Kind::with_shader`, from the same constant.
`compute::prewarm(shaders)` SHALL return at once, open the host on a thread of its own and call
`ComputeHost::prepare` for each shader not asked before; `prepare` compiles the pipeline and dispatches
it once over zero buffers. A job that compiles a text SHALL count it in `kt_gpu::pipelines_in_frame()`
and a first dispatch of a pipeline never dispatched in `kt_gpu::cold_draws()`.
Why: found-defects 64 — the first Datamosh frame opened the compute device and compiled its shaders
inside the frame (6.6 s under load); measured on llvmpipe, the residual shader alone compiles 4.1 s
with an empty Mesa cache.

#### Scenario: A prepared Datamosh compiles nothing in its first frame
- **WHEN** the host prepares the shaders Datamosh names, then the first intra frame is encoded, a predicted one and a decode run
- **THEN** no pipeline is created and none dispatched cold
- **TEST** t2122 (`src/plugins/fx-video-codec/tests/compute_prewarm.rs`)

#### Scenario: Every generator kind names its shader
- **WHEN** the host prepares what every generator kind of both families names, then each kind draws its first frame at its defaults
- **THEN** no frame creates a pipeline or dispatches one cold (before: the leader named none)
- **TEST** t2123 (`src/libs/kt-gpu/tests/compute_prewarm.rs`)

#### Scenario: The prewarm returns at once and prepares each shader once
- **WHEN** two shaders are prewarmed twice while the host is still opening, then one of them again beside a new one
- **THEN** both calls return before the host opens; each shader is prepared once, and the third call prepares only the new one
- **TEST** t2124 (`src/libs/kt-plugin-api/tests/compute_prewarm.rs`)

### Requirement: A smoother knows when it stands still, and every variant has a general reference
`dsp::SmoothedParam::still` SHALL be true exactly when a tick would leave the value's bits unchanged (a
fixed point of the tick, which then holds until the target moves; not `value == target`), and
`advance(n)` SHALL give the bits of `n` ticks, stopping at the first tick that changes nothing. A kernel's
"stands still" variant (a constant-parameter block, a skipped re-tune, a skipped computation whose answer
is known) SHALL give the bytes of its general path; `cpu::with_general` runs every such variant on the
calling thread on its general path, the reference the tests hold the variants to. No variant adds a
platform math call or an x86-only intrinsic without a fallback.
Why: the owner, 2026-11-09 (kkt-26.11.09 I9): "запускай edit-without-rebuild и kernel-specialization";
engine-audit §3, tier T2 — specialized Rust chosen at run time; invariant 11.

#### Scenario: Still is a fixed point; advance is n ticks
- **WHEN** smoothers glide up, down, through −0, to a target they stop short of, at 0.01–150 ms, and `advance(n)` runs on fresh glides on both paths
- **THEN** `still()` agrees with every tick, no smoother moves after standing still, and `advance(n)` and the tick after it equal `n` and `n+1` ticks to the bit
- **TEST** t2170 (`src/libs/kt-plugin-api/tests/stands_still.rs`)

### Requirement: A point-wise kernel declares that it fuses, and the registry checks the text
`kt_plugin_api::fuses_in_runs(plugin)` SHALL be the one answer to "does this stage fuse into a
point-wise run": a video plugin of class `PointWise` whose GPU side is a `tap_<id>` WGSL and that has
no barrier kernel (a point-wise plugin whose barrier position is a kernel of its own runs that kernel
there, and ends a run). The planner and the GPU executor SHALL ask it and nothing else. `Registry::register`
SHALL refuse a point-wise plugin whose WGSL does not define `fn tap_<id>(`, or that reads a texture
(`textureLoad`, `textureSample`, `textureDimensions`) or the host's uniform `P`: the tap is a pure
function of its value, its numbers and its coordinate, so the host can paste it into any pass.
Why: fused-pointwise-runs — a run's pass is generated from the stages' tap functions; a tap that
reads a neighbourhood or the host's bindings would compile in its own pass and break in a run.

#### Scenario: The registry refuses a tap that is not a pure point function
- **WHEN** point-wise plugins register with a WGSL lacking its `tap_<id>`, with a `textureLoad`, and with a read of `P`
- **THEN** each is refused with the reason; every built-in point-wise plugin registers, and `fuses_in_runs` is true for posterize, color grade, duotone and vignette and false for noise, greenscreen, every barrier and geometry plugin and every audio kind
- **TEST** t2194 (`src/libs/kt-plugin-api/tests/registry.rs`), t2195 (`src/libs/kt-plugins-builtin/tests/fuses.rs`)

### Requirement: A stream kernel stops between history positions when nobody wants the frame
`StreamInput` SHALL offer `stop()` (default `false`, additive to the plugin API). A stream kernel SHALL
ask it before each history position it computes and, when it answers `true`, return `None` with what it
computed kept in the memo, so a later render resumes there and gives the same bytes. The host answers
from a stop check its caller set: the preview's compose thread stops a frame that a newer ask or an
edit replaced, the prefetch thread an advance the playhead or an edit left, a reel strip's or a poster's
pre-roll a cancelled job. An export never stops.
Why: row 147 — a composite the UI had left ran its stream history to the end (up to 300 frames of
compute jobs): in live `ui_wait` the preview's thread held 4 cores for seconds after the pause.

#### Scenario: A history stops within one position and resumes to the same bytes
- **WHEN** a Datamosh history of 40 positions is rendered with a stop that turns true after the 5th compute job, then rendered again with no stop, and a third time in a fresh memo with no stop
- **THEN** the first render returns `None` after at most one more position, the memo holds what it computed, the second finishes from there, and its bytes equal the third's
- **TEST** t2257 (`src/plugins/fx-video-codec/tests/stop.rs`)

#### Scenario: The preview stops a history it left
- **WHEN** a frame's long stream history runs under the compose thread's stop check, and the check turns true (a newer ask, an edit)
- **THEN** the history stops before its end, the frame is incomplete, the stop is traced as the compose thread's, the memo keeps what was computed, and the next composite resumes to the bytes of an uninterrupted one
- **TEST** t2258 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The look-ahead stops when the playhead leaves
- **WHEN** the prefetch thread's advance of a long history runs under its stop check, and the check turns true (a seek, an edit)
- **THEN** the advance stops at the next position, says it is still pending, and the trace marks the advance stopped; a later advance finishes it
- **TEST** t2259 (`src/libs/kt-engine/tests/decode_storm.rs`)
