# Design: stage24-reds5

## D1. The picture of the last gesture (845)

`compose.current` in the dump: no ask is pending, and the frame drawn holds the composite of the moment and the size
this frame asks for, whole. The state is in it: an edit, a retune or a build changes the ask, which is pending
until answered. `kt_live.picture()` waits for a dump from a frame after the gesture's that says so, then for
one more frame (the dump is written inside a frame, the screen has it after). It waits once per gesture; playing, no
picture stands and the read is the scenario's own.

Proof: 700 ms planted before every composite: `fade` red with the old readers, green with the new. `gen_shapes` is green
under it both ways: its red was a read under Loading, the first generator on the device waiting 16.6 s for the
compute device to open (found-defects 920: 24 s on a standing disk; the wait is now named in the trace). Its proof is 8 s
planted in the opening. Rejected: reading until two reads agree (`pixel_settled`'s old rule: it agrees on the stale
picture), and a fixed pause.

Class walk, every reader of the stand's screen: `pixel` (43 calls in 29 scenarios), `brightness`, `brightness_red`
(through it), `ink_box`, `pixel_settled` wait; `screen_diff` and `wait_still` wait for a still screen first, which a
late picture breaks: they wait too; `step` and `shot` are for the eyes, and a scenario photographs Loading on purpose: not;
`datamosh_sharp.detail` reads `screen_image` itself: it calls `picture()`. `readme_shots` waits for its own frame
(`preview.loading` gone and a still picture).

## D2. A lap is what was played (846)

`lap_bounds`: a lap begins at Play, a wrap or a seek while playing; the render thread's own seek at the head Play just
started at moves the lap's start and cuts nothing. A seek of a paused transport starts none.

A person on a slow disk: the trace has 26 restores, 154 ms in reading together (6 ms each), no read past the 250 ms
bound: the first lap came from disk. Nothing in the product to fix.

## D3. The trace's tail (918)

`flush` waits for a dying process's sake at most two seconds. It answers `false` when the tail is not there; `kt-cli` says
it on stderr, the editor in its journal. Tests and captures wait by `trace::testing::flush`, with `test_deadline`.

## D4. An admission's order (919)

Proof: 30 ms planted for the frame's class between the lock's release and the record's end gives `[2, 0]`; the same
sleep after the record's end under the lock is green.

## D5. A source that comes back (921)

`picture()` waited two minutes in `nestparam`: the paused frame was in Loading, and the trace showed the prefetch asking
60 frames of an address with no worker. `sync_generators` compares a wanted address with `gen_states`, what it last set in the
live sources; `remove_media` shut the worker and left its entry, so the address, back with the same values, was "as it was".
The entry goes with the source.

Class walk, what else remembers a removed source: `natives` forgotten in the same place; `raised` is a counter, never read
as presence; the copies forget by a raise; the cache forgets by `sync_generators` itself; a media removed and added by the host
goes through `add_media`, which asks no memory. `resync_gen_others` raises nothing for an address without an entry: the
builder that follows does.
