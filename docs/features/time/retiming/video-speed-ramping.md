---
id: F-SPD-001
title: Скорость события — постоянная и переменная
status: кандидат
priority: B
cost: M
category: time/retiming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: частично
crates: [kt-model, kt-engine, kt-fx, kt-app]
invariants: [1, 2, 4, 5, 7, 8, 11]
ui_rules: [4, 9, 13, 17, 20]
round: Р.37
point: 247
tests: [246, 247, 248, 249, 250]
depends_on: []
checked: 2026-09-07
---

# F-SPD-001 — Скорость события — постоянная и переменная

## Суть

Монтажёр говорит «пусть это событие идёт вдвое медленнее» и имеет в виду одну
из двух РАЗНЫХ вещей. Первая: материал тот же, событие стало вдвое длиннее, всё
за ним поехало вправо. Вторая: событие той же длины, за него проходит вдвое
меньше материала, соседи не двигались. Сверху накладывается «переменная»
скорость: внутри события она едет по кривой (разгон, замедление к удару,
стоп-кадр). И к каждому варианту прицеплены три вопроса, которые монтажёр
задаёт всегда: что стало со звуком, что стало с его высотой тона и откуда взять
кадры, которых в материале нет.

## У кого и как

Вопрос ко всем один: меняет ли смена скорости ДЛИНУ события, двигаются ли
соседи, и что при этом происходит со звуком.

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026 | Playback rate + Velocity envelope | ДВА независимых механизма. `Ctrl`+перетаскивание края («hold Ctrl while dragging the right or left edge of an event») растягивает событие: длина меняется, материал тот же; у аудио-события процент после растяжения пишется в правом нижнем углу при включённом Active Take Information. Velocity-огибающая — только на ВИДЕО-событиях, и она длину не меняет: «Each video event in your project has a specific duration that is not changed by velocity envelopes». Пресеты — «from 100% reverse-speed playback to 0% playback (a freeze-frame effect) up to 300%», отрицательное значение играет событие задом наперёд. Верх огибающей поднят до 1000% ещё в VP14 build 161 (20.09.2016): «to increase the maximum possible overall velocity of video on the timeline to 40 times normal speed». Сглаживание — отдельный переключатель события Resample (Force/Smart). Высота тона звука настраивается своей парой ручек в Event Properties (Time stretch/pitch shift, движок élastique от zplane) |
| Adobe Premiere | 26.x | Speed/Duration, Rate Stretch tool | Один диалог на обе модели: Speed и Duration связаны кнопкой-цепочкой («To have the speed or duration adjusted together click the gang button so that it shows an unbroken link»), плюс три галки — Reverse Speed, Maintain Audio Pitch («Check this to have the pitch remain the same as the duration is changed») и Ripple Edit, Shifting Trailing Clips («Checking this will cause any adjacent clips to move if needed»). Тот же результат жестом — инструмент Rate Stretch на краю клипа. Как добывать промежуточный кадр — отдельный список Time Interpolation (Frame Sampling / Frame Blending / Optical Flow, см. F-SPD-003). Переменная скорость — не здесь, а в Time Remapping (F-SPD-002) |
| DaVinci Resolve | 21 | Change Clip Speed, Retime Controls | Диалог даёт одно и то же тремя способами — Speed %, Frames per second, Duration — плюс Ripple Sequence, Pitch Correction, Reverse Speed, Freeze Frame. Ripple Sequence двигает всё, что за клипом, под его новую длину; со снятой галкой клип остаётся на месте. Переменная скорость — ретайм-полоса и ретайм-кривая (F-SPD-002), сглаживание — Optical Flow: «building new frames so you can slow down and extend the action in a sequence without repeating or blending frames» |
| Final Cut Pro | 12.3 | Retiming (Retime Editor) | Модель длины — основная и по умолчанию сквозная: «Constant speed changes alter the duration of a clip. By default, if a constant speed change causes the duration of a clip to become longer or shorter, all clips coming after it ripple forward or backward». В окне Custom Speed ripple снимается галкой, и тогда «the clip remains at its current length but a shorter piece of the action is seen» — то есть ровно вегасовская velocity-модель. Ретайм-полоса красится по направлению (оранжевая — замедление, синяя — ускорение). Высота тона сохраняется ПО УМОЛЧАНИЮ. Качество — Normal / Frame Blending / Optical Flow, на Apple silicon добавлен Smooth Slo-Mo (Best, Machine Learning). Automatic Speed приводит HFR-материал к таймлайну |
| After Effects | 26.x | Time Stretch, Time Remapping, Timewarp | Не событие, а слой: Time Stretch задаёт множитель и меняет длину слоя, дальше «Time-stretching, time-remapping, and the Timewarp effect are all useful for creating slow motion, fast motion, freeze frame». Соседей двигать нечему — слои лежат параллельно, ripple как понятия нет. Сглаживание — переключатели Frame Blending слоя (Frame Mix / Pixel Motion) плюс мастер-тумблер композиции. Сохранения высоты тона НЕТ вовсе: официальный ответ сообщества — растянуть звук в Audition (или в Premiere с Maintain Pitch) и вернуть готовым файлом |

