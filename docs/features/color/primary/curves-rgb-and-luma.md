---
id: F-CLR-002
title: Кривые RGB/Luma и кривые «по признаку»
status: кандидат
priority: A
cost: L
category: color/primary
editors: [resolve-20, premiere-26, fcp-12, vegas-pro-2026, after-effects-26]
we_have: нет
crates: [kt-model, kt-fx, kt-gpu, kt-engine, kt-otio, kt-app]
invariants: [7, 11, 12]
ui_rules: [1, 8, 9, 13, 17]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-CLR-002 — Кривые RGB/Luma и кривые «по признаку»

## Суть

График «вход → выход» на квадрате. По диагонали — ничего не изменилось;
поднял середину — картинка светлеет; поставил две точки, утопил левую и
поднял правую — получилась S: тени чернее, света ярче, середина почти не
тронута. Отдельно правится общая яркость (композитная кривая) и отдельно
каждый из R, G, B — так вынимают паразитный оттенок из теней, не трогая
света. Вторая, принципиально другая группа: по оси X не значение канала,
а ПРИЗНАК пикселя — его тон или насыщенность, и кривая говорит, что с
найденными пикселями сделать. «Приглушить только красные» — одно движение
мышью, без выделения и без маски.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| DaVinci Resolve | 20.3.2 | Curves: Custom + Hue Vs Hue / Hue Vs Sat / Hue Vs Lum / Lum Vs Sat / Sat Vs Sat | палитра на цветовой странице, своя у каждого узла. Custom — Y/R/G/B, каналы связываются и развязываются. Полный набор кривых по признаку, и точка ставится КЛИКОМ ПО КАРТИНКЕ в вьюере, а не по графику («Simply click in the viewer to choose a value»). Эталон, на который ссылаются пользователи всех остальных |
| Adobe Premiere | 26.x | Lumetri Color → Curves: RGB Curves + Hue Saturation Curves | одна секция панели, два блока: классические RGB/Luma и кривые по признаку с пипеткой (Hue vs Sat подтверждается тредами поддержки). Панель — эффект на клипе, тот же Lumetri переносится в After Effects. Репутация у секции тяжёлая: жалобы на паразитный оттенок от самих Lumetri-кривых и «единственный выход — старый RGB Curves» тянутся с 2019 года |
| Final Cut Pro | 12.3 | два ОТДЕЛЬНЫХ эффекта: Color Curves и Hue/Saturation Curves | Color Curves — Luma, Red, Green, Blue плюс произвольная кривая на «custom color»: «click the eyedropper at the top of one of the color curves, then click or drag over a color in the viewer». Hue/Saturation Curves — шесть кривых (Hue vs Hue, Hue vs Sat, Hue vs Luma и три по диапазонам насыщенности и яркости). Это ЭФФЕКТЫ, которые добавляют, а не постоянная палитра |
| VEGAS Pro | 2026.0 | «Color Curves» (OFX) и панель Color Grading | ДВЕ несвязанные системы в одном продукте. Старый OFX Color Curves — композит + R/G/B, точки с ТИПАМИ, как у кейфреймов анимации (Linear — бело-серая, Hold — красная). Панель Color Grading — колёса, HSL-ползунки, кривые, LUT; кривые по признаку приехали поздно (по release history — Hue vs Hue в VP20, август 2022; Hue vs Sat, Sat vs Sat и пипетка для кривых — VP20 build 214, октябрь 2022; Hue vs Lum и Lum vs Sat в истории релизов не значатся). Панель при этом НЕ умеет кейфреймов: «The new Color Grading Panel doesn't support keyframing (yet)» |
| After Effects | 26.x | эффект Curves + эффект Lumetri Color | свой Curves — только каналы (RGB и по отдельности, плюс альфа), карандаш для произвольной карты, сохранение и загрузка кривой файлом. Кривые по признаку приезжают не своим эффектом, а тем же Lumetri, что в Premiere |

## Фишка фишки

