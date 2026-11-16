## Context

The pass timer (`pass_time.rs`) counts a frame by the role its caller names: `present` and
`present_kept` say `Preview`, the readback path (`Inner::execute`) said `Offline`, the export ring
`Export`. Before gl-context-holder only the window's device presented, so "preview = presents on the
window's device" held. gl-context-holder moved the GL preview to its own headless device and the bytes
path (`keep_composites_for_bytes`), and every preview frame there became an offline one.

## Decisions

### D1. The role of a read-back frame is the engine's, not the path's

`Inner::readback_role`: `Preview` when the engine keeps the preview's bytes-path composites
(`keeps_bytes`, the engine off the display) or owns the window's surface (its only read-back caller is
the preview under `KT_PREVIEW_READBACK`); else `Offline`. The offline engine and its side engine never
set either, so the slow route, thumbnails, strips and the loop fill stay offline, as t2020 holds.
Rejected: a role argument on `execute_into` — the `VideoGpuExec` trait is the engine's door for every
caller, and the player cannot know which device it holds.

### D2. The device's role is named once, where it opens

`GpuContext::headless_with` takes the role; `new_headless_for_preview` passes `preview`, so the trace
describes the device as `gpu.preview.*` once (it was `gpu.headless.*`) and `errors::watch` counts its
errors under `preview`. `GpuFxEngine::role()` and `Inner::role()` name the preview for the holds and
the panic report's calls in flight.

### D3. The preview's ring fronts the person's frame

`serve_preview` on the sibling: a software ring then bounds and cuts lower classes in front of the
preview's frame, as the slow route's does (scheduler-followups D6). `gpu_admit.c` = 2 says so in the
trace; `trace gpu` printed only "(carries the display)", and a GL run showed no ring serving the person.

### D4. An engine is made and released at its device's gate

Found while proving D1 (`cargo test` runs a binary's tests as threads): t2747's sibling, released when
the next test's call replaced it, destroyed its textures past the gate while t2745 held the device for
seven seconds, and that thread panicked "Could not lock adapter context"; under load t2746's new sibling
did the same creating its textures. `sibling()` takes the gate while it assembles, and `Drop for
GpuFxEngine` drops the frame state (`inner`, now a `ManuallyDrop`) inside the gate. No thread drops the
last reference of an engine from inside that device's gate: every gated call runs on a reference its
caller keeps, and the memo's shrink hook takes no gate. Closes found-defects 683 for every engine,
`app-drop`'s included. t2748 holds it.

## The walk (CLAUDE.md: a problem is a class)

The mechanism: something counted or named "the preview" by "the window's device" or by "the present
path". Checked:

- **GPU record roles** (`pass_time` `Role`, the journal's `GPU` line, Diagnostics' "Video card that
  second", `kt-cli preview`'s record): fixed (D1). `read_kept` (a replayed moment on the bytes path) is
  a copy, not a frame: it runs no pass and is not counted, as the display path's replay counts only its
  display pass.
- **Device role in errors, trace and holds** (`errors::role`, `trace_device`, `holds`): fixed (D2);
  `read_kept` and every submit/wait hold take the engine's role instead of `surface ? display : headless`.
- **Ring role** (`serve_preview`, `gpu_admit`, `trace gpu`): fixed (D3).
- **V-fps and the performance panel** (`PerfState::take_composite`, `composites`,
  `complete_composites`, `video_frames_shown`): counted from `take_composed` answers, whatever device
  composed — correct as is.
- **Trace frame route** (`frame_plan.c`: display / slow / bytes, `player.rs`): the GL preview's frames
  are `bytes`, which is what they are; `compose.ui_only` in the dump says why. Correct as is.
- **Load report** (`load_watch`: `LOAD` + `GPU`, `<dump>.load.json`): carries the record's line only;
  fixed with D1.
- **`gpu_exec` (98)**: names the executor (device or processor) per piece, not a role; the ring id ties
  it to `gpu_admit`, which now carries D3's mark.
- **`kt-cli preview`**: opens a plain headless engine and presents on it on every backend, so on GL it
  measures the display path the app no longer takes there — left, found-defects 686.

## Risks

- A read-back frame on the window's device from some future caller other than the preview would count
  as preview. The rule is in kt-gpu's CLAUDE.md "What a pass costs".
