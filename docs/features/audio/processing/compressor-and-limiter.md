---
id: F-AUD-002
title: Dynamics processing — compressor, limiter, gate, expander
status: done
priority: B
cost: M
category: audio/processing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-fx, kt-model, kt-engine, kt-app, kt-media]
invariants: [1, 2, 3, 5, 6, 11]
ui_rules: [13, 17, 20]
round: "R.48"
point: 338
tests: [322, 326, 334, 335, 336, 341]
depends_on: [F-AUT-002, F-CLR-006]
trackers: [kde-bugzilla, kde-invent, olive, openshot, mlt, pitivi]
checked: 2026-09-14
---

# F-AUD-002 — Dynamics processing — compressor, limiter, gate, expander

## Essence

One and the same technique: the volume is adjusted not by the hand but by the signal itself. A detector
measures the level, the "input → output" curve decides by how many dB to press, and smoothing
(attack/release) decides how fast. A compressor presses down what is LOUDER than the threshold; a gate and
an expander push down what is QUIETER than it; a limiter is the same compressor, but with the promise "not
one sample above the ceiling will be let out". The editor sees this as "the lines do not jump from shot to
shot", "the noise in the pauses between phrases has disappeared" and "the export does not clip".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Track Compressor · Track Noise Gate · Graphic Dynamics · Wave Hammer · Multi-Band Dynamics | ordinary plugins in a chain, and there are several levels of chains: "Audio effect settings are displayed when you click the button on a track, bus, or assignable FX control" — plus the level of the event (Event FX, F-MPH-005). A track plugin acts on everything: "Track-level DirectX or VST plug-ins are applied to every event on the selected track". The parameters of track/bus/assignable effects are automated by envelopes. [NOT VERIFIED: the levels of chains are from the official help, but the NAMES of the devices are assembled from forum posts and the wiki; a named list of audio plugins was not found in the Vegas 22/23 help, the composition of the 2026 line was not checked] |
| Adobe Premiere | 26.x | Dynamics / Hard Limiter / DeEsser effects + the Essential Sound panel with a Ducking block | the effects are live, in the stack of a clip or track; but ducking is NOT live processing: the Generate Keyframes button places a volume effect on the music clip and writes keys into it. Regeneration overwrites the previous ones: "Each time you click Generate Keyframes, existing keyframes are replaced with new keyframes based on the current ducking settings" [NOT VERIFIED against the primary source: helpx.adobe.com does not open from this environment, the text is taken from a search snippet] |
| DaVinci Resolve | 21 | Dynamics in the channel strip (Expander/Gate + Compressor + Limiter) + Ducker in Track FX | the dynamics is not a plugin but a PART OF THE CHANNEL: "All channel strips include a fader…, along with dynamics and a 6 band parametric equalizer on every channel", and "Double clicking the dynamics area will open the dynamics window featuring an expander/gate, compressor and limiter dynamics processing". Ducking is a live sidechain: "The track FX ducker plug-in uses the signal from one or more dialogue tracks to automatically control volume reduction on a target track" |
| Final Cut Pro | 12.3 | a set of Logic effects in the "Levels" group: Compressor, Limiter, Adaptive Limiter, Expander, Noise Gate, Multipressor, DeEsser 2, Enveloper, Spectral Gate, Surround Compressor | effects on a clip/role; there is no concept of a bus or a track (F-MPH-005). The gate has both a lookahead and an external key input: "When no external side chain is selected, the input signal is used as the side chain" |
| After Effects | 26.x | none | the list of audio effects contains only Backwards, Bass & Treble, Delay, Flange & Chorus, High-Low Pass, Modulator, Parametric EQ, Reverb, Stereo Mixer, Tone — no compressor, no limiter, no gate [NOT VERIFIED against the primary source: the list page on helpx is unavailable, the composition is assembled from a snippet] |

## The trick behind it

