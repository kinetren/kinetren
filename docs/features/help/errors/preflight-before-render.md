---
id: F-HLP-011
title: Project check before render starts
status: candidate
priority: B
cost: M
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136]
we_have: partial
crates: [kt-engine, kt-app, kt-media]
invariants: [7, 9]
ui_rules: [9, 16, 19]
round: null
point: null
tests: []
depends_on: [F-HLP-008, F-EXP-002]
checked: 2026-09-07
---

# F-HLP-011 — Project check before render starts

## Essence

The editor presses "Render" and walks away. Forty minutes later he returns either to a finished file or to a message about what could have been said in the very first second: there was not enough disk space, a source went missing, the encoder is absent, an empty region was selected. Preflight is the list of everything the program knows about the upcoming render, shown ONCE before the button, with a split between "cannot" and "will not come out as you think".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none: there is no check of its own before the start | "Render As" checks nothing. The "Estimated Properties" section is in the PROGRESS window, that is, after the button: it expands into the remaining free disk space, the expected total duration of the render and the end time. The price figure exists, but it arrives when the price has already started to be paid. [checked against the help of line 21; in 2026 the section, judging by the composition of the window, is the same — NOT CHECKED live] |
| Adobe Premiere | 26.x | a warning at export | There is no separate report, there are point warnings. About offline: "Offline material is present in this export and will be encoded using the offline media graphic" — a warning, not a refusal, and it names the CONTENT of the future file. It also fires for a clip that is DISABLED in the edit. Disk space is checked against an estimate of the size — the estimate is systematically understated (multiplexing requires as much again) |
| DaVinci Resolve | 21 | a warning when a job is ADDED to the queue | The moment of the check has moved from "Render" to "Add to Render Queue": according to the manual, a job with offline material gives a warning, and one can cancel the addition or add it knowing that one renders offline. Separately — a setting that aborts the render when it REACHED an offline clip. In Deliver, the File tab — a rough estimate of how much space will remain after the render; it is insensitive to the chosen codec |
| Final Cut Pro | 12.3 | a BLOCKER at Share | The only one of the five that actually refuses: "[project] has missing or offline titles, effects, generators, or media". The category is named, the culprit is not. False positives are known (the Deinterlace effect, disabled in Info but live in Effects). There is also a second refusal, a narrow one: "This item cannot be shared while it is still referencing media on the camera" |
| After Effects | 26.x | before the render — no; a separate REPORT — yes | Missing material is silently rendered as colour bars (AE substitutes the placeholder itself). But there is the only report file in the five: File > Dependencies > Collect Files, the checkbox "Generate Report Only" — Report.txt with the list of what was not found, WITHOUT copying files. There is no free-space check at all — it is requested as a feature request |

**Outside the core.** Without VS Code the third button cannot be described: of the five NLEs a refusal either blocks or lets through, and none of them offers to GO to the culprit — and that is exactly what the FCP message lacks.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Visual Studio Code | 1.136 | "Error exists after running preLaunchTask 'build'." | Four buttons: Open launch.json · Show Errors · Cancel · Debug Anyway. The policy is taken out into the setting "On Task Errors" (the value `prompt` returns the dialog to those who disabled it): the user decides ONCE whether it is a blocker or a warning. The list of findings lives separately from the moment of launch — in the Problems panel, where each row has its own address in the code and its own severity |

## The trick behind it

The cargo that preflight carries and that one status-bar line cannot hold is a LIST: several findings at once, each with its own weight and its own address. A line shows one at a time and only to the one who has already hovered; here everything has to be shown at once to the one who has not yet suspected anything. For us this cargo goes into the body of an ALREADY OPEN render-settings window, between the settings and the "Start" button — a window the user came to on his own.

Three details that are absent from the feature lists.

1. **Resolve moved the check from the "Render" button to "Add to Render Queue".** This is not cosmetics: the queue is the last moment when an edit is still cheap, and before the button itself hours may pass. The second, older mechanism — the setting "abort the render at an offline clip" — costs exactly as much render time as there is until the problem: an abort at the 42nd minute of a forty-five-minute programme eats 42 minutes. It is exactly this arithmetic that gave rise to the first mechanism.
2. **The Premiere warning describes not the project but the FUTURE FILE:** "will be encoded using the offline media graphic". The practical difference: a message about the project asks you to go and fix it, a message about the file lets you decide without leaving the export — "yes, let those four seconds be a banner". The downside, in "Cons": it does not know whether the clip will get into the render at all.
3. **The AE report is a file, not a modal window,** and it is made by a checkbox that says "only check, do nothing else" (Generate Report Only inside Collect Files). The report survives the dialog: it can be attached to an email "here is what I am missing", which no pop-up window can do.

