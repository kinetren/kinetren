---
id: F-UI-001
title: Экранные контролы во вьюере: рамка трансформации, ручки, разметка
status: сделано
priority: B
cost: M
category: ui/viewer
editors: [after-effects-26, premiere-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: да
crates: [kt-app, kt-engine, kt-gpu, kt-model]
invariants: [7, 12]
ui_rules: [1, 2, 3, 6, 9, 10, 13, 15, 16, 17, 20]
round: "Р.64"
point: "change geometry-on-screen-controls (летопись заморожена на §470)"
tests: [759, 760, 761, 762, 763, 764, 765, 766]
depends_on: []
checked: 2026-09-25
---

# F-UI-001 — Экранные контролы во вьюере: рамка трансформации, ручки, разметка

## Суть

Правка положения, масштаба и поворота картинки прямо в окне превью:
вокруг слоя рисуется рамка с восемью ручками, в её центре — точка
привязки, сбоку — ручка вращения; кроп тянется своими ручками, маска —
точками с безье-касательными, движение по ключам показано траекторией.
Поверх кадра же лежит разметка, которая никуда не рендерится: сетка,
направляющие, центр, безопасные зоны для титров и действия. И там же
живут режимы сравнения — «до/до» и «до/после» в одном кадре, разделённом
шторкой. Всё это переключается кнопкой «показать/скрыть»: контролы лежат
ровно на том, ради чего человек в кадр и смотрит.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| After Effects | 26.x | Layer Controls, Anchor Point (Pan Behind) tool, Motion Path, Guides / Proportional Grid / Title-Action Safe | ЭТАЛОН. Контролы есть, пока слой выделен: рамка, якорь «прицелом», касательные масок, траектория движения с ручками. Якорь двигается ОТДЕЛЬНЫМ инструментом (`Y`), и это единственный жест, правящий два параметра сразу (цитата ниже). Разметка — линейки, направляющие, пропорциональная сетка, title/action safe (Preferences → Grids & Guides). Всё гасится одной командой `View > Show Layer Controls` (Ctrl+Shift+H), состав — `View Options` (Ctrl+Alt+U). При проигрывании контролы гасятся САМИ, и Adobe объясняет зачем (цитата ниже) |
| Adobe Premiere | 26.x | Motion / transform handles в Program Monitor, Comparison View, Safe Margins | Контролы появляются только когда в Effect Controls ВЫДЕЛЕНО слово Motion (или другой эффект с пространственным параметром) — отсюда классическая претензия «не могу их вызвать» (цитата ниже). Второй способ — двойной клик в Program Monitor. Safe Margins — в меню-гаечнике монитора. Сравнение вынесено в Comparison View: Side by Side, Vertical Split, Horizontal Split — сравниваются КАДРЫ монтажа, а не «с эффектом / без» |
| DaVinci Resolve | 21 | Transform / Crop / Dynamic Zoom (Timeline Viewer), View → Viewer Overlay, Safe Area, Split Screen + Image Wipe (Color) | Контролы включаются кнопкой «Transform/Crop/Dynamic Zoom button at the bottom left of the Timeline Viewer», активен ровно ОДИН набор: «These on-screen controls can also be selected by choosing an option from the View > Viewer Overlay submenu». Выбранный оверлей гасится `Shift-\`` либо пунктом Show Controls в меню вьюера. Safe-зоны — `View > Safe Area`, с отдельными переключателями частей. Сравнение — развитый инструмент страницы Color: панель Wipe / Split Screen / Highlight, шторка с кадром из галереи или другой ВЕРСИЕЙ грейда |
| Final Cut Pro | 12.3 | Onscreen controls (Transform Shift-T / Crop Shift-C / Distort / Reorient), Title & Action Zones, Comparison Viewer | «Many effects, transitions, and other items use onscreen controls, superimposed over the video in the viewer». Включаются меню в ЛЕВОМ нижнем углу вьюера; разметка — в ПРАВОМ верхнем (`Show Title/Action Zones`, title-safe = внутренние 80 %, action-safe = 90 %, «Output (exported) media files are not affected»). Угловые ручки держат пропорции, боковые их меняют, белый кружок ПОКАЗЫВАЕТ якорь, синяя ручка вращения физически растёт ИЗ этого кружка. Про якорь Apple пишет прямо: «Anchor X and Y… **This control isn't available in the onscreen controls**». Сравнения шторкой нет вовсе — вместо него ВТОРОЙ вьюер рядом (Comparison Viewer, Control-Command-6, режимы Saved / Timeline) |
| VEGAS Pro | 2026.0 | Event Pan/Crop (отдельное окно), Overlays, Split-Screen View, Crop OFX (interact controls) | Трансформации СОБЫТИЯ в главном превью нет: Pan/Crop — своё окно со своим холстом и рамкой поверх исходного кадра (там же и своя точка вращения — «drag the point at the center of the selection point»), а результат смотрят в другом окне. В самом Video Preview — только разметка (Overlays: Grid, Safe Areas «outer border… visible on a television screen», внутренняя для титров, изоляция каналов, 360°) и Split-Screen View: сравнение «FX Bypassed» либо «Clipboard», причём область сравнения — ПРОИЗВОЛЬНЫЙ прямоугольник, который тянут мышью (пресеты Select Left/Right Half, Select All). Исключение, которое легко пропустить: OFX-плагины ручки в превью рисуют — документация 2026 противопоставляет «Crop OFX… offers an intuitive way to crop using the **Video Preview interact controls**» внутреннему Pan/Crop с его отдельным окном |
| Kinotrain | — | нет | Поверх кадра нет ни одного интерактивного элемента, кроме одного `ui.interact` с `Sense::click()` (play/pause и двойной клик — отделить окно). Рисуется рамка в 1 px, скрим «Загрузка» и ambient-glow. Ни ручек, ни якоря, ни сетки, ни safe-зон, ни сравнения. При этом параметры под контрол уже есть: у `Pan3d` — pos/rot/scale И `anchor_x`/`anchor_y` |

## Фишка фишки

**1. Точка привязки — единственная ручка, которая обязана менять ДВА
параметра сразу, и именно из-за этого её либо прячут, либо выносят в
отдельный инструмент; а компенсация ломается на повороте.** Позиция слоя
меряется ДО якоря, поэтому «просто сдвинуть якорь» невозможно: слой
поедет. After Effects решает это тем, что жест и число намеренно делают
РАЗНОЕ — «The Anchor Point tool, sometimes called the Pan Behind tool in
the toolbar, moves the Anchor Point around the layer without changing the
position», тогда как правка того же якоря числом в таймлайне «the
Position property does not change, so the layer moves» (обе цитаты —
Rick Gerard, ниже). То есть у самого известного редактора парные способы
одного действия УМЫШЛЕННО неэквивалентны — из-за этого новички годами не
понимают, почему «крутится не вокруг того». Apple пошла в другую сторону
и записала отказ прямо в справку: белый кружок якорь ПОКАЗЫВАЕТ, но
«This control isn't available in the onscreen controls» — единственный
параметр трансформации без экранного контрола, потому что перетаскивание
якоря столкнулось бы с жестом «тянуть внутри картинки = двигать её».
Vegas спрятал его ещё глубже: точка вращения есть, но живёт внутри
отдельного окна Pan/Crop. И у самой компенсации есть дно, о которое
бьются в форумах: она считается в НЕПОВЁРНУТОЙ системе координат, и
«It works perfectly until I rotate the layer, at which point when I move
the Anchor Point, the layer also shifts position» (цитата ниже). Для нас
это готовый тест-кейс: у `Pan3d` повороты идут порядком ZXY, и сдвиг
якоря обязан компенсироваться ТОЙ ЖЕ матрицей, иначе повторим чужой баг.

Побочная деталь того же узла, и лучшая находка по эргономике: у FCP
ручка вращения — синий «стебель», физически ВЫРАСТАЮЩИЙ из кружка
якоря, поэтому связь «эта ручка крутит вокруг вот этой точки» видна без
объяснений. У Resolve ручка вращения лежит в центре изображения
отдельно — и в апреле 2026 продакт-менеджеру Blackmagic пришлось лично
объяснять пользователю, где она, потому что тот искал её по краям рамки,
как в фоторедакторах.

**2. Контрол живёт в координатах ЭКРАНА, параметр — в координатах КАДРА,
и когда между ними встаёт доля разрешения, ломается именно попадание
мышью.** Vegas выбрал радикальный вариант: убрал СВОЮ трансформацию из
превью в собственное окно с собственным холстом — и получил целый класс
жалоб вида «the image in the event pan/crop tool doesn't match up to the
image in the preview window or the render» (цитата ниже). Показательно,
что чужим OFX-плагинам он ручки в превью рисовать РАЗРЕШАЕТ: то есть
дело не в невозможности, а в том, что своя старая подсистема живёт в
другом пространстве координат и переезжать не стала. Цена другого варианта
видна у Premiere: контролы рисуются поверх настоящего кадра, но
показываются, только когда в Effect Controls выделен нужный эффект, —
и половина вопросов на форуме не про трансформацию, а про то, как её
вообще увидеть — вплоть до того, что у одного из спрашивающих ручки
вернулись только после переноса Program Monitor на другой монитор и
изменения его размера, а стандартный совет экспертов в таких тредах —
уменьшить ЗУМ монитора программы. Это самое близкое к нашей проблеме,
что вообще нашлось в чужих форумах: видимость и попадание в контрол
зависят от масштаба показа превью. Наш собственный код показывает, где
это ломается численно: превью в 1/16 от 1920×1080 — это 120×66
(округление до чётного режет 67.5 до 66), соотношение сторон 1.818
против 1.778, а `world_scale` в плане берётся ТОЛЬКО по ширине.

**3. «Сравнить до и после» — это не оверлей, а второй композит того же
тика, и «до» у всех означает разное.** У Vegas Split-Screen View
сравнивает кадр с ним же БЕЗ всех видео-эффектов либо с содержимым
БУФЕРА ОБМЕНА, и делит кадр не шторкой, а произвольным прямоугольником,
который тянут мышью. У Premiere Comparison View сравнивает два КАДРА
монтажа (Side by Side / Vertical Split / Horizontal Split) и про эффекты
не знает вовсе. У Resolve шторка на странице Color сравнивает с кадром из
галереи или с другой ВЕРСИЕЙ грейда, то есть «до» — это «до этого узла».
У FCP шторки нет вовсе: там второй вьюер рядом (Comparison Viewer). Одно
и то же название кнопки в четырёх редакторах отвечает на четыре разных
вопроса, и выбирать, на какой отвечаем мы, придётся отдельно.

Обратная сторона того же — переключатель «скрыть контролы», и он не
роскошь. Adobe гасит их при проигрывании САМА и объясняет прямым
текстом: «Layer Controls are intentionally hidden during preview playback
so masks and shape paths don't get in the way»; Apple — так же
автоматически и без права выбора. Resolve оставил это на человека — и
получил обе цены сразу: жалобу «there is no direct way to turn the on
screen controls on/off. You either need to look away from the screen to
find the Ctrl+k keys or go down two levels of right click menu» и
пользователей, покупающих ТРЕТИЙ монитор под чистый выход, чтобы видеть
кадр без оверлеев. Причём гасить надо не всё скопом: главный мусор — не
ручки, а траектория, потому что overlay рисует ключи ВСЕХ клипов сразу
(«there are so many line and control points that appear, i literally
cannot even see the video content under it all. It's like a birds nest»),
и запрос звучит как «hide the vectors and still be able to transform with
a mouse». Раздельного тумблера «ручки / траектория» нет ни у кого из
пяти.

## Плюсы

- Контрол не занимает места: он лежит поверх уже существующего кадра.
  Это единственный класс интерфейса, который добавляет действия, не
  отнимая ни строки, ни колонки (правило 13 чек-листа — прямой аргумент ЗА).
- Одно движение мыши выражает пару чисел сразу: точка на экране — это
  (x, y), а в сетке параметров это два поля и взгляд на кадр между ними.
- Масштаб жеста задаётся кадром, а не диапазоном числа: «поставить слой
  вот сюда» не требует знать, что позиция меряется в мировых пикселях,
  а якорь — в долях слоя.
- Ручка вращения показывает ЦЕНТР вращения тем, что нарисована вместе с
  якорем: невидимая связь «вокруг чего крутится» становится картинкой.
- Разметка (сетка, safe-зоны, центр) — не эффект: она не попадает в
  экспорт, не меняет ни одного числа и гасится целиком одной командой.
- Режим сравнения не знает, что сравнивает: это два композита одного
  тика, поэтому он работает на любом материале и любом наборе эффектов.
- У Vegas видно, что разметку можно завести ОТДЕЛЬНО от трансформации:
  Overlays в Video Preview есть, а ручек своей трансформации там нет. То
  есть фичу можно брать по частям, и разметка — самая дешёвая часть.

## Минусы

- Контролы закрывают то, ради чего человек смотрит в кадр. Отсюда у
  одних ручной выключатель (AE — Ctrl+Shift+H, Resolve — Show Controls),
  у других жёсткое автоматическое правило: «Onscreen controls are always
  hidden when you play clips in the timeline» (FCP) — там их не прячут по
  желанию, их прячут всегда.
- Точность жеста падает вместе с качеством превью: ручка в 8 экранных px
  на превью в 1/16 соответствует сотне пикселей кадра, и «поставить
  ровно 100 %» мышью невозможно в принципе — числовое поле остаётся
  обязательным.
- Один кадр обслуживает несколько инструментов (трансформация, кроп,
  маска, траектория), и они конкурируют за ту же мышь. Отсюда развилка:
  либо активен ровно один набор (Resolve, `View > Viewer Overlay`), либо
  контролы привязаны к выделению эффекта (Premiere) — и оба варианта
  порождают вопрос «где мои ручки».
- Каждый дополнительный «гейт видимости» умножает вопрос «где мои
  ручки»: у Resolve OFX-контрол виден, только если ОДНОВРЕМЕННО включён
  Open FX Overlay во вьюере и On-Screen Controls в инспекторе, и это
  стабильный источник обращений.
- Траектория движения — не украшение, а мусор: у Resolve оверлей
  трансформации рисует ключи не только текущего клипа, и через десяток
  ключей кадра под ними не видно вовсе. Значит выключатель нужен не
  один общий, а раздельный: ручки отдельно, траектория отдельно.
- Сдвиг якоря переписывает смысл ВСЕХ уже поставленных ключей вращения
  и масштаба; жест при этом один, и undo-шаг один. А компенсация,
  удерживающая картинку на месте, у эталона ломается на повёрнутом слое.
- Разделение «правка в своём окне, показ в главном» (Vegas) экономит
  всю математику попадания, но платит рассогласованием двух картинок —
  и это подтверждённая жалобами цена, а не теория.
- Перетаскивание требует, чтобы кадр пересобирался на каждое движение
  мыши: контрол без живой картинки бесполезен, а живая картинка — это
  полный перекомпозит кадра в темпе жеста.

## Что говорят пользователи

Классическая формулировка непонимания якоря — человек меняет число и
получает не то, чего ждал:

> Anchor Point is 612, 360 according to timeline data. When you change
> the Anchor Point value to 0, 0 only still moves not anchor point.
> — MahaMN, Adobe Community, 24.05.2022, https://community.adobe.com/t5/after-effects-discussions/anchor-point-issue/m-p/12978119

Ответ эксперта в том же треде и есть «фишка фишки» в одной строке —
инструмент и число делают РАЗНОЕ:

> The Anchor Point tool, sometimes called the Pan Behind tool in the
> toolbar, moves the Anchor Point around the layer without changing the
> position so that you can change the center of the scale and rotation
> properties.
> — Rick Gerard, Adobe Community, 25.05.2022, https://community.adobe.com/t5/after-effects-discussions/anchor-point-issue/m-p/12978119

И дно самой компенсации: инструмент держит картинку на месте ровно до
первого поворота слоя —

> the problem is that when the anchor point moves, the layer moves with
> it (the pan-behind tool won't help in this case, it only messes up the
> rest of the animation).
> — Greg Digenti, Creative COW, 19.06.2009, https://creativecow.net/forums/thread/animate-anchor-point-without-moving-layer/

> It works perfectly until I rotate the layer, at which point when I move
> the Anchor Point, the layer also shifts position.
> — там же, тот же тред (попытка скомпенсировать выражением)

Цена решения «контролы показываются только при выделенном эффекте»
(Premiere) — вопрос не про трансформацию, а про то, как её увидеть:

> Sometimes without even trying, transform handles appear in my program
> window. But whenever I want them, it's always a battle to make them
> show up.
> — Steve Zeeeee, Adobe Community, 08.08.2020, https://community.adobe.com/t5/premiere-pro-discussions/where-are-my-transform-handles/m-p/11346974/highlight/true

Цена противоположного решения — «правим в своём окне, смотрим в другом»
(Vegas Event Pan/Crop):

> the image in the event pan/crop tool doesn't match up to the image in
> the preview window or the render
> — Daniel Johnson, Creative COW, 15.07.2019, https://creativecow.net/forums/thread/image-crop-doesnt-match-preview-or-render/

Она же — в большом списке претензий к VEGAS Pro 2026 на официальном
форуме Boris FX (автор сам помечает текст как «Translated using GPT» —
это машинный перевод его корейского оригинала, поэтому цитата точная, но
формулировка не авторская):

> The workflow is also slower than adjusting the event directly in the
> Preview window, as in Premiere Pro.
> — cglaa5000, Boris FX Forum, 01.08.2026, https://forum.borisfx.com/t/suggestions-for-boris-regarding-vegas-pro-2026-very-long-post/24911/2

> When using masks, the Pan/Crop window where the actual editing takes
> place is separated from the Preview window where the result appears.
> — там же

Зачем нужен выключатель контролов — официальный ответ Adobe:

> Layer Controls are intentionally hidden during preview playback so
> masks and shape paths don't get in the way.
> — Nishu Kushwaha, Adobe Community Manager, 03.06.2026, https://community.adobe.com/questions-529/layer-controls-are-not-staying-visible-1626156

И что бывает, когда выключатель есть, но спрятан, а траектория рисуется
целиком:

> But there are so many line and control points that appear, i literally
> cannot even see the video content under it all. It's like a birds nest.
> — Ed Brister, Blackmagic Forum, 01.12.2024, https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=212742

> One constant irritant for me is that there is no direct way to turn the
> on screen controls on/off. You either need to look away from the screen
> to find the Ctrl+k keys or go down two levels of right click menu.
> — Iain Fisher, Blackmagic Forum, 28.01.2026, https://forum.blackmagicdesign.com/viewtopic.php?f=22&t=232477

Первоисточник о том, что якорь — исключение среди контролов, и о том,
что при воспроизведении контролы гасятся всегда:

> Anchor X and Y: Move the image's center point (the point that the image
> rotates around). Note: This control isn't available in the onscreen
> controls.
> — Final Cut Pro User Guide, Apple, https://support.apple.com/guide/final-cut-pro/resize-move-and-rotate-clips-verfc8a4713/mac

> Onscreen controls are always hidden when you play clips in the timeline.
> — Final Cut Pro User Guide, Apple, https://support.apple.com/guide/final-cut-pro/show-onscreen-controls-verbd553847/mac

## Как ложится на Kinotrain
CAPABILITY: openspec/specs/ui-timeline/ — контракт в `spec.md`, «почему» и пути в коде в `design.md`.
Рамка трансформации ведущего узла «Геометрия» над кадром превью (перенос, восемь ручек
масштаба, поворот, якорь с компенсацией, кроп) и раздел полей «Трансформация» рядом с ней;
прямоугольник размещения считает ОДНА функция `kt-fx::geom`, общая с рендером.
Камеры и `Pan3d`, маски, траектория ключей, разметка и режимы сравнения — не входят.

## Оценка

| Что | Ответ |
|---|---|
| Объём | M — 2–3 стадии. С1: общая геометрия (перенос `pan3d_mvp` из kt-gpu в общий код + обращение «экран → плоскость слоя», починка гейта перекомпозиции и масштаба по вертикали). С2: ОДИН общий виджет контрола над кадром (рамка, 8 ручек, якорь, вращение) на `Pan3d` и камерах, парный к `params_grid`. С3: разметка (safe-зоны, сетка, центр) и переключатель показа |
| Где основная работа | kt-app (`app.rs::draw_preview` + новый общий виджет рядом с `widgets.rs`/`automation.rs`), kt-engine (`video_plan.rs` — геометрия слоя наружу), kt-gpu (матрица становится общей, а не своей) |
| Формат проекта | не трогает: правятся существующие поля существующими командами. Состояние «показывать контролы/сетку» — вьюпорт, а он по решению Р.7.1 хранится в проекте |
| Тесты, которые придётся написать | харнесс `layout_tests.rs`: попадание в ручку с первого раза и её хват-зона; жест якоря не двигает картинку — **и на повёрнутом слое тоже** (чужой баг из цитат); экранный жест и `params_grid` дают одно и то же число (правило 17); один undo на жест; контролы гаснут при воспроизведении; тумблеры «ручки» и «траектория» независимы. Чистые функции: «экран → кадр» на всех пяти долях (включая 120×66) и обратно, обращение MVP против прямой — всё на CPU, без адаптера. Живой сценарий в `src/scripts/live/`: перетащил слой — пиксель в кадре СМЕНИЛСЯ (`pixel_settled(differs_from=…)`), плюс `img_rect` в `KT_GEOM_DUMP` |
| Зависит от | ничего обязательного. Обратная зависимость сильнее: F-MSK-001 (маски) и F-TXT-001 (текст) обе упираются в «нет ни одного жеста над кадром» — эта работа их разблокирует |
| Ломает совместимость | нет |

## Вердикт

Берём, но начинать не с рамки, а с геометрии. Параметры под контрол уже
есть целиком (`Pan3d` с якорем, камеры с look-at), формат не меняется,
места на экране контрол не занимает — по соотношению «цена/эффект» это
самая дешёвая из оставшихся крупных UI-фич.

Решающий аргумент — камеры. `CameraParams` — семь чисел, из которых
позиция и точка интереса заданы в мировых пикселях, а повороты в
градусах ZXY; выставлять кадр камеры вслепую через `DragValue` в окне
свойств никто в здравом уме не станет. Пока экранного контрола нет, вся
ветка Р.29 (3D, несколько камер, курс «в сторону After Effects» —
формулировка владельца) остаётся недоступной практически, а не
технически.

Первым делом чинятся два дефекта, которые контрол вскроет немедленно и
которые полезны сами по себе: гейт перекомпозиции превью, не смотрящий на
ревизию модели, и превью на 1/16–1/8, не подобное кадру проекта. Режимы
сравнения (split-screen, шторка «до/после») в этот заход не входят: это
второй композит того же тика и отдельная заметка — там своя развилка
«что считать стороной „до“».

### Итог Р.64 (2026-09-25)

СДЕЛАНО: рамка трансформации над кадром у выделенного видео-события — перенос за тело,
восемь ручек масштаба, поворот (Shift — 15°), якорь с компенсацией позиции (верной и на
повёрнутом слое, где у AE дефект), четыре ручки кропа; один шаг истории на жест, живая
картинка в темпе жеста, параметр с кривой правится КЛЮЧОМ. Оба дефекта, названные выше как
«вскроются немедленно», закрыты: гейт перекомпозиции — ещё в Р.62 (t722b), подобие доли
превью — здесь (t765, ближайшая ЧЁТНАЯ пара). Поля приведены в раздел «Трансформация»
(группы, пары X/Y, единицы, ползунки, схема рамки, сброс).

ОТЛОЖЕНО и почему: экранные контролы КАМЕР и `Pan3d` (свои единицы — мировые px — и своя
развилка «какая камера активна под курсором»), маски и траектория ключей (свой набор ручек
над тем же кадром), разметка (safe-зоны, сетка, центр) и режимы сравнения «до/после»
(второй композит того же тика). Каждое — отдельный набор над кадром и отдельный ответ на
вопрос «где мои ручки»; решать их за владельца одним заходом нельзя.

## Источники

- Final Cut Pro User Guide, «Intro to onscreen controls» (дата обращения
  2026-09-07) — https://support.apple.com/guide/final-cut-pro/intro-to-onscreen-controls-verbd553437/mac
- Final Cut Pro User Guide, «Show onscreen controls» —
  https://support.apple.com/guide/final-cut-pro/show-onscreen-controls-verbd553847/mac
- Final Cut Pro User Guide, «Resize, move, and rotate clips» —
  https://support.apple.com/guide/final-cut-pro/resize-move-and-rotate-clips-verfc8a4713/mac
- VEGAS Pro Help (MAGIX), «Split-screen previews» —
  https://help.magix-hub.com/video/vegas/22/en/content/topics/5-preview/split-screen_previews.htm
- VEGAS Pro Help (MAGIX), «Using the Video Preview window» (Overlays:
  Grid, Safe Areas) —
  https://help.magix-hub.com/video/vegas/22/en/content/topics/5-preview/videopreview.htm
- VEGAS Pro Help (MAGIX), обзор Pan/Crop —
  https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/pan-crop-overview.htm
- Adobe Community, тред «Anchor Point Issue» —
  https://community.adobe.com/t5/after-effects-discussions/anchor-point-issue/m-p/12978119
- Adobe Community, тред «Where are my transform handles?» —
  https://community.adobe.com/t5/premiere-pro-discussions/where-are-my-transform-handles/m-p/11346974/highlight/true
- Creative COW, тред «Image crop doesn't match preview or render» —
  https://creativecow.net/forums/thread/image-crop-doesnt-match-preview-or-render/
- VEGAS Pro 2026, документация на CDN Boris FX: список кнопок Video
  Preview и оговорка про Crop OFX —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/5-preview/videopreview.htm
  и https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/pan-crop-overview.htm
- Boris FX Forum, «Suggestions for Boris Regarding VEGAS Pro 2026» —
  https://forum.borisfx.com/t/suggestions-for-boris-regarding-vegas-pro-2026-very-long-post/24911/2
  (автор помечает текст как машинный перевод с корейского)
- Creative COW, тред «Animate anchor point without moving layer» —
  https://creativecow.net/forums/thread/animate-anchor-point-without-moving-layer/
- Blackmagic Forum: «Transform tool overlay has unwanted vectors to
  Keyframes» — https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=212742 ;
  тред о переключателе контролов —
  https://forum.blackmagicdesign.com/viewtopic.php?f=22&t=232477
- Adobe Community, «Layer Controls are not staying visible» —
  https://community.adobe.com/questions-529/layer-controls-are-not-staying-visible-1626156
- DaVinci Resolve, страница Color на сайте Blackmagic (Wipe / Split
  Screen / Highlight, стили галереи) —
  https://www.blackmagicdesign.com/products/davinciresolve/color
- Формулировки руководства Resolve («Transform/Crop/Dynamic Zoom button
  at the bottom left of the Timeline Viewer»; «View > Viewer Overlay
  submenu»; `Shift-\``; `View > Safe Area`) взяты из зеркал официального
  руководства (18.6 и 17) —
  https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part2092.htm
  и https://manualzz.com/doc/o/235u5a/blackmagic-davinci-resolve-17-instruction-manual-compositing-and-transforms-in-the-timeline
  **[НЕ ПРОВЕРЕНО по руководству Resolve 21]**: прямые страницы зеркал из
  этого окружения отдавали 403/404, версия 21 живьём не сверялась;
  механику независимо подтверждает продакт-менеджер Blackmagic в треде
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=234723 (14.04.2026).
- Названия режимов Premiere Comparison View (Side by Side / Vertical
  Split / Horizontal Split), Safe Margins в меню Program Monitor и
  команда After Effects `View > Show Layer Controls` (Ctrl+Shift+H,
  состав — `View Options`, Ctrl+Alt+U) взяты из сниппетов справки Adobe
  и подтверждений экспертов Adobe Community (в том числе Todd Kopriva,
  Creative COW, 18.03.2010 —
  https://creativecow.net/forums/thread/cant-see-bounding-boxes-or-pan-behind-tool/):
  `helpx.adobe.com` из этого окружения не открывается **[НЕ ПРОВЕРЕНО
  первоисточником]**.
- Про попадание мышью на превью в ДОЛЕ разрешения (half/quarter) прямых
  жалоб в чужих форумах найти не удалось — ближайшее — советы менять
  размер и зум Program Monitor, чтобы контролы появились. Численный
  пример 120×66 посчитан по нашему коду, а не взят из чужого багрепорта.
- Все утверждения «у нас есть / у нас нет» проверены грепом по `src/` и
  чтением кода на коммите `60c85a0`; поведение живьём не запускалось —
  соответствующие места помечены в тексте.

## Kinotrain: change `geometry-on-screen-controls` следом за Р.63

Владелец в Р.63 сказал прямо: **«fx геометрии по прежнему очень скуден,
сплошные скучные поля ввода, даже в Vegas и то лучше сделано, а нам надо
лучше чем в vegas, в том же Pr/Ae это удобнее чем в Vegas, нужно удобнее
всех делать. КАК МИНИМУМ у нас должно быть редактирование прям в превью
удобное, это как минимум»**. Спрошенный прямо, он уточнил: экранные
контролы — **в ДОПОЛНЕНИЕ к полям, а не вместо них**, и сами поля «в
нормальном виде».

Отсюда вердикт по этой фиче меняется с «кандидат» на «делаем следующим
change» — `geometry-on-screen-controls`: рамка трансформации ведущего узла
«Геометрия» прямо в превью (перетаскивание, углы, поворот, якорь), при
сохранении числовых полей и их автоматизации. Разведка выше (Vegas Pan/Crop,
Premiere Motion, AE Layer Controls) — вход в эту работу.

**Реализовано в Р.64** — change `geometry-on-screen-controls`; дальнейшая жизнь фичи —
в `openspec/specs/ui-timeline/` и `openspec/specs/video-plan/`.
