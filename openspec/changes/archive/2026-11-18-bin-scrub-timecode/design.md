# Design

## D1. The file's own time, the project's timecode form

The moment is the file's time from its start (FCP's Skimmer Info shows SOURCE timecode: in the bin
there is no project time yet). It is written with `util::format_timecode` at the project's fps, the
form the timeline and the timecode mask use, so a person reads one notation everywhere.

## D2. One mapping

`frac = (hover_x − left) / width`, clamped to `[0, 0.999]`, times the duration — the same expression
the thumbnail key is taken from. The picture is the NEAREST stored thumbnail (up to half a ladder step
away); the number is the cursor's moment. No second mapping exists to disagree.

## D3. No obvious label, no new strip (UI rules 9, 13)

The number alone, over the bottom of the thumbnail on a 170-alpha black backing, centred. A narrow card
takes the first of four forms that fits `card width − 8`: full → without "00:" hours → without the
frame → without the milliseconds.

## D4. Cost at rest

The label exists only on frames the pointer already causes (a hover). The galley is kept in
`KtApp::bin_scrub_tc` keyed by (media, moment, fps, room) and replayed (`Painter::galley`), so a hover
frame on the same moment formats nothing (t3101). No decode, no wake, no lane job.

## Class walk

The mechanism: a scrub that shows a picture of a moment without the moment's number. Checked where a
moment is scrubbed by hovering: the bin's file cards (fixed here); the bin's AUDIO cards draw a
waveform with no hover scrub at all (no moment is picked — nothing to label); the reel cards draw a
mini-map, not a scrub; the timeline's filmstrip is not hover-scrubbed (the playhead and the ruler
carry the time); the FX picker's cells follow the playhead, not the pointer. Only the file card had
the gap.
