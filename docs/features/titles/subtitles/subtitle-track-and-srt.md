---
id: F-TXT-004
title: Дорожка субтитров, SRT/VTT и текстовый монтаж
status: кандидат
priority: B
cost: L
category: titles/subtitles
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: нет
crates: [kt-model, kt-otio, kt-engine, kt-ffmpeg, kt-media, kt-app]
invariants: [7, 8, 9, 10, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-TXT-001]
checked: 2026-09-07
---

# F-TXT-004 — Дорожка субтитров, SRT/VTT и текстовый монтаж

## Суть

Монтажёр получает не надпись на картинке, а СПИСОК реплик: у каждой начало,
конец и текст. Список живёт отдельной строкой таймлайна, правится как текст,
проверяется на длину строки и скорость чтения и уезжает наружу тремя разными
способами — файлом рядом с видео, потоком внутри контейнера или выжженным в
пиксели. Сверху за два года наросла автотранскрипция: список появляется сам,
а в Vegas/Premiere/Resolve правка ТЕКСТА начала менять МОНТАЖ.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Speech to Text / Insert Subtitles | отдельной ДОРОЖКИ субтитров нет: транскрипция кладёт результат событиями титров — «Vegas Pro creates Titles & Text events on a new track in your timeline». Текстовый монтаж двусторонний: «Vegas Pro removes the selected text and the corresponding section from the timeline», обратно — «Vegas Pro updates the corresponding events on the timeline to match the transcript edits». Offline, 7 моделей от «Tiny - Fast (~78MB)» до «Large v3 - Latest (~3100MB)», 100+ языков, экспорт SRT. Только по подписке: «This feature is available only with a Vegas Pro subscription» |
| Adobe Premiere | 26.x | Text panel (Transcript + Captions), Text-Based Editing | панели «Транскрипт» и «Субтитры» на уровне клипа и секвенции, дорожка субтитров в таймлайне; транскрипция локальная (Adobe сообщил Larry Jordan, что перешли на локальную обработку «sometime in 2022»); по умолчанию SRT, плюс CEA-608, CEA-708, EBU, Teletext, австралийский; ширина строки задаётся в символах. В 26.3 (июнь 2026) добавлены пословные субтитры: «Premiere can now generate single-word captions directly from a transcript» |
| DaVinci Resolve | 21 (проверялось по документации 20.x и странице продукта) | Subtitle track / Create Subtitles from Audio | НАСТОЯЩАЯ дорожка данных: «Subtitles appear in the timeline above of your video tracks and can be moved and trimmed like any other media». Импорт «timed text TTML, SRT, XML and embedded MXF/IMF subtitles», экспорт «rendered into the final video or exported as separate TTMLs, SRT or VTT files». Анимация цепляется к ЗАГОЛОВКУ дорожки: «Drag one of the five animation options into the caption track header». STT только в Studio; IntelliScript собирает монтаж по сценарию, сверяя его с транскрипцией |
| Final Cut Pro | 12.3 | Closed captions (роли CEA-608 / iTT / SRT), Generate Captions | субтитры лежат «in a closed caption lane at the top of the timeline», у каждого формата своя роль и саброль на язык. Автотранскрипция ограничена железом и языком: «The Generate Captions feature requires a Mac with Apple silicon», «available in U.S. English only», результат — обычные титры («standard title clips created using the default Subtitle title») в роли Subtitles. При импорте выбирается точка отсчёта: «Relative to Timeline» либо абсолютная |
| After Effects | 26.x | нет | отдельной дорожки субтитров нет: текст — обычный текстовый слой, тайминг — ключи и маркеры. Импорт SRT как списка реплик документацией не подтверждён — `helpx.adobe.com` из окружения не открывался, см. «Источники» [НЕ ПРОВЕРЕНО] |

## Фишка фишки

