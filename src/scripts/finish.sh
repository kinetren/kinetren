#!/usr/bin/env bash
# Финал работы над правками (правило п.7 р.12).
#
# `cargo test` собирает ОТДЕЛЬНЫЙ тест-бинарь и НЕ трогает
# target/debug/kt-app — тот, который запускает человек. Из-за этого
# пользователь дважды запускал устаревшую программу и считал дефекты
# неисправленными. Поэтому любой агент заканчивает работу этим скриптом:
# пересобирает то, что запускают, называет версию и подчищает мусор.
#
# С Р.68 скрипт — ЕДИНСТВЕННАЯ точка проверки: прогон тестов мастерской
# стал его стадией (раньше это была отдельная команда агента, и бюджет с
# вердиктом распадались на две несвязанные половины). Каждая стадия
# называет свою цену, а в конце печатается таблица и бюджет — пять минут,
# поставленные владельцем. Перерасход ПОМЕЧАЕТСЯ, но не делает прогон
# красным: правило «тесты из спек не ослабляются» сильнее бюджета.
#
#   scripts/finish.sh            # debug-сборка + проверки + версия + чистка
#   scripts/finish.sh --release  # ещё и релизный тарбол в контейнере
#   scripts/finish.sh --deep     # плюс чистка промежуточных объектов
set -euo pipefail
cd "$(dirname "$0")/.." # src/

RELEASE=0
DEEP=0
for a in "$@"; do
    case "$a" in
        --release) RELEASE=1 ;;
        --deep) DEEP=1 ;;
        *) echo "неизвестный ключ: $a" >&2; exit 2 ;;
    esac
done

VER=$(git log -1 --format=%cd --date=format:%y.%m.%d 2>/dev/null || echo 00.00.00)
HASH=$(git rev-parse --short=8 HEAD 2>/dev/null || echo nogit)
FULL="kt$VER-$HASH"
DIRTY=""
[ -n "$(git status --porcelain 2>/dev/null)" ] && DIRTY=" (есть незакоммиченные правки)"

# Лог одной стадии: вердикт читаем из кода возврата cargo, а короткий
# хвост печатаем человеку (Р.58).
STAGE_LOG="$(mktemp -t kt-finish-stage.XXXXXX)"
trap 'rm -f "$STAGE_LOG"' EXIT

# Хронометраж стадий (Р.68). Файл замеров — «имя<TAB>секунды<TAB>код»;
# таблицу и бюджет считает scripts/finish-budget.py. ЗАГОЛОВОК СТАДИИ
# ПЕЧАТАЕТСЯ ТОЛЬКО ОТСЮДА: иначе стадию можно тихо вынести из бюджета —
# тот же класс, что «красное чужим именем» (Р.55) и «вердикт у tail»
# (Р.58). Сторож `finish-budget.py --check-script` это проверяет.
mkdir -p target
TIMES="target/finish-times.txt"
: > "$TIMES"

stage() { # stage <имя> <команда...> — вывод команды идёт человеку как есть
    local name="$1"; shift
    printf '== %s ==\n' "$name"
    local s rc=0
    s=$(date +%s)
    "$@" || rc=$?
    printf '%s\t%s\t%s\n' "$name" "$(($(date +%s) - s))" "$rc" >> "$TIMES"
    return $rc
}

stage_log() { # stage_log <имя> <команда...> — вывод в файл, вердикт у команды
    local name="$1"; shift
    printf '== %s ==\n' "$name"
    local s rc=0
    s=$(date +%s)
    "$@" > "$STAGE_LOG" 2>&1 || rc=$?
    printf '%s\t%s\t%s\n' "$name" "$(($(date +%s) - s))" "$rc" >> "$TIMES"
    tail -4 "$STAGE_LOG"
    if [ "$rc" != 0 ]; then
        grep -E "^(test .* FAILED|failures:|error)" "$STAGE_LOG" | head -20 >&2
    fi
    return $rc
}

stage "сборка того, что запускает пользователь" cargo build -p kt-app -p kt-cli

if [ "$RELEASE" = 1 ]; then
    stage "релизный тарбол" scripts/release-build.sh
fi

