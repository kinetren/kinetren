## Context

`gpu-render-hardening` left a render on a software adapter at 8.7 GB and 4 devices on the showcase. The
CPU build is not coming back; it stays a yardstick.

## Decisions

### D1. The end of a file is met once

The packet scan at opening stops where the file stops, so `video_keyframes` holds what is really there,
whatever the header says. Three things followed from reading only the header:

- `prepare_for` sought for any target past the 4 s forward bound. Past the real end every key was such a
  target, and each seek reopened the decoder and decoded the last GOP again. It now decodes on when no
  keyframe lies between the cursor and the target: a seek would land behind the cursor. This also holds
  in the middle of a long GOP.
- A seek is clamped to the last keyframe the scan found: an MP4's index names samples past the cut, and
  a seek to one finds no packet.
- The audio sought once per burst past the end, and on a Matroska file with no cues each seek walks the
  clusters. Past the fifo's end with the stream ended it answers at once.

`cursor_for` asks the same rule (`video_reach`), or a second decoder opens for the same end of file. A
packet the file ended inside of (`AV_PKT_FLAG_CORRUPT`) is not sent to the decoder or indexed: the
decoder refused it, and the read failed.

Counters on the 12 s fixtures: t1760 301 reopens to 1, t1762 130 seeks to 1, t1763 62 reopens to 1. A
40 s event over the end of a cut 4K-class VP9: not done in 280 s before, 33 s on one device at load 60.