**1. У одного и того же текста три РАЗНЫЕ судьбы на экспорте, и редактор
показывает только одну из них.** Сайдкар (SRT/VTT/TTML) — файл рядом,
зритель включает и выключает, поисковик индексирует. Вшитый поток — это не
«тот же текст внутри файла»: CEA-608/708 едут в user data картинки, то есть
внутри КОДИРОВАННОГО видео («CTA-708 captions are injected into MPEG-2 video
streams in the picture user data»), и потому переживают перекодирование
только вместе с ним. Burn-in — вообще не текст, а пиксели. Формальный тест
на разницу даёт FCP: CEA-608 «can be imported or exported as tracks embedded
in a media file or as separate files», а iTT и SRT «can't be embedded in an
output media file the way CEA-608 closed captions can». Resolve добивает
наглядно: пять анимаций субтитров живут только в выжженном варианте —
«Captions exported as a separate file can not be animated». Значит красивая
подача, которую монтажёр видел в превью, существует ровно в одном из трёх
выходов, а в двух других он отдаёт голый текст и об этом узнаёт от заказчика.

**2. Половина рынка «дорожки субтитров» не имеет — там это титры, и видно
это по побочным эффектам.** Vegas 2026 официально кладёт транскрипцию
событиями «Titles & Text» на новую дорожку. Отсюда ровно те жалобы, которых
не бывает у списка реплик: повторная генерация не заменяет прежние реплики, а
кладёт вторые поверх («competing subtitles… they will superimpose upon each
other»), правки текста в панели не удерживаются, а генерация ломается ПОСЛЕ
нарезки клипов. У списка реплик правка текста — это правка поля записи, у
титров — правка ИСТОЧНИКА кадра, то есть перерисовка и инвалидация кэша.

**3. Текстовый монтаж — не фича субтитров, а смена того, что считается
исходником монтажа.** Формулировка Vegas дословна: «Vegas Pro removes the
selected text and the corresponding section from the timeline», и обратно —
правка таймлайна переписывает транскрипт. То есть транскрипт и монтаж стали
двумя проекциями одной структуры, а её единица — СЛОВО со своим временем.
Что пословное время реально хранится, доказывает Premiere 26.3: субтитры «по
одному слову» делаются из уже имеющегося транскрипта без повторного прогона
распознавания. Неочевидная цена: слово должно быть привязано к тику
МАТЕРИАЛА, иначе первый же trim/move/reverse рассинхронизирует текст с
картинкой — тот же выбор домена, что у ключей автоматизации (F-AUT-002), но
для тысячи слов вместо десятка ключей. И второе: «удалил слово» — это ripple
той дорожки, которой принадлежит транскрипт; что при этом происходит с
B-roll и музыкой сверху, каждый решает сам, а undo обязан откатывать текст и
монтаж ОДНИМ шагом, иначе они разъезжаются.

## Плюсы

- Реплика — запись данных, а не источник кадра: правка текста не трогает
  граф рендера и не инвалидирует кэш кадров, поэтому вычитка сорока реплик
  стоит сорок правок полей, а не сорок перерисовок.
- Один рендер видео обслуживает N языков: сайдкар-файлов можно приложить
  сколько угодно, и зритель их переключает. Выжигание умножает рендеры на
  число языков.
- Текст как данные позволяет ПРОВЕРЯТЬ доставку: 42 символа в строке, не
  больше двух строк, до 20 знаков в секунду (детские программы — 17) — это
  проверяемые числа из вещательного регламента, а не вкусовщина; по пикселям
  такую проверку не сделать вовсе.
- Пословный транскрипт делает монтаж адресуемым текстом: «выкинуть запинку» —
  одна операция над диапазоном слов вместо trim + сдвиг соседей.
- Отдельная дорожка данных не участвует в композите, поэтому её скролл и
  правка не порождают ни декода, ни перерисовки картинки.

## Минусы

- Три выхода — три разные правды. Larry Jordan прямо отмечает у Resolve
  расхождение показанного и выжженного, из-за чего решение о переносе строки
  приходится принимать вслепую; FCP советует при экспорте SRT снимать
  форматирование, потому что «may not appear correctly in your intended
  player».
- Тайминг реплик живёт в миллисекундах (SRT/VTT), монтаж — в кадрах. На
  23.976/29.97 граница реплики никогда не попадает на границу кадра, и
  «съехало на кадр» — не баг импорта, а неизбежное округление. Видимый
  симптом — выбор в FCP при импорте: «Relative to Timeline» (прибавить старт
  проекта) против абсолютного.
- Субтитры-как-титры засоряют пул медиа по одному объекту на реплику и не
  умеют заменяться при повторной генерации (см. цитаты).
