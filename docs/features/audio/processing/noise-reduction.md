---
id: F-AUD-003
title: Noise reduction and speech restoration
status: candidate
priority: B
cost: L
category: audio/processing
editors: [premiere-26, after-effects-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: no
crates: [kt-fx, kt-engine, kt-model, kt-otio, kt-media, kt-app]
invariants: [1, 2, 3, 5, 6, 8, 11]
ui_rules: [2, 12, 13, 16, 20]
round: null
point: null
tests: []
depends_on: [F-MSK-005, F-MSK-006, F-SPD-004]
checked: 2026-09-07
---

# F-AUD-003 — Noise reduction and speech restoration

## Essence

Speech recorded on location comes together with the air conditioner, the street, the preamp hiss and the room echo. The editor asks the program to "remove the noise and keep the voice". Historically they did it by hand: they showed the program a piece of PURE noise, the program remembered its spectrum and subtracted it from the whole clip. Today, in four of the five editors, this is one "Enhance" button and a strength slider. Next to it sit two separate tasks: the de-esser (removing hissing "s" sounds) and de-reverb (removing the room, not the noise).

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Premiere | 26 | Enhance Speech (in Premiere Beta 27.0 — Enhance Audio) | An `Enhance` button in the Essential Sound panel on a clip marked as Dialogue, plus a `Mix Amount` slider (blends the raw sound into the processed one). Version V1 is computed LOCALLY: "Unlike Adobe Podcast's service, no internet connection is required". Beta 27.0 (02.09.2026) added Enhance Audio — splitting the recording into layers "speech, music, ambience, sound effects, and reverb" — and split the path in two: source separation on the device, while the V2 model runs in the CLOUD in Firefly, "two generative credits per minute" |
| After Effects | 26 | none | There is no audio restoration at all. Among the audio effects there is a basic set such as `Gate` ("silences audio that falls below a certain volume threshold"); sound cleanup in the Adobe ecosystem is handed to Audition, not AE |
| DaVinci Resolve | 21 | Fairlight FX: Noise Reduction, De-Esser, De-Hummer + AI: Voice Isolation, Dialogue Separator, Music Remixer | TWO generations side by side, and this is deliberate. The classic one: "The noise reduction plug-in automatically detects noise in a selection or you can teach it by selecting noise and clicking on "learn"" — that very profile. The AI layer runs on the local DaVinci Neural Engine and works in real time in the Fairlight chain, but is marked as Studio: "DaVinci Neural Engine AI voice isolation, music remixer and dialogue separator". There is no de-reverb as a separate effect; the closest is Dialogue Separator, which moves the reverb into a separate stem |
| Final Cut Pro | 12.3 | Audio Enhancements: Voice Isolation, Noise Removal, Hum Removal | A section in the audio inspector, switched on by the `Enhance Audio` button or by hand. `Voice Isolation` — "Prioritizes human voices over other parts of the audio signal" with an `Amount` slider; separately `Background Noise Removal` ("Reduces background noise") with its own `Amount`, and `Hum Removal` ("at either 50 or 60 Hz"). The key detail of the design: "Audio enhancement works on the component level, not the clip level" |
| VEGAS Pro | 2026 | NO built-in spectral noise reducer | Audio effects are a DirectX/VST chain in the Plug-In Chooser, and a noise reducer comes as a plugin (historically a separately sold Sony/MAGIX Noise Reduction, today third-party ones). All of the 2026 AI went into sound, but not into the noise reducer: "Fully Offline Speech-To-Text" and "Fully Offline Text-To-Speech", "No internet connection required", both Subscription Only. AI sound cleanup is mentioned only for the senior Plus/Ultimate editions in a SECONDARY review — `[NOT VERIFIED]` against Boris FX documentation |

## The trick behind it

**1. "Musical noise" is not an implementation defect but a property of the method, and it grows exactly with aggressiveness.** Spectral subtraction works bin by bin: an estimate of the noise is subtracted from the magnitude spectrum of the frame, and the result is clamped to a floor. The estimate is an AVERAGE, while the real noise in each bin dances randomly around it. The bins where the instantaneous noise happened to exceed the average estimate survive — and every surviving bin sounds like a short sinusoid that appears and disappears at every step of the window. Hence the industry names for the artifact: "twittering or birdies" in one place, "random bursts of very short tones at random frequencies, sometimes called "musical noise", "bird song" or "tinkly-bells"" in another. The deeper you press, the more bins end up right at the threshold, and the more such "birdies" there are. That is why the cure in classic noise reducers is not a "better algorithm" but SMEARING the decision: `Frequency Smoothing (bands)` "spreads the noise reduction into the specified number of neighboring bands" and `Sensitivity`, "how much of the audio will be considered as noise". So the noise reducer's knobs are not about quality but a TRADE-OFF, exactly like `Crop Less ↔ Smooth More` on the stabilizer (F-MSK-006).

**2. The neural-network path changed the CLASS of the operation, and Adobe's history proves it in both directions.** Enhance Speech was born in the cloud (the Adobe Podcast web service), moved to the user's machine in September 2023 for the sake of Premiere — "has been optimized to perform locally on your machine, allowing you to run Enhance Speech on an unlimited number of clips" — and in September 2026 went back: "The source separation runs on-device, while Enhance Speech V2, our highest-quality voice model, runs in the cloud as part of Firefly", "two generative credits per minute". An effect that was computed by a button became a service with a counter; an Adobe employee sends users from the local version to the web, because there "more power from servers". And the most important thing for us: a neural network does not SUBTRACT but RE-SYNTHESIZES the speech — which is why for a Russian-speaking user it adds an American accent and substitutes words. A classic noise reducer physically cannot invent a word; a generative one can. This is the real price, not the processing time.

**3. In FCP the noise reducer lives on the COMPONENT of the sound, not on the clip.** Verbatim: "Audio enhancement works on the component level, not the clip level". On a clip with a lavalier and an on-camera microphone each component carries its OWN `Voice Isolation Amount` — "clean the lavalier, leave the on-camera mic as it is" is expressed without cutting the clip and without a second event on a second track. Neither Premiere, nor Vegas, nor we have an effect chain with such addressing: it is glued to the event as a whole. Next to it stands a second design decision — Resolve KEPT the classic noise reducer with "learn" after Voice Isolation appeared, and not out of conservatism: the profile remains the only way to remove noise that the neural network was not trained on (the hum of a specific device, the tone of a specific room). The neural network knows exactly one opposition — "voice against everything else".

## Pros

- **The profile is DATA, so the second phase is free.** Taking the profile goes over seconds of material and is done once; the suppression depth, sensitivity and frequency smoothing can be turned instantly afterwards. Exactly the same boundary "analysis is expensive, application is cheap" as in the stabilizer, only two orders of magnitude cheaper.
- **The profile is a function of the RECORDING, not of the timeline.** One profile per shoot serves all clips from that camera; trimming, cutting and moving the event do not spoil it. For us this falls out for free: the key domain is already "material tick" (F-AUT-002, test t212).
- **The classic noise reducer is explainable and deterministic.** The same input gives the same output always, and you can point a finger at the bin that ate the "s". The neural result changes together with the model version.
- **The neural network takes what subtraction cannot reach** — non-stationary noise (a passing car, a keyboard) and material where there is NO piece of clean noise to take a profile from. This is not "better", it is a different set of tasks.
- **Wet/dry is mandatory even for a generative process.** Adobe adds a `Mix Amount` with the words "allows you to mix your raw audio with the enhanced audio, allowing natural background noise to come through": a fully cleaned track sounds dead. This is exactly the blend slider the project owner demanded for morphers — the same idea in another place.
- **Separating the generations does not make you pay twice** (Resolve): the realtime path does no analysis pass, the classic path does not drag in a model.

## Cons

- **Musical noise grows with aggressiveness — it cannot be avoided, only traded** for smearing (frequency smoothing) and for residual noise.
- **The profile is the weakest link, and the error is silent.** Taken from a place where the speaker breathes or where the room is different, the speech energy gets into the profile, and it is subtracted from the whole clip FOREVER: the voice dulls, and the cause is not visible. Taken too short (minimum — 2048 samples) — the noise statistics are garbage. Taken from another scene — useless.
- **The noise reducer does not distinguish "noise" from the quiet part of the useful signal.** Reverb tails and the tone of the room are exactly what it eats first; hence the classic complaint "removed the hum, but a new sound appeared, like an echo trailing from the speech".
- **The neural path stops being an effect.** It is not real-time, requires a pass, and produces NEW material: comparing "before/after" and undo stop being free, and the source stops being what is heard.
- **A neural project is not reproducible.** The model is updated together with the editor, and the same source sounds different six months later — Adobe users documented this twice (the December quality rollback and the switch V1 → V2). For a project archive this is a format defect, not a sound defect.
- **The strength slider of the neural network is not a slider.** Users report that there is almost no difference between 1 % and 100 %, and the whole jump falls on 0 → 1 %: the strength is blended into the result rather than controlling the processing.
- **The scope of applicability is narrow, and the vendor writes it itself.** Adobe explicitly lists where Enhance Speech does not work: a very poor recording, several speakers at once, whisper, loud music, singing.
- **The paywall stands right here.** In Resolve the AI sound is Studio; in Vegas the offline STT/TTS is Subscription Only; in Adobe V2 is generative credits. Without an extra payment on top of the program itself, the noise reducer is available only to the FCP user; Resolve needs Studio, Adobe needs credits for V2, Vegas needs a third-party plug-in. (Whether FCP computes locally, Apple does not state on the help page — `[NOT VERIFIED]`.)

## What users say

About "underwater" — at a strength above 0.3 (the same aggressiveness from "the trick behind it", only in the neural noise reducer):

> The Enhance Audio feature makes my audio, which is recorded both by an
> external mic and camera mic, sound like it's underwater if it's
> anywhere above 0.3.
> — altk3y, Adobe Support Community, 28.01.2024,
> https://community.adobe.com/announcements-732/now-released-enhance-speech-in-premiere-pro-313485

About the fact that the neural network re-synthesizes speech rather than cleaning it:

> This feature is useless for russian\ukrainian or any other Slavic
> language since it adds an american accent to words (and even
> subsitutes slavic words to english ones sometimes)
> — AndrewTheGreat, Adobe Support Community, 18.01.2024,
> https://community.adobe.com/announcements-732/now-released-enhance-speech-in-premiere-pro-313485

An Adobe employee answers a complaint about the quality of local processing with advice to go to the cloud — the price of the "local" path is stated officially:

> It harnesses more power from servers to do the processing.
> — Kevin J. Monahan, Sr. Community and Engagement Strategist, Adobe,
> 01.05.2025,
> https://community.adobe.com/t5/premiere-pro-discussions/enhance-speech-has-somehow-gotten-worse-samples-attached/m-p/14808523

About the strength slider that does nothing:

> there is barely any difference between 1% processing and 100%
> — Jenni_Lawson, Adobe Support Community, 06.12.2024,
> https://community.adobe.com/questions-514/enhance-speech-v2-is-here-1498584/index1.html

A pre-intellectual, but not outdated, formulation of the essence of a noise reducer:

> In my experience, noise reduction is always a compromise between
> minimizing unwanted background noise and how much digital artifacts on
> the wanted audio you can tolerate.
> — Steve King, Creative COW, 21.11.2014,
> https://creativecow.net/forums/thread/noise-within-the-dialogue-unable-to-remove/

And the most vivid description of what a noise reducer does to tails:

> Yes. When I tried it, it cleaned up some of the hum, but created a new
> noise; like an echo extending from the dialogue. Kind of like a spring
> sort of.
> — Debbie King, Creative COW, 19.11.2014,
> https://creativecow.net/forums/thread/noise-within-the-dialogue-unable-to-remove/

About Voice Isolation in Resolve — an artifact heard as the doubling of a source:

> Hey, the Voice Isolation doesn´t work properly anymore. When i active
> the effects the audio jumps randomly and it sounds like there are 2
> signals from the same source over each other.
> — spookyberlin, Blackmagic Design Forum, 10.08.2023,
> https://forum.blackmagicdesign.com/viewtopic.php?p=971448

## How it fits Kinotrain

### What is already there

We have exactly two audio effects, and this is verified, not quoted from the briefing: `FxInstance::Gain` (`src/libs/kt-model/src/project.rs`, line 1902) and `FxInstance::SlowedReverb` (line 1909) — the only two cases of `FxClass::Audio` (line 2114). There is no EQ, no compressor, no noise reducer; a grep for `noise|denoise|spectral|fft` over `src/libs` finds only the VIDEO effect `Noise` and the dev dependency `realfft` in tests.

- **An FFT already lives in the tree.** `realfft = "3"` is declared in `src/Cargo.toml` (line 52) and connected as a **dev** dependency of `kt-fx` and `kt-engine`: test t4 measures the spectral purity of the warp with it (`src/libs/kt-engine/tests/fx.rs`, lines 88–101). Moving it to a regular dependency of `kt-fx` is one line of the manifest; we do not bring in a new foreign ecosystem. For a project that does not link even with libav*, this is a rare case where the dependency is already paid for.
- **The path is ideal for spectral analysis.** f32/48 kHz/2ch on the whole path (invariant 1) — the FX never see the source's sample rate, that is, the window length and the step are fixed forever. A burst = 1024 frames (`AUDIO_BURST_FRAMES`, `src/libs/kt-time/src/lib.rs`, line 19), and `ClipProcessor::produce` ALREADY cuts any request into chunks of 1024 (`AUTOMATION_STEP_FRAMES`, `src/libs/kt-fx/src/lib.rs`, line 1326).
- **There is a place for state, and it is the right one.** `FxStateStore` keyed by `ClipId` (`src/libs/kt-engine/src/fxstore.rs`), owned by the audio-render thread; `chain_fingerprint` (`kt-fx/src/lib.rs`, line 966) is computed over the DISCRIMINANTS of the variants, so a new kind of FX breaks no existing state, and a parameter change arrives through `apply_params` without zeroing the buffers (invariant 3).
- **The pre-roll is already longer than any reasonable STFT window.** `MIN_PREROLL_FRAMES` = 4800 frames (100 ms, `src/libs/kt-fx/src/lib.rs`, line 24), and `chain_source_window_ext` (line 905, `preroll` at line 913) extends the source window to the LEFT by exactly that much — the prefetch already brings in the material to warm up the analyzer.
- **Group delay in the path is not news.** The sinc resampler of the warp gives ~3 thousand source frames; this is written in `src/libs/kt-fx/CLAUDE.md` and accounted for by the tolerance of test t4. So "an effect with delay" is an already-passed class, not a discovery.
- **The effect registry is designed for growth.** `src/apps/kt-app/src/fx_registry.rs` — "the single place of choice of effects for all four add points… scalable to hundreds of effects", with search and categories; numeric parameters are drawn by one `widgets::param_row` from the `ParamSpec` of the `params_for!` declaration (`project.rs`, line 2308 and further).
- **Selecting a piece of material for a profile already exists in the UI, in three forms:** a region (`Project::region`, `project.rs`, line 461), a range marker (`Marker` with `range`, lines 1138–1141), and a named area (`MarkedArea`, line 1163, item 241).
- **A morpher can carry an audio chain:** `MorpherEvent.patch.audio_fx` (`project.rs`, line 694). A noise reducer placed by a morpher covers the whole scene at once — this is our answer to the Vegas fork "Media FX or Event FX" from F-MSK-006, only without the choice that is forced on the user there.
- **The place for a heavy offline result** is the sidecar `<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`, `docs/media-index.md`).

