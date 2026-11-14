# Tasks: chain-everywhere

## 1. The stream stage's inputs run their chains (kt-engine)

- [x] 1.1 `chain_plan_with` (masks); `ClipMaskPlan` answers the mask ops of a set of slots remapped to a sub-chain
- [x] 1.2 The front run gets its slots' masks and camera (found-defects 541)
- [x] 1.3 The second input runs its event's picture slots; `ident` names that chain (found-defects 540)
- [x] 1.4 t2550–t2553

## 2. Effect cells render the real frame (kt-app)

- [x] 2.1 `FxThumbBase::Scene`: the player's project, the anchor, the moment; the cell job appends the candidate, solos the anchor's track, composes through `frame_plan` on the offline device with the service's own sources, cache, stream memo and build memo
- [x] 2.2 Loading in the cell until complete; no sample for an event; decode only after the guard; cancellation by generation reaches a stream pre-roll
- [x] 2.3 The effects window passes the scene; the morpher passes its target's scene or no pictures
- [x] 2.4 t2554–t2559 (t546/t544e kept for the sample path)

## 3. Class walk, load, records

- [x] 3.1 Every place an effect's picture or sound is built outside its chain, every base/still stand-in, the audio pairs — the list in `design.md`
- [x] 3.2 Processor load with the effects window open on a heavy project, before and after
- [x] 3.3 found-defects 460, 540, 541 out; 580+ in; What's new line; specs
- [x] 3.4 nextest of kt-engine, kt-gpu, kt-app; clippy; commit, target-prune; archive
