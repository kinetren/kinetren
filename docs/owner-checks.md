# What the owner is asked to check

The orchestrator's requests to the owner, in one place (owner, 2026-11-20: "свои просьбы запиши в
отдельный список того что мне надо проверять и для себя там же пиши связи чтобы помнить куда пойти
и что чинить по моим замечаниям").

Each row has two parts:
- **Check** — what to do and on which release. It is written for him.
- **Where to go** — the orchestrator's own links: the claim, the found-defects row, the change, and
  the executor. When his answer arrives, the work starts from there.

When his answer or trace arrives:
1. Register it (`docs/ktprompts/`, `docs/user-feedback.md`).
2. Move the row to "Answered" with a pointer to where it went.

Rows nobody has answered stay open across releases. A newer release replaces the release named in
a row; it does not close the row.

## Open

| # | Since | Check (for him) | Where to go (for the orchestrator) |
|---|---|---|---|
| C1 | 2026-11-19, kt26.11.19-732d7d7f | **problem_region loop.** Play the loop on `problem_region` on the new release and send the `<data>/trace` folder. Is it complete earlier than lap ~15? Do the processor and the card work? | kkt-26.11.18 T2 → ahead-kept (t2930–t2932); `kt-cli trace loop`; found-defects 741, 776; executor mosh-sharp (prerender/hw) |
| C2 | 2026-11-19, kt26.11.19-732d7d7f | **Simple region render speed.** Render a simple region and send the trace. Is it still ~1× real time? | kkt-26.11.18 T3 → export-readback (t2960–t2962); found-defects 740, 741 (VAAPI surface handoff unmeasured on his card); `kt-cli trace … export` |
| C3 | 2026-11-19, kt26.11.19-732d7d7f | **Close while catching up.** If closing ever hangs again, send the trace; `kt-cli trace … shutdown` names the wait. | kkt-26.11.18 T1 → close-while-busy (t2950–t2953, live `close_busy`); found-defects 751, 791 |
| C4 | 2026-11-19, kt26.11.19-732d7d7f | **Datamosh Sharp.** Datamosh at 144p with "Sharp" on: is the break kept and the picture sharp at the project's size? New instances start with Sharp on. | kkt-26.11.18b U1, U2 → mosh-sharp (t2970–t2975, live `datamosh_sharp`); F-FX-015; found-defects 761, 767, 768 |
| C5 | 2026-11-19, kt26.11.19-732d7d7f | **Disk prerender.** After a restart, does a range he already played come back without recomputing? Is the hardware encoder found on his machine? The log names it. | kkt-26.11.18b U4 → render-cache-disk; found-defects 770, 773–776; executor mosh-sharp (hw decode, branch `hw-segment-decode`) |
| C6 | 2026-11-19, kt26.11.19-732d7d7f | **Effects graph.** Does the graph in the effects window and the morpher (v1a–v1c) look and work the way he wanted? Are sliders in nodes and Camera ISP compact? Does it fit without scrolling? | kkt-26.11.18b U3, U7, U8 → change `fx-graph` (design F-UI-008); executor fx-graph |
| C7 | 2026-11-19, kt26.11.19-732d7d7f | **Small features.** Check these: scrubbing and dragging from the media bin, cutting and copying a region, marker notes, the kind galleries, curve keys with Shift, the eyedropper, and the effects centre on the preview. | kkt-26.11.18b U6, U9; v26.10 A1, MW1 → executor features-slow |

## Questions waiting for his decision

