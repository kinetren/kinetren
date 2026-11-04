# Test fixtures

Small real media files for conformance/golden tests (tests 10, 11). They are generated locally by the script `gen.sh` (it will appear in M3)
via ffmpeg: aac 44.1k mono, flac 48k stereo, h264, vp9, muxed A/V — about 2 seconds each.

The generated files go into `out/` (they are not committed to git, see .gitignore) and
are checked by hash before use.

## fake_va

`fake_va/ktfake_drv_video.c`: a VA-API driver without a GPU, built by the tests that use it (t1551–t1556,
live `hwencode`). It runs the hardware export path through the real libva and FFmpeg on any Linux
machine, and fails on request (`KT_FAKE_VA`). It proves our path, never a real driver.
