## ADDED Requirements

### Requirement: The load report needs no setup and stays on the machine
From the first second a thread of its own (`load-watch`) SHALL sample once a second which threads
hold the processor and which work counters moved, keeping a quarter of an hour in memory. A load of a
whole core for three seconds SHALL write `load-report.txt` in the data directory by itself, at most
once in five seconds, so that a person in such lag has nothing to reach. The report names the build,
every device the machine offers, which card drives the monitors and which was chosen. Diagnostics
shows the same live and opens the folder with one click. Nothing in telemetry reads it. At rest the
watch costs one read of `/proc/self/task` a second and no write. On Windows it has no per-thread
times yet: the counters still say what ran.
Why: the owner: "И у меня даже нет ИНСТРУМЕНТОВ чтобы тебе принести и доказать свою проблему. ... Я
даже не уверен что смог бы в таких лагах его включить вообще."

#### Scenario: A sample names the threads and the work
- **WHEN** two readings of the threads and the counters are a second apart
- **THEN** a pool is one row, a thread that did nothing is no row, a name with brackets is read whole, and a second of nothing reads "at rest"
- **TEST** t1839 (`src/apps/kt-app/src/load_watch.rs`)
