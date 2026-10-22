---
id: F-TL-003
title: JKL navigation and dynamic trim
status: candidate
priority: B
cost: M
category: timeline/trimming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app, kt-engine, kt-model]
invariants: [3, 5, 6, 12]
ui_rules: [2, 4, 9, 13, 17]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-TL-003 — JKL navigation and dynamic trim

## Essence

Three keys under the right hand: **J** — back, **K** — stop, **L** — forward; a repeated press doubles the speed, `K+J`/`K+L` give slow motion, holding `K` with a tap on `J`/`L` — frame-by-frame. The left hand meanwhile rests on the editing keys. In trim mode the same three keys move not the cursor but the SELECTED JOINT — right during playback, with the sound of all tracks. The editor watches a looped transition and presses the key when "it is time": the decision is made by rhythm, not by counting frames.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 21 (checked against the manual of 12/18.6) | Dynamic Trim Mode (`W`) + Quick Trim (`Cmd-J`/`Cmd-L`) | the benchmark among the core. `W` enters a mode where "all you can do is trim": JKL adjust the selected joint, the cursor is yellow, the word Dynamic is in the toolbar. All four operations work — resize/ripple/roll and slip/slide (switched by `S`). Slow and frame-by-frame trim are available ONLY in the mode; outside it there is a reduced Quick Trim (`Cmd-J`/`Cmd-L`) — "only trim forward and backward at 100 percent speed or greater". `/` — play around, `Cmd-/` — loop, pre/post-roll in the Edit panel of the project settings |
| Adobe Premiere | 26.x | Trim mode (`T`), "dynamic trimming" | the joint is selected, `T` opens the trim mode with a two-window view; JKL shuttle to the new points for ripple and roll. A separate command Play Around (`Shift+K`) plays around the cursor, the durations are taken from Preferences → Playback (Preroll/Postroll) — **[NOT VERIFIED against the primary source]**: `helpx.adobe.com` does not open from this environment |
| VEGAS Pro | 2026.0 | "Scrub playback (J, K, L)" | JKL exist, but this is officially SCRUBBING, not a shuttle with steps: the speed and the RANGE are taken from the common setting `JKL / shuttle speed` (the Editing tab in Preferences), and `K+J`/`K+L` are described as an emulation of the shuttle ring. There is no trim mode with editing the joint on the fly at all; loop — `Q` or `Ctrl+Shift+L` |
| Final Cut Pro | 12.3 | J/K/L (transport only) | JKL are full-fledged: a repeated press doubles the speed up to 32×, `K` + holding `J`/`L` — 1/2×, `K` + tap — a frame. But there is no editing of the joint on them: trimming is the Trim tool and the Precision Editor, where "you can skim over these areas to view and play back the media as you decide where to trim" (skimming with the mouse, not playback under the keys) |
| After Effects | 26.x | none (J/K are taken by something else) | there are no transport JKL at all: `J` — the previous keyframe, `K` — the next. AE also has no editing model with joints — there is nothing for dynamic trim to apply to |
| _(outside the core)_ Avid Media Composer | — | Trim Mode + Play Loop | the historical author of the whole scheme: trim mode with looped playback around a joint and the pre/post-roll setting on the Trim tab. **[NOT VERIFIED against official documentation]** — Avid is not in the first wave (`_editors.md`), the official PDF (24 MB) did not open in this environment, the details are taken from a third-party tutorial analysis |

## The trick behind it

**1. It is not "more precise", it is a DIFFERENT way to make a decision — and the vendor says so in plain text.** The Resolve manual motivates the mode not by precision but by the fact that the edit goes under live viewing: trimming during playback "has the advantage of letting you get emotionally involved in what you're watching, as well as experiencing the timing of a clip as it plays". Technically this rests on one detail that is in none of the feature lists: **during dynamic trim the WHOLE mix sounds**, not the clip being trimmed — "all audio clips in all audio tracks will play back as they cross the playhead, so you can hear your entire mix as you're trimming". The joint is fitted to the music and the line of the neighboring track, not to its own waveform.

**2. Pre-roll/post-roll is not cosmetics but a parameter of the DECISION.** The method works only by repetition: "play loop around edit" runs the same transition in a circle, and the editor listens to it for the fifth time until the hand presses in time. Therefore, for everyone who has the mode, the length of the run-up is moved into settings: in Resolve — the Edit panel of the project settings (`Cmd-/` turns on looping, `/` plays around), in Premiere — Preferences → Playback (Preroll/Postroll, command `Shift+K`). The difference between a post-roll of 1 s and 3 s changes the VERDICT about the joint, not the convenience: a short tail forgives a dragged-out entry, a long one does not.