**1. A limiter must see the future, so it is not an effect but a DELAY, and the manufacturer writes this in
plain text.** A compressor reacts AFTER the peak has already passed through it: while the detector raises the
control signal, the first milliseconds of the peak get out untouched. A true ceiling does not hold this way,
so the limiter acquires a lookahead — "Adjust how far ahead (in milliseconds) Limiter analyzes the audio signal.
This enables it to react earlier to peak volumes by adjusting the amount of reduction" (Apple, the Limiter help).
The price is named there too: "Lookahead causes latency, but this has no perceptible effect when you use Limiter
as a mastering effect on prerecorded material", and for Adaptive Limiter Apple goes as far as an operating
instruction: "Using Adaptive Limiter adds latency when the Lookahead parameter is active… Bypass Adaptive
Limiter while recording". The key subtlety that this paragraph hides: **delay is NOT a tail.** A tail (reverb)
extends the END of an event and moves nothing; a delay shifts the WHOLE output of the effect, that is, it
desynchronizes this track both with the video and with the unprocessed neighbors. In a DAW this is cured by
plugin delay compensation; in NLEs PDC is either absent or partial and tied to the bus routing mode — according
to discussions, Fairlight's compensation of track instances works in Flexi Bus and does not work in Fixed Bussing
[NOT VERIFIED: forum.blackmagicdesign.com returns 403 in this session, the fact is taken from a search snippet of
thread 145413].

**2. A gate has not one threshold but two, and this is the only thing that distinguishes a gate from "silence on
condition".** Hysteresis: "Set the difference (in decibels) between the threshold values that open and close the
gate" (Apple, the Noise Gate help). With one threshold a gate chatters on a noise shelf — it opens and closes dozens
of times a second on one and the same noise; with a second threshold (below the first) the decision becomes stable.
Also there is Hold — "Set the time the gate remains open after the signal falls below the threshold": the gate
cannot be closed at once, otherwise it cuts off the tails of words. Neither hysteresis nor hold is present in any
marketing list, and without them a gate is unusable for speech.

