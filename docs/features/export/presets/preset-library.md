---
id: F-EXP-001
title: Библиотека экспорт-пресетов
status: кандидат
priority: B
cost: M
category: export/presets
editors: [premiere-26, vegas-pro-2026, resolve-21, fcp-12, after-effects-26]
we_have: нет
crates: [kt-app, kt-engine, kt-ffmpeg, kt-cli]
invariants: [7, 9, 10, 11]
ui_rules: [4, 13, 16, 19, 20]
round: null
point: null
tests: []
depends_on: [F-AUT-004, F-TXT-002]
checked: 2026-09-07
---

# F-EXP-001 — Библиотека экспорт-пресетов

## Суть

Один и тот же монтаж уезжает в четыре места: заказчику мастер, в мессенджер
превьюшка, на YouTube длинная версия, в сторис вертикальный кусок. Каждый раз
это одни и те же полтора десятка решений — контейнер, кодек, битрейт,
разрешение, частота кадров, звук, имя файла. Пресет даёт набору ИМЯ, переживает
закрытие проекта и переезжает на вторую машину. Вопрос заметки не «можно ли
сохранить галочки», а **где проходит граница пресета**: что он несёт сам, а что
молча берёт у проекта — потому что ровно на этой границе рождается «в превью
было не так» и «откуда взялись чёрные полосы».

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| Adobe Premiere | 26.x | Export preset `.epr` + Export Preset Manager | пресет — отдельный ФАЙЛ, его импортируют и раздают; лежит он не у Premiere, а в профиле Media Encoder (`Documents/Adobe/Adobe Media Encoder/<версия>/Presets`), и «экспортировать» его наружу умеет только AME — отдельное приложение с собственной очередью (сюжет F-EXP-002). Мажорное обновление регулярно оставляет пользователя без библиотеки. Есть «Match Source» — пресет, берущий параметры у секвенции. Preset Manager появился в 22.3.1 (2022) [часть деталей НЕ ПРОВЕРЕНА: `helpx.adobe.com` из окружения не открывается] |
| VEGAS Pro | 2026 | Render template в окне «Render As» | шаблоны сгруппированы по ФОРМАТУ (свой список у MAGIX AVC/AAC, свой у Sony MXF, свой у стороннего Voukoder), пользовательский создаётся кнопкой «Customize Template» там же. В 2026 в шаблон рендера добавили настраиваемую цветовую информацию для ProRes-экспортов. Диалог не помнит выбранную группу между запусками (цитата ниже) |
| DaVinci Resolve | 21 | Deliver: пресеты рендера + очередь + Quick Export | «The deliver page gives you total control over all encoding options and formats, along with a render queue for exporting multiple jobs!» — ПРЕСЕТ и ЗАДАНИЕ очереди здесь разные сущности. Правила вписывания в пресет не входят: они живут в настройках ПРОЕКТА двумя отдельными ступенями — Input Scaling и Output Scaling с вариантами вида «scale entire image to fit». В 21 в render settings приехал MainConcept для H.265 и MV-HEVC 4:2:0/4:2:2 |
| Final Cut Pro | 12 | Destination (Settings → Destinations) | назначение = Format + Video Codec + Resolution + Audio Format + галочки (segmentation, chapter markers) + **Action** («Save only», открыть в QuickTime/Compressor, положить в Photos). Создаются перетаскиванием шаблона в список и командой «Duplicate», складываются в **Bundle** («New Bundle from Selection»), живут в настройках приложения. Цветовое пространство в назначение НЕ входит принципиально. Имя выходного файла назначение не параметризует |
| After Effects | 26 | ДВА независимых списка: Render Settings templates и Output Module templates | что рендерить (качество, разрешение, прокси, диапазон) и во что писать (формат, кодек, Stretch, Crop, аудио) — разные шаблоны, выбираются двумя разными выпадающими списками в очереди рендера [НЕ ПРОВЕРЕНО по первоисточнику: `helpx.adobe.com` недоступен] |

## Фишка фишки

