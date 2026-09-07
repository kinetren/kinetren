---
id: F-CLR-005
title: Приборы — waveform, parade, вектороскоп, гистограмма, false color
status: кандидат
priority: B
cost: M
category: color/scopes
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: нет
crates: [kt-app]
invariants: [4, 7, 8, 11, 12]
ui_rules: [3, 6, 9, 13]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-CLR-005 — Приборы — waveform, parade, вектороскоп, гистограмма, false color

## Суть

Пять способов посмотреть на ОДИН кадр числами вместо глаз. Waveform кладёт
яркость по горизонтали кадра: столбик графика стоит там же, где столбик
пикселей, поэтому видно не только «пересвет», но и ГДЕ он — небо слева или
лицо в центре. Parade разносит те же данные по R, G, B: разъехавшиеся низы
трёх графов — это цветовой сдвиг в тенях, по нему ставят баланс белого.
Вектороскоп — круг: угол = оттенок, радиус = насыщенность. Гистограмма
теряет положение в кадре, зато считает, СКОЛЬКО пикселей на каждом уровне.
False color красит сам кадр зонами экспозиции — он пришёл с камерных
мониторов, и им пользуются те, кто снимал.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| DaVinci Resolve | 20.3.2 | Video Scopes | пять приборов (Waveform, Parade, Vectorscope, Histogram, CIE Chromaticity) в ОТДЕЛЬНОМ плавающем окне (Cmd-Shift-W) или на втором экране; раскладки 1/2/4/9-up, причём в одной раскладке можно поставить ТРИ вектороскопа с разными тональными диапазонами (Low/Mid/High) и смотреть их разом. Шкала waveform/parade переключается: 10 бит (умолчание), 12 бит, проценты 0–100, мВ, HDR-ниты. Отдельно — качество отрисовки High/Medium/Low/Auto «для медленных станций», Low Pass Filter (сглаживание трассы) и Extents (обводка истинных выбросов), Display Qualifier Focus (пипетка во вьюере обводит кружком выбранные пиксели ПРЯМО В ГРАФЕ прибора). Мануал прямо пишет, что приборы GPU-ускорены и всё равно едят реалтайм, а их закрытие «освобождает всю обработку» |
| Adobe Premiere | 26.x | Lumetri Scopes; плюс приборы нового Color mode | классика — панель Lumetri Scopes (Window → Lumetri Scopes), вкладка рядом с Effect Controls. В 2026-м Adobe выкатила новый Color mode со своими приборами, и по форуму видно, чем он отличается от старой панели: waveform там КЛАМПИТ сигнал в 0–100 (не видно, ушло ли за пределы), а сами приборы всплывают поверх program monitor без регулируемой панели. Пользователи просят вернуть «unclamp» и не убивать Lumetri-панель при слиянии режимов |
| Final Cut Pro | 12.3 | Video Scopes | приборы живут ВНУТРИ вьюера (Cmd-7), делят с картинкой её место; до четырёх приборов сразу, 12 раскладок. Waveform: RGB Parade, RGB Overlay, отдельные R/G/B, Luma, Luminance (только PQ), Chroma, Y'CbCr Parade; шкалы IRE, мВ, ниты. Вектороскоп: опорный уровень 100 % или 133 %, поворот сетки Vector/Mark3 (красный сверху либо справа), Show/Hide Skin Tone Indicator. False color нет — вместо него Range Check: УЗОР поверх кадра (Luma / Saturation / All), пороги зависят от цветового пространства проекта |
| VEGAS Pro | 2026.0 | Video Scopes | та же панель, что и в прошлой линейке: в списке нового за 2026 (речь-в-текст, Storyblocks, Z-Depth 2.0, титры Continuum, 4:2:2 на Blackwell) приборов нет ВООБЩЕ. Настройки — четыре галки: Enable ST.2084 HDR Scope, Skin Tone Line, 7.5 IRE Setup, Studio RGB (16/64 to 235/940). Линия skin tone включается галкой, её угол по умолчанию 123° («11 часов») и его можно ПОМЕНЯТЬ. Умолчание Studio RGB осталось от времён, когда проект по умолчанию был 8-bit limited, — и с тех пор не менялось |
| After Effects | 26.x | панель Lumetri Scopes | **живьём не проверено**: справка Adobe (helpx.adobe.com) из окружения проверки недоступна, а в r/AfterEffects за полгода обсуждений приборов нет ни одного. Структурно модель другая: монитора программы нет, прибор меряет вьюер композиции, а глубина проекта переключается (8/16/32 бита на канал), из-за чего вопрос «в каких единицах шкала» решается не так, как в NLE |