**3. A separate mode exists for the sake of unambiguous keys, and it is also the main price.** In Dynamic mode JKL STOP being transport, so the manual is forced to end the instruction with a warning: "You always want to be sure to turn Dynamic mode off when you're done, because otherwise using JKL will continue trimming selections, instead of playing the Timeline". From the same source comes the two-levelness that marketing does not show: in order not to enter the mode, Resolve gave a SECOND command with other keys (`Cmd-J`/`Cmd-L`) and reduced capabilities — there is no slow or frame-by-frame trim in it. One and the same trim with two pairs of keys with different sets of speeds is exactly what our checklist calls a violation of rule 17.

## Pros

- The joint is adjusted to the sound of the WHOLE mix at 1× speed — the frame is chosen by ear against the neighboring track; when fitting with the mouse only one's own waveform is heard.
- Searching for a joint and editing the joint sit on the SAME keys: the hand does not leave the keyboard between "scrolled to" and "moved", and `K` works as a modifier — three tools (fast / slow / by frame) from three keys, without separate binds for the frame step.
- The mode overrides ONLY JKL, so mouse and keyboard trim remain one operation: "you see the same two-up or four-up display, the same Timeline overlays… The only difference is that you're trimming while your program plays".
- Pre/post-roll is one setting for all "play around" commands, so the mode is not multiplied for each command: looping around a joint, preview after an edit and trim on the fly use the same run-up.

## Cons

- Modality: the keys silently change their meaning. The official manual itself warns that the mode must be switched off — forget it, and JKL cut the edit instead of playing, and each "run" is an edit in the history.
- The decision by feel is NOT reproducible: two passes give two different frames. Looping grows out of this — without it the method does not work, and with it each circle leaves an undo step.
- Trimming on the fly requires the engine to give the position with frame accuracy DURING playback. Any buffering of the output turns "pressed in time" into "pressed N frames later", and the error is systematic, in one direction — that is, invisible: the edit simply drifts.
- Fast rewind changes what has to be decoded every few frames: `J`/`L` at 8× is a change of direction and of the read step without a seek.
- Bypassing the modality costs a second command with another layout and another set of speeds (Quick Trim against Dynamic mode) — "paired ways" diverge in capabilities at the vendor itself.

## What users say

> after using Avid's trim tool for years, its really frustrating to have trim
> any other way and once you get used to "rock & roll" style trimming, you
> notice jsut how organic your cuts become.
> — Neil Goodman, Creative COW, 27.02.2013, https://creativecow.net/forums/thread/dynamic-trimming-2/

In the same thread he is answered with exactly the objection worth holding in mind before one calls the feature magical:

> the idea that it somehow makes your edits better is bull plop
> — Steve Connor, Creative COW, 27.02.2013, https://creativecow.net/forums/thread/dynamic-trimming-2/

(Connor, however, agrees that the process becomes faster and easier; the dispute is about whether the RESULT is better, not about whether the work is more convenient.) A half-hearted implementation causes a separate irritation — on dynamic trimming in FCPX:

> i actually thought the video did more to show the shortcomings of Dynamic
> trimming in FCPX than anything else.
> — Neil Goodman, Creative COW, 28.01.2015, https://creativecow.net/forums/thread/dynamic-trimming-in-x/

A characteristic detail about Vegas: the slow steps are there, but they live in folklore, not in the interface — they are suggested on the forum as a trick:

> If you use the JKL shuttle keys, try holding K while pressing L or J, and
> you'll get playback rates of 0.25 and 0.5 instead of only 1 and greater.
> — Matt Crowley, Creative COW, 16.04.2012, https://creativecow.net/forums/thread/trimmer-change-playback-speedae/

## How it fits Kinotrain

### What already exists

Our transport is rich — what is missing is precisely the LINK "playback + editing of the joint".