**1. Правило приведения (растянуть / вписать с полями / обрезать) не живёт в
пресете НИ У КОГО — и именно поэтому «откуда чёрные полосы» остаётся вечным
вопросом.** У Resolve оно вынесено в настройки ПРОЕКТА и разложено на две
независимые ступени: Input Scaling (что делать с чужим по размеру клипом на
таймлайне) и Output Scaling (что делать с таймлайном, не совпавшим с выдачей),
у каждой свой набор вариантов вида «scale entire image to fit». Пресет Deliver
про них не знает, поэтому один и тот же пресет в двух проектах даёт разную
геометрию — и жалоба «поставил 2160×3840 везде, исходник тот же 2160×3840, а
полосы сверху всё равно есть» (цитата ниже) существует именно потому, что
третья настройка сидит в другом окне. У Premiere та же логика спущена ещё ниже,
на КЛИП («Set to Frame Size» против «Scale to Frame Size»), — и человек,
пришедший оттуда, формулирует свою претензию к Resolve как отсутствие
match-source сразу в двух местах: у таймлайна и у экспорта. Та же развилка
повторяется с цветом, и там она видна документально: справка FCP пишет про
Color Space назначения — «determined by the project settings and the destination
video codec», то есть назначение цветом не владеет ПО УСТРОЙСТВУ; VEGAS Pro
2026 в том же году сделал ровно обратное — в release notes сборки 66
(27.03.2026) значится «Customizable Color Information now available for render
template in ProRes exports». Один год, два противоположных ответа на один
вопрос. Отсюда общий вывод: слово «пресет» у всех пяти обозначает договор
РАЗНОЙ ШИРИНЫ, и непереносимы пресеты не потому, что форматы файлов разные, а
потому, что за границей пресета у каждого лежит своё.

**2. Пресет фиксирует ЖЕЛЕЗО автора, а не выдачу, и ломается от вещей, которых
в нём нет.** Шаблон с аппаратным энкодером — не «быстрее то же самое», а
привязка к конкретной машине: «Are you trying to render using a template with
NVENC option and a graphics driver later than the 581 series? If so it will
fail» (цитата ниже) — сломала шаблон СМЕНА ВЕРСИИ ДРАЙВЕРА, при неизменных
пресете, проекте и программе. Тот же класс — числа: битрейт в шаблоне это
ЗАПРОС к энкодеру, а не свойство файла (320 kbps в шаблоне → 125 kbps в файле,
и «There is no switch to change between variable and constant bitrate»).
Отсюда правда про пресеты «для площадки»: требования загрузки формулируются в
терминах готового файла, пресет — в терминах запроса к кодеку, а площадка
перекодирует всё равно («The file I uploaded is almost 6GB, the file downloaded
from YouTube is only 138MB»). Ценность такого пресета в том, чтобы файл
ПРИНЯЛИ, а называется он именем площадки и потому обещает качество.

**3. Имя выходного файла пресету не принадлежит, хотя вся информация для него у
пресета есть.** В FCP можно собрать Bundle из четырёх назначений и одним
нажатием получить четыре файла — но различить их по имени нечем: на прямой
вопрос «как дописать кодек или имя настройки суффиксом» автор профильного
руководства отвечает «FCP does not support automatically appending the codec to
the exported file name». Четыре файла с одинаковым именем в четырёх папках —
это и есть «пресет, который знает про себя всё, кроме того, как назваться». У
нас, к слову, ситуация зеркальная и оттого забавная: шаблон имени
(`kt-<проект>-<регион>-<длина>s.mp4`) есть и работает, а пресета, чьё имя туда
можно было бы подставить, нет.

## Плюсы

- Пресет отдельным ФАЙЛОМ обслуживает сразу два приложения: интерактивный
  экспорт и очередь читают один `.epr`, поэтому «в очереди получилось не то,
  что в редакторе» не может случиться из-за расхождения настроек.
- Действие после экспорта внутри назначения (FCP Action) убирает ручной шаг
  целиком: «отрендерить и открыть в Compressor» — не два действия человека, а
  одно свойство пресета, и оно не забывается.
- Bundle превращает выдачу в СПИСОК: добавили площадку — добавили строку, и все
  следующие релизы уезжают полным комплектом без «а вертикальный ты сделал?».
- Ограничение выбора исходником («Available choices are based on the source clip
  or project media format», FCP) ловит несовместимость кодека ДО рендера, а не в
  конце трёхчасового просчёта.
- Разделение «пресет» и «задание очереди» (Resolve Deliver) позволяет одному
  пресету обслуживать десять заданий с разными диапазонами и именами — то, что
  у нас сейчас невыразимо в принципе.

## Минусы

