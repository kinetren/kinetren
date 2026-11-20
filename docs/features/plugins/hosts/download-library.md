---
id: F-EXT-006
title: One library for everything the program downloads
status: candidate
priority: B
cost: L
category: plugins/hosts
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, kdenlive-26.08]
we_have: partial
crates: [kt-dirs, kt-ffmpeg, kt-text, kt-plugin-api, kt-app, kt-cli]
invariants: [9, 10, 11]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-MED-007, F-EXT-003]
trackers: []
services: []
checked: 2026-11-20
---

# F-EXT-006 — One library for everything the program downloads

## Essence

Some parts of the program are too big, too licence-bound or too optional to ship in the release: the
codec library on Windows, the speech-recognition engine and its model, font packs for scripts the
release does not carry, and later plugins. The person sees ONE place that lists them. For each it shows
what it is, its size, its licence, what in the program needs it and whether it is installed. One click
downloads it, and a second removes it. A feature that needs a missing part sends the person to its row
there.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | installers | effects packs and the publisher's plugins are separate installers run outside the program; there is no list inside VEGAS |
| Premiere | 26.x | Creative Cloud app, "Plugins" / Exchange; speech language packs | plugins are installed by the Creative Cloud desktop app, outside Premiere; the speech-to-text language packs are fetched from inside the Text panel when first needed |
| DaVinci Resolve | 21 | no in-app library; Reactor (community) | Resolve ships its own decoders and models in the installer; third-party Fusion tools come through Reactor, a community package manager that installs "atoms" into Fusion's folders |
| Final Cut Pro | 12.3 | no; FxFactory (third party) | Motion templates and FxPlug plugins are installed by their vendors' installers or by the FxFactory store app; FCP itself lists nothing it could fetch |
| After Effects | 26.x | Creative Cloud "Plugins" | the same Creative Cloud marketplace as Premiere, outside the program |

**Outside the core.** Blender and Kdenlive are the two programs where downloaded parts live INSIDE the
program, and their documents and bug trackers show what goes right and what goes wrong.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 | Extensions (Get Extensions) | repositories are listings of packages; the online repository stays off until the person allows online access once; each package's manifest declares the permissions it needs (files, network, clipboard, camera, microphone) |
| Kdenlive | 26.08 | Settings → Plugins (speech to text) | the Whisper runtime is installed through pip and the models are downloaded into a cache folder; installed models are marked and can be removed |

## The trick behind it

**1. A listing is data, so the same library works from a folder with no network.** Blender's
repository is a listing of packages, and a third party can host one as a single JSON file. The program
needs no server of its own. The same list read from a local folder serves a machine that is never
online. For us this means the catalogue of items and the network are two separate things: an item can
be installed from a file the person brings, checked against the same pinned hash.

**2. The failures come from two places disagreeing about where an item lives, or about which
versions go together.** Kdenlive's Flatpak build downloaded the model into one folder and looked for it
in another. A Whisper update made the downloaded models unusable, and the program said nothing. Both
are the absence of ONE function that answers "where is item X version V" and of a check at load that
names the mismatch. The download itself was not the problem.

**3. Opting in to the network once is not the same as consenting to every download.** Blender stays
offline until the person allows online access, and even then nothing is installed without a click. Our
update check and telemetry already ask consent. A download is a separate, explicit act each time.

## Pros

- A feature that needs a big part does not have to grow its own window, progress bar, mirror list and
  cleanup. FFmpeg's window (`src/apps/kt-app/src/ffmpeg_setup.rs`) already holds all of these, once.
- The release stays small: the speech model alone (140 MB to 1.5 GB) is larger than the whole program.
- The person can see and reclaim the disk the parts take, in one list.

## Cons

- A part that is not installed is a hole in a project moved to another machine: a title in a CJK font
  renders differently without its pack. The project has to name the parts it relies on (F-PRJ-003).
- A catalogue compiled into the program ages with it. A plugin library has to change faster than
  releases, and that needs a signed list from a server.
- Every downloaded item is foreign code or data on the machine. Pinning by hash proves only that it is
  the file we tested, not that it is safe.

## What users say

> Speech to text setup on kdenlive flatpak downloads whisper model to wrong location, thus speech to
> text does not work without manual copying of file.
> — Kdenlive bug 499012, 2025-01-22, https://bugs.kde.org/show_bug.cgi?id=499012

