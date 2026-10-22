---
id: F-MED-003
title: Images, frame sequences and footage with alpha
status: in-progress
priority: A
cost: L
category: media/sources
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-media, kt-ffmpeg, kt-engine, kt-gpu, kt-model, kt-otio, kt-app]
invariants: [4, 5, 7, 8, 11]
ui_rules: [13, 17, 20]
round: R.67
point: null
tests: [t796, t796b, t796c, t796d]
depends_on: []
checked: 2026-09-27
---

# F-MED-003 — Images, frame sequences and footage with alpha

## Essence

Three kinds of material that the editor takes for one ("not video"), while the editor must tell them apart. **A single picture** — it has no duration: how many seconds it hangs in the frame is decided not by the file but by a setting. **A frame sequence** (`shot_0001.png`…`shot_0300.png`) — a set of files that behaves as one clip, and it has no fps of its own: the speed is set from outside. **Material with alpha** (PNG, ProRes 4444, QuickTime RGBA, WebM VP9) — brings transparency, and the main question is not "is there alpha" but **how to interpret the RGB under it**: premultiplied or straight.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | "New still image length", "Open sequence" | the length of a picture is seconds in Options → Preferences → Editing ("In the New still image length box, type the number of seconds"); a numbered sequence is opened as ONE event via the "Open sequence" checkbox in the open dialog, and the help says directly: "Each image in the sequence will be displayed for one frame" — that is, the fps of the sequence = the fps of the project, not a property of the material. The switch of alpha interpretation in the media properties is not confirmed by the primary source [NOT VERIFIED] |
| Adobe Premiere | 26 | "Still Image Default Duration", "Indeterminate Media Timebase", "Interpret Footage" | the length of a picture — Preferences → Timeline → Still Image Default Duration (seconds/frames); a sequence is imported with a checkbox at import, and its fps is taken from the GLOBAL setting Preferences → Media → Indeterminate Media Timebase (default 30/29.97) and is not applied retroactively to what is already imported — it is corrected one by one via Modify → Interpret Footage → "Assume this frame rate". Alpha: the official guide gives "Ignore Alpha Channel" and "Invert Alpha Channel"; no separate straight/premultiplied switch was found in Interpret Footage [NOT VERIFIED: helpx.adobe.com does not open from this environment] |
| DaVinci Resolve | 21 (also checked against 18.x/20.x) | "Standard still duration", Clip Attributes → Alpha Mode | the length of a picture — Preferences → User → Editing → Standard still duration (default 5 s); numbered frames the Media Pool assembles into one clip by itself, the Frame Display Mode switch "Sequence"/"Individual" is sticky — it holds until the next change [NOT VERIFIED: the information comes from search results of the BMD forum, the page itself did not open from this environment]; fps is corrected PER CLIP in Clip Attributes → Video → Frame Rate; alpha — there too, Alpha Mode: "choose the type of alpha channel (Premultiplied or Straight), or invert the alpha channel", plus None (ignore) |
| Final Cut Pro | 12.3 | Settings → Editing → Still images | "The default initial duration for still images is 4 seconds, but you can change this setting in the Editing pane of Final Cut Pro settings" — the only one of the five whose default is not 5 s. There is no native IMPORT of a numbered sequence (the help describes only exporting image sequences); the path is conversion via QuickTime/Motion/Compressor or a plug-in. There is no switch of alpha interpretation [NOT VERIFIED: discussions.apple.com is unavailable] |
| After Effects | 26 | Interpret Footage → Alpha, Preferences → Import, effect Unmult | four variants of alpha interpretation: Ignore, Straight – Unmatted, Premultiplied – Matted With Color (the colour of the backing is chosen), Guess; the length of a picture and "Sequence Footage: N frames per second" — in Preferences → Import; since version 26 a BUILT-IN effect Unmult has appeared (Effect → Keying): "Unmult removes solid black or white backgrounds from a layer to restore the original transparency from when your footage was captured" |

## The trick behind it

