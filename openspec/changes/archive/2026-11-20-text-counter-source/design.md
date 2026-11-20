# Design

## D1. A source of the text, not a kind

His G3 answer: the timecode becomes a MODE of the Text generator so the full text passport works on
it. The stopwatch and the countdown follow the same path: one kind, `source` chooses where the shown
string comes from; `title_text_at(kind, pts)` is the one function that makes it (kt-model), and
`kt-media::text_params` asks it with the source's `pts` — the MATERIAL tick, as the timecode kind did,
so trimming the event's start moves the counter with the material. Integer ticks throughout: the frame
is a function of `(kind, pts, quality)` (invariant 4/11).

## D2. Up is floor, down is ceiling

A stopwatch shows what has passed (floor to the shown unit); a countdown shows what is left (ceiling):
at 0.2 s of a 5-second countdown it still says 5, and it says 0 only when the time is up, then holds.
This is how a broadcast countdown reads; a floor would show 4 for nearly the whole first second.

## D3. Bytes and format

The four fields are `skip_serializing_if` at their defaults: a typed title writes the former JSON and
gen path (t3704); `has_text_sources` makes the file format 7 only with a running title — an older
build would read the title without its source and show and save the typed text. Two number rows
(`count_from`, `count_fps`) join the title's passport AT ITS END, so the index addressing of R.59 is
stable; t1048's "tuned" title row (every number at 70 %) carries them, its default row does not move.

## D4. The window

A row of buttons under the text, the alignment's look; the format row for a stopwatch or a countdown;
the grid hides `count_from` for a typed title and `count_fps` for anything but a timecode (rule 9). A
note under it says the value runs with the event's own time and how `{}` places it.

## Class walk

The mechanism: a value that is text drawn by a pixel font, outside the text passport. Checked: the
timecode kind (G3 — its removal and the import conversion are G3's own change; it stays here), the
countdown leader (a picture with a digit — a leader, not a title), the sync flash (no text), the bin
scrub timecode (UI, not a picture).
