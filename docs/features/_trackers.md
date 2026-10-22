# Tracker registry: sources

**A tracker is a source, not a column.** `_editors.md` answers the question "what do we compare with?"; this file answers the question "where was the number taken from?". A tracker line does NOT give an editor the right to a line in the table "Who has it and how": it gets there only through `_editors.md` and only by a mechanism. Here live the registry, the export method, the lexicon of topics and the table "where each topic landed". **There are no verdicts in this file and there cannot be** — the verdict lives in a note that has exactly one feature.

The first two waves read documentation and forums. The documentation of another editor tells WHAT it has; a tracker is the only available source that tells **what it costs**, and it does so by a count, not an impression.

Set up in the third wave (R.41). Updated FIRST in a new wave — the same order as `_editors.md` in rule 3.

## Registry

The column "Query" is the exact base URL of the export; `tracker-fetch.py` reads the registry from here and does not duplicate it in code. The export pin is the triple "date + number of tasks + sha256 of the normalized `.jsonl`"; drift is caught by `tracker-rank.py --check`.

| Key | Engine | Query | Tasks | Dump from | sha256 | Corpus |
|---|---|---|---|---|---|---|
| `kde-bugzilla` | bugzilla | `https://bugs.kde.org/rest/bug?product=kdenlive&limit=0` | 6346 | 2026-09-08 | `8b7608e9c1038d07` | nle |
| `kde-invent` | gitlab | `https://invent.kde.org/api/v4/projects/multimedia%2Fkdenlive/issues` | 2217 | 2026-09-08 | `94c2d87c52449f19` | nle |
| `pitivi` | gitlab | `https://gitlab.gnome.org/api/v4/projects/GNOME%2Fpitivi/issues` | 2672 | 2026-09-08 | `33e354f8d834b837` | nle |
| `shotcut` | github | `https://api.github.com/repos/mltframework/shotcut/issues` | 1368 | 2026-09-08 | `da38f084020cbacc` | nle |
| `mlt` | github | `https://api.github.com/repos/mltframework/mlt/issues` | 601 | 2026-09-08 | `3e1d460a91c3b21d` | nle |
| `otio` | github | `https://api.github.com/repos/AcademySoftwareFoundation/OpenTimelineIO/issues` | 680 | 2026-09-08 | `a6a428b122058c54` | nle |
| `olive` | github | `https://api.github.com/repos/olive-editor/olive/issues` | 1853 | 2026-09-08 | `78fd2ebfa068b81e` | nle |
| `losslesscut` | github | `https://api.github.com/repos/mifi/lossless-cut/issues` | 1920 | 2026-09-08 | `a078275dc3bd98fe` | nle |
| `flowblade` | github | `https://api.github.com/repos/jliljebl/flowblade/issues` | 981 | 2026-09-08 | `543779eee959c1f1` | nle |
| `openshot` | github | `https://api.github.com/repos/OpenShot/openshot-qt/issues` | 4907 | 2026-09-08 | `5fff45b0cdccee1d` | nle |
| `blender-vse` | gitea | `https://projects.blender.org/api/v1/repos/blender/blender/issues?labels=Interest%2FVideo%20Sequencer` | 3992 | 2026-09-08 | `bef31a931da614e2` | nle |
| `ffmpeg-trac` | trac | `https://trac.ffmpeg.org/query?status=!closed&format=csv&col=id&col=summary&col=type&col=component&col=priority&col=time&col=changetime&max=0` | 3095 | 2026-09-08 | `9fad84cabc92b2d3` | format |
| `syncthing` | github | `https://api.github.com/repos/syncthing/syncthing/issues` | 5946 | — | — | collab |
| `opencue` | github | `https://api.github.com/repos/AcademySoftwareFoundation/OpenCue/issues` | 834 | — | — | collab |
| `kitsu` | github | `https://api.github.com/repos/cgwire/kitsu/issues` | 741 | — | — | collab |
| `ayon-core` | github | `https://api.github.com/repos/ynput/ayon-core/issues` | 527 | — | — | collab |
| `flamenco` | gitea | `https://projects.blender.org/api/v1/repos/studio/flamenco/issues?type=issues` | 351 | — | — | collab |

The numbers in the "Tasks" column are an estimate taken when the wave was planned; the actual number after the export is recorded in the same place together with the date and the sha256.

