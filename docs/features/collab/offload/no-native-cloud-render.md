---
id: F-COL-041
title: No one has cloud rendering
status: rejected
priority: C
cost: L
category: collab/offload
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-engine, kt-media, kt-ffmpeg]
invariants: [5, 8, 9]
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [bm-cloud, frame-io, resolve-project-server]
checked: 2026-09-08
---

# F-COL-041 — No one has cloud rendering

## Essence

The editor wants to press "to the cloud" and get a finished file without heating their own machine for forty minutes. There is no such button in any of the five core editors, and this is not a gap in the feature list but a result. What is sold under the name "cloud rendering for editing" turns out, on closer inspection, to be a rented computer with a video card, onto which the user installs their own licence and sits down to work remotely. What moves is not the job but the PERSON — and the reason for this is not vendor greed, but that video rendering is limited not by computation but by bytes, which are not present on the other side.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 build 66 | none | neither remote nor cloud rendering; everything is computed by the machine on which the project is open |
| Adobe Premiere | 26.x | none | at Adobe everything except rendering is cloud: Team Projects carries the project, Frame.io — the review and the media. Encoding is still done by Media Encoder on the user's machine |
| DaVinci Resolve | 21 Studio | Remote Rendering — but this is its OWN network | the only standard "render not here" in the whole core, and it is local: a second machine on the same network, seeing the same project database and the same media. Blackmagic Cloud, despite the name, carries projects, not computation |
| Final Cut Pro | 12.3 | none | Apple has no cloud for editing at all (`docs/features/_services.md`): a shared volume — and that is all; distribution is in Compressor and lives in the local network |
| After Effects | 26.x | none | `aerender` on its own machines; Adobe's "cloud" means storage and review, but not computation |

The services row is deliberately not placed here: the verdict lives with the editor, while `_services.md` answers the question "what is on the other side". There too, in the section "Whom we do NOT take", it is already written why render farms as a service (iRender, AceCloudHosting) are not included in the registry: **this is a remote desktop with a GPU, onto which the user installs THEIR licence.**

## The trick behind it

**The reason is not in computation but in the pipe — and this is said by a practitioner verbatim, ten years ago and unchanged since.** To the question "what about making a farm for Resolve?" the answer is: "It'll take longer to load a frame and get it back from a render node than it will to process the frame", and next to it is named where the bottleneck really is: "Generally with Resolve renders, the bottleneck isn't image processing, it's bus pipe or drive speed limitations". It follows that the difference explains the whole domain: for a 3D farm the job is a small scene file that UNFOLDS into pixels already on the worker; for video editing the INPUT is the pixels themselves, and they must travel. A farm wins where data is small and computation is large; in editing it is the opposite.

**So they built not a farm but a viewer.** The joint work of Blackmagic and AWS is described as remote monitoring: "The DaVinci Resolve Remote Monitoring application combined with a high-performance remote desktop solution, such as NICE DCV, enables live 4K video streaming to system monitors". That is, the engineering effort was spent on making the PICTURE reach the person in an acceptable form, not on making the job reach the machine. This is a direct inversion of expectation: what travels to the cloud is not the work, but the eye.

**And the "cloud" product of the vendor is not a rendering product by design.** Blackmagic Cloud carries the library of projects; it does not store or compute the media — therefore "the cloud" at Blackmagic and "remote rendering" at Blackmagic are two different mechanisms that cannot be connected. The question "does Remote Rendering work with Blackmagic Cloud" exists on the vendor's forum as a separate thread — that is, even users find it not obvious that these are not one thing but two.

## Pros

- The model "the person moves" honestly solves the main problem: the material stays where it already lies, and does not travel anywhere at all. No synchronisation, no duplicates, no question of "whose copy is fresher".
- A rented machine gives exactly what is missing — a video card and a disk — and does not require from the editor a single line of code: all functions work, including those that a distributed render never has in principle.
- Local remote rendering (Resolve in its own network) removes the bottleneck of the pipe by the simple fact that there is no pipe: the material lies on a shared volume visible to both machines.
- The absence of cloud rendering spares the vendor a whole class of promises: what happens to an unfinished job when payment lapses does not have to be explained — this question, according to `_services.md`, is not disclosed ANYWHERE by anyone.

## Cons

- A remote desktop transfers not the job but the workplace: colour accuracy, input latency, and sound become a matter of separate engineering (which is why NICE DCV with 4:4:4 10-bit H.265 was needed).
- Rent is counted by time, not by work: the machine costs money both when it computes and when the person thinks.
- "Cloud" in the product's name does not mean computation, and this is not obvious to the user: at Blackmagic one name covers the projects, while rendering lives in a separate mechanism in its own network.
- Local remote rendering requires a shared file system and agreed paths — that is, it moves the problem from computation to `assets/`, where it lives (F-COL-001).

## What users say

The most valuable quote in the domain: it names the MECHANISM, not the symptom, and closes the question of render farms for editing entirely.

> It'll take longer to load a frame and get it back from a render node than it
> will to process the frame.
> — Juan Salvo, thread "resolve rendering farm?", Lift Gamma Gain, 28.09.2014,
> https://www.liftgammagain.com/forum/index.php?threads/resolve-rendering-farm.3594/

In the same thread — where the bottleneck really is:

> Generally with Resolve renders, the bottleneck isn't image processing, it's
> bus pipe or drive speed limitations.
> — Juan Salvo, same thread, 28.09.2014

And this is what was built instead of a farm: a remote monitor, not remote computation.

