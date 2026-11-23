# Tasks: arm-build

> DESIGN ONLY until the orchestrator's go; Q1 decided (D5: Linux aarch64 first). Tests: t3980–t3982.

## 0. Notes

- [x] 0.1 note F-EXT-007
- [x] 0.2 proposal (the owner's two items and this design's reading), design (D1–D4), delta `distribution`; `openspec validate arm-build --strict`
- [x] 0.3 the owner's answer folded in (D5: Linux aarch64 first)

## 1. Build and CI

- [ ] 1.1 the aarch64 Dockerfile stage, `release-build.sh --target linux-aarch64`, the checks
- [ ] 1.2 the ARM CI job; the digests equal x86; t3980
- [ ] 1.3 the tarball test with the system FFmpeg; t3981; the crash report; t3982
- [ ] 1.4 the release artifact and the Flatpak arch list

## 2. Close

- [ ] 2.1 What's new; F-EXT-007 verdict; registry M4 moved
- [ ] 2.2 `src/scripts/finish.sh` (orchestrator), version in the report
