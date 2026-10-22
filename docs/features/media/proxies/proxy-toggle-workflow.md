---
id: F-MED-002
title: Proxies — editing on light copies, rendering from originals
status: candidate
priority: B
cost: L
category: media/proxies
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-media, kt-ffmpeg, kt-cache, kt-engine, kt-otio, kt-app]
invariants: [4, 5, 9, 10, 11]
ui_rules: [12, 13, 16, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MED-002 — Proxies — editing on light copies, rendering from originals

## Essence

The editor drops an hour of 4K camera footage onto the timeline, and the preview stops keeping up: scrubbing stutters, the picture lags behind the sound, the fan howls. A proxy is when the program re-encodes the sources once into small light copies, links them to the originals and edits on them, and for the final render it returns to the originals by itself, without a reminder. The feature has one switch, and all its value lies in the fact that it can be forgotten to be turned off.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Create Video Proxy (RMB in Project Media) | the file `.sfvp0` is placed **next to the source, with the same name**, and it is NOT shown in Project Media — it is visible only in Explorer. There is no separate "proxy/original" toggle at all: the proxy is switched by the same control as preview quality — "When the preview quality is set to Draft or Preview, the proxy file is used. When the preview quality is set to Good or Best, the original file is used". For 4K, proxies are built automatically. Rendering in principle does not see the proxy |
| Adobe Premiere | 26.x | Proxies: Ingest → Create Proxies / Attach Proxies, the Toggle Proxies button in Program Monitor | proxies are created via Media Encoder by an ingest preset (the typical "Low Resolution Proxy" — 1024×540); the files are written to the folder chosen at import. Foreign proxies are attached with the Attach Proxies command, and Adobe requires the name `<original>_Proxy.<ext>` in a separate folder next to the masters. The export dialog has a **Use Proxies** checkbox (appeared in 14.3.2), and Adobe warns directly: "The "Use Proxies" checkbox is not affected by enabling or disabling proxies while editing" — the editing state and the render decision are NOT LINKED. The checkbox is saved into the export preset; the neighbouring "Use Previews" takes priority over it |
| DaVinci Resolve | 21 (mechanics checked against 18.x–20.x) | Proxy Media, Optimized Media, Render Cache — THREE different things | Optimized Media — an internal, non-portable and non-exportable cache of re-encoded material; Proxy Media — ordinary files (ProRes Proxy on macOS, DNxHR/H.264 on Windows; quarter/eighth resolution), which can be carried away and handed out; the folder is set by Working Folders → Proxy generation location, and the separate free application Blackmagic Proxy Generator builds them from a watch folder. In Render Settings on Deliver there is a "Use optimized media" switch — by default the render runs from the originals |
| Final Cut Pro | 12.3 | Optimized media / Proxy media, View → Media menu | a proxy is Apple ProRes 422 Proxy **or** H.264, "in a variety of frame sizes from 12.5 percent to 100 percent of the original", stored inside the library or in an external storage location. There are THREE modes: Optimized/Original, **Proxy Preferred** (if there is no proxy, it silently takes the original) and **Proxy Only**. The protection against rendering with proxies is a phrase in the help: "make sure to switch back to optimized/original media before sharing your project" |
| After Effects | 26.x | Proxy (Set Proxy → File), Placeholder, Comp Proxy | a proxy is set on a SPECIFIC project item or on a composition, and it is switched by a small square next to the item's name. A separate line in Render Settings — **Proxy Use**: `Use No Proxies` (the default in the Best Settings template) / `Use Comp Proxies Only` / `Use All Proxies` / `Current Settings`, that is, the decision is taken at the level of the render job, not the project |

## The trick behind it

**1. Protection against "rendered with the proxy" is not a checkbox but a DIFFERENT
DESIGN, and the spread between editors is maximal.** VEGAS does not give such an
option at all: the help says "Video proxy files are not used
for rendering your project", and there is no "proxy/original" toggle there — the proxy
is enabled by the same control as preview quality (Draft/Preview —
proxy, Good/Best — original). That is, Vegas FUSED "resolution share" and
"proxy" into one control and thereby closed the question. Premiere went
exactly the other way and did it explicitly: the Use Proxies checkbox lives in the
export dialog, and Adobe in its announcement underlines that it "is not affected
by enabling or disabling proxies while editing" — that is, for Premiere the editing
state and the render decision are TWO DIFFERENT switches, not linked to each other, and moreover saved into the export preset. This is where the
threads "Premiere is exporting my proxies" come from. FCP holds the middle — "Proxy
Preferred" itself substitutes the original when there is no proxy, while "Proxy Only"
honestly gives a render from the proxy, and Apple's only protection is a suggestion
in the help. The same question, three incompatible answers.

**2. A proxy must match by FRAME RATE, NAME and SOUND — and the resolution is exactly what may differ.** Adobe requires a foreign proxy to be named `<original>_Proxy.<ext>` and to lie in a separate folder next to
the masters; Larry Jordan adds that the frame rate of the proxy must match the master. The least obvious thing concerns sound: if the master has more than
two audio channels (even empty ones), the proxy cannot be made in H.264,
because Premiere writes H.264 only into MP4, and MP4 carries two channels —
ProRes Proxy or DNxHR LB remain. That is, "I'll make a proxy with ffmpeg in
h264" breaks not on the video but on the audio, and breaks silently.

**3. A proxy of a different resolution changes not the speed but the PICTURE.** An effect whose parameter is set in pixels is computed in the pixels of the CURRENT frame: a blur with a radius of 20 px on a 1/4 proxy covers four times the share
of the image that the same blur covers on the original. The trap is not hypothetical — people step on it even without proxies, simply by working at a reduced preview
resolution. And it applies to us directly: we already have a preview resolution
share of 1/16…1/1, and EXACTLY ONE effect is scaled by it — the three-dimensional one
(`Cam3d.world_scale`); `Blur`, `Glow`, `Edges`, `Noise`, `Vhs` with the unit
`Px` are computed in the pixels of the frame as they are.

## Pros

- The saving goes to DECODING — the only expense item that neither the result cache nor the preview resolution share reduces: those work only after the frame has already been decoded.
- Swapping the source does not touch the edit. A clip addresses material by time and media identifier, not by file, so the toggle is one operation on the media, not a rewrite of forty events.
- Proxies are files on disk: they survive a restart of the program, they can be made once for the whole studio and distributed over the network, and proxies from the location (camera proxies) can be attached as they are. A warm cache dies together with the process, and the resolution share is not handed over anywhere at all.
- Proxies are set per file: heavy material is proxied, light material stays as it is — "optimising" the whole project is not required.
- Resolve shows what is paid for refusing files: Optimized Media starts up faster, but it "cannot be exported, and is not user accessible" — that is, you cannot share it with a colleague or take it to another machine.

## Cons

- Exactly one open file stands behind one media item at any moment, so switching the toggle zeroes everything warm for that source: the original and the proxy cannot lie in the cache at the same time without splitting the truth about warmth in two.
- A proxy is a second set of files: disk space, the need to recreate them when the original is edited, the question "where are my proxies?" when a project is moved, and a separate procedure of relinking across hundreds of clips.
- An effect with a parameter in pixels gives a different picture on a proxy. This is not an implementation defect but a consequence of the effect not knowing at what scale it lives.
- Proxies are encoded lossy and usually in 8-bit 4:2:0, so everything that is judged by fine gradations — banding, noise, the edge of a key, the result of a grade — cannot be judged on a proxy. Proxies speed up editing but not colour correction: there one needs a full intermediate (Optimized Media in Resolve, ProRes 422 in FCP), and that is no longer a "light copy".
- Generation is a full "decode + encode" pass over each source: on an hour of 4K that is tens of minutes, which have to be shown somewhere, allowed to be cancelled, and survive the closing of the dialog.
- The matching requirements (name, frame rate, number of audio channels) are nowhere checked in advance: you learn about them from a silent refusal of the link or from a desynchronised sync.

## What users say

Exporting with proxies happens even to people who know what they are doing:

> HELP! Premiere is exporting my proxies instead of the source file
> — thread title by markalext, Adobe Community, https://community.adobe.com/questions-729/help-premiere-is-exporting-my-proxies-instead-of-the-source-file-1390164

> If You used the proxies in the timeline, that is not the way to go.
> — Ann Bens (Community Expert), Adobe Community, ibid.

The expert's answer in the same thread explains why there is no way back: "you
will have to replace all proxy files on the timeline with the original to
get it exported with the original files" — a proxy that got into the edit as an ordinary clip stops being a proxy and becomes material.

Reduced resolution changes the picture, and this is found out at the end of the project:

> I typically preview at half resolution to use less CPU/RAM, and unfortunately, I just finished an entire project only to realize that when I render or preview at the full 1080p resolution, any text layers that have layer styles enabled are rendering completely wrong.
> — James Inniani, Creative COW, 15.10.2018, https://creativecow.net/forums/thread/ae-half-resolution-preview-not-accurate/

> Be careful working at a resolution other than 'full', as many things like plugins, not just styles, can render incorrectly or in unexpected ways.
> — Andrew Somers, same thread, 15.10.2018

Attaching someone else's proxies is a separate story; Adobe's requirements were voiced by its own employee, and they concern NAMES, not the picture:

> The Proxies must have names matching the Full Res/original with '_Proxy' appended to the name. e.g.: Full Res = scene01.mxf, Proxy = scene01_Proxy.mov
> — Peter Garaway (Adobe), Creative COW, 24.10.2016, https://creativecow.net/forums/thread/proxies-do-not-automatically-attach-to-original-fo/

In the same thread — the price of an error on a large project: the post's author works with "over 500 clips", and another participant (02.02.2021) writes about RED proxies: "These can only be attached one at a time – trying to do so as a batch
usually causes the programme to crash".

Users work out between themselves where the proxies physically lie:

> If you look in the folder of your source media the proxy files will have the same name but the extension will be '*.sfvp0'.
> — George Dean, Creative COW, 07.02.2019, https://creativecow.net/forums/thread/problems-withquestions-about-video-proxies/

The difference "proxy versus optimised media" in Resolve is explained on the BMD forum with a quote from the installer's own documentation:

> Optimized Media is managed internally by DaVinci Resolve, cannot be exported, and is not user accessible. In contrast, Proxy Media creates fully portable and independent media that can be easily managed by the user.
> — Singularity, forum.blackmagicdesign.com, 12.11.2020, https://forum.blackmagicdesign.com/viewtopic.php?p=686710

What could NOT be found: a live quote about a proxy that fell out of sync with the original in duration or timecode ("proxies out of sync"). I searched `creativecow.net`, `community.adobe.com` and through search queries like "proxy different duration / out of sync / timecode mismatch" — the results go either to reddit (unavailable from this environment) or to video tutorials, from which there is nothing to quote. The requirement of matching frame rate is confirmed by the primary source (Larry Jordan); the duration mismatch itself is so far only documented.

## How it fits Kinotrain

### What already exists

**Three savings, of which we have two.** Proxies substitute the SOURCE, the cache keeps the RESULT, the resolution share reduces the WORK. Ours are the two:

- *Preview resolution share* 1/16 … 1/1 — `draw_preview_controls` in `src/apps/kt-app/src/app.rs` (the array `fracs`, `q_for`, `Player::set_preview_quality`). The share lives "on top of" the project resolution: when the resolution changes, the share is kept, not the pixels. There is no automatic lowering under load — the choice always belongs to the person.
- *Cache* — `src/libs/kt-cache/src/lib.rs`: the pre-FX `BurstKind::Video{quality}` and the post-FX memoisation `PostFxVideo{quality, fx_hash}` (idx = timeline tick). `quality` is part of the key: records of different qualities are not interchangeable, and coverage is counted separately for each quality (invariant No. 4).

Also an answer to the question "are proxies the same axis as `quality`?": **no**. `quality` is a request TO THE DECODER for the output size within ONE source. A proxy is a different file behind the same `MediaId`. These are different axes, and they must not be mixed in the cache key (see "What gets in the way").

**The saving of the resolution share does NOT touch decoding.** `FfmpegSource` decodes the frame entirely through `avcodec`, and `Quality` is applied afterwards, in `sws_scale` (`src/libs/kt-ffmpeg/src/source_impl.rs`). So 1/16 saves swscale, the FX chain, the composite and the bytes of the cache — but a 4K H.265 is still decoded in full, frame by frame. It is precisely this expense item that ONLY proxies close, and that is the main argument for them.

**The switch machinery is already written and covered by tests.** `Player::replace_media` (`src/libs/kt-engine/src/player.rs`) does exactly what the "proxy/original" button needs, and in the right order: it stops the `MediaWorker` → `cache.invalidate_source` → starts a new worker → bumps `project_version` (otherwise prefetch at idle would not notice the change) → wakes the loop. Transport, position, FX state and the cache of the other sources are not touched. The background part is `KtApp::reopen_rx` and the thread `reopen-media` in `src/apps/kt-app/src/app.rs`. Tests t209 (placeholder), t210 (render with placeholders), t211 (a returning file replaces the placeholder) — this is the same scenario "a different file stood up behind `MediaId`", only the cause is different. This is a strong argument of "cheap": the most nervous part of the feature — substituting the source on the fly without losing position or FX tails — is already debugged.

**The sidecar mechanism "file next to the project" is worked out.** `<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`, `docs/media-index.md`) already stores probe, packet index, peak pyramid and thumbs; the identity of a file is checked by `size + mtime + xxhash`, and ONLY the affected media is rebuilt. The table binding "media → proxy file" is added there additively, without a bump of `SCHEMA_VERSION` — `index.rs` has a ready technique for this (`CREATE TABLE IF NOT EXISTS` on every opening; this is how the `poster` table was set up).