- **Signed speed with a ceiling of ±8×.** `Player::set_playback_rate` (`src/libs/kt-engine/src/player.rs`), `ENGINE_RATE_MAX = 8.0`; `rate < 0` — reverse transport, `0` — a stop of consumption (silence, the playhead stands still). A change of SIGN is handled in the audio-render thread as a mini-seek without a generation bump: a reset of the FX state, the ring and `render_head`.
- **Speed widget with two bands** 170×20 (`KtApp::draw_speed_control`, `src/apps/kt-app/src/app.rs`, items 18-19 of R.7): the upper band — constant speed, the lower — a shuttle with auto-return on release, double-click = 1×. The mapping `slider_to_rate` is a logarithm 0.25…2.0 with a dead zone of 0.07 (test 46); the cursor leaving the edge of the widget — a "second wind" (`overshoot_rate`, 60 px per doubling) up to ±8×.
- **Acceleration by holding the space bar** up to 4× over 5 s (`SPACE_BOOST_MAX`, `SPACE_BOOST_SECS`, the analytic exponent `space_boost_step`; test 120, item 3 of R.16 and item 3 of R.17 of `docs/user-feedback.md` — the user asked for "like on YouTube" and separately ordered the acceleration stretched from 2 s to 5 s).
- **Navigation over stop points.** `stop_points` (`src/apps/kt-app/src/app.rs`): zero, the region boundaries, point and range markers, the boundaries of JOINED clips, the end of the project; `A`/`D` and `Home`/`End`, `Shift+A`/`Shift+D` — the edges of the region (test 119, a direct request of the user), `Ctrl+Home`/`End` — the edge of the project. After a jump — the view "scrolls" to the cursor (`pending_reveal`, test 88).
- **Loop with a region and punch-out**: `Transport::advance_frames_wrapped` wraps in both directions (test 43), `Player::set_loop_region` pins the head of the region in the cache so that the circle does not start cold (test 14), the FX seams at the wrap are closed by t166–t168.
- **Scrub by the ruler** (`Drag::Scrub`, `src/apps/kt-app/src/timeline.rs`); while scrubbing the meters show the level under the cursor (t121).
- **The whole joint-editing is already there**: trim of the edges, the roll of the joint (the pair `TrimEnd` + `TrimStart` from one mouse drag over the joint, `timeline.rs`), and, most importantly, **trim-to-cursor** of the whole link group: `kt_model::ops::trim_group_start_to` / `trim_group_end_to` (`src/libs/kt-model/src/ops.rs`) plus `KtApp::trim_selected_start_to_cursor` and the toolbar buttons. This is exactly the primitive that dynamic trim requires: "pressed the key — the joint moved to the cursor".
- **Playback SURVIVES an edit of geometry** — checked by the code, not by the invariant. `Player::set_project` only substitutes the project and sets `model_dirty`; the audio-render thread on the next turn rebuilds the `AudioRenderer` and calls `sync_fx` ("state survives the rebuild, inv. No. 3"), the transport and the generation are not touched, the ring is not reset, pause is not set. A live precedent of an edit on EVERY frame of a gesture already exists: `Drag::Level` in `src/apps/kt-app/src/timeline.rs` pours the modified `Project` into the player without a record in the history. Invariant No. 3 covers the FX state; the code shows that the transport itself also survives a model edit.

### What is missing

- **The keys themselves.** `J` and `K` in `KtApp::handle_keys` (`src/apps/kt-app/src/app.rs`) are free, but **`L` is taken by toggling the loop** (`loop_toggle: i.key_pressed(egui::Key::L)`) — a direct conflict with the industry standard. The ready way out is suggested by Vegas: there the loop is `Q` or `Ctrl+Shift+L`.
- **Speed steps.** We have a continuous speed (the band, the overshoot, the space-bar acceleration); "pressed `L` three times → 1× / 2× / 4×" does not exist as a concept, and `speed_temp_slider()` knows exactly two sources of temporary speed — the shuttle and the space boost.
- **A frame step does not exist at all.** By grep over `src/apps/kt-app/src`: `ArrowLeft`/`ArrowRight` are bound to nothing. That is, we have neither `K`+tap nor its mouse equivalent — the hole is wider than JKL.
- **There is no concept of "the selected joint".** CLIPS are selected (`selected`), and the roll lives only inside the mouse drag over a joint. There is neither a choice of a joint from the keyboard (the analogue of `V` in Resolve), nor a choice of a SIDE — left / right / both (the analogue of `U`), and without a side there is nothing for JKL to move.
- **There is no "play around edit".** One can loop around a joint only by hand via the region, but the region is user data of the project (it is written into `.otio`), and pre/post-roll is a parameter of display, and we have no pre/post-roll settings at all.
- There is no trim mode as a mode, and no indication of such a mode.

### What gets in the way

