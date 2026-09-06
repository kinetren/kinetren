---
id: F-MPH-003
title: Adjustment Clip: эффект поверх композита
status: кандидат
priority: B
cost: L
category: morpher/analogues
editors: [resolve-20, vegas-pro-2026, premiere-26, fcp-12, after-effects-26]
we_have: частично
crates: [kt-model, kt-engine, kt-gpu, kt-app]
invariants: [4, 7, 11]
ui_rules: [13, 17]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MPH-003 — Adjustment Clip: эффект поверх композита

## Суть

Пустой клип из библиотеки эффектов, который кладут на верхнюю дорожку. Всё,
что лежит под ним на дорожках ниже в его окне, склеивается в одну картинку, и
уже к ней применяются его эффекты, грейд и трансформ. Монтажёру он выглядит
обычным клипом: у него есть длина и ручки, прозрачность, режим наложения, имя,
его можно утащить в бин и достать оттуда снова. Границы действия — это его
собственные границы на таймлайне: продлил ручкой — накрыл ещё две склейки.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| DaVinci Resolve | 20.3 | Adjustment Clip | клип из Effects Library → Toolbox → Effects; на него ложатся ResolveFX/OFX, Inspector (Composite, Transform, Cropping, Dynamic Zoom), Fusion-комп и грейд с sizing'ом Color-страницы |
| VEGAS Pro | 2026 | Adjustment Event / Adjustment Track | событие или целая дорожка; область действия по умолчанию «всё ниже», но сужается до компоновочных детей дорожки; Level-ползунок дорожки правит СИЛУ эффекта на всём, что она накрывает |
| Adobe Premiere | 26 | Adjustment Layer | синтетический элемент из бина проекта; эффекты действуют вниз, а фиксированный Motion — нет: масштаб и поворот композита делают эффектом Transform из Distort |
| Final Cut Pro | 12 | Adjustment Clip | нативный только с 11.1 (Edit → Add Adjustment Clip, Option-A); до этого — самодельный титр-пустышка, собранный в Motion |
| After Effects | 26 | Adjustment Layer | галка у слоя; действует на композит слоёв НИЖЕ в этой же композиции, а маска самого слоя ограничивает ОБЛАСТЬ действия эффекта |

## Фишка фишки

**1. «Применить ко многому» в Resolve — это два семейства, дающие разный
результат, и adjustment clip в одном из них одинок.** Всё, что живёт на
Color-странице (Group Pre-Clip → Clip с node stack layers → Group Post-Clip →
Timeline, плюс shared nodes), выполняется ПО РАЗУ НА КАЖДЫЙ КЛИП и ДО композита:
руководство про верхний из этих уровней говорит буквально «Node trees assembled
in this mode affect every single clip in the entire Timeline». Adjustment clip —
единственный механизм, который выполняется ОДИН раз по склеенной картинке.
Отсюда практическое: виньетка «на весь фильм» через Timeline-грейд на
картинке-в-картинке даёт ДВЕ виньетки, а на adjustment-клипе — одну; и наоборот,
привод исходников в рабочее пространство (CST) корректен только на per-clip
уровне, потому что на композите источники уже неразличимы. Порядок уровней при
этом не совсем жёсткий: галка проекта «Post Group Node Stack Layer» переносит
последний слой node stack ЗА Group Post-Clip.

**2. Opacity у adjustment-клипа — не «сила эффекта», а композитинг его выхода
поверх того же самого композита снизу**, то есть wet/dry-микс — и он сломан:
ПУСТОЙ adjustment clip на 50 % заметно затемняет картинку, а к 0 % яркость
возвращается. Числа сходятся с моделью «альфа применена дважды»
(a·a + (1−a) = a²−a+1: минимум 0.75 ровно на 50 %). Настоящий wet/dry в Resolve
живёт этажом ниже, у ноды: Key Output Gain, который «reduces the strength of
that node's contribution» при смешении через Layer/Parallel Mixer.

