## ADDED Requirements

### Requirement: The preview's executor on GL is off the window's device
Where the window's device is the UI thread's alone (GL), the preview's executor SHALL be an engine on a
headless GL device of the process (`preview_engine_off_display`: the device and its pipelines made
once, a `sibling` engine per project) and the preview SHALL take the bytes path; the geometry dump SHALL
say so (`compose.ui_only`). Elsewhere the display path stays.
Why: on GL every wgpu call holds the adapter's one context lock, and the display engine's compose,
build and warm work on the window's device made egui's paint wait for it (gl-context-holder).

#### Scenario: A kept moment is read back, not composed
- **WHEN** the preview's engine off the window's device composes a moment, then is asked the same moment again
- **THEN** the first is kept (the coverage bars see it), the second is a replay with nothing composed, and both carry the same bytes
- **TEST** t2746 (`src/libs/kt-gpu/tests/gl_context_holder.rs`)

#### Scenario: Video FX on GL show their bytes
- **PATH** open two flat light-gray clips, the second with Edges → click the first: the preview is light → click the second: the preview is black → the dump says the preview composed off the window's device on GL (on another backend: the picture stays on the device)
- **TEST** t2744 live scenario `src/scripts/live/gpufx.py`
