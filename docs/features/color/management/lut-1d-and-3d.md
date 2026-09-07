---
id: F-CLR-004
title: LUT 1D и 3D (.cube) — технические против творческих
status: кандидат
priority: B
cost: M
category: color/management
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: нет
crates: [kt-model, kt-fx, kt-gpu, kt-engine, kt-otio, kt-app]
invariants: [4, 7, 11]
ui_rules: [8, 11, 13, 16]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-CLR-004 — LUT 1D и 3D (.cube) — технические против творческих

## Суть

Монтажёр берёт текстовый файл `.cube`, подсовывает его клипу — и картинка
целиком меняет цвет: плоский серый лог становится нормальным Rec.709 или
приобретает «киношный» оттенок. Никаких колёс, кривых и скопов при этом не
нужно: вся работа сделана заранее, в файле лежит готовая таблица «цвет на
входе → цвет на выходе». Ползунок силы разбавляет результат исходником.

Ровно поэтому LUT — самая дешёвая цветовая фича из существующих: у неё нет
собственного редактора. И ровно поэтому она самая опасная: файл ничего не
знает о том, куда его положили.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Color Grading → LUTs (Camera LUT / Look LUT); отдельный OFX «LUT Filter» | ДВА слота в панели грейда, у каждого свой файл, свой **Strength** и своя галка **Tetrahedral Interpolation**. Справка сама диктует порядок: «first normalize with a Camera LUT, then blend in a Look LUT». Плюс независимый OFX-плагин LUT Filter, который вешается на четыре уровня — Media, Event, Track, Main output (уровни — F-MPH-005), и там переключатель называется «Linear (good) / Tetrahedral (best)». Пресеты Camera LUT — попарно «Log to Linear» и «Log to REC.709». Пользовательские `.cube` в списке НЕ появляются: их кладут внутрь установленной программы (`…\OFX Video Plug-Ins\Vfx1.ofx.bundle\Contents\Resources\AutoLooks\cameraLuts`), иначе каждый запуск начинается с «Browse» |
| Adobe Premiere | 26.x | Lumetri Color: Basic Correction → **Input LUT**; Creative → **Look** + **Intensity** | ДВА слота внутри ОДНОГО эффекта, на разных этажах панели: технический сверху, творческий ниже. Ползунок силы есть только у творческого — штатный ответ на «ослабь мой LUT» звучит как «положи его на adjustment layer и убавь Opacity слоя» (та же композитная развязка, что в F-MPH-006). Путаница слотов — массовая: log-нормализацию грузят в Look, творческий пресет — в Input LUT. Параллельно в 2026-м Adobe выкатила в бету новый **Color Mode** со Styles, где LUT на клип ещё НЕ сделан вовсе |
| DaVinci Resolve | 20.3.2 | LUT на ноде; LUT на исходном клипе; Input/Output LUT проекта; Display/Viewer/Scopes LUT | Слотов не два, а шесть, и их порядок зафиксирован схемой в мануале: Camera Raw → **INPUT LUT** → обработка Color-страницы → **OUTPUT LUT** → Deliver, а **DISPLAY LUT** ответвляется только в монитор и «never applied to rendered media». Внутри одного шага «1D LUTs at each step are always applied before 3D LUTs». Вид интерполяции — настройка ВСЕГО ПРОЕКТА (Trilinear / Tetrahedral), не эффекта. LUT на исходном клипе не переносится ColorTrace, поэтому мануал советует ставить LUT нодой. Библиотека LUT — общесистемный каталог (`/opt/resolve/LUT` на Linux) плюс список дополнительных путей в System Preferences |
| Final Cut Pro | 12.3 | Camera LUT (инспектор Info, уровень медиа) и эффект Custom LUT | Технический слот вынесен из эффектов ВООБЩЕ: камерный LUT живёт в метаданных медиа на уровне библиотеки и, по формулировке Apple, «applied at the media level (across the library) and are applied before any clip effects». Творческий — обычный эффект из браузера, и Apple прямо требует: «Make sure the Custom LUT effect is the last effect in the effects list». То есть порядок задан не кодом, а инструкцией пользователю |
| After Effects | 26.x | эффект **Apply Color LUT**; плюс тот же Lumetri Color | Слотов как таковых нет: LUT — рядовой эффект в стеке слоя, файл выбирается диалогом при перетаскивании эффекта. Порядок «технический → творческий» монтажёр строит сам порядком эффектов или этажами adjustment-слоёв (F-MPH-002); никакой подсказки от программы, что этот LUT — входной, а этот — образ, нет |

