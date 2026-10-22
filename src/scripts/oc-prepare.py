#!/usr/bin/env python3
"""Preparation of a REAL film for the cross-render (test 179).

Input  — .otio from fixtures/otio-oc and the source lying next to it.
Output — <out>/prepared.otio (the media reference is absolute), <out>/prepared.mlt
        (for melt) and <out>/manifest.json with the frame rate, duration and
        the positions of the joins.

Difference from otio-cross-prepare.py: there the media is synthetic, here the media is
real — it must not be touched, one may only rewrite the reference and, if needed, cut off the tail
of the montage (so that the test does not encode three minutes of
1080p just to check the joins).
"""
import json
import os
import sys

import opentimelineio as otio


def main(src, outdir, window_s):
    os.makedirs(outdir, exist_ok=True)
    tl = otio.adapters.read_from_file(src)
    base = os.path.dirname(os.path.abspath(src))

    rate = None
    cuts, kept = [], 0
    for track in tl.tracks:
        cursor = 0.0
        keep = []
        for item in list(track):
            dur = item.duration().to_seconds()
            if rate is None and isinstance(item, otio.schema.Clip):
                rate = item.duration().rate
            # window: we take whole clips until we have collected window_s
            if window_s and cursor >= window_s:
                break
            keep.append(item)
            cursor += dur
            cuts.append(round(cursor, 6))
            kept += 1
        del track[len(keep):]
        # media references are absolute: prepared.otio lives in a temporary
        # directory, while the source stays in its own place
        for item in track:
            mr = getattr(item, "media_reference", None)
            url = getattr(mr, "target_url", None)
            if url and not os.path.isabs(url):
                mr.target_url = os.path.join(base, url)

    # WE get the file AS IS — with the material's start timecode baked in
    # (the film's available_range starts at an hour). This is part of the check:
    # the reader must subtract the timecode itself (item 180).
    prepared = os.path.join(outdir, "prepared.otio")
    otio.adapters.write_to_file(tl, prepared)

    # But the MLT adapter cannot handle the timecode: it writes available_range
    # straight into the producer in/out; for MLT this means “rewind the file to frame 86160,”
    # yet there are only 3890 frames in it — melt renders BLACK. So FOR IT we strip
    # the timecode to zero: we must compare renders, not other people's conventions.
    for track in tl.tracks:
        for item in track:
            mr = getattr(item, "media_reference", None)
            av = getattr(mr, "available_range", None)
            if av is None or item.source_range is None:
                continue
            off = av.start_time
            item.source_range = otio.opentime.TimeRange(
                start_time=item.source_range.start_time - off,
                duration=item.source_range.duration,
            )
            mr.available_range = otio.opentime.TimeRange(
                start_time=off - off, duration=av.duration
            )

    mlt, mlt_error = os.path.join(outdir, "prepared.mlt"), None
    try:
        otio.adapters.write_to_file(tl, mlt, adapter_name="mlt_xml")
    except Exception as e:  # the adapter may be absent — the test will say why
        mlt, mlt_error = None, f"{type(e).__name__}: {e}"

    json.dump(
        {
            "prepared": prepared,
            "mlt": mlt,
            "mlt_error": mlt_error,
            "rate": rate,
            "clips": kept,
            "duration_seconds": tl.duration().to_seconds(),
            "cuts": cuts,
        },
        open(os.path.join(outdir, "manifest.json"), "w"),
        indent=1,
    )


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], float(sys.argv[3]) if len(sys.argv) > 3 else 0.0)
