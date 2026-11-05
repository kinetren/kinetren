# Design

## Context: the cause, measured (2026-11-04)

Temporary logging on the demo at 1080p (`kt-cli export`, libx264, `missing layers 5`):

- The cache stays full for the whole export: nothing removes a written frame, the budget does. It
  reached 957 MiB of its 960 at frame 120, and stayed there.
- The first unread eviction came at chunk 252..258 with the ledger at 759 MB, and there were none
  before it: losses need the look-ahead near its limit.
- Frame 232 lost both its layers with the writer at 216: it was composed 16 frames ahead of the
  writer, in a chunk decoded about 40 frames earlier.

So: `get_video` from the composite refreshes the LRU stamp of frames that are being composed and will
never be needed again, and these move ahead of the chunks decoded but not yet read. The live data
(the ledger's 759 MB plus a chunk) and about 100 MB of freshly read dead frames do not fit in 960, and
the oldest entry is a LIVE one. Which entry that is depends on how far the workers have got, so
the loss differs per run. The ledger's own assumption ("the frames fit, so LRU keeps them") is
wrong because LRU's order is not the order of need.

The loss needs a look-ahead far larger than the frames in flight at the workers, because only then
do frames wait decoded and unread. It is not a plain ratio of budget to frame size: on the old code a
quarter of the budget at 1080p lost nothing, nor did any budget tried at 270p with this delay but one,
and 640×360 at a quarter lost 4–8 layers on every run. So t1601's size and budget are proven values.

The serial export cannot do this: it composes a chunk straight after decoding it. A cold look at the
showcase shows why it was not seen: its composite is slow and its look-ahead never approached the limit
(0 missing, md5 equal in two runs), and t1451 renders 90 frames at 720p.

## Goals / Non-Goals

**Goals:** the pipelined export gives the serial stream, every run, at any relative speed of the
stages. A frame without a layer never goes unreported. No loss of throughput.

**Non-goals:** a third way out for a frame that cannot be decoded at all: the export reports it and
goes on, as the owner's rule for unavailable sources does (a render proceeds and says so).

## Decisions

### D1. Holds in the cache, not a cleverer order
`BurstCache` gets a per-key hold count: `hold_video` (present? then +1), `insert_video_held`
(atomic with the insert), `release_video`, `held_video_bytes`. Eviction skips a held entry, as it skips
a pin, and the budget may be exceeded for it. Each chunk holds every frame it names, whether it
decoded it or found it; the writer's progress releases them.
Rejected: `refresh` in chunk order (a second LRU, still a race between stamps); a count instead of a
flag is needed because `pin_range` is one bit and a frame is shared by chunks; removing a written
chunk's frames (a frame the next chunks share would be decoded again, and a read of a dead entry still
reorders); a store of its own in the pipeline (the composite reads the cache: one code path).

### D2. The look-ahead is the bytes held
The ledger summed the bytes a chunk DECODED: a frame found in the cache was counted by nobody, though
the chunk needs it. `held_video_bytes` counts each frame once, for as long as any chunk needs it, so
decode waits on the truth. The budgets are unchanged, so memory does not grow: the live set fitted
under them before, and dead frames still go first.

### D3. The check happens at the moment of the hold
`chunk_requests`' check and the hold are one step (`hold_video` answers "present" under the cache's
lock). A check then a hold would leave the same window the defect lived in.

### D4. A frame without a layer is reported where it happens
With holds, and the retry of D7, a key is absent only because the decoder cannot give it, so waiting
would wait for nothing. Each stage knows `complete`; the writer (the pipeline) and the loop (serial) pass the frame's
time in frame order, and consecutive frames become one `ExportLog::IncompleteFrames { from, to, media }`.
The media names come from `VideoRenderer::absent_layers`, asked when the frame is composed, when the
cache still shows what was missing. The summary line stays, and the log opens by itself.

### D7. A frame a span dropped is asked again before it is composed
Found while writing t1602: the serial export runs `stream_preroll` before every frame, and besides
its stream stages that re-reads any frame missing from the cache. So a span that fails half way, which
drops every key after the failure, heals there one frame later, and the pipeline has no such step:
one failed span cost 46 frames in t1605. `decode_requests` now asks each key the span did not give
again, alone, as the pre-roll does per frame, inserted held. This is the "export waits for the layer"
half of the rule; D4 is what is left for a frame no decoder can give.

### D5. "Equal to serial" is the streams, at the same thread count
The two FILES cannot be equal. `threads=auto` against the serial's `1` makes a different x264 stream
(t1456, on purpose), and the serial writes all video before the audio, so the file's interleave differs.
Equality is held at the encoder's input by t1601, frame by frame, and on the demo's file by comparing the
packet streams with `--encoder-threads 1`.

### D6. The window's words
`ExportError::Io` displays the message alone: "io:" names a Rust type. A failed run that opened no
encoder shows "no encoder opened" in the Video row, a new catalog key in five languages.

## Risks / Trade-offs

- [A hold never released keeps a frame for the whole export] → the export's cache dies with it, and
  t1600 holds the counts.
- [Held bytes over the budget on a project of huge frames] → the same as before, a chunk is at least a
  frame; decode waits as soon as held bytes pass the look-ahead.
- [Which frames are found in the cache still depends on timing, so the set of keys asked from a
  decoder does too] → seeks are sample-accurate (t10), and the showcase's run against `--serial` is
  the check on real files.

## Migration Plan

Nothing stored changes. Rollback = revert.
