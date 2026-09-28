#!/usr/bin/env python3
"""Раскладка живых сценариев по работникам ПО ЦЕНЕ, а не по порядку (Р.68).

Круговая раздача («первый — первому, второй — второму») выглядит честной и
не является ею: цена сценариев разнится в тридцать раз (`nestparam` — 136 с,
`audiofx` — 4 с). Работник, которому достался самый долгий, держит весь
прогон, пока остальные уже закончили: при четырёх работниках прогон стоил
594 с вместо возможных ~300.

Поэтому раздача жадная и «самый дорогой — наименее занятому» (LPT): цену
берём из `times.txt`, который работники дописывают после каждого сценария,
а незнакомому сценарию назначаем среднюю (двадцать секунд) — иначе новый
сценарий всегда уезжал бы в конец.

    balance.py <файл времён> <число работников> <сценарий> [<сценарий> ...]

Печатает по строке на работника: имена через пробел.
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
                    # берём ХУДШЕЕ из виденного: раскладка должна выдерживать
                    # медленный прогон, а не только удачный
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
        i = min(range(workers), key=lambda i: load[i])
        buckets[i].append(name)
        load[i] += cost.get(name, DEFAULT_COST)
    for b in buckets:
        print(" ".join(b))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
