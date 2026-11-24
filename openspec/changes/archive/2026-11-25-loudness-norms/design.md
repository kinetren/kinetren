# Design: loudness-norms

> Revision 3 (2026-11-24), base `286c406c`; built 2026-11-25, what differs is D10. Revision 2 (2026-11-22, the owner's B6) put the norm in the
> project; revision 1 in the render settings. What revision 3 corrected is D8.

## Context

- The meter (`kt_fx::audio::loudness`, BS.1770-4 in f64, ×4 true peak with `TP_TAPS` 8) is fed by the export pass
  (`export.rs:171–188`) and the peak-pyramid pass.
- The player's master gain is monitoring only (`player.rs` `audio_render_loop`, after `mix.render`); the mix ends there.
- The kept mix (`mix.rs`, audio-render "A covered range keeps its final mix") holds what the renderer produced, keyed by
  `scene_hash`; `durable-history-and-mix` will keep the same bursts on disk.
- The R.48 limiter (`fx-audio-dynamics`, `Limiter`) is a SAMPLE-peak limiter: its sliding maximum is of `|x|`, its
  window is at least 1 ms, and its threshold below its ceiling adds gain. It cannot hold a dBTP ceiling.
- Background analyses: `analysis.rs`, one job at a time, kinds locked to `Loudness(MediaId)` by t774.

The professional path, read 2026-11-22:
- Resolve: the target loudness in Project Settings → Fairlight (default −23 LUFS, −2 dBTP), clips analysed by BS.1770-4
  and normalized to a target with a true-peak target; the Deliver page's "normalize to standard" scales until the
  loudness or the peak limit governs. (Blackmagic forum threads "Audio normalization ignores BS1770 target loudness",
  "17.2.1 Audio normalization and fairlight meter disagree", "Loudness Normalisation not standard"; davinciresolveclub
  "audio levels LUFS"; Hollyland "How to Normalize Audio in DaVinci Resolve".)
- Premiere: Essential Sound Auto-Match sets each clip's gain so clips of a type reach a loudness (−23 LUFS reported);
  short clips are miscounted (Adobe community "premiere pro does not automatch loudness correctly", "auto match volume
  is simply lowering volume", "auto match loudness problem when cutting dialogue"; Adobe help "create audio mix").
- [NOT VERIFIED against the manuals: the sources are forums and vendor blogs.]

## D1. The project target

`ProjectSettings.loudness`: `standard` (EBU R128 −23/−1, ATSC A/85 −24/−2, Streaming −14/−1, Podcast −16/−1, Custom),
`target_lufs`, `ceiling_dbtp`. Not written at its default (EBU R128). It is the DOCUMENT's: the settings window opened
inside a reel edits the reel (`settings_window.rs`, `is_reel`) and shows no loudness rows.

## D2. Match loudness of clips

The job renders each chosen event ALONE (a document copy with that one event, its chain, its curves, its track stage
left out) over its span with `render_raw_audio_range(…, program: None)` into a meter; then `gain_db += target − lufs`,
clamped to the gain's passport. Events shorter than 3 s on one track from one source are one material (their renders fed
to one meter in timeline order) and get one shared correction: the gated average needs at least 3 s. An event with no
number (silence) is left and named in the finishing notice. One `SetGain` batch, one undo. The targets are the rows of
the small window (project target, −8 LU, −4 LU, a number); the last pick is remembered by the user store.

## D3. The program stage — at the output, after the kept mix

`Project.program: Option<ProgramStage>`; `None` (every project until the person asks) builds nothing and the path is
byte for byte today's.

**Where.** Two places were possible: inside the renderer (the last track stage) or at the output after the kept mix.
Inside, a change of the gain would change `scene_base`, drop the whole kept mix (and the disk runs), and every "Match the
program" would make three minutes render again; the look-ahead would move every event's window. At the output the kept
mix and its key are untouched, and the stage costs one gain and one limiter per played frame, beside the monitoring
gain. Taken: the output. Callers, all of them:

| Caller | Stage |
|---|---|
| `player.rs` `audio_render_loop`, forward | the stage, then the monitoring gain, then the meters |
| the same, reverse (`render_*_raw`, effects dry) | gain and a clamp at the ceiling, no look-ahead |
| `Player::sample_levels_at` (a paused burst) | gain and the clamp: a level, not a sound |
| `export::render_raw_audio_range` ← `export.rs:837`, `export_pipeline.rs:789` | the document's stage |
| the same ← `render_raw_audio` (tests, golden) | none: the golden stream is the mix |
| the same ← `reel_preview::reel_envelope`, D2's and D4's jobs | none: a reel, an event, an unmatched mix |
| a reel opened for editing in the player | none: a reel is not the program |
| `kt-cli export` | the document's stage; `probe` prints its gain beside the mix |
| `kt-cli main.rs:612, 620` (`--time`) | none: it times the mix |

**The look-ahead.** `LA` = 72 frames + half the interpolator. The stage is fed the mix `LA` ahead of what it emits: at a
seek, a start or a loop change it is reset and primed with the `LA` frames at the head; each chunk then reads
`mix.render` at `head + LA`. The wrap of an armed loop is inside `mix.render` (the folded head), so the limiter hears the
lap that really comes next. An export primes the same way and reads `LA` frames past its range (silence past the end),
so the frame count does not change. With the stage absent `LA` is 0.

**The kernel** (`kt_fx::audio::program`): detection by the METER's own interpolator (the same table, so the limiter
holds what the report will measure), a sliding maximum over `LA`, the reduction smoothed by a box of `LA` (a linear
attack that has finished when the peak arrives), release 50 ms, an inner ceiling 0.1 dB under the project's for the gain's
own modulation; the gain smoothed over 20 ms when it changes. `kt_plugin_api::fast!`, frozen math, no allocation.

