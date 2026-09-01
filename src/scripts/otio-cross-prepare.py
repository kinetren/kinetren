#!/usr/bin/env python3
"""Подготовка чужого .otio к кросс-рендеру (тест 158, Р.25).

Запускается python'ом из target/otio-venv (эталонная библиотека OTIO +
адаптер mlt_xml). На вход — сэмпл из fixtures/otio-samples, на выходе:

    <out>/media/mNN.mp4   синтетика: сплошной цвет из палитры + тишина
    <out>/prepared.otio   тот же монтаж, все ссылки → на синтетику
    <out>/prepared.mlt    он же для melt (может отсутствовать: см. mlt_error)
    <out>/manifest.json   палитра, окна переходов, длительность

Почему сплошные цвета: «что играет в момент t» превращается в цвет пикселя,
и сравнение двух рендеров становится классификацией, устойчивой к разнице
кодеков. Палитра разносится так, чтобы её пережила любая одиночная путаница
BT.601/709 и limited/full range (см. классификатор в cross_render.rs).

Использование:
    otio-cross-prepare.py <sample.otio> <out_dir> [--ffmpeg ffmpeg]
"""
import json
import math
import os
import subprocess
import sys

import opentimelineio as otio
from opentimelineio.opentime import RationalTime, TimeRange

# Кадр синтетики. 720p: у MLT эвристика colorspace по высоте кадра (< 720 —
# BT.601, иначе BT.709), и мы не хотим, чтобы она сработала неожиданно.
W, H = 1280, 720
# Минимальное расстояние между вариантами цветов палитры (евклид в RGB).
# Классификатор принимает точку при расстоянии <= 32, поэтому 96 > 2*32
# даёт доказуемую разделимость даже при неверно угаданной конвенции.
MIN_DIST = 96.0
# Длина источника, выше которой клипы ребейзятся к нулю (см. ниже).
LONG_SOURCE_S = 60.0


def hsv_rgb(h, s, v):
    i = int(h * 6) % 6
    f = h * 6 - int(h * 6)
    p, q, t = v * (1 - s), v * (1 - f * s), v * (1 - (1 - f) * s)
    r, g, b = [(v, t, p), (q, v, p), (p, v, t), (p, q, v), (t, p, v), (v, p, q)][i]
    return (round(r * 255), round(g * 255), round(b * 255))


def yuv_variants(rgb):
    """Все значения, в которые цвет может превратиться при ОДНОЙ путанице
    конвенций: матрица (601/709) и диапазон (limited/full) на кодировании и
    на декодировании — 16 комбинаций, после дедупа меньше."""
    out = []
    for kr1, kb1 in ((0.299, 0.114), (0.2126, 0.0722)):
        for lim1 in (True, False):
            y, pb, pr = encode(rgb, kr1, kb1, lim1)
            for kr2, kb2 in ((0.299, 0.114), (0.2126, 0.0722)):
                for lim2 in (True, False):
                    v = decode(y, pb, pr, kr2, kb2, lim2)
                    if v not in out:
                        out.append(v)
    return out


def encode(rgb, kr, kb, limited):
    r, g, b = [c / 255.0 for c in rgb]
    kg = 1 - kr - kb
    y = kr * r + kg * g + kb * b
    pb = (b - y) / (2 * (1 - kb))
    pr = (r - y) / (2 * (1 - kr))
    if limited:
        return (
            clamp8(round(16 + 219 * y)),
            clamp8(round(128 + 224 * pb)),
            clamp8(round(128 + 224 * pr)),
        )
    return (clamp8(round(255 * y)), clamp8(round(128 + 255 * pb)), clamp8(round(128 + 255 * pr)))


def decode(yq, cbq, crq, kr, kb, limited):
    if limited:
        y = (yq - 16) / 219.0
        pb = (cbq - 128) / 224.0
        pr = (crq - 128) / 224.0
    else:
        y = yq / 255.0
        pb = (cbq - 128) / 255.0
        pr = (crq - 128) / 255.0
    kg = 1 - kr - kb
    r = y + 2 * (1 - kr) * pr
    b = y + 2 * (1 - kb) * pb
    g = (y - kr * r - kb * b) / kg
    return tuple(clamp8(round(c * 255)) for c in (r, g, b))


def clamp8(v):
    return max(0, min(255, int(v)))


def dist(a, b):
    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))


