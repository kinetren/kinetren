## MODIFIED Requirements

### Requirement: A ring that carries a display holds at most one bounded piece in front of it
On a ring that carries the editor's display device, the lower-class work (every class below `now`) that
the display's NEXT submission would find in front of it SHALL stay under the ring's bound. The ring
predicts its queue: every admitted submission carries its predicted device time and, once submitted, its
submit instant, and the queue drains in submission order (a submission past its predicted end and not yet
done ends now); the display's next submission is expected one frame period after its last while it
presents, else now. A lower-class submission SHALL be admitted while the predicted drain with it ends no
later than the bound past the later of the display's next submission and the end of the display's own frame
in flight; at most three lower-class submissions of a ring SHALL be uncompleted at once. A `now`
submission keeps one bound of predicted time in flight, and one piece is always admitted on an idle ring.
The bound is half a frame period of the preview's project (20 ms at 25 fps), a general rule; a lower-class
frame or compute job SHALL be cut at pass (dispatch) boundaries into pieces whose predicted device time
stays under half the bound, so one piece runs while the next is queued (a pass longer than that is a piece
of its own). A ring with no display neither bounds nor cuts: an export's ring of frames and a software
device keep their pipelining. The cut frame's bytes SHALL equal the uncut frame's.
Why: row 149 — on a real card the display and the offline device share the graphics ring, and one
long submission held the display. Reproduced on llvmpipe by running the preview and the offline work
on ONE device. Found-defects 480 — waiting for the earlier piece's completion, which stood behind the
display's own uncounted 32.5 ms composite, cost a Datamosh walk 96–98 % of its wall on his iGPU: 16 ms of
device time a position in 106 ms.

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
- **THEN** with it the display's frame waits for at most one bound of lower work (its latency stays below the offline frame's own device time, and its p95 is not worse than with the completion-per-piece rule); without it the display waits for whole offline frames
- **TEST** t2254 (`src/libs/kt-gpu/tests/arbiter.rs`), bench `kt-cli gpu-share`

#### Scenario: The compute host admits the frame on screen first
- **WHEN** a near-class Datamosh job runs on the compute host and a near and a now job wait
- **THEN** the now job is admitted next, and every job's admission is in the trace with its class
- **TEST** t2255 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: A lower class keeps a piece queued while one runs
- **WHEN** a displayed ring idle of display work takes lower-class pieces of a quarter of the bound each, none completing
- **THEN** pieces are admitted while their predicted drain fits the bound from now (more than one in flight at once), and the next waits only until the drain has room; never more than three are uncompleted
- **TEST** t2430 (`src/libs/kt-gpu/src/arbiter.rs`)

### Requirement: A lower-class piece is admitted against the time in flight
On a bounded ring, a lower-class frame or compute job cut into pieces SHALL submit a piece and go on
encoding the next without waiting for the piece's completion; the next piece SHALL be admitted by the
ring against its predicted drain (the device time still ahead of each submission in flight, the display's
present included), waiting only until the drain has room, a higher class waits, or three lower pieces are
uncompleted. The waiter SHALL wake when its claim fits by the prediction, not only at a completion. A ring
with a display still never holds a piece of several passes predicted over the bound in front of the
display, and the cut frame's bytes equal the uncut frame's.
Why: found-defects 392 — `Piece::cut` waited for the device to finish each piece before admitting the
next: ~45 % of a 720-line Datamosh walk in his trace was the arbiter's own waits. Found-defects 480 — counted
by completions, a piece queued behind the display's frame held the ring for the whole frame.

#### Scenario: A cut goes on while the bound has room
- **WHEN** a compute ring with no display and a ring with a display each take a lower-class job cut into pieces whose predicted times fit the bound together, and pieces that do not
- **THEN** the pieces that fit are admitted without waiting for the earlier piece's completion; the ones that do not wait until the drain has room; t2253 holds
- **TEST** t2356 (`src/libs/kt-gpu/src/arbiter.rs`), t2253 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: A waiter is admitted when the predicted drain makes room
- **WHEN** a displayed ring holds a submitted lower piece predicted at the whole bound, never completing, and another lower piece of half the bound waits
- **THEN** the waiter is admitted once half the earlier piece's predicted time has passed, with no completion
- **TEST** t2431 (`src/libs/kt-gpu/src/arbiter.rs`)