## Фишка фишки

**1. «Линия кожи» — это не про кожу, а про угол.** Resolve называет её
честно: «optional skin tone reference graticule (otherwise known as the
In-phase reference)» — то есть ось I старого NTSC-кодирования, а не
измеренная антропология. FCP описывает её как диагональ «between the yellow
and red color bar targets», Vegas ставит на 123°. Практический смысл в том,
ЧТО именно по ней проверяется: угол, то есть ОТТЕНОК. Темнее или светлее
кожа — это радиус (насыщенность) и высота на waveform, а не другой угол,
поэтому линия одна на все типы кожи и «выставить кожу» = довернуть оттенок,
а не подтащить облако к центру. Отсюда и рабочий приём из форумов: замаскировать
всё, кроме лица, — и совмещать с линией уже пик, а не общее облако кадра.
Из пяти редакторов угол линии даёт менять только Vegas; Resolve и FCP —
только включить/выключить.

**2. Сглаживание трассы врёт ровно про то, ради чего прибор открывали.**
У Resolve есть Low Pass Filter — фильтр шума, делающий график читаемым, — и
мануал В ТОМ ЖЕ АБЗАЦЕ предупреждает, что с ним «highlight and shadow
excursions on the graph seem a bit shy of where they actually are», и
рекомендует включать Extents (обводку истинных выбросов). То есть красивая
трасса систематически занижает клиппинг — единственное, за чем к прибору и
приходят. Это же объясняет, почему у каждого прибора там отдельный ползунок
яркости графа: яркая трасса показывает мелкие детали, но перестаёт показывать,
где сигнала МНОГО, а где единичные пиксели.

**3. Шкала — это галка, и её умолчание может врать.** Blackmagic решил
вопрос радикально: WFM и Parade показывают 10-битные full-range данные
0–1023 ВСЕГДА, независимо от Video/Data Level в настройках проекта, —
«a window into how the image is being processed prior to being output».
Vegas сделал наоборот: галка Studio RGB (16/64 to 235/940), причём умолчание
отстало от смены умолчания проекта, и на форуме это разбирают до сих пор —
при неверной галке кадр читается как клиппующий, хотя не клиппует. Тот же
класс — «7.5 IRE Setup»: сдвиг чёрного на 7.5 единиц, наследие NTSC, которое
до сих пор стоит галкой рядом.

## Плюсы

- Прибор ничего не меняет в кадре, поэтому у него нет ни undo, ни состояния
  в модели, ни влияния на эталоны рендера: его можно поставить в продукт,
  где цветокоррекции ещё нет вовсе, и он сразу отвечает на вопрос «этот
  исходник уже клиппит?» и «на каком уровне встал экспорт».
- Он считается по ГОТОВОМУ кадру, а не по плану сцены, поэтому один и тот
  же код обслуживает любой бэкенд отрисовки: разные исполнители отдают одни
  и те же RGBA-байты, и второго пути композиции не появляется.
- Parade даёт баланс белого без единого инструмента цвета: чтобы увидеть
  зелёный сдвиг в тенях, ничего крутить не надо — достаточно, чтобы низ
  зелёного графа стоял выше низов красного и синего.
- Waveform и гистограмма не заменяют друг друга по устройству, а не «по
  вкусу»: waveform сохраняет горизонтальную координату (видно, какая часть
  кадра выбита), гистограмма её теряет, но считает количество пикселей на
  уровне. Поэтому у всех пяти редакторов они стоят рядом.
- False color устроен иначе всех: он не строит график, а перекрашивает сам
  кадр зонами, — и потому читается тем же взглядом и тем же опытом, что
  камерный монитор на площадке.

