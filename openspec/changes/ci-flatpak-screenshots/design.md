# Design: ci-flatpak-screenshots

## Context

`ci-red-again` gave the host compose its SVG loader, and the `flatpak` job of 21505ad2 got past
compose, the build and the install for the first time. Its next step, `flatpak-builder-lint
org.flatpak.Builder repo`, had never run. ci-red-again's design already saw the cause in passing:
"the webp screenshots still give `screenshot-save-error` warnings … they do not fail compose,
which stores no media under flatpak-builder". Compose does not fail. The linter does.

## What the linter checks (flatpak-builder-lint `checks/screenshots.py`, master of 2026-11-03)

For an app ref of a `desktop-application`, the `repo` check reads the composed catalogue
`files/share/app-info/xmls/<ID>.xml.gz`:
- no `screenshots/screenshot/image` in it → `appstream-missing-screenshots`;
- an image URL that does not start with `https://dl.flathub.org/media` →
  `appstream-external-screenshot-url`;
- no `screenshots/<arch>` ref in the repository → `appstream-screenshots-not-mirrored-in-ostree`;
- no `.png` file in that ref → `appstream-screenshots-files-not-found-in-ostree`.

The mirror checks are skipped only for a `test` branch inside Flathub's own pipeline
(`should_skip_mirror_check`). Flathub's linter page (docs.flathub.org/docs/for-app-authors/linter)
says of all four: "This exception is never granted". For an externally built app it names the
flow: build "with `--compose-url-policy=full --mirror-screenshots-url=https://dl.flathub.org/media`",
then commit the media as `screenshots/<arch>`, which "is automated since Flatpak builder 1.4.5".
So these checks DO apply to a local or CI build, and an exception would only hide the defect.

## D1. Compose drops the WebP screenshots: install the WebP loader

**Measured** in `ubuntu:24.04` (AppStream 1.0.2) on the manifest's own install lines:

| host | `--mirror-screenshots-url` | catalogue `<screenshots>` | hint |
|---|---|---|---|
| CI packages of 21505ad2 | no | none | `screenshot-save-error` ×4, "Unrecognized image file format" |
| CI packages of 21505ad2 | yes | none | the same |
| + `webp-pixbuf-loader` | yes | 4 screenshots, 13 PNGs under `media/` | none |

Compose downloads every screenshot, with or without the mirror, and loads it through gdk-pixbuf to
size it. Ubuntu's gdk-pixbuf has PNG, JPEG and (with `librsvg2-common`) SVG loaders. WebP is the
separate package `webp-pixbuf-loader`, which nothing in the install pulls. The listing keeps its
WebP files. AppStream allows them ("using JPEG, WebP or JXL is also permitted for screenshot
images", AppStream metadata spec), compose stores PNGs anyway (`image-1_orig.png`,
`image-1_1248x702@1.png`, …), and the URLs are pinned to the tag v26.10.26. That tag is public:
each URL answers 200 `image/webp`. `webp-pixbuf-loader` is a row of `RECOMMENDS_NEEDED`, next to
the SVG loader.

## D2. A flatpak-builder that mirrors with full URLs: 1.4.6+ from ppa:flatpak/stable

flatpak-builder 1.4.2 (Ubuntu 24.04) can take `--mirror-screenshots-url`. It then passes
`--media-baseurl`/`--media-dir`, but compose writes partial URLs (`com/kinetren/Kinetren/<hash>/
screenshots/image-1_orig.png`). The linter refuses those (`appstream-external-screenshot-url`).
Full URLs need compose's `--no-partial-urls`, which flatpak-builder passes for
`--compose-url-policy=full`. That option is absent from 1.4.2 and 1.4.5 and present from 1.4.6
(checked in `src/builder-main.c` of each tag). 1.4.5's NEWS adds "Support committing mirrored
media as screenshot ref to exported repo" (`commit_screenshot_ref`: it runs when the mirror URL and
`--repo` are given).

**Decision.** CI and the release add `ppa:flatpak/stable`, which flatpak.org's Ubuntu setup page
names and the Flatpak maintainers publish. On 2026-11-03 it ships flatpak-builder 1.4.8 and
Flatpak 1.18.4 for noble. `flatpak-build.sh` builds with Flathub's own arguments, those of
`flathub-build` in `org.flatpak.Builder`: `--mirror-screenshots-url=https://dl.flathub.org/media
--compose-url-policy=full`. It refuses an older flatpak-builder before anything else, naming the
PPA. A 1.4.2 would otherwise build for ten minutes and fail in the linter.

**Rejected.**
- *Build with the Builder flatpak* (`flatpak run --command=flathub-build org.flatpak.Builder`,
  the Flathub submission page's local flow). It is the same toolchain as Flathub. But it runs
  flatpak-builder, and the build's bwrap, inside a sandbox, and its wrapper asks `flatpak-spawn
  --host` over a session bus that the runner does not have. Nested bwrap under the runner's AppArmor
  user-namespace restriction cannot be reproduced on this machine. It also hard-codes `repo` and
  `builddir` in the working directory. The host build is the path that already passes on the
  runner. The PPA changes only its version.
- *Commit `screenshots/x86_64` by hand with `ostree commit`* after a 1.4.2 build. That gives the
  ref, but the catalogue's URLs stay partial, so the linter still refuses.
- *PNG screenshots in the metainfo.* They would need new files at a new release tag (the URLs are
  pinned to a tag, so they do not move), and the cause would stay: any WebP added later would be
  dropped the same silent way. The pre-flight catches it now.
- *A linter exception.* The docs say these are "never granted", and an exception would let a
  listing without screenshots reach Flathub.

## D3. The pre-flight checks the screenshots

`appstream-compose-check.sh` composes with the mirror arguments that flatpak-builder 1.4.8 passes
(`--no-partial-urls`, `--media-baseurl`, `--media-dir`), because compose said `Success!` while
dropping the screenshots. It then compares: the number of `<screenshot>` in the metainfo against
the catalogue's, and every catalogue image is under `https://dl.flathub.org/media/`, is a `.png`,
and exists in the media folder. The media folder is what the build commits as
`screenshots/<arch>`, and these are the linter's four checks above. On failure it prints the hints
and names the WebP loader. Measured in `ubuntu:24.04`: with the packages of 21505ad2 it fails in
1.5 s ("0 of 4 screenshots reached the catalogue", the four `screenshot-save-error` hints). With
`webp-pixbuf-loader` it passes in 1.2 s.

Compose fetches the screenshots, so the pre-flight needs the network, and so does the build. t1381
and t1411 say they did not run when compose is missing or the first screenshot is unreachable. The
CI job runs the script through `flatpak-build.sh` either way.

## Not done

- The Flathub submission itself, and its own pipeline (vorarbeiter, its builder container) are
  not exercised: the CI job is the local pre-submission flow.