**The encoder exists.** `kt-ffmpeg::Muxer` (`src/libs/kt-ffmpeg/src/muxer_impl.rs`) accepts RGBA frames and f32 audio and writes MP4 h264+AAC through the system FFmpeg with automatic choice of a hardware encoder (invariants No. 9, No. 10). Generating a proxy is NOT an export of the timeline: it is a separate small loop "read `MediaSource` at a reduced `Quality` → write to `Muxer`".

**In the project format the place for proxies ALREADY exists — and it is not ours.** OTIO keeps for a clip not one reference to the material but a map: `Clip.media_references: IndexMap<String, Option<MediaRefNode>>` + `active_media_reference_key` (`src/libs/kt-otio/src/schema.rs`), `DEFAULT_MEDIA_KEY = "DEFAULT_MEDIA"`, plus the written upgrade Clip.1 → Clip.2. We read only the active reference, but we carry the WHOLE map as passthrough (`ClipExtra::media_refs` in `src/libs/kt-otio/src/convert.rs`) — that is, a project that came from a foreign tool with a key of the form `proxy` is already not lost with us; we simply do not know how to switch to it.

### What is missing

1. In OUR model a media item has one path: `MediaRef { id, path }` (`src/libs/kt-model/src/project.rs`). There is no second path and no sign of "the proxy is now active".
2. There is no transcoder "file → file". `export_mp4_with_backends` (`src/libs/kt-engine/src/export.rs`) renders a COMPOSITION and receives the sources from outside; for proxies a different, much simpler loop is needed.
3. There is no queue of background jobs with progress. A proxy for an hour of material is tens of minutes of work, which must run in the background and survive the closing of the render window.
4. There is not one place in the UI where a SEPARATE media item has properties and actions: the bin has cards and a context menu for GROUPS (`src/apps/kt-app/src/bin_panel.rs`), media has none.
5. There is no "attach proxies": there is nowhere and nothing to attach a foreign (camera) proxy and check that it fits.

