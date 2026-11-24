# Tasks: real-3d-groups

> IMPLEMENTATION-READY (revision 2, 2026-11-25, orch `fc60a5d9`). Tests: t3952–t3956, t4148–t4150. Areas: MODEL (**v+**), `video.rs`, `video_plan.rs`, GPU (new `mesh.rs`, `engine.rs` one job), GEOM (`pan3d.rs`: the matrix), PREVIEW-UI (the handle), TIMELINE-UI (headers). After `shared-events`; not beside
> `track-stream-stage` 1.2–2.2, `scopes` (`engine.rs`). Read: design D1–D5. Commit per section; target-prune after each.

## 0. Notes

- [x] 0.1 note F-CMP-005 (the bug-tracker dump R2–R3 needs network access — a separate task)
- [x] 0.2 proposal, design (D1–D9, class walk D8), deltas; `--strict`
- [x] 0.3 the owner's answers (kkt-26.11.20b) folded in

## 1. Model

- [ ] 1.1 `Track.three_d`, `Mask.extrude`, the format, OTIO; t3956; commit

## 2. The plan

- [ ] 2.1 `pan3d::matrix`, `pan3d_forward` from it; t298, t187 green; commit alone
- [ ] 2.2 `VideoGroup3d`, `PlanLayer::Group`, members, places, `M` and the order; `moment_key` and every reader of D5; t3953, t4149; GOLDEN_VIDEO
- [ ] 2.3 `cargo test -p kt-engine`; commit

## 3. The pass

- [ ] 3.1 `mesh.rs`: planes, mips, MSAA, depth, the two draws; prewarm, `Slot`, the arbiter; t3952, t4148
- [ ] 3.2 extruded masks; t3954; `FrameTol::MESH` measured; `cargo test -p kt-gpu`; commit

## 4. Interface

- [ ] 4.1 the header's switch and bracket, the track window's lines, the mask row; i18n ×5; reach, layout, font tests
- [ ] 4.2 the depth handle, hit-testing, "Split into depth layers…"; t3955, live `parallax3d` (OUR fixture: a generated picture with two masks; a screenshot per step)
- [ ] 4.3 the idle check with a group on the heavy showcase; t4150

## 5. Close

- [ ] 5.1 What's new ("3D tracks: layers cross and stand behind each other; a photo cut by its masks becomes a parallax in one step"); the guide's page; F-CMP-005 verdict; registry R1–R4, E1; kt-gpu `CLAUDE.md` (`mesh.rs` is the one place with a sampler and unorm targets besides the display); commit
