# Spec Delta

## ADDED Requirements

### Requirement: A barrier sees all its rows through the values door
`BarrierKernel` SHALL offer `frame_values` and `gpu_values`, which receive the slot's values in
passport order; their defaults SHALL be `frame`/`gpu` over `cook`, so a barrier that implements only
the cooked forms keeps its bytes. The host SHALL call the values forms everywhere it runs or
plans a barrier: the CPU executor's barrier position, the GPU barrier op and the registry's graph
check on the defaults and the top of the passport.
Why: `Cooked` is eight numbers. A simulation (camera_isp, ~50 rows) cannot pack its passport into
them without quantizing every automated row into bit-fields; the values door keeps the rows as
the passport declares them.

#### Scenario: A row past the eighth reaches the kernel on both executors
- **WHEN** a test barrier with twelve rows, whose output depends only on row 11, is registered and
  run through the CPU executor and its GPU graph is built, with row 11 at two values; and a
  built-in barrier (blur) runs through the same doors
- **THEN** the two values of row 11 give two different outputs on the CPU and two different graphs,
  the registry checked the graph on row 11's top, and blur's bytes are its frozen golden
- **TEST** t1210 (`src/libs/kt-fx/tests/barrier_values.rs`), t1051