- Пресет фиксирует запрос, а не результат: битрейт из шаблона файлом не
  гарантируется, а переключателя VBR/CBR может не быть вовсе — исправить
  расхождение тогда нечем.
- Разделение «что в пресете, а что в проекте» нигде не показано. Пользователь
  видит два списка настроек и не видит третьего — списка того, что пресет молча
  берёт снаружи. Все жалобы «экспорт не похож на превью» живут на этом стыке.
- Шаблон, привязанный к ФОРМАТУ (VEGAS), заставляет заводить один и тот же
  смысл в нескольких списках: «1080p для клиента» существует отдельно под AVC,
  отдельно под HEVC, отдельно под ProRes, синхронизировать их нечем; вдобавок
  диалог не помнит, в какой группе ты был в прошлый раз.
- Библиотека, лежащая в профиле ВЕРСИИ приложения, не переживает обновление:
  пресеты исчезают при переходе на новую мажорную версию, а спасённые файлы
  импортируются обратно не всегда («The selected preset is not a valid MPEG2
  preset. No exporter found for this preset»). Тот же класс проблемы для другого
  типа пресетов разобран в **F-AUT-004**.
- Пресет с прибитым аппаратным энкодером описывает машину автора: смена
  драйвера ломает его без единого изменения в самом пресете.
- Пакетный экспорт по бандлу — всё или ничего: выбрать из бандла один пресет
  нельзя, а падение одной ветки роняет результат целиком.
- Пресеты площадок стареют быстрее программы: требования сервисов меняются без
  нового релиза редактора, а имя пресета продолжает обещать актуальность.

## Что говорят пользователи

> Are you trying to render using a template with NVENC option and a graphics
> driver later than the 581 series? If so it will fail.
> — andyrpsmith, форум Boris FX / VEGAS Pro, 18.04.2026,
> https://forum.borisfx.com/t/render-problem/22547

> Hello! I would like to know how this is possible. So, I have a render template
> that has 320 kbps but the rendered video is significantly lower with 125 kbps.
> — Hadrian Embalsado (newecreator), форум Boris FX / VEGAS Pro, 01.04.2026,
> https://forum.borisfx.com/t/rendered-audio-is-of-significantly-lower-bitrate-than-the-bitrate-of-the-render-template/21593

> It's a variable bitrate encode and you need to use media info to see the true
> maximum bitrate used by the encoder.
> — VegasFlorian (модератор), там же, 02.04.2026,
> https://forum.borisfx.com/t/rendered-audio-is-of-significantly-lower-bitrate-than-the-bitrate-of-the-render-template/21593

> Project and timeline resolution set to 2160×3840 (Use vertical video box
> ticked). Output scaling set to the same as timeline. However, i'm still
> getting black bars at top of image in the record viewer.
> — Sean Lennon, Creative COW (DaVinci Resolve), 20.01.2026,
> https://creativecow.net/forums/thread/black-bars-on-export-issue/

> I'm used to Premiere where I can a) match the timeline settings to the original
> footage and b) match the export settings to the original footage. But
> apparently you can do neither in Resolve.
> — Tim Gallaher, Creative COW (DaVinci Resolve), 11.02.2026,
> https://creativecow.net/forums/thread/matching-settings-of-original-dv-prores-422-footage-in-exports-for-resolve/

> I'd love to have the codec or name of the export setting as a suffix, but can't
> figure out how to add it. Maybe this has been removed?
> — Patrick, комментарий на larryjordan.com, 30.06.2023 (ответ Larry Jordan в тот
> же день: «FCP does not support automatically appending the codec to the
> exported file name»),
> https://larryjordan.com/articles/fcp-x-export-to-multiple-destinations-at-once/

> I just upgraded from CS6 to CC. All of my export setting presets were wiped out
> with the install.
> — Dave Fleming, Creative COW (Premiere Pro), 02.05.2014,
> https://creativecow.net/forums/thread/export-setting-presets/

> what matters most to me is getting what I see when I render and upload to
> YouTube. I've only been able to achieve that in the past by setting Vegas
> projects to full-range and rendering with Vegas default ltd-range.
> — Howard Vigorita, форум Boris FX / VEGAS Pro, 08.04.2026,
> https://forum.borisfx.com/t/vp2026b066-color-differences-issue-in-8-bit-full-pixel-range/22064

