---
id: F-TL-005
title: Multicam editing
status: researched
priority: C
cost: XL
category: timeline/structure
editors: [vegas-pro-2026, premiere-26, resolve-21, resolve-20, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-engine, kt-cache, kt-app, kt-otio]
invariants: [3, 4, 5, 7, 8, 12]
ui_rules: [2, 9, 12, 13, 17]
round: null
point: null
tests: []
depends_on: [F-SPD-004, F-TL-004]
checked: 2026-09-07
---

# F-TL-005 — Multicam editing

## Essence

One event is shot by three to nine cameras at once: a concert, an interview on
two cameras and a lapel mic, a stream with a switcher. The editor sees all the cameras at once in a
grid, starts playback and presses the digits 1–9, like a director at the switcher —
the programme itself cuts and switches the camera at the moment of the keypress. In one pass
a rough cut of the whole take is obtained. The joints are then corrected with an ordinary
roll trim: the switch point is simply a splice. At the end the multicam clip can be
"expanded" into ordinary events and worked with as usual.

## Who has it and how

The key question for all of them is one: **what the multicam clip is structurally** —
a nested sequence, a container in the bin, or a set of alternatives on one
event. Everything else depends on this: where the effects go, what "expansion" does
and what happens to the sound.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Multicamera editing (`Tools → Multicamera`) | **not nesting, but TAKES**: several tracks are merged into one multicamera track, each event carries several takes, exactly one is active. The grid is right in the preview window ("a multicamera tiled view showing the contents of all takes simultaneously. The active take is highlighted with a colored border"). Choosing a take "splits the event at the call point and switches the active take". Backwards — "Expand to multiple tracks" with an option to delete unused takes |
| Adobe Premiere | 26 | Multi-camera source sequence | **a nested source sequence**: from the bin a source sequence is created, it is placed on the timeline as a clip, Multi-Camera is switched on in the Program Monitor; switching by digits. A separate toggle "Multi-Camera Audio Follows Video" (visible in `Edit → Keyboard Shortcuts`). The reverse operation — Flatten |
| DaVinci Resolve | 21 (and 20) | Multicam Clip (Edit) **and** Sync Bin (Cut) — two different mechanisms | Multicam Clip: "full multicam interface … with 4, 9, 16, 25 or more cameras", "you can even step into the multicam clip to trim and color correct angles independently". Sync Bin is **fundamentally different** — there is no "multicam clip" object at all: the bin shows the clips synchronous with the current point of the timeline, and source overwrite places the selected one "perfectly synced to the clip below". BMD explicitly calls this "much better than a traditional multi-cam feature". Studio-only: AI Multicam SmartSwitch (Resolve 20) |
| Final Cut Pro | 12.3 | Multicam clip + Angle Viewer + angle editor | **a container in the browser** that groups existing clips of the event. Angle Viewer — "banks of 2, 4, 9, or 16 angles"; a click = cut&switch (cuts and switches), Option-click = switch (changes the angle without a cut). The angle editor opens by double-click, the angles are rows; the parent's edits are "inherited by all of its child clips, in all projects" |
| After Effects | 26 | none | nested compositions exist, but there is no editing multicam — AE has no model at all of "an event shot by N cameras" |

## The trick behind it