**The "Corpus" column is not decoration, but the denominator.** The corpora are NOT compared with one another: shares are computed within their own. `nle` — twelve editing programs, the corpus of the third wave. `format` — FFmpeg trac: Trac has neither votes nor a comment counter, so its weight is incomparable. `collab` — five projects of the fourth wave, taken because none of the five core editors has a public tracker at all, and the mechanics of joint work are countably discussed only here. `tracker-rank.py --corpus <name>` (default `nle`) reads this column; previously in its place the script had a hardcoded `OUT_OF_RANK`, that is, exactly the duplication of the registry in code that this file forbids itself.

The column was added LAST deliberately: `read_registry()` in `tracker-fetch.py` and `read_tracker_keys()` in `features-index.py` are anchored on the first three columns, and a permutation would break both. `read_corpora()` finds the column by its HEADER, not by position.

## Who we do NOT take — and why

| Who | Reason |
|---|---|
| VEGAS, Premiere, Resolve, Final Cut, After Effects | **there is no public tracker for any of them.** The vendor's forum is not a tracker: it has no state, no deduplication, no decision date. This is precisely why the wave was entirely open-source — not "open source is more interesting", but "for the others the pain is not public" |
| Avidemux, Cinelerra-GG, VirtualDub | there is no tracker with an API (SourceForge, mailing list); a manual analysis breaks the requirement "reproducible by script" |
| Natron | archived: "is not fixed for years" loses its meaning where nobody fixes anything |
| CapCut, Descript, Runway | closed, there is no tracker |
| VLC, mpv, OBS, HandBrake | not editors; the HandBrake sample duplicates FFmpeg trac |
| Audacity, Ardour | sound only, and our audio path is our own (invariants 1, 2, 6); Ardour moreover uses Mantis without an API |
| egui, wgpu and our other dependencies | this is OUR pain from dependencies, not the editor's pain; its home is `docs/porting-notes.md` |

## How to read these numbers

1. **"Closed" does NOT mean "fixed".** For Kdenlive, `WORKSFORME` is 1,549 tasks out of 6,346 (24.4 %): a mass closing of stale ones. Everywhere `fixed_ratio` = FIXED / all closed topics is computed, and the formulation "closed, therefore fixed" is forbidden in the notes.
2. **Numbers are compared by shares, not by counts.** Trackers differ in size and culture: at Blender people comment three times as actively as at Flowblade. The weight of a task is divided by the median of its own tracker (not by the mean — the tails are heavy), and topics are compared by their share within their own tracker.
3. **FFmpeg trac is not in the common ranking.** Trac has neither votes nor a comment counter, and the sample is only open tasks: the denominator is incomparable. It answers one question — which formats keep breaking for years.
4. **What each engine does not give:** Bugzilla — votes and a comment counter (verified); Gitea — reactions in the list; Trac — neither of the two; GitLab — reactions beyond `upvotes`.
5. **GitHub returns pull requests in `/issues` as well** — entries with the field `pull_request` are discarded during normalization, therefore the actual number of tasks is lower than the number of entries.

## Complaint against a request — by the declared field

The wave collects **both features and requests**: the `wishlist` half is literally "what the users of other editors want", that is, the direct subject of the base.

| Engine | Request (wish) | Complaint |
|---|---|---|
| bugzilla | `severity == wishlist` (686 of 6346 at Kdenlive) | everything else |
| gitlab | label `Enhancement` | `Bug`, `Crash`, `Regression` |
| github | labels `enhancement\|feature\|idea\|RFE` | `bug\|crash\|regression` |
| gitea | `Type/Design`, `Type/To Do` | `Type/Bug` |
| trac | `type == enhancement` | `type == defect` |

Where there is no label — a heuristic by the title (`^(add|support|allow|please|would be nice)` → request; `crash|freeze|hang|broken|corrupt` → complaint), and **the share of unlabeled entries is printed on a separate quality line**. A hidden heuristic whose size is kept quiet is the fastest way to go stale.

A separate list holds the **rejected** items (`not_planned`, `WONTFIX`, `INVALID`, `Not Actionable`): "what the maintainers do NOT want to do" — for us the most direct signal that "the feature is not expressed in this architecture".

## Lexicon of topics

