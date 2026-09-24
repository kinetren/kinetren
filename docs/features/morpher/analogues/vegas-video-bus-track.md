---
id: F-MPH-005
title: Уровни применения видео-FX в VEGAS и Video Bus Track
status: в-работе
priority: B
cost: XL
category: morpher/analogues
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: частично
crates: [kt-model, kt-engine, kt-gpu, kt-otio, kt-app]
invariants: [7, 11]
ui_rules: [13, 17]
round: Р.62
point: null
tests: [t735, t736, t737, t739]
depends_on: []
checked: 2026-09-07
---

# F-MPH-005 — Уровни применения видео-FX в VEGAS и Video Bus Track

## Суть

В VEGAS один и тот же плагин можно повесить на четыре разных места, и от
выбора места зависит, ЧТО он увидит на входе: медиафайл целиком (Media FX),
одно событие (Event FX), выход одной дорожки (Track FX) или готовый кадр
всего проекта (Video Output FX). Последний уровень живёт на отдельной строке
таймлайна — Video Bus Track, — где кроме цепочки эффектов есть три
огибающие на весь микс. Порядок между уровнями жёсткий и не настраивается:
монтажёр выбирает не «когда», а «где». Adjustment event — пятый способ, и он
не заменяет ни один из четырёх.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Media FX · Event FX · Track FX · Video Output FX (Video Bus Track) | четыре уровня в жёстком порядке; вход каждого — результат предыдущего; шина — строка таймлайна с цепочкой и тремя огибающими на весь проект |
| Adobe Premiere | 26.x | Adjustment Layer; Master Clip Effects; вложенная секвенция | дорожечных ВИДЕО-FX нет вообще: уровень «дорожка» подменяется клипом-слоем сверху, уровень «мастер» — вложением секвенции. У аудио есть всё: Track FX, Submix, Master |
| DaVinci Resolve | 20.3 | Clip · Group Pre-Clip · Group Post-Clip · Timeline (+ Adjustment Clip на Edit-странице) | те же четыре уровня, но грейд уровня Timeline применяется к КАЖДОМУ клипу отдельно, а не к смикшированному кадру; дорожечных FX на Edit-странице нет |
| Final Cut Pro | 12.3 | Adjustment Clip (встроен с 11.1) | дорожек нет как понятия, значит нет и дорожечного уровня; группировка — compound clip; уровня «выход проекта» нет |
| After Effects | 26.x | Adjustment Layer, precomp | уровень «дорожка» отсутствует (слой = клип), уровень «выход» подменяется вложением композиции в композицию |

## Фишка фишки

**1. Кнопка Track FX означает две разные вещи в зависимости от флага
дорожки.** Обычная дорожка: вход цепочки — только её собственные события.
Дорожка, вставленная через `Insert | Adjustment Track`: «Adjustment tracks
show the Track FX button in the track header by default. You can click this
button to access the Plug-In Chooser and add the video effects you want to
pass through to other tracks» — та же кнопка, то же окно, но на входе уже
смикшированная картинка всех дорожек НИЖЕ. Ограничить охват можно, объявив
нижние дорожки compositing children: «any tracks that are not children of
the adjustment tracks — even though they are below the adjustment track —
are no longer influenced by it». То есть уровень задаётся не тем, куда ты
положил плагин, а свойством строки, и по внешнему виду цепочки отличить
один случай от другого нельзя.

**2. Внутри уровня порядок свободен, между уровнями — прибит гвоздями, и
именно поэтому существует скрытый пятый уровень.** В цепочке события
Pan/Crop — такой же перетаскиваемый узел, как плагины: «you can choose
whether effects are applied to the event before or after any event panning
and cropping by arranging the Pan/Crop button in the plug-in chain at the
top of the Video Event FX window». А вот поменять местами Event FX и Track
FX нельзя ничем: единственный способ «переставить» — перенести плагин на
другой уровень. Ради этого и держат Media FX, до которого добираются
Alt+кликом по иконке Event FX: это единственное место РАНЬШЕ цепочки
события. Классический сценарий («хочу ч/б на кусок поверх дорожечной
коррекции») решается не флагом, а переездом коррекции на медиа-уровень.

