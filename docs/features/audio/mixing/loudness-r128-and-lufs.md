---
id: F-AUD-005
title: Loudness normalization to standards — LUFS, EBU R128, true peak
status: done
priority: B
cost: M
category: audio/mixing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-engine, kt-app, kt-media, kt-model]
invariants: [1, 2, 5, 6, 8, 11, 12]
ui_rules: [1, 9, 13, 17]
round: "R.49"
point: 370
tests: [403, 404, 405, 406, 407, 411, 412]
depends_on: []
checked: 2026-09-15
---

# F-AUD-005 — Loudness normalization to standards — LUFS, EBU R128, true peak

## Essence

The editor mixes the sound by ear and by the peak meter, and delivers the file — and the platform
(YouTube, a broadcaster) brings it to its own loudness by itself. If it was delivered louder than the
norm, the platform turns it down as a whole: the quiet places sink, and the dynamics for whose sake
the mix was made get eaten. If it is quieter, the sound becomes limp next to other people's clips.
The dBFS meter says nothing about this: it measures the PEAK, while the platform measures the
LOUDNESS — the perceived one, averaged over the whole material, per the ITU-R BS.1770 standard.
Two numbers are needed: the integrated loudness in LUFS (to hit the target) and the true peak in
dBTP (not to exceed the ceiling), plus a way to adjust the first in one action rather than by picking
the fader by ear.

## Who has it and how

The question for everyone is one: **where does the meter live, is there normalization in one action,
and is loudness measured at EXPORT** (that is, for the file that will actually go to the platform).

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Loudness Meters (`View → Window → Loudness Meters`), loudness log | the meter window: M (400 ms), S (3 s), I (for the whole program), LRA and True Peaks. Two modes — "EBU R 128 Mode" (target −23 LUFS) and "ATSC A/85 Mode" (target −24 LUFS). Appeared in Vegas Pro 13 build 290 (17.04.2014): "Added loudness meters and logging". **The only one of the five that writes loudness at render**: the checkbox "Save loudness log next to media file" in Render Options puts a `<name>_loud.txt` next to the file with the file name, format, meter mode and values over the whole length; a separate command `Tools → Generate Loudness Log` — for files that are already compressed. There is no one-action LUFS normalization: event normalization in Vegas works by peak/RMS, not by BS.1770 [NOT VERIFIED against the primary source] |
| Adobe Premiere | 26.x | Loudness Meter / Loudness Radar (effects), Essential Sound → Auto-Match, Loudness Normalization at export | the meter is an EFFECT that is placed on the master bus (usually through an FX slot of the track mixer), not a window of the program. Loudness Radar is licensed from TC Electronic and **does not work on Apple silicon**, therefore since 2021 on M-machines the Loudness Meter is the only way to see the average loudness. One-action normalization is Essential Sound → Auto-Match, but the target is HARD-CODED at −23 LUFS and is not editable in a field. At export (Adobe Media Encoder, the Effects tab) there is Loudness Normalization: a choice of ITU BS.1770-3 / ATSC A/85 / EBU R128, a True Peak Limiter with a recovery time, and the checkbox "Write Loudness Report" |
| DaVinci Resolve | 21 | Fairlight: loudness meters in the monitoring panel, loudness graph on the timeline, `Normalize Audio Levels`, `Analyze Audio Level` | the meter is built into the Fairlight page: "Loudness meters located at the right side of the monitoring panel let you quickly assess program loudness. Just click the options menu to choose your preferred metering standard." The meters are colored: "blue if levels meet the selected standard, yellow for tolerance and red when levels exceed tolerance". Plus a live loudness graph of the master bus RIGHT ON THE TIMELINE ("updates live during playback, so you'll know exactly where the issues are"), plus offline analysis via right-click on a clip. One-action normalization exists — `Normalize Audio Levels` "automatically adjusts the level of clips to a specific target level, and you can choose the method used to analyze each audio clip's levels"; the methods include variants of ITU-R BS.1770 alongside peak ones |
| Final Cut Pro | 12.3 | no loudness meter; "Loudness" is a compressor | the FCP help does not contain the words LUFS/LKFS/R128/true peak at all. "Analyzes audio for hum, noise, and loudness" — is a problem detector, not a meter; "Loudness: Improves the main audio signal and makes it more uniform" with sliders Amount and Uniformity, that is, compression, not bringing to a target number. Larry Jordan (2017): "The closest we come to measuring LUFS is the Multimeter in RMS view. However, there's a significant difference between RMS and LUFS, so this should not be considered accurate." The standard path is a third-party plugin or Compressor |
| After Effects | 26.x | none | the Audio panel shows the level (peak/VU), there is no loudness measurement per BS.1770; users attach third-party VST/AU plugins such as Youlean Loudness Meter [NOT VERIFIED against the primary source: helpx.adobe.com does not open from this environment]. For AE this is consistent: it is a layer compositor, delivering the master is not its task |

