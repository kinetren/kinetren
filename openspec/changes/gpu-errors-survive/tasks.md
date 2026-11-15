## 1. Devices never panic on an error

- [x] 1.1 `kt_gpu::errors`: `watch` (uncaptured-error handler, device-lost callback), `set_sink`, per-role counts, rate limit (t2720, t2723)
- [x] 1.2 Install at every opener: `headless_with`, `new_d3d12`, `compute::open_device`, `from_parts`; kt-app watches eframe's device at creation and sets the sink to the journal

## 2. A frame's error is its error

- [x] 2.1 `errors::scoped` and `Inner::guarded` around every composing entry point of `GpuFxEngine` (t2721)

## 3. The display yields to a refused reconfigure

- [x] 3.1 The display role's handler holds the display for two UI frames; `ui()` ticks it; the display engine and `yield_to_display` callers wait at most 250 ms (t2722)
- [x] 3.2 Live `resize_loop` (t2724): the bench's `gpu_busy`, the `title.drag` probe; red on the old code (the owner's panic), green now

## 4. Docs

- [x] 4.1 design.md class walk; specs durability and video-plan; kt-gpu and kt-app CLAUDE.md; found-defects rows for what is left; claims registry row N1; What's new