### What gets in the way

**The binding point is `SourceId`, and it equals `MediaId` one to one** (`SourceId(clip.media.0)` in `src/libs/kt-engine/src/video.rs`). This is good: a proxy naturally stands "behind the same `MediaId`", and the whole engine — the frame plan, prefetch, coverage bars, the keys of post-FX — notices nothing; `kt-ffmpeg` is already locked behind the `MediaSource` trait (`docs/architecture.md`). But the price is that the original and the proxy CANNOT lie in the cache at the same time: `invalidate_source` removes all records of the source, and every toggle of the switch zeroes the warmth. Introducing the proxy as a third axis of the cache key (next to `quality`) is not allowed: it would double the coverage and break the wording of invariant No. 4, "the cache is the single source of truth about warmth" — there would be two truths, and the bars would have to choose which one to show.

**A proxy must match the original in DURATION, and we already have a rake lying here.** The frame key in the cache is time, not the frame number: `video_cache_idx(source_pts) = source_pts.0` (`src/libs/kt-engine/src/video.rs`). Therefore a proxy with a different fps is not frightening to us — we address material by ticks, not by frames. What is frightening is a proxy of a different DURATION, and our own muxer does exactly that: `MuxConfig.fps` is an integer, `time_base = 1/fps` (`src/libs/kt-ffmpeg/src/muxer_impl.rs`), and fractional rates are snapped to the integer grid (`grid_fps(23.976…) = 24`, t178 in `src/libs/kt-otio/tests/convert.rs`). A proxy of an hour of 23.976 fps material, written by our muxer at 24 fps, will come out shorter than the original by almost four seconds — and the tail of the clip will drift. The very first implementation must either teach the muxer fractional rates or guarantee the duration some other way. Incidentally: our MP4 carries stereo (invariant No. 1), so Premiere's restriction "an h264 proxy is not suitable for multichannel audio" is structurally the same for us — we simply do not know multichannel audio anyway.