## The trick behind it

**1. A standard is not a scale but a rule of WHAT NOT TO COUNT.** The naive "average loudness" and
LUFS give different numbers not because of the K-filter, but because of gating. BS.1770 cuts the
material into blocks of `Tg` = 400 ms with 75 % overlap, computes the loudness of each block, and then
applies TWO gates in a row: the absolute Γa = −70 LKFS throws out silence, and then the relative Γr is
computed FROM THE MATERIAL ITSELF — it is the loudness obtained with the one absolute gate, minus
10 LKFS — and throws out everything quieter than it. An unfinished block at the end of the interval is
not used at all. Hence a non-obvious consequence: the selection threshold depends on the content, so
inserting a loud stinger can raise Γr so much that the quiet scenes drop out of the count, and the
integrated loudness shifts "in the wrong direction". And second: **on one and the same meter two numbers
are computed with DIFFERENT gates.** LRA (EBU Tech 3342) takes a 3 s window, and its relative gate is
−20 LU (not −10), and the result is the difference between the 10th and 95th percentiles of the
distribution: "The lower percentile of 10%, can, for example, prevent the fade-out of a music track
from dominating Loudness Range. The upper percentile of 95% ensures that a single unusually loud sound,
such as a gunshot in a movie, cannot by itself be responsible for a large Loudness Range." None of the
product feature lists has this — they say "has an R128 meter".

**2. True peak is also not exact, and −1 dBTP was chosen precisely for this reason.** Our meter is
exactly the "peak-sample meter" about which BS.1770-5 (Annex 2, Attachment 1) writes: "Peak meters in
digital audio systems often register 'peak-sample' rather than 'true-peak'… they do not always register
the true-peak value of the audio signal", and it lists three symptoms: the readings jump from run to run,
overload happens on a file where NOT ONE sample is close to full scale, and transients are undercounted
"commonly several dBs". The cure is oversampling: an attenuation of 12.04 dB, ×4 (48 → 192 kHz), a 48th-order
FIR interpolator with 4 phases, the modulus, `20·log10`, and back +12.04 dB. But even after this the meter
undercounts: the recommendation itself contains a table of the maximum shortfall, and at ×4 it is **0.554 dB**
(f_norm = 0.45) and **0.688 dB** (f_norm = 0.5) — ×8 gives 0.136/0.169. So the −1 dBTP ceiling is not a "round
number for a margin", but almost exactly the error of the ×4 meter, which the standard itself permits.

**3. Even where loudness is measured at render, the WRONG file is measured.** Vegas writes a loudness log,
but according to its own help, "Loudness logging is performed after the plug-in chain, but before any codec
is applied", and for compressed files it offers a separate command `Tools → Generate Loudness Log`. That is,
the number in the log is the loudness of the signal handed to the codec, not of what comes out after AAC;
the true peak can drift after lossy encoding. The very same boundary will be ours if we measure in
`render_raw_audio_range` (see below) — and it is more honest to say this in the interface than to pretend
that the output file was measured.

## Pros

- Gating makes the number comparable between materials of DIFFERENT length and structure: a 15-second
  clip and a one-and-a-half-hour film are measured the same way, and the leading and trailing silence do
  not affect the result. That is exactly why one number is enough for a platform to level the whole catalog.
- The unit is linear in decibels (1 LU = 1 dB), so bringing to a target is ONE multiplication by a constant:
  the mix is not touched, it just shifts as a whole. It follows that "normalize" needs no new effect and fits
  the already existing gain field.
- The measurement is a pure function of the samples: the same material always gives the same number. So it
  is not an opinion but a check, and it can be closed by a regression test with reference signals (EBU Tech
  3342 publishes outright a table of test signals with tolerances of ±1 LU).
- True peak measures a defect that does NOT YET exist in the samples: the overload that will arise for the
  listener after the DAC or re-encoding. A peak meter physically cannot see this, no matter how much it is
  improved.

## Cons

- The integrated loudness by definition requires a pass over ALL the material: a partial answer does not
  exist. Until one has played through or computed it offline there is no number, and a number that appears
  only after a full pass is a number that people forget to look at.
