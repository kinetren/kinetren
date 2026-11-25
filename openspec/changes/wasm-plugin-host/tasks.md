# Tasks: wasm-plugin-host

> IMPLEMENTATION-READY (revision 2, 2026-11-25, orch `fc60a5d9`). Tests: t3957–t3963, t4151, t4152. Areas: new `kt-plugin-wasm`, `kt-plugin-sdk`; kt-plugin-api (`Decl`'s two fields), the two mains (the trial), kt-cli, SHELL (Preferences), FX-UI (a mark). It shares no hot file with anybody; `Decl`'s type change touches the audio plugins' declarations (mechanical, one commit).
> The network is needed once (wasmtime's crates). Read: design D1–D6, `kt-plugin-api/CLAUDE.md`, `openspec show plugins --type spec`. Commit per section; target-prune after each.

## 0. Notes

- [x] 0.1 F-EXT-003 refreshed (since 2026-11-20)
- [x] 0.2 proposal, design (D1–D8, class walk), deltas `plugins`, `ui-reach`, `project-format`; `openspec validate wasm-plugin-host --strict`
- [x] 0.3 the owner's answers (kkt-26.11.20b) folded in

## 1. Host

- [ ] 1.1 `Frames` in `Decl` (no behaviour moves: t322f, t322g); commit alone
- [ ] 1.2 `kt-plugin-wasm`: the package, the manifest through `Registry::register`, the runtime's configuration, the import surface; t3961, t3963 (refusals)
- [ ] 1.3 audio, transition, generator, `v_cook`; the WGSL through the engine's own door; t3957, t2684 over a wasm slot
- [ ] 1.4 fuel, traps, the faulted kind, the export's refusal; t3958, t3959, t4151; `cargo test -p kt-plugin-wasm -p kt-engine`; commit

## 2. SDK and trial

- [ ] 2.1 `kt-plugin-sdk`, `template-wasm`, CI's wasm32 build; `kt-cli plugin check|test|pack`; t3962
- [ ] 2.2 `--plugin-trial`; t4152; commit

## 3. Interface

- [ ] 3.1 Preferences › "Add a plugin…", the restart line, the marks, the notices (Library rows with `download-library` 5.2); i18n ×5; reach, layout, font tests; t3960, live `wasm_plugin` (OUR template package; a screenshot per step)
- [ ] 3.2 the idle check with a plugin installed and used on the heavy showcase

## 4. Close

- [ ] 4.1 the size of the dist binary before and after, into design D9; dist-guard; What's new; the guide's "Plugins" page (what a package is, the SDK); F-EXT-003 verdict; registry P1–P5 (P6 to its own design); the root `CLAUDE.md` workspace map (the orchestrator's text); commit