# Прогон тестов мастерской — стадия ЭТОГО скрипта (Р.68). Здесь же живёт
# GPU-конформанс kt-gpu на настоящем адаптере машины: отдельная стадия для
# крейта kt-gpu была дублем этого прогона и снята (её десять секунд уходили
# на то, что мастерская уже прогнала). Без адаптера тесты kt-gpu скипаются
# сами с печатью причины.
#
# ЕСТЬ NEXTEST — ГОНЯЕМ ИМ (замер Р.68): 216 с против 310 с у `cargo test`,
# и — главное — ЗЕЛЁНО. `cargo test` запускает 96 тестовых бинарей по одному,
# а внутри бинаря все тесты разом: t663 ждёт фоновой отрисовки ленты и под
# такой толпой не дожидается (60 с таймаута). nextest раздаёт тесты по
# процессам, планируя по ядрам. Доктестов в мастерской нет (проверено:
# `cargo test --doc` — ноль), так что ничего не теряется. Нет nextest —
# идём прежним путём, он остаётся эталоном.
if command -v cargo-nextest > /dev/null; then
    WS_CMD=(cargo nextest run --workspace --no-fail-fast)
else
    WS_CMD=(cargo test --workspace -q)
fi
if stage_log "тесты мастерской" "${WS_CMD[@]}"; then
    echo "тесты мастерской: ok"
else
    cp "$STAGE_LOG" target/finish-workspace-fail.log 2>/dev/null || true
    echo "ТЕСТЫ МАСТЕРСКОЙ ПРОВАЛИЛИСЬ (полный лог: target/finish-workspace-fail.log)" >&2
    WS_FAILED=1
fi

# SIMD-конформанс (Р.32): AVX2-ядра обязаны быть байт-в-байт со скаляром.
# KT_REQUIRE_SIMD=1 запрещает вакуумный skip там, где AVX2 есть (урок
# т.194). Плюс golden'ы на ПРИНУДИТЕЛЬНОМ скаляре: он остаётся эталоном.
# Своё окружение у каждого прогона — поэтому это отдельные команды, а не
# часть прогона мастерской; стоят они пару секунд на все четыре.
#
# Вердикт у cargo, не у `tail`: `if cargo test | tail` был зелёным ВСЕГДА —
# статус в конвейере отдаёт ПОСЛЕДНЯЯ команда, и стадия не могла сообщить
# о красном в принципе (тот же класс, что §443 и §458).
simd_stage() {
    local name="$1"; shift
    if "$@" > "$STAGE_LOG" 2>&1; then
        tail -3 "$STAGE_LOG"
        return 0
    fi
    tail -3 "$STAGE_LOG"
    grep -E "^(test .* FAILED|failures:|error)" "$STAGE_LOG" | head -20 >&2
    cp "$STAGE_LOG" target/finish-simd-fail.log 2>/dev/null || true
    echo "SIMD: красная стадия $name (полный лог: target/finish-simd-fail.log)" >&2
    return 1
}
simd_all() {
    simd_stage "kt-fx simd_parity" env KT_REQUIRE_SIMD=1 cargo test -p kt-fx --test simd_parity -q \
        && simd_stage "kt-engine simd_parity" env KT_REQUIRE_SIMD=1 cargo test -p kt-engine --test simd_parity -q \
        && simd_stage "kt-fx video_fx (скаляр)" env KT_SIMD=0 cargo test -p kt-fx --test video_fx -q \
        && simd_stage "kt-engine golden_export (скаляр)" env KT_SIMD=0 cargo test -p kt-engine --test golden_export -q
}
if stage "SIMD-конформанс (t198)" simd_all; then
    echo "simd: ok (AVX2 == скаляр, golden'ы зелены обоими путями)"
else
    echo "SIMD-КОНФОРМАНС ПРОВАЛИЛСЯ" >&2
    SIMD_FAILED=1
fi

# Живая проверка в виртуальном экране (Р.27): настоящий бинарь, настоящий X,
# настоящие XTest-жесты. Без Xvfb — просто пропускаем, ничего не ломая;
# NO_LIVE=1 выключает явно (например когда гоняешь finish.sh в цикле).
if [ -z "${NO_LIVE:-}" ] && command -v Xvfb > /dev/null; then
    if stage "живые сценарии" ./scripts/live-check.sh; then
        echo "живые сценарии: ok"
    else
        echo "ЖИВЫЕ СЦЕНАРИИ ПРОВАЛИЛИСЬ (скриншоты: target/live-out)" >&2
        LIVE_FAILED=1
    fi
