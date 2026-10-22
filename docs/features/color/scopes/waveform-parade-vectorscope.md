---
id: F-CLR-005
title: Scopes — waveform, parade, vectorscope, histogram, false color
status: candidate
priority: B
cost: M
category: color/scopes
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: no
crates: [kt-app]
invariants: [4, 7, 8, 11, 12]
ui_rules: [3, 6, 9, 13]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-CLR-005 — Scopes — waveform, parade, vectorscope, histogram, false color

## Essence

Five ways to look at ONE frame through numbers instead of eyes. The waveform puts brightness along the horizontal axis of the frame: a column of the graph stands at the same place as the column of pixels, so you see not only "clipping" but WHERE it is — sky on the left or a face in the centre. The parade spreads the same data into R, G and B: the bottoms of the three graphs that have drifted apart are a colour shift in the shadows, and white balance is set by them. The vectorscope is a circle: angle = hue, radius = saturation. The histogram loses the position in the frame, but counts HOW MANY pixels there are at each level. False colour paints the picture itself with zones of exposure — it came from camera monitors, and it is used by those who shot the footage.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 20.3.2 | Video Scopes | five instruments (Waveform, Parade, Vectorscope, Histogram, CIE Chromaticity) in a SEPARATE floating window (Cmd-Shift-W) or on a second screen; layouts 1/2/4/9-up, and within one layout you can place THREE vectorscopes with different tonal ranges (Low/Mid/High) and watch them at once. The waveform/parade scale switches: 10-bit (default), 12-bit, percent 0–100, mV, HDR nits. Separately — drawing quality High/Medium/Low/Auto "for slow workstations", a Low Pass Filter (smoothing of the trace) and Extents (outlining of true excursions), and Display Qualifier Focus (the eyedropper in the viewer circles the chosen pixels RIGHT IN THE GRAPH of the instrument). The manual says plainly that the instruments are GPU-accelerated and still eat real-time, and that closing them "frees all processing" |
| Adobe Premiere | 26.x | Lumetri Scopes; plus the instruments of the new Color mode | the classic is the Lumetri Scopes panel (Window → Lumetri Scopes), a tab next to Effect Controls. In 2026 Adobe shipped a new Color mode with its own instruments, and the forum shows how it differs from the old panel: the waveform there CLAMPS the signal to 0–100 (you cannot see whether it went beyond the limits), and the instruments pop up over the program monitor without an adjustable panel. Users ask to bring back "unclamped" display and not to kill the Lumetri panel when the modes are merged |
| Final Cut Pro | 12.3 | Video Scopes | the instruments live INSIDE the viewer (Cmd-7), sharing its space with the picture; up to four instruments at once, 12 layouts. Waveform: RGB Parade, RGB Overlay, separate R/G/B, Luma, Luminance (PQ only), Chroma, Y'CbCr Parade; scales IRE, mV, nits. Vectorscope: reference level 100 % or 133 %, the grid rotated Vector/Mark3 (red at the top or on the right), Show/Hide Skin Tone Indicator. There is no false colour — instead there is Range Check: a PATTERN over the picture (Luma / Saturation / All), with thresholds that depend on the colour space of the project |
| VEGAS Pro | 2026.0 | Video Scopes | the same panel as in the previous line: in the new list for 2026 (speech-to-text, Storyblocks, Z-Depth 2.0, Continuum titles, 4:2:2 on Blackwell) there are NO instruments at all. Settings — four ticks: Enable ST.2084 HDR Scope, Skin Tone Line, 7.5 IRE Setup, Studio RGB (16/64 to 235/940). The skin tone line is switched on with a tick; its angle by default is 123° ("11 o'clock") and it CAN be changed. The Studio RGB default has stayed from the time when the project default was 8-bit limited — and has not changed since then |
| After Effects | 26.x | the Lumetri Scopes panel | **not checked live**: Adobe's help (helpx.adobe.com) is unavailable from the verification environment, and in r/AfterEffects over the last six months there is not a single discussion of the instruments. The model is structurally different: there is no program monitor, the instrument measures the composition viewer, and the project depth is switched (8/16/32 bits per channel), so the question "in what units is the scale" is answered not the way it is in an NLE |

## The trick behind it

