---
id: F-AUD-008
title: Automatic slicing by silence
status: candidate
priority: B
cost: M
category: audio/processing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, logic-pro-12]
we_have: no
crates: [kt-media, kt-model, kt-app]
invariants: [1, 4, 8, 12]
ui_rules: [4, 12, 16, 20]
round: null
point: null
tests: []
depends_on: [F-TL-001, F-TXT-004]
checked: 2026-09-08
trackers: [kde-invent, kde-bugzilla, losslesscut, olive, pitivi, shotcut]
---

# F-AUD-008 — Automatic slicing by silence

## Essence

One action over a clip, a track or a region: the program itself finds the places where the sound stays below a set level for longer than a set time, and does something with them — places markers, cuts at the edges, or deletes them with a shift. For a podcast, a talking head and let's-play videos this is the only editing automation that saves hours rather than minutes: pauses, breaths and "mmm" eat up to forty percent of a take, and they are cut one by one by hand. There are always four and a half parameters: the threshold in dB, the minimum pause duration, the padding before, the padding after — and the merging of findings that are too close together.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none of its own; Vegasaur → Silence Detector | a third-party package puts REGIONS on the timeline inside the silences by "Signal Below level" + "Minimum Duration" + "Minimum Distance"; deletion is a separate second step, "Delete Inside Regions" of the Markers tool. The result can be edited and exported before anything is lost |
| Adobe Premiere | 26.x | no one-click tool of its own; Text-Based Editing | cuts not by LEVEL but by recognized text: a pause is visible in the transcript as a gap between words, and is deleted along with it. There is no silence threshold in the interface at all, but transcription is required. The gap is filled by the extension market (AutoCut, Silence Remover, Phantom Editor) |
| DaVinci Resolve | 21 (mechanism since 20.2) | Clip → Audio Operations → Ripple Delete Silence | the only one of the five with a BUILT-IN mechanism, and it is in the free version. Five controls: Threshold, Pre Head, Post Tail, Minimum to Trip, Cross fade audio. First it shows what it found as red bars over the clip, and only on confirmation does a ripple delete |
| Final Cut Pro | 12.3 | none | never appeared, although in Logic Pro from the same vendor the mechanism has lived for twenty years. The niche is taken by a third-party tool (SilenceScout: threshold + minimum duration and THREE kinds of result — "delete", "replace with gap clips", "only cut at the silence places") |
| After Effects | 26.x | none | the closest is Convert Audio to Keyframes: per-frame amplitude into keys, and only that; there is no threshold or duration, and in complete silence the values do not drop to zero. Cutting is done by a third-party script (Audio Silent Cutter) |

**Outside the core.** Four of the five core lines are "none", and the shape of the result and the set of parameters themselves were born not in video editing: they were set by a sound editor, and Resolve repeats them almost word for word twenty years later.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Apple Logic Pro | 12.3.1 | Remove Silence (historical name — Strip Silence) | a separate window with four parameters: Threshold, Minimum Time to Accept as Silence, Pre Attack-Time, Post Release-Time. It deletes nothing: it CUTS the material into regions at the found boundaries, and the user then does what they want with the regions |

## The trick behind it

**First: everyone who brought this to people arrived at "show, then cut" — and arrived by different paths.** Logic does not delete at all, it cuts into regions. Vegasaur places regions, and deletion is a SEPARATE command of another tool ("Delete Inside Regions" in Markers), that is, a second conscious step. SilenceScout for Final Cut keeps three kinds of result, and the third — "just cut at the silence places" — is for manual review. Even Resolve, which is called Ripple Delete Silence and cuts at once, first draws red bars over the clip and waits for confirmation. The reason is the same for all: the threshold is always wrong, because it depends on the microphone and the room, not on the material — and fixing a hundred OTHER people's cuts costs more than moving twenty markers.

**Second: there are two paddings, they are asymmetric, and they matter more than the threshold.** Logic calls them Pre Attack-Time and Post Release-Time and explains in its help that they have DIFFERENT causes: the front one saves the attack of a non-percussive sound (voice, winds, strings), the back one — the tail that fades gradually and is cut off the harder, the higher the threshold. Resolve renamed them Pre Head / Post Tail but kept both. The customer of the feature in LosslessCut phrased their request through the padding, not through the threshold, and named the number: 500 ms.

