#!/usr/bin/env bash
# Живая проверка в ВИРТУАЛЬНОМ экране (Р.27, параллель и стенд — Р.68).
#
# Раньше «живая проверка» была ручной: запусти окно, поводи мышью, посмотри
# скриншот. Это не воспроизводилось, требовало свободного экрана (заблокировал
# сессию — проверить нечем) и не годилось для регресса. Здесь то же самое, но
# в Xvfb: настоящий бинарь kt-app, настоящий X-сервер, настоящие XTest-события
# и настоящий wgpu (софт-рендер) — в отличие от harness-тестов, которые гоняют
# egui без окна.
#
# С Р.68 сценарии раздаются РАБОТНИКАМ (решение владельца: «Да, с ограничением
# нагрузки и авто-перезапуском красного»). У каждого работника свой Xvfb, своё
# пользовательское хранилище и один живой kt-app — стенд, в котором проекты
# сценариев открываются подряд (scripts/live/worker.py). Запрет Р.59 §468
# («живые сценарии в два потока не гоняются») снят вместе с его причиной:
# красный сценарий автоматически перегоняется ОДИН в одиночку, и только
# второе падение считается красным.
#
#     ./scripts/live-check.sh                # все сценарии
#     ./scripts/live-check.sh handles fade   # только названные
#     KT_LIVE_WORKERS=1 ./scripts/live-check.sh   # строго по одному, без перегонов
#
# Скриншоты и логи — target/live-out/<сценарий>/.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
# КАТАЛОГ ПРОГОНА — переопределяемый (Р.68): сторожа t801/t802 гоняют
# live-check во время прогона мастерской, а он чистит каталог вердиктов —
# и чужой ИДУЩИЙ прогон терял свои вердикты, объявляя красным всё подряд.
out="${KT_LIVE_OUT:-$root/target/live-out}"
scenarios_dir="$root/scripts/live"
display_base="${KT_LIVE_DISPLAY:-:99}"
display_base="${display_base#:}"

if ! command -v Xvfb > /dev/null; then
    echo "нет Xvfb: sudo apt install xvfb openbox" >&2
    exit 2
fi
if [ ! -x "$root/target/debug/kt-app" ]; then
    echo "нет target/debug/kt-app: cargo build -p kt-app" >&2
    exit 2
fi

mkdir -p "$out"
rm -rf "$out/verdicts"

want=("$@")
if [ ${#want[@]} -eq 0 ]; then
    # kt_live.py — каркас, worker.py — работник, `_*.py` — подставные
    # сценарии сторожей (t801/t802): в общий прогон они не входят
    mapfile -t want < <(cd "$scenarios_dir" && ls *.py \
        | grep -v -E '^(kt_live|worker|_.*)\.py$' | sed 's/\.py$//')
fi

# Сколько работников. Один kt-app в софт-рендере держит нагрузку около семи
# (замер Р.68), поэтому делим ядра на восемь: именно нагрузка ~14 на две
# копии и красила зелёные сценарии в §468. Потолок — шесть: дальше упираемся
# в диск и в X-серверы, а выигрыш становится шумом.
cores=$(nproc 2> /dev/null || echo 4)
auto=$((cores / 8))
[ "$auto" -lt 1 ] && auto=1
[ "$auto" -gt 6 ] && auto=6
workers="${KT_LIVE_WORKERS:-$auto}"
[ "${#want[@]}" -lt "$workers" ] && workers="${#want[@]}"
[ "$workers" -lt 1 ] && workers=1

run_worker() { # run_worker <номер> <сценарии...>
    local n="$1"; shift
    [ $# -eq 0 ] && return 0
    local disp=":$((display_base + n))"
    python3 "$scenarios_dir/worker.py" "$disp" "$out" "$@" 2>&1 \
        | sed "s/^/[$n] /"
}

echo "живые сценарии: ${#want[@]} шт., работников $workers (ядер $cores)"
pids=()
for ((w = 0; w < workers; w++)); do
    bucket=()
    for ((i = w; i < ${#want[@]}; i += workers)); do
        bucket+=("${want[i]}")
    done
    run_worker "$w" "${bucket[@]}" &
    pids+=($!)
done
for p in "${pids[@]}"; do wait "$p" || true; done

verdicts="$out/verdicts"
reds=()
for name in "${want[@]}"; do
    if [ "$(cat "$verdicts/$name" 2> /dev/null || echo fail)" != ok ]; then
        reds+=("$name")
    fi
done

if [ ${#reds[@]} -eq 0 ]; then
    echo "живые сценарии: ${#want[@]} из ${#want[@]} зелёные"
    exit 0
fi

# ПЕРЕГОН КРАСНОГО В ОДИНОЧКУ (Р.68). Покраснение под нагрузкой — не
# вердикт: §468 показал, что второй прогон рядом красит зелёные сценарии, и
# ровно это правило уже записано для «упавшего на паузе или порядке фонового
# потока». Теперь его исполняет машина, а не память агента. Молчаливым
# перегон быть не имеет права — иначе флак становится невидимкой.
if [ "$workers" = 1 ]; then
    echo "КРАСНЫЕ (прогон и так одиночный): ${reds[*]}" >&2
    exit 1
fi
echo "перегон в одиночку: ${reds[*]}"
again=()
for name in "${reds[@]}"; do
    # `|| true`: работник возвращает код сценария, а `set -e` на нём молча
    # обрывал ВЕСЬ перегон — отчёт терял и «прошёл со второго раза», и
    # «красные дважды» (поймано тестом t801)
    run_worker 0 "$name" || true
    if [ "$(cat "$verdicts/$name" 2> /dev/null || echo fail)" != ok ]; then
        again+=("$name")
    fi
done

for name in "${reds[@]}"; do
    case " ${again[*]} " in
        *" $name "*) ;;
        *) echo "ПРОШЁЛ СО ВТОРОГО РАЗА (в одиночку): $name — разобрать сценарий" ;;
    esac
done

if [ ${#again[@]} -eq 0 ]; then
    echo "живые сценарии: зелёные (часть — со второго раза)"
    exit 0
fi
echo "КРАСНЫЕ ДВАЖДЫ: ${again[*]} (скриншоты: target/live-out/<сценарий>)" >&2
exit 1
