# Design

## Context

The motivation is in `proposal.md`. Only the measured numbers and the limits of the mechanics matter here
(measured on 5c7dc09, 32 cores, warm tree; the full table — `docs/user-feedback.md`, R.68).

- Live scenarios: 50 of them, **1012 s** sequentially, spread 4–136 s (`nestparam` — 136 s).
- The share of unconditional `time.sleep` inside a scenario: `nestparam` 130.5 s of 135.9 (591 calls),
  `curvesrc` 41.9 of 43.2 (213), `level` 4.2 of 4.7 (24). That is, **96 %** of a live run
  is sleep, not the program's work and not gestures.
- `cargo test --workspace` (a warm run, without build) — **259 s**; inside: kt-app ~71 s,
  kt-gpu ~28 s, kt-engine ~26 s, the other crates 0–3 s each. There are 96 test binaries, and
  cargo runs them ONE AFTER ANOTHER.
- Build: the cascade from an edit in kt-model (`--no-run` of the whole workspace) — 61 s,
  build of kt-app + kt-cli — 43 s, relinking of one test binary — 2 s.
- The finish.sh stages other than live scenarios: kt-gpu 10 s, four SIMD stages ~2 s in all.
- One kt-app in software rendering (`WGPU_BACKEND=gl`, llvmpipe) holds a load of about 7 on
  32 cores; a frame in this mode takes ~0.2 s (documented in the frame).