**3. Adjustment clip — не тип слоя в конвейере, а клип, у которого источник =
«композит снизу».** Из этого следует всё остальное, и хорошее и плохое: он
попадает в список клипов Color-страницы и КРАДЁТ выделение при каждом переходе
на неё (лечится Alt-кликом по номеру дорожки в мини-таймлайне — номер краснеет,
клип пропадает из списка, а эффект остаётся); несколько adjustment-клипов можно
собрать в группу и грейдить группой сразу все несмежные куски таймлайна; его
Inspector-Transform двигает весь композит целиком (в Premiere ровно это не
работает); и кэшируется он не там, где грейды, а в Sequence Cache — отдельном
кэше «таймлайновых» эффектов (переходы, прозрачность, режимы наложения),
тогда как грейды и OFX сидят в Node Cache.

## Плюсы

- Эффект считается РОВНО ОДИН раз на выходной кадр, а не по разу на слой:
  свечение, зерно и виньетка не удваиваются ни на кроссфейде, ни на
  картинке-в-картинке.
- Область действия — это длина объекта на таймлайне: ручка вправо накрывает ещё
  две склейки, и ни один клип при этом не правится (и не копится undo по клипам).
- Так как это клип, на него бесплатно работает вся клиповая машинерия: грейд,
  Fusion-комп, группы, стили из галереи, Paste Attributes, рендер-кэш, перенос
  в бин как медиа и повторное использование.
- Inspector-Transform / Cropping / Dynamic Zoom действуют на склеенный кадр,
  поэтому «наехать камерой на всю сцену» — это один объект, а не N одинаковых
  трансформов по слоям.
- Выключается поштучно или целой дорожкой, не теряя настроек.

## Минусы

- Вход — уже склеенная картинка, источники в ней неразличимы: нормализация по
  камере, баланс и подгон планов друг к другу на этом уровне невозможны в
  принципе, для них нужен другой механизм (shared node / группа).
- Область действия жёсткая — «всё, что ниже»: на переходе он накрывает обе
  стороны сразу, а «только ближайшая дорожка вниз» не выражается вовсе;
  единственный обходной путь — заворачивать нижние слои в compound clip.
- Он клип — значит, он в списке клипов Color-страницы: ворует выделение,
  участвует в нумерации, требует отдельного трюка, чтобы его оттуда убрать.
- Ползунок прозрачности не эквивалентен силе эффекта и на пустом клипе портит
  картинку; «правильный» wet/dry находится на другом уровне (Key Output Gain) и
  требует собрать в ноде Layer/Parallel Mixer.
- Сам adjustment clip есть и в бесплатной версии, но бо́льшая часть того, что на
  него обычно кладут — шумодав, Magic Mask, плёночное зерно, оптический блюр и
  ещё десятки ResolveFX, — только в Studio.

## Что говорят пользователи

> It's the transitions that give me trouble with Adjustment clips, because the
> "adjustment" affects everything underneath it, including the incoming clip.
> — kinvermark (Mark Wilson), 23.12.2019,
> https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=104651

> In other applicatons like Photoshop etc. there's a checkbox somewhere that
> sets the authority of the adjustment layer to all layers below it or just to
> the next layer underneath the adjustment layer.
> — Hardy Heinlin, 21.09.2024,
> https://forum.blackmagicdesign.com/viewtopic.php?t=208739

> From 100% to 50% the opacity/brightness of the clips underneath decreases
> gradually, from 50% and bellow it starts to increase.
> — panos_mts (Panagiotis Vellios), 22.12.2019,
> https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=104651

Там же, следующим сообщением: «I can confirm this behaviour. It looks like a bug
to me.» (Peter Cave, 23.12.2019). Тред 2019 года, свежих подтверждений для 20.x
живьём не нашлось — считать это актуальным дефектом нельзя, но модель поведения
ползунка («фейд между "с эффектом" и "без"») в треде описана верно и совпадает с
тем, что у Vegas сделано ползунком Level.

Отдельный сюжет — то, что adjustment clip перехватывает выделение на
Color-странице; тред тянется с 2019 по 2022, и решение там нашли не
разработчики, а пользователи:
https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=94192

## Как ложится на Kinotrain

### Что уже есть