1. **Our "moment of pressing" is coarser than a frame — and trim already reads it wrongly.** Invariant No. 6: the playhead moves in quanta of the audio callback (PipeWire — up to 42.7 ms, that is more than a frame at 24 fps), and `Transport::playhead()` is also the position BEFORE the output: one hears what lags behind by `latency_smooth`. For DISPLAY this is solved — `Player::video_playhead()` interpolates between the quanta and subtracts the latency (test 56, invariant No. 12). But `KtApp::trim_selected_start_to_cursor` / `..._end_to_cursor` take the RAW `self.player.transport.playhead()`: on pause it is correct, on the move it gives a systematic miss by a quantum plus the output latency, always in one direction. The first thing that will have to be done for dynamic trim is to make the time source for the EDIT `video_playhead()`, aligned to the fps grid. This is the same class that invariant No. 12 forbids for display, only now it is a question of writing to the model.
2. **The ring is already rendered a third of a second ahead.** `RING_CAPACITY_FRAMES = AUDIO_BURST_FRAMES * 16` = 16384 frames ≈ **341 ms** (`src/libs/kt-engine/src/player.rs`): the audio-render thread runs ahead of the playhead, and a model edit takes effect not at once — right after the press a third of a second of the PRE-revision edit still sounds. For the level of a clip this is unnoticeable (`Drag::Level` lives by this), for a moved joint it is heard as "the key worked not at the right time". Therefore a forced `AudioShared::flush` with a rollback of `render_head` WITHOUT a generation bump is needed: at present the flush is done only by `Player::seek`, and it bumps the generation and thereby resets the FX tails (invariant No. 3) and tears the sound.
3. **Changing the joint = changing what is being prefetched, right now.** Invariant No. 5: one MediaWorker owns the decoder, and all clients go through its priority queue `Playback > NearPrefetch > FarPrefetch > Thumbnail` (`src/libs/kt-engine/src/lib.rs`) with generation cancellation. During trim on the fly the needed material changes without a seek — the stale prefetch must be put out without putting out the playback, that is, a way is needed to cancel orders other than by bumping the generation.
4. **`set_project` clones the whole Project.** `KtApp::apply_coalesced` sends the command every frame of a gesture and every time does `self.player.set_project(self.model.project().clone())`. During trim on the fly this is dozens of full clones of the project per second SIMULTANEOUSLY with playback — directly against invariant No. 12 ("no allocations, no cloning of structures in draw paths per frame"). In addition, each step of JKL trim must be one undo step for the whole run, not one per frame.
5. **The modality against the checklist.** Rule 17 ("paired ways of one action behave the same") requires that the keyboard roll coincides with the mouse one to the tick; rule 9 ("nothing superfluous appeared") — that the indication of the mode does not turn into a separate strip (rule 13), but lives in an occupied place, as in Resolve with the cursor colour. Rule 4 ("the default is useful") suggests that entering the mode should not be mandatory: we already have the selected clip and the cursor, so Quick Trim without a mode fits better than the modal variant.
6. Rule 2 of the checklist ("works during PLAYBACK… the player can seek on the fly") here plays FOR us: trim has no "paused only" gates, there is nothing to look for and remove.

### Which invariants it touches

The root ones: **6** (the master clock and the callback quantum — the time source for the edit), **12** (interpolation of display, no cloning of structures per frame), **5** (the sole owner of the decoder, priorities and cancellation of orders), **3** (a model edit does not kill the FX state — everything rests on this). UI rules: **2**, **4**, **9**, **13**, **17**. The project format is not touched at all: pre/post-roll is an application setting, and the joint is moved by the existing `TrimStart` / `TrimEnd`.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages of the round (JKL transport and the frame step — one; choice of the joint + play-around — the second; trim on the fly with honest time — the third) |
| Where the main work is | kt-app (keys, steps, choice of the joint and side, indication, play-around), kt-engine (frame-accurate position for the EDIT, a ring flush without a generation bump, cancellation of prefetch without a seek) |
| Project format | does not touch it |
| Tests that will have to be written | the JKL steps as a pure function "press → speed" and `K` as a modifier; choice of joint/side from the keyboard (kt-app harness, next to t119/t120); "trim on the fly == trim on pause in the same frame" (engine-level, without wall-clock — invariant No. 11); play-around does not spoil the region of the project and does not write it into the file; moving the bind of `L` does not break t96 (loop) and t120; layout/i18n tests for the mode indication and the hint (t76/t83, t100/t125) |
| Depends on | — |
| Breaks compatibility | no; but the hotkey `L` (loop) moves — a change visible to the user, it requires his sanction (`docs/user-feedback.md`) |

## Verdict

