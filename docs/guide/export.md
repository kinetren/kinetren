# Export and kt-cli

## Render to MP4

![The render settings](../images/render-dialog.webp)

**File → Render to MP4…** (or the render button) exports the region, or the whole project
when no region is set.

| Setting | Values |
|---|---|
| Video codec | *Auto (hardware first)* or any encoder your FFmpeg can open |
| Quality | CRF 14–32 (default 23) or a bitrate of 0.5–80 Mbit/s |
| Audio bitrate | 128, 192, 256 or 320 kbps — AAC, 48 kHz stereo |
| Resolution | the project's, 720p, 1080p, 1440p, 2160p, or the largest source |
| Container | MP4 with faststart |

The file is named after the project, region marker and length, e.g.
`kt-song-chorus-4s.mp4`. Before starting, the dialog warns about missing sources and
about muted or solo tracks.

Rendering runs inside the editor through the system's libavcodec and libavformat. There
are no temporary files and no external `ffmpeg`. It uses the same composition code as
the preview, so the file contains what you saw. It can be paused.

## kt-cli

`kt-cli` is the same engine without a window:

```bash
cd src
cargo run -p kt-cli -- <command> …
```

| Command | What it does |
|---|---|
| `version` | print the version |
| `new <out.kt>` | an empty project |
| `demo <out.kt> [--files]` | a demo project on synthetic sources (renders with no media at all) |
| `showcase <out.kt> --sources <dir>` | a dense montage cut from every video in a directory — the README project |
| `import <in.otio\|in.ktproj> <out.kt>` | import OpenTimelineIO or an old Kinetren project |
| `export-otio <in.kt> <out.otio>` | export OpenTimelineIO |
| `roundtrip <in.otio> [out.otio]` | read and write OTIO back (a check of the exchange door) |
| `render <in.kt> <out.wav> [--allow-missing]` | render the audio |
| `export <in.kt> <out.mp4> [encoder]` | render video and audio to MP4 |
| `probe <in.kt> <from_s> <to_s> [--time]` | what plays in that range: effective events after morph layers, prefetch order, decode order; `--time` measures whether audio rendering keeps up with real time |

`--force` lets any command overwrite an existing file. Without it, nothing is
overwritten.

Judge speed only on a release build (`cargo build --release -p kt-cli`); the debug build
is several times slower.

## OpenTimelineIO

**File → Export ▸ OpenTimelineIO…** writes a self-contained `.otio`:

- standard schemas for everything OTIO can express;
- Kinetren's own data under `metadata["kt"]`;
- data from other programs kept untouched.

**File → Import older Kinetren project…** reads an `.otio` back into a new `.kt`.
