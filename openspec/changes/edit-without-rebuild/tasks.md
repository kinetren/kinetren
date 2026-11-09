# Tasks: edit-without-rebuild

## 1. Measure before
- [x] 1.1 Release `kt-cli probe` build lines on the relinked copy and the heavy showcase
- [x] 1.2 `kt-cli edit-cost`: the UI-thread time and the process CPU of one edit and of one slider step

## 2. Shareable renderers and one build per version
- [x] 2.1 `AudioRenderer` is `Sync`: `src_ends` a `OnceLock`, the loop region a thread-local keyed by the renderer
- [x] 2.2 `builds` module: the `engine-build` thread, one audio + one video build per version (or facts change), published as `Arc`s
- [x] 2.3 The render thread swaps to the published build (`sync_fx_since`), never builds; a pause's build holds the start
- [x] 2.4 Prefetch and the probe use the published build; the probe's FX states are synced off the UI thread; the loop head is pinned on publish
- [x] 2.5 The preview's renderer is the published one; a compose waits (off the UI) for the build of its state
- [x] 2.6 `set_project(Arc<Project>)`; the app hands `Model::doc_arc`; the live fader too

## 3. Per-event memo
- [x] 3.1 One resolve per reel instance per build (a build scope)
- [x] 3.2 Audio events and video layers memoized by the hash of their inputs; swept to the last build
- [x] 3.3 `kt_time::cost` rows: renderer builds, event derivations

## 4. Tests
- [x] 4.1 t2000 one shared build per version; t2001 zero builds on the calling thread; t2002 a region change builds nothing
- [x] 4.2 t2003 N edits of one event derive that event only; t2004 incremental = fresh (bytes, plans, scene hash)
- [x] 4.3 t2005 one resolve per reel per build; t2006 a frame is composed by its state's renderer
- [x] 4.4 t2008 the app hands the shared snapshot; t1875 updated (zero builds for region and meters)
- [ ] 4.5 Live scenario t2007: a slider dragged in the effects window while playing

## 5. Contracts, numbers, check
- [ ] 5.1 Spec deltas; design.md with the class walk; CLAUDE.md rules (kt-engine, kt-app)
- [ ] 5.2 Release numbers before/after; found-defects rows 110, 111, 117, 150, 155; What's new
- [ ] 5.3 `cargo test` of the touched crates, clippy, live `cached_range`, `idle`, `audiofx`, target-prune