- Морфер-слой: `MorpherEvent` в `src/libs/kt-model/src/project.rs`, цели
  `MorphTargets` = `Below` | `Clips` | `Marks` | `Areas`, патч = подмена `media`,
  сдвиг `source_in`, цепочки `video_fx`/`audio_fx`, побитовые `*_bypass`.
  Применяется в `Project::effective_clips()` (там же) — по комментарию в коде
  «ЕДИНСТВЕННАЯ точка выдачи клипов движку, экспорту и отрисовке», накопление
  снизу вверх, порядок детерминирован по `MorphId`.
- **Отсюда главный вывод разведки: наш морфер по устройству — это не adjustment
  clip, а Resolve'овские shared node / Group Post-Clip.** Он правит КЛИПЫ до
  композита, поэтому один морфер с эффектом на трёх слоях = три исполнения
  эффекта, каждое по своему слою. Композитного уровня у нас нет вообще.
- Композит: `FramePlan` в `src/libs/kt-engine/src/video_plan.rs` — слои
  back-to-front, у каждого своя `fx`, свой `fx_hash` и `alpha_k`; стадии «после
  склейки» в плане нет ни одной, `PlanLayer` знает только `One` и `Dissolve`.
- Прозрачность есть у клипа (`ClipEvent::opacity`, `src/libs/kt-model/src/
  project.rs`, в план уходит как `alpha_k` в `src/libs/kt-engine/src/video.rs`),
  а у `MorpherEvent` нет ни ползунка силы, ни режима наложения — только
  бинарный `enabled` и бинарные bypass'ы слотов. Это ровно претензия п.245
  `docs/porting-notes.md` («ползунком управлять прозрачностью наложения»).
- Кэши: `BurstKind::PostFxVideo { quality, fx_hash }` в
  `src/libs/kt-cache/src/lib.rs` — ключ ПО СЛОЮ; `ReadbackCache` в
  `src/libs/kt-gpu/src/pool.rs` — уже по хэшу целого плана, то есть «по
  композиту». Разделение Node Cache / Sequence Cache у Resolve — это про то же
  самое, и у нас половина такого разделения уже есть.
- UI морфера — `src/apps/kt-app/src/morpher_editor.rs`, автоматизация
  параметров — `src/apps/kt-app/src/automation.rs` (домен ключа = тик
  материала, linear/smoothstep).
- Цветокоррекции нет вообще: grep по `src/` на колёса, насыщенность, кривые,
  LUT и scopes даёт только сгенерированные биндинги FFmpeg в `target/`. Поэтому
  сравнение с Resolve неравное — половину того, что там кладут на
  adjustment-клип, у нас просто нечем сделать. Но сам механизм «эффект по
  результату» от набора эффектов не зависит: наши Blur/Glow/Edges/VHS на
  композите вели бы себя ровно так же.

### Чего не хватает

1. Ползунка силы у морфера (wet/dry): «наложить эффект на 40 %». Сегодня
   единственная градация — включён/выключен.
2. Второго уровня применения — стадии «эффект по СКЛЕЕННОМУ кадру». Сейчас
   «виньетка на всю сцену» = N копий эффекта на N слоёв, и на кроссфейде две
   виньетки смешиваются между собой; на adjustment-клипе такого не бывает.

### Что мешает

- `FramePlan` плоский: пост-композитная стадия — это новый узел плана, новый
  вход у ОБОИХ исполнителей (CPU-эталон и WGSL-зеркало) и новый вид ключа кэша,
  считаемый не от `fx_hash` слоя, а от хэша самой стадии.
- Wet/dry на слое тоже не бесплатен: FX считаются пер-слой, результат уходит в
  композит, так что смешивать «до/после» надо ВНУТРИ слоя, а значит держать
  пре-FX кадр рядом с пост-FX — плюс буфер на слой на обоих бэкендах.
- Целеуказание разъедется. У нас `targets` богаче, чем у Resolve
  (`Marks`/`Areas` — именованные диапазоны и области «время × дорожки»), а
  композитная стадия по устройству умеет только «всё, что ниже». Если повесить
  её галкой на тот же `MorpherEvent`, у одного объекта окажутся два разных
  смысла с разной ценой — правило 17 (парные способы ведут себя одинаково)
  требует либо одинакового поведения, либо разных объектов с разными именами.

