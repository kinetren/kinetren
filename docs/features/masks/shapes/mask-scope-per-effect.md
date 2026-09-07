---
id: F-MSK-002
title: Где живёт маска — у слоя, у эффекта или отдельным объектом
status: кандидат
priority: B
cost: L
category: masks/shapes
editors: [premiere-26, after-effects-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: нет
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-cache, kt-otio, kt-app]
invariants: [3, 4, 7, 11]
ui_rules: [1, 8, 13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-MSK-001, F-CLR-006, F-MPH-006]
checked: 2026-09-07
---

# F-MSK-002 — Где живёт маска — у слоя, у эффекта или отдельным объектом

## Суть

Задача монтажёра формулируется в одну строку: на ОДНОМ клипе размыть лицо и
поднять яркость в углу кадра. Дальше всё решает то, чьим свойством является
контур. Если маска принадлежит слою — она режет слой целиком, и для двух разных
областей нужны два слоя. Если маска принадлежит эффекту — оба действия живут
слотами одной цепочки, но общий контур двум эффектам не отдать. Если маска —
отдельный сигнал со своим входом, контур переиспользуется и инвертируется у
потребителя, но у эффекта появляется второй вход, а у программы — редактор
связей. Это не «где кнопка»: развилка определяет схему данных, ключ кэша,
копипаст слота и формат проекта, и переигрывается только переписыванием всего
перечисленного.

Соседняя заметка F-MSK-001 отвечает на вопрос «что такое контур» (скалярные
примитивы берём, произвольный безье — нет) и явным текстом откладывает вопрос
«чей он» сюда. Эта заметка его закрывает.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| Adobe Premiere | 26.x | маски в Effect Controls, ВНУТРИ эффекта | контур — свойство КОНКРЕТНОГО эффекта, «You can only apply one mask to one effect»; нарисованная без выбранного адресата попадает в раздел Unassigned masks и перетаскивается мышью на нужный эффект либо на Opacity (Opacity = маска слоя); у маски галка Inverted, растушёвка, трекинг |
| After Effects | 26.x | маски СЛОЯ + Compositing Options → Mask Reference у эффекта | ГИБРИД: контур живёт на слое и по умолчанию режет слой целиком (его альфу), но у эффекта есть своя группа Compositing Options с Effect Opacity и списком Mask Reference — эффект ССЫЛАЕТСЯ на маску слоя, не владея ею; отдельно Set Matte берёт канал другого слоя (по описаниям пользователей — на выбор до или после его собственных эффектов) |
| DaVinci Resolve | 21 | Power Windows у НОДЫ (Color) + маски-ноды в Fusion | «all of the windows within a particular node work together to limit that node’s grade»; внутри ноды до пяти окон, у каждого галка Mask = складывать или вычитать; обратная область — не вторая маска, а Outside-нода («Corrector nodes with the Key Palette’s Key Input Invert control enabled»); в Fusion маска — ОТДЕЛЬНАЯ нода в синий Effect Mask вход, плюс отдельные входы pre-mask, Garbage и Solid Matte |
| Final Cut Pro | 12.3 | семь мask-эффектов в стеке + «effect masking» внутри эффектов | ТОЖЕ гибрид, но другой: Draw / Shape / Graduated / Image / Vignette / Magnetic / Scene Removal — обычные эффекты клипа («Add them to any clip in the timeline just as you would add any other clip effect»), и такая маска режет ВЕСЬ слой; ограничить одну коррекцию можно только встроенной маской самого эффекта, и режимы Add/Subtract/Intersect есть ТОЛЬКО у встроенных |
| VEGAS Pro | 2026 | Bézier Masking OFX в цепочке плагинов (плюс Cookie Cutter, Mask Generator, маска в Pan/Crop) | ПОЗИЦИОННАЯ модель: плагин-маска стоит в цепочке события; по умолчанию он вырезает дырку в кадре, а галка Mask FX превращает его в ограничитель — «to mask the effects of all plug-in(s) in the effect chain that precede the Bézier mask plug-in»; до пяти масок в экземпляре, Invert и Blend (сила) — общие на экземпляр |

Оговорка по версиям: онлайн-справка VEGAS доступна по 23-ю линейку
включительно, поэтому строка VEGAS проверена по справке VEGAS Pro 23 —
предшественнику линейки 2026 (см. `_editors.md`); расхождений в устройстве
Bézier Masking в release notes 2026 не заявлено, но живьём это
**[НЕ ПРОВЕРЕНО]**.

