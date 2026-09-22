---
id: F-CLR-008
title: Управление цветом — теги пространства, рабочее пространство, вывод
status: исследовано
priority: B
cost: XL
category: color/management
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: нет
crates: [kt-ffmpeg, kt-media, kt-engine, kt-gpu, kt-model, kt-app]
invariants: [7, 9, 11]
ui_rules: [4, 16, 19]
round: null
point: null
tests: []
depends_on: [F-CLR-005, F-MED-004]
trackers: []
services: []
checked: 2026-09-23
---

# F-CLR-008 — Управление цветом — теги пространства, рабочее пространство, вывод

## Суть

Телефон снимает HLG BT.2020, камера — log, скриншот — sRGB полного диапазона, старый
DV — BT.601 ограниченного. Положенные рядом без приведения они разного цвета и разной
яркости, а экспорт без тегов заставляет плеер угадывать. Управление цветом — это три
решения: что означает файл (теги), в чём считаем (рабочее пространство) и во что выдаём
(вывод с тегами).

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026 | Project Properties → Pixel format (8-bit / 32-bit full range / video levels), ACES Color Space | рабочее пространство — свойство проекта; интерпретация файла — Media Properties [НЕ ПРОВЕРЕНО по первоисточнику] |
| Adobe Premiere | 26 | Lumetri Color → Settings; Sequence Settings → Working Color Space; Interpret Footage → Color Space Override | три уровня: файл, секвенция, вывод [НЕ ПРОВЕРЕНО по первоисточнику] |
| DaVinci Resolve | 21 | Project Settings → Color Management: DaVinci YRGB / DaVinci YRGB Color Managed / ACES; Input и Output color space, per-clip Input Color Space | правило у проекта, переопределение у клипа; при выключенном управлении всё считается «как есть» [НЕ ПРОВЕРЕНО по первоисточнику] |
| Final Cut Pro | 12.3 | Library → Color Processing (Standard / Wide Gamut HDR); Project → Color Space; Info inspector → Color Space Override | рабочее пространство — свойство БИБЛИОТЕКИ: Standard — «linear RGB with Rec. 709 color primaries, the color space that previous versions of Final Cut Pro have used»; вывод — свойство проекта: «the project's render files are generated in that color space, and your project is exported in that color space if the destination codec supports it» |
| After Effects | 26 | Project Settings → Color: Working Space, Linearize; Interpret Footage → Color Management | рабочее пространство у проекта, интерпретация у футажа [НЕ ПРОВЕРЕНО по первоисточнику] |

## Фишка фишки

**1. У FCP рабочее пространство — свойство БИБЛИОТЕКИ, а не проекта.** Выше проекта
есть ещё один уровень, и переключить его для одного монтажа нельзя; зато два проекта
одной библиотеки не могут разойтись в математике. У Resolve и AE тот же выбор стоит на
проекте — и два таймлайна одного проекта тоже делят его.

**2. Переопределение тега — на ФАЙЛЕ, а не на событии.** Color Space Override у FCP и
Input Color Space у Resolve правят интерпретацию медиа: один файл — одно значение,
сколько бы раз он ни стоял на таймлайне. Это противоположно правилу приведения кадра
(F-MED-004), где исключение живёт у события: «что означает файл» — факт о файле, «как его
положить» — решение монтажа.

**3. Вывод «в пространстве проекта, если кодек умеет» — молчаливое условие.** FCP пишет
render-файлы в пространстве проекта и экспортирует в нём же «if the destination codec
supports it»; когда не умеет — приводит, не спрашивая. Тег на выходе — часть пресета
экспорта, а не проекта (родня F-EXP-001).

## Плюсы

- Один уровень выше проекта (FCP) — проекты библиотеки не расходятся математикой.
- Переопределение на файле — одно исправление чинит все экземпляры.
- Управляемый режим Resolve — log-камеры и телефон ложатся в один ряд без LUT.

## Минусы

