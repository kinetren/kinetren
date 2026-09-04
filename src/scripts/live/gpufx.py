#!/usr/bin/env python3
"""Живой сценарий Р.29: видео-FX на GPU-пути реально применяются в превью.

Живой kt-app в Xvfb идёт по НАСТОЯЩЕМУ GPU-коду (софт-рендер llvmpipe —
тоже «GPU есть»). Два клипа одного светло-серого медиа: без эффектов и с
цепочкой Edges — на плоском цвете Собель гасит кадр в чёрный. Проверяется
то, что видит человек: превью без FX светлое, с FX — тёмное. Заодно ловит
«молчаливый CPU-фолбэк со сломанным нотисом»: цепочка обязана работать в
обоих случаях, а бэкенд печатается в лог сценария из stderr программы.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    media = color_media(out, "gray", "D0D0D0", seconds=10)
    path = os.path.join(out, "gpufx.otio")
    project_with_clips(path, [(0.0, 4.0, media), (4.0, 4.0, media)])
    add_edges_to_second_clip(path)

    with Live("gpufx", out, project=path) as app:
        preview = app.preview_center()
        app.shot("open")

        # клип без FX: плоский светлый кадр
        app.click(app.x_at(int(1.5 * SEC)), app.ruler_y())
        plain = app.brightness(preview)
        app.shot("plain")

        # клип с Edges: на плоском цвете градиентов нет → чёрный кадр
        app.click(app.x_at(int(5.5 * SEC)), app.ruler_y())
        fx = app.brightness(preview)
        app.shot("edges")

        print(f"   яркость превью: без FX {plain:.1f}, с Edges {fx:.1f}")
        app.expect(plain > 120.0, f"кадр без FX светлый: {plain:.1f}")
        app.expect(fx < plain * 0.2, f"Edges погасил плоский кадр: {fx:.1f} vs {plain:.1f}")
        return app.done()


def add_edges_to_second_clip(path):
    """Цепочка kt:edges на второй клип видео-дорожки (формат convert.rs)."""
    with open(path) as f:
        doc = json.load(f)
    track = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    clips = [c for c in track["children"] if c.get("OTIO_SCHEMA", "").startswith("Clip")]
    clips[1]["effects"] = [
        {
            "OTIO_SCHEMA": "Effect.1",
            "name": "",
            "effect_name": "kt:edges",
            "enabled": True,
            "metadata": {"kt": {"kind": "edges", "threshold": 48}},
        }
    ]
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
