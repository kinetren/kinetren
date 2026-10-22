#!/usr/bin/env python3
"""Preparation of a foreign .otio for the cross-render (test 158, R.25).

Run with python from target/otio-venv (the reference OTIO library +
the mlt_xml adapter). Input — a sample from fixtures/otio-samples, output:

    <out>/media/mNN.mp4   synthetic: solid color from the palette + silence
    <out>/prepared.otio   the same montage, all references → to the synthetic media
    <out>/prepared.mlt    the same for melt (may be absent: see mlt_error)
    <out>/manifest.json   palette, transition windows, duration

Why solid colors: “what plays at moment t” turns into the pixel color,
and comparing two renders becomes a classification that is stable against the difference
of codecs. The palette is spread so that any single confusion of
BT.601/709 and limited/full range survives (see the classifier in cross_render.rs).

Usage:
    otio-cross-prepare.py <sample.otio> <out_dir> [--ffmpeg ffmpeg]
"""
import json
import math
import os
import subprocess
import sys

import opentimelineio as otio
from opentimelineio.opentime import RationalTime, TimeRange

# A synthetic frame. 720p: MLT's colorspace heuristic by frame height (< 720 —
# BT.601, otherwise BT.709), and we do not want it to fire unexpectedly.
W, H = 1280, 720
# Minimum distance between palette color variants (Euclidean in RGB).
# The classifier accepts a point at distance <= 32, so 96 > 2*32
# gives provable separability even when the convention is guessed wrongly.
MIN_DIST = 96.0
# Source length above which clips are rebased to zero (see below).
LONG_SOURCE_S = 60.0


def hsv_rgb(h, s, v):
    i = int(h * 6) % 6
    f = h * 6 - int(h * 6)
    p, q, t = v * (1 - s), v * (1 - f * s), v * (1 - (1 - f) * s)
    r, g, b = [(v, t, p), (q, v, p), (p, v, t), (p, q, v), (t, p, v), (v, p, q)][i]
    return (round(r * 255), round(g * 255), round(b * 255))


def yuv_variants(rgb):
    """All values into which the color can turn under ONE confusion of
    conventions: matrix (601/709) and range (limited/full) on encoding and
    on decoding — 16 combinations, fewer after dedup."""
    out = []
    for kr1, kb1 in ((0.299, 0.114), (0.2126, 0.0722)):
        for lim1 in (True, False):
            y, pb, pr = encode(rgb, kr1, kb1, lim1)
            for kr2, kb2 in ((0.299, 0.114), (0.2126, 0.0722)):
                for lim2 in (True, False):
                    v = decode(y, pb, pr, kr2, kb2, lim2)
                    if v not in out:
                        out.append(v)
    return out


def encode(rgb, kr, kb, limited):
    r, g, b = [c / 255.0 for c in rgb]
    kg = 1 - kr - kb
    y = kr * r + kg * g + kb * b
    pb = (b - y) / (2 * (1 - kb))
    pr = (r - y) / (2 * (1 - kr))
    if limited:
        return (
            clamp8(round(16 + 219 * y)),
            clamp8(round(128 + 224 * pb)),
            clamp8(round(128 + 224 * pr)),
        )
    return (clamp8(round(255 * y)), clamp8(round(128 + 255 * pb)), clamp8(round(128 + 255 * pr)))


def decode(yq, cbq, crq, kr, kb, limited):
    if limited:
        y = (yq - 16) / 219.0
        pb = (cbq - 128) / 224.0
        pr = (crq - 128) / 224.0
    else:
        y = yq / 255.0
        pb = (cbq - 128) / 255.0
        pr = (crq - 128) / 255.0
    kg = 1 - kr - kb
    r = y + 2 * (1 - kr) * pr
    b = y + 2 * (1 - kb) * pb
    g = (y - kr * r - kb * b) / kg
    return tuple(clamp8(round(c * 255)) for c in (r, g, b))


def clamp8(v):
    return max(0, min(255, int(v)))


def dist(a, b):
    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))


