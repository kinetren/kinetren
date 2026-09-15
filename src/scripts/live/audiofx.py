#!/usr/bin/env python3
"""Живой сценарий Р.47 (п.330): новые аудио-эффекты достижимы из окна FX
и переживают сохранение.

Демо-проект несёт на аудио-дорожке слот Filter (правка .otio нашей же
библиотекой: формат один). Двойной клик по событию открывает окно FX —
там полоса цепочки с «Filter» и сетка параметров (Cutoff/Resonance).
Проверка следствия: (1) окно открылось (экран изменился), (2) ctrl+s —
слот kt:filter с cutoff_hz пережил round-trip через настоящую программу
(UI-сейв не теряет и не искажает новые виды эффектов).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = os.path.join(out, "audiofx.otio")
    subprocess.run([KT_CLI, "demo", proj, "--force"], check=True, capture_output=True)
    # первому АУДИО-событию демо-проекта дописываем слот Filter с
    # неумолчальным cutoff — по нему узнаем свой слот после сейва
    with open(proj) as f:
        doc = json.load(f)
    audio_clip = None
    for tr in doc["tracks"]["children"]:
        if tr.get("kind") == "Audio":
            for ch in tr["children"]:
                if ch.get("OTIO_SCHEMA", "").startswith("Clip"):
                    audio_clip = ch
                    break
        if audio_clip:
            break
    assert audio_clip is not None, "demo project must have an audio clip"
    audio_clip.setdefault("effects", []).append({
        "OTIO_SCHEMA": "Effect.1",
        "name": "",
        "effect_name": "kt:filter",
        "enabled": True,
        "metadata": {
            "kt": {
                "kind": "filter",
                "mode": "lowpass",
                "cutoff_hz": 1234.0,
                "resonance": 2.5,
            }
        },
    })
    with open(proj, "w") as f:
        json.dump(doc, f, indent=4)

    with Live("audiofx", out, project=proj) as app:
        y = app.lane_y("audio", 0, frac=0.5)
        before = app.shot("open")
        x = app.x_at(3 * SEC)
        app.double_click(x, y)
        app.shot("fx-window")
        d = app.screen_diff(before)
        app.expect(d > 0.01, f"двойной клик открыл окно FX (экран изменился на {d:.1%})")

        # round-trip через настоящую программу: слот выжил и не исказился
        app.saved_project()  # ctrl+s — файл перезаписан программой
        with open(proj) as f:
            saved = json.load(f)
        slots = [
            e
            for tr in saved["tracks"]["children"]
            for ch in tr.get("children", [])
            for e in ch.get("effects", [])
            if e.get("effect_name") == "kt:filter"
        ]
        app.expect(bool(slots), "слот kt:filter пережил сохранение программой")
        if slots:
            kt = slots[0]["metadata"]["kt"]
            app.expect(
                abs(kt.get("cutoff_hz", 0) - 1234.0) < 1e-6,
                f"cutoff_hz сохранён без искажений: {kt.get('cutoff_hz')}",
            )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
