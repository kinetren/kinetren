#!/usr/bin/env python3
"""Wake the orchestrator when an executor session stops.

    python3 src/scripts/executor-watch.py <worktree-name> [<worktree-name> ...]

Run in the background (it exits when it has something to say, and that exit wakes the session).
It reads the executors' transcripts and exits as soon as one has stayed silent for QUIET seconds
after an assistant message without a pending tool call: the app wakes an orchestrator only when
NONE of its sessions works, and an idle notice is one-shot, so neither sees a turn that ends while
another executor runs (2 h 18 min lost on 2026-11-04). The transcripts are read, nothing is sent.
"""
import glob
import json
import os
import sys
import time

QUIET = 180
POLL = 30


def last_stop(name):
    """(seconds since the transcript last moved, the last message is a finished turn)."""
    pattern = os.path.expanduser(f"~/.claude/projects/*{name}*/*.jsonl")
    files = sorted(glob.glob(pattern), key=os.path.getmtime)
    if not files:
        return None, False
    with open(files[-1], errors="replace") as f:
        lines = f.readlines()[-40:]
    stopped = False
    for line in reversed(lines):
        try:
            d = json.loads(line)
        except ValueError:
            continue
        if d.get("type") == "assistant":
            content = d.get("message", {}).get("content")
            stopped = not (isinstance(content, list)
                           and any(c.get("type") == "tool_use" for c in content if isinstance(c, dict)))
            break
        if d.get("type") == "user":
            break
    return time.time() - os.path.getmtime(files[-1]), stopped


def main():
    names = sys.argv[1:]
    if not names:
        raise SystemExit(__doc__)
    while True:
        for name in names:
            quiet, stopped = last_stop(name)
            if quiet is not None and stopped and quiet >= QUIET:
                print(f"{name}: stopped {quiet / 60:.0f} min ago")
                return 0
        time.sleep(POLL)


if __name__ == "__main__":
    sys.exit(main())
