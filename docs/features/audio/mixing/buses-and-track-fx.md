---
id: F-AUD-004
title: Buses, track FX, sends and solo/mute
status: in-progress
priority: B
cost: XL
category: audio/mixing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-fx, kt-otio, kt-app]
invariants: [1, 2, 3, 6, 11]
ui_rules: [13, 17]
round: R.62
point: null
tests: [t732, t732b, t733, t733b, t734, t739, t744]
depends_on: [F-AUD-002, F-MPH-005]
checked: 2026-09-07
---

# F-AUD-004 — Buses, track FX, sends and solo/mute

## Essence

A processing level BETWEEN the clip and the master. A track gets its own fader, mute/solo and
effects chain; several tracks can be merged into a common bus (submix) and processed at once,
or a COPY of the signal can be sent to a bus (send/return) and the processed copy mixed in with
the unprocessed one. The practical meaning is exactly three things: one reverb for ten lines
instead of ten reverbs, one compressor for the whole dialogue, and separate stems
dialogue/music/effects in one render — the last is not a convenience but an item of the
broadcast delivery specification.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Audio Bus Tracks · Assignable FX · Mixing Console | inserts on a track/bus + sends; "An audio bus track exists for each bus or assignable effects chain in your project"; up to 32 assignable chains of 32 plugins each; mute/solo on a track, a bus and a chain; a send to a bus is PRE-fader by default |
| Adobe Premiere | 26.x | Audio Track Mixer · Submix track · Sends | per-track AUDIO effects exist (unlike video, F-MPH-005); a submix is a track of a special type to which the output of ordinary tracks is switched; up to five sends per track; the type of submix (mono/stereo) is set at creation and does not change |
| DaVinci Resolve | 21 | Fairlight FlexBus | "Instead of fixed aux, sub and main busses, FlexBus has multi purpose busses, up to 36 channels wide, that allow every track to send post fader information to as many as 20 sets of busses, with each set containing up to 3 busses, for a total of 60 destinations"; each channel and bus has 6 insert slots, EQ and dynamics |
| Final Cut Pro | 12.3 | none (roles + audio lanes + compound clips) | there is no concept of a track, hence no buses and no sends; "Roles are metadata text and color labels that you assign to clips"; lanes are only a view; grouping of processing is a compound clip; stems are an export by roles |
| After Effects | 26.x | none | there is no mixer, no tracks and no buses: sound is effects on a layer, grouping is precomposition [NOT VERIFIED: the Adobe help is not opening from this environment] |

## The trick behind it

**1. In VEGAS a send to a bus is PRE-fader AND PRE-mute by default — that is, a track that
is switched off keeps feeding the reverb.** Verbatim: "Bus sends are pre-volume (and pre-mute)
by default. When bus sends are pre-volume, you can create a cue mix that is independent of your
main mix". This is done for the sake of the performer's headphones, but in editing it produces
the classic "the sound is gone, but the reverb has stayed": you pull the fader — the dry signal
leaves, the wet one stays at its previous level, and the tail sounds from nowhere. Fairlight
chose the opposite default — buses take "post fader information", and there the same knob
gives the reverse defect: you pull the fader — the tail disappears too, cut off mid-word.
There is no correct default, so every send has a Pre/Post switch, and every project has its
own set of surprises.

**2. Not every plugin can be put on a bus, and VEGAS colors such chains.** "Using non-in-place
plug-ins (such as Time Stretch, Pitch-Shift) will cause audio to play out of synchronization
with the waveform display in the timeline and with other tracks": a yellow chain icon means
automatic delay compensation is enabled, a red one means the chain is unsuitable for live
monitoring and is BYPASSED automatically. The reason is structural, not whim: a send creates a
second branch of the same signal, the branches must rejoin sample for sample, and a plugin that
changes the length of the material cannot be brought back together in principle. Plugin delay
compensation (plugin delay compensation) is not a decoration of the mixer but the condition of
its existence; it is exactly the same machinery that a lookahead limiter needs (F-AUD-002), and
it makes sense to build it once for both features.