## Фишка фишки

**1. Маска у эффекта работает ПОСЛЕ эффекта, а не до него — и поэтому
маскировать можно не всё.** Мануал Fusion говорит прямо: «While masks (or
mattes) are connected via an input, they are actually applied “post effect,”
which means the node first applies its effect to the entire image, and then the
mask is used to limit the result by copying over unaffected image data from the
input» (Resolve 21 Reference Manual, гл. 79 «Rotoscoping with Masks»). Из этого
следуют две вещи, которых нет ни в одном списке возможностей. Первая: есть
эффекты, к которым такая маска НЕ применяется, и мануал перечисляет их
поимённо — «notably Savers, Time nodes, and Resize, Scale, and Crop nodes»,
то есть ровно те, у которых вход и выход лежат в разных растрах и «скопировать
незатронутое из входа» нечем. Вторая: у свечения и бликов в Fusion есть ВТОРОЙ,
pre-mask вход — «a pre-mask restricts the effect to certain areas of the image
but allows the result of that effect to extend beyond the limits of the mask», —
и он существует именно потому, что post-effect маска ведёт себя неправильно.
Ту же самую физику пользователь Premiere описывает своими словами: «The problem
with doing things like adding a glow to a masked clip is that the glow is masked
as well, so it doesn’t “glow” beyond the bounds of the mask». Маска у эффекта —
это не «эффект посчитался только здесь», а «эффект посчитался везде, а показали
здесь»: цена кадра не падает, а свечение и размытие обрезаются по контуру.

**2. У позиционной маски (VEGAS, FCP) область — это ПРЕФИКС цепочки, и вторая
маска режет первую.** Справка VEGAS отмечает это отдельной сноской: «Note that
the Bézier Masking OFX masks all of the preceding effects. This applies also
other instances of the Bézier Mask, in other words, if you have more than one
mask, everything outside the latest mask is not affected at all». Разбор их же
примера: «wrong color» → маска 1 → блюр → маска 2 даёт в итоге, что внутри
второй маски размыто И покрашено, а СНАРУЖИ второй маски отменяется в том числе
покраска, ограниченная первой. То есть две маски дают не две независимые
области, а пересечение, и «ограничить второй эффект из пяти» позицией не
формулируется вообще. У FCP та же ловушка приходит с другой стороны: mask-эффект
там режет слой, а не соседний эффект, и пользователь описывает это буквально —
«Draw mask affects the whole layer, not the colour board». Модель «маска
принадлежит владельцу» (Premiere: эффект; Resolve: нода) от этого свободна: пять
эффектов — пять независимых областей, и вопрос «что именно я маскирую» имеет
ответ по построению, а не по порядку слотов.

**3. Инверсия у контура и инверсия у ПОТРЕБИТЕЛЯ — разные механизмы, и это
видно только на второй области.** Галка живёт у маски: Premiere — Inverted
(«if you want the mask to apply to the background (checked) or foreground
(unchecked)»), FCP — «A checkbox that, when selected, reverses the mask—swapping
its solid and transparent areas», VEGAS — Invert в General Options, то есть
сразу на все пять масок экземпляра, Resolve — галка Mask у каждого окна внутри
ноды («whether one window adds to another window, or subtracts from that
window»). Такой инверсией нельзя получить ВТОРУЮ коррекцию по обратной области,
не заведя второй контур: копируешь окно — и дальше два контура живут своей
жизнью и расходятся с первой правки. Resolve решает это тем, что инвертирует не
маску, а ВХОДЯЩИЙ КЛЮЧ у соседа: «Outside nodes are really just Corrector nodes
with the Key Palette’s Key Input Invert control enabled» (гл. 146), и если галку
снять, получится вторая коррекция ТОЙ ЖЕ области. Тот же результат у AE
достигается ссылкой, а не проводом: Mask Reference берёт маску слоя, поэтому
двум эффектам можно дать один контур и править его в одном месте. У Premiere
такого нет вовсе — «You can only apply one mask to one effect», и общий контур
существует только в виде копии; у Resolve на странице Color переиспользование
формально есть, но это «Copy Window / Paste Window» в другую ноду, то есть тоже
копия.

## Плюсы

Здесь и ниже — следствия УСТРОЙСТВА каждой из моделей, а не оценки.