> The DaVinci Resolve Remote Monitoring application combined with a
> high-performance remote desktop solution, such as NICE DCV, enables live 4K
> video streaming to system monitors.
> — Zach Willner, Matt Herson, Mike Owen, "Blackmagic Design advances remote
> DaVinci Resolve workflows with AWS", AWS Media Blog, 19.07.2022,
> https://aws.amazon.com/blogs/media/blackmagic-design-advances-remote-davinci-resolve-workflows-with-aws/

`trackers:` is empty deliberately (rule 12). There are no public trackers for the five core editors (`docs/features/_trackers.md`), and in the twelve exported trackers of open-source NLEs "cloud rendering" is absent as a class — there is not one task of this kind, and filling the field would mean fabricating a signal.

## How it fits Kinotrain
CAPABILITY: openspec/specs/export/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Rejected: there is no cloud rendering — offline rendering owns its source on one machine (`DirectFetcher`), and there is no one to share it with.
OFFLINE: DEGENERATES — `src/libs/kt-engine/src/fetch.rs:28` — "cloud rendering" degenerates into what it is for everyone: one machine on which both the material and the code that computes it reside; and this is visible here — offline rendering owns the sources and pulls from them on demand, that is, it computes not the job but the whole material, and the question "where to compute" is already answered for us by the question "where the bytes lie".
NETWORK COST: all the material on the other side, and a common way to name "the same file" — media address — the job is kilobytes for us, the material is gigabytes, and our own export reads it three times: a scan of packets on first opening plus separate passes of video and audio over two independent sets of sources.

## Estimate

| What | Answer |
|---|---|
| Scope | L — and the scope is beside the point here: the work lies not in computation but in the transport of material |
| Where the main work is | there is none: the task belongs entirely to `assets/` (delivery of bytes), not to `offload/` |
| Project format | not touched |
| Tests to write | none — the feature is not opened. What would be useful is the reverse: a count of "how many bytes of material this export will read", which today nothing counts |
| Depends on | F-COL-001 (shared volume), F-MED-002 (proxies — the only way to reduce what is transported), F-COL-042 (what to send instead of a render) |
| Breaks compatibility | no — the feature is not opened |

## Verdict

**Rejected.**

The reason is not our poverty but the subject, and it was first formulated not by a vendor but by a practitioner: to upload a frame to a node and to get it back takes longer than to compute it. A farm wins where the input is small and the computation large; for video editing the input is the picture itself. That is exactly why there is no cloud rendering at any of the five, and what is sold under that name is a rented computer onto which the person moves. On our side this closes **invariant 9**: export is in-process, and a second executor outside the process would be a second product, not a feature.

**What is NOT the reason for rejection:** not the absence of a network (its absence is known without this note, and here it is not the main thing at all), not the absence of money for hosting, not the complexity of the protocol. The seam for a foreign source exists in us — `MediaSource` is already implemented by both the synthetic source and the stub. The rejection is about the fact that through this seam gigabytes would have to be carried for the sake of a kilobyte-sized instruction.

**The cheap equivalent — the same as in the industry, and we already have it half-way.** One must reduce not the time of computation but the volume carried: proxies (F-MED-002) are the only mechanism that changes the ratio, and it is also the only one for whose sake the question "compute it not here" makes any sense to ask. And what should be handed out is not a render but an analysis: its result is small, portable, and already has an entry point in us (F-COL-042).

**The condition for reopening — exactly one, and it is not about the cloud.** If the material ever stops travelling — because a light representative (a proxy) has appeared and a way to prove that it is "the same file", by content rather than by path — the question will return. But it will return as a question about proxies and about the media address, not as a question about rendering, and the note must be reopened starting from F-MED-002.

Boundaries: where the bytes lie — F-COL-001; the form of the job — F-COL-038; who executes it and why two executors cannot be considered identical — F-COL-039; assembly of results — F-COL-040; what is actually worth sending — F-COL-042.

## Sources

- https://www.liftgammagain.com/forum/index.php?threads/resolve-rendering-farm.3594/ — thread "resolve rendering farm?", both quotes by Juan Salvo, 28.09.2014 (access date 2026-09-08)
- https://aws.amazon.com/blogs/media/blackmagic-design-advances-remote-davinci-resolve-workflows-with-aws/ — Zach Willner, Matt Herson, Mike Owen, 19.07.2022: remote monitoring and NICE DCV instead of sending jobs (2026-09-08)
- https://forum.level1techs.com/t/davinci-resolve-remote-render-server-complete-guide-windows-linux/138807 — Remote Rendering as a LOCAL scheme: a shared project database and shared access to media are mandatory (eposvox, 12.02.2019; 2026-09-08)
- `docs/features/_services.md` — the section "Whom we do NOT take": render farms as a service are remote desktops with a GPU, onto which the user installs their own licence; also there — that Blackmagic Cloud carries projects, not media, and that the fate of projects on non-payment is disclosed nowhere.
- **[NOT VERIFIED]** against the primary source: the Blackmagic forum thread "Does remote rendering work with Blackmagic Cloud?" exists in the search index, but `forum.blackmagicdesign.com` is closed to this environment by Cloudflare (`docs/features/_services.md`, the availability table), so there is no verbatim quote with a date from it.
- Our code checked against the tree on 08.09.2026: `src/libs/kt-engine/src/fetch.rs:28`, `src/libs/kt-engine/src/export.rs`, `src/libs/kt-ffmpeg/src/source_impl.rs:132`, `src/apps/kt-cli/src/main.rs`, `src/apps/kt-app/src/render_dialog.rs`.
