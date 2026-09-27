#!/usr/bin/env python3
"""Живой сценарий Р.66: КОНТРАКТ ФОРМЫ в настоящем окне.

Прямая просьба владельца (Р.64): «и не забудь на будущее во всех других
виджетах и окнах тоже пройтись надо будет». Харнесс держит контракт
машиной (t779, t780), но «видно ли кнопку и жмётся ли она на невысоком
экране» он показать не может: окно egui в харнессе зажимается своим
прямоугольником, а тут окно настоящее.

Сценарий:

1. уменьшает окно программы до низкого (1100×640);
2. открывает окно эффектов двойным кликом по событию;
3. убеждается, что кнопки действия ВИДНЫ внутри окна и нажимаются;
4. сворачивает группу «Параметры» и видит, что поля исчезли, а после
   повторного клика вернулись — свёртка живёт и ничего не ломает.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, require_tools  # noqa: E402

SEC = 90000


def probes(app):
    return {f["param"]: f for f in app.geom().get("fields", [])}


def main(out):
    require_tools()
    path = os.path.join(out, "forms.otio")
    subprocess.run([KT_CLI, "demo", path, "--force"], check=True, capture_output=True)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("forms", out, project=path) as app:
        # 1) НИЗКОЕ окно — там, где кнопки и уезжают за кромку
        app.resize(1100, 640)
        g = app.geom()
        print(f"   окно {g['window']['w']:.0f}×{g['window']['h']:.0f}")

        # 2) окно эффектов двойным кликом по событию
        y = app.lane_y("video", 0, frac=0.5)
        app.double_click(app.x_at(1 * SEC), y)
        app.expect(app.has_probe("fx.cancel"), "окно эффектов открылось")
        app.shot("fx-low")

        # 3) кнопки ДЕЙСТВИЯ видны внутри окна и жмутся
        f = probes(app)
        win = geometry()
        for name in ("fx.done", "fx.cancel"):
            r = f.get(name)
            app.expect(r is not None, f"кнопка «{name}» отдана программой")
            if r:
                inside = 0 <= r["y"] and r["y"] + r["h"] <= g["window"]["h"]
                app.expect(inside, f"кнопка «{name}» видна в окне (низ {r['y'] + r['h']:.0f})")

        # 4) группа сворачивается и разворачивается
        grp = next((k for k in f if k.startswith("grp.")), None)
        app.expect(grp is not None, "у формы есть свёртываемая группа")
        if grp:
            before = len(probes(app))
            app.click_probe(grp, "заголовок группы")
            folded = len(probes(app))
            app.expect(folded < before, f"свёртка спрятала поля: было {before}, стало {folded}")
            app.click_probe(grp, "заголовок группы (обратно)")
            back = len(probes(app))
            app.expect(back >= before - 1, f"разворот вернул поля: {back} против {before}")
        app.shot("fx-folded")

        # кнопка «Отмена» действительно закрывает окно
        app.click_probe("fx.cancel", "кнопка «Отмена» окна эффектов")
        app.expect(not app.has_probe("fx.cancel", tries=4), "окно закрылось по кнопке")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out")
