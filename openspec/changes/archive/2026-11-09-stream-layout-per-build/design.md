# Design: stream-layout-per-build

## Context

Row 8 of the order table of `docs/engine-audit.md`. The owner's complaint (kkt-26.11.09, I2): a heavy
Datamosh on `problem_region_26_11_09` (event 2149 of his project: Camera ISP → Datamosh gop 66, 720
lines, block 8, search 64, `mv_rotate` drawn by a zigzag curve → VHS Tape; the event is 209 s long)
stays "Loading" for seconds, paused and on repeated laps. The stream stage's own share, measured
with `kt-cli stream-cost` (release, this machine's llvmpipe, the relinked copy's region, 1280×720
stream), the old and the new binary one after the other at the same load (load average 10–20):

| | before | after |
|---|---|---|
| the advancing thread, outside the jobs | 7.5–18.6 ms a frame | 2.1–3.4 ms |
| the advancing thread, inside the jobs (submission, copies) | 2.8–8.9 ms a frame | 2.1–4.0 ms |
| uploaded / read back | 10.0–10.6 / 17.2–18.3 MB a frame | 9.6–10.2 / 5.8–6.2 MB |
| the prefetch's 2 s window question (`wanted_frames`) | 12.2–19.9 ms | 2.2–3.6 ms |
| second lap of the whole region (667 positions) | 690 encodes again, 102 ms a frame | none: the loop is held (1577 MB memo) |