elif [ -z "${NO_LIVE:-}" ]; then
    echo "живые сценарии пропущены: нет Xvfb (sudo apt install xvfb openbox)"
fi

clean_targets() {
    # старые тарболы и распакованные каталоги: оставляем только текущий
    if [ -d target/dist-out ]; then
        find target/dist-out -maxdepth 1 -name 'kinotrain-*' \
            ! -name "kinotrain-${FULL#kt}-*" -exec rm -rf {} + 2>/dev/null || true
    fi
    if [ "$DEEP" = 1 ]; then
        # release/ и dist-container/ полностью регенерируемы и весят гигабайты;
        # debug/ не трогаем — иначе следующая правка ждёт полную пересборку
        rm -rf target/release target/dist-container
    fi
    du -sh target/debug target/release target/dist-container target/dist-out 2>/dev/null || true
}
stage "чистка" clean_targets

mkdir -p target/dist-out
printf '%s\n%s\n' "$FULL" "$(pwd)/target/debug/kt-app" > target/dist-out/LATEST
echo

# Цена проверки: таблица стадий, итог и бюджет пять минут (Р.68).
python3 scripts/finish-budget.py --report "$TIMES"
echo

# Красное называется СВОИМ ИМЕНЕМ (Р.55): один флаг стоял на трёх стадиях,
# и падение GPU или SIMD печаталось как «живые сценарии красные» — отчёт врал
# о том, ЧТО сломалось, и посылал искать не туда.
[ -n "${LIVE_FAILED:-}" ] && echo "ВНИМАНИЕ: живые сценарии красные — см. target/live-out"
[ -n "${WS_FAILED:-}" ] && echo "ВНИМАНИЕ: тесты мастерской красные — см. target/finish-workspace-fail.log"
[ -n "${SIMD_FAILED:-}" ] && echo "ВНИМАНИЕ: SIMD-конформанс красный (t198)"
echo "ВЕРСИЯ: $FULL$DIRTY"
echo "запускать: $(pwd)/target/debug/kt-app"

# Р.67: ШАГИ ПУТИ ПОЛЬЗОВАТЕЛЯ из живых сценариев — в отчёт. Правило «думай
# о пользователе» держится двумя механизмами: строкой ПУТЬ в спеке (сторож
# ниже) и КАРТИНКАМИ того, что человек увидит. Без списка они остаются
# лежать в target/live-out, и отчёт снова превращается в «тесты зелёные».
STEPS=$(ls -1 target/live-out/*/[0-9]*-step-*.png 2>/dev/null | sort)
if [ -n "$STEPS" ]; then
    echo "шаги живых сценариев (в отчёт):"
    echo "$STEPS" | sed 's|^|  |'
else
    echo "шагов живых сценариев нет — прогон: ./scripts/live-check.sh"
fi

# сторож «пути пользователя» у новых UI-сценариев (Р.67)
python3 "$(dirname "$0")/specs-user-path.py" --check || exit 1
# сторожа живой проверки (Р.68): ожидание вместо сна и живое покрытие —
# сколько мест программы человек не трогал мышью НИ РАЗУ
python3 "$(dirname "$0")/live-sleeps.py" --check || exit 1
python3 "$(dirname "$0")/live-coverage.py" --check --brief || exit 1
# Р.43 п.301: debug-сборка честно считает, но в 9 раз медленнее. На плотном
# монтаже аудио-рендер там уходит за 100 % реального времени, кольцо пустеет
# и растут callback-underrun'ы — их слышно тишиной. Это цена профиля, а не
# дефект, и судить о скорости по debug нельзя.
if [ ! -x target/release/kt-app ]; then
    echo "о СКОРОСТИ судить только по релизу: cargo build --release -p kt-app"
else
    echo "для замеров скорости: $(pwd)/target/release/kt-app"
fi
[ "$RELEASE" = 1 ] && echo "тарбол:   $(ls -1 target/dist-out/*.tar.gz 2>/dev/null | tail -1)"
exit 0
