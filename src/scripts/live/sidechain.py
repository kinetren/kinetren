#!/usr/bin/env python3
"""Живой сценарий Р.65: ДАКИНГ — сайдчейн-компрессор, настроенный руками.

Решение владельца (Р.63, подтверждено Р.65): дакинг перестал быть снимком
кривой и стал живым прибором на дорожке; форма — два компрессора, ключ
выбирается в окне аудиодорожки.

Сценарий делает это руками в настоящем окне:

1. открывает окно дорожки МУЗЫКИ (бейдж FX в её заголовке);
2. выбирает дорожку-КЛЮЧ в комбо секции дакинга;
3. жмёт Ctrl+S и проверяет, что ключ лёг в файл (и версия конверта v12);
4. СЛУШАЕТ результат: рендерит сохранённый проект `kt-cli render` в wav и
   меряет уровень музыки под речью и в паузе — «замер по выходу, а не по
   картинке».
"""
import json
import os
import struct
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, require_tools  # noqa: E402
from kt_live import _clip, _gap  # noqa: E402

SEC = 90000


def rms(frames, a, b):
    """RMS куска (в кадрах) из распакованного моно-списка."""
    sl = frames[a:b]
    if not sl:
        return 0.0
    return (sum(v * v for v in sl) / len(sl)) ** 0.5