**3. Шина — это дорожка, а не просто цепочка, ради трёх огибающих, две из
которых вообще не фильтры.** Кроме fade-to-color на Video Bus Track живут
motion blur amount и video supersampling — они не обрабатывают готовый
кадр, а меняют, СКОЛЬКО промежуточных кадров посчитает композитор:
«supersampling cannot improve the appearance of existing video», работает
только по движению, которое VEGAS порождает сам (track motion, pan/crop,
переходы, кейфреймы эффектов). Такому параметру нужна линия во времени, а
цепочке эффектов — нет; отсюда отдельная строка. И там же вылезает домен
ключей: у шины событий нет вообще, поэтому её ключи всегда во времени
ПРОЕКТА, а на обычных дорожках то, поедут ли ключи за событием, решает
глобальный переключатель `Options | Lock Envelopes to Events` — свойство
приложения, а не эффекта и не клипа.

## Плюсы

- Плагин, повешенный на шину, считается по одному разу на кадр; тот же
  плагин на уровне события считается столько раз, сколько событий в кадре.
  На шестислойном композите это разница в шесть прогонов ядра.
- Уровень «выход проекта» видит кадр ПОСЛЕ композита, поэтому финальные
  вещи (лимитер уровней, зерно, виньетка, LUT показа) применяются один раз
  и одинаково ко всему, включая фон и края, куда не достаёт ни одно
  событие.
- Дорожечный уровень наследуется автоматически: новое событие, брошенное на
  дорожку, уже обработано её цепочкой — ничего не надо копировать. Именно
  этого не хватает в Premiere, где дорожечных видео-FX нет и слой-заменитель
  приходится растягивать руками.
- Adjustment event, будучи ОБЫЧНЫМ событием («just like any video event, you
  can add up to 32 effects in the chain of effects for the event»), даром
  получает всё событийное: трим, сплит, копипаст, уголковые фейды и
  ползунок непрозрачности. Отсюда и подмешивание эффекта частично, и
  кроссфейд между двумя разными коррекциями.
- Обход есть на каждом уровне и на каждом плагине: галка у отдельного
  плагина в окне цепочки, «disable all / enable all» по правому клику на
  иконке Track FX, одна кнопка «Bypass FX and Envelopes» на шине.

## Минусы

- Четыре уровня — четыре места, где искать, почему картинка не такая, и
  ни одно из них не видно с таймлайна: иконка Track FX выглядит одинаково с
  цепочкой и без, Media FX не имеет своего значка на событии вовсе, а
  Video Output FX живёт в строке, которая по умолчанию скрыта (View | Video
  Bus Track).
- Порядок нельзя ни увидеть, ни изменить. Ожидание «я поставил эффект на
  событие, значит он последний» ломается всегда одинаково, и лечится только
  знанием списка уровней наизусть.
- Результат зависит от уровня не только по порядку, но и по тому, что
  смешивается: на кроссфейде между двумя событиями коррекция, стоящая на
  дорожке, и та же коррекция, стоящая на событиях, дают разную картинку —
  ловится по вылету чёрного на RGB-параде.
- Раздельного предпросмотра по уровням нет. Split-screen «FX Bypassed»
  выключает всё сразу — «bypasses all video effects (track effects, event
  effects, media effects, and video output effects)»; посмотреть «без
  дорожечного уровня, но с событийным» нельзя.
- Шина ровно одна: «a single bus track exists as a timeline representation
  of the main video output». Ни сабмиксов, ни посылов, ни параллельных
  веток — при том что у АУДИО в том же VEGAS есть и шины, и assignable FX.
  Заменитель видео-сабмикса — adjustment track с compositing children, то
  есть та же работа делается совсем другим механизмом.
- У шины нет событий, значит нет и обрезки: ограничить шинный эффект куском
  фильма можно только ключевыми кадрами (или байпасом руками), а ключи там
  во времени проекта — врезка в начало таймлайна уводит их все.

## Что говорят пользователи

> Vegas processes FX in this order: Media FX –> Event FX –> Track FX –>
> Master Video FX. This is why your Track FX and overriding your Event FX.
> — John Rofrano, Creative COW, 12.05.2011,
> https://creativecow.net/forums/thread/any-way-to-apply-event-fx-after-track-fx/

> Aha, "Media FX." I am new to Vegas, I had NO IDEAL that these existed.
> — David Hall (автор темы), Creative COW, 12.05.2011,
> https://creativecow.net/forums/thread/any-way-to-apply-event-fx-after-track-fx/

> Right click the track fx icon for a given track and you should see a menu
> giving options to disable all and enable all fx for that track.
> — Norman Black, Creative COW, 06.06.2013,
> https://creativecow.net/forums/thread/vide-fx-bypass-ae/
> (спрашивавший ответил, что искал это в мануале и не нашёл)

