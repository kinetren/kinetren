---
id: F-COL-046
title: Anonymous usage telemetry — launches, time in the program, clean exit
status: кандидат
priority: C
cost: M
category: collab/depend
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, blender-5.2]
we_have: нет
crates: [kt-app, kt-version]
invariants: [12]
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [posthog]
checked: 2026-10-22
---

# F-COL-046 — Anonymous usage telemetry — launches, time in the program, clean exit

## Суть

The editor itself sees nothing: no new panel, no new step in the work. Once, on the
first start, it is told in one place that the program counts its own launches,
how long a session lasts and whether the session ended with a normal close, and
where the switch is that turns this off. After that the only visible trace is the
switch itself. What the person gets back is indirect: the author learns how many
people actually run a given build and how often it dies under them.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Preferences → General → «Send analysis data to Boris FX» | a checkbox inside the program, next to «Use Newsfeed» (update notices). The documentation says only what it does when checked; the default and the content of «analysis data» are not documented |
| Adobe Premiere | 26.x | «Desktop app usage» (Account → Data and privacy) | ONE account-wide switch on `account.adobe.com` for all Adobe desktop apps; per community answers the in-app privacy menu was removed. Diagnostic data is still collected after opt-out; beta/prerelease builds always send usage data; Enterprise/Education accounts have collection disabled and no switch **[НЕ ПРОВЕРЕНО по первоисточнику: `adobe.com` не открылся, текст FAQ — из поисковых выдержек]** |
| DaVinci Resolve | 21 | no documented switch | neither the manual excerpts nor the Blackmagic privacy policy (updated 02.10.2026) describe usage analytics of the program; the policy names only device data «for the purposes of registration or activation of the software». Absence of a documented switch is not proof of absence of collection |
| Final Cut Pro | 12.3 | Final Cut Pro → Privacy & Analytics → «Share Analytics Data» | an in-app menu. Apple's privacy page lists what opening the app collects (session length, app version, device type, country code, random identifiers, OS, subscription) and says it is stored against a random identifier, not the Apple Account. Separately, macOS has a system-wide «Share with app developers» |
| After Effects | 26.x | «Desktop app usage» | the same Adobe account-wide switch as Premiere: the setting is not in the program at all, so a person looking for it in AE preferences does not find it **[НЕ ПРОВЕРЕНО по первоисточнику, как и строка Premiere]** |

**Вне ядра.** Three of the five core rows are «a switch somewhere» with an undocumented
default, so the mechanism — levels, the first-run notice, the cost of getting it wrong —
can only be described by software that published it.

| Программа | Версия | Как называется | Как устроено |
|---|---|---|---|
| Visual Studio Code | 1.136 | `telemetry.telemetryLevel` | four levels, not a boolean: `all` (crash + error + usage), `error` (crash + error), `crash`, `off`. «Developer: Show Telemetry» opens an Output channel that shows every event as it is sent. For GDPR Microsoft places «a notification in product for all existing and new users» |
| Blender | 5.2 LTS | Preferences → System → «Allow Online Access» | not telemetry at all: since 4.2 one opt-in switch gates EVERY network access (extensions, update checks), asked on first start, with command-line overrides `--offline-mode` / `--online-mode`. Blender sends no usage data **[НЕ ПРОВЕРЕНО по первоисточнику в этой сессии: `docs.blender.org` отдал 403]** |
| Audacity | 3.0 line, PR #835 (05.2021) | «Basic telemetry for the Audacity» | the precedent of a backlash: telemetry that was «strictly optional and disabled by default» (Google Analytics for session start/end, errors, effects and formats; Yandex Metrica for daily active users), closed unmerged a week after it was opened |

## Фишка фишки

**1. «Abnormal session = a start without a close» counts the network, not only
crashes.** The owner's decision (only an exit event, no crash marker on disk) means the
denominator of «died» is every session whose exit event did not ARRIVE. A person who
closes the program on a train, a laptop that sleeps and is then powered off, a firewall
that lets the start out but not the exit — all of them look exactly like a panic. The
PostHog Rust SDK makes it sharper: `capture` only queues the event for a background
worker, and the documentation warns to call `flush()` or `shutdown()` before exit or
«queued events may be lost». So the clean-exit share is a LOWER bound of clean exits,
and it can be read only next to a second number that says how many sessions had no
network at all. Neither VS Code nor Apple publishes how they separate «crashed» from
«did not report»; Audacity's PR tracked «session start and end» and never got far
enough for the question to arise.