### Requirement: The prefetch advances a run of stream frames as one walk
The prefetch thread SHALL advance a stream stage's frames ahead of the frame on screen as one walk to the
last frame of each run that does not wrap, not as one walk per frame; the walk reaches the frame on screen
first. The run has a NEAR part, while playing: the frames the display reaches before the next pass, twice
over, at class near; and a FAR part at class background: with the playhead inside a loop region, to the
region's end and round from its start, as far as the stream memo holds the positions; without one, while
playing, up to a quarter of the memo ahead. On a pause inside a loop region the far part runs too; on a pause
outside one, only the frame on screen. Both parts skip kept moments, stop at the first position whose source
is not decoded, and stop when the transport or the project leaves them. The player SHALL tell the stream memo
its view (the loop region's start while the playhead is inside it, else the frame on screen), from which on
a walk keeps every picture.
Why: found-defects 391 — one advance per frame made every position its own submission while playing.
Found-defects 482 and the owner's 11.11 L4 ("я не против чтобы оно наперед считало") — twice the display's
reach was ~100 positions a lap of his 715-position loop, six laps to finish it.

#### Scenario: Frames ahead are one walk
- **WHEN** a mosh is advanced from the frame on screen through eight frames ahead
- **THEN** the history is computed by at most ⌈9 / batch⌉ + 1 jobs, and every one of the nine frames then needs nothing
- **TEST** t2360 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A loop is computed to its end, playing or paused
- **WHEN** a loop region over a mosh is set with every source frame cached, and the prefetch's advance runs once with the playhead at its start, paused and playing
- **THEN** the far part reaches the region's last frame in that pass (no frame of the region needs anything after it), ran at class background, and a pass with the history complete runs nothing
- **TEST** t2432 (`src/libs/kt-engine/tests/decode_storm.rs`)

## ADDED Requirements

### Requirement: The display's present is counted on its ring
The display's present (a composite, or a kept composite's display pass) SHALL enter its ring's flight as a
`now` submission predicted at its measured device time, without waiting for admission, and leave it when the
device completes it; the ring SHALL know when the display last submitted.
Why: found-defects 480 — the display's 32.5 ms composite on his card was invisible to the ring.

#### Scenario: A present is in flight until it completes
- **WHEN** a displayed engine presents a measured chain and its ring is read before and after the device completes it
- **THEN** the flight holds a now submission of the frame's predicted time with no admission wait, and nothing after completion
- **TEST** t2433 (`src/libs/kt-gpu/tests/arbiter.rs`)

### Requirement: A pass never measured is predicted
A render pass or a compute dispatch SHALL be predicted from the device time measured for it on this device:
in this process, or — for one never measured in it — from a table kept on disk keyed by the adapter's name
and driver, the pass's label and its work size (scaled linearly from the nearest measured size of the same
label). A compute job SHALL learn its dispatches' times from its own first pieces while it runs. The table is
a cache: it changes where frames are cut, never their bytes.
Why: found-defects 481 — a pass never measured claimed the whole bound, so the first walk was one dispatch a
piece (267 in 267).

#### Scenario: A new process predicts what an earlier one measured
- **WHEN** pass times are noted for an adapter, saved, and a fresh table loads the file; a size never measured is asked, and another adapter asks the same label
- **THEN** the fresh table predicts the noted times, the unmeasured size from the nearest measured one scaled by work, and nothing for the other adapter
- **TEST** t2434 (`src/libs/kt-gpu/src/seeds.rs`)

#### Scenario: A cold walk is cut by what its first pieces measured
- **WHEN** a compute job of many dispatches of four names, never measured, runs on a bounded ring, then the same job again
- **THEN** the first run is cut into fewer pieces than half its dispatches, and its bytes equal the second's
- **TEST** t2435 (`src/libs/kt-gpu/tests/compute_start.rs`)

### Requirement: Work a person waits for uses the machine
Work a person waits for — the catch-up of a stream history, the Loading of a paused frame, a loop's first
laps, a render or export, thumbnails and strips he scrolls to, an analysis he started — SHALL use the idle
device and the idle cores within the limits the owner set: the display first (the ring's bound), and rest
costs nothing (no work at rest, invariant 12). A path that can split its work without changing a byte SHALL
split it across cores; a CPU executor SHALL take a device's work only where its bytes equal the device's,
or by a choice that does not follow the load (invariant 11). It is measured from pass timestamps and thread
time, not wall-clock guesses.
Why: found-defects 485 — the owner, kkt-26.11.10b K3: "не грузит проц - ленится как будто опять или
упирается в одно ядро иногда, у нас нет спек на эту тему??"; 11.11 L4: "и чтобы и процессор тоже
использовался, для тех же вычислений".

#### Scenario: A walk keeps the device fed
- **WHEN** a cold Datamosh walk of many positions runs at class near on a compute ring that carries a display with no display traffic
- **THEN** from the trace's per-piece device starts and ends, the device stands idle between consecutive pieces for less than a fifth of the walk's device span, and the walk's admission waits are under half its wall
- **TEST** t2436 (`src/libs/kt-gpu/tests/compute_start.rs`)
