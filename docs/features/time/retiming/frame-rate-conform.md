---
id: F-SPD-005
title: Приведение частоты кадров источника к сетке проекта
status: исследовано
priority: B
cost: L
category: time/retiming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: частично
crates: [kt-engine, kt-media, kt-model, kt-app]
invariants: [4, 7, 11]
ui_rules: [4, 16, 19]
round: null
point: null
tests: []
depends_on: [F-SPD-003, F-MED-004]
trackers: []
services: []
checked: 2026-09-23
---

# F-SPD-005 — Приведение частоты кадров источника к сетке проекта

## Суть

В проект 25 fps попадает клип 30p, 60p и 23.976. На каждом кадре проекта надо решить,
какой кадр источника показать: ближайший, предыдущий (пол), смесь двух соседних или
синтезированный по движению. Глазами монтажёра — либо лёгкое дёрганье раз в несколько
кадров, либо мягкое двоение, либо «как родное», за цену рендера. Вопрос заметки — где
живёт это правило и что происходит, когда частоты совпадают.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026 | Resample mode у СОБЫТИЯ (Smart / Force / Disable) и умолчание в свойствах проекта | правило у события с умолчанием проекта; Smart смешивает соседние кадры только при несовпадении частот, Disable — ближайший кадр [НЕ ПРОВЕРЕНО по первоисточнику: справка Vegas из этого окружения не открывалась; известно по интерфейсу 19–22] |
| Adobe Premiere | 26 | Interpret Footage → Assume this frame rate; Time Interpolation (Frame Sampling / Frame Blending / Optical Flow) у клипа | частота файла переопределяется ИНТЕРПРЕТАЦИЕЙ (меняет длительность клипа), способ приведения — свойство клипа [НЕ ПРОВЕРЕНО по первоисточнику] |
| DaVinci Resolve | 21 | Project Settings → Mixed Frame Rate Format; Retime Process (Nearest / Frame Blend / Optical Flow) у проекта и у клипа | правило у ПРОЕКТА: «DaVinci will create extra frames for the 23.97 fps clips using the frame interpolation settings of the retiming process (nearest, frame blend, or optical flow, as set in the project settings), and it will drop every second frame of the 60 fps clips» |
| Final Cut Pro | 12.3 | Rate Conform (Video inspector): Fast (Floor) / Fast (Nearest Frame) / Good (Frame Blending) / Better (Optical Flow) / Best (Machine Learning) | правило у КЛИПА; «If the selected clip has the same frame rate as the project, the Rate Conform section doesn't appear in the Video inspector»; «Fast (Floor) is the default setting. Final Cut Pro duplicates frames or rounds down to the nearest whole frame to match the clip's frame rate to the project's frame rate» |
| After Effects | 26 | Interpret Footage → Conform to frame rate; переключатель Frame Blending у слоя (Frame Mix / Pixel Motion) | частота — интерпретация ФУТАЖА, смешение — переключатель слоя; композиция считает на своём fps [НЕ ПРОВЕРЕНО по первоисточнику] |
| Kdenlive | 26.08 | профиль проекта MLT | клип приводится к профилю ближайшим кадром, своего правила у клипа нет; отсюда просьбы «изменить проект под клип» — те же, что у F-MED-004 |

## Фишка фишки

**1. У FCP секция появляется только при несовпадении — и это тот же приём, что у
Spatial Conform.** Контрол не занимает места и одновременно СООБЩАЕТ факт: видна секция
— частоты разошлись. У Resolve, напротив, правило у проекта, и человек узнаёт о
несовпадении по дёрганью.

**2. У Resolve частота проекта ЗАПИРАЕТСЯ первым клипом.** Пока таймлайн пуст, fps
меняется; после первого клипа поле серое, и единственный выход — новый проект. Это и
есть ответ на «первый клип задаёт проект», доведённый до крайности: не подсказка, а
замок.

**3. «Пол» и «ближайший» — разные ответы, и умолчание у FCP — пол.** Floor дублирует
кадр назад и никогда не показывает кадр из будущего; Nearest округляет в обе стороны и
местами опережает звук на полкадра. Наш `clip_frame_keys` ставит пол по сетке файла —
ровно умолчание FCP, без имени и без выбора.

## Плюсы

- Правило у клипа (FCP) — 60p-футаж под замедление получает Optical Flow, а интервью
  25→30 — пол; один проект, разные ответы.
- Правило у проекта (Resolve) — переключение задним числом на весь монтаж.
- Секция-по-факту (FCP) — контрол не занимает места, пока решать нечего.

## Минусы

- Замок частоты (Resolve) — ошибка первого клипа стоит нового проекта.
- Смешение и оптический поток стоят полного второго декода и не детерминированы
  между версиями (см. F-SPD-003).
