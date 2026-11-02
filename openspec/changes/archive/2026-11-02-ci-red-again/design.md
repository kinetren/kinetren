# Design: ci-red-again

## Context

`ci-green-again` installed the Recommends the checks need and the jobs of 9e1f1a45 went further
than any run before: the `test` and `ffmpeg9` jobs reached t935's Kotlin assertion with a real
`mimetype`, and the `flatpak` job reached `appstreamcli compose`. Both steps failed. The owner's
desktop (Linux Mint, Cinnamon) passes both: its xdg-mime asks GIO, and it has the full packages.

## D1. t935 asks GIO, not xdg-mime

**What xdg-mime does.** `xdg-mime query filetype` picks a program by the desktop it detects:
`gio info` on GNOME, Cinnamon, MATE, Xfce and LXDE, `kmimetypefinder` on KDE, and perl's `mimetype`
(File::MimeInfo::Magic) otherwise, on a runner, in a container, and under i3, sway or another bare
window manager. The perl module reads the magic rules of priority ≥ 80 first, then the globs, then
the rest of the magic. For an extension it keeps only the FIRST type `globs2` lists
(`$extension{$1} = $string unless exists $extension{$1}`). It never weighs two equal globs
against the content.

**Measured** (our MIME file + the system's `freedesktop.org.xml` in a temporary
`XDG_DATA_HOME`; `film.kt` written by `project_file::to_text`, `Main.kt` a Kotlin source):

| system | shared-mime-info | `globs2` order of `*.kt` | GIO `Main.kt` / `film.kt` | Qt | perl `Main.kt` / `film.kt` |
|---|---|---|---|---|---|
| Ubuntu 22.04 (build container) | 2.1 | ours, Kotlin | Kotlin / ours | — | **ours** / ours |
| Ubuntu 24.04 (runner, this machine) | 2.4 | ours, Kotlin | Kotlin / ours | Qt 5 (kde-cli-tools 5.27): Kotlin / ours | **ours** / ours |
| Fedora 41 | 2.3 | Kotlin, ours | Kotlin / ours | Qt 6 (kde-cli-tools 6.4): Kotlin / ours | Kotlin / ours |

GIO is 2.72, 2.80 and 2.82 there. So the CI failure is real but belongs to the perl module: it is
not the shared-mime-info spec, which says to sniff when globs conflict. The order of equal-weight
lines differs between shared-mime-info versions, so perl's answer about Kotlin is a coin toss
between machines. Our project is named right by every program, perl included, because its magic
rule has priority 80.

**Decision.** t935 asks GIO (`gio info -a standard::content-type`, `GIO_USE_VFS=local`): Nautilus,
Nemo, Caja and Thunar ask it, and so does xdg-mime on most desktops. It must name all three files.
Where `mimetype` exists, t935 also asks it about `film.kt` and `demo.ktproj`: that is what `xdg-open`
on a bare window manager opens, and it must stay ours. `Main.kt` is not asked of perl. CI's test
job and the build container's base stage install `libglib2.0-bin`; a machine without `gio` skips
t935 and names the package. `dist-check.py` holds the package (`RECOMMENDS_NEEDED`, t1380).

**The MIME file stays.** Rejected for now: a lower weight for our glob (`weight="40"`). It was
measured too. Against the worry that GIO then returns the single heavier glob without sniffing,
GIO 2.72/2.80/2.82, Qt 5 and Qt 6 all still name `film.kt` ours, through the priority-80 magic. It
would also fix perl's answer for Kotlin. Its cost is every guess made from the NAME alone: GIO's
`standard::fast-content-type`, and gvfs listings of network shares, which do not read the file.
These would call every project Kotlin on every system. Today they follow the `globs2` order: ours
on Ubuntu, Kotlin on Fedora. That trades a Kotlin file opening in Kinetren under a bare window
manager for our projects showing as Kotlin on a NAS. It is a user-visible choice, so it waits for
the owner. The MIME file's comment is corrected: it said "shared-mime-info sniffs", and the
program that does not sniff is the one CI asked.

## D2. Compose needs the SVG loader

flatpak-builder 1.4.2 runs `appstreamcli compose` on the HOST after the build:

```
appstreamcli compose --prefix=/ --origin=<ID> --result-root=<files> --data-dir=<files>/share/app-info/xmls \
  --icons-dir=<files>/share/app-info/icons/flatpak --components=<ID>,<ID>.desktop <files>
```

(its strings; `--media-*` only with a screenshot mirror). Reproduced in `ubuntu:24.04` with
`flatpak-builder appstream appstream-compose elfutils`, installed the runner's way: the same two
errors. Compose's hints file names the cause: `file-read-error` "Unable to read data from file
//share/icons/hicolor/scalable/apps/com.kinetren.Kinetren.svg: Unrecognized image file format".
Compose renders the SVG through gdk-pixbuf, whose SVG loader `libpixbufloader-svg.so` ships in
`librsvg2-common`, a Recommends of `librsvg2-2`. With it, compose exits 0 and writes the catalog
and the 48/64/128 px cached icons. `filters-but-no-output` is only the consequence: the one
component was dropped.

The fix is the `RECOMMENDS_NEEDED` row `flatpak-builder` → `librsvg2-common` in `ci.yml` and
`release.yml`. The icon stays an SVG: Flathub requires a scalable or ≥ 128 px icon and has its own
compose. What remains are four warnings, `screenshot-save-error` "Unrecognized image file format"
for the `.webp` screenshots: gdk-pixbuf has no WebP loader there (`webp-pixbuf-loader`). They do not
fail the run, and flatpak-builder sets no media folder, so nothing is stored. The webp loader is
not added.

## D3. The compose runs before the build

The flatpak job learnt of the missing loader only after ten minutes of `cargo build --profile
dist`. `src/scripts/appstream-compose-check.sh` installs the manifest's own `install … /app/share/…`
lines into a temporary folder: it reads them from the manifest, so there is no second list to
drift. It then runs compose with flatpak-builder's arguments and prints compose's hints when it
fails. `flatpak-build.sh` runs it first. Without `appstreamcli compose` it exits 0 with a note,
because flatpak-builder would fail on it anyway and a laptop without appstream-compose should still
build. t1381 runs the script on the tree and on a copy with a broken icon where compose exists, and
skips with the package's name elsewhere. This machine lacks `appstream-compose`, so it ran in the
`ubuntu:24.04` container. `dist-check.py` requires the call in `flatpak-build.sh` (t1380).

## Risks

- The next CI run is the verdict: GitHub CI cannot be run from here. The steps after compose
  (`flatpak-builder-lint`, `appstreamcli validate --strict`, `flatpak-check.sh`) have never run on
  the runner. If one of them is red, it is a new item, not this change's regression.
- The base stage of the Dockerfile changes, so the cached container images rebuild once.
