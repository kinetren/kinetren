#!/usr/bin/env python3
"""Not a scenario: the live bench's channel paths, shared by `kt_live.py` and `worker.py` (the worker
cannot import `kt_live`, which opens the X display at import). The `_` keeps it out of the run."""
import hashlib
import os
import shutil


def channel_dir(owner, fresh=False):
    """Where the program's dump (`KT_GEOM_DUMP`) and the bench's command file live: a tmpfs folder
    per run, not the run's folder on the working disk (test-hygiene). A write and rename there
    stood 10–20 s while builds and the sidecars synced the disk, and the scenarios read a dump that
    many seconds old (`space_hold`: the head stood in the dump for 17 s of a playback that ran).
    `owner` names the run's folder; `fresh` empties the channel (a new program reads the last
    command it finds). `keep_channel` copies it beside the screenshots."""
    base = os.environ.get("XDG_RUNTIME_DIR") or "/dev/shm"
    d = os.path.join(base, "kinetren-live", hashlib.sha1(os.path.abspath(owner).encode()).hexdigest()[:16])
    if fresh:
        shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d, exist_ok=True)
    return d


def keep_channel(chan, out):
    """The dump and its companions, copied into the scenario's folder as its evidence."""
    try:
        for f in os.listdir(chan):
            if f.startswith("geom.json") and not f.endswith(".tmp"):
                shutil.copy2(os.path.join(chan, f), os.path.join(out, f))
    except OSError:
        pass