Показательно не то, что люди путаются, а КТО путается: автор первой темы
монтировал в VEGAS не первый день и просто не знал о существовании
медиа-уровня, пока порядок обработки не ударил его по рукам. Отрицательный
фидбек по шине почти отсутствует — её просто не включают: строка скрыта по
умолчанию, и большинство сценариев закрывается adjustment-событием.

## Как ложится на Kinotrain

### Что уже есть

Уровней применения видео-FX у нас ровно два, и оба — событийные.

- Цепочка события: `ClipEvent.fx` / `fx_bypass` / `fx_auto` / `fx_off` в
  `src/libs/kt-model/src/project.rs` (около строки 1502).
- Морфер-слой: `MorpherEvent.patch.video_fx`, который в
  `MorphPatch::apply_to` (там же) делает буквально `c.fx.extend(add)` —
  ДОПИСЫВАЕТ свои плагины в цепочку каждой цели. Раскладка целей —
  `Project::effective_clips()`, накопление снизу вверх по
  `Project.lane_order` / `LaneKind`, правка через `Command::SetLaneOrder`
  (`src/libs/kt-model/src/model.rs`).
- Исполнение: `LayerSrc.fx` в `src/libs/kt-engine/src/video_plan.rs` —
  цепочка ПЕР-СЛОЙ; `apply_video_chain` в `src/libs/kt-fx/src/video.rs`
  зовётся до композита, композит (`compose_from_cache` в
  `src/libs/kt-engine/src/video.rs`) идёт после.
- Уровень клипа одной линией: `ClipEvent.opacity` / `gain_db`, уголковые
  фейды — переход с одним живым концом.
- Аудио-мастер существует, но только как громкость: `set_master_gain` в
  `src/libs/kt-engine/src/player.rs:375`, ручка — в
  `src/apps/kt-app/src/meters.rs:822`.
- Автоматизация: `ParamCurve` / `AutoKey` (`project.rs`), виджет —
  `src/apps/kt-app/src/automation.rs`. Домен ключа — ТИК МАТЕРИАЛА:
  `curve_value(keys, ease, at)` берёт материальную позицию, поэтому обрезка
  ключи прячет, а не сдвигает. Это ровно противоположный выбор относительно
  шины VEGAS (время проекта) и относительно `Lock Envelopes to Events`
  (глобальный тумблер): у нас домен один и переключателя нет.

### Чего не хватает

Грепом по `src/` (`track_fx|lane_fx|bus_fx|master_fx|output_fx|submix`) —
ноль совпадений. Нет ни дорожечного, ни шинного, ни мастер-уровня для
видео; для аудио нет дорожечных FX и сабмиксов. Главное же: у нас нет НИ
ОДНОГО уровня, вход которого — смикшированная картинка. Морфер выглядит как
adjustment layer, но по устройству он ближе к Timeline-грейду Resolve
(правится один раз, действует на каждый клип отдельно), а не к Video Bus
Track и не к adjustment event VEGAS. Отсюда же тянется отсутствие ползунка
силы у морфера (п.245 `docs/porting-notes.md`, Р.36
`docs/user-feedback.md`): «частично подмешать» нечего, пока нет отдельного
результата композита.

### Что мешает

`FramePlan` (`src/libs/kt-engine/src/video_plan.rs`) описывает СЛОИ, а не
конвейер: у него есть `layers`, но нет объекта «готовый кадр», к которому
можно приложить цепочку. Появление шинного уровня — это новая стадия ПОСЛЕ
композита, и её надо родить сразу в двух исполнителях: CPU
(`compose_from_cache*`) и GPU (kt-gpu, второй проход по цели). Ключ
пост-FX-мемоизации сейчас пер-слойный (`LayerSrc.fx_hash`) — шинная цепочка
меняет весь кадр, значит нужен ключ уровня кадра, иначе кэш начнёт отдавать
чужое. Плюс: цепочка уровня проекта — новое поле модели, а значит версия
формата `KT_META_V5` в `src/libs/kt-otio/src/meta.rs` (сейчас
`KT_META_VERSION = 4`). И строка шины на таймлайне — это ещё один вид
`LaneKind`, который обязан попасть в `lane_order` и не сломать тест 49.

### Какие инварианты задевает

- **№7 (код композиции один).** Стадия «эффекты на смикшированном кадре»
  обязана появиться внутри `VideoRenderer::frame_plan`, а не отдельной
  веткой в экспорте — иначе получим ровно второй путь, против которого
  инвариант и написан.
- **№11 (детерминизм).** Новая стадия после композита проходит через
  GOLDEN_VIDEO и через конформанс CPU↔GPU (`kt_gpu::assert_frames_close`),
  и через «превью == экспорт байт-в-байт» на одном бэкенде.
