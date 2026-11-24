# Tasks: real-3d-groups

> DESIGN ONLY until the orchestrator's go (questions decided). Tests: t3952–t3956 (t3957–t3959 spare).

## 0. Notes

- [x] 0.1 note F-CMP-005 (reconnaissance; the bug-tracker dump R2–R3 needs network access — a separate task)
- [x] 0.2 proposal, design (D1–D6), deltas `video-plan`, `ui-reach`, `project-format`; planned OTIO row; `openspec validate real-3d-groups --strict`
- [x] 0.3 the owner's answers (kkt-26.11.20b) folded in

## 1. Model and format

- [ ] 1.1 `Track.three_d`, `Mask.extrude`, the member transform rows; format by content; t3956

## 2. Plan and GPU

- [ ] 2.1 `Layer::Group3d` in `frame_plan`; groups from the track order; t3953
- [ ] 2.2 the mesh pipeline with depth, planes; t3952
- [ ] 2.3 extruded masks (flatten, triangulate, caps, sides), content-cached; t3954

## 3. Interface

- [ ] 3.1 the track switch, the group bracket, the depth handle, "Split into depth layers…"; i18n ×5; t3955, live `parallax3d`
- [ ] 3.2 idle measured with a 3D group on screen

## 4. Close

- [ ] 4.1 What's new; F-CMP-005 verdict; registry R1–R4, E1 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
