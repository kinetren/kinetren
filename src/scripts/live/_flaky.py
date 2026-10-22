#!/usr/bin/env python3
"""STUB scenario for t801: red the first time, green the second.

A live run cannot prove “rerunning the red alone” on a real
scenario: a real one is either green or broken. A scenario with KNOWN
behavior is needed — and that is what lies here. Names with an underscore: `live-check.sh` does not take such into
the common run.

OWN_PROCESS is needed so that the worker does not start kt-app for its sake: the program
is not needed by this scenario at all.
"""
import os
import sys

OWN_PROCESS = True
# this scenario does not launch the program — it does not need an X server
NO_DISPLAY = True


def main(out):
    os.makedirs(out, exist_ok=True)
    mark = os.path.join(out, "already-ran")
    if os.path.exists(mark):
        print("   ok   stub scenario green the second time")
        return 0
    open(mark, "w").close()
    print("   FAIL stub scenario red the first time")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