**1. Premultiplied versus straight — the least noticeable error in the industry, and its price is that the bytes are indistinguishable.** A file with alpha has three entities, not two: RGB, A, and WHAT the semi-transparent pixels are mixed over. In a straight file RGB is the colour of the object as it is, and transparency lives only in A. In a premultiplied file RGB is already multiplied by A, that is, it is "how the object looks on black". Formally these are different data, but physically it is one and the same RGBA array: they can be told apart only by convention. Reading a premultiplied file as straight gives a DARK fringe at the anti-aliased edge (the semi-transparent edge is multiplied by alpha twice); reading a straight file as premultiplied gives a light halo. The symptom at the same time never points to the cause: the fringe is visible ONLY ON THE COMPOSITE, while in the preview of the file it is absent, and it is explained as "badly cut out". The direct admission of the problem by the industry is the **Unmult** effect, which Adobe has made built-in in AE 26: it is needed to pull transparency out of a frame where it has already been flattened over black, that is, to repair the consequences of exactly this indistinguishability. Resolve gives an `Alpha Mode` switch per clip, AE has four variants and "Guess"; Premiere and FCP give no choice at all and decide for the user.

**2. A sequence carries no fps of its own — and a gap in the numbering silently shortens it.** Checked by measurement on the system FFmpeg on 07.09.2026: `ffprobe "shot_%04d.png"` on five files gives `format_name=image2`, `avg_frame_rate=25/1`, `duration=0.200000` — twenty-five frames per second were NOT taken from the files, the demuxer invented them by default. Therefore one and the same set of frames runs at different speeds in two editors, and the editor sees not "a wrong fps" but "the video is too fast". The second thing the same check does: we delete `shot_0003.png` — and `duration` becomes `0.080000`, that is, **two frames** instead of four. The demuxer does not skip the gap and does not complain: it stops at it, and the tail of the sequence simply disappears. On top of this, one more thing lies with us: the mapping "greatest pts ≤ t" holds the last frame, so the end of a trimmed sequence turns not into black (which would be noticeable) but into a freeze frame.

**3. In Premiere "Scale to Frame Size" and "Set to Frame Size" are two neighbouring items of ONE menu with different results.** An Adobe employee in the official community: "Set to Frame Size will adjust Motion > Scale to make the clip fit
within the Sequence frame size. Scale to Frame Size will rasterize the file to
fit the Sequence frame size". That is, the first item touches the motion parameter and leaves the full resolution of the file (a 4000 px logo in a 1080p sequence can later be enlarged without loss), while the second rasterises the file to the sequence size — after it the file is physically 1080p, and enlarging makes it blurry. The names differ by one word, the result by irreversibility; in the same thread a community expert proposes renaming the second item to "Rasterize to frame size". This is exactly the detail that is missing from the list of features and because of which the question "why is my logo soft" arises.

## Pros

- The default duration is COPIED INTO THE EVENT at placement, rather than read from a setting at playback: 200 photographs turn into a slideshow with one drop, and a later edit of the default does not rewrite events already placed.
- A sequence collapsed into one clip inherits the WHOLE usual mechanics of an event: trim, split, reverse and warp work, because its mapping "tick → source frame" is the same as for a video file; 3000 files take up one bin card.
- Interpreting alpha as a property of the MEDIA (AE, Resolve) fixes the fringe without re-rendering and without editing each event: you switch `Alpha Mode` — and all clips that use this file are updated.
- Material with alpha needs neither a mask nor a key: the composite already knows alpha-over, and the graphics lie on a track like ordinary video. This is the only way to put external graphics into a film without introducing drawing or text tools into the editor.

## Cons

- The fps of a sequence lives OUTSIDE the file, so it cannot be checked: the editor substitutes its own default silently, and in Premiere it is also global — a change of the setting does not affect what is already imported, one must re-import.
- A gap in the numbering is not an error but a silent truncation: the duration is counted over the continuous run from the first file, the remaining frames do not exist.
- Interpretation of alpha is a property of the FILE, not of the event: two different uses of one PNG cannot be interpreted differently without creating a second media record.
- Rasterising to the sequence size (Premiere "Scale to Frame Size") makes the file exactly as the sequence is: further enlargement makes it blurry, and this can be undone only by removing the attribute and setting the scale again.
- A default duration in SECONDS in frame-based editing gives a non-integer number of frames (5 s at 23.976 — 119.88 frames), so editors either round silently or store the setting in frames, and two projects with different fps get different slideshows from one folder.

## What users say

> If I set it to premultiplied (black), the black edge is less prominent but
> it's still there.
> — Dustin Brown, Creative COW, 13.10.2008, https://creativecow.net/forums/thread/alpha-track-matte-yields-edge-or-halo/

