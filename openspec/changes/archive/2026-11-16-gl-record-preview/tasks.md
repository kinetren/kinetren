## 1. The preview's device counts as the preview

- [x] 1.1 Read-back frames of `keeps_bytes` or surface engines timed as `Role::Preview`; holds name the role per engine
- [x] 1.2 `GpuContext::new_headless_for_preview` opens the preview device under the role `preview`; `serve_preview` on its sibling
- [x] 1.3 `gpu_admit.c` = 2 for a fronting ring without a display; `kt-cli trace gpu` marks it (t2263 extended)
- [x] 1.4 t2747: two preview frames off the display count preview, none offline (red on the old role: "2 frames (offline 2)")
- [x] 1.5 Walk of every counter that assumed "preview frames = the window's device", recorded in design.md
- [x] 1.6 `sibling` under the gate, `Drop for GpuFxEngine` releases its state under the gate (row 683); t2748
- [x] 1.7 Live `gpu_record` and its neighbours
