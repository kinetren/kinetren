## ADDED Requirements

### Requirement: A generator keeps its share of the short side in a format
In a format of another shape than the master, a title and every generator row sized by the frame's height SHALL keep their
share of the frame's short side, and a title's lines longer than nine tenths of the width SHALL break at spaces by the
title's own font advances. A title with a running value or an animated offset SHALL NOT be broken. The master's frames and
the file SHALL not change.
Why: the option he chose (A17, Q26 a) says "a title reflows to the narrow width"; a size by the height alone makes a title
1.8 times larger in a frame 1.8 times narrower.

#### Scenario: A lower third in a vertical
- **WHEN** titles of 22 and 33 letters at size 0.07 with a curved size, a crawl, a timecode title and a plate of 0.2 of the height are drawn in 1920×1080 and in 1080×1920
- **THEN** in the vertical the letters are as high as in the master, the first stands on one line and the second on two, nothing passes the frame's edges, the crawl and the timecode are on one line, the plate keeps its share of the short side; the master's frames are the stored ones
- **TEST** t4236 (`src/libs/kt-engine/tests/frame_formats.rs`), t4237 (`src/libs/kt-text/tests/wrap.rs`)
