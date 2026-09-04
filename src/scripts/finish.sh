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
if cargo test -p kt-gpu -q 2>&1 | tail -4; then
    echo "kt-gpu: ok"
else
    echo "KT-GPU ТЕСТЫ ПРОВАЛИЛИСЬ" >&2
    LIVE_FAILED=1
fi

# SIMD-конформанс (Р.32): AVX2-ядра обязаны быть байт-в-байт со скаляром.
# KT_REQUIRE_SIMD=1 запрещает вакуумный skip там, где AVX2 есть (урок
# т.194). Плюс golden'ы на ПРИНУДИТЕЛЬНОМ скаляре: он остаётся эталоном.
echo "== SIMD-конформанс (t198) =="
if KT_REQUIRE_SIMD=1 cargo test -p kt-fx --test simd_parity -q 2>&1 | tail -3 \
    && KT_REQUIRE_SIMD=1 cargo test -p kt-engine --test simd_parity -q 2>&1 | tail -3 \
    && KT_SIMD=0 cargo test -p kt-fx --test video_fx -q 2>&1 | tail -2 \
    && KT_SIMD=0 cargo test -p kt-engine --test golden_export -q 2>&1 | tail -2; then
    echo "simd: ok (AVX2 == скаляр, golden'ы зелены обоими путями)"
else
    echo "SIMD-КОНФОРМАНС ПРОВАЛИЛСЯ" >&2
    LIVE_FAILED=1
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
[ -n "${LIVE_FAILED:-}" ] && echo "ВНИМАНИЕ: живые сценарии красные — см. target/live-out"
echo "ВЕРСИЯ: $FULL$DIRTY"
echo "запускать: $(pwd)/target/debug/kt-app"
[ "$RELEASE" = 1 ] && echo "тарбол:   $(ls -1 target/dist-out/*.tar.gz 2>/dev/null | tail -1)"
exit 0