- **Маска у эффекта:** «размыть лицо» и «поднять яркость в углу» — два слота
  ОДНОГО клипа. Ни дубликата клипа, ни adjustment-слоя, ни второго декода
  источника; пользователи AE описывают ровно этот переход — «Usually I’d just
  apply the effect on the entire layer, then duplicate that layer and mask it».
- **Маска у эффекта:** соседний эффект в цепочке маски не видит, поэтому правка
  контура не может испортить чужую настройку — область и её потребитель связаны
  жёстко.
- **Маска у слоя:** контур — это альфа слоя, и потому один механизм сразу даёт
  прозрачность, мягкий край и участие в композите; поэффектная маска альфу слоя
  не трогает и в композит не попадает.
- **Маска у слоя:** одно место правки на весь клип — пять эффектов гарантированно
  сидят в одном контуре и не могут разъехаться.
- **Маска отдельным объектом (нода/ссылка):** контур кормит сколько угодно
  потребителей, правка расходится ко всем сразу, а «внутри/снаружи» — состояние
  одного сигнала (Outside), а не два объекта, которые монтажёр держит
  синхронными руками.
- **Позиционная маска (маска-эффект в цепочке):** ноль новых понятий в схеме
  данных. Маска — обычный слот, поэтому она бесплатно попадает в копипаст
  цепочки, в галку обхода, в пресеты и в формат проекта.

## Минусы

- **Маска у эффекта:** один контур нельзя отдать двум эффектам. Premiere говорит
  это прямым текстом, Resolve предлагает копирование окна в другую ноду — и с
  первой же правки копии расходятся; синхронность становится обязанностью
  человека.
- **Маска у эффекта:** её не видно. Она спрятана внутри свёрнутого слота, и
  цена такой невидимости измеряется годами: механизм в AE существует с 2014 г.,
  а обходные слои-дубликаты люди строили до 2026-го (цитаты ниже).
- **Маска у эффекта:** применима не ко всем эффектам — те, что меняют растр
  (масштаб, кроп, ресемпл), из модели выпадают, потому что «незатронутое» брать
  неоткуда.
- **Маска у слоя:** чтобы ограничить ОДИН эффект, нужен второй объект —
  adjustment-слой, прекомп или дубликат клипа. Это лишний элемент в стопке,
  второй просчёт и лишний шаг в истории; в FCP та же нужда лечится дублированием
  клипа даже для объединения двух масок (Add-режим у mask-эффектов отсутствует,
  «Dragging multiple instances of the Draw Mask or the Shape Mask to the same
  clip creates the equivalent of the Intersect blend mode»).
- **Позиционная маска:** порядок слотов начинает значить две вещи сразу — что за
  чем считается И что чем ограничено; вторая маска пересекается с первой
  накопительно (сноска VEGAS выше), и предсказать картинку по скриншоту цепочки
  нельзя.
- **Маска отдельным объектом-нодой:** у эффекта появляется второй вход, у
  программы — холст со связями. Цена разобрана в F-CLR-006 и там же признана
  неподъёмной для нас.
- **Общее для всех моделей:** маска — это анимируемая ГЕОМЕТРИЯ, а не число.
  Ключи по точкам контура не ложатся ни в один общий редактор кривых, и каждый
  редактор заводит для них отдельный режим: у VEGAS это собственный «Animate
  Curve 1» с Edit Mode, у Premiere — отдельное переключение Clip/Frame, и
  пользователи считают это регрессом: «now the controls and tools are scattered
  in to tree different places».

## Что говорят пользователи

Про невидимость поэффектной маски (AE, механизм с 2014 года):

> I have been using AE since 2003. I just learned about Compositing Options in
> the year of our lord 2026.
> — u/newaccount47, r/AfterEffects, 28.06.2026, https://www.reddit.com/r/AfterEffects/comments/1k6zqqf/did_you_know_this_pro_tip/oua43gb/

Про то, что она заменяет:

> Usually I'd just apply the effect on the entire layer, then duplicate that
> layer and mask it.
> — u/MX010, r/AfterEffects, 18.06.2026, https://www.reddit.com/r/AfterEffects/comments/1u8yfk4/after_20_years_i_finally_figured_out/ose8yp0/

Про два этажа силы (сила эффекта против прозрачности слоя — прямая связь с
F-MPH-006):

> The “Opacity” controller on the Fill effect itself actually makes the layer
> itself transparent, which isn’t really that helpful. But if you use the
> Opacity controller in the Compositing options it changes the opacity of just
> the Fill.
> — u/Stinky_Fartface, r/AfterEffects, 18.06.2026, https://www.reddit.com/r/AfterEffects/comments/1u8yfk4/after_20_years_i_finally_figured_out/osczsom/