**3. The bus graph has two non-obvious rules, and both are about what ends up in the file.** The
first is the prohibition of cycles: "A bus cannot send to a bus that is directly or indirectly
routed to the Master bus", checked NOT on playback but at the moment the output is chosen. The
second is silent disappearance: a bus can be routed not to Master but to a physical output of
the sound card, and then "outputs from those busses will not be included in the mix when you
render your project". That is, a mixer that sounds correct in the room may render without an
entire branch, and there is no warning at render time. A related pettiness in Premiere: the
type of submix (mono/stereo) is nailed down at the moment of creation — "you can't change it
from Mono or Stereo. You'd need to delete that Submix and create a new one", that is, an error
in routing is cured not by editing but by deleting the node.

## Pros

- One instance of an effect for N sources: one state (one delay line), one CPU cost and one
  parameter edit instead of N — and, more importantly, ONE undo step instead of N.
- An insert on a bus is the only point where the signal is already mixed: the compressor hears
  the sum of the lines and holds the common level. Clip effects have no such point physically,
  no matter how many you place.
- A pre-fader send gives an independent branch: the dry signal can be brought to zero and only
  the wet one left — "reverb only" without a second clip and without duplicating material.
- A track chain is inherited by its content: a new line dropped onto the track is already
  processed — nothing needs copying.