**1. The "skin tone line" is not about skin but about an angle.** Resolve names it honestly: "optional skin tone reference graticule (otherwise known as the In-phase reference)" — that is, the I axis of the old NTSC encoding, not a measured anthropology. FCP describes it as a diagonal "between the yellow and red color bar targets", and Vegas sets it at 123°. The practical meaning is in WHAT is checked along it: the angle, that is, the HUE. Darker or lighter skin is the radius (saturation) and the height on the waveform, not a different angle; so the line is one for all skin types, and "setting the skin" = turning the hue, not dragging the cloud towards the centre. Hence the working technique from the forums: mask out everything except the face, and align the peak, not the whole cloud of the frame, with the line. Of the five editors, only Vegas lets you change the angle of the line; Resolve and FCP only let you switch it on and off.

**2. Smoothing the trace lies about exactly what the instrument was opened for.** Resolve has a Low Pass Filter — a noise filter that makes the graph readable — and the manual, IN THE SAME PARAGRAPH, warns that with it the "highlight and shadow excursions on the graph seem a bit shy of where they actually are", and recommends turning on Extents (outlining true excursions). So a pretty trace systematically understates clipping — which is the only thing people come to the instrument for. This also explains why each instrument there has a separate brightness slider for the graph: a bright trace shows fine detail but stops showing where the signal is MANY and where there are single pixels.

**3. The scale is a tick box, and its default can mislead.** Blackmagic settled the question radically: WFM and Parade always show 10-bit full-range data 0–1023, regardless of the Video/Data Level setting in the project — "a window into how the image is being processed prior to being output". Vegas did the opposite: the Studio RGB tick (16/64 to 235/940), and the default fell behind the change of the project default, and the forum still discusses this — with the wrong tick, a frame reads as clipping when it does not clip. The same class: "7.5 IRE Setup" — a shift of black by 7.5 units, a legacy of NTSC, which still sits as a tick box beside it.

## Pros

- The scope changes nothing in the frame, so it has no undo, no state in the model, and no influence on render references: it can be added to a product where color correction does not exist at all, and it immediately answers the questions "is this source already clipping?" and "at what level did the export land?".
- It is computed from the FINISHED frame, not from the scene plan, so one and the same code serves any render backend: different executors return the same RGBA bytes, and no second composition path appears.
- The parade gives white balance without a single color tool: to see a green shift in the shadows, nothing needs to be turned — it is enough that the bottoms of the green graph stand higher than the bottoms of the red and blue.
- The waveform and the histogram do not replace each other by design, not "by taste": the waveform keeps the horizontal coordinate (one can see which part of the frame is blown out), the histogram loses it but counts the pixels at each level. That is why all five editors place them side by side.
- False color works differently from all the others: it does not build a graph, it recolors the frame itself by zones — and is therefore read with the same eye and the same experience as the camera monitor on set.

## Cons

- The price is a full pass over the frame on EVERY frame, and this is visible: Blackmagic keeps a separate quality switch for the scopes (High/Medium/Low/Auto) and writes that closing the scopes window returns processing to the grid.
- A scope takes space, and all five pay differently: FCP gives the scopes part of the viewer (the picture is shrunk), the new Color mode of Premiere covers the program monitor with them, Resolve moved them into a separate window — and got an extra window.
- A single scope has more knobs than a typical effect: the vectorscope in Resolve has ten of them (tonal range, Colorize, Extents, Combine, graph brightness, grid brightness, Low/High Range, 2×, skin tone, graticule) — and all of that is for DISPLAY, without a single change to the picture.
- The readings depend on the scale checkbox: Studio RGB / 7.5 IRE shift the reading by 16 and 7.5 units respectively, and an error in the checkbox produces false clipping — exactly what the scope was opened to check.
- Without an editing tool the scope makes a diagnosis and does not cure: it will honestly say "overexposed", but the choice remains between another take and a different export exposure.

## What users say

> Please add back in the ability to "unclamp" the signal on the waveform
> monitor so we can see if the signal is going above 100 or below 0 like you
> can on the Lumetri scope!
> — u/ShinyWolverine, r/premiere, 16.04.2026, https://www.reddit.com/r/premiere/comments/1sm5fes/announcing_premieres_new_color_mode/ogm1ufm/

> this might just be me, but it seems like when you select a control and drag
> it, the scopes that pop up immediately supercede the footage in the program
> monitor, so i literally can't see the thing i'm working on.
> — u/ClimbThatTree, r/premiere, 02.07.2026, https://www.reddit.com/r/premiere/comments/1sozula/the_new_color_panel_effects_pane_is_a_complete/ov6jqe2/

