## ADDED Requirements

### Requirement: A heavy effect's result is kept on disk per layer and read before the layer's chain runs
The persistent cache SHALL keep, besides whole frames, the picture of a layer after a heavy point of its own plan — a
node, stream stage, track stage or reel instance output whose measured device time is at least half a frame period —
under a key made only of what determines that picture (the source's identity and material position, the decode size,
the chain prefix's content and its curve values at that moment), with the decoder's major version, the versions of the kernels that ran and the composite path's version, not the
program build; before
a layer runs its chain, the plan SHALL start from the deepest held cut of that layer and run only the nodes after it.
An edit of another layer, of a node after the cut, or of the event's timeline position SHALL keep the entry.
Why: the owner, kkt-26.11.20b B1 — "У НАС СУТЬ ПО БОЛЬШЕЙ ЧАСТИ В КЭШИРОВАНИИ ЧТОБЫ ЭФФЕКТЫ ТЯЖЕЛЫЕ НЕ СЧИТАТЬ!"

#### Scenario: Moving a title over a heavy event keeps the heavy work
- **WHEN** a range with a heavy Datamosh event under a title is prerendered, the title is moved, a Blur after the Datamosh is changed, and the event is moved 2 s later on the timeline
- **THEN** after each edit the Datamosh stage is read from the store (no stream decode for it in the trace), only the nodes after the cut and the composite run; the shown frames are within the codec's tolerance of a fresh compose
- **TEST** t3451 (`src/libs/kt-engine/tests/persistent_cache.rs`)

#### Scenario: The key changes exactly with its inputs and survives an unrelated update
- **WHEN** a stage key is computed, then each input is changed one at a time (source file, material position, decode size, a prefix node's value, a prefix curve, a mask), then a node after the cut, then the build stamp with the kernels unchanged, then one kernel's WGSL
- **THEN** each prefix input and the changed kernel change the key; the node after the cut and the build stamp do not
- **TEST** t3452 (`src/libs/kt-engine/tests/persistent_cache.rs`)

### Requirement: The store's three kinds share one budget and one value order
Layer results, frames and source copies SHALL live in one store in the project's cache folder under one byte budget,
evicted lowest value per byte first (measured cost saved per read, times reads, over bytes); a frame this process used
SHALL not be evicted; the coverage bars SHALL draw a moment bright only when its frame or every heavy layer's cut is
held.
Why: one store, not three (the owner's B1 with B2's folder); invariant 4.

#### Scenario: A cheap copy goes before an expensive stage
- **WHEN** the store is full with a layer result saving 40 ms per read and a source copy saving 3 ms per read of the same size, and a new entry arrives
- **THEN** the copy is evicted first; the bars over a moment whose heavy layer is held but whose frame is not are bright
- **TEST** t3453 (`src/libs/kt-engine/tests/persistent_cache.rs`)