## Фишка фишки

**1. Две скорости МНОЖАТСЯ, и клампы у них разные.** В Vegas свойство события
(Playback rate, до 4×) и velocity-огибающая — разные механизмы с разными
последствиями: первое меняет ДЛИНУ события, второе длину не трогает вовсе.
Работают они одновременно и перемножаются: с VP14 огибающая тянется до 1000%,
и в релиз-нотах это записано именно как «максимальная общая скорость на
таймлайне — 40×». При этом меню пресетов огибающей заканчивается на 300%,
поэтому на форуме Boris FX в августе 2026 считают вслух «4× свойством × 3×
огибающей = 12×» и просят поднять до 100×, а старожил отвечает, что потолок
пробит «уже давно». Итог: по внешнему виду события узнать его скорость нельзя —
надо открыть свойства И посмотреть на огибающую.

**2. Ripple — это не удобство, а переключатель МОДЕЛИ, и он спрятан в галку.**
Apple формулирует прямо: со снятым ripple «the clip remains at its current
length but a shorter piece of the action is seen». То есть одна и та же галка
превращает «премьеровскую» модель (скорость задаёт длину, хвост монтажа едет) в
«вегасовскую» velocity-модель (длина зафиксирована, меняется кусок действия) — и
это единственное, что их разделяет. У Resolve эта же галка называется Ripple
Sequence, у Premiere — Ripple Edit, Shifting Trailing Clips. Никто из троих не
называет её сменой модели, хотя за ней стоит вопрос «изменение локальное или
глобальное»: с ripple смена скорости одного события переписывает тайминг всего,
что после него, включая соседние дорожки, если они не были залочены.

**3. «Сохранять высоту тона» — отдельная галка, потому что это другой
алгоритм.** Ресемплинг (скорость и высота тона едут вместе, «пластинка на
другой скорости») и тайм-стретч с сохранением питча (фазовый вокодер, у Vegas —
élastique от zplane) — это два разных DSP, а не режим одного. Поэтому у FCP
питч сохраняется ПО УМОЛЧАНИЮ и выключается пунктом меню; у Premiere это
чекбокс в том же диалоге; у Resolve — Pitch Correction, который «becomes less
transparent as the speed change grows»; у AE его нет вообще; а у Vegas на
velocity-пути звука нет вовсе — velocity живёт только на видео-событии, и
парный аудиоклип надо растягивать вручную. Отсюда весь класс жалоб «после
замедления звук уехал».

**4. Ось кривой скорости не может быть в потреблённом материале — это уравнение
на саму себя.** Деталь, которой нет ни в одной справке, и мы нашли её у себя
(Р.37, п.246): если ключ кривой `rate` индексировать долей ПОТРЕБЛЁННОГО
материала, то положение ключа зависит от значений самой кривой — подвинул
значение, и точка уехала по экрану; правая часть графика вообще недостижима,
потому что доехать можно только до `duration · avg_rate`, и автоключ в конце
кривой не срабатывает никогда. Поэтому ось кривой скорости обязана быть в
ВЫХОДНОМ времени события — иначе редактор кривой врёт.

## Плюсы

- Модель «скорость задаёт длину»: скорость видна геометрией — событие стало
  длиннее, значит замедлено; отдельного числа искать не надо. Побочно
  получается приём «набери длительность, а процент пусть решает редактор» —
  подгонка куска под музыкальную фразу делается вводом таймкода, а не подбором.
