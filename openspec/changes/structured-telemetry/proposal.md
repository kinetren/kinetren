# Proposal: structured-telemetry

## Why

The owner, 2026-11-09, on release kt26.11.09-85ea51da (`docs/ktprompts/kkt-26.11.09.txt` lines 6–9,
registry rows I5–I8): a heavy Datamosh (Camera ISP → Datamosh gop 66, 720 lines, search 64 → VHS Tape)
left the preview on "Loading" for two to five seconds even on a pause; his journal said only how many
Datamosh compute jobs and composites ran each second, never why the frame waited. The orchestrator
proposed a reproduction bench and a version bisect; he refused both:

- "нет. я тут посмотрел и через ПАРУ СЕКУНД... или ПЯТЬ он в конце-концов может и прогружает картинку."
- "а еще у тебя в логах срач, очень не структурированные логи - действительно ли в них тебе удобно работать?..."
- "давай делай телеметрию так - чтобы ты мог понять что у тебя происходит, без мышиной возни туда-сюда версиями или что ты там за бред мне прелагаешь с стендом. у тебя телеметрия должна быть такая, чтобы всё было ок а не возня в ASCII логи которые вообще мало что говорят"
- "и по поводу кэша тоже пиздешь - если хоть сто раз пройтись по небольшому куску - он все равно что-то loading делает."

The mechanism: the program's diagnostics are prose lines and per-second counters. They say THAT work
ran, never WHICH frame asked for it, what that frame waited for on another thread, and why a cache
missed. A run on his machine therefore cannot explain itself, and every complaint turns into a
reproduction on ours.

## What Changes

- **Stage 1 — a structured trace.** `kt_time::trace`: typed fixed-size records (frame asked / plan /
  waits / executed / shown or abandoned; decode request → span → decoded; cache eviction with the key's
  parts and the insert that caused it; stream stage runs and advances; compute jobs; the executor's
  kept-composite memo stored / hit / evicted; pipeline warm-ups and cold draws; audio render chunks and
  the kept-mix decision; playback state changes; jobs of thumbnails, reel previews, effect thumbnails,
  posters, analysis and export; UI frames; the person's actions and edits and what each invalidated;
  the machine). Ids link cause and effect across threads: a frame's id rides with every request it
  makes. Records go into a lock-free ring (no allocation, no lock, never a block); one writer drains it
  into bounded ring files under `<data>/trace/` and sleeps while nothing comes.
- **A query tool**, `kt-cli trace`: `summary`, `loading` (why the preview was Loading from t1 to t2: the
  chain of waits ranked by time), `loop` (pass by pass over a loop region: which frames missed the kept
  composite and mix, and why), `during` (what ran in a window), `slow` (slowest frames and their
  critical path), `frame`, `machine`, `dump` (JSON lines), `chrome` (Perfetto/Chrome trace JSON).
- The text journal stays for people; its records are mirrored into the trace on the same clock.
- **Stage 2 — the defect it explains (I8, I2/I5):** a short region looped many times with a heavy chain
  is not served from the kept composites and mix, and a paused frame under that chain waits seconds.
  Found with the trace on our fixtures, fixed as a class (every effect with history; pause, slow rate,
  loop, scrub).

## Impact

- `kt-time` (new `trace` module), `kt-engine` (compose queue, player, worker, prefetch, stream stage,
  mix, export), `kt-gpu` (memo, compute host, warm worker), `kt-cache` (evictions),
  `kt-plugin-api` (stream memo evictions), `kt-app` (install, UI frames, actions, jobs, load seconds),
  `kt-cli` (`trace` subcommand, `--trace` on `preview`/`pause-scan`).
- No project format change. Nothing is sent anywhere. At rest the trace wakes no thread.
- Capabilities: `telemetry` (the trace and the query), `video-plan` and `audio-render` (stage 2).
