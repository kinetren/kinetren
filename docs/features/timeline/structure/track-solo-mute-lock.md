---
id: F-TL-006
title: Track switches — mute, solo, lock and "hide"
status: in-progress
priority: B
cost: L
category: timeline/structure
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-otio, kt-engine, kt-app]
invariants: [3, 6, 7, 11]
ui_rules: [5, 9, 13, 16, 20]
round: R.62
point: null
tests: [t729, t730, t731, t732, t738, t740, t740b, t741, t742, t743, t745, t746]
depends_on: [F-TL-001]
trackers: [pitivi, kde-bugzilla, kde-invent, olive, shotcut]
checked: 2026-09-08
---

# F-TL-006 — Track switches — mute, solo, lock and "hide"

## Essence

Small switches live in the header of every track, and the editor presses them dozens of times a shift: **mute** — this track does not sound; **solo** — only it sounds; **lock** — everything on the track is visible and audible, but it cannot be touched; for video, **hide** is added to these — the track does not take part in the picture. These are not four variants of one "turn off": mute and solo are about what the editor HEARS now, hiding is about what will get into the file, and the lock is not about sound and picture at all, it is about which edits are allowed.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Mute, Solo (in the header of video and audio tracks) | solo is not "mute of the others" but a GROUP: a click adds the track to the solo group, Ctrl+click leaves only one in it. The track has NO lock: the header's control list does not have it at all, events are locked instead (right-click → Switches → Lock), and "Lock Envelopes to Events" is about envelopes, not about editing |
| Adobe Premiere | 26.x | Toggle Track Output / Mute Track (M) / Solo Track (S) / Toggle Track Lock / Toggle Sync Lock | TWO locks on one track. Track Lock forbids editing. Sync Lock is a separate switch deciding whether the content of this track moves when ripple/insert shifts the neighboring track |
| DaVinci Resolve | 21 | Enable/Disable track, M, S, Lock, Auto Track Selector, Sync Lock | the same split: Auto Track Selector says "whether the track takes part in editing", the lock says "do not touch", and since 20.2 a separate Sync Lock has appeared in the header next to them |
| Final Cut Pro | 12.3 | there are no tracks: Solo (Opt+S), Disable (V) on the CLIP; roles and audio lanes | the boundary is drawn by meaning, not by the line: Solo/Mute are only for listening, and Disable means "invisible and silent and does not get into any output". To mute a "track" means to mute a role |
| After Effects | 26.x | layer switches: Video (eye), Audio, Solo, Lock, Shy | solo is a layer switch, but the RENDER SETTINGS have a separate override "Solo Switches: All Off": the program knows in advance that solo will be forgotten to be unset, and does not let the viewing state leak into the output |

## The trick behind it