**3. Sidechain and "automatic ducking" are TWO DIFFERENT solutions of one task, and they diverge not by quality but
by architecture.** In FCP's gate the detector input is a separate signal, which by default is simply shorted to the
main one: "When no external side chain is selected, the input signal is used as the side chain". The Ducker in
Resolve is the same principle at the level of tracks: "uses the signal from one or more dialogue tracks to
automatically control volume reduction on a target track". Premiere took a different road: analysis + a GENERATED
ENVELOPE. The mechanics can be seen from the inside of someone else's bug — "an amplify audio effect is added to the
music clip, and no keyframes are generated" (that is, normally the feature places a volume effect and writes keys
into it). The difference is decisive: the envelope is edited by hand, delays nothing, does not require the effect
to have a second input, and is computed once — but it is a SNAPSHOT, and after moving the music or re-editing the
lines it stays old. A live sidechain is always up to date, but requires the effect to have a second input and the
project to have buses and sends. This is exactly the same pressure point against which F-CLR-006 ran ("the key input
is a full-fledged signal, not a parameter inside the effect"), only reached from the audio side.

## Pros

- The detector and the gain stage are separated, therefore a compressor, an expander and a gate are ONE DSP with
  a curve bent in different directions from the threshold: one state, one set of knobs, three effects in the
  catalog. A limiter is the same object with ratio → ∞ and the lookahead switched on.
- A lookahead does not "improve the sound" but changes causality: the reduction of gain begins BEFORE the peak,
  so the ceiling holds without distorting the front. This is exactly why a limiter without lookahead and a limiter
  with it are different devices, not a setting.
- The dynamics in the channel strip (Resolve) sits at a fixed place in the chain: it cannot be accidentally dragged
  after a plugin, and "why is it quiet" is looked for in one and the same place on any channel.
- Envelope ducking gives the editor an object that he can read and edit: these are ordinary volume keys, not the
  opaque reaction of a device. Moreover it introduces no delays and requires no recalculation at export.
- Hysteresis and hold make the decision of the gate stable against noise of the same level as the threshold —
  without them a gate is not applicable to speech at all.

## Cons

- A lookahead is paid for with a delay, and the manufacturer advises SWITCHING OFF the device when recording. A host
  without compensation silently drifts the processed track out of step with the rest; a host that has it must postpone
  the whole mix by the maximum over the chains — that is, pay with monitoring delay for one device on one clip.
- Peak versus RMS is the source of the eternal "the compressor does not work, I can see the peak". An RMS detector
  legitimately does not react to a short spike: it measures energy over an integration window. The knobs, meanwhile,
  are the same, and the difference cannot be seen from them.
- The "musicality" of a compressor is the shape of the knee and the integration time of the detector, not a set of
  parameters. Two devices with identical threshold/ratio/attack/release sound differently, and they cannot be compared
  by a list of knobs.
- A program-dependent release in "analog" emulations makes the result a function of the material: the same clip, cut
  half a second earlier, gives a different envelope. For an NLE with a byte-exact render reference this is a direct conflict.
- A generated envelope becomes outdated silently. Regeneration overwrites manual edits entirely, rather than merging with them.
- Automation breaks in states nobody thought about: in Premiere ducking does not generate keys when the target clip is
  solo (confirmed by Adobe as a bug, see the quotes), and for some users it crashes the application.
- Dynamics at the level of a track/bus is invisible on the clip: you look at the event and do not understand why it is
  quieter than in the bin.
- The sidechain sits not in the device but in the HOST: Olive can load someone else's sidechain compressor, and it is still
  useless, because there is nothing to direct another track's signal into it with (Olive #1753). A device without
  addressing of the second input is not half of the feature, but zero.
- Dynamics devices break not in the algorithm but at the junction with the path. In Kdenlive, of four tasks about existing
  dynamics, two in a row are "Cannot set sink channel counts" for Compressor/Expander (#489276, #490884): the compressor
  must agree on the number of channels, and this fails more often than the DSP itself.
- The price of dynamics is paid in the bit depth of the chain, not by the effect: in MLT the sound inside the chain was
  silently reduced to 16 bit, and the same compression sounded "dirty" in Kdenlive and clean in Audacity (#1284). A device
  cannot be assessed in isolation from the point of summing.
- A limiter is asked for not as an effect on a clip but as a property of the EXPORT ("so that the export does not clip") —
  that is, the request comes to a level which the editor may not have at all.

## What users say

> the 'generate keyframes' button very briefly turns gray, an amplify audio
> effect is added to the music clip, and no keyframes are generated.
> — Logan_Barney5958, Adobe Support Community, 02.07.2025, https://community.adobe.com/t5/premiere-pro-bugs/essential-sound-ducking-won-t-generate-keyframes-when-target-clip-is-soloed/idc-p/15405003

> When I press 'Generate Keyframes', PPro crashes.
> — Stanley Arthur, same place, 03.07.2025, https://community.adobe.com/t5/premiere-pro-bugs/essential-sound-ducking-won-t-generate-keyframes-when-target-clip-is-soloed/idc-p/15405003

> I've checked with the team, and you are correct; it does appear to be a bug.
> I've written it up for our engineers.
> — Rach McIntire, Adobe Community Manager, same place, 07.07.2025, https://community.adobe.com/t5/premiere-pro-bugs/essential-sound-ducking-won-t-generate-keyframes-when-target-clip-is-soloed/idc-p/15405003

> I'd start with audio compression. I tend to use the Track Compressor plug-in,
> but you can also do compression via the Graphic Dynamics plug-in.
> — Brett Underberg-davis, Creative COW, 13.06.2009, https://creativecow.net/forums/thread/audio-difficulties/

> You can use the Track EQ to create a "notch" on the music track to keep the
> music from stomping on the VO.
> — Bob Peterson, Creative COW, 05.08.2010, https://creativecow.net/forums/thread/audio-compression-howae/

One should read this as follows. The first three quotes are not about a "bad algorithm", but about the fact that
Premiere's automatic ducking is a SEPARATE STAGE with its own state: it can fail to fire, overwrite other people's keys
and crash the host, while a live sidechain has nothing to break in these places. The last quote is valuable in reverse:
in a thread on "how to lower the music under the voice", an experienced person of 2010 advises cutting a NOTCH with an
equalizer instead of ducking — because Vegas still has no ducking mechanism, and people work around it by hand.

Live negative feedback about the Resolve Ducker and about the latency of the limiter could not be collected (the BMD
forum returned 403 in this session), but the trackers of the open-source editors answer the same question with a count —
and they answer more sharply, because there one sees not an opinion but the state of a task.

The master limiter is requested not as an effect but as a property of the EXPORT:

> I really miss a master limiter effect that'd allow to export videos without
> clipping in case something gets too loud.
> — Olive #2064, 14.10.2022, closed,
> https://github.com/olive-editor/olive/issues/2064

A live sidechain that has no addressing of a second input is useless together with the whole plugin — and this is exactly
the pressure point against which F-CLR-006 ran:

> However, there is no way to direct the sidechain from one of the other audio
> tracks into the plugin, thus rendering it useless.
> — Olive #1753, 29.09.2021, closed,
> https://github.com/olive-editor/olive/issues/1753

Dynamics devices are requested where they are absent in exactly the same words as in the forum threads of 2010 — only with a
ready workaround through another program:

> I miss a great feature in all NLE about audio, is the audio "autoduck"
> functionality. […] Is available in Audacity but I need to do the process OUT
> of the video editor and incorporate the result to the project.
> — OpenShot #2813, 29.05.2019, closed,
> https://github.com/OpenShot/openshot-qt/issues/2813

And where ducking is done by hand, it is done PRECISELY with an envelope — that is, by the path which for us is the cheapest:

> This way the super frequent performed manual audio ducking with keyframes, to
> lower e.g. music when a speaker is speaking […] would benefit from a massive
> workflow speed boost […]
> — Kdenlive (kde-invent) #1958, 27.01.2025, open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1958

The level of application is a separate request, and it sounds about dynamics literally:

> It would be very useful to be able to add effects on full layers for instance
> using one track for chroma-keying or compressor /limiter on audio track etc
> and not worry about adding effect to all clips in same track.
> — Pitivi #2472, 19.05.2020, open,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2472

And the price that the path pays, not the algorithm (invariant No. 1 of our engine is literally about this):

> I'd noticed that Kdenlive was producing quite rough audio output when applying
> effects (e.g. fade out / compression). The same compression algorithm was
> sounding clean on Audacity.
> — MLT #1284, 29.08.2026, closed,
> https://github.com/mltframework/mlt/issues/1284

SIGNAL: dynamics devices — 6 tasks in kde-bugzilla, `auto.?duck|\bducking\b|\blimiter\b|\bcompressor\b|noise gate|side.?chain`; share 0.1% against 0.2% in olive

The number is small, and small it is honestly: the topic "sound" has 2522 tasks over the eleven ranked trackers, of which dynamics is 12 tasks, that is, 0.5 % of the topic. The finding lies not in the magnitude but in the DIFFERENCE OF COMPOSITION at twice the share where the device is absent. Olive (there is no dynamics at all) — all three of its tasks are "give us": a limiter, an auto-ducker, a sidechain. Kdenlive (it has dynamics, via MLT/avfilter) has two tasks that are requests about auto-ducking (one of them, #392623, has hung CONFIRMED since 2018), and the remaining four are breakages of what already exists: the compressor "explodes when attributes changed" (#403650), "Cannot set sink channel counts" twice in a row for Compressor/Expander (#489276 and #490884), and distortion on a whole set of devices, including the limiter (#513447). That is, the class behaves not like a "rare exotic thing" but like this: while the device is absent — it is requested; as soon as it has appeared — it breaks at the junction with the path, not in the DSP itself. This is exactly why the only complaint about the sound (MLT #1284) turned out to be not about the algorithm but about the bit depth of the chain.

## How it fits Kinotrain
CAPABILITY: openspec/specs/audio-fx/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
The compressor and the limiter are nodes of the audio chain with a lookahead (PDC: the input runs ahead of the output), without a project master limiter.
ADDRESS: `src/libs/kt-engine/src/player.rs:375` — the class of MLT #1284 ("the chain was silently reduced to 16 bit, and the compression sounds dirty with a clean algorithm") is impossible with us, because invariant No. 1 holds f32/48k/2ch from decoding to output, and any deviation is caught by the byte-wise golden_export. But the class of Olive #2064 ("the export clips, and there is nowhere to put a limiter") IS possible, and nothing catches it: the only object named "master" is the MONITOR fader of the preview, `Player::set_master_gain` with the comment "Master volume of the preview" (`player.rs:375`), living in an atomic for the cpal callback; the export builds the same `AudioRenderer` without it at all (`src/libs/kt-engine/src/export.rs:81`, `:103`). That is, in the model there is NOT A SINGLE level shared by preview and export to which a clipping-protection limiter could be attached — and golden_export guards the bytes of what we computed and knows nothing about the existence of a ceiling. This is exactly why the master limiter in the "Estimate" stands at L, not M: the price is not in the DSP.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages: (1) envelope ducking, (2) compressor + limiter + gate as one family, (3) separation of pre-roll and tail. A master limiter raises it to L (it needs a level of the mix), the live sidechain — to XL (buses + format) |
| Where the main work is | kt-fx (DSP + pre-roll), kt-model (the `FxInstance` variant and a line in `params_for!`), kt-engine (`chain_tail` → two numbers, the prefetch window), kt-app (`fx_registry.rs`, i18n ×5, font subsets) |
| Project format | the version does not move: a new `FxInstance` variant is an additive change of the same class as `Gain.pan` (R.33). A foreign/unknown `kt:` effect no longer breaks loading; it is kept as foreign with a notice (`src/libs/kt-otio/src/convert.rs:1214`) |
| Tests that will have to be written | the byte-determinism of the compressor through seek/pre-roll (invariant No. 11); the compressor does NOT extend `ext_end` (the clip does not sound after its end); the limiter's ceiling holds on a synthetic peak; the lookahead does not shift the output (sample-exact match with the gain reference); the prefetch window covers the lookahead — zero underruns (invariant No. 5); ducking: the curve is deterministic and is built from the peak pyramid without decoding (invariant No. 8); plus t48 (registry), t100/t125 (glyphs and catalogs), a layout test on the effect line |
| Depends on | F-AUT-002 (the key domain — for the ducking envelope), F-CLR-006 (the second input: the same architectural pressure), F-AUT-003 (if ducking is wanted on a track rather than on a clip) |
| Breaks compatibility | no |

## Verdict

Take it, but as TWO DIFFERENT decisions, and not in one stage.

The first — automatic ducking by generating a `ParamCurve` on `Gain.db`: it closes the most frequent live complaint
("the music drowns out the voice"), does not touch a single invariant, introduces no delay, and is assembled almost
entirely from ready-made pieces (curves, analysis over the peak pyramid, the automation widget).

The second — compressor/limiter/gate as one family at the clip level, with a lookahead implemented by peeking into
the SOURCE (a forward pull), rather than by a delay of the output; along the way `chain_tail` is split into "how much
to warm up" and "how much to sound" — without this the compressor either loses determinism or sounds after the end
of the clip.

~~The live sidechain — NOT now: it needs a second input on the effect, buses and a format change, that is the same
XL wall as F-CLR-006, and the practical benefit is covered by the envelope.~~ **CANCELLED by the owner (R.63, confirmed in R.65):**
"Ducking → a LIVE SIDECHAIN COMPRESSOR… remove the snapshot entirely". The XL estimate did not hold up: buses were not
needed (the key is a TRACK, and the track stages appeared in R.62), the second input turned out to be an argument of
processing rather than a new graph, and the format change is one field by content (v12). It was done by the change
`sidechain-ducking-compressor`; the envelope snapshot was removed entirely, tests t767–t778, the live scenario
`src/scripts/live/sidechain.py`. The master limiter is a separate question: it runs into not the DSP but the fact that
the master fader today exists only in the preview (`src/libs/kt-engine/src/player.rs:375`) and does not reach the export.

## Sources

- Trackers (dumps of 08.09.2026 in `~/.cache/kinotrain/trackers/`, they do not go into the repository —
  there is no license, see `_trackers.md`): `kde-bugzilla` 6346 tasks, `kde-invent` 2217, `olive` 1853,
  `openshot` 4516 (the download was still running, the registry has 4907), `mlt` 601, `pitivi` 2672. The number
  and the shares of the `SIGNAL:` line are reproduced by `python3 src/scripts/tracker-rank.py --query
  'auto.?duck|\bducking\b|\blimiter\b|\bcompressor\b|noise gate|side.?chain'` on this composition; `match()`
  looks at the title, the labels and the component, therefore matches only in the body of a task do not count
  (quotes from bodies are given by name with links).
- https://www.blackmagicdesign.com/products/davinciresolve/fairlight — Fairlight: dynamics in the channel strip,
  the Ducker as Track FX (accessed 2026-09-07)
- https://support.apple.com/en-lamr/guide/final-cut-pro-logic-effects/lgex931a007e/mac —
  Limiter: the difference from a compressor, Lookahead, latency (2026-09-07)
- https://support.apple.com/en-au/guide/final-cut-pro-logic-effects/lgex931a149c/mac —
  Adaptive Limiter: "adds latency when the Lookahead parameter is active", Out Ceiling, True Peak (2026-09-07)
- https://support.apple.com/guide/final-cut-pro-logic-effects/noise-gate-controls-lgex931a1816/mac —
  Noise Gate: Hysteresis, Hold, Lookahead, Side Chain; there is also the whole list of the "Levels" group (2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/9-audio/audio_plug-ins.htm —
  Vegas chain levels: track / bus / assignable FX (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/9-audio/adding_audioeffects.htm —
  a track plugin acts on all events of the track; envelopes of parameters (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-bugs/essential-sound-ducking-won-t-generate-keyframes-when-target-clip-is-soloed/idc-p/15405003 —
  the ducking bug, the "volume effect + keys" mechanics from the inside, Adobe's confirmation (2026-09-07)
- https://creativecow.net/forums/thread/audio-difficulties/ — the set of Vegas dynamics plugins as seen by a practitioner (2026-09-07)
- https://creativecow.net/forums/thread/audio-compression-howae/ — presets of Track Compressor / Wave Hammer, bypassing ducking via a notch (2026-09-07)

Marked `[NOT VERIFIED]` and why: (1) the Adobe wording about Generate Keyframes and (2) the composition of the
After Effects audio effects are taken from search snippets — `helpx.adobe.com` does not open from this environment
("proxy refused the connection"); (3) the PDC behavior of Fairlight in Flexi Bus versus Fixed Bussing is from the
snippet of the forum.blackmagicdesign.com thread t=145413, the forum itself returned 403 on all URLs checked. The
Premiere and After Effects rows in the table "Who has it and how" rest on the official Adobe community and snippets
of the official help, and were not checked live.

**Implemented in R.47–R.48** — the compressor and the gate in R.47 (item 329), the look-ahead limiter and auto-ducking
in R.48 (items 337–338, 341). The live sidechain and the master limiter remain outside what is done — the reasons are
in the section "What gets in the way" above; the further life of the feature is in docs/porting-notes.md.