> Btw, the Skin Tone line is on the Vectorscope. The default is at 11 o'clock
> (123 degrees) which is an alignment reference for Caucasian skin tone.
> — Howard-Vigorita, Boris FX forum (VEGAS Pro), 10.08.2026, https://forum.borisfx.com/t/video-scopes-settings/25018

> 8bit Limited Range was the project default once upon a time but they didn't
> change the scopes default when that changed. Be nice if it had an Auto
> option.
> — Howard-Vigorita, Boris FX forum (VEGAS Pro), 10.08.2026, https://forum.borisfx.com/t/video-scopes-settings/25018

> Nearly every video editing program has a skin tone line in the scopes
> window. Line up the color of the skin on that line (actually just above a
> small amount) and you have accurate Caucasian skin tones.
> — u/denalidenizen, r/colorists, 01.03.2026, https://www.reddit.com/r/colorists/comments/1rbkxcb/skintones_andor/o7z2g23/

A counter-reply in the same community — against believing in the line as if it were law:

> Depends on the scene. For daylight you might want the skin to be close to
> the skin tone line. For Bar Scene... Neon Lights.. the Skin can be anything.
> — u/I-am-into-movies, r/colorists, 21.01.2026, https://www.reddit.com/r/colorists/comments/1qj6a1r/when_do_you_throw_out_the_search_for_good_skin/o0x37mw/

And on what the scope is opened for before grading at all — in a reply to the classic question "why does everything look different on another screen" (further in the same post: you can control only your own environment, "That and trusting your ears (eyes) and meters (scopes)"):

> You mix (or color grade) in a properly calibrated and balanced environment.
> — u/whyareyouemailingme, r/davinciresolve, 05.09.2026, https://www.reddit.com/r/davinciresolve/comments/1w80jtx/how_do_you_mix_audio_for_phones_tablets_and_tvs/p7yzfbw/

## How it fits Kinotrain

### What already exists

- **A finished frame in RAM, every new frame, on both backends.**
  `Player::compose_preview_into` (`src/libs/kt-engine/src/player.rs:555`)
  writes RGBA8 into the passed `Vec<u8>`; in the UI this is `KtApp::preview_buf`, and it is
  REUSED between frames (`src/apps/kt-app/src/app.rs`, the `if need { … }` branch around `compose_preview_into`, and there the `ColorImage` and the texture upload).
- **The readback on the GPU path is already done and already paid for.**
  `exec.execute_into(&plan, out)` (`player.rs:582`) goes into
  `Inner::execute` (`src/libs/kt-gpu/src/engine.rs:579`), which reads the composite back into a CPU buffer and memoizes the bytes in `ReadbackCache`
  (`src/libs/kt-gpu/src/pool.rs:138`, budget `READBACK_BUDGET` = 64 MB, key — the plan hash, `engine.rs:23`). A separate readback "for the sake of the scope" is not needed: egui loads the frame as a `ColorImage` anyway.
- **It will compute exactly once per NEW frame.** The preview key is aligned to the fps grid (`preview_key_tick`, invariant 12, t39), recomposition happens only when `need` is set, and `preview_key` also remembers the "completeness" of the frame. A scope hung on the same branch inherits this frequency for free.
- **We know how to draw.** Multi-series rolling graphs with callouts —
  `src/apps/kt-app/src/graphs.rs` (a ring of 180 samples, up to 5 series, a common scale, `Axis::Percent` / `LinearAuto`). Thousands of primitives per frame are
  collected into one `egui::Mesh` — `src/apps/kt-app/src/meters.rs` (two
  places); this is a direct rule of `src/apps/kt-app/CLAUDE.md` (t40). The second ready technique is to assemble the picture in a buffer and upload it as a texture, like the glow mip of the preview (`app.rs`, every 4th frame).
- **A place for it and its memory.** The right panel `meters_col` with its own separator strip
  and the limits `METERS_W_MIN`/`METERS_W_MAX`
  (`src/apps/kt-app/src/app.rs`); the width lives in `UiState.meters_w`
  (`src/libs/kt-model/src/project.rs:1275`) and travels into the project file through
  `metadata.kt.ui` (`src/libs/kt-otio/src/meta.rs`).
- **The program already knows how to do a separate window:** detaching the preview via
  `show_viewport_immediate` (item 51, `src/apps/kt-app/CLAUDE.md`) — the path "like Resolve's" is not closed.
