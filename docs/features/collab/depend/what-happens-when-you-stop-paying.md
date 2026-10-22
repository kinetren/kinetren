---
id: F-COL-043
title: The cost of collaboration and silence about non-payment
status: researched
priority: C
cost: S
category: collab/depend
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-app, kt-version, kt-cache, kt-media]
invariants: [9]
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [frame-io, bm-cloud, resolve-project-server, team-projects, premiere-productions]
checked: 2026-09-08
---

# F-COL-043 — The cost of collaboration and silence about non-payment

## Essence

Working together with all five editors costs money, and it costs it every month. The editor sees the tariff down to the cent: this much per person, this much per terabyte, this much for a project library. What they do not see anywhere is what happens to work already done when the payment stops: the subscription is closed, the card is declined, the studio falls apart, the project was delivered a year ago. This note is about that second half of the price. The first half is published and verifiable; the second half is not described by a single vendor, and its absence is itself the finding.

The boundary with `resilience/`: there the program is installed and something inside IT or in someone else's hardware failed; here the program is sound, the machine is intact, the files are in place — and it still cannot be worked with, because it has not been paid for. The refusal comes not from the machine but from the other party, and this is the only kind of failure that cannot be fixed while sitting at your own computer.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no collaboration at all, so there is no second bill either: you pay once for the license (or monthly for that same license), and "working together" has no price, because it does not exist |
| Adobe Premiere | 26.x | Productions · Team Projects · Frame.io | three different mechanisms and three different bills for one task. Productions is a shared folder, not billed separately at all (only a volume is needed). Team Projects is a service: "everyone you invite will need to have a Creative Cloud Team or Enterprise license". Frame.io is a fourth bill, charged SEPARATELY from Creative Cloud: inside CC a reduced set is included, and for more you pay Frame.io directly |
| DaVinci Resolve | 21 | Disk database · Project Server · Blackmagic Cloud | three answers with three prices, and this is the only row of the table where there is a genuine zero. The disk database is free (for one person). Project Server is **free** and for many people, but on YOUR OWN host. Blackmagic Cloud is $5/month for the project library plus $15 per terabyte of media per month (the price was halved in November 2024 from $30), and it does not count people at all |
| Final Cut Pro | 12.3 | Shared storage | there is no cloud; the cost of working together equals the cost of the HARDWARE — a volume visible to both machines. There is no monthly bill in any form, and the requirement is placed not on the wallet but on the protocol: "A Linux Network Attached Storage (NAS) SMB volume must originate from a Linux server running Samba 4.3.4 or later configured with modules for Apple compatibility" |
| After Effects | 26.x | Team Projects · Frame.io | the same two bills as Premiere, and the same peculiarity: a comment in the project column is not text but a link to the service, which means that even the right to READ one's own column is paid for monthly |

A second table (software outside the core) is deliberately absent here. The subscription mechanism does not originate in video editing, but there is nothing to explain it with another program: what is interesting is not the subscription itself, but what exactly it measures editing work by, and that is visible only in editing services. The services themselves do not enter the table by the doctrine of the three registries (`docs/features/_services.md`): the "Who has it and how" section carries the verdict "take it / do not take it", and a service does not carry a verdict — it carries a price.

## The trick behind it

**1. "How much room do I have" is not one number but two mutually exclusive pools, and the useful one is six to eight times smaller than the advertised one.** At Frame.io a plan is called by the volume of Standard Storage (Pro — 2 TB, Team — 3 TB), and next to it, in a separate column, stands Mounted Storage: 250 GB for Pro and 500 GB for Team. The difference is not in speed or in reliability: Standard is "lies there and opens in the web", Mounted is "visible from the desktop and streamed into the editing program". The help article states it directly: "Counts against your Mounted Storage quota (separate from Standard)", and right there — "Assets count against only one storage type at a time". Hence a consequence that is absent from every tariff comparison: **the number in the plan's name describes the volume of STORAGE, while the volume you can WORK with is a second number, six to eight times smaller.** A person buys two terabytes and discovers that they can edit from only two hundred and fifty gigabytes.