**Export with proxies is prevented structurally, not by a checkbox.** The render sources are NOT opened by the engine: `render_dialog::open_all` (`src/apps/kt-app/src/render_dialog.rs`) and the function of the same name in `src/apps/kt-cli/src/main.rs` go by `MediaRef::path` from the model and pass the ready list to `export_mp4_with_backends`. While the proxy substitution lives ONLY in the player path (`Player::replace_media`), export physically does not see the proxy file — this is the construction of VEGAS, not of Premiere, and it is correct. But it must not be "remembered" — it must be locked in: a test that turns on proxies, renders, and compares against the reference made from originals is cheap and directly protects invariant No. 11.

**Pixel parameters of FX already lie on the resolution share, and a proxy will make this worse.** In `params_for!` (`src/libs/kt-model/src/project.rs`) the unit `Px` is carried by `Blur.radius/radius_y`, `Edges.thickness`, `Noise.size`, `Vhs.rgb_shift/scan_step`, `Glow.radius`. Exactly ONE of them is scaled by the resolution share: the 3D one — `Cam3d.world_scale = quality.width / project.width` (`src/libs/kt-engine/src/video.rs`, `src/libs/kt-engine/src/video_plan.rs`). The blur, however, is implemented as a downscale with a step `(radius/3).max(1)+1` in the pixels of the CURRENT frame (`src/libs/kt-fx/src/video.rs`), so on a 1/4 preview a radius of 8 gives a blur four times coarser relative to the picture than in the export. Invariant No. 7 is formally intact (preview == export byte for byte is checked at ONE quality), but in the editor's eyes this is exactly the trouble that users complain about with other people's proxies. The rule of the kt-fx crate ("the default of a new parameter = the previous behaviour byte for byte") forbids fixing this silently: scaling of the `Px` parameters would shift the goldens, and the decision about it is a separate one, not an incidental one.

