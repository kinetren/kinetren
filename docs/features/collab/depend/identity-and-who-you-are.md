---
id: F-COL-045
title: An account as a launch requirement
status: researched
priority: C
cost: S
category: collab/depend
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-app, kt-version, kt-model]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [frame-io, team-projects, bm-cloud, fcp-shared-library, resolve-project-server]
checked: 2026-09-08
---

# F-COL-045 — An account as a launch requirement

## Essence

For shared features an account is not a convenience and not "a login to a personal cabinet", but a load-bearing structure. Without it one cannot name the author of a remark, cannot name the owner of a lock, cannot send an invitation, and cannot show who is in the project right now: all four quantities are expressed by one and the same identifier. Hence a consequence that the editor feels before he learns the price: "trying it together for half an hour" is impossible — the second participant must exist as a RECORD before making the first move.

The note is about where among others the boundary of identity runs, what exactly the one who did not set it up pays, and about the fact that we have no identity in any form: neither a person's name, nor a machine name, nor a difference between two copies of the program.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no identity at all, and no shared features either — this is the only row of the table where the absence of one is explained by the absence of the other, and not the reverse |
| Adobe Premiere | 26.x | Adobe ID | the account is the condition of entry to Team Projects and Frame.io, and it is also the unit of licensing: "everyone you invite will need to have a Creative Cloud Team or Enterprise license in order to collaborate". Productions has no service, and identity is taken not from the account but from the machine — the `.prlock` lock names the owner by means of the file system **[NOT VERIFIED against the primary source: `helpx.adobe.com` is unavailable]** |
| DaVinci Resolve | 21 | Blackmagic Cloud ID | creating it is free — "Creating your Blackmagic Cloud ID is easy and there's no charge!" — but creating it is MANDATORY: the cloud library addresses people by it. Project Server has two identities, and they are different: a PostgreSQL user for the connection and a name inside the project database for the lock |
| Final Cut Pro | 12.3 | none (OS account) | there is no service, but identity is needed all the same: the library is held by one Mac, and the message about the busy state names the person and the machine. The identifier is taken from the operating system; Apple's article about shared storage does not concern identity at all — there the requirement is set on the protocol, not on who you are |
| After Effects | 26.x | Adobe ID | the same entry as for Premiere, and the hardest dependency: a comment in the project column is not text but a link to a service, therefore without logging in one cannot read one's own document |

A second table (software outside the core) is absent here. The mechanism of an account comes from far beyond video editing, but what is interesting is not it in itself, but WHAT exactly is entrusted to it in the editing program — authorship, lock, invitation and presence at the same time — and this is visible only in the editing services. The services themselves do not get into the table: there lives the verdict "take/don't take", and the service bears the price (`docs/features/_services.md`).

## The trick behind it

**1. Identity at Frame.io has two kinds, and they differ not by powers but by what they hold in time.** A participant must create an account — "Inviting a user as a Collaborator requires them to create a Frame.io account"; a reviewer is not required to — "Sharing for Review allows external users to leave comments without creating Frame.io accounts". But an identity without an account exists exactly for a SESSION: "If you are not logged into your Frame.io account and you leave your review session, you will not be able to edit your previous session's comments" (updated 11.12.2025). That is, the account here is not a signature and not a line "who said this", but an ABILITY: without it one can say, but cannot take back what was said. The right to edit one's own words is the only thing that registration buys, and in the list of capabilities it is not present in a single line.

**2. The account is a unit of billing, not a unit of a person, and from this follows a non-obvious economic turn.** The plan's limit is expressed in participants (Free — 2, Pro — 5, Team — 15), and each next one costs monthly; reviewers without an account are not counted by anyone, ever. So the service is INTERESTED in some people not having an identity — and it is precisely for this reason that the accountless path is so well made: one can watch without logging in, one can comment without logging in, the name and address are asked exactly once and only at the first remark. The free status of the reviewer is not generosity but a way not to sell him a seat. The reverse side of the same: as soon as a person needs to upload a file or see the whole project, he instantly becomes paying, because for this a key is needed, not a session.

