#!/usr/bin/env bash
# venv с эталонной библиотекой OTIO и MLT-адаптером — для кросс-рендер
# конформанса (тест 158, Р.25). Запускает человек/CI, тест только проверяет
# маркер и подсказывает эту команду.
#
# Адаптер otio-mlt-adapter официально поддерживает otio <= 0.14, поэтому
# пины перебираются лестницей: работает свежий — берём свежий.
set -euo pipefail

venv="$(cd "$(dirname "$0")/.." && pwd)/target/otio-venv"
marker="$venv/.kt-ok"

smoke() { # проверка, что адаптер РЕАЛЬНО пишет .mlt на этой версии otio
    "$venv/bin/python3" - <<'PY'
import opentimelineio as otio
from opentimelineio.opentime import RationalTime, TimeRange
tl = otio.schema.Timeline(name="smoke")
tr = otio.schema.Track(name="V", kind="Video")
c = otio.schema.Clip(name="a")
c.media_reference = otio.schema.ExternalReference(target_url="a.mp4")
c.source_range = TimeRange(RationalTime(0, 24), RationalTime(24, 24))
tr.append(c)
tl.tracks.append(tr)
xml = otio.adapters.write_to_string(tl, adapter_name="mlt_xml")
assert "<mlt" in xml, xml[:200]
import otio_mlt_adapter as a
print(f"{otio.__version__} {getattr(a, '__version__', '?')}")
PY
}

if [ -x "$venv/bin/python3" ] && [ -f "$marker" ] && smoke > /dev/null 2>&1; then
    echo "уже готов: $(cat "$marker")"
    exit 0
fi

rm -rf "$venv"
python3 -m venv "$venv"
"$venv/bin/pip" install -q --upgrade pip

for pin in "opentimelineio" "opentimelineio==0.17.*" "opentimelineio==0.16.*" \
           "opentimelineio==0.15.*" "opentimelineio==0.14.1"; do
    echo "пробую $pin…"
    if ! "$venv/bin/pip" install -q "$pin" otio-mlt-adapter 2>/dev/null; then
        continue
    fi
    if out=$(smoke 2>/dev/null); then
        echo "otio $out (адаптер mlt_xml работает)" > "$marker"
        echo "готово: $(cat "$marker")"
        exit 0
    fi
done

echo "ни один пин opentimelineio не дал рабочий адаптер mlt_xml" >&2
exit 1
