## ADDED Requirements

### Requirement: A headless render needs no display and opens its devices from one instance
Every headless device (export engines, the offline engine, kt-cli), the compute host and the
adapter list SHALL come from one `wgpu::Instance` per backend set, created at the first ask under one
process-wide opening lock and never dropped; devices SHALL be opened one at a time; the adapter list
SHALL be taken once per process, and in the editor before eframe opens its own instance. An export
run with no `DISPLAY` and no `WAYLAND_DISPLAY` SHALL finish.
Why: found-defects 131 — `env -u DISPLAY kt-cli export … libx264` died by SIGSEGV every run:
`export_engines` opened two to four devices on threads at once on a software adapter, each creating
its own instance, and the Vulkan loader's instance-level calls raced (`vkEnumerateInstanceExtension
Properties` beside `vkCreateInstance`, gdb). The load watch re-created a whole instance — every ICD
loaded, every driver's configuration parsed — each five seconds of a hot spell, beside the preview.

#### Scenario: Export with no display, four devices
- **WHEN** kt-cli exports a short project to libx264 three times with `DISPLAY` and `WAYLAND_DISPLAY` removed and `KT_VIDEO_ENGINES=4`
- **THEN** each run exits 0 and writes a file (before: exit 139 at the first run)
- **TEST** t2086 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Devices opened at once share the one instance
- **WHEN** four threads open an export device at once, three times, and the adapter list is asked twice
- **THEN** every device opens and the process created exactly one instance
- **TEST** t2087 (`src/libs/kt-gpu/tests/one_instance.rs`)