> The file I uploaded is almost 6GB, the file downloaded from YouTube is only
> 138MB.
> — Chippe01, форум Boris FX / VEGAS Pro, 31.08.2026,
> https://forum.borisfx.com/t/version-2026-0-3-b-189-oh-soooo-close/25227

## Как ложится на Kinotrain

### Что уже есть

**Диалог настроек рендера — `src/apps/kt-app/src/render_dialog.rs`,
`RenderSetup` (строка 46) и `draw_render_setup()` (строка 267). Полный
перечень того, что вообще можно выбрать, — шесть строк грида:**

1. **Video codec** — комбо: «Auto (hardware first)» либо конкретный энкодер из
   `kt_media::muxer::available_encoders()`.
2. **Quality** — переключатель CRF / Bitrate: CRF-слайдер 14…32 (умолчание 23)
   либо логарифмический слайдер битрейта 0.5…80 Mbit/s (умолчание 8 Mbit/s).
3. **Audio bitrate** — комбо 128 / 192 / 256 / 320 kbps (умолчание 192).
4. **Resolution** — «Project (W×H)» либо 1280×720 / 1920×1080 / 2560×1440 /
   3840×2160 / «Max (W×H)» по источникам.
5. **Audio** — не выбор, а НАДПИСЬ: `AAC, 48 kHz stereo`.
6. **Container** — тоже надпись: `MP4 (+faststart)`.

Плюс предупреждение о недоступных исходниках (п.208 `docs/porting-notes.md`) и
кнопки «Render…» / «Cancel». Диапазон в окне не показан вообще: `launch_render`
(строка 180) молча берёт `project.region`, а если региона нет — весь проект.

**Что доезжает до движка** — `ExportOptions`
(`src/libs/kt-engine/src/export.rs:337`): ровно пять полей `quality`,
`video_bitrate`, `audio_bitrate`, `force_encoder`, `resolution`. Оттуда в
`MuxConfig` (`src/libs/kt-ffmpeg/src/lib.rs:67`) — те же пять плюс `width`,
`height`, `fps`.

**Муксер умеет ровно одно сочетание.** `src/libs/kt-ffmpeg/src/muxer_impl.rs`:
контейнер зашит строкой `"mp4"` (строка 64), аудио — только `aac`, иначе явная
ошибка «system FFmpeg has no AAC encoder» (строка 173), видео — пять кандидатов
`VIDEO_ENCODERS` (строка 15): `h264_nvenc` → `h264_vaapi` → `h264_qsv` →
`libx264` → `mpeg4`. Пиксельный формат всегда `YUV420P`, `gop_size = fps*2`,
`max_b_frames = 0`, `movflags=+faststart`. **Ни одного цветового тега не
пишется** (`color_primaries`/`color_trc`/`colorspace` не трогаются), метаданных
файла — тоже.

**Автовыбор энкодера уже устроен правильно, и это половина ответа на вопрос про
пресет с прибитым кодеком.** `Muxer::open` перебирает кандидатов и проверяет их
НАСТОЯЩИМ `avcodec_open2` (наличие в сборке ничего не значит — nvenc без
драйвера падает); при `force_encoder` цикл сужается до одного имени, и неудача
даёт ошибку `no video encoder could be opened (last error: …)` без всякого
фолбэка. Это инварианты №9 и №10 в действии, закрыто тестом 25
(`src/libs/kt-engine/tests/export_options.rs`, включая
`t25_force_unknown_encoder_is_explicit_error`).

**Шаблон имени файла существует и уже параметризован — только не
пользователем.** `render_file_name()` (`render_dialog.rs:129`) собирает
`kt-<проект>-<регион>-<длина в секундах>s.mp4`, санитизируя куски
(`sanitize_name_part`, строка 113), а `default_render_name()` (строка 147)
подставляет имя метки-диапазона ТОЛЬКО если она точно совпала с регионом. Тест
— t201 в `src/apps/kt-app/src/layout_tests.rs:4252`. Важно: шаблон поставлен ПО
ПРЯМОЙ ПРОСЬБЕ пользователя (Р.33 п.9, `docs/user-feedback.md`: «в целом шаблон
для всех рендеров всегда `kt-<projectname>-<region_name>-<его длинна в
секундах>`»), поэтому параметризация не имеет права менять умолчание.