**3. Identity is asymmetric, and the order of its consumers is the same for all three.** One can look without it, write — already with a reservation, hold a lock — under no conditions. Apple has no service at all, yet identity is required all the same, and it is taken from the operating system — because the lock must NAME someone. From this follows a conclusion that reverses the usual order of thinking about shared work: the first consumer of identity is not a remark and not a notification, but the LOCK. A remark can still be signed with a hand-typed line; a lock cannot be signed with a line, because its owner has to be recognized in the next session, on another machine and after a crash. Therefore "let's try it together" is cut off not by the price, but precisely by this: registration is required before the first move, and it is required of both.

## Pros

- The signature of each reply comes for free and is mandatory by construction: it is used to sort, filter and count unread items.
- The lock names its owner, so "busy" turns from a fact into a message — it is visible whom to go to.
- The invitation is addressable: one can invite someone who has not yet opened the document, and this is the only way to let a second person in before he has arrived.
- The no-account path lets a client create nothing: he can look and say something without signing anything or paying.
- Rights are distinguishable: "view", "write", "own" are expressed by one quantity and therefore do not contradict each other.

## Cons

- "Together for half an hour" is impossible: registration of both is required before the first move. For a small team this blocking condition weighs more than the price — the price can be paid today, but a user account cannot be created for another person.
- A no-account identity lives for one session, and its holder loses the right to his own words simply by closing the tab.
- Identity is a unit of accounting, so it is economised: some people are deliberately left without an identifier, and their statements have no history.
- The user account is a point of failure independent of the work: the service is unavailable or sign-in fails — the work stops, although neither the files nor the program have been harmed.
- The name the second person sees is set by the service, not by the document: change your surname — and the history of all replies is rewritten; close the account — it is rewritten too.

## What users say

Verbatim on the point that identity at Adobe is inseparable from the licence — the reply of an Adobe employee to a person who simply did not get an "add" button:

> everyone you invite will need to have a Creative Cloud Team or Enterprise
> license in order to collaborate
> — PeterELee (Adobe Employee), thread «Can't add collaborators», 02.06.2019,
> https://community.adobe.com/t5/team-projects-discussions/can-t-add-collaborators/td-p/10468415

And the question itself, from a user who thought that turning a project into a shared one is an operation on the PROJECT, not on people:

> However I am not getting the option to add collaborators.
> — ziggyo39280110, 01.06.2019, same thread

The note has no counting part: `trackers:` is empty deliberately. A user account is what free editors do not have and do not plan to have, so in the twelve exported open-source trackers no tasks about it exist at all. Filling the field just for the form's sake would mean fabricating a signal (rule 12).

## How it fits Kinotrain

### What already exists

Nothing — and this has been verified, not assumed. Across all of `src/`, 08.09.2026:
`whoami|hostname|gethostname|env::var("USER")|machine_id|uid()` — **0
matches**. The program knows neither the person's name, nor the machine's name, nor that it is not the only one running at all.

- `src/apps/kt-app/src/app.rs:5361` — `.with_app_id("kinotrain")`; next to it
  `.with_title("Kinotrain")` (`:5363`) and `eframe::run_native("kinotrain", …)`
  (`:5367`). Three constants, identical for all copies: for the window manager,
  for the `.desktop` file and for eframe two running copies are one and the same
  program.
- `src/apps/kt-app/src/app.rs:5294` and `:5296` — the eframe storage is addressed
  by the same name: `lang` and `last_project` are written into one set per machine. A second
  copy will overwrite the first one's choice without noticing.
- `src/apps/kt-app/src/app.rs:5025` — `autosave_path()`: the recovery file of
  an unnamed project has exactly one fixed path,
  `$XDG_DATA_HOME/kinotrain/autosave-project.otio`. No pid, no user, no instance
  mark: two copies of the program write to one file, and the first silently loses
  its unsaved work.
