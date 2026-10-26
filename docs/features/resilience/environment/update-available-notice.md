---
id: F-RES-013
title: "A new version is available" — a notice, not an updater
status: candidate
priority: B
cost: S
category: resilience/environment
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, shotcut-26.09, vscode-1.136]
we_have: no
crates: [kt-app, kt-version]
invariants: [12]
ui_rules: [10, 13, 19, 22, 27]
round: update-check
point: null
tests: []
depends_on: []
trackers: [pitivi, kde-bugzilla, shotcut, losslesscut, kde-invent]
services: [github-releases]
checked: 2026-10-27
---

# F-RES-013 — "A new version is available" — a notice, not an updater

## Essence

The person runs a build that is a month old and does not know that the defect they are fighting
was fixed two releases ago. The program itself says once, quietly, that a newer normal release
exists, and keeps saying it in a place the person can glance at; it does not download, replace or
restart anything. The question of the note is not "whether to check" but **where the answer lives,
how often it is asked, who decides what "newer" means, and what it costs the person who does not want it**.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Options → Preferences → General → "Use Newsfeed to stay informed about Vegas product updates" | the editor itself checks; with the box on "you will receive notification when updates are available", and clearing it bypasses "the Newsfeed dialog" — that is, the notice is a DIALOG. The update itself arrives through a separate application, Boris FX Hub (F-HLP-021). The same box sits next to "Send analysis data to Boris FX", the telemetry switch — the two network paths share one page. Forum threads about the same dialog carrying edition-upgrade offers that came back after "do not show again" are no longer reachable: the old forum redirects to `forum.borisfx.com` **[NOT VERIFIED]** |
| Adobe Premiere | 26.x | Creative Cloud desktop app → Apps → Enable auto-updates | the editor does NOT check: the subscription manager does, with a master toggle and a per-app toggle. The cycle starts when the Creative Cloud app refreshes or restarts, and an app that is OPEN at that moment is skipped; "Remove older versions" decides whether the previous build stays. Adobe itself advises turning auto-update off for an app when a team needs everyone on one version (helpx returned 403; read from the search excerpt) |
| DaVinci Resolve | 21 | DaVinci Resolve → Check for Updates…; a window at launch | the editor checks itself: at start it shows a window when a newer build exists, plus a manual menu item. No documented switch was found **[NOT VERIFIED against the manual: only third-party guides and a user report, see "What users say"]**. The user on Linux was notified immediately and got `DaVinci_Resolve_21.0.1_Linux.run` |
| Final Cut Pro | 12.3 | Mac App Store | no check of its own: the operating system's store updates it. Apple's own support article for "Final Cut Pro doesn't work after an update" tells the person to "Turn off automatic software updates on your Mac" and to back up the app and the libraries first — a library opened in the new version does not go back to the old one |
| Adobe After Effects | 26.x | Creative Cloud desktop app | the same as Premiere: the editor does not check, the manager does, and an open app is skipped |

**Outside the core.** Three of the five core lines answer "the editor does not check, a manager or the OS store does", and the two that do check show a dialog; the device of a check INSIDE an editor that does not interrupt, and the question of who decides "newer", are visible only here.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Shotcut | 26.9.27 | Help → Upgrade; the status label on the player | the ORIGIN of a non-modal notice in an editor. At EVERY start, with automatic checks on, it fetches `https://check.shotcut.org/version.json` (HTTPS, redirects onto cleartext refused); with them off, the same label offers "Click here to check for a new version of Shotcut." for 15 seconds — something is shown at every start either way. It compares `QVersionNumber`s, never reports for an `adhoc` (developer) build, and shows "Shotcut version %1 is available! Click here to get it." as a clickable status label for **15 seconds**; the code says of this path "This version does not currently log its message". A download URL from the JSON is accepted only if it is HTTPS on `shotcut.org`. On a network error it shows "Failed to read version.json when checking. Click here to go to the Web site." A manual check asks whether to check automatically in future, until "Do not show this anymore." is ticked; `--noupgrade` / `noupgrade=true` removes both the check and the menu item (for schools and institutions) |
| Visual Studio Code | 1.136 (source read from `main` on 2026-10-27; the latest release is 1.143.1) | `update.mode` = none / manual / start / default; an update indicator in the title bar | the first check is **30 seconds AFTER start**, then every hour; `start` means "Check for updates only on startup". The client does not compare versions: it sends its own COMMIT in the URL (`/api/update/{platform}/{quality}/{commit}`) and the server answers whether there is something newer. The state is shown passively in the title bar (`update.titleBar`, default on) and on the Manage gear; automatic updates are deferred on a metered connection. Unlike us it downloads and installs itself on Windows/macOS; on Linux the FAQ leaves updates to the package manager |

