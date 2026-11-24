# Registry of the owner's list `kkt-26.11.22`

Source — [`kkt-26.11.22.txt`](kkt-26.11.22.txt) (verbatim, his answers of 22–23.11.2026 to Q55–Q58). Columns and states —
as in [`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| D1 | 4 | the persistent cache keeps the FINAL preview frame only, not results at stages of an effect chain | no: feature, change `persistent-cache` reduced to final frames (+ the heavy-source copies of B1), `project-cache-folder` layout B loses `layers/` | spec-features, spec-engine | 23 |
| D2 | 5 | he asks whether a separate cache sits after every chain today and whether we store far too much data | question: what does each cache keep today (memory and disk), per stage or final, and how many bytes on his project? (an audit by spec-engine; the answer relayed to him) | `docs/cache-audit.md` (spec-engine audit, all caches) | 23 |
| D3 | 6 | letter physics is not a control in a form: design a COMPOSITING SYSTEM that makes such effects possible among others, and walk the whole program for the places it applies | no: feature, change `text-letter-physics` replaced by the architecture design `simulated-elements` (objects with simulated state inside the composite) with a class walk | spec-features | 23 |
| D4 | 7 | OpenFX frames through shared memory (RAM shm or GPU shared memory), no copy per frame | no: feature, change `openfx-host` (Q1 a; Q2 own process with zero-copy shared memory) | spec-features | 25 |
| D5 | 8 | ARM: Linux aarch64 first | no: feature, change `arm-build` (Q1 a) | executor | 25 |
| D6 | 9 | the Datamosh history is not stored on disk: final frames only, the history recomputed from the cut on a miss (replaces A14's "build now" for the history half) | no: feature, change `durable-history-and-mix` drops the disk history (the kept mix stays as decided) | spec-engine | 23 |
| D7 | 10 | decode sizes of R.46/R.62 stay; what he expects is the render cache: a held stretch is decoded as video and shown, nothing else | no: feature, changes `project-cache-folder` and `persistent-cache` (executor mosh-sharp, next) | executor mosh-sharp | 24 |
