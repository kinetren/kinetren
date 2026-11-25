# Design: auto-ducking-by-role

> DESIGN ONLY, revision 2 (2026-11-22), base `1e31056c`: rebuilt from the owner's whole item (v26.10 lines 84–88) and
> B5. Revision 1 keyed one rule by roles and had no title sections. Q1 in `proposal.md`; the default is recommended.

## Context

- R.65 (capability `audio-fx`, "Compressor with a KEY input"): one compressor kind; the key a processing argument; the
  key is ONE track (`RowRole::SidechainKey`, `sidechain_keys`, `sidechain_cycle`); the track window draws two
  compressors and a dashed wire.
- Title generators (`GeneratorKind::Text`) are events on video tracks; the timeline index (`EventIndex`) answers spans.
- The level of an audio event is `gain_db` with curves (automation); track stages run after the events' sum.

## D1. The two rules

On an audio track, `Ducking`:
- `content`: `reduce_db` (0…−60, −∞; default −12), `fade_ms` (default 250), `key: Set { tracks, roles }`. Realised by
  the R.65 compressor on the track's stage, its rows solved in closed form so a key at the sensitivity level gives
  exactly `reduce_db` (attack = fade/4, release = fade); the compressor's rows dimmed under "Details".
- `titles`: `Raise(db)` (0…+12, default +6) | `Mute` | `Leave`, with `title_fade_ms` (default 400). Realised as a gain
  envelope G(t) from the title spans: 1 outside, the raise or 0 inside, linear fades of `title_fade_ms` centred on each
  span edge; a pure function of the spans (no state), so seek, loop and export agree.
- `Mute` also unmutes: a track tagged "Title sound" whose ducking rule is `titles: OnlyInTitles` plays only inside title
  spans (the same envelope inverted) — the owner's "размут заголовочного звука".

## D2. The key of several tracks and roles

`Set { tracks, roles }`: the key per window is the sum of the post-chain, pre-fader outputs of every event on the
listed tracks and of every event (same reel) whose effective role is in `roles`, the keyed track excluded. Cycles
refused by name. Order: the keyed track renders after every key holder (the R.65 rule).

## D3. Title spans (Q1 a)

`Project::title_spans(reel)`: the spans of Text generator events MINUS the spans in which an event of the content key (its listed tracks, its roles; an audible body, not a silence of its sound) lies, plus the spans of events and tracks tagged
`Title`, minus those tagged `Not a title`; a remainder shorter than `2 × title_fade_ms` is dropped, spans closer than that are merged; indexed per revision. (Revision 3, 2026-11-25: revision 2 took every Text event, so a lower third over speech
raised the music under it.) Moving or trimming a title moves its section.

## D4. Hand edits

The background events' own `gain_db` and its curve are summed in dB AFTER the automatic gain: the person's key at a
spot raises or lowers it there whatever the automation does. The band (D5) shows the automatic part; the event's level
line shows the person's part; "Convert to volume curve" writes the automatic part into the event's curve (summed
with the person's) and turns both rules off — one undo step.

## D5. The band

The analysis lane computes the automatic gain over the project (the compressor's detector and gain code over the key's
loudness, times G(t)), cached by the content hash of the key events, the title spans and the rules; drawn as a shaded
band under each background event's level line; matches the rendered gain within 0.5 dB.

## D6. Format and OTIO

Roles, `Ducking` and `Set` keys by content (the next free `.kt` version when present); a project with R.65's single
track key reads as `Set { tracks: [it], roles: {} }` with no text change. OTIO: in `metadata["kt"]`.

## D7. Class walk — "a mix setting that must follow the edit"

| Setting | Follows the edit today? | Here |
|---|---|---|
| background under speech | yes if the speech is on the one key track | several tracks + roles |
| background during titles | no (by hand) | title spans from the edit |
| title sound only during titles | no (by hand) | `OnlyInTitles` |
| loudness to a target | no | `loudness-norms` |
| crossfades at cuts | yes (transitions) | — |

## D8. Idle

Nothing at rest; the band once per content hash on a background lane, paused by playback.

## Rejected

- **A loudness-only detector for titles** — a quiet title would not count; the edit knows where the titles are.
- **A generated curve as the only result** (Premiere's keys) — stale after a re-edit; the owner keeps hand edits on top
  instead.
