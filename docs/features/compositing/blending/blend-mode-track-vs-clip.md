---
id: F-CMP-003
title: Режим наложения — у дорожки или у клипа
status: в-работе
priority: B
cost: M
category: compositing/blending
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: частично
crates: [kt-model, kt-engine, kt-gpu, kt-app, kt-otio]
invariants: [7, 11]
ui_rules: [9, 13, 17]
round: Р.62
point: null
tests: [t737]
depends_on: [F-MED-004, F-MPH-007]
trackers: []
services: []
checked: 2026-09-23
---

# F-CMP-003 — Режим наложения — у дорожки или у клипа

## Суть

Верхний слой не просто накрывает нижний, а умножает, складывает, осветляет
или вычитает его: дым «Screen» поверх кадра, логотип «Multiply» на белом
фоне, вспышка «Add». Глазами монтажёра это один список из десятка слов рядом
с непрозрачностью. Вопрос заметки — не «какие формулы», а **чей это выбор:
дорожки или клипа**, и что происходит там, где верхний слой кончился.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026 | Compositing Mode — кнопка в заголовке ДОРОЖКИ (Source Alpha, Multiply (Mask), Add, Subtract, Screen, Overlay, …, 3D Source Alpha) | режим принадлежит дорожке и действует на ОДНУ дорожку ниже; чтобы накрыть несколько — «Make Compositing Child», и тогда у родителя появляется второй режим, Parent Composite Mode. «Since lower tracks show through higher tracks, it is the compositing mode of the higher track that determines how much of the lower track shows though» |
| Adobe Premiere | 26 | Opacity → Blend Mode (инспектор Effect Controls) | свойство КЛИПА внутри встроенного эффекта Opacity; действует на всё, что ниже; ключуется как любой параметр |
| DaVinci Resolve | 21 | Composite Mode (Video Inspector → Composite) | свойство КЛИПА; на странице Color те же режимы живут в Layer Mixer и с 18.5 на каждой ноде — три места для одного слова |
| Final Cut Pro | 12.3 | Blend Mode (Video inspector → Compositing) | свойство КЛИПА; «the pixels of an image with a selected blend mode are combined with the pixels of any clip immediately below it in the timeline»; отдельный режим Premultiplied Mix чинит кайму у premultiplied-графики |
| After Effects | 26 | Blending Mode (колонка Mode в панели слоёв) | свойство СЛОЯ; слой — и есть дорожка: одна сущность, один адрес |

## Фишка фишки

**1. У Vegas режим — свойство ДОРОЖКИ и действует ровно на одну дорожку ниже,
поэтому там, где верхнего клипа нет, нижние дорожки могут просто исчезнуть.**
Multiply на дорожке 1 умножает дорожку 2 на дорожку 1; в дыре между клипами
дорожка 1 пуста — и произведение чёрное. Тред 2013 года про «layers below
(4 – 10 layers) are sometimes blacked out» лечится не режимом, а родством:
сделать дорожку 2 ребёнком, чтобы режим применялся к композиту детей. То есть у
дорожечного режима есть цена, которой нет у клипового: ПУСТОТА дорожки — не
«ничего», а операнд. Клиповый режим (Premiere, Resolve, FCP) этой ловушки не
имеет: нет клипа — нет операции.

**2. Режим и непрозрачность — разные оси, и обе стоят ПОСЛЕ альфы.** FCP
говорит это прямо: «Blend modes work in addition to a clip's alpha channel and
Opacity control». Формула — `out = lerp(below, blend(below, above), alpha ×
opacity)`, а не `blend(below, above × opacity)`: при непрозрачности 0.5 Multiply
даёт половину пути к произведению, а не произведение с полупрозрачным
множителем. Реализация, перемножившая opacity в источник до блендинга,
расходится с любым из пяти редакторов на первом же ползунке.

**3. У Resolve одно слово живёт в трёх местах, и это обратная сторона
гибкости.** Composite Mode на клипе, Layer Mixer в дереве нод, режим на каждой
ноде с 18.5 — три уровня с одним списком, и порядок их применения нельзя
поменять из интерфейса. Vegas платит за это иначе: два режима у одной
дорожки-родителя (свой и Parent Composite Mode), и на форуме ровно этот вопрос
— «A “Parent” track can have two compositing modes… Why?».

## Плюсы

- Режим у дорожки (Vegas, AE): одно решение на всю дорожку дыма или логотипов,
  не двести клипов; при переносе клипа на другую дорожку режим остаётся с
  дорожкой, а не уезжает с клипом.
- Режим у клипа (Premiere, Resolve, FCP): нет операнда «пустота дорожки»,
  ключуется вместе с остальными свойствами клипа, копипаст несёт его с собой.
- Раздельные оси «режим» и «непрозрачность» дают Multiply «наполовину» без
  второго слоя.

## Минусы

- Дорожечный режим действует на ОДНУ дорожку ниже, и «накрыть всё» требует
  второй сущности (родство) и второго режима у родителя — два места, где искать.
- Дыра на дорожке с режимом — не прозрачность, а чёрный (Multiply) или белый
  (Screen) результат: ошибка, которую пользователь видит как «пропали слои».
- Клиповый режим у переходов и перекрытий неоднозначен: у пары в кроссфейде два
  режима, и какой действует на зоне смешения, у каждого редактора решено
  по-своему (у Resolve — тред о переходах и composite modes).
- Premultiplied-графика с любым режимом, кроме Normal, даёт тёмную кайму, и
  чинится это отдельным пунктом списка (FCP Premultiplied Mix), а не автоматом.