**Third: the parameter that decides the fate of the result is missing from every list of features — merging of neighbouring findings.** In Vegasaur it is called Minimum Distance and merges intervals between which the distance is less than the set value. It is dual to the minimum pause duration: the first sets how much SILENCE is needed for it to be noticed, the second — how much SPEECH is needed for two pauses to count as different. Without the second, a threshold set a couple of decibels above the needed one produces two hundred micro-cuts inside one phrase; with it — a dozen real pauses. It is precisely on this parameter that "works" and "does not work" diverge for one and the same detector.

## Pros

- The result in the form of MARKERS destroys nothing: the threshold turned out to be wrong — the markers are removed with one action, and the material is untouched. Cuts cannot be removed like that: there is no way to glue a hundred joins back together except undo, and by that moment undo has already been spent on something else.
- The analysis works on the signal level, so it works in any language, without a network, without models and without installing anything at all. This is its only but decisive advantage over text-based editing (F-TXT-004): the latter answers the question "what is said", this one answers the question "is anything said", and the second question is cheaper by several orders of magnitude.
- Two margins instead of one make the result usable right away: the front margin saves the breath before a word, the back margin saves the decaying tail. One symmetric margin in dialogue always either cuts the breath or leaves junk.
- Merging neighbours (Minimum Distance) turns the setting from a lottery into a scale: the user turns one number and sees two hundred findings become twenty, without touching the threshold.

## Cons

- The threshold is a property of the MICROPHONE AND THE ROOM, not of the material: a value suitable for everything does not exist, and the first application almost always gives junk. Hence the mandatory preview, and with it — the second half of the cost of the feature.
- A level detector does not distinguish a pause from a quiet word, a whisper and a fading phrase. Everything said below the threshold is silence for it, and that is exactly what gets caught by hand when the result is reviewed.
- On mixed material it is useless: a music bed keeps the level above the threshold all the time. It works only on an isolated speech track — that is, it requires the edit to already be laid out on tracks.
- Ripple deletion shortens the duration and pulls EVERYTHING to the right, including what is not connected with speech — titles, music, markers. This is not a property of the detector but the price of the chosen form of the result, and it is paid every time.
- The "cut" form multiplies joins: where there was one pause, there become two events and a join between them, and a join is the place where clicks, transitions and rounding errors later appear.

## What users say

> it would save a lot of time and make shorter videos without taking hours to
> cut the silences manually.
> — kde-invent #247 «Feature request: add "auto cuts silences" option»,
> 12.06.2019, https://invent.kde.org/multimedia/kdenlive/-/work_items/247

The request is still open (last activity 11.02.2026) and holds **eleven votes — the maximum of the whole tracker**: among all 2217 tasks in `kde-invent` exactly one has eleven or more upvotes, and it is this one. The next by votes ("Add easing modes for keyframes") gathered nine and has long been closed.

> to automatically detect when the audio is under a certain level, add 500
> milliseconds of padding and mark to cut there.
> — losslesscut #1408 «automatically cut silence», 27.12.2022,
> https://github.com/mifi/lossless-cut/issues/1408

The wording is worth rereading: the person described the feature as "threshold + PADDING + MARK", not as "threshold + cut". The padding and the marking entered the request before anyone's implementation did.

> I like 'auto cut silence' from Olive_1, it fit's my workflow and assume it's
> same for plenty of others. Alas it's missing in Olive_2 ( or did I overlook? )
> — olive #2348 «UI / missing option 'auto cut silence'», 02.09.2024,
> https://github.com/olive-editor/olive/issues/2348

The negative side: the feature WAS there and disappeared when the editor was rewritten, and the user was left, in his words, between two half-finished solutions.

> A part at the end of the file remains undetected as silent, and executing
> "Invert all segments in timeline" results in a very short segment of 0ms.
> — losslesscut #2191 «Silent Detection Misses at File End», 11.10.2024,
> https://github.com/mifi/lossless-cut/issues/2191