Как поэффектная маска выглядит в работе у Premiere 26 (маска сначала ничья,
потом её НАЗНАЧАЮТ владельцу):

> assuming you're using a recentish version, use one of the mask tools, then
> drag that mask from "unassigned masks" down onto your blur effect
> — u/reginaltus, r/premiere, 24.07.2026, https://www.reddit.com/r/premiere/comments/1v5ecar/need_help_with_applying_gaussian_blur/ozj86ju/

И отрицательный отзыв о том же самом — назначение владельца отдельным жестом
раздражает:

> Seems convoluted but maybe there's a deeper reason they did it that way. I
> still default to After Effects for real masking.
> — u/DaleFairdale, r/premiere, 18.08.2026, https://www.reddit.com/r/premiere/comments/1vrodzr/do_you_prefer_the_new_masking_method_in_premiere/p4fjb0p/

Про «маска режет весь слой, а не мой эффект» — FCP, где mask-эффект стоит в
стеке рядом с коррекцией и всё равно её не ограничивает:

> Draw mask affects the whole layer, not the colour board. You could make it
> work though by duplicating the layer, make the colour board on the lower layer
> and draw mask on the upper layer.
> — u/Silver_Mention_3958, r/finalcutpro, 09.04.2026, https://www.reddit.com/r/finalcutpro/comments/1sgzj4a/draw_mask/of8y3mc/

Про post-effect применение маски, увиденное со стороны монтажёра:

> The problem with doing things like adding a glow to a masked clip is that the
> glow is masked as well, so it doesn't "glow" beyond the bounds of the mask.
> — u/mcarterphoto, r/premiere, 15.08.2026, https://www.reddit.com/r/premiere/comments/1vpcel9/how_to_apply_effects_to_a_masked_object_without_a/p3x32jn/

Про обратную область руками — ровно то, что Resolve делает одной галкой на
чужом ключе, здесь делается копией контура со снятой галкой Inverted:

> Copy the object mask of the person and remove inverted checkbox.
> — u/NLE_Ninja85, r/premiere, 19.04.2026, https://www.reddit.com/r/premiere/comments/1spzhis/how_can_i_mimic_this_effect_in_premiere/oh44na6/

И самый дорогой вид фидбека — «остался на старой версии из-за этого»: человек
приносил контур из внешнего трекера (Mocha Pro) и вставлял его в клип, чтобы
повесить на него эффект, а в 26-й это перестало работать:

> I have both 2025 and 2026 installed, but use 2025 for this reason.
> — u/pmarval, r/premiere, 16.04.2026, https://www.reddit.com/r/premiere/comments/1r89e87/premiere_pro_2026_hate_it_or_love_it/ogiox3f/

Про то, что копия контура — это НЕ общий контур (FCP, но диагноз общий):

> With Draw Mask, the animation is locked to that specific mask instance, so if
> you paste keyframes onto a different mask layer even if it looks identical it
> won’t behave the way you expect.
> — u/StupidRaisins, r/finalcutpro, 24.02.2026, https://www.reddit.com/r/finalcutpro/comments/1rd3n79/keyframe_copy_and_paste_not_working_on_mask/o76n9sw/

Читать это надо так. Жалоб «маска у эффекта — плохая идея» нет ни одной;
жалуются на две вещи. Первая — что её НЕ НАХОДЯТ: в AE механизм существует с
2014 года, в Premiere и Resolve — сколько себя помнят, а треды 2026-го
называются «after 20 years i finally figured out». Вторая — на швы вокруг:
назначение владельца отдельным жестом, разъезд копий контура и режимы
редактирования, растащенные по трём панелям. То есть
дорого не устройство, а его подача.

## Как ложится на Kinotrain

### Что уже есть

- **Цепочка эффектов клипа** — `ClipEvent.fx: Vec<FxInstance>`
  (`src/libs/kt-model/src/project.rs:1522`) плюс параллельные `fx_bypass`,
  `fx_auto` и мастер-выключатель `fx_off`; движку её отдаёт `active_fx()`
  (там же, 1569). Морфер-слой дописывает свои слоты в ТУ ЖЕ цепочку, а
  `Project::effective_clips()` (`project.rs:3013`) — единственная точка выдачи
  клипов движку, экспорту и отрисовке.
