---
id: F-CLR-006
title: Нодовый граф против линейной цепочки эффектов
status: отвергнуто
priority: C
cost: XL
category: color/workflow
editors: [resolve-20, premiere-26, fcp-12, vegas-pro-2026, after-effects-26]
we_have: нет
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-cache, kt-otio, kt-app]
invariants: [4, 7, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-MPH-005, F-MPH-006]
checked: 2026-09-07
---

# F-CLR-006 — Нодовый граф против линейной цепочки эффектов

## Суть

Развилка в том, ЧЕМ описывается обработка клипа. В цепочке (Premiere, VEGAS,
FCP, мы) это упорядоченный список: каждый эффект берёт результат предыдущего,
порядок правится перетаскиванием, ветвиться нечему. В графе (Resolve, Fusion,
Nuke) это схема из узлов и проводов: у узла есть вход картинки, выход картинки
и ОТДЕЛЬНЫЙ вход/выход ключа, узлы можно ставить параллельно, смешивать их
результаты режимами наложения и тянуть матте из одного места схемы в другое.
Монтажёру разница видна не в списке возможностей, а в том, какие задачи
формулируются в один жест, а какие не формулируются вообще.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| DaVinci Resolve | 20.3 | Node Editor (страница Color); отдельно Fusion page | ориентированный граф Corrector-нод: RGB-вход/выход **плюс отдельные вход и выход ключа**; готовые структуры Serial / Parallel Mixer / Layer Mixer / Outside / Key Mixer / Splitter-Combiner / Shared; поверх графа — ФИКСИРОВАННЫЙ стек: pre-clip, L1…L4, post-clip, Timeline |
| Adobe Premiere | 26.x | Effect Controls | линейный стек эффектов на клипе, порядок — перетаскиванием; несколько экземпляров Lumetri на одном клипе; маска живёт ВНУТРИ эффекта, «одна маска на один эффект»; матте от другого элемента — только Track Matte Key по чужой дорожке |
| Final Cut Pro | 12.3 | Video inspector + Color inspector | графа нет, зато ДВА списка на один клип: коррекции выбираются в Color inspector и одновременно стоят строками в Effects списка Video inspector; порядок — перетаскиванием, Balance/Match прибиты к верху, встроенные эффекты не переставляются |
| VEGAS Pro | 2026.0 | Plug-In Chain | линейная цепочка (до 32 плагинов на событие) на четырёх уровнях применения — F-MPH-005; ветвлений нет ни на одном уровне |
| After Effects | 26.x | стек эффектов на слое | графа нет, но параметр типа «слой» (Set Matte, Mask Reference в Compositing Options — см. F-MPH-002), track matte и прекомпы дают ссылки между слоями: связи есть, редактора связей нет |

## Фишка фишки

**1. Матте в графе — это ПОЛНОЦЕННЫЙ СИГНАЛ, а не параметр внутри эффекта.**
У каждой Corrector-ноды Resolve вход и выход ключа отдельны от картинки:
«Each node’s key input and key output makes it possible to route key channel
data from one node to another» (Reference Manual 20, гл. 146, «Introduction to
Manipulating and Combining Keys»). Отсюда трюк, который в цепочке не
формулируется вообще: поставить ноду, у которой RGB-вход
взят от САМОГО НАЧАЛА дерева, картинку она никуда не отдаёт, а её выход ключа
воткнут в ноду в конце — и ключ снимается с чистого кадра, «regardless of
what’s happening to the picture in between» (гл. 146, «Feeding Keys From One
Node to Another»). Мануал прямо объясняет, почему это работает: «Keep in mind
that a key is just a grayscale image» (гл. 146, «Connecting Key Outputs to RGB
Inputs, and Vice Versa») — значит ключ можно пустить в RGB-вход другой ноды,
обработать его кривыми, блюром или шумодавом, и вернуть в ключевой вход
третьей. Key Mixer
складывает, вычитает и пересекает ключи от разных нод. В цепочке маска — поле
внутри эффекта: в Premiere 26 «You can only apply one mask to one effect», и
поделиться одной маской между двумя эффектами нельзя ничем, кроме копии,
которая дальше живёт своей жизнью.

