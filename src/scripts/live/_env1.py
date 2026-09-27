#!/usr/bin/env python3
"""ПОДСТАВНОЙ сценарий для t802 (копия 1): печатает экран и хранилище своего работника.

Изоляция работников — это не «мы так написали», а наблюдаемый факт: у каждого
свой номер экрана и свой каталог хранилища. Сценарий записывает их в файл,
тест сверяет, что они различны (см. `_flaky.py` про имя с подчёркивания).
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
    print(f"   ok   работник: DISPLAY={os.environ.get('DISPLAY')}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
