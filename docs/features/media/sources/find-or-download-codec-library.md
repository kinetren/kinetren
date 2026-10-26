---
id: F-MED-007
title: The codec library is the user's — find it, or download it on a click
status: done
priority: A
cost: M
category: media/sources
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08, audacity-4.0]
we_have: yes
crates: [kt-ffmpeg, kt-media, kt-app, kt-cli]
invariants: [9, 10]
ui_rules: []
round: "windows-build"
point: "change windows-build"
tests: [844, 845, 846, 847, 848, 849, 850]
depends_on: [F-RES-006, F-RES-008]
trackers: []
services: [gyan-dev]
checked: 2026-10-24
---

# F-MED-007 — The codec library is the user's — find it, or download it on a click

## Essence

The editor installs the program, and it does not open a single video: the codecs are not
in it, they belong to a library the person has to have. On Linux the package manager puts
it in place, and the question never comes up. On Windows nobody does that. The person needs
the program to tell them, in one place, whether it found the library, where, which version,
why every other copy on the disk was not good enough, and to offer to fetch the right one
without leaving the program.

## Who has it and how

| Editor | Version | Called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no | the decoders ship with the program (File I/O plug-ins) plus what Windows provides; there is nothing to find |
| Adobe Premiere | 26.x | no | its own media engine, licensed codecs inside the install |
| DaVinci Resolve | 21 | no | its own decoders inside the install; codec gaps are answered by "Studio" or a transcode, not by a library |
| Final Cut Pro | 12.3 | no | AVFoundation of the platform; the library is the OS's |
| Adobe After Effects | 26.x | no | the same media engine as Premiere |

**Outside the core.** The five commercial editors buy their codecs and never meet the
problem. It exists only for programs that use FFmpeg and do not ship it: Kdenlive answers it
by bundling FFmpeg in its Windows installer (the question is closed by shipping), and
Audacity is the only well-known program that does NOT ship FFmpeg on Windows and finds the
user's copy.

| Program | Version | Called | How it works |
|---|---|---|---|
| Kdenlive | 26.08 | Windows installer | FFmpeg DLLs are shipped next to the exe; nothing to find, and the version is the one the release was tested with |
| Audacity | 4.0.1 | Preferences → Libraries: Locate… / Browse… | FFmpeg is not bundled (patents); Audacity looks for `avformat-NN.dll` of the versions in its table, the person can point at the DLL by hand, and the support page sends them to a third-party installer that puts FFmpeg at a fixed path Audacity checks |

## The trick behind it

- **The version is in the FILE NAME, and the program carries a table of names.** On Windows
  `avformat-61.dll` is FFmpeg 7, `avformat-62.dll` is FFmpeg 8; Audacity's page keeps a row
  per release ("3.5+: adds avformat-61"). A new FFmpeg on the user's disk is simply a name
  the table does not have yet — which is exactly the complaint users write.
- **The fixed path beats every heuristic.** Audacity's recommended route is an installer
  that puts FFmpeg into one known folder; after that the program "should now automatically
  detect FFmpeg". The search works because someone guaranteed where the files are, not
  because the search is clever.
- **"Full shared build, not individual DLLs".** The support page says so explicitly: a set
  of DLLs gathered from different places is the failure mode, because Windows binds an
  import by base name to whatever with that name is already loaded or first on the path.

## Pros

- The program carries no codec licence and no patent exposure: the person fetches the
  author's build from the author's server.
- One binary works with several FFmpeg majors, so a newer library on the machine does not
  require a new program release — until a major the table does not know.

## Cons

- The first start on Windows does not play anything until the person acts.
- A newer FFmpeg (a name not in the table) is rejected even though it would work after a
  loader update; the table must follow FFmpeg's releases.
- The most popular one-line install (`winget install Gyan.FFmpeg`) is the STATIC build:
  an `ffmpeg.exe` with no DLLs at all, and the program has to say exactly that.

## What users say

> "These are from later versions of ffmpeg. How would it know about them?"
> — mwhelm, Audacity forum, 2025-04-01, https://forum.audacityteam.org/t/ffmpeg-issues-windows-11/135281/15

## How it fits Kinotrain

CAPABILITY: openspec/specs/media-index/
Decision: find the user's shared FFmpeg in a fixed order of folders (`src/libs/kt-ffmpeg/src/loader.rs`), load all five DLLs from one folder, name every rejection, and download one pinned gyan.dev build on a click; invariants 9 and 10.

## Estimate

| What | Answer |
|---|---|
| Scope | M — two stages of the windows-build change |
| Where the main work is | kt-ffmpeg (search plan, loading), kt-app (window, download job) |
| Project format | does not touch |
| Tests that will have to be written | the pure search plan on any OS; loading under wine/Windows; download with hash and atomic install; the window's user path |
| Depends on | F-RES-006, F-RES-008 |
| Breaks compatibility | no |

## Verdict

Take, as the owner decided (2026-10-22): find the user's shared FFmpeg in a fixed order of
folders and name every rejection; on a click, download one pinned gyan.dev shared build
checked against a hash in our source. Do not ship DLLs — the rule "no FFmpeg build of our
own" stands, and it also keeps us out of distributing a GPL build.

## Sources

- https://support.audacityteam.org/basics/installing-ffmpeg (access date 2026-10-24)
- https://www.audacityteam.org/changelog/ (access date 2026-10-24)
- https://forum.audacityteam.org/t/ffmpeg-issues-windows-11/135281/15 (access date 2026-10-24)
- https://www.gyan.dev/ffmpeg/builds/ (access date 2026-10-22)