**Экспорт с разрешением ≠ проектному считается честно, не апскейлом в конце.**
`export_mp4_with_backends` (`export.rs:391`) превращает `resolution` в
`Quality{width,height}`, и весь композит строится сразу в целевом размере —
`VideoRenderer::frame_plan` тот же, что у превью (инвариант №7). Это лучше, чем
у большинства: у нас нет ступени «отрендерить в размер таймлайна, потом
отмасштабировать».

**kt-cli знает про экспорт ровно одну настройку.**
`src/apps/kt-cli/src/main.rs`: `kt-cli export <in.otio> <out.mp4> [encoder]`,
всё остальное — `ExportOptions::default()`. Пресет — естественный (и,
пожалуй, единственный не уродливый) способ дать CLI то же, что GUI, не
изобретая десяток флагов.

### Чего не хватает

- **Имени.** `RenderSetup` — поле `KtApp` (`app.rs:255`), инициализируется
  `Default::default()` (строки 487 и 623). Настройки не переживают ни перезапуск
  программы, ни ОТКРЫТИЕ ДРУГОГО ПРОЕКТА (`KtApp` при открытии пересоздаётся —
  `src/apps/kt-app/CLAUDE.md`, раздел про GPU). Каждый рендер начинается с
  умолчаний. Даже «запомнить последнее» у нас нет — то, отсутствие чего в AME
  люди называют дефектом.
- **Частоты кадров в экспорте нет вообще.** `fps` берётся из
  `project.settings.fps` (`export.rs:423`), переопределить нечем, поля в
  `ExportOptions` нет. Разрешение переопределяется, частота — нет; асимметрия
  ничем не объяснена.
- **Контейнера, кроме MP4, и кодеков, кроме h264+AAC.** Пресет «ProRes для
  заказчика» упирается не в UI, а в `muxer_impl.rs`.
- **Правил приведения нет — точнее, правило ЕСТЬ, оно «растянуть», и его никто
  не выбирал.** Кадр декодируется сразу в целевой размер: `to_rgba`
  (`src/libs/kt-ffmpeg/src/source_impl.rs:904`) зовёт `sws_getContext(f.width,
  f.height, …, dst_w, dst_h, …)` — соотношение сторон не участвует. Ни
  `ProjectSettings` (`src/libs/kt-model/src/project.rs:1489`: только
  `fps`/`width`/`height`/`sample_rate`), ни `Quality`
  (`src/libs/kt-media/src/lib.rs:31`) не знают про pixel aspect ratio. Дойти
  до этого можно штатным кликом: пункт «Max (W×H)» берёт максимум ширины и
  максимум высоты НЕЗАВИСИМО (`max_source_resolution`, `app.rs:1646`), поэтому
  проект с горизонтальным 1920×1080 и вертикальным 1080×1920 предложит «Max
  (1920×1920)» — соотношение, которого нет ни у одного источника, и весь ролик
  поедет квадратом. У всех чужих редакторов на этом месте стоит явный выбор из
  трёх-четырёх правил; у нас — молчание.
- **Цветовых тегов и метаданных в файле.** Тот самый full-range/ltd-range сюжет
  из цитаты у нас даже не выражен.
- **Действия после рендера** (открыть папку, поставить в очередь) — ни самого
  действия, ни места, где оно жило бы.

### Что мешает

**1. Хранилищу негде жить, и это уже ТРЕТЬЯ фича подряд, упирающаяся в одно и то
же.** Проект — один файл `.otio` (`src/libs/kt-otio/`), и `KtSettings`
(`src/libs/kt-otio/src/meta.rs:114`) несёт только `fps`/`width`/`height`/
`sample_rate`; настройкам экспорта там места нет и по смыслу быть не должно —
пресет живёт МЕЖДУ проектами. Всё, что у программы есть на уровне приложения, —
каталог `$XDG_DATA_HOME/kinotrain/` с единственным файлом автосейва
(`autosave_path()`, `src/apps/kt-app/src/app.rs:4770`) и key-value-хранилище
eframe на две строки (`lang`, `last_project` — `app.rs:5036`). Файла настроек у
kt-app нет вообще. Ровно этот вывод уже записан в **F-AUT-004** (библиотека
FX-пресетов, раздел «Что мешает», п.3) и в **F-TXT-002** (шаблоны титров).
Три независимые фичи самостоятельно пришли к «нужно постоянное пользовательское
хранилище вне проекта» — значит это не деталь трёх заметок, а **отдельная
задача со своим жизненным циклом**: бэкап, перенос на вторую машину, конфликт
имён, миграция схемы при изменении `ExportOptions`/`FxInstance`, поведение при
смене версии программы. Судьба `.prfpset` у Premiere (цитата про стёртую
установкой библиотеку) показывает, чем платят за монолит в профиле версии.
Решать этот вопрос внутри любой из трёх заметок нельзя (правило 8 `README.md`),
но и делать три раза подряд по-своему — тем более.

