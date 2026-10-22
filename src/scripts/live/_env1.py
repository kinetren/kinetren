#!/usr/bin/env python3
"""STUB scenario for t802 (copy 1): prints the screen and the storage of its worker.

Worker isolation is not “we wrote it so”, but an observed fact: each
has its own screen number and its own storage directory. The scenario writes them to a file,
the test checks that they differ (see `_flaky.py` about the name with an underscore).
"""
import json
import os
import sys

OWN_PROCESS = True


def main(out):
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "env.json"), "w") as f:
        json.dump(
            {
                "display": os.environ.get("DISPLAY"),
                "store": os.environ.get("KT_USER_STORE_DIR"),
                "bench": os.environ.get("KT_LIVE_BENCH"),
            },
            f,
        )
    print(f"   ok   worker: DISPLAY={os.environ.get('DISPLAY')}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