### Which invariants it touches

- **No. 4** (the cache is the single source of truth about coverage): the switch of proxies goes only through `invalidate_source`; a separate axis is not to be introduced into the key.
- **No. 5** (exactly one MediaWorker owns the decoder): the substitution is done by replacing the worker, as in `replace_media`, and not by a second worker on the same `MediaId`.
- **No. 9 / No. 10** (export is in-process, the binary is not linked with libav*): the proxy generator must go through `kt-ffmpeg::Muxer`, and not call an external `ffmpeg`, although the temptation is great — this is the simplest part of the work, the easiest to do wrongly.
- **No. 11** (determinism): export must go from the originals. The only item for which the feature requires a TEST, not discipline.
- **No. 7** is not violated: a proxy changes the bytes of the SOURCE, not the path of the composition.
- UI rules of `src/apps/kt-app/CLAUDE.md`: **20** ("nothing grows or gets cut off by itself" — proxies are not switched on by themselves nor switched off silently), **13** (the toggle must sit in space already occupied — for example, in the bin card, and not in a new strip), **16** (there is no proxy for this media — this is an ERROR of the gesture, not a faded hint), **12** (generation is background work: not during a gesture and not during playback).

## Estimate

| What | Answer |
|---|---|
| Scope | L — a whole round: the transcoder, a background queue with progress, a second path for media, the toggle, protection of export |
| Where the main work is | `kt-media`/`kt-ffmpeg` (the "file → file" loop on top of `Muxer`), `kt-app` (queue, progress, the toggle and the sign in the bin), `kt-model` (a second path and an active choice for media), `kt-otio` (mapping into the standard `media_references`) |
| Project format | does NOT require its own version: the place is standard — OTIO's `media_references` + `active_media_reference_key`, which we already parse and carry as passthrough. A subtlety: in OTIO the reference map hangs on the CLIP, while with us media lives in the project once — on writing, the reference will be multiplied across all clips of the media, and on reading back this must be collapsed |
| Tests to write | export with proxies enabled matches the reference made from originals (protection No. 11); the duration of a proxy equals the duration of the original to the frame; switching back and forth does not move the playhead and does not break the FX state (an extension of t211); round-trip of `media_references` with a non-empty non-default key; the additive binding table in `ktindex.db` does not bump `SCHEMA_VERSION` |
| Depends on | nothing blocks it; adjacent — F-SPD-004 (timecode and sync), F-CLR-005 (scopes: they must on principle not be looked at on a proxy) |
| Breaks compatibility | no |