## Минусы

- Цена — полный проход по кадру на КАЖДЫЙ кадр, и это видно: Blackmagic
  держит для приборов отдельный переключатель качества (High/Medium/Low/Auto)
  и пишет, что закрытие окна приборов возвращает обработку гриду.
- Прибор требует места, и все пятеро платят по-разному: FCP отдаёт приборам
  часть вьюера (картинка ужимается), новый Color mode Premiere накрывает
  ими program monitor, Resolve вынес их в отдельное окно — и получил лишнее
  окно.
- Ручек на один прибор больше, чем на иной эффект: у вектороскопа Resolve
  их десять (тональный диапазон, Colorize, Extents, Combine, яркость графа,
  яркость сетки, Low/High Range, 2×, skin tone, graticule) — и это ради
  ПОКАЗА, без единого изменения картинки.
- Показания зависят от галки шкалы: Studio RGB / 7.5 IRE сдвигают чтение на
  16 и 7.5 единиц соответственно, и ошибка в галке даёт ложный клиппинг —
  ровно то, за чем прибор и открывали.
- Без инструмента правки прибор ставит диагноз и не лечит: он честно скажет
  «пересвечено», но выбор останется между другим дублем и другой экспозицией
  экспорта.

## Что говорят пользователи

> Please add back in the ability to "unclamp" the signal on the waveform
> monitor so we can see if the signal is going above 100 or below 0 like you
> can on the Lumetri scope!
> — u/ShinyWolverine, r/premiere, 16.04.2026, https://www.reddit.com/r/premiere/comments/1sm5fes/announcing_premieres_new_color_mode/ogm1ufm/

> this might just be me, but it seems like when you select a control and drag
> it, the scopes that pop up immediately supercede the footage in the program
> monitor, so i literally can't see the thing i'm working on.
> — u/ClimbThatTree, r/premiere, 02.07.2026, https://www.reddit.com/r/premiere/comments/1sozula/the_new_color_panel_effects_pane_is_a_complete/ov6jqe2/

> Btw, the Skin Tone line is on the Vectorscope. The default is at 11 o'clock
> (123 degrees) which is an alignment reference for Caucasian skin tone.
> — Howard-Vigorita, форум Boris FX (VEGAS Pro), 10.08.2026, https://forum.borisfx.com/t/video-scopes-settings/25018

> 8bit Limited Range was the project default once upon a time but they didn't
> change the scopes default when that changed. Be nice if it had an Auto
> option.
> — Howard-Vigorita, форум Boris FX (VEGAS Pro), 10.08.2026, https://forum.borisfx.com/t/video-scopes-settings/25018

> Nearly every video editing program has a skin tone line in the scopes
> window. Line up the color of the skin on that line (actually just above a
> small amount) and you have accurate Caucasian skin tones.
> — u/denalidenizen, r/colorists, 01.03.2026, https://www.reddit.com/r/colorists/comments/1rbkxcb/skintones_andor/o7z2g23/

Контр-реплика в том же сообществе — против веры в линию как в закон:

> Depends on the scene. For daylight you might want the skin to be close to
> the skin tone line. For Bar Scene... Neon Lights.. the Skin can be anything.
> — u/I-am-into-movies, r/colorists, 21.01.2026, https://www.reddit.com/r/colorists/comments/1qj6a1r/when_do_you_throw_out_the_search_for_good_skin/o0x37mw/

И про то, ради чего прибор вообще заводят раньше грейда, — в ответе на
классический вопрос «почему у меня всё выглядит иначе на другом экране»
(дальше в том же сообщении: контролировать можно только своё окружение,
«That and trusting your ears (eyes) and meters (scopes)»):

> You mix (or color grade) in a properly calibrated and balanced environment.
> — u/whyareyouemailingme, r/davinciresolve, 05.09.2026, https://www.reddit.com/r/davinciresolve/comments/1w80jtx/how_do_you_mix_audio_for_phones_tablets_and_tvs/p7yzfbw/

## Как ложится на Kinotrain

### Что уже есть