We take it, but as two different things, not one. **The first — JKL as transport** (steps from repeated presses, the `K` modifier, the frame step): cheap, it sits on the ready `set_playback_rate` and `stop_points`, closes the hole "a frame step does not exist at all", and touches none of the invariants — except the conflict `L` = loop, which is resolved by moving the bind with the user's sanction. **The second — dynamic trim**: it costs more than it looks, because it runs not into trim (we have it, up to trim-to-cursor of the whole link group) but into HONEST TIME OF THE PRESS — the audio-callback quantum, the output latency and the 341 ms of the already rendered ring. To make it without "play around edit" with adjustable pre/post-roll is pointless: the method works by repetition, not by a single pass — and that is the essence of the mode for everyone who has it.

## Sources

- DaVinci Resolve Reference Manual, ch. 17 "Trimming", sections "Keyboard Trimming During Looped Playback", "Dynamic Trimming", "Trimming in Dynamic Mode" — https://ltbits.github.io/davinci-resolve-manuals/DR12/DR12-RM-17.pdf
  (accessed 2026-09-07). A mirror of the 18.6 manual (the same wording of the first paragraph of the section) —
  https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1084.htm
  — a direct query gave 404, the text is known only from a search snippet, so the quotes are taken from the PDF manual.
- Blackmagic Design, the Edit page of DaVinci Resolve:
  "You get context sensitive cursor tools, as well as dynamic JKL trimming
  which lets you trim during looping playback" —
  https://www.blackmagicdesign.com/products/davinciresolve/edit (2026-09-07)
- VEGAS Pro 22, help: hotkeys ("Scrub playback — J, K, or L";
  "Looped playback — Q or Ctrl+Shift+L") —
  https://help.magix-hub.com/video/vegas/22/en/content/topics/13-appendix/shortcutkeys.htm
  and the section "Scrub control" ("Press and hold K while pressing J or L to
  emulate a shuttle knob mode"; the setting `JKL / shuttle speed` on the Editing tab) —
  https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/scrubcontrol.htm
  (2026-09-07)
- Final Cut Pro User Guide, "Play media": doubling of speed by repeated pressing up to 32×, `K`+`J`/`L` — 1/2×, `K`+tap — a frame —
  https://support.apple.com/guide/final-cut-pro/play-media-ver90ba4ef0/mac
  (2026-09-07); "Use the Precision Editor" —
  https://support.apple.com/guide/final-cut-pro/use-the-precision-editor-verc1fac344/mac
- Adobe Support Community, Kevin J. Monahan (Adobe), 07.03.2020: "Dynamic
  trimming takes place in Trim mode, so you can use the JKL keys to shuttle
  to new edit points so that you can ripple and roll trim more intuitively
  and precisely" —
  https://community.adobe.com/questions-729/premiere-pro-guided-workflow-trimming-1360531
  (2026-09-07)
- Premiere, Preroll/Postroll in the Playback settings — the help page
  `helpx.adobe.com/premiere/desktop/get-started/preferences-and-settings/playback-preferences.html`
  **does not open from this environment** (see `_editors.md`): the presence of the Preroll/Postroll fields is known from a snippet of this page in the search results, the Play Around shortcut `Shift+K` — from a secondary analysis of the settings (PremiumBeat, filmora). NOT confirmed by the primary source — to be checked against helpx in the next wave.
- Forum quotes: Creative COW, the threads "Dynamic trimming" (2013),
  "Dynamic trimming in X" (2015), "Trimmer change playback speed" (2012) —
  links in the section "What users say" (2026-09-07).
- Avid Media Composer: the official Editing Guide (24 MB PDF on
  resources.avid.com) did not load in this environment, `community.avid.com`
  returns 403. The details of the trim mode (the key `U`, Play Loop, pre/post-roll on the Trim tab, "Hold K & L together and it plays at 8 fps") are taken from a third-party tutorial analysis
  https://www.chrisnicholas.net/tutorials/avid/trimming.html
  and are marked in the table as unverified.
- After Effects: `J`/`K` = the previous/next keyframe — confirmed by secondary sources and by the Adobe Community discussion of `Shift+J`/`Shift+K`
  (https://community.adobe.com/questions-534/feature-focus-new-shift-j-and-shift-k-keyboard-shortcuts-in-after-effects-314162);
  the official AE shortcut page on helpx is unavailable.
- Our code (by reading, 2026-09-07): `src/libs/kt-engine/src/player.rs`,
  `src/libs/kt-engine/src/transport.rs`, `src/libs/kt-engine/src/lib.rs`,
  `src/libs/kt-model/src/ops.rs`, `src/apps/kt-app/src/app.rs`,
  `src/apps/kt-app/src/timeline.rs`, `src/apps/kt-app/CLAUDE.md`,
  `docs/user-feedback.md` (R.16 item 3, R.17 item 3 — the space-bar acceleration; R.16 item 2 —
  Shift+A/D).
