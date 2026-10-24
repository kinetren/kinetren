---
id: F-AUD-001
title: Parametric equalizer
status: done
priority: A
cost: S
category: audio/processing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-fx, kt-model, kt-app, kt-engine, kt-otio]
invariants: [1, 2, 3, 11]
ui_rules: [13, 17, 20]
round: "R.48"
point: 347
tests: [322, 324, 348, 349, 350]
depends_on: []
checked: 2026-09-14
---

# F-AUD-001 — Parametric equalizer

## Essence

A tool for "making the voice audible": remove the air-conditioner hum from below, take the "barrel" out of speech, add intelligibility. The editor puts several bands on an event, each with three numbers — frequency, gain and width — and hears the result at once. Without an equalizer, the only available gesture over sound is to make it quieter, and quieter muddy speech does not become more intelligible. This is the most basic audio tool: it is present in all five core editors, including After Effects, which is not an NLE at all.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | VEGAS Track EQ (plus Paragraphic EQ / Graphic EQ in the plugin list) | Lives in the TRACK chain and is already there: every new audio track has three plugins by default — Noise Gate, Track EQ, Compressor, all in a neutral state. There are four bands (low, low-mid, high-mid, high), each with frequency, gain and Q, plus a frequency-response graph on top [NOT VERIFIED against the primary source: the magix-hub help describes only the Track FX button, the plugin's own page was not found]. A separate Event FX chain is placed on an event, and a second instance of the same plugin goes there |
| Adobe Premiere | 26.x | Parametric Equalizer and Simple Parametric EQ (Audio Effects → Filter and EQ); Essential Sound → Clarity → EQ | The ONLY one of the five where EQ is a regular effect: it is dragged onto a clip, a track or a submix, and is copied together with the clip's effects. In Essential Sound the same effect is reduced to a list of presets and a single Amount slider — a "recipe" instead of knobs |
| DaVinci Resolve | 21 | Fairlight EQ | Not a library effect but part of the channel: "All channel strips include a fader…, dynamics and a 6 band parametric equalizer on every channel", opened by double-clicking the EQ area in the mixer. Plus a SECOND one on every clip: "Each clip in the timeline has its own volume, pan, pitch and 6-band parametric EQ controls" |
| Final Cut Pro | 12.3 | Channel EQ and Linear Phase EQ (both from Logic) | 8 bands with hard-wired roles: 1 — high-pass, 2 — low shelf, 3–6 — parametric bells, 7 — high shelf, 8 — low-pass. An analyzer with a Pre/Post switch over the curve. Linear Phase EQ — the same interface and the same eight bands on a different engine |
| After Effects | 26.x | Parametric EQ, High-Low Pass, Bass & Treble | Old audio effects of a layer: keyframes work like for any property, but there is no mixer, no channels and no analyzer. Parametric EQ has three bands [NOT VERIFIED: helpx.adobe.com does not open from this environment] |

## The trick behind it

**1. One and the same Q knob does three different things on different band types, and on the gentlest slope it does nothing at all.** For a bell, Q is the ratio of the center frequency to the width: a higher Q means a narrower hump. A shelf has no width at all, and the same number controls the KINK at the cutoff: around ~0.7 the shelf gets an overshoot, that is, a dip appears next to the boost (and vice versa). An editor who "made the shelf narrower to be more precise" has in fact carved a hole in the neighbouring range. In cut filters, Q is the resonance at the cutoff frequency, and it exists only starting from the second order. Apple states this in plain text about its extreme bands: "The Q parameter of band 1 and band 8 has no effect when the slope is set to 6 dB/Oct" — 6 dB/oct is the first order, with one pole, so there is nothing to resonate. From this follows all the arithmetic of slope: one biquad = second order = 12 dB/oct, and 24/36/48 dB/oct are two, three, four biquads in a row, so the choice of slope in the interface literally means "how many filters to compute" and "how many degrees of phase to rotate" (by 90° per order around the cutoff).

**2. A linear-phase EQ is not "better" but a different trade-off, and its price is latency, which the manufacturer itself mentions first.** Any IIR equalizer is minimum-phase: change the frequency response and you change the phase too, there is no other way. Alone this is not audible, but it is audible when the processed signal is SUMMED with an unprocessed copy of the same signal: two microphones on one source, parallel processing, a clip duplicated on top of itself, M/S. The industry's answer is an FIR with a symmetric kernel: the phase does not turn at all, but there appear a constant delay (half the kernel length) and pre-ringing before a transient. Apple about its Linear Phase EQ: "perfectly preserves the phase of the audio signal", and right after it — "Linear Phase EQ also introduces greater amounts of latency. Therefore, it is strongly recommended that you use it for mastering previously recorded audio". The same place exposes the device from the inside: "uses a fixed amount of CPU resources, regardless of how many bands are active" — the cost of FIR convolution is the same per block whether one band or eight are enabled, exactly the opposite of IIR, where each band costs its own. For a video editor this trade-off is worse than for a DAW: a DAW has end-to-end plugin delay compensation and the editor looks at the latency figure, while an NLE must have sound match the picture frame by frame, and the live preview is driven by scrubbing and looping. A kernel of 4096 samples gives 2048 samples of delay — 43 ms at 48 kHz, which is more than one frame at 24 fps (41.7 ms) — and every seek must refill the FIR history anew.

**3. The place where the equalizer lives decides who controls it — and this is the source of the most frequent complaint.** In Resolve and Vegas the EQ is bolted to the channel: it cannot be dragged onto another clip, but it is always at hand and always at one point in the signal path. In Premiere it is an effect, and on a track it falls under the mixer's automation mode — the knobs obediently return to the recorded value, and the user sees an "equalizer glitch". Vegas, meanwhile, puts its EQ on every track IN ADVANCE and in a neutral position, so it pays for convenience by having three plugins always sitting in the project that do nothing. Resolve has two equalizers altogether — on the clip and on the channel — they look the same and stand at different points in the signal path, and this is a regular source of the question "why do I turn it and nothing changes".

## Pros

- A band is three numbers and two or three filter states, so the equalizer is fully described by the same parameter passport as any other effect: ranges, step, units, automation and clamping come for free, and nothing special for it is needed in the model.
- The state of a biquad is two numbers per channel, so "a model edit does not kill the tail" comes here for free: there is nothing to lose when rebuilding, and the transient is shorter than a millisecond.
- An IIR equalizer has zero latency, so it needs neither delay compensation nor a shift of the source window: output of frame N is computed exactly from frame N. This is the reason all five editors have a standard EQ that is IIR, and the linear-phase one is a separate "for mastering".
- The CPU cost is linear in the number of ENABLED bands: a disabled band costs zero, and this is visible in the UI as an honest link "enabled — paid".

## Cons

- Changing the frequency response of an IIR is inseparable from changing its phase, so an equalizer placed on one of two signals that are summed changes their sum more than its own curve shows — and the culprit looks not like the equalizer but like "the microphones are out of phase".
- Slope steepness costs phase: 48 dB/oct means eight orders of rotation at the cutoff frequency, hence "the HPF ate the punch", which forum posts describe in exactly the same words as the pre-ringing of linear phase.
- The analyzer over the curve provokes editing by eye: the spectrum shows where it is LOUD, not where it is BAD; speech always has a hump at 200–400 Hz, and a voice "levelled by the picture" sounds gutted. That is exactly why editors hide the recipe bands (hum 80–120, muddiness 200–400, presence 3–5 kHz) in presets with a single "how much" knob: Premiere in Essential Sound keeps a list of presets and an Amount slider, Vegas keeps a preloaded but neutral plug-in.
- There are always few bands: Fairlight has six, and requests for "more than 6 bands?" have hung on the BMD forum for years; FCP has eight, but the roles of the bands are hard-wired — six bells in a row cannot be rearranged to it.
- Q is one word for three different quantities (bell width, shelf knee, cutoff resonance), and on the gentlest slope it silently does not work. This is not a flaw of the interface, it is the physics of the filter showing through in the UI.

## What users say

> Yes, those three effects are set to "do nothing" by default but are
> very commonly used effects which is why they are there by default.
> — Edward Troxel, Creative COW, 25.01.2022, https://creativecow.net/forums/thread/default-audio-effects-track-when-open-any-file/

(About the three plug-ins — Noise Gate, Track EQ, Compressor — that Vegas puts on every audio track in advance.)

> Parametric equalizer glitch! it just records my every tweak and then
> goes back to where it was and then it plays back like this...
> — CTMPRE, Adobe Support Community, 23.09.2022, https://community.adobe.com/t5/premiere-pro-discussions/parametric-equalizer-glitch/m-p/13218990

(The reply of Phillip Harvey in the same thread: the cause is the automation mode in the Audio Track Mixer — "This setting is going to affect the behavior of your audio effect". An equalizer as a TRACK effect is subject to the mixer's automation, and the user reads this as the equalizer being broken.)

> the linear phase algo has a tendancy to introduce pre and/or post
> ringing - this is an oscilation that occurs before and/or after
> transients.
> — plexuss, KVR Audio, 22.09.2019, https://www.kvraudio.com/forum/viewtopic.php?t=532089

> Not while tracking or writing/arranging, though, as it adds latency
> which isn't good for getting real-time playability from your
> instruments.
> — simon.a.billington, KVR Audio, 06.10.2019, https://www.kvraudio.com/forum/viewtopic.php?t=532089

> My best advice is to mix with your ears, not your eyes.
> — Larry Jordan, larryjordan.com, 23.07.2025, https://larryjordan.com/articles/use-channel-eq-to-improve-voice-quality-in-apple-final-cut-pro/

(From an article on Channel EQ in Final Cut Pro — that is, on an equalizer with a built-in analyzer.)

## How it fits Kinotrain
CAPABILITY: openspec/specs/audio-fx/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
A parametric EQ — biquads in the node chain; the frequency-response graph is computed from the kernel coefficients, not drawn separately.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage of the round for the core (DSP + model variant + registry + field grid). A graph with points is a separate stage; it is not required to take it in the same pass |
| Where the main work is | kt-fx (the biquad and its place in `ClipProcessor`), kt-model (`params_for!`), kt-app (registry, fields, i18n catalogs) |
| Project format | not touched: a new `FxInstance` variant does not bump `KT_META_VERSION` (`src/libs/kt-otio/src/meta.rs`, precedent Pan3d) |
| Tests that will have to be written | per the kt-fx map in the root `CLAUDE.md`: `kt-engine/tests/fx.rs` (incl. t31 and t213 — audio automation), `warp_prefetch`, `reverse`, `loop_fx` (t166 seam), `golden_export` (the reference must stay the same — there is no effect in the reference project), kt-fx unit tests, `kt-fx/tests/{integral_map,material_range}.rs`. Plus our own: the biquad's frequency response against the analytic formula at control points, silence at zero gain byte-for-byte, no click on frequency automation, `t100` (glyphs) and `t125/t128` for new catalog strings |
| Depends on | nothing. F-AUD-004 (buses and track FX) is not needed: it would only give the equalizer a second PLACE — the channel instead of the event |
| Breaks compatibility | no |

## Verdict

Take it. This is probably the cheapest real feature in the whole base: the path is already f32/48k, the FX state dispatcher already exists, the parameter passport and automation come for free from `params_for!`, the project format does not move, there is no tail, and the source window does not change. Start with the IIR equalizer on an EVENT and with a grid of numbers driven by `ParamSpec`; do not consider the linear-phase variant at all while the engine has no delay compensation — Apple itself recommends its Linear Phase EQ only "for mastering previously recorded audio", while we have a live preview and frame-accurate sync with the picture. The round must decide two things aloud: one band = one chain slot, or N bands in one effect; and what to do about the fact that `ClipProcessor` currently ignores the order of slots (rule 17). A graph with draggable points is a second pass, and only as a second VIEW of the same `ParamSpec`, following the pattern of `src/apps/kt-app/src/automation.rs`.

## Sources

- https://www.blackmagicdesign.com/products/davinciresolve/fairlight (accessed 2026-09-07) — official Fairlight page, Resolve 21: a 6-band parametric EQ on every mixer channel and a separate 6-band EQ on every clip
- https://support.apple.com/guide/final-cut-pro-logic-effects/channel-eq-controls-lgex92fd757d/mac (2026-09-07) — makeup of the eight bands of Channel EQ, the Pre/Post analyzer, "The Q parameter of band 1 and band 8 has no effect when the slope is set to 6 dB/Oct"
- https://support.apple.com/en-ca/guide/final-cut-pro-logic-effects/lgex92fd69c2/mac (2026-09-07) — Linear Phase EQ: phase preservation, fixed CPU cost regardless of the number of bands, "introduces greater amounts of latency"
- https://support.apple.com/guide/final-cut-pro-logic-effects/intro-to-equalizers-lgex25ccd530/mac (2026-09-07) — list of equalizers available in FCP
- https://creativecow.net/forums/thread/default-audio-effects-track-when-open-any-file/ (2026-09-07) — the three default plug-ins on a Vegas audio track
- https://community.adobe.com/t5/premiere-pro-discussions/parametric-equalizer-glitch/m-p/13218990 (2026-09-07) — an equalizer on a track under the mixer's automation mode
- https://www.redsharknews.com/how-to-use-the-essential-sound-panel-in-adobe-premiere-pro (2026-09-07) — Clarity → Dynamics/EQ/Vocal Enhancer, "Premiere Pro offers a list of presents and a slider to adjust the amount"
- https://www.kvraudio.com/forum/viewtopic.php?t=532089 (2026-09-07) — pre-ringing and delay of the linear-phase EQ, when it is taken and when it is not
- https://larryjordan.com/articles/use-channel-eq-to-improve-voice-quality-in-apple-final-cut-pro/ (2026-09-07) — "mix with your ears, not your eyes" about Channel EQ in FCP

Verification limits: `helpx.adobe.com` and `reddit.com` do not open from this environment, `forum.blackmagicdesign.com` returned 403 on access, the Vegas forum has moved to `forum.borisfx.com` and its search returns an empty page. Therefore the Premiere and After Effects rows rest on adobe.com / the Adobe community and secondary sources; the number of bands in After Effects' Parametric EQ and the makeup of the VEGAS Track EQ bands are marked `[NOT VERIFIED]` right in the table. The names, number and roles of the bands in Resolve and FCP are taken from primary sources (blackmagicdesign.com, support.apple.com).

**Implemented in R.47–R.48** — the three-band EQ came in R.47 (item 327), the interactive frequency-response graph in R.48 (item 347, t348–t350). Further life of the feature — its OpenSpec capability.