- **Готовый кадр в оперативке, каждый новый кадр, на обоих бэкендах.**
  `Player::compose_preview_into` (`src/libs/kt-engine/src/player.rs:555`)
  пишет RGBA8 в переданный `Vec<u8>`; в UI это `KtApp::preview_buf`, и он
  ПЕРЕИСПОЛЬЗУЕТСЯ между кадрами (`src/apps/kt-app/src/app.rs`, ветка
  `if need { … }` вокруг `compose_preview_into`, там же `ColorImage` и
  заливка текстуры).
- **Readback на GPU-пути уже сделан и уже оплачен.**
  `exec.execute_into(&plan, out)` (`player.rs:582`) уходит в
  `Inner::execute` (`src/libs/kt-gpu/src/engine.rs:579`), который читает
  композит обратно в CPU-буфер и мемоизирует байты в `ReadbackCache`
  (`src/libs/kt-gpu/src/pool.rs:138`, бюджет `READBACK_BUDGET` = 64 МБ,
  ключ — хэш плана, `engine.rs:23`). Отдельный readback «ради прибора» не
  нужен: egui всё равно грузит кадр как `ColorImage`.
- **Считать будет ровно раз на НОВЫЙ кадр.** Ключ превью выровнен по сетке
  fps (`preview_key_tick`, инвариант 12, t39), перекомпозит идёт только при
  `need`, а `preview_key` помнит и «полноту» кадра. Прибор, повешенный на
  ту же ветку, наследует эту частоту бесплатно.
- **Рисовать умеем.** Многосерийные rolling-графики с выносками —
  `src/apps/kt-app/src/graphs.rs` (кольцо 180 сэмплов, до 5 серий, общая
  шкала, `Axis::Percent` / `LinearAuto`). Тысячи примитивов за кадр
  собираются в один `egui::Mesh` — `src/apps/kt-app/src/meters.rs` (два
  места), это прямое правило `src/apps/kt-app/CLAUDE.md` (t40). Второй
  готовый приём — собрать картинку в буфер и залить текстурой, как glow-мип
  превью (`app.rs`, каждый 4-й кадр).
- **Место и его память.** Правая панель `meters_col` со своей полосой-
  сепаратором и пределами `METERS_W_MIN`/`METERS_W_MAX`
  (`src/apps/kt-app/src/app.rs`), ширина живёт в `UiState.meters_w`
  (`src/libs/kt-model/src/project.rs:1275`) и ездит в файл проекта через
  `metadata.kt.ui` (`src/libs/kt-otio/src/meta.rs`).
- **Отдельное окно программа уже умеет:** деатач превью через
  `show_viewport_immediate` (п.51, `src/apps/kt-app/CLAUDE.md`) — путь
  «как у Resolve» не закрыт.
- **Есть чем тестировать вёрстку:** `egui_kittest::Harness` гоняет настоящие
  кадры без окна и GPU (`src/apps/kt-app/src/layout_tests.rs`).

### Чего не хватает

- Приборов нет ни одного: `grep -rniE "histogram|vectorscope|parade|false
  ?color|гистограм|вектороскоп"` по `src/` даёт РОВНО одно совпадение —
  гистограмма РАЗНИЦЫ кадров в `kt_gpu::assert_frames_close`
  (`src/libs/kt-gpu/src/lib.rs:39`), тестовая диагностика, не прибор.
- Не решено, ЧТО меряем. Превью считается в `preview_quality` — доле
  разрешения проекта (1/16, 1/8, 1/4, 1/2, 1/1 в
  `src/apps/kt-app/src/app.rs::draw_preview_controls`; умолчание 640×360,
  `src/libs/kt-engine/src/player.rs:264`). Это НЕ даунскейл экспортного
  кадра: качество входит в ключ кэша, источник декодируется сразу в этот
  размер. Прибор по превью на 1/4 честно показывает то, что видит
  пользователь, и может не показать клиппинг, который будет в экспорте
  (`compose_export_into`, `src/libs/kt-engine/src/video.rs:405`, считает в
  полном разрешении проекта).
