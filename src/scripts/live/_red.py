#!/usr/bin/env python3
"""STUB scenario for t801: red ALWAYS (see `_flaky.py`)."""
import os
import sys

OWN_PROCESS = True
# this scenario does not launch the program — it does not need an X server
NO_DISPLAY = True


def main(out):
    os.makedirs(out, exist_ok=True)
    # what a red try leaves: t2210 finds it in `first-try/` after the lone retry
    open(os.path.join(out, "01-step-red.png"), "w").close()
    print("   FAIL stub scenario red always")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
