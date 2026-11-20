---
id: F-AUD-007
title: Auto-ducking by the role of a clip — music under speech, titles over the background
status: candidate
priority: B
cost: M
category: audio/mixing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-app]
invariants: [1, 2, 3, 6, 11, 12]
ui_rules: [16, 21, 23, 24]
round: null
point: null
tests: []
depends_on: [F-AUD-002, F-AUD-004]
checked: 2026-11-20
---

# F-AUD-007 — Auto-ducking by the role of a clip — music under speech, titles over the background

## Essence

A video made of sections (an intro, a title, content, the next title, more content) has a background sound
under all of it. During the content the background must sit under the speech; during a title it may rise, or
it may give way to the title's own sound. Doing it by hand means a volume curve per section, redone after every
re-edit. The owner asked for a "thing that switches by itself", and chose the clip TYPE as what decides it
(2026-10-30, question 10): the background knows it is the background, speech knows it is speech, and the
program ducks one under the other wherever they meet.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | no role-based ducking; a sidechain on Track Compressor in some builds | ducking is done with a compressor keyed by a bus or by hand envelopes [NOT VERIFIED live] |
| Adobe Premiere | 26 | Essential Sound: clip types Dialogue, Music, SFX, Ambience; Music → Ducking → "Duck against" | a clip is TAGGED with a type; on a Music clip "Ducking" picks which types it ducks against (dialogue, SFX, ambience, unassigned), with Sensitivity, Reduce By (dB), Fades; "Generate Keyframes" writes volume keys onto the music clip (they are replaced on the next generate) [NOT VERIFIED against the primary source] |
| DaVinci Resolve | 21 | Fairlight Track FX "Ducker"; clip/track roles via Fairlight "dialogue" tagging | a live ducker on the target track fed by one or more source tracks; amount, attack, hold, release [NOT VERIFIED live] |
| Final Cut Pro | 12 | Roles (Dialogue, Music, Effects) + Audio Enhancement "Ducking" in older versions, third-party plugins today | roles are metadata on every clip; ducking by role is a plugin's job [NOT VERIFIED live] |
| After Effects | 26 | none | — |

## The trick behind it

**1. The role is the address, not the signal.** Premiere and FCP do not ask the person which TRACK is the speech:
the clip carries its type, and the music says "duck against Dialogue". Moving a line to another track, adding a
second narrator, or nesting a section changes nothing, because the rule names a role, not a place. A track-keyed
sidechain (ours since R.65, Resolve's Ducker) breaks the moment the speech is not on the one key track.

**2. The person's knobs are not the compressor's knobs.** Premiere's ducking has three: how much (dB), how
sensitive, how fast. A compressor has threshold, ratio, knee, attack, release, makeup — and to get "music −12 dB
under speech" from it a person has to invert the compression law in their head. The owner said exactly this about
ours: "очень не очевидно что и куда крутить". The device can stay a compressor; the form must speak dB of
reduction.

**3. A generated curve is visible; a live device is not.** Premiere's keys are a snapshot (stale after a re-edit,
regenerated over hand edits); a live ducker is always right but shows nothing on the timeline. The useful
combination is a live device whose reduction is DRAWN on the background event, and a "convert to curve" for the
rare hand fix.

## Pros

- One tag per clip replaces a curve per section; re-edits need nothing.
- The same mechanism serves "background up during titles" (a role-level offset) and "title sound mutes the
  background" (a deep duck under another role).

## Cons

- A role is one more property to set; untagged clips must have a sane default (by track).
- Drawing the reduction needs the key's loudness ahead of playback: an analysis pass, kept in a cache.

## What users say

The owner, `docs/ktprompts/v26.10.txt` lines 84–88 (the background muted or raised by sections — titles vs content —
"можно это всё руками проставлять … а можно какую-то штуку придумать, чтобы оно автоматом могло туда-сюда
переключаться"), and on 2026-10-30 about our R.65 ducking: "у нас дакинг сделан как-то вообще не интуитивно
понятно... очень втупую сделан дакинг у нас" (`docs/user-feedback.md`, question 10).

NO QUOTES: no web access in this spec session (2026-11-20). The search would be the Adobe community threads on
Essential Sound "Generate Keyframes" (some are quoted in F-AUD-002), r/premiere and r/davinciresolve for "auto
ducking" and "ducker"; the editor rows are [NOT VERIFIED] for the same reason.

## How it fits Kinotrain

### What already exists

- The compressor with a key input (R.65, capability `audio-fx`): the live sidechain, keyed by ONE audio track
  (`src/libs/kt-model/src/project.rs`, `sidechain_keys`), in the track window as two compressors
  (`src/apps/kt-app/src/track_window.rs`).
- Track objects with chains (R.62 B), the level line of an event, curves with one automation widget.
- The analysis lane (`src/apps/kt-app/src/analysis.rs`) for background audio passes (loudness, sync).

### What is missing

- A role on the track and the event; a key that is "every event of these roles"; a ducking form in dB; the drawn
  reduction; the boost/mute rules of the owner's sections.

### What hinders

- The key today is a track id (`RowRole::SidechainKey`); a role key needs a mix of matching events computed in the
  render thread before the background's chain — the same order rule as a track key (the key renders first).

### Which invariants it touches

- **1, 2, 3** — the key mix is f32 48 kHz on the render thread; the compressor keeps its state across edits.
- **6, 11** — no new clock; the same bytes every run.
- **12** — the drawn reduction is computed once per revision on the analysis lane, never in a frame.
- UI rules 21 (the industry name "Auto-ducking"), 23 (the form remembers), 24 (one compressor, not a copy).
- CAPABILITY: openspec/specs/audio-fx/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | kt-model (roles, the role key), kt-engine (role key mix), kt-app (roles, the ducking form, the drawn reduction) |
| Project format | yes: roles and role keys — a `.kt` version by content |
| Tests | the role key equals a track key on the same material; moving speech to another track changes nothing |
| Depends on | F-AUD-002 (the keyed compressor), F-AUD-004 (track chains) |
| Breaks compatibility | no (new rows by content) |

## Verdict

Take it as a revision of R.65 rather than a new device: roles on tracks and events (Dialogue, Music, Effects,
Ambience, Title), the compressor's key becomes "every event of these roles", and the background track gets an
"Auto-ducking" form in the person's terms (reduce by dB, sensitivity, fade) that drives the same compressor; the
reduction is drawn on the background events.

## Sources

- `docs/ktprompts/v26.10.txt` lines 84–88 (N3); registry `v26.10-index.md` N3; `docs/user-feedback.md`
  question 10 of 2026-10-30.
- F-AUD-002 `audio/processing/compressor-and-limiter.md` (Premiere's ducking snapshot, Resolve's Ducker).