## Фишка фишки

**1. Вид интерполяции куба — переключатель, ВИДИМЫЙ пользователю, и его
умолчание выбрано ради совместимости с чужими программами.** 3D LUT — это
решётка узлов (17³ = 4913, 33³, 65³), а всё, что между узлами, домысливает
интерполяция. Resolve пишет про свой умолчательный Trilinear буквально:
«backward compatible with grades that use LUTs from previous versions of
DaVinci Resolve and matches the look of LUTs being applied in other
applications», а про Tetrahedral — «higher image quality LUT and DCTL
processing, with reduced color-banding». То есть знаменитое «один и тот же
`.cube` в Premiere и в Resolve выглядит по-разному» — не легенда и не
плацебо, а документированное следствие настройки, которую в Resolve ставят
на ПРОЕКТ, в Vegas — на КАЖДЫЙ экземпляр эффекта («Linear (good) /
Tetrahedral (best)»), а в Premiere не ставят никак. И увидеть разницу можно
не на любом кадре: на плоской заливке узлы совпадают и оба метода дают одно
и то же — расходятся они на ГРАДИЕНТАХ, полосами. Vegas формулирует это
прямо: тетраэдральная «yields smoother gradients … especially with coarse or
high-contrast LUTs».

**2. Слот у LUT не один, и второй стоит на другом этаже конвейера.**
Технический LUT (log → Rec.709) обязан идти ДО творческой коррекции, а
творческий — ПОСЛЕ; поэтому у Vegas в панели грейда две отдельные строки
(Camera LUT и Look LUT) с собственными файлами и ползунками, у Premiere —
Input LUT в Basic Correction против Look в Creative, у FCP технический слот
вообще вынесен из стека эффектов на уровень медиа («applied before any clip
effects»), а творческому предписано быть последним в списке. У Resolve
вместо двух слотов шесть, и мануал фиксирует их взаимный порядок схемой,
включая правило «1D LUTs at each step are always applied before 3D LUTs».
Ошибка выбора слота не даёт ни ошибки, ни предупреждения — только
«почему-то не как в примере».

**3. Ползунок силы есть НЕ у каждого слота, и это видно по обходным
путям.** Vegas даёт Strength обеим строкам и документирует его как честный
wet/dry: «The LUT filter produces an image that is a blend of the original
image and the color-adjusted image… A setting of 0 would be equal to no LUT
being applied», а справка советует держать творческий LUT «around 0.2 to
0.4». Premiere даёт Intensity только творческому Look — и в результате
типовая инструкция поставщиков LUT для Premiere звучит не «убавь силу», а
«создай adjustment layer, положи LUT на него и убавь Opacity слоя». Это
ровно та же композитная развязка вместо поэффектной, о которой F-MPH-006:
если у эффекта нет своего wet/dry, его отращивают слоем.

## Плюсы

- Фича состоит из парсера текстового файла и одной формулы интерполяции:
  ни редактора кривых, ни колёс, ни скопов, ни гистограммы. Это
  единственная цветовая возможность, у которой НЕТ собственного UI, кроме
  кнопки «выбрать файл» и одного числа.
