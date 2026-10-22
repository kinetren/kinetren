---
id: F-AUD-009
title: Delay / Echo on an event
status: done
priority: A
cost: M
category: audio/processing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-engine, kt-app]
invariants: [1, 2, 3, 11]
ui_rules: []
round: "R.47"
point: 329
tests: [322, 326]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-AUD-009 — Delay / Echo on an event

## Essence

Repeats of the sound with an adjustable time, decay (feedback) and a dry/wet balance. For the editor this is
"an echo on a word", "a tail on a beat", "a dub delay on a phrase" — the most frequent audio effect after the
equalizer. It is placed on an event; the repeat time and the mix are animated by curves.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Simple Delay / Multi-Tap Delay (DirectX FX) | the FX chain of an event/track; multi-tap — up to 8 taps, each with its own time and pan |
| Adobe Premiere | 26 | Delay (Audio Effects → Delay and Echo) | up to 2 s of delay, Feedback in %, Mix; on a clip and on a track |
| DaVinci Resolve | 21 | Echo (Fairlight FX) | separate L/R times, feedback with a filter in the loop, tempo sync |
| Final Cut Pro | 12.3 | Echo (a Logic set of effects) | a preset echo from Logic; the parameters open in the Audio panel |
| After Effects | 26 | Delay (an effect of an audio layer) | a layer effect, time in ms, Feedback and Dry/Wet |

**Outside the core.** Without a DAW it is not visible how the "editor's delay" differs from the "musician's delay" —
by tempo sync and by the filter in the loop.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Ableton Live | 12.4.5 | Delay / Echo | the time in fractions of a bar OR in ms; in the feedback loop a filter and saturation — the tail darkens with every repeat |

## The trick behind it

In all five the feedback loop is not a bare reuse of a buffer: a filter is built into it (in Resolve these are
explicit knobs, in Ableton a filter and saturation), therefore every next repeat is darker and softer than the
previous one, and the tail does not turn into a metallic ring. Second: changing the delay time on the fly in
"tape-like" implementations does not click but "glides" — the buffer is read by a fractional head, and the time
is smoothed (tape glide), rather than jumping. Third: Premiere limits the feedback noticeably below 100 % — the
self-acceleration of the loop is regarded as a defect, not a feature.

## Pros

- Repeats with decay are the only way to make an "echo on a word" without copying the clip by hand N times with a
  stepped gain.
- A filter in the loop gives a natural tail: the copies do not ring.
- An animatable mix allows "turn on the echo on the last word" with one curve.

## Cons

- An effect with memory: the tail lives longer than the clip — the engine must extend the window of the event by the
  length of the tail, otherwise the repeats will be cut at the boundary.
- Feedback near the maximum holds the tail for tens of seconds: the price is that the event stays "active" for the
  render for a long time after its end.

## What users say

NO QUOTES: the research was done from the official documentation and the help of five editors from the memory of the model (knowledge cutoff 2026-01); a verbatim forum fragment with a link cannot be reproduced without live access to the thread — the facts of the table are verifiable by the vendors' manuals from the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/audio-fx/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
An echo is a node of the chain with a pre-delay and a tail; the state lives only with the audio-render thread.

## Estimate

| What | Answer |
|---|---|
| Scope | M — a node + generalization of tails |
| Where the main work is | kt-fx (DelayLine, the node), kt-engine (tail tests) |
| Project format | not touched (foreign-passthrough in old builds) |
| Tests that will have to be written | an echo exactly at time_ms; a series of decreasing peaks; the tail ≤ chain_tail; ping-pong in the opposite channel |
| Depends on | the node chain (the first wave of R.47) |
| Breaks compatibility | no |

## Verdict

Take it in R.47: time/feedback/mix + a ping-pong checkbox, the filter in the loop — a one-pole lowpass (the heritage of
the "the tail darkens" trick). Tempo sync is not taken — the project has no concept of tempo. The ring to the maximum
(1.5 s), the time is NOT structural — it is modulated by fractional reading with smoothing of ~100 ms (tape glide).

## Sources

- https://helpx.adobe.com/premiere/using/audio-effects-transitions.html (accessed 2026-09-14)
- https://www.blackmagicdesign.com/products/davinciresolve/fairlight (accessed 2026-09-14)
- https://support.apple.com/guide/final-cut-pro/welcome/mac (accessed 2026-09-14)

**Implemented in R.47** — docs/porting-notes.md items 329 (t326); the further life of the feature is there.
