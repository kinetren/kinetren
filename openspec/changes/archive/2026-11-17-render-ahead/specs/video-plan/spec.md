## ADDED Requirements

### Requirement: Playing, the display presents finished frames and one producer renders ahead
While the transport plays and the player's render-ahead producer has an executor, the preview's display
SHALL only present composites the display's executor already keeps: a moment asked that is not kept SHALL
give no picture (the preview keeps its last full picture under Loading), run no plan, ask no decode and
wake the producer. The on-demand composite SHALL remain for a paused transport, scrubbing and seeking, for
an ask without a display target (the bytes path), and for a player whose producer has no executor.
`KT_RENDER_AHEAD=off` turns the producer off and restores the on-demand composite while playing (an A/B of
one binary).

The producer SHALL be one `near` service of the scheduler. Each pass SHALL look from the frame on screen
plus its measured lead (the wall a moment takes, at least one frame) forward: inside a loop region round
the region (every moment of the lap); outside one forward until the first moment whose layers are not all
decoded. It SHALL skip moments the display's executor holds (noting them for the coverage bars) and moments
skipped this lap — one that came out incomplete (a stream stage short of its frame included) or that the
display's memo did not keep — and compose the rest in that order, keeping each complete composite on the
display's executor under its plan key. It SHALL ask no decode and withdraw no request. Nothing SHALL run
while nothing plays.

The producer SHALL compose on an engine of the display's own device (its own lock), on a card and on a
software rasterizer alike, and the composite SHALL reach the display's present memo on the device, with no
readback, no upload and no wait for the device; the producer learns that the device finished a moment from a
done flag the device sets. Only where the display's executor has no engine of its own device (or
`KT_RENDER_AHEAD_DEVICE=side`) it SHALL compose on the offline device's side engine and hand the bytes to the
display's executor. The moments in flight at once (the depth) SHALL be measured, never a constant of
one machine: the wall a moment takes from its plan to its completion over the producer's own time per
moment, at least one, at most the moments whose bytes the memory ledger has room for. A kept composite's
bytes SHALL equal the bytes the display's own `present` would have kept for the same plan.
Why: `docs/architecture-review-26.11.17.md` §1–§2, R2 — on the owner's iGPU the loop fill's moment took ~174
ms of wall for ~12 ms of device, the card was 4–26 % busy while frames were missing, and a VHS Tape loop the
card could compose in real time took seven laps; the owner raised the bound and asked for computing ahead
(kkt-26.11.17 S4, kkt-26.11.11 L4). Found-defects 251 — a display slower than the project's frames kept only
the moments it showed.

