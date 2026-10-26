# Services: what stands on the other side of the network

Set up in the fourth wave (R.42). Updated FIRST — the same order as `_editors.md` in rule 3 and `_trackers.md` in rule 12.

**A service is not an editor.** A service line does NOT grant the right to a line in the table "Who has it and how": only a core editor gets there, and only through `_editors.md`. There are no verdicts here and there cannot be — the verdict lives in the note, which has exactly one feature. The key is written in the `services:` field of the header; the `editors:` field will not accept it and must not.

**Why a separate file and not a third table in `_editors.md`.** The reason is mechanical, not aesthetic: `read_editor_keys()` in `src/scripts/features-index.py` takes by regexp ANY line of the file whose first cell is a slug in backticks, without binding to a section. A third table would silently make `frame-io` a legal value of the `editors:` field — that is, it would let a service into the table where the verdict "take / do not take" lives. The separation of registries here is the only thing that stops the check from agreeing to nonsense.

From this the doctrine of the three registries of the base follows:

| File | Answers the question | Header field |
|---|---|---|
| `_editors.md` | **what we compare with** (verdict) | `editors:` |
| `_trackers.md` | **where the number comes from** (count) | `trackers:` |
| `_services.md` | **what stands on the other side of the network** (cost) | `services:` |

## Registry

The "Type" column is a closed dictionary, and it is more important than the others: it says in advance what to expect from the `NO NETWORK:` line in a note. `shared folder` almost always gives `REMAINS`, `service` — almost always `DOES NOT REMAIN`, `own server` — `DEGENERATES`.

| Key | What it is | Type | Whose and what it is tied to | Unit of claiming | What is needed on that side | Checked |
|---|---|---|---|---|---|---|
| `frame-io` | Frame.io | service | Adobe; a panel inside Premiere 26 and AE 26 | does not claim: a review over a version of the cut | an account + someone else's server + uploaded material | 2026-09-08 |
| `team-projects` | Adobe Team Projects | service | Adobe; Premiere/AE, Teams subscription | **sequence** | an account + a project server | 2026-09-08 |
| `premiere-productions` | Premiere Productions | shared folder | Adobe; there is NO server | **project file** (the lock beside the file) | only a shared file system | 2026-09-08 |
| `bm-cloud` | Blackmagic Cloud | service | Blackmagic; Resolve 21 Studio | **bin and timeline** | an account + the project library as a service | 2026-09-08 |
| `resolve-project-server` | DaVinci Resolve Project Server | own server | Blackmagic; installed by oneself, PostgreSQL | **bin and timeline** | OWN host in one's own network; no account needed | 2026-09-08 |
| `fcp-shared-library` | Final Cut: shared library | shared folder | Apple; FCP 12 | **the whole library entirely** | a volume visible to both machines; SMB — Samba ≥ 4.3.4 | 2026-09-08 |
| `posthog` | PostHog Cloud (EU region, `eu.i.posthog.com`) | service | PostHog Inc.; not tied to any editor — receives product-analytics events over a public POST endpoint (`/i/v0/e/`, `/batch/`) | does not claim anything: it only receives events, nothing on our side waits for it | the owner's project token built into the binary + their host; the person running the program needs no account | 2026-10-22 |
| `gyan-dev` | gyan.dev FFmpeg builds (`www.gyan.dev/ffmpeg/builds/packages/`) | service | Gyan Doshi; not tied to any editor — static and shared FFmpeg builds for Windows, GPLv3, mirrored on GitHub (`GyanD/codexffmpeg`) | does not claim anything: a versioned archive is downloaded once on the person's click | nothing on our side but the pinned archive name and its SHA-256 in our source; the person needs no account | 2026-10-22 |
| `github-releases` | GitHub Releases of `kinetren/kinetren` (REST `GET /repos/{owner}/{repo}/releases/latest`) | service | GitHub (Microsoft); not tied to any editor — our own release channel since change `distribution` ("Release on a version tag": a `v*` tag builds the Linux tarball and the Windows zip and attaches them with their `.sha256`) | does not claim anything: one anonymous read of the latest normal release; "latest" is the most recent non-prerelease, non-draft release sorted by the date of its COMMIT | nothing but the public repository; unauthenticated reads are limited to 60 per hour per IP address, and the person needs no account | 2026-10-27 |
| `syncthing` | Syncthing | tool | nobody's, free; the ORIGIN of two-way synchronization | does not claim: resolves after the fact | a second machine, but NOT a server | 2026-09-08 |

