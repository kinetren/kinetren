## Context

Since render-ahead (2026-11-17) and ahead-kept (2026-11-18) one producer composes the moments ahead of the
playhead and keeps each complete composite in the display executor's present memo under the plan key
(`VideoGpuExec::present_key`), and the display only presents kept composites while playing. A kept moment
costs nothing on the next lap — inside one process and while the memo holds it. A restart, an eviction or
an edit elsewhere that moves the memo makes a heavy region slow again for every lap until it is kept.

Review R3 (`docs/architecture-review-26.11.17.md`) proposed a disk tier; the owner chose its mode: "Сам в
фоне" (kkt-26.11.17 S5) — the program prerenders slow regions by itself in the background. Feature note:
`docs/features/export/pipeline/background-prerender.md` (F-EXP-008): Resolve's Smart cache picks the work
by cost class, FCP starts 5 s after the last input including pointer motion and stops at the first input,
both keep the cache on disk with a cap the user complains about when there is none.

## Goals / Non-Goals

**Goals:** a moment composed once at a measured slow place plays later from disk with no decode and no
effect, after a restart too; the program fills those places by itself at rest and yields at once; the disk
is bounded; nothing an export writes changes; the trace says what the tier did.

**Non-Goals:** an explicit "prerender region" command (the owner chose the background mode; a command can
come later on the same store); the export reading the tier (R3's "the export may reuse cached frames" — it
would make the export's bytes depend on what the preview happened to prerender, which invariant 11 forbids
unless proven equal per device; not this change); a lossy format; the audio (its kept final mix is
`cached-range-plays`).

## Decisions

### D1. The durable key: the plan key with what a process numbers replaced by what determines it

`kt-gpu`'s plan key hashes the whole plan, and the frames by their CONTENT SERIAL (R.53 item 409), a number
the frames cache gives per process; the key of an undecoded frame carries serial 0. Neither survives a
restart. `VideoRenderer::probe_durable(cache, t, q, files)` builds the probe plan (no stream stage runs,
`StreamKernel::key` names it) and hashes it as `plan_hash` does — layer kinds, alphas, fades and transition
mixes by plugin id, each source's address, frame index, decode step, `fx_hash`, `u`, masks, camera, lead,
blend weight and blend mode, nested plans with their reel revision and inner tick — but NOT the serials and
NOT the input frame's presence or size (both follow from the address, the index and the step). What the
serial stood for is mixed per leaf while the plan is built (a thread-local the builder fills only for this
call):

- a file leaf: the file's identity — its path, size and modification time (`RenderCache::file_ids`, read
  from the project's media on the thread that asks, never the UI thread); a missing file — its path and a
  "missing" mark (the placeholder draws the name);
- a procedural leaf (generator, title): `src_rev`, the hash of the event's generator values and curves —
  the same identity the codec memo trusts (its address is per event and per override set already);
- a stream stage: its `fx_hash` already carries the kernel's key (media, position, codec size, the front's
  key, `src_rev`); its second input reads another event's file, so the identities of every file of the
  project are mixed too (conservative: replacing any file invalidates moments with a stream stage).

The key is salted by the format version, the program's build (the host passes its version and the FFmpeg
major: a shader or a decoder change is another picture) and the device (`GpuBackendInfo` name and backend):
the folder `<root>/v1-<build>/<device>/`. Why not the serials made durable: the frames cache, the upload
cache and the memos all key by them, and t496/t211 rest on their being numbers; one more key computed beside
the probe costs a hash per moment.

