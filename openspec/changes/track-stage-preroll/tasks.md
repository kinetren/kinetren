# Tasks: track-stage-preroll

> Waits for Q1 (`proposal.md`) and the orchestrator's go. Tests reserved: t3361–t3366. Base: orch `f1f063c4`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D6, class walk D5), delta `audio-render`; `openspec validate track-stage-preroll --strict`
- [ ] 0.2 his answer Q1 (orchestrator); found-defects 420 points here

## 1. The pre-roll

- [ ] 1.1 `AudioRenderer`: per-stage pre-roll on a real seek (D1), dependency order, key branches; Q1's bound
- [ ] 1.2 `cold_after_seek` 0 after a full pre-roll (D4); the trace record (D2)
- [ ] 1.3 t3361–t3366; t166, t167, t732, t2373, GOLDEN_AUDIO green
- [ ] 1.4 `kt-cli probe … --time` on a release build over a track reverb seek (the cost line); clippy;
      target-prune; commit; What's new line ("a seek into a track's reverb tail now rings")