## Pros

- One list in one place before one button — the price of recognition is paid once. Otherwise it is paid once per finding, and each time it is the price of the render, not the price of the dialog.
- A check before the start consumes no resource that it protects: it costs milliseconds, while it guards hours. The "abort when we reach it" mechanism (Resolve) costs exactly as much as there is until the problem.
- Typing "blocker / warning" lets ONE mechanism hold both the refusal and "it will come out not as you think": Premiere and Resolve give a button "anyway", FCP does not — and both positions are legitimate, as long as the author chooses them and not chance.
- A list with addresses (Problems in VS Code) turns a refusal into an action: the third button leads to the culprit instead of merely not letting you through.

## Cons

- The check knows the PROJECT, while the render renders the RENDER. Premiere warns about an offline clip that is disabled and will not get into the file. A false alarm is more expensive than it seems: it trains people to press "anyway" blindly, and the next — real — alarm will be skipped too.
- The estimate of free space lies depending on the device: the real consumption depends on the codec and on the temporary copy during multiplexing, and the practical advice of the communities is to keep three to four times what the estimate showed. A check that said "there is enough space" and was wrong is worse than no check: now the program is to blame.
- A blocker without the culprit's name (FCP) turns the check into a puzzle: the program KNOWS the object, because it found it, but the message gives only the category.
- The report file (AE) does not interfere and does not help: one has to remember it and run it by hand, so it is run only after one has already been burned.

## What users say

On the price of a late discovery — the author of the request for a warning in the Resolve queue (Mel Matsuoka, 08.11.2019), about an abort of the render at the 42nd minute of a 45-minute programme:

> «you wouldn't know about it until after you've wasted all that time»
> — forum.blackmagicdesign.com, 08.11.2019,
> https://forum.blackmagicdesign.com/viewtopic.php?f=33&t=102245