## D4. "Match the program"

`AnalysisKind::Program`: pass 1 renders the whole document mix with no stage into the meter. `g0 = target − lufs`. When
`dbtp + g0 ≤ ceiling` the limiter never acts and `g0` is exact. Otherwise the limiter takes loudness away, so pass 2
renders once more through four stage copies at `g0, g0+1, g0+2, g0+3` dB, four meters (the copies are cheap next to the
decode), and the gain is interpolated to the target; when even `g0+3` falls short, `g0+3` is taken and the stage says
"the limiter holds back N dB: the mix is too dynamic for this target". `SetProgram` writes `gain_db` and
`measured = { lufs, dbtp, sound }`; one undo step. A result whose `sound` no longer matches the document is dropped.

`sound` is what is heard, not a revision: xxh3 of the `.kt` text of the effective audio events, the audio tracks' own
fields, the audio transitions and, by the same rule, every reel they play. A moved title does not touch it; a revision
counter would flag every edit and would mean nothing after a reopen. It is computed on the builder thread when the
stage exists and a new version lands.

## D5. The report

The branch's table (`loudness_rows`, `Verdict`), the norms read from D1: integrated within 1 LU of the target plain,
within 3 amber, past it or louder than −9 red; peak over the ceiling red; range above 20 LU amber; channels 3 LU apart
amber, 6 LU or one silent red. `ExportStats.loudness` gains `spans: Vec<SoundSpan>` (over the ceiling, a silent channel
over 1 s, an imbalance over 1 s), drawn under the table as rows that move the playhead. `render-result-check`'s lines
stand above it, `picture-check` adds its kinds to the same list.

## D6. Class walk — "a level decided per file instead of per project"

| Level | Today | Here |
|---|---|---|
| program loudness | a grey reference line at render | D3, D4 |
| clip loudness by type | by hand | D2 |
| background under speech | the keyed compressor by hand | `auto-ducking-by-role` |
| true peak ceiling | the limiter effect, which holds sample peaks | D3 on the master; the effect's passport says "ceiling" for samples: its hint says so |
| render presets | no norm | stays so |
| the bin card's loudness of a source | a number without a norm | left: a raw file has no delivery norm |
| the meters' scale | dBFS zones | left; the stage shows LUFS beside |
| kt-cli export | prints nothing of loudness | prints the report's lines |

