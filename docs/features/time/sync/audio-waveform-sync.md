---
id: F-SPD-004
title: Syncing camera and recorder by waveform and timecode
status: done
priority: B
cost: M
category: time/sync
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-media, kt-model, kt-app]
invariants: [1, 5, 8, 11]
ui_rules: [13, 17, 19]
round: "R.49"
point: 374
tests: [413, 414, 415, 416, 417, 418, 419]
depends_on: [F-SPD-001]
checked: 2026-09-15
---

# F-SPD-004 — Syncing camera and recorder by waveform and timecode

## Essence

The camera records sound with its built-in microphone - "for reference", it is boomy and noisy.
Clean sound is recorded by a separate recorder: a lavalier, a shotgun, a mixer. On the timeline
there come out two events that have nothing in common, except that they were recorded in one room. Lining them
up by hand means moving the sound by frame until the clap meets the picture, and doing this for every take. The "synchronise" button
does this in one gesture: the program itself finds the shift and puts the partner in place. We have no such button:
there is only the A/V magnet, which pulls the partner by the PROXIMITY of the starts - that is, it already requires that everything is almost lined up.

## Who has it and how

The question for all is one: by which means the shift is searched for, WHERE the found result is stored, and what can then be done with it.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | "Audio synchronization for multicam" (appeared in VEGAS Pro 17, build 284, 05.08.2019) | the synchronisation lives INSIDE the multicamera path (`Tools → Multicamera`); there is no separate command "line this event up with that one"; the result is shifted events on the timeline, no new object. According to the forum, it matches the shape of the ENVELOPE, not transients, and is designed for cameras with a common generator; on "camera microphone against lavalier" it drifts apart |
| Adobe Premiere | 26.x | `Synchronize…` (on the timeline) and `Merge Clips…` (in the bin) | two different tools. `Synchronize` moves already laid-out events and synchronises by one of the points: Audio, Timecode, Clip Marker, In/Out Points. `Merge Clips` in the bin creates a NEW project item - a "merged clip", which is "not a real clip" but a bundle of the source clips synchronised by sound. Taking a merged clip apart again is not standard: for this there is a third-party `UnMerge` from PlumePack |
| DaVinci Resolve | 21 | `Auto Sync Audio` (media pool), `Auto Align Clips` (timeline), Sync Bin (Cut page) | the only one of the five that writes the result into the SOURCE: `Auto Sync Audio` changes the binding of the audio channels of a media pool CLIP - the switch "Retain embedded audio" decides whether to replace the camera sound or add channels alongside it. Options - by timecode, by waveform, by markers/in-points. `Auto Align Clips` on the timeline aligns the selected clips on the lowest track. Sync Bin - "shows only those clips that are in sync with the timeline" and places them at the cursor position |
| Final Cut Pro | 12.3 | `Synchronize Clips` → **synchronized clip** | creates a NEW container clip in the browser, the source clips are not touched: "Final Cut Pro analyzes the clips for sync points such as markers you've added, timecode, file creation date, and audio content". The tick `Use audio for synchronization` enables refinement by waveform, `Disable audio components on AV clips` mutes the camera sound. The container opens inward (`Clip → Open Clip`) - that is, the synchronisation can be corrected by hand without taking the object apart |
| After Effects | 26.x | none | not about this: AE is a layer compositor, it has no double-system sound as a task. The merging of material is done before AE, in the NLE |

## The trick behind it

**First and foremost: the result of synchronisation lives in DIFFERENT places in all five, and
this decides not convenience but the fate of the material.** Vegas puts the result on the timeline
(the events are simply shifted). Premiere - into the bin, as a new item: a merged clip, according to Larry
Jordan's description, is "not a real clip", but a bundle of two source ones; the source
clips in the bin at the same time REMAIN out of sync, and the next editor who takes them
directly again gets the desync. Resolve writes into the SOURCE ITSELF - the set of audio channels of a media
pool clip changes, therefore every subsequent use of this material is already synchronous, and there is no need to
synchronise a second time. FCP - into a container that can be opened inward and corrected. Hence a practical
consequence that is absent from every list of features: in Resolve synchronisation is done ONCE per project, in Premiere - once for each
merged clip, and there are no standard means to take a merged clip apart again (the third-party PlumePack created a separate
command `UnMerge` for this, "clips on the timeline built from the source Merged Clips are automatically linked to the new project items").