## Что говорят пользователи

> I have a video on top of another video, and have it blending nicely into the
> background with the composting mode set on multiply of that top channel. The
> problem is now layers below (4 – 10 layers) are sometimes blacked out (rather
> than showing through).
> — Thoreau Bakker, Creative COW, 20.06.2013, https://creativecow.net/forums/thread/compositing-model-multiply-mask-black-out/

> In the Track 1 'Track Header', make Track 1 a Parent to your Track 2. Now
> whenever there is a gap you will see the other tracks and not turn black.
> — Graham Bernard, там же, 20.06.2013

> Since lower tracks show through higher tracks, it is the compositing mode of
> the higher track that determines how much of the lower track shows though.
> — справка VEGAS Pro, «Compositing and masks», https://help.magix-hub.com/video/vegas/23/en/content/topics/8-design/compositingandmasks.htm (дата обращения 2026-09-23)

> Blend modes work in addition to a clip's alpha channel and Opacity control.
> — Final Cut Pro User Guide 12.3, «Intro to blend modes», https://support.apple.com/guide/final-cut-pro/intro-to-blend-modes-vera85320379/mac (дата обращения 2026-09-23)

## Как ложится на Kinotrain

CAPABILITY: openspec/specs/video-plan/
Режим — свойство ДОРОЖКИ на композит её событий («дорожка = неявная лента»): дыра — прозрачность, а не операнд; оси «режим» и «уровень» раздельны и стоят после альфы. Решение владельца Р.62, change `track-object-and-track-fx`.

Сделано в Р.62 B: `BlendMode` живёт на дорожке (`src/libs/kt-model/src/project.rs`),
формула `out = lerp(below, blend(below, above), α × уровень)` написана ПАРОЙ —
`blend_over` в `src/libs/kt-engine/src/video.rs` и её зеркало в
`src/libs/kt-gpu/src/shaders/composite.wgsl` (правятся одним коммитом, как ядра
переходов). Дорожка со стадией входит в план вложением
(`src/libs/kt-engine/src/video.rs`, `track_stage_layer`), поэтому «дырка» в событии
показывает нижнюю дорожку, а не чёрное. Выбор режима — окно дорожки
(`src/apps/kt-app/src/track_window.rs`), сверка с аналитической формулой и GPU —
t737 (`src/libs/kt-engine/tests/track_blend.rs`, `src/libs/kt-gpu/tests/parity.rs`).
Клиповый режим наложения ОТЛОЖЕН: у события уже есть уровень и переходы, и смешивать
две оси в одном раунде значило бы решать за владельца.

## Оценка

| Что | Ответ |
|---|---|
| Объём | M: ядра блендинга парой CPU + WGSL с конформансом, поле у дорожки, пункт в свойствах дорожки |
| Где основная работа | kt-engine (композит), kt-gpu (шейдер композита), kt-model (поле), kt-app (свойства дорожки), kt-otio (конверт трека) |
| Формат проекта | поле в конверте трека, версия раунда Р.62 |
| Тесты, которые придётся написать | каждый режим против аналитической формулы на CPU; конформанс GPU; непрозрачность 0.5 — половина пути; дыра на дорожке с Multiply не чернит нижние; Normal — байт-в-байт прежний композит |
| Зависит от | F-MED-004 (геометрия слоя в плане — та же стадия), F-MPH-007 (композит дорожки как объект) |
| Ломает совместимость | нет: Normal по умолчанию — прежние байты |

## Вердикт

**Берём, у дорожки, на композит дорожки.** Владелец просил «свои возможности
наложения у треков» (Р.62); дорожечный уровень выбран вместе с цепочкой FX
дорожки и её уровнем — одна стадия, три свойства. Ловушку Vegas с пустотой как
операндом обходим тем, что операнд — композит дорожки с честной альфой, а не
«кадр дорожки»; клиповый режим не заводим, пока о нём не попросят: у клипа есть
уровень и фейды, а два режима на одной паре в кроссфейде — неоднозначность,
которую никто из пяти не решил хорошо.

## Источники

- Справка VEGAS Pro, «Compositing and masks» (режимы, родство, Parent Composite
  Mode): https://help.magix-hub.com/video/vegas/23/en/content/topics/8-design/compositingandmasks.htm
  (дата обращения 2026-09-23)
- Creative COW, «Compositing model 'Multiply' (Mask) – Black out» (2013):
  https://creativecow.net/forums/thread/compositing-model-multiply-mask-black-out/
- Creative COW, «A “Parent” track can have two compositing modes… Why?»:
  https://creativecow.net/forums/thread/a-parent-track-can-have-two-compositing-modes-w/
- Final Cut Pro User Guide, «Intro to blend modes» и «Types of blend modes»
  (Premultiplied Mix): https://support.apple.com/guide/final-cut-pro/intro-to-blend-modes-vera85320379/mac ,
  https://support.apple.com/guide/final-cut-pro/types-of-blend-modes-ver007271c06/mac
  (дата обращения 2026-09-23)
- DaVinci Resolve 18.6 manual, «Compositing and Transforms in the Timeline»,
  «More About Composite Modes»: https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1241.htm
  (дата обращения 2026-09-23); тред о переходах и composite modes:
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=220360
- Adobe Premiere: Blend Mode внутри эффекта Opacity — по документации helpx.adobe.com
  [НЕ ПРОВЕРЕНО по первоисточнику из этого окружения; формулировка по памяти
  интерфейса 26.x]