**2. The backlash was not about the default — it was about whose host.** Audacity's PR
was opt-in and off by default, and it still collected over 2,000 thumbs-down and was
closed. The retreat statement (Martin Keary, 13.05.2021) named the real cause: the
convenience of Google and Yandex as hosts was at odds with how trustworthy the program
looked, «so we will be self-hosting instead». Adobe shows the mirror image: the
switch is opt-out and lives off the program, on a web page, yet the 2019 complaint
quoted below is not about the data — it is that the promised switch could not be FOUND.
The two cheap levers are therefore the ones that cost nothing in code: name the host and
the region in the notice, and put the switch where the notice says it is.

**3. Anonymous is a field, not a promise — and the defaults are not anonymous.** In
PostHog an event creates a person profile unless it carries `$process_person_profile:
false` (such events are also «up to 4x cheaper»), and the GeoIP plugin enriches every
event with city and coordinates from the sender's IP unless the event carries
`$geoip_disable: true`. The official Rust crate `posthog-rs` 0.27.0 (24.09.2026) depends
on `os_info` and `reqwest` with `rustls`/`blocking`/`json`/`gzip`, i.e. it collects the OS
fingerprint by itself. The «do not track» convention has its own trap: `DO_NOT_TRACK=1`
is honoured by a number of CLIs (Dagger, Railway, Contrast), but the convention's own home
page `consoledonottrack.com` is, on 22.10.2026, a squatted unrelated site — a convention
without a living reference, which a program can honour but cannot link to.

## Плюсы

- Three numbers answer the question nobody can answer today: how many people run a
  given RELEASE build, how long they stay, and which build dies more often. Version is
  already a single string (`kt_version::SHORT`), so «which build» needs no new identity.
- Opt-out with a one-time notice gives a usable sample; Audacity shows that opt-in can
  cost the same trust as opt-out, while producing a sample too small to read.
- Only two events per session (start and exit), so the volume stays far inside a free
  tier and the payload is small enough to show in full in the notice itself.
- Events are anonymous by construction (`$process_person_profile: false`) — the cheaper
  path in PostHog is also the private one.

## Минусы

- «Start without close» conflates crash, kill, sleep and no network (point 1). The number
  is honest only with that caveat written next to it.
- The first network code in the whole tree: a TLS stack and an HTTP client become new
  dependencies of the release binary, built under `scripts/release-build.sh` with its
  `GLIBC_` ceiling.
- The exit event must leave inside `on_exit`, which already waits up to 5 s for the
  background autosave; a slow network adds to the time between closing the window and
  the process really ending.
- Opt-out is a judgement the community may not share: Audacity was attacked even with
  opt-in, and «on by default» will be quoted against the program in exactly that
  register.
