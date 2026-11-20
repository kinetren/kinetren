# Design: auto-ducking-by-role

> DESIGN ONLY, revision 1 (2026-11-20), base `ea0aff6f`. Q1–Q2 in `proposal.md`; defaults recommended.

## Context

- R.65 (capability `audio-fx`, "Compressor with a KEY input"): one compressor kind with an optional key; the key is
  a processing argument on the same window; the key is a TRACK (`RowRole::SidechainKey`, `sidechain_keys`,
  `sidechain_cycle` in `kt-model/src/project.rs`); the track window draws two compressors and a dashed wire
  (`track_window.rs`, `node_editor.rs`). The snapshot curve was removed by the owner (t774 guards it).
- The owner, 2026-10-30, question 10: sections by clip TYPE; our ducking is "not intuitive".
- Rule 21 (industry name), 23 (a tool remembers), 24 (one widget, not a copy).

## D1. Roles

`AudioRole { Dialogue, Music, Effects, Ambience, Title }` (closed, append-only codes). `Track.role:
Option<AudioRole>`, `ClipEvent.role: Option<AudioRole>`; the effective role of an audio event is its own, else
its track's, else none. A video event's linked audio inherits through its audio track as today. A reel instance's
inner events keep their roles; an instance event itself has the role of what it sounds like only if set on it
(it then counts as one key event — its whole mix).

## D2. The role key

`SidechainKey = Track(TrackId) | Roles(RoleSet)`. For a `Roles` key the audio render builds the key per window as
the sum of the post-chain, pre-track-fader outputs of every event in the SAME reel whose effective role is in the
set and which is not on the keyed track itself. The render already computes those outputs; the key mix is one
accumulate per key event (no second decode, no second chain run). Order: the keyed track renders after every track
holding a key event — the same scheduling rule as a track key; a cycle (a track whose own events carry a role in
its key set, or two tracks keying on each other's roles) is refused by `validate` with a named error.
Invariants 1–3: f32 48 kHz on the render thread; the compressor's state is untouched by an edit that changes only
which events are key events.

## D3. The Auto-ducking form

In the track window of an audio track (and "Auto-ducking…" in its header menu, which opens the same window at the
section), one section, the industry name first: "Auto-ducking (background under speech and titles)".

| Field | Unit | Maps onto the compressor |
|---|---|---|
| Duck under | roles ✓ | key = `Roles(set)` |
| Reduce by | dB, 0…−60, −∞ | ratio and threshold such that a key at the sensitivity level gives exactly this reduction (the gain computer solved in closed form) |
| Sensitivity | %, 0…100 | threshold on the key, −60…−10 dBFS |
| Fade | ms, 10…2000 | attack = fade/4, release = fade |
| Between them: raise by | dB, 0…+12 | the track's level +N and the reduction deepened by N, so the level under the key is unchanged |

The section writes the compressor rows (one command per change, rule 25); the compressor's own rows are shown
under "Details", dimmed but editable — editing one there marks the section "custom" instead of fighting it. The
values are remembered per user (`user_store`, rule 23) as the defaults of the next background.

## D4. The drawn reduction (Q1 a)

A background job on the analysis lane computes, per background track with a role key, the gain the compressor
applies over the project, from the key events' loudness envelope (the same detector code, block rate 1920 ticks),
keyed by the content hash of the key events, the background's chain and the reel (the cache rule: keyed by what
determines it). The timeline draws it as a shaded band under each background event's level line (one mesh per
event, rule 12). It is a picture of what the live device does, not a second definition: a test compares it with
the rendered gain within 0.5 dB. "Convert to volume curve" writes the band as gain keys onto the background
events (thinned to ≤ 4 keys per duck) and switches the compressor off, one undo step.

## D5. Format and OTIO

`role` on tracks and events and `Roles` keys are written by content: the `.kt` version rises (the next free
number at apply time; 8 today) only when one is set — an older build would drop them on save, so it refuses.
OTIO: in `metadata["kt"]` of the clip and track envelopes; a foreign editor gets the background un-ducked. A
planned row in "What OTIO export loses".

## D6. Class walk — "a device whose knobs are not the person's question"

| Place | Person's question | Today | Here |
|---|---|---|---|
| ducking | "how much quieter under speech" | threshold/ratio | D3 |
| limiter | "the ceiling" | ceiling dB (already the question) | unchanged |
| gate | "cut the noise between words" | threshold/hysteresis | listed for a later form, not here |
| loudness | "how far from the norm" | a reference line | change `loudness-norms` |
| sidechain by track elsewhere (video keyers) | — | no track key in video | none |

## D7. Idle

Roles and the form cost nothing at rest. The band is computed once per content hash on the analysis lane (class
Background, paused by playback), never in a frame; a hover redraws the cached mesh.

## Rejected

- **A new "Ducker" effect kind** — rule 24 and the audio-fx requirement "A second kind of effect for ducking SHALL
  NOT exist".
- **Sections by regions (`MarkedArea` with a role)** — the owner chose the clip type.
- **A key by track only, with a "pick all speech tracks" helper** — breaks when a line moves track; the role is the
  address (F-AUD-007 trick 1).