## The trick behind it

**1. The same transient notice earns both complaints at once — the cure is persistence plus a passive state, not a louder notice.** Shotcut's "a new version is available" lives 15 seconds in a status label and is not logged. One person calls it "this annoying feature" and asks how to stop it "without updating"; another, on the same mechanism, believes there is "no function for checking for newer versions". Both are true: a notice that repeats at every start and vanishes on its own is too frequent for the one who stays on an old build deliberately and too short for the one who looked away. VS Code solved it with two surfaces of different kind — an EVENT (the notification) and a STATE (an indicator in the title bar that simply stays until the build changes). A state does not interrupt and does not repeat, so it cannot nag.

**2. "Newer" is decided on one axis, and GitHub's axis is the COMMIT date, not the publication date.** VS Code does not compare versions on the client at all: it sends its commit, and the server decides. Shotcut compares semantic versions and suppresses the whole thing for `adhoc` builds, so developer builds never nag. GitHub's "latest release" is "the most recent non-prerelease, non-draft release, sorted by the created_at attribute", and "the created_at attribute is the date of the commit used for the release, and not the date when the release was drafted or published". This is exactly the axis of our version string: `FULL = ktYY.MM.DD-<hash8>` by the commit date. A release published later from an OLDER commit does not become "latest", and a tag named by hand (`v26.10.22`) is not the comparator — the asset name (`kinetren-<YY.MM.DD-hash>-…`) carries `FULL` without the `kt` prefix.

**3. Frequency is a privacy decision, not a performance one.** Pitivi's check requested `releases.txt` over plain HTTP "every time it is run": the issue that ended it named three costs at once — the server learns who uses the program and how often, anyone on the path can rewrite the answer, and an overlong version string in the answer "can prevent Pitivi from launching". The check broke the very thing it was meant to serve. VS Code waits 30 seconds after start before the first request, so starting never waits on the network. One request per calendar day with the answer cached reduces "how often" to "on which days", and GitHub's unauthenticated limit — 60 requests per hour per IP address — never matters, even for a classroom behind one address.

## Pros

- The notice in the bell persists until it is read, and the title suffix is a state, not an event: the two halves of Shotcut's complaint are covered by two existing surfaces, and neither interrupts (rule 10: one hint, one place, no pop-ups).
- The check is a read of a public JSON from the release channel that already exists (`distribution`: a `v*` tag publishes the tarball and the zip). Nothing new is hosted, signed or kept running.
- GitHub's "latest" and our version string are ordered by the same thing — the commit date — so the comparison needs neither semver nor a server of our own; a pre-release (`-rc1`) is excluded by the endpoint itself.
- Notice-only keeps the reasoning of F-RES-008 intact: no write into the install directory, no partial self-replacement, no class "the updater broke the binary".
- One request per calendar day tells GitHub on which days the program started, not how many times.

## Cons

- A second network path next to telemetry (F-COL-046), with its own switch. A person who turned telemetry off may expect silence; VEGAS puts both switches on one page, and the note on telemetry records Blender's single "Allow Online Access" gate for every network access as the opposite device.
- The release title shows `SHORT = ktYY.MM`: "kt26.10 (outdated)" can stand next to a release of the same month, and the title alone does not say which build is installed and which is out — the notice has to.
- Notice-only means the person still downloads and unpacks the archive by hand (F-RES-008); a notice without a route to the file is half of the job.
- A notice for the same release repeated at every start is exactly Shotcut's complaint. Our notices live in memory per process, so "the same release, again" is not caught by anything that exists today.
- A build installed by a distribution package (none today) would be told about a release its package manager does not have yet — VS Code leaves Linux updates to the package manager for this reason, and the Resolve report below shows a vendor check announcing a build on Linux.

