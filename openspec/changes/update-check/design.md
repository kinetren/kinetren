# Design

## Context

See proposal.md — Why. What is already there and shapes the approach:

- **Releases** are GitHub Releases of `kinetren/kinetren`, made only from a `v*` tag by
  `.github/workflows/release.yml`; a `-` suffix marks a pre-release. The assets are named by
  `src/scripts/release-build.sh` as `kinetren-<YY.MM.DD-hash8>-<linux|windows>-x86_64.<ext>` —
  the same commit-date version `kt-version` compiles into the binary (`kt_version::FULL` =
  `ktYY.MM.DD-hash8`, `SHORT` = `ktYY.MM`, the title shows `display()`). The distribution design
  says the tag is only the release's name.
- **Network** already exists in kt-app: `ureq` 3 with rustls (no OpenSSL in `ldd`) for telemetry
  (`telemetry.rs`) and the Windows FFmpeg download (`ffmpeg_setup.rs`, environment proxy, none for
  loopback). `serde_json` is a kt-app dependency. No new crate is needed.
- **Telemetry is the template** for a process-wide background helper: `install`/`global` before
  the first `KtApp` in `run`, the first-frame hook in `draw_all`, the choice in eframe storage
  next to `lang`, the View checkbox next to Language with a disabled state and a reason, a debug
  override URL that release builds ignore, tests through `_in(dir)` functions on temp dirs.
- **Notices**: `KtApp::push_notice(Severity, String)` feeds the ring the bell counts. Notices are
  plain text, per session (not persisted).
- **Title**: `draw_all` builds `"{name}{ *} — {display()}"`, sends it to the OS when it changes
  (`last_title`), and `toolbar.rs` paints `last_title` in the custom title row (grey, 11.5 px)
  and places the nest crumbs right after its rect.
- **Clock**: kt-app has no time-zone library; `chrono_like_now()` is UTC.

## Goals / Non-Goals

**Goals:**
- One request per UTC day per data directory, never on the UI thread, never waited for.
- Never call a build "outdated" by mistake (the conservative direction is silence).
- Dev builds, tests and live scenarios never reach GitHub.

**Non-Goals:**
- Downloading, verifying or installing a release; a self-updater (F-RES-008's verdict about a
  self-updating binary stands — this change adds a notice, not a channel that writes files).
- A "Check now" menu item, release notes in the program, a check during a session that crosses
  midnight, pre-release channels.
- Telling kt-cli users anything.

## Decisions

### D1. Ask `GET /repos/kinetren/kinetren/releases/latest` of the GitHub API
That endpoint already returns only the newest published normal release (no drafts, no
pre-releases), so one small JSON answers the question. We read only `tag_name`, `draft`,
`prerelease` and `assets[].name` into a minimal serde struct; unknown fields are ignored; the
body is read up to 1 MiB.
Alternatives: the `releases.atom` feed (XML parsing, no pre-release flag); listing tags (no
pre-release flag, paging); a `LATEST` file on a website (there is no website; `LATEST` exists only
in local `dist-out`); scraping the releases page (breaks with markup).

### D2. "Newer" comes from the asset name, not the tag
The binary knows its commit date and hash; the asset name carries exactly that pair. Newer ⇔ the
release's date is strictly later AND the hash differs. A same-day release with another hash is
"not newer": we cannot order two commits of one day from the date alone, and the cost of
silence is one day. Tags are not compared because the tag can be dated after its commit
(`v26.11.01` on a commit from 26.10.30) — the very release would call itself outdated.
`published_at` against a build date was rejected for the same reason (the binary has no build
date by design: determinism and the commit-date version). t872 also reads `release-build.sh` to
catch a rename of the asset form.

### D3. The day is the UTC day
"Today we have not checked yet" needs a calendar day. A local day needs a time-zone database
(a new dependency, per-OS behaviour); the UTC day shifts the boundary by at most a few hours,
which changes only WHEN the next daily request goes out. Computed from `SystemTime` (wall clock
is allowed in the UI; nothing offline touches it).

### D4. The day's answer lives in `<data_dir>/update-check`; the on/off choice in eframe storage
The answer arrives on the background thread, which writes it itself (`day=YYYY-MM-DD`,
`latest=ktYY.MM.DD-hash8` or empty; tmp + rename, so a kill mid-write leaves the old file).
eframe storage is written by `save()` every 30 s and on exit: an answer kept only there would be
lost by a crash in the first 30 s and re-asked. The choice is a per-machine preference and goes
where `lang` and `telemetry` already are. Two instances on one day may both ask once — harmless.
Alternative: storing the date in eframe storage — rejected for the crash window above.