Rejected: a scene hash of the active events (the audio's `scene_hash`): it would have to repeat every
input the plan reads (cameras, track stages, masks, transitions, the step) and would silently miss the next
one; the plan already names them all, and a new input to a frame must reach the plan key anyway.

### D2. The store: lossless files, bounded, least recently used out

`render_cache::DiskStore`: one file per moment, `<key>.ktf` — a 24-byte header (magic, width, height, raw
length) and the RGBA filtered by a left-pixel byte delta, deflated at level 1 (`miniz_oxide`, already in
the lock file). Lossless: the restored picture is the composite's bytes. Writes go to a temporary file and
are renamed. The index (key → bytes, last use) is read from the folder at open, the oldest by modification
time first. The cap is 2 GiB (`KT_RENDER_CACHE_MB` overrides); a store evicts least-recently-used entries
to fit a write; the prerender (D4) never evicts an entry used in this session — it stops when the cap is
full of them. At open the folders of other builds under the root are removed: their pictures can never be
asked again. A short, corrupt or foreign file is removed and is a miss.

### D3. Read before composing

A moment the display's executor does not hold is looked up by its durable key before anything composes it:

- in the producer's pass, before the "decoded?" check (a moment on disk needs no decode): found → read,
  `display.keep(k', …)`, noted for the bars, the pass goes on; a miss is remembered for the lap (one probe
  per moment per lap);
- in `show_kept` (the compose thread: the display while playing, the on-demand path paused).

`k'` = the present key mixed with the durable key: a probe plan of an undecoded moment carries serial 0,
and a memo entry under that key alone could be shown after the file under the same media id was replaced.
The bars (`final_video_runs`) draw the moments restored (they are kept) and the moments the store holds for
the current state (`on_disk`, filled by the prerender's writes and by a cheap scan at rest of the loop
region, or of ±5 s around the playhead, which computes durable keys and asks the index — no device, no
decode).

### D4. The prerender at rest, by measured cost

A thread of the player, `render-cache`. It parks with no clock while there is nothing to do. Work exists
when the tier is on and the preview's cost map (`PreviewCosts`, device ns per moment measured by every
composite, render-honest D2) holds moments of at least HALF a frame period not on disk — a moment that
takes half the period of the device cannot keep real time beside the display, the stream history and the
decode. Candidates: those moments and their unmeasured neighbours (±1 frame); a neighbour composed at rest
is measured, so a heavy run grows to its measured end and stops at the first light moment. Order: inside
the loop region first, then nearest the playhead.

It runs only when: not playing; no export vote (`sched::paused() & why::EXPORT`); the last input
(`Player::note_input`, an atomic stamp, no notify — a pointer move costs a store) is at least `QUIET` =
1.5 s ago (FCP's trigger, shorter: the owner asked for it); the store has room (D2). While input comes it
waits for the remainder of the quiet (one wake per 1.5 s of continuous motion, none at rest).

Per moment: its layers are asked of the player's workers at `Priority::Thumbnail` (behind every preview
request, invariant 5: no source of its own), the stream stages advanced to it (`stream_advance`), the plan
composed on the producer's executor with `execute_into` under `sched::with_class(Background)` — the rings
admit it behind every present, cut at pass boundaries. Its device time (`last_device_ns`, else its wall) is
noted in the cost map; heavy → written, noted `on_disk`; light → nothing written, the run stops there. The
stop check (`stop_when` for a stream history, and between every step): Play, an input since the moment
began, an export vote, the player's close. An abandoned moment writes nothing.

Why a thread and not a scheduler lane: the quiet period is a deadline after the last input, and a lane
has no clock ("nothing here ever wakes on a clock"); the thread waits with a timeout only while input keeps
coming and work exists. Rejected: running whenever paused (it would compete with the hand on the mouse — the
cost the owner forbade); prerendering every moment (a plain edit costs disk for nothing: Resolve's Smart
mode caches by cost class for the same reason); guessing heaviness from the effects' names.

### D5. Determinism and the export

The tier is the preview's alone: no export path reads or writes it (invariant 11 binds the export's
bytes, and a frame of the preview's executor is not proven equal to the export's settled form on a card).
A restored composite is the bytes `execute_into` gave for the same durable key on the same device and build;
on a software rasterizer that equals `present`'s keep (t2922), on a card within the run-form tolerance the
preview already has (gpu-leftovers D2). With the tier off (`KT_RENDER_CACHE=off`, no root set: tests, the
CLI without the flag) the preview runs exactly as before.

### D6. The trace

