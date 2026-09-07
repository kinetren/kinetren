---
id: F-MSK-003
title: Track matte — один клип задаёт прозрачность другого
status: кандидат
priority: B
cost: L
category: masks/keying
editors: [after-effects-26, premiere-26, vegas-pro-2026, resolve-21, fcp-12]
we_have: нет
crates: [kt-model, kt-engine, kt-gpu, kt-fx, kt-media, kt-cache, kt-otio, kt-app]
invariants: [4, 5, 7, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MSK-003 — Track matte — один клип задаёт прозрачность другого

## Суть

Монтажёр кладёт два клипа друг на друга и говорит: верхний не показывай,
а используй его как трафарет для нижнего. Белое (или непрозрачное) в
трафарете — нижний виден, чёрное (прозрачное) — дырка, серое —
полупрозрачно. Четыре вида: **Alpha** (берётся альфа трафарета),
**Alpha inverted**, **Luma** (берётся ЯРКОСТЬ трафарета), **Luma
inverted**. На этом стоят титры «текст-окно», световые вскрытия,
reveal-переходы, размытие лица по движущемуся пятну — всё, где форма
прозрачности меняется во времени. В быту почти всегда берут luma: любой
градиент, любое чёрно-белое видео, любая надпись на чёрном становятся
маской без единого нарисованного контура.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| After Effects | 26.x | Track Matte — свойство СЛОЯ (плюс отдельный эффект Set Matte) | у слоя колонка TrkMat: вид (Alpha / Alpha Inverted / Luma / Luma Inverted) + ССЫЛКА на слой-матт. До AE 23.0 (10.2022) ссылки не было — маттом служил слой ровно сверху. С 23.0: «Any layer in your comp, no matter where it sits in the layer stack, can be used as a Track Matte source for any other layer. You can even have multiple layers all reference a single layer for their matte». Видимость слоя-матта AE гасит САМ и обратно не включает |
| Adobe Premiere | 26.x | Track Matte Key — ЭФФЕКТ | вешается на клип-носитель; параметр `Matte` — выпадающий список ДОРОЖЕК (`Video 2`, `Video 3`, …), `Composite Using` — `Matte Alpha` / `Matte Luma`, плюс `Reverse`. Дорожку-трафарет гасит монтажёр, редактор её не трогает. Трафарет ездит за Position носителя НЕЗАВИСИМО от порядка эффектов — по словам модератора Adobe, «longstanding quirk» и «it has never behaved the same way as it does in After Effects». В версии 26 Adobe переселила МАСКИ (не матты) из Effect Controls в Object Mask / Vector Masking на панель инструментов и рекомендует их вместо Track Matte Key |
| VEGAS Pro | 2026.0 | режима «track matte» нет: композит-режим дорожки `Multiply (Mask)` + Parent/Child + плагин Mask Generator | матт — это ДОРОЖКА-родитель, а маскируемое — её дочерние дорожки: «The parent track is the highest track in a group of tracks and the behavior of the child tracks is determined by the parent track». Не-серое изображение доводится до матта плагином Mask Generator как Track FX. Родство дорожек — часть структуры проекта, а не свойство клипа (та же логика, что у Adjustment Events, F-MPH-001, и Video Bus Track, F-MPH-005) |
| DaVinci Resolve | 21 (проверялось и по 20.x) | на Edit-странице сущности «track matte» нет | три пути: (1) композит-моды `Alpha` / `Inverted Alpha` / `Foreground`, которыми верхняя дорожка работает маттом для нижней — появились в Resolve 17, release notes: «Support for compositing using track mattes or external alpha» [сниппет выдачи, первоисточник не открылся]; (2) External Matte на Color-странице — файл цепляется к клипу в медиапуле («Add as Matte») и становится входом узла; (3) Fusion — MatteControl/Merge, альфа возвращается на Edit |
| Final Cut Pro | 12.3 | track matte НЕТ, эффекта Set Matte НЕТ | вместо ссылки — режимы наложения группы «alpha channel» у самого клипа-трафарета: Stencil Alpha, Stencil Luma, Silhouette Alpha, Silhouette Luma. Apple: «Does the same thing as the Stencil Alpha blend mode, but uses the affected clip's luma value to define transparency. Stencil Luma is useful if the clip you want to use for cropping has no alpha channel of its own». Ближайший «матт из чужой картинки» — эффект Image Mask, но он берёт канал из ОТДЕЛЬНОГО изображения, а не из клипа на соседней дорожке |

## Фишка фишки

**1. Матт-слой выключается из показа — и три редактора решают это тремя
разными способами, из чего растут три разных грабли.** AE гасит «глазик»
слоя-матта сам и НЕ включает обратно, когда матт снимают: слой остаётся в
композиции, но невидим, и новичок считает, что потерял его. Это не баг, а
следствие модели: «Layers used as mattes are not normally on (visible)»,
и с AE 23.0 иначе нельзя — один матт обслуживает несколько слоёв, и
«вернуть глазик» после снятия матта у ОДНОГО из них было бы ошибкой.
Premiere не гасит ничего: дорожка-трафарет продолжает рисоваться поверх
всего, и её выключают руками — забыл, и вместо вскрытия видно сам
трафарет. FCP не гасит и не может: у него трафарет — это клип с режимом
наложения Stencil, то есть он и есть носитель эффекта.

**2. Явная ссылка на матт стоит дороже, чем кажется, и Adobe пришлось
откатывать половину.** В AE 23.0 неявную связь («слой ровно сверху»)
заменили на выбор слоя из списка и pick whip. Выигрыш очевиден — один
трафарет на N слоёв. Цена оказалась тройной: (а) частый случай стал
медленнее, и это сказали прямым текстом — «The new feature solved a
problem that sometimes happens, but completely ruined the workflow for
90% of the rest of the shots»; Adobe вернула команду «Matte with Layer
Above» с назначаемым хоткеем только в 23.4, через полгода; (б) ссылка
попала в формат проекта, поэтому 23.0 ломает совместимость — «Projects
saved in the 23.0 beta will not be able to be opened in other versions of
After Effects»; (в) ссылка не переезжает при копировании: «I really wish
these track mattes would update their parent link when you copy and paste
a group of them». Пункт (в) у нас уже закрыт для морферов (п.242
`docs/porting-notes.md`, тесты 249/250: ссылка на СКОПИРОВАННОЕ имя
переезжает на копию, на нескопированное — остаётся на оригинале), и это
готовая семантика для матта.

**3. Матт живёт в чужом времени и в чужой геометрии — и это ломается до
сих пор.** Трафарет — отдельный клип со своей длиной, обрезкой и
ключами; носитель — другой. Никто не растягивает трафарет по носителю:
берут кадр трафарета в ТОЙ ЖЕ точке таймлайна. Отсюда дефект Premiere
26.0.1: «the Track Matte key plug-in does not recognise that the source
clip has been trimmed and offsets the keys as if the clip is untrimmed» —
ровно тот вопрос, который у нас закрыт решением «домен ключа = тик
МАТЕРИАЛА» (F-AUT-002). По геометрии — зеркальная беда: Position
носителя ТАЩИТ трафарет за собой независимо от порядка эффектов, штатное
лечение — nesting, а Community Expert Adobe в том же треде отвечает «Have
not used Track Matte in a long time».

**4. Мелочь, которая портит края: премультиплицированная альфа и размер.**
У AE эффект Set Matte несёт чекбоксы «Premultiply Matte Layer» и «Stretch
Matte to Fit»: трафарет чужого размера РЕСЕМПЛИТСЯ, а не кропится, а
премультиплицированный без чекбокса даёт тёмную кайму. Плюс исторический
подвох: Set Matte брал ИСХОДНИК слоя, игнорируя его маски и эффекты, —
чинилось на уровне SDK («Effects that use layers as an input, such as Set
Matte and Displacement Map, can now target the input layer's masks and
effects, instead of only the source of the layer»). То есть «матт из
слоя» и «матт из того, что этот слой рисует» — две РАЗНЫЕ вещи, и их
путали сами авторы.

## Плюсы

- Трафарет — обычный клип, поэтому на него действует ВСЁ, что действует
  на клип: цепочка эффектов, автоматизация, обрезка, реверс. Мягкий край
  получается блюром трафарета, а не параметром `feather` у каждого
  эффекта; форма во времени — обычными ключами.
- Luma-матт не требует альфы ниоткуда: чёрно-белый градиент, снятая тень,
  титр на чёрном — готовые маски. Формулировка Apple ровно об этом: luma
  нужен, «if the clip you want to use for cropping has no alpha channel of
  its own». Larry Jordan объясняет выбор luma тем же: «while Final Cut
  text has alpha channels built into it, the generated shapes do not».
- Явная ссылка (AE 23.0+) делает трафарет переиспользуемым объектом:
  правка одного клипа переписывает форму сразу у N носителей — без N
  правок и N шагов undo.
- Для композита это ОДНА операция «умножь альфу слоя на число из другого
  кадра». Второго кода композиции не появляется — меняется источник
  альфы, а не способ складывать слои.

## Минусы

- Трафарет исчезает из картинки, оставаясь в проекте, и не помечен ничем,
  кроме потушенного «глазика»: его можно случайно сдвинуть, обрезать или
  удалить — сломается место, которого не трогали.
- Ссылка по номеру дорожки (Premiere) ломается от вставки, удаления и
  перестановки дорожек — операций, которые делают не думая.
- Ссылка по слою (AE) стоит скорости в частом случае «трафарет ровно
  сверху» и попадает в формат проекта, ломая обратную совместимость.
  Копирование пары «носитель + трафарет» ссылку не перевязывает.
- На кадр читаются ДВА источника вместо одного, каждый со своей цепочкой:
  удваивается декод, объём кэша и вероятность не успеть к дедлайну.
- Трафарет и носитель не связаны ни по времени, ни по геометрии: обрезал
  носитель — трафарет остался; подвинул носитель Position'ом в Premiere —
  трафарет уехал вместе с ним, хотя его об этом не просили.
- Премультиплицированная альфа трафарета даёт кайму, а трафарет другого
  разрешения масштабируется под носителя, а не кропится: «поставил 4K на
  HD» — это не кроп, и края поедут.

## Что говорят пользователи

Слой действительно «пропадает», и об этом спрашивают прямым текстом:

> This may be normal AE behavior, does a layers visability (eye ball)
> always stay off after selecting/removing a track mattee layer in the
> drop down?
> — 55878965, Adobe Support Community, 21.12.2023,
> https://community.adobe.com/questions-529/layer-visability-stays-off-after-removing-track-matte-is-that-normal-ae-behavior-54298

Ответ Adobe Community Expert в том же треде — это устройство фичи целиком:

> In the latest versions of After Effects, you can use the same layer as a
> track matte for multiple layers. Turning the Matte layer back on after
> turning off the Track matte on one of the layers would be a mistake.
> — Rick Gerard, там же, 21.12.2023

Смена модели на явную ссылку (AE 23.0) — самый ценный отрицательный
фидбек всей заметки: фичу просили годами, а получив, пожалели:

> The new feature solved a problem that sometimes happens, but completely
> ruined the workflow for 90% of the rest of the shots 😞
> — Filip23315833k75u, Adobe Community (After Effects Ideas), 11.11.2022,
> https://community.adobe.com/t5/after-effects-ideas/2023-new-track-matte-feature-needs-some-work/idi-p/13339012

Та же смена, увиденная через день после релиза:

> It looks like bug to me because there shouldn't be two Parent & Link
> windows/dropdowns. Especially when one is labeled Track Matte but works
> like parent & link. It's a bug
> — аноним, Adobe Community, 21.10.2022,
> https://community.adobe.com/t5/after-effects-discussions/track-matte-disapeared/m-p/13291308

Ссылка не переезжает при копировании — ровно то, что у нас закрыто п.242:

> I really wish these track mattes would update their parent link when you
> copy and paste a group of them.
> — jackthegiantkiller, Adobe Community, 31.05.2023,
> https://community.adobe.com/t5/after-effects-beta-discussions/feature-focus-selectable-track-matte-layers-have-arrived/td-p/13110013

Модель «эффект со ссылкой на дорожку» (Premiere) даёт репутацию
ненадёжной вплоть до отказа от фичи — причём у Community Expert:

> I've found the Track Matte effect in Premiere Pro to be unreliable
> (sometimes it mattes, sometimes it doesn't, even in the same sequence
> with the same matte).
> — Chris Walsh, Creative COW, 10.09.2020,
> https://creativecow.net/forums/thread/track-matte-premiere-pro/

> Have not used Track Matte in a long time: switch over to the Properties
> Panel. Clip and matte can be moved around independently.
> — Ann Bens (Community Expert), Adobe Community, 07.04.2026,
> https://community.adobe.com/questions-729/track-matte-key-does-not-respect-effect-order-1556713

Свежий (март 2026) баг-репорт — про развязку времён трафарета и носителя:

> Now in 2026.0.1 the Track Matte key plug-in does not recognise that the
> source clip has been trimmed and offsets the keys as if the clip is
> untrimmed.
> — LastArkTom, Adobe Support Community, 02.03.2026,
> https://community.adobe.com/bug-reports-728/track-matte-key-not-working-with-keyframes-in-pp-2026-0-1-1552070

Жёсткий порядок дорожек — первая ошибка новичка, и её принимают за
поломку ВИДА матта:

> The effect works, but only in the mode 'Matte Luma' and not in 'Matte
> Alpha'... when i use the setting Alpha Matte it dosen't work as a mask.
> — Alejandro Andersson, Creative COW, 20.04.2010,
> https://creativecow.net/forums/thread/frustrating-error-whit-track-matte-key/

> I think your matte clip has to go on the next track up, not underneath.
> — Eric Jurgenson, там же, 20.04.2010

Почему в быту берут luma, а не alpha:

> Why use Travel Matte – Luma? Because, while Final Cut text has alpha
> channels built into it, the generated shapes do not.
> — Larry Jordan, 15.05.2011,
> https://larryjordan.com/articles/travel-mattes-blurring-a-face-to-protect-the-innocent/

И то же правило бытовым языком в Premiere-треде:

> If you keep it white use the luma matte setting with the Track Matte
> Key. If you make it a color use the alpha matte setting.
> — Paul Neumann, Creative COW, 11.09.2014,
> https://creativecow.net/forums/thread/track-matte-problem-cant-choose-target-video-fro/

Наконец — как это выглядит для беженца из AE в Vegas (апрель 2026):

> I right click on the b/w clip and change alpha to 'straight (unmatted)
> as seen on a tutorial but nothing works? How do I use this to knock out
> the background correctly please?
> — johnnyswedish, forum.borisfx.com (VEGAS Pro), 07.04.2026,
> https://forum.borisfx.com/t/22023

> On the track header use 'Multiply (Mask)' From the video FX window -
> Utility - drag Mask Generator onto the B/W mask. Parent/Child the mask &
> logo clip to see the track below.
> — Gid Joiner, там же, 07.04.2026

## Как ложится на Kinotrain

### Что уже есть

- **Композит уже умеет альфу, и умеет её STRAIGHT** (не
  премультиплицированную): `alpha_over` в
  `src/libs/kt-engine/src/video.rs` (стр. 908), AVX2-близнец
  `alpha_over_avx2` в `src/libs/kt-engine/src/video_simd.rs` (стр. 154),
  WGSL-зеркало `over()` в
  `src/libs/kt-gpu/src/shaders/composite.wgsl` (стр. 9). Это ключевой
  факт для оценки: при straight-альфе матт — умножение ОДНОГО канала,
  RGB трогать не надо. При премультиплицированной пришлось бы умножать
  все четыре и перегенерировать golden-эталоны.
- **Умножение альфы на вес уже написано трижды** — скаляром, AVX2 и в
  шейдере: `scale_alpha_into` / `scale_alpha_inplace`
  (`src/libs/kt-engine/src/video.rs`, стр. 818 и 808),
  `scale_alpha_avx2` (`video_simd.rs`, стр. 107), `scale_into`
  (`composite.wgsl`, стр. 22). Матт — ровно эта операция, но множитель
  берётся из пикселя ДРУГОГО кадра, а не из скаляра `alpha_k`.
- **Целочисленная яркость для luma-матта уже есть и уже заморожена**:
  `(77·R + 150·G + 29·B) >> 8` в `src/libs/kt-fx/src/video.rs` (стр. 419
  у `edges`, стр. 685 у `glow`). Своей формулы luma заводить не нужно —
  наоборот, вторая формула была бы дефектом.
- **Флаг альфы у кадра**: `VideoFrame.has_alpha`
  (`src/libs/kt-media/src/lib.rs`, стр. 58) и `Frame.has_alpha`
  (`src/libs/kt-fx/src/video.rs`, стр. 25).
- **Единственный производитель альфы сегодня — хромакей**: `greenscreen()`
  ставит `f.has_alpha = true` (`src/libs/kt-fx/src/video.rs`, стр. 385),
  зеркало — `src/libs/kt-gpu/src/shaders/greenscreen.wgsl`.
- **Прецедент «слой берёт ВТОРОЙ кадр из кэша, а мешает их общий код
  плана»**: `LayerSrc.blend` / `LayerBlend` в
  `src/libs/kt-engine/src/video_plan.rs` (стр. 62 и 67), заполняется в
  `layer_src` (`video.rs`, стр. 486). Комментарий там прямо объясняет,
  почему смешивание сделано в плане, а не в исполнителях: «инвариант №7 —
  код композиции один, бэкенд это параметр исполнения». Матт ложится на
  этот прецедент один в один.
- **Прецедент ссылки по семантическому ключу, а не по номеру строки** — и
  он принят осознанно: `MarkedArea.lanes`,
  `src/libs/kt-model/src/project.rs` (стр. 1163+): «СЕМАНТИЧЕСКИЕ ключи
  дорожек, не номера строк: перестановка строк область не двигает».
  Рядом — `MorphTargets::Clips { ids }` (стр. 613): замороженный список
  `ClipId`, переживающий move/split/trim.
- **Прецедент перевязки ссылок при копипасте** — то самое, чего у AE до
  сих пор нет: п.242 `docs/porting-notes.md`, тесты 249/250. `ops::
  ClipBlock` возит морферы, области и метки; «ссылка морфера на
  СКОПИРОВАННОЕ имя переезжает на копию, на нескопированное — остаётся на
  оригинале», а элемент, у которого целей не осталось, ПРОПУСКАЕТСЯ и
  считается, а не «доправляется» (правило 20).
- **z-order и правило Vegas**: `ClipEvent.lane` (`project.rs`, стр. 1509,
  «z-order для видео, меньше = выше») и сортировка
  `clips.sort_by_key(|c| (Reverse(c.lane), c.start.0))`
  (`src/libs/kt-engine/src/video.rs`, стр. 331) — «сверху тот, кто
  начался позже».
- **Пост-FX мемоизация с ключом**: `fx_chain_hash`
  (`src/libs/kt-engine/src/video.rs`, стр. 153) и
  `BurstKind::PostFxVideo { quality, fx_hash }`
  (`src/libs/kt-cache/src/lib.rs`, стр. 31).
- **Масок, matte и rotoscoping нет вообще**: греп `matte|mask` по
  `src/libs` и `src/apps` даёт только `front_mask` (форма перехода в
  `transitions.rs` и `composite.wgsl`) и `mask_digits` (маска таймкода в
  `src/apps/kt-app/src/util.rs`). Ни одного поля модели, ни одного FX.

### Чего не хватает

- **Источника альфы из ФАЙЛА.** `FfmpegSource::read_video_frame` и
  `read_video_span` жёстко ставят `has_alpha: false`
  (`src/libs/kt-media/src/ffmpeg.rs`, стр. 133 и 160) — при том что
  swscale конвертирует кадр в `AV_PIX_FMT_RGBA`
  (`src/libs/kt-ffmpeg/src/source_impl.rs`, стр. 928), то есть байты
  альфы в буфере физически лежат, но движку велено считать кадр
  непрозрачным. То же у `placeholder.rs` (стр. 254) и `synthetic.rs`
  (стр. 204). **Вывод: Alpha matte у нас сегодня взять неоткуда, кроме
  выхода хромакея; Luma matte — единственный вид, который заработает без
  правок в декодере.** Это не вкусовое решение, а следствие одной строки.
- **Поля модели «этот клип — мой трафарет»** и вида матта: в `ClipEvent`
  (`src/libs/kt-model/src/project.rs`, стр. 1502+) нет ничего похожего.
- **Ещё одного входа у композита.** Шапка `composite.wgsl`: «T0 = acc_in,
  T1 = слой A, T2 = слой B (dissolve)»; бинд-группа отдаёт ровно три
  текстуры (`src/libs/kt-gpu/src/engine.rs`, стр. 1039–1041, биндинги
  1–3). Трафарет — четвёртая.
- **Доступа цепочки FX к чужому кадру.** `apply_video_chain_pooled(fx,
  frame, ctx, scratch)` (`src/libs/kt-fx/src/video.rs`, стр. 61)
  получает РОВНО ОДИН кадр и никакого доступа к кэшу. Значит модель
  Premiere («матт — это эффект») к нам не приделывается в принципе:
  матт обязан родиться в ПЛАНЕ, где кадры и берутся.

### Что мешает

1. **Модель «дорожка строго сверху» на нашу схему не ложится.** У нас нет
   дорожек-контейнеров с фиксированным порядком: есть `lane` (z-order) и
   ЗАКОННЫЕ перекрытия на одной lane с авто-кроссфейдом. Положить
   трафарет «строго над носителем» значит: (а) занять им lane, где он
   участвует в общей сортировке; (б) отдать судьбу пары правилу «сверху
   тот, кто начался позже» — сдвинул носитель вправо, и он сам стал
   верхним; (в) при перекрытии трафарета с соседом по lane получить между
   ними кроссфейд, к матту отношения не имеющий. Модель Premiere
   («матт = номер дорожки») ещё хуже: порядок строк правится через undo
   (`Project::set_lane_order`, `project.rs`, стр. 2989), и любая
   перестановка переписала бы все ссылки — ровно то, от чего мы уже
   отказались в `MarkedArea.lanes`. **Совместима с нашей моделью только
   явная ссылка на клип по `ClipId`**, как `MorphTargets::Clips { ids }`:
   она переживает move, trim, split и перестановку строк, и она же даёт
   бесплатно то, ради чего AE в 23.0 ломала формат, — один трафарет на
   несколько носителей, с уже написанной семантикой копипаста (п.242).
2. **Явная ссылка обязана иметь быстрый жест.** Опыт AE 23.0 («ruined the
   workflow for 90% of the rest of the shots», возврат команды «Matte
   with Layer Above» через полгода) читается как прямое требование
   правила 17 чек-листа UI: пикер клипа и жест «взять трафаретом клип
   ровно сверху» должны существовать ОБА и давать одинаковый результат и
   одинаковую запись в истории.
3. **Трафарет — это клип со своей цепочкой, а не картинка.** План строит
   для него ПОЛНЫЙ `LayerSrc`: свой источник, свой `cache_idx`, своя
   `Cow<[FxInstance]>` с уже разрешённой автоматизацией, возможно свой
   `blend`. Это рекурсия внутри `build_plan`
   (`src/libs/kt-engine/src/video.rs`, стр. 538), и её надо оборвать:
   матт у матта — либо запрет в модели, либо ограничение глубины.
4. **Исключение трафарета из показа.** Технически это `continue` в
   `build_plan`. Но правило 20 чек-листа `src/apps/kt-app/CLAUDE.md`
   («ничего не отрастает и не отрубается само») прямо запрещает молча
   гасить то, что пользователь положил на таймлайн, — а именно на этом в
   AE теряют слой. Значит клип-трафарет остаётся ВИДИМЫМ на таймлайне и
   явно помеченным, а «не рисуется в кадре» читается с его блока, а не
   выясняется методом исключения.
5. **Ключи кэша врут.** `fx_chain_hash` и
   `BurstKind::PostFxVideo { fx_hash }` не знают про матт: два клипа с
   одинаковой цепочкой и разными трафаретами схлопнулись бы в один
   пост-FX кадр. Тот же дефект у `plan_hash` GPU-исполнителя.
6. **GPU.** Четвёртая текстура в бинд-группе, новый `mode` в
   `composite.wgsl` и байт-в-байт зеркало целочисленной формулы. Правило
   `src/libs/kt-fx/CLAUDE.md` — скаляр, AVX2-ядро и WGSL правятся ОДНИМ
   коммитом, SIMD байт-в-байт со скаляром.
7. **Формат.** Новое поле → `KT_META_VERSION`
   (`src/libs/kt-otio/src/meta.rs`, стр. 17) поднимается с 4 до 5:
   старый ридер потерял бы ссылку молча, и проект открылся бы «просто без
   вскрытия». Ломающая смена формата у AE 23.0 — предупреждение, а не
   образец: у нас версия конверта уже есть и умеет ругаться (п.243).
8. **Голод.** Трафарет — второй источник на кадр, то есть вторая причина
   для `complete = false` в `execute_plan_cpu`
   (`src/libs/kt-engine/src/video.rs`, стр. 624). Политику выбирают ОДИН
   раз и одинаково для CPU и GPU: «трафарета нет → носитель виден
   целиком» и «трафарета нет → носителя не видно» дают разную картинку на
   холодном кэше, и разойдись исполнители — превью перестанет совпадать
   с экспортом.

### Какие инварианты задевает

- **№7 «код композиции ОДИН» — центральный.** Инвариант перечисляет
  поимённо, что считается единым кодом `VideoRenderer::frame_plan`: слои,
  порядок Vegas, веса переходов/фейдов, УРОВНИ и маппинг «тик → кадр
  источника». Матт — попиксельный уровень, то есть та же сущность, что
  `alpha_k`. Значит «кто трафарет, какой у него кадр, какой вид и
  инверсия» разрешается в плане, а исполнители получают готовое — как это
  уже сделано для `LayerBlend`. Отдельного «матта в шейдере» быть не может
  по определению инварианта.
- **№4 «кэш — единственная правда о coverage»** — матт входит в ключ
  пост-FX кэша, а его пре-FX кадр — обычная запись `BurstKind::Video`,
  которая обязана попадать в полоски прогретости наравне с носителем.
- **№5 «декодером владеет один MediaWorker»** — второй источник на кадр
  идёт через ту же приоритетную очередь; префетч обязан греть и трафарет,
  иначе «нет данных к дедлайну → последний кадр» будет срабатывать вдвое
  чаще и незаметно (кадр-то есть, просто без вскрытия).
- **№11 «детерминизм»** — CPU байт-в-байт против GOLDEN_VIDEO, и в первую
  очередь при ОТСУТСТВИИ матта (эталоны не перегенерируются); GPU против
  CPU — только через `kt_gpu::assert_frames_close`.
- **№8** — тумбнейл клипа-трафарета берётся из sidecar-индекса как у всех,
  никакого «нарисуем трафарет по-настоящему на таймлайне».
- Правила UI `src/apps/kt-app/CLAUDE.md`: **20** (трафарет не гаснет и не
  прячется сам — он помечен), **13** (ссылка живёт в уже существующем
  инспекторе клипа, а не новой панелью), **17** (пикер и жест «клип
  сверху» ведут себя одинаково и дают одну undo-запись).

## Оценка

| Что | Ответ |
|---|---|
| Объём | L — целый раунд. Дешевле, чем кажется снаружи: композит уже straight-alpha, умножение альфы уже написано скаляром/AVX2/WGSL, целочисленная luma уже заморожена, семантика перевязки ссылок при копипасте уже есть (п.242). Дороже, чем «ещё один эффект»: матт обязан родиться в плане, доехать до GPU четвёртой текстурой и попасть в ключи кэша и в формат |
| Где основная работа | kt-engine (`video.rs`/`video_plan.rs`: `LayerSrc` трафарета, исключение из стека, политика голода, ключ), kt-gpu (четвёртая текстура + режим в `composite.wgsl` + parity), kt-model (ссылка `ClipId`, вид, инверсия; поведение при удалении/сплите/замене источника и при копипасте), kt-cache (ключ), kt-otio (v5), kt-app (пикер + жест + метка на клипе-трафарете). Отдельно и НЕОБЯЗАТЕЛЬНО — kt-media: снять `has_alpha: false`, если нужен Alpha matte |
| Формат проекта | нужна v5 (`KT_META_VERSION` 4 → 5); поле не сериализуется при `None` — старые файлы байт-стабильны |
| Тесты, которые придётся написать | round-trip модели и чтение файлов без поля; luma-матт против CPU-эталона; «трафарета нет → GOLDEN_VIDEO не двигается»; конформанс CPU↔GPU по образцу t181/t182 через `kt_gpu::assert_frames_close`; превью == экспорт на одном бэкенде (образец t67/t184); различение трафаретов в ключе пост-FX кэша (образец t66); трафарет удалён / сплитнут / заменён заглушкой (`placeholder.rs`, образец t209/t211); копипаст пары «носитель + трафарет» перевязывает ссылку, копипаст одного носителя — нет (образец t205 и теста 249); трафарет под морфером и bake (t35/t36); трафарет на реверснутом клипе; layout-тест метки и пикера в `apps/kt-app/src/layout_tests.rs` |
| Зависит от | ничего — для luma-матта из видео. Alpha matte зависит от «альфы из файла» (правка `kt-media/src/ffmpeg.rs` + конформанс-тест на файле с альфой). Трафарет из НАРИСОВАННОЙ формы — от F-MSK-001. Титры как трафарет — от категории `titles/` |
| Ломает совместимость | нет |

## Вердикт

Берём — и берём **luma-матт по ЯВНОЙ ссылке на клип (`ClipId`)**, а не
модель «дорожка строго сверху». Обоснование не вкусовое: (1) наша схема
слоёв — это lane плюс законные перекрытия с правилом «сверху тот, кто
начался позже», и в ней «строго сверху» не выражается без того, чтобы
пара «носитель + трафарет» разъезжалась от обычного move; (2) ссылку по
номеру строки мы уже отвергли один раз, в `MarkedArea.lanes`, и повторять
ошибку Premiere («Matte: Video 3») незачем; (3) явная ссылка бесплатно
даёт то, ради чего AE ломала формат в 23.0, — один трафарет на несколько
носителей, и у нас к ней уже написана недостающая AE половина: перевязка
при копипасте (п.242).

Из ошибок AE берём урок целиком: явная ссылка обязана идти В ПАРЕ с
быстрым жестом «трафаретом служит клип сверху» (правило 17), иначе
повторим «ruined the workflow for 90% of the rest of the shots» и будем,
как Adobe, возвращать команду через полгода.

Alpha-матт откладываем осознанно и с записанной причиной: у нас нет
источника альфы из файла (`has_alpha: false` в
`src/libs/kt-media/src/ffmpeg.rs`), поэтому «Alpha matte» сегодня был бы
пунктом меню, работающим только на выходе хромакея. Это отдельная
небольшая работа (декодер + конформанс-тест), и она должна быть сделана
ДО того, как в UI появится слово «Alpha».

Трафарет у нас не гаснет молча (правило 20): он помечается на таймлайне
и остаётся видимым блоком. Это прямое расхождение с AE — и именно оно
снимает самую массовую жалобу по этой фиче.

## Источники

- https://community.adobe.com/t5/after-effects-beta-discussions/feature-focus-selectable-track-matte-layers-have-arrived/td-p/13110013 — **первоисточник по AE**: Catie Bass (Adobe), 02.08.2022 — анонс Selectable Track Matte Layers для 23.0 beta («Any layer in your comp, no matter where it sits in the layer stack, can be used as a Track Matte source for any other layer… You can even have multiple layers all reference a single layer for their matte»), предупреждение о ломающемся формате («Projects saved in the 23.0 beta will not be able to be opened in other versions of After Effects») и о выражениях, считающих по индексам слоёв; там же — «this feature has shipped to After Effects 23.0» (18.10.2022), меню-команды в 23.3 build 9 (26.01.2023), возврат команды «Matte with Layer Above» в 23.4 (14.06.2023), жалоба на неперевязку ссылок при копипасте (31.05.2023). Дата обращения 2026-09-07
- https://community.adobe.com/t5/after-effects-ideas/2023-new-track-matte-feature-needs-some-work/idi-p/13339012 — AE 23.0: «completely ruined the workflow for 90% of the rest of the shots» (11.11.2022; обращение 2026-09-07)
- https://community.adobe.com/t5/after-effects-discussions/track-matte-disapeared/m-p/13291308 — AE 23.0: «Where is Track Matte?» и «It's a bug» через день после релиза (20–21.10.2022; обращение 2026-09-07)
- https://community.adobe.com/questions-529/layer-visability-stays-off-after-removing-track-matte-is-that-normal-ae-behavior-54298 — AE: гашение видимости трафарета как штатное поведение, словами Adobe Community Expert (21.12.2023; обращение 2026-09-07)
- https://www.meganfriesth.com/aefundamentals/mattes — AE, сводка механики: «In previous versions of After Effects, you could only use the layer directly above the affected layer as a matte», «After Effects will automatically turn off the visibility (eyeball icon) of the matte layer» (учебный ресурс; согласуется с первоисточником Adobe выше; 2026-09-07)
- https://creativecow.net/selectable-track-mattes-in-after-effects/ — Andy Ford, 24.10.2022: «you can either pickwhip to the layer or choose it from a dropdown» (2026-09-07)
- https://community.adobe.com/bug-reports-728/track-matte-key-not-working-with-keyframes-in-pp-2026-0-1-1552070 — Premiere 2026.0.1: Track Matte Key не учитывает обрезку клипа-трафарета (02.03.2026; обращение 2026-09-07)
- https://community.adobe.com/questions-729/track-matte-key-does-not-respect-effect-order-1556713 — Premiere 26: трафарет ездит за Position носителя независимо от порядка эффектов; модератор Adobe Rach McIntire — «longstanding quirk… it has never behaved the same way as it does in After Effects», рекомендация перейти на маски Properties panel; Ann Bens — «Have not used Track Matte in a long time» (07.04.2026; обращение 2026-09-07)
- https://community.adobe.com/questions-729/premiere-2026-masking-faqs-links-1555462 — Adobe (kes_akalaonu), 27.03.2026: в Premiere 26 маски переехали из Effect Controls в Object Mask / Vector Masking, «this isn't a bug» (обращение 2026-09-07)
- https://creativecow.net/forums/thread/track-matte-problem-cant-choose-target-video-fro/ — Premiere: выбор трафарета по НОМЕРУ дорожки и невозможность выбрать нужный номер; там же бытовое правило «белый → luma, цветной → alpha» (11.09.2014; обращение 2026-09-07)
- https://creativecow.net/forums/thread/frustrating-error-whit-track-matte-key/ — Premiere: «matte clip has to go on the next track up» и alpha-матт, не работающий без альфы (20.04.2010; обращение 2026-09-07)
- https://creativecow.net/forums/thread/track-matte-premiere-pro/ — Premiere: репутация Track Matte как ненадёжного (10.09.2020; обращение 2026-09-07)
- https://charlescabrera.com/using-the-track-matte-key-effect-in-premiere-pro-2022-all-you-need-to-know/ — Premiere: параметр `Matte` — список ДОРОЖЕК («under matte, I'm going to select video 2»), Matte Alpha против Matte Luma (вторичный источник; 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/8-design/compositingandmasks.htm — VEGAS: «The Compositing Mode button determines how the transparency in a video track is generated», «The parent track is the highest track in a group of tracks and the behavior of the child tracks is determined by the parent track», режим `Multiply (Mask)`, Mask Generator (справка Vegas 22; страница той же темы на CDN Boris FX для 2026 вернула 403, поэтому «в 2026 механика та же» — [НЕ ПРОВЕРЕНО] по справке 2026, но подтверждается живым тредом ниже; 2026-09-07)
- https://forum.borisfx.com/t/22023 — VEGAS Pro, 07.04.2026: пользователь, пришедший из AE, и рабочий рецепт «Multiply (Mask) + Mask Generator + Parent/Child» (обращение 2026-09-07)
- http://vegaspro.helpmax.net/en/using-vegas-software/applying-effects/compositing-video/compositing-and-masks/ — VEGAS: «To reproduce the 4.0 parent/child masking behavior, set the compositing mode of the parent track to Multiply, and then apply the Mask Generator» (зеркало старой справки; 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/types-of-blend-modes-ver007271c06/mac — FCP: Stencil Luma «Does the same thing as the Stencil Alpha blend mode, but uses the affected clip's luma value to define transparency. Stencil Luma is useful if the clip you want to use for cropping has no alpha channel of its own» (страница длинная, WebFetch отдаёт её усечённой — определение получено из индексированного сниппета той же страницы; 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/intro-to-masking-verce5464e1a/mac — FCP: инструментарий масок (Shape/Draw/Graduated/Image/Magnetic/Scene Removal/Vignette/Auto Mask); Image Mask берёт канал из ОТДЕЛЬНОГО изображения, а не из клипа соседней дорожки; ни track matte, ни Set Matte в FCP нет (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/use-alpha-channels-ver52cf382e/mac — FCP: альфа-канал как способ композита; про straight/premultiplied в руководстве не сказано (2026-09-07)
- https://larryjordan.com/articles/travel-mattes-blurring-a-face-to-protect-the-innocent/ — travel matte требует трёх слоёв, и почему берут luma (15.05.2011; обращение 2026-09-07)
- https://mixinglight.com/color-grading-tutorials/external-mattes/ и https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolveManual_files/part494.htm — Resolve, External Matte: матты цепляются к клипу в медиапуле и становятся входами узла на Color-странице (тексты получены из индексированных сниппетов: прямая загрузка зеркала руководства вернула 404; 2026-09-07)
- https://ae-plugins.docsforadobe.dev/print_page/ — AE SDK: «Effects that use layers as an input, such as Set Matte and Displacement Map, can now target the input layer's masks and effects, instead of only the source of the layer» (версия, в которой это появилось, [НЕ ПРОВЕРЕНО]; 2026-09-07)
- https://www.freevisuals.net/post/after-effects-set-matte — AE Set Matte: «Take Matte From Layer», выбор канала, «Invert Matte», «Premultiplied»; отличие от Track Matte — «without requiring that source to sit in any specific position relative to each individual layer it is controlling» (вторичный источник; 2026-09-07)
- `docs/porting-notes.md` п.242 (копипаст возит морферы, области и метки;
  тесты 249/250) и п.243 (явные ошибки конверта `metadata.kt`)
- Код (прочитан, не пересказан по аналогии): `src/libs/kt-engine/src/video.rs`,
  `src/libs/kt-engine/src/video_plan.rs`, `src/libs/kt-engine/src/video_simd.rs`,
  `src/libs/kt-gpu/src/shaders/composite.wgsl`, `src/libs/kt-gpu/src/engine.rs`,
  `src/libs/kt-fx/src/video.rs`, `src/libs/kt-model/src/project.rs`,
  `src/libs/kt-media/src/lib.rs`, `src/libs/kt-media/src/ffmpeg.rs`,
  `src/libs/kt-media/src/placeholder.rs`, `src/libs/kt-ffmpeg/src/source_impl.rs`,
  `src/libs/kt-cache/src/lib.rs`, `src/libs/kt-otio/src/meta.rs`

Живьём ни один из пяти редакторов не запускался. `helpx.adobe.com`,
`reddit.com` и `forum.blackmagicdesign.com` из этого окружения не
открылись (403/недоступны), поэтому строки AE и Premiere опираются на
официальное сообщество Adobe (включая посты сотрудников Adobe — это
первоисточник по факту изменения в 23.0), Creative COW и вторичные
разборы; строка Vegas — на справку 22 плюс живой тред на официальном
форуме Boris FX; строка Resolve — на зеркала руководства и сниппеты
выдачи, и наличие композит-модов `Alpha`/`Inverted Alpha` на Edit-странице
Resolve 17+ помечено `[НЕ ПРОВЕРЕНО]`. Утверждения «у нас есть / у нас
нет» проверены грепом по `src/`.
