---
id: F-COL-001
title: Shared drive instead of cloud — network as a file system
status: researched
priority: B
cost: M
category: collab/assets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-media, kt-otio, kt-app]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [fcp-shared-library, premiere-productions, resolve-project-server, bm-cloud]
checked: 2026-09-08
---

# F-COL-001 — Shared drive instead of cloud — network as a file system

## Essence

Two people edit one volume: the material and the project lie on it, and there is no service between them — the network works only as a file system. The editor expects from this scheme exactly two things: that the program opens the file by a network path just as it does a local one, and that the second person does not overwrite his work. The first almost always "just works" — until the moment it stops working completely and without explanation; the second is done by almost nobody, and whoever does it does it with a lock file next to the project. This is the cheapest shared scheme in the industry: it costs no subscription and does not need someone else's host.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no separate mechanism | the project and media are simply paths; a network volume is visible as a folder, and there is no lock either on the project or on the event: two people who open it get two copies in memory, and whoever saves last writes. |
| Adobe Premiere | 26.x | Productions (project locking) | a shared FOLDER without a server: a `.prlock` is placed next to each `.prproj`; opening reads a foreign lock and shows "Project Locked". There is no service at all — all synchronization is the existence of a file on the shared volume |
| DaVinci Resolve | 21 | Disk database on shared storage / Project Server / Blackmagic Cloud | three different answers to one question: a disk database in a folder (for one person), a PostgreSQL server on its own host (for many), a cloud project library (by subscription). Shared work begins where the database LEAVES the file system and goes into a process |
| Final Cut Pro | 12.3 | Shared storage (NFS/SMB) | the library lies entirely on the volume; the requirement for SMB is Samba 4.3.4 or newer with Apple compatibility modules (`vfs objects = catia fruit streams_xattr`); one Mac holds the library, and the granularity of occupancy is the whole library |
| After Effects | 26.x | none | a shared AE project is Team Projects, that is, a service; a `.aep` on a share behaves like Vegas', and Adobe explicitly offers a service instead |

There is no second table here (software outside the core): the mechanism comes from video editing
proper, three of the five core lines are filled in substantively, and the "trick behind it" is taken
from the core — by the criteria of `_editors.md` a line outside the core would be water. The services
standing next to this scheme (`_services.md`) are deliberately not included in the table: the verdict
"take / do not take" lives there, and a service carries no verdict.

## The trick behind it

**Apple did not make a cloud at all, and instead presented a requirement on the PROTOCOL.** The line
`vfs objects = catia fruit streams_xattr` in someone else's `smb.conf` is not "an optimization for the Mac",
but an admission that Final Cut keeps part of its data NOT in the file but NEXT TO it: named streams and
Finder metadata, which on macOS live in extended attributes. `fruit` and `streams_xattr` put these streams into
the xattr of the Linux file system, `catia` re-encodes characters in names that are illegal for SMB. The practical
consequence, absent from every list of features: the volume may be mounted, readable, and deliver ten gigabits —
and Final Cut will still refuse, because what it checks is not speed but the ability of the protocol to carry
metadata OUTSIDE the file's content. The question "does our NAS support it" turns out to be a question about
xattr, not about performance, and this is the only reason why the cheapest shared scheme in the industry requires
reading the Samba documentation.

**Resolve answered the same question exactly the opposite way — it removed metadata from the file system altogether.**
A disk database in a folder on a share is officially not recommended, and the "correct" path is PostgreSQL on its own
host: there is no xattr there, no locks, and no dependence on the Samba version, because the file system stops taking
part in the coordination at all. Two programs, one question, two opposite answers: **either demand more from the
protocol, or stop asking it for anything.** The industry has found no third option, and this is the most useful fork
for us: our choice has already been made silently and coincides half-way with the second answer — we keep the
material's metadata in our own SQLite next to the project, not in the file attributes.

**The lock in this scheme is not a mechanism but a convention.** At Premiere a `.prlock` blocks nothing at the level
of the file system: it is an ordinary file whose EXISTENCE means "occupied". Hence its whole class of failures — the
lock survives a crash of the program, and clearing the occupancy looks like "delete a file you cannot see".

## Pros