**2. The only place in the whole industry where the documentation says what happens when the limit is exceeded describes OVERUSE, not non-payment — and the difference between these two cases is the finding.** About overuse it is said in detail and in an engineering way: "If enabled projects exceed your Mounted Storage entitlement, your account enters a grace period. Enabling new projects for mounting is blocked until the overage is resolved — either by disabling projects or purchasing additional capacity. All media remains accessible via the web app". There is a grace period, the blocked action is named, the non-blocked action is named. And about non-payment there is one phrase in the whole help article, and it shifts the work onto the user: "Please plan accordingly as you may need to delete files to stay within the reduced storage limits". What the service will do if the user does NOT delete is written nowhere — neither at Adobe nor at Blackmagic. This is not a gap in the documentation: there is nothing to promise here, because a promise would tie the vendor's hands, and promising nothing is cheaper.

**3. The two axes of price are incommensurable, so the question "how much does it cost to work together" has no answer.** Frame.io counts PEOPLE: the plan's limit is expressed in members (Free — 2, Pro — 5, Team — 15), and each next one costs monthly. Blackmagic counts BYTES and LIBRARIES and does not count people at all. One and the same team receives bills that differ several-fold depending on what it has a lot of — people or material: five editors with a hundred gigabytes are expensive at Adobe and almost free at Blackmagic, two with twenty terabytes are exactly the reverse. Against this background the only genuine zero in the industry stands apart: "DaVinci Resolve Project Server is free so you can share the same project or timeline with people simultaneously around the world". The subscription price is replaced there by the price of the host — PostgreSQL on your own machine, on your own network — and this is exactly the exchange that we have already made silently and entirely in favour of the host.

## Pros

- A subscription turns a second participant into a question of configuration, not of installation: you invite them by address — it works, nothing needs to be installed, raised or opened to the outside. This is exactly what people pay for.
- Payment per seat (Frame.io) makes the bill predictable with a stable team: it does not depend on how much material arrived this week.
- Payment per byte and per library (Blackmagic) does not punish for the size of the team — the third and tenth person cost nothing, and this is named directly by users as the reason for the choice.
- The free self-hosted Project Server is the only point in the industry where working together has no monthly price at all; a consequence of the fact that coordination is moved into ordinary PostgreSQL and not into a service.
- The grace period on overuse is built correctly: what is blocked is EXPANSION, not access, and what is already loaded keeps opening.

## Cons

- The useful capacity does not coincide with the purchased one and does not stand in the plan's name: you can edit from 250 GB of the two purchased terabytes.
- Non-payment is not described. You cannot plan an exit from the service in advance, because it is unknown how much time there will be to download and whether there will be any at all.
- At Adobe one task has three bills, and "but I already have a Creative Cloud subscription" means almost nothing: the set included with Frame.io is 2 GB and 2 projects.
- At Blackmagic material gets into the cloud only through Resolve or the camera: "For right now the only way to get media onto the BMD cloud is to load it into Resolve or direct from the camera/camera app". That is, the price includes the only permitted route for the bytes.
- The only free option requires its own host and its own network — the price has not disappeared, it has changed currency from money to administration.
- A monthly bill makes working together a state rather than a property: it is switched on and off from outside, and no local sign of the project says anything about it.

## What users say

Verbatim, about the incommensurability of the two axes — from a colorist who compared both tariffs right after Blackmagic Cloud appeared:

> It seems the pricing is about what it is at frame.io BUT with BMD you don't pay
> for extra users.
> — Evan Anthony, thread "Do you understand the new Blackmagic Cloud (sept 2023)",
> 15.09.2023, http://www.liftgammagain.com/forum/index.php?threads/do-you-understand-the-new-blackmagic-cloud-sept-2023.18231/

Verbatim, about the fact that at Adobe the price of people is not a place in the service but a license for each person, and without it there is no one to invite (a reply from an Adobe employee in the thread "Can't add collaborators"):

> everyone you invite will need to have a Creative Cloud Team or Enterprise
> license in order to collaborate
> — PeterELee (Adobe Employee), 02.06.2019,
> https://community.adobe.com/t5/team-projects-discussions/can-t-add-collaborators/td-p/10468415

The only phrase in the industry about non-payment is not a user's but from the help article, and it is quoted because a user's version could not be found by any query: the service is silent, and there is nothing to discuss.

> Please plan accordingly as you may need to delete files to stay within the
> reduced storage limits.
> — Frame.io, "How do I cancel my subscription?", updated 09.04.2024,
> https://help.frame.io/en/articles/9105522-how-do-i-cancel-my-subscription

