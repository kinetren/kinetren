# Proposal: decode-waste

## Why

Found-defects row 712 (render-speed, kkt-26.11.15 N2–N4: "the preview, the final render and processor use
are too slow"): on the heavy showcase a 20–30 s stretch decoded 12,289 pictures for 1,924 used, with 157
decoder reopens — in `kt-cli export` and in `kt-cli preview` alike (10,326 for 1,982).

## What Changes

- Every video read of a file (`read_video_frame`, `read_video_span`, `read_video_span_qualities`) goes
  through one rule of choosing a decoder per key: a run goes on while its decoder is the nearest that
  reaches the next key; two places of one file read together keep a decoder each. Keys closer than 2 s
  used to be one run on one decoder, which walked from the first place to the second and sought back to
  the first in the next chunk.
- When every decoder of a file is taken, the one a later key of the same request would be given is not
  the one sent away (it was the least recently used, often the next run's).
- Decoders per file follow the picture: two for 4K, up to four for a file of a quarter of 4K's pixels or
  less — the frame pools hold no more than two 4K decoders' did.
- A span at one size seeks per key, as a span at several sizes does; it sought only before its first
  key and decoded every picture up to a far one (reel strip, effect thumbnails, a stream stage's history,
  the export's retry).
- What stays: most of the remaining pictures are each cut's lead-in from its keyframe (4–5 s GOPs of the
  showcase footage against 0.4 s cuts). That cost is the material's and is filed as found-defects row
  (a lead-in reused across the timeline needs a decoded-picture tier or proxies).

## Capabilities

### Modified Capabilities
- `media-index`: one decoder choice for every read of a file.

## Impact

kt-ffmpeg (`video_reach_from`), kt-media (`ffmpeg.rs`: `read_runs`, `cursor_for`, `goes_on`,
`cursors_for_size`; tests t2880–t2883). No format change; OTIO unaffected; decoded frames and export
bytes are the same (the frame of a key does not depend on the decoder that read it).
