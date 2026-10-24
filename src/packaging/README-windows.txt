Kinetren for Windows (x86_64) — portable build
==============================================

Unpack the folder anywhere and start kinetren.exe. Nothing is installed; your
settings live in %APPDATA%\kinetren\data.

FFmpeg
------
Kinetren decodes and exports through FFmpeg, which is NOT included in this zip.
On the first start without FFmpeg the program says so and offers:

  * Download  — fetches the FFmpeg 8.1.2 shared build by gyan.dev (GPLv3,
                about 60 MB) into %LOCALAPPDATA%\kinetren\ffmpeg; the archive is
                checked against a checksum built into Kinetren.
  * Locate folder… — point at a folder that holds avcodec-NN.dll and the other
                FFmpeg DLLs of one release.

You can also install a SHARED build yourself (FFmpeg 6, 7 or 8):

  winget install Gyan.FFmpeg.Shared --version 8.1.2
  scoop install main/ffmpeg-shared@8.1.2

The plain "Gyan.FFmpeg" winget package is a static build (ffmpeg.exe only, no
DLLs) and cannot be used. FFmpeg 9 is not supported yet.

View → FFmpeg… shows which folder was used and why other folders were not.

Windows SmartScreen
-------------------
The executables are not code-signed yet, so SmartScreen may warn on the first
start: "More info" → "Run anyway".

kt-cli.exe is the command-line renderer; `kt-cli ffmpeg` prints the FFmpeg
search report.