**1. A lock does not hold sync — it breaks it, and that is why everyone got a second lock.** The editor locks a track with a watermark and music exactly so that it is not touched. But ripple/insert shifts the neighbors, and the locked one does not move, and after ten edits the music has drifted off the picture by a second. The industry solved this NOT by fixing the lock, but by a second, separate switch: Premiere Sync Lock, Resolve Auto Track Selector (and since 20.2 — also Sync Lock). The lock answers "may THIS track be edited", the sync switch answers "does it move when the NEIGHBOR is edited", and these are two different answers. In the Kdenlive tracker both halves of the error are visible at once: first a bug is caught that the spacer does move the content of locked tracks (#355936), then users ask for the opposite — for the spacer to IGNORE locked ones (kde-invent #1436, #1158). One and the same button turned out twice to be not what was wanted from it.

**2. Solo is a state, not "mute of all the others".** If solo is implemented as a record `muted = true` on the other tracks, it can no longer be removed with one movement: the original mutes are overwritten, and after "solo is released" what was sounding falls silent. In VEGAS solo is membership in a GROUP (a click adds to the group, Ctrl+click leaves one), in Kdenlive this did not exist, and there is a request exactly about it: the solo button worked on one track at a time, and shift-click for several was asked for (#421225, closed FIXED). That is, "several solos" is not an exotic case but the norm that every editor grows toward.

**3. Mute/solo do not belong to the project file at all, while hiding does.** FCP puts it in words: solo and mute are listening, and Disable means "does not get into any output". After Effects puts it in a setting: the render settings have a separate "Solo Switches: All Off". Whoever did not draw this boundary pays literally: the Kdenlive tracker has a request to warn the user that the master is muted, "otherwise there will be no sound on export" (#498560, ASSIGNED since 01.2025).

## Pros

- Mute/solo, left as a VIEWING state, cost the project format nothing, do not get into undo and cannot spoil the output: they cannot be forgotten to be unset in a way that the client notices, rather than the editor.
- Hiding a video track is the only way to compare two variants of one piece lying one above the other without moving a single event: the alternative is dragging the upper layer aside and back, and that is an edit which gets into undo and breaks the joints.
- The lock makes multi-track editing gestural: one can cut and drag without aiming or pre-selecting, because a locked track does not respond to a click. This is exactly what is written about in pitivi #2421: without a lock one has to select the needed track before every cut.
- Solo as a group gives what it is pressed for: to assemble 2–3 tracks out of twelve and listen only to them, and then restore everything with one movement.

## Cons

- Three switches per row are three new ways to get "my sound has disappeared" and not one new way to notice it. The track's mute and the event's level add up silently: quiet can come from two causes at once, and neither is visible where the editor is looking.
- The lock and hiding must survive saving — which means they are in the file, which means a foreign build that does not know about them will render a hidden track as visible and silently deliver the wrong film.
- A hidden track does not stop costing processor time by itself: in Kdenlive this is a separate request ("Muted Video Tracks Should Not Consume CPU", #497534, open since 12.2024). A switch that turns off only the picture but not the work is half a feature.
- Hiding changes the PICTURE, and therefore interacts with everything that is computed "by the layers below": with us this is the morpher with `MorphTargets::Below`, in Vegas — the compositing children. It is easy to err here, and hard to see the error: the result is plausible, just wrong.

## What users say

SIGNAL: track switches — 6 tasks in pitivi, `\b(mute|unmute|muting|solo|unsolo|lock|unlock|hide|unhide|hidden|invisible)\w*\b\s+(the\s+|a\s+|all\s+|multiple\s+|some\s+|whole\s+|video\s+|audio\s+|subtitle\s+)*(track|layer)s?\b|\b(track|layer)s?\b\s*(is|are|be|seems? to)?\s*(mute|muted|solo|soloed|lock|locked|hidden|invisible)\w*\b`; share 0.2% against 0.2% in kde-bugzilla

The share matched to the tenth across five trackers in a row — pitivi 6/2672, kde-bugzilla 11/6346, kde-invent 4/2217, olive 3/1853, shotcut 3/1368 (a sixth, flowblade 1/788, gives 0.1%) — but the CONTENT of it is opposite. Where there are no switches, these are requests: at pitivi 4 of 6 are "add" (mute a layer, lock a layer, hide a layer, make a layer invisible), at olive 3 of 3 — the same. Where the switches have existed for ten years, these are complaints about how the built thing breaks: at kde-bugzilla 9 of 11 — the spacer moves locked tracks, a crash when inserting onto a locked track, the render mixes hidden tracks in as black, subtitles are burned in despite a hidden track. The same share of attention goes either to "give it" or to "fix it" — and the second half shows what the feature costs.

SIGNAL: lock against the shift tool — 2 tasks in kde-invent, `(spacer|remove space|ripple|insert edit|sync ?lock)[^.]{0,30}lock|lock\w*[^.]{0,30}(spacer|remove space|ripple|sync)`; share 0.1% against 0.0% (1 task of 6346) in kde-bugzilla

> Usecase : On the 1st layer you have the sound recorded live with the picture
> whereas on the 2nd you've added a sountrack.
> — pitivi #1648, 01.11.2015 (25 comments, 2 votes — the most weighty task of the sample),
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1648

> a lot of time when i have a watermark or a soundtrack i'm not able to use
> this tool to "create" some space for missing clips.
> — kde-invent #1436, 18.05.2022, about the locked track blocking the
> spacer, https://invent.kde.org/multimedia/kdenlive/-/work_items/1436

> So when I want to clip only the camera angle track, I have to select this
> before I make the cut if I don't want to cut any other clips at that same
> position.
> — pitivi #2421, 10.02.2020 (open to this day),
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2421

The negative half — the same theme through the eyes of those who have the feature:

> Timeline/Render includes hidden tracks as black/empty space after moving
> shown clips around
> — kde-bugzilla #476434, 01.11.2023, UNCONFIRMED,
> https://bugs.kde.org/show_bug.cgi?id=476434

> Performance: Muted Video Tracks Should Not Consume CPU
> — kde-bugzilla #497534, 16.12.2024, wishlist,
> https://bugs.kde.org/show_bug.cgi?id=497534

## How it fits Kinotrain

ADDRESS: src/libs/kt-model/src/project.rs — the class "the track switch broke" is impossible for us, because a TRACK AS AN ENTITY DOES NOT EXIST: there are the counters `video_lanes`/`audio_lanes`/`morph_lanes`/`camera_lanes`, the order vector of the rows `lane_order` and three parallel vectors of "properties" (`lane_names`, `lane_heights` in `UiState`, `bin_groups` — about the bin); there is simply nowhere to hang a flag. But a neighboring class is possible, and nothing catches it: `src/libs/kt-otio/src/convert.rs:366` writes `enabled: true` hard-coded, so a foreign OTIO track with `"enabled": false` silently turns on after our opening and re-saving, and a disabled foreign CLIP (`convert.rs:849`) turns into a gray `ForeignEvent` placeholder, and there is nothing to return it to the edit with.

### What already exists

- **The master mute of the preview — and it already stands where it should.** `MeterState.muted` in `src/apps/kt-app/src/meters.rs:32` silences the master fader, the value goes to `Player::set_master_gain` (`src/libs/kt-engine/src/player.rs:375`) and is applied in the RENDER THREAD (`player.rs:930`), already AFTER `AudioRenderer::render_frames`. The export (`src/libs/kt-engine/src/export.rs:103`) calls the same `AudioRenderer` directly and does not see the master gain at all. That is, the boundary "listening versus output" from the third trick is already drawn by us — by accident, by the device — and the class of defects of Kdenlive #498560 does not exist for us.
- **The event level** — `ClipEvent.gain_db`, becomes a multiplier in `src/libs/kt-engine/src/audio.rs:165` and is applied per sample together with the fade and overlap weights (`audio.rs:351`). This is a second, independent multiplier — the very one that in "Cons" adds up with the track mute.
- **The master switch of the chain** (`fx_off`) and the slot checkboxes (`fx_bypass`) are switches of EFFECTS, not of sound; they already exist and are already unified (item 242, R.35). This matters for rule 9 of the checklist: a fourth switch on the row must differ in meaning from them, otherwise it is forbidden.
- **The lock of a bin group** — `BinGroup.locked` (`project.rs:1586`, `src/apps/kt-app/src/bin_panel.rs:113`). This is the ONLY live lock in the program, and it concerns material cards, not editing. The verb and the icon for a track will have to be taken from there (rule 17 — paired ways behave the same).
- **The "gesture ran into an obstacle" mechanism** already exists and already shows an error rather than staying silent (t172, rule 16): a locked track fits into it ready-made.

### What is missing

- An entity of the track. The flags have nowhere to be put, and putting them as a fourth parallel vector `Vec<(LaneKind, u32, bool)>` next to the three existing ones is a direct violation of rule 20 (nothing grows by itself) and a guaranteed class "the name is there but the flag is not" after insertion/deletion/reordering of a row: today the synchronicity is held by the three separate functions `lane_names_insert`/`_remove`/`_swap`/`_rotate` and `lane_order_insert`/`_remove`, and each new vector adds one more instance of the same arithmetic to them.
- A second, sync switch (trick 1) — but it is not needed yet while there is no ripple: there is nothing for us to shift the neighbors with, `src/libs/kt-model/src/ops.rs:374` says directly "Without ripple". It becomes mandatory at exactly the moment **F-TL-001** is made, and a "lock" without it will be the same wrong answer as everyone else's.
- Room in the interface. The gutter strip — `GUTTER_W = 34.0` (`src/apps/kt-app/src/timeline.rs:22`), and the track label already lives in it (`timeline.rs:1611`, a label over the start of the track — the reference of rule 13). Three buttons will not fit there; creating a full-height strip for them is forbidden by the same rule 13.

### What gets in the way

- **`effective_clips()` — the common entry for audio, video and preflight.** `src/libs/kt-engine/src/video.rs:214`, `audio.rs:138`, `export.rs:302`. This means "disable a track" CANNOT be implemented by dropping its clips there: hiding a video track would at the same time silence the sound, and muting an audio track would blank the picture. Each switch must be applied in its own renderer, and these are two edits, not one.
- **Mute cannot be done by removing a clip from the graph at all** — this is invariant 3: the chain disappears from `AudioRenderer` → `sync_fx`/`retain_clips` (`audio.rs:224`) will wipe its FX state, and lifting the mute will start from a cold reverb. Mute must be a multiplier of 0 next to `clip.gain` (`audio.rs:351`), not a deletion.
- **The composition and the order of rows are DIFFERENT orders.** `VideoRenderer` sorts by `by_key(|c| (Reverse(c.lane), c.start))` (`video.rs:291`) — by the NUMBER of the track, while the morpher `MorphTargets::Below` computes "what is below" by the vector of rows `lane_order` (`src/libs/kt-model/src/project.rs:1078` — `covers()` compares the POSITIONS in `rows`). Hiding must choose one of the two and live there: if it removes the row from `rows`, the set of morpher targets silently changes — that is, "hiding a track" would recolor the neighbors; if it filters by `c.lane` in `frame_plan` (`video.rs:406`), the morpher will go on computing a batch for clips that are not in the frame. The right answer (the flag lives in the composition, and the row in `lane_order` is not touched) is not obvious — and this is the main substantive trap of the feature.
- **The GPU preflight counts by `effective_clips`** (`export.rs:298`, `gpu_required_clips`): a hidden video track with a 3D effect will go on requiring the GPU and will drop the export with `ExportError::GpuRequired` over a frame that nobody will see.
- **The format.** Hiding changes the picture → an old build will silently render the wrong thing, and this is exactly the criterion by which v2…v5 were raised (`src/libs/kt-otio/src/meta.rs:17`): **v6** is needed. The lock does not change the picture — it does not require a version. At the same time one will have to stop lying in `Track.enabled` (`convert.rs:366`): in OTIO this is a STANDARD field, and a hidden track must travel in it, not only in the `kt` envelope.

### Which invariants it touches

- **3** (a model edit does not kill FX state) — mute as a multiplier, not as removal from the graph; see above.
- **6** (in the cpal callback there are no allocations and no locks) — the set of solo/mute is read by the audio path; if it lives in the UI, it must travel as an atomic snapshot, like `master_gain` (`player.rs:137`), not as a mutex.
- **7** (composition code is one) — the filter of hidden tracks must stand inside `frame_plan`, otherwise the preview and the export will diverge at the first hiding.
- **11** (determinism) — mute/solo left outside the project do not touch the GOLDEN references at all; hiding that gets into the file requires new golden cases.
- Checklist rules: **5** (lifting solo must restore the previous mutes, not extinguish them "for company"), **9** (three buttons per row — as icons, and they must differ in meaning from the chain checkboxes already present), **13** (the 34 px gutter is already occupied by the label), **16** (a gesture on a locked track is an error, not silence), **20** (the track flags must not grow as a fourth parallel vector).

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round, but it splits into two very different halves: mute/solo as the VIEWING state (S, does not touch the model or the format at all — by the pattern of `master_gain`) and lock/hide as project content (L: the track entity + format + undo) |
| Where the main work is | kt-model (the track as an entity and absorption of `lane_names`/`lane_heights`), kt-otio (v6 + honest `Track.enabled`), kt-engine (`video.rs::frame_plan` and the multiplier in `audio.rs`), kt-app (three icons in 34 px and the gesture ban) |
| Project format | v6 is needed — but only for the sake of hiding; mute/solo do not touch the file, the lock does not change the picture and does not require a bump |
| Tests that will have to be written | round-trip of the flags and their synchronicity on insertion/deletion/reordering of rows (kin of t49/t90); "mute does not kill the FX tail" (kin of t31, invariant 3); "hidden track: preview == export" (kin of t11/t67, invariant 7); "solo released — the previous mutes returned" (rule 5); "a gesture on a locked track is an error" (kin of t172); reading foreign OTIO with `Track.enabled: false` and re-saving it without loss (today it would fail; the hole is named in ADDRESS) |
| Depends on | F-TL-001 (ripple): the sync half of the lock is pointless before it, and mandatory after it |
| Breaks compatibility | no: old files are read; new ones with a hidden track will be honestly rejected by the old build (`KtMetaTooNew`), like v2…v5 |

## Verdict

We take it, but we cut it along the boundary that the reconnaissance found, not along the list of buttons.
**Mute and solo — a state of viewing**: they live next to the master fader (`meters.rs` + `master_gain`), are not written into the file, do not get into undo, and do not touch the export. This is cheap, it closes the loudest request of the trackers (pitivi #1648 — the most weighty task of the sample), and it is the only variant in which "forgot to unset solo" cannot spoil a delivered work.

**Hiding and lock — project content**, and they wait: hiding runs into the choice between `lane` and `lane_order` (the morpher `Below`) and into the format version v6, while the lock without ripple is half an answer, because its second half, sync-lock, appears only together with **F-TL-001**. To make the lock before ripple is to repeat someone else's error with open eyes: at Kdenlive it cost a bug "the spacer moves locked tracks" and two requests to do the opposite.

Separately and out of turn — a hole found along the way and covered by no one: `convert.rs:366` writes `enabled: true` hard-coded, and a foreign disabled track turns on silently after our re-saving. This is not a feature, it is a defect of the passthrough passport (kin of t145/t146), and it should be closed with a test regardless of the fate of this note.

## Sources

- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/videotrack.htm
  — the composition of the header controls of the VEGAS video track (no lock in the list), the wording of Mute and Solo (accessed 2026-09-08)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/9-audio/audiotrack.htm
  — the solo group: a click adds the track, Ctrl+click leaves one (accessed 2026-09-08)
- https://helpx.adobe.com/premiere/desktop/edit-projects/change-clip-sequence/sync-lock-to-prevent-changes.html
  — Sync Lock as a switch separate from Track Lock (accessed 2026-09-08)
- https://support.apple.com/guide/final-cut-pro/solo-or-disable-clips-ver4e2fbd2/mac
  — Solo/Disable of a clip, "invisible and silent … do not appear in any output" (accessed 2026-09-08)
- https://helpx.adobe.com/after-effects/using/basics-rendering-exporting.html
  — the render setting "Solo Switches: Current Settings / All Off" (accessed 2026-09-08)
- https://dvresolve.com/tutorial/editing-auto-track-selector/ — Auto Track Selector and the appearance of Sync Lock in the track header of Resolve 20.2+ (accessed 2026-09-08)
- Trackers: pitivi #1648, #2421, #2506, #2557; kde-bugzilla #355936, #368044,
  #421225, #440181, #476434, #497534, #498560; kde-invent #421, #1158, #1436;
  olive #420, #775; shotcut #837, #849, #1811. All are permanent links; the dates are
  given with the quotes.
- **Caveat on the completeness of the sample.** At the time of writing, the cache holds nine dumps out of the twelve in the registry: `flowblade`, `kde-bugzilla`, `kde-invent`,
  `losslesscut`, `mlt`, `olive`, `otio`, `pitivi`, `shotcut` (18445 records,
  `tracker-rank.py --check`). `openshot`, `blender-vse` and `ffmpeg-trac` were still downloading; the numbers of the SIGNALs do not depend on them (both lines were read on
  kde-* and pitivi), but the share of "universal" on the topic may change when they appear, and it will have to be recalculated.
- No live check (Vegas/Premiere/Resolve/FCP/AE) was done: the device is taken from the vendors' documentation and from the trackers, as `_editors.md` stipulates.