**Second: synchronisation is ONE NUMBER, and the discrepancy can be sloped.** All four methods (timecode, waveform,
marker, metadata) answer the question "by how much to shift", that is, they find one point. But the camera and the recorder are clocked by
different crystals: a frequency discrepancy of 10 ppm over a 40-minute take gives 24 ms
(more than half a frame at 25 fps), 50 ppm - 120 ms, three frames. A take perfectly lined up by the clapper at the beginning
noticeably drifts apart by the end, and repeated auto-sync is useless: it again finds ONE number and again aligns either the start
or the middle. This is cured not by synchronisation but by stretching one of the tracks by a fraction of a percent - and none of the five editors
does this in the "synchronise" command. That is exactly why film crews still run a timecode generator:
it is not "more convenient", it is the only thing that removes the slope, not the offset.

**Third: at the moment of failure everyone behaves differently, and silence is the worst option.** Apple directly documents FCP's behaviour: "If no sync points can be
found, the clips are synced at their respective starting points" - that is, when analysis fails, the clips still end up "synchronised", just by
their starts, and the editor learns of this at the viewing. Vegas, according to users, on unsuitable material gives a result that "doesn't work at all". A refusal with
an explanation ("no common sound was found, confidence 0.2") is more honest than any number.

## Pros

- Waveform synchronisation requires nothing from the film crew: neither
  a timecode generator, nor a clapper, nor a single recorder. It is enough that
  the camera microphone HEARS the same as the recorder - and it hears almost always.
- The timecode method does not depend on the content of the sound at all: it is equally accurate on silence, in wind and on a take where no one spoke. Where a generator was,
  synchronisation is arithmetic, not analysis, and its result is reproducible.
- A result placed into the source (Resolve) pays off in all the further work:
  the synchronisation survives a reassembly of the edit, because it is not a property
  of the event on the timeline.
- A result placed into a separate object (Premiere merged clip, FCP
  synchronized clip) can be handed to editors as ready material: they physically
  cannot take the unsynchronised version, because they take the object, not two files.

## Cons

- The price of "putting it into the source": the synchronisation becomes a property of the material, and
  if the analysis was wrong, the error spreads across all episodes at once. Undoing it
  means touching the media pool, not the timeline, that is, going beyond the undo of the edit.
- The price of "a new object": a merged clip cannot be taken apart by standard means (for this one sells
  a plug-in), a merged clip cannot be made from a merged clip, and the source clips in the bin
  remain out of sync - two versions of the same material appear in the project.
- The price of "one number": the device of the method excludes drift. No precision of correlation
  saves a long take, because the quantity sought is not a shift but a straight line with a slope, and the command returns a constant.
- The price of envelope analysis: the match of SHAPE is sought in signals that should be similar. The camera microphone and the lavalier are similar by transients, but not by
  envelope (different frequency response, different compressor, different distance) - hence
  "works on copies of one sound and does not work on a real pair". That Vegas is built exactly this way is the conclusion of a forum user (plarsen), not documentation;
  none of the five has an official description of the algorithm **[NOT VERIFIED]**.
- The price of the silent fallback (FCP): "synchronised by starts" is outwardly indistinguishable from "synchronised", and the cost of the error is shifted to the viewing.

## What users say

> I tried it twice and never again.
> — RedRob, Boris FX forum (VEGAS Pro), 26.04.2026, https://forum.borisfx.com/t/what-do-you-all-think-of-the-performance-of-the-audio-sync-feature-built-into-vegas/22331