- The selection threshold is taken from the material itself (Γr = absolutely-gated loudness − 10 LKFS), so
  editing one piece changes which OTHER pieces get into the count. The meter stops being "the sum of what is
  heard", and in editing this looks like unpredictability.
- Bringing to a target with one gain also raises the peaks: for a loud mix "normalize in one action" either
  misses −1 dBTP, or drags a limiter behind it, and the limiter ALREADY CHANGES the mix. An honest button must
  say that it has hit the ceiling, rather than quietly switch the dynamics on.
- Targets differ between platforms, and most of them are not published by the platforms: broadcasting is set by
  standards (EBU R 128 — −23 LUFS ±0.5 LU, −1 dBTP; ATSC A/85 — −24 LKFS), while the streaming figures (of the
  order of −14 LUFS for YouTube and Spotify, −16 LUFS for Apple) are known from third-party measurements [NOT
  VERIFIED against the primary source]. You deliver by a number that someone else measured.
- LRA lies on short material: EBU Tech 3342 explicitly warns that for very short programs and for programs with
  leading/trailing silence the values come out "misleadingly high". Showing LRA on a 10-second event is harmful.

## What users say

> Premiere Pro doesn't allow any adjustability towards the "Loudness Match"
> feature of the Essential Sounds panel. A -23.00 LUFS is great for broadcast,
> but for content creators, social media, or web-use is completely useless.
> — efficient_person1243, 21.05.2026, https://community.adobe.com/questions-729/auto-match-loudness-has-no-adjustability-1624392

> I know of several users who need to make different loudness levels of the same
> content for different national market regulations or different distributor
> requirements. So they really need to grab some of the "upper" audio effects or
> send to Audition or ProTools due to this issue.
> — R Neil Haugen (Adobe Community Legend), 21.05.2026, https://community.adobe.com/questions-729/auto-match-loudness-has-no-adjustability-1624392

The diagnosis of both replies is the same, and for us it matters more than the very fact that a meter exists:
**normalization without a choice of target is useless.** A button "bring to the standard" with a hard-coded
number serves exactly one scenario (broadcasting) and pushes everyone else into another program.

## How it fits Kinotrain
CAPABILITY: openspec/specs/audio-render/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
The LUFS/peak meter is computed by the offline service on f64 sums; the export measures with the same meter; normalization is a command, not an automatic action.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages; the first stage ("measure at export and print at the end") is by itself S |
| Where the main work is | kt-engine (`export.rs` — an accumulator during the pass; `player.rs` — the live meter next to `Meters`), kt-app (`meters.rs` + `render_dialog.rs`), kt-media (`index.rs` — the source loudness in the peak pass), kt-model (the normalization target, if it is stored in the project) |
| Project format | not touched, if the target is not stored in the project; storing a target value and a mode (R128/ATSC/own number) is a new field in `Project`, a version of the kt envelope |
| Tests that will have to be written | the reference signals of EBU Tech 3342 (six test cases, tolerance ±1 LU) and a sine 997 Hz at 0 dBFS → −3.01 LKFS; an intersample test: a signal whose sample peak is < 0 dBFS but whose true peak is > 0; unchanged GOLDEN_AUDIO with the measurement enabled; a t38-like test "the display of loudness does not hold 60 fps" |
| Depends on | nothing; of the same class "offline analysis" — F-AUD-003, F-SPD-004 |
| Breaks compatibility | no |

## Verdict

**We take it, but not all of it and not at once.** The strong and cheap part is to measure the integrated
loudness, LRA and the true peak RIGHT ON THE EXPORT PASS (`render_raw_audio_range` already runs over the
whole material) and print the result at the end of the render next to the frames and the encoder; this is
a stage S, it requires neither a new UI, nor a format, nor a change of the signal, and at once closes the
main pain — "delivered and learned about the problem from the platform". A fair caveat: Vegas has been doing
this since 2014 (a loudness log at render), so the idea is not ours — but for us it comes cheaper, because it
does not require a separate analysis pass.

The live EBU mode of the meter (M/S/I/LRA/dBTP in the panel) is a separate stage and a separate discussion
about the place on the screen. "Normalization in one action" is taken only together with a CHOICE OF TARGET:
both quotes above are about the fact that a button with a hard-coded −23 LUFS is useless to everyone except
broadcasters, and pushes the user into another program.

## Sources

Primary sources (downloaded and read in full):