- The scheme needs no service, no host and no subscription: the same folder, merely mounted. It is the only one in the whole category where the second person costs no money.
- The granularity of occupancy is visible to the eye (a file next to the project) and can be fixed by hand — unlike occupancy inside someone else's database, where there is no one to tell you how to "remove the lock".
- For a program that reads a file only through `open`/`read`/`seek`, a network volume is not a special case: the code that accesses the material does not change by a single line.
- Relative paths turn "the volume is mounted differently" from a catastrophe into a minor matter: a project that lies together with its material moves as a whole.

## Cons

- **The protocol requirement is checked where it is not noticeable.** The error comes as one line when choosing a location ("Unsupported volume type"), and it has no word about xattr — that is, the cause of the refusal is not reported at all, and it is fixed by reading a third-party manual on Samba.
- Everything the program has put into extended attributes is lost in an ordinary copy: `cp` without `-a`, zip, sending by mail, uploading to a "wrong" NAS. The data exists, but next to the file it is no longer there.
- A file-lock does not survive a crash: the `.prlock` stays on disk, and the second person sees "occupied" with no owner.
- Nobody locks the material — they lock the document. Two people who simultaneously overwrite the same source on a shared volume get neither a warning nor a trace.
- The latency of a network file system hits not the reading of frames but the service passes: any full scan of a file on opening becomes traffic.

## What users say

The verbatim response of Final Cut, quoted by a person who assembled his own NAS and ran into exactly the protocol requirement:

> Unsupported volume type - Choose a local, SAN, or supported SMB location.
> — geerlingguy, "Configure Samba smb.conf attributes for use with Final Cut
> Pro", issue #9 of the repository `geerlingguy/arm-nas`, 03.04.2024,
> https://github.com/geerlingguy/arm-nas/issues/9

The wording of the requirement itself is from Apple's documentation; it is useful because it names the VERSION of someone else's daemon as the condition for the editor to work:

> If you're using a shared storage system that uses the SMB protocol, the SMB
> volume must originate from a macOS server or a Linux server running Samba
> 4.3.4 or later configured with modules for Apple compatibility.
> — Apple, "Use shared storage with Final Cut Pro", updated 15.12.2023,
> https://support.apple.com/en-us/HT207128

This note has no counting part and never will: `trackers:` is empty on purpose. The five core editors have no public trackers
(`docs/features/_trackers.md`, "Whom we do NOT take"), and across the twelve exported trackers of open-source shared work there is practically
no such request — one genuine request among 25 442 issues. Filling in the field for the sake of the form would mean fabricating a signal (rule 12).

## How it fits Kinotrain

### What already exists

- `src/libs/kt-media/src/ffmpeg.rs` — the only place where a path becomes bytes: `FfmpegSource::open` passes the path string to kt-ffmpeg, and nothing more is required of the file system. Not a single extended attribute, not a single file lock: a grep across all of `src/` gives 0 matches for `xattr|flock|lock_exclusive|O_EXCL`. The answer to "does your NAS support it" is therefore "can it do open/read/seek".
- `src/libs/kt-otio/src/url.rs` — the path to the material is written RELATIVE to the project file (decision R.24 item 7), with a fallback to an absolute `file://`. This is exactly what makes "the volume is mounted in different places" harmless: as long as the project lies together with the material, on a second machine the same file resolves.
- `src/apps/kt-app/src/util.rs` — `resolve_media_path(base, path)`: the same resolution on the application side (a duplicate of the resolver, noted in the reconnaissance).
- `src/libs/kt-media/src/index.rs` — the material's metadata (probe, peaks, thumbnails, the batch index) lives in OUR SQLite, not in file attributes: half of Resolve's answer is made by us for another reason — for the sake of invariant 8.

### What is missing

- The cost of a network volume is nobody's concern: the first opening of each file does a third full pass over the whole packet stream (the batch index without decoding, `FfmpegSource::open`). Locally that is tens of milliseconds; on a share it is reading the whole file across the wire. A rescue exists and works — `open_indexed` takes the index from the sidecar DB — but only up to the first opening and only while the sidecar is alive (F-COL-002).
- A sign of "this volume is network" exists nowhere: neither in diagnostics nor in the performance panel. The program cannot warn, nor choose a different behaviour.
- No occupancy of the document exists at all: `write_project_guarded` (`src/apps/kt-app/src/app.rs`) compares the revision and knows exactly one writer. It is a guard of its own work, not a lock against someone else's.

### What gets in the way