Type: `service` (someone else's host) · `own server` (you install it yourself) · `shared folder` (the network only as a file system) · `tool` (not a service, but the origin of a mechanism).

**The unit of claiming is the most useful column of the wave.** Nobody in the industry merges edits; everyone prevents divergence by locking, and they differ exactly in granularity. It is always equal to the unit of a RECORD, not the unit of an edit: Final Cut writes the library as a whole — occupies it as a whole; Resolve writes bins and timelines as separate database records — occupies them apart. **Nobody locks a clip.**

## Who we do NOT take — and why

| Who | Reason |
|---|---|
| Avid Media Composer and its bin locking | decision of R.36: Avid is not part of the base. The primary source is moreover physically unavailable — `avid.com`, `help.avid.com`, `kb.avid.com` return 403. The mechanism is analyzed in the prose of the note with the mark `[NOT VERIFIED]`, as is already done in three notes of the base; a registry key without real verification is exactly the water against which the base was started |
| LucidLink, Suite Studio, Dropbox, Google Drive, Nextcloud | not tied to an editor; for us this is a `shared folder` in general, and it is already described by the type. Separate lines would add not a single mechanism |
| iCloud Drive | Apple has not made cloud joint work for Final Cut: a library in iCloud Drive is the same `shared folder` with the same exclusive lock. It does not deserve a separate line |
| Render farms as a service (iRender, AceCloudHosting) | this is a remote desktop with a GPU, onto which the user installs THEIR OWN license. What is transferred is a person, not a job — it has nothing to do with the services of an editor |

## How to verify and what ages fastest

1. **The price and the composition of the subscription — the most perishable**: a date and a link are mandatory.
2. **A change of the service owner matters more than a change of the editor version.** Frame.io is a precedent: it was bought by Adobe, and the move to V4 broke the return of comments in Premiere.
3. **"Works offline" in the vendor's marketing almost always means "caches", not "works".** Only a direct statement of the documentation that something can be done without a connection is valid. For Blackmagic Cloud there is NO such statement in the documentation at all; for Team Projects the only substantive formulation was found on a forum, not in the help.
4. **What happens to projects on non-payment is not said ANYWHERE by anyone** — neither by Adobe nor by Blackmagic. The absence of an answer is itself a finding, and it must not be replaced by a guess.

## Availability of sources as of 08.09.2026

Checked by queries, not by assumptions. Changed significantly since the third wave.

| Source | Access | Remark |
|---|---|---|
| `community.adobe.com` | **available** | **new**: fully replaces `helpx.adobe.com`, gives live bug threads with nicknames and dates |
| `frame.io`, `help.frame.io`, `support.frame.io` | available | pricing and both knowledge bases (V4 and legacy V3) |
| `blackmagicdesign.com` | available | product pages, support, press releases |
| `support.apple.com` | available | **trap**: an incorrect slug `/guide/final-cut-pro/…` silently redirects to the table of contents and returns 200 — take slugs from search, do not guess |
| `docs.syncthing.net` | available | the only formally described model of the conflict |
| `liftgammagain.com`, `creativecow.net` | available | the second only via WebFetch: curl gets 403 |
| `helpx.adobe.com`, `adobe.com` | **no** | the connection is not established; Wayback does not help (429, the needed snapshot is absent) |
| `forum.blackmagicdesign.com` | **no** | Cloudflare; threads are read only through search snippets — verbatim quotes with dates cannot be obtained from there |
| `avid.com` and all subdomains | **no** | 403 everywhere; only secondary sources (elements.tv, Hedge, redsharknews) |
| `reddit.com` | **no** | a hard block-list of the fetcher |