- Текстовый монтаж жёстко привязан к состоянию таймлайна, на котором сделан
  транскрипт: нарезали события — генерация перестаёт работать, транскрипт
  пересобирается, вычитка теряется.
- Автотранскрипция — это веса: от 78 МБ до 3.1 ГБ на модель, отдельно на
  язык, и качество прямо превращается в часы вычитки. Ограничения бывают
  жёстче, чем кажется: у FCP «requires a Mac with Apple silicon» и «U.S.
  English only», у Vegas — только по подписке.
- CJK/RTL упираются не в логику, а в шрифт и шейпинг: у Vegas корейский текст
  выходит вопросительными знаками и ломает пресеты, и это претензия к
  титровальной системе, а не к распознаванию.

## Что говорят пользователи

> I just now tried AI Speech to Text–WORTHLESS!! Testing this on both the
> Small Good Quality model AND on the Large V2 model on several events, I
> either get only a couple of words detected, or, on one transcription, I even
> got what looked to be JAPANESE CHARACTERS!!
> — richkut, форум Boris FX (Vegas Pro), 19.04.2026,
> https://forum.borisfx.com/t/ai-speech-to-text-disappointing/22602

> But, you have to make sure that you have deleted all the previous title
> cards in Project Media, or else you'll have competing subtitles (they will
> superimpose upon each other).
> — kevin.vonduuglasittu, форум Boris FX, 21.04.2026,
> https://forum.borisfx.com/t/problems-editing-ai-speech-to-text-subtitles-using-vegas-pro-23/22688

> I'll double click on words in 'Subtitle' view and edit them…and then 2
> minutes later they have reverted.
> — kevin.vonduuglasittu, там же, 21.04.2026,
> https://forum.borisfx.com/t/problems-editing-ai-speech-to-text-subtitles-using-vegas-pro-23/22688

> when I tried to edit my first TikTok video with the software, the subtitle
> generation didn't work after I cut my different clips.
> — AlexandreTHEGAMER, форум Boris FX, 11.04.2026,
> https://forum.borisfx.com/t/my-subtitle-generation-doesnt-work-on-vp-2026/22213

> I am using speech to text to create subtitles. The spoken text in the video
> is korean and the machine I am working on is a german windows PC.
> — walter.k, форум Boris FX, 16.05.2026 (дальше в том же посте: часть
> символов показывается вопросительными знаками),
> https://forum.borisfx.com/t/speech-to-text-missing-characters-shown-as-question-marks/23543

> At the moment, VEGAS Pro effectively does not have a modern title system
> that fully supports Korean text properly.
> — storyyou, форум Boris FX, 08.05.2026 (в ответе Boris FX: прототип нового
> титровальщика был, вместо него временно включили Title Studio),
> https://forum.borisfx.com/t/about-vegas-pro-text-systems-concerns-from-a-korean-user-protype-titler-titles-text-and-bcc-title-studio/23289

> I chose Premiere for its accuracy and ease of correction. Final Cut is
> simply inadequate and Resolve goes out of its way to make the process
> excessively cumbersome.
> — Larry Jordan, сравнение субтитрования в FCP 11 / Premiere 2025 /
> Resolve 20, 16.08.2025,
> https://larryjordan.com/articles/comparing-captioning-in-final-cut-pro-11-premiere-pro-2025-resolve-20/

Чего найти НЕ удалось: связки «перешёл на другой редактор из-за текстового
монтажа» и жалоб на burn-in вида «выжег и не смог поправить» — r/editors,
r/premiere и discussions.apple.com из этого окружения не открываются, поиск
форума Blackmagic (`search.php`) отдаёт 403 гостю, у creativecow не нашлось
рабочего URL поиска. Всё найденное живьём — с форума Boris FX (Vegas), там
претензии к автотранскрипции и к титровальной системе, а не к дорожке
субтитров как таковой.

## Как ложится на Kinotrain

### Что уже есть

- **Тайминговые именованные записи и их round-trip.** `Marker { t, duration,
  name, color }` и `MarkedArea { name, start, duration, lanes, color }` —
  `src/libs/kt-model/src/project.rs`. Метки уезжают в OTIO как `Marker.2`
  (`src/libs/kt-otio/src/convert.rs`), области — в конверт `metadata.kt`
  (`src/libs/kt-otio/src/meta.rs`, `KT_META_VERSION = 4`), тесты 89/90.