**2. Пресет с прибитым энкодером против нашего автовыбора.** Правильное
поведение у нас УЖЕ выбрано и реализовано: `force_encoder` = «только этот, не
открылся — ошибка», без тихого съезда (инварианты №9/№10 и правило UI 16
«невозможность действия показывается как ОШИБКА»). Цитата про NVENC и драйвер
581 подтверждает, что тихий съезд был бы хуже: пользователь получил бы файл,
сделанный не тем, чем он просил, при неизменном имени пресета. Незакрытым
остаётся другое: комбо в диалоге показывает энкодеры по НАЛИЧИЮ В СБОРКЕ
(`available_encoders()` фильтрует `avcodec_find_encoder_by_name`,
`muxer_impl.rs:24`), а не по «реально открывается», — то есть выбрать
`h264_nvenc` на машине без драйвера можно уже сегодня. Пресет сделает эту
ситуацию переносимой между машинами, поэтому у него обязаны быть ДВА разных
значения: «Авто» и «именно этот энкодер», причём «Авто» — умолчание (правило UI
4: полезный режим включён по умолчанию). Формулировка и место — в раунде.

**3. Диалогу некуда девать список (правило UI 13, «не тратить место впустую»).**
Окно уже держит шесть строк грида, предупреждение о недоступных исходниках и
две кнопки, и оно `resizable(false)` с якорем в центре
(`render_dialog.rs:275-280`). Отдельная панель со списком пресетов ради
элемента, которым пользуются раз в сессию, — ровно тот дефект, против которого
правило написано. Эталоны из чек-листа (имя метки на строке подписей времени,
имя группы бина в разрыве линии рамки) задают направление — искать УЖЕ занятое
место, — но конкретное решение эта заметка не проектирует.

**4. NTSC-ряд у нас невыразим, и пресеты это вскроют.** Тик = 1/90000 c, а
`Project::validate` требует `TICK_HZ % fps == 0`
(`src/libs/kt-model/src/project.rs:3117`, ошибка «settings: fps must divide
90000»). 23.976 / 29.97 / 59.94 на сетку не ложатся: импорт снапает их к
ближайшей целой с дрейфом 0.1 % и говорит об этом (`grid_fps`,
`src/libs/kt-otio/src/convert.rs:1357`, тест 178). Пресет «YouTube 23.976» или
«broadcast 29.97» у нас поэтому существовать не может — и об этом надо СКАЗАТЬ
вслух (правило UI 16), а не молча отдать 24.

**5. Правило приведения — это код композиции, а не опция муксера.** Сейчас
вписывание происходит в декодере (`source_impl.rs`, `sws_scale` в целевой
размер), то есть ВНЕ `frame_plan`. Любое «вписать с полями» / «обрезать» меняет
геометрию слоя и обязано жить в общем коде плана, одинаково у CPU- и
GPU-исполнителя — инвариант №7 в редакции Р.29, с конформансом kt-gpu
(t181/t182). Это самая дорогая часть фичи и единственная, где «пресет»
перестаёт быть снимком настроек.

### Какие инварианты задевает

- **Инвариант 7** (код композиции один): правило приведения и любое изменение
  геометрии кадра — в `frame_plan`, парой CPU + WGSL, иначе превью и экспорт
  разойдутся. Байтовое равенство превью и экспорта (t11/t67, t184) держится
  именно на отсутствии таких развилок.
- **Инвариант 9** (экспорт полностью внутрипроцессный): новый контейнер из
  пресета — это новый `oformat` в `kt_ffmpeg::Muxer`, а не вызов внешнего
  `ffmpeg`. Соблазн «пусть пресет хранит командную строку» запрещён
  архитектурой.
- **Инвариант 10** (бинарь не линкуется с libav*): состав кодеков — свойство
  СИСТЕМНОГО FFmpeg конкретной машины, а не пресета. Пресет может НАЗВАТЬ
  кодек, но обязан проверяться реальным открытием у загруженной версии (6/7/8),
  как это уже делает `Muxer::open`.
