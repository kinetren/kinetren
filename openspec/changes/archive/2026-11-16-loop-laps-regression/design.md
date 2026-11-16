# Design: loop-laps-regression

## Context

kkt-26.11.16: "a loop put on problem_region_26_11_09 plays through without loading only from about the
25th lap in the new version, from about the 11th in kt26.11.10"; the same comparison on a piece without
Datamosh (rendereg). Evidence: his two trace pairs, kt26.11.16-0c48dc8f against kt26.11.10-4275c0f8.

## Diagnosis from his traces

`kt-cli trace … gpu`, problem region:

| | kt26.11.10 | kt26.11.16 |
|---|---|---|
| Datamosh pieces on the device | 94 (one a job) | 1764 |
| device busy / idle between pieces | 44.2 s / 0.3 s | 10.0 s / 27.6 s, 25.0 s of it with nothing queued |
| near admission waits / the jobs' wall | 82.9 s / 87.6 s (95 %) | 176.1 s / 36.2 s (486 %) |
| display present wait p50 | 60.3 ms | 53.9 ms |

His near pieces claim exactly a quarter of the bound (4.17 ms at his 30 fps) and wait p50 16.8 ms, p90 62 ms
each. `trace loop` of the new run: the history walks 30–58 positions a lap of 11 s; 54–114 frames a lap
are incomplete for want of it.

rendereg (no Datamosh): the loop fill (`job` 13 on the scheduler's lane, class near) composed 607 moments
in 87.5 s, 144 ms each, 55.9 s of it admission waits; the display's own composites are as fast as in
kt26.11.10 (present wait p50 55 ms against 67 ms).

### D1. "The frame's key changed" was the report, not the keys

`trace loop` named 6–25 moments a lap "the frame's key changed: layer 0 is another stream frame / layer 1
was decoded again". Every complete composite of each of the 577 moments of the run had ONE key: the earlier
record was an INCOMPLETE composite (a stream stage short) whose layer content was 0 or another key. The
report now names a moment's key by complete composites only. t2760.

### D2. A saturating display keeps near work at the bound

The mechanism: gpu-throughput D1 (`80dd96cb`, stage 11: admission by the predicted drain, lower pieces cut
at half the bound, at most three ahead) with gpu-leftovers D4 (`8de88b13`, stage 12, found-defects 491: a
display that fills its period leaves `near` a quarter bound and the classes below it nothing). His display
fills its period in both versions (present wait 54–60 ms at 33 ms), so the look-ahead, its stream walk and
the loop fill got a quarter bound a period while the device idled. Now `near` keeps the bound,
`visible`/`background` a quarter at least, an export only the slack (`arbiter::room`). The owner's latency
limit stays: no piece longer than the bound, at most `AHEAD_PIECES` (3) lower pieces of half the bound
uncompleted in front of the display — kt26.11.10 put whole 380 ms jobs there. t2517, t2433 rewritten.

### Checked and not a mechanism

- The fill composing frames that cannot be kept: 430 of its 720 composites in his problem-region run came
  out incomplete, and they cost 53 ms in all — an incomplete plan stops before its first upload
  (no-partial-picture). Left.
- Kept-composite key churn between laps: none (D1).
- Stage 11–15 work that adds per-frame admission, scheduling or memo churn, walked: the ring's admission
  (D2 — every lower class); the loop fill's lane (`near`, D2); the far stream walk (background, D2 and
  stream-round-trips D8); the compute host's pieces (D2); the present memo and stream memo evictions (his
  `stream_memo_evict` 720 / `memo_evict` 0 follow the loop's want against the ledger — the memos kept what
  the history produced; no churn found beyond the history's lag); the scheduler's lanes (`sched_run` 841 are
  the fill's jobs, D2).

## Cost

Release, kt26.11.10-4275c0f8 against this branch, started together, each on its own copy of a project the
old binary wrote (`showcase --heavy`, and `--heavy --mosh`), footage in `/dev/shm`, a 2 s loop played 60 s at
640×360 (`kt-cli preview … --loop 60`):

| | kt26.11.10 | this branch |
|---|---|---|
| plain piece (20–22 s): first lap served wholly from kept composites | 14 | 7 |
| plain piece: V-fps | 20.6 | 21.4 |
| Datamosh piece (11.2–13.2 s): first lap served wholly from kept composites | 17 | 9 |
| Datamosh piece: V-fps | 18.8 | 19.0 |

Here the loop fill and the stream walk run on rings of their own (llvmpipe gives every device one), so this
is stage 11–15's work without D2's effect: on his iGPU the shared ring is what held them back. D2 cannot act
here; `kt-cli gpu-share` on ONE llvmpipe
device walked 25 positions a second under both rules (row 692).
