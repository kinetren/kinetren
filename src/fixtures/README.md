# Test fixtures

Small real media files for conformance/golden tests (tests 10, 11 from
`docs/porting-notes.md`). They are generated locally by the script `gen.sh` (it will appear in M3)
via ffmpeg: aac 44.1k mono, flac 48k stereo, h264, vp9, muxed A/V — about 2 seconds each.

The generated files go into `out/` (they are not committed to git, see .gitignore) and
are checked by hash before use.