- Debug builds (the owner's daily `target/debug/kinetren`, live scenarios, tests) must
  never report, otherwise the author's own launches dominate the sample.

## Что говорят пользователи

> On install I was forced to agree to share usage info with Adobe promising I can turn
> it off later in my profile. Where exactly can I do it?
> — user5682590, Adobe Community, 18.09.2019,
> https://community.adobe.com/questions-617/how-to-turn-off-usage-sharing-info-with-adobe-570742

> I have used audacity for decades and am very disappointed.
> — @eevvoor, GitHub audacity/audacity PR #835, 10.05.2021,
> https://github.com/audacity/audacity/pull/835

> Overall there are much better alternatives to baking telemetry directly into the core
> application.
> — @BenBE, там же, 11.05.2021

The positive side has no quote: nobody writes «I switched to this editor because it
counts my launches». That silence is the expected shape of the domain — telemetry is
noticed only when it goes wrong.

## Как ложится на Kinotrain

CAPABILITY: openspec/specs/durability/
(the closest existing capability: `openspec/specs/telemetry/` does not exist yet and the
checker requires an existing one; the planned change `telemetry-posthog` is to create it.)

### Что уже есть

- `src/apps/kt-app/src/app.rs:9013` — `pub fn run()`: the single start; `data_dir()` is
  resolved first (`:9017`), `eframe::run_native("kinetren", …)` at `:9043`,
  `install_panic_hook()` at `:9058`, and at `:9079` `take_crash_report()` turns the last
  session's panic into a warning. This is the place where «a session started» is known.
- `src/apps/kt-app/src/app.rs:8957` — `fn on_exit`: the one point of a normal close; it
  already blocks up to 5 s on the background autosave (`recv_timeout`) and writes recovery.
  `:8943` — `fn save(&mut self, storage…)` stores `lang` and `last_project` in eframe's
  storage every 30 s; a «notice was shown» flag and the switch belong to the same storage.
  `:9050` — `KT_LANG` env override read before storage: the existing pattern for an env
  override of a stored choice.
- `src/apps/kt-app/src/journal.rs` — `data_dir()` (`$XDG_DATA_HOME/kinetren`, the single
  resolver for recovery, journal, locks and eframe storage), `install_panic_hook()`,
  `crash_path()`, `take_crash_report()`. A panic already leaves a file and is reported
  locally at the next start; SIGKILL, abort and power loss leave nothing.
- `src/apps/kt-app/src/toolbar.rs:473` — the Language submenu inside the View menu,
  placed there by checklist rule 13 («no new strip for it»); the natural neighbour for a
  usage-data switch.
- `src/libs/kt-version/src/lib.rs` — `FULL`/`SHORT` and `display()` switching on
  `cfg!(debug_assertions)`: the build-type fact that must also gate reporting.
- `src/Cargo.lock` — 509 packages, none of `reqwest|hyper|ureq|rustls|native-tls|openssl|
  tokio|http`; no `std::net` or `TcpStream` anywhere in `src/`. There is no network code
  today, in any form.

### Чего не хватает

A session id, a start event, an exit event that is flushed before the process ends, a
first-start notice, the switch and its env override (including honouring `DO_NOT_TRACK`),
and a translated notice text in every catalogue under `src/apps/kt-app/i18n/`.

### Что мешает

Two copies of the program share one eframe storage (F-COL-045): the «notice shown» flag
and the switch are per machine-user, not per copy — acceptable, but the second copy must
not show the notice again or overwrite the first copy's choice. `on_exit` is not reached
at all when the app is re-created in place (`*self = app`, see `Drop for KtApp`), which is
correct for telemetry (that is not an exit) but means the exit event cannot be tied to
`Drop`.

### В категории `collab/` — две обязательные строки (правило 13)

БЕЗ СЕТИ: ВЫРОЖДАЕТСЯ — `src/apps/kt-app/src/journal.rs` — on one machine «did the last session end cleanly» already degenerates into the panic report: the hook writes kt-app-crash.txt into the data directory and the next start shows it once, so a panic is known locally; launches, session length and every non-panic death (kill, abort, power loss) are recorded nowhere, and nothing is ever summed across machines.

ЦЕНА СЕТИ: summing starts and exits over every installation into daily users, session length and the clean-exit share — внешний исполнитель — the aggregation is done by PostHog's host, not by our process; without it each copy knows only its own last panic, and the question «how many people run this build and how often it dies» has no one to answer it.

### Какие инварианты задевает

Invariant 12 (UI does not heat the system): sending must not wake repaints or run in a
draw path; it lives on its own thread and only at start and exit. Invariant 10 (no
linking with libav*) is untouched, but the release guard in `scripts/release-build.sh`
also checks the `GLIBC_` ceiling, which a TLS crate with native code must pass. Invariant
11 (no wall-clock in offline paths) is not touched: kt-cli and export never report.

## Оценка

| Что | Ответ |
|---|---|
| Объём | M — 2 stages: sending + build gating, then the notice, switch and i18n |
| Где основная работа | kt-app (start/exit hooks, notice, switch); a new HTTP/TLS dependency |
| Формат проекта | does not touch it: nothing is written into `.kt` |
| Тесты, которые придётся написать | debug build never sends; switch off and `DO_NOT_TRACK=1` send nothing; payload is exactly the documented fields with `$process_person_profile: false` and `$geoip_disable: true`; the notice is reachable and translated |
| Зависит от | the owner's decisions of 2026-10-22 (PostHog Cloud EU, opt-out, exit event only) |
| Ломает совместимость | no |

## Вердикт

Taken: `кандидат` → planned OpenSpec change `telemetry-posthog` (new capability
`telemetry`). Why: two anonymous events answer «how many run it and how often it dies»,
which nothing in the tree can answer today, at the price of one dependency and one
notice — and the research says the price of failure is trust, so the notice must name
the host and region and the switch must be where the notice says.

## Источники

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/12-preferences/prefgeneraltab.htm
  — «If this is checked, Vegas Pro will send analysis data to Boris FX for product
  improvement purposes.»; neighbour «Use Newsfeed» (дата обращения 2026-10-22).
- https://www.adobe.com/uk/privacy/desktop-app-usage-info-faq.html — Usage Data FAQ:
  account-wide «Desktop app usage», diagnostic data after opt-out, betas always send,
  Enterprise/Education disabled. Сам сайт не открылся («proxy refused the connection»);
  содержание — по поисковым выдержкам, дата обновления в выдержке 05.03.2026 (обращение
  2026-10-22).
- https://community.adobe.com/questions-617/how-to-turn-off-usage-sharing-info-with-adobe-570742
  — user5682590, 18.09.2019, and replies about `account.adobe.com/privacy` (обращение
  2026-10-22).
- https://www.blackmagicdesign.com/privacy — last updated 02.10.2026; device data for
  registration/activation, nothing about usage analytics of DaVinci Resolve (обращение
  2026-10-22).
- https://www.apple.com/legal/privacy/data/en/final-cut-pro — «Final Cut Pro > Privacy &
  Analytics … Share Analytics Data», random identifier not associated with the Apple
  Account; page date 29.09.2026 (обращение 2026-10-22).
- https://code.visualstudio.com/docs/configure/telemetry — `telemetry.telemetryLevel`
  table, «Developer: Show Telemetry», GDPR notification; page date 21.10.2026 (обращение
  2026-10-22). The default value `all` is the setting's default in the product, not stated
  on this page.
- https://github.com/audacity/audacity/pull/835 — «Basic telemetry for the Audacity»,
  opened 04.05.2021 by crsib, closed unmerged; comments of @eevvoor and @BenBE (обращение
  2026-10-22).
- https://www.theregister.com/2021/05/14/audacity_telemetry/ — Martin Keary's statement
  of 13.05.2021 in https://github.com/audacity/audacity/discussions/889, quoted from the
  article (обращение 2026-10-22).
- https://posthog.com/docs/data/anonymous-vs-identified-events — `$process_person_profile:
  false`, «up to 4x cheaper» (обращение 2026-10-22).
- https://posthog.com/docs/api/capture — `/i/v0/e/` and `/batch/`, `https://eu.i.posthog.com`
  for EU Cloud, required `api_key`, `distinct_id`, `event` (обращение 2026-10-22).
- https://github.com/PostHog/posthog-plugin-geoip/blob/main/README.md — `$geoip_disable`
  skips GeoIP enrichment of the event (по поисковой выдержке, обращение 2026-10-22).
- https://posthog.com/docs/libraries/rust and crates.io API for `posthog-rs` 0.27.0
  (24.09.2026): `capture` queues, `flush()`/`shutdown()` before exit; dependencies
  `reqwest` (rustls, blocking, json, gzip), `os_info`, `uuid` (обращение 2026-10-22).
  The docs page still shows version 0.14 — it lags the crate.
- `DO_NOT_TRACK=1`: honoured per docs.dagger.io (CLI telemetry), docs.railway.com/cli/telemetry,
  docs.edgeless.systems (Contrast); `consoledonottrack.com` itself served an unrelated site
  on 2026-10-22.
- Blender «Allow Online Access» (4.2+) — по памяти и release notes 4.2; `docs.blender.org`
  отдал 403, живьём не проверялось.
- Free-tier volume of PostHog Cloud is not quoted here: `posthog.com/pricing` returned no
  pricing text to the fetcher.
- Наш код проверен чтением и грепом 2026-10-22: `src/apps/kt-app/src/app.rs` (`:8943`,
  `:8957`, `:9013`, `:9043`, `:9050`, `:9058`, `:9079`), `src/apps/kt-app/src/journal.rs`,
  `src/apps/kt-app/src/toolbar.rs:473`, `src/libs/kt-version/src/lib.rs`,
  `src/Cargo.lock` (509 packages, no HTTP/TLS crate). None of the five core editors was
  checked live; all rows are from documentation and forums.