- Нет шкалы, которую можно подписать честно. После декода тракт — RGBA8;
  `sws_getContext` (`src/libs/kt-ffmpeg/src/source_impl.rs:922`) зовётся
  БЕЗ `sws_setColorspaceDetails`, то есть трактовка диапазона отдана
  умолчанию swscale, а в kt-gpu все текстуры `Rgba8Uint`
  (`src/libs/kt-gpu/CLAUDE.md`). 10-битная шкала 0–1023 у нас была бы
  враньём; честны только 0–255 и 0–100 %. Заодно это вопрос, который у
  Vegas превратился в многолетний форумный тред, — его надо закрыть один
  раз и вслух, а не галкой с отставшим умолчанием.
- Нет «картинки ДО грейда», потому что нет грейда, — но развилку надо
  зафиксировать заранее: «после» — это готовый `preview_buf` и ноль нового
  кода; «до» — это ВТОРАЯ композиция того же тика.
- Нет зума/панорамы превью (искать нечего — их в `app.rs` нет), поэтому
  вопрос «какая часть кадра меряется» сегодня имеет простой ответ «весь
  кадр превью». Но выбор всё равно есть: превью ПРОЕКТА или превью
  МАТЕРИАЛА в бине (t169) — прибор обязан говорить, что именно он мерит.

### Что мешает

- **Шейдерный путь у нас закрыт архитектурно.** `src/libs/kt-gpu/CLAUDE.md`
  запрещает compute-пассы (дефолтный device egui-wgpu на GL-бэкенде
  создаётся с downlevel_webgl2-лимитами, а live-check гоняет llvmpipe) и
  запрещает атомики (условие детерминизма на устройстве, t183). Гистограмма
  без атомиков — это либо 256 фрагментных пассов, либо сортировка; проход по
  кадру 640×360, который уже лежит в памяти, — это 230 тыс. пикселей и
  дешевле любой такой конструкции. На 1/1 в 4K это 8.3 млн пикселей, и там
  решает уже не бэкенд, а ЧАСТОТА: считать обязано на новый кадр, а не на
  UI-кадр.
- **Место.** Правило 13 чек-листа (`src/apps/kt-app/CLAUDE.md`) прямо
  запрещает заводить новую полосу, пока не проверено «нельзя ли поставить на
  уже занятое»; при этом вектороскоп по природе квадратный, а панель метров
  по умолчанию ~70 pt шириной. Это решение по вёрстке, а не по коду, и оно
  главное в фиче.
- **Правило 9 «ничего лишнего» против правды прибора.** Десять
  переключателей вектороскопа Resolve чек-лист не пропустит, но со
  сглаживанием и без обводки выбросов прибор систематически занижает
  клиппинг (см. «Фишка фишки»). Что из этого включено НАВСЕГДА, а что
  вынесено ручкой, придётся решить явно.
- **Любой новый текст в UI** — это каталоги `apps/kt-app/i18n/*.toml`,
  тесты t100/t125/t128 и пересборка сабсетов шрифтов
  (`python3 scripts/subset-fonts.py`).

### Какие инварианты задевает

- **7 (код композиции один)** — НЕ задевает, пока прибор читает готовый
  кадр. Отдельный «шейдер прибора» или композиция «до грейда» немедленно
  превращают его во второй путь, и это надо считать ценой, а не деталью.
- **12 (repaint только при движении)** — задевает прямо: на паузе прибор
  обязан не считаться и не просить перерисовку, по образцу
  `MeterState::is_active()` (t38, t20-repaint). Затухающего состояния у него
  нет вовсе — это в плюс.
- **11 (детерминизм)** — не задевает: прибор только показывает,
  GOLDEN_VIDEO/GOLDEN_AUDIO не трогаются.
- **4 (кэш — единственная правда о coverage) и 8 (скролл/зум не порождает
  декода)** — не задевает при условии, что прибор берёт УЖЕ составленный
  кадр и никогда не заказывает свой.
- Правила UI: **3** (ничего не скачет при смене прибора — ширина
  фиксируется галлеей), **6** (у растяжки панели есть min и max), **9**
  (ничего лишнего), **13** (не тратить место впустую).

## Оценка