### D5. One checker per process, published to the app through a slot
Like telemetry: `update_check::install(Checker::for_process(stored_choice))` in `run`, read with
`update_check::global()` by every `KtApp` constructor, so the nine `*self = KtApp::…` rebuilds
(File → New/Open) neither re-ask nor re-post the notice. The thread is spawned lazily on the first
frame only if the check is on and due. It stores its `Outcome` (`Newer(version)` / `NotNewer` /
`Failed`) in a `Mutex<Option<…>>` and calls `ctx.request_repaint()` once; `draw_all` takes the
outcome on the next frame, posts the notice once (a `noticed` flag in the checker), and the
title reads `checker.outdated()` every frame (a cheap atomic/mutex read, no allocation when
unchanged). No repaint loop while waiting (invariant 12).

### D6. `on_exit` does not wait
Unlike telemetry's `app_closed`, nothing is lost if the process ends mid-request: the day is not
recorded and the next start asks again. The thread is detached; exit is never delayed.

### D7. The title word: one string for the OS, a split paint in our row
`draw_all` appends `" (" + tr(k::update::OUTDATED) + ")"` — the parentheses live in the catalog
entry, so Arabic and Chinese can use their own — to the title when `outdated()`; the OS title
gets the whole string. `toolbar.rs` paints the base title as today and, when outdated, the word
right after it in the warning colour the bell uses, with its own hover rect → `self.hint(…)`
showing the new version and the releases address (rule 10: one hint, in the status bar); the
crumbs start after the word. `KtApp` keeps `title_outdated: bool` next to `last_title` instead of
re-parsing the string.

### D8. One notice, info severity, text from one catalog entry
`tf!(k::update::NOTICE, new_version, own_version)` — the text names both versions, the releases
address `github.com/kinetren/kinetren/releases` (not a clickable link: notices are text) and the
View item by the same catalog key the checkbox uses, so the two cannot disagree (as telemetry
does). Severity Info: a newer version is not a fault, and the warn colour of the bell stays for
faults. The versions in the notice are the full `ktYY.MM.DD` form (both), so a person can tell
two releases of one month apart.

### D9. Consent and the debug door
`resolve_consent(debug, env_url, env_switch, stored)` returns `{on, url, ignore_day, block}`,
a pure function (t877). Debug without `KT_UPDATE_URL` → blocked by the build (the checkbox is
disabled with the reason). Debug with `KT_UPDATE_URL` → on, that URL, the day ignored (so the live
scenario can repeat). Release → the GitHub URL always; `KINETREN_UPDATE_CHECK=0|off|false` blocks;
stored `off` turns off. Turning the checkbox on in a session does not start a request — the next
start does (no network surprise mid-work); turning it off clears `outdated()` at once.

### D10. The HTTP agent copies the existing rules
Same construction as `ffmpeg_setup.rs`: environment proxy, none for loopback; connect 5 s, global
10 s; headers `Accept: application/vnd.github+json` and `User-Agent: kinetren/<FULL>` (GitHub
rejects requests without a User-Agent). Any non-200 (including 403/429 rate limit and 404 while
the repository is private) is `Failed`: silent, one journal line.

## Risks / Trade-offs

- [GitHub's unauthenticated limit is 60 requests/hour per IP; a shared NAT could hit it] →
  one request per machine per day; a 403 is a silent failure and the next start retries.
- [The repository or asset naming changes and every release becomes "not newer" forever,
  silently] → t872 reads `release-build.sh`'s naming line; the repository address is one
  constant next to the distribution design's "final repository" decision.
- [A wrong system clock] → it only moves when the daily request happens; "newer" never uses
  the clock.
- [The second network path of the program: GitHub sees the IP once a day] → the checkbox hover
  text says what is asked and that nothing but the version is sent; the switch is in View and
  in the environment; the telemetry notice is not touched because this is not telemetry.
- [The notice is lost when the person never opens the bell] → by design: the title word stays
  for the whole session, every session, until the person updates.
- [A session open across midnight does not re-check] → accepted (Non-Goals); the next start does.

## Migration Plan

Nothing to migrate: a missing `update-check` file means "not checked today"; a missing
`update_check` storage key means "on". Rollback = remove the module; the file left in the data
directory is ignored by older builds.