- **Два байт-совпадающих исполнителя цепочки**: `apply_video_chain_pooled`
  (`src/libs/kt-fx/src/video.rs:61`) — цикл `for f in fx` над одним живым
  кадром, и его зеркало `chain_ops` (`src/libs/kt-gpu/src/engine.rs:124`),
  собирающее `Vec<FxOp>`.
- **Альфа как канал уже существует**: `greenscreen`
  (`src/libs/kt-fx/src/video.rs:348`) пишет её в `px[3]`, дальше она уходит в
  `alpha_over` композита (`src/libs/kt-engine/src/video.rs`). Ключ физически
  есть, но у него нет ни имени, ни адреса (диагноз F-CLR-006).
- **Нечисловое поле-ССЫЛКА у эффекта — прецедент есть**: `Pan3d.camera:
  Option<CameraId>` (`project.rs:2096`). Числовые параметры рисует единая сетка
  по `ParamSpec`, а ссылка — комбо-боксом в нечисловом хвосте
  `draw_video_fx_params` (`src/apps/kt-app/src/fx_editor.rs:557` и далее).
- **Ссылка по ИМЕНИ с живучей мёртвой ссылкой — тоже прецедент**:
  `MorphTargets::Areas` смотрит на `MarkedArea` (`project.rs:1163`) по имени,
  удалённая область не забывается и рисуется ошибочным стилем (правило 20).
- **Паспорт параметров у структуры** — `params_for_struct!` (им описана
  `CameraParams`): форма-контур с числовыми полями получит и общую сетку полей,
  и общий виджет кривых без единой новой строки в UI.
- **Автоматизация** — `ParamCurve` вешается только на `ParamKind::Smooth`
  (`project.rs:2150`, 2174), домен ключа — тик МАТЕРИАЛА, поэтому trim/split/
  reverse кривые не трогают.
- **Кэш** — `fx_chain_hash` (`src/libs/kt-engine/src/video.rs:153`) хеширует
  Debug ВСЕЙ цепочки плюс геометрию клипа; пост-FX кадр мемоизируется в
  `layer_frame_cpu` (там же, 721).
- **Копипаст слота** — `ops::FxSlotBlock` (`src/libs/kt-model/src/ops.rs:851`)
  и `FxChainBlock`: JSON в системном буфере, переживает перезапуск и переезжает
  между окнами (t205/t215/t217).
- **Формат** — `FxInstance` уезжает serde-конвертом в `metadata.kt` эффекта
  (`src/libs/kt-otio/src/convert.rs:548`), версия конверта ставится ПО
  СОДЕРЖИМОМУ (`src/libs/kt-otio/src/meta.rs:17`, сейчас `KT_META_VERSION = 4`).

### Чего не хватает

Масок нет вообще: грепом `mask|matte|rotoscop` по `src/` находятся только
`front_mask` внутри `transitions.rs` и маска ввода таймкода в `util.rs`. Нет
геометрии контура, нет растеризатора, нет понятия «область кадра» — и это стоит
подчеркнуть: наша `MarkedArea` — это ВРЕМЯ × ДОРОЖКИ, пространственного смысла
в слове «область» у нас пока нет вовсе, а имя уже занято (пространство имён у
областей и меток общее). Нет и экранного редактирования: превью сегодня умеет
ровно один жест — клик play/pause (`src/apps/kt-app/src/app.rs:3205`, t74), ни
одного гизмо поверх кадра не существует.

### Что мешает

1. **Внутри цепочки живут РАЗНЫЕ растры.** `Pan`, `CamMotion`, `EventGrid` и
   `Pan3d` ресемплят кадр в размер проекта (видно по `chain_ops`), остальные
   семь видео-FX размер сохраняют. Маска на ресемплящем слоте не определена:
   «оригинал» и «результат» лежат в разных системах координат — то самое
   ограничение, которое Fusion документирует поимённо для Resize/Scale/Crop.
2. **Кадр приходит в разрешении КАЧЕСТВА.** Декодер отдаёт кадр уже в
   `Quality { width, height }`, и одна и та же цепочка считается на разных
   растрах в превью и в экспорте (`q_key` в `layer_frame_cpu`). Значит
   координаты контура обязаны быть нормированными; «пиксели» тут не годятся.
