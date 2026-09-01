#!/usr/bin/env bash
# Живая проверка в ВИРТУАЛЬНОМ экране (Р.27).
#
# Раньше «живая проверка» была ручной: запусти окно, поводи мышью, посмотри
# скриншот. Это не воспроизводилось, требовало свободного экрана (заблокировал
# сессию — проверить нечем) и не годилось для регресса. Здесь то же самое, но
# в Xvfb: настоящий бинарь kt-app, настоящий X-сервер, настоящие XTest-события
# и настоящий wgpu (софт-рендер) — в отличие от harness-тестов, которые гоняют
# egui без окна.
#
#     ./scripts/live-check.sh                # все сценарии
#     ./scripts/live-check.sh handles fade   # только названные
#     KEEP=1 ./scripts/live-check.sh handles # не гасить экран после (для отладки)
#
# Скриншоты и логи — target/live-out/<сценарий>/.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/target/live-out"
scenarios_dir="$root/scripts/live"
display_num="${KT_LIVE_DISPLAY:-:99}"
screen="${KT_LIVE_SCREEN:-1920x1080x24}"

if ! command -v Xvfb > /dev/null; then
    echo "нет Xvfb: sudo apt install xvfb openbox" >&2
    exit 2
fi
if [ ! -x "$root/target/debug/kt-app" ]; then
    echo "нет target/debug/kt-app: cargo build -p kt-app" >&2
    exit 2
fi

want=("$@")
if [ ${#want[@]} -eq 0 ]; then
    mapfile -t want < <(cd "$scenarios_dir" && ls *.py | sed 's/\.py$//')
fi

pids=()
cleanup() {
    [ -n "${KEEP:-}" ] && return 0
    for p in "${pids[@]:-}"; do kill "$p" 2> /dev/null || true; done
    wait 2> /dev/null || true
}
trap cleanup EXIT

Xvfb "$display_num" -screen 0 "$screen" -nolisten tcp > "$out/xvfb.log" 2>&1 &
pids+=($!)
export DISPLAY="$display_num"
# ждём готовности сервера (xdpyinfo не гарантирован — пробуем через xlib)
for _ in $(seq 40); do
    python3 -c "import Xlib.display, os; Xlib.display.Display(os.environ['DISPLAY'])" 2> /dev/null && break
    sleep 0.25
done

# оконный менеджер нужен для разворачивания окна (правило «проверять в
# РАЗВЁРНУТОМ окне», р.13) и для жестов с кромками
if command -v openbox > /dev/null; then
    openbox > "$out/openbox.log" 2>&1 &
    pids+=($!)
    sleep 0.5
else
    echo "openbox не найден — окно будет своего размера (maximize не проверить)" >&2
fi

mkdir -p "$out"
fail=0
for name in "${want[@]}"; do
    script="$scenarios_dir/$name.py"
    if [ ! -f "$script" ]; then
        echo "нет сценария $name ($script)" >&2
        fail=1
        continue
    fi
    mkdir -p "$out/$name"
    printf '== %s ==\n' "$name"
    if python3 "$script" "$out/$name"; then
        printf '   ok\n'
    else
        printf '   ПРОВАЛ (скриншоты: %s)\n' "$out/$name"
        fail=1
    fi
done

exit $fail