- Модель «длина зафиксирована» (velocity): правка скорости не двигает НИЧЕГО
  за событием — ни соседей, ни переходы, ни метки; ramp внутри залоченного по
  длине куска (реклама, хронометраж) делается без перемонтажа хвоста.
- Разделение «скорость» и «как достать промежуточный кадр» на два независимых
  контрола (Vegas Resample, FCP Video Quality, Premiere Time Interpolation)
  позволяет платить за дорогое сглаживание только там, где на него смотрят.
- Сохранение высоты тона по умолчанию (FCP) означает, что замедленное интервью
  остаётся разборчивым без единой настройки, а выключение галки становится
  осознанным художественным приёмом.
- Отрицательная и нулевая скорость на той же огибающей (Vegas: −100%…0%…1000%)
  даёт стоп-кадр и «отмотку назад» без отдельного объекта на таймлайне: реверс
  куска — это точка кривой, а не свойство события.

## Минусы

- Два перемножающихся механизма (Vegas) означают, что «скорость события» —
  не число, а произведение двух чисел из разных мест интерфейса; на форуме это
  регулярно всплывает вопросом «почему больше 12× не даёт».
- Модель фиксированной длины молча теряет материал: замедлил вдвое — вторая
  половина действия просто не поместилась и не показана, никакого
  предупреждения нет.
- Модель ripple столь же молча ломает синхронность с другими дорожками:
  сдвинулось всё, что на ЭТОЙ дорожке, а параллельная музыка/титры остались.
- Тайм-стретч с сохранением питча тем заметнее портит звук, чем больше
  коэффициент; на сильных замедлениях выбор становится «неразборчиво низкий
  голос» либо «металлический артефакт».
- Скорость и обработка кадров — ортогональные настройки, поэтому «просто
  замедлить» по умолчанию даёт повтор кадров и рывок: с точки зрения
  пользователя фича «работает», но выглядит сломанной.
- Редактирование velocity-огибающей в Vegas 2026 до сих пор промахивается
  мимо события (баг заведён в апреле 2026, к сентябрю не закрыт) — цена того,
  что огибающая рисуется поверх события и конкурирует с его же хват-зонами.

## Что говорят пользователи

> I wanted to ask if there is a way to watch playback at a faster speed while
> keeping the audio at the same pitch
> — Yazura, 09.07.2026,
> https://forum.borisfx.com/t/playback-audio-when-playing-at-faster-speed/24562

> ExpressFX Time Stretch changes the audio playback rate while attempting to
> not alter the pitch. If you double the playback speed of a video clip that
> contains an embedded audio stream, try a 50% time stretch on the audio
> portion.
> — Howard-Vigorita, ответ в том же треде, 10.07.2026,
> https://forum.borisfx.com/t/playback-audio-when-playing-at-faster-speed/24562

> I speed up the video to 4x and then add a velocity envelope to speed it up
> even further.
> — galKINO, 25.04.2026,
> https://forum.borisfx.com/t/velocity-envelopes-and-legacy-plugins-in-vegas-2026/22851

> the footage comes out all choppy, and when I scrub through it, every other
> frame is the same as the previous one.
> — James Strecker, 20.09.2016,
> https://creativecow.net/forums/thread/60fps-slow-motion-not-workingae/

> I don't have a clear repro yet, but this is a bug.
> — VegasFlorian (разработчик) о промахах курсора при правке velocity-огибающих,
> 02.04.2026,
> https://forum.borisfx.com/t/timeline-jumping-when-editing-with-velocity-envelopes/21603

> Is there a way to time-stretch a comp, but keep audio clip pitch and
> duration?
> — hhhhhhhhhhhhhh5, 07.10.2020; принятый ответ Rick Gerard: «Stretch your
> audio in Audition.»,
> https://community.adobe.com/questions-529/time-stretch-but-keep-audio-pitch-33246

## Как ложится на Kinotrain

### Что уже есть

Р.37 (пп.246–250) закрыл бо́льшую часть механики, поэтому заметку надо читать не
как «дыра», а как «сделана одна из двух моделей».

- Скорость — параметр `rate` слота `SlowedReverb` (`src/libs/kt-model/src/`
  `project.rs`, декларация `params_for!`: 0.25…32, шаг 0.01), автоматизируется
  обычной кривой; `FxInstance::warps_time()` помечает такой слот.
