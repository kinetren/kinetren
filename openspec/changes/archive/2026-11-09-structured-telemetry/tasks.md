# Tasks: structured-telemetry

## 1. The trace

- [x] 1.1 Features note F-HLP-036 (`docs/features/help/errors/structured-trace.md`), the change's artifacts
- [x] 1.2 `kt_time::trace`: the record, the lock-free ring, the writer, bounded ring files, names, threads, the machine, the panic flush (t2140–t2143)
- [x] 1.3 The reader and `kt-cli trace`: summary, loading, loop, during, slow, frame, machine, dump, chrome (t2146–t2149)
- [x] 1.4 Records across the class: preview frame life, decode queue, cache evictions, stream stages and their memo, compute host, kept composites, warm-ups, audio render and kept mix, playback, jobs, UI frames, actions and invalidations, load seconds (t2144, t2145, t2150)
- [x] 1.5 kt-app installs it (data dir, panic flush, journal mirror); `kt-cli preview`/`pause-scan --trace DIR`
- [x] 1.6 Cost at rest on the heavy showcase: pause, pointer over a still place, an effects window open

## 2. The cache defect (I8, I2/I5)

- [x] 2.1 A heavy loop on our fixtures (Camera ISP → Datamosh gop 66 / 720 lines / search 64 → VHS Tape), the trace's answers before
- [x] 2.2 The mechanism found and fixed as a class (history effects; pause, slow rate, loop, scrub), with tests
- [x] 2.3 Live scenario `heavy_loop` with a PATH line; `cached_range` stays green
- [x] 2.4 Archive: specs merged, registry rows I5–I8 moved, What's new line, found-defects rows