| # | Since | Question (with context) | Where to go |
|---|---|---|---|
| Q1 | 2026-11-20 | **Generator from the Edit menu and the media bin.** On a track, the right-click menu now has one item ("Create a generator…"). On a video track it places a solid colour, on an audio track a tone, and the kind is changed in the generator window. The Edit menu and the media bin have no track, so they still show the list of kinds. Should they also have one item? If so, which kind: a solid colour, or by the selected track? | v26.10 G1 → branch `generator-from-track`; executor features-slow |
| Q2 | earlier | **Rename the menu item "Рендер в MP4…" to "Рендер…"?** The render writes more than MP4. | export UI; nobody has taken it yet |
| Q3 | 2026-11-20 | **Graph v2, order of delivery.** v2 lets a chain branch and merge (blur only the highlights and lay them back, parallel compression). (a) recommended: picture branches first; a branch on sound refuses with a reason until the sound stage; ships sooner. (b) picture and sound together; nothing ships until the sound graph is ready. | `openspec/changes/fx-graph-dag` (design b61bef6e), stages v2a–v2d; executor fx-graph |
| Q4 | 2026-11-20 | **Graph v2, inputs from outside the layer.** Today an event's chain sees only its own picture. (a) recommended: not in v2; a Merge mask comes only from its own picture; loading, coverage bars and keys stay as they are. (b) a mask from another track (Track Matte in Premiere/AE): the event then waits for that track's frame, prefetch reads both, an edit there re-renders this event; a step toward compositing between layers (v3). | `fx-graph-dag` design, question 2; executor fx-graph |
| Q5 | 2026-11-20 | **Graph v2, Slowed / Datamosh / pitch inside a branch.** Each runs once per layer today (Slowed picks the source frame, Datamosh decodes its own stream with history, pitch reads past the event's edges). (a) recommended: only before the first split; the editor refuses and says why; one read per layer. (b) allowed in a branch at the cost of a second read: the layer's decode and cache cost double, the bars show two reads, Datamosh keeps a second history. | `fx-graph-dag` design, question 3; executor fx-graph |
| Q6 | 2026-11-20 | **Scopes, where they live.** (a) recommended: a Scopes window, resizable, can go on a second screen, 1–4 instruments; (b) inside the preview area, the picture shrinks while shown; (c) one small instrument under the meters. | `openspec/changes/scopes` Q1; spec-features |
| Q7 | 2026-11-20 | **Scopes, which frame they measure.** (a) recommended: the preview frame while playing, the full-size frame once on a pause; (b) always the preview frame (preview resolution); (c) always full size (halves the playable effect load). | `scopes` Q2 |
| Q8 | 2026-11-20 | **Ripple, an event standing across the edit point on another track** (a music bed on track 3). (a) recommended: refuse, highlight it, say "split it or ripple affected tracks only"; (b) let them overlap (a crossfade appears where you did not edit); (c) leave spanning events, move only those wholly after. | `openspec/changes/ripple-and-trim-modes` Q1 |
| Q9 | 2026-11-20 | **Ripple, markers and areas.** A morpher layer finds its window by a marker's name. (a) recommended: two scopes, markers always follow; (b) Vegas's three, a ripple that would detach a layer is refused. | `ripple-and-trim-modes` Q2 |
| Q10 | 2026-11-20 | **Auto-ripple on moving an event?** (a) recommended: not in the first stage (delete, cut, paste, trims ripple; a move stays a move); (b) yes, as Vegas. | `ripple-and-trim-modes` Q3 |
| Q11 | 2026-11-20 | **Proxies switch.** (a) recommended: a separate "Proxies" switch beside the quality; (b) merged with quality as Vegas (at 1/2 and below proxies turn on). | `openspec/changes/proxies` Q1 |
| Q12 | 2026-11-20 | **Proxies, size and place** (the disk is short). (a) recommended: 1/2 size in `<project>.proxies/`; (b) 1/4; (c) in the data folder with a limit (lost when the project moves). | `proxies` Q2 |
| Q13 | 2026-11-20 | **Proxies, made by themselves?** (a) recommended: never; on a slow decode a single hint; (b) at import, like Premiere's ingest. | `proxies` Q3 |
| Q14 | 2026-11-20 | **Speech recognition engine** (your decision: downloaded on a click, like FFmpeg on Windows). (a) recommended: whisper.cpp and a model pinned by sha256, downloaded at the first use; (b) FFmpeg 8's `whisper` filter (usually absent in distributions); (c) compiled into the program. | `openspec/changes/subtitles-and-text-editing` Q1 |
| Q15 | 2026-11-20 | **Where the Text panel lives** ("мне не нужно топорное окно"). (a) recommended: a tab beside the media bin; (b) its own window; (c) under the preview. | `subtitles-and-text-editing` Q2 |
| Q16 | 2026-11-20 | **Fonts for CJK and Arabic subtitles.** (a) recommended: font packs downloaded on a click; (b) in the release, about +40 MB; (c) burn-in only for covered scripts, the rest as a side file. | `subtitles-and-text-editing` Q3 |
| Q17 | 2026-11-20 | **Decoding sources on the card: on by default?** The picture is byte-equal, so a wrong guess costs speed, never a frame. (a) recommended: on wherever card and file allow, your next trace shows who decoded and the copy cost; (b) off until your trace with it on shows a win. | `openspec/changes/device-decode-sources` Q1; spec-engine |
| Q18 | 2026-11-20 | **A visible switch for card decoding?** (a) recommended: none; Diagnostics says "on the card N files, on the processor M (why)"; (b) a checkbox in Settings. | `device-decode-sources` Q2 |
| Q19 | 2026-11-20 | **Export with no host copy at all (dma-buf into VAAPI)?** This change already moves the colour conversion onto the card and reads 2.7× less. (a) recommended: only if your next render's trace shows the copy or upload binding it; (b) build it now for Linux VAAPI. | `openspec/changes/export-host-copy` Q1; found-defects 741 |
| Q20 | 2026-11-20 | **May an export take frames from the preview's disk prerender?** (a) recommended: never; an export reuses only earlier EXPORTS at the same settings; (b) a "draft render" option that uses the prerender (preview size, lossy). | `openspec/changes/cached-final-render` Q1; found-defects 773; v26.10 X1 |
| Q21 | 2026-11-20 | **Cached render bytes depend on the files it reuses** (a copied GOP is not the GOP a fresh encode makes). (a) recommended: accept for renders with "Cached render" on; with it off, byte-identical as today; (b) no, then X1 is dropped. | `cached-final-render` Q2 |
| Q22 | 2026-11-20 | **Where a rendered file's index lives.** (a) recommended: a file beside the output (`<name>.kt-render`) plus a list in the data folder, nothing inside the video; (b) SEI in the video; (c) both, SEI off by default. | `cached-final-render` Q3 |
| Q23 | 2026-11-20 | **Keep Datamosh history and the heard mix across a restart: now or after measuring?** Today every Datamosh moment not on disk walks its history again after a restart, and covered sound is rendered live again. (a) recommended: first a trace line that counts what a restart re-did; your next trace decides; (b) build all three stages now. | `openspec/changes/durable-history-and-mix` Q1; found-defects 770, 771; spec-engine |
| Q24 | 2026-11-20 | **How the Datamosh history is stored on disk** (must be lossless: one changed value changes every later frame). (a) recommended: raw planes (no processor work, ~9 MB per checkpoint at 1080 lines); (b) LZ4, 2–4× more history in the same cap, a few ms of a core per read. | `durable-history-and-mix` Q2 |
| Q25 | 2026-11-20 | **Seek into a track reverb's tail: how long may Play wait?** Today the tail is silent after such a seek. (a) recommended: always compute the full tail first, in the preview and the render (0.1–0.4 s for a 4-s reverb on release); (b) full tail in the render, at most 0.5 s in the preview. | `openspec/changes/track-stage-preroll` Q1; found-defects 420 |
| Q26 | 2026-11-20 | **Several frame formats: how an event without its own correction sits in 9:16.** (a) recommended: each event is fitted into the format's frame by its own geometry (a full shot fills the vertical by its centre, a PiP keeps its corner, a title reflows); (b) the format is a crop of the finished master frame (titles may be cut). | `openspec/changes/frame-format-variants` Q1; spec-features |
| Q27 | 2026-11-20 | **Safe-area overlays per format.** (a) recommended: presets for YouTube Shorts, Instagram Reels, TikTok and a plain 90/80 %, in the preview only; (b) only the generic title/action-safe frames. | `frame-format-variants` Q2 |
| Q28 | 2026-11-20 | **What a multicam is.** (a) recommended: a composition of angles, and each piece of its instance picks an angle (Premiere/Resolve; effects per angle; a switch is a split + a value, so undo/ripple work as usual); (b) a morpher-like switch track over the angle tracks; (c) Vegas's takes, a list of sources per event (no effects per angle). | `openspec/changes/multicam` Q1 |
| Q29 | 2026-11-20 | **Does the sound follow the angle?** (a) recommended: no by default, one chosen sound through every cut, "audio follows video" is a mode; (b) follows by default, as Vegas. | `multicam` Q2 |
| Q30 | 2026-11-20 | **The angle grid.** (a) recommended: in the preview area by a button, cells with each angle's own effects at 1/8 size, dropping frames before the program picture does; (b) only in the detached preview window; (c) raw cells without effects, cheaper. | `multicam` Q3 |
| Q31 | 2026-11-20 | **Shared events for huge projects (10 000+ events): now or after measuring?** Your project has 1974 events; at ×10 a slider drag costs 128 % of a core. (a) recommended: measure a drag at your size first, build only if it costs more than half a core; (b) build now (a wide change in kt-model, no format change); (c) later, when such a project appears. | `openspec/changes/shared-events` Q1; found-defects 192, 641; spec-engine |
| Q32 | 2026-11-20 | **Tracker: point first, planar later?** (a) recommended: AE's point tracker now (1–4 points: position, rotation/scale, corner pin), planar later behind the same address; (b) planar (Mocha) first. | `openspec/changes/point-tracker` Q1; spec-features |
| Q33 | 2026-11-20 | **Where the tracker's controls live.** (a) recommended: on the preview like masks (right click → "Track point", backward/forward, threshold, Apply in the preview head; the effects window lists the trackers); (b) a Tracker window like AE's panel. | `point-tracker` Q2 |
| Q34 | 2026-11-20 | **Magic wand: an algorithm now, a neural object mask later?** Your keyer rule: an algorithm as the base, the same bytes everywhere, a neural matte only as a later assist. (a) recommended: the algorithmic wand (contiguous or colour range, seeds can follow a tracker) now; (b) a neural segmentation now, downloaded on a click. | `openspec/changes/magic-wand-mask` Q1 |
| Q35 | 2026-11-20 | **What a wand on an effect reads.** (a) recommended: the event's source picture before effects, on the processor ("select the sky, grade it"); (b) the picture at the effect's input, exact but a read back from the card every frame. | `magic-wand-mask` Q2 |

## Answered

| # | Asked | Answer | Went to |
|---|---|---|---|
| — | 2026-11-18 | fx-graph questions (layout, sliders, boxes, sound) | kkt-26.11.18b lines 26–29, U8 |
| — | 2026-11-19 | A1 (curve keys) and MW1 (bin drag) | kkt-26.11.18b lines 31–32, U9 |
