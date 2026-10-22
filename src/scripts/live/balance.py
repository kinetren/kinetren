#!/usr/bin/env python3
"""Distribution of live scenarios across workers BY COST, not by order (R.68).

Round-robin dealing (“the first to the first, the second to the second”) looks fair and
is not: the cost of scenarios differs thirty-fold (`nestparam` — 136 s,
`audiofx` — 4 s). The worker who got the longest one holds the whole
run while the others have already finished: with four workers the run cost
594 s instead of a possible ~300.

So dealing is greedy: “the most expensive — to the least busy” (LPT): the cost
is taken from `times.txt`, which the workers append to after each scenario,
and an unknown scenario is assigned the average (twenty seconds) — otherwise a new
scenario would always go to the end.

    balance.py <file of times> <number of workers> <scenario> [<scenario> ...]

Prints one line per worker: names separated by spaces.
"""
import sys

DEFAULT_COST = 20.0


def costs(path):
    out = {}
    try:
        with open(path, encoding="utf-8") as f:
            for line in f:
                name, _, secs = line.partition("\t")
                try:
                    # we take the WORST of what was seen: the distribution must hold up
                    # a slow run, not only a lucky one
                    out[name] = max(out.get(name, 0.0), float(secs))
                except ValueError:
                    continue
    except OSError:
        pass
    return out


def main(argv):
    path, workers, names = argv[0], int(argv[1]), argv[2:]
    cost = costs(path)
    load = [0.0] * workers
    buckets = [[] for _ in range(workers)]
    for name in sorted(names, key=lambda n: -cost.get(n, DEFAULT_COST)):
        # At EQUAL load we take the one with fewer scenarios: otherwise
        # the cheap scenarios (the stand-ins cost 0.0 s) all flock together to
        # worker number zero — so the isolation guard t802 saw one screen
        # instead of three.
        i = min(range(workers), key=lambda i: (load[i], len(buckets[i]), i))
        buckets[i].append(name)
        load[i] += cost.get(name, DEFAULT_COST)
    for b in buckets:
        print(" ".join(b))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