**2. Outside-нода — не «вторая маска», а ОДНА ГАЛКА на чужом ключе.** Мануал
снимает с неё весь ореол: «Outside nodes are really just Corrector nodes with
the Key Palette’s Key Input Invert control enabled» (гл. 146, «Outside
Nodes»). Из этого следуют две вещи, которых в цепочке нет. Первая: обратная
область не требует второй маски и не может с ней разъехаться — правишь окно в
исходной ноде, обе коррекции переезжают. Вторая: если галку инверсии СНЯТЬ,
получится вторая коррекция ТОЙ ЖЕ области — «If, instead of using the Outside
node to invert the incoming key, you want to copy the existing key in order to
perform another operation to the same isolated region, you can disable the key
input’s Invert control» (там же). То есть «внутри/снаружи» — это состояние
одного сигнала, а не два объекта, которые монтажёр обязан держать синхронными
руками.

**3. Ползунок силы у ветки живёт в палитре КЛЮЧА, а не эффекта.** Key Output
Gain: «lowering this value reduces the strength of that node’s contribution to
the Layer Mixer» (гл. 145, «Adjusting Layer Node Strength Using Key Output
Gain»), по умолчанию 1.00, и тот же параметр работает у Parallel Mixer.
Практическое следствие видно только у тех, кто работал: ноду
превращают в Shared (правка расходится по всем клипам, где она стоит), а рядом
вешают ПУСТУЮ layer-ноду, и её output gain становится персональной
непрозрачностью общего лука на каждом кадре отдельно. Общий объект и
индивидуальная сила наложения — это ровно то, чего нет у нашего морфера
(F-MPH-006), и в графе оно получается не отдельной фичей, а побочным эффектом
того, что вес ветки — свойство её ключевого канала.

## Плюсы

- Parallel Mixer подаёт всем веткам ОДНО И ТО ЖЕ состояние картинки: «This
  makes it easy to apply multiple secondary operations without worrying about
  whether or not a change to one will affect the keys of the others»
  (гл. 145, «Parallel Node Structures»). В цепочке вторая вторичка кеит уже
  испорченный первой кадр, и правка первой ломает выделение второй.
- Layer Mixer даёт режимы наложения МЕЖДУ ВЕТКАМИ ОДНОГО КЛИПА: тот же список
  композит-режимов, что у клипов на таймлайне (гл. 145, «Using Composite
  Modes With the Layer Mixer»). Свечение делается парой «резкая кривая + блюр»
  через Add внутри клипа, без второй копии клипа на соседней дорожке и без
  второго декода источника.
- Приоритет в Layer Mixer задаётся ПОРЯДКОМ ВХОДОВ, а не порядком операций:
  нижний вход перекрывает верхние (гл. 145, «Layer Mixer Prioritization»),
  поэтому «кожа поверх стилизации» — это перецепить провод, а не переставить
  полцепочки.
- Ключ, снятый один раз, кормит сколько угодно коррекций; правка квалификатора
  расходится по всем потребителям сразу.
- Shared Node: «Changes made to a Shared node are automatically rippled to all
  other instances of that node in the grades of other clips» (гл. 142, «What
  Are Shared Nodes Good For?»), при этом общие и личные ноды перемешиваются в
  любом порядке. Копипаст цепочки такого не умеет в принципе: копии расходятся
  с первой же правкой.
- Нода без выхода картинки — законный объект. Генератор ключа не обязан быть
  чьим-то эффектом, поэтому его можно поставить где угодно и подпитать любым
  состоянием кадра.

## Минусы

- Порядок операций перестаёт читаться. У цепочки «последний в списке —
  последний по счёту»; у графа с двумя мешалками и веткой ключа ответ на
  вопрос «почему картинка такая» требует прочесть схему целиком, и по
  скриншоту двух похожих деревьев результат не предсказывается: у Layer Mixer
  всё решает, В КАКОЙ вход воткнут провод.
- Количество нод стало мемом (см. цитаты): свобода ветвиться превращается в
  деревья на 22–37 узлов, где большая часть — «default & not touched».
- Граф не организует работу сам. Resolve пришлось положить поверх графа
  ЛИНЕЙНЫЙ стек — Node Stack Layers, «applied sequentially (in order from
  lowest to highest)» (гл. 143, «Using Node Stack Layers»), с количеством
  слоёв, задаваемым в настройках ПРОЕКТА. Цена честно записана там же: «If
  for some reason you return to the settings and lower the amount of node
  stack layers later, all the grades in those layers are removed across your
  entire project». То есть флагман нодов
  расставляет реальную работу по фиксированным уровням — ровно как VEGAS
  (F-MPH-005), только уровней шесть и они настраиваемые.