#### Scenario: A loop shown in part becomes all-kept
- **WHEN** a loop of 24 moments plays with its frames decoded and a display that presents only every fourth moment it reaches, for three laps, with a producer executor and with `KT_RENDER_AHEAD=off`
- **THEN** with the producer every moment of the loop is kept by the display's executor by the third lap and the display's answers of the last lap are all kept composites; without it a part stays unkept
- **TEST** t2334 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: The producer asks no decode and rests with the player
- **WHEN** a loop plays with half its frames not decoded, then the play stops
- **THEN** the producer composes only moments whose frames are decoded, the workers received no request from it, and after the stop it runs no job
- **TEST** t2335 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: The producer skips what the memo does not keep
- **WHEN** a loop plays whose moments the display's memo refuses past the first eight
- **THEN** the producer keeps eight, skips the rest for the lap, and composes nothing more until the next lap
- **TEST** t2336 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: On a card several moments are in flight on one engine
- **WHEN** a loop of 24 moments plays with a producer executor that reports a hardware device, returns from each moment at once and finishes it 40 ms later (a device's done flag)
- **THEN** at least two moments are in flight at once, no moment is composed twice in a lap, and at least half of the device latency the same executor pays when every moment is waited for is hidden
- **TEST** t2920 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: The depth comes from measurement and memory
- **WHEN** the depth is asked for a moment's wall and the producer's own time per moment, with and without a memory room
- **THEN** it is the wall over the own time rounded up, at least one, one for a synchronous moment, and never more moments than the room holds
- **TEST** t2921 (`src/libs/kt-engine/src/render_ahead.rs`)

#### Scenario: A moment kept on the device is the display's own composite
- **WHEN** a heavy chain's plan is kept on a display engine by a sibling of it (on the device), by another engine (bytes), and presented by the display engine itself
- **THEN** the sibling's path made no readback, all three keep the same key, and the kept composites read back equal bytes
- **TEST** t2922 (`src/libs/kt-gpu/tests/render_ahead.rs`)

#### Scenario: Playing, the display shows only kept moments
- **WHEN** a player with a producer executor plays, and the display asks for a moment not kept and then for one kept; then the player pauses and asks the first moment again
- **THEN** playing, the first answer has no picture and ran no plan on the display's executor, the second is the kept composite; paused, the moment is composed on demand
- **TEST** t2925 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: Measured on a software rasterizer
- **WHEN** `kt-cli preview <showcase> 70 74 --size 640x360 --loop 40` runs on llvmpipe (release) on the stage-18 base and on this change, each recorded with `--trace`, and `kt-cli trace loop` reads each run
- **THEN** both runs and the device's busy share while frames were missing are in the change's design record
- **TEST** bench `kt-cli preview --loop` + `kt-cli trace loop`

#### Scenario: Loop playback in the program
- **WHEN** the live scenario opens a project with a heavy effect, sets a loop region and plays it for several laps
- **THEN** the preview shows the loop's frames (no lasting Loading after the first laps), the coverage bar over the region reads kept, and the process rests after the stop
- **TEST** t2927 (`src/scripts/live/render_ahead.py`)
- **PATH** open the project → drag over the ruler above the first event to set a loop region → press Space → the preview plays the region round and round; after the first laps the region's bar turns bright → press Space → the preview stands on its last frame

## MODIFIED Requirements

### Requirement: A ring that carries a display holds at most one bounded piece in front of it
On a ring that carries the editor's display device, the lower-class work (every class below `now`) that
the display's NEXT submission would find in front of it SHALL stay under the ring's bound. The ring
predicts its queue: every admitted submission carries its predicted device time and, once submitted, its
submit instant, and the queue drains in submission order (a submission past its predicted end and not yet
done ends now); the display's next submission is expected one frame period after its last while it
presents, else now. A lower-class submission SHALL be admitted while the predicted drain with it ends no
later than the bound past the later of the display's next submission and the end of the display's own frame
in flight; every lower class has the same bound, and no count of lower submissions limits it. A `now`
submission keeps one bound of predicted time in flight, and one piece is always admitted on an idle ring.
The bound is ONE frame period of the preview's project (40 ms at 25 fps), a general rule; a lower-class
frame or compute job SHALL be cut at pass (dispatch) boundaries into pieces whose predicted device time
stays under the bound (a pass longer than that is a piece of its own). A ring with no display neither bounds
nor cuts: an export's ring of frames and a software device keep their pipelining. The cut frame's bytes
SHALL equal the uncut frame's.
Why: row 149 — on a real card the display and the offline device share the graphics ring, and one
long submission held the display. Found-defects 480 — waiting for the earlier piece's completion cost a
Datamosh walk 96–98 % of its wall on his iGPU. The bound was half a frame period until 2026-11-17; two
designs called it "the owner's latency limit", which he never set (gpu-arbiter D3: "a general rule … the
owner may want to revisit"). Asked with context, he raised it ("Поднять", kkt-26.11.17 S4): a lower piece
may delay a displayed frame by up to one period, and while playing the display presents kept frames only.

#### Scenario: The in-flight bound applies only with a display
- **WHEN** a ring with a display holds pieces whose predicted drain reaches the bound, and the same pieces wait on a ring without one
- **THEN** the displayed ring admits the next only when the drain leaves room for it (one piece always fits an idle ring), the other admits them all
- **TEST** t2251 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: A long frame is cut on a displayed ring with the same bytes
- **WHEN** a heavy chain (VHS Tape, Camera ISP) is executed twice at 1080p by an offline engine on a device that also carries a display, once warm with its passes measured
- **THEN** the measured run is submitted in more than one piece, no piece but a single pass predicted over the bound, and its bytes equal the uncut run's
- **TEST** t2253 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: The display's latency on a shared device is bounded
- **WHEN** the preview presents a light frame every frame period on one device while an offline heavy chain and a Datamosh history run on the same device, with the arbiter and without it
- **THEN** with it the display's frame waits for at most one bound of lower work (its latency stays below the offline frame's own device time); without it the display waits for whole offline frames
- **TEST** t2254 (`src/libs/kt-gpu/tests/arbiter.rs`), bench `kt-cli gpu-share`

#### Scenario: The compute host admits the frame on screen first
- **WHEN** a near-class Datamosh job runs on the compute host and a near and a now job wait
- **THEN** the now job is admitted next, and every job's admission is in the trace with its class
- **TEST** t2255 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: A lower class keeps a piece queued while one runs
- **WHEN** a displayed ring idle of display work takes lower-class pieces of a quarter of the bound each, none completing
- **THEN** four are admitted at once (their drain fits the bound from now), and the fifth waits until the drain has room
- **TEST** t2430 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: Every lower class keeps the bound in front of the display
- **WHEN** the display presents frames costing a quarter period, twice the period, and then stops, and a near, a background and an offline piece of three quarters of the bound ask in turn
- **THEN** each class is admitted with the same room, the bound past the display's next present; none has a quarter bound or a slack of its own
- **TEST** t2923 (`src/libs/kt-gpu/src/arbiter.rs`)

### Requirement: The display's present is counted on its ring
The display's present (a composite, or a kept composite's display pass) SHALL enter its ring's flight as a
`now` submission predicted at its measured device time, without waiting for admission, and leave it when the
device completes it; the ring SHALL know when the display last submitted.
Why: found-defects 480 — the display's 32.5 ms composite on his card was invisible to the ring.

