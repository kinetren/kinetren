# Design: gpu-leftovers

## Context

Eight found-defects rows, each with a diagnosis and a suggested fix (proposal). The machine has no card:
llvmpipe/lavapipe only. Every test added here is arithmetic, structure (counters, forms, admissions) or
bytes; none reads a duration, because on a software device the device's time is the processor's and
stage 11's timing tests went red under load (t2435, t2436, t2255).

## D1. The compute host's submissions and bindings (511, 520)

The clock calibration (one empty compute pass with a timestamp, three brackets the first time) and the
warm-up dispatch of `prepare` are admitted on the compute host's ring like every job piece: by the caller's
class, with a claim of 20 µs / 100 µs. The calibration's bracket is taken AFTER the admission, so a wait
for room never widens it. The ring counts what it admitted (`Ring::admitted`, t2510).

A job writes every step's uniform into one arena (slots aligned to the device's
`min_uniform_buffer_offset_alignment`) with ONE `write_buffer`, and binds a step's slot by a dynamic offset.
A bind group serves every step with the same shader, slot size and buffers within the job: a 267-dispatch
Datamosh walk of two kernels makes two bind groups, not 267, and one pooled buffer, not 267 (t2511: 240
dispatches over two shaders, each with its own uniform, give the exact sum and two bind groups). Bind groups
are not kept across jobs: every job takes its buffers from the pool, so a group of the last job names other
buffers. Rejected: a bind group per (shader, buffers) kept on the host — the pool hands out different
buffers per job, so the keep would almost never hit and would hold buffers alive.

## D2. The run form is a function of the shape's state, and the keys name it (510)

The trial used to alternate the OUTPUT between the fused pass and the own passes, and the readback memo, the
present memo and the export kept whichever ran. On a card the two forms are within 1 LSB a boundary, not
equal. Now:

- A frame's output form is a function of its shape's state alone: own passes while the shape is pending,
  untried or trying; the verdict once it is in (fused, or split into halves that start again); a device
  without timestamps runs fused once warm. The trial times the fused pass as a SHADOW pass: it runs beside
  the own passes into a scratch target and is thrown away; both are timed in the same frame (the pass
  timer sums a piece per form). Five frames of a shape pay the extra pass, and the trial is better
  interleaved than before (both forms in one frame under the same load).
- `compiled::forms_revision()` — one process-wide counter — moves whenever any shape's output form can
  change: a shape compiled (by the worker or a settled frame), a verdict, a trial that ended fused for lack
  of samples. `plan_key` is the plan's hash mixed with it, so the readback memo and the present memo (and the
  slow route's `keep` into the display's memo, keyed by the same function) never serve a composite kept
  under other forms. A composite whose resolve saw the revision move is not kept at all. Process-wide, not
  per device: the slow route keeps another device's composite under the display's key, and a revision that
  differs between the two engines would make that keep unreachable. The moves are rare (a shape's life has
  two or three), so the misses they cause are a few re-composes at a session's start.
- t2514: a trial never changes what a frame shows; t2515: a plan served from the memo is composed again once
  the revision moves.

Rejected: asserting equality on every adapter (the suggestion's other half) — unverifiable here and a
promise about compilers we do not control.

## D3. An export composes runs in the SETTLED form (510)

`execute_once` (the serial export, now — it called the memo path `execute_into`) and `submit` (the export's
readback ring) resolve with `settled`: every run of two or more stages runs its generated pass, compiled in
the frame when the worker has not compiled it yet (`Compiled::run_settled`), with no trial and no split. A
text the device refuses runs as its own passes, deterministically. So an export's bytes are a function of
its plan and its device, whatever the editor tried before (t2516: a shape no frame met runs fused in the
export's first frame and the same bytes every repeat; on llvmpipe they equal the own passes'). An export
already settles the worker before its first frame (`settle_prewarm`), so the in-frame compile is the rare
case of a shape the prewarm did not reach. Rejected: the own passes in an export (deterministic too, but it
gives back the memory traffic fused-pointwise-runs saved); the verdict persisted on disk (an export before
and after the first trial on a device would still differ).

## D4. A lower class gets the display's slack (491)

`note_display(ns)` keeps an average of the display present's predicted (= last measured) cost. While the
display presents, what a lower class may leave in front of its next present is `room(class)`: the part of
the period the display leaves free, at most the bound; for `near` (what feeds the display) at least a quarter
bound; for `answer` the bound. A display that fills its period (his iGPU: 32.5 ms at 16.7 ms) therefore
leaves `visible`, `background` and `offline` nothing until it stops presenting, instead of one bound a frame
that only made the display later; `near` keeps a quarter bound so the stream walk the display waits for
still moves. t2517 is arithmetic on a fixed instant. t2433's last assertion held the old rule (a whole bound of near work
admitted behind a display late by four periods); it now holds this one — a quarter bound admitted, a whole
bound and any background piece refused — the change row 491 asked for. Not measured on a card (none here): the row's own
verdict "needs a card's trace" stands for the measurement, row 560.

## D5. An export never writes a frame incomplete for its own reasons (522)