## Verdict

We take it, but not before there is material on which this shows: the preview resolution share covers everything except decoding, and while projects do not run up against decoding 4K/H.265, proxies save other people's pain, not ours. The argument "for" is not performance as such, but the fact that half of the feature is already written and tested (`Player::replace_media`, the sidecar DB, `Muxer`, `media_references` in OTIO), and that protection against rendering with proxies comes out for us by VEGAS's construction — export opens its own sources by `MediaRef::path` and does not see the proxy file at all. The work must be done in this order: first the test "render goes from the originals", then everything else.

## Sources

- VEGAS Pro 23 Help, "Creating proxy files for high-definition editing":
  https://help.magix-hub.com/video/vegas/23/en/content/topics/5-preview/creating_intermediate_files.htm
  (accessed 07.09.2026) — `.sfvp0` next to the source, binding to Preview Quality, "Video proxy files are not used for rendering your
  project". The page refers to line 23; in 2026.0 the mechanics were not checked live — **[NOT VERIFIED]** that it has not changed.
- Final Cut Pro User Guide, "Create optimized and proxy files":
  https://support.apple.com/guide/final-cut-pro/create-optimized-and-proxy-files-verb8e5f6fd/mac
  (07.09.2026) — ProRes 422 Proxy or H.264, "frame sizes from 12.5
  percent to 100 percent of the original", storage in the library or in an
  external storage location.
- Final Cut Pro User Guide, "Control playback quality":
  https://support.apple.com/guide/final-cut-pro/ver2fd7a8b94/mac
  (07.09.2026) — definitions of Optimized/Original, Proxy Preferred, Proxy
  Only and the warning about switching before Share.
- Final Cut Pro User Guide, "Create a proxy-only project":
  https://support.apple.com/guide/final-cut-pro/create-a-proxy-only-project-vere88b1ee9f/mac
  (07.09.2026).