- **Готовый UI-прецедент «полоса подписанных диапазонов»**:
  `marker_band_rect`, `pick_range_marker`, инлайн-правка имени
  (`marker_edit`), протяжка края (`Drag::MarkerEdge` →
  `Command::SetMarkerDuration`) — `src/apps/kt-app/src/timeline.rs`, тест
  170. Рисовать и таскать ленту реплик уже умеем.
- **Детерминированный растеризатор текста ВНУТРИ движка уже существует** —
  `src/libs/kt-media/src/placeholder.rs`: встроенный пиксельный шрифт 3×5,
  намеренно не зависящий ни от системных шрифтов, ни от egui, «поэтому кадр
  детерминирован и одинаков в превью, экспорте и тестах» (тест 208). Это же
  и прецедент «источник, который рисует кадр сам» — заглушка является обычным
  `MediaSource` и живёт под тем же MediaWorker (инвариант 5).
- **RTL-опыт**: `src/apps/kt-app/src/i18n/bidi.rs` (переупорядочивание
  атомов начертания по UAX#9, тест 126) и 5 каталогов `apps/kt-app/i18n/`.
- **Муксер**: `src/libs/kt-ffmpeg/src/muxer_impl.rs`. Типы `AVSubtitle` /
  `AVSubtitleRect` bindgen УЖЕ генерирует (`allowlist_type("AV.*")` в
  `src/libs/kt-ffmpeg/build.rs`, заголовки вендорены для FFmpeg 6/7/8).

### Чего не хватает

- Текстового растеризатора с настоящими шрифтами — шейпинг, перенос строк,
  RTL, обводка. Это F-TXT-001, и отдельно его заводить нельзя (правило 17
  чек-листа: два способа положить текст на экран — запрещённая пара).
- Списка реплик как модели: сейчас его нет вообще, и метки им не являются
  (см. ниже).
- Пятого вида дорожки: `LaneKind` — ровно `Video | Audio | Morph | Camera`
  (`src/libs/kt-model/src/project.rs`), их знает `normalize_lane_order`
  (тест 49), порядок строк ходит через undo, а `MarkedArea.lanes` хранит
  семантические ключи `(LaneKind, u32)`.
- Слоя, нарисованного нами: `LayerSrc` в
  `src/libs/kt-engine/src/video_plan.rs` жёстко держит `SourceId` +
  `cache_idx` в burst-кэше; ветки «слой из готовой RGBA-картинки» нет.
- Парсера/писателя SRT и VTT и субтитрового потока в MP4: в `api.rs` нет
  `avcodec_encode_subtitle`, а `avformat_new_stream` в муксере зовётся ровно
  дважды — видео и аудио.

### Что мешает

- **Честно про метки: как UI — прецедент, как данные — не годятся.**
  `Project::normalize()` делает `markers.sort_by_key(t)` и
  `markers.dedup_by_key(t)`: идентичность метки — её тик, поэтому двух реплик
  на один тик не бывает и адресация команд идёт по времени
  (`Command::MoveMarker { from, to }`). Имена меток-диапазонов и областей
  лежат в ОДНОМ пространстве имён с проверкой уникальности
  (`ProjectError::DuplicateName`) — два субтитра «Да.» его нарушат. Имя
  однострочное, без переносов, стиля и позиции, и рисуется на строке подписей
  времени. Плюс метки — цели магнита (`snap_targets` в
  `src/apps/kt-app/src/timeline.rs` строит `Vec<i64>` на вызов): тысяча
  реплик сорокаминутного фильма превратит это в аллокацию на кадр, что прямо
  запрещено инвариантом 12. Вывод: реплики — своя коллекция с id и своей
  дорожкой, а не расширение `Marker`.
- **Шрифты — главный упор, и он неожиданный.** Поставляемый CJK-шрифт —
  САБСЕТ каталогов интерфейса: «codepoints 522 (from i18n/*.toml)»
  (`src/apps/kt-app/assets/SUBSET-MANIFEST.txt`, собирается
  `src/scripts/subset-fonts.py`, тест 100). Любой китайский субтитр за
  пределами словаря UI даст квадраты — то есть ровно тот дефект, на который
  жалуются корейские пользователи Vegas. Брать системные шрифты нельзя:
  инвариант 11 требует байт-в-байт видео против закоммиченного эталона, а
  системный шрифт делает эталон машинозависимым. Значит либо в репозиторий
  едет полный шрифт (Noto CJK ~19 МБ), либо burn-in честно ограничивается
  поставленным набором. Сегодняшний единственный растеризатор внутри движка
  решает это по-своему: `placeholder.rs` рисует всё вне таблицы как «?».
- **Инвариант 7 (код композиции один).** Выжигание нельзя делать «в GPU
  шейдером, а на CPU по-другому»: это ровно тот класс расхождений, из-за
  которого превью и экспорт покажут разное. Прецедент решения уже есть —
  п.250 (смешивание кадров сделано в ОБЩЕМ коде плана, а не в исполнителях),
  и по нему видно, что реплику логично растеризовать один раз в RGBA и
  отдать обоим бэкендам одинаковый слой. Конкретный способ — дело раунда.
- **Инварианты 9 и 10 при экспорте.** Сайдкар SRT/VTT пишем сами — это
  обычный файл, никаких зависимостей. Вшитый поток обязан идти через
  dlopen-нутый системный libavformat: в MP4 из текстовых субтитров практически
  возит `mov_text` (3GPP Timed Text, `tx3g`); нужен либо новый символ в
  vtable `src/libs/kt-ffmpeg/src/api.rs`, либо сборка сэмплов вручную.
  Пройдёт ли mp4-муксер FFmpeg без энкодерного описания потока — [НЕ
  ПРОВЕРЕНО]. CEA-608/708 из этого списка выпадают вовсе: их место — внутри
  кодированного видео, а энкодер у нас выбирается автоматически из пяти
  кандидатов (nvenc/vaapi/qsv/x264/mpeg4), и вкладывать в каждый свои
  side data — отдельная большая история.
- **Речь-в-текст.** Веса от 78 МБ до 3.1 ГБ на модель, отдельно на язык,
  плюс рантайм инференса. Проект, который даже FFmpeg не линкует, а грузит
  через dlopen (инвариант 10), не может вшить это в бинарь. Реалистичных
  вариантов два: (а) импорт SRT, сделанного снаружи, — именно это советуют на
  форуме Vegas взамен встроенной транскрипции; (б) необязательный внешний
  движок по тому же принципу «есть в системе — используем». Своей модели у
  нас не будет, и это надо сказать прямо, а не подразумевать.
- **Правила UI 13 и 20.** Дорожка субтитров — ещё одна строка в и без того
  плотном таймлайне; она не должна отрастать сама (правило 20) и не должна
  занимать место, когда субтитров в проекте нет (правило 13).

### Какие инварианты задевает

7 (выжигание — общий код плана, не второй путь), 8 (лента реплик — данные,
декода порождать не должна), 9 и 10 (сайдкар пишем сами; поток — только через
dlopen-нутый FFmpeg, новых библиотек не добавлять), 11 (детерминизм растра →
шрифт вендорится, системные не берём), 12 (тысяча реплик в draw-пути и в
`snap_targets` — без аллокаций и с отсечением по вьюпорту). Правила
чек-листа UI: 13 (место), 17 (титры и субтитры не должны стать парой способов
сделать одно и то же), 20 (дорожка не отрастает сама).

## Оценка

| Что | Ответ |
|---|---|
| Объём | L — целый раунд, и то без автотранскрипции |
| Где основная работа | kt-model (коллекция реплик + пятый `LaneKind` + команды/undo), kt-app (строка таймлайна, редактор текста, проверки длины строки), kt-engine (слой выжигания в `video_plan`), kt-ffmpeg (субтитровый поток), kt-otio (сохранение), kt-media (шрифт/растр по образцу `placeholder.rs`) |
| Формат проекта | нужен `KT_META_VERSION` v5: новая коллекция + новый вид дорожки в `lane_order` (сейчас v4, `src/libs/kt-otio/src/meta.rs`) |
| Тесты, которые придётся написать | round-trip SRT/VTT (миллисекунда ↔ тик точна: 90000/1000 = 90, деления без остатка); квантование границы реплики к сетке fps (та самая «ошибка на кадр»); golden-кадр выжженного субтитра (CPU-эталон + допуски GPU, инвариант 7); otio round-trip с пятой дорожкой; `lane_order` с новым видом (расширение теста 49); харнесс-тест строки субтитров и покрытие глифов (расширение теста 100) |
| Зависит от | F-TXT-001 (титры и растеризатор текста) — без него делать нечего |
| Ломает совместимость | нет: старые проекты открываются, новые с субтитрами не откроются старой сборкой (проверка `check_kt_version`) |

## Вердикт

Берём, но разрезанным и в правильном порядке. Первое — СПИСОК РЕПЛИК как
самостоятельная коллекция с id и своей дорожкой плюс импорт/экспорт SRT и
VTT: это ценность, не требующая ни единого пикселя текста, и она честно
опирается на уже имеющиеся тайминговые записи и OTIO-конверт. Второе —
выжигание, и только поверх растеризатора F-TXT-001, одним кодом плана
(инвариант 7) и с вендоренным шрифтом (инвариант 11). Третье — вшитый
`mov_text` в MP4, когда появится спрос.

Автотранскрипцию и текстовый монтаж в этом виде НЕ берём: веса моделей
(78 МБ…3.1 ГБ) несовместимы с принципом «ничего не тащим внутрь», а без
своего распознавания текстовый монтаж вырождается в «импортируй чужой SRT и
режь по нему» — что как раз и стоит поддержать в первом шаге. Метки-диапазоны
переиспользовать под реплики нельзя: дедуп по тику и общее уникальное
пространство имён с областями — это не мелкое ограничение, а другая модель
идентичности.

## Источники

- https://support.apple.com/guide/final-cut-pro/intro-to-captions-ver00e40835d/mac (дата обращения 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/generate-subtitles-automatically-verd6d52c4e8/mac (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/import-closed-captions-ver4185ef95a/mac (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/export-closed-captions-ver245c7581b/mac (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/8-design/speechtotext.htm (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/1-before/whatsnew.htm (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/edit (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/whatsnew (2026-09-07, страница Resolve 21)
- https://community.adobe.com/announcements-727/what-s-new-in-adobe-premiere-26-3-june-2026-1628369 (2026-09-07)
- https://larryjordan.com/articles/comparing-captioning-in-final-cut-pro-11-premiere-pro-2025-resolve-20/ (2026-09-07)
- https://larryjordan.com/articles/create-animated-captions-using-davinci-resolve-20/ (2026-09-07)
- https://partnerhelp.netflixstudios.com/hc/en-us/articles/217350977-English-Timed-Text-Style-Guide (2026-09-07) — числа регламента: 42 символа в строке, максимум две строки, 20 (17 для детских программ) знаков в секунду
- https://en.wikipedia.org/wiki/CEA-708 (2026-09-07) — как вшитые captions едут внутри картинки
- https://en.wikipedia.org/wiki/MPEG-4_Part_17 (2026-09-07) — `tx3g` в MP4
- Форум Boris FX (Vegas Pro), треды 22602, 22688, 22213, 23543, 23289 (2026-09-07)

Оговорки. Строка Premiere собрана по официальному анонсу 26.3 в сообществе
Adobe и по сравнительному разбору Larry Jordan: `helpx.adobe.com`,
`adobe.com/products/premiere*` и `blog.adobe.com` из этого окружения не
открывались, поэтому формулировки про локальную транскрипцию и список
форматов — вторичный источник, а не справка Adobe. Строка After Effects —
отсутствие фичи по косвенным данным, справкой не подтверждено [НЕ
ПРОВЕРЕНО]. Resolve проверялся по странице продукта и «What's New» 21-й
версии; «AI Animated Subtitles» под таким именем на сайте Blackmagic найти не
удалось — описание анимированных субтитров взято из разбора Larry Jordan по
Resolve 20 [НЕ ПРОВЕРЕНО как официальное имя фичи]. Живьём ни один из пяти
редакторов не запускался. Поиск: бюджет `WebSearch` в сессии исчерпан,
работали прямые URL и поисковики через `WebFetch`; DuckDuckGo отдавал капчу,
Brave/Ecosia/Yandex — 403/429/капчу.