- **There is something to test the layout with:** `egui_kittest::Harness` runs real
  frames without a window or GPU (`src/apps/kt-app/src/layout_tests.rs`).

### What is missing

- There are no instruments at all: `grep -rniE "histogram|vectorscope|parade|false
  ?color|гистограм|вектороскоп"` over `src/` returns EXACTLY one match —
  the histogram of frame DIFFERENCES in `kt_gpu::assert_frames_close`
  (`src/libs/kt-gpu/src/lib.rs:39`), which is test diagnostics, not an instrument.
- It is not decided WHAT we measure. The preview is computed at `preview_quality` —
  a fraction of the project resolution (1/16, 1/8, 1/4, 1/2, 1/1 in
  `src/apps/kt-app/src/app.rs::draw_preview_controls`; default 640×360,
  `src/libs/kt-engine/src/player.rs:264`). This is NOT a downscale of the export
  frame: quality is part of the cache key, and the source is decoded straight to
  that size. An instrument on the 1/4 preview honestly shows what the user sees, and
  may fail to show clipping that will be present in the export
  (`compose_export_into`, `src/libs/kt-engine/src/video.rs:405`, computes at the
  full project resolution).
- There is no scale that can be honestly labeled. After decoding, the path is RGBA8;
  `sws_getContext` (`src/libs/kt-ffmpeg/src/source_impl.rs:922`) is called WITHOUT
  `sws_setColorspaceDetails`, so the interpretation of the range is left to the
  swscale default, while in kt-gpu all textures are `Rgba8Uint`
  (`src/libs/kt-gpu/CLAUDE.md`). A 10-bit scale of 0–1023 would be a lie for us;
  only 0–255 and 0–100 % are honest. Besides, this is a question that Vegas turned
  into a years-long forum thread — it has to be settled once and out loud, not with
  a checkbox and a stale default.
- There is no "picture BEFORE the grade", because there is no grade — but the fork
  has to be fixed in advance: "after" is the ready `preview_buf` and zero new code;
  "before" is a SECOND composition of the same tick.
- There is no zoom/pan for the preview (there is nothing to search for — there are none
  in `app.rs`), so the question "which part of the frame is measured" today has a simple
  answer: "the whole preview frame". But the choice still exists: the PROJECT preview or
  the MATERIAL preview in the bin (t169) — the instrument must say what exactly it measures.

### What gets in the way

- **The shader path is architecturally closed for us.** `src/libs/kt-gpu/CLAUDE.md`
  forbids compute passes (the default egui-wgpu device on the GL backend is created with
  downlevel_webgl2 limits, and the live check runs on llvmpipe) and forbids atomics
  (a condition of determinism on the device, t183). A histogram without atomics is either
  256 fragment passes or a sort; a pass over a 640×360 frame that already sits in memory
  is 230 thousand pixels and cheaper than any such construction. At 1/1 in 4K it is
  8.3 million pixels, and there the deciding factor is no longer the backend but the
  FREQUENCY: it must be computed for each new frame, not for each UI frame.
- **Space.** Rule 13 of the checklist (`src/apps/kt-app/CLAUDE.md`) explicitly forbids
  opening a new strip until it has been checked whether it can go on something already
  occupied; meanwhile a vectorscope is square by nature, and the meter panel is by default
  ~70 pt wide. This is a layout decision, not a code one, and it is the main thing in the feature.
