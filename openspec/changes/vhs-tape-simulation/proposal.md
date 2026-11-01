# Proposal: vhs-tape-simulation

## Why

The owner, kkt-26.11.01 (registry row V1, `docs/ktprompts/kkt-26.11.01.txt` line 40): besides the
"frankly cheap" `vhs` effect, a REAL VHS filter that conveys the physics of the process — really
write a magnetic signal onto a tape and really read it back; drums of good quality, with dirty video
heads, with four heads instead of two (still frame), with one of two heads chipped; two drums
(record and play) or one doing both. His answer to question 4 (`docs/user-feedback.md`, 2026-11-01):
"you pay a lot of attention to the UI and completely ignore the complexity of the effects' physical
mechanics". The root `CLAUDE.md` rule written in the same round: an IMITATION of a physical effect
is not a SIMULATION of it.

The existing `vhs` (`src/plugins/fx-video-stylize`) is an imitation: an RGB shift, scan lines and
row jitter. The reconnaissance (`docs/features/fx/simulate/vhs-tape-simulation.md`, F-FX-009) lays
out the physical chain; nobody in the market writes a tape and reads it back (ntsc-rs, the nearest,
stops at the baseband).

## What Changes

- **A new barrier plugin `vhs_tape`** ("VHS tape (simulation)") in a new family crate
  `src/plugins/fx-video-tape`. It turns the frame into the VHS signal at a standard-definition grid
  sampled at 4 fsc (PAL 1024×576, NTSC 832×480 samples), and runs the physical chain:
  luma pre-emphasis and clip → FM (3.8–4.8 / 3.4–4.4 MHz) → colour-under (626.953 / 629.371 kHz with
  the per-track phase rotation) → recording with tape saturation → helical scan geometry (track
  pitch per speed, head widths of a 2- or 4-head drum, tracking offset, scan-angle mismatch, still)
  → the channel of each head (spacing loss, gap loss, azimuth loss of the neighbour track as an
  integration along w·tan 12°) → tape and amplifier noise, dropouts → head switching → FM
  demodulation (Hilbert discriminator), dropout compensation, de-emphasis → colour-under
  demodulation with the rotation undone, ACC, the 1 H / 2 H comb → time base with or without a TBC.
  Its parameters are physical quantities (µm, µs, dB, per-head and per-drum), with presets
  ("good deck", "dirty heads", "clogged head", "chipped head", "4-head still", "worn rental tape",
  "LP"). One deck or two (the record drum's defects are written into the tape).
- **The barrier contract grows by two additive pieces** (`kt-plugin-api`, `kt-gpu`, `kt-fx`):
  - `Node::PassAt` — a plugin pass rendered at its own size (≤ the intermediate ceiling), so a
    signal grid independent of the frame size lives on the GPU; `check_graph` tracks its size;
  - `BarrierKernel::frame_values` / `gpu_values` — a kernel reads the passport values directly
    when its cooked form does not fit eight numbers; the defaults cook as before, so every existing
    barrier keeps its bytes.
- **Two parameter units**: micrometre and microsecond (`ParamUnit::Um`, `ParamUnit::Us`).
- **The panel**: the effect's window shows a deck face drawn from the shared widgets (a drum with
  its heads coloured by their condition, the speed switch, the tracking knob, the head-condition
  selector); the parameter grid stays under it. Secondary to the physics.
- The imitation `vhs` is unchanged and not renamed.

Out of scope (decided): the linear and HiFi audio tracks — an audio effect of its own, not part of a
video kind; a candidate in the note.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `video-fx`: a physical tape simulation kind and its consequences as requirements.
- `plugins`: the barrier contract's pass at its own size and raw-value kernels.

## Impact

- New crate `src/plugins/fx-video-tape` (workspace member, assembler `kt-plugins-builtin`).
- `src/libs/kt-plugin-api/src/{barrier.rs,lib.rs}`, `src/libs/kt-gpu/src/engine.rs`,
  `src/libs/kt-fx/src/video.rs` — small hunks.
- `src/apps/kt-app`: registry order, catalogues ×5, units, the deck face.
- Project format: a new plugin id, no format change; OTIO export carries it as every plugin kind.
- Tests t1180–t1209.
