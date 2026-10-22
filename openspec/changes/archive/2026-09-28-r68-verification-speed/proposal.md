# Proposal

## Why

One verification cycle takes **21 minutes** on a warm tree of a 32-core machine (measured on 5c7dc09,
details in `docs/user-feedback.md`, R.68): live scenarios — 1012 s, `cargo test
--workspace` — 259 s, the build cascade from an edit in kt-model plus the build of kt-app/kt-cli — another
~100 s. The owner, verbatim: "tests in finish.sh or something like that - take a very long time
to run, we need to come up with something".

The cost is not in the minutes, but in the fact that the expensive check starts to be bypassed: the rule "the work ends with
`finish.sh`" exists precisely because the owner three times received unverified work ("you didn't check" ×3). A check that takes a quarter of an hour
itself creates the temptation against which this rule protects. Moreover, 85 % of the time goes not to
checks, but to mechanics: 50 cold starts of kt-app at 3–5 s each, fixed pauses
(`sleep 0.35` per click, 0.65 s for EACH typed character) instead of waiting for a frame, and a strictly
sequential run on a machine where 7 of 32 cores are busy.

## What Changes

- **`finish.sh` becomes the single point of verification and names the price of each stage.**
  The script runs the workspace test run itself (now it is a separate command of the agent),
  prints the time of each stage and the total, keeps a **5-minute budget** (the owner's decision), and
  flags a stage that exceeded its share of the budget. The `cargo test -p kt-gpu` stage is removed
  as covered by the workspace run; the SIMD stages remain — they have THEIR OWN environment
  (`KT_REQUIRE_SIMD=1`, `KT_SIMD=0`), and they take two seconds for all four.
- **The live stand: one kt-app per worker, projects are opened one after another.** The program
  gets a test readiness channel (an extension of the existing `KT_GEOM_DUMP`): "open a
  project, reset the workplace, report readiness" and the NUMBER OF THE DRAWN FRAME. The frame `kt_live.py`
  waits for the frame, not a fixed sleep. Scenarios that check the START itself (recovery after SIGKILL, the question about unsaved work, the project lock, importing an old
  file) declare "their own process" and get it.
- **Live scenarios run in several workers** (the owner's decision, a revision of the rule
  R.59 §468): each has its own Xvfb, its own `XDG_DATA_HOME` and a reduced number of software-rendering
  threads; the number of workers is limited by cores; a red scenario is automatically
  re-run ALONE, and only a second red counts as red.
- **The workspace linker — mold** (a direct request of the owner; 2.30.0 is already installed on the system).
  The `release`/`dist` profiles and invariant No. 10 ("ldd without libav*") do not change.
- **A revision of the live checks set without weakening it** (the owner's decision: "move some of the tests
  to non-gui… click through the complex moments or those that were never clicked even once").
  A machine report "what was never touched live" appears (the fields and windows from the registry
  `WINDOWS` and the geom dump compared with what the scenarios click). The simple repetitive mass
  moves to the harness/model tests with the `TEST` lines in the specs preserved; the mouse stays on
  the complex places and the never-clicked ones. Not a single spec scenario disappears silently: each moved one gets a new test address on the same spec line.
- **The budget is held by a test, not by the agent's memory**: a guard checks that the script prints
  the stage times and the total, and that the budget is declared; the work report carries these numbers.

Not included: reducing the set of checks itself (the owner chose "only the mechanics, don't touch the set"), editing the `release`/`dist` profiles, and analysis of the two red scenarios `fxfield` and
`fxpicker`, found during the measurement (separate work, noted in `docs/user-feedback.md`).

## Capabilities

### New Capabilities

(none — the verification machinery is already described by the requirements of `durability` and `ui-reach`)

### Modified Capabilities

- `durability`: the requirement "The work ends with `finish.sh`, and the proof is a live scenario" — the ban "live scenarios are not run in two threads" is removed (replaced by
  bounded parallelism with an automatic single re-run of the red), a time budget is added, printing the price of each stage and the rule "the workspace run is a stage of the same
  script"; the kt-gpu stage is removed as covered.
- `ui-reach`: the requirement "A test asserts an observable consequence from the point where the person acts; for each item — a live scenario" — a CRITERION appears for what is checked live
  (the complex and never-clicked) and what moves to headless, a machine report of live
  coverage, and the rule of the live stand (a reused process versus "its own process" for the startup
  scenarios).

## Impact

- `src/scripts/finish.sh` — the stages, their timing, the budget, the removed kt-gpu stage.
- `src/scripts/live-check.sh` — workers, their own screens and storages, the re-run of the red.
- `src/scripts/live/kt_live.py` and 50 scenarios in `src/scripts/live/` — waiting for a frame
  instead of pauses, declaring "its own process" for the startup scenarios.
- `src/apps/kt-app` — the test readiness channel and the frame number next to `KT_GEOM_DUMP`
  (under an environment variable, dead in the product).
- `src/.cargo/config.toml` (a new file) — mold for the host platform.
- A new live coverage guard in `src/scripts/`, new tests from **t800**.
- `openspec/specs/durability/{spec,design}.md`, `openspec/specs/ui-reach/{spec,design}.md`.
- Tests moved from live scenarios into the harness/model: the addresses in the `TEST` lines of the
  affected capabilities.