def build_palette(n):
    """Жадный отбор цветов, разнесённых с запасом ПО ВСЕМ конвенционным
    вариантам (и от чёрного — он занят под пустоту/леттербокс/фон melt)."""
    taken = []  # [(rgb, [варианты])]
    black = [(0, 0, 0)]
    candidates = []
    for i in range(12):
        for s, v in ((1.0, 1.0), (1.0, 0.55), (0.45, 1.0)):
            candidates.append(hsv_rgb(i / 12.0, s, v))
    for rgb in candidates:
        vs = yuv_variants(rgb)
        ok = all(dist(v, k) >= MIN_DIST for v in vs for k in black)
        ok = ok and all(dist(v, u) >= MIN_DIST for v in vs for _, us in taken for u in us)
        if ok:
            taken.append((rgb, vs))
        if len(taken) >= n:
            break
    if len(taken) < n:
        sys.exit(
            f"палитра вмещает {len(taken)} цветов, а источников {n}: "
            "нужен другой сэмпл или менее строгий MIN_DIST"
        )
    return [rgb for rgb, _ in taken]


def walk(comp, offset, out, transitions, rate_holder):
    """Рекурсивный обход дорожек/стеков: собирает клипы и окна переходов."""
    prev_end = offset
    cursor = offset
    for item in comp:
        if isinstance(item, otio.schema.Transition):
            # окно перехода вокруг ТЕКУЩЕГО стыка
            cut = cursor
            transitions.append(
                (
                    to_sec(cut - item.in_offset.value / rate_holder[0]),
                    to_sec(cut + item.out_offset.value / rate_holder[0]),
                )
            )
            continue
        dur = 0.0
        try:
            dur = item.duration().value / item.duration().rate
        except Exception:
            pass
        if isinstance(item, (otio.schema.Track, otio.schema.Stack)):
            walk(item, cursor, out, transitions, rate_holder)
        elif isinstance(item, otio.schema.Clip):
            out.append((item, cursor, dur))
        cursor += dur
        prev_end = cursor
    return prev_end


def to_sec(x):
    return float(x)


def media_key(clip):
    """Идентичность источника: URL, а без него — имя клипа (EDL-семантика:
    одноимённые события ссылаются на одну катушку)."""
    mr = clip.media_reference
    for attr in ("target_url", "target_url_base", "generator_kind"):
        v = getattr(mr, attr, None) if mr is not None else None
        if v:
            return str(v)
    return f"name:{clip.name or id(clip)}"


def read_timeline(src):
    """Читает сэмпл эталоном. Апстримовские файлы бывают новее релиза
    библиотеки (Marker.3 в premiere_example/screening_example — её же
    сэмплы!), и тогда она отказывается читать файл целиком. На картинку
    маркеры не влияют, поэтому для рендера читаем копию без них и
    отмечаем это в манифесте."""
    try:
        return otio.adapters.read_from_file(src), False
    except Exception:
        with open(src) as f:
            doc = json.load(f)
        strip_markers(doc)
        return otio.adapters.read_from_string(json.dumps(doc)), True


