#!/usr/bin/env python3
"""Подготовка НАСТОЯЩЕГО фильма к кросс-рендеру (тест 179).

Вход  — .otio из fixtures/otio-oc и лежащий рядом исходник.
Выход — <out>/prepared.otio (ссылка на медиа абсолютная), <out>/prepared.mlt
        (для melt) и <out>/manifest.json с частотой, длительностью и
        позициями склеек.

Отличие от otio-cross-prepare.py: там синтетика вместо медиа, здесь медиа
настоящее — трогать его нельзя, можно только переписать ссылку и при
необходимости отрезать хвост монтажа (чтобы тест не кодировал три минуты
1080p ради проверки склеек).
"""
import json
import os
import sys

import opentimelineio as otio


def main(src, outdir, window_s):
    os.makedirs(outdir, exist_ok=True)
    tl = otio.adapters.read_from_file(src)
    base = os.path.dirname(os.path.abspath(src))

    rate = None
    cuts, kept = [], 0
    for track in tl.tracks:
        cursor = 0.0
        keep = []
        for item in list(track):
            dur = item.duration().to_seconds()
            if rate is None and isinstance(item, otio.schema.Clip):
                rate = item.duration().rate
            # окно: берём целые клипы, пока не набрали window_s
            if window_s and cursor >= window_s:
                break
            keep.append(item)
            cursor += dur
            cuts.append(round(cursor, 6))
            kept += 1
        del track[len(keep):]
        # ссылки на медиа — абсолютные: prepared.otio живёт во временном
        # каталоге, а исходник остаётся на своём месте
        for item in track:
            mr = getattr(item, "media_reference", None)
            url = getattr(mr, "target_url", None)
            if url and not os.path.isabs(url):
                mr.target_url = os.path.join(base, url)

    # НАМ файл отдаётся КАК ЕСТЬ — со вшитым стартовым таймкодом материала
    # (у фильма available_range начинается с часа). Это часть проверки:
    # ридер обязан вычесть таймкод сам (п.180).
    prepared = os.path.join(outdir, "prepared.otio")
    otio.adapters.write_to_file(tl, prepared)

    # А вот MLT-адаптеру таймкод не по зубам: он пишет available_range
    # прямо в producer in/out, для MLT это «отмотай файл на 86160-й кадр»,
    # а кадров в нём 3890 — melt рендерит ЧЁРНОЕ. Поэтому ДЛЯ НЕГО сносим
    # таймкод к нулю: сравнивать надо рендеры, а не чужие конвенции.
    for track in tl.tracks:
        for item in track:
            mr = getattr(item, "media_reference", None)
            av = getattr(mr, "available_range", None)
            if av is None or item.source_range is None:
                continue
            off = av.start_time
            item.source_range = otio.opentime.TimeRange(
                start_time=item.source_range.start_time - off,
                duration=item.source_range.duration,
            )
            mr.available_range = otio.opentime.TimeRange(
                start_time=off - off, duration=av.duration
            )

    mlt, mlt_error = os.path.join(outdir, "prepared.mlt"), None
    try:
        otio.adapters.write_to_file(tl, mlt, adapter_name="mlt_xml")
    except Exception as e:  # адаптера может не быть — тест скажет причину
        mlt, mlt_error = None, f"{type(e).__name__}: {e}"

    json.dump(
        {
            "prepared": prepared,
            "mlt": mlt,
            "mlt_error": mlt_error,
            "rate": rate,
            "clips": kept,
            "duration_seconds": tl.duration().to_seconds(),
            "cuts": cuts,
        },
        open(os.path.join(outdir, "manifest.json"), "w"),
        indent=1,
    )


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], float(sys.argv[3]) if len(sys.argv) > 3 else 0.0)