- forum.blackmagicdesign.com, thread "Proxy media vs. Optimized Media",
  12.11.2020: https://forum.blackmagicdesign.com/viewtopic.php?p=686710
  (07.09.2026; the page opens only through the reader `r.jina.ai`) —
  a quote from the Resolve installer documentation.
- Adobe Community, "HELP! Premiere is exporting my proxies instead of the
  source file":
  https://community.adobe.com/questions-729/help-premiere-is-exporting-my-proxies-instead-of-the-source-file-1390164
  (07.09.2026) — the title and the answers of Ann Bens. The post dates on the page
  do not match the version 23.3.1 mentioned in them, so the post date is not
  quoted here — **[NOT VERIFIED]**.
- Creative COW, "Proxies do not automatically attach to original
  footage", 23.10.2016 – 02.02.2021:
  https://creativecow.net/forums/thread/proxies-do-not-automatically-attach-to-original-fo/
  (07.09.2026) — Adobe's naming requirements (`_Proxy`), the problems of batch attaching RED proxies.
- Creative COW, "Problems With/Questions About Video Proxies", 07.02.2019:
  https://creativecow.net/forums/thread/problems-withquestions-about-video-proxies/
  (07.09.2026) — `.sfvp0` in the source folder.
- Creative COW, "AE half resolution preview not accurate", 15.10.2018:
  https://creativecow.net/forums/thread/ae-half-resolution-preview-not-accurate/
  (07.09.2026) — effects at reduced resolution.
- Larry Jordan, "Linking media to proxies created outside Premiere Pro",
  24.07.2021:
  https://larryjordan.com/articles/linking-media-to-proxies-created-outside-premiere-pro/
  (07.09.2026) — matching of the frame rate, the MP4 limit of two
  audio channels, the choice of ProRes Proxy / DNxHR LB.
- OpenTimelineIO Python API, `opentimelineio.schema.Clip`:
  https://opentimelineio.readthedocs.io/en/latest/api/python/opentimelineio.schema.html
  (07.09.2026) — `media_references`, `active_media_reference_key`,
  `DEFAULT_MEDIA_KEY = 'DEFAULT_MEDIA'`. The specification does NOT explain the purpose of multiple references: the reading "this is the place for a proxy" is
  **[NOT VERIFIED]**; neither `otio-file-format-specification` nor `versioning-schemas` says anything about them.
- Adobe Community (announcement), "New option to use proxy media when
  exporting":
  https://community.adobe.com/announcements-732/discuss-new-option-to-use-proxy-media-when-exporting-313379
  (07.09.2026) — the appearance of the checkbox in 14.3.2, its independence from the editing toggle, the priority of "Use Previews", saving into the preset.
- The Premiere lines (preset 1024×540, proxy folder) and Resolve 21 (codecs
  and the place of proxy generation, the checkbox
  "Use optimized media" in Render Settings) were assembled from secondary
  sources and search snippets: `helpx.adobe.com` does not open from this environment, and the mirror of the Resolve manual on `steakunderwater.com` returned
  404 for the needed pages. Both lines are marked as only partly checked against documentation — **[NOT VERIFIED]** in the part concerning the exact UI wording.
- Kinotrain code: `src/libs/kt-cache/src/lib.rs`,
  `src/libs/kt-media/src/{lib.rs,index.rs}`,
  `src/libs/kt-ffmpeg/src/{lib.rs,muxer_impl.rs,source_impl.rs}`,
  `src/libs/kt-engine/src/{player.rs,video.rs,video_plan.rs,export.rs}`,
  `src/libs/kt-model/src/project.rs`,
  `src/libs/kt-otio/src/{schema.rs,convert.rs,lib.rs}`,
  `src/apps/kt-app/src/{app.rs,render_dialog.rs,util.rs,bin_panel.rs}`,
  `src/apps/kt-cli/src/main.rs`, `docs/architecture.md`,
  `docs/media-index.md` (read 07.09.2026).
