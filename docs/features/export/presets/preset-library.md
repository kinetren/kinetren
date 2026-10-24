---
id: F-EXP-001
title: Library of export presets
status: candidate
priority: B
cost: M
category: export/presets
editors: [premiere-26, vegas-pro-2026, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-app, kt-engine, kt-ffmpeg, kt-cli]
invariants: [7, 9, 10, 11]
ui_rules: [4, 13, 16, 19, 20]
round: null
point: null
tests: []
depends_on: [F-AUT-004, F-TXT-002]
checked: 2026-09-07
---

# F-EXP-001 — Library of export presets

## Essence

The same edit goes to four places: a master for the client, a small preview for the messenger, a long version for YouTube, and a vertical cut for stories. Each time it is the same fifteen or so decisions: container, codec, bitrate, resolution, frame rate, sound, file name. A preset gives the set a NAME, survives closing the project and moves to a second machine. The question of the note is not "can the checkboxes be saved", but **where the boundary of a preset lies**: what it carries itself, and what it silently takes from the project. Precisely on this boundary "in the preview it was not like that" and "where did the black bars come from" are born.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Premiere | 26.x | Export preset `.epr` + Export Preset Manager | a preset is a separate FILE, which is imported and shared; it lives not in Premiere but in the Media Encoder profile (`Documents/Adobe/Adobe Media Encoder/<version>/Presets`), and only AME can "export" it outward, a separate application with its own queue (the storyline of F-EXP-002). A major update regularly leaves the user without a library. There is "Match Source", a preset that takes parameters from the sequence. The Preset Manager appeared in 22.3.1 (2022) [some details NOT VERIFIED: `helpx.adobe.com` does not open from the environment] |
| VEGAS Pro | 2026 | Render template in the "Render As" window | templates are grouped by FORMAT (its own list for MAGIX AVC/AAC, its own for Sony MXF, its own for the third-party Voukoder); a user template is created by the "Customize Template" button there. In 2026 customizable colour information was added to the render template for ProRes exports. The dialog does not remember the selected group between launches (quote below) |
| DaVinci Resolve | 21 | Deliver: render presets + queue + Quick Export | "The deliver page gives you total control over all encoding options and formats, along with a render queue for exporting multiple jobs!" — a PRESET and a queue JOB are different entities here. The rules for fitting into a preset are not included: they live in the PROJECT settings in two separate stages, Input Scaling and Output Scaling, with options like "scale entire image to fit". In 21 MainConcept for H.265 and MV-HEVC 4:2:0/4:2:2 appeared in the render settings |
| Final Cut Pro | 12 | Destination (Settings → Destinations) | a destination = Format + Video Codec + Resolution + Audio Format + checkboxes (segmentation, chapter markers) + **Action** ("Save only", open in QuickTime/Compressor, put into Photos). They are created by dragging a template into the list and by the "Duplicate" command, are put into a **Bundle** ("New Bundle from Selection"), and live in the application settings. The colour space is deliberately NOT part of a destination. A destination does not parametrise the output file name |
| After Effects | 26 | TWO independent lists: Render Settings templates and Output Module templates | what to render (quality, resolution, proxy, range) and where to write (format, codec, Stretch, Crop, audio) are different templates, chosen by two different drop-down lists in the render queue [NOT VERIFIED against the primary source: `helpx.adobe.com` is unavailable] |

## The trick behind it

**1. The fit rule (stretch / fit with borders / crop) lives in the preset NOWHERE — and that is precisely why "where do the black bars come from" remains an eternal question.** Resolve moved it to the PROJECT settings and split it into two independent stages: Input Scaling (what to do with a clip on the timeline that differs in size) and Output Scaling (what to do with a timeline that does not match the output), each with its own set of options like "scale entire image to fit". The Deliver preset knows nothing about them, so one and the same preset in two projects gives different geometry, and the complaint "I set 2160×3840 everywhere, the source is the same 2160×3840, but there are still bars at the top" (quote below) exists precisely because the third setting sits in another window. In Premiere the same logic is lowered even further, to the CLIP ("Set to Frame Size" against "Scale to Frame Size"), and a person who came from there formulates their claim against Resolve as the absence of match-source in two places at once: in the timeline and in the export. The same fork repeats with colour, and there it is visible from the documentation: the FCP help writes about the destination's Color Space that it is "determined by the project settings and the destination
video codec", that is, the destination does not own the colour BY DESIGN; VEGAS Pro
2026 in the same year did exactly the opposite — the release notes of build 66
(27.03.2026) list "Customizable Color Information now available for render
template in ProRes exports". One year, two opposite answers to one
question. Hence the general conclusion: the word "preset" at all five means an agreement of
DIFFERENT width, and presets are not portable not because the file formats differ, but
because behind the boundary of each one lies something of its own.