- Видео идёт по интегралу скорости: `clip_material_pos` и `clip_video_warp` в
  `src/libs/kt-engine/src/video.rs`, карта — `kt_fx::warp_material_ticks`
  (`src/libs/kt-fx/src/lib.rs`, `RATE_CLAMP = (0.25, 32.0)`). Правка ОДНА, от
  неё следуют `layers_at`, `wanted_frames`, префетч и экспорт — инвариант №7
  соблюдён по построению.
- Режим добычи кадра — `FrameMode::{Nearest, Blend}`, смешивание считается в
  ОБЩЕМ коде плана (`clip_frame_keys` → `blend_frames`), а не в бэкендах, и
  оба кадра пары попадают в префетч (тесты 250/250b). Продолжение — F-SPD-003.
- Пара A/V держит скорость общей: `sync_warp_to_link_group` в мутациях
  `SetFx`/`SetFxAuto`/`SetFxBypass` (`src/libs/kt-model/src/ops.rs`), слот
  ищется по виду, а не по индексу (тест 248).
- Превью события считает растяжение той же картой: `ClipWarp::material_off`
  (`src/libs/kt-engine/src/lib.rs`), кэш карт по ревизии модели в
  `src/apps/kt-app/src/app.rs` (`warp_maps()`), тест 249. Инвариант №8 цел:
  меняется только «пиксель → ключ», нового декода нет.
- Реверс события — отдельный булев `ClipEvent.reversed`, composeится с варпом
  как «варп, ПОТОМ зеркало» (тест 247c).
- Тесты: `src/libs/kt-engine/tests/video_warp.rs` (247, 247b–e, 250),
  `src/libs/kt-engine/tests/fx.rs` (246, 246b), `src/libs/kt-model/tests/`
  `fx_auto.rs` (248), `src/apps/kt-app/src/layout_tests.rs` (248-video,
  249), живой сценарий `scripts/live/slowed.py`.

### Чего не хватает

1. **Второй модели целиком.** В `Command` (`src/libs/kt-model/src/model.rs`)
   нет ни одной операции «скорость ↔ длина»: есть `TrimStart`/`TrimEnd`/
   `Split`/`MoveClip`, скорость приезжает только через `SetFx`. Чтобы
   замедлить кусок и показать его целиком, пользователь обязан вручную
   вытянуть край события на нужную длину — то есть проделать за редактор
   арифметику `duration ÷ rate`.
2. **Ripple отсутствует как понятие.** Единственное упоминание в коде —
   комментарий `// Без ripple.` в `src/libs/kt-model/src/ops.rs`.
3. **Сохранения высоты тона нет и быть не может «галкой»:** наш варп — это
   `rubato::SincFixedOut` с отношением `1/rate` (варispeed), высота тона едет
   за скоростью по построению. Более того, для эффекта с названием
   Slowed+Reverb это не дефект, а жанр. Тайм-стретч — второй алгоритм.
4. **Нет нуля и отрицательной скорости.** `RATE_CLAMP` = 0.25…32: стоп-кадра
   (0%) и «отмотки внутри события» (−100%) нет, реверс — свойство всего
   события. У Vegas это одна огибающая от −100% до 1000%.
5. **Скорость не видна и называется не так.** Число живёт внутри слота
   `"Slowed+Reverb"` (`src/apps/kt-app/src/fx_editor.rs`), на самом событии
   не написано ничего; монтажёр, ищущий «скорость», найдёт её только через
   FX-пикер. Vegas пишет процент растяжения прямо в углу аудио-события.
6. **`Split`/`TrimStart`/`punch_out_region` считают `source_in` линейно** и на
   варпнутом клипе врут — записано в решениях Р.37 (`docs/user-feedback.md`)
   как известный дефект вне объёма раунда. Пока это лечится вето: морфер-слой
   такой клип не режет вовсе (тест 247e).

### Что мешает

- **Монотонность карты — предусловие декодера, а не стиль.** `read_video_span`
  (`src/libs/kt-media/src/lib.rs`) требует, чтобы ключи были «выровнены по
  сетке, строго возрастают», и на этом стоит «GOP декодируется один раз на
  спан». Отрицательная скорость внутри события ломает это предусловие, а не
  просто расширяет кламп; заодно она немонотонит `wanted_frames`
  (`src/libs/kt-engine/src/video.rs`), а на равенстве «префетч == прочитанное»
  стоит инвариант №4 и тест 247d.