- `src/apps/kt-app/src/app.rs:805` — the window title: the file name, the dirty
  asterisk and `kt_version::display()`. Everything by which one copy differs from another
  on screen is the BUILD VERSION, that is, the commit; there is no person in the title.
- `src/libs/kt-version/src/lib.rs:14` — and this is the only "identity" the program
  has at all: `ktYY.MM.DD-<hash8>` in debug, `ktYY.MM` in release.
  It answers "which build is this", not "who launched it".
- `src/libs/kt-model/src/project.rs:1467` — `Marker` has `t`, `duration`,
  `name` and `color`, and nothing more. There is no author, no creation time; a marker
  is addressed by its tick, not by an identifier.

### What is missing

The lack of a person's identity is exactly as large as the lack of a second
person — that is, it is missing consistently. But the lack of the INSTANCE's distinguishability is missing for real, and this is a complaint about single work, not joint work: two kt-app on one machine today share one recovery file, one storage and one app_id. A second instance learns nothing about the existence of the first in any way, and the price of this is silently overwritten unsaved work. No user account is needed for the fix: it is enough that an instance is a process, and a process has an identifier.

### What gets in the way

Nothing constructive. What gets in the way is the temptation to solve the wrong task: the reconnaissance shows that the first consumer of identity for everyone is the LOCK, not the signature — and therefore "let's add an author field to the marker" would be a start from the wrong end. The author cannot be taken from anywhere while there is no one who could be one, whereas the program instance exists already today and already conflicts with itself.

### In category `collab/` — two mandatory lines (rule 13)

OFFLINE: DOES NOT REMAIN — `src/apps/kt-app/src/app.rs:5361` — we have no identity in any form, and it is most visible in the three neighboring launch constants: app_id, title and application name are identical for all copies, therefore two programs running on one machine are indistinguishable to the window manager, to the settings storage, and to the recovery file, which has one fixed path without a pid and without a user. Today "who did this" a person keeps in his head and in the name of the project file, while "which copy is this" is learned from the build version in the title — that is, from the commit, not from oneself.

NETWORK COST: a permanent identifier, set up before the first move and surviving the session — a user account — without it no shared quantity is expressed: the lock has no one to name as its owner, the invitation has nowhere to be sent, and the signature under a remark degenerates into a hand-typed line which cannot be recognized the next time, nor distinguished from someone else's.

### Which invariants it touches

None — and this is a strong argument for "cheap" for the half that the note proposes to do. One can distinguish two instances of the program without touching any of the twelve invariants and any of the twenty rules of the checklist: this concerns the path of the recovery file and the storage key, that is, places lying entirely outside the engine, outside the format and outside the drawing.

## Estimate

| What | Answer |
|---|---|
| Volume | S — one stage, and all of it is about the distinguishability of the INSTANCE, not about the identity of a person |
| Where the main work is | kt-app (the recovery file and the storage), nowhere else |
| Project format | does not touch it: not a single schema field appears |
| Tests that will have to be written | two copies of the program on one machine do not overwrite each other's recovery file; recovery after a crash picks its own file, not someone else's |
| Depends on | F-COL-013 (two processes over one project file), F-COL-043 (the price of shared work) |
| Breaks compatibility | no |

## Verdict

We do not set up an identity — there is no second participant, and the note has no right to declare his appearance. An author field with a single possible value would be a lie in the schema, and logging into an account in order to sign one's own marker with one's own name is the exact repetition of what others are scolded for. The wave formula: we take the local core now, and we do not set up the server half.

Three conclusions remain from the reconnaissance.

**First, and this is the only WORK.** We need not a person's identity but the distinguishability of the INSTANCE, and it is needed by a solo user. Today two copies of kt-app on one machine share one recovery file `autosave-project.otio` (`src/apps/kt-app/src/app.rs:5025`) and one settings storage (`:5294`, `:5296`): the second copy silently overwrites the unsaved work of the first, and this can be learned only after the fact. No user account is needed for the fix — it is enough that the process has an identifier, and the file can have a name.