### Какие инварианты задевает

- **7** (код композиции один): пост-композитная стадия обязана жить в
  `frame_plan` и исполняться одинаково на CPU и GPU, иначе превью разойдётся с
  экспортом.
- **4** (кэш — единственная правда о coverage): новая стадия требует своего вида
  ключа, иначе полоски прогретости соврут.
- **11** (детерминизм): CPU-эталон байт-в-байт плюс кросс-бэкенд по допускам
  через `kt_gpu::assert_frames_close`.
- Правила UI **13** (ползунок обязан встать на уже занятое место в полосе
  цепочки морфера, а не завести новую строку) и **17**.

## Оценка

| Что | Ответ |
|---|---|
| Объём | L — но делится: ползунок силы отдельно S, пост-композитная стадия L |
| Где основная работа | kt-engine (`video_plan`/`video`), kt-gpu (шейдер + пул), kt-model (поля), kt-app (виджет в полосе цепочки) |
| Формат проекта | ползунок — новое поле `MorpherEvent` со `skip_serializing_if`, старые файлы байт-стабильны; композитная стадия — новая версия схемы |
| Тесты, которые придётся написать | round-trip модели и парити операций; конформанс CPU↔GPU новой стадии по образцу t181/t182; превью == экспорт (t67/t184); харнесс-тест ползунка в окне морфера |
| Зависит от | ничего (цветокоррекции нет, но механизм от неё не зависит) |
| Ломает совместимость | нет |

## Вердикт

Ползунок силы у морфера берём: это прямая просьба пользователя (п.245
`docs/porting-notes.md`), у Vegas он есть, а у Resolve на клиповом уровне он
сломан — значит, копировать надо не Opacity, а Key Output Gain, то есть честный
wet/dry внутри слоя.

Пост-композитную стадию («морфер, который правит РЕЗУЛЬТАТ, а не клипы») в этот
заход не берём, но фиксируем как отдельную дыру: у Resolve таких уровней два, у
нас один, и наш — это shared node, а не adjustment clip. Стадия задевает
инвариант 7 и оба бэкенда сразу, поэтому ей нужен свой раунд, а не галка в
морфере.

## Источники

- DaVinci Resolve 20 Reference Manual (PDF), главы 47 «Editing, Adding, and
  Copying Effects and Filters» (Adjustment Clips), 142 «Grade Management»
  (Using Adjustment Clips, Using Shared Nodes, группы Pre/Post-Clip), 143 «Node
  Editing Basics» (Clip vs. Timeline Grading, Node Stack Layers, Post Group
  Node Stack Layer), 145 (Key Output Gain), 8 (Node Cache и Sequence Cache):
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf
  (дата обращения 2026-09-07)
- https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=104651 — «Adjustment
  clip opacity», поведение ползунка и переходы (2026-09-07)
- https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=94192 — «Adjustment
  clip problem in Color page», кража выделения и Alt-клик по номеру дорожки
  (2026-09-07)
- https://forum.blackmagicdesign.com/viewtopic.php?t=208739 — «Want adjustment
  clip to affect next layer underneath only» (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/adjustmentevents.htm
  — Adjustment events в VEGAS, ограничение области компоновочными детьми
  (2026-09-07)
- https://larryjordan.com/articles/create-an-adjustment-layer-for-final-cut-pro-11-using-motion-5-9/
  — нативный Adjustment Clip в FCP появился в 11.1 (2026-09-07)
- https://community.adobe.com/t5/premiere-pro/adjustment-layers-can-scale-be-used/m-p/9164885
  — в Premiere Motion у adjustment-слоя не действует на нижние клипы
  (2026-09-07)
- https://helpx.adobe.com/hk_en/after-effects/using/mask-reference.html — маска
  adjustment-слоя как ограничитель области эффекта в AE (2026-09-07)

По Resolve заметка собрана из официального руководства 20.x и форума
Blackmagic; живьём в программе не проверялась.