**1. Точка на кривой по признаку — не точка, а ТРОЙКА, и две её крайние
точки И ЕСТЬ тот самый qualifier.** Справка Apple описывает механизм
буквально: пипеткой берут цвет с картинки, и «Three control points appear
on the curve… The middle control point represents the color you selected
with the eyedropper. The two outer control points constrain the selection
to a narrow range of hues». То есть один жест «ткнуть в листву» делает
то, ради чего в классическом гейдже строят вторичную коррекцию: выбирает
диапазон тона, задаёт ему ширину (softness — это буквально расстояние до
крайних точек) и открывает ручку модификации. Колористы ценят это не как
удобство, а как ДРУГОЕ КАЧЕСТВО результата: «hue vs hue/hue vs sat is
faster to use than secondaries… sometimes it will produce less artifacts
then qualifier». Отсюда же видно, чем такая кривая ОТЛИЧАЕТСЯ от
RGB-кривой, а не «похожа на неё»: у RGB-кривой обе оси — одна и та же
величина (вход и выход одного канала), и «ничего не делать» — это
ДИАГОНАЛЬ; у кривой по признаку оси — разные величины (X — тон, Y —
насыщенность), «ничего не делать» — ГОРИЗОНТАЛЬ посередине, и пипетка
обязательна, потому что попасть в нужный тон по горизонтали графика на
глаз нельзя: на графике нет картинки.

**2. Домен у этой кривой — КОЛЬЦО, а виджет — прямоугольник, и это до сих
пор ломается у эталона.** Тон циклический: 360° это снова 0°, значит
кривая обязана продолжаться через шов, а точка у красного — жить на обоих
краях сразу. Resolve этого не делает: «when adjusting the green input hue
on hue vs hue/sat curves on the color page, it gets stuck if you try to
shift the input hue further toward yellow than 0.00. It would be great if
adjusting past 0.00 went back to 359.99». Тот же класс ошибки на другом
конце — у RGB-кривой концы прибиты к (0,0) и (255,255), и сдвинуть чёрную
точку вправо нельзя без лишних точек: «Allow curve end points to be moved
along the X axis (like Resolve etc.), without having to make extra points
(and fiddle with their tangency) in order to achieve that». И третий
случай той же болезни — шаг: первым пунктом раздела «COLOR GRADING CURVES
IMPROVEMENTS» идёт просьба двигать точку стрелками «in 256 increments
(corresponding to 8-bit levels), rather than 100 increments». Три разные
жалобы, одна причина: виджет кривой нарисован без учёта того, какой у неё
домен и какой квант у тракта под ней.

**3. Пять кривых по признаку выглядят однотипно, но ломаются по-разному —
и первой ломается та, что трогает ЯРКОСТЬ.** «When I e.g. try to darken
the blue sky by slight Hue to Lum curve adjustment, the clip scene will
start to fall apart in small "fragments or mosaic structure"… With all the
other curve adjustments (like Hue vs Hue and Hue vs Sat) everything work
just fine» — то есть при ОДНОМ и том же по силе движении одна кривая
работает, а соседняя разваливает кадр. Ответ колориста тот же: «Hue vs.
Lum is a very, very tricky correction where you need lots of detail and
soft correction in order to make it work. It gets choppy very quickly». И
это не про восемь бит: у Sat vs Sat полосы получают на 12-битном BRAW —
«With Sat vs Sat, the gradation between what is affected and what is not,
seems a lot harsher. It creates unnatural bands within the red of the
bike». Практический вывод для того, кто это строит: артефакт даёт не
разрядность, а ЖЁСТКОСТЬ спада селекции, и мягкость границы выборки —
такой же обязательный параметр, как сама кривая.

## Плюсы

- Селекция и модификация — ОДИН объект: нечего рассинхронизировать,
  нечего забыть выключить, и правка ширины выборки делается теми же
  точками, что и правка её края.
- Мягкость видна как наклон кривой, а не как число «softness» в поле:
  на глаз понятно, будет ли переход рваным, ещё до того как отпустил мышь.
- Меньше артефактов, чем у qualifier'а на той же задаче — по прямому
  свидетельству колористов, и при этом без второго слоя и без ключа.
- Композитная кривая и каналы — один виджет с одним словарём: выучил жест
  один раз, применяешь и к яркости, и к цветовому сдвигу.
- В целочисленном 8-битном тракте кривая — самый дешёвый эффект из
  возможных: таблица на 256 входов считается раз на кадр, применение —
  одно обращение к массиву на канал, дешевле любой свёртки.