| Что | Ответ |
|---|---|
| Объём | M — 2 стадии: один прибор с панелью и решённой шкалой, затем остальные виды |
| Где основная работа | kt-app целиком: расчёт по готовому кадру + отрисовка (меш либо текстура) + вёрстка панели. Движок не трогается: `compose_preview_into` уже отдаёт байты |
| Формат проекта | не трогает: одно поле в `UiState` (`src/libs/kt-model/src/project.rs:1249`) с `#[serde(default)]`, как `meters_w`; перепроверить round-trip kt-otio (t144–t147) |
| Тесты, которые придётся написать | unit на синтетическом кадре (полосы 0/128/255 → известные числа парада, гистограммы и угла вектороскопа; детерминизм между прогонами); `layout_tests.rs` — панель не раздувается и не скачет при смене прибора (класс t76/t86), на паузе repaint отпущен (класс t20-repaint); t100/t125 на новые тексты |
| Зависит от | ни от чего. Обратное верно: цветокоррекция (колёса, кривые, LUT) без приборов — гадание, поэтому эта заметка идёт ПЕРЕД ними |
| Ломает совместимость | нет |

## Вердикт

Берём, и первой в цветовой ветке. Единственная «цветовая» фича, которая
ничего не меняет в картинке: ноль риска для инвариантов 7 и 11, ноль
изменений формата, вся работа в kt-app. Она полезна сама по себе даже
сегодня — показывает клиппинг у исходников и уровень готового экспорта, —
и она же обязательное условие для остальных заметок цветовой ветки
(F-CLR-001 «колёса», F-CLR-003 «вторичная коррекция» и соседние): ставить
баланс белого «на глаз» на некалиброванном мониторе бессмысленно, о чём
монтажёры и пишут.
Главный вопрос фичи не алгоритмический, а два вёрсточных: где прибор живёт
(панель метров / деатач-окно) и в каких единицах подписана шкала — второе
у Vegas не решено до сих пор.

## Источники

- DaVinci Resolve 20 Reference Manual, глава 127 «Viewers, Monitoring, and
  Video Scopes», с. 3008–3022 (шкалы, Low Pass Filter/Extents, качество
  отрисовки, опции каждого прибора) — https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf (дата обращения 2026-09-07)
- Final Cut Pro User Guide: «Video scopes», «Display video scopes»,
  «Waveform monitor display options», «Vectorscope display options»,
  «Detect out-of-gamut colors» — https://support.apple.com/guide/final-cut-pro/video-scopes-ver4a3c67b8/mac (дата обращения 2026-09-07)
- VEGAS Pro 2026 Online Help, «What's New» (приборов в списке нового нет) —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/1-before/whatsnew.htm (дата обращения 2026-09-07)
- Форум Boris FX (VEGAS Pro), тред «Video Scopes Settings?», 09–11.08.2026
  (четыре галки настроек, угол линии skin tone, отставшее умолчание Studio
  RGB) — https://forum.borisfx.com/t/video-scopes-settings/25018 (дата обращения 2026-09-07)
- r/premiere: «Announcing Premiere's new Color mode» (клампинг waveform),
  «The new color panel effects pane is a complete…» (приборы поверх program
  monitor), «What is everyone's current thoughts on Color mode» —
  https://www.reddit.com/r/premiere/comments/1sm5fes/announcing_premieres_new_color_mode/ , https://www.reddit.com/r/premiere/comments/1sozula/the_new_color_panel_effects_pane_is_a_complete/ (дата обращения 2026-09-07)
- r/colorists: треды про линию skin tone и её пределы —
  https://www.reddit.com/r/colorists/comments/1rbkxcb/skintones_andor/ , https://www.reddit.com/r/colorists/comments/1qj6a1r/when_do_you_throw_out_the_search_for_good_skin/ , https://www.reddit.com/r/colorists/comments/1qkycdd/xrite_colour_check_currently_working_with_davinci/ (дата обращения 2026-09-07)
- **Не проверено живьём:** справка Adobe (helpx.adobe.com) из окружения
  проверки недоступна (CloudFront блокирует запросы), поэтому состав настроек
  панели Lumetri Scopes в Premiere и наличие такой панели в After Effects
  взяты не из первоисточника, а из форумных обсуждений; строка After Effects
  в таблице помечена как непроверенная.
