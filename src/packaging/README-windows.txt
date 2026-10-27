Kinetren for Windows (x86_64) — portable build
==============================================

Unpack the folder anywhere and start kinetren.exe. Nothing is installed; your
settings live in %APPDATA%\kinetren\data.

FFmpeg
------
Kinetren decodes and exports through FFmpeg, which is NOT included in this zip.
On the first start without FFmpeg the program says so and offers:

  * Download  — fetches the FFmpeg 9.0.2 shared build by gyan.dev (GPLv3,
                about 60 MB) into %LOCALAPPDATA%\kinetren\ffmpeg; the archive is
                checked against a checksum built into Kinetren. A build an
                earlier Kinetren downloaded there keeps working.
  * Locate folder… — point at a folder that holds avcodec-NN.dll and the other
                FFmpeg DLLs of one release.

You can also install a SHARED build yourself (FFmpeg 6, 7, 8 or 9):

  winget install Gyan.FFmpeg.Shared --version 9.0.2
  scoop install main/ffmpeg-shared@9.0.2

The plain "Gyan.FFmpeg" winget package is a static build (ffmpeg.exe only, no
DLLs) and cannot be used.

View → FFmpeg… shows which folder was used and why other folders were not.

Running under wine
------------------
Under wine the window gets the system title bar of your window manager, with
Kinetren's own row (logo, File / Edit / View) below it: wine decorates every
window, and a borderless one would lose its top row under the frame. For the
same look as on Windows and Linux, turn wine's decorations off and ask for the
borderless window:

  wine reg add "HKCU\Software\Wine\X11 Driver" /v Decorated /t REG_SZ /d N
  KT_DECORATIONS=0 wine kinetren.exe

KT_DECORATIONS=1 asks for the system title bar on any system.

Windows SmartScreen
-------------------
The executables are not code-signed yet, so SmartScreen may warn on the first
start: "More info" → "Run anyway".

kt-cli.exe is the command-line renderer; `kt-cli ffmpeg` prints the FFmpeg
search report.