Still open. The author checked the samples in the tail (±6 on the 16-bit scale, that is about −74 dBFS) and made sure the threshold has nothing to do with it — the material traversal itself misses. We keep this task separately: it lands exactly in our code, see `ADDRESS:`.

SIGNAL: silence-based cutting — 8 tasks in losslesscut, `(detect|cut|remove|split|trim).{0,20}silen|silen.{0,20}(detect|scene|part|section)`; share 0.4% against 0.0% in kde-invent

In total on the topic across twelve trackers — eleven tasks, and eight of them belong to the single project where the detector works TODAY (in Olive it existed in the first version and did not survive the rewrite). There it is already hardly a request: two tasks are requests for the feature itself (#1334, #1408), four are about the detector lying ("finds nothing at all" #1721 and #1742, "does not see the end of the file" #2191, "the second mode creates segments twice" #2236), and two more are requests to finish what is already done ("can search only the first audio track" #2354, "also find an empty video"
#2833). The conclusion for which the count was made: requests for this feature across the whole industry are a handful, while its OPERATION by the one who built it produced four times more tasks than the request itself; the cheap part here is the detector, the expensive part is everything around it.

SIGNAL: the price of the text path — 17 tasks in kde-invent, `speech.?to.?text|whisper|transcri|speech recognition`; share 0.8% against 0.2% in shotcut

This is the same feature, chosen in another form, and its count is twice as large — moreover in the tracker of the one who never made offline cutting. In the body of kde-invent #247 there is a checklist of four implementation variants (FFmpeg `silencedetect`/`silenceremove`, AutomEditor, jumpcutter, speech recognition), and exactly one — the last — is ticked. Kdenlive went the text way, and of the seventeen tasks TEN are not about the quality of cutting but about INSTALLATION and the environment: "Can't install missing Whisper dependencies", "System provided Whisper not detected", "System Python packages not detected", "Unable to install whisper in venv on Arch Linux", "Windows: detect GPU for WHISPER". Offline cutting by level has nonetheless never been done — seven years and first place by votes.

A reverse confirmation from the same side: in LosslessCut, where the level detector works, the proposal to extend it with speech recognition was closed by the maintainer as `not_planned` (#2984 "[Feature Proposal] AI Speech Transcription &
Script-Comparison Auto-Cutting", 28.07.2026 → 24.08.2026,
https://github.com/mifi/lossless-cut/issues/2984).

## How it fits Kinotrain

ADDRESS: src/libs/kt-media/src/index.rs:389 — the class "the detector misses at the very end of the file" (losslesscut #2191) is POSSIBLE with us, and nothing catches it: `PeakPyramid::build` takes `total_bursts = duration_frames.div_euclid(1024)` and reads only WHOLE bursts, therefore a tail up to 1023 samples long (21.3 ms) never gets into the pyramid. The safeguard below — `if in_block > 0` on line 408 — never fires: 1024 is a multiple of 256, and after any number of full bursts no unclosed block remains. For a waveform a shortfall of 21 ms is invisible; for a silence threshold this is exactly the defect people have complained about since 2024; there is no regression test for the pyramid's tail either in `src/libs/kt-media/tests/index_db.rs` or in `src/libs/kt-engine/tests/viewport.rs`.

### What is already there

The main point: **we do not need a new pass over the material.** The peak pyramid is already built in the background for the sake of waveforms, lives in the sidecar DB and is fully loaded into the application's memory — the silence threshold is computed from it without a single decode.

- **The pyramid.** `src/libs/kt-media/src/index.rs:378` — `PeakPyramid`, `levels[level][channel][block] = (min, max)` in `i16`. Level 0 is a block of `PEAK_BLOCK_FRAMES = 256` frames (`src/libs/kt-media/src/lib.rs:97`), each next one twice as long; there are exactly two channels, because the source is by invariant 1 always 48 kHz stereo f32. 256 frames at 48 kHz is **5.333 ms**, that is, the resolution of the pause search is better than anyone needs: a typical "minimum pause" starts at 200 ms.
- **Storage and the background worker.** The table `audio_peaks (media_id, level, channel, data)` in `<project>.ktindex.db` (`docs/media-index.md`), the writer — `MediaIndexDb::build_audio_peaks` (`index.rs:220`); it is built by a short-lived `FfmpegSource`, not by the playback decoders (invariant 5). The worker — `spawn_index_worker` in `src/apps/kt-app/src/util.rs:89`, called from three places in `src/apps/kt-app/src/app.rs` (opening a project, adding media, reopening).
- **Everything in memory.** The field `peaks: Arc<Mutex<HashMap<MediaId, Peaks>>>` of the `KtApp` struct (`src/apps/kt-app/src/app.rs`), where `Peaks = Vec<(Vec<(i16,i16)>, Vec<(i16,i16)>)>` (`util.rs:11`) — ALL levels of both channels. An hour of material is about 5.4 MB at the zero level and half as much at all the others together.
- **The result forms already exist as model commands.** `Command::AddMarker { at, duration }` (`src/libs/kt-model/src/model.rs:180`) with `duration > 0` is a RANGE marker (OTIO `marked_range`), exactly what Logic and Vegasaur hand over as the findings; `SetMarkerName` next to it. Cutting — `Command::Split { id, at, new_id, new_group }` (`model.rs:124`).
- **A batch is atomic and costs one undo step.** `Model::apply_batch` (`model.rs:370`): validation of the whole series on a clone, a failure of any command = zero changes and zero entries in the history. Two hundred markers or two hundred cuts — one "Undo". This is a direct answer to the main drawback of other implementations.
- **The surface for the call is ready and half empty** — the timeline's context menu: there already live `marker_region`, `punch_out` ("Cut out the interior of the region", `src/libs/kt-model/src/ops.rs:375`) and `delete_in_region` (`src/apps/kt-app/i18n/ru.toml`, section `[tl.menu]`). The second surface, which the second wave named the cheapest (F-HLP-016), is already occupied here by related items.
- **Displaying the paddings is free.** A range marker is drawn as a band on the time-label row (`src/apps/kt-app/src/timeline.rs`, the loop `for m in &self.model.project().markers`, `marker_band_rect`), and its edges are dragged with the mouse (`Drag::MarkerEdge`) — that is, the preview "with red bars", which Resolve draws specially, we get as a standard widget, and it can even be moved.
- **The class "offline analysis of material" is already named.** A quote of the first wave's conclusion (README, "What the first wave showed"): "Five different features run into ONE missing subsystem — 'offline analysis of material'… and for it we ALREADY have a place: the SQLite sidecar `<project>.ktindex.db`". This note is the sixth feature of the same class and the only one that does NOT need a new pass: its data in the pyramid already lies there.

### What is missing

- **A detector.** There is no function anywhere that "runs the pyramid through a threshold and returns the ranges": `grep -ri silence src/ --include=*.rs` gives only `AudioPattern::Silence` in the synthetic test sources. There is also no conversion of an `i16` peak to dBFS.
- **Merging of neighbouring findings** (Minimum Distance in Vegasaur) — the very parameter without which the result turns into two hundred micro-cuts.
- **A back-projection "source → timeline".** The pyramid lives in the MATERIAL domain (like automation keys, F-AUT-002), and markers live in the timeline domain (`AddMarker { at: Tick }` — the absolute tick of the project). The mapping today is affine and exact: `source_in + (t − start)`, and `reversed` mirrors it (`ClipEvent`, `src/libs/kt-model/src/project.rs:1784`) — but it still has to be written, and it will shift when the event's speed appears (F-SPD-001).
- **Ripple.** The very thing for which the feature asks — "make shorter videos". We have `punch_out_region` and `delete_region` (`ops.rs:375`, `ops.rs:403`), and both leave a HOLE: the model has no shifting of everything to the right (F-TL-001, priority A, cost L; closing holes — F-TL-008). Without it, auto-cutting honestly does only half the work, and this must be said aloud, not discovered after launch.

### What gets in the way

- **The identity of a marker is its tick.** `Marker` is deduplicated by `t` (`project.rs:1410`), and `MarkedArea` requires a non-empty name, UNIQUE within the namespace shared with markers (`project.rs:1427`). So named areas are not suitable at all for a mass result (two hundred auto-names are two hundred traps in the morpher picker), and markers are suitable only as long as the pass is ONE: two passes over two tracks easily give two pauses starting at the same tick, and the second will silently disappear. F-TXT-004 has already found exactly this boundary ("deduplication by tick and a shared unique namespace of names with areas is not a minor limitation, but a different identity model"), and there is no need to rediscover it: it means that the unit of work is one clip or one region, not "the whole project at once".
- **A marker does not know about a track.** `Project.markers` is a flat list for the whole project (`project.rs:458`); `MarkedArea` has tracks, a marker does not. Silence found on A2 will be shown as markup of the whole project.
- **A threshold below −70 dBFS runs into the quantization of the pyramid.** Peaks are stored as `i16` (`index.rs:394`), and the unit of the scale is −90.3 dBFS. A threshold of −60 dB is 33 units, a threshold of −80 dB is already 3, and the difference between "silence" and "ADC noise" melts away. For a speaker's recording this is not a hindrance; for "find absolute silence in a live recording" (losslesscut #2833) it is a hindrance.
- **Rule 20 of the checklist.** Markers that the feature placed must not "regrow" on the next opening of the project or on an index rebuild: the result of analysis is the user's edit, not a derivative of the material. So the pass must have a launch point and must not be a background re-application.

### Which invariants it touches

- **1 (audio always f32 48 kHz/2ch).** It helps: the pyramid is already built after resampling, so a threshold in dBFS means the same thing for any source, and a block of 256 frames is always 5.333 ms. In other products, where analysis runs at the file's rate, the pause duration has to be recalculated.
- **4 (the cache is the only truth about coverage).** Not touched and must not be: the detector reads the pyramid, not the burst cache. The temptation to "count over the warmed-up bursts" is a direct violation.
- **8 (scroll/zoom does not spawn decode requests).** The same principle as for waveforms: the analysis must live on the pyramid. A pass over the material by a decoder for the sake of a threshold would make the feature more expensive than stabilization.
- **12 (repaint only on movement; thousands of primitives — as meshes).** The marker drawing loop (`src/apps/kt-app/src/timeline.rs`) walks the WHOLE list linearly every frame, and for visible named markers it calls `m.name.clone()` and `layout_no_wrap`. Today there are single markers; two hundred markers from a pass would be two hundred string clones in the draw path, which invariant 12 does not allow.
- **Rule 16 of the checklist (impossibility is an ERROR).** `MIN_CLIP_DURATION = Tick(90)` (1 ms, `model.rs:95`) rejects a `Split` that produces too short a piece, and `apply_batch` is atomic — so one bad cut would drop the WHOLE batch of two hundred. This is correct, and it must be shown as an error with a number, not silently skipped (compare `ops.rs:54`: what was skipped during insertion is counted and goes into the message).
- **Rule 4 (the default is useful).** A threshold suitable for everyone does not exist — so the default must not be a number pulled out of thin air but a preview: show what was found before anything is touched.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2 stages of the round: the detector over the pyramid + the result as markers; cutting — separately and later |
| Where the main work is | kt-media (a pass over `PeakPyramid` with a threshold, paddings and merging), kt-app (parameter window, preview, call from the timeline's context menu) |
| Project format | not touched: the range marker and `Split` are already in the format, `marked_range` has been traveling in .otio since R.24 |
| Tests that will have to be written | the detector on synthetic input (threshold, minimum pause, both paddings, merging of neighbours); **the tail of the pyramid — a regression for the defect in `ADDRESS:`**; the back-projection "source → timeline" with `reversed`; atomicity of a batch of markers in one undo; rejection of a batch of cuts by `MIN_CLIP_DURATION` as an ERROR with a number; drawing with 200+ markers (invariant 12) |
| Depends on | F-TL-001 (ripple — only for the "delete" form), F-TXT-004 (the boundary with text editing and the identity model of markers) |
| Breaks compatibility | no |

## Verdict

**Take it, and take it as markup.** The first stage — a pass over the peak pyramid and the result as range markers: it has no new subsystem, no new pass over the material, and not a single line in the project format — the data already lies in memory for the sake of waveforms, both model commands already exist, the atomic batch already gives "two hundred markers — one Undo", and the preview, which Resolve draws with special code, for us is a standard marker band that, in addition, can be moved with the mouse. This is the cheapest feature of the "offline analysis of material" class out of six, and the only one that does not need a new pass — and a good pilot for the class instead of the noise profile.

**Cutting — the second stage, and only after ripple (F-TL-001).** The user's request sounds like "make shorter videos"; without shifting the right-hand part we would do half and leave holes. The "delete at once" form is not taken at any stage: all five implementations that reached people differed in details and converged on one thing — show first.

**Speech recognition is not part of this note** and stays with F-TXT-004: the count of 17 tasks against 8 shows what the text path costs, and it is paid not in the quality of cutting but in installing someone else's runtime — the very thing we do not have under invariant 10.

Separately, regardless of the verdict: the defect of the pyramid's tail from the `ADDRESS:` line should be closed with a test NOW, whether or not the feature happens. Today it shows up as an unnoticed shortfall of 21 ms at the end of a waveform, and with the detector it will become the same bug that has been open in LosslessCut since 2024.

## Sources

- kde-invent #247 "Feature request: add "auto cuts silences" option",
  https://invent.kde.org/multimedia/kdenlive/-/work_items/247 (12.06.2019,
  open, last activity 11.02.2026; 11 upvotes — the tracker's maximum)
- kde-bugzilla #368775 "[Wishlist] Remove automatically silent part of video.",
  https://bugs.kde.org/show_bug.cgi?id=368775 (13.09.2016, RESOLVED/MOVED — the ancestor of
  #247)
- olive #2348 "UI / missing option 'auto cut silence'",
  https://github.com/olive-editor/olive/issues/2348 (02.09.2024, open)
- pitivi #1999 "Splliter Video based on audio silence",
  https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1999 (10.02.2017, closed)
- losslesscut #1408, #1334, #1721, #1742, #2191, #2236, #2354, #2833, #2984 —
  the whole life of the feature in the single project that implemented it:
  https://github.com/mifi/lossless-cut/issues/1408 and so on by number
- The numbers of the `SIGNAL:` lines are reproduced by `python3 src/scripts/tracker-rank.py
  --query '<filter>'` over the dumps in `~/.cache/kinotrain/trackers/` (they are not put into the repository, `docs/features/_trackers.md`); on the check date there were 12
  trackers of the registry in the cache
- Logic Pro, "Remove silent passages in Logic Pro", Apple Support,
  https://support.apple.com/guide/logicpro/lgcp86ee7672/mac — parameters Threshold,
  Minimum Time to Accept as Silence, Pre Attack-Time, Post Release-Time (accessed 2026-09-08); version — Logic Pro 12.3.1 per
  https://support.apple.com/en-us/109503 (14.08.2026)
- Vegasaur Silence Detector, https://vegasaur.com/silence-detector — "Signal
  Below level", "Minimum Duration", "Minimum Distance", the result as regions,
  deletion via "Delete Inside Regions" of the Markers tool. **The site did not open from this environment** (the proxy rejected the connection), the wording is taken from the search results for this same page, not verified live
- DaVinci Resolve, Clip → Audio Operations → Ripple Delete Silence: Threshold,
  Pre Head, Post Tail, Minimum to Trip, Cross fade audio; preview with red
  bars; mechanism since 20.2, available in the free version —
  https://davinciresolveclub.com/remove-silence-davinci-resolve/ (accessed
  2026-09-08). **Secondary source:** the primary source would be the Blackmagic help, which could not be reached from this environment
- SilenceScout for Final Cut Pro / DaVinci Resolve (three kinds of result),
  https://fxfactory.com/info/silencescout/ — per the search results, not verified live;
  Premiere and After Effects — per the search results and the Adobe community;
  `helpx.adobe.com` is still unavailable from this environment (see `_editors.md`,
  "What was unavailable during the first wave")
- Our code was checked by grep on 08.09.2026: `src/libs/kt-media/src/index.rs`,
  `src/libs/kt-media/src/lib.rs`, `src/libs/kt-model/src/{model,project,ops}.rs`,
  `src/apps/kt-app/src/{app,util,timeline}.rs`, `docs/media-index.md`
