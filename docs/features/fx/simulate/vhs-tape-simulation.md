---
id: F-FX-009
title: VHS as a simulation — write the tape, read it back
status: done
priority: B
cost: L
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-plugin-api, kt-gpu, kt-fx, kt-app]
invariants: [4, 7, 11]
ui_rules: []
round: kkt-26.11.01
point: V1
tests: [t1180, t1181, t1182, t1183, t1184, t1185, t1186, t1187, t1188, t1189, t1190, t1191, t1192, t1193]
depends_on: [F-FX-007]
trackers: []
services: []
checked: 2026-11-01
---

# F-FX-009 — VHS as a simulation — write the tape, read it back

## Essence

The owner (2026-11-01, `docs/ktprompts/kkt-26.11.01.txt` line 40): besides the "frankly cheap"
`vhs` look, a REAL VHS filter that conveys the physics — the picture is really written as a
magnetic signal onto a tape and really read back; drums of good quality, with dirty heads, with
four heads instead of two, with one head chipped; two drums (record, play) or one doing both.
His answer to question 4: "you pay a lot of attention to the UI and completely ignore the
complexity of the effects' physical mechanics".

So this note is about the physical chain. Our existing `vhs` (`src/plugins/fx-video-stylize`)
is an IMITATION: an RGB shift, scan lines and row jitter reproduce the LOOK by image
operations. A SIMULATION reproduces the CAUSES: the look comes out of an FM carrier that a
clogged gap no longer reads, of a chroma signal that an adjacent track leaks into, of a time base
that a stretched tape bends. The imitation stays for old projects (root `CLAUDE.md`: an imitation
is never renamed into a simulation).

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | none built in; third-party **Sapphire S_VHSDamage** (Sapphire 2025), Red Giant Universe "VHS" | procedural looks: tape noise, wiggle, distortion modules, presets. The parameters are LOOK quantities (amount of wiggle, noise type), not device quantities |
| Adobe Premiere | 26.x | the same third-party plugins | as AE |
| DaVinci Resolve | 21 | none built in; OFX plugins (Sapphire, **ntsc-rs** OFX) | ntsc-rs runs inside Resolve as OFX |
| Final Cut Pro | 12.3 | none built in; Motion templates and third-party packs | overlays and looks |
| VEGAS Pro | 2026.0 | none built in; Boris FX (now the publisher) Sapphire, ntsc-rs OFX | as AE |

**Outside the core.** The two programs that actually model the signal:

| Program | What it is | What it models | What it does not |
|---|---|---|---|
| **ntsc-rs** (open source, standalone + OFX + AE/Premiere) | a composite-video and VHS emulator | composite encoding, Y/C separation, chroma subsampling, a "tape speed" HF cutoff, head-switching and tracking-noise groups | no FM carrier, no tape, no heads: the VHS stage is filters and noise on the baseband picture — its tape speed is a cutoff, not a wavelength |
| **vhs-decode** (open source, a fork of ld-decode) | decodes REAL RF captured from a VCR head amplifier at ~28–40 MS/s | the playback half for real: FM demodulation, de-emphasis with its time constant, colour-under down-converted and PS de-rotated per track, the comb, dropout detection | there is no recording half — it reads real tapes. Its `format_defs/vhs.py` is the best public table of the standard's numbers (used below) |

Nobody in the table writes a tape and reads it back. ntsc-rs is the nearest, and it stops at the
baseband.

## The trick behind it

**1. Every VHS defect people recognise is a consequence of THREE physical facts, not a look of its
own.** (a) The luma is an FM carrier whose wavelength on tape is about a micrometre, so anything
that lifts the head off the tape by a micrometre removes it (a clog → snow, not "noise"). (b) There
is no guard band between tracks; the neighbour track is rejected by AZIMUTH (±6° gaps), and
azimuth loss grows with frequency — so it rejects the FM luma well and the 627 kHz colour-under
badly; VHS cancels the chroma leak with a per-line phase rotation and a comb filter, which is
why its colour is vertically smeared. (c) The time base comes from a rotating drum and a
stretching tape; without a TBC every line starts a little early or late. A simulation that models
these three gets snow, chroma crawl, the noise bar, the head-switching tear and line jitter for
free, with their correct dependencies (the noise bar MOVES with the tracking knob; LP is noisier
because its track is narrower, NOT because its bandwidth is lower — the writing speed is the same).