> Premiere Pro always default image sequences to 30 fps and you must manually
> change that.
> — Averdahl (Community Expert), Adobe Community, 01.07.2022, https://community.adobe.com/questions-729/image-sequence-export-issue-frame-rate-always-30fps-1392096

> About time! 32bit, linear compatible, GPU accelerated, AddGrain/FilmGrain/
> MatchGrain next?
> — Shebbe, Adobe Community (announcement of the Unmult effect in the AE 26 beta), 23.10.2025, https://community.adobe.com/announcements-532/new-in-beta-unmult-effect-about-time-right-314503

The third quote is enthusiasm, but its value lies not in the enthusiasm: "about time" about an effect that pulls transparency out of a frame flattened over black means that the indistinguishability of premultiplied/straight had been tormenting AE users for years and was being closed by a third-party plug-in.

## How it fits Kinotrain

### What already exists

Checked not by reading the code but by running it (07.09.2026, `kt-cli export`, built as `target/debug/kt-cli`):

- **A single PNG already works as a clip.** A project with `ExternalReference.target_url = "alpha.png"` and a `source_range` of 1 s exports: `ok: 30 video frames, missing layers 0`. The picture holds for the whole length of the event — the mapping "greatest pts ≤ t" in `src/libs/kt-engine/src/video.rs` gives a freeze frame for free.
- **A sequence also already works — if the template is typed into the path by hand.** `target_url = "shot_%04d.png"` opens (FFmpeg takes the image2 demuxer) and PLAYS BACK: in a 30-fps project frames 0/2/4/6 showed files 1/2/4/5 of the sequence — that is, the demuxer's default 25 fps against the project's 30 fps, and after the end of the sequence the last frame is held.
- **The "5 seconds" default we ALREADY have, but it is hard-coded.** `src/apps/kt-app/src/app.rs`, `place_media_at()`: `let dur = if info.duration.0 > 0 { info.duration } else { Tick(90_000 * 5) };`. For a single PNG this is what fires: `ffprobe` on it gives `format_name=png_pipe, duration=N/A`, our probe (`src/libs/kt-ffmpeg/src/source_impl.rs`, the branch `if sc.duration > 0`) puts `Tick(0)`, and the five-second margin is switched on.
- **A precedent of a source without a normal length already exists** — `src/libs/kt-media/src/placeholder.rs`, `PLACEHOLDER_DURATION` = 1000 hours with the comment "The placeholder does not know how long the missing file lasted, and it must not trim the edit". That is, the `MediaSource` trait (`src/libs/kt-media/src/lib.rs`) already tolerates a source whose duration is invented rather than read.
- **The composite handles alpha and handles it as STRAIGHT.** `alpha_over_scalar` (`src/libs/kt-engine/src/lib.rs`) computes `s*sa + d*inv*da/255`, that is, it multiplies the source's RGB by its alpha AT THE MOMENT OF COMPOSITING — so the input must be NOT premultiplied. The only producer of alpha today is the chroma key: `greenscreen()` in `src/libs/kt-fx/src/video.rs` (line 385) changes only `px[3]` and sets `f.has_alpha = true`, leaving RGB untouched. The GPU mirror is `over()` in `src/libs/kt-gpu/src/shaders/composite.wgsl`, the flags `a_has_alpha`/`b_has_alpha` travel in the uniform. A detailed analysis of the same place is in F-MSK-003 (track matte), section "How it fits".

### What is missing