- Формат `.cube` (Adobe/Iridas) общий на всю отрасль: заголовок
  `LUT_3D_SIZE`/`LUT_1D_SIZE`, необязательные `DOMAIN_MIN`/`DOMAIN_MAX`,
  `TITLE`, дальше построчно тройки чисел. Файл читается блокнотом —
  форумная формулировка: «they're text files that you can inspect with
  notepad… the more points as defined in the header, the more accurate».
  Поддержав его, программа сразу получает библиотеки производителей камер
  и весь коммерческий рынок пресетов, не заводя своего формата.
- Технический LUT делает лог-материал смотрибельным ДО всякой коррекции:
  монтировать можно сразу, не имея ни одного инструмента грейда. Vegas
  так и советует новичку — «start with Log to REC.709 for a fast, good
  looking baseline».
- Сила наложения — обычное число 0..1, то есть попадает в уже готовые
  механизмы (единая сетка параметров и кривые автоматизации, F-AUT-002)
  бесплатно, без отдельного виджета.

## Минусы

- Куб имеет конечную детализацию, и всё между узлами — домысел. Мануал
  Resolve: «one might have a 17x17x17 LUT that specifies 4913 individual
  color transforms… color values falling between the … color transforms
  specified by the LUT need to be interpolated». Отсюда бэндинг на
  градиентах и спор о методе интерполяции, который пользователь обязан
  решать сам.
- Куб определён на 0..1, и всё, что вне, режется: «when a 3D LUT is fed
  values that are outside of the range that LUT is designed to handle, the
  out-of-range data will be clipped… feeding a video signal with
  super-white in it to a 3D LUT that's designed for full-range data (0–1)
  will clip the super-white part of the signal». Отраслевой ответ —
  shaper LUT, то есть ПАРА из 1D и 3D, где 1D загоняет сигнал в диапазон,
  а 3D его возвращает. Одним 3D-кубом задача не решается.
- Файл не знает, что ему подали. Лог-LUT на уже нормализованную картинку
  и Rec.709-LUT на лог оба «работают» и оба врут; поставщики LUT пишут об
  этом прямым текстом: «You cannot use Rec709 LUTs here as the result will
  look very different from the expected outcome». Ни один из пяти
  редакторов не проверяет соответствие и не предупреждает.
- Библиотека LUT живёт ВНЕ проекта — в каталоге программы (Vegas) или ОС
  (Resolve). Значит она не переезжает вместе с проектом и теряется при
  переустановке и обновлении: в Vegas пользовательские кубы кладут внутрь
  папки конкретной мажорной версии.
- В 8 битах LUT плюс последующая коррекция дают заметный бэндинг —
  на форуме Vegas стандартный ответ «you also need to be in 32-bit mode
  (at least at the render stage) for maximum quality». Дешевизна фичи и
  «как в примере» расходятся ровно здесь.

## Что говорят пользователи

> When I want to use them as a Camera LUT, I must click the "Browse for
> Camera LUT" and select the .cube file every time I use Vegas. Can I make
> it persist in the list somehow?
> — ryan4, VEGAS Pro, 16.05.2026, https://forum.borisfx.com/t/how-can-i-make-my-custom-luts-persist-in-vegas/23550