catch-up-shows-loading D5 said the export retries a frame whose blend pair lacked a side; it did not: the
serial export's pre-roll asked a missing frame once, and the pipelined export asked a key the span dropped
once. Now every export asks a frame its span did not give alone up to `RETRIES` (2) times, and a frame that
still comes out incomplete has its missing frames decoded alone again (serial) and is composed again (both
exports, and the readback ring's collected frames), `RETRIES` times. What is still missing after that is the
input's own answer, given the same way twice from the same cache: it is written without the layer and named,
as t1602 and t1606 hold (the owner's contract: a damaged file costs its frames, not the render). t2518: a
source whose every frame fails its first two reads exports with nothing lost, serially and pipelined, the two
files the same.

## D6. The strip row's key carries a version (524)

`strip_key` = the media fingerprint mixed with `STRIP_ROW_VERSION` (2). Rows written before — some with the
empty generator cells t2455 fixed — are misses and render again once; one version per reel, so the new row
replaces the old. Rejected: treating a row with an empty cell as a miss — an empty cell is also a legitimate
frame the reel could not compose (catch-up-shows-loading D6), and such a reel would render at every opening.
t2519.

## The class walk

### Every submission outside the ring
| Where | Before | Verdict |
|---|---|---|
| compute `calibrate` | not admitted | admitted (D1) |
| compute `prepare` warm-up | not admitted | admitted, the warm thread's class |
| compute `run` pieces | admitted (`Piece`) | unchanged |
| engine `execute`/`submit`/`present`/`present_kept` | `Piece` / `note_display` | unchanged |
| `warm.rs` (pipeline warm draws) | not admitted | short by construction, after `wait_frames`; gpu-arbiter recorded it; unchanged |
| engine `read_target` (`read_display`/`read_glow`) | not admitted | tests and exit paths only; unchanged |
| `pass_time.rs` submit | test code | not the class |
| egui's own frame on the display device | not ours | counted through `note_display` timing only |

### Every per-dispatch or per-pass allocation
| Where | Found | Verdict |
|---|---|---|
| compute `run`: uniform buffer + `write_buffer` + bind group per dispatch | yes | D1 |
| compute `run`: staging/learn buffers | per job/piece, pooled | unchanged |
| engine passes | arena + `Staged` + bind memo since local-cost-queue D3 | unchanged |
| `prepare` warm-up buffers | per shader once | not the steady state |
| `kt-plugin-api` generator compute frame (`generator.rs`) | one `Step` and uniform a frame | one dispatch a job: not the class |
| Datamosh/VHS job builders (plugins) | a `Step` per dispatch on the CPU (slices, no device objects) | CPU vectors, not device objects; unchanged |

### Every cache that may hold an incomplete result
| Cache | Found | Verdict |
|---|---|---|
| ReadbackCache | kept incomplete | D-523: complete only |
| PresentMemo | complete only (cached-range-plays) | unchanged |
| slow route `keep` into the display memo | `complete` only (player) | unchanged |
| reel strip disk tier | incomplete cells empty since catch-up D6; old rows stayed | D6 |
| poster (`compose_offline_frame`) | complete only since catch-up D6 | unchanged |
| FX picker cells | session RAM, the base frame comes ready; the look question is row 460 | unchanged |
| stream memo (history) | a history frame is the stage's own state at its position | not the class |
| kept final mix (audio) | a burst kept after its effects ran on what was there; audio has no absent layer, the decode either gives the burst or silence is rendered | not the class |
| thumbnails / peaks (sidecar) | pre-FX source frames | not the class |

### Every offline writer that may write incomplete
| Writer | Found | Verdict |
|---|---|---|
| serial export | read once; wrote and named | D5 |
| pipelined export (workers, ring `collect`, `depth == 0`) | read alone once; wrote and named | D5 |
| reel strip disk tier | D6 | D6 |
| poster disk | complete only | unchanged |
| kt-cli `render`/`preview` bench | through the export / the player | follow D5 / unchanged |

### Every result whose form timing picks (510's mechanism)
| Where | Found | Verdict |
|---|---|---|
| point-wise run fused/own (trial) | yes | D2, D3 |
| geometry shape specialized vs interpreter (`geom_specialized`) | pending → interpreter, then the shape: the same mechanism; t1766 holds the two equal on the adapter it runs on, a card is unverified here | recorded as row 561 |
| arbiter cuts | same passes, same bytes (t2253) | not the class |
| compute pieces | the buffers stay on the device; same dispatches | not the class |
| export engines count | the file does not depend on it (t1573) | not the class |

## Measured

None of these is a speed claim that a test asserts. On this machine (llvmpipe, load 17–21 from other
sessions) a speed reading means nothing; the compute host's arena removes 2 × 267 device-object creations and
266 queue writes from a 267-dispatch walk by construction (t2511 counts them).

At rest nothing new runs: no thread, timer or poll is added; the arbiter's room is arithmetic at an
admission, the compute arena and the calibration run only inside a job, the forms revision is a counter, and
an export's retries run only on an incomplete frame. The idle-load measurement of a live run was not repeated
here (no GUI in this session; the orchestrator's stage run covers it).