It is read by the script from here and is not duplicated in code — the same technique as reading categories from `README.md`. **A lexicon line is added BEFORE it is known into which note it will land.** Coverage is printed in the header of each table; uncovered items are shown by `tracker-rank.py --unmatched`.

| Topic | Regexp |
|---|---|
| crash | `crash\|segfault\|freeze\|hang\|abort\|not respond` |
| data loss | `lost\|lose\|corrupt\|data loss\|unrecoverable\|damaged project` |
| render/export | `render\|export\|encod\|output file\|transcod` |
| formats/import | `import\|codec\|container\|mp4\|mov\|mkv\|hevc\|h26\|prores\|vfr\|variable frame` |
| hardware/driver | `gpu\|opengl\|vulkan\|driver\|nvidia\|amd\|intel\|vaapi\|nvenc\|hardware accel` |
| performance | `slow\|lag\|perform\|freeze up\|stutter\|memory\|\bram\b\|leak` |
| proxy | `proxy\|proxies` |
| effects/transitions | `effect\|filter\|transition\|keyframe\|fade` |
| audio | `audio\|sound\|volume\|waveform\|sync.*audio\|mixer` |
| timeline | `timeline\|clip\|track\|trim\|ripple\|snap\|marker` |
| color | `color\|colour\|lut\|grade\|grading\|scope` |
| titles/text | `title\|text\|subtitle\|caption\|font` |
| interface | `ui\|gui\|layout\|theme\|icon\|dpi\|scaling\|shortcut` |
| install/build | `install\|build\|appimage\|flatpak\|snap\|package\|compile` |
| translation | `translat\|locale\|language\|i18n\|rtl` |
| project/file | `save\|autosave\|project file\|open project\|recent` |
| joint work | `collaborat\|multi.?user\|shared project\|team project\|(two\|multiple) (people\|users\|editors\|artists)\|concurrent(ly)? edit\|simultaneous(ly)? edit\|check.?(in\|out) (a )?(project\|file\|asset)` |
| cloud service | `\bcloud\b\|dropbox\|google drive\|onedrive\|nextcloud\|owncloud\|frame\.io\|web (app\|version\|interface)\|browser.based\|\bsaas\b` |
| network storage | `network (drive\|share\|storage\|folder\|path\|location)\|\bnas\b\|\bsmb\b\|\bcifs\b\|\bnfs\b\|samba\|unc path\|mounted (drive\|share\|volume)\|remote (drive\|storage\|folder)` |
| farm and jobs | `render farm\|render node\|worker node\|job (queue\|submission\|scheduler)\|distributed render\|opencue\|flamenco\|\bdeadline\b` |
| lock and access | `lock ?file\|\.lock\b\|already (open\|in use)\|another instance\|stale lock\|checked out\|read.?only (file\|project\|mode)\|permission denied\|\bacl\b` |
| review and comments | `annotat\|review (note\|round\|tool\|workflow)\|approval\|comments? (on\|to\|in) (a )?(clip\|strip\|frame\|shot\|timeline\|video)\|timecode.{0,12}(comment\|note)` |
| project transfer | `archive project\|bundle (the )?project\|consolidat\|relink\|package (the )?project\|(transfer\|move\|copy\|share) (the \|a \|my )?project\|portable project` |
| file sync | `file sync\|sync (folder\|files\|director)\|two.way sync\|syncthing\|rsync\|conflicted copy\|conflict(ing)? (copy\|file)` |

**Correction of the lexicon from 08.09.2026.** `ram` stood without word boundaries and caught `frame`, `program`, `parameter` — the topic "performance" was inflated threefold (966 against 271). It was found while writing `resilience/performance/responsiveness-under-load.md`. This is exactly the case for which the `--unmatched` mode and the rule "a lexicon line is added before the note is known" were set up: the lexicon is corrected FROM THE DATA.

**Eight lines at the bottom were added by the fourth wave (R.42)** — for the topics of joint work. Each was checked for false positives ON THE OLD corpus, which works here as a control group. Rejected as unfit: a bare `\bsync` (167 hits, almost all — AUDIO synchronization), `collaborat` alone (3 false out of 4: "Haiku port/collaboration", "collaboration from CinGG", "Archive project don't bundle fonts"), `merge|conflict` (76 out of 121 in losslesscut, where merge is the concatenation of video files, a profile function of the product), `\bgit\b` (368 hits in titles like "git master 2015-06-03 — Segfault") and `\block\b` (94 hits, almost all — the lock of a TRACK in the timeline, that is, our topic "timeline").

