# Registry of the owner's list `kkt-26.11.10b`

Source — [`kkt-26.11.10b.txt`](kkt-26.11.10b.txt) (verbatim, one message of 10.11.2026, his answer to
the stage-10 report). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). His run:
release kt26.11.10-a3c98bf7 (stage 10). His files: `/media/e/developer/kt.9/fails/26.11.10.2/`
(`load-report.txt`, `trace/` — three runs of 10.11 evening plus the earlier run's files).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| K1 | 1 | found-defects 403: while the history catches up inside the loop, show Loading, not a held frame from before the loop | partial: decided by him; goes to an executor | found-defects 403 | decided |
| K2 | 3 | the "catching up" label belongs where "Loading" is written | partial: decided by him; goes with K1 | K1 | decided |
| K3 | 4 | on `problem_region_26_11_09` "catching up" runs with 3 GB of cache taken and little processor, as if lazy or one core; from about the tenth lap it gives everything at once — is there no spec for using the machine? | question: what bounds the catch-up on his card, and why is there no rule that work uses the machine? — diagnosed from his trace | his trace | diagnosing |
| K4 | 6 | did the fixes check the same problem in other places? | question: which class walks did each change of stages 9–10 make, and what did they miss? — answered from the design records | the archives of stages 9–10 | answering |
| K5 | 7 | while catching up, VHS Tape seems laid over a frame Datamosh has not made — either the frame is there or not | question: is a later stage applied over a held stream frame? — answered from the code and his trace; K1 removes it inside a loop | K1 | diagnosing |
| K6 | 8 | after everything loaded, toggling VHS Tape switches at once with nothing loading — how long are such stale frames kept, are they ever erased? | question: what keeps a composite of a former chain and when does it leave? — answered from the memory ledger and his trace | `memory-ledger`, `frames-cache-growth` | answering |
| K7 | 9 | telemetry in `kt.9/fails/26.11.10.2`; is `load-report.txt` needed or is everything in the trace — drop the txt? | question: does the trace carry everything the load report has? — checked against his two files | his files | answering |
| K8 | 11 | answer first; then finish the performance work, he is tired of it | partial: answered in the report of 10.11 | K3–K7 | answering |
| K9 | 14 | future: place Datamosh keyframes by hand, not only at a fixed interval; the same rule for other effects | future: a feature note in `docs/features/` first (how other editors and datamosh tools let a person place I-frames), then a change | — | future |