Limits that must not be touched: the set of checks is not weakened (the owner's choice), tests from
the specs are not removed or weakened, the verdict of a stage is taken from the checking command (R.58),
red is called by its own name (R.55), the `release`/`dist` profiles and invariant No. 10 ("ldd without
libav*") remain as they are.

## Goals / Non-Goals

**Goals:**
- `finish.sh` — one command that checks EVERYTHING and fits within 5 minutes, naming the price
  of each stage.
- The live run stops sleeping: the wait is tied to an observable event of the program.
- The live run uses the machine: workers with isolated screens and storages,
  red is told apart from "turned red under load" automatically.
- What is live is what is checked live (the hit of a gesture, the unclicked, complex
  paths); the repetitive mass moves to headless without losing the spec scenarios.

**Non-Goals:**
- Reducing the set of checks, a "fast mode" with a partial run, editing
  `release`/`dist`, analysis of the red `fxfield`/`fxpicker` (separate work).
- Merging the 96 test binaries into one: the test addresses stand in hundreds of `TEST` lines of the specs, and
  the gain (2 s per relinking) does not pay for rewriting the contracts.
- A real graphics card in the virtual screen: one must check the path that a person without
  a card takes, and kt-gpu conformance is run anyway as a separate stage on a real adapter.

## Decisions

### 1. Revision: "live scenarios are not run in two threads" → bounded parallelism with a single re-run of the red

**Revision: the ban on parallelism is lifted — the cause of the ban is closed by a mechanism, not by hope.** The ban appeared in R.59 §468: a second `live-check.sh` under a load of ~14 turned green scenarios red, while a single run gave 36 of 36. The owner allowed parallelism with a direct answer
("Yes, with a load limit and an automatic restart of the red"), and the mechanism closes exactly
that mechanism of failure:

- there are no more workers than cores divided by seven (one kt-app in software rendering), with a cap from
  an environment variable; for software rendering each worker's thread count is reduced
  (`LP_NUM_THREADS`), so that the workers do not take cores from each other;
- each worker has its own `DISPLAY` (its own Xvfb + openbox), its own `XDG_DATA_HOME`/`KT_USER_STORE_DIR`
  and its own directory in `target/live-out` — otherwise `probe.window()` finds a neighbor's window by
  `WM_CLASS`, and the storage mixes journals and locks;
- **red in parallel is not a verdict**: the scenario is re-run once, alone, and only
  a second failure counts as red. This is exactly the rule that is already written for
  "failed at a pause or in the order of a background thread" — now it is enforced by the machine, not by the agent's memory. The report names those who passed on the second attempt: a silent re-run would turn
  a flake into an invisible thing.

Alternatives: keep a single thread (the owner decided otherwise; and without parallelism the 5-minute goal
is unattainable); run live scenarios in containers, one per core (the same isolation at the cost
of an image and rights); hand the parallelism to `pytest -n` (the scenarios are self-contained programs, not
pytest tests; one more dependency for the same loop over a directory).

### 2. The sleep is replaced by an observable event; the program reports the FRAME NUMBER

96 % of live time is sleep, because the frame waits "by eye": 0.35 s after a click, 0.45 after
RMB, 0.6 after a drag, 0.2 on `focus()` and **0.65 s for each typed character**. All of this is an
attempt to wait for a FRAME that the program could report itself.

- kt-app, next to `KT_GEOM_DUMP`, reports the number of the drawn frame (and a sign "waiting for input");
  the frame gets `wait_frames(n)` and waits for a frame, not for the clock. At a frame of 0.2 s the gain on
  a click is small, but for 591 waits of `nestparam` it adds up to minutes, and most importantly,
  the cause of false reds disappears: under load the wait STRETCHES together with the
  frame, which a fixed pause cannot do.
- typing a line goes in full: egui reads input events per frame, and the whole line lands in one
  frame — instead of `len(text) × 0.65 s` there is a single wait.
- `focus()` before each gesture is not needed while the window has not changed: raising the window remains on a
  CHANGE of window (modal forms, pop-ups) and on the first gesture of a scenario.
- the remaining pauses are legitimate where there is nothing to wait for: the egui double-click time threshold (0.04 s
  between clicks — measured in R.34: 5 of 5 against 0 of 5), autosave by the clock, a pause inside
  a drag between positions. Each such pause carries its reason in words, and the guard
  (`live-sleeps.py`) fails on a pause without a reason — the same technique as the window registry of R.66 and
  the PATH guard of R.67: a rule in prose does not hold, a guard with a measurable exception does.

Alternatives: raise the frame rate in the stand (reduces the price of waiting, but does not remove
the blindness — left as a separate measurement task: llvmpipe threads against window size);
`xdotool --sync` (can only wait for a window to appear); waiting by a screenshot on each gesture
(the ImageMagick snapshot itself costs 0.3–1 s — more than a frame).

### 3. The stand keeps the process, the scenario declares "its own process"

The cold start is `_wait_window` with snapshots via ImageMagick until "the screen is not monochrome"
plus 0.5 s on top, that is 3–5 s for each of the 50 scenarios. The stand opens projects one after another
in one process:

- the control channel is switched on by an environment variable (dead in the product) and can do exactly
  three things: "open a project", "bring to a clean state", "report readiness and the frame number".
  Opening a project in the stand does NOT ask about unsaved work — otherwise a scenario would start
  with the modal window of the previous one;
- cleanliness is proven, not assumed: the list of open windows in the geometry dump is empty,
  the address of the open project is the one that was requested. Without this, a leak of state between
  scenarios would produce exactly the class of problem by which Kinotrain was already sick: "green, because
  we looked in the wrong place";
- the startup scenarios (`recover.py` — SIGKILL, `unsaved.py` — the question about unsaved work,
  `oldfile.py`/`import_old.py` — reading a foreign version, checking the lock) declare "their own
  process" by a flag in the scenario itself: the decision about what is checked belongs to the scenario, and not to
  the run's setting.

Alternatives: keep the process, but open projects through the File menu with the mouse (the same stand
at the cost of a dozen gestures per scenario and dependence on the file dialog); fork the process
from the warmed-up state (wgpu/X connections do not survive a fork).

### 4. The linker is mold; the profiles and invariant No. 10 do not change

`src/.cargo/config.toml` sets mold for `x86_64-unknown-linux-gnu` (2.30.0 is installed on the system;
the toolchain's `rust-lld` ships alongside as a fallback path). The linker flags are
`rustflags`, that is, a one-time full rebuild of the tree; the gain is measured on the
cascade from kt-model (61 s) and on the build of kt-app/kt-cli (43 s), and if it is less than 20 %, mold
stays, but the task honestly states that the matter was not the linker (relinking of one binary
already costs 2 s — most of the cascade is COMPILATION). `release-build.sh` builds in the
Ubuntu 22.04 container: if mold is not installed there, the config must not break the release build —
checked by running `scripts/release-build.sh` and by the GLIBC_ threshold with "ldd without libav*".

### 5. The workspace run is a stage of the script; kt-gpu is removed; nextest — by measurement

Today `cargo test` is run by the agent, while `finish.sh` runs only kt-gpu and SIMD; the budget and the verdict
fall into two halves, and the kt-gpu stage duplicates the workspace. The workspace run
becomes a stage of the script, the kt-gpu stage is removed, the SIMD stages remain (they have their own
environment and take 2 s).
`cargo test` runs the 96 test binaries one by one — with 1078 tests and 32 cores this is
the main cause of 259 s. `cargo-nextest` distributes tests across processes and usually gives 2–4×, but
it changes the isolation model (a test in its own process), so it is taken ONLY by measurement and only
if the full set stays green; `cargo test` remains the reference and the path without an external
tool. If nextest changes the verdict of at least one test — it is not taken, and the discrepancy
is analyzed as an isolation defect.

### 6. The budget is counted by the script, the guard holds it

Each stage is wrapped in a measurement, the total is printed as a table, the budget is 300 s. The budget
arithmetic and the formatting live in a separate script (`finish-budget.py`), so that they can be
checked by a test on synthetic data, and the guard additionally verifies: each stage that the script
EXECUTED has a timing line (otherwise a stage could be quietly removed from the
budget — the same class as "red under someone else's name").

### 7. What moves to headless — by the coverage report, not by taste

`live-coverage.py` combines: the windows of the `WINDOWS` registry, the fields of the geom dump and the gestures of the scenarios
(`src/scripts/live/*.py`) — and prints which places a person has never once reached with the mouse.
The report gives two things: a list of candidates for moving (checks whose gesture is already clicked in another
scenario, and whose assertion is about the model/file) and a list of holes (places not clicked even once) —
exactly what the owner asked for. A move must keep the spec scenario and its `TEST` line
with the new address; removing a scenario is a violation of the rule "tests from the specs are not weakened".

## Risks / Trade-offs

- **Parallelism brings back the flake** → the load limits the workers, each has its own screen
  and storage, a red one is re-run alone; those who passed on the second attempt are printed, and a repeated
  "second time" is a signal to analyze the scenario, not to put up with it.
- **A reused process leaks state and makes the check falsely green** → cleanliness is proven before the first gesture
  (an empty list of windows, the project address); the startup scenarios
  have their own process; the stage comparing "the stand against cold starts" is run in full once
  and checks the verdicts against each other.
- **Moving checks to headless takes away exactly what was caught only in the window** → the criterion
  is written in the spec (gesture, hit, unclicked — live), the coverage report shows the holes,
  and each move leaves the spec scenario in place.
- **mold or nextest break a rare build/isolation path** → the delivery is checked by
  `release-build.sh`, nextest is taken only with a full green and keeps `cargo test`
  as the reference.
- **The 5-minute budget will start to press on the set** → the budget prints and flags, but does not remove
  checks; exceeding the budget is a reason to speed up the mechanics or split the stage, not to throw out
  a test (the rule "tests from the specs are not weakened" outweighs the budget).
- **The stand's test channel is a hole in the product** → it is switched on only by an environment variable,
  a reachability guard checks that without it the channel is dead.

## Data for stage 5 (collected by the machine, R.68)

The `scripts/live-coverage.py` report at the time of the round: the program publishes **174**
named places, the scenarios touch **66**, and **108 were never touched with the mouse**
(the list is grouped by places: `ask.*`, `track.menu.*`, `pub.row.*`,
`settings.*`, `bin.reel.*` and others) — this is the queue for live checking.

Candidates for moving to headless are counted by the same principle "an assertion without
a picture": **27 scenarios of 50 read not a single pixel** (`nestenter`,
`nestopen`, `rate`, `nestshare`, `nestzero`, `formula`, `nestwarp`, `nest`,
`curves`, `curvesrc`, `geomctl`, `nestpreview`, `oldfile`, `seekjump`,
`fxfield`, `corner`, `select`, `stills`, `unsaved`, `envclip`, `forms`, `keys`,
`sidechain`, `level`, `recover`, `import_old`, `ruler`), that is, their verdict is
the state of the model and the file. What moves is not the whole scenario, however, but the
CHECK: the gesture stays live where it is the only one (a hit test, the edges,
the order of windows), and the part that repeats an already clicked gesture
for the sake of checking the model moves out.

## Migration Plan

1. Timing and budget (without speeding anything up) — so that further comparison is with a number, not with
   memory; here also the workspace run as a stage and the removal of kt-gpu.
2. mold and the measurement of the build.
3. The stand: the readiness channel and the frame number, waiting by frame, the whole line typed at once;
   the startup scenarios declare "their own process".
4. Parallel workers and the re-run of the red alone.
5. The live coverage report; moving the repetitive mass into headless; closing the holes with new
   live checks.

Rollback: each step is reversible separately — `.cargo/config.toml` is deleted, parallelism
is switched off with a key (`KT_LIVE_WORKERS=1`), the stand is switched off by an environment variable (the frame
returns to a cold start), the timing is harmless.
