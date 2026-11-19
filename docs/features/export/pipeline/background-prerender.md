---
id: F-EXP-008
title: Background prerender of slow regions to disk
status: done
priority: B
cost: M
category: export/pipeline
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-engine, kt-app, kt-cli]
invariants: [4, 7, 11, 12]
ui_rules: [12]
round: kkt-26.11.17
point: change render-cache-disk (S5; architecture review 2026-11-17 R3)
tests: [t2980, t2981, t2982, t2983, t2984, t2985, t2987, t2988]
depends_on: []
trackers: []
services: []
checked: 2026-11-18
---

# F-EXP-008 — Background prerender of slow regions to disk

## Essence

A region the machine cannot compose in real time is composed once, slower than real time, and its
finished frames are kept on disk. Later it plays from the files: no decode, no effect, at any lap,
after a restart too. The editor sees which part of the timeline is ready by a colour on the ruler.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Selectively Prerender Video | an explicit command over the loop region; the frames go to a prerender folder as files of a chosen codec, keyed by the region's events; an edit inside drops the file; no background mode |
| Premiere | 26 | Render In to Out / preview files | the red/yellow/green bar over the ruler; an explicit render writes preview files of the sequence's preview codec; an edit turns the bar red again; no background mode for effects (only for some auto-renders of nested sequences) |
| Resolve | 21 | Smart Render Cache | the red/blue bar over the ruler; in Smart mode it caches effects-heavy clips, transitions and composites BY ITSELF after a delay of inactivity set in Project Settings ("Enable background caching after N seconds"); the files are `.dvcc`, usable only inside Resolve |
| Final Cut Pro | 12 | Background rendering | on by default: it starts 5 s after the person stops working and moving the pointer, renders every effect that is not real-time, and stops at the first input |
| After Effects | 2026 | Disk cache | RAM previews spill to a disk cache keyed by the frame's content; the green bar on the time ruler; cached frames survive a restart when the project and the footage are unchanged |

## The trick behind it

- **The idle trigger is "the person stopped", not "the program is free"**: FCP counts 5 s after the last
  input INCLUDING pointer motion, Resolve has its own inactivity delay. A background render that
  starts while the hand is on the mouse costs the person the reaction of the next gesture.
- **Resolve picks the work by the clip's cost class, not by the timeline**: in Smart mode only what is
  "too slow" (effects-heavy, transitions, composites) is cached, so a long plain edit costs no disk.
- **The key is the content, not the position**: AE's disk cache and Resolve's cache survive moving the
  clip on the timeline as long as what is computed is the same; Vegas's prerender is tied to the region
  and dies with any edit inside it.

## Pros

- A slow region costs its composite time once per edit, not once per lap.
- The background mode needs no gesture: the person who just played a slow part and stopped gets it
  ready by the next play.

## Cons

- Disk: FCP and Resolve users report render files filling the system drive; a cap and an eviction
  rule are part of the feature, not an option.
- Work at rest: the machine is busy while the person reads the screen. FCP and Resolve make it stop at
  the first input; a prerender that holds the device past a gesture makes the interface feel stuck.

## What users say

> "Rendering creates render files that tend to fill hard drives fast, especially when you are low on space."
> — Apple Support Communities, thread 255103156, 2023, https://discussions.apple.com/thread/255103156

> "If background caching or auto cache transitions/composites/fusion is enabled - caching renders can start during delivery output renders."
> — Blackmagic forum, Resolve 15b3 report, 2018, https://forum.blackmagicdesign.com/viewtopic.php?p=413350

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-plan/
Done in `render-cache-disk` (2026-11-18): composites of moments measured at least half a frame period are written at rest after 1.5 s with no input under a durable plan key, read before composing, never by the export.

## Estimate

| What | Answer |
|---|---|
| Scope | M — one change |
| Where the main work is | kt-engine (key, store, prerender thread), kt-app (data folder, input signal), kt-cli (trace view) |
| Project format | does not touch |
| Tests that will have to be written | key stability across processes and its invalidation; store cap; restore equals the composite; prerender yields |
| Depends on | render-ahead (F: producer) |
| Breaks compatibility | no |

## Verdict

Take, as Resolve's Smart mode with FCP's trigger: only moments whose measured cost does not fit the
frame period, after a quiet period of no input, stopped by input, playback or an export.

## Sources

- https://support.apple.com/guide/final-cut-pro/ (background rendering, access date 2026-11-18)
- https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part163.htm (Resolve render cache settings, access date 2026-11-18)
- https://discussions.apple.com/thread/255103156 (access date 2026-11-18)
- https://forum.blackmagicdesign.com/viewtopic.php?p=413350 (access date 2026-11-18)
