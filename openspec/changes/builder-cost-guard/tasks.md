# Tasks: builder-cost-guard

## 1. Row 151
- [x] 1.1 Release `kt-cli probe` on the relinked copy, before: `AudioRenderer::new` 388–412 ms
- [x] 1.2 `sign_knots` and `span_over` lay the map out once (`Integral::over`), bit for bit (t2060)
- [x] 1.3 `kt_time::cost`, the thread-local walk meter; `warp::segments` counts (t2061)

## 2. The class: every builder and per-build pass
- [x] 2.1 Survey of every builder and per-build pass (design.md, "The class walked")
- [x] 2.2 `curve_value` by halving (the profile sampling, link resolution, decode zoom probes)
- [x] 2.3 `Reel::clip` + `Indexed`: transition ends in both builds, validation, the timeline's draw and hit tests
- [x] 2.4 `PieceEdges`: a transition's engine events by (original, edge) in both builds
- [x] 2.5 Reel revisions once per build (`revisions_scope`), a nested reel by its revision
- [x] 2.6 Links: leaders first, each found once per curve (`resolve_derived`)
- [x] 2.7 `ZoneIndex`: the cut pass's forbidden zones laid out once
- [x] 2.8 `reconcile_reel` by id; the app's per-revision caches (preview keys, names, overlaps, media durations)

## 3. The counting guard
- [x] 3.1 `tests/build_cost.rs`: six generated axes ×1/×4/×16, six passes, walked elements and bytes (t2062–t2067)
- [x] 3.2 Red on the former code (every axis), green on the new
- [x] 3.3 t1935's scene guard: the builder thread's test hold instead of a quadratic build

## 4. Contracts and numbers
- [x] 4.1 Spec deltas: time-model, audio-render, video-plan, compositions, morpher, automation, project-format
- [x] 4.2 `kt-cli probe` prints the walk counts of each build
- [x] 4.3 Release numbers before/after on the relinked copy; found-defects row 151; CLAUDE.md rules
- [x] 4.4 What's new line

## 5. Check
- [x] 5.1 `cargo test` of the touched crates, clippy, target-prune
