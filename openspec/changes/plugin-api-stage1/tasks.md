# Tasks

Test numbers t972–t977 are reserved by this change. Every stage ends green and is committed at
once (English message, first line = the stage and what was done).

## 0. Records before code

- [x] 0.1 The owner's words (item 7 of `kkt.txt`, the answer "an internal API + crates", ktprompts P1–P6) are recorded in `docs/user-feedback.md`, round "The owner's list of 2026-10-27 and ktprompts v26.10.18"; this proposal recorded
- [x] 0.2 F-EXT-003 (`docs/features/plugins/hosts/plugin-abi-and-sandbox.md`) carries the dated section "Revised by the owner, 2026-10-27" (order wasm before dlopen/OFX, refusal split by media). Verify `python3 src/scripts/features-index.py --check` and `openspec validate plugin-api-stage1 --strict` are green, then commit stage 0

## 1. Baseline measurement (before any code moves)

- [x] 1.1 On a warm tree of this commit, on the dev profile, measure and record here: (a) touch `tap_poster` in `src/libs/kt-fx/src/geom.rs` → `cargo build -p kt-app -p kt-cli` wall time and the list of recompiled workspace crates (`cargo build -v` "Compiling" lines); (b) the same after touching the `Posterize` variant in `src/libs/kt-model/src/project.rs`; (c) `cargo test --workspace --no-run` after (b); the machine's core count and the commit. Put the R.68 numbers (61 s / 43 s / 10 s) next to them. The script `src/scripts/plugin-rebuild-check.py` is written here in its measuring form (it becomes t977 in stage 6)
  - Commit cb338e6, 32 cores, dev profile, warm tree, mold. `src/scripts/plugin-rebuild-check.py
    measure <file> [-- cargo args]` makes a REAL one-line edit (a private const appended and removed
    after; a bare touch is nearly free under rustc's incremental cache, 3.8 s):
    | edit | `build -p kt-app -p kt-cli` | `test --workspace --no-run` | recompiled workspace crates |
    |---|---|---|---|
    | `kt-fx/src/geom.rs` (where `tap_poster` lives) | 2.0 s | 79.3 s | kt-fx, kt-engine, kt-gpu, kt-app, kt-cli |
    | `kt-model/src/project.rs` (where `Posterize` lives) | 3.7 s | 169.7 s | + kt-model, kt-cache, kt-media, kt-otio |
    R.68 (2026-09-27): 61 s / 43 s / 10 s. **The finding that matters for this change:** the binaries
    themselves rebuild in seconds (incremental + mold); the cost is the TEST binaries — 80–170 s,
    most of it the integration test crates of every dependent. A plugin crate depended on by kt-app
    will still rebuild kt-app's test binary; what the move can save is the core crates' test
    binaries (kt-model's, kt-media's, kt-otio's, kt-cache's) and their cascade — t977 must measure
    `test --no-run`, not `build`, or it would report a win that is not there.
- [x] 1.2 Record the line counts of kt-model (19,252 / `project.rs` 9,741), kt-fx, kt-gpu, kt-engine, kt-app at this commit next to the timings, so stage 6 can report what moved out
  - At cb338e6 (Rust lines under `src/`): kt-model 19,336 (`project.rs` 9,735, `genmedia.rs` 1,857),
    kt-fx 8,828, kt-gpu 2,564 + 1,846 WGSL, kt-engine 10,920, kt-app 69,300.

## 2. API, registry, assembler, injection, the pilot (Posterize)

- [x] 2.1 Create `src/libs/kt-plugin-api` (deps: kt-time, serde): move the passport types and macros from `project.rs:499–6756`, re-export them from kt-model; add `Manifest`, `Decl`, `ParamValues`, the kernel traits and `Registry` with the D4 refusals. Write t972 in `src/libs/kt-plugin-api/tests/registry.rs` first and see it red, then green. t641, t190, t643 stay green
  - `src/libs/kt-plugin-api` (deps: kt-time, serde): the passport moved verbatim from `project.rs`
    (FlagSpec, ParamScope, HasParams with its default value functions, text/choice specs,
    ParamKind/Unit/Type/Value, ColorGroup, ParamName, ParamSpec; 737 lines) together with `MediaId`
    (`ParamValue::Media` needs it); the four passport macros are `#[macro_export]` and name
    everything through `$crate`. kt-model `pub use`s every name, so no caller changed. New: `Manifest`,
    `Decl` (state, tail/look-ahead as `fn(&ParamValues) -> i64`, threads, video class, GPU side,
    built-in), `Media`, `VideoClass`, `Gpu`, `ParamValues`, `Plugin`/`VideoKernel`, `Registry` with the
    D4 refusals, `tail_frames`/`lookahead_frames` (the one home of each formula), and
    `MIN_PREROLL_FRAMES` (kt-fx re-exports it — one home). t972 written first and red ("unresolved
    imports"), then green. t641, t190, t643, t822 green (t822 is red only under `cargo test -p
    kt-model` alone: `preserve_order` of serde_json is unified in from kt-otio — before this change
    too). The disk filled during the run (310 of 325 GB): `target/win-container` and
    `target/debug/incremental` of this worktree were removed (regenerable caches).
- [ ] 2.2 Create `src/libs/kt-plugins-builtin` (`BUILTIN_IDS`, `builtin()`, the install-once slot) and the empty-but-registered family crate `src/plugins/fx-video-color`; add both to `src/Cargo.toml` members. Write t975 in `src/apps/kt-cli/tests/plugin_graph.rs` (manifest reading, no cargo invocation). Verify t975 green
- [ ] 2.3 `FxInstance::Plugin` and `FxInstance::Missing` with the hand-written serde of D3; write t973 in `src/libs/kt-model/tests/plugin_format.rs` with the frozen pre-move serializer copied verbatim, then t973b in `src/libs/kt-engine/tests/plugin_missing.rs`. Verify both green on a project with every kind and with a missing id
- [ ] 2.4 Move Posterize: CPU kernel and tap into `fx-video-color`, its WGSL tap into the same crate; `plan_chain` (`src/libs/kt-fx/src/plan.rs`) asks the manifest's class; `GpuFxEngine::new(ctx, &Registry)` assembles the tap dispatch of `geom_fused.wgsl` and the own pass (D5); kt-app/kt-cli inject `builtin()`. Write t974 in `src/libs/kt-gpu/tests/parity.rs`. Verify t974, t328, t329, t181 green and `GOLDEN_VIDEO` unchanged
- [ ] 2.5 `src/apps/kt-app/src/fx_registry.rs` builds Posterize's entry from its manifest; the test door `register_for_test`; write t976 in `src/apps/kt-app/src/reach_tests.rs` with the PATH of the spec. Verify t976, t48, t125, t100 green and `src/scripts/specs-user-path.py --check` (t799) accepts the PATH
- [ ] 2.6 Speed check of the pilot on release: `cargo build --release -p kt-cli` and `kt-cli probe <fixture> 0 10 --time` before/after on a chain with Posterize; record here; over 3 % slower blocks the stage (design, Risks)
- [ ] 2.7 Commit stage 2 (`cargo test --workspace`, `cargo clippy --all-targets` with no new warnings)

## 3. Audio FX

- [ ] 3.1 Move the 17 audio plugin kinds into `fx-audio-dynamics`, `fx-audio-tone`, `fx-audio-modulation`; `ClipProcessor` holds plugin state in a host slot (D2), `chain_tail`/`chain_lookahead` read the manifests. Verify t322 (all items), `GOLDEN_AUDIO`, t337, t338 green; t973 extended to all audio kinds
- [ ] 3.2 Rewrite the "add an audio effect" checklist in `src/libs/kt-fx/CLAUDE.md` for the plugin form; commit stage 3

## 4. Video FX

- [ ] 4.1 Move the remaining 24 video plugin kinds into `fx-video-color`, `fx-video-stylize`, `fx-video-distort`, `fx-video-blur`: geo kinds through `geo()` primitives (no WGSL), point-wise through tap pieces, barriers through passes. Verify t328, t329, t181, t297, t330, t332, t333, t198, t974 green; t973 extended
- [ ] 4.2 FX picker, variants and thumbnails from manifests (`fx_registry.rs`, `fx_thumbs.rs`); verify t48, t252, t253, t976 and `layout_tests.rs`/`reach_tests.rs` green; rewrite the "add a video effect" checklist in `src/libs/kt-fx/CLAUDE.md` and `src/libs/kt-gpu/CLAUDE.md`; commit stage 4

## 5. Transitions

- [ ] 5.1 Move the 33 kinds into `transitions-basic`, `transitions-styled`, `transitions-volume`; the composite's `trans` dispatch assembled from the registry; `put_mix` hashes the id. Verify t223, t224, t459, t468, t222 green and the transition window form (t780); t973 extended; commit stage 5

## 6. Generators

- [ ] 6.1 Move the 14 non-title kinds into `generators-basic` and `generators-test`; the factory (`src/libs/kt-media/src/factory.rs`) asks the registry and raises a named placeholder for a missing id. Verify t469, t470, t471b, t504, t518, t560 and t973b (`src/libs/kt-media/src/factory.rs`) green
- [ ] 6.2 The title into `generators-title` (kt-text dependency declared in t975's closed list) — or, if `text_anim_curves`/`migrate_gen_to_events` must stay in kt-model, keep `text` a core kind and record why here. Verify the title golden and t973 green; commit stage 6

## 7. Acceptance and cleanup

- [ ] 7.1 Delete the enum variants, `tag()` arms and template tables of the moved kinds; the core keeps `geometry`, `cam_motion`, `layer_mix`, `slowed_reverb`. Verify `cargo test --workspace` green
- [ ] 7.2 Turn `src/scripts/plugin-rebuild-check.py` into t977: touch a plugin kernel, assert the recompiled set is {family, kt-plugins-builtin, kt-app, kt-cli, dependent test targets}, print the times next to task 1.1 and R.68; wire it into `finish.sh` as an optional stage (`--rebuild-check`). Verify t977 green and record the numbers here
- [ ] 7.3 Update `docs/architecture.md` (the crate map) and the root `CLAUDE.md` workspace map with `kt-plugin-api`, `kt-plugins-builtin` and `src/plugins/`; commit stage 7

## 8. Finish

- [ ] 8.1 Run `./scripts/finish.sh` from `src/`; record the version `vYY.MM.DD-hash` and the t977 numbers in the report
- [ ] 8.2 After archiving: F-EXT-003 verdict line names `CAPABILITY: openspec/specs/plugins/`; append the "why" (D3 ids-as-tags, D4 the media split, D5 closed geometry, D10 the order) to `openspec/specs/plugins/design.md` and the automation design's "Deferred → Plugins" line