### What is missing

- **The spectral kernel itself**: STFT (window, step, window function, overlap-add), an estimate of the noise spectrum, the per-bin decision rule, smoothing across frequency and across time. None of these exist in `src/libs/kt-fx/src/lib.rs` — there are freeverb, rubato and gain.
- **A non-scalar parameter.** `params_for!` declares ONLY scalars: the generated `fx_set_param(fx, name, v: f64)` and `fx_get_param(fx, name) -> Option<f64>` (`macro_rules! params_for`, `project.rs`, lines 2261–2305; the declaration itself — line 2308). A noise profile is a vector of ~513 numbers per channel. In this system there is nowhere to declare it: `param_row` will not draw it, `src/apps/kt-app/src/automation.rs` will not automate it, and the FX editor will not show it. This is the first parameter that falls out of the ONLY parameter mechanism — and this is an architectural question, not a layout detail.
- **A background pass with progress, cancellation and a single undo step** — exactly the same gap as in F-MSK-005 and F-MSK-006.
- **De-esser and de-reverb.** A de-esser is a narrow-band compressor (a detector in the 5–9 kHz band controls the gain), so it requires neither FFT nor a profile and costs an order of magnitude less than a noise reducer. De-reverb is the opposite: reverberation is not additive noise but a CONVOLUTION of the signal with an unknown impulse response of the room, and it cannot be "subtracted" in principle; blind deconvolution or a trained model is needed. Therefore there is no de-reverb as a separate effect in Vegas or Resolve, and Adobe has it only inside its neural source separation into layers.