- Порог входа платится не знанием кнопок, а необходимостью развидеть слои:
  новичок раскладывает ноды в ряд, получает «почти то же самое» и годами не
  узнаёт, зачем вообще нужен второй вход.
- Мемоизация дорожает: у цепочки результат один на клип, у графа — по
  результату на каждый узел, и узел с двумя потребителями (Shared, ветка
  ключа) читается дважды, а хранится один раз.
- Даже у Resolve графов ДВА, с разными наборами узлов и разными правилами
  (Color и Fusion), и переносить схему между ними нельзя.

## Что говорят пользователи

> 22 nodes for what amounts to “decrease exposure, crank blue”. This could be
> done in a single node, or three if you want to be fancy about it.
> — u/ore_wa_kuma, r/colorists, 22.06.2026, https://www.reddit.com/r/colorists/comments/1u9w0j4/dayfornight_grade_practice_looking_for_feedback/ot36nyq/

> Also idk who made this power grade, but 28 nodes is disgusting honestly.
> — u/Namisaur, r/davinciresolve, 03.09.2026, https://www.reddit.com/r/davinciresolve/comments/1w5o1rj/can_anyone_explain_what_im_looking_at/p7hy62d/

> I’m also completely new to Resolve and tutorials using 4–5 nodes are quite
> confusing.
> — u/Kevin_Blackberry93, r/davinciresolve, 26.08.2026, https://www.reddit.com/r/davinciresolve/comments/1vyixyx/struggling_with_dlog_m_color_gradin_is_there_an/p5xrhrh/

> There’s nothing wrong with thinking of nodes as layers, at least to start
> out. If you just use them one after the other, that’s basically how they’re
> functioning.
> — u/Legomoron, r/colorists, 12.07.2026, https://www.reddit.com/r/colorists/comments/1utrv88/need_help_with_colour_matching/owzr44i/

> What I do is having a node that is only connected to input with no output. I
> can make a qualification inside of it and then use the alpha out connected to
> a node in my node tree.
> — u/Adridulte, r/colorists, 21.06.2026, https://www.reddit.com/r/colorists/comments/1ubvd22/secondaries_before_primaries/osznna1/

> If I need to alter the image before it goes to my qualification node, I can
> add a node before and do an adjustment that will only affect the
> qualification and not the rest of the grade.
> — u/Adridulte, там же, 21.06.2026, https://www.reddit.com/r/colorists/comments/1ubvd22/secondaries_before_primaries/osznna1/

> For each shared node in that tree I also add a layer node which is left
> blank. Altering the output gain of this blank layer node becomes an opacity
> control, which can be altered on a shot by shot basis.
> — u/useless_farmoid, r/colorists, 12.05.2026, https://www.reddit.com/r/colorists/comments/1tafmf9/look_dev_questions/olbnuwf/

Читать это надо так. Отрицательный фидбек про граф почти никогда не звучит как
«ноды хуже слоёв» — он звучит как «у тебя слишком много нод». Жалуются не на
механизм, а на свободу, которую механизм даёт; за пять сотен просмотренных
комментариев r/colorists и r/davinciresolve не встретилось ни одного «вернулся
на слои». Зато встречается обратное движение: человек приходит из Photoshop,
раскладывает ноды как слои, и ему отвечают, что так тоже можно — «If you just
use them one after the other, that’s basically how they’re functioning». Это и
есть честная мера полезности графа: 90% реального дерева — линейная цепочка,
а весь выигрыш сидит в нескольких местах, где нужен второй вход.

## Как ложится на Kinotrain

### Что уже есть

У нас цепочка строго линейная, и это записано в четырёх местах сразу.

- Модель: `ClipEvent.fx: Vec<FxInstance>` плюс параллельные ему `fx_bypass`,
  `fx_auto`, мастер-выключатель `fx_off` —
  `src/libs/kt-model/src/project.rs:1522`. Единственный вид цепочки, который
  видит движок, отдаёт `ClipEvent::active_fx()` (там же).
- Морфер дописывает свои слоты В ТУ ЖЕ цепочку: `MorphPatch::apply_to`
  (`project.rs:850`) заканчивается буквально `c.fx.extend(add)` (строка 876),
  а раскладку целей делает `Project::effective_clips()`. Морфер — не второй
  уровень обработки, а генератор хвоста списка.
- Исполнение CPU: `apply_video_chain_pooled` в
  `src/libs/kt-fx/src/video.rs:61` — `for f in fx { frame = … }` с
  единственным живым кадром и возвратом буферов в `FxScratch`.