3. **`params_for!` знает только числа.** `ParamSpec` — это
   min/max/default/step/unit/kind, и ссылка на маску в него не лезет: она
   рисуется руками в хвосте `draw_video_fx_params`, как `Pan3d.camera`. Числовые
   поля самой формы (центр, размер, поворот, растушёвка) в макрос ложатся
   идеально — и вместе с ними бесплатно приезжает автоматизация. Безье-путь не
   ложится: ключи по точкам — это не `ParamCurve`, и он стал бы ВТОРЫМ
   исключением после `CamMotion.keyframes`, у которого уже есть собственный
   мини-редактор в `fx_editor.rs` вопреки правилу «кривые рисует один виджет».
4. **Ключ пост-FX кэша.** `fx_chain_hash` хеширует Debug цепочки. Инлайн-контур
   он поймает сам; ССЫЛКА в Debug есть, а геометрия — нет, и правка общего
   контура не инвалидировала бы пост-FX кадры (инвариант №4). Лечится приёмом
   морфера: разрешать ссылку в `effective_clips()` ДО построения плана
   (`VideoRenderer::new` и так зовёт его первым делом) — движок увидит
   инлайн-геометрию, и хеш снова верен без единой правки в кэше.
5. **Копипаст между проектами.** `FxSlotBlock` — сериализуемый блок в системном
   буфере; ссылка по имени в чужом проекте окажется висячей. Прецедент решения
   есть: `ClipBlock` возит с собой `areas`/`markers`, а `paste_block` считает
   `PasteSkipped` вместо того, чтобы «доправлять» вставку.
6. **Формат.** Новое поле уедет в `metadata.kt` само, но старая сборка (v4) его
   молча проигнорирует и отрендерит эффект НА ВЕСЬ КАДР — это ровно случай
   «тихой порчи», ради которого заведён отказ `KtMetaTooNew`. Значит v5, по тому
   же правилу «версия по содержимому», по которому поднимались v3 (домен ключей)
   и v4 (морфер-слой).
7. **Растеризатор пишется дважды.** Смеситель «оригинал ↔ результат по маске» и
   сама форма нужны и в скалярном ядре с AVX2, и в WGSL, причём SIMD обязан
   совпадать со скаляром байт-в-байт. Аналитическая форма (эллипс/прямоугольник
   через знаковое расстояние с растушёвкой) для этого пригодна; полигональный
   скан-лайн с антиалиасингом — главный риск расхождения CPU↔GPU.

### Какие инварианты задевает

- **№7 (код композиции ОДИН).** Маска обязана быть частью того же плана кадра,
  а не вторым путём: смеситель живёт в цепочке и повторяется в обоих
  исполнителях. Это дороже обычного эффекта ровно вдвое, но по форме — тот же
  `FxOp`, а не новый интерпретатор (в отличие от графа, F-CLR-006).
- **№11 (детерминизм) и правило крейта kt-fx.** Умолчание «маски нет» обязано
  давать прежние байты — тогда GOLDEN_VIDEO и golden-эталоны видео-FX не
  перегенерируются. Схема `#[serde(default, skip_serializing_if …)]` у нас на
  это уже настроена.
- **№4 (кэш — единственная правда о coverage).** Геометрия обязана попасть в
  ключ пост-FX; зато общая маска, изменённая один раз, инвалидирует пост-FX
  кадры всех клипов-потребителей — это правильное поведение, но это чёрн, и
  его надо ждать.
- **№3 (правка модели не уничтожает FX-состояние).** Поле маски осмысленно
  только у видео-вариантов; появление его у слота не должно считаться сменой
  цепочки для аудио-состояния при diff по id клипа.
- **Правила UI.** 13 — маска не заводит собственной панели: числа идут в ту же
  сетку карточки эффекта, ссылка — в её нечисловой хвост, форма правится гизмо
  поверх УЖЕ существующего превью. 17 — «маска-эффект в цепочке» и «маска-поле
  у слота» описывают одно и то же действие, поэтому в продукт берётся РОВНО
  ОДНА из моделей. 20 — мёртвая ссылка на удалённый контур не самоудаляется
  (прецедент областей морфера). 1 — точки контура обязаны иметь хват-зону шире
  рисунка, и гизмо не должно ломать клик play/pause по превью (t74) и двойные
  клики (t206/t207). 8 — гизмо рисуется в кадре, а не «стандартным виджетом».

## Оценка