**2. A preset fixes the author's HARDWARE, not the output, and it breaks from things that are not in it.** A template with a hardware encoder is not "the same thing, but faster", but a binding to a specific machine: "Are you trying to render using a template with
NVENC option and a graphics driver later than the 581 series? If so it will
fail" (quote below) — a CHANGE OF DRIVER VERSION broke the template, with the preset, the project and the program unchanged. The same class — numbers: a bitrate in a template is a REQUEST to the encoder, not a property of the file (320 kbps in the template → 125 kbps in the file, and "There is no switch to change between variable and constant bitrate"). Hence the truth about presets "for a platform": upload requirements are formulated in terms of the finished file, the preset in terms of the request to the codec, and the platform re-encodes everything anyway ("The file I uploaded is almost 6GB, the file downloaded
from YouTube is only 138MB"). The value of such a preset lies in the file being ACCEPTED, and it is named after the platform and therefore promises quality.

**3. The output file name does not belong to the preset, although all the information for it is in the preset.** In FCP one can assemble a Bundle of four destinations and get four files with one click — but there is nothing to tell them apart by name: to the direct question "how to append the codec or the setting name as a suffix" the author of the profile guide answers "FCP does not support automatically appending the codec to the
exported file name". Four files with the same name in four folders — this is exactly "a preset that knows everything about itself except how to be named". For us, by the way, the situation is mirrored, and therefore amusing: a name template (`kt-<project>-<region>-<length>s.mp4`) exists and works, but there is no preset whose name could be substituted there.

## Pros

- A preset as a separate FILE serves two applications at once: interactive
  export and the queue read the same `.epr`, so "in the queue the result came out different
  from the editor" cannot happen because of a divergence of settings.
- The action after export inside a destination (FCP Action) removes the manual step
  entirely: "render and open in Compressor" is not two human actions, but one
  property of the preset, and it is not forgotten.
- A Bundle turns the output into a LIST: a platform was added — a line was added, and all
  subsequent releases go out as a full set without "and did you make the vertical one?".