- Исполнение GPU: `chain_ops` в `src/libs/kt-gpu/src/engine.rs:124`,
  подписанное «Зеркало трансформаций apply_video_chain_pooled» — тот же
  линейный проход, только собирающий `Vec<FxOp>`.
- План кадра: `LayerSrc { fx: Cow<[FxInstance]>, fx_hash, … }` в
  `src/libs/kt-engine/src/video_plan.rs`; FX считаются ПЕР-СЛОЙ, композит
  (`alpha_over` в `src/libs/kt-engine/src/video.rs:651`) идёт после них.
- Кэш: `BurstKind::PostFxVideo { quality, fx_hash }` в
  `src/libs/kt-cache/src/lib.rs:31`; ключ считает `fx_chain_hash`
  (`src/libs/kt-engine/src/video.rs:153`) — xxh3 от `format!("{fx:?}|{auto:?}
  |…")`, то есть от СПИСКА целиком плюс геометрия клипа.
- UI: одна полоса `widgets::chain_strip`
  (`src/apps/kt-app/src/widgets.rs:213`) ровно на два места —
  `fx_editor.rs:203` и `morpher_editor.rs:375`. Перестановка мышью,
  мастер-галка, галки слотов, буфер обмена цепочки `ops::FxChainBlock`
  (`src/libs/kt-model/src/ops.rs:864`), тесты t205/t215/t217.
- Формат: цепочка уезжает в `.otio` УПОРЯДОЧЕННЫМ списком объектов `Effect` с
  `effect_name: "kt:<тег>"` (`src/libs/kt-otio/src/convert.rs:548`) и
  читается обратно по порядку индексов (там же, 1195).
- Матте у нас всё-таки есть — ровно один и намертво зашитый:
  `greenscreen` в `src/libs/kt-fx/src/video.rs:348` пишет альфу в `px[3]`,
  и дальше она уходит в композит собственного слоя. То есть сигнал ключа
  физически существует, но не имеет ни имени, ни адреса: он не может ни
  разветвиться, ни попасть в соседний эффект.
- Двухвходовая композиция существует в системе ровно однажды —
  `PlanLayer::Dissolve { a, b, k, mix }` (`video_plan.rs`), и это переход,
  который пользователь не может распаять руками.

### Чего не хватает

Грепом по `src/` (`mask|matte|rotoscop`) — ни одного попадания по существу:
только маска таймкода в `apps/kt-app/src/util.rs` и `front_mask` внутри
`transitions.rs`. Нет масок, рото и track matte; ни у одного эффекта нет
второго входа — ни картинки, ни ключа; нет параллельных веток, режимов
наложения между ветками одного клипа и силы отдельной ветки; нет разделяемого
экземпляра эффекта (есть только копипаст блока цепочки, а копии расходятся).

### Что мешает

Мешает не объём кода, а то, что «цепочка — это упорядоченный список»
одновременно является:

1. **Ключом кэша.** `fx_chain_hash` хеширует список целиком, и на клип
   приходится ОДИН пост-FX результат. У графа результат есть у каждого узла,
   хеш узла — это хеш всего его поддерева, а узел с двумя потребителями
   (Shared-нода, ветка ключа) хранится один раз, а читается дважды.
   Тождество «слой ↔ fx_hash», на котором стоят t66/t66c, перестаёт
   существовать.
2. **Формой исполнителя.** `apply_video_chain_pooled` владеет ровно одним
   `Frame` и гоняет его по циклу; графу нужен живой набор промежуточных
   картинок и топологический порядок — это интерпретатор со стеком значений,
   а не `for`. И написать его нужно ДВАЖДЫ, байт-в-байт совпадающими: CPU и
   `chain_ops`/wgsl.
3. **Форматом проекта.** В OpenTimelineIO `Item.effects` — по спецификации
   СПИСОК. DAG в него не выражается; он уехал бы целиком в `metadata["kt"]`,
   и любой чужой инструмент увидел бы клип без эффектов или с бессмысленным
   плоским остатком. Идемпотентность и passthrough (t144–t147) и golden
   легаси-импорта (t148) держатся именно на списке.
4. **Одним виджетом цепочки.** `chain_strip` — полоса, одна на все места,
   и это записанный инвариант UI. Граф — это холст с проводами, попаданием
   мышью, авторазметкой и зумом; полоса и холст, описывающие одно и то же, —
   ровно «парные способы сделать одно и то же» из правила 17.

