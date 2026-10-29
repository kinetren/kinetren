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
- [x] 2.2 Create `src/libs/kt-plugins-builtin` (`BUILTIN_IDS`, `builtin()`, the install-once slot) and the empty-but-registered family crate `src/plugins/fx-video-color`; add both to `src/Cargo.toml` members. Write t975 in `src/apps/kt-cli/tests/plugin_graph.rs` (manifest reading, no cargo invocation). Verify t975 green
  - `src/libs/kt-plugins-builtin` (`BUILTIN_IDS` — empty until the pilot lands in 2.4, `builtin()`
    which panics unless the families give exactly that list, `install()`), the install-once slot in
    the API (`install`/`installed`), the family crate `src/plugins/fx-video-color` (depends on the
    API only, no plugins yet); kt-app's `run` and kt-cli's `main` install the set before any project
    is read. t975 (`apps/kt-cli/tests/plugin_graph.rs`, the manifests read as text) green; red proof:
    kt-fx given the assembler as a dependency → "core crate kt-fx depends on kt-plugins-builtin".
- [x] 2.3 `FxInstance::Plugin` and `FxInstance::Missing` with the hand-written serde of D3; write t973 in `src/libs/kt-model/tests/plugin_format.rs` with the frozen pre-move serializer copied verbatim, then t973b in `src/libs/kt-engine/tests/plugin_missing.rs`. Verify both green on a project with every kind and with a missing id
  - `FxInstance::Plugin(PluginSlot)` and `FxInstance::Missing(MissingSlot)` — both `#[serde(untagged)]`
    and LAST, so a `kind` no variant names is tried as a registered plugin, then kept raw (serde
    1.0.229 supports untagged variants in a tagged enum; no hand-written serde for the 46 others).
    `PluginSlot` writes `kind` and the passport rows in their file form — `Manifest.rows` /
    `RowDecl { store, omit_default }` / `Store::cast` (`fx_cast!`'s rule) — and reads them back;
    `MissingSlot` keeps the raw value (kt-model now asks serde_json for `preserve_order` itself —
    kt-otio turned it on for the whole build anyway; this also makes t822 green under `-p kt-model`).
    `param_specs`/`get`/`set`/`class`/`tag` answer from the installed registry. A missing slot drops
    out of the ACTIVE chain with its curves (`is_missing`), so the engine never sees it and every
    cache key is the chain's without it; `Project::missing_plugins()` names (id, event) once each,
    and kt-app warns per id on open (`msg.plugin_missing`, five catalogs) — the display name
    `fx.missing_plugin`. t973 (`kt-model/tests/plugin_format.rs`): the frozen-oracle loop (empty
    until 2.4) and the missing slot's bytes (red first: the raw keys came back sorted); t973b
    (`kt-engine/tests/plugin_missing.rs`): `[blur, wobble_x, glow]` renders as `[blur, glow]` (red
    first: the planner took the missing slot for a barrier and panicked) and the id is named.
    Revision of the design's "fails loudly in debug": the first workspace run had six kt-app tests
    time out — background workers (presets, sidecar, recovery) deserialize FX, and the "no registry"
    panic killed them silently. Before an install the registry is EMPTY (every plugin kind reads as
    `Missing`, written back unchanged), and every `KtApp` constructor installs the built-in set.
    Workspace: green except t815 (the recovery write took 5–20 s: the machine's I/O pressure was
    92 % — `/proc/pressure/io` — while another process filled the disk, 37 → 15 GB free).
- [x] 2.4 Move Posterize: CPU kernel and tap into `fx-video-color`, its WGSL tap into the same crate; `plan_chain` (`src/libs/kt-fx/src/plan.rs`) asks the manifest's class; `GpuFxEngine::new(ctx, &Registry)` assembles the tap dispatch of `geom_fused.wgsl` and the own pass (D5); kt-app/kt-cli inject `builtin()`. Write t974 in `src/libs/kt-gpu/tests/parity.rs`. Verify t974, t328, t329, t181 green and `GOLDEN_VIDEO` unchanged
  - Posterize is `src/plugins/fx-video-color` (passport `levels` 2..32 default 6, `U32` row,
    `PointWise`, ONE WGSL text `posterize.wgsl` with `fn tap_posterize(v, a, b)`); the variant, its
    specs and `default_levels` left kt-model, `FxInstance::posterize()`/`plugin(id)`/`plugin_with`
    build slots. `VideoKernel` became `tap(&self, p: &[f64], rgba)` + `gpu_params(p) -> [f32; 7]`.
    kt-fx: `TapKind::Plugin(PluginTap)` (a `Copy` tap holding the kernel and up to 8 values) took
    the place of `Poster`; `is_pointwise`/the barrier arm ask the manifest's class.
    kt-gpu: `Pipe::Posterize` and `shaders/posterize.wgsl` are gone; `assembled_shaders()` pastes
    one dispatch arm per point-wise plugin before the `// @PLUGIN_TAP_ARMS` marker of
    `geom_fused.wgsl` (kind `6 + i`, `< 16`) and generates its own pass; the engine keeps
    `plugin_pipes`. Deviation from the task text: the engine reads the INSTALLED registry
    (`kt_plugin_api::installed()`) instead of taking `&Registry` in `new` — the registry is one per
    process (2.2), and a second source of the list would be a place to disagree. kt-app installs
    in every `KtApp` constructor and in `fx_registry::registry()` (the picker can be built before a
    KtApp: tests, the thumbnail worker); kt-cli in `main`. Tests that build `FxInstance::all()`
    install the set first (dev-dependency `kt-plugins-builtin`; t975 still green).
    t974 (`kt-gpu/tests/parity.rs`): alone / inside a geometry series (one run) / as a barrier
    between blurs, CPU vs GPU within `FrameTol::THRESHOLD`, the assembled text is pure and lists
    `posterize` once. t973 got its frozen `Frozen::Posterize { levels }` with 6/4/32 and
    `t973_a_moved_kind_reads_the_old_defaults` (a file without `levels` reads as 6).
    Red proofs: default 6 → 5 turned `t973_moving_a_kind_does_not_change_a_byte` and
    `..._reads_the_old_defaults` red (the second was green at first — it compared with
    `posterize()`, the same moved default; now against the frozen form); dropping the arms from
    the fused text turned t974 and t329 red. t328 goldens, t329, t181 green; no `GOLDEN_*` moved.
    Workspace (nextest, 1229 tests): green except t815 (the recovery write took 30 s at an I/O
    pressure of 93 %) and t663 (its 60 s wait loop under load) — both green alone (0.06 s, 0.03 s).
- [x] 2.5 `src/apps/kt-app/src/fx_registry.rs` builds Posterize's entry from its manifest; the test door `register_for_test`; write t976 in `src/apps/kt-app/src/reach_tests.rs` with the PATH of the spec. Verify t976, t48, t125, t100 green and `src/scripts/specs-user-path.py --check` (t799) accepts the PATH
  - `src/apps/kt-app/src/plugins.rs` is the app's one install entry (every `KtApp` constructor and
    `fx_registry::registry()` call it): the built-in set, plus `test_tint` through the test door in a
    test build (`kt_plugins_builtin::install_with`). `Registry::register_for_test` runs the same
    refusals and marks the id (`is_test_only`); `Registry::plugins()` iterates in order.
    `fx_registry::registry()` appends one row per installed video plugin after the core templates:
    name, category and keywords from the manifest (`i18n::key_by_name`, a linear search once per
    plugin); Posterize left `video_fx_templates` and `video_meta`. Its curated variants (Hard /
    Comic / Soft) stay in kt-app's `video_variants`, keyed by the manifest's name: they are picker
    curation with translated names, not plugin data (stage 4 decides per family). Posterize's row
    is last in the colour category, so the gallery order did not move.
    t125 counts a catalog key a manifest names as used (`plugins::manifest_keys`); t48/t543 count
    the plugin rows (the test plugin has no variants and is skipped by t543's curation check).
    t976 (`reach_tests.rs`): the closed list and `FxInstance::all()` do not hold `test_tint`, the
    registry's built-ins equal `BUILTIN_IDS`, every manifest key exists; the harness path of the
    spec — FX window → "+" → "tint" (Blur is filtered out) → "Test Tint" → the last chain node is
    the plugin at its default `amount` 0.5, the parameter row "Amount" is drawn. Red proof: without
    the plugin rows t976 ("the picker has the test plugin's row") and t48 are red. t799 accepts the
    PATH; t100, t125, t547, t254 green.
    The workspace run turned t804 red (82 untouched places over the limit 81): the coverage guard
    reads every quoted dotted string in kt-app's sources as a place, and the test plugin's
    `"fxreg.cat.color"` (then a comment's own example) was one. The test plugin names the key by
    its constant (`i18n::key_name`); the rule is in kt-app's CLAUDE.md; the limit did not move.
    Workspace: green except t476 (load; green alone); kt-app + plugin crates + kt-cli 453/453.
- [x] 2.6 Speed check of the pilot on release: `cargo build --release -p kt-cli` and `kt-cli probe <fixture> 0 10 --time` before/after on a chain with Posterize; record here; over 3 % slower blocks the stage (design, Risks)
  - Measured the CPU path, not `kt-cli probe --time`: the probe times the AUDIO render, and this
    stage moves a video kernel only (no audio path changed; that measurement belongs to stage 3).
    A scratch release test (`apply_video_chain`, 1920×1080, median of 15) on the pre-move tree
    f21b4a1 (`FxInstance::Posterize { levels: 5 }`) and on this one (the plugin slot); the plans
    are the same (`Bar0` / `Run0..3` / `Bar0, Bar1, Bar2`):
    | chain | before | after 2.4 | after the fix |
    |---|---|---|---|
    | Posterize alone (barrier) | 22.2–22.6 ms | 36.4 ms (+63 %) | 22.3 ms (≈0 %) |
    | Mirror, Posterize, Mirror (fused run) | 78.3 ms | 88.6 ms (+13 %) | 77.4–77.8 ms (−0.9 %) |
    | Blur 2, Posterize, Blur 2 (barrier between) | 43.7 ms | 57.9 ms (+33 %) | 43.9 ms (+0.4 %) |
    2.4 blocked the stage (design, Risks): a dynamic `tap` per pixel that re-derived the rounded
    `levels` each call. The fix is the design's own rule — the trait call per frame, the inner loop
    monomorphic inside the plugin: `VideoKernel::cook` prepares the parameters once per plan
    (`PluginTap` keeps them), `VideoKernel::frame` is the barrier position as ONE dynamic call
    (its provided body is compiled per kernel, so `tap` inlines), and `kt_plugin_api::frame_with`
    is the one text of that loop (the bytes of kt-fx's `pointwise_barrier`). Posterize overrides
    `frame` to take `n` through its `u32` range once per frame: the same value, and the compiler
    again knows `n - 1 >= 1`, as the variant's `u32` field told it — without that the barrier loop
    stayed 6 % slower; doing the same cast per sample in the fused pass cost 8 %, so `tap` reads
    the cooked value as is. t328 goldens, t974, t329, t181, kt-fx/kt-gpu/kt-model/kt-engine/plugin
    crates 612/612 green; kt-app + kt-cli 447/452 under an I/O pressure of 56–82 % (t815, t476,
    t411, t952, t951 — all five green alone).
- [x] 2.7 Commit stage 2 (`cargo test --workspace`, `cargo clippy --all-targets` with no new warnings)
  - Stage 2 is in f2b13cb (2.4), e99f487 (2.5), dc8b4b4 (2.6). `cargo test --workspace` (one process
    per test binary, so the registry install order is exercised): 1226 passed, 4 failed in kt-app —
    t802, t411, t640, t795 — at an I/O pressure up to 82 % (kt-app took 98 s); the four together
    and then the whole kt-app binary again under `cargo test` green, 416/416 in 16 s. clippy: no new
    warnings. Before stage 3 the audio contract was revised in design.md (D2a): the D2 sketch did
    not carry the per-chunk curve targets, the LFO's copied curve, by-position curve reads, the
    sidechain key or the pitch engine option the node chain gives a node today.

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
