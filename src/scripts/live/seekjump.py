#!/usr/bin/env python3
"""Живой сценарий Р.63 (п.13): прыжок в длинный файл не глушит звук.

Жалоба владельца дословно: «я просто прыгнул на участок с одним длинным
событием (да над ним есть другие, и да у этого есть эффекты) но это не
повод так долго подгружать видео с такими underruns». По флайт-логу каждый
прыжок давал 1-6 с полной тишины при цене самого рендера 2 % реального
времени.

Здесь это проверяется в настоящем окне: прыжки по длинному H.264 при игре,
а счётчик underrun'ов читается ИЗ САМОЙ ПРОГРАММЫ (дамп геометрии), а не с
картинки. Если рядом лежит файл владельца (`sources/kt-untitled-review-870s
.mp4`), сценарий гоняет и его — на нём дефект и наблюдался.

ЧЕСТНАЯ ОГОВОРКА (Р.63): на ЭТОЙ машине сценарий зелёный и на коде ДО
починки — воспроизвести голодание им не удалось даже тремя слоями поверх
длинного события. Значит он сторож от регрессии, а не доказательство
починки: доказательство — движковый тест t758 (на прежнем коде 0 из 8
бёрстов за 250 мс) и замеры стоимости декода у владельца (бёрст аудио
14 мкс против 22-61 мс на кадр 1080p). Сцена владельца была тяжелее:
шесть видеодорожек с эффектами, четыре медиа, GPU-композит.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, pause, project_with_clips, require_tools  # noqa: E402


def stack_tracks(dest, copies):
    """Сложить ЕЩЁ N видеодорожек тем же медиа: у владельца над длинным
    событием лежали другие, и именно так спан набирает длину — заказ идёт
    сразу по нескольким слоям."""
    doc = json.load(open(dest))
    vid = [t for t in doc["tracks"]["children"] if t.get("kind") == "Video"]
    base = vid[0]
    nid = 100
    for _ in range(copies):
        clone = json.loads(json.dumps(base))
        for c in clone["children"]:
            if c.get("OTIO_SCHEMA", "").startswith("Clip"):
                nid += 1
                c["metadata"]["kt"]["id"] = nid
        doc["tracks"]["children"].append(clone)
    json.dump(doc, open(dest, "w"), indent=4)
    return dest

SEC = 90000
OWNER_FILE = "/media/e/developer/kt.9/sources/kt-untitled-review-870s.mp4"


def long_media(out, seconds):
    """Длинный H.264 1080p: кадр дорогой, как у владельца."""
    path = os.path.join(out, f"long{seconds}.mp4")
    if os.path.exists(path):
        return path
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-f", "lavfi", "-i", f"testsrc=size=1920x1080:rate=30:duration={seconds}",
            "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000",
            "-shortest",
            "-c:v", "libx264", "-preset", "veryfast", "-g", "60", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "128k",
            path,
        ],
        check=True,
    )
    return path


def run_case(app, name, dur_s, jumps):
    """Прыжки при игре: после каждого звук обязан пойти без голодания.

    Прыгаем МАСКОЙ ТАЙМКОДА (Ctrl+L), а не кликом по линейке: монтаж в
    320 с шире окна, и `x_at` отдал бы координату за экраном — ровно та
    ловушка, что записана в Р.37. Маска — настоящий человеческий способ
    попасть в произвольное место."""
    app.key("space")
    pause(
        1.5,
        "воспроизведение идёт в РЕАЛЬНОМ времени: полторы секунды игры нужны, "
        "чтобы курсор реально уехал от начала",
    )
    base = app.geom().get("underruns", 0)
    worst = 0
    for frac in jumps:
        secs = int(dur_s * frac)
        app.key("ctrl+l")
        for ch in str(secs):
            app.key(ch)
        app.key("Return")
        # звук обязан пойти БЫСТРО: даём вдвое больше кольца вывода
        pause(
            1.2,
            "кольцо вывода живёт в РЕАЛЬНОМ времени: ждём вдвое больше его "
            "длины, чтобы отличить «пошло быстро» от «пошло вообще»",
        )
        now = app.geom().get("underruns", 0)
        grew = now - base
        worst = max(worst, grew)
        base = now
        print(f"      {name}: прыжок на {frac:.0%} — underrun'ов прибавилось {grew}")
    app.key("space")
    # порог: полсекунды тишины (24 000 кадров) — это уже слышно; до Р.63
    # каждый прыжок давал 48 000 кадров в секунду и так по нескольку секунд
    app.expect(
        worst < 24_000,
        f"{name}: прыжок не глушит звук (худший прирост underrun'ов {worst})",
    )


def main(out):
    require_tools()
    dur = 320
    media = long_media(out, dur)
    path = project_with_clips(os.path.join(out, "seekjump.otio"), [(0.0, float(dur), media)])
    stack_tracks(path, 2)  # три слоя одного медиа: заказ шире, спан длиннее
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("seekjump", out, project=path) as app:
        app.expect(len(app.live_clips()) >= 1, f"событие на монтаже: {app.live_clips()}")
        run_case(app, "синтетика 320 с", dur, [0.9, 0.5, 0.75])
        app.shot("after-jumps")

    if not os.path.exists(OWNER_FILE):
        print("   (файла владельца нет на этой машине — второй прогон пропущен)")
        return 0

    # ТОТ САМЫЙ файл, на котором дефект и наблюдался
    path2 = project_with_clips(os.path.join(out, "owner.otio"), [(0.0, 870.0, OWNER_FILE)])
    stack_tracks(path2, 2)
    with Live("seekjump-owner", out, project=path2) as app:
        run_case(app, "review 870 с", 870, [0.94, 0.6, 0.2])
        app.shot("owner-after-jumps")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/seekjump"))