| Что | Ответ |
|---|---|
| Объём | L — целый раунд: объект-контур в модели, смеситель и растеризатор в ДВУХ исполнителях, гизмо поверх превью, версия формата |
| Где основная работа | kt-model (объект маски + ссылка у слота), kt-fx (смеситель + форма: скаляр и AVX2), kt-gpu (WGSL-зеркало и новый `FxOp`), kt-app (гизмо в превью, хвост карточки эффекта), kt-otio (v5), kt-engine (разрешение ссылки в `effective_clips`, ключ кэша) |
| Формат проекта | нужна версия v5: старый ридер молча отрендерил бы эффект на весь кадр — штатный отказ честнее тихой порчи |
| Тесты, которые придётся написать | round-trip модели и копипаст слота со ссылкой (в духе t205/t215); «без маски — прежние байты» (golden_export t11/t67, video_fx); правка контура инвалидирует пост-FX кадр (в духе t66/t66c); конформанс растеризации CPU↔GPU ТОЛЬКО через `kt_gpu::assert_frames_close` (t181-стиль); мёртвая ссылка резолвится в ничто и рисуется ошибкой (t241-стиль областей); layout-тест гизмо и «клик по превью всё ещё играет» (t74/t76) |
| Зависит от | F-CLR-006 (граф отвергнут — значит ветвится ПОЛЕ, а не провод), F-MPH-006 (сила наложения: тот же смеситель, один буфер) |
| Ломает совместимость | нет: файл без масок байт-в-байт прежний; файл с масками старая сборка честно откажется открыть |

## Вердикт

**Берём модель 2 по семантике и модель 3 по хранению: область действия
принадлежит СЛОТУ цепочки, а геометрия — отдельному именованному объекту
проекта, на который слот ссылается (плюс галка инверсии у ссылки).** Это
единственная комбинация, которая отвечает на исходный вопрос «размыть лицо и
поднять яркость в углу на одном клипе», не заводя ни дубликата клипа, ни
второго морфер-слоя, и при этом не платит главной ценой модели 2 —
расползающимися копиями контура.

Почему не остальные. **Модель 1 (маска у слоя)** у нас означала бы «маска режет
клип целиком», а «ограничить один эффект» пришлось бы делать морфер-слоем или
дубликатом события — то есть воспроизвести ровно ту жалобу, с которой люди
уходят в AE («duplicate that layer and mask it»). **Позиционная модель
(маска-эффект в цепочке, VEGAS/FCP)** соблазнительна дешевизной — новых понятий
ноль, — но её область есть ПРЕФИКС цепочки, вторая маска пересекает первую, и
«ограничить второй эффект из пяти» в ней не выражается; вдобавок порядок слотов
начал бы значить две вещи сразу. **Модель 3 в чистом виде (нодовая, со вторым
входом и холстом связей)** отвергнута в F-CLR-006, и открывать её заново эта
заметка не предлагает: ветвится только АДРЕС контура, картинка остаётся
линейным списком.

Три опоры, на которых рекомендация держится в НАШЕМ коде, уже стоят: ссылка
нечисловым полем у эффекта — `Pan3d.camera`; ссылка по имени с живучей мёртвой
ссылкой — области морфера; разрешение ссылки в инлайн-данные до построения
плана — `effective_clips()`, благодаря которому движок, ключ пост-FX кэша и оба
исполнителя не узнают о существовании ссылок вовсе. Дивиденды: смеситель
«оригинал ↔ результат по маске» — это ТОТ ЖЕ код, что нужен F-MPH-006 для силы
наложения (маска с равномерным уровнем и есть wet/dry слота), а параметрическая
форма получает автоматизацию, копипаст и хеширование даром, потому что её поля —
числа в `params_for!`.

Согласование с F-MSK-001. Она приходит к «маска у слота» с другой стороны — от
геометрии — и это подтверждение, а не совпадение; расходимся мы в одном месте,
и его стоит записать. Там сказано «у слота цепочки — как в FCP», но живьём у
FCP всё сложнее: его семь mask-эффектов режут ВЕСЬ слой (цитата про Draw Mask и
Color Board ниже), а поэффектная область там существует только как встроенные
маски внутри самих эффектов, и только у них есть булевы режимы. Эталон для нас
не FCP, а Premiere по владельцу и AE по адресации: владелец — слот, контур —
общий объект, на который слот ссылается. Форму контура, число точек и режимы
сложения масок внутри слота описывает F-MSK-001, чужую альфу как источник
ключа — F-MSK-003; эта заметка про них ничего не решает.

