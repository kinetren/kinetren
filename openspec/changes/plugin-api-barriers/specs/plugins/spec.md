# Spec Delta

## ADDED Requirements

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
