# Tasks: segv-crashes

- [x] 1.1 `kt-crash`: the async-signal-safe handler (raw report: signal, code, address, thread, pc, stack or stack scan, threads, mappings; re-raise; watchdog) and the symbolizer (line tables, symbol tables, separate debug files) — t2080, t2081, t2083
- [x] 1.2 kt-app: the handler in the editor's process; the supervisor symbolizes into `kt-app-crash.txt` with the summary and the run's telemetry-journal lines (by pid); the window shows them; the next start absorbs a raw report no supervisor took; the journal's crash record carries the summary — t2084, t2085
- [x] 1.3 kt-cli: the handler starts `kt-cli crash-report`, the report on stderr; `crash-probe` — t2082
- [x] 2.1 Row 131 reproduced under gdb (vkEnumerateInstanceExtensionProperties beside vkCreateInstance on export_engines' threads); one instance per backend set, one opening at a time, the adapter list cached and taken before eframe — t2086, t2087
- [x] 2.2 Row 132 (t985 SIGSEGV under nextest): the handler in the test process named it — the compute host opened on a media worker beside the offline engine, the same loader race (33 of 60 on the old code); closed by 2.1; t2087 opens the compute host and the list beside the devices
- [x] 2.3 The owner's SIGSEGVs (fails/26.11.08): what the files say, which candidates the walk closes, what stays open (design D5)
- [x] 3.1 Proof: three full `cargo nextest run --workspace` without DISPLAY (no SIGSEGV; t985 green in each); t985 stress 40/40 under a full run; live `fault`, `crash`, `idle` green
- [x] 3.2 design.md: the mechanism and the walk of every native path; found-defects rows 131/132 moved; registry H4/H5; What's new line
- [x] 3.3 Archive: specs merged