- **Прецедент реверс-транспорта.** `render_frames_raw`
  (`src/libs/kt-engine/src/audio.rs`) намеренно играет клипы БЕЗ FX: при
  подаче материала задом наперёд потоковые состояния (реверб, ресемплер)
  бессмысленны — «хвост реверба звучал бы из будущего», и там же записано, что
  Vegas в reverse-скрабе ведёт себя так же. Отрицательная скорость внутри
  события — это тот же случай, только теперь он попал бы в ОБЫЧНОЕ
  воспроизведение и в экспорт, где отключить FX нельзя (инварианты №2 и №7).
- **Длина события и ось кривой связаны.** Ключи хранятся в тиках материала, а
  доля вдоль события считается как `(k.t − source_in)/duration`
  (`AutoProfiles::from_curves`). Операция «скорость меняет длину» меняет
  `duration` — и тем самым переразмечает ВСЕ кривые события. Это надо решать
  явно (пересчёт ключей в той же мутации), иначе рампа поедет.
- **Ripple против правила UI №20** («ничего не отрастает и не отрубается само»,
  «умная логика, которая доправляет соседей за пользователя, — дефект»). Это
  не запрет, а требование: ripple может существовать только как ЯВНО
  выбранный режим/галка в диалоге, а не как автоматическое поведение смены
  скорости, и обязан попадать в тот же undo-шаг, как это уже сделано у
  `reconcile_transitions` и `sync_warp_to_link_group`.
- **Тайм-стретч упирается в детерминизм.** `GOLDEN_AUDIO` перегенерирован
  «второй и последний раз» по прямому решению пользователя (п.246), значит
  новый DSP обязан по умолчанию быть выключен и не менять ни одного байта на
  существующих проектах (правило крейта kt-fx: умолчание нового параметра =
  прежнее поведение байт-в-байт). Инварианты №1 и №11.

### Какие инварианты задевает

№1 (звук f32/48k — тайм-стретч живёт в том же тракте), №2 (состояние
тайм-стретчера — только в аудио-render-потоке), №4 (смена длины/скорости меняет
множество нужных кадров — coverage считает кэш), №5 (очередь MediaWorker и
предусловие спан-ридера), №7 (карта «тик → кадр источника» одна на превью и
экспорт — правка только в `clip_material_pos`), №8 (превью событий по карте, но
без нового декода), №11 (GOLDEN_AUDIO/GOLDEN_VIDEO). Правила UI: №4 (полезное
умолчание — «ближайший» кадр и ripple выключен), №9 (никаких вторых
выключателей того же смысла — галка слота уже есть), №13 (процент скорости
писать на УЖЕ занятом месте события, как метку/уровень, а не новой полосой),
№17 (жест `Ctrl`-drag края и пункт меню обязаны давать одно и то же), №20.

## Оценка

| Что | Ответ |
|---|---|
| Объём | M — 2–3 стадии: операция «скорость ↔ длина» + ripple как явный режим + показ процента на событии. «Сохранение высоты тона» сюда НЕ входит: это отдельный DSP и отдельная заметка (L) |
| Где основная работа | kt-model (новая команда, пересчёт кривых, правило в `mutate_on` рядом с `sync_warp_to_link_group`), kt-app (жест `Ctrl`+край, показ процента, диалог), kt-engine — почти не трогается: карта варпа уже есть |
| Формат проекта | не трогает: скорость уже сериализуется параметром слота, длина — обычный `duration` |
| Тесты, которые придётся написать | команда меняет длину и не портит кривые (модель, undo одним шагом); ripple двигает ровно то, что за событием, и ничего на других дорожках; пара A/V меняет длину синхронно; `Split`/`Trim` на варпнутом клипе считают `source_in` по карте (закрывает известный дефект Р.37); харнесс-тест жеста и показа процента |
| Зависит от | F-SPD-002 (кривая ретайма — UI переменной скорости), F-SPD-003 (генерация промежуточных кадров), F-AUT-002 (домен ключа) |
| Ломает совместимость | нет |

## Вердикт

Берём — но именно НЕДОСТАЮЩУЮ половину: операцию «скорость задаёт длину
события» с явным ripple и показом процента на событии. Дорогая часть (карта
варпа, префетч, превью, синхронность пары A/V) уже сделана в Р.37, поэтому
вторая модель — это операция модели плюс жест, а не раунд движка.

