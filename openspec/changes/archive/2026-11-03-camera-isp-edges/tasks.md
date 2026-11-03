# Tasks: camera-isp-edges

Test number t1430 is used by this change (t1431–t1434 reserved and free).

## 1. The edges

- [x] 1.1 Locate the stage with `view` on an edge-ramp frame (P3b for "Phone at night", P0 for barrel lenses)
- [x] 1.2 `lens_fit` in P0 and `isp_crop` in P3b, CPU and WGSL as a pair; the defaults keep their bytes
- [x] 1.3 t1220's geometric reference at the output's own magnification; t1430 on both executors, red on the old kernel
- [x] 1.4 The video-fx scenario and design; What's new
  - Done. `cargo test --workspace --no-fail-fast`: 201 suites green but t815 (a wall-clock bound on writing the restore record; load average 44 on 32 cores from parallel sessions; green alone); clippy clean on the touched crates
- [x] 1.5 Re-take `fx-camera-isp`; finish.sh
  - Done. The shot re-taken by `readme-shots.py --sources …/showcase-media` on the release build
    (display :241 — another session held :231); at 1:1 the old shot's edge streaks (left, top,
    right, bottom) are gone. finish.sh kt26.11.03-485fc95a: workshop tests 1450/1450, SIMD
    conformance green; live scenarios at load 20–44 — all green alone but `formula` (its
    "space plays" check, red at load 10–19 on two different steps; this change touches no
    kt-app, automation or formula code; a sibling session saw the same load red)