def read_wav_mono(path):
    """Левый канал WAV. `kt-cli render` пишет float32 (формат 3), который
    модуль `wave` читать отказывается, — разбираем чанки сами."""
    data = open(path, "rb").read()
    assert data[:4] == b"RIFF" and data[8:12] == b"WAVE", "это не WAV"
    pos, fmt, ch, bits, body = 12, None, 2, 32, None
    while pos + 8 <= len(data):
        cid, size = data[pos : pos + 4], struct.unpack("<I", data[pos + 4 : pos + 8])[0]
        chunk = data[pos + 8 : pos + 8 + size]
        if cid == b"fmt ":
            fmt, ch = struct.unpack("<HH", chunk[:4])
            bits = struct.unpack("<H", chunk[14:16])[0]
        elif cid == b"data":
            body = chunk
        pos += 8 + size + (size & 1)
    assert body is not None, "в WAV нет данных"
    if fmt == 3 and bits == 32:
        vals = struct.unpack("<%df" % (len(body) // 4), body)
    elif fmt == 1 and bits == 16:
        vals = [v / 32768.0 for v in struct.unpack("<%dh" % (len(body) // 2), body)]
    else:
        raise AssertionError(f"неожиданный формат WAV: fmt={fmt} bits={bits}")
    return [vals[i] for i in range(0, len(vals), ch)]


def drop_ducker(doc):
    """Убрать с дорожек ПРИБОР целиком — эталон «как звучало бы без него».

    Снимать один ключ мало: компрессор без ключа сжимает СВОЙ вход, и на
    речи оба прогона случайно совпадают (первый прогон сценария так и
    соврал «просадки нет»).
    """
    for t in doc["tracks"]["children"]:
        if t.get("kind") == "Audio":
            t["effects"] = []


def key_of(path):
    """Ключ сайдчейна из файла проекта (или None)."""
    with open(path) as f:
        doc = json.load(f)
    for t in doc["tracks"]["children"]:
        for e in t.get("effects", []):
            kt = e.get("metadata", {}).get("kt", {})
            for v in kt.values() if isinstance(kt, dict) else []:
                if isinstance(v, dict) and "key" in v:
                    return v["key"]
        kt = t.get("metadata", {}).get("kt", {})
        if isinstance(kt, dict) and "key" in json.dumps(kt):
            return json.dumps(kt)
    return None


def main(out):
    require_tools()
    # МУЗЫКА — ровный тон на дорожке 0, РЕЧЬ — две реплики на дорожке 1.
    # Своими руками, а не `project_with_clips`: тому нужны ВИДЕО-дорожки, а
    # тут вся суть в двух АУДИО.
    path = os.path.join(out, "sidechain.otio")
    subprocess.run([KT_CLI, "new", path, "--force"], check=True, capture_output=True)
    with open(path) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["settings"].update(fps=30, width=1280, height=720)
    doc["metadata"]["kt"]["bin_media"] = []

    def atrack(children):
        return {
            "OTIO_SCHEMA": "Track.1",
            "name": "",
            "kind": "Audio",
            "children": children,
            "source_range": None,
            "effects": [],
            "markers": [],
            "enabled": True,
            "metadata": {},
        }

    music = atrack([_clip(1, 6.0, "synth:sine:220", 1)])
    speech = atrack(
        [
            _gap(1.0),
            _clip(2, 1.0, "synth:sine:700", 2),
            _gap(2.0),
            _clip(3, 1.0, "synth:sine:700", 2),
        ]
    )
    doc["tracks"]["children"] = [
        t for t in doc["tracks"]["children"] if t.get("kind") != "Audio"
    ] + [music, speech]
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)

    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("sidechain", out, project=path) as app:
        g = app.geom()
        rows = [r for r in g["rows"] if r["kind"] == "audio"]
        app.expect(len(rows) >= 2, f"в окне две аудиодорожки: {len(rows)}")
        if len(rows) < 2:
            return
        # 1) окно дорожки МУЗЫКИ — бейджем FX в её заголовке (тем же
        # телом, что пункт меню: правило 17)
        app.click_probe("track.a1.fx", "бейдж FX дорожки музыки")
        app.expect(app.has_probe("duck.key"), "в окне дорожки есть выбор ключа")
        app.shot("track-window")

        # 2) выбираем ключ: клик по комбо и по первому пункту под ним
        from kt_live import geometry as geo

        f = next(x for x in app.geom()["fields"] if x["param"] == "duck.key")
        w0 = geo()
        cx = w0["x"] + int(f["x"] + f["w"] / 2)
        cy = w0["y"] + int(f["y"] + f["h"] / 2)
        app.click(cx, cy)
        app.shot("key-popup")
        # пункты попапа идут вниз от комбо: «нет», затем дорожки
        app.click(cx, cy + int(f["h"] * 2.2))
        app.shot("key-picked")

        # 3) сохраняем и смотрим в файл
        app.save_now()
        with open(path) as fh:
            saved = fh.read()
        app.expect('"key"' in saved, "ключ сайдчейна записан в файл")
        app.expect('"v": 12' in saved or '"v":12' in saved, "версия конверта поднялась до v12")

    # 4) СЛУШАЕМ: рендерим тот же файл — и ТОТ ЖЕ файл БЕЗ ПРИБОРА. Речь в
    # обоих одна и та же, значит вся разница под ней — это музыка. Мерить
    # один прогон нельзя: речь в миксе громче музыки и просадку маскирует.
    def render(src, dst):
        subprocess.run([KT_CLI, "render", src, dst], check=True, capture_output=True)
        return read_wav_mono(dst)

    keyed = render(path, os.path.join(out, "keyed.wav"))
    plain_path = os.path.join(out, "plain.otio")
    with open(path) as fh:
        doc = json.load(fh)
    drop_ducker(doc)
    with open(plain_path, "w") as fh:
        json.dump(doc, fh, indent=4)
    plain = render(plain_path, os.path.join(out, "plain.wav"))

    sr = 48000
    under_k = rms(keyed, int(1.4 * sr), int(1.9 * sr))
    under_p = rms(plain, int(1.4 * sr), int(1.9 * sr))
    pause_k = rms(keyed, int(2.8 * sr), int(3.6 * sr))
    pause_p = rms(plain, int(2.8 * sr), int(3.6 * sr))
    print(f"   под речью: с дакером {under_k:.4f}, без прибора {under_p:.4f}")
    print(f"   в паузе:   с дакером {pause_k:.4f}, без прибора {pause_p:.4f}")
    bad = []
    if not under_k < under_p * 0.95:
        bad.append("под речью музыка не просела")
    if abs(pause_k - pause_p) > pause_p * 0.05:
        bad.append("в паузе прибор обязан быть прозрачен (звук как без него)")
    for line in bad:
        print(f"   ПРОВАЛ {line}")
    if bad:
        raise SystemExit(1)
    print("   ok   музыка проседает под речью и в паузе звучит как была")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out")