**2. The writing speed, not the tape speed, sets the wavelengths.** The head moves over the tape at
≈ 4.85 m/s (PAL, 25 drum revolutions/s) or 5.80 m/s (NTSC); the tape itself crawls at 23.4 or
33.35 mm/s in SP. LP halves the tape speed and therefore the track pitch, but the wavelengths stay.
ntsc-rs models "tape speed" as an HF cutoff — the opposite of the physics.

**3. The colour-under chroma survives a jittering time base, the luma does not.** The playback deck
up-converts the 627/629 kHz chroma with an oscillator locked to the played-back sync and burst, so
a time-base error moves the chroma's envelope with the line but does not rotate its phase; the luma
FM is demodulated as is. That is why a TBC-less VHS picture has wobbling edges and stable hue.

## The physical chain, step by step

Numbers per standard (PAL / NTSC VHS), from SMPTE's VHS documents and `vhs-decode`'s
`format_defs/vhs.py` (sources 1–4). **Bold** marks what becomes a parameter in physical units.

1. **Y/C split of the picture.** VHS records luma and chroma separately (colour-under), so there
   is no composite stage: the camera's (here — the frame's) Y and the U/V difference signals go
   in separately. Luma is band-limited to ~3 MHz on record, chroma to ~±0.5 MHz.
2. **Luma pre-emphasis and white/dark clip.** A lead network with τ₁ = 1.3 µs and a 14 dB (×5) HF
   gain (vhs-decode: `deemph_tau = 1.30e-6`, gain 13.98 dB, mid 274 kHz) boosts edges before FM;
   the overshoots are then clipped (**white clip, dark clip** in % of the white level) — the source
   of the dark/white streaks after hard edges. Later decks added NONLINEAR emphasis (level-dependent);
   modelled as linear here (design: simplifications).
3. **FM modulation.** Sync tip → white maps linearly onto the carrier: **PAL 3.8 → 4.8 MHz, NTSC
   3.4 → 4.4 MHz** (1 MHz deviation; S-VHS 5.4 → 7.0). The phase is the integral of the
   instantaneous frequency.
4. **Colour-under.** The chroma (4.43 / 3.58 MHz subcarrier) is heterodyned down to **626.953 kHz
   (40.125 fH, PAL) / 629.371 kHz (40 fH, NTSC)** and recorded LINEARLY under the FM, which acts
   as its AC bias. Its phase is rotated per line to defeat crosstalk ("PS"): vhs-decode's
   `chroma_rotation` is **[0, −90°/line]** for PAL tracks A/B and **[−90°, +90°]/line** for NTSC.
5. **Recording onto the tape.** The sum magnetises the tape through the record head: tape
   SATURATION bends the sum (intermodulation of chroma and FM), the record head's own **spacing**
   (a clogged record head writes a weak signal FOREVER — the defect survives a clean playback deck).
6. **Helical scan geometry.** A **62 mm drum** with two heads 180° apart (wrap a little over 180°)
   rotates at 25 / 29.97 rev/s; each head writes one FIELD as one slanted track at **≈ 5.96°**.
   Track pitch = tape speed per field: **PAL SP 49 µm, LP 24.5 µm; NTSC SP 58 µm, LP 29 µm, EP/SLP
   19.3 µm**. Heads are a bit wider than the pitch; each new track overwrites the edge of the
   previous one — no guard band.