### Какие инварианты задевает

- **№7 (код композиции ОДИН).** Самое дорогое. Появляются два интерпретатора
  графа вместо двух исполнителей списка; любое расхождение в порядке обхода
  или в моменте свёртки ветки — это второй путь, против которого инвариант и
  написан. Проверять придётся не «те же байты на том же плане», а «тот же
  обход», а такого теста у нас нет.
- **№11 (детерминизм).** GOLDEN_VIDEO и «превью == экспорт байт-в-байт»
  переезжают целиком: даже линейная цепочка, исполненная интерпретатором,
  может дать другие байты на границах округления. Правило крейта kt-fx
  («умолчание нового параметра = прежнее поведение байт-в-байт») при смене
  способа исполнения выполнить нечем.
- **№4 (кэш — единственная правда о coverage).** `PostFxVideo` становится
  узловым; узел, разделяемый двумя клипами, ломает нынешнюю привязку ключа к
  `SourceId` слоя.
- **Правило UI 13 («не тратить место впустую»).** Холст графа — это большая
  постоянная площадь ради инструмента, которым в 90% случаев рисуют прямую
  линию.
- **Правило UI 17 («парные способы»).** Полоса цепочки и граф описывают одно и
  то же; оставить оба — расхождение поведения, оставить один — переписать оба
  места, где полоса сейчас работает.
- **Правило UI 20 («ничего не отрастает само»).** Удобство графа в Resolve
  наполовину состоит в том, что «Add Parallel» САМ дорисовывает мешалку и
  провода. У нас такое запрещено прямым текстом.

## Оценка

| Что | Ответ |
|---|---|
| Объём | XL — и не один раунд: переписывается способ исполнения FX в ОБОИХ бэкендах, ключ пост-FX кэша, формат проекта и редактор эффектов |
| Где основная работа | kt-fx + kt-engine (интерпретатор графа вместо цикла), kt-gpu (второй такой же), kt-cache (ключ по узлу, а не по цепочке), kt-otio (DAG вне спецификации OTIO), kt-app (холст вместо полосы) |
| Формат проекта | несовместимо: `Item.effects` — список, DAG уезжает в `metadata.kt` целиком, passthrough теряет смысл; версия v5+ обязательна |
| Тесты, которые придётся написать | заново — весь блок kt-fx/video.rs (video_fx, t68, t228), postfx_cache t66/t66c, golden_export t11/t67, kt-gpu t181/t182/t184, kt-otio t144–t148; плюс новый класс «порядок обхода графа одинаков на CPU и GPU», которого сейчас нет |
| Зависит от | — (F-MPH-005 и F-MPH-006 — соседняя, более дешёвая развилка: уровни и сила вместо проводов) |
| Ломает совместимость | да — чужой инструмент перестаёт читать наши эффекты из `.otio` |

## Вердикт

**Отвергнуто.** Причина не «дорого», а вот эта: у нас «цепочка клипа —
упорядоченный список» работает одновременно ключом пост-FX кэша
(`fx_chain_hash`), формой обоих байт-совпадающих исполнителей (инварианты 7 и
11), списком `Item.effects` в формате проекта и единственным виджетом
`chain_strip`. Граф отменяет это свойство во всех четырёх местах сразу, и
платой за него будет не «новая панель», а новый класс расхождений
CPU↔GPU, для которого у нас нет ни тестов, ни эталонов. При этом три из пяти
редакторов ядра живут без графа, а флагман нодов сам положил поверх графа
линейный стек уровней — значит граф не является условием профессионального
грейда, он является способом организации, и способ этот воспроизводит наши же
уровни (F-MPH-005) более дорогим механизмом.

**Что из графа берём — и берём БЕЗ графа.** Ценность, которую цепочка
действительно теряет, ровно одна: матте как отдельный сигнал. Она снимается
не проводами, а полем: у эффекта появляется ИСТОЧНИК КЛЮЧА (ссылка на другой
слот цепочки, на именованную область или на альфу соседнего слоя) плюс галка
инверсии — и мы получаем и общую маску на два эффекта, и Outside-поведение,
не заводя DAG: картинка остаётся списком, ветвится ТОЛЬКО ключ, а ключ, по
собственным словам мануала Resolve, «is just a grayscale image». Сила
отдельной ветки и разделяемый экземпляр закрываются своими заметками
(F-MPH-006, F-MPH-005), а не холстом.