- **Alpha from the file does not reach the engine at all.** `FfmpegSource` sets `has_alpha: false` hard-coded — in `read_video_frame` and in `read_video_span` (`src/libs/kt-media/src/ffmpeg.rs`, lines 133 and 160). The alpha bytes in the buffer ARE there: sws converts to `AV_PIX_FMT_RGBA` (`src/libs/kt-ffmpeg/src/source_impl.rs`, `to_rgba`), and an `rgba` PNG is decoded with `pix_fmt=rgba` — but the composite has been told to treat the frame as opaque. Measured: a PNG with a solid `red@0.5` (RGBA 255,0,0,128) exports as **(253, 0, 0)** — full red. The alpha is dropped silently. This is not "no feature", it is a WRONG result on correct input, and that is why the priority is A.
- **There is no choice of interpretation at any level.** Neither a field in `MediaRef` (`src/libs/kt-model/src/project.rs`, line 1497 — there are only `id` and `path`), nor an item in the UI. Our composite is straight-only; foreign graphics rendered as premultiplied (the usual output of AE and 3D renderers) will give exactly that dark fringe as soon as alpha is switched on.
- **The duration of a picture and the fps of a sequence are not configurable.** `Tick(90_000 * 5)` is a constant in the body of the function; in the project model (`ProjectSettings` in `src/libs/kt-model/src/project.rs`) there are no fields for either. UI rule 20 ("nothing grows or gets cut off by itself") requires that such a default be a visible setting, not a surprise.
- **A sequence is not imported by a gesture.** `rfd::FileDialog::pick_file()` (`src/apps/kt-app/src/app.rs`) picks ONE file; the template `%04d` can be typed into the path only by hand in `.otio`. There is no recognition of numbering, no dialog "take so many frames", and no separate kind of card in the bin.
- **OTIO `ImageSequenceReference` falls into passthrough and loses the path.** `decode_media_ref` (`src/libs/kt-otio/src/schema.rs`, line 462) knows only `ExternalReference.1` and `MissingReference.1`, everything else is `MediaRefNode::Unknown` (the raw object is carried verbatim, the file is not corrupted). But in `src/libs/kt-otio/src/convert.rs` (line 1145) the path is taken ONLY from `External`, so a foreign project with a sequence is imported as a clip with an empty path — that is, as a placeholder for a missing source. For a format whose purpose is exchange, this is a noticeable hole: `ImageSequenceReference` in OTIO was devised exactly for this case and carries `rate`, `frame_step`, `missing_frame_policy`.
- **The index DB is almost entirely pointless for a picture.** `src/libs/kt-media/src/index.rs`: `packet_index` (positions of audio packets and video keyframes) is empty for a single PNG, a peak pyramid is not needed, and of the whole schema only `thumbnail`/`poster` make sense — and even those degenerate for a single picture into "a reduced copy of itself". For a SEQUENCE, on the contrary, everything is meaningful and expensive: hashing the file (`hash_file`) on 3000 files is not what is needed.

### What gets in the way

Three stops, and all three are constructive rather than volumetric.

1. **Straight is not a solution but half of one.** Enabling `has_alpha` honestly is one line; after it any premultiplied graphics will start giving a fringe, and the editor will be to blame. So together with the flag there must appear the interpretation as a PROPERTY OF THE MEDIA, carried through to the project format and to both backends. Moreover, the unmatte operation itself (dividing RGB by A) is not expressible without loss in integers: our composite is integer and byte-for-byte, and division by a small alpha is noisy exactly where the fringe is visible.
2. **Inside us there already lives a straight/premultiplied seam.** In the frame plan (`src/libs/kt-engine/src/video.rs`, `execute_plan_cpu`) a solo fade by CROSSFADE goes through `scale_alpha_into` — RGB is not touched, only alpha is multiplied (straight, and `alpha_over` multiplies it correctly). But a solo fade with ANY OTHER kind goes through `transitions::mix_into`, where the empty side is `Img::Clear` = `[0,0,0,0]`, so RGB is interpolated TOWARDS BLACK together with alpha (premultiplied), and `alpha_over` multiplies it a second time. On the hard edges of a wipe (alpha 0 or 255) there is no difference — and t225b does not catch it — but the soft band of the transition is darker than it should be with a straight composite. This is exactly the fringe the whole note is about, only our own. **Separately:** the preview loads the finished composite via `egui::ColorImage::from_rgba_unmultiplied` (`src/apps/kt-app/src/app.rs`, line 3168), and `ecolor` multiplies RGB by alpha once more — so on frames with A < 255 (any fade, any key on a lower layer) the display differs from the export, which simply drops alpha at RGBA→YUV420P (`src/libs/kt-ffmpeg/src/muxer_impl.rs`). Invariant 7 speaks of byte equality of the ASSEMBLED FRAME and is not violated, but the user sees the difference. Both places must be decided consciously before foreign alpha enters the composite.
3. **A sequence breaks the assumption "one media file = one path".** The model stores `MediaRef { id, path }`, the index DB identifies media by size/mtime/xxh3 of ONE file, `resolve_media_path` resolves one path. A sequence needs a template + range + rate + gap policy; that means a field in the model, a version of the `metadata.kt` envelope (currently v3), and an index invalidation rule that does not exist today.

### Which invariants it touches

