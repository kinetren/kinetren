#!/usr/bin/env python3
"""Живой сценарий Р.58 (п.462): внутри полного входа событие уезжает ЗА НОЛЬ.

Претензия владельца дословно: «При редактировании полностью открытой
композиции я не могу закинуть событие за ее пределы, а при раскрытой
композиции в root - могу. Воспроизводить в полностью открытой композиции зону
до 0 мне не надо».

Стена стояла только в интерфейсе (`.max(0)` над тиком жеста). Решение
владельца: зону до нуля ПОКАЗЫВАТЬ с вуалью, курсор и воспроизведение туда не
пускать. Харнесс-тесты t637/t637b держат правило; здесь — настоящая мышь.

Шаги:

1. свернуть среднее событие в ленту, войти в неё двумя Enter;
2. утащить событие ленты левее нуля — start < 0 в живой модели;
3. клик по дорожке левее нуля ставит курсор в 0, а не левее;
4. Esc наружу и Enter обратно — событие на месте.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    path = os.path.join(out, "nestzero.otio")
    proj = project_with_clips(
        path,
        [(0.0, 2.0, "synth:sine:440"), (2.0, 2.0, "synth:ramp"), (6.0, 2.0, "synth:Ramp")],
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestzero", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)

        # --- 1. свернуть СРЕДНЕЕ событие и войти -------------------------
        app.click(app.x_at(3 * SEC), y)
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "пункт сворачивания")
        app.click(app.x_at(3 * SEC), y)
        app.key("Return")
        app.key("Return")
        g = app.geom()
        app.expect(g["nest"]["depth"] == 1, f"не вошли в ленту: глубина {g['nest']['depth']}")
        inner = min(app.live_clips())
        app.expect(inner[0] == 0, f"содержимое ленты обязано начинаться в нуле: {inner}")

        # --- 2. утащить событие ЗА НОЛЬ ----------------------------------
        # хват на 1 с от начала, бросок на 0.3 с: начало уходит на −0.7 с.
        # Точка броска правее нуля — до Р.58 сюда же бросали и получали 0
        y = app.lane_y("video", 0, 0.6)
        app.drag(app.x_at(1 * SEC), y, app.x_at(int(0.3 * SEC)), y)
        moved = min(app.live_clips())
        app.shot("crossed")
        app.expect(
            moved[0] < 0,
            f"внутри ленты событие обязано уезжать левее нуля, а оно на {moved}",
        )
        app.near(moved[0], -int(0.7 * SEC), 0.08 * SEC, "start после броска")

        # --- 3. курсор ЛЕВЕЕ нуля не ставится -----------------------------
        # зона до нуля видна (пол вида ниже нуля), клик по ней — курсор в 0
        g = app.geom()
        x_before_zero = app.x_at(-int(0.2 * SEC))
        app.expect(
            x_before_zero > app.x_at(g["view_start"]),
            "зона до нуля обязана быть на экране внутри ленты",
        )
        app.click(x_before_zero, app.lane_y("video", 0, 0.8))
        app.expect(
            app.playhead() == 0,
            f"клик левее нуля обязан ставить курсор в 0, а не в {app.playhead()}",
        )

        # --- 4. наружу и обратно — событие на месте -----------------------
        # Esc — каскад (снять выделение → свернуть полосу → выйти), жмём,
        # пока не окажемся в корне; клик по экземпляру и Enter-каскад обратно
        for _ in range(4):
            if app.geom()["nest"]["depth"] == 0:
                break
            app.key("Escape")
            poll_step(0.35)
        g = app.geom()
        app.expect(g["nest"]["depth"] == 0, f"Esc не вывел наружу: глубина {g['nest']['depth']}")
        app.click(app.x_at(3 * SEC), app.lane_y("video", 0, 0.6))
        for _ in range(2):
            app.key("Return")
            poll_step(0.7)
            if app.geom()["nest"]["depth"] == 1:
                break
        g = app.geom()
        app.expect(g["nest"]["depth"] == 1, f"Enter не вернул внутрь: глубина {g['nest']['depth']}")
        back = min(app.live_clips())
        app.shot("re-entered")
        app.expect(back[0] == moved[0], f"после выхода и входа событие уехало: {back} против {moved}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