> Whisper got an update, and unbeknownst to me, this invalidated the models that Whisper uses to do the
> speech detection. There certainly was no message from Kdenlive that this was the case
> — Kdenlive issue 1831, https://invent.kde.org/multimedia/kdenlive/-/issues/1831

> When first downloading any whisper model, cache directory is not created, but no indication of why
> download not started is shown
> — KDE Discuss, 2025-01-21, https://discuss.kde.org/t/kdenlive-not-creating-cache-whisper-on-first-download/28586

> Staying true to the principle that Blender won't connect to the Internet by default, this repository
> must be manually enabled the first time in order to be used.
> — Blender developers' blog, 2024-05-15, https://code.blender.org/2024/05/extensions-platform-beta-release/

## How it fits Kinotrain

CAPABILITY: openspec/specs/media-index/ (the FFmpeg half today); the change `download-library` designs the rest.

### What already exists

- The FFmpeg download on Windows (F-MED-007):
  - `src/apps/kt-app/src/ffmpeg_setup.rs` pins 9.0.2 by sha256 and size, with two mirrors;
  - it downloads with a progress bar and Cancel, unpacks only what it needs, and installs by a rename;
  - it cleans up the leftovers of a killed run;
  - the folder is `kt_dirs::ffmpeg_download_root()` (`src/libs/kt-dirs/src/lib.rs`).
- The search plan (`src/libs/kt-ffmpeg/src/search.rs`) already reads every downloaded version.
- Planned downloads, each about to copy that window:
  - the Whisper library and its model (change `subtitles-and-text-editing`, design lines 51–53);
  - CJK and Arabic font packs (the same change, Q3).
- The plugin registry (`src/libs/kt-plugin-api/src/lib.rs`) has no door for foreign plugins yet. The
  wasm host is stage 2 of `openspec/specs/plugins/design.md`.

### What is missing

- One catalogue of items with their versions, hashes, sizes, licences and dependencies.
- One folder and one function that resolves an item.
- One window that lists the items.
- Installing from a file, for an offline machine.
- A load-time check that names a mismatch, the class of Kdenlive's issue 1831.

### What hinders

- Fonts take part in title bytes (invariant 11): a title must render the same with and without the
  library on the machine that made it. So a project names the packs it uses, and a missing pack draws a
  marked placeholder, not a silent fallback.
- Plugins need the wasm host first. A catalogue of plugins is empty until it exists.

### Which invariants it touches

9 and 10 (FFmpeg stays in-process, found by the search plan; the library is one more place in that plan),
11 (a font pack or plugin version is part of what a frame depends on).

## Estimate

| What | Answer |
|---|---|
| Scope | L — a round: the library core and FFmpeg moved into it, then speech and fonts, then plugins with the wasm host |
| Where the main work is | kt-app (the window, the jobs), kt-dirs (the folder), kt-ffmpeg (the search plan), kt-text (font packs) |
| Project format | names the packs a title uses; no new version if absent means "none" |
| Tests that will have to be written | catalogue pin check, atomic install and removal, install from a file, a load-time mismatch named, the window's user path |
| Depends on | F-MED-007, F-EXT-003 |
| Breaks compatibility | no |

## Verdict

Take: the owner asked for it in so many words (kkt-26.11.20 A11: "все что у нас качается - надо
переносить и делать архитектурно в виде некоторой библиотеки, типо как плагины"). Two downloads are
already designed to copy the FFmpeg window, so this is the moment to make it one place. The plugin
library comes with the wasm host.

## Sources

- Kdenlive bug 499012 (access date 2026-11-20), https://bugs.kde.org/show_bug.cgi?id=499012
- Kdenlive issue 1831 (access date 2026-11-20), https://invent.kde.org/multimedia/kdenlive/-/issues/1831
- KDE Discuss thread 28586 (access date 2026-11-20), https://discuss.kde.org/t/kdenlive-not-creating-cache-whisper-on-first-download/28586
- Kdenlive manual, "Speech to Text" (access date 2026-11-20), https://docs.kdenlive.org/getting_started/configure_kdenlive/configuration_speech2text.html
- Blender, "Extensions Platform Beta Release" (access date 2026-11-20), https://code.blender.org/2024/05/extensions-platform-beta-release/
- Reactor for Blackmagic Fusion, ProVideo Coalition (access date 2026-11-20), https://www.provideocoalition.com/reactor-for-blackmagic-fusion/