**A new mechanism of false positives, which wave 3 did not know.** The regexp `sign.?in` gave 28 hits and zero real ones: `match()` glues the labels of a task with a SPACE, and the free character `.?` jumps across the boundary between them — `Interest/Design Interest/Video Sequencer` reads as "…sign Interest…". The lesson is broader than about `ram` without a word boundary: dangerous is not only a class of characters that is too wide, but any pattern capable of crossing the joint of two glued fields. A new lexicon line must be checked not only for WHAT it catches, but also for BETWEEN WHAT it can slip through.

## Where each topic landed

Filled in batch 1 after the first ranking and pasted by hand from
`tracker-rank.py --table` (rule 20 — nothing grows on its own). **Not a single ranking
row is left without an assignment**: either `F-XXX-NNN`, or "not a feature: reason".

<!-- routing:begin -->
Ranking of the `nle` corpus as of 08.09.2026, 11 trackers, **25,442 tasks**, lexicon
coverage 74.5 %.

**The numbers were recalculated on 08.09.2026 and became SMALLER than before.** Previously
this stood at 27,537, and that was an 8 % overstatement: the `blender-vse` dump lay in
duplicate in full (3992 lines for 1996 tasks — resuming appended to the file without
deduplication) and contained 100 pull requests, while `ffmpeg-trac` had 3095 records, ALL
of them empty (`csv.DictReader` parsed a response with a BOM and capitalized column names
and silently produced garbage). Both sources were re-downloaded, the class closed by checks.
**Not one countable claim in the notes was affected**: these two keys appear in no
`trackers:` field and in no `SIGNAL:` line, and the shares are computed WITHIN their own
tracker. The earlier numbers are kept in this paragraph deliberately — by the same rule
under which the "Caveat about the mobility of the numbers" is kept below.
Columns: `n` — tasks, `fixes` — share of REAL fixes among closed ones, `universal` —
minimum share of the topic across trackers (high = the pain is structural), `gap` — spread
of the shares.

| Topic | n | fixes | universal | gap | Where it landed |
|---|---|---|---|---|---|
| timeline | 5630 | 0.81 | 5.5% | 1.9 | F-TL-006, F-TL-007, F-TL-008, F-TL-009, F-TL-010 |
| crash | 4072 | 0.79 | 1.0% | 1.4 | F-RES-004, F-HLP-014 |
| render/export | 3701 | 0.74 | 2.6% | 2.0 | F-EXP-004, F-RES-007, F-RES-012 |
| effects/transitions | 3602 | 0.76 | 3.8% | 2.6 | F-AUT-005, F-TL-010 |
| formats/import | 2453 | 0.83 | 4.4% | 1.2 | F-RES-009, F-MED-004, F-MED-006 |
| sound | 2262 | 0.86 | 0.4% | 1.3 | F-AUD-002, F-AUD-006, F-AUD-008 |
| interface | 2186 | 0.88 | 4.6% | 2.5 | F-UI-003, F-UI-004, F-UI-005, F-HLP-027 |
| installation/build | 1676 | 0.79 | 2.3% | 1.8 | F-RES-008 |
| titles/text | 1620 | 0.83 | 0.3% | 2.4 | F-TXT-005, F-TXT-006 |
| performance | 746 | 0.87 | 1.6% | 1.6 | F-RES-010, F-RES-011 |
| color | 710 | 0.84 | 1.0% | 1.6 | **not a feature of the wave**: the `color/` category was closed by the first wave (6 notes), the tracker gave no new topics |
| project/file | 675 | 0.80 | 0.1% | 1.5 | F-RES-001, F-RES-003, F-RES-005, F-PRJ-002, F-PRJ-003 |
| hardware/driver | 374 | 0.74 | 0.3% | 4.0 | F-RES-006 |
| data loss | 417 | 0.81 | 0.1% | 1.8 | F-RES-002, F-MED-005 |
| proxies | 350 | 0.86 | 0.1% | 2.4 | **not a feature of the wave**: F-MED-002 (first wave) already covered proxies; the tracker confirmed its conclusion and gave no new topics |
| translation | 328 | 0.89 | 0.2% | 1.4 | F-HLP-033 |