In the real player (`kt-cli preview … --size 640x360 --loop 240 --trace`, llvmpipe, the old binary
at load average ~32, the new one at ~100 — another session's build): the old binary's laps showed at
most 4 kept composites of 110–350 frames a lap; the new one's last laps showed 164 of 222 and 170 of
203 (the loop converging on what is kept); a composite's stage run 1.65 ms before, 2.8 ms after at
that load (9.8 ms while the output's conversion was a device job, D4). On llvmpipe the rest of the
chain (VHS Tape, Camera ISP: 65 ms a frame at 640×360) keeps a lap far from real time; whether the
loop becomes all-kept also depends on the present memo's constant 512 MB (found-defects row 310).

The jobs' device time does not move with this change (23–30 ms a frame at load 10, 72–83 at load 20 on
llvmpipe): it is the card's. A first run read 175 ms there at load ~49; the paired runs above are the
ones to compare.

## The mechanism (the class)

**A per-build constant recomputed per question.** A stage whose output at position N is a pure
function of the rows, the curves and the layout derives a structure from them — a GOP schedule, the
rows at every position, content keys — and the code derived it again at every call: per frame, per
audio block, per position of a window. The cost is O(questions × history), invisible on a short test
stream, and it multiplies: the prefetch asks once per frame of its 2 s window, a composite asks three
times, and each question walks the dependency chain back (up to 600 nodes) reading the curves at each.
Beside it, two per-element constants of the same kind: 32-bit words for 8- and 16-bit samples on the
device boundary, and a per-pixel colour conversion on the CPU in a stage whose math is on the device.

## Decisions

### D1. A compiled schedule, filled forward, owned by the host's build

`mosh::Plan` holds, per position from the stream's first: the rows with their curves, the reset edge,
the I-frame and dropped flags, the packet position (bloom), the node with the refresh applied, its
dependency, the raw chain index from its anchor, the broken decoder's key, the clean encoder's key and
GOP start per side, and the recurrences that make each O(1): the starts of the bloom and freeze runs,
the latest reset edge, the latest self-contained position, the bloom run's first P-frame. Every one
depends only on positions before it, so the plan is filled forward, lazily, up to the furthest
position asked, under a lock; questions read stored values.

- **Where it lives:** `StreamLayout::plan_cell` — a host-owned `PlanCell` (a `OnceLock` of an opaque
  `Arc`). The engine keeps one per stage and stream size in `StreamStage::plans`, replaced in
  `link_stages` on every build: the stage itself may be the last build's copy (edit-without-rebuild),
  and its neighbours (the event before, the second input) are linked there, so no edit can leave a
  stale schedule. A layout without a cell (tests, a host that gives none) compiles per call to the
  same values.
- **Why forward from the first position and not a walk with a memo:** the keys are prefix hashes
  along the chain and the GOP; a forward fill makes every key one `mix` from a stored one. The first
  question about a position far into a long event compiles everything before it once (209 s at 60 fps
  = 12.5 k positions, ~200 bytes each, ≈ 2.5 MB); the walk paid its chain on every question.
- **Same values:** t2270 pins the keys, needs and held positions of three streams (a cut mosh; every
  gate drawn per position with a second input of two events; a held mosh over 1400 frames) from the
  per-call walk, before the change; t2271 checks a scrambled order with and without a cell and that a
  cell reads each position's rows once.
- **Revision — chains longer than 8192 frames:** the walk cut a chain at `MAX_WALK` and counted the
  refresh grid from that cut, so in a mosh held for minutes the "anchor" moved with every frame and no
  memo entry was ever reused. The plan counts the grid from the true anchor. Below 8192 nothing changes
  (t2270's third stream is 1400 frames); above, the frames now continue each other as D3 of
  datamosh-codec-path intended.

### D2. The audio half keeps one plan

`MoshSound` keeps an `OnceLock<Arc<Plan>>`, dropped when `apply` brings other values, `bind` other
curves or `place` another placement. A codec frame shorter than a picture's frame asks a position at
`k1` — the plan is compiled to `k1` (the first attempt compiled to `k1 − 1` and read the static rows
there; t2272 caught it). t2272 pins the sound's bytes under every gate with a seek, from the per-frame
schedule.

### D3. Packed planes on the device boundary

The residual shader writes two 16-bit samples to a word (the even column of a pair, after a barrier);
the decoder reads them sign-extended and writes its picture four bytes to a word (a block's row is
whole words: 16, 8 or 4 samples at a multiple of itself). The CPU keeps `Residual` as `i16` and planes
as bytes, so a readback is a copy. Per 1280×720 frame: the encoder reads back 4.2 MB instead of
11.1, the decoder 1.4 instead of 5.5; the decoder uploads its residual at 2 bytes a sample.

### D4. The colour conversion is a compute step

`convert.wgsl` is the encoder job's first step when it is given RGBA (`codec::encode_rgba`): one
invocation per 8×2 luma samples writes two luma words per row and one word of each chroma plane — the
integer BT.601 of `Planes::from_rgba`, edge replication included; `from_rgba` stays as the reference
(t2274 compares packets and reconstructions at sizes that pad). The trade: the encoder uploads the
frame's RGBA (4 bytes a pixel) instead of planes (1.5), and the processor does no per-pixel work.

- **Rejected after measuring — the output's conversion on the device.** `rgba.wgsl` converted the
  shown picture back as a job of its own. In the real player over his region (llvmpipe, 640×360, a
  240 s loop, traces) a composite's stage run grew from 1.65 to 9.8 ms: the job queued behind the
  frame's VHS and Camera ISP passes on the same device, where the CPU's conversion of one picture is a
  millisecond and waits for nothing. Unlike the source's conversion it is not a step of a job that runs
  anyway. `Planes::to_rgba` stays on the CPU, once per shown frame (an advance converts none, D6).

### D5. The preview memo sized by the loop and the machine

`stream_memo_bytes(base, per_position, loop_positions, available)`: with a loop, every position of it
plus an eighth at two stage frames a position, never below the base (256 MB, today's bound, so no
machine gets less than before), never above a quarter of the memory the process can still take
(`kt_gpu::memory::available`, injected into the engine as a probe: the engine does not depend on the
device crate). The player sizes it when the loop region or the stage's frame size changes — a pass that
changes neither reads nothing, so rest costs nothing — and a smaller bound evicts at once. A
`stream_memo_cap` record names each sizing.

### D6. An advance converts no picture

`StreamKernel::advance` (default: `render` into a scratch buffer). The prefetch's advance and the
export's pre-roll threw the RGBA away; Datamosh's advance runs the decoder loop into the memo only
(t2278). Found by the first after-measurement: 240 extra jobs over 240 frames.

### D7. A loop keeps the pictures; the encoder's state goes first

The first sized loop still recomputed every lap: a position kept the decoded picture (1.5 bytes a
pixel) and the clean encoder's state (reconstruction, 16-bit residual, vectors: 4.5 bytes a pixel), and
a quarter of the 12 GB the test process had was 3 GB against 3.7 GB needed. A loop shows the pictures
again; the encoder's state is read only by the next frame's encoder. `StreamMemo::put_transient` marks
such entries; past the newest four (the last frame's state on each side) they are evicted before any
other. The bound counts half the stage's RGBA frame a position over the 256 MB base. Result: 1577 MB,
laps 2 and 3 run no job (t2276 holds the rule on a small stream).

## The class walk (what was checked)

| Place | Acts? | What |
|---|---|---|
| Datamosh picture: `needs`/`held`/`key`/`render` | **yes — fixed** (D1) | a schedule + two maps + a chain walk with curve reads per call |
| Datamosh sound: `MoshSound::sched` per 1024 samples | **yes — fixed** (D2) | a layout + schedule per codec frame, gate runs walked back |
| Datamosh codec jobs | **yes — fixed** (D3, D4) | a word per sample on the boundary; per-pixel conversion on the CPU |
| kt-engine: prefetch `layers_at` per frame of the 2 s window, `stream_advance_ahead`, the composite's `key`/`needs`/`held` | the multiplier; **fixed by D1** | each call is now a stored-chain read |
| the preview stream memo (256 MB constant) | **yes — fixed** (D5, D7) | a constant against the loop's length; the encoder's state crowding the pictures out |
| the prefetch's advance and the export's pre-roll | **yes — fixed** (D6) | a picture converted and thrown away per advanced frame |
| the export's stream memo (64 MB) | no | the export reads forward once; no loop to keep |
| the present memo of kept composites (512 MB constant) | **yes — not fixed here** | at 1080p it holds ~60 composites, so a longer loop is composed again every lap: found-defects row 310 |
| VHS tape sound `set_targets` (`fx-video-tape/src/sound.rs:607-631`) | **yes — not fixed** | per chunk with curves: ~40 curve lookups, `Deck::new`, a cache drop, an FIR redesign when gap/spacing/bias are curved: row 311 |
| VHS tape sound dropout cache (`sound.rs:513`) | mildly | O(back) per 1 ms cell: row 311 |
| VHS tape video `gpu_values` per frame | no (two small vectors a frame) | — |
| Camera mic noise suppressor (`fx-video-camera/src/mic.rs:242-286`) | **yes — not fixed** | a fixed sqrt-Hann window (1024 `fsin`) and two vectors per 256 samples: row 312 |
| Camera mic per codec frame, `set_targets`, video `Plan::new` | mildly | allocations; ~28–48 name scans per chunk/frame: row 312 |
| Pitch (WSOLA) `hann()` and candidates per grain (`fx-audio-modulation/src/lib.rs:1005-1021`) | **yes — not fixed** | 1024 `fcos` per 512 samples for a fixed window: row 313 |
| vocoder, LFO nodes, dynamics, tone | no | windows fixed, LFOs compiled at `bind`, coefficients memoized |
| `audio_codec` encoder/decoder | partly | tables are `OnceLock`; 6–8 scratch allocations per frame and channel: row 314 |
| other compute jobs (`generator.rs`) | no | RGBA8 already one word a pixel |

## Risks

- **Bytes (invariant 11):** t1239 pinned bytes unchanged, t2272 the sound's, t2270 the keys.
- **Memory:** the plan grows with the event (≈ 200 B a position); the memo may grow to a quarter of
  the memory available when the person loops a long mosh — that is the trade the owner's loop asks.
- **gpu-arbiter** works in `kt-gpu/src/compute.rs`; this change does not touch it.