- **Инвариант 11** (детерминизм): если пресет начнёт фиксировать бэкенд
  (GPU/CPU), он начнёт фиксировать БАЙТЫ — кросс-бэкендное равенство только по
  допускам. Пресет не должен молча выбирать бэкенд.
- **Правило UI 4** (умолчание — полезное): «Авто» у энкодера остаётся
  умолчанием и внутри пресета.
- **Правило UI 13** (не тратить место впустую): см. «Что мешает», п.3.
- **Правило UI 16** (невозможность — как ошибка): недоступный энкодер,
  невыразимая частота кадров, несовместимый кодек — ошибки, а не блёклые
  подсказки.
- **Правило UI 19** (диалог говорит, где потом изменить решение): пресет,
  применённый молча, обязан оставить след — какой применён и чем
  переопределяется.
- **Правило UI 20** (ничего не отрастает само): список пресетов не пополняется
  «последним использованным» — только явным сохранением.

## Оценка

| Что | Ответ |
|---|---|
| Объём | M — 2–3 стадии: (1) именованный снимок `ExportOptions` + хранилище, (2) шаблон имени с подстановками и приём пресета в kt-cli, (3) частота кадров и правила приведения. Стадия 3 в одиночку тянет на L и трогает инвариант 7 |
| Где основная работа | kt-app (список, окно, применение), kt-engine (`ExportOptions` растёт; `frame_plan` — если берём приведение), kt-ffmpeg (если контейнер шире MP4/h264/AAC), kt-cli (принять пресет вместо `[encoder]`) |
| Формат проекта | `.otio` НЕ трогает. Заводит хранилище уровня приложения — общее с F-AUT-004 и F-TXT-002, решаться должно один раз на все три |
| Тесты, которые придётся написать | round-trip пресета (запись/чтение/незнакомое поле); «пресет с недоступным энкодером = явная ошибка» по образцу t25; подстановки в имени файла при неизменном умолчании t201; харнесс-тест окна в `layout_tests.rs`; при стадии 3 — конформанс CPU↔GPU приведения (t181/t182) и неизменность GOLDEN_VIDEO для пресета «как проект» |
| Зависит от | F-AUT-004 (где живёт библиотека между проектами), F-EXP-002 (очередь/Media Encoder: там же вопрос «пресет против ЗАДАНИЯ») |
| Ломает совместимость | нет |

## Вердикт

**Берём, но в узком виде и не раньше, чем решён вопрос хранилища.** Именованный
снимок пяти полей `ExportOptions` — работа на одну стадию, и она честно
закрывает «надоело выставлять заново каждый запуск» плюс расхождение GUI и
kt-cli. Главного она не закрывает: у нас нет ни частоты кадров в экспорте, ни
правил приведения, ни цветовых тегов — пресету пока нечего нести, кроме кодека
и битрейта, а самое интересное (что делать при несовпадении кадра) решено молча
и не в ту сторону — растяжением, до которого доезжаешь штатным пунктом «Max».
Порядок разумный такой: сначала общее пользовательское хранилище (третья заметка
подряд просит одно и то же), потом снимок настроек с именем, и только потом
приведение — как отдельная работа в `frame_plan`, а не как галочка в диалоге.

## Источники

- Final Cut Pro, состав назначения (Format / Video Codec / Resolution / Color
  Space / Action) и цитата про цветовое пространство:
  https://support.apple.com/guide/final-cut-pro/export-file-and-apple-devices-destinations-ver13664388c/mac
  (дата обращения 2026-09-07)
- Final Cut Pro, создание назначений, Duplicate и Bundle:
  https://support.apple.com/guide/final-cut-pro/create-share-destinations-ver9fd008a21/mac
  и https://support.apple.com/guide/final-cut-pro/intro-to-destinations-settings-ver9ac16f8f/mac
  (дата обращения 2026-09-07)
- Final Cut Pro, экспорт в несколько назначений и невозможность дописать кодек
  в имя файла (ответ автора статьи в комментариях):
  https://larryjordan.com/articles/fcp-x-export-to-multiple-destinations-at-once/
  (дата обращения 2026-09-07)
- VEGAS Pro 2026 build 66, release notes («Customizable Color Information now
  available for render template in ProRes exports», 27.03.2026):
  https://forum.borisfx.com/t/release-vegas-pro-2026-build-66/21161
  (дата обращения 2026-09-07)
