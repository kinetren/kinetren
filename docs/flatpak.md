# Kinetren as a Flatpak

Kinetren on Flathub has the ID `com.kinetren.Kinetren`. It uses the FFmpeg of the Freedesktop runtime
(with its `codecs-extra` extension: H.264 and AAC encoders), so nothing has to be installed on the host
for opening media and exporting. This page is about the GPU: what works by itself, what to check, and how
to give the sandbox more access when a hardware encoder does not open.

> The per-vendor results below are the expected behaviour of the runtime and its GL/VA extensions; the
> measured results on real Intel, AMD and NVIDIA machines are recorded in the change `flatpak-flathub`
> (tasks 4.1) and this page is corrected from them.

## Check first

Inside the sandbox, ask Kinetren what it found:

```bash
flatpak run --command=kt-cli com.kinetren.Kinetren ffmpeg --probe-encoders
```

The output names the FFmpeg that loaded, each render node (`/dev/dri/renderD128` …) with whether it opens
as a VAAPI device and its driver, and each hardware encoder: `opens`, or the part that fails. During a
render the progress window's **Video** row names the encoder that opened and, for VAAPI, its device.

## Intel and AMD (VAAPI)

The manifest grants `--device=dri`, and the runtime carries Mesa (AMD: `radeonsi`) and the Intel media
driver extension (`iHD`). `h264_vaapi` should open without any action. If the report shows a node that
does not open, or the wrong driver was picked, name the driver:

```bash
flatpak override --user --env=LIBVA_DRIVER_NAME=iHD com.kinetren.Kinetren
```

(`radeonsi` for AMD, `iHD` for Intel since Broadwell.) The old `intel-vaapi-driver` (`i965`) is no longer
in the runtime.

## NVIDIA (NVENC)

NVENC needs the Flatpak GL extension that matches the host driver version
(`org.freedesktop.Platform.GL.nvidia-<version>`); `flatpak update` installs it when Flathub has one for
your driver. If `h264_nvenc` still does not open with `--device=dri` only, give the sandbox all device
nodes (NVENC needs `/dev/nvidia-uvm` and similar ones on some flatpak versions):

```bash
flatpak override --user --device=all com.kinetren.Kinetren
```

NVIDIA's VA-API driver in the runtime decodes only; it does not encode.

## Undo

Every override above is undone with:

```bash
flatpak override --user --reset com.kinetren.Kinetren
```

## Files and settings

Kinetren opens projects whose media live anywhere (USB disks, `/media`, network mounts), so it asks for
access to the host file system. Its settings live in the sandbox (`~/.var/app/com.kinetren.Kinetren/`);
a tarball install of Kinetren keeps its own settings, and the two show as two menu entries until one is
removed.

## For maintainers: the Flathub submission

1. Fork `flathub/flathub`, branch from `new-pr`.
2. Add `com.kinetren.Kinetren.yml` (from `src/packaging/flatpak/`, with the first source changed from the
   working tree to `type: git`, `url: https://github.com/kinetren/kinetren.git` and the release `tag`),
   `cargo-sources.json` of that tag (`make flatpak-sources`), and `flathub.json` with
   `{"only-arches": ["x86_64"]}`.
3. Open the PR titled "Add com.kinetren.Kinetren" and answer the review.
4. For the `com.kinetren` domain, put the verification token Flathub gives at
   `https://kinetren.com/.well-known/org.flathub.VerifiedApps.txt` (the site repository).
5. After the merge, every release is a PR to `flathub/com.kinetren.Kinetren` with the new tag, the
   regenerated `cargo-sources.json` and the `KT_VERSION_*` of the tag.

The listing shows the licence as proprietary (the Kinetren Non-Commercial License is not an OSI licence;
the owner confirmed that it allows Flathub to distribute the program).
