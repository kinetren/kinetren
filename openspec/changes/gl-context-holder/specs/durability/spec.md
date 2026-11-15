## ADDED Requirements

### Requirement: On GL only the UI thread calls into the window's device
On the GL backend no thread of the program but the UI thread SHALL call into the window's (eframe's)
device: the preview SHALL compose on a headless GL device of its own (one per process, a sibling engine
per project) and show its pictures by the bytes path, and the bench's stand-ins SHALL leave the
window's device alone. A GL call of ours stalled for longer than wgpu-hal's six-second context-lock
timeout SHALL cost preview frames, never the program. Every wgpu call of ours that can hold a device
(pipeline creation, submission, blocking wait) SHALL be recorded by thread, role and operation; a call
on the window's device lasting past one second SHALL be written to the journal, and the editor's panic
report SHALL list the calls in flight.
Why: kkt-26.11.15 N1 class, found-defects 651 — three live scenarios died at one second in egui's paint
on "Could not lock adapter context": a thread of ours held the window's GL context, stalled by the
system, past six seconds.

#### Scenario: A call in flight names its thread and operation
- **WHEN** a thread named `t2740-holder` is inside a registered call and then returns
- **THEN** the list of calls in flight holds "headless device: thread t2740-holder in t2740 wait for … ms" while it runs and nothing of it after
- **TEST** t2740 (`src/libs/kt-gpu/src/holds.rs`)

#### Scenario: A stalled GL device holds no other device
- **WHEN** one GL device's context lock is held by another thread and a second GL device of the process creates a buffer, writes it, submits and waits
- **THEN** the second device's work completes while the first is held (with both on ONE device the waiter panics "Could not lock adapter context" after six seconds)
- **TEST** t2741 (`src/libs/kt-gpu/tests/gl_context_holder.rs`)

#### Scenario: Only a GL window device is left to the UI
- **WHEN** the window's adapter is GL, Vulkan, DX12 or Metal
- **THEN** it is left to the UI thread alone exactly on GL
- **TEST** t2742 (`src/libs/kt-gpu/tests/gl_context_holder.rs`)

#### Scenario: The preview's device stalled past six seconds
- **PATH** open the demo project → the preview shows its first frame → (the bench holds the preview's GL device for nine seconds) → click the ruler again and again: the cursor moves, the window keeps drawing, nothing closes → after the release click the ruler: the preview shows the clicked moment
- **TEST** t2743 live scenario `src/scripts/live/gl_stall.py`