- Интерпретация футажа (Premiere/AE) меняет ДЛИТЕЛЬНОСТЬ клипа и рассинхронизирует
  его со своим же звуком, если тот не переинтерпретирован.

## Что говорят пользователи

> DaVinci will create extra frames for the 23.97 fps clips using the frame
> interpolation settings of the retiming process (nearest, frame blend, or optical
> flow, as set in the project settings), and it will drop every second frame of the
> 60 fps clips.
> — Frame.io blog, «Mixing Frame Rates in DaVinci Resolve – Part 2», 07.10.2019,
> https://blog.frame.io/2019/10/07/mixed-rates-resolve-part-2/

> If the selected clip has the same frame rate as the project, the Rate Conform
> section doesn't appear in the Video inspector.
> — Final Cut Pro User Guide 12.3, «Conform frame sizes and rates»,
> https://support.apple.com/guide/final-cut-pro/conform-frame-sizes-and-rates-ver3363b44e/mac
> (дата обращения 2026-09-23)

## Как ложится на Kinotrain

CAPABILITY: openspec/specs/video-plan/

### Что уже есть

- Пол по сетке файла — `clip_frame_keys` (`src/libs/kt-engine/src/video.rs`), одна
  функция на заказ и чтение (инвариант 4); процедурный источник считается на
  непрерывном тике (Р.56).
- Смешивание кадров у события («Кадры: смешивание», Р.37) — `blend_frames` там же,
  ядро `dissolve_into`, пара решается той же функцией.
- Синтетика с собственной сеткой кадров — `SyntheticSource::with_video_fps`
  (`src/libs/kt-media/src/synthetic.rs`): на ней различимы пол и ближайший.
- fps проекта — делители 90 000 (`TICK_HZ % fps == 0`), 29.97 невыразим.

### Чего не хватает

- Самого правила как выбора: пол зашит, «ближайший» и «смешение соседних по
  дробной позиции» не выбираются; «оптический поток» — F-SPD-003.
- Места правила: у проекта с исключением у события — та же пара, что у правила
  приведения кадра (F-MED-004, Р.62).
- Сообщения о несовпадении при постановке — Р.62 сообщает про пропорции; частота
  — соседняя строка той же подсказки.

### Что мешает

- Ключ кадра в кэше — pts источника (инвариант 4): «ближайший» меняет карту
  тик→кадр, и заказ обязан считаться той же функцией, что чтение.
- Инвариант 11: смена умолчания двинула бы GOLDEN_VIDEO на фикстурах 30p в проекте
  25p — умолчание обязано остаться полом.

## Оценка

| Что | Ответ |
|---|---|
| Объём | L: правило у проекта и события, карта тик→кадр с тремя ответами, подсказка |
| Где основная работа | kt-engine (`clip_frame_keys`), kt-model (поля), kt-app (подсказка, окно проекта) |
| Формат проекта | поле правила у проекта и события, версия раунда |
| Тесты | пол/ближайший/смешение на синтетике с сеткой; GOLDEN не двигается на умолчании |
| Зависит от | F-MED-004 (место правила), F-SPD-003 (оптический поток) |
| Ломает совместимость | нет при умолчании «пол» |

## Вердикт

**Отложено (Р.62).** Владелец назвал «разные fps» в одной претензии с соотношением
сторон, но приведение частоты — другая стадия конвейера (карта тик→кадр, не геометрия),
и в раунде Р.62 решено не смешивать. Умолчание «пол» у нас совпадает с FCP; недостаёт
имени и выбора. Брать после F-MED-004, в той же паре «проект + исключение у события»
и с той же подсказкой при постановке.

## Источники

- Final Cut Pro User Guide 12.3, «Conform frame sizes and rates»:
  https://support.apple.com/guide/final-cut-pro/conform-frame-sizes-and-rates-ver3363b44e/mac
  (дата обращения 2026-09-23)
- Frame.io, «Mixing Frame Rates in DaVinci Resolve» (части 1–5, 2019):
  https://blog.frame.io/2019/09/30/mixed-rates-resolve-part-1/ ,
  https://blog.frame.io/2019/10/07/mixed-rates-resolve-part-2/
- DaVinci Resolve 18.6 manual, «Mixed Frame Rates» (страница из этого окружения не
  открылась, 404): https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1321.htm
- Замок частоты после первого клипа: https://beginnersapproach.com/davinci-resolve-frame-rate-issues-fixes/
  (дата обращения 2026-09-23)
- Vegas / Premiere / After Effects — по памяти интерфейса, первоисточники не открыты
  [НЕ ПРОВЕРЕНО]
