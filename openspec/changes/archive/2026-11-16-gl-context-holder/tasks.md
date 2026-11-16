## 1. Name the holder

- [x] 1.1 `kt_gpu::holds`: `hold` (one call), `within` (a span), `in_flight`; a long call on the window's device is a journal line (t2740)
- [x] 1.2 Holds at pipeline creation, every engine and warm submission, the blocking waits; spans at the entry points; the panic report lists the calls in flight

## 2. The window's GL device is the UI thread's alone

- [x] 2.1 `display_is_ui_only`, `preview_engine_off_display` (one device per process, a sibling per project), the exit park (t2741, t2742)
- [x] 2.2 kt-app routes GL to it and takes the bytes path; the dump's `compose.ui_only`; `gpufx` knows the GL route (t2744)

## 2b. Waiters and the covered range on the preview device

- [x] 2b.1 The GL gate on `Compiled`, taken by every entry point, the warm worker and the bench (t2745)
- [x] 2b.2 The bytes-path preview keeps its composites in the present memo and replays them (t2746)

## 3. Proof

- [x] 3.1 Bench `gl_stall`; live `gl_stall` (t2743) red on the old route with the owner-class panic, green now
- [x] 3.2 `gpu_busy` does nothing on GL; `resize_loop` expects no refused reconfigure there
- [x] 3.3 render, hwencode, renderregion, readme_shots, gl_stall, cached_range, loop under load (28 busy processes, load 66–84, beside a workspace nextest), three times: 7 of 7, 7 of 7, 7 of 7 with render green on its retry (row 685)

## 4. Docs

- [x] 4.1 design.md class walk; specs; kt-gpu and kt-app CLAUDE.md; found-defects 651 out, 680–682 in; What's new
