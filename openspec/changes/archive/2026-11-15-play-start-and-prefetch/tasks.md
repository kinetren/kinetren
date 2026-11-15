# Tasks: play-start-and-prefetch

## 1. Row 525
- [x] 1.1 Reproduce under gdb with busy loops: the pre-fix test hangs 8/12 at load 48, the current one passes 42/42 at 48–96 (design D1)
- [x] 1.2 Class walk of every hand-off to a parked thread (design D2)
- [x] 1.3 Compose thread's stop under its lock; t2640 (red on the unlocked stop)
- [x] 1.4 The trace writer's handshake fenced on the producer's side

## 2. The start and its flush
- [x] 2.1 `Player::seek` stores the flush before the generation; a start holds while a drained-by-the-consumer flush is pending (`drains_flush`, `start_hold` b 3); t2641 (red without the hold)

## 3. Play's first sound (row 516)
- [x] 3.1 `play_at_ns`, `first_sound_us`, trace `play` aux `first_sound`
- [x] 3.2 `preview_bench` device double with `device_open`; `kt-cli preview --device-open-ms`; `kt-cli trace summary` first sounds, opens, resumes; t2645
- [x] 3.3 The device pauses at rest and resumes on Play (`Stream` trait, `PAUSE_CHECK`, a seek while paused drained by a short resume); t2642–t2644; t2380–t2384 on a device that cannot pause
- [x] 3.4 Live `idle`: the sound threads stand on a pause (paused, no longer gone)

## 4. Measure and close
- [x] 4.1 Release, heavy showcase from /dev/shm, beside 96 busy loops: `kt-cli preview` first sound with device opens 0/20/2000 ms
- [x] 4.2 Tests of the specs, `cargo test` of what changed, clippy, live `idle`
- [x] 4.3 found-defects 525, 516 leave; CLAUDE.md lines (kt-engine, kt-app); What's new; archive