Отрицательную и нулевую скорость внутри события НЕ берём: она ломает
предусловие «ключи строго возрастают» у спан-ридера и повторяет проблему
реверс-транспорта (FX при обратной подаче бессмысленны), а стоп-кадр и реверс у
нас уже выражаются иначе.

«Сохранение высоты тона» — отдельная заметка и отдельный раунд: это второй DSP,
а не галка на существующем, и он обязан по умолчанию не менять ни одного байта
звука.

## Источники

- https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/adjusting_eventslength.htm — `Ctrl`+drag края, показ процента, Time stretch/pitch shift (дата обращения 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/event_envelopes.htm — velocity-огибающая: длина события не меняется, −100%…0%…300%, отрицательное = реверс (2026-09-07)
- https://help.magix-hub.com/video/vegas/en/content/topics/faq/faq_slowmo.htm — три способа замедления и переключатель Resample (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/adjustmentevents.htm — «Certain functions, such as reversing or adding a Velocity envelope, do not apply to adjustment events» (2026-09-07)
- https://forum.borisfx.com/t/vegas-pro-release-history/24284 — VP14 build 161: потолок огибающей 1000%, общая скорость 40× (2026-09-07)
- https://forum.borisfx.com/t/suggestions-for-boris-regarding-vegas-pro-2026-very-long-post/24911 — счёт «4× × 3× = 12×» и ответ jetdv, 01.08.2026 (2026-09-07)
- https://forum.borisfx.com/t/timeline-jumping-when-editing-with-velocity-envelopes/21603 и https://forum.borisfx.com/t/patch-vegas-pro-2026-0-3-build-189/25206 — открытый баг правки огибающих, апрель–сентябрь 2026 (2026-09-07)
- https://forum.borisfx.com/t/playback-audio-when-playing-at-faster-speed/24562 — питч при ускоренном воспроизведении, 07.2026 (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/change-clip-speed-ver40b00150/mac — постоянная скорость меняет длину и рипплит по умолчанию; снятый ripple = «shorter piece of the action» (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/preserve-pitch-in-retimed-clips-verb6aca45d/mac — питч сохраняется по умолчанию (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/whatsnew — Resolve 21, Optical Flow speed change (2026-09-07)
- https://davinciresolveclub.com/change-clip-speed-davinci-resolve/ — поля диалога Change Clip Speed, Ripple Sequence, Pitch Correction (2026-09-07)
- https://helpwiki.evergreen.edu/wiki/index.php/Speed_and_Duration_-_Premiere_Pro — формулировки Maintain Audio Pitch, Ripple Edit Shifting Trailing Clips, кнопка-цепочка (2026-09-07)
- https://community.adobe.com/questions-529/time-stretch-but-keep-audio-pitch-33246 — в AE сохранения питча нет, ответ «растяните в Audition» (2026-09-07)
- https://creativecow.net/forums/thread/60fps-slow-motion-not-workingae/ и https://creativecow.net/forums/thread/frame-blending-issue-when-changing-speed-in-premie/ — рывки от повторов кадров, поведение frame blending (2026-09-07)
- Наша сторона: `docs/porting-notes.md` пп.246–251, `docs/user-feedback.md`
  раунд Р.37.

Оговорки к достоверности. Живьём не запускался ни один из пяти редакторов.
Строки Vegas собраны по официальной справке (версии 22/23 — структура топиков
2026 г. по тем же адресам не открылась) и форуму Boris FX 2026 г.; строка
Premiere опирается на вторичные источники, потому что `helpx.adobe.com` из
этого окружения не открывается — формулировки диалога Speed/Duration взяты с
учебного вики Evergreen и потому помечаю их как **[НЕ ПРОВЕРЕНО по
первоисточнику]**. Поля диалога Resolve взяты из вторичного разбора
(davinciresolveclub, 2026) — **[НЕ ПРОВЕРЕНО по Reference Manual]**; там же
упомянута пара «Maintain Timing / Stretch to Fit» для ключей при ретайме,
которую подтвердить первоисточником не удалось, поэтому в тело заметки она не
вынесена. `reddit.com` и `web.archive.org` недоступны, бюджет `WebSearch`
исчерпан — поиск шёл прямыми запросами к Yahoo и к встроенному поиску Discourse
форума Boris FX.