> The automated process never worked for me and I started on version 12.
> — plarsen, Boris FX forum (VEGAS Pro), 25.05.2026, https://forum.borisfx.com/t/how-to-sync-audio-to-align-events/23748

> The auto-sync feature in VEGAS is meant for gen-locked cameras.
> — Mohammed-Anis, Boris FX forum (VEGAS Pro), 25.05.2026, https://forum.borisfx.com/t/how-to-sync-audio-to-align-events/23748

> The V23 sync was not usable at all. Even the 3 cameras were not sync'd satisfactory.
> — rraud, Boris FX forum (VEGAS Pro), 14.04.2026, https://forum.borisfx.com/t/what-do-you-all-think-of-the-performance-of-the-audio-sync-feature-built-into-vegas/22331

About the structure of the merged clip in Premiere:

> not a “real” clip. Instead it looks real but is simply two clips linked so that their audio is in sync.
> — Larry Jordan, "Adobe Premiere Pro CC: Sync Double-system Sound", 16.02.2020, https://larryjordan.com/articles/adobe-premiere-pro-cc-sync-double-system-sound/

It is worth noting separately that the feedback on Vegas is exactly the genre the knowledge base values above raptures: "tried it once and turned it off for good" (RedRob),
"never worked, and I've been with it since version 12" (plarsen) and the explanation of WHY
("designed for cameras with a common generator", Mohammed-Anis). All three advise
the same thing: third-party tools (PluralEyes, Syncalia, SnapAlign) or
a manual double clap.

For Premiere and Resolve it was not possible to obtain live forum quotes in this wave: the
WebSearch budget is exhausted, `reddit.com` and `helpx.adobe.com` do not open from the environment,
the search page `community.adobe.com/t5/forums/searchpage/…` and
`forum.blackmagicdesign.com/search.php` return 403 (the threads themselves open by direct links, but
there is no way to find the right one without search). Bing and DuckDuckGo from this
environment return either a captcha or navigation junk and silently ignore the
`site:` operator. Therefore the Premiere/Resolve lines rely on the official
Blackmagic pages, Larry Jordan's analysis and Creative COW, not on forums.

## How it fits Kinotrain
CAPABILITY: openspec/specs/audio-render/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Waveform sync - over peak pyramids (a block of 256 frames) in the offline service; a refusal instead of a guess when the correlation is weak.

## Estimate

| What | Answer |
|---|---|
| Scope | M - 2–3 stages of the round: (1) correlation over the pyramid in `kt-media` + unit tests; (2) the command and its ops in `kt-model`; (3) the gesture/menu/notice in `kt-app`. Timecode is a separate stage, because it pulls in `kt-ffmpeg` and `probe_json` |
| Where the main work is | `src/libs/kt-media` (correlation, later - timecode in `MediaInfo`), `src/libs/kt-model/src/ops.rs` (the chain of commands next to `magnetize_commands`), `src/apps/kt-app` (the menu item, notice) |
| Project format | does NOT touch, if the result is a shift + a link group (both things are already serialised). A version would be needed only if a bin object in the spirit of a merged clip were to be created - and creating one is not proposed |
| Tests to be written | correlation on synthetic material with a shift KNOWN in advance (different levels, different frequency response, added noise); determinism (two runs - the same tick); refusal when there is no common sound (confidence below the threshold → the command is not issued, rather than issuing zero); an ops test of the command chain (the partner is NOT trimmed - the difference from test 30); a layout test of the menu item and the notice; for timecode - a round-trip of `probe_json` and a test against the already existing test 180 |
| Depends on | F-SPD-001 (constant speed of the event) and F-SPD-002 - only for compensating DRIFT; the basic synchronisation by one point depends on nothing |
| Breaks compatibility | no |

## Verdict