`Ev::RenderCache` (121): `aux` the kind — `RESTORE` (a moment read from disk into the display's memo; `a`
tick, `b` bytes, `c` durable key, its span the read and inflate), `STORE` (written: `b` file bytes),
`EVICT`, `HEAVY` / `LIGHT` (the prerender's measured moment: `b` its device ns), `ABANDON` (stopped by
input, Play or an export), `SCAN` (the bars' scan: `a` moments looked at, `b` found). `kt-cli trace cache`
prints the counts, the bytes, the restores per lap of a loop and why the prerender stopped.

### D7. The host

kt-app sets the root `<data>/render-cache` and the build salt (`kt_version` full + FFmpeg major) once the
player exists, and calls `note_input` from a frame whose input has events. kt-cli `preview --render-cache
DIR` sets the same and, at the end of its range, pauses and waits for the prerender (bounded), so two runs
measure a restart.

## Class walk (a problem is a class)

The mechanism: work whose result is a pure function of the plan is redone because the result's key lives
only in one process's memory. Checked:

- preview composites while playing (the producer) — this change;
- composites on pause/scrub (the on-demand path) — read from disk by `show_kept`; written only by the
  prerender (a moment the person looks at once is not heavy by measurement);
- the slow-chain route (offline device) — its bytes go through the display's `keep`; its moments enter the
  cost map and the prerender like any other;
- the stream history (Datamosh) — its memo is per process; a moment on disk needs no history, which is the
  main saving at Datamosh places; the history itself on disk: not this change (its entries are device
  buffers and codec state; row 770);
- the audio's final mix — per process too (`MixStore`); the audio render is real time on release (item 301),
  no disk need measured: row 771;
- reel strips and posters — already on disk (sidecar, posters folder);
- pass times — already on disk (`seeds.rs`); compiled pipelines — the driver's cache;
- decoded frames — not kept: the file is their disk tier.

## Risks / Trade-offs

- [The machine works at rest] → only after measured slow moments exist, after 1.5 s of no input, stopped at
  the first input. Every composite is measured, the paused frame on screen included: opening a heavy
  project prerenders the slow run around the playhead by itself (live `render_cache`: 54 moments, 13 s of
  work after the opening), then rests. The `idle` live scenario on the heavy project: quiet after the
  opening, then 1 % of a core on a pause and 1 % with the effects window open — the program rests once
  the measured run is on disk.
- [Disk use] → a cap with eviction, other builds' folders removed at open.
- [A stale picture] → the durable key repeats the plan key's inputs; a new input of a frame that reaches the
  plan key (a rule since R.53) and is hashed by `plan_hash` must be hashed by `durable_hash` too — t2981
  compares their fields; the file identity and the program build cover what a process numbers.
- [A replaced file with the same size and time] → missed (the same blind spot as the sidecar's fast path,
  which hashes the content only when size or time moved).

## Measurements

Release `kt-cli`, llvmpipe (LLVM 20.1.2, Vulkan), the machine shared with other sessions (load average
18–26 while measuring: the absolute numbers are noisy, the restart's are not). The project is ours:
`kt-cli showcase --heavy` (fixture footage in `/dev/shm`), the one-second loop at its cursor (17.36 s)
under Slowed, VHS Tape and Camera ISP, `kt-cli preview heavy.kt 17.36 18.36 --size 640x360 --loop 20`.
Three runs one after the other: without the tier; with `--render-cache DIR` on an empty folder (the
first process); again over the same folder (the restart).

| | no tier | first process | restart |
|---|---|---|---|
| lap 0: frames shown / kept | 45 / 1 | 35 / 17 | 25 / 25 |
| first lap shown wholly from kept frames | none in 20 laps | lap 6 | lap 0 |
| V-fps over 20 s, incomplete composites | 5.6, 877 | 22.8, 63 | 25.0, 0 |
| device busy per lap | 42–156 % | 57–132 % until lap 6, then 2 % | 2–6 % |
| process CPU over the run | 9.5 cores | — | 0.2 cores |
| restored from disk | — | 11 | 26 (lap 0: 25) |

The first process prerendered at rest after its run: 54 moments measured heavy (device 233 ms a moment
on average, 721 ms at most — the loop's 25 and the heavy moments next to it, the run growing to its
measured end), 2 light (6 ms, not written), 30 MB on disk in 12.4 s; it then stopped ("nothing left").
Its own lap 0 already restored 11 moments: the bench's settle composes on demand while paused, so those
moments were measured and prerendered before the play began. The restart composed nothing: every frame
of every lap came from the disk (`trace cache`: 25 restored in lap 0, 105 ms of reading for 26 moments).