The note has no tracker part: `trackers:` is empty deliberately. Public trackers do not exist for the five core editors, and the price of a subscription to someone else's service is not reflected in open-source trackers at all — there is not a single task there about what does not exist in the product and cannot exist. Filling in the field for the sake of the form would mean fabricating a signal (rule 12).

## How it fits Kinotrain

### What already exists

- `src/apps/kt-app/src/main.rs:35` — the whole startup path: `fn main() -> eframe::Result { app::run() }`.
  Between the person and the program there is nothing: no license check, no login,
  no expiry, no launch counter. Checked by grep across all of `src/` on 08.09.2026:
  `license|licence|trial|activation|entitlement|subscription|expire|watermark`
  outside the strings `license.workspace` and comments — **0 matches**.
- `src/libs/kt-version/src/lib.rs:14` — the only thing by which the build declares itself:
  `display()` returns `ktYY.MM.DD-<hash8>` in debug and `ktYY.MM` in release. This is
  a mark of the COMMIT, not a right to run: it has no expiry, no signature, no party
  that could revoke it.
- Invariant 9 (export is fully in-process) and the absence of `std::process::Command` in product code mean that the finished file never passes through someone else's process. To count our bytes and issue a bill for them is physically impossible.
- Auxiliary data is kept next to the project and dies with it: `src/libs/kt-media/src/index.rs` (the sidecar `<project>.ktindex.db`) and `src/libs/kt-cache/src/lib.rs` (the burst cache, a limit in bytes) know exactly one project. We have no shared pool that would outlive a project and that could be measured as a whole, in any form.

### What is missing

Nothing that should be ADDED. One thing is missing — a recorded decision. The property "the program cannot be switched off from outside" has no test and no line in a checklist, so it holds only on the fact that no one has thought to break it. The first function that begins with the words "and let it pull presets from the server" will take it away silently and for free.

### What gets in the way

Nothing — and this is exactly the case where reconnaissance is needed not to understand how to do something, but to understand what not to do. An analysis of other people's tariffs gives us not a list of tasks but a list of traps: useful capacity that does not match the one sold; a refusal that comes not through the user's fault; and an undescribed exit.

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: REMAINS — `src/apps/kt-app/src/main.rs:35` — the whole launch of the program is these two lines, and between them there is no rights check, no expiry, no counter: "stopping paying" has no subject for us, because there is no party who is paid and no place where a refusal could occur. An installed copy works tomorrow exactly as it does today, and this property is bought by renouncing the entire service half: a second participant appears neither for money nor for free.

NETWORK COST: someone else's pool of bytes that somebody measures and bills — cross-project storage — a bill is possible only where storage outlives an individual project and is measured as a whole; ours has the sidecar index lying next to the project, and the burst cache is bounded by its own byte limit, and a quantity of "how much this person occupies in total" does not exist in any structure.

### Which invariants it touches

Invariant 9 is the only one, and it touches not the note itself but everything that could grow out of it: export is in-process, therefore neither "rendering by subscription", nor "a watermark until payment", nor any other point where someone else's side decides the fate of the finished file, has a place to stand in our system. An invariant forbidding a rights check at LAUNCH is not in the list — today its role is performed by the empty grep result, and the note proposes treating this as a decision rather than a coincidence.

## Estimate

| What | Answer |
|---|---|
| Size | S — there is no code work at all; one stage would go to wording the prohibition and its guard |
| Where the main work is | nowhere: reconnaissance closes the question entirely |
| Project format | does not touch it |
| Tests to write | zero, if the prohibition stays on paper; one test for the absence of such lines at the level of `reach_tests`, if it is decided to guard it mechanically |
| Depends on | F-COL-044 (what survives the service), F-COL-045 (user account as a condition of launch) |
| Breaks compatibility | no |

## Verdict

We do not open subscriptions and will not open them — but this is not the note's conclusion, it is its starting point: it was known before the reconnaissance. The conclusion is different, and it is worth stating precisely.

**First.** "Free" and "without a subscription" are different things, and the industry demonstrates this with its own example. The only zero among others' offerings (Resolve Project Server) is not free: it costs a host, a network and PostgreSQL, only the bill arrives in another currency. Our choice is the same exchange, carried to its end: the price of working together is zero for us in money and infinite in capability, because there is no second participant. The formula for the wave: we take the local core now, and we do not open the server half — there is no participant, and the note has no right to announce his arrival.