## What users say

> So I find this annoying feature on Shotcut.
> — DiggaDigga2010, forum.shotcut.org, 31.05.2023, https://forum.shotcut.org/t/shotcut-update-box-how-do-i-disable-it/39192

> as far as I know there is no function for checking for newer versions. Wish it were…
> — KSnewbie, forum.shotcut.org, 09.05.2020 (the same 15-second label, missed), https://forum.shotcut.org/t/check-for-updates-feature/17664

> Currently, Pitivi makes a request to http://www.pitivi.org/releases.txt every time it is run.
> — pitivi #2276 "Update check is insecure and not private", 27.01.2019, https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2276

> Also immediately it told me there were a new version
> — Potente, discuss.cachyos.org (DaVinci Resolve on Linux), 24.06.2026, https://discuss.cachyos.org/t/davinci-resolve/31869

SIGNAL: an update check that already exists — 3 tasks in pitivi, `update.?check|check(ing)? for (new )?(updates?|upgrades?)|updates? (available|notification|notifier|reminder)|new (version|release) (is )?available|new version check|version checker|disab\w* (of )?version checks?`; a share of 0.1% against 0.0% (none of 6346) in kde-bugzilla

Across the whole `nle` corpus the filter finds 7 tasks out of 25,442, and 6 of them are about a check that ALREADY exists: how to switch it off (pitivi #190, 2007; shotcut #1275 — the build-time switch did not work with cmake), it broke (pitivi #1489 — the Python 3 port; losslesscut #290 — disabled on Windows), it is not private (pitivi #2276), its text is wrong (losslesscut #1543). The only request to have one is kde-invent #689 (2020, "An easy 1st step would be just a link to our download page"); next to it, outside the filter, kde-bugzilla #453416 "Ability to Auto-update Kdenlive" was closed WORKSFORME. The pain of this feature is born by the feature: nobody files "I did not know about the new version", people file "make it stop" and "it broke my start".

## How it fits Kinotrain

CAPABILITY: openspec/specs/distribution/ — the release channel ("Release on a version tag") exists; the requirement for the notice will be added there by the change.

### What already exists

- The release channel: `.github/workflows/release.yml` publishes a GitHub Release per `v*` tag with `kinetren-<YY.MM.DD-hash>-linux-x86_64.tar.gz` and `kinetren-<…>-windows-x86_64.zip` plus `.sha256`; a tag with `-` is a pre-release. The asset name is built in `src/scripts/release-build.sh` (`VERSION="${KT_VERSION_FULL#kt}"`), so it carries the full commit-date version.
- The version: `src/libs/kt-version/src/lib.rs` — `FULL = ktYY.MM.DD-<hash8>` by the commit date, `SHORT = ktYY.MM`, `display()` gives FULL in debug and SHORT in release.
- The title: `src/apps/kt-app/src/app.rs` ≈1496 builds `"{name}{ *} — {kt_version::display()}"` and sends `ViewportCommand::Title` (≈1506) only when the string changed — a suffix costs nothing per frame.
- The notice: `push_notice` (`app.rs` ≈3777) appends to the bell's log, collapses consecutive duplicates into "×N" and still raises the unread counter; the bell with its badge is drawn in the status bar (`app.rs` ≈6990).
- The network: `src/apps/kt-app/src/telemetry.rs` — a `ureq` agent with 1 s connect / 2 s total timeouts, one background thread spawned only on the first event, a compile-time key so that `cargo run`, tests and live scenarios never send; its checkbox stands in the View menu (`src/apps/kt-app/src/toolbar.rs:498`).

### What is missing

- Any read of the release channel from the program; any "what is the newest release" state; a per-day memory of the answer; the title suffix; the switch.

### What hinders

ADDRESS: src/apps/kt-app/src/app.rs — the class "the check delayed or broke the start" (pitivi #1489, #2276) is impossible for us only if the answer arrives the way telemetry sends: off the UI thread, with a hard timeout, and with a malformed or oversized answer treated as "unknown"; the class "the same release announced at every start" (Shotcut's complaint) is POSSIBLE and nothing catches it today — `push_notice` deduplicates only consecutive notices inside ONE process, and the bell's log is not kept across starts.

- The comparator: `SHORT` drops the day, so the comparison must use `FULL`'s date (or the asset name), not the title string and not the hand-written tag.
- Invariant 12: the UI must not wait for the network or repaint for it; the title already changes only on a string change.
- kt-app rules: 10 (no pop-up — the bell and the title, one place each), 13 (no new strip or row for the suffix), 19 (the notice says where it is switched off — the View menu), 22 (no new window), 27 (a useful default, remembered).

### Which invariants it touches

Invariant 12 (no waiting and no extra repaint in the UI). Invariant 11 is not touched: the check lives only in the interactive release program, not in an offline path. Invariant 10 is not touched: the TLS stack is the one telemetry already links (`ureq` with `rustls`), FFmpeg is unrelated.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage of a round |
| Where the main work is | kt-app (background read, cached answer, title suffix, View-menu switch); kt-version (comparison by the commit date) |
| Project format | does not touch |
| Tests that will have to be written | comparison of versions (older / same / newer / pre-release / malformed answer); "release build only"; no request on a second start the same day; harness: the notice goes to the bell and nothing modal opens; the switch in the View menu |
| Depends on | the release channel of `distribution` (exists) |
| Breaks compatibility | no |

## Verdict

Take, as a notice and nothing more. The difference from the refusal in F-RES-008 ("neither a server nor a signature, and a self-updating binary is a separate class of defects") is that the server now exists — GitHub Releases is our release channel — and a notice writes nothing to disk except its own cached answer, so the class "the updater broke the binary" does not arise. The tracker shows where the risk is: not in missing the release but in the check itself — it must never delay the start, must not repeat the same release, and must be easy to switch off. The owner has decided to add it; the status becomes `accepted` when the change opens, and then this section collapses to the capability and one line.

## Sources

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/12-preferences/prefgeneraltab.htm — VEGAS Pro 2026, General tab: "Use Newsfeed", "Send analysis data" (access date 2026-10-27)
- https://helpx.adobe.com/creative-cloud/help/update-app.html — Creative Cloud auto-update: returned 403; content from the search excerpt (access date 2026-10-27)
- https://support.apple.com/119959 — "If Final Cut Pro doesn't work on your Mac after an update", published 23.04.2024 (access date 2026-10-27)
- https://discuss.cachyos.org/t/davinci-resolve/31869 — Resolve 21.0.1 on Linux, the notice at start (access date 2026-10-27); the Resolve menu item from third-party guides only, not checked live
- https://github.com/mltframework/shotcut/blob/master/src/mainwindow.cpp — `versionCheckRequest`, `showUpgradePrompt`, `on_actionUpgrade_triggered`, `onUpgradeCheckFinished`, `showStatusMessage` (read 2026-10-27); https://shotcut.org/blog/new-release-1709/ — `--noupgrade`
- https://forum.shotcut.org/t/shotcut-update-box-how-do-i-disable-it/39192 and https://forum.shotcut.org/t/check-for-updates-feature/17664 (access date 2026-10-27)
- https://github.com/microsoft/vscode/blob/main/src/vs/platform/update/electron-main/abstractUpdateService.ts and `src/vs/platform/update/common/update.config.contribution.ts` — 30 s delay, hourly, `update.mode`, `update.titleBar`, the commit in the URL (read 2026-10-27); https://code.visualstudio.com/docs/supporting/faq — Linux and the package manager
- https://docs.github.com/en/rest/releases/releases#get-the-latest-release — "latest" = non-prerelease, non-draft, sorted by the commit date; https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api — 60 unauthenticated requests per hour per IP (access date 2026-10-27)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2276, https://invent.kde.org/multimedia/kdenlive/-/work_items/689 — tracker tasks; the count by `tracker-rank.py --query` over the dumps of 2026-09-08
- Nothing here was checked live in the five core editors: by documentation, a support article, forum reports and source code.