def build_palette(n):
    """Greedy selection of colors spread with a margin ACROSS ALL the convention
    variants (and away from black — it is taken by emptiness/letterbox/melt background)."""
    taken = []  # [(rgb, [variants])]
    black = [(0, 0, 0)]
    candidates = []
    for i in range(12):
        for s, v in ((1.0, 1.0), (1.0, 0.55), (0.45, 1.0)):
            candidates.append(hsv_rgb(i / 12.0, s, v))
    for rgb in candidates:
        vs = yuv_variants(rgb)
        ok = all(dist(v, k) >= MIN_DIST for v in vs for k in black)
        ok = ok and all(dist(v, u) >= MIN_DIST for v in vs for _, us in taken for u in us)
        if ok:
            taken.append((rgb, vs))
        if len(taken) >= n:
            break
    if len(taken) < n:
        sys.exit(
            f"the palette holds {len(taken)} colors, and the sources {n}: "
            "another sample is needed or a less strict MIN_DIST"
        )
    return [rgb for rgb, _ in taken]


def walk(comp, offset, out, transitions, rate_holder):
    """Recursive walk of the tracks/stacks: collects clips and transition windows."""
    prev_end = offset
    cursor = offset
    for item in comp:
        if isinstance(item, otio.schema.Transition):
            # transition window around the CURRENT junction
            cut = cursor
            transitions.append(
                (
                    to_sec(cut - item.in_offset.value / rate_holder[0]),
                    to_sec(cut + item.out_offset.value / rate_holder[0]),
                )
            )
            continue
        dur = 0.0
        try:
            dur = item.duration().value / item.duration().rate
        except Exception:
            pass
        if isinstance(item, (otio.schema.Track, otio.schema.Stack)):
            walk(item, cursor, out, transitions, rate_holder)
        elif isinstance(item, otio.schema.Clip):
            out.append((item, cursor, dur))
        cursor += dur
        prev_end = cursor
    return prev_end


def to_sec(x):
    return float(x)


def media_key(clip):
    """Identity of the source: URL, and without it the clip name (EDL semantics:
    same-named events refer to one reel)."""
    mr = clip.media_reference
    for attr in ("target_url", "target_url_base", "generator_kind"):
        v = getattr(mr, attr, None) if mr is not None else None
        if v:
            return str(v)
    return f"name:{clip.name or id(clip)}"


def read_timeline(src):
    """Reads the sample with the reference library. Upstream files are sometimes newer than the library
    release (Marker.3 in premiere_example/screening_example — the very same samples!), and then
    it refuses to read the file whole. Markers do not affect the picture, so for the render we read
    a copy without them and
    note this in the manifest."""
    try:
        return otio.adapters.read_from_file(src), False
    except Exception:
        with open(src) as f:
            doc = json.load(f)
        strip_markers(doc)
        return otio.adapters.read_from_string(json.dumps(doc)), True