- Кривая тривиально сводится к LUT и обратно, поэтому её можно выгрузить
  и применить чужим инструментом; ни у одного другого вида коррекции
  такого обмена нет.

## Минусы

- Восемь бит: крутая кривая СКЛЕИВАЕТ соседние уровни. «This is an issue
  with 8-bit in general: it falls apart very quickly if you try to hit it
  too hard. Skies have always been a problem». Плата за дешевизну
  таблицы — ровно она.
- Жёсткий спад селекции даёт полосы независимо от разрядности — Sat vs
  Sat бандит на 12-битном BRAW. Значит у каждой кривой по признаку обязан
  быть свой контроль мягкости, и это ещё одна сущность в UI.
- Концы прибиты. Клипануть тени или света без отдельной механики (сдвиг
  конца по X, soft clip) нельзя — пользователь ставит лишние точки и
  борется с их касательными.
- Ось тона замкнута, а прямоугольник — нет: правка «через 0°» требует
  либо честной циклической интерполяции, либо дублирования точек на обоих
  краях. Без этого красные тона правятся хуже всех остальных — и это
  наблюдаемый дефект, а не теория.
- Две системы цвета в одном продукте расходятся: у VEGAS старый OFX умеет
  кейфреймы, новая панель — нет, и пользователь держит в голове, каким из
  двух инструментов он сегодня красит.
- Точка кривой, сделанная из кейфрейма, тащит за собой чужие свойства: в
  VEGAS у неё есть тип Linear/Hold, настройка «тип нового ключа» на неё не
  действует, и объяснить это пользователю невозможно, потому что у
  цветовой кривой «hold» не значит ничего.
- Пипетка привязывает инструмент к ВЬЮЕРУ: окно кривой перестаёт быть
  самодостаточным и требует кадра под рукой. И хват точки после пипетки —
  отдельная болезнь: в Lumetri после выбора цвета пипеткой точку бывает
  не взять мышью.
- Кривая по признаку не знает про пространство: «приглушить красные»
  глушит их и в лице, и в вывеске на заднем плане. Ограничить область
  можно только маской — инструментом, которого у кривой нет.

## Что говорят пользователи

Почему за кривые по признаку держатся — формулировка колориста, а не
маркетинг:

> First - hue vs hue/hue vs sat is faster to use than secondaries /
> Second - sometimes it will produce less artifacts then qualifier
> — Val Andreev, Lift Gamma Gain, 23.10.2014,
> https://www.liftgammagain.com/forum/index.php?threads/hue-vs-hue-as-compared-to-using-secondaries.3743/

Не все кривые равны — ломается именно та, что трогает яркость:

> When I e.g. try to darken the blue sky by slight Hue to Lum curve
> adjustment, the clip scene will start to fall apart in small "fragments
> or mosaic structure" with huge lumination variations.
> — Jan Frojdman, Blackmagic Forum, 02.03.2015,
> https://forum.blackmagicdesign.com/viewtopic.php?t=33024

> Hue vs. Lum is a very, very tricky correction where you need lots of
> detail and soft correction in order to make it work. It gets choppy very
> quickly.
> — Marc Wielage, там же, 03.03.2015

Полосы дают не только восемь бит — жёсткость спада селекции тоже:

