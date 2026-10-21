# Spec Delta

## ADDED Requirements

### Requirement: kt-cli showcase builds a dense project from a directory of real footage

`kt-cli showcase <out.kt> --sources <dir> [--force]` SHALL write a valid project file built
from the media files found directly in `<dir>` (non-recursive, ordered by file name, media
durations and streams probed through the same media layer the editor uses). Video files
feed the picture tracks; audio-only files (music) feed the music beds and never land on a
picture track — without any, the beds come from the longest videos with sound. The project
SHALL contain, at minimum: three named video tracks and three named audio tracks plus one
morph track; on the main video track a montage of at least 24 events drawn from every source
file, with linked audio partners and at least three different transition kinds; at least one
overlay event with non-default on-screen geometry, at least three different video FX, at
least one title generator event; on a music track an audio event carrying Slowed+Reverb
whose `rate` and `mix` parameters are each automated by a curve of at least four keys; one
morpher batch; at least four named markers; a region with loop enabled; a media bin grouped
by role. Media SHALL be referenced by absolute path and MUST NOT be copied. For the same
directory contents the output SHALL be byte-identical between runs (no wall-clock, no
randomness without a fixed seed). A missing directory, one without any readable video file, or one where no video
file has an audio stream (the music bed needs one) SHALL fail with a non-zero exit code
and a message naming the directory; an existing
output file MUST NOT be overwritten without `--force`, exactly as for `new`/`demo`. Project
settings SHALL be 1920×1080 at 25 fps regardless of source resolutions, so sources of
different size and aspect exercise the conform rule.
Why: the README must show a timeline "packed with all kinds of editing" cut from real
footage (owner, 2026-10-21); a hand-built project can neither be re-created after a format
change nor re-shot after a UI change, and `demo` must stay synthetic (t388).

#### Scenario: Showcase from fixture media is rich, valid and deterministic
- **WHEN** `kt-cli showcase` runs twice on a directory holding copies of the generated fixture videos (`src/fixtures/out/*.mp4`) and one audio-only fixture (`sine48k_stereo.flac`)
- **THEN** both runs exit 0 and the files are byte-identical; the reloaded project validates; it has ≥ 24 main-track events from every source, ≥ 3 transition kinds, an overlay with geometry, ≥ 3 video FX kinds, a title generator, a Slowed+Reverb event on the audio-only file with `rate` and `mix` curves of ≥ 4 keys each (the music file is on no picture track), a morpher, ≥ 4 markers, a looped region; every media path is absolute and points into the directory
- **TEST** t824 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Missing or empty sources and existing output are refused
- **WHEN** `showcase` gets a non-existent directory / a directory with no video file / an existing output path without `--force` (the no-audio refusal shares the code path of the no-video one)
- **THEN** non-zero exit, message names the directory (or mentions `--force`), the existing file is untouched; with `--force` it is written atomically and no `.tmp` remains
- **TEST** t825 (`src/apps/kt-cli/tests/cli.rs`)