Nothing constructive — and that is the main conclusion. The "shared volume" scheme already works with us, because we ask nothing of the protocol. What gets in the way is the opposite: as soon as someone wants to put even one byte of metadata next to a file (a clip colour, a lock, a "busy" mark), they will have to choose the very answer on which Apple and Blackmagic parted ways — and that choice must be made deliberately, not in a round where it surfaces by accident.

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: REMAINS — `src/libs/kt-media/src/ffmpeg.rs:28` — access to the material with us reduces to opening the file by path and reading with a seek, so a project lying on a mounted volume opens today without a single setting and without a single protocol requirement; we pay for this not in compatibility but in traffic: the first opening of each file is a full pass over the whole packet stream.

NETWORK COST: a volume mounted on both machines, and the same way of naming it — the media address — the path in the project is relative to the project file, while the key of the sidecar index row is absolute; as long as the machine is one, these two addresses are compatible, but on a shared volume `/Volumes/RAID` and `/mnt/raid` they are no longer compatible, and "the same file" stops being recognized even though the bytes and xxh3 match.

### Which invariants it touches

None — and this is a strong argument for "cheap". Invariant 8 (thumbnails and waveforms are taken from the sidecar index, not from decoding) is not touched by the scheme as long as the sidecar does not move to the share together with the project; precisely this case is analysed by F-COL-002. Invariant 5 (one MediaWorker owns the decoder) is indifferent to where the file is located.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages of the round, and both are about diagnostics, not about shared work |
| Where the main work is | kt-media (a sign of a network volume and the cost of opening), kt-app (telling the person about it) |
| Project format | does not touch it: relative paths already exist |
| Tests to write | opening a project whose material lies by an absolute path outside the project's directory; a cross-check of the kt-otio resolver against the kt-app resolver on one set of paths (today there are two) |
| Depends on | F-COL-002 (the service data next to the project), F-PRJ-003 (portability of the project — there it is about the DOCUMENT travelling, here about where the BYTES lie) |
| Breaks compatibility | no |

## Verdict

There is nothing to take — it already exists, and in the better of the two possible variants. We ask nothing of the protocol: no extended attributes, no named streams, no file locks; and therefore the question "does your NAS support it", which stands in Apple's way, is not asked by us at all. The local core of the scheme works today; the server half (occupancy of the document, someone else's lock, someone's name next to it) we do not introduce — there is no second participant, and the note has no right to declare his appearance.

What the note must leave behind is a PROHIBITION, symmetric to invariant 7: no Kinotrain data in extended attributes, in named streams, and in service files whose existence means something. The cost of such a decision is known in advance from others' experience: at Apple — dependence on the version of someone else's daemon; at Adobe — a lock that survives a crash. Our answer is closer to Blackmagic's (all metadata in our own database), and the next note shows what we pay for it.

Boundaries: the fate of the document itself on a second machine is F-PRJ-003; the service database next to the project is F-COL-002; "the material has not arrived yet" is F-COL-003; relinking sources is F-MED-005.

## Sources

- https://support.apple.com/en-us/HT207128 — Apple, "Use shared storage with Final Cut
  Pro", updated 15.12.2023 (accessed 2026-09-08). The same page gives the list of supported
  volumes: "Store Final Cut Pro libraries on local storage volumes, a SAN like an Xsan storage volume,
  on NFS or SMB file system volumes…".
- https://github.com/geerlingguy/arm-nas/issues/9 — "Configure Samba smb.conf attributes for use with
  Final Cut Pro", 03.04.2024 (accessed 2026-09-08).
- https://community.adobe.com/t5/premiere-pro-discussions/issue-with-productions-showing-projects-as-locked-when-they-are-not/td-p/14579493
  — the thread about `.prlock` files left after a crash, 25.04.2024 (accessed
  2026-09-08); analysed in more detail in F-COL-002.
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration — the Resolve collaboration
  page, from which the three variants of placing the database are taken (accessed 2026-09-08).
- The service registry and the unit of occupancy for each — `docs/features/_services.md`.
- Our code, checked by grep across `src/` on 08.09.2026: `kt-media/src/ffmpeg.rs`,
  `kt-media/src/index.rs`, `kt-otio/src/url.rs`, `kt-app/src/util.rs`;
  `xattr|flock|lock_exclusive|O_EXCL` — 0 matches.