- Rec. ITU-R BS.1770-5 (11.2023) — the K-filter (Tables 1, 2), a 400 ms block with 75 % overlap, the gates
  Γa = −70 LKFS and Γr = −10 LKFS, channel weights, Annex 2 "Guidelines for accurate measurement of
  'true-peak' level" (12.04 dB, ×4, a 48th-order FIR with 4 phases) and the table of shortfall at different
  oversampling factors: https://www.itu.int/dms_pubrec/itu-r/rec/bs/R-REC-BS.1770-5-202311-I!!PDF-E.pdf
  (accessed 2026-09-07)
- EBU Tech 3342 v4 (11.2023) — Loudness Range: a 3 s window, ≥10 Hz, the cascaded gate
  −70 LUFS / −20 LU, the 10 % and 95 % percentiles, the table of test signals:
  https://tech.ebu.ch/docs/tech/tech3342.pdf (accessed 2026-09-07)
- EBU R 128 (summary): −23 LUFS ±0.5 LU, −1 dBTP, M/S/I:
  https://en.wikipedia.org/wiki/EBU_R_128 (accessed 2026-09-07)

Editors:

- VEGAS Pro, official help: the Loudness Meters window (M/S/I/LRA/True Peaks, the EBU R 128 and ATSC A/85
  modes) — https://help.magix-hub.com/video/vegas/22/en/content/topics/9-audio/loudness_meters.htm ;
  logging at render, "after the plug-in chain, but before any codec is applied", `Tools → Generate Loudness
  Log`, the file `_loud.txt` — https://help.magix-hub.com/video/vegas/22/en/content/topics/9-audio/loudness.htm ;
  "Save loudness log file next to media file" in Render As — https://help.magix-hub.com/video/vegas/21/en/content/topics/render.htm ;
  "Added loudness meters and logging" (Vegas Pro 13 build 290, 17.04.2014) —
  https://forum.borisfx.com/t/vegas-pro-release-history/24284 (all — 2026-09-07).
  The help was checked along branches 22/21: the pages for 2026.0 did not open at these addresses; we
  assume the behavior has not changed [NOT VERIFIED]
- DaVinci Resolve, the Fairlight page on blackmagicdesign.com (the quotes about loudness meters in the
  monitoring panel, the color indication, the loudness graph on the timeline and offline analysis via
  right-click): https://www.blackmagicdesign.com/products/davinciresolve/fairlight (2026-09-07). The wording
  about `Normalize Audio Levels` is taken from the Resolve 18 manual via a search snippet — the manual PDF
  itself does not open from this environment, the composition of normalization modes [NOT VERIFIED]
- Final Cut Pro, the official guide: "Analyzes audio for hum, noise, and loudness" —
  https://support.apple.com/guide/final-cut-pro/audio-analysis-options-verb6acaf60/mac ;
  "Loudness: Improves the main audio signal and makes it more uniform", the Amount/Uniformity sliders —
  https://support.apple.com/guide/final-cut-pro/enhance-audio-verc1fab873/mac
  (2026-09-07). The words LUFS/LKFS/R128 do not occur in the guide
- Adobe Premiere: helpx.adobe.com does not open from this environment, therefore the device of the meter and
  Loudness Normalization at export are taken from Larry Jordan (the meter is an effect on the master bus, Loudness
  Radar does not work on Apple silicon): https://larryjordan.com/articles/premiere-pro-new-loudness-meter/ (16.05.2021)
  and https://larryjordan.com/articles/adobe-media-encoder-loudness-normalization/
  (ITU BS.1770-3 / ATSC A/85 / EBU R128, True Peak Limiter, Write Loudness Report); the quote about the RMS
  multimeter in FCP — https://larryjordan.com/articles/why-lufs-and-lkfs-are-essential-to-video/
  (19.02.2017). The fixed −23 LUFS of Auto-Match is confirmed by the Adobe community thread (quotes in the section above)
- The target values of streaming platforms (≈ −14 LUFS for YouTube/Spotify, ≈ −16 LUFS for Apple) are NOT
  confirmed by a primary source: the platforms themselves do not publish them, the available sources give
  estimates of third-party measurements (Larry Jordan, 2017: "YouTube normalizes between -12 and -14 LUFS").
  Marked `[NOT VERIFIED]` in the text
- **A correction to the premise of the wave:** the Vegas forum at `vegascreativesoftware.com/us/forum` no longer
  exists — it redirects to the Discourse `forum.borisfx.com/c/vegas-pro/55`, where as of 2026-09-07 there are
  only about 11 topics, and the historical archive of threads is unavailable. Live quotes about loudness could
  not be obtained from there; the part of `_editors.md` about "where to look" is outdated on this point

**Implemented in R.49** — item 370
(tests t403, t404, t405, t406, t407, t411, t412).
