# Design

## Decisions

### D1. The rule is the model's, the durations are the index's
`lint::past_end(project, range, |media| -> Option<Tick>) -> Vec<PastEnd { clip, at, missing, kind }>` walks events that
meet the range (a reel's instances are not opened: an event's OWN file is what can run out) and compares the material
span it reads (`clip_material_at` over the rate profile, signed, ramps included: the LARGEST material tick read) with the file's
length, with half a frame of tolerance (one file of 25 fps read on a 30 fps grid ends inside its last frame). `None`
from the closure (a file not indexed yet) yields nothing. A still, a generator and a looped source never run out. The
frame-hold is not a run-out (a hold is the person's own act).

### D2. Shown when the window opens, kept while the project is
The lines stand above Start, in the collapsing group the loudness report uses when there are more than three; with none
there is no group and no added height (idle cost zero: the list is computed when the window opens and when the range
or the project's revision changes, not per frame).

### D3. Truncated
The index knows a file's header length and the last decodable moment (`kt-ffmpeg` gives it at end-of-file); the export
log line is written by the producer that met it, once per file. The window lists the files the index has already seen
truncated; the others come in the log.

### D4. Class walk
The same silence elsewhere: the preview (a held last frame) — a hatch on the event's tail past its file is a timeline
matter, not asked; the audio of a linked event past its file — counted as `kind: Sound`; a reel instance longer than its
reel — the same lint on the instance's length; a generator's stream past its declared end — has none; an OTIO import
with a wrong length — same events, same lines. Transitions read past an event's edge (handles) are by design.

## Open Questions

None.