def strip_markers(node):
    if isinstance(node, dict):
        if isinstance(node.get("markers"), list):
            node["markers"] = []
        for v in node.values():
            strip_markers(v)
    elif isinstance(node, list):
        for v in node:
            strip_markers(v)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    ffmpeg = "ffmpeg"
    for i, a in enumerate(sys.argv):
        if a == "--ffmpeg" and i + 1 < len(sys.argv):
            ffmpeg = sys.argv[i + 1]
    if len(args) != 2:
        sys.exit(__doc__)
    src, outdir = args
    os.makedirs(os.path.join(outdir, "media"), exist_ok=True)

    tl, stripped = read_timeline(src)
    rate = 24.0
    try:
        rate = tl.duration().rate
    except Exception:
        pass
    clips, transitions = [], []
    rate_holder = [rate]
    # WARNING: the reference tl.duration() also sums Transitions, although they take
    # no time on the track (transition_test has 170 frames instead of 150).
    # We compute the edit length ourselves — from the ends of the real items.
    timeline_end = 0.0
    for track in tl.tracks:
        end = walk(track, 0.0, clips, transitions, rate_holder)
        timeline_end = max(timeline_end, end)
    if not clips:
        sys.exit(f"{src}: no clips found")

    # unique sources and the required length of each
    need = {}
    for clip, _start, _dur in clips:
        k = media_key(clip)
        try:
            sr = clip.source_range
            end = (sr.start_time.value + sr.duration.value) / sr.start_time.rate
        except Exception:
            end = 0.0
        av = getattr(clip.media_reference, "available_range", None)
        if av is not None:
            end = max(end, (av.start_time.value + av.duration.value) / av.start_time.rate)
        need[k] = max(need.get(k, 0.0), end)

    # EDL edits take pieces from the 60th minute of the reel (screening_example:
    # start_time 86438 frames) — there is no point generating an hour of solid color:
    # for single-color material the entry point into the picture makes no difference, so
    # such clips are rebased to zero. It is noted in the manifest.
    rebased = 0
    for clip, _start, _dur in clips:
        k = media_key(clip)
        if need.get(k, 0.0) <= LONG_SOURCE_S or clip.source_range is None:
            continue
        sr = clip.source_range
        clip.source_range = TimeRange(RationalTime(0, sr.start_time.rate), sr.duration)
        rebased += 1
    if rebased:
        need = {}
        for clip, _start, _dur in clips:
            k = media_key(clip)
            sr = clip.source_range
            end = (sr.start_time.value + sr.duration.value) / sr.start_time.rate if sr else 0.0
            need[k] = max(need.get(k, 0.0), end)

    keys = sorted(need)
    palette = build_palette(len(keys))
    media = []
    for idx, k in enumerate(keys):
        rgb = palette[idx]
        path = os.path.join(outdir, "media", f"m{idx:02d}.mp4")
        dur = max(need[k], 1.0) + 1.0
        if not os.path.exists(path):
            gen(ffmpeg, path, rgb, dur, rate)
        media.append({"key": k, "path": path, "rgb": list(rgb)})

    # replacement of ALL references to the synthetic media; a clip without source_range gets it set
    # from available_range (otherwise the MLT adapter does not know what to cut)
    by_key = {m["key"]: m for m in media}
    for clip, _start, _dur in clips:
        m = by_key[media_key(clip)]
        av_dur = need[m["key"]] + 1.0
        if clip.source_range is None:
            # the clip takes the whole material: the length comes from ITS ORIGINAL
            # available_range, not from the length of our synthetic media (otherwise the edit
            # would stretch and there would be nothing to compare)
            orig = getattr(clip.media_reference, "available_range", None)
            clip.source_range = (
                orig
                if orig is not None
                else TimeRange(RationalTime(0, rate), RationalTime(round(rate), rate))
            )
        clip.media_reference = otio.schema.ExternalReference(
            target_url=os.path.abspath(m["path"]),
            available_range=TimeRange(RationalTime(0, rate), RationalTime(round(av_dur * rate), rate)),
        )

    prepared = os.path.join(outdir, "prepared.otio")
    otio.adapters.write_to_file(tl, prepared)

    mlt_path, mlt_error = os.path.join(outdir, "prepared.mlt"), None
    try:
        otio.adapters.write_to_file(
            tl,
            mlt_path,
            adapter_name="mlt_xml",
            width=W,
            height=H,
            frame_rate_num=int(round(rate)),
            frame_rate_den=1,
        )
    except Exception as e:  # the adapter does not support everything — that is a valid outcome
        mlt_error = f"{type(e).__name__}: {e}"
        mlt_path = None

    manifest = {
        "sample": os.path.basename(src),
        "stripped_markers": stripped,
        "rebased_clips": rebased,
        "rate": rate,
        "width": W,
        "height": H,
        "duration_seconds": timeline_end,
        "media": media,
        "transitions": [{"start_s": a, "end_s": b} for a, b in transitions],
        "mlt": mlt_path,
        "mlt_error": mlt_error,
        "prepared": prepared,
    }
    with open(os.path.join(outdir, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2)
    print(json.dumps({"media": len(media), "transitions": len(transitions), "mlt_error": mlt_error}))


def gen(ffmpeg, path, rgb, dur, rate):
    color = "0x%02X%02X%02X" % rgb
    tmp = path + ".tmp.mp4"
    subprocess.run(
        [
            ffmpeg, "-v", "error", "-y",
            "-f", "lavfi", "-i", f"color=c={color}:size={W}x{H}:rate={rate}:duration={dur:.3f}",
            "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo",
            "-shortest",
            "-vf", "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p",
            "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
            "-color_range", "tv",
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "15", "-g", "30",
            "-c:a", "aac", "-b:a", "96k",
            tmp,
        ],
        check=True,
    )
    os.replace(tmp, path)


if __name__ == "__main__":
    main()