7. **Azimuth ±6°.** Head A's gap leans +6°, head B's −6°. Reading a strip of the neighbour track of
   width w with a gap tilted 12° against its recording integrates the signal along a length
   w·tan 12°: the loss is sinc(π·w·tan12°/λ). At λ ≈ 1 µm (FM) it rejects ~30 dB; at λ ≈ 7.7 µm
   (colour-under) ~13 dB only — the chroma leak step 4 defeats.
8. **Losses of the channel**, as functions of the wavelength λ = v_write / f: **gap loss**
   sinc(π·g/λ) (**gap length g**, ~0.3 µm), **spacing loss** exp(−2π·d/λ) (**head-to-tape spacing
   d**: a clog is a d of micrometres), thickness/self-demagnetisation of the coating, and the
   **head efficiency** (a chipped head: lower output, a worse gap).
9. **Noise.** Tape (particle) noise grows as √(track width read), the signal as the width: a
   narrow LP track loses ~3 dB of SNR per halving. Head-amplifier noise is constant. **Tape grade /
   wear** sets the particle noise and the **dropout rate** (oxide missing: the RF vanishes for
   microseconds; the deck's dropout compensator substitutes the previous line).
10. **Tracking.** A linear control track (CTL, one pulse per frame) lets the playback servo place
    the heads over the tracks; the **tracking knob** is a delay of that servo — a lateral
    **head-to-track offset**. When the recorder and the player are different decks, their scan
    angles differ slightly and the offset changes ALONG the field: where it passes the edge of the
    track the head reads the other-azimuth neighbour, the FM fails and a NOISE BAR appears; the knob
    moves it up and down.
11. **Head switching.** The output switches from one head to the other ~**6.5 H before the vertical
    sync**: the last lines of the field come from the start of the next head's track, with the
    switching transient and that head's **timing skew** — the tear at the bottom of a VHS picture.
12. **Playback: FM demodulation.** A limiter and a discriminator turn the read RF back into the
    instantaneous frequency; a carrier drowned in noise gives impulsive "clicks" (the white and black
    sparkles of a worn tape). Then the luma LPF and the **de-emphasis** (the inverse of step 2).
13. **Playback: chroma.** The colour-under is separated by a low-pass, up-converted with the
    rotation undone, and **comb-filtered**: 1 H (NTSC) or 2 H (PAL) — after de-rotation the
    neighbour's leak alternates in sign line to line and cancels; ACC normalises its level.
14. **Time base.** Tape stretch, capstan wow and drum flutter move each line in time; a deck **with
    a TBC** rebuilds the lines on a stable clock, **without** one they jitter.
15. **Speeds and transports.** **SP/LP(/EP)** change the track pitch, not the wavelengths. **Still**:
    the tape stops, the scan path crosses one pitch per field, so a two-head deck's head runs off its
    track mid-field (a noise bar in the pause). **Four heads**: a second, narrower pair for LP/EP
    (search confirmed — sources 5–6); vendors also use it for a cleaner still/slow motion (search
    did NOT confirm the exact pairing — modelled as "read both fields with heads of the track's
    azimuth": a noise-free field still).
16. **One deck or two.** One combined deck plays its own recording with its own geometry (no
    interchange error, its own head defects twice). Two decks: the record drum's defects are baked
    into the tape; the play drum adds its own, and the scan-angle mismatch makes tracking errors.

The linear and HiFi AUDIO tracks (hiss, wow, a 10 kHz linear track, the HiFi FM pair under the
video) are a separate audio effect, not part of this one (design: decisions).

## Pros

- The defects come out of causes, so their DEPENDENCIES are right without tuning: the noise bar
  moves with tracking, a clogged head kills every other field, LP gets noisier and keeps its
  sharpness, colour is soft and stable while luma edges wobble.
- The parameters are the quantities a technician measures (μm, MHz, %), so presets are devices —
  "a clean deck", "dirty heads", "a chipped head", "a worn rental tape" — not "amount: 0.7".
- Two decks are expressible: a defect of the recorder survives playback on a good machine.

## Cons

- Cost: a line of the signal is ~1000 samples at 4 fsc; every stage is a filter over it. The
  imitation is a handful of operations per pixel; this is hundreds (measured in the change's
  design).
- The output is fixed at standard definition: a 4K frame becomes a VHS-resolution picture. That is
  the point, but a person who wants "a little VHS feel at full sharpness" wants the imitation.
- A frame-pure effect has no previous frame: the neighbour track's content is taken from the other
  field of the SAME frame (exact for still pictures; for motion the neighbour field is 20 ms older
  on a real tape).

## What users say

NO QUOTES: searched "ntsc-rs VHS emulation", "Sapphire S_VHSDamage", "Red Giant Universe VHS"
(2026-11-01). The results were vendor pages and summaries of a blog post on ntsc-rs that paraphrase
rather than quote; no forum thread with a verbatim complaint about a look-only VHS effect opened
here. The demand is the owner's own (`docs/ktprompts/kkt-26.11.01.txt` line 40).

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-fx/
Done as the barrier plugin `vhs_tape` (`src/plugins/fx-video-tape`, change `vhs-tape-simulation`): the chain above on the standard's 4 fsc grid, CPU reference and GPU graph; the imitation `vhs` stays.

## Estimate

| What | Answer |
|---|---|
| Scope | L — one round: the plugin crate, two small contract extensions, the panel |
| Where the main work is | a new family crate `src/plugins/fx-video-tape`; `kt-plugin-api` barrier; `kt-gpu` engine |
| Project format | does not touch it: a new plugin id |
| Tests that will have to be written | physical consequences: FM round trip, colour-under bandwidth, a clogged head's fields, the moving noise bar, azimuth crosstalk, determinism, CPU against GPU |
| Depends on | F-FX-007 (the catalogue), plugin-api-barriers |
| Breaks compatibility | no |

## Verdict

**Done** (round kkt-26.11.01, V1, 2026-11-01): the barrier plugin `vhs_tape` beside the
imitation, its parameters physical quantities, its look the result of the chain above (change
`vhs-tape-simulation`, tests t1180–t1193). Left for later: the linear and HiFi audio tracks as an
audio effect, nonlinear emphasis, SECAM and S-VHS.

## Sources

1. vhs-decode, `vhsdecode/format_defs/vhs.py` — https://github.com/oyvindln/vhs-decode (FM levels,
   `deemph_tau`, `color_under_carrier`, `chroma_rotation`; accessed 2026-11-01)
2. SMPTE ST 230 / the VHS SMPTE documents — https://pub.smpte.org/doc/st230/19961010-pub/st0230-1996_stable2010.pdf
   (drum 62.00 mm, writing speed 5.80 m/s NTSC, azimuth ±6°, track angle 5°58′9.9″; search summary)
3. Sencore Tech Tip 189 "Comparison of VCR formats" and 135 "How to set VCR FM modulator
   frequencies" — https://xdevs.com/doc/Sencore/Sencore_Tech_Tips/ (3.4/4.4 MHz, 1 MHz deviation)
4. US 4881134 "Comb filter for the rejection of intertrack chrominance crosstalk" —
   https://patents.google.com/patent/US4881134 (PS ±90°/line per channel; 1 H NTSC / 2 H PAL comb)
5. HowStuffWorks, "How VCRs work" — https://electronics.howstuffworks.com/vcr4.htm (four heads: a wide
   pair for SP, a narrow pair for slower speeds)
6. digitalFAQ forum, VCR repair threads — https://www.digitalfaq.com/forum/vcr-repair/7795-panasonic-nv-hd620.html
   (separate head pairs for SP and SLP; search summary)
7. ntsc-rs — https://mltframework.org/plugins/FilterOpenfx-wtf-vala^ntscrs/ (its OFX parameters: tape
   speed as an HF cutoff, head switching, tracking noise)
8. Sapphire 2025 S_VHSDamage — https://borisfx.com/videos/sapphire-2025-vhs-damage-basics