> With Sat vs Sat, the gradation between what is affected and what is not,
> seems a lot harsher. It creates unnatural bands within the red of the
> bike.
> — InterrobangMP (Paul Rayner), Blackmagic Forum, 16.08.2022 (на вопрос
> «это 8 бит?» автор ответил «It's BRAW»),
> https://forum.blackmagicdesign.com/viewtopic.php?t=165970

> This is an issue with 8-bit in general: it falls apart very quickly if
> you try to hit it too hard. Skies have always been a problem,
> particularly with drastic changes in luminance and saturation.
> — Marc Wielage, Blackmagic Forum, 23.10.2015,
> https://forum.blackmagicdesign.com/viewtopic.php?t=33024

Самое ценное — «ушёл из-за этого» и «включил один раз и больше никогда»:

> I shot Prores HQ footage and I color graded it in Vegas Pro 21, in Vegas
> I get color banding but in Premiere pro it looks perfect.
> — Jimmie Brady, Creative COW, 04.06.2024 (там же его «I prefer editing
> with Vegas and I would hate to switch», и совет в ответ: «maybe do your
> color grading in Premiere and re-output the results into Vegas»),
> https://creativecow.net/forums/thread/color-banding-only-in-vegas-pro-not-other-softwares/

> I tried to use the RGB Curves (obsolete) and it worked perfectly, no
> yellow tint, so I'm no longer using the Lumetri Curves.
> — Jorge Jaramillo Hdz, Adobe Support Community, ≈01.2022,
> https://community.adobe.com/t5/premiere-pro-discussions/problem-with-lumetri-curves/td-p/10422834

Домен кривой не замкнут — у эталона, в 2024 году:

> when adjusting the green input hue on hue vs hue/sat curves on the color
> page, it gets stuck if you try to shift the input hue further toward
> yellow than 0.00.
> — shmowells, Blackmagic Forum, 12.03.2024,
> https://forum.blackmagicdesign.com/viewtopic.php?t=197921

Виджет кривой: точку не взять после пипетки, а у соседа клик по точке
роняет программу:

> After using the eyedropper to select a color area in Hue vs Sat in
> Lumetri Color, I am not able to adjust the saturation. Mouse selecting
> one of the control "dots" does not work.
> — gsasaki, Adobe Support Community, ≈11.2025,
> https://community.adobe.com/t5/premiere-pro-discussions/hue-vs-sat/m-p/15548381

> I am using 'Color curves' video effect. When I am adding new points they
> are still 'Linear' (white/grey points), but I want them to be red as a
> default one ('Hold' points).
> — marcin_z, форум VEGAS Pro, 22.04.2026,
> https://forum.borisfx.com/t/hold-type-of-video-effect-as-a-default-point-in-vegas/22713

И чего просят у кривой те, кто ей действительно пользуется:

> Allow curve end points to be moved along the X axis (like Resolve etc.),
> without having to make extra points (and fiddle with their tangency) in
> order to achieve that.
> — NickHope, форум VEGAS Pro, 04.04.2026 (в том же списке — «Allow anchor
> points to be moved up and down with the arrow keys in 256 increments
> (corresponding to 8-bit levels), rather than 100 increments»),
> https://forum.borisfx.com/t/suggestions-improvements-new-features-boris-vegas-pro-2026/21773

## Как ложится на Kinotrain

### Что уже есть

- **Ничего из цветокоррекции.** Проверено грепом по `hue|saturation|lut|
  gamma|colorspace|scopes|histogram|vectorscope` в `src/libs/kt-fx/src`,
  `src/libs/kt-gpu/src`, `src/libs/kt-model/src` — ноль совпадений.
  Дизеринга тоже нет ни одного.
- **Готовый 8-битный тракт целиком.** `src/libs/kt-media/src/lib.rs:51`
  («Видеокадр: RGBA8») → swscale в RGBA
  (`src/libs/kt-ffmpeg/src/source_impl.rs`) → цепочка FX
  (`src/libs/kt-fx/src/video.rs`, «RGBA8 in/out») → композит в целых
  (`alpha_k` 0..4096, `src/libs/kt-engine/src/video.rs:53`) → мукс
  RGBA→`AV_PIX_FMT_YUV420P` (`src/libs/kt-ffmpeg/src/muxer_impl.rs:242`).
  Формат GPU-текстур — `Rgba8Uint` (`src/libs/kt-gpu/src/pool.rs:13`) и
  выбран СОЗНАТЕЛЬНО: `src/libs/kt-gpu/src/shaders/common.wgsl` — «u8/255.0
  в f32 теряет точность и ломает зеркала целочисленной CPU-арифметики».
- **Дорога добавления видео-FX протоптана и коротка.** Grep
  `FxInstance::Glow` даёт ровно шесть файлов: `kt-model/src/project.rs`,
  `kt-fx/src/video.rs`, `kt-fx/tests/video_fx.rs`, `kt-gpu/src/engine.rs`,
  `kt-gpu/tests/parity.rs`, `kt-app/src/fx_editor.rs`.
- **Пипетка цвета уже нарисована** — `ui.color_edit_button_rgb` для
  `key_*`/`color_*`/`tint_*` плюс механизм `params_grid(..., skip)`
  (`src/apps/kt-app/src/widgets.rs:123`,
  `src/apps/kt-app/src/fx_editor.rs:565`). И, главное, СОБРАННЫЙ КАДР
  превью лежит у UI в RGBA на руках: `compose_preview_into(Tick(key_tick),
  &mut rgba)` — `src/apps/kt-app/src/app.rs:3135`. Значит «взять цвет
  прямо с картинки» — это отображение позиции мыши в буфер, а не новая
  подсистема.
- **Детерминированный шум под дизер уже есть**: counter-based
  `noise_hash`/`noise_seed(burst_index)` —
  `src/libs/kt-fx/src/video.rs:459`, CPU и GPU байт-в-байт (Р.29).
- **Прецедент «эффект со своим редактором точек» есть, и он ПОХОРОНЕН**:
  `FxInstance::CamMotion { keyframes: Vec<CamKeyframe> }` со своими
  `DragValue` в `src/apps/kt-app/src/fx_editor.rs:708`; из списка
  шаблонов убран (Р.34 К2, `fx_editor.rs:515`).

### Чего не хватает

- Типа «кривая» в паспорте параметров. `ParamSpec { name, min, max,
  default, step, unit, kind }` (`src/libs/kt-model/src/project.rs:2203`)
  описывает СКАЛЯР, и на этом стоит вся генерация UI, чтения и записи из
  `params_for!`. Кривая — не скаляр и в эту декларацию не лезет.
- Пост-композитной стадии. `FramePlan` — это `layers`
  (`src/libs/kt-engine/src/video_plan.rs:76`), места «сделать что-то с
  готовым кадром» в нём нет вовсе.
- Дизеринга и вообще любого понятия о разрядности.

### Что мешает

1. **Виджет автоматизации переиспользовать нельзя — и правило
   `src/apps/kt-app/CLAUDE.md` («Кривые рисует ОДИН виджет
   `automation::draw`; своего редактора кейфреймов ни у одного эффекта
   быть не должно») здесь задето и требует решения пользователя.**
   Аргумент не «на вкус», а по коду `src/apps/kt-app/src/automation.rs`:
   `AutoCtx` несёт `range: (Tick, Tick)`, `to_timeline`, `to_curve` и
   `frame_step`; `graph()` рисует линейку ТАЙМЛАЙНА
   (`crate::timeline::ruler_steps`, `format_ruler_time`), красную линию
   главного курсора, жест по линейке отдаёт `out.seek` в главный плейхед,
   а новая точка снапится на сетку кадров. Вертикальное окно считается ПО
   КЛЮЧАМ (`value_window`, запас 15 %) и замораживается на драг. У
   цветовой кривой ось X — не время: курсор ставить некуда, seek некуда,
   снапить не на что; окно по ОБЕИМ осям обязано быть жёстко 0..255, иначе
   «ничего не делать» перестаёт быть диагональю — то есть плавающая шкала
   здесь прямой дефект. Оставить один виджет — значит выкинуть из него
   линейку, курсор, seek, снап и автоокно, после чего останется пустой
   прямоугольник с точками. Честный ответ: правило говорит про редактор
   КЕЙФРЕЙМОВ (ось = время), и переформулировать его надо именно так.
   Цена смешения видна у VEGAS: там точка цветовой кривой действительно
   унаследовала тип кейфрейма (Linear/Hold), и пользователь спрашивает,
   почему настройка «тип нового ключа» на неё не действует — вопрос без
   ответа, потому что у цветовой кривой «hold» не значит ничего. Что
   переиспользовать ОБЯЗАНО — общий низ: постановка точки кликом по
   пустому месту, перетаскивание, удаление двойным кликом, хват 9 px,
   выделенная точка белым. Его надо ВЫДЕЛИТЬ из `automation.rs`, а не
   скопировать: п.221 (`slowed` и `pan3d` разъехались, потому что у
   каждого был свой редактор) — ровно про это.
2. **Кривая не пролезает через план кадра.** `video_plan.rs`:
   `fx: Cow<'a, [FxInstance]>` — «цепочка С УЖЕ ПРИМЕНЁННОЙ автоматизацией
   (Р.30): кривые разрешаются ОДИН РАЗ при построении плана».
   `ParamCurve` схлопывается в ОДИН `f64` через
   `curve_value`/`apply_fx_auto` (`kt-model/src/project.rs:1652`, `:1673`)
   ДО того, как исполнитель что-то увидит. Цветовая кривая обязана доехать
   до исполнителя ЦЕЛИКОМ. Это не «ещё один параметр», а новый вид данных
   внутри `FxInstance`.
3. **GPU: один bind group layout на весь движок.**
   `src/libs/kt-gpu/src/engine.rs:458` — униформа с
   `min_binding_size: 128` плюс РОВНО три текстурных слота T0/T1/T2
   (`texture_entry(1..3)`), и структура `Params` объявлена в
   `shaders/common.wgsl`, который подклеивается ко ВСЕМ десяти пайплайнам.
   Таблица на 256 входов × 4 канала — 1024 байта, в 128-байтную униформу
   она не помещается ни при какой упаковке. Вариантов ровно два: положить
   LUT текстурой (формат пула `Rgba8Uint`, 256×1 подходит без единой
   правки `pool.rs`) — но это четвёртый слот либо отъём T2 у пассов,
   которые им пользуются; или растить униформу — а это правка общей шапки,
   то есть перекомпиляция и ревалидация всех пассов. Любой вариант трогает
   движок целиком, а не один шейдер.
4. **Восемь бит — это устройство, а не «качество» (инвариант №11).**
   Кривая в 8 битах считается один раз в таблицу на 256 входов и
   применяется индексацией — дешевле всего, что у нас есть. Но каждая
   крутая кривая склеивает уровни, и на градиенте это полосы: ровно та
   жалоба, что процитирована выше про небо. Лечение — дизер, то есть ШУМ,
   и он обязан быть детерминированным: `rand` убьёт байт-в-байт и
   GOLDEN_VIDEO. Стройматериал уже лежит — тот же counter-based
   `noise_hash` (`kt-fx/src/video.rs:459`), CPU и GPU байт-в-байт. Не
   блокер, но решение, которое принимается ДО первого ядра, а не после
   первой жалобы. Отдельно: полосы от Sat vs Sat на 12-битном BRAW
   показывают, что мягкость спада селекции — такой же обязательный
   параметр ядра, как сама кривая, и «сделаем потом» тут не работает.
5. **Грейд некуда повесить «на всё».** FX считаются пер-слой
   (`video_plan.rs`), пост-композитной стадии нет. Через морфер тоже не
   выходит: `MorphPatch.video_fx` применяется в
   `Project::effective_clips()` (`src/libs/kt-model/src/project.rs`) ДО
   построения плана, то есть ложится на КАЖДЫЙ клип отдельно — та же дыра,
   что описана в F-MPH-007. Пока её нет, «цвет всей сцены» — это N
   одинаковых кривых на N клипов.
6. **Экспорт прореживает цветность ПОСЛЕ грейда.**
   `muxer_impl.rs:242` — RGBA→YUV420P: цветовых отсчётов вдвое меньше по
   обеим осям. Инвариант №7 при этом цел (превью и экспорт байт-в-байт по
   RGBA), но кривая по ПРИЗНАКУ работает по тону каждого пикселя, и на
   границах насыщенных цветов результат в файле будет мягче, чем в
   превью. Это надо знать заранее — иначе будет заведён дефект «экспорт
   красит не так».
7. **Домен тона замкнут, и это надо решить в модели, а не в рисовании.**
   У Resolve кривая упирается в 0.00 вместо перехода на 359.99 (цитата
   выше) — значит циклическая интерполяция задаётся типом кривой в
   `kt-model`, там же, где сегодня живёт `curve_value`, и закрывается
   микро-тестом на шов. Отложить это «на вторую итерацию» нельзя:
   не-циклическая кривая тона — не «упрощение», а неверный результат на
   красных.

### Какие инварианты задевает

- **№7 (код композиции один).** Формула ложится в `kt-fx` и зеркалится в
  WGSL; по правилу крейта (`src/libs/kt-fx/CLAUDE.md`) скаляр, AVX2-ядро
  и шейдер правятся ОДНИМ коммитом, а SIMD обязан быть байт-в-байт со
  скаляром. Своей версии кривой в шейдере быть не может.
- **№11 (детерминизм).** Новые golden рядом с t26; GOLDEN_VIDEO и
  GOLDEN_AUDIO на проектах без кривой не должны шелохнуться. Дизер —
  только счётчиковый хеш.
- **№12 (UI греет систему только при движении).** Таблица на 256 входов
  пересчитывается при СМЕНЕ кривой, а не на UI-кадр; предпросмотр кривой в
  редакторе — не повод трогать `cache.epoch`.
- **Правила UI**: 1 (точка обязана браться с первого раза, хват шире
  рисунка — жалоба «не могу попасть по точке после пипетки» у Lumetri
  ровно про это), 8 (редактор рисуется в языке программы, а не ставится
  готовым контролом), 9 (никаких подписей «R:», «G:» — цвет линии и есть
  подпись), 13 (второй график не заводит второй полосы — вкладки внутри
  одного прямоугольника), 17 (пипетка и клик по графику — парные способы
  поставить точку и обязаны давать одинаковый результат).
- Правило «один виджет кривых» — задето прямо и разбирается выше.
- №1, №2, №4, №5, №6 не задеваются: ни аудио, ни кэша, ни владения
  декодером, ни часов это не касается.

## Оценка

| Что | Ответ |
|---|---|
| Объём | L — целый раунд. Это ДВА разных ядра (канальная таблица и селекция по признаку) и ДВА разных редактора, а не один эффект с галочкой |
| Где основная работа | `kt-fx` (скаляр + AVX2), `kt-gpu` (шейдер и решение, куда класть LUT — слот текстуры или рост униформы), `kt-model` (тип «кривая» рядом с `ParamSpec`, который сегодня описывает только скаляр, плюс циклический домен тона), `kt-app` (второй редактор кривых + пипетка по кадру превью) |
| Формат проекта | нужна версия конверта v5 (`KT_META_VERSION` = 4, `src/libs/kt-otio/src/meta.rs:17`); старая сборка обязана отвергнуть файл через `KtMetaTooNew`, а не прочитать «почти так же» |
| Тесты, которые придётся написать | golden новых эффектов рядом с t26 (`kt-fx/tests/video_fx.rs`); SIMD-парность рядом с t198; конформанс CPU↔GPU рядом с t181 (только через `assert_frames_close`); неизменность t11/t67 на проектах без кривой; round-trip и v5 рядом с t192/t222; циклическая интерполяция тона через шов 0°/360° — отдельным микро-тестом; вёрстка редактора и хват точки после пипетки — рядом с t221 в `layout_tests.rs`; повторяемость двух прогонов при включённом дизере |
| Зависит от | — . Но F-MPH-007 определяет, появится ли «грейд на всё»: без неё кривая живёт только на клипе |
| Ломает совместимость | вперёд нет, назад да |

## Вердикт

Берём. Цветокоррекция и LUT вошли в роадмап по прямому выбору
пользователя (`docs/user-feedback.md`, Р.18 п.6), а кривые — та её часть,
которая в нашем целочисленном 8-битном тракте стоит дешевле всего:
таблица на 256 входов ложится на существующую дорогу видео-FX без единого
нового понятия в движке.

Порядок обязателен: сначала RGB/Luma — одно ядро, одна таблица, одно окно;
кривые по признаку вторым шагом. Дороже они не арифметикой, а тремя
вещами, каждая из которых у чужих сломана и наблюдаема: точка на них
ТРОЙКА (центр плюс две границы выборки), домен тона ЗАМКНУТ, а пипетка
обязана брать цвет С КАРТИНКИ. Копировать надо не список из пяти кривых —
он есть уже и у VEGAS с 2022 года, — а именно эти три свойства: без них
получится ещё один инструмент, который «есть», но которым не пользуются.

Два решения принимаются ДО кода и не в раунде: (1) правило «редактор
кривых ровно один» переформулируется в «редактор КЕЙФРЕЙМОВ ровно один» —
иначе цветовая кривая либо получит чужую ось времени, либо выпотрошит
общий виджет; (2) дизер включён или нет — он единственный ответ на
бандинг, и он обязан быть счётчиковым, иначе гибнет инвариант №11.

## Источники

- https://support.apple.com/guide/final-cut-pro/use-the-color-curves-effect-verf87ca7b62/mac
  (Luma/Red/Green/Blue, произвольная кривая и пипетка «click or drag over
  a color in the viewer»; дата обращения 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/use-the-huesaturation-curves-effect-ver463347c6b/mac
  (шесть кривых; пипетка ставит ТРИ точки, крайние ограничивают диапазон
  тона; дата обращения 2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/color
  (Custom Curves + Hue Vs Hue / Hue Vs Sat / Hue Vs Lum / Lum Vs Sat /
  Sat Vs Sat; «Simply click in the viewer to choose a value»; дата
  обращения 2026-09-07)
- https://www.liftgammagain.com/forum/index.php?threads/hue-vs-hue-as-compared-to-using-secondaries.3743/
  (кривые против вторичной коррекции глазами колористов; 2014)
- https://forum.blackmagicdesign.com/viewtopic.php?t=33024
  (Hue vs Lum разваливает кадр там, где Hue vs Hue и Hue vs Sat работают;
  8 бит; 2015)
- https://forum.blackmagicdesign.com/viewtopic.php?t=165970
  (полосы от Sat vs Sat на BRAW — жёсткость спада, а не разрядность; 2022)
- https://forum.blackmagicdesign.com/viewtopic.php?t=197921
  (кривая тона упирается в 0.00 вместо 359.99; 2024)
- https://community.adobe.com/t5/premiere-pro-discussions/problem-with-lumetri-curves/td-p/10422834
  (паразитный оттенок от Lumetri-кривых, уход на старый RGB Curves;
  2019–2022)
- https://community.adobe.com/t5/premiere-pro-discussions/hue-vs-sat/m-p/15548381
  (Hue vs Sat в Lumetri, пипетка, точку не взять мышью; 2025)
- https://creativecow.net/forums/thread/color-banding-only-in-vegas-pro-not-other-softwares/
  (бандинг в VEGAS и нет в Premiere на том же исходнике; совет красить в
  Premiere и возвращать в VEGAS; 2024)
- https://forum.borisfx.com/t/suggestions-improvements-new-features-boris-vegas-pro-2026/21773
  (раздел «COLOR GRADING CURVES IMPROVEMENTS»: шаг в 256 уровней, линии 16
  и 235, подвижные концы по X, экспорт в LUT; 2026)
- https://forum.borisfx.com/t/hold-type-of-video-effect-as-a-default-point-in-vegas/22713
  (типы точек Linear/Hold у OFX Color Curves; «The new Color Grading Panel
  doesn't support keyframing (yet)»; 2026)
- https://forum.borisfx.com/t/banding-noise/22231
  («bug involving how levels are handled in 8 bit»; 2026)
- https://www.vegascreativesoftware.com/vegas-pro/ (состав цветового
  инструментария VEGAS: баланс белого, HSL-ползунки, кривые,
  пользовательские LUT)

Оговорки по честности проверки (все даты обращения — 2026-09-07):

- **Справка Adobe в этот заход недоступна**: `helpx.adobe.com` и
  `www.adobe.com` отказывали на каждом запросе (прокси). Наличие в
  Lumetri секции Curves с Hue vs Sat и пипеткой подтверждено ТРЕДАМИ
  поддержки (`community.adobe.com`), а не документацией; полный список
  кривых Lumetri и точное поведение её пипетки (сколько точек ставит) при
  следующей волне надо подтвердить по справке. Трёхточечный механизм
  задокументирован пока только у Apple.
- **Строка After Effects опирается на общеизвестный состав эффектов
  Curves и Lumetri Color**, по справке Adobe в этот заход не проверена.
- **Форум `vegascreativesoftware.info` больше не существует**: все его
  адреса отдают 301 на корень `forum.borisfx.com`. Темы переехали
  (поиск работает), но ПРЯМЫЕ ССЫЛКИ на старые треды мертвы — в том числе
  те, что стоят в источниках F-AUT-001. Практическое следствие для базы:
  пласт жалоб VEGAS-пользователей до 2022 года (то есть до появления там
  кривых по признаку) достать больше неоткуда, и вывод «в VEGAS этого
  нет» по сегодняшнему форуму строить НЕЛЬЗЯ.
- Живьём ни один из пяти редакторов не запускался: всё выше — пункты 1–2
  порядка доверия из `docs/features/_editors.md`.
