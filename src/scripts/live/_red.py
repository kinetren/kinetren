#!/usr/bin/env python3
"""ПОДСТАВНОЙ сценарий для t801: красный ВСЕГДА (см. `_flaky.py`)."""
import os
import sys

OWN_PROCESS = True


def main(out):
    os.makedirs(out, exist_ok=True)
    print("   ПРОВАЛ подставной сценарий красный всегда")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
