# Design: kept-mix-settle

## Context

His trace at +240.149 (`kt-cli trace … dump`, `mix_chunk` records, the region 4690.339..4690.502 =
7802 frames):

```
+240.1497  mix_chunk ×7  aux 1 (replay)   heads rs .. rs+6144
+240.2010  mix_chunk     aux 2 (kept)     head rs+7168, c 0x101 (lap, never kept)  ← reset here
+240.2016… mix_chunk ×8  aux 2 (kept)     the whole second lap, under Lap(rs, re)
+240.2383… mix_chunk     aux 1            every later lap
```

The 7 replayed chunks were recorded at +104 during a linear play from long before. The 8th chunk
reset the effects (`fx.reset_all()`), each event's chain pre-rolled `max(tail, 100 ms)` from the
timeline, and the store kept it and the eight after it as THE second lap; every later lap replays
that recording.

Measured on the relinked copy of his project (`repro/26.11.09/haski-relinked.kt`, event 2133:
Slowed+Reverb with a rate curve, VHS Tape (sound), Datamosh (sound)), the same sequence through
`MixPlay` (a temporary `kt-cli` subcommand, removed): before, the kept second lap matched the
continuous play from the seek in 742 of 15606 samples, −28 dB of difference that does not decay lap
over lap (the warp's resampler restarted at another phase); after, 15606 of 15606, and every later
lap equals it. The heard loudness of the lap head was within 1 % either way on this machine's copy:
what he heard as "hollow" on his own state we could not reproduce offline (his project has changed
since the copy), but the mechanism — a kept lap that is not the continuous play's — is the defect
the trace shows, and it is closed exactly.

## Decisions

### D1. Bring the effects back, do not just stop keeping (the task offered both)

Two ways were open: (a) keep nothing until the chain has settled after the reset; (b) warm the
chain by a discarded play before the chunk. (a) alone leaves the live sound after the reset wrong
for the settle length (a loop shorter than a reverb's tail would play several wrong laps live
before keeping), and what it keeps after settling is still not the continuous play's bytes (the
resampler's phase never converges: −28 dB for ever on his event). (b) is what the engine already
does for a reel's move (`NestFetcher`: a canonical move from the block boundary minus the pre-roll)
and for a seek (`hard_seek`). So (b), in two forms:

- **Continued** — the replayed run is within the settle length: the effects never reset; the
  replayed frames are rendered again, discarded, from where the effects stopped. The kept chunk is
  the continuous play's, byte for byte (t2370, t2374). His case: 7168 frames.
- **Window** — longer: reset, then the settle length before the chunk on the player's chunk grid,
  through `render_looped_with` when the head is in the loop (t2372). The past a lap hears is the lap
  before it, not the timeline before the region. Cost: the settle length rendered once per exit
  (on his event 1.3 s of material, the reverb's tail at decay 0.84; a seek already pre-rolls that
  much per event).

(a) stays for what cannot be warmed (D3).

### D2. The settle length is asked of the timeline frames the chunk plays

`AudioRenderer::settle_frames(range)`: the events active there with a chain — `max(tail, 100 ms) +
look-ahead`, what `hard_seek` warms — and the track stages' `tail + look-ahead` when their lane is
heard. Under a loop the render head runs on linearly; it is folded by `pieces` first (asking the
unfolded head found nothing past the region's first lap — t2372 caught it). A renderer-wide maximum
was rejected: a 10 s delay anywhere would make every exit render 10 s.

### D3. What cannot be warmed is not kept for the settle length

- a chunk with an underrun or a pre-roll miss: the hole rings on in the effects; before, the chunk
  itself was dropped and the next one kept (t2371);
- a warm-up that missed the source: the same;
- a seek where a track stage still rings: a stage has no pre-roll (`cold_after_seek`, t2373). The
  stage's own pre-roll was not added: it would change the bytes of every seek and every export range
  that starts inside a track's tail (found-defects 420).

`MixPlay::fx_reset(renderer, head)` now takes the seek's head and renderer for this.

### D4. The class walk: stored audio next to freshly started state

Mechanism: a state that restarts (a reset, a seek, a new stream) is joined to audio produced by
another history (a recording, a ring, a cache), and the joint is heard or kept. Every place:

| Place | Stored audio | Fresh state | Found |
|---|---|---|---|
| Final-mix store, exit after replay | kept chunks | `reset_all` + per-event pre-roll | **the defect**: fixed (D1) |
| Final-mix store, a starved chunk | the next chunks | effects that took in silence | **kept a hole's echo**: fixed (D3) |
| Final-mix store, after a seek | chunks after it | track stage with no pre-roll | **kept a cold stage**: fixed (D3); the stage's pre-roll itself → row 420 |
| Loop laps of a context chain | the ringing laps | `start_lap` cold processor | correct: a lap is a seek plus the earlier laps (t1440); `reset_all` drops them, the window warm-up rebuilds them |
| Sidechain key branch | — | not reset by `reset_all` | **a seek carried the old place into the detector**: fixed (t2375) |
| Nested reel's sub-mix ring | bursts by index | canonical move | **a reel's track stage got no pre-roll**: fixed (t2376) |
| Reverse / varispeed | the ring | `render_frames_raw` is dry; a direction change resets and flushes | none: no effect state, the store is not read (`OUTSIDE`) |
| Scrub | — | the program has no audio scrub (reverse transport only) | none |
| Punch-in | — | no recording in the program; "punch-out" is an edit | none |
| Export pre-roll | — | per-event `hard_seek` at the range start | same as a seek: a track stage starts cold at a range start → row 420 |
| The UI's probe renderer (`sample_levels_at`) | — | its own store, a level reading | none (not heard, not kept) |
| A freshly built track chain | — | `TrackProcessor::new` differs from a reset one (t2375's first form found it) | → row 421 |
| The output device (part 2) | the ring | a stream opened on Play | the ring is kept across close/open; a flush while closed is drained by the output thread (D6) |

## Part 2: the output device opens on Play

### D5. An output thread owns the stream

`cpal::Stream` is not `Send`, and opening one blocks (row 274's reason it was left). A thread of its
own (`audio-output`) owns it: it parks on the player's `Wake` (the notify count the render thread
parks on) with no clock, opens the stream when the transport plays, and drops it once the transport
has stood for `CLOSE_AFTER_REST` (1 s: the device's own buffer has played out long before; a second
spares a reopen to a Space held and released, the space_hold scenario). The UI thread never opens
a device.

The ring's consumer lives in the callback while the stream is open and comes back to the thread
when cpal drops the callback (`ReturnOnDrop` sends it through a channel from its `Drop`): nothing in
the callback locks or allocates (invariant 6). The output edge (rate and channel map) is built per
open, before the stream, for the device it opens.

### D6. Time stands before the first frame, a flush is never lost

With no stream nothing consumes, so the playhead stands until the first callback hands out frames —
invariant 6's exception, the same as a start waiting for its sound. A seek on a pause sets the flush
flag while the stream is closed; the output thread drains the ring itself when it wakes (a seek
notifies), so the first callback after an open never drops the new start's pass.

### D7. Measured, traced, reported

`play` records with aux `device_open` (`b` the open's µs, `c` open → first callback µs),
`device_close`, `device_error`. An open error reaches the bell once per Play. Numbers: see tasks 4.x.
