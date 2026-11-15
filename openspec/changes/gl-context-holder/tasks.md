## 1. Name the holder

- [x] 1.1 `kt_gpu::holds`: `hold` (one call), `within` (a span), `in_flight`; a long call on the window's device is a journal line (t2740)
- [x] 1.2 Holds at pipeline creation, every engine and warm submission, the blocking waits; spans at the entry points; the panic report lists the calls in flight

## 2. The window's GL device is the UI thread's alone

- [x] 2.1 `display_is_ui_only`, `preview_engine_off_display` (one device per process, a sibling per project), the exit park (t2741, t2742)
- [x] 2.2 kt-app routes GL to it and takes the bytes path; the dump's `compose.ui_only`; `gpufx` knows the GL route (t2744)

## 3. Proof

- [x] 3.1 Bench `gl_stall`; live `gl_stall` (t2743) red on the old route with the owner-class panic, green now
- [x] 3.2 `gpu_busy` does nothing on GL; `resize_loop` expects no refused reconfigure there
- [x] 3.3 render, hwencode, renderregion, readme_shots, gl_stall, cached_range, loop under load (28 busy processes), three times

## 4. Docs

- [x] 4.1 design.md class walk; specs; kt-gpu and kt-app CLAUDE.md; found-defects 651 out, 680–682 in; What's new
