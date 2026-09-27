#!/usr/bin/env python3
"""Живой сценарий Р.59 (стадия 3): новостной шаблон целиком, руками.

Претензия владельца дословно: «кто-то создал проект-шаблон для новостного
выпуска… очевидно не будут же люди каждый раз лезть в композицию и менять
там текст логотипы».

Харнесс-тесты t682-t685 проверяют достижимость и вёрстку в синтетической
сцене. Здесь вопрос другой: доходит ли путь ЦЕЛИКОМ настоящей мышью — от
правого клика по подписи внутри композиции до второго выпуска, у которого
свой заголовок, и обратно до правки шаблона, которую видят оба.

Шаги, каждый со своим наблюдаемым следствием:

1. собрать шаблон: титр и цветная плашка-логотип, свёрнутые в композицию;
2. ВНУТРИ композиции опубликовать заголовок (правый клик по полю текста)
   и ширину логотипа (правый клик по ПОДПИСИ поля) — в файле у ленты
   появляются две объявленные ручки;
3. поставить композицию ВТОРОЙ раз из бина;
4. у второго выпуска открыть «Параметры композиции» и поменять заголовок
   и ширину — в файле переопределения лежат ТОЛЬКО у него;
5. поправить заголовок в САМОМ шаблоне — правка доезжает до обоих: у
   первого выпуска меняется показанный текст (своего у него нет), у
   второго остаётся его собственный.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def walk(node):
    """Все узлы документа .otio — публикации и переопределения лежат в
    сайдкаре `metadata.kt`, а вложение в файле это Stack внутри трека."""
    if isinstance(node, dict):
        yield node
        for v in node.values():
            yield from walk(v)
    elif isinstance(node, list):
        for v in node:
            yield from walk(v)


def published_of(doc):
    """Объявленные ручки всех лент документа."""
    out = []
    for n in walk(doc):
        kt = n.get("metadata", {}).get("kt") if isinstance(n.get("metadata"), dict) else None
        if isinstance(kt, dict) and kt.get("published"):
            out.extend(kt["published"])
        if isinstance(n.get("published"), list) and n.get("published"):
            out.extend(n["published"])
    # один и тот же сайдкар встречается в обходе один раз, но подстрахуемся
    seen, uniq = set(), []
    for p in out:
        key = json.dumps(p, sort_keys=True, ensure_ascii=False)
        if key not in seen:
            seen.add(key)
            uniq.append(p)
    return uniq


def override_sets(doc):
    """Наборы переопределений всех событий — по одному списку на событие,
    у которого он непуст."""
    out = []
    for n in walk(doc):
        meta = n.get("metadata")
        if not isinstance(meta, dict):
            continue
        kt = meta.get("kt")
        if isinstance(kt, dict) and kt.get("overrides"):
            out.append(kt["overrides"])
    return out


def title_text(doc):
    """Текст титра ВНУТРИ шаблона — то, что правится один раз на всех."""
    for n in walk(doc):
        meta = n.get("metadata")
        if not isinstance(meta, dict):
            continue
        kt = meta.get("kt")
        if isinstance(kt, dict) and isinstance(kt.get("gen"), dict):
            g = kt["gen"]
            if "text" in g:
                return g["text"]
    return None


def leave_reel(app):
    """Наружу Esc-КАСКАДОМ: одна клавиша не делает два дела, поэтому
    выделение, полоса и выход снимаются по очереди (Р.55)."""
    for _ in range(5):
        if app.geom()["nest"]["depth"] == 0:
            return True
        app.key("Escape")
    return app.geom()["nest"]["depth"] == 0


def make_generator(app, group, kind, at_ticks):
    """Создать генератор из меню бина — тот же путь, что у человека."""
    win = geometry()
    b = app.geom()["bin"]
    app.click(app.x_at(at_ticks), app.ruler_y())
    app.right_click(
        win["x"] + int((b["left"] + b["right"]) / 2),
        win["y"] + int(b["bottom"]) - 10,
    )
    app.click_probe("gen.new", "пункт «Создать генератор»")
    app.hover_probe(f"gen.new.group.{group}", f"группа «{group}»")
    app.click_probe(f"gen.new.{kind}", f"вид «{kind}» в подменю")
    # созданный генератор открывает своё окно эффектов — закрываем, иначе
    # оно перекрывает бин и таймлайн
    app.key("Escape")


def video_row_ys(app):
    """Экранные y всех ВИДЕО-строк. Раскрытая полоса добавляет свои строки
    с номерами из хвоста числового пространства (Р.55), и угадывать их
    сценарию нечем — он их перебирает."""
    g = app.geom()
    win = geometry()
    return [
        win["y"] + int(r["top"] + r.get("h", g["lane_h"]) * 0.5)
        for r in g["rows"]
        if r["kind"] == "video"
    ]


def open_window_in_rows(app, x, probe, avoid=None, limit=6):
    """Открыть окно эффектов события, строка которого заранее неизвестна.

    ПУНКТОМ МЕНЮ, а не двойным кликом: внутри раскрытой полосы двойной
    клик — жест самой полосы (сворачивание, Р.55), и сценарий закрывал бы
    то, что сам же открыл. `avoid` — строка экземпляра, её не трогаем."""
    for yy in video_row_ys(app)[:limit]:
        if avoid is not None and abs(yy - avoid) < 12:
            continue
        app.right_click(x, yy)
        if app.has_probe("tl.fx", tries=2):
            app.click_probe("tl.fx", "пункт «Эффекты…» события")
            if app.has_probe(probe, tries=4):
                return True
        app.key("Escape")
    return False


def main(out):
    require_tools()
    path = os.path.join(out, "nestparam.otio")
    proj = project_with_clips(path, [(0.0, 2.0, "synth:Ramp")])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestparam", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)

        # --- 1. шаблон: титр и цветная плашка ---------------------------
        make_generator(app, "titles", "text", 3 * SEC)
        make_generator(app, "shapes", "rect", 6 * SEC)
        app.shot("built")
        clips = app.live_clips()
        app.expect(len(clips) == 3, f"генераторы не встали на монтаж: {clips}")

        gens = sorted(c for c in clips if c[0] >= 3 * SEC)
        app.click(app.x_at(gens[0][0] + gens[0][1] // 2), y)
        app.click(app.x_at(gens[1][0] + gens[1][1] // 2), y, mods=("ctrl",))
        app.right_click(app.x_at(gens[1][0] + gens[1][1] // 2), y)
        app.click_probe("nest.collapse", "пункт сворачивания в композицию")
        app.expect(
            len(app.live_clips()) == 2,
            f"шаблон не свернулся в одну композицию: {app.live_clips()}",
        )

        # --- 2. публикация ВНУТРИ композиции ----------------------------
        inst = max(app.live_clips())
        inst_x = app.x_at(inst[0] + inst[1] // 2)
        app.right_click(inst_x, y)
        app.click_probe("nest.enter", "пункт «Войти в ленту»")
        app.expect(app.geom()["nest"]["depth"] == 1, "вход внутрь композиции не состоялся")

        inner = sorted(app.live_clips())
        # ЗАГОЛОВОК — на самом поле текста: подписи у многострочного поля
        # нет и заводить её ради одной надписи нельзя (правило 13)
        app.double_click(app.x_at(inner[0][0] + inner[0][1] // 2), y)
        app.expect(app.has_probe("titlew.text"), "окно титра внутри композиции открылось")
        app.right_click_probe("titlew.text", "поле текста титра")
        app.click_probe("pub.publish.text", "пункт «Опубликовать…» у заголовка")
        app.expect(app.has_probe("pub.dialog.name"), "диалог имени ручки открылся")
        app.shot("publish-dialog")
        app.click_probe("pub.dialog.ok", "подтверждение публикации")
        app.key("Escape")

        # ШИРИНА ЛОГОТИПА — правым кликом по ПОДПИСИ числового поля
        app.double_click(app.x_at(inner[1][0] + inner[1][1] // 2), y)
        app.right_click_probe("lbl.w", "подпись ширины плашки")
        app.click_probe("pub.publish.w", "пункт «Опубликовать…» у ширины")
        app.click_probe("pub.dialog.ok", "подтверждение публикации ширины")
        app.key("Escape")
        app.shot("published")

        saved = app.saved_project()
        pubs = published_of(saved.doc)
        app.expect(
            len(pubs) == 2,
            f"объявленных ручек в файле не две: {[p.get('name') for p in pubs]}",
        )
        ids = {p["target"]["param"]: p["id"] for p in pubs}
        app.expect("text" in ids and "w" in ids, f"в файле не те ручки: {ids}")

        # --- 3. второй выпуск -------------------------------------------
        app.expect(leave_reel(app), "Esc-каскад вывел наружу")
        app.click(app.x_at(10 * SEC), app.ruler_y())
        card = next((f for f in app.geom().get("fields", []) if f["param"] == "bin.reel"), None)
        app.expect(card is not None, "карточка композиции в бине показана")
        if card:
            win = geometry()
            app.right_click(
                win["x"] + int(card["x"] + card["w"] / 2),
                win["y"] + int(card["y"] + card["h"] / 2),
            )
            app.click_probe("bin.place", "пункт «Поставить» у карточки композиции")
        places = app.live_clips()
        app.expect(len(places) == 3, f"второй выпуск не встал: {places}")

        # --- 4. у второго выпуска СВОИ значения -------------------------
        second = max(places)
        # целимся в НАЧАЛО второго выпуска, а не в середину: экземпляр
        # длиной 8 с встаёт под курсор на 10-й секунде, и его середина
        # (14 с) при 100 px/s лежит ЗА правой кромкой окна — правый клик
        # уходил в корневое меню WM, а сценарий валился каскадом
        # («`x_at` умеет только то, что НА ЭКРАНЕ», Р.37)
        sx = app.x_at(second[0] + app.seconds(1))
        win_right = app.geom()["window"]["w"] + geometry()["x"]
        app.expect(sx < win_right - 8, f"точка клика внутри окна: {sx} < {win_right}")
        app.click(sx, y)
        app.right_click(sx, y)
        app.click_probe("pub.inst_open", "пункт «Параметры композиции»")
        app.shot("instance-window")
        app.expect(
            app.has_probe(f"inst.value.{ids['text']}"),
            "строка заголовка в окне параметров композиции",
        )
        app.click_probe(f"inst.value.{ids['text']}", "поле заголовка второго выпуска")
        for ch in ["v", "t", "o", "r", "o", "y"]:
            app.key(ch)
        # ширина: клик по DragValue переводит его в ввод числом
        app.click_probe(f"inst.value.{ids['w']}", "поле ширины второго выпуска")
        for ch in ["ctrl+a", "0", "period", "9"]:
            app.key(ch)
        app.key("Return")
        app.key("Escape")

        # --- 4b. КРИВАЯ ручки: заводится названным действием и ВИДНА ----
        # Претензия Р.60: «кривых автоматизаций я нигде не вижу… ни одной
        # кнопки в окне про автоматизацию нет». Живьём проверяем ровно
        # это: у действия есть имя, после него в окне появляется виджет,
        # а ключи уезжают в файл.
        app.right_click(sx, y)
        app.click_probe("pub.inst_open", "окно параметров композиции ещё раз")
        app.expect(
            not app.has_probe(f"inst.curve.{ids['w']}"),
            "в окне остался безымянный глиф записи ключей",
        )
        app.click_probe("inst.auto_add", "меню «+ кривая»")
        app.click_probe(f"inst.auto_add.{ids['w']}", "пункт «ширина» в меню кривой")
        app.expect(app.has_probe("inst.auto"), "виджет автоматизации появился в окне")
        app.shot("instance-curve")
        app.key("Escape")

        saved = app.saved_project()
        sets = override_sets(saved.doc)
        curved = [
            o
            for st in sets
            for o in st
            if o["pub_id"] == ids["w"] and o.get("curves")
        ]
        app.expect(
            len(curved) == 1,
            f"кривая ручки не доехала до файла: {sets}",
        )
        keys = curved[0]["curves"][0].get("keys", []) if curved else []
        app.expect(
            len(keys) >= 2,
            f"стартовая кривая не легла константой на весь материал: {keys}",
        )
        app.expect(
            len(sets) == 1,
            f"переопределения лежат не у одного события: наборов {len(sets)}",
        )
        app.expect(
            bool(sets) and len(sets[0]) == 2,
            f"второй выпуск переопределил не обе ручки: {sets}",
        )
        app.shot("second-differs")

        # --- 5. правка ШАБЛОНА догоняет оба выпуска ----------------------
        was = title_text(saved.doc)
        app.expect(was is not None, f"текст титра шаблона найден в файле: {was!r}")
        # ПЕРВЫЙ ВЫПУСК — экземпляр той же ленты, а не исходный клип
        # проекта: `min` по всему монтажу взял бы его и щёлкнул мимо
        first = min(c for c in app.live_clips() if c[1] == second[1])
        fx = app.x_at(first[0] + first[1] // 2)
        app.right_click(fx, y)
        app.click_probe("nest.enter", "вход в композицию через ПЕРВЫЙ выпуск")
        inner = sorted(app.live_clips())
        app.double_click(app.x_at(inner[0][0] + inner[0][1] // 2), y)
        app.click_probe("titlew.text", "поле текста титра шаблона")
        for ch in ["z", "z"]:
            app.key(ch)
        app.key("Escape")
        app.expect(leave_reel(app), "Esc-каскад вывел наружу после правки шаблона")

        saved = app.saved_project()
        now = title_text(saved.doc)
        app.expect(
            now != was,
            f"правка шаблона не записалась: было {was!r}, стало {now!r}",
        )
        sets = override_sets(saved.doc)
        app.expect(
            len(sets) == 1 and len(sets[0]) == 2,
            f"правка шаблона задела переопределения второго выпуска: {sets}",
        )
        # у ПЕРВОГО выпуска своего заголовка нет — он показывает новый
        # текст шаблона; у ВТОРОГО свой, и он остался
        own = [o for o in sets[0] if o["pub_id"] == ids["text"]]
        app.expect(len(own) == 1, f"переопределение заголовка второго выпуска пропало: {sets}")
        # значение переопределения лежит тегированным (`{"t":…,"v":…}`)
        app.expect(
            own and own[0]["value"].get("v") != now,
            f"второй выпуск сравнялся с шаблоном: {own} против {now!r}",
        )
        app.shot("template-edited")

        # --- 6. ПУБЛИКАЦИЯ ИЗ ПОЛОСЫ не тащит внутрь (Р.63 п.11) --------
        # Владелец: «если опубликовать параметр не входя полностью в
        # композицию — то зачем-то мы автоматически полностью входим в
        # композицию». Раскрываем полосу (двойной клик) и публикуем
        # ОТТУДА: глубина входа обязана остаться нулевой.
        first = min(c for c in app.live_clips() if c[1] == second[1])
        fx0 = app.x_at(first[0] + app.seconds(1))
        rows_before = len(video_row_ys(app))
        # раскрытие — ПУНКТОМ МЕНЮ, а не двойным кликом: у двойного клика
        # тот же эффект, но он же и сворачивает, и сценарий, промахнувшись
        # строкой, закрыл бы полосу молча
        app.right_click(fx0, y)
        app.click_probe("nest.expand_here", "пункт «Раскрыть здесь»")
        app.settle()
        app.expect(
            len(video_row_ys(app)) > rows_before,
            f"полоса раскрылась: строк было {rows_before}, стало {len(video_row_ys(app))}",
        )
        app.expect(
            app.geom()["nest"]["depth"] == 0,
            "полоса раскрыта, а человек остался в корне",
        )
        app.expect(
            open_window_in_rows(app, fx0, "titlew.text", avoid=y),
            "окно титра открылось ИЗ ПОЛОСЫ",
        )
        app.right_click_probe("lbl.size", "подпись кегля титра в полосе")
        app.click_probe("pub.publish.size", "пункт «Опубликовать…» у кегля")
        app.click_probe("pub.dialog.ok", "подтверждение публикации из полосы")
        app.expect(
            app.geom()["nest"]["depth"] == 0,
            "подтверждение публикации утащило внутрь композиции (п.11)",
        )
        app.shot("published-from-band")
        saved = app.saved_project()
        pubs = published_of(saved.doc)
        app.expect(
            len(pubs) == 3,
            f"ручка из полосы не легла в ленту: {[p.get('name') for p in pubs]}",
        )

        # --- 7. п.10: РУЧКА ЭФФЕКТА у экземпляра меняет КАДР ------------
        # «накинул Radial blur, опубликовал параметр количества, поменял
        # его на композиции — а оно не меняется».
        app.click_probe("fxreg.add", "пикер эффектов в окне титра")
        for ch in "radial":
            app.key(ch)
        app.click_probe("fxreg.tpl.radial_blur", "строка «Радиальное размытие» в пикере")
        # у титра выбрана ГОЛОВА (его источник), поэтому новый слот надо
        # ещё и выбрать — полосой, как человек. Нулевой слот полосы — это
        # ВЕДУЩИЙ УЗЕЛ «Геометрия» (он вклеен в полосу на свою позицию,
        # Р.62), размытие встало следом.
        app.click_probe("fx.slot.1", "слот размытия в полосе цепочки")
        app.shot("blur-added")
        app.expect(app.has_probe("lbl.amount"), "параметры размытия показаны")
        app.right_click_probe("lbl.amount", "подпись количества размытия")
        app.click_probe("pub.publish.amount", "пункт «Опубликовать…» у количества")
        app.click_probe("pub.dialog.ok", "подтверждение публикации количества")
        app.key("Escape")
        saved = app.saved_project()
        pubs = published_of(saved.doc)
        amount = next((p["id"] for p in pubs if p["target"]["param"] == "amount"), None)
        app.expect(amount is not None, f"ручка количества не объявлена: {pubs}")

        # полосу сворачиваем — тем же пунктом меню (правило 14: жест
        # обязан работать ПОВТОРНО)
        app.right_click(fx0, y)
        app.click_probe("nest.expand_here", "пункт «Раскрыть здесь» второй раз")
        app.settle()

        # курсор — на второй выпуск, запоминаем кадр
        app.click(app.x_at(second[0] + app.seconds(1)), app.ruler_y())
        app.settle()
        before_px = app.pixel_settled(app.preview_center())
        app.shot("before-amount")

        app.click(sx, y)
        app.right_click(sx, y)
        app.click_probe("pub.inst_open", "пункт «Параметры композиции»")
        # ГОЛОВА ПОЛОСЫ, а не своё окно (Р.63 п.12)
        app.expect(app.has_probe("fx.head"), "параметры композиции — голова полосы цепочки")
        app.expect(
            amount is not None and app.has_probe(f"inst.value.{amount}"),
            "ручки количества нет в голове полосы",
        )
        app.click_probe(f"inst.value.{amount}", "поле количества у второго выпуска")
        for ch in ["ctrl+a", "0", "period", "9"]:
            app.key(ch)
        app.key("Return")
        app.settle()
        app.shot("after-amount")
        after_px = app.pixel_settled(app.preview_center(), differs_from=before_px)
        app.expect(
            after_px != before_px,
            f"кадр второго выпуска не изменился от ручки размытия: {before_px} → {after_px}",
        )
        saved = app.saved_project()
        sets = override_sets(saved.doc)
        app.expect(
            any(o["pub_id"] == amount for st in sets for o in st),
            f"переопределение количества не доехало до файла: {sets}",
        )

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