**First: switching an angle is not an edit but a change of parameter, and therefore different
PROPERTIES of the clip have different fates on switching.** Apple documents this
literally, item by item: when the angle is changed, "Video and audio effects", "Keyframing
(including audio volume and panning)" and "Role assignments" are bound to the
specific angle and are NOT carried over, while split edits, markers, connected clips and
retiming stay in place. That is, one and the same gesture is a harmless change of parameter for
half of the properties, and a silent loss of work for the other half:
you coloured the frame, changed your mind, switched the angle — the grade disappeared, while the slow-motion stayed.
And there is a mirror trap at the other end: an effect laid on the multicam clip
FROM OUTSIDE is applied to what is playing, that is, to all the angles at once, including
those that will never appear in this splice. Resolve separates these two places
explicitly ("step into the multicam clip to trim and color correct angles
independently"), FCP — with two different surfaces (the timeline and the angle editor).
No list of features says anything about this.

**Second: when switching, the sound should almost never follow the video — and
this is exactly the place where everyone trips.** The default "we switch both" is physically correct for a concert, where each camera has its own meaningful
source, and harmful for speech: every splice changes the microphone, and together with it
the timbre, room tone and level. Therefore Premiere has a separate toggle
"Multi-Camera Audio Follows Video", FCP has three modes (both / video only /
audio only, the last two via a modifier), and Resolve 21 added a variant
"All Angles". The VEGAS help is silent about sound in multicamera mode altogether.
The practical recipe everywhere is one: the clean mix of the recorder lies on a separate
continuous track, and ONLY the video is switched.

**Third: Vegas honestly writes the price of the grid, and this price is disabled processing.**
In multicamera mode "video envelopes, effects, and motion are bypassed in
the Video Preview window to preserve your playback frame rate", plus separately:
"VEGAS Pro can preview many takes at once in multicamera mode, but your computer
performance will limit the playback frame rate". That is, the grid of angles is not
N honest previews but N raw decodes, and everything that makes the picture correct is
switched off exactly at the moment the editor makes the decision. This is not
a shortcoming but the only way to show nine streams in real time
on an ordinary machine: N angles = N simultaneous decodes, and no algorithm
cancels this.

**Fourth, about the advertising: "AI Multicam SmartSwitch" (Resolve 20, Studio) is
an automatic placement of switches by the active speaker.** According to the analysis by davinciresolveclub, the tool "analyzes the active speaker, audio, lip movement
and whether an angle is wide or close", and there, honestly: "The result is a
starting timeline, not a finished edit" — it gets confused by off-screen voices,
overlapping speech, laughter, silent reactions, and an angle without useful sound.
[NOT VERIFIED against the primary source: on the current blackmagicdesign.com pages
(Resolve 21) the word SmartSwitch is no longer there — the feature is described only in secondary
sources and in the archive of the Resolve 20 announcement.]

## Pros

- Synchronization lives in ONE place for the whole project: an edit inside a multicam clip propagates to all episodes at once (FCP: changes to the parent are "inherited by all of its child clips, in all projects"). Without an object, every episode would have to be fixed separately.
- Switching is a mutation of a single field, not a pair of "cut + replace the source": a concert with 200 splices gives 200 parameter changes, not 400 editing commands, and re-cutting on top of an old cut leaves no garbage.
- The switch point is structurally an ordinary junction, so an edit after a live pass needs no new operation at all: the same roll trim works as everywhere else.
- Sync Bin wins through what it does NOT have: an object. There is no need to create a multicam clip, to unpack it back, or to decide where the effects live — ordinary clips sit on the timeline right away.
- A live pass gives a rough cut in the time of the duplicate: a 40-minute interview is "edited" roughly in 40 minutes, not in five hours of manual searching for splices.

## Cons

- N angles = N simultaneous decodes, and the cost is paid in frames per second at exactly the moment when responsiveness is needed. It is worked around either with proxies (FCP explicitly advises "Set Final Cut Pro to use proxy media" and "Create optimized media for multicam clips") or by turning processing off (Vegas). Both workarounds mean that the editor makes the decision NOT on the picture they will actually get.
- Effects attached to an angle are lost on switching (FCP), while effects placed outside apply to all angles. The same phrase "applied an effect to the multicam clip" means two different results, and which one depends on the surface you were on.
- The default "audio follows video" spoils speech, and it is cured not by a clip setting but by rebuilding the whole project: a separate continuous track of clean audio, with only the video switching.
- A multicam is useless without synchronization, and synchronization is ONE number for the whole take (see F-SPD-004): if the clocks of the camera and the recorder drift apart, a take synced by the clapper slides apart toward its end, and the multicam clip slides apart with it, as a whole.
- SmartSwitch saves the first pass and adds a third: according to the analysis, the result requires three viewings (speaker accuracy, pacing, junctions and technical quality), because the automatic tool confidently errs on laughter and voice-over.
- [NOT VERIFIED] From a search-engine snippet for the topic at forum.blackmagicdesign.com t=201395 ("Very slow playback"), the multicam angles in Resolve "might still be calculated during playback even though their viewer is hidden" — that is, the grid's cost is paid even by those who have closed it. The thread itself returns 403, and this could not be confirmed; the item is kept as a hypothesis about a class of defect, not as a fact.

## What users say

> The problem being that I have two characters and therefore two audio tracks
> but when I do my multicam sequence I set the synchronization to the audio but
> the two microphones are off the multicam. that is to say that the multicam
> does not automatically cut the correct audio track but that of the camera.
> — Elisa35574238nk52, Adobe Support Community, 05.01.2025,
> https://community.adobe.com/t5/premiere-pro-discussions/multicam-and-some-micro-problem-sound/m-p/15074101

This is exactly the scenario for which multicam is set up (an interview on two cameras and two lapel mics), and the first thing a person trips over is not the switching of the picture but the sound: the multicam took the CAMERA's microphone, because it synchronized by it. The expert in the same thread answers not with "turn on the option" but with "open the sequence and go through the audio tracks by hand".

I additionally searched for feedback on performance ("multicam performance", "FCP multicam vs premiere", "resolve sync bin multicam") — threads were found (forum.blackmagicdesign.com: "Multicam 2x2 performance issue", 12.07.2025; "Choppy multicam playback", four angles; "Switch Multicam Angle grayed out", 08.01.2026), but forum.blackmagicdesign.com returns 403 for a direct load of the thread, the mobile mirror fails the certificate check, and reddit is unavailable from this environment. I did not quote from the search-engine snippets.

## How it fits Kinotrain

### What already exists

**The main thing — a correction to the premise. We ALREADY HAVE N simultaneous live decodes.**
Invariant No. 5 reads "exactly one MediaWorker owns ONE source", not "one worker per application": `MediaWorker::spawn_with_stats` starts a thread named `media-worker-{id}` for EACH `MediaId` (`src/libs/kt-engine/src/worker.rs:111`), the worker map is `HashMap<MediaId, MediaWorker>` (`src/libs/kt-engine/src/player.rs:645`), and it is filled one per each `MediaRef` of the project (`src/apps/kt-app/src/app.rs:359`). A project with nine files right now holds nine open FFmpeg decoders in nine threads, and there is no upper limit — neither an LRU over sources nor a constant `MAX_OPEN_SOURCES` in the engine. So multicam does NOT challenge invariant No. 5. It challenges the cache and the priorities.

- **Priority queue and generation cancellation.** `DecodeRequest { kind, prio, generation }` (`src/libs/kt-engine/src/worker.rs:61`), `Priority` — `Playback | NearPrefetch | FarPrefetch | Thumbnail` (`src/libs/kt-engine/src/lib.rs:198`), selection of the next one `pick_next` (`worker.rs:347`), cancellation `cancel_generations_below` (`worker.rs:152`) from `Player::seek` (`player.rs:434`). **`Priority::Thumbnail` is never submitted in the working code** — across the whole workspace it occurs exactly once, in the iteration over priorities when a span is filled (`worker.rs:236`). A free priority level for the "grid cells" is already declared.
- **Cache.** `BurstKey { source, kind, idx }` (`src/libs/kt-cache/src/lib.rs:44`), **quality is part of the video key** (`BurstKind::Video { quality }`, same file :19), the limits of `CacheConfig::default()` = 64 MB audio + 512 MB video (`kt-cache/src/lib.rs:73`) are set in `Player::new` (`src/libs/kt-engine/src/player.rs:187`); LRU eviction only from an overfilled pool (`kt-cache/src/lib.rs:429`), the counter `unused_evictions` — "evicted before the first read", that is, prefetch thrash (`same file :189, :451`), `coverage()` (:460) — the only truth about warmth (invariant No. 4).
- **Preview resolution fraction 1/16…1/1.** `fracs = [1/16, 1/8, 1/4, 1/2, 1]` (`src/apps/kt-app/src/app.rs:3490`), labels and combo box (`app.rs:3548`), application of `set_preview_quality` (`app.rs:3560`), storage and getter (`player.rs:351`). The lookahead depth is computed FROM it and from the cache budget: `video_lookahead_ticks` (`player.rs:959`). A ready-made lever for "the grid runs coarser than the main preview".
- **Detached preview is a real second window.** `ctx.show_viewport_immediate` with `ViewportId::from_hash_of("kt-preview-window")` (`src/apps/kt-app/src/app.rs:952`), the flag `preview_detached` (`app.rs:95`), a double click on the preview (`PreviewClick::ToggleDetach`), Esc returns it. A ready-made place for the angle grid that does not break the layout of the main window.
- **A grid of thumbnails already exists and goes AROUND the engine.** The media bin (`src/apps/kt-app/src/bin_panel.rs`, cards 128×72, scrubbing on hover) is fed by `ThumbService` (`src/apps/kt-app/src/thumbs.rs`): its own thread, its own short-lived `FfmpegSource` instances, `MAX_OPEN_SOURCES = 4` (`thumbs.rs:111`), JPEGs are persisted in the sidecar `<project>.ktindex.db`. This is literally "an angle grid made of thumbnails" — only not a live one.
- **The closest relative of the angle switch is the morpher.** `MorphPatch.media: Option<MediaId>` substitutes the source for all clips covered by the block (`src/libs/kt-model/src/project.rs:677`), `apply_to` writes `c.media` and `c.source_in`, `src_off` is the switch for the substitution WHILE KEEPING the settings (`project.rs:687`), the targets `Below | Clips | Marks | Areas` (`project.rs:608`), it is applied in `Project::effective_clips()` — the morpher engine does not see it (`src/libs/kt-engine/src/video.rs:254`), there is `BakeMorpher` (`src/libs/kt-model/src/model.rs:228`). And, most important of all, the morpher layer SLICES a partially covered target (`SLICE_TAG`, `project.rs:1078`) — this is exactly the Vegas mechanism of "take selection splits the event at the call point".
- **A coordinated mutation of the A/V pair** already has a precedent: `sync_warp_to_link_group` (`src/libs/kt-model/src/model.rs:28`, test 248) — the rule lives in the model, not in the window, because the mutation arrives also from copy-paste, bake, the format reader and kt-cli.
- **Replacing the source on a live player**: `Player::replace_media` stops the worker → `cache.invalidate_source` → starts a new one (`player.rs:463`), test 211.
- **Synchronization** is a prerequisite, analyzed in F-SPD-004: the peak pyramid is already built and lies in memory (`src/libs/kt-media/src/index.rs`), but there is not a single line of correlation.

### What is missing

- **There are no multicams in the code at all**: a grep over `src/` gives zero. The word `angle` is already taken in our code by the angle of the WIPE (`TransitionKind::Wipe { dir, angle, softness }`, `src/libs/kt-model/src/project.rs`), which has nothing to do with a camera.
- **There is no nesting** (F-TL-004): `nested_example.otio` lies in the fixtures as FOREIGN material, nested `Stack`s are dropped into `ForeignEvent`, and `docs/porting-notes.md:335` says directly that there is nothing to compare against melt, "until nested compositions are supported". There is nothing to put under the "multicam clip as a nested sequence" path.
- **An event has exactly one source**: `ClipEvent.media: MediaId` (`src/libs/kt-model/src/project.rs:1512`), `MediaRef { id, path }` (same file :1496). There is neither a list of alternatives on the event nor a set of channels on the media.
- **There is no second source of decode requests.** The live composite of the preview is called exactly ONE time per UI frame — the single call site `self.player.compose_preview_into(...)` (`src/apps/kt-app/src/app.rs:3135`), and it warms frames only for the layers FROM THE PLAN (`video.layers_at(aligned)`, `player.rs:560`). Inactive angles do not get into the plan, so nobody will ever request or warm them: the grid requires an explicit separate consumer of frames.
- **There is no format for this**: `.otio` with the `metadata.kt` envelope (`src/libs/kt-otio/`) knows neither multicam clips, nor angles, nor takes.

### What gets in the way

1. **Not invariant No. 5 but invariant No. 4 — the cache budget, and quality in the key.**
   512 MB of video is a common pool for ALL sources. A 960×540 RGBA frame is 2.07 MB; nine angles at the 1/2 fraction eat the whole budget in 247 frames, that is, less than 1.1 s of lookahead at 25 fps, after which `unused_evictions` (`src/libs/kt-cache/src/lib.rs:451`) starts to grow — the very prefetch thrash that the performance panel can already show. At the 1/16 fraction (120×68, 33 KB) nine angles with a two-second lookahead cost 14.7 MB, that is, for free. Conclusion: the grid MUST run at its own coarse fraction — and since quality is part of the key (`BurstKind::Video { quality }`), the active angle will lie in the cache TWICE: once for the main preview, once for its own cell. This is not a defect, it is the price, and it has to be named in advance. A side effect of the same property is already visible today: changing the fraction zeroes the coverage bars in the timeline (`src/apps/kt-app/src/timeline.rs:1253`).
2. **Invariant No. 12 and UI rule 12: the grid has no right to heat up the system.**
   Vegas solved this by turning processing off and honestly wrote about it in the help. For us that solution is available more cheaply: a grid cell is ONE layer, without `frame_plan`, without FX and transitions. But then the grid shows not what will get into the edit, and invariant No. 7 says "composition code is ONE". This is resolved only by the grid being an ANGLE-CHOOSING tool, not a preview of the result, and it must be explicitly marked as raw material; otherwise we would silently repeat the Vegas trap. Separately: while paused, without motion, the cells must be taken from thumbnails (`src/apps/kt-app/src/thumbs.rs`), and live decode must be switched on only during playback — otherwise invariant No. 8 will be violated.
3. **The multicam clip structurally has nowhere to be placed, and there are exactly two paths.**
   (a) "The Vegas way", through takes: a set of alternative `MediaId`s on the EVENT, one active. It fits the existing model almost without violence: switching = mutation of a field + a cut of the event (the cut mechanics are already written for the morpher layer, `SLICE_TAG`), "unpacking" is not needed at all — the events are ordinary anyway, and the format gets one new field with the default "as before".
   (b) "The Final Cut way", through a nested sequence: an object in the bin, an angle editor as a second surface, effects in two places. Path (b) is an order of magnitude more expensive and hangs entirely on F-TL-004.
4. **Our A/V pair is two events in a link group, and this unexpectedly helps.**
   `ClipEvent.link_group` (`src/libs/kt-model/src/project.rs:1507`): the mode "switch only the video" is simply a mutation of the video event; nobody touches the audio. That is, the CORRECT default ("the sound does not follow the video", exactly what the author of the quote above trips over) comes out of our structure by itself, while the incorrect one would have to be written specially — as a coordinated mutation of the pair, for which a precedent already exists (`sync_warp_to_link_group`, `src/libs/kt-model/src/model.rs:28`). A rare case where our structure is better than the foreign one; it should not be lost.
5. **Live switching is a stream of undo steps.** UI rule 2 requires that switching works during playback (otherwise there is no feature at all), and a model edit on the fly is safe for us under invariant No. 3 (diff of chains by clip id, FX state survives the mutation). But every press of 1–9 is a command, and a pass through a 40-minute concert gives hundreds of history steps, after which it is impossible to roll back to "before the pass". What is needed is session coalescing (`apply_coalesced` is already used for window sessions, item 248 of `docs/porting-notes.md`) or an explicit "one live pass = one step".
6. **The queue priorities will have to be split.** Nine angles means nine workers, and if the cells start submitting `Priority::Playback`, the active angle loses its advantage and the main preview starts to starve (and starvation under our invariant No. 5 legitimately yields "the last frame" — that is, the grid will get stuck unnoticed). The free level is already declared and unused: `Priority::Thumbnail` (`src/libs/kt-engine/src/lib.rs:198`, the only mention in the code — `worker.rs:236`).
7. **Without F-SPD-004 the feature is dead.** Multicam begins with the angles already being synchronized; we have only the A/V magnet by start proximity (`kt_model::ops::magnetize_commands`, `src/libs/kt-model/src/ops.rs:591`), which itself requires that everything is almost aligned already, and also trims the partner.

### Which invariants it touches

- **No. 4 (the cache is the only truth about coverage)** — touched directly and more than anything else: the requirements multiply by N, quality in the key gives double storage of the active angle, `unused_evictions` becomes the main health metric.
- **No. 5 (exactly one MediaWorker owns the decoder)** — **not violated**; a worker is already created one per source, N live decoders is today's norm. This is the main correction to the task statement.
- **No. 3 (a model edit does not destroy FX state)** — used: a live pass mutates the model dozens of times a minute, and reverb tails must survive.
- **No. 7 (composition code is one)** — touched at the edge: a grid cell must not become a second display path. Only the explicit reading "the grid is a choice of material, not a preview of the result" saves it.
- **No. 8 (scroll/zoom does not generate decode requests)** — the grid, while paused, must live on thumbnails, not on decoding.
- **No. 12 (the UI heats the system only during motion)** — nine cells have no right to redraw more often than the picture moves; V-fps and V-drops of the performance panel become the acceptance criterion.
- **UI rule 2** — switching must work during playback; a "paused only" gate here kills the whole feature.
- **UI rule 9** — "the sound follows the video" is drawn by a modifier or a mode icon, not by a new checkbox.
- **UI rule 13** — the grid lives in the already existing detached preview window (`src/apps/kt-app/src/app.rs:952`), not in a new full-width panel.
- **UI rule 17** — the digits 1–9, a click on a cell and a menu item must do exactly the same thing, including the case "several events are selected".

## Estimate

| What | Answer |
|---|---|
| Size | XL — a whole round plus a new format field; the nesting path (b) is two rounds, and one of them belongs to someone else (F-TL-004) |
| Where the main work is | kt-model (a set of alternatives on the event, the switch command, pass coalescing), kt-app (the grid in the detached preview window, keys 1–9, the sound mode), kt-engine (a second consumer of frames + a priority level for the cells), kt-cache (the grid budget separate from the preview), kt-otio (a field in the format) |
| Project format | a new version is needed: a list of alternative `MediaId`s on `ClipEvent`; the default "one alternative" = the previous behavior byte for byte, old files are read as they are |
| Tests that will have to be written | model: switching = mutation of the active one + a cut of the event at the point, a live pass collapses into one undo step, an unpacking similar to `BakeMorpher`; kt-cache: the cells do not evict the main preview (`unused_evictions` is bounded), the active angle at two qualities; kt-engine: the cells' requests at a priority below `Playback`; kt-app layout: the grid in the detached window, 1–9 during playback, "video only" by default, N cells do not drop V-fps; kt-otio: round-trip of the new field + reading files without it |
| Depends on | F-SPD-004 (hard: without synchronization the feature is pointless), F-TL-004 (only for the "nesting" path; the "takes" path does NOT depend on it) |
| Breaks compatibility | no on the "takes" path (a new field with a default); yes on the "nesting" path |

## Verdict

Not taking it now. Priority C is honest: this is a feature for concerts, interviews and streams, not for everyone, and it is completely useless without synchronization (F-SPD-004), which does not exist yet.

The main thing worth finding out: **the premise "multicam breaks invariant No. 5" is wrong** — N live decoders already work for us, a worker is created for EVERY source (`src/libs/kt-engine/src/worker.rs:111`). The feature runs into not the decoders but the cache budget (invariant No. 4, 512 MB for everything) and the silence of the interface (invariant No. 12); both are solved by a coarse resolution fraction for the cells and the free, unused `Priority::Thumbnail` level.

When we take it, we take the Vegas path (a set of alternatives on the event), not the Final Cut one (a nested sequence): it fits what is already written (cutting the target by the morpher layer, `src_off`, the link group), does not require F-TL-004, and gives "unpacking" for free, because there is nothing to unpack. And we keep our accidental advantage: by default, our sound does NOT follow the video, because video and audio are two events, not one.

## Sources

- Apple. Final Cut Pro User Guide — Intro to multicam editing;
  Cut and switch camera angles; Sync and adjust camera angles; Edit multicam
  clips; Multicam editing tips.
  https://support.apple.com/guide/final-cut-pro/intro-to-multicam-editing-ver23c76439/mac ·
  https://support.apple.com/guide/final-cut-pro/cut-and-switch-camera-angles-ver23c76c9c/mac ·
  https://support.apple.com/guide/final-cut-pro/sync-and-adjust-camera-angles-ver23c76b1a/mac ·
  https://support.apple.com/guide/final-cut-pro/edit-multicam-clips-ver23c76d65/mac ·
  https://support.apple.com/guide/final-cut-pro/multicam-editing-tips-verb1160b2d/mac
  (accessed 2026-09-07)
- MAGIX/Boris FX. VEGAS Pro Help — Editing multicamera video; Editing
  multicamera shoots (version 22; pages for 2026 are not published on help.magix-hub.com, checked — 404).
  https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/editing_multicamera_video.htm ·
  https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/editing_multicamera_shoots.htm
  (accessed 2026-09-07)
- Blackmagic Design. DaVinci Resolve — Edit page; Cut page (Sync Bin);
  What's new (Resolve 21).
  https://www.blackmagicdesign.com/products/davinciresolve/edit ·
  https://www.blackmagicdesign.com/products/davinciresolve/cut ·
  https://www.blackmagicdesign.com/products/davinciresolve/whatsnew
  (accessed 2026-09-07)
- Adobe Support Community. "Multicam and some micro problem sound",
  05.01.2025 — the quote from the section "What users say".
  https://community.adobe.com/t5/premiere-pro-discussions/multicam-and-some-micro-problem-sound/m-p/15074101
  (accessed 2026-09-07)
- Creative COW. "Audio follows video in multicam" (Herb Sevush, 10.03.2014) —
  a thread without replies, valuable for the wording of Adobe's question itself.
  https://creativecow.net/forums/thread/audio-follows-video-in-multicam-aeaeaeae/
  (accessed 2026-09-07)
- davinciresolveclub.com. "DaVinci Resolve Multicam Editing: Sync to
  SmartSwitch" — **a secondary source**, the only available detailed analysis of SmartSwitch; taken from it are the description of the analysis (active speaker,
  sound, lip movement, close-up/wide shot plan) and the caveat "The result is a starting
  timeline, not a finished edit".
  https://davinciresolveclub.com/davinci-resolve-multicam-editing/
  (accessed 2026-09-07)

**What could not be confirmed.** The Premiere pages on `helpx.adobe.com`
(including the new path `helpx.adobe.com/premiere/desktop/edit-projects/...`) do not open from this environment — the Premiere line is assembled from the Adobe Support
Community, Creative COW and the titles of help pages found by search; I do not quote Adobe's formulas verbatim. A primary source for "AI Multicam SmartSwitch" was not
found: blackmagicdesign.com currently documents Resolve 21, where this word does not appear, and the Resolve 20 announcement is unavailable (web.archive.org is blocked) — the item is marked `[NOT VERIFIED]`. The forum.blackmagicdesign.com threads on multicam performance return 403 on direct load, the mobile mirror fails the TLS certificate, reddit is unavailable entirely; the `WebSearch` budget in the session is exhausted, and the search went through direct queries to Yahoo.
