# kt-plugins-builtin

> The crate's contracts — `openspec/specs/plugins/spec.md` (the built-in set is closed; family
> crates are leaves); the reasons — `openspec/specs/plugins/design.md`.

The assembler: `BUILTIN_IDS` (the closed, ordered list the golden and parameterized tests iterate)
and `install()`, which registers every family crate under `src/plugins/`.

- **A normal dependency of kt-app and kt-cli ONLY** (t975). A core crate takes it as a
  DEV-dependency for its tests, never for its library — otherwise an edit of a plugin would
  recompile the core again (t977, `scripts/plugin-rebuild-check.py check`).
- **Every test that builds a plugin kind calls `kt_plugins_builtin::install()` itself**:
  `finish.sh` runs nextest one test per process, and a test that only passed because a neighbour
  installed the set goes red there (plugin-api-stage1 6.1, sixteen such tests).
- A new built-in: its family crate in `src/Cargo.toml`'s members and here, its id in
  `BUILTIN_IDS` and in its kind list's order (`VIDEO_ORDER`, `TransitionKind::all()`,
  `GeneratorKind::all()`'s `ORDER`), and its frozen form in t973/t1048.