- VEGAS Pro, битрейт шаблона против битрейта файла; шаблон с NVENC и версия
  драйвера; группы шаблонов в «Render As» не запоминаются:
  https://forum.borisfx.com/t/rendered-audio-is-of-significantly-lower-bitrate-than-the-bitrate-of-the-render-template/21593 ,
  https://forum.borisfx.com/t/render-problem/22547 ,
  https://forum.borisfx.com/t/first-impression-after-vp22/22672
  (дата обращения 2026-09-07)
- VEGAS Pro, full-range проект + ltd-range рендер ради совпадения с YouTube и
  бессмысленность высокобитрейтных YouTube-пресетов:
  https://forum.borisfx.com/t/vp2026b066-color-differences-issue-in-8-bit-full-pixel-range/22064 ,
  https://forum.borisfx.com/t/version-2026-0-3-b-189-oh-soooo-close/25227
  (дата обращения 2026-09-07)
- DaVinci Resolve, страница Deliver, очередь рендера и Quick Export:
  https://www.blackmagicdesign.com/products/davinciresolve/ ; версия 21 и
  MainConcept в render settings:
  https://www.blackmagicdesign.com/products/davinciresolve/whatsnew
  (дата обращения 2026-09-07)
- DaVinci Resolve, Input/Output Scaling и чёрные полосы при совпадающих
  разрешениях; отсутствие match-source у таймлайна и экспорта:
  https://creativecow.net/forums/thread/black-bars-on-export-issue/ и
  https://creativecow.net/forums/thread/matching-settings-of-original-dv-prores-422-footage-in-exports-for-resolve/
  (дата обращения 2026-09-07). Варианты вписывания названы по формулировкам из
  тредов; **по справочнику Resolve НЕ ПРОВЕРЕНО** — reference manual в этом
  окружении недоступен
- Premiere / Media Encoder: расположение `.epr` в профиле AME, пропажа пресетов
  при обновлении версии, отказ импорта спасённых файлов, отсутствие пресета по
  умолчанию:
  https://creativecow.net/forums/thread/export-setting-presets/ ,
  https://creativecow.net/forums/thread/updated-premiere-2017-cc-and-have-no-more-presets/ ,
  https://creativecow.net/forums/thread/custom-export-preset-location-on-mac-using-premier/ ,
  https://creativecow.net/forums/thread/how-do-i-set-a-default-preset-in-ame/
  (дата обращения 2026-09-07)
- Premiere, Export Preset Manager и Quick Export (22.3.1, 2022), вторичный
  источник:
  https://larryjordan.com/articles/new-quick-export-and-export-presets-manager-in-adobe-premiere-pro-22-3-1-v/
  (дата обращения 2026-09-07)
- Adobe Media Encoder как отдельное приложение линейки CC 2026 (26.0):
  https://en.wikipedia.org/wiki/Adobe_Media_Encoder (дата обращения 2026-09-07)
- **Что НЕ удалось проверить и почему.** `helpx.adobe.com` из этого окружения не
  открывается («proxy refused the connection»), поэтому строки Premiere и After
  Effects в таблице собраны по вторичным источникам и помечены
  `[НЕ ПРОВЕРЕНО]`: для AE это разделение Render Settings templates и Output
  Module templates, для Premiere — точный формат и расположение `.epr` (косвенно
  подтверждены форумными тредами выше). Поисковые движки в этой сессии почти не
  работали: Mojeek и Ecosia — 403, Brave — 429, обе html-версии DuckDuckGo —
  CAPTCHA, Bing — нерелевантная выдача. **Поправка к
  `docs/features/_editors.md`, раздел «Как проверять»:** форум VEGAS переехал с
  `vegascreativesoftware.com` на Discourse `forum.borisfx.com`
  (категория `/c/vegas-pro/55`) и отлично отдаётся через
  `search.json?q=` / `t/<slug>/<id>.json` / `raw/<id>/<номер поста>`;
  `forum.blackmagicdesign.com` в эту волну оказался НЕДОСТУПЕН целиком (403 и на
  корне, и на `viewforum.php`, и на `search.php`), поэтому весь Resolve-фидбек
  взят с Creative COW; у `community.adobe.com` открываются только листинги досок
  `/t5/<board>/bd-p/<board>`, поиск закрыт (403). Живьём ни один из пяти
  редакторов не проверялся.
</content>