### What gets in the way

**The main conclusion of this note is not about the noise reducer.** "Take a noise profile" is an offline pass over the material with the result saved, the same class as tracking (F-MSK-005), stabilization (F-MSK-006) and analysis for waveform sync (F-SPD-004). **This is the FOURTH independently arrived-at feature that runs into one and the same missing subsystem.** Four different authors, four different areas — and the same conclusion "a background pass over the material with the result saved is needed" — this is no longer a coincidence but a signal: the subsystem must be built separately and BEFORE any of the four features.

And the noise reducer is the best candidate for a pilot, because it is the cheapest of the four on all three axes:

1. **The analysis goes over seconds, not over the whole file.** The profile is taken from a region of 1–3 s; a pass over a twenty-minute video is not needed, and so a progress bar is not needed as a condition for launching the feature (Anstey's complaint from F-MSK-006 does not apply here).
2. **The result is 2–4 KB, not 95 MB.** The F-MSK-006 dilemma "a sidecar DB is declared a cache, but analysis must not be lost" does not arise for the profile: it fits INTO THE PROJECT. But it runs into something else (see "Format").
3. **The pass reads AUDIO, and our audio is always and everywhere a byte-for-byte deterministic path** (invariant 11, `GOLDEN_AUDIO`). The test "the same material → the same profile" is written in ten lines, whereas for tracking it is separate work with fixing the seed and the traversal order.

Next are the concrete points of attention.

- **Invariant 2 and where the profile comes from.** The FX state is touched only by the audio-render thread. The profile is not state but a PARAMETER: it must arrive through the model and `apply_params`, not through a mutex or a channel bypassing it. So the analyzer writes the result into the MODEL as one undo command, and the render thread learns of it by the same path as a change of `mix`. Any "direct delivery to the processor" is a direct violation of No. 2, the very reason this invariant was introduced (defect b of the old engine: 4 threads without locks on a shared delay line).
- **Invariant 3 and the overlap buffer: it fits, and here is exactly where.** `ClipProcessor::read(local, out, pull)` is read strictly sequentially, and `produce` already cuts any request into 1024 — but `local` is NOT aligned to any absolute grid (`from = (clip.start − clip.head_ext).max(range.start)` in `render_fx_clip`, `src/libs/kt-engine/src/audio.rs`, line 434), and the offline export calls `read` in a chunk the size of the whole clip. Therefore the STFT must hold ITS OWN input ring and its own OLA accumulator INSIDE `ClipProcessor` and make no assumptions about the size of the request — exactly the way rubato already does with its fractional cursor. The cost is two buffers the length of the window per channel (tens of kilobytes) in an object that already owns eight comb lines of freeverb. By invariant 3 a profile change is NOT structural (the window geometry is the same) — the buffers are not zeroed, the OLA tail survives the edit; but a change of the WINDOW LENGTH is structural, like `reverb_delay_ms`/`predelay_ms` (`Structural` in `params_for!`), and it is better not to give it to the user at all.
- **Delay is the real price, and it is easy to overlook.** Overlap-add gives an algorithmic delay of one window. The pre-roll `max(tail, 100 ms)` covers it by VOLUME, but not by MEANING: the pre-roll warms up the state, it does not shift the output. For the output to stay aligned to the timeline, the processor must consume one window MORE of material before the first output frame — that is, `source_offset` (`kt-fx/src/lib.rs`, line 1265) and the prefetch window (`chain_source_window_ext`, line 905) must know about the delay just as `chain_tail` knows about the reverb tail. If this is not done, the sound will drift against the picture by exactly one window, and NOT A SINGLE artifact test will catch it: the sound itself will be clean. The class is the same as the group delay of the sinc, already described in `src/libs/kt-fx/CLAUDE.md`.
- **Invariant 6 saves us, but the budget will have to be checked.** The FFT is computed in the audio-render thread, not in the cpal callback (the callback only reads the SPSC ring), so a 1024–2048 window in real time is a question of budget, not a prohibition. Today rubato and freeverb live in the thread; the noise reducer will add two transforms per 1024 frames PER EACH clip with the effect, and there may be several clips under the playhead.
- **Invariant 11 and `GOLDEN_AUDIO`.** The reference `0x5a83eef245ab43f0` (`src/libs/kt-engine/tests/golden_export.rs`, line 274) is not regenerated — by the kt-fx rule, the default of a new effect must give the previous sound byte-for-byte (for the noise reducer this is simple: without a profile — a straight pass-through). The harder part is different: the FFT must be bit-portable between platforms. The prohibitions of the kt-fx crate (FMA, reassociation of sums, rcp/rsqrt, f32 instead of f64) are written for the video kernels, but in spirit they are exactly about this, and any "optimization of the sum" inside a butterfly is a regression of determinism, not a speed-up.
- **Format: three variants, and all three are costly.** (a) The profile in `.otio` inside `FxInstance` — the project is self-contained and portable, but an array of half a thousand numbers per clip goes into the envelope, and `KT_META_VERSION` (`src/libs/kt-otio/src/meta.rs`, line 17) will go from 4 to 5. (b) The profile in a sidecar DB — but it is declared a CACHE ("loss or corruption is not an error, the DB is recreated", `index.rs`, lines 1–5), and losing the profile changes the SOUND, and silently at that. (c) Do not store it at all, but keep a "media + range" reference and recompute on opening — then the project depends on the presence of the source even where everything else is already mixed. The decision must be made BEFORE the first line of code, and the decision is shared with F-MSK-006. One good piece of news: an additive table in the sidecar can be created WITHOUT bumping `SCHEMA_VERSION` — the precedent is `poster` (`index.rs`, lines 133–143): the table is created `IF NOT EXISTS` on every opening precisely so as not to wipe the peaks and thumbnails of all the project's media for the sake of one section.
- **The neural path — honestly: no.** It would require weights of tens to hundreds of megabytes in the distribution, an inference runtime (ONNX Runtime / GGML / compute shaders), and either giving up determinism or enormous work to keep it. The project deliberately does not link even with libav* (invariant 10: FFmpeg is loaded through dlopen precisely so that one binary works on FFmpeg 6/7/8), and the release build is checked for "ldd without libav*" and the `GLIBC_` ceiling (`src/scripts/release-build.sh`). A tensor runtime within this frame is another project. And judging by the feedback, the refusal costs less today than it seems: at Adobe the local model is worse than the cloud one BY THE ADMISSION OF Adobe itself, the cloud one costs credits per minute, the strength slider changes almost nothing, and the result shifts between editor versions — that is, the project stops being reproducible, which our invariant 11 does not allow at all.

### Which invariants it touches

- **No. 1** (audio always f32 48 kHz/2ch) — in its favour: the window length and the step are fixed forever; the noise reducer will never see the file's sample rate.
- **No. 2** (FX state only in the audio-render thread) — the profile travels as a parameter through the model, with no channels bypassing it.
- **No. 3** (a model edit does not kill the FX state) — a profile change is not structural, the OLA buffer survives the edit; the window length is structural.
- **No. 5** (the decoder is owned by one MediaWorker) — the profile-taking pass opens ITS OWN short-lived source by the pattern of `spawn_index_worker` (`src/apps/kt-app/src/util.rs`, line 89), and does not stand in the MediaWorker queue. The wording of the invariant should be specified in the same round as for F-MSK-006.
- **No. 6** (the cpal callback is the master clock) — the FFT lives in the render thread; the callback still only reads the ring.
- **No. 8** (scroll/zoom does not spawn decodes) — the profile is taken ONLY by an explicit action, never "by itself, when the clip landed on the timeline". This is also rule 20 of the UI checklist.
- **No. 11** (determinism) — `GOLDEN_AUDIO` does not move; the FFT is bit-portable; the profile from the same material must be the same.
- **UI rules**: 2 (the profile can be taken during playback too — the "only when paused" gate protects nothing here), 12 (a background pass does not jolt the interface), 13 (the "take profile" button goes into an already occupied place of the FX editor, not a new strip), 16 (no region / the region is not noise — an explicit ERROR, not a grey button), 20 (the profile is not recomputed by itself when the clip is edited).

## Estimate

| What | Answer |
|---|---|
| Scope | L — a whole round. The spectral kernel in kt-fx, the offline-pass mechanism, a non-transparent parameter in the model, a decision on the format, the UI for taking the profile. The de-esser is a separate stage S on top (a narrow-band compressor, without FFT and profile); de-reverb is NOT included in the scope |
| Where the main work is | kt-fx (STFT + OLA inside `ClipProcessor`, `realfft` moved from dev to regular dependencies), kt-model (a new `FxInstance` + the first NON-scalar parameter), kt-engine (delay in `chain_source_window_ext`/`source_offset`), kt-app (taking the profile from a region, `fx_registry`, i18n) |
| Project format | a DECISION is needed before the code: profile in `.otio` → bump `KT_META_VERSION` 4 → 5; profile in the sidecar → a table `IF NOT EXISTS` without bumping `SCHEMA_VERSION` (precedent `poster`), but a cache cannot silently change the sound |
| Tests that will have to be written | `GOLDEN_AUDIO` did not move with defaults; the same material → the same profile (twice and after a restart); FFT bit-for-bit between runs; **no A/V shift**: a 440 Hz tone through the noise reducer comes out phase-aligned with the bypass path; the loop-region seam (t166–t168) does not break OLA; rebuilding the graph in the middle of playback gives a byte-identical output (t6 — the state survived the rebuild); the profile survives trim/split (material domain, t212); taking the profile does not go through the MediaWorker |
| Depends on | the offline-pass mechanism — shared with F-MSK-005 / F-MSK-006 / F-SPD-004; the format decision — shared with F-MSK-006 |
| Breaks compatibility | no, if the profile goes into the sidecar table; yes (v5 of the `kt` envelope), if into `.otio` |

## Verdict

Take the classic spectral noise reducer — and take it as a PILOT of the subsystem "offline analysis of material", not as an audio effect. It is the fourth feature of the base that needs the same background pass with the result saved, and it is also the cheapest of the four: seconds of material instead of hours, kilobytes instead of megabytes, and a deterministic audio path where the reproducibility test is written right away. We set two conditions without which it must not be taken: the OLA delay must be accounted for in `chain_source_window_ext`/`source_offset` (otherwise the sound drifts from the picture by one window, and the tests will not see it), and the place for storing the profile must be chosen BEFORE the code, because the sidecar is declared a cache, and losing the profile changes the sound. The neural path — we **reject** it, with a reason: it requires weights and a tensor runtime in the distribution, breaks invariant 11 (the result changes together with the model version) and, by Adobe's own admission, still loses to the cloud locally — that is, we would pay with the portability of the binary and the reproducibility of the project for the decidedly worse of two foreign results. The de-esser we pick up in a separate cheap stage; de-reverb we do not take at all — it is a convolution, not additive noise, and without a model it is not solvable.

## Sources

- Adobe Support Community, "Now Released: Enhance Speech in Premiere Pro", David Polk, 13.09.2023,
  https://community.adobe.com/announcements-732/now-released-enhance-speech-in-premiere-pro-313485
  (accessed 2026-09-07) — local processing, `Mix Amount`, the list "does not work well with", quotes from altk3y and AndrewTheGreat
- Adobe Support Community, "Now in Beta: Enhance Audio in Premiere", Adolfo H., 02.09.2026,
  https://community.adobe.com/announcements-732/now-in-beta-enhance-audio-in-premiere-1640126
  (2026-09-07) — the hybrid "separation on the device / V2 in the Firefly cloud", two generative credits per minute, Premiere Beta 27.0 (Build 36), layers speech/music/ambience/SFX/reverb
- Adobe Support Community, "Enhance Speech has somehow gotten worse",
  https://community.adobe.com/t5/premiere-pro-discussions/enhance-speech-has-somehow-gotten-worse-samples-attached/m-p/14808523
  (2026-09-07) — quote from Kevin J. Monahan; evidence of changes in the model's behaviour between versions
- Adobe Support Community, "Enhance Speech V2 is here",
  https://community.adobe.com/questions-514/enhance-speech-v2-is-here-1498584/index1.html
  (2026-09-07) — quote from Jenni_Lawson about the strength slider
- Blackmagic Design, DaVinci Resolve — Fairlight,
  https://www.blackmagicdesign.com/products/davinciresolve/fairlight
  (2026-09-07) — "teach it by selecting noise and clicking on "learn"", de-esser, de-hummer, "over 25 Fairlight FX plugins"
- Blackmagic Design, DaVinci Resolve Studio,
  https://www.blackmagicdesign.com/products/davinciresolve/studio
  (2026-09-07) — "DaVinci Neural Engine AI voice isolation, music remixer and dialogue separator" as Studio features
- Blackmagic Design Forum, thread about Voice Isolation,
  https://forum.blackmagicdesign.com/viewtopic.php?p=971448
  (2026-09-07) — quote from spookyberlin
- Apple, Final Cut Pro User Guide, "Enhance audio in Final Cut Pro",
  https://support.apple.com/guide/final-cut-pro/enhance-audio-verc1fab873/mac
  (2026-09-07) — Voice Isolation / Background Noise Removal / Hum Removal / Loudness / Equalization, "works on the component level, not the clip level"
- Boris FX, VEGAS Pro 2026, "What's New",
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/1-before/whatsnew.htm
  (2026-09-07) — Fully Offline Speech-To-Text / Text-To-Speech, Subscription Only; no noise reducer in the list
- MAGIX, VEGAS Pro 22 help, "Audio plug-ins",
  https://help.magix-hub.com/video/vegas/22/en/content/topics/9-audio/audio_plug-ins.htm
  (2026-09-07) — Vegas audio effects are a chain of DirectX/VST in the Plug-In Chooser
- Creative COW, "Noise within the dialogue — unable to remove",
  https://creativecow.net/forums/thread/noise-within-the-dialogue-unable-to-remove/
  (2026-09-07) — quotes from Steve King and Debbie King
- Audacity Manual, "Noise Reduction",
  https://manual.audacityteam.org/man/noise_reduction.html
  (2026-09-07) — "Get Noise Profile", minimum of 2048 samples, `Sensitivity`, `Frequency Smoothing (bands)`, the description "musical noise / bird song / tinkly-bells". Not a core editor, but a primary source on the classic algorithm — unlike the marketing pages of the five editors, which do not describe the mechanism
- Wikipedia, "Noise print", https://en.wikipedia.org/wiki/Noise_print
  (2026-09-07) — "twittering or birdies" as the industry name of the artifact

**Honest about the gaps.** `helpx.adobe.com` (Premiere/AE help), `reddit.com` and `discussions.apple.com` do not open from this environment, and the `WebSearch` budget is exhausted — the search went through direct queries to Yahoo and reading the accessible sites. Therefore: the composition of Premiere 26 audio functions (not beta 27.0) is taken from Adobe community announcements, not from the help; the list of audio plug-ins shipped with VEGAS Pro 2026 could not be confirmed against Boris FX documentation — the "Audio plug-ins" page describes the mechanism (DirectX/VST), but not the composition, and the line about AI sound cleanup in the Plus/Ultimate editions is taken from a secondary review and is marked `[NOT VERIFIED]` in the table. The assignment of Voice Isolation to Studio is taken from the Blackmagic site; secondary sources place it in the free version, and Dialogue Leveler/Separator in Studio — this discrepancy is not resolved. No direct quotes of the form "sounds underwater" about Resolve Voice Isolation could be found: they live on reddit, which is unavailable. None of the five editors was run live.
