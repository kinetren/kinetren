# Design: stream-layout-per-build

## Context

Row 8 of the order table of `docs/engine-audit.md`. The owner's complaint (kkt-26.11.09, I2): a heavy
Datamosh on `problem_region_26_11_09` (event 2149 of his project: Camera ISP → Datamosh gop 66, 720
lines, block 8, search 64, `mv_rotate` drawn by a zigzag curve → VHS Tape; the event is 209 s long)
stays "Loading" for seconds, paused and on repeated laps. The stream stage's own share, measured
before this change with `kt-cli stream-cost` (release, this machine's llvmpipe, 4 s of the region,
1280×720 stream):

| | before |
|---|---|
| the advancing thread's own processor time, outside the jobs | 15.7–22.9 ms a frame |
| inside the jobs (submission, copies) | 5.4–9.9 ms a frame |
| uploaded / read back | 10.63 / 18.28 MB a frame |
| the prefetch's 2 s window question (`wanted_frames`) | 23.3 ms |
| second lap over 240 frames, the 256 MB memo | 264 encodes again (everything) |

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
integer BT.601 of `Planes::from_rgba`, edge replication included. `rgba.wgsl` converts the decoded
picture back (`codec::to_rgba_gpu`), with the CPU path as the fallback when the device fails. Both CPU
functions stay as the reference (t2274 compares packets, reconstructions and RGBA at sizes that pad and
crop). The trade: the encoder uploads the frame's RGBA (4 bytes a pixel) instead of planes (1.5), and
the processor does no per-pixel work.

### D5. The preview memo sized by the loop and the machine

`stream_memo_bytes(base, per_position, loop_positions, available)`: with a loop, every position of it
plus an eighth at two stage frames a position, never below the base (256 MB, today's bound, so no
machine gets less than before), never above a quarter of the memory the process can still take
(`kt_gpu::memory::available`, injected into the engine as a probe: the engine does not depend on the
device crate). The player sizes it when the loop region or the stage's frame size changes — a pass that
changes neither reads nothing, so rest costs nothing — and a smaller bound evicts at once. A
`stream_memo_cap` record names each sizing.

## The class walk (what was checked)

| Place | Acts? | What |
|---|---|---|
| Datamosh picture: `needs`/`held`/`key`/`render` | **yes — fixed** (D1) | a schedule + two maps + a chain walk with curve reads per call |
| Datamosh sound: `MoshSound::sched` per 1024 samples | **yes — fixed** (D2) | a layout + schedule per codec frame, gate runs walked back |
| Datamosh codec jobs | **yes — fixed** (D3, D4) | a word per sample on the boundary; per-pixel conversion on the CPU |
| kt-engine: prefetch `layers_at` per frame of the 2 s window, `stream_advance_ahead`, the composite's `key`/`needs`/`held` | the multiplier; **fixed by D1** | each call is now a stored-chain read |
| the preview stream memo (256 MB constant) | **yes — fixed** (D5) | a constant against the loop's length |
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