- **Правило UI 13 («не тратить место впустую»).** Постоянная строка шины
  ради одной цепочки — ровно тот случай, за который у нас ругают: у VEGAS
  она скрыта по умолчанию, и это признание проблемы, а не решение.
- **Правило UI 17 («парные способы»).** Если уровень появится, у него
  сразу должны быть и байпас уровня, и A/B-показ, иначе повторим минус
  VEGAS: обход есть только «всё сразу».

## Оценка

| Что | Ответ |
|---|---|
| Объём | XL — раунд плюс версия формата и новая стадия рендера |
| Где основная работа | kt-engine (`video_plan`/`video`: стадия после композита), kt-gpu (второй проход), kt-model (поле проекта + undo + `LaneKind`), kt-otio (v5), kt-app (строка и её FX-редактор) |
| Формат проекта | нужна версия v5 (`KT_META_V5`); старые файлы = пустая цепочка |
| Тесты, которые придётся написать | round-trip модели с шинной цепочкой; конформанс CPU↔GPU новой стадии; превью == экспорт (расширение t67/t184); неизменность t36 (морфер == bake) и GOLDEN_VIDEO без шинной цепочки; ключ пост-FX-кэша уровня кадра |
| Зависит от | — (но «ползунок силы морфера» логически идёт следом: обе фичи упираются в отсутствие объекта «смикшированный кадр») |
| Ломает совместимость | нет |

## Вердикт

Берём в план, но не первым: ценность здесь не в «ещё одном месте для
плагинов», а в том, что уровень со смикшированным кадром на входе — это
недостающий кусок под ползунок силы морфера и под честные финальные
эффекты. Четыре уровня целиком копировать НЕ надо: минус «четыре места, где
искать» реален, и VEGAS платит за него скрытой по умолчанию строкой. Наш
разумный минимум — один уровень выхода проекта плюс, отдельным решением,
переключатель «морфер работает на композите ниже» — то есть то, чего у
VEGAS как раз нет: у них Track FX и adjustment track различаются флагом
строки, а не переключателем на самом слое.

## Источники

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/bus_tracks-video.htm — Video bus track, VEGAS Pro 2026 (дата обращения 2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/adjustmenttracks.htm — Adjustment tracks, 2026 (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/adjustmentevents.htm — Adjustment events, 2026 (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/8-design/addvideofx_media.htm — Adding video media effects, 2026 (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/1-before/whatsnew.htm — What's New в 2026: изменений в уровнях FX НЕТ, новое — Speech-to-Text/Text-to-Speech, Storyblocks, Z-Depth 2.0, Continuum Title Pack, Blackwell 4:2:2 (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/8-design/addvideofx.htm — Adding video effects, v22 (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/5-preview/split-screen_previews.htm — Split-screen previews / FX Bypassed (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/2-window/pancropwindow.htm — Pan/Crop как узел цепочки события (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/11-autoscript/insertvidenv.htm — Video track automation, Lock Envelopes to Events (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/videotrack.htm — состав заголовка видеодорожки (2026-09-07)
- https://creativecow.net/forums/thread/any-way-to-apply-event-fx-after-track-fx/ — порядок уровней и Media FX (2026-09-07)
- https://creativecow.net/forums/thread/vide-fx-bypass-ae/ — байпас цепочки дорожки правым кликом (2026-09-07)
- https://creativecow.net/forums/thread/vegas-12-crossfades-with-color-correction/ — расхождение картинки на кроссфейде между уровнями (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/can-i-apply-a-video-effect-to-a-track/m-p/2793436 — в Premiere дорожечных видео-FX нет (2026-09-07)
- https://mixinglight.com/color-grading-tutorials/revisiting-resolve-grouping-using-post-clip-as-a-timeline-level-grading-alternative/ — уровни грейда Resolve (2026-09-07)
- https://www.cined.com/apple-final-cut-pro-11-1-introduces-adjustment-clips-and-enhances-magnetic-masks/ — Adjustment Clips встроены в FCP с 11.1 (2026-09-07)

Замечание о доступности: форум vegascreativesoftware.info с 2026 г. отвечает
301-редиректом на корень https://forum.borisfx.com/ — прямые ссылки на
старые треды Vegas-форума мертвы, поэтому фидбек здесь собран с Creative COW
и Adobe/Blackmagic-площадок. Живьём в VEGAS 2026 не проверялось: заметка
собрана по официальной справке 2026 г. и форумным тредам.