Границы решения, которые стоит зафиксировать сразу: маска разрешена только у
слотов, не меняющих растр (Fusion перечисляет тот же список запретов); первый
этап — параметрические формы (эллипс, прямоугольник, линейный градиент), как и
постановила F-MSK-001, потому что ключи по точкам произвольного контура не
ложатся в наш единственный виджет кривых. И честная оговорка:
пользователь масок пока не просил (`docs/user-feedback.md` о них молчит) —
заметка закрывает архитектурную развилку заранее, чтобы первая же маска не
приехала «эффектом в цепочке», из которого потом не выбраться без смены формата.

## Источники

- DaVinci Resolve 21 Reference Manual (июль 2026, прочитан локально): гл. 138
  «Secondary Windows» — «all of the windows within a particular node work
  together to limit that node’s grade», «If you don’t create a new node before
  creating a window, you’ll discover you’ve inappropriately changed a primary
  correction into a secondary correction», раздел «Combining Power Windows with
  the Mask Control» (add/subtract), раздел «Copying and Pasting Windows»
  (Copy Window / Paste Window в другую ноду) и «Saving Window Presets»
  («Loaded window presets overwrite whatever other windows were set up in that
  node»); гл. 146 «Combining Keys and Using Mattes» — Outside Nodes и Key Input
  Invert; гл. 79 «Rotoscoping with Masks» — Effect Mask вход, «applied “post
  effect”», список нод без поддержки масок, Pre-Masking Inputs, Garbage/Solid
  Matte. https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf (дата обращения 2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/8-design/fxplugins/beziermask.htm
  — VEGAS Bézier Masking: Mask FX («mask the effects of all plug-in(s) in the
  effect chain that precede the Bézier mask plug-in»), сноска про накопительное
  пересечение масок, Invert, Blend как сила, до пяти масок, трекинг, анимация
  точек «new since VEGAS Pro 23» (2026-09-07). Справки для линейки 2026 в
  открытом доступе нет — см. оговорку в «У кого и как».
- https://support.apple.com/guide/final-cut-pro/intro-to-masking-verce5464e1a/mac
  — FCP: семь mask-эффектов, «Add them to any clip in the timeline just as you
  would add any other clip effect», отдельное «effect masking» у большинства
  эффектов, «Masking blend modes … are available for effect masking only»
  (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/use-multiple-masks-ver1b0945a01/mac
  — FCP: две маски на клипе = Intersect, режим Add требует дубликата клипа
  связанным клипом сверху (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/mask-controls-verf8949b45e/mac
  — FCP: инверсия маски («reverses the mask—swapping its solid and transparent
  areas») (2026-09-07)
- https://larryjordan.com/articles/adobe-updates-and-modifies-masking-in-premiere-26/
  — Premiere 26: маска «located inside the effect», «You can only apply one mask
  to one effect», Unassigned-маска перетаскивается на имя эффекта, галка
  Inverted (2026-09-07)
- Реддит читался через архив Arctic Shift (прямые ссылки на reddit.com из этого
  окружения не открываются); цитаты приведены дословно по архивным записям,
  ссылки указывают на исходные пермалинки: r/AfterEffects
  (`did_you_know_this_pro_tip`, `after_20_years_i_finally_figured_out`),
  r/premiere (`do_you_prefer_the_new_masking_method_in_premiere`,
  `need_help_with_applying_gaussian_blur`,
  `how_to_apply_effects_to_a_masked_object_without_a`,
  `how_can_i_mimic_this_effect_in_premiere`,
  `premiere_pro_2026_hate_it_or_love_it`), r/finalcutpro
  (`draw_mask`, `keyframe_copy_and_paste_not_working_on_mask`) — все
  2026-09-07.
- Строка After Effects взята из соседней заметки F-MPH-002 (Compositing
  Options, Effect Opacity, Mask Reference — там они проверены по
  `helpx.adobe.com`): в этот заход `helpx.adobe.com` не открывался вовсе
  (прокси обрывает соединение), поиск Adobe Community отдаёт 403. Механизм
  дополнительно подтверждён пользовательскими описаниями «hit the “+” symbol.
  It’ll auto-add the first mask on the layer»
  (https://www.reddit.com/r/AfterEffects/comments/1twm374/one_comment_one_great_advice_the_holy_thread_of/opww2u8/).
- Живьём ни в одном редакторе не проверялось: Resolve — по официальному мануалу
  21-й версии, FCP — по официальной справке 12.3, VEGAS — по справке 23-й
  линейки, Premiere — по разбору обновления 26 и форумным описаниям.