**Когда заметку можно открыть заново.** Только если у нас появится отдельная
страница композитинга (аналог Fusion) — там граф уместен и не обязан ничего
обещать про грейд на таймлайне. Для страницы грейда вопрос закрыт: сама по
себе «нодовость» не даёт ни одной возможности, которую нельзя выразить полем
у эффекта, и стоит при этом всей нашей системы детерминизма.

## Источники

- DaVinci Resolve 20 Reference Manual (PDF, прочитан целиком локально):
  гл. 142 «Grade Management», разделы Using Shared Nodes / What Are Shared
  Nodes Good For?; гл. 143 «Node Editing Basics», раздел Using Node Stack
  Layers; гл. 145 «Serial, Parallel, and Layer Nodes», разделы Parallel Node
  Structures, Layer Mixer Prioritization, Using Composite Modes With the Layer
  Mixer, Adjusting Layer Node Strength Using Key Output Gain; гл. 146
  «Combining Keys and Using Mattes», разделы Introduction to Manipulating and
  Combining Keys, Outside Nodes, Feeding Keys From One Node to Another,
  Connecting Key Outputs to RGB Inputs and Vice Versa (около с. 3297–3345). https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf (дата обращения 2026-09-07)
- https://www.reddit.com/r/colorists/comments/1ubvd22/secondaries_before_primaries/osznna1/ — нода-генератор ключа без выхода картинки (2026-09-07)
- https://www.reddit.com/r/colorists/comments/1tafmf9/look_dev_questions/olbnuwf/ — Shared-нода + пустая layer-нода как ползунок силы (2026-09-07)
- https://www.reddit.com/r/colorists/comments/1u9w0j4/dayfornight_grade_practice_looking_for_feedback/ot36nyq/ — «22 nodes» (2026-09-07)
- https://www.reddit.com/r/davinciresolve/comments/1w5o1rj/can_anyone_explain_what_im_looking_at/p7hy62d/ — «28 nodes is disgusting» (2026-09-07)
- https://www.reddit.com/r/davinciresolve/comments/1vyixyx/struggling_with_dlog_m_color_gradin_is_there_an/p5xrhrh/ — порог входа новичка (2026-09-07)
- https://www.reddit.com/r/colorists/comments/1utrv88/need_help_with_colour_matching/owzr44i/ и …/owzpe9m/ — «ноды как слои» у пришедшего из Photoshop (2026-09-07)
- https://www.reddit.com/r/davinciresolve/comments/1w8lcrb/affect_colour_adjustment_to_previous_node_only/p83zosx/ — что делает Outside-нода словами пользователя (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/add-color-correction-effects-ver9f5631358/mac — FCP 12.3: несколько коррекций на клип, порядок перетаскиванием в Video inspector, Balance/Match прибиты к верху, и отдельно: «Applying multiple color corrections to a clip has no effect on the color masks in the additional color corrections» — маска второй коррекции строится по ИСХОДНОЙ хроме, то есть тот самый «ключ из начала дерева», но зашитый намертво (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/change-clip-effect-order-ver761c7810/mac — FCP: порядок эффектов перетаскиванием, «You can’t change the order of the built-in effects» (2026-09-07)
- https://larryjordan.com/articles/adobe-updates-and-modifies-masking-in-premiere-26/ — Premiere 26: «You can only apply one mask to one effect», маска создаётся Unassigned и перетаскивается на эффект (2026-09-07)
- https://www.provideocoalition.com/tool-tip-tuesday-for-adobe-premiere-pro-2026-mask-tool-changes-to-know/ — там же про Unassigned-маску и назначение на Opacity или на эффект (2026-09-07)
- https://creativecow.net/forums/thread/any-way-to-apply-event-fx-after-track-fx/ — VEGAS: порядок уровней вместо ветвления (2026-09-07)

Оговорки о доверии. Строка After Effects взята из соседней заметки F-MPH-002
(Mask Reference / Compositing Options), а не из первоисточника: `helpx.adobe.com`
из этого окружения не открывается вовсе (обрыв соединения на любом URL), поиск
Adobe Community отдаёт 403 CloudFront. Reddit прямыми ссылками тоже недоступен —
комментарии читались через архив Arctic Shift, цитаты приведены дословно по
архивным записям, ссылки указывают на исходные пермалинки. Живьём ни в одном
редакторе не проверялось: Resolve — по официальному мануалу 20-й версии, FCP —
по официальной справке 12.3, Premiere — по двум разборам обновления 26.