- **4 (the cache is the single source of truth about coverage):** a picture has one source frame for any length of the event; a naive warm-up would ask for a frame per grid cell and inflate the cache for nothing.
- **5 (exactly one MediaWorker owns the decoder):** for a sequence the "decoder" is thousands of file openings; the priority queue and generation cancellation remain in force, but the cost of a request stops being "step through the GOP".
- **7 (the composition code is one):** the interpretation of alpha must be computed in `frame_plan`/the common code, not in two places; CPU and WGSL are corrected as a pair, otherwise t181/t182 diverge.
- **8 (scroll/zoom does not generate decode requests):** thumbs of a picture, and especially of a sequence, must come from the sidecar index; for a sequence the very setting of a "thumb strip" changes — it is N files, not N positions in one.
- **11 (determinism):** new goldens on material with alpha; the kt-fx rule "the default of a new parameter = the previous behaviour byte for byte" means that the default interpretation must give today's result for material WITHOUT alpha.
- UI rules **13** (the settings of duration and fps are two lines, not a panel), **17** (do not create a second way to put a picture beside the usual import), **20** (the five-second default must become a visible setting, and not "grow by itself").

## Estimate

| What | Answer |
|---|---|
| Scope | L — a whole round. Cheaper than it seems: a single picture and a sequence ALREADY play and export (measured), the composite is already straight-alpha, the `has_alpha` flag already travels to both backends. More expensive than "remove `has_alpha: false`": the interpretation of alpha must become a property of the media in the model, the format and the UI, otherwise switching alpha on will make the result worse than now |
| Where the main work is | kt-media (`ffmpeg.rs`: an honest `has_alpha` from `pix_fmt`; the source of a sequence; `index.rs`: what to index for a picture), kt-model (`MediaRef`: the kind of source, the interpretation of alpha, the template/rate/gaps of a sequence; `ProjectSettings`: the duration of a picture), kt-otio (reading and writing `ImageSequenceReference`, the envelope version), kt-engine + kt-gpu (unmatte in the common plan code + a mirror in `composite.wgsl`, as a pair), kt-app (importing a sequence by a gesture, settings, the alpha property on the bin card) |
| Project format | the envelope version `metadata.kt` v4 is needed (currently v3): fields for the kind of source, the interpretation of alpha, the template and rate of a sequence. Reading of files up to v3 — as in t212 |
| Tests to write | kt-media conformance on a PNG with alpha (straight reference) and on a sequence with a GAP in the numbering; a composite golden "graphics with alpha over video" on CPU + tolerance conformance of the GPU via `kt_gpu::assert_frames_close`; round-trip of the model and of `.otio` with `ImageSequenceReference` (including a foreign file not written by us); a test "the default interpretation changes not a single existing reference"; separately — a regression for the straight/premultiplied seam in a solo fade with a kind and for the preview display against the export |
| Depends on | nothing hard. Intersects with F-MSK-003 (track matte): Alpha matte is impossible while alpha does not come from the file — both notes edit one line `has_alpha: false`. Gives a cheap workaround for F-TXT-001 (titles) |
| Breaks compatibility | no, with a careful default: material without alpha must be computed byte for byte as it is today |

## Verdict

**We take it, priority A.** Not because "there are no pictures" — measurement showed that a single PNG and even a sequence by template already play and export for us. But because a PNG with transparency gives a WRONG result NOW: alpha is dropped silently, and the logo lies on the video as an opaque rectangle (measured: RGBA 255,0,0,128 → (253,0,0) on output). A wrong result on correct input is worse than a missing capability.

A separate and perhaps the main argument: **this is the cheapest way to close the need for "text on screen" before real titles appear.** We have no titles yet (F-TXT-001, priority A, cost L), no drawing, no generators. But a title made in any external editor and saved as a PNG with alpha is a ready lower third, which only has to be put on a track. One honest line `has_alpha` plus interpretation turns "we cannot write a word in the frame" into "we can, only the text is typed outside". This does not replace F-TXT-001, but it changes its urgency.

The order inside the round is proposed as follows: first alpha with interpretation (it also unblocks Alpha matte from F-MSK-003 and requires closing our own straight/premultiplied seam in solo fades and in preview loading), then the duration of a picture and the fps of a sequence as settings, and only then importing a sequence by a gesture and `ImageSequenceReference` in the format.

### WHAT WAS DONE (R.67)

The single picture and alpha are closed, following the owner's complaint "static pictures do not display at all.. there is an eternal Loading...":