- Stems come out in one render and by construction match the mix: roles/buses are the very
  description of what goes into which stem ("delivering files to match broadcast
  specifications").
- Solo/mute at the level of routing answers the question "what does this branch give", without
  touching a single clip — that is, without editing the model and without the risk of forgetting
  to put things back as they were.

## Cons

- Volume starts living in four places (clip, track, bus, master), and from the look of the
  timeline it is impossible to tell in which of them it was turned down.
- The default of a send (pre or post) is inevitably wrong in half of the cases: pre-fader gives
  "pulled the sound, the reverb stayed", post-fader gives "pulled the sound, the tail got cut
  off". The switch is mandatory, which means every send gets one more state that must be stored,
  shown and copied.
- Parallel branches of different length diverge in time, and the host must align them; a plugin
  that changes the length of the material does not yield to compensation at all — VEGAS honestly
  colors it red and switches it off.
- A track chain is not an object that can be copied: a track is a container, not an event, and
  "copy the track's FX into another project" has been requested in Premiere since 2018.
- A branch routed past Master silently drops out of the render.
- The price of refusing tracks (FCP) is symmetrical: there are neither buses nor sends nor a
  track fader — "one reverb for ten lines" is solved by a compound clip, that is, a nested
  sequence, which has its own costs.

## What users say

> Is there a track mixer that I am not seeing. I know about adjusting
> volume per clip. I want to set db per track.
> — Nathan Kendall, Creative COW, 23.08.2017, https://creativecow.net/forums/thread/lj-cuts-audio-tracksmixer/

It is telling that this is the complaint not of a newcomer to an unfamiliar button but of an
editor to a MISSING model: in FCP there is nothing to answer, because there is no object
"track" — and in the thread they answer him exactly that.

> That doesen't work either. I tried everything: Clip, Track, sending
> Aidoo to a Submix....as soon as the file is finished, the reverb tail
> stops. this is just not professional.
> — Neil Raouf, Adobe Community, 18.04.2023, https://community.adobe.com/questions-729/how-to-get-reverb-to-keep-going-after-the-clip-ends-1339525

The thread has been running since 2018 ("when applying reverb to a clip, the reverb stops
abruptly when the clip stops" — djmattyz, 31.03.2018, same place). This is the most valuable
negative feedback in the note: the existence of a submix does NOT solve the task automatically —
if the host counts sound only where there is a clip, the tail is cut off at any level of
processing.

> For several years, users like me have been looking for a way to copy and
> paste audio track effects.
> — Mr. Positronic, Adobe Community, 21.03.2018, https://community.adobe.com/t5/premiere-pro-discussions/audio-track-effects-workarounds/m-p/9732171

> I put Tonelib GFX and the Ext Send pre-fader. This allowed me to mute the
> track and still hear the reverb from the bus.
> — Majik (Keith Milner), Ardour Discourse, 20.11.2023, https://discourse.ardour.org/t/aux-sends-buses-and-panning/109505

The last quote is from an editor OUTSIDE the core five, given as an illustration of the
mechanics: exactly the behavior that the "trick behind it" section describes as a source of
bewilderment is here switched on DELIBERATELY and is a working technique. This is the argument
for a switch, not for a "correct" default.

## How it fits Kinotrain

### What already exists

- **The mix is a single accumulation into one buffer.** `AudioRenderer::render_frames_with`
  (`src/libs/kt-engine/src/audio.rs`) selects the active clips and adds them directly into the
  output stereo buffer: `out[oi + ch] += b.data[si + ch] * w`, where
  `w = fade_weight × overlap_at × clip.gain`. The header of the file states this in words:
  "different tracks are summed without weights". No intermediate level between the clip and the
  output exists.
- **A track is an index, not an object.** `ClipEvent.lane: u32`
  (`src/libs/kt-model/src/project.rs`, comment: "z-order for video, mix track for audio") — the
  only thing that connects a clip to a track. The track itself has not a single field: the order
  of the rows lives in a separate vector `Project.lane_order: Vec<(LaneKind, u32)>`, and the name
  lies outside undo altogether, in `UiState.lane_names: Vec<(LaneKind, u32, String)>`, and is
  bound "to (kind, index)". `lane_order_insert` / `lane_order_remove` on inserting and removing a
  track RENUMBER the neighbors (test 49).
- **FX only on the clip.** `FxStateStore` (`src/libs/kt-engine/src/fxstore.rs`) is a
  `HashMap<ClipId, ClipProcessor>`; `AudioRenderer::sync_fx` synchronizes it by `clip.id`, and
  `retain_clips` throws out those that have disappeared. The diff by `chain_fingerprint` is
  exactly the "same chain structure → same state object" of invariant No. 3.
- **The FX tail can already sound beyond the end of the clip** — `AudioClip.ext_end` and
  `kt_fx::chain_source_window_ext`: "the reverb rings out beyond the clip boundary — invariant
  tail, defect f of the old engine". That is, Neil Raouf's complaint is closed for us at the
  level of the CLIP; at the level of a bus it would have to be closed anew.
- **The master exists, but it is a monitor.** `Player::set_master_gain`
  (`src/libs/kt-engine/src/player.rs`, ceiling `MASTER_GAIN_MAX = 16.0` after an incident with
  10^20 from the UI). It is applied IN THE AUDIO-RENDER THREAD: a `*s *= gain` loop over the
  chunk, then `meters.fold(&chunk)` and push into the SPSC. In `src/libs/kt-engine/src/export.rs`
  there is NO master gain at all — the fader does not affect the file.
- **Half of the mixer UI is already written.** `src/apps/kt-app/src/meters.rs` — stereo meters
  with peak-hold and decay (2.8 / 0.2 dB per tick), a fader −60…+24 dB with inertia, double-click
  = 0 dB, an M (mute) button. The `meters_col` panel in `src/apps/kt-app/src/app.rs` with its own
  drag strip.
- **The chain editor is ready, but it is addressed by clip.** `src/apps/kt-app/src/fx_editor.rs`:
  `ChainSnapshot = (ClipId, Vec<FxInstance>, …)`, `selected: HashMap<ClipId, usize>`, the window
  id is the event id.

### What is missing

A grep over `src/` (`solo|mute|bus|submix|send|track_fx`) finds not a single track entity:
`fade_in_solo`/`fade_out_solo` in `src/libs/kt-engine/src/video.rs` is a solo fade of a
transition (another meaning of the word), and `muted` in `meters.rs` is the master monitor.
There is no track fader, mute, solo, track FX, buses, sends or stems.

### What gets in the way

1. **A bus has no clip id — and this is not a trifle but a question about the identity of a
   track as such.** Invariant No. 3 rests on `ClipId`: rebuilding the `AudioRenderer` on any
   edit of the model does not kill the FX state precisely because the key is stable. The key
   `(LaneKind, u32)` will not do: `lane_order_insert`/`lane_order_remove` renumber the tracks,
   and inserting one track at the top would destroy the state of all chains below — these are
   literally defects b/f of the old engine, for whose sake the invariant was written. So a bus
   (and a track, if it has a chain) needs its own stable id of the `ClipId`/`GroupId` level, and
   `FxStateStore` becomes a composite key. **Introducing buses = creating in the project the
   first track-object**, which does not exist now; and along the way one has to decide what
   deleting a track does to this object, the copy-paste of a block (item 242) and the bake of a
   morpher.
2. **`ClipProcessor` has the shape of a clip.** Its constructor is
   `new(fx, auto, src_in_frames, clip_len_frames)`; it pulls material through the `pull`
   callback from the MEDIA, normalizes automation to `clip_len`, knows `head_ext`/`tail_ext`,
   reverse, `seam` and the warp "output → material". A bus processor has neither a source nor a
   length: it processes a stream. Either a second type of processor appears, or `ClipProcessor`
   splits into "material fetcher" and "chain" — and that is an edit of kt-fx, whose every kernel
   is byte-frozen.
3. **The automation domain of a bus has to be chosen anew.** For a clip the key is stored in
   the tick of the MATERIAL (R.33/R.37, item 246) — that is exactly why trim, split and reverse
   do not move the curves. A bus has no material; the honest answer is the tick of the timeline,
   and that is a SECOND domain in one project. VEGAS arrived at exactly the same thing (a bus of
   events has no events, so its keys are always in project time — analyzed in F-MPH-005), and
   this has already cost them a global switch `Lock Envelopes to Events`, that is, an application
   setting instead of a property of the object. Our only curve widget
   (`src/apps/kt-app/src/automation.rs`) will have to be taught a second domain.
4. **The mix turns from a sum into a graph.** Now it is one accumulation into `out`; buses are N
   intermediate buffers, a topological order and a check of acyclicity (VEGAS formulates it
   literally: "A bus cannot send to a bus that is directly or indirectly routed to the Master
   bus"). Moreover, the TAIL of a bus is computed separately: `ext_end` is today derived from the
   chain of the CLIP, while the tail of a bus reverb belongs to no clip — not computing it means
   reproducing Neil Raouf's complaint in our own code.
5. **Our only heavy audio effect is precisely non-in-place.** `SlowedReverb` changes the length of
   the material (warp, `rate`), and since R.37 it also synchronizes in an A/V pair
   (`sync_warp_to_link_group`, item 248). This is exactly the class that VEGAS colors red on a bus
   and bypasses. It cannot be put on a bus without delay compensation; the compensation is the
   common machinery shared with the lookahead limiter (F-AUD-002).
6. **Format.** `KT_META_VERSION = 4` (`src/libs/kt-otio/src/meta.rs`) → v5. OTIO has no buses, so
   they live entirely in the envelope `metadata.kt`, like `lane_order` and named areas; an old
   build must refuse (`KtMetaTooNew`) rather than silently reduce a project without its bus.
7. **Monitor versus render.** The master fader today does not get into the file. The bus fader
   gets in by definition. A first level of volume will appear that lives in the model and affects
   the export — next to a master that does not affect it. This must be separated in the UI,
   otherwise we get "mixed in the monitor, delivered something else".

### Roles instead of tracks: why this may turn out cheaper for us

The FCP model here is not an exotic one but a direct hit on our design.

- A role is a **label on a clip**: "Roles are metadata text and color labels that you assign
  to clips". This is a field of the same class as the existing `ClipEvent.link_group:
  Option<GroupId>`, and it does NOT create an object with state. Problem No. 1 ("a track has no
  identity") is not solved in this case but disappears: a role has its own id, and the
  renumbering of tracks is indifferent to it.
- Our audio tracks are already in fact "only a view": the summation goes without weights, a track
  has no fields, and the order of the rows is a separate vector `Project.lane_order`. FCP says the
  same thing about its own lanes, literally: "Organizing the timeline in this way does not affect
  the content of your project or how it plays back in the viewer — it simply adjusts the timeline
  appearance". That is, we are ALREADY in this model, and introducing a track-object breaks it,
  while introducing a role continues it.
- The mechanism "name a set of events" in the project already exists too:
  `MorphTargets::{Clips, Marks, Areas}` and named areas (`src/libs/kt-model/src/project.rs`,
  item 241). A role stands next to them and reuses the same idea of addressing.
- Stems by roles are N export passes with a filter by label, not routing:
  `src/libs/kt-engine/src/export.rs` is called several times, and the mix graph does not change at
  all.
- **But a role does not cover everything.** A role gives GROUPING (a common insert for a set, a
  common level, a stem) and does not give a SEND — a parallel branch with its own level. "One
  reverb for ten lines" through roles is solved the same way as in FCP: by a nested clip, that is,
  by something we do not have (compound clips). The honest division is therefore: **roles = the
  cheap half (group + stem + group solo/mute), buses = the expensive half (send, parallel branch,
  delay compensation)**.

### Which invariants it touches

- **No. 1 (audio always f32 48 kHz/2ch)** — not touched: a bus works in the same format as
  everything else. This, by the way, is a rare case where the invariant HELPS: a bus cannot have
  its "own" sample rate.
- **No. 2 (FX state is touched only by the audio-render thread)** — fits without a fight. A bus
  is a new node of the graph IN THE SAME thread, next to `FxStateStore`; no mutexes around FX
  appear, and prefetch still fills only the pre-FX cache. This is the main positive answer of the
  note.
- **No. 3 (a model edit does not kill FX state)** — touched directly, see item 1 of "What gets in
  the way": a bus has no clip id, a new identity key is needed.
- **No. 6 (the cpal callback is the master clock)** — NOT touched, and this has to be said aloud:
  the master gain and the meters are computed in the render thread, the callback only reads the
  SPSC. The faders, mute and solo of buses must travel the same way — by atomics (like
  `master_gain`) or by rebuilding the graph on a change of model version — and in no case be read
  from the callback.
- **No. 11 (determinism, audio byte-for-byte)** — touched: a project without buses must be
  computed by the old path (an empty set of buses = identity), otherwise GOLDEN_AUDIO would have to
  be regenerated, and that must not be done.
- **UI rule 13 ("do not waste space")** — the meters panel cannot be widened: its width is clamped
  by `METERS_W_MIN = 64` … `METERS_W_MAX = 90` (`src/apps/kt-app/src/app.rs`), the ceiling is
  checked by t101 and was set at the owner's request (item 6 of R.13). And the vertical volume rail
  of v1 was already removed AT HIS DIRECT REQUEST: "the fade window was drawn the same way, and
  the vertical volume rail of v1 inside the fade zone distinguished them… The rail was removed"
  (`docs/user-feedback.md`, "After R.28 — uniformity of level", port item 176). Conclusion: there
  must be no permanent columns and rails on a clip/track — **the mixer is a SEPARATE panel, opened
  explicitly**. This is a lawful way around rule 13, not a violation of it: space is used when it
  has been asked to be opened, and is released together with the panel.
- **UI rule 17 ("paired ways of doing the same thing")** — the volume of a track/bus and the
  `gain_db` of a clip (the line along the top of the event, item 176) would become two ways of
  doing "quieter". They must be separated, otherwise we repeat the flaw of others' mixers: "turned
  it down in one of four places".

## Estimate

| What | Answer |
|---|---|
| Scope | XL — a round plus a version of the format and a new level in the mix graph |
| Where the main work is | kt-engine (`audio.rs` — a graph instead of a sum, `fxstore.rs` — an identity key, the tail of a bus), kt-fx (a processor without a source), kt-model (a bus/role object + id + undo + renumbering of tracks), kt-otio (v5), kt-app (a mixer panel, the second domain of curves) |
| Project format | a v5 version is needed (`KT_META_V5`); files without buses are byte-stable, an old reader must refuse rather than silently lose the bus |
| Tests that will have to be written | a project without buses == GOLDEN_AUDIO byte-for-byte; "two tracks into a bus" == direct sum; pre-fader versus post-fader send (muting the track with pre — the wet stays); the tail of a bus reverb beyond the end of the last clip; refusal on a cycle in the bus graph; SURVIVAL of the bus chain state on insertion/deletion/reordering of tracks (analog of invariant No. 3 for the new key); solo/mute identical in preview and in export; round-trip of the model and .otio; stems == sum of stems |
| Depends on | F-AUD-002 (delay compensation is common machinery; without it a send with a warp breaks sync), F-MPH-005 (the same question "a level above the event", the video side; the Audio Bus Track is a direct relative of the Video Bus Track) |
| Breaks compatibility | no (an empty set of buses = identity of the old path) |

## Verdict

Take it, but in two parts and not all at once. **The first part — a track as a target: level,
mute and solo without chains.** It requires neither a bus processor, nor a second domain of
automation, nor delay compensation: by its structure it is one more multiplier in the place where
`fade_weight × overlap_at × clip.gain` is now, and it closes the most frequent complaint of editors
("I want to set dB per track"). The estimate for this part is S–M, not XL.

**The second part — buses and sends — do not start before F-AUD-002.** Our only heavy audio effect
changes the length of the material, that is, belongs exactly to the class that VEGAS disables
automatically on a bus; without delay compensation the send will break the A/V sync rather than help.

**The third, separate: roles.** Before copying the classic mixer, one should seriously consider the
FCP model — for us tracks are no longer objects, and the order of the rows already lives in
`Project.lane_order`. Roles give grouping, a common insert and stems cheaply and without a new
track-object; they do not give a send, and this must be admitted honestly, not passed off as the
same thing.

## Sources

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/bus_tracks-audio.htm — Audio bus tracks, VEGAS Pro 2026 (accessed 2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/9-audio/spconsoleopt_audio_track_channel_strips.htm — track sends, Pre/Post, mute/solo, Track FX; "Bus sends are pre-volume (and pre-mute) by default" (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/9-audio/spconsoleopt_bus_channel_strips.htm — bus channel strips: the ban on cycles, output to hardware does not get into the render (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/9-audio/spconsoleopt_assignable_effects_channel_strips.htm — assignable FX channel strips, mute/solo of chains (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/9-audio/spconsoleopt_creating_a_cue_mix_with_the_mixing_console.htm — why a pre-volume send (cue mix) (2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/9-audio/using_assignable_fx.htm — up to 32 assignable chains of 32 plugins; a yellow/red chain during monitoring (2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/9-audio/adding_audioeffects.htm — non-in-place plugins and desynchronization, automatic delay compensation (2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/9-audio/businsertfx.htm — bus effects, "track effects will be processed before the bus effects", pre/post fader for bus inserts (2026-09-07)
- https://help.magix-hub.com/video/vegas/en/content/topics/busassigntracks.htm — assigning tracks to buses, Post Volume via right click (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/fairlight — FlexBus: 20 sets of 3 buses, 60 destinations, post fader; 6 inserts per channel (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/intro-to-roles-verb71cbcbe/12.3/mac/15.6 — roles as clip metadata, five default roles, stems (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/organize-the-timeline-with-audio-lanes-verb71cb913/12.3/mac/15.6 — "does not affect the content of your project… simply adjusts the timeline appearance" (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/export-roles-verb71cb7e5/12.3/mac/15.6 — exporting roles as stems, "to match broadcast specifications" (2026-09-07)
- https://larryjordan.com/articles/premiere-pro-cc-audio-mixer-sends-and-buses/ — Premiere: creating a submix, up to five sends, an unchangeable submix type (29.06.2016, updated 15.02.2025; accessed 2026-09-07)
- https://creativecow.net/forums/thread/lj-cuts-audio-tracksmixer/ — "I want to set db per track" (2026-09-07)
- https://creativecow.net/forums/thread/add-tracks-to-timeline/ — "There are no tracks in Final Cut Pro X" (2026-09-07)
- https://community.adobe.com/questions-729/how-to-get-reverb-to-keep-going-after-the-clip-ends-1339525 — the reverb tail is cut off on the clip, on the track and on the submix (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/audio-track-effects-workarounds/m-p/9732171 — track FX cannot be copied (2026-09-07)
- https://discourse.ardour.org/t/aux-sends-buses-and-panning/109505 — a pre-fader send as a deliberate technique (an editor outside the core five) (2026-09-07)

Verification limits: `helpx.adobe.com`, `reddit.com`, `discussions.apple.com` and
`forum.blackmagicdesign.com` did not open from this environment (the last one answered 403 to all
threads), therefore the rows for Premiere and After Effects rest on the official Adobe community
and secondary sources, and Fairlight rests on the Blackmagic product page, not on the Reference
Manual. None of the five editors was checked live: the note was assembled from help and forums.
The quotes from the VEGAS help are given in the form in which the pages were supplied on 07.09.2026.

## Kinotrain: live sidechain — change `sidechain-ducking-compressor` (R.63)

In R.63 the owner took our auto-ducking apart piece by piece (the name is not an industry one
("duck under a track"), control goes through its own window, the parameters are not remembered,
there are no curves) and gave a conceptual remark: **"as ducking one can show the same duplicated
audio-compressor widget with the fields dimmed, i.e. a compressor on the background object and a
compressor on the foreground object"**. Asked directly, he chose a LIVE SIDECHAIN COMPRESSOR instead
of a curve snapshot.

What has been done in R.63 (and this is exactly as much as can be done without touching the
engine): an industry name, a section in the audio-track window instead of its own window, the
parameters in `user_store`. The curve snapshot remained a snapshot.

The next change — `sidechain-ducking-compressor`: a compressor with a CONTROL INPUT (the speech
track), that is, a real dynamics in the audio graph, and not a pre-recorded envelope. Then the caveat
"this is a snapshot: you changed the voice — call it again" also disappears, and ducking ceases to be
a separate entity: it is a compressor whose sidechain listens to its neighbor.