**Caveat about the mobility of the numbers.** The first ranking was computed over THREE
trackers, and there the share of fixes for "hardware/driver" was 0.46 — the worst of the
sixteen topics. Over the full eleven it is 0.76, sharing last place with "render/export"
(0.75), and the whole spread compressed to 0.75–0.89. So the early "sensation" turned out
to be an artifact of a small corpus. Only those notes survived which DECLARED the scope of
the count ("0.46 across three trackers") — and that is the practical argument for the
requirement of rule 12 to state the filter and the trackers explicitly.
<!-- routing:end -->

## Where each topic of the second corpus landed (wave 4, R.42)

Ranking of the `collab` corpus as of 08.09.2026: 4 trackers, 2453 tasks, lexicon coverage
37.4 %. **`syncthing` (5946 tasks) is still being downloaded as of this date** — without a
GitHub token it returns 60 requests per hour. The numbers below refer to the four downloaded
sources and will be recalculated; the scopes of the counts for the `SIGNAL:` rows are stated
in the notes themselves.

Low lexicon coverage here is not a defect but a property: the lexicon is tuned to editing
programs, while the corpus is pipelines and render farms. Two columns cannot be read in this
corpus: `duplicates` is zero everywhere, because GitHub does not return `dupe_of`; corpora
can be compared by shares of fixes, but not by density of duplicates.

| Topic | n | fixes | gap | Where it landed |
|---|---|---|---|---|
| farm and jobs | 123 | 0.96 | 6.9 | F-COL-038, F-COL-039, F-COL-042 |
| cloud service | 20 | 0.71 | 5.1 | F-COL-041, F-COL-043, F-COL-044 |
| review and comments | 15 | 1.00 | 6.2 | F-COL-020…F-COL-027 |
| network storage | 5 | 1.00 | 0.0 | F-COL-001, F-COL-002 |
| file synchronization | — | — | — | F-COL-032, F-COL-035 (the count awaits the `syncthing` download) |
| lock and access | 1 | 1.00 | 0.0 | F-COL-013…F-COL-019 |
| co-working | 1 | 1.00 | 0.0 | F-COL-029 (owns the count "one request per 25,442") |
| project transfer | 1 | 1.00 | 0.0 | F-COL-007, F-COL-009, F-COL-010 |

**The main number of the wave is not a table row but the comparison of the two corpora.**
Eight collaboration topics cover **0.33 % of the corpus of editing programs** (85 tasks out
of 25,442) and **6.52 % of the corpus of pipeline tools** (160 out of 2453) — a twenty-fold
difference. The pain of collaboration exists in the world; in video editors it does not.

**The second number is the share of real fixes.** In the `collab` corpus it is 0.94–1.00
for almost all topics (0.96 for "farm and jobs"), in the `nle` corpus 0.74–0.89. Pipeline
tools fix what they are reported about; editing programs do not. A caveat of honesty: the
corpora differ in size and age, and the explanation may be either the culture of maintenance
or the fact that for a tool for engineers the task is filed by someone able to reproduce it.

## Licenses and what does not go into the repository

None of the twelve sources grants permission for mass redistribution of task texts: KDE
Bugzilla and invent.kde.org — there is no explicit license on the text of reports; GNOME
GitLab — none; GitHub — the ToS grant a license to GitHub, not to us; projects.blender.org —
under Blender's rules; FFmpeg trac — none.

The conclusion is the same for all of them: **we do not redistribute the texts in bulk.** The
dumps live outside the repository — `${XDG_CACHE_HOME:-~/.cache}/kinotrain/trackers/`,
because they amount to ~150 MB, there is no license, and no test needs them offline (unlike
`src/fixtures/otio-oc`, which lies in the tree under `.gitignore` for the sake of test 179).
Only a quote of ≤2 sentences with a link and a date goes into the repository — exactly what
rule 4 of the catalog already requires.

Reproducibility rests on **a pin, not on bytes**: the exact request URL, the download date and
the sha256 of the normalized dump are in the registry table above.

## Token

GitHub without authorization — 60 requests per hour, with a token — 5000. The owner puts the
token by hand into `.env` in the project root (the file is excluded by the `.gitignore` line
`/.env`). The script reads from there EXACTLY the key `GITHUB_TOKEN`, **and never prints,
logs or saves it**; in reports only "token: present/absent" is visible. Without a token
everything works — just more slowly.