- Restricting the choice by the source ("Available choices are based on the source clip
  or project media format", FCP) catches codec incompatibility BEFORE the render, not at the end of a three-hour calculation.
- The separation of "preset" and "queue job" (Resolve Deliver) allows one
  preset to serve ten jobs with different ranges and names — something that
  we currently cannot express at all.

## Cons

- A preset fixes the request, not the result: the bitrate from a template is not guaranteed in the file, and a VBR/CBR switch
  may not exist at all — then there is nothing to fix the discrepancy with.
- The split "what is in the preset and what is in the project" is shown nowhere. The user sees two lists of
  settings and does not see a third — a list of what the preset silently
  takes from outside. All complaints that "the export does not look like the preview" live at this junction.
- A template tied to a FORMAT (VEGAS) forces the same meaning to be entered into several lists: "1080p for the client" exists separately under AVC,
  separately under HEVC, separately under ProRes, and there is nothing to synchronise them with; moreover,
  the dialog does not remember which group you were in last time.
- A library that lives in the profile of the application VERSION does not survive an update: presets
  disappear on moving to a new major version, and rescued files are not always
  imported back ("The selected preset is not a valid MPEG2
  preset. No exporter found for this preset"). The same class of problem for another
  type of preset is analysed in **F-AUT-004**.
- A preset with a nailed hardware encoder describes the author's machine: a change of
  driver breaks it without a single change in the preset itself.
- A batch export from a bundle is all or nothing: one preset cannot be picked from the bundle,
  and the failure of one branch brings down the whole result.
- Platform presets age faster than the program: the requirements of services change without
  a new release of the editor, while the preset name continues to promise up-to-dateness.

## What users say

> Are you trying to render using a template with NVENC option and a graphics
> driver later than the 581 series? If so it will fail.
> — andyrpsmith, Boris FX / VEGAS Pro forum, 18.04.2026,
> https://forum.borisfx.com/t/render-problem/22547

> Hello! I would like to know how this is possible. So, I have a render template
> that has 320 kbps but the rendered video is significantly lower with 125 kbps.
> — Hadrian Embalsado (newecreator), Boris FX / VEGAS Pro forum, 01.04.2026,
> https://forum.borisfx.com/t/rendered-audio-is-of-significantly-lower-bitrate-than-the-bitrate-of-the-render-template/21593

> It's a variable bitrate encode and you need to use media info to see the true
> maximum bitrate used by the encoder.
> — VegasFlorian (moderator), there, 02.04.2026,
> https://forum.borisfx.com/t/rendered-audio-is-of-significantly-lower-bitrate-than-the-bitrate-of-the-render-template/21593

> Project and timeline resolution set to 2160×3840 (Use vertical video box
> ticked). Output scaling set to the same as timeline. However, i'm still
> getting black bars at top of image in the record viewer.
> — Sean Lennon, Creative COW (DaVinci Resolve), 20.01.2026,
> https://creativecow.net/forums/thread/black-bars-on-export-issue/

> I'm used to Premiere where I can a) match the timeline settings to the original
> footage and b) match the export settings to the original footage. But
> apparently you can do neither in Resolve.
> — Tim Gallaher, Creative COW (DaVinci Resolve), 11.02.2026,
> https://creativecow.net/forums/thread/matching-settings-of-original-dv-prores-422-footage-in-exports-for-resolve/

> I'd love to have the codec or name of the export setting as a suffix, but can't
> figure out how to add it. Maybe this has been removed?
> — Patrick, comment on larryjordan.com, 30.06.2023 (Larry Jordan's answer the same
> day: "FCP does not support automatically appending the codec to the
> exported file name"),
> https://larryjordan.com/articles/fcp-x-export-to-multiple-destinations-at-once/

> I just upgraded from CS6 to CC. All of my export setting presets were wiped out
> with the install.
> — Dave Fleming, Creative COW (Premiere Pro), 02.05.2014,
> https://creativecow.net/forums/thread/export-setting-presets/

> what matters most to me is getting what I see when I render and upload to
> YouTube. I've only been able to achieve that in the past by setting Vegas
> projects to full-range and rendering with Vegas default ltd-range.
> — Howard Vigorita, Boris FX / VEGAS Pro forum, 08.04.2026,
> https://forum.borisfx.com/t/vp2026b066-color-differences-issue-in-8-bit-full-pixel-range/22064

> The file I uploaded is almost 6GB, the file downloaded from YouTube is only
> 138MB.
> — Chippe01, Boris FX / VEGAS Pro forum, 31.08.2026,
> https://forum.borisfx.com/t/version-2026-0-3-b-189-oh-soooo-close/25227

## How it fits Kinotrain

### What already exists

**The render settings dialog — `src/apps/kt-app/src/render_dialog.rs`,
`RenderSetup` (line 46) and `draw_render_setup()` (line 267). The full
list of what can be chosen at all — six rows of the grid:**

1. **Video codec** — a combo: "Auto (hardware first)" or a concrete encoder from
   `kt_media::muxer::available_encoders()`.
2. **Quality** — a switch CRF / Bitrate: a CRF slider 14…32 (default 23)
   or a logarithmic bitrate slider 0.5…80 Mbit/s (default 8 Mbit/s).
3. **Audio bitrate** — a combo 128 / 192 / 256 / 320 kbps (default 192).
4. **Resolution** — "Project (W×H)" or 1280×720 / 1920×1080 / 2560×1440 /
   3840×2160 / "Max (W×H)" over the sources.
5. **Audio** — not a choice but a LABEL: `AAC, 48 kHz stereo`.
6. **Container** — also a label: `MP4 (+faststart)`.

Plus a warning about unavailable sources (item 208) and
the "Render…" / "Cancel" buttons. The range is not shown in the window at all: `launch_render`
(line 180) silently takes `project.region`, and if there is no region — the whole project.

**What reaches the engine** — `ExportOptions`
(`src/libs/kt-engine/src/export.rs:337`): exactly five fields `quality`,
`video_bitrate`, `audio_bitrate`, `force_encoder`, `resolution`. From there to
`MuxConfig` (`src/libs/kt-ffmpeg/src/lib.rs:67`) — the same five plus `width`,
`height`, `fps`.

**The muxer supports exactly one combination.** `src/libs/kt-ffmpeg/src/muxer_impl.rs`:
the container is hard-coded as the string `"mp4"` (line 64), audio — only `aac`, otherwise an explicit
error "system FFmpeg has no AAC encoder" (line 173), video — five candidates from
`VIDEO_ENCODERS` (line 15): `h264_nvenc` → `h264_vaapi` → `h264_qsv` →
`libx264` → `mpeg4`. The pixel format is always `YUV420P`, `gop_size = fps*2`,
`max_b_frames = 0`, `movflags=+faststart`. **Not a single colour tag is
written** (`color_primaries`/`color_trc`/`colorspace` are not touched), nor file metadata.

**The automatic encoder choice is already built correctly, and this is half the answer to the question about a preset with a nailed codec.** `Muxer::open` iterates over the candidates and checks them with a REAL `avcodec_open2` (presence in the build means nothing — nvenc without
a driver fails); with `force_encoder` the loop narrows to a single name, and failure
gives the error `no video encoder could be opened (last error: …)` without any
fallback. This is invariants No. 9 and No. 10 in action, covered by test 25
(`src/libs/kt-engine/tests/export_options.rs`, including
`t25_force_unknown_encoder_is_explicit_error`).

**A file name template exists and is already parametrised — only not by the user.** `render_file_name()` (`render_dialog.rs:129`) builds
`kt-<project>-<region>-<length in seconds>s.mp4`, sanitising the parts
(`sanitize_name_part`, line 113), and `default_render_name()` (line 147)
substitutes the name of a range marker ONLY if it matched the region exactly. The test
is t201 in `src/apps/kt-app/src/layout_tests.rs:4252`. Important: the template was set by
the DIRECT REQUEST of the user (R.33 item 9, `docs/user-feedback.md`: "generally the template
for all renders is always `kt-<projectname>-<region_name>-<its length in
seconds>`"), therefore parametrisation has no right to change the default.

**An export with a resolution different from the project's is computed honestly, not by an upscale at the end.**
`export_mp4_with_backends` (`export.rs:391`) turns `resolution` into
`Quality{width,height}`, and the whole composite is built right away at the target size —
`VideoRenderer::frame_plan` is the same as for the preview (invariant No. 7). This is better than
most: we have no stage "render at the timeline size, then scale".

**kt-cli knows about export exactly one setting.**
`src/apps/kt-cli/src/main.rs`: `kt-cli export <in.otio> <out.mp4> [encoder]`,
everything else is `ExportOptions::default()`. A preset is a natural (and,
perhaps, the only non-ugly) way to give the CLI the same as the GUI
without inventing a dozen flags.

### What is missing

- **A name.** `RenderSetup` is a field of `KtApp` (`app.rs:255`), initialised by
  `Default::default()` (lines 487 and 623). The settings survive neither a restart of the
  program nor OPENING ANOTHER PROJECT (`KtApp` is recreated on opening — 
  `src/apps/kt-app/CLAUDE.md`, the section on GPU). Every render starts from defaults.
  We do not even have "remember the last one" — the absence of which people in AME call a defect.
- **There is no frame rate in the export at all.** `fps` is taken from
  `project.settings.fps` (`export.rs:423`), there is nothing to override it with, the field is absent from
  `ExportOptions`. The resolution can be overridden, the frame rate cannot — the asymmetry
  is not explained by anything.
- **Containers other than MP4, and codecs other than h264+AAC.** A preset "ProRes for the
  client" runs into not the UI, but `muxer_impl.rs`.
- **There are no fit rules — more precisely, there IS a rule, it is "stretch", and nobody
  chose it.** A frame is decoded straight into the target size: `to_rgba`
  (`src/libs/kt-ffmpeg/src/source_impl.rs:904`) calls `sws_getContext(f.width,
  f.height, …, dst_w, dst_h, …)` — the aspect ratio does not take part. Neither
  `ProjectSettings` (`src/libs/kt-model/src/project.rs:1489`: only
  `fps`/`width`/`height`/`sample_rate`) nor `Quality`
  (`src/libs/kt-media/src/lib.rs:31`) know about the pixel aspect ratio. One can reach this
  with a standard click: the item "Max (W×H)" takes the maximum width and the
  maximum height INDEPENDENTLY (`max_source_resolution`, `app.rs:1646`), therefore
  a project with a horizontal 1920×1080 and a vertical 1080×1920 will offer "Max
  (1920×1920)" — a ratio that no source has, and the whole clip will come out square. All other editors have an
  explicit choice of three or four rules in this place; we have silence.
- **Colour tags and metadata in the file.** The very full-range/ltd-range story from the quote
  is not even expressed in ours.
- **Actions after render** (open the folder, put into the queue) — neither the action itself nor a
  place where it would live.

### What gets in the way

**1. The storage has nowhere to live, and this is ALREADY the THIRD feature in a row that runs into the same thing.** The project is one `.otio` file (`src/libs/kt-otio/`), and `KtSettings`
(`src/libs/kt-otio/src/meta.rs:114`) carries only `fps`/`width`/`height`/
`sample_rate`; there is no place for the export settings there, and in meaning there should not be one —
a preset lives BETWEEN projects. Everything the program has at the application level is
the directory `$XDG_DATA_HOME/kinotrain/` with a single autosave file
(`autosave_path()`, `src/apps/kt-app/src/app.rs:4770`) and the eframe key-value store of two lines (`lang`, `last_project` — `app.rs:5036`). kt-app has no settings file at all. Exactly this conclusion is already recorded in **F-AUT-004** (the library of
FX presets, section "What gets in the way", item 3) and in **F-TXT-002** (title templates).
Three independent features have themselves arrived at "a permanent user storage outside the project is needed" — so this is not a detail of three notes, but a **separate
task with its own life cycle**: backup, transfer to a second machine, name conflicts, schema migration when `ExportOptions`/`FxInstance` change, behaviour on a change of
program version. The fate of Premiere's `.prfpset` (the quote about a library wiped out by an install) shows what one pays for a monolith in the version profile.
This question must not be decided inside any of the three notes (rule 8 of `README.md`),
but making it three times in a row, each one in its own way, is even less acceptable.

**2. A preset with a nailed encoder versus our automatic choice.** The correct
behaviour is ALREADY chosen and implemented with us: `force_encoder` = "only this one, did not
open — error", without a quiet slide (invariants No. 9/No. 10 and UI rule 16
"an impossible action is shown as an ERROR"). The quote about NVENC and driver
581 confirms that a quiet slide would be worse: the user would get a file made
not by what they asked for, with the preset name unchanged. What remains open is another thing: the combo in the dialog shows encoders by PRESENCE IN THE BUILD
(`available_encoders()` filters `avcodec_find_encoder_by_name`,
`muxer_impl.rs:24`), not by "actually opens" — that is, one can already choose
`h264_nvenc` today on a machine without a driver. A preset will make this
situation portable between machines, therefore it must have TWO different values: "Auto" and "exactly this encoder", and "Auto" is the default (UI rule 4: the useful mode is on by default). The wording and place are for the round.

**3. The dialog has nowhere to put a list (UI rule 13, "do not waste space").**
The window already holds six rows of the grid, a warning about unavailable sources and
two buttons, and it is `resizable(false)` with an anchor in the centre
(`render_dialog.rs:275-280`). A separate panel with a list of presets for the sake of an
element used once per session is exactly the defect against which the rule is written. The references from the checklist (the marker name on the time label row, the bin group name in the gap of the frame line) set the direction — to look for an ALREADY occupied
place — but this note does not design the concrete solution.

**4. The NTSC family is inexpressible for us, and presets would expose this.** The tick = 1/90000 s, and `Project::validate` requires `TICK_HZ % fps == 0`
(`src/libs/kt-model/src/project.rs:3117`, error "settings: fps must divide
90000"). 23.976 / 29.97 / 59.94 do not fit the grid: the import snaps them to the
nearest integer with a drift of 0.1 % and says so (`grid_fps`,
`src/libs/kt-otio/src/convert.rs:1357`, test 178). A preset "YouTube 23.976" or
"broadcast 29.97" therefore cannot exist with us — and this must be SAID aloud (UI rule 16),
rather than quietly handing over 24.

**5. The fit rule is composition code, not a muxer option.** Right now the fitting happens in the decoder (`source_impl.rs`, `sws_scale` to the target
size), that is, OUTSIDE `frame_plan`. Any "fit with borders" / "crop" changes the
geometry of the layer and MUST live in the shared plan code, the same for the CPU and
GPU executors — invariant No. 7 in the R.29 edition, with kt-gpu conformance
(t181/t182). This is the most expensive part of the feature and the only place where a "preset"
stops being a snapshot of settings.

### Which invariants it touches

- **Invariant 7** (the composition code is one): the fit rule and any change of
  frame geometry — in `frame_plan`, as a CPU + WGSL pair, otherwise the preview and export
  will diverge. Byte equality of preview and export (t11/t67, t184) holds
  precisely on the absence of such forks.
- **Invariant 9** (export is fully in-process): a new container from a preset is a new `oformat` in `kt_ffmpeg::Muxer`, and not a call of an external
  `ffmpeg`. The temptation "let the preset store the command line" is forbidden
  by the architecture.
- **Invariant 10** (the binary is not linked with libav*): the set of codecs is a property of the
  SYSTEM FFmpeg of a specific machine, not of a preset. A preset may NAME
  a codec, but it must be verified by a real opening on the loaded version (6/7/8),
  as `Muxer::open` already does.
- **Invariant 11** (determinism): if a preset starts to fix the backend (GPU/CPU), it will start to fix the BYTES — cross-backend equality holds only within tolerances. A preset must not silently choose the backend.
- **UI rule 4** (the default is the useful one): "Auto" for the encoder remains the default
  inside a preset as well.
- **UI rule 13** (do not waste space): see "What gets in the way", item 3.
- **UI rule 16** (impossibility is shown as an error): an unavailable encoder, an inexpressible
  frame rate, an incompatible codec — are errors, not faded hints.
- **UI rule 19** (the dialog says where the decision can be changed later): a preset applied
  silently must leave a trace — which one was applied and what overrides it.
- **UI rule 20** (nothing grows by itself): the list of presets is not replenished
  with "the last used" — only by an explicit save.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages: (1) a named snapshot of `ExportOptions` + storage, (2) a name template with substitutions and reception of a preset in kt-cli, (3) frame rate and fit rules. Stage 3 alone pulls to L and touches invariant 7 |
| Where the main work is | kt-app (list, window, application), kt-engine (`ExportOptions` grows; `frame_plan` — if we take fitting), kt-ffmpeg (if the container is wider than MP4/h264/AAC), kt-cli (accept a preset instead of `[encoder]`) |
| Project format | `.otio` is NOT touched. Creates application-level storage — shared with F-AUT-004 and F-TXT-002; it must be decided once for all three |
| Tests to write | round-trip of a preset (write/read/unknown field); "a preset with an unavailable encoder = an explicit error" after the pattern of t25; substitutions in the file name with the default unchanged t201; a harness test of the window in `layout_tests.rs`; at stage 3 — CPU↔GPU conformance of the fitting (t181/t182) and unchanged GOLDEN_VIDEO for the preset "as the project" |
| Depends on | F-AUT-004 (where the library between projects lives), F-EXP-002 (queue/Media Encoder: there is the same question "preset versus JOB") |
| Breaks compatibility | no |

## Verdict

**We take it, but in a narrow form and not before the storage question is settled.** A named
snapshot of five fields of `ExportOptions` is work for one stage, and it honestly
closes "tired of setting it again every launch" plus the discrepancy between GUI and
kt-cli. It does not close the main thing: we have neither a frame rate in the export, nor
fit rules, nor colour tags — a preset has nothing to carry yet except the codec
and the bitrate, and the most interesting thing (what to do when the frame does not match) is decided silently
and in the wrong direction — by stretching, which you reach by the standard item "Max".
The sensible order is this: first the common user storage (the third note in a row
asks for the same thing), then a settings snapshot with a name, and only then
fitting — as a separate piece of work in `frame_plan`, and not as a checkbox in the dialog.

## Sources

- Final Cut Pro, composition of a destination (Format / Video Codec / Resolution / Color
  Space / Action) and the quote about colour space:
  https://support.apple.com/guide/final-cut-pro/export-file-and-apple-devices-destinations-ver13664388c/mac
  (access date 2026-09-07)
- Final Cut Pro, creating destinations, Duplicate and Bundle:
  https://support.apple.com/guide/final-cut-pro/create-share-destinations-ver9fd008a21/mac
  and https://support.apple.com/guide/final-cut-pro/intro-to-destinations-settings-ver9ac16f8f/mac
  (access date 2026-09-07)
- Final Cut Pro, export to several destinations and the impossibility of appending the codec
  to the file name (the article author's answer in the comments):
  https://larryjordan.com/articles/fcp-x-export-to-multiple-destinations-at-once/
  (access date 2026-09-07)
- VEGAS Pro 2026 build 66, release notes ("Customizable Color Information now
  available for render template in ProRes exports", 27.03.2026):
  https://forum.borisfx.com/t/release-vegas-pro-2026-build-66/21161
  (access date 2026-09-07)
- VEGAS Pro, template bitrate versus file bitrate; a template with NVENC and the driver
  version; groups of templates in "Render As" are not remembered:
  https://forum.borisfx.com/t/rendered-audio-is-of-significantly-lower-bitrate-than-the-bitrate-of-the-render-template/21593 ,
  https://forum.borisfx.com/t/render-problem/22547 ,
  https://forum.borisfx.com/t/first-impression-after-vp22/22672
  (access date 2026-09-07)
- VEGAS Pro, a full-range project + ltd-range render for the sake of matching YouTube, and the pointlessness of high-bitrate YouTube presets:
  https://forum.borisfx.com/t/vp2026b066-color-differences-issue-in-8-bit-full-pixel-range/22064 ,
  https://forum.borisfx.com/t/version-2026-0-3-b-189-oh-soooo-close/25227
  (access date 2026-09-07)
- DaVinci Resolve, the Deliver page, render queue and Quick Export:
  https://www.blackmagicdesign.com/products/davinciresolve/ ; version 21 and
  MainConcept in render settings:
  https://www.blackmagicdesign.com/products/davinciresolve/whatsnew
  (access date 2026-09-07)
- DaVinci Resolve, Input/Output Scaling and black bars with matching resolutions; absence of match-source for the timeline and export:
  https://creativecow.net/forums/thread/black-bars-on-export-issue/ and
  https://creativecow.net/forums/thread/matching-settings-of-original-dv-prores-422-footage-in-exports-for-resolve/
  (access date 2026-09-07). The fitting options are named by the wording from the
  threads; **NOT VERIFIED against the Resolve reference** — the reference manual is unavailable in this
  environment
- Premiere / Media Encoder: location of `.epr` in the AME profile, disappearance of presets
  on a version update, rejection of import of rescued files, absence of a default preset:
  https://creativecow.net/forums/thread/export-setting-presets/ ,
  https://creativecow.net/forums/thread/updated-premiere-2017-cc-and-have-no-more-presets/ ,
  https://creativecow.net/forums/thread/custom-export-preset-location-on-mac-using-premier/ ,
  https://creativecow.net/forums/thread/how-do-i-set-a-default-preset-in-ame/
  (access date 2026-09-07)
- Premiere, Export Preset Manager and Quick Export (22.3.1, 2022), secondary source:
  https://larryjordan.com/articles/new-quick-export-and-export-presets-manager-in-adobe-premiere-pro-22-3-1-v/
  (access date 2026-09-07)
- Adobe Media Encoder as a separate application of the CC 2026 line (26.0):
  https://en.wikipedia.org/wiki/Adobe_Media_Encoder (access date 2026-09-07)
- **What could NOT be verified and why.** `helpx.adobe.com` does not open from this environment ("proxy refused the connection"), therefore the Premiere and After
  Effects lines in the table are assembled from secondary sources and marked
  `[NOT VERIFIED]`: for AE this is the split between Render Settings templates and Output
  Module templates; for Premiere — the exact format and location of `.epr` (indirectly
  confirmed by the forum threads above). The search engines barely worked in this session: Mojeek and Ecosia — 403, Brave — 429, both html versions of DuckDuckGo —
  CAPTCHA, Bing — irrelevant results. **Correction to
  `docs/features/_editors.md`, section "How to verify":** the VEGAS forum has moved from
  `vegascreativesoftware.com` to the Discourse `forum.borisfx.com`
  (category `/c/vegas-pro/55`) and is served excellently through
  `search.json?q=` / `t/<slug>/<id>.json` / `raw/<id>/<post number>`;
  `forum.blackmagicdesign.com` turned out to be entirely UNAVAILABLE in this wave (403 on the root, on `viewforum.php` and on `search.php`), therefore all the Resolve feedback
  was taken from Creative COW; `community.adobe.com` opens only the board listings
  `/t5/<board>/bd-p/<board>`, search is closed (403). None of the five
  editors was checked live.
