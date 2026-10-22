#!/usr/bin/env python3
"""Live scenario R.65: DUCKING — a sidechain compressor set up by hand.

The owner’s decision (R.63, confirmed by R.65): ducking stopped being a snapshot of a
curve and became a live instrument on the track; the form is two compressors, the key
is chosen in the audio track window.

The scenario does this by hand in a real window:

1. opens the window of the MUSIC track (FX badge in its header);
2. selects the KEY track in the ducking section’s combo box;
3. presses Ctrl+S and checks that the key landed in the file (and the envelope version is v12);
4. LISTENS to the result: renders the saved project with `kt-cli render` to a wav and
   measures the music level under speech and in a pause — “a measurement by the output, not by
   the picture”.
"""
import json
import os
import struct
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401
from kt_live import _clip, _gap  # noqa: E402

SEC = 90000


def rms(frames, a, b):
    """RMS of a piece (in frames) from the unpacked mono list."""
    sl = frames[a:b]
    if not sl:
        return 0.0
    return (sum(v * v for v in sl) / len(sl)) ** 0.5


def read_wav_mono(path):
    """Left channel of the WAV. `kt-cli render` writes float32 (format 3), which the
    `wave` module refuses to read — so we parse the chunks ourselves."""
    data = open(path, "rb").read()
    assert data[:4] == b"RIFF" and data[8:12] == b"WAVE", "this is not a WAV"
    pos, fmt, ch, bits, body = 12, None, 2, 32, None
    while pos + 8 <= len(data):
        cid, size = data[pos : pos + 4], struct.unpack("<I", data[pos + 4 : pos + 8])[0]
        chunk = data[pos + 8 : pos + 8 + size]
        if cid == b"fmt ":
            fmt, ch = struct.unpack("<HH", chunk[:4])
            bits = struct.unpack("<H", chunk[14:16])[0]
        elif cid == b"data":
            body = chunk
        pos += 8 + size + (size & 1)
    assert body is not None, "the WAV has no data"
    if fmt == 3 and bits == 32:
        vals = struct.unpack("<%df" % (len(body) // 4), body)
    elif fmt == 1 and bits == 16:
        vals = [v / 32768.0 for v in struct.unpack("<%dh" % (len(body) // 2), body)]
    else:
        raise AssertionError(f"unexpected WAV format: fmt={fmt} bits={bits}")
    return [vals[i] for i in range(0, len(vals), ch)]


def drop_ducker(doc):
    """Remove the INSTRUMENT from the tracks entirely — the reference of “how it would sound without it”.

    Taking off one key is not enough: a compressor without a key compresses ITS OWN input, and on
    speech both runs happen to coincide (the scenario’s first run lied in just this way:
    “there is no dip”).
    """
    for t in doc["tracks"]["children"]:
        if t.get("kind") == "Audio":
            t["effects"] = []


def key_of(path):
    """Sidechain key from the project file (or None)."""
    with open(path) as f:
        doc = json.load(f)
    for t in doc["tracks"]["children"]:
        for e in t.get("effects", []):
            kt = e.get("metadata", {}).get("kt", {})
            for v in kt.values() if isinstance(kt, dict) else []:
                if isinstance(v, dict) and "key" in v:
                    return v["key"]
        kt = t.get("metadata", {}).get("kt", {})
        if isinstance(kt, dict) and "key" in json.dumps(kt):
            return json.dumps(kt)
    return None


def main(out):
    require_tools()
    # MUSIC — an even tone on track 0, SPEECH — two lines on track 1.
    # By hand, not `project_with_clips`: that needs VIDEO tracks, while
    # here the whole point is in two AUDIO tracks.
    path = os.path.join(out, "sidechain.otio")
    new_otio(path)
    with open(path) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["settings"].update(fps=30, width=1280, height=720)
    doc["metadata"]["kt"]["bin_media"] = []

    def atrack(children):
        return {
            "OTIO_SCHEMA": "Track.1",
            "name": "",
            "kind": "Audio",
            "children": children,
            "source_range": None,
            "effects": [],
            "markers": [],
            "enabled": True,
            "metadata": {},
        }

    music = atrack([_clip(1, 6.0, "synth:sine:220", 1)])
    speech = atrack(
        [
            _gap(1.0),
            _clip(2, 1.0, "synth:sine:700", 2),
            _gap(2.0),
            _clip(3, 1.0, "synth:sine:700", 2),
        ]
    )
    doc["tracks"]["children"] = [
        t for t in doc["tracks"]["children"] if t.get("kind") != "Audio"
    ] + [music, speech]
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)

    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("sidechain", out, project=path) as app:
        g = app.geom()
        rows = [r for r in g["rows"] if r["kind"] == "audio"]
        app.expect(len(rows) >= 2, f"two audio tracks in the window: {len(rows)}")
        if len(rows) < 2:
            return
        # 1) the MUSIC track window — by the FX badge in its header (by the same
        # body as the menu item: rule 17)
        app.click_probe("track.a1.fx", "FX badge of the music track")
        app.expect(app.has_probe("duck.key"), "the track window has a key choice")
        app.shot("track-window")

        # 2) we select the key: a click on the combo box and on the first item under it
        from kt_live import geometry as geo

        f = next(x for x in app.geom()["fields"] if x["param"] == "duck.key")
        w0 = geo()
        cx = w0["x"] + int(f["x"] + f["w"] / 2)
        cy = w0["y"] + int(f["y"] + f["h"] / 2)
        app.click(cx, cy)
        app.shot("key-popup")
        # the popup items go down from the combo box: “none”, then the tracks
        app.click(cx, cy + int(f["h"] * 2.2))
        app.shot("key-picked")

        # 3) we save and look into the file
        app.save_now()
        # R.70: the saved project is .kt; the kt envelope is visible through the export door
        saved = open(app.project_now().otio_path).read()
        app.expect('"key"' in saved, "sidechain key written to the file")
        app.expect('"v": 12' in saved or '"v":12' in saved, "envelope version rose to v12")

    # 4) LISTEN: we render the same file — and the SAME file WITHOUT THE DEVICE. The speech in
    # both is one and the same, so the whole difference under it is the music. One run
    # cannot be measured: the speech in the mix is louder than the music and masks the dip.
    def render(src, dst):
        # kt-cli renders the PROJECT file; the exchange file is first imported
        subprocess.run([KT_CLI, "render", import_project(src), dst], check=True, capture_output=True)
        return read_wav_mono(dst)

    keyed = render(app.project, os.path.join(out, "keyed.wav"))
    plain_path = os.path.join(out, "plain.otio")
    with open(app.project_now().otio_path) as fh:
        doc = json.load(fh)
    drop_ducker(doc)
    with open(plain_path, "w") as fh:
        json.dump(doc, fh, indent=4)
    plain = render(plain_path, os.path.join(out, "plain.wav"))

    sr = 48000
    under_k = rms(keyed, int(1.4 * sr), int(1.9 * sr))
    under_p = rms(plain, int(1.4 * sr), int(1.9 * sr))
    pause_k = rms(keyed, int(2.8 * sr), int(3.6 * sr))
    pause_p = rms(plain, int(2.8 * sr), int(3.6 * sr))
    print(f"   under speech: with the ducker {under_k:.4f}, without the instrument {under_p:.4f}")
    print(f"   in a pause:   with the ducker {pause_k:.4f}, without the instrument {pause_p:.4f}")
    bad = []
    if not under_k < under_p * 0.95:
        bad.append("under speech the music did not dip")
    if abs(pause_k - pause_p) > pause_p * 0.05:
        bad.append("in a pause the instrument must be transparent (sound as without it)")
    for line in bad:
        print(f"   FAILURE {line}")
    if bad:
        raise SystemExit(1)
    print("   ok   the music dips under speech and in a pause sounds as it was")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out")
