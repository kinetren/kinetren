---
id: F-UI-006
title: Playback performance readout — a light in the viewer, the detail on demand
status: done
priority: B
cost: S
category: ui/viewer
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: [4, 6, 12]
ui_rules: [9, 13, 22]
round: diagnostics-three-graphs
point: "kkt-26.11.01 item 14 (registry K14)"
tests: [1145, 1146, 1147, 1148, 1149]
depends_on: []
trackers: []
services: []
checked: 2026-11-01
---

# F-UI-006 — Playback performance readout — a light in the viewer, the detail on demand

## Essence

While a person plays the edit, the editor answers one question at a glance: "does this play in
real time, and if not — is it me or the machine?". The answer comes in two layers: a sign that is
always in view and costs no space (a light, a number in a strip that is already there), and a
detailed view that is opened on purpose when the sign turns bad. The subject of this note is
where each layer lives and what the detailed one shows.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | the Video Preview info bar | the line under the preview shows the project, preview and display formats and, while playing, the ACTUAL frame rate in place of the nominal one; there is no separate graph window [NOT VERIFIED in 2026, the help text is old] |
| Adobe Premiere | 26 | Show Dropped Frame Indicator (monitor's wrench menu) | a small light in the Source/Program monitor: green, yellow once a frame drops; it resets on each playback, the tooltip carries the count. Off by default |
| DaVinci Resolve | 21 | the viewer's fps indicator + "Stop playback when dropped frame is detected" | the fps number over the viewer with a dot that turns red when playback falls below real time; a user preference can STOP playback on a drop. The render-cache strip over the ruler (red/blue) says what is cached |
| Final Cut Pro | 12 | "If frame drops on playback, stop and warn" (Settings → Playback) | no live readout at all: a drop is an event that stops playback and opens a warning [NOT VERIFIED] |
| After Effects | 26 | Info panel / preview fps | while previewing, the Info panel prints the achieved fps and "(Not real-time)" in red when it falls behind [NOT VERIFIED] |

## The trick behind it

**First.** Nobody of the five shows a GRAPH to the editor by default. The always-visible layer is
one bit (Premiere's light, Resolve's red dot) or one number (Vegas's actual fps); the bit is
cumulative for the run (Premiere's light stays yellow until the next play), so a drop of one frame
three seconds ago is still visible when the person looks. A live number alone misses it.

**Second.** The detailed layer answers "WHY", not "how much": Resolve pairs the red dot with the
render-cache strip (is this range cached?), Premiere with the yellow/red render bar over the
timeline. The useful split is not "fps vs. CPU" but three causes — the display could not keep up,
the decoder/disk could not keep up, the cache threw away what was just made. A readout that does
not separate them sends the person to the wrong fix (Resolve guides send people to proxies when
the cause is a slow drive).

**Third.** Resolve's "stop on drop" is the most hated part of the area: guides tell people to
switch it off first, because a drop stopping the work is worse than the drop.

## Pros

- A one-bit sign in a strip that already exists costs no screen, and is read without opening anything.
- A cumulative sign keeps a short drop visible after it passed; the person does not need to stare.
- Detail on demand keeps the working layout clean: the graphs exist when someone is diagnosing.

## Cons

- Premiere's light is off by default: a person who needs it most does not know it exists.
- Resolve's and Vegas's readouts do not say WHICH stage fell behind; the user guesses between GPU, disk and proxies.
- Final Cut's stop-and-warn turns a diagnostic into an interruption.

## What users say

> If your video preview is dropping frames, you'll see the actual frame rate in this section.
> — VEGAS help, quoted by Robin Davies-Rollinson, dvinfo.net, 17.01.2012,
> https://www.dvinfo.net/forum/what-happens-vegas/504172-view-preview-display.html

> UNCHECK Stop playback when dropped frame is detected
> — Larry Jordan, "Get Started: Multicam Editing in DaVinci Resolve 18", 06.02.2024,
> https://larryjordan.com/articles/get-started-multicam-editing-in-davinci-resolve/

Our owner, kkt-26.11.01 item 14: "Perfomance надо переместить в окно diagnostics, чтобы там три
графика было" (Performance must move into the Diagnostics window, so that there are three graphs).

## How it fits Kinotrain

CAPABILITY: openspec/specs/ui-timeline/ — contract in `spec.md`, why and code paths in `design.md` (change `diagnostics-three-graphs`).
The always-visible layer stays the status line (`src/apps/kt-app/src/app.rs`, `draw_statusbar`: vfps, cache, underruns in ms); the left-column perf box moves into the Diagnostics window, whose three graphs answer the three causes — playback (FPS/V-fps/hit), FFmpeg activity, cache churn.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one change |
| Where the main work is | kt-app (`app.rs` perf box and Diagnostics window) |
| Project format | does not touch |
| Tests that will have to be written | harness: the column has no perf box, the window shows all three graphs and every former line, the graphs grow with the window, no repaint while idle |
| Depends on | — |
| Breaks compatibility | no |

## Verdict

Done in change `diagnostics-three-graphs` (2026-11-01, capability `ui-timeline`). It takes the split the industry converged on: a sign always in view (our status line already has
vfps, underruns in ms, cache — it stays), the detail in one window opened on purpose (the owner's
request: the perf box goes into Diagnostics). Our detail keeps what the five do not have — three
graphs by CAUSE (display, FFmpeg, cache churn). Do not take stop-on-drop.

## Sources

- https://www.dvinfo.net/forum/what-happens-vegas/504172-view-preview-display.html — quote of the VEGAS help (access date 2026-11-01)
- https://larryjordan.com/articles/get-started-multicam-editing-in-davinci-resolve/ — Resolve UI preference (access date 2026-11-01)
- Premiere's dropped frame indicator: community.adobe.com threads ("how to analyze video for dropped frames", "dropping frames in Premiere Pro") — not fetchable from here (blocked), content from the search excerpts (2026-11-01)
- Resolve's red fps dot: forum.blackmagicdesign.com viewtopic p=1147488 ("fps indicator is missing in vertical mode") — 403, content from the search excerpt (2026-11-01)
- Nothing here was checked live in the five core editors.