- **Rule 9 "nothing superfluous" against the truth of the instrument.** Resolve's ten
  vectorscope toggles would not pass the checklist, but without smoothing and without
  outlier outlining the instrument systematically understates clipping (see "The trick
  behind it"). What is included FOREVER and what is moved to a knob has to be decided explicitly.
- **Any new UI text** — these are the catalogs `apps/kt-app/i18n/*.toml`, tests
  t100/t125/t128 and a rebuild of the font subsets (`python3 scripts/subset-fonts.py`).

### Which invariants it touches

- **7 (one composition code)** — does NOT touch it, as long as the instrument reads a
  finished frame. A separate "instrument shader" or a composition "before the grade"
  immediately turns it into a second path, and that has to be counted as a cost, not a detail.
- **12 (repaint only on motion)** — touches it directly: while paused the instrument must
  not compute and must not request a redraw, following the pattern of
  `MeterState::is_active()` (t38, t20-repaint). It has no decaying state at all — that is a plus.
- **11 (determinism)** — does not touch it: the instrument only displays;
  GOLDEN_VIDEO/GOLDEN_AUDIO are not affected.
- **4 (cache is the only truth about coverage) and 8 (scroll/zoom does not generate decode)** —
  do not touch it, on the condition that the instrument takes an ALREADY composed frame
  and never orders its own.
- UI rules: **3** (nothing jumps when the instrument changes — the width is fixed by a gallery),
  **6** (the panel stretch has a min and a max), **9** (nothing superfluous),
  **13** (do not waste space).

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: one instrument with a panel and a settled scale, then the other kinds |
| Where the main work is | kt-app as a whole: computation on the finished frame + drawing (mesh or texture) + panel layout. The engine is not touched: `compose_preview_into` already yields the bytes |
| Project format | not touched: one field in `UiState` (`src/libs/kt-model/src/project.rs:1249`) with `#[serde(default)]`, like `meters_w`; re-check the kt-otio round-trip (t144–t147) |
| Tests that will have to be written | unit on a synthetic frame (bars 0/128/255 → known numbers of the parade, histograms and the vectorscope angle; determinism between runs); `layout_tests.rs` — the panel does not swell and does not jump when the instrument changes (class t76/t86), on pause the repaint is released (class t20-repaint); t100/t125 for the new texts |
| Depends on | nothing. The reverse is true: color correction (wheels, curves, LUT) without instruments is guesswork, so this note goes BEFORE them |
| Breaks compatibility | no |

## Verdict

We take it, and first in the color branch. It is the only "color" feature that changes
nothing in the picture: zero risk for invariants 7 and 11, zero changes to the format,
all the work is in kt-app. It is useful in itself even today — it shows clipping in the
sources and the level of the finished export — and it is also a prerequisite for the
other notes of the color branch (F-CLR-001 "wheels", F-CLR-003 "secondary correction"
and the neighbouring ones): setting white balance "by eye" on an uncalibrated monitor
makes no sense, which is what editors write about.
The main question of the feature is not algorithmic but two layout questions: where the
instrument lives (the meter panel / a detached window) and in what units the scale is
labeled — the second one Vegas has still not resolved.

## Sources

- DaVinci Resolve 20 Reference Manual, chapter 127 "Viewers, Monitoring, and
  Video Scopes", pp. 3008–3022 (scales, Low Pass Filter/Extents, drawing quality, the
  options of each instrument) — https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf (accessed 2026-09-07)
- Final Cut Pro User Guide: "Video scopes", "Display video scopes",
  "Waveform monitor display options", "Vectorscope display options",
  "Detect out-of-gamut colors" — https://support.apple.com/guide/final-cut-pro/video-scopes-ver4a3c67b8/mac (accessed 2026-09-07)
- VEGAS Pro 2026 Online Help, "What's New" (no instruments in the list of new things) —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/1-before/whatsnew.htm (accessed 2026-09-07)
- Boris FX forum (VEGAS Pro), thread "Video Scopes Settings?", 09–11.08.2026
  (four checkbox settings, the angle of the skin tone line, the stale Studio
  RGB default) — https://forum.borisfx.com/t/video-scopes-settings/25018 (accessed 2026-09-07)
- r/premiere: "Announcing Premiere's new Color mode" (clamping of the waveform),
  "The new color panel effects pane is a complete…" (instruments over the program
  monitor), "What is everyone's current thoughts on Color mode" —
  https://www.reddit.com/r/premiere/comments/1sm5fes/announcing_premieres_new_color_mode/ , https://www.reddit.com/r/premiere/comments/1sozula/the_new_color_panel_effects_pane_is_a_complete/ (accessed 2026-09-07)
- r/colorists: threads about the skin tone line and its limits —
  https://www.reddit.com/r/colorists/comments/1rbkxcb/skintones_andor/ , https://www.reddit.com/r/colorists/comments/1qj6a1r/when_do_you_throw_out_the_search_for_good_skin/ , https://www.reddit.com/r/colorists/comments/1qkycdd/xrite_colour_check_currently_working_with_davinci/ (accessed 2026-09-07)
- **Not verified live:** the Adobe help (helpx.adobe.com) is unavailable from the
  verification environment (CloudFront blocks the requests), so the composition of the
  Lumetri Scopes settings in Premiere and the presence of such a panel in After Effects
  are taken not from the primary source but from forum discussions; the After Effects
  row in the table is marked as unverified.