- Включение управления цветом меняет вид ВСЕГО, что было собрано без него.
- Linear/f32 рабочее пространство удваивает память промежуточных буферов и ломает
  8-битные байт-эталоны.
- Ошибочный тег в файле (частый случай) виден только глазами.

## Что говорят пользователи

> the project's render files are generated in that color space, and your project is
> exported in that color space if the destination codec supports it.
> — Final Cut Pro User Guide 12.3, «Use wide-gamut HDR color processing»,
> https://support.apple.com/guide/final-cut-pro/use-wide-gamut-hdr-color-processing-ver1cd9629a5/mac
> (дата обращения 2026-09-23)

НЕТ ЦИТАТ: живых жалоб не искали — заметка заведена по решению Р.62 как фиксация
отложенного, бюджет поиска раунда ушёл на приведение кадра и дорожки.

## Как ложится на Kinotrain

CAPABILITY: openspec/specs/video-plan/

### Что уже есть

- Конвейер — 8-битный RGBA без тегов: `to_rgba` (`src/libs/kt-ffmpeg/src/source_impl.rs`)
  зовёт `sws_getContext` без флагов цветового пространства и диапазона; кадр
  считается «как есть».
- Экспорт — `MuxConfig` (`src/libs/kt-ffmpeg/src/lib.rs`) без тегов пространства;
  плеер получателя угадывает.
- Цветокоррекция событием — ColorGrade/Duotone/Posterize (F-CLR-005), LUT (F-CLR-004)
  — всё в 8 битах поверх «как есть».

### Чего не хватает

- Тегов у `MediaInfo` (`src/libs/kt-media/src/lib.rs`): пространство, передаточная
  функция, диапазон — kt-ffmpeg их читает бесплатно, но не отдаёт.
- Рабочего пространства проекта и тегов вывода в экспорте.
- Приведения на входе (диапазон 16–235 → 0–255 хотя бы) — самая дешёвая и самая
  частая половина.

### Что мешает

- Инвариант 11: любое приведение на входе двигает байты; golden'ы сняты на
  синтетике без тегов — их обязано хватить, чтобы «нет тегов = как есть» осталось
  байт-в-байт.
- Инвариант 7: приведение обязано жить в плане, парой CPU + WGSL, не в декодере
  (урок F-MED-004: решение в декодере — вне единого кода).

## Оценка

| Что | Ответ |
|---|---|
| Объём | XL: теги в фактах источника, рабочее пространство, приведение парой CPU + WGSL, теги вывода, окно проекта |
| Где основная работа | kt-ffmpeg (чтение тегов), kt-media, kt-engine/kt-gpu (приведение), kt-model, kt-app |
| Формат проекта | поля проекта и медиа, версия раунда |
| Тесты | «нет тегов = как есть» байт-в-байт; диапазон 16–235 → 0–255 на фикстуре с тегом; конформанс GPU |
| Зависит от | F-MED-004 (окно проекта, факты источника), F-CLR-005 |
| Ломает совместимость | нет при умолчании «как есть» |

## Вердикт

**Отложено (Р.62).** Владелец назвал «разные цветности» в одной претензии с
соотношением сторон; разведка показала, что это отдельный раунд размером с переходы
(теги, рабочее пространство, пара ядер, вывод). Первый разумный кусок — только теги в
фактах источника и диапазон на входе, с умолчанием «как есть» байт-в-байт. Брать после
F-MED-004: окно проекта и факты источника появляются там.

## Источники

- Final Cut Pro User Guide 12.3, «Use wide-gamut HDR color processing» и «Intro to wide
  color gamut and HDR»: https://support.apple.com/guide/final-cut-pro/use-wide-gamut-hdr-color-processing-ver1cd9629a5/mac ,
  https://support.apple.com/guide/final-cut-pro/intro-to-wide-color-gamut-and-hdr-ver09be4f91f/mac
  (дата обращения 2026-09-23)
- Vegas / Premiere / Resolve / After Effects — по памяти интерфейса, первоисточники не
  открыты [НЕ ПРОВЕРЕНО]