Ответ в том же треде — положить кубы внутрь установленной программы, в
`…\Vegas Pro 2026\OFX Video Plug-Ins\Vfx1.ofx.bundle\`
`Contents\Resources\AutoLooks\cameraLuts`.
То есть «мои LUT пропали» — это не баг, а устройство: библиотека привязана
к версии программы, а не к проекту и не к пользователю.

> I tried and the Luts generally throw the gamma way off and it's hard to
> compensate for it. More harm than good really.
> — steve6, VEGAS Pro, 24.04.2026, https://forum.borisfx.com/t/guidance-for-hlg-to-rec709-in-vp/22762

Классическое «почему мой LUT выглядит выцветшим/не так»: пользователь
подаёт HLG в LUT, рассчитанный на другой вход, и бросает инструмент
насовсем — «включил один раз и выключил навсегда».

> You only need to use the built in "LUT Filter" and set the interpolation
> to "Tetrahedral (best)".
> — Steve_Rhoden, VEGAS Pro, 02.07.2026, https://forum.borisfx.com/t/best-lut-ofx-and-best-color-accuracy-and-most-efficient/24444

Штатный совет по «точности цвета» — не сменить LUT, а сменить
ИНТЕРПОЛЯЦИЮ. В том же треде: «using an ofx camera log-lut with aces would
be a huge mistake» — про наложение технического LUT поверх уже
преобразованной картинки.

> Now if the source in EXR has a high brightness, after applying ACES to
> R.709 we get a clamp. I would like the exposure and gamma operations to
> be applied first, and only then the 3d lut to be applied.
> — AMP, Boris FX, 21.03.2025, https://forum.borisfx.com/t/request-image-preprocessor-luts-pocess-order/19865

Заявка ровно про порядок стадий: клиппинг возникает не от плохого LUT, а
от того, что LUT стоит не на своём месте в конвейере.

> How and where do you add and change LUTs on a per clip basis
> — h.sevush, Adobe Premiere (Beta), 18.08.2026, https://community.adobe.com/announcements-727/color-mode-in-adobe-premiere-1626244

Ответ R Neil Haugen (19.08.2026): «That's coming, the thing isn't nearly
done yet! Still beta, remember.» Adobe в анонсе Color Mode
(Kevin-Monahan, 02.06.2026) противопоставляет свои Styles именно LUT'ам —
«unlike LUTs, every parameter is fully adjustable and non-destructive», —
но клиповых LUT в новом режиме пока нет.

## Как ложится на Kinotrain

### Что уже есть

- **Тракт 8-битный на всём пути.** CPU: `Frame { w, h, rgba: Vec<u8>,
  has_alpha }` в `src/libs/kt-fx/src/video.rs`. GPU: `pub(crate) const
  FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Uint`
  (`src/libs/kt-gpu/src/pool.rs:13`).
- **Цепочка видео-FX пер-слой, одна на превью и экспорт** —
  `apply_video_chain_pooled` (`src/libs/kt-fx/src/video.rs`) через план
  `src/libs/kt-engine/src/video_plan.rs`, инвариант №7. Порядок эффектов
  в цепочке уже редактируется пользователем («первый — ближайший к
  декодеру»), поэтому «до/после коррекции» у нас решается перестановкой
  слотов, а не вторым слотом в UI, как у Premiere.
- **Этаж «поверх нижних» уже есть** — морфер (`MorpherEvent`,
  `Project::effective_clips()` в `src/libs/kt-model/src/project.rs`):
  технический LUT ложится на клип, творческий — на морфер-слой, ровно как
  Camera LUT против Look LUT (F-MPH-001, F-MPH-007).
- **Единая декларация параметров** `params_for!`
  (`src/libs/kt-model/src/project.rs:2308`) даёт спеки, чтение, запись с
  клампом, общую сетку `widgets::params_grid` и кривые
  `automation::draw` — «сила LUT» получает автоматизацию даром
  (домен ключа — материал, F-AUT-002).
- **Относительные пути к файлам в проекте уже решены** —
  `src/libs/kt-otio/src/url.rs` (`media_target_url`, `resolve_media_path`,
  решение Р.24 п.7); запись идёт через `convert.rs:119`.
- **Модель поведения при пропавшем файле уже есть** —
  `src/libs/kt-media/src/placeholder.rs` (`PlaceholderSource`: кадр с
  именем файла, проект открывается, рендер идёт), состав — в
  `KtApp.placeholder_media`, предупреждение в рендер-диалоге —
  `src/apps/kt-app/src/render_dialog.rs:106` (тесты 209/210).
- **Прецедент нечислового поля в редакторе эффектов** —
  `src/apps/kt-app/src/fx_editor.rs`: цветовой пикер у Glow, галка у
  Noise, комбо камеры у Pan3d рисуются ДО общей сетки чисел.

### Чего не хватает

- Разбора `.cube`: `LUT_3D_SIZE` / `LUT_1D_SIZE`, `DOMAIN_MIN` /
  `DOMAIN_MAX`, `TITLE`, дальше `size³` строк по три числа — с проверкой
  количества строк и внятной ошибкой на битом файле.
- Строкового параметра у эффекта. `fx_cast!`
  (`src/libs/kt-model/src/project.rs:2216`) умеет ровно `f64/f32/u32/u8`;
  путь к файлу станет ПЕРВЫМ нечисловым параметром эффекта в системе, и в
  `ParamSpec` ему места нет.
- Понятия «внешний файл, на который ссылается эффект». Единственный
  внешний файл проекта сегодня — медиа из бина, и даже морфер ссылается на
  него через `MorphPatch.media: Option<MediaId>`, а не путём.
- Места под сам куб на GPU: пассы биндят ровно три текстуры
  `texture_2d<u32>` (T0/T1/T2, `src/libs/kt-gpu/src/shaders/common.wgsl`)
  при одной 64-байтной униформе.

### Что мешает

1. **Интерполяция куба — ровно тот случай, где CPU и GPU обязаны
   разъехаться, и крейт это уже запретил.** `src/libs/kt-gpu/CLAUDE.md`:
   «Сэмплеров нет вообще: субтексельная сетка фильтрации гуляет по
   драйверам — билинейка руками (4 texelLoad, зеркало `bilinear_into`)».
   Аппаратная трилинейная фильтрация 3D-текстуры — именно то, что этим
   правилом запрещено; вдобавок `Rgba8Uint` целочисленный и в WGSL не
   сэмплируется в принципе. Значит трилинейку (8 узлов) или тетраэдр
   (4 узла) придётся писать руками и держать зеркалом скаляра и AVX2-ядра
   kt-fx одним коммитом. Проверять это будет t181
   (`src/libs/kt-gpu/tests/parity.rs`) через `assert_frames_close`, а
   допуск `FrameTol::BILINEAR` — это `max_lsb: 2`, `mean_lsb: 0.5`,
   `outlier_frac: 0.001`: подкручивать пороги под аппаратную фильтрацию
   запрещено (инвариант №11, «провал на грани — расследование, не
   подкрутка»).
2. **Куб некуда положить, не тронув общий layout.** Сейчас «Униформа одна
   на все пассы (64 байта, generic-слоты) — один bind group layout на весь
   движок». Отдельная `texture_3d` меняет layout ВСЕХ пассов; альтернатива
   — уложить куб плиткой в свободный T2 и адресовать вручную. Выбор из
   двух неприятных вариантов, и он архитектурный, а не косметический.
3. **Ключ кэша не видит содержимого файла.** `fx_chain_hash`
   (`src/libs/kt-engine/src/video.rs:153`) считает
   `xxh3_64(format!("{fx:?}|{auto:?}|…"))`: путь в ключ попадёт, а правка
   самого `.cube` на диске — нет. Пост-FX кэш (`PostFxVideo`) и
   `ReadbackCache` GPU молча отдадут кадры со старым грейдом, что прямо
   нарушает инвариант №4 («кэш — единственная правда»).
4. **Пропавший LUT — это не пропавшее медиа.** Вся машинерия
   относительных путей и заглушек привязана к бину: релятивизирует пути
   только `kt-otio/src/convert.rs:119` для `media.path`, предупреждает
   перед рендером только `render_dialog.rs:106`. Для файла LUT это
   придётся либо продублировать, либо признать LUT «медиа особого рода» —
   и тогда он поедет в бин, где ему по смыслу не место.
5. **8 бит.** Наш тракт u8 сквозной, а форумная практика требует под LUT
   32 бита хотя бы на рендере. Фичу это не отменяет (у Vegas тот же
   пользователь пишет, что 8-битный LUT ему нравится больше 32-битного
   ACES), но обещать «как в Resolve» на градиентах нельзя, и в UI это
   должно быть честно.

### Какие инварианты задевает

- **№7** — код композиции один: LUT считается в цепочке `frame_plan`, а не
  «на GPU по-своему»; начатый на GPU экспорт не может доехать на CPU.
- **№11** — детерминизм: CPU-путь байт-в-байт против `GOLDEN_VIDEO`, GPU —
  только через `FrameTol`/`assert_frames_close`, ad-hoc epsilon запрещены.
- **№4** — кэш единственная правда о прогретости: ключ обязан меняться при
  смене LUT (см. упор 3).
- Правило крейта kt-fx: скаляр + AVX2 + WGSL одним коммитом, SIMD
  байт-в-байт со скаляром. Про «умолчание = прежнее поведение» здесь
  просто: эффекта раньше не было, golden-эталоны не двигаются.
- UI чек-лист: **п.8** (кнопка выбора файла рисуется в том же
  прямоугольнике, что заменяемое значение, а не «поставленным» диалогом),
  **п.11** (новые строки → сабсеты шрифтов и t100), **п.13** (имя файла
  LUT — на уже занятое место, не новой полосой), **п.16** (пропавший или
  битый `.cube` — красная ошибка, а не блёклая подсказка).

## Оценка

| Что | Ответ |
|---|---|
| Объём | M — 2–3 стадии раунда |
| Где основная работа | kt-fx (парсер `.cube` + скаляр + AVX2), kt-gpu (WGSL-зеркало и место под куб), kt-model (нечисловой параметр эффекта), kt-app (выбор файла, ошибка пропавшего LUT), kt-otio (путь в файле проекта) |
| Формат проекта | нужна новая версия конверта `metadata.kt`: у `FxInstance` появляется строковое поле, и файлы прошлых версий обязаны читаться как есть (прецедент — t212 «чтение файлов до v3») |
| Тесты, которые придётся написать | разбор `.cube` (корректный, битый, `DOMAIN_MIN/MAX`, 1D и 3D, размеры 17/33/65); golden CPU для 3D LUT в `video_fx.rs`; SIMD-паритет `simd_parity`; конформанс CPU↔GPU в `parity.rs` (t181) — по допуску, не по epsilon; round-trip модели и `kt-otio/tests/format.rs` на относительный путь; поведение при пропавшем файле (по образцу t209/t210); t100 на новые строки i18n во всех пяти каталогах |
| Зависит от | ничего (ни цветокоррекции, ни scopes не требует); ползунок силы — та же механика, что в F-MPH-006, кривая — F-AUT-002 |
| Ломает совместимость | нет: старые проекты читаются, эталоны не перегенерируются |

## Вердикт

**Берём — как первый шаг в цвет.** LUT единственный в своей категории не
требует ни редактора параметров, ни скопов: файл плюс интерполяция, а
«цветокоррекция и LUT» стоят в роадмапе, выбранном самим владельцем
(`docs/user-feedback.md`, Р.18 п.6; строка лендинга «Цветокоррекция:
кривые, баланс белого, LUT и скопы»).

Дёшево это, однако, ровно до места интерполяции: у нас GPU и CPU обязаны
считать ОДНО (инвариант №7) при том, что kt-gpu намеренно отказался от
аппаратной фильтрации, — значит трилинейка/тетраэдр пишутся руками в трёх
экземплярах (скаляр, AVX2, WGSL) и проверяются t181. Это цена честного
паритета, и она известна заранее, а не выясняется в середине раунда.

Что НЕ надо тащить следом: отдельный «слот Input LUT» рядом со «слотом
Look» — у нас порядок применения уже выражен порядком эффектов в цепочке и
этажами морфера, и второй слот был бы парным способом сделать одно и то же
(п.17 чек-листа UI). Ошибку выбора слота, на которой спотыкаются новички
Premiere, нам заводить незачем.

## Источники

- VEGAS Pro 2026 Online Help, «LUTs» (Color Grading window): Camera LUT /
  Look LUT, Strength, Tetrahedral Interpolation, `.cube` —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/8-design/colorgrading_luts.htm
  (дата обращения 2026-09-07; страница помечена «Last modified on
  April 15, 2026»)
- VEGAS Pro 2026 Online Help, «LUT Filter» (OFX): уровни применения,
  Interpolation Linear/Tetrahedral, Strength как смесь с оригиналом —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/microcontent/Microcontent/_FxPlugins/lut-filter.htm
  (дата обращения 2026-09-07)
- VEGAS Pro 2026 Online Help, «Color Grading» —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/8-design/colorgrading.htm
  (дата обращения 2026-09-07)
- DaVinci Resolve 20 Reference Manual (Blackmagic Design, июль 2025):
  гл. 6 «Project Settings» — схема Input/Output/Display LUT, правило «1D
  перед 3D», 3D Lookup Table Interpolation (Trilinear/Tetrahedral),
  каталоги LUT, «33x33x33 cubes with 32-bit floating point»; гл. 150
  «Using LUTs» — shaper LUT и клиппинг за пределами 0–1, LUT на исходном
  клипе против LUT на ноде; гл. 9 «Process Node LUTs in» —
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf
  (дата обращения 2026-09-07)
- Final Cut Pro User Guide, «Apply LUTs» —
  https://support.apple.com/guide/final-cut-pro/apply-luts-ver24f966423/mac
  (дата обращения 2026-09-07)
- Final Cut Pro User Guide, «Use custom LUT effects to color correct
  ProRes RAW video» («Make sure the Custom LUT effect is the last effect
  in the effects list») —
  https://support.apple.com/guide/final-cut-pro/custom-lut-effects-color-correct-prores-raw-ver572cacef4/mac
  (дата обращения 2026-09-07)
- Adobe Community, «Color Mode in Adobe Premiere» (Kevin-Monahan,
  02.06.2026; ветка про LUT на клип — h.sevush 18.08.2026,
  R Neil Haugen 19.08.2026) —
  https://community.adobe.com/announcements-727/color-mode-in-adobe-premiere-1626244
  (дата обращения 2026-09-07)
- Boris FX Forum (VEGAS): треды 23550 (персистентность пользовательских
  LUT), 22762 (HLG → Rec.709, «Luts throw the gamma way off»), 24444
  (интерполяция, ACES против LUT, 32 бита), 19865 (порядок стадий и
  клиппинг), 25314 (баг замены LUT в панели грейда, build 189) —
  https://forum.borisfx.com/ (дата обращения 2026-09-07)
- Lutify.me Docs, «How do I apply 3D LUTs in Adobe Premiere Pro» (Basic
  Correction → Input LUT для лог-нормализации, Creative → Look для
  творческого) и «How can I change the strength/opacity of the 3D LUTs in
  Adobe Premiere Pro» (обход через Opacity adjustment-слоя), «How do I
  apply 3D LUTs in Adobe After Effects» (эффект Apply Color LUT) —
  https://lutify.me/docs/ (дата обращения 2026-09-07)
- Lutify.me, «How to correctly color grade RED IPP2 footage in Premiere
  Pro» («You cannot use Rec709 LUTs here…») —
  https://lutify.me/how-to-correctly-color-grade-red-ipp2-footage-in-premiere-pro/
  (дата обращения 2026-09-07)

**Оговорка о полноте.** На дату проверки `helpx.adobe.com` был недоступен
ни напрямую, ни через два независимых внешних прокси (обрыв на уровне
HTTP/2), поэтому строки Premiere и After Effects опираются на анонс в
официальном сообществе Adobe и на документацию поставщика LUT, а не на
справку Adobe. При следующей ревизии заметки строки Premiere/AE проверить
по `helpx.adobe.com` заново — прежде всего наличие и поведение Intensity у
Creative → Look и отсутствие ползунка силы у Basic Correction → Input LUT.
