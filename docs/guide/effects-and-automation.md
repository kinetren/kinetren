# Effects and automation

Double-click an event (or choose **Effects…** in its menu) to open its effects window.
A linked pair opens as one window with a section for the picture and one for the sound.

## The chain

The top row of each section is the chain: a master switch, then one slot per effect with
its own switch and a cross to remove it. Drag slots to reorder them. **+ effect…** opens
the picker, **Preset…** saves or applies a whole chain, **Copy the chain** and **Paste the
chain** move chains between events (curves included). The first slot of a video event is
**Geometry** — position, zoom, rotation, crop and anchor — and it cannot be removed.

![The video effect gallery](../images/video-fx.webp)

The picker is a gallery: each video effect and each preset variant is previewed on the
event's own frame. Type to filter.

**Video:** Geometry, Mirror, Corner Pin, Blur, Sharpen, Radial Blur, Greenscreen, Drop
Shadow, Edges, Noise, VHS, Event Grid, Vignette, Pixelate, Halftone, Glow, Pan 3D,
Cylinder 3D, Color Grade, Duotone, Posterize, Chromatic Aberration, Lens Distort, Twirl,
Ripple, Kaleidoscope — and Slowed+Reverb, which slows the picture too.

**Audio:** Gain, Slowed+Reverb, Pitch Shift, Filter, EQ, Distortion, Bitcrusher, Tremolo,
AutoPan, Vibrato, Chorus, Flanger, Phaser, Delay, Compressor, Gate, Limiter, Stereo
Width. Effects that look ahead (the limiter, pitch shift) are latency-compensated, so
the sound stays in sync with the picture.

## Slowed+Reverb

![Slowed+Reverb with automated rate and mix](../images/slowed-reverb-automation.webp)

- **Rate** slows the material (0.86 is a classic "slowed" feel).
- **Mix** sets how much reverb is heard.
- **Delay, Damp, Decay, Wet, Pre-delay and Width** shape the reverb itself.

On a linked pair the picture slows down with the sound. The video side has its own
switch in the chain, so you can slow only the sound.

The rate is a real time map. The event's filmstrip and waveform show the material it
actually plays, and the timeline cursor stays in sync with them.

## Automation curves

Any numeric parameter can change over time:

1. Press **+ curve** under the parameters and pick the parameter. It appears as a
   coloured button above the graph.
2. Click on the graph to add keys and drag them with the mouse. The parameter field
   always shows the value at the playhead; editing the field edits the key under the
   playhead.
3. Each key sets the shape of the segment after it:
   - linear, smooth, ease in, ease out;
   - hold (a step);
   - zigzag (triangle, square or sine, with periods and amplitude);
   - a formula.

   A curve can also be *linked* to another property, with an expression.
4. Switch **Graph** / **Table** to edit the same keys as numbers.
5. **Copy the envelope** / **Paste the envelope** move curves between parameters, events
   and even different effects. Matching fields are pasted silently; for the rest you
   choose.

Keys are stored in the time of the source material. Trimming or moving an event never
shifts its automation relative to the picture.

The same curves drive transitions, generators (an animated title is just curves on the
title), the Geometry node and published reel parameters.

## Track effects and ducking

Every track has its own chain in the track window (right-click the header). The
compressor has a sidechain key: put it on the music track, key it from the dialog track,
and the music ducks under speech — that is the **Audio ducking** section of the track
window.

Next: [The morpher](morpher.md).