**Second, as the order of work for the future.** If an identity ever appears, its first consumer will be the LOCK, not the signature — this is so for all three, including Apple, which has no service at all, yet an identifier is needed all the same, because the lock has to name someone. Therefore any future request "sign the marker with the author" must first answer where the name comes from and whether it survives the session; otherwise we will get exactly the accountless identity of Frame.io, which lives until the tab is closed and does not even give the right to one's own words.

**Third, as a decision.** A user account has no right to become a condition of LAUNCHING anything in Kinotrain — neither the program, nor a feature, nor an effect. This is the same decision recorded in F-COL-043 about payment, and it is formulated separately because it is approached from the other side: a payment can simply not be made, while a login can simply fail to pass, and the second misfortune befalls a person who has paid.

Boundaries. **F-COL-022** (the author and time of a remark) — there it is about what happens to the two fields of a foreign comment on the border with the editing; here it is about the identifier itself and about the fact that it is carried by the LOCK before the signature. **F-COL-019** (who is in the project now) — about presence that is derived from the lock; here it is about what the participant himself is expressed by. **F-COL-013** (two processes over one project file) — the nearest neighbor and the direct addressee of our work: there it is about declaring a claim, here it is about the fact that there is nothing to declare it with today, because instances are indistinguishable. **F-COL-014** (a lock that outlived its owner) — about what to do when the identifier of the owner is out of date. **F-COL-043** (the price) — about the second half of the condition of entry: there one pays with money, here with registration, and the second cuts off more strongly. **F-UI-003** (layouts) — about the same storage field, but from the point of view of the convenience of one person.

## Sources

- https://help.frame.io/en/articles/1731442-the-difference-between-adding-a-user-to-your-project-and-sharing-for-review-legacy
  — "Inviting a user as a Collaborator requires them to create a Frame.io
  account" and "Sharing for Review allows external users to leave comments without
  creating Frame.io accounts" (access date 2026-09-08; the article does not show an update date).
- https://help.frame.io/en/articles/9105242-share-links-explained-for-clients —
  "Share links explained for clients", updated 11.12.2025: the name and address are asked at the
  first remark, and without logging in one cannot edit the remarks of a previous session
  (access date 2026-09-08).
- https://frame.io/pricing — the plan limits expressed in participants: Free — 2,
  Pro — up to 5, Team — up to 15 (access date 2026-09-08).
- https://community.adobe.com/t5/team-projects-discussions/can-t-add-collaborators/td-p/10468415
  — ziggyo39280110 (01.06.2019) and the reply of PeterELee, Adobe Employee (02.06.2019)
  about the license of each invited person (access date 2026-09-08).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  "Creating your Blackmagic Cloud ID is easy and there's no charge!" (access date
  2026-09-08).
- https://support.apple.com/en-us/HT207128 — "Use shared storage with Final Cut
  Pro", updated 15.12.2023. Checked specifically: the article does not say a word about
  identity — the requirement is set on the protocol (Samba 4.3.4 and compatibility modules),
  not on who you are. The message about a busy library naming the user and the
  machine is absent from this article and is not confirmed in the primary source.
- The registry of services and the column "what is needed on the other side" —
  `docs/features/_services.md`.
- Our code was checked by grep and reading over `src/` on 08.09.2026:
  `whoami|hostname|gethostname|env::var("USER")|machine_id|uid()` — 0 matches;
  `apps/kt-app/src/app.rs:805` (title), `:5025` (recovery file), `:5294`,
  `:5296` (storage), `:5361`, `:5363`, `:5367` (app_id, title, application name);
  `libs/kt-version/src/lib.rs:14`;
  `libs/kt-model/src/project.rs:1467` (`Marker`).
