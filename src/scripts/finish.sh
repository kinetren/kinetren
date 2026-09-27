#!/usr/bin/env bash
# Финал работы над правками (правило п.7 р.12).
#
# `cargo test` собирает ОТДЕЛЬНЫЙ тест-бинарь и НЕ трогает
# target/debug/kt-app — тот, который запускает человек. Из-за этого
# пользователь дважды запускал устаревшую программу и считал дефекты
# неисправленными. Поэтому любой агент заканчивает работу этим скриптом:
# пересобирает то, что запускают, называет версию и подчищает мусор.
#
#   scripts/finish.sh            # debug-сборка + версия + чистка
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

echo "== сборка того, что запускает пользователь =="
cargo build -p kt-app -p kt-cli

if [ "$RELEASE" = 1 ]; then
    echo "== релизный тарбол =="
    scripts/release-build.sh
fi

# Лог одной стадии: вердикт читаем из кода возврата cargo, а короткий
# хвост печатаем человеку (Р.58).
STAGE_LOG="$(mktemp -t kt-finish-stage.XXXXXX)"
trap 'rm -f "$STAGE_LOG"' EXIT

# Живая проверка в виртуальном экране (Р.27): настоящий бинарь, настоящий X,
# настоящие XTest-жесты. Без Xvfb — просто пропускаем, ничего не ломая;
# NO_LIVE=1 выключает явно (например когда гоняешь finish.sh в цикле).
if [ -z "${NO_LIVE:-}" ] && command -v Xvfb > /dev/null; then
    echo "== живые сценарии =="
    if ./scripts/live-check.sh; then
        echo "живые сценарии: ok"
    else
        echo "ЖИВЫЕ СЦЕНАРИИ ПРОВАЛИЛИСЬ (скриншоты: target/live-out)" >&2
        LIVE_FAILED=1
    fi
elif [ -z "${NO_LIVE:-}" ]; then
    echo "живые сценарии пропущены: нет Xvfb (sudo apt install xvfb openbox)"
fi

# GPU-конформанс (Р.29): parity-тесты kt-gpu на НАСТОЯЩЕМ адаптере этой
# машины (дискретная карта/железный драйвер, а не только lavapipe из CI).
# Без адаптера тесты скипаются сами с печатью причины.
echo "== GPU-конформанс (kt-gpu) =="
# Итог стадии берётся у CARGO, а не у `tail` (Р.58): в конвейере статус
# отдаёт ПОСЛЕДНЯЯ команда, поэтому `if cargo test | tail` был зелёным
# ВСЕГДА — стадия не могла сообщить о красном в принципе. Тот же класс,
# что §443 (скрипт называл красное чужим именем) и §458 (вывод не
# фильтруется), только здесь терялся сам вердикт.
cargo test -p kt-gpu -q > "$STAGE_LOG" 2>&1 && GPU_RC=0 || GPU_RC=$?
tail -4 "$STAGE_LOG"
if [ "$GPU_RC" = 0 ]; then
    echo "kt-gpu: ok"
else
    grep -E "^(test .* FAILED|failures:|error)" "$STAGE_LOG" | head -20 >&2
    cp "$STAGE_LOG" target/finish-gpu-fail.log 2>/dev/null || true
    echo "KT-GPU ТЕСТЫ ПРОВАЛИЛИСЬ (полный лог: target/finish-gpu-fail.log)" >&2
    GPU_FAILED=1
fi

# SIMD-конформанс (Р.32): AVX2-ядра обязаны быть байт-в-байт со скаляром.
# KT_REQUIRE_SIMD=1 запрещает вакуумный skip там, где AVX2 есть (урок
# т.194). Плюс golden'ы на ПРИНУДИТЕЛЬНОМ скаляре: он остаётся эталоном.
echo "== SIMD-конформанс (t198) =="
# та же правка, что у GPU-стадии: вердикт у cargo, не у `tail`
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
if simd_stage "kt-fx simd_parity" env KT_REQUIRE_SIMD=1 cargo test -p kt-fx --test simd_parity -q \
    && simd_stage "kt-engine simd_parity" env KT_REQUIRE_SIMD=1 cargo test -p kt-engine --test simd_parity -q \
    && simd_stage "kt-fx video_fx (скаляр)" env KT_SIMD=0 cargo test -p kt-fx --test video_fx -q \
    && simd_stage "kt-engine golden_export (скаляр)" env KT_SIMD=0 cargo test -p kt-engine --test golden_export -q; then
    echo "simd: ok (AVX2 == скаляр, golden'ы зелены обоими путями)"
else
    echo "SIMD-КОНФОРМАНС ПРОВАЛИЛСЯ" >&2
    SIMD_FAILED=1
fi

echo "== чистка =="
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

mkdir -p target/dist-out
printf '%s\n%s\n' "$FULL" "$(pwd)/target/debug/kt-app" > target/dist-out/LATEST
echo
# Красное называется СВОИМ ИМЕНЕМ (Р.55): один флаг стоял на трёх стадиях,
# и падение GPU или SIMD печаталось как «живые сценарии красные» — отчёт врал
# о том, ЧТО сломалось, и посылал искать не туда.
[ -n "${LIVE_FAILED:-}" ] && echo "ВНИМАНИЕ: живые сценарии красные — см. target/live-out"
[ -n "${GPU_FAILED:-}" ] && echo "ВНИМАНИЕ: GPU-конформанс красный (kt-gpu)"
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