## D7. Idle

Jobs run only on request on the analysis lane, paused while playing; the stage runs per played frame and nowhere else;
`sound` once per landed version, off the UI thread, only while a stage exists.

## D8. What revision 3 corrected

- The base: the unmerged branch, its tests and its note id (F-EXP-009 now belongs to `picture-check`; the report's note
  becomes the next free one at the merge: the channel branch adds export notes too).
- The limiter: the R.48 effect has no ×4 detection and cannot promise dBTP; the stage has a kernel of its own.
- The stage was "always on unless switched off": every existing project's bytes would change. It is absent by default.
- The place relative to the kept mix, which had none; the callers of `render_raw_audio_range`, one of which renders reels.
- The gain computed without the limiter missed the target whenever the limiter acted (t3919 expected both −14 LUFS and
  a gain that could not give it).
- `measured.revision` did not survive a reopen.
- Roles do not exist yet: the window's rows replace them.
- `ExportReport` does not exist: the type is `ExportStats`.

## D9. Against `loudness-target` (the branch `claude/youtube-channel-specs`, 2026-11-20, unmerged)

Another session designed the same claim two days before B6, with the target in the render settings ("NOT in the
project"), normalization in a measuring pass of each export, and a "Listen as delivered" switch. B6 answers against its
first half in the owner's words ("на уровне проекта нормализация а не в пресетах рендера"), and with the stage in the
project the preview always plays what is delivered, so the switch has no work. Its Q60 (a limiter or no limiter) and Q61
(the fader) fall: the professional path has the limiter, and the fader stays a monitor. Taken from it:
- the meter's interpolator gets one home that the stage's detector reads too (`loudness::TruePeakInterp`);
- the report's fifth row, what the stage did: the gain, what the limiter took in LU and its deepest reduction, amber past
  1 LU; the line "measured before the codec"; a red row is a line of the render log too;
- `kt-cli export` prints the same rows.
Not taken: a memo of the measurement (the stage stores it), a −14 factory render preset, the target as render keys.

## D10. As built

- The target and the stage are fields of `Project`, not of `ProjectSettings` (216 literals of it); `swap_reel` leaves
  both, `reel_document` and `reel_as_root` clear the stage.
- `LOOKAHEAD` is 80 frames: a box of 72 and the interpolator's 8, and the reduction is held 88 — found by asking which
  samples a peak was computed from. t4201 was green with it at the first run; the noise at +12 dB reads −1.1 dBTP.
- A stage taken away while it plays stays as a plain delay until the next seek: the head does not jump by 1.7 ms.
- `measured` carries the target, the ceiling and the gain it was found for: "changed since measured" covers another
  target and a hand-moved gain, and "the limiter holds back" is said only of the state that was measured.
- `sound` takes a media's path and not its length: the length is read anew when the file opens (t3923 found it).
- An event's level has at most +6.02 dB: what the match cannot reach is named ("the level has no more travel").
- A missing file refuses the match by name: a placeholder is silence, and the number would be of another mix.
- The match needs no window of its own per kind: one dialog, its last pick in the store.
- Clipped samples are not a finding of their own: the true peak over the ceiling is at each of them.
- 4.2: no parameter has a hint of its own, so the limiter effect's row says it in its name ("Sample-peak ceiling").
- The program window opened fitted to its content without a stage and did not grow when the stage came: its gain and
  the "changed" line lay under the fold. Found by the live scenario; it has a size of its own, and the scenario asks.

## Rejected

- **The norm in render presets** (revision 1) — the owner: "на уровне проекта … а не в пресетах рендера".
- **Normalizing the rendered file afterwards** — another encode; the stage is in the mix.
- **The stage inside the renderer** — D3.
- **A gain that follows edits** — proposal, Decided.
