# Proposal: datamosh-history-wait

## Why

kkt-26.11.16, the owner's words: "поставил loop туда просто первое что попалось под руку - и он ни с 5 ни с
10 ни с 20го раза, а раза с 25го смог всё прогрузить чтобы оно шло разом без прогрузок, старая версия смогла
по ощущениям с 11-го раза прогнать. это пиздец." His trace `problem_region_26_11_09/new` (kt26.11.16-8c4ca513,
Ryzen 7700X iGPU, RADV) read in `trace gpu` as compute jobs whose buffers came back "p50 607 ms p95 1990 ms"
after the device ended them (found-defects 693), and near work waiting for admission 486 % of its wall while
the card idled (found-defects 662).

## What Changes

- `trace gpu` reads a compute job's pieces as ONE job (its span on its thread). Every piece's record carries
  its own instant, so the old grouping counted every piece as a job's last one: the 607 ms was the job's own
  admission waits between pieces. Read right, his buffers come back p50 37 ms after the device end (p95 54).
- Every wait for a device names its own submission: the engine's read-back frame (the loop fill's lanes,
  effect cells, reel strips, the slow route), its pipeline warm-up, the compute host's clock calibration and
  shader warm-up. A wait for "everything submitted" also waited for every other thread's work on the device
  and held the device's lock through it. `kt_gpu::waits_for_all()` counts the waits that remain (a parked
  engine, `finish_present`).
- A ring's waiter whose flights are on several devices looks at all of them every 2 ms: the devices of one
  card finish out of each other's order, and a blocking wait on the oldest piece of one device missed the
  room a piece on another made.
- The preview's present checks stop growing at the frame's own time (at least 4 ms): unbounded doubling saw a
  frame the device ended late at up to twice its lateness.

## Capabilities

### Modified Capabilities
- `video-plan`: waits name their submission; the ring's waiter; the present checks.
- `telemetry`: `trace gpu` reads a job's pieces as one job.
