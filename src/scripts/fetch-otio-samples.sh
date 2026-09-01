#!/usr/bin/env bash
# Обновление чужих .otio-сэмплов для кросс-рендер конформанса (тест 158, Р.25).
#
# Файлы КОММИТЯТСЯ (решение пользователя Р.25 п.2: «из интернета» — про
# происхождение, а не про способ доставки; тест обязан работать офлайн).
# Скрипт нужен только чтобы поднять пин и увидеть дрейф через git diff.
set -euo pipefail

# Пин каталога tests/sample_data апстрима (Apache-2.0).
PIN=73803eff1dbf86b82bcf97179567a243f4036b90
RAW=https://raw.githubusercontent.com/AcademySoftwareFoundation/OpenTimelineIO/$PIN

dir="$(cd "$(dirname "$0")/../fixtures/otio-samples" && pwd)"
cd "$dir"

get() { # get <относительный путь в репо> <имя файла у нас>
    curl -fsSL "$RAW/$1" -o ".tmp.$2"
    mv ".tmp.$2" "$2"
}

for f in simple_cut multiple_track multitrack preflattened transition_test \
         screening_example nested_example premiere_example \
         generator_reference_test; do
    get "tests/sample_data/$f.otio" "$f.otio"
    echo "  $f.otio"
done
get LICENSE.txt LICENSE

echo "пин: $PIN (обновляя, поправь и README.md)"