The rest is retold, without quotation marks. A feature request for After Effects dated 09.04.2026 asks for exactly a preflight by place: estimate the size of the queue, compare it with the free space on the destination disks and warn; the worst case, in the words of the author, is to queue a batch for the night and find in the morning that NOT ONE thing rendered, because there was not enough space for the first in the queue
(https://community.adobe.com/feature-requests-530/after-effects-available-space-check-at-render-time-1556977).
The negative side of the mechanism — there too, from Premiere: the user BrianDavison (11.05.2023) considers the warning about offline material of a DISABLED clip, which will not appear in the export anyway, to be superfluous
(https://community.adobe.com/questions-729/on-export-premiere-warns-about-offline-clips-in-sequence-even-if-clip-is-disabled-1410012).
About FCP's nameless blocker — Larry Jordan calls this message the most mysterious and the most painful in the program, and separately notes that it does not say which clip is at fault
(https://larryjordan.com/articles/potential-export-bug-in-final-cut-pro-11-2/).

## How it fits Kinotrain

### What already exists

There are exactly TWO checks, and this is confirmed by grep (`preflight`, `GpuRequired`, `missing_sources` across the whole `src/`).

1. **One refusal.** `ExportError::GpuRequired(usize)` in `src/libs/kt-engine/src/export.rs:293`; it fires in `export_mp4_with_backends` (there too, lines 403–408) before the muxer is created, when `backends.video_gpu.is_none()` and `gpu_required_clips()` returned a non-empty list. The decision is written directly in the code and sounds like a ready formulation of the whole note: "R.29: the project has 3D effects, and there is no GPU engine. A refusal BEFORE the start — a silently spoiled file (clips without 3D) is worse than an honest error." The list itself is computed through `effective_clips()` and collapses the pieces of a morpher layer into the source clip (`slice_origin`), so as not to name synthetic ids that the user does not see in the project (export.rs:296–309).
2. **One warning.** `missing_sources()` in `src/apps/kt-app/src/render_dialog.rs:101`; the list is assembled in `start_render()` and shown in the settings window BEFORE the "Start" button (render_dialog.rs:400) under the key `render.missing_warn` ("unavailable sources (N), they will get into the file as placeholders: …"). This is a WARNING, not a refusal, and it was decided so deliberately — a direct request of the owner, R.33 item 10: "during render and preview the absence of sources must not be a blocker; the user must be notified and the render must proceed; for missing video a placeholder must be substituted". Implementation — item 208, the guard — t210 (`layout_tests.rs:3871`), which checks exactly that the list reaches the settings window. Changing this to a refusal is NOT allowed.

So the difference "blocker vs warning", which in the five editors came about by chance, we already have made deliberately and for different reasons: the GPU is a blocker, because a silent result differs from the intended one; the stubs are a warning, because the result differs VISIBLY and the user asked for it.

Three neighbouring mechanisms that are not preflight, but belong to the same honesty: the summary AFTER the render names the stubs and the lost layers (`render.missing_after`, `render.missing_layers`, render_dialog.rs:551–568); an error and a cancel delete the unfinished file (`cleanup_on_err`, export.rs:468–471); the video encoder is chosen by a REAL opening attempt (`src/libs/kt-ffmpeg/src/muxer_impl.rs:78–165`) — and, more important for this note, the whole chain of probes happens BEFORE `avio_open`, so an unsuccessful encoder leaves no empty file on disk. This is a direct consequence of invariant 9: the export is in-process, so of all of us we are the ones who can answer "this encoder will open" not by guessing but by a fact.

### What is missing

None of the listed items EXISTS (checked by grep; `statvfs`, `free_space`, `disk` do not occur in `src/` at all):

- **Free space.** It is not checked anywhere, in any way.
- **The destination file.** Whether the output matches the project's SOURCE is not checked: `launch_render()` takes the path from the system save dialog and does not compare it with `project.media`. The cost is high because of `cleanup_on_err`: a render over its own source not only spoils the source, but also deletes it on error — together with the unfinished output it is one and the same file.
- **An empty render.** `project_range()` (export.rs:71) on an empty project gives zero, `total_video`/`total_audio` become zeros, the loops do not run even once — the muxer honestly writes a valid file without frames and returns `Ok`. Success with zero content. kt-cli HAS such a check, but only in the wav path: "empty project - nothing to render" (`apps/kt-cli/src/main.rs:67`).
- **The encoder.** The combo box of the settings window is filled by `kt_media::muxer::available_encoders()`, and that is `avcodec_find_encoder_by_name` — PRESENCE in the build, not openability. Worse: a pinned encoder turns off the fallback (in the loop of `muxer_impl.rs` all other candidates are skipped), and choosing `h264_nvenc` on a machine without a driver turns the automatic choice described in invariant 9 into a guaranteed error. The list that the window shows and the list that actually opens are different, and this is visible only from the code.
- **Resolution.** The combo box offers fixed variants and `Max (w×h)`; what happens with another aspect ratio is not said, and letterbox/pillarbox is absent in the compositor (grep for `aspect` finds only `kt-engine/src/transitions.rs`) — the picture will be silently stretched. This is exactly the case "it will come out not as you think", that is, a warning, not a refusal.
- **Clips outside the timeline** — not our defect: `project_range()` takes the maximum of the material and `timeline_end`, so nothing beyond the edge is lost. There is nothing to check.
- **The addressability of the single refusal.** `GpuRequired` carries a `usize`, although `gpu_required_clips()` returned exactly the clip IDs — the list was computed and thrown away. It reaches the window through `res.map_err(|e| e.to_string())` as an English thiserror string inside `render.error_msg` ("Error: {0}"): the frame is translated, the contents are not; the catalogues do not touch it, t125 does not see it, and rule 19 ("the dialog says where to change the decision later") is not met — not a word about `KT_VIDEO_GPU`, about the CPU path, and about which clips to disarm.
- **The scope of the check is wider than the render.** `missing_sources()` walks `project.media` as a whole — that is, the entire bin, not what will get into the file. The warning will come because of material that lies in the bin without a single clip, and because of material outside the region. This is word for word BrianDavison's complaint about Premiere, except that for us no one has voiced it yet.
- **GUI and CLI diverge.** On the same project the render window will substitute a placeholder and go on, while `kt-cli export` refuses the whole thing (`main.rs:287` and `295–300`: the first non-opening row of `project.media` → exit 1). The decision of R.33 item 10 reached the GUI and did not reach the CLI.

### What gets in the way

Free space requires `statvfs`, and there is no direct dependency on `libc`/`rustix` in any of our crates (by grep over all `Cargo.toml`); `libc` 0.2.189 is present transitively in the graph, so the price is one line in `kt-app`, but not in the core: the rule "do not pull dependencies into kt-time/kt-model/kt-fx/kt-cache and the kt-engine core" forbids adding it where the rest of the preflight is computed. That means the list of findings will have to be assembled from two sources — the engine's (the project and its clips) and the application's (the disk, the path, the permissions).

The second constraint is the moment when the backend becomes known. `export_video_gpu()` (`src/apps/kt-app/src/app.rs:5215`) raises its own headless device INSIDE the render thread, after the save dialog; a GPU refusal physically cannot arise in the settings window, while the decision about the backend is made later than this window closes. Invariant 7 ("a started export finishes on the backend on which it started") makes this not a detail but a requirement: since the backend cannot be changed mid-export, the question "will it be there at all?" has to be asked BEFORE the start, not in the thread.

### Which invariants it touches

- **Invariant 7** — a started export finishes on its own backend, so the check "is this project rendered on this backend?" must happen before the start. Today's `GpuRequired` already embodies this logic, but a thread late.
- **Invariant 9** — the export is in-process: we check the availability of the encoder not by guessing but by `avcodec_open2`. This is our strong side, and in preflight it must end up on the button, not behind it.
- **Rule 16** (the impossibility of an action is an ERROR, not a faded hint) and **rule 9** (do not make obvious labels: a list of "everything is fine" with eight green ticks is junk; silence is "everything is fine").
- **Rule 19** — the dialog says where the decision can be changed later; today this is not met in a single line, and the GPU refusal is the first candidate for correction.
- Invariant 12 is not touched: the list is computed once when the window opens, not per frame.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2 stages: the list of findings in the engine + a second surface in the settings window |
| Where the main work is | kt-engine (a pure function "project + options → findings"), kt-app (the display and what requires the disk and the path), i18n |
| Project format | not touched |
| Tests to write | `reach_tests::preflight_names_what_it_refuses` (the refusal is translated and names the culprit); a unit test on an empty range and on an output that coincides with a source; an extension of t210 to the scope "only what gets into the file" |
| Depends on | F-HLP-008 (the anatomy of a message: what / why / where to repair), F-EXP-002 (the render queue — it is about background and queueing, here only the refusal before the start) |
| Breaks compatibility | no |

## Verdict

We take it. The check is not absent in our case — it is computed and lost along the way: the single refusal knows the names of the clips and gives a number, knows the interface language and speaks English, knows about the CPU path and does not offer it. The cheapest thing is to start not with new checks but with the addressability of the ones we have.

TEST: reach_tests::preflight_names_what_it_refuses — fails today: the single refusal before the start (`ExportError::GpuRequired`, `src/libs/kt-engine/src/export.rs:293`) carries a `usize` instead of the computed `gpu_required_clips()` ids and reaches the window through `res.map_err(|e| e.to_string())` as an English thiserror string inside `render.error_msg`, bypassing the i18n catalogues and t125.

ONE LINE: overlap — not touched | delay — not touched | capacity — zero new characters in the status-bar strip; the list of findings lies in the body of the ALREADY OPEN render-settings window, between the settings and the "Start" button, where today stands a single warning about the stubs (`render_dialog.rs:400`).

## Sources

- https://forum.blackmagicdesign.com/viewtopic.php?f=33&t=102245 — the request for an offline warning in the Resolve queue (accessed 2026-09-07, through the text proxy `r.jina.ai`)
- https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part3956.htm — the Resolve manual, "Using the Render Queue": the warning when adding a job with offline material (the page did not open directly; the text was obtained from search results by an exact phrase; accessed 2026-09-07)
- https://community.adobe.com/t5/Premiere-Pro/quot-Offline-material-is-present-in-this-export-and-will-be/td-p/3932408 — the verbatim text of the Premiere warning (2026-09-07)
- https://community.adobe.com/questions-729/on-export-premiere-warns-about-offline-clips-in-sequence-even-if-clip-is-disabled-1410012 — the warning fires for a disabled clip (2026-09-07)
- https://community.adobe.com/feature-requests-530/after-effects-available-space-check-at-render-time-1556977 — the feature request "check free space" for AE, 09.04.2026 (2026-09-07)
- https://community.adobe.com/t5/adobe-media-encoder-discussions/rendering-keeps-failing-after-filling-up-c-drive/td-p/10839707 — the render fails at the end when the disk runs out (2026-09-07)
- https://larryjordan.com/articles/potential-export-bug-in-final-cut-pro-11-2/ — the FCP message "has missing or offline titles, effects, generators, or media" and the fact that it does not name the clip (2026-09-07)
- https://support.apple.com/en-us/HT207791 — the second FCP refusal at Share (material on a camera/card) (2026-09-07)
- https://help.magix-hub.com/video/vegas/21/en/content/topics/render.htm — "Estimated Properties" of the VEGAS progress window (2026-09-07)
- https://helpx.adobe.com/after-effects/desktop/work-with-footage-items/manage-footage-items/footage-items.html — the colour bars as the automatic placeholder for missing material (2026-09-07)
- https://github.com/microsoft/vscode/issues/68084 — the text of the VS Code dialog and its buttons (2026-09-07)
- The five core rows were assembled from the documentation and forums of the vendors; live, none of the editors was launched in this environment. The row outside the core (VS Code) — all the more so: a live check for non-NLEs is not assumed (`_editors.md`).
