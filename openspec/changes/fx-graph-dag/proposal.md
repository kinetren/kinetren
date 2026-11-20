# Proposal

> DESIGN ONLY. The owner sees this design first; code starts on his word ("discussion is not a
> go"). The questions that are his are at the end of `design.md`, each with his words, today's state
> and what each option means.

## Why

The owner, kkt-26.11.18b item 3 (registry row U3, `partial`): "нужно что-то типо компактного удобного
графа делать". His answer U7 chose a node editor "типо как в blender". `fx-graph` v1 (v1a–v1d, in
master) gave every chain that editor, over today's model, where a chain is a LINE. A node editor
promises more than a line, and v1 says so on every refused gesture: "branches arrive in v2" (the
`ui-reach` requirement of `fx-graph`: "a gesture the engine cannot execute yet (a branch, a second
input, a cycle) SHALL be refused with a reason").

What a line cannot say and every node compositor can:

- **blur only the highlights and lay them back** (a key of the highlights, a blur, a merge over the
  original) — the classic glow done by hand;
- **an effect through a matte made from the picture itself** (a luma key of one branch drives how
  much of another branch shows), the per-pixel version of the per-slot shape masks;
- **parallel sound** (parallel compression; a reverb that only part of the sound goes into; a
  compressor keyed by a filtered copy of its own input — the in-chain sidechain EQ).

v2 makes those graphs the ENGINE's: `frame_plan` executes them for the preview and the export
through one path (invariant 7), and the audio render thread executes them on the sound.

## What Changes

- **The chain carries its wiring.** Every chain in the model (an event's picture and sound, a track
  stage, a morpher patch's two chains, a composition instance) keeps its `fx` vector and gains one
  sparse field: a slot's inputs when they are not "the slot before". Slot order stays a processing
  order (inputs point only to earlier slots, so no cycle can be written). A chain whose every slot
  takes the slot before is today's line, byte for byte: same file text, same hashes, same frames,
  same samples.
- **One new node, Merge** (the Nuke/Fusion name): a Background input, a Foreground input, an
  optional Matte input (another branch's alpha or luma), a mode and a mix. Picture modes are the
  nine layer blend modes, with the same kernels; sound has Sum and Crossfade. Merge is a core node
  (routing, like the layer strength), not a plugin: the plugin contract stays single-input.
- **The picture plan runs a graph as a straight program with held frames.** `plan_chain` turns a
  graph into today's op list plus three ops — hold the current frame, load a held one, merge — and
  still fuses every linear run between them (geometry series, point-wise runs, K× barriers,
  untouched). The executor already holds two frames this way (the layer strength's dry side, a
  slot mask's dry side); v2 generalises it to a small numbered set.
- **The sound processor runs a graph** in slot order on pool buffers, aligning branches by their
  look-ahead before a merge (the layer strength's dry ring, generalised) and summing in a fixed
  order (invariant 11).
- **Time and history stay on the trunk.** Slowed (the warp), the stream stage (Datamosh), the
  pitch's edge reading and the leading Geometry node stand before the first branch: each layer still
  reads exactly one source at one material moment, so prefetch, coverage, the cache keys and the
  stream history do not change (invariants 4, 5, 8). A wire that would put one inside a branch is
  refused with the reason (owner question Q3).
- **The editor draws and makes branches**: a wire dragged from a node's output into empty canvas
  starts a branch there; a wire dropped on a Merge's socket connects it; arms are laid out
  automatically as parallel rows (his U8 answer: automatic layout, nothing stored).
- **Format by content**: a file with a branch or a Merge is project format 6 (the next free number
  at merge) and `metadata["kt"]` v17; a line project keeps its version and bytes. OTIO export keeps
  writing the chain as its ordered `Effect.1` list and carries the wiring in each effect's `kt`
  metadata; a foreign editor never ran our `kt:` effects, and now also could not read their
  wiring — a row in "What OTIO export loses".

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-plan`: a chain is a graph; one plan builder lowers it; the Merge law; a line graph is the
  chain byte for byte.
- `audio-fx`: the sound chain executes a graph; branches aligned by look-ahead; the trunk-only kinds.
- `morpher`: a patch's graph is spliced after its target's output.
- `project-format`: the wiring and Merge raise the file by content; OTIO carries the wiring in the
  envelope.
- `ui-reach`: the node editor makes branches and merges instead of refusing them.

## Impact

- kt-model: `FxInput` per slot, the Merge kind, validation (earlier slots only, one lane, the
  trunk-only kinds, at most `MAX_HELD` live branches), the morpher splice, copy/paste and presets
  carry the inputs, `project_file` and `normalize`.
- kt-fx: `plan_chain` lowers a graph (`Hold`/`Load`/`Merge` ops); `ClipProcessor` executes a graph.
- kt-gpu: the held set, the Merge pass (the layer blend kernels + the strength mix pass with a
  matte made on the device), cost rows, prewarm.
- kt-engine: keys (`fx_hash`, `durable_hash`, the audio fingerprint) hash the wiring when present;
  the Viewer cuts to a node's ancestors.
- kt-otio: the `in` field on `Effect.1` metadata, `kt:merge`.
- kt-app: branch gestures and the arms' layout in `node_editor`; i18n ×5.
- Tests reserved: t3200–t3229 (shared notes `fx-graph.md`).
- Feature note: F-UI-008 (`docs/features/ui/editing/fx-chain-graph.md`) gains the branch/merge
  reconnaissance.