Take it. This is a rare case where a serious feature rests almost entirely on
ready infrastructure: the data for cross-correlation (the peak pyramid by blocks of
256 samples, mip levels, both channels) is already computed in the background, already lies in the
application's memory, and is already brought to a single 48 kHz grid by invariant 1; the place for
the result (shift + link group) already exists in the model and in the format. What needs writing is the
correlation and one command - the project format does not change in the process. Separately valuable is that we can
immediately do better than Vegas, whose auto-sync users describe as "tried it twice and never again": correlation by transients
instead of the shape of the envelope, plus an honest refusal at low confidence instead of
a silent alignment by starts, as in FCP.

Drift of long takes is NOT part of this feature: it requires stretching, video retiming does not have it (F-SPD-001), and pretending that "synchronise" cures it is
worse than having no button. Synchronisation by timecode is the second queue: it is
simpler algorithmically, but drags along reading `tmcd` in `kt-ffmpeg` and a new
field in `MediaInfo`, and without a generator on the shoot it is still useless.

## Sources

- https://forum.borisfx.com/t/what-do-you-all-think-of-the-performance-of-the-audio-sync-feature-built-into-vegas/22331 — Boris FX forum (former Vegas Creative Software forum; `vegascreativesoftware.info/us/forum` now redirects here with a 301), April 2026 (accessed 2026-09-07)
- https://forum.borisfx.com/t/how-to-sync-audio-to-align-events/23748 — ibid., May 2026 (accessed 2026-09-07)
- https://forum.borisfx.com/t/vegas-pro-release-history/24284 — "Vegas Pro Release History", the entry for VEGAS Pro 17 build 284 (05.08.2019): "Audio synchronization for multicam: Synchronize Multicam footage by analyzing and matching the audio track of the recordings" (accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/sync-audio-and-video-verc1fabc30/12.3/mac/15.6 — official FCP 12.3 help: synchronized clip, synchronisation points, the fallback "synced at their respective starting points" (accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/create-multicam-clips-ver23c764f1/12.3/mac/15.6 — Angle Synchronization and "Use audio for synchronization" (accessed 2026-09-07)
- https://larryjordan.com/articles/adobe-premiere-pro-cc-sync-double-system-sound/ — the structure of the merged clip in Premiere, the synchronisation points of Merge Clips (16.02.2020)
- https://larryjordan.com/articles/apple-final-cut-pro-x-sync-double-system-sound/ — `Disable audio components on AV clips`, opening the sync clip inward (16.02.2020)
- https://creativecow.net/synchronize-audio-in-adobe-premiere-pro/ — the `Synchronize` command on the Premiere timeline (11.01.2024)
- https://www.blackmagicdesign.com/products/davinciresolve/edit — officially: a multicam is synchronised "via audio waveform, timecode or in/out points" (accessed 2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/cut — officially about Sync Bin (accessed 2026-09-07)
- https://davinciresolveclub.com/sync-audio-video-davinci-resolve/ — details of `Auto Sync Audio` / `Auto Align Clips` and the switch "Retain embedded audio" (secondary source: the official `DaVinci_Resolve_21_Reference_Manual.pdf` was not read in this pass - a multi-hundred-megabyte PDF; the Resolve line is to be checked against it in the next wave)
- https://www.autokroma.com/PlumePack/Download — the third-party `UnMerge` command for Premiere; an indirect but firm confirmation that there is no standard way to take a merged clip apart
- The drift figures (10 ppm → 24 ms over 40 minutes, 50 ppm → 120 ms) are arithmetic, not a
  source; the typical spread of crystals in household equipment ±10…50 ppm is taken as a
  general engineering value and was NOT confirmed by a primary source in this wave
  **[NOT VERIFIED]**
- The full list of synchronisation points of the `Synchronize` dialog on the Premiere timeline
  (Clip Start / Clip End / Timecode / Clip Marker / Audio) is given from the analysis by Larry Jordan and Creative COW; the official help `helpx.adobe.com` does not open from this
  environment **[NOT VERIFIED]**
- The Premiere limit "one video clip + up to 16 audio clips per merged clip"
  is DELIBERATELY not asserted in the text of the note: there was nothing to confirm it with

**Implemented in R.49** - item 374
(tests t413, t414, t415, t416, t417, t418, t419).
