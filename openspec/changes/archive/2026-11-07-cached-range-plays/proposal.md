# Proposal: cached-range-plays

## Why

The owner (`docs/ktprompts/kkt-26.11.07b.txt`, line 3–4; registry rows G2, G3, G4) played a range of
his project that the bars showed as covered, 408 ms long, in a loop: "16vfps, картинка тормозит,
процессор 45%" and "три проблемы - производительность, хрип звука, не рабочий кэш". Asked whether a
covered range keeps its sound after the effects (a, changing invariant 2) or only the effects get
faster (b), he chose "вариант А".

His load report (`fails/26.11.07/load-report.txt`, 711–645 s before the end) and `kt-cli preview` on
the relinked copy of his project say what happens there:

- **The picture.** 0 decodes, 16 composites a second, one per UI frame: the decoded frames are held,
  but every frame of every lap is composed again (VHS Tape + Camera ISP over a nested reel; 72 ms of
  device time per frame on this machine's llvmpipe at 640×360). The memo the `video-plan` spec
  promises ("a composed frame SHALL be memoized in the preview … keyed by the plan key") lives only on
  the readback path; since preview-throughput D2 the preview presents on the device and neither asks
  nor fills it.
- **The sound.** Every burst of every lap runs the effects (invariant 2 forbade keeping their output).
  On his range that costs 5–6 % of a core and never falls behind (0 render and 0 callback underruns
  in an 8 s loop). The clicks he hears there are the VHS Tape sound's own dropouts (full-scale
  single-sample steps, 123 a second as set in his panel; without the effect the largest step is 0.11
  against 0.29). What does fall behind is an edit during playback: the audio render thread rebuilds
  its renderer itself, 412 ms on his project against an 85 ms ring (his report: 12–16 bursts a second
  instead of 47 while the prefetch and audio threads take 66–87 %).
- **The bars.** They show decoded sources, not what plays without work — invariant 4 read as "what the
  bars show plays without work" did not hold for either half.

## What Changes

- **A shown composite is kept** (video): the GPU executor keeps the composites it presented, keyed by
  the plan key, within a byte budget; presenting a frame whose key it holds runs only the display pass.
  A parameter change moves the keys of exactly the frames it touches.
- **A covered range keeps its final mix** (audio, the owner's "вариант А", a change of invariant 2): the
  audio render thread keeps the final mix it rendered (after effects, track stages and nested reels,
  before the master gain) per timeline burst, keyed by a hash of what sounds there; a burst it holds is
  handed out with no decode and no effect run. Linear play and the laps of a loop are kept apart. A
  parameter change drops only the bursts whose sound it determines. Leaving a kept range for live
  rendering continues exactly as a seek to that point does.
- **An edit does not stop the sound**: the renderer is rebuilt off the audio render thread; the old one
  plays until the new one is ready.
- **A start does not wait for what the store holds**, and the playback prefetch does not order sources
  for bursts the store holds.
- **The bars tell the two apart**: the strip under an event is bright where the range plays without
  work (final composite or final mix held) and the former green where only its sources are decoded.
- `kt-cli preview --loop S --wav out.wav` plays a range as a loop and records what the device played;
  the work counters `mix_replays` and `composite_replays` name the free bursts and frames.

## Capabilities

### Modified Capabilities

- `audio-render`: invariant 2 reworded (the final mix of a covered range is kept by the render
  thread; FX state is still touched by that thread alone); a requirement for the mix store.
- `video-plan`: the composite memo covers the presented preview; the bars' requirement names the
  final layer.
- `media-index`: the coverage requirement names the two layers and drops "audio post-FX MUST NOT be
  cached".
- `transport`: a start does not wait for held bursts; an edit does not stop the audio render.

## Impact

- Code: `kt-gpu` (`pool.rs`, `engine.rs`: present memo), `kt-engine` (`mix.rs` new, `audio.rs` scene
  hashes, `player.rs` render loop, builder, coverage, `video_plan.rs` trait), `kt-time` (two work rows),
  `kt-app` (`timeline.rs`, `app.rs` strip), `kt-cli` (`preview --loop --wav`).
- Memory: the composite memo 512 MB of device memory, the mix store 64 MB.
- Root `CLAUDE.md` invariants 2 and 4; `kt-engine`, `kt-cache`, `kt-gpu` CLAUDE.md lines that said the
  opposite.
- Tests t1930–t1941; the live scenario `cached_range.py`.