#### Scenario: The display is counted, never admitted
- **WHEN** a lower piece holds the bound and the display presents, then the display presents behind its frames
- **THEN** the present enters the flight with no wait; a lower class may queue what drains before the display's next present plus one frame period and not more; behind a late display frame a lower piece waits only for the bound past that frame's end
- **TEST** t2433 (`src/libs/kt-gpu/src/arbiter.rs`)

### Requirement: A lower-class piece is admitted against the time in flight
On a bounded ring, a lower-class frame or compute job cut into pieces SHALL submit a piece and go on
encoding the next without waiting for the piece's completion; the next piece SHALL be admitted by the
ring against its predicted drain (the device time still ahead of each submission in flight, the display's
present included), waiting only until the drain has room or a higher class waits. Where a submitted piece
of a device without a surface is in flight, the waiter SHALL wait for the oldest one's completion and look
again the moment it comes; with none (a surface device's pieces, pieces still being encoded) it SHALL wake
when its claim fits by the prediction. A piece whose time is a guess (a pass never measured) SHALL make room
only by its completion, never by the clock. A ring with a display still never holds a piece of several
passes predicted over the bound in front of the display, and the cut frame's bytes equal the uncut frame's.
Why: found-defects 392 — `Piece::cut` waited for the device to finish each piece before admitting the
next: ~45 % of a 720-line Datamosh walk in his trace was the arbiter's own waits. Found-defects 480 — counted
by completions, a piece queued behind the display's frame held the ring for the whole frame. Stage 11's
finish.sh (t2435, t2436 red under the full parallel run, green alone): a guessed time drained by the clock,
so on a loaded machine the next piece went before the guessed one's timestamps came back and nothing was
learned; a waiter sleeping on a timer saw a completion only at its own next wake, and the device stood
idle between pieces. render-ahead (2026-11-17): the count of three lower pieces is gone with the half-frame
bound; the predicted drain alone bounds what stands in front of the display.

#### Scenario: A cut goes on while the bound has room
- **WHEN** a compute ring with no display and a ring with a display each take a lower-class job cut into pieces whose predicted times fit the bound together, and pieces that do not
- **THEN** the pieces that fit are admitted without waiting for the earlier piece's completion; the ones that do not wait until the drain has room; t2253 holds
- **TEST** t2356 (`src/libs/kt-gpu/src/arbiter.rs`), t2253 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: A waiter is admitted when the predicted drain makes room
- **WHEN** a displayed ring holds a submitted lower piece predicted at the whole bound, never completing, and another lower piece of half the bound waits
- **THEN** the waiter is admitted once half the earlier piece's predicted time has passed, with no completion
- **TEST** t2431 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: A guess makes room only by its completion
- **WHEN** a displayed ring holds a submitted lower piece whose time is a guess at the whole bound, never completing, and another lower piece of half the bound waits for twice that time
- **THEN** the waiter is not admitted by the clock; it is admitted when the earlier piece completes
- **TEST** t2441 (`src/libs/kt-gpu/src/arbiter.rs`); t2435 and t2436 (`src/libs/kt-gpu/tests/compute_start.rs`) hold under a full parallel run

### Requirement: The prefetch advances a run of stream frames as one walk
The prefetch thread SHALL advance a stream stage's frames ahead of the frame on screen as one walk to the
last frame of each run that does not wrap, not as one walk per frame; the walk reaches the frame on screen
first. The run has a NEAR part, while playing: the frames the display reaches before the next pass, twice
over, at class near; and a FAR part at class background: with the playhead inside a loop region, to the
region's end and round from its start, as far as the stream memo holds the positions; without one, while
playing, up to a quarter of the memo ahead. On a pause inside a loop region the far part runs too; on a pause
outside one, only the frame on screen. Both parts skip kept moments, stop at the first position whose source
is not decoded, and stop when the transport or the project leaves them. A far part left pending SHALL run
again on the next pass only while it walks or while the decoded-frame cache gained a video frame since the
pass before; one that walked nothing over a pass that decoded nothing waits for the pass state to move. The player SHALL tell the stream memo its view (the loop region's start while the playhead is inside
it, else the frame on screen), from which on a walk keeps every picture.
Why: found-defects 391 — one advance per frame made every position its own submission while playing.
Found-defects 482 and the owner's 11.11 L4 ("я не против чтобы оно наперед считало") — twice the display's
reach was ~100 positions a lap of his 715-position loop, six laps to finish it. render-ahead D6: the eight
retries of `FAR_STALLS` were a count of one machine; a decoded frame is what lets a stalled walk move.

#### Scenario: Frames ahead are one walk
- **WHEN** a mosh is advanced from the frame on screen through eight frames ahead
- **THEN** the history is computed by at most ⌈9 / batch⌉ + 1 jobs, and every one of the nine frames then needs nothing
- **TEST** t2360 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A loop is computed to its end, playing or paused
- **WHEN** a loop region over a mosh is set with every source frame cached, and the prefetch's advance runs once with the playhead at its start, paused and playing
- **THEN** the far part reaches the region's last frame in that pass (no frame of the region needs anything after it), ran at class background, and a pass with the history complete runs nothing
- **TEST** t2432 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A stalled far walk waits for a decoded frame
- **WHEN** a far walk is left pending after a pass that walked positions, one that walked none while the cache gained a frame, and one that walked none while the cache gained nothing; and video frames and an audio burst go into a cache
- **THEN** the first two run again on the next pass, the third stops until the state moves; the cache's count moves by every video frame and not by audio
- **TEST** t2924 (`src/libs/kt-engine/tests/decode_storm.rs`)

## REMOVED Requirements

### Requirement: A playing loop's unshown moments are composed below the display
**Reason**: The loop fill (lanes, a width from the scheduler's pool and `RING_LOWER_PIECES`, claims, narrow
mode after an incomplete moment, the display gate on a software device, `KT_LOOP_FILL*`) is replaced by the
render-ahead producer, which composes the same moments and those of linear play, on the display's device
on a card, with a measured depth (owner, kkt-26.11.17 S4; review R2).

**Migration**: "Playing, the display presents finished frames and one producer renders ahead". t2334–t2336
move to `tests/render_ahead.rs` with the same scenarios; t2860–t2862 are replaced by t2920 and t2921 (the
reasons are in render-ahead's design).

### Requirement: A saturating display keeps near work at the bound
**Reason**: The rooms by class (a quarter bound for `visible`/`background`, the display's slack for an
export) and `AHEAD_PIECES` are deleted with the half-frame bound (owner: "Поднять", kkt-26.11.17 S4).
While playing the display presents kept frames, so its present no longer saturates the period.

**Migration**: every lower class keeps the bound of one frame period ("A ring that carries a display holds
at most one bounded piece in front of it", t2923); t2517 is removed.