**Second, and this is the only thing the note is obliged to leave behind — the PROHIBITION.** No function of Kinotrain has the right to depend on a continuing payment to someone else's side: neither launch, nor opening a project, nor export, nor a separate effect. The basis is not an ideology but a concrete result of someone else's: the service has described in detail what happens on overuse and has described nothing about non-payment, and therefore a user who has bought such a dependency cannot plan even their own exit. Today the prohibition is observed by itself (0 matches for `license|trial|activation|entitlement`), and the cost of breaking it will not be noticed at once — which is why it is recorded here, rather than being re-derived in a year.

**Third, applied.** From someone else's tariff we take one idea, applicable even alone: a capacity can have TWO numbers, and the useful one is not the one on the shop window. We already have this fork here and have resolved it the other way: the limit of the burst cache (`src/libs/kt-cache/src/lib.rs`) is exactly "how much can be edited right now", and it is not connected in any way to the size of the disk. If this limit is ever shown to a person, what must be shown is exactly it, and not the free space on the disk: otherwise we would repeat the very substitution of a number for which Frame.io is criticised.

Boundaries. **F-COL-044** — what survives the service: there it is about the lifetime of a DOCUMENT, not about a bill, and about what our text format pays with. **F-COL-045** — user account: there it is about the second participant having to exist as a record before anything is done, and this cuts off small teams more strongly than the price does. **F-COL-001** — shared disk: the only scheme among others' that, like ours, does not cost a subscription. **F-COL-002** — auxiliary data next to the project: there it is about where the bytes physically live, which here would be counted as money. **F-PRJ-003** — project portability: there the document travels to another machine; here it travels nowhere, but simply stops opening by someone else's decision.
**`resilience/environment/packaging-and-install.md`** (F-RES-008) — a neighbouring, but different, misfortune: there the program did not start because of packaging and third-party libraries; here it would start, but it is not permitted to run.

## Sources

- https://frame.io/pricing — the Free / Pro / Team / Enterprise tariffs, the Storage and Mounted Storage columns, the number of members and projects (accessed 2026-09-08). From the same page, the wording about the warning when a limit is exceeded: "If you exceed your current plan limits, the application will warn you about which limit you've exceeded".
- https://help.frame.io/en/articles/14501614-getting-started-with-frame-io-drive-mounted-storage
  — definitions of Standard and Mounted Storage, "Assets count against only one storage type at a time", the behaviour on overuse (accessed 2026-09-08).
- https://help.frame.io/en/articles/9105522-how-do-i-cancel-my-subscription —
  "How do I cancel my subscription?", updated 09.04.2024: the only phrase in the industry about the fate of files after payment stops (accessed 2026-09-08).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  "DaVinci Resolve Project Server is free…" and "Creating your Blackmagic Cloud ID is easy and there's no charge!" (accessed 2026-09-08). The prices of Blackmagic Cloud themselves ($5/month for the project library, $15/TB/month for media, halved in November 2024) are not published on blackmagicdesign.com — checked; the figures are taken from secondary sources and marked as such.
- http://www.liftgammagain.com/forum/index.php?threads/do-you-understand-the-new-blackmagic-cloud-sept-2023.18231/
  — the thread about the appearance of Blackmagic Cloud, the quote from Evan Anthony, 15.09.2023 (accessed 2026-09-08).
- https://community.adobe.com/t5/team-projects-discussions/can-t-add-collaborators/td-p/10468415
  — a reply from an Adobe employee about a license for each invited person, 02.06.2019 (accessed 2026-09-08).
- https://support.apple.com/en-us/HT207128 — "Use shared storage with Final Cut Pro", updated 15.12.2023: the requirement for Samba, named instead of a price (accessed 2026-09-08).
- The service registry, the unit of occupancy, and the rule "what happens on non-payment is not stated anywhere" — `docs/features/_services.md`.
- Our code, checked by grep across `src/` on 08.09.2026: `apps/kt-app/src/main.rs`,
  `libs/kt-version/src/lib.rs`, `libs/kt-cache/src/lib.rs`,
  `libs/kt-media/src/index.rs`; `license|licence|trial|activation|entitlement|
  subscription|expire|watermark` in product code — 0 matches.