* a still is recognised by the DEMUXER (`image2`, `*_pipe`), it has one frame at any tick, a "practically infinite" duration and silence in the sound; **no seek is made on a picture** — it is precisely that which left the stream without a single frame, and that was the cause of "Loading…";
* `has_alpha` is queried from the format descriptor: a PNG with transparency no longer lies down as an opaque rectangle (the very hole measured by this note in R.52);
* the fixtures `still.jpg` and `still_alpha.png` were added in `fixtures/gen.sh` — before R.67 the picture path was not covered by a single test.

REMAINS: a sequence by template (`shot_%04d.png`) as ONE media item with a duration by the number of files — it opens through the image2 demuxer, but is not an object of its own in the bin.

## Sources

Measurements on this machine (07.09.2026, system FFmpeg, built as
`src/target/debug/kt-cli`): `ffprobe` of a single PNG (`png_pipe`,
`duration=N/A`, `pix_fmt=rgba`), `ffprobe "shot_%04d.png"` (`image2`,
`avg_frame_rate=25/1`, `duration=0.200000`; with `shot_0003.png` removed —
`duration=0.080000`), `kt-cli export` of a project from one PNG and from a sequence with a
per-frame pixel check of the result. This is the primary source for all statements of "this is how it is with us now".

- https://help.magix-hub.com/video/vegas/en/content/topics/faq/faq_creatingpictureslideshow.htm — Vegas, "New still image length" in Options → Preferences → Editing (accessed 07.09.2026). The default value itself is not named in the help
- https://help.magix-hub.com/video/vegas/23/en/content/topics/3-projects/open.htm — Vegas, the "Open sequence" checkbox and "Each image in the sequence will be displayed for one frame" (accessed 07.09.2026). The page is for line 23; for 2026 it was not separately re-checked
- https://community.adobe.com/questions-729/image-sequence-export-issue-frame-rate-always-30fps-1392096 — Premiere, "Indeterminate Media Timebase" and the quote by Averdahl (accessed 07.09.2026)
- https://community.adobe.com/questions-506/image-sequence-to-video-at-24fps-ends-up-too-fast-995424 — Premiere/AME, "Interpret Footage → Assume this frame rate", David Arbor, 01.11.2021 (accessed 07.09.2026)
- https://community.adobe.com/questions-729/set-to-frame-size-vs-scale-to-frame-size-1423901 — Premiere, quote by Trent Happel (Adobe), 19.04.2017 (accessed 07.09.2026)
- https://community.adobe.com/announcements-532/new-in-beta-unmult-effect-about-time-right-314503 — AE 26, announcement of the Unmult effect, 22.10.2025 (accessed 07.09.2026)
- https://creativecow.net/forums/thread/alpha-track-matte-yields-edge-or-halo/ and https://creativecow.net/forums/thread/mattes-leaving-a-fine-black-line-around-the-edge/ — premultiplied/straight and the black fringe, 2007–2008 (accessed 07.09.2026). The threads are deliberately old: the problem is not new, and that is part of the argument
- https://support.apple.com/guide/final-cut-pro/add-still-images-verfc8a3df7/mac — FCP, "The default initial duration for still images is 4 seconds" (accessed 07.09.2026)
- DaVinci Resolve, Clip Attributes → Alpha Mode ("choose the type of alpha
  channel (Premultiplied or Straight), or invert the alpha channel") and
  Preferences → Editing → Standard still duration: the wording from the Resolve 18.6 manual in a documentation mirror; the official Resolve 21 PDF was not opened from this environment — considered verified against 18.6, not against 21 [NOT VERIFIED for 21]
- https://helpx.adobe.com/premiere/desktop/get-started/preferences-and-settings/timeline-preferences.html — mentioned as the place of the setting "Still Image Default Duration"; the page itself does not open from this environment, the content is taken from search results [NOT VERIFIED]

Unavailable from this environment (and therefore the Premiere/AE/FCP lines rely on the
official Adobe community and the Apple help, not on helpx): `helpx.adobe.com`,
`reddit.com`, `discussions.apple.com`, `web.archive.org`,
`forum.blackmagicdesign.com` (403 on direct `viewtopic.php`), the search engines
Mojeek/Ecosia/Brave/DuckDuckGo. The search that worked was Yahoo. The `WebSearch` budget in
the session was exhausted before the work began.