def strip_markers(node):
    if isinstance(node, dict):
        if isinstance(node.get("markers"), list):
            node["markers"] = []
        for v in node.values():
            strip_markers(v)
    elif isinstance(node, list):
        for v in node:
            strip_markers(v)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    ffmpeg = "ffmpeg"
    for i, a in enumerate(sys.argv):
        if a == "--ffmpeg" and i + 1 < len(sys.argv):
            ffmpeg = sys.argv[i + 1]
    if len(args) != 2:
        sys.exit(__doc__)
    src, outdir = args
    os.makedirs(os.path.join(outdir, "media"), exist_ok=True)

    tl, stripped = read_timeline(src)
    rate = 24.0
    try:
        rate = tl.duration().rate
    except Exception:
        pass
    clips, transitions = [], []
    rate_holder = [rate]
    # ВНИМАНИЕ: tl.duration() эталона суммирует и Transition'ы, хотя времени
    # на дорожке они не занимают (у transition_test 170 кадров вместо 150).
    # Монтажную длину считаем сами — по концам реальных элементов.
    timeline_end = 0.0
    for track in tl.tracks:
        end = walk(track, 0.0, clips, transitions, rate_holder)
        timeline_end = max(timeline_end, end)
    if not clips:
        sys.exit(f"{src}: клипов не найдено")

    # уникальные источники и требуемая длина каждого
    need = {}
    for clip, _start, _dur in clips:
        k = media_key(clip)
        try:
            sr = clip.source_range
            end = (sr.start_time.value + sr.duration.value) / sr.start_time.rate
        except Exception:
            end = 0.0
        av = getattr(clip.media_reference, "available_range", None)
        if av is not None:
            end = max(end, (av.start_time.value + av.duration.value) / av.start_time.rate)
        need[k] = max(need.get(k, 0.0), end)

    # EDL-монтажи берут куски с 60-й минуты катушки (screening_example:
    # start_time 86438 кадров) — генерировать час сплошного цвета незачем:
    # у одноцветного материала точка входа на картинку не влияет, поэтому
    # такие клипы ребейзятся к нулю. Отмечается в манифесте.
    rebased = 0
    for clip, _start, _dur in clips:
        k = media_key(clip)
        if need.get(k, 0.0) <= LONG_SOURCE_S or clip.source_range is None:
            continue
        sr = clip.source_range
        clip.source_range = TimeRange(RationalTime(0, sr.start_time.rate), sr.duration)
        rebased += 1
    if rebased:
        need = {}
        for clip, _start, _dur in clips:
            k = media_key(clip)
            sr = clip.source_range
            end = (sr.start_time.value + sr.duration.value) / sr.start_time.rate if sr else 0.0
            need[k] = max(need.get(k, 0.0), end)

    keys = sorted(need)
    palette = build_palette(len(keys))
    media = []
    for idx, k in enumerate(keys):
        rgb = palette[idx]
        path = os.path.join(outdir, "media", f"m{idx:02d}.mp4")
        dur = max(need[k], 1.0) + 1.0
        if not os.path.exists(path):
            gen(ffmpeg, path, rgb, dur, rate)
        media.append({"key": k, "path": path, "rgb": list(rgb)})

    # подмена ВСЕХ ссылок на синтетику; клипу без source_range он проставляется
    # из available_range (иначе адаптер MLT не знает, что резать)
    by_key = {m["key"]: m for m in media}
    for clip, _start, _dur in clips:
        m = by_key[media_key(clip)]
        av_dur = need[m["key"]] + 1.0
        if clip.source_range is None:
            # клип берёт материал целиком: длина — из ЕГО ИСХОДНОГО
            # available_range, а не из длины нашей синтетики (иначе монтаж
            # растянется и сравнивать будет нечего)
            orig = getattr(clip.media_reference, "available_range", None)
            clip.source_range = (
                orig
                if orig is not None
                else TimeRange(RationalTime(0, rate), RationalTime(round(rate), rate))
            )
        clip.media_reference = otio.schema.ExternalReference(
            target_url=os.path.abspath(m["path"]),
            available_range=TimeRange(RationalTime(0, rate), RationalTime(round(av_dur * rate), rate)),
        )

    prepared = os.path.join(outdir, "prepared.otio")
    otio.adapters.write_to_file(tl, prepared)

    mlt_path, mlt_error = os.path.join(outdir, "prepared.mlt"), None
    try:
        otio.adapters.write_to_file(
            tl,
            mlt_path,
            adapter_name="mlt_xml",
            width=W,
            height=H,
            frame_rate_num=int(round(rate)),
            frame_rate_den=1,
        )
    except Exception as e:  # адаптер не всё умеет — это валидный исход
        mlt_error = f"{type(e).__name__}: {e}"
        mlt_path = None

    manifest = {
        "sample": os.path.basename(src),
        "stripped_markers": stripped,
        "rebased_clips": rebased,
        "rate": rate,
        "width": W,
        "height": H,
        "duration_seconds": timeline_end,
        "media": media,
        "transitions": [{"start_s": a, "end_s": b} for a, b in transitions],
        "mlt": mlt_path,
        "mlt_error": mlt_error,
        "prepared": prepared,
    }
    with open(os.path.join(outdir, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2)
    print(json.dumps({"media": len(media), "transitions": len(transitions), "mlt_error": mlt_error}))


def gen(ffmpeg, path, rgb, dur, rate):
    color = "0x%02X%02X%02X" % rgb
    tmp = path + ".tmp.mp4"
    subprocess.run(
        [
            ffmpeg, "-v", "error", "-y",
            "-f", "lavfi", "-i", f"color=c={color}:size={W}x{H}:rate={rate}:duration={dur:.3f}",
            "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo",
            "-shortest",
            "-vf", "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p",
            "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
            "-color_range", "tv",
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "15", "-g", "30",
            "-c:a", "aac", "-b:a", "96k",
            tmp,
        ],
        check=True,
    )
    os.replace(tmp, path)


if __name__ == "__main__":
    main()
