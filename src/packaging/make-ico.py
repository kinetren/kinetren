#!/usr/bin/env python3
"""kinetren.ico from the committed PNG sizes (change windows-build).

An ICO may hold PNG images directly (Windows Vista and later), so no image library is
needed: the PNGs of icons/<size>/kinetren.png are packed as they are. Run after changing
the icon; the .ico is committed and compiled into kinetren.exe by kt-app's build.rs.

    python3 packaging/make-ico.py
"""
import pathlib
import struct

HERE = pathlib.Path(__file__).resolve().parent
SIZES = [16, 24, 32, 48, 64, 128, 256]

images = [(s, (HERE / "icons" / str(s) / "kinetren.png").read_bytes()) for s in SIZES]
header = struct.pack("<HHH", 0, 1, len(images))
offset = len(header) + 16 * len(images)
entries, data = b"", b""
for size, png in images:
    dim = 0 if size >= 256 else size  # 0 means 256
    entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(png), offset + len(data))
    data += png
(HERE / "kinetren.ico").write_bytes(header + entries + data)
print(f"kinetren.ico: {len(images)} sizes, {len(header + entries + data)} bytes")
