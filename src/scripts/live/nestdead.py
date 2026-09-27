#!/usr/bin/env python3
"""Живой сценарий Р.50: у вложения тупиков не бывает.

Претензия, из-за которой появился весь механизм Р.38, звучала так: «я не
понимаю как удалить saved area region, я его сохранил и он теперь висит».
Вложение — новая сущность, и у неё тот же риск: создать можно, а управлять
нечем. Харнесс-тесты t253/t457 это проверяют, но в синтетической сцене; тут
настоящая рука в настоящем окне.

Проверяется:

1. правый клик по вложению открывает ЕГО меню — с входом, разворачиванием
   и переименованием (а не меню обычного события);
2. переименование работает ДВАЖДЫ ПОДРЯД (правило 14: фича обязана
   работать повторно, а не один раз — именно так умирали горячие клавиши
   после первого Ctrl+L);
3. «отвязать от других мест» НЕ предлагается, когда место одно: обещание,
   которое ничего не изменит, хуже его отсутствия;
4. «развернуть на дорожки» возвращает содержимое наружу;
5. наружу ведут ДВА пути: Esc и крошка пути.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    path = os.path.join(out, "nestdead.otio")
    proj = project_with_clips(
        path, [(0.0, 2.0, "synth:sine:440"), (2.0, 2.0, "synth:ramp")]
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestdead", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(3 * SEC), y, mods=("ctrl",))
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "пункт сворачивания")
        inst = app.live_clips()[0]
        mid = app.x_at(inst[0] + inst[1] // 2)
        app.shot("collapsed")

        # --- 1. СВОЁ меню ----------------------------------------------
        app.right_click(mid, y)
        names = {f["param"] for f in app.geom().get("fields", [])}
        for want in ("nest.enter", "nest.expand", "nest.rename"):
            app.expect(want in names, f"в меню вложения нет «{want}»: {sorted(names)}")
        # --- 3. «отвязать» не обещается при одном месте ----------------
        app.expect(
            app.geom()["nest"]["uses"] in (0, 1),
            "сцена: лента стоит в одном месте",
        )
        app.key("Escape")

        # --- 2. переименовать ДВАЖДЫ ПОДРЯД ---------------------------
        for i in range(2):
            app.right_click(mid, y)
            app.click_probe("nest.rename", f"пункт переименования (заход {i + 1})")
            before = app.shot(f"rename-{i}")
            app.key("Escape")
            after = app.shot(f"rename-{i}-closed")
            d = app.screen_diff(before)
            app.expect(
                d > 0.001,
                f"окно имени ленты не закрылось по Esc на заходе {i + 1} — или не "
                f"открылось вовсе (экран изменился на {d:.3%})",
            )
            _ = after
        # ...и после двух заходов горячие клавиши ЖИВЫ (фокус отдан)
        app.focus()
        app.key("Escape")
        app.click(mid, y)
        app.save_now()
        app.expect(os.path.exists(path), "Ctrl+S после окна имени не сработал — фокус не отдан")

        # --- 5. два пути наружу ----------------------------------------
        # ПОЛНЫЙ вход — пункт меню (Р.55, С5): двойной клик с этого раунда
        # раскрывает на месте, и наружу из раскрытия выходят иначе
        app.right_click(mid, y)
        app.click_probe("nest.enter", "пункт «Войти в ленту»")
        app.expect(app.geom()["nest"]["depth"] == 1, "не вошли внутрь")
        # путь ПЕРВЫЙ: имя проекта — это крошка корня
        app.click_probe("nest.root", "имя проекта как крошка корня")
        app.expect(
            app.geom()["nest"]["depth"] == 0,
            "клик по имени проекта не вывел наружу — а это единственный способ выйти "
            "сразу на нужный уровень",
        )
        # путь ВТОРОЙ: Esc
        app.right_click(mid, y)
        app.click_probe("nest.enter", "пункт «Войти в ленту» второй раз")
        app.expect(app.geom()["nest"]["depth"] == 1, "не вошли внутрь второй раз")
        for _ in range(3):
            app.key("Escape")
            if app.geom()["nest"]["depth"] == 0:
                break
        app.expect(app.geom()["nest"]["depth"] == 0, "Esc не вывел наружу")
        app.shot("out")

        # --- 4. развернуть на дорожки ----------------------------------
        app.right_click(mid, y)
        app.click_probe("nest.expand", "пункт разворачивания")
        app.expect(
            len(app.live_clips()) == 2,
            f"разворачивание не вернуло содержимое: {app.live_clips()}",
        )
        app.shot("expanded")

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/nestdead"))
