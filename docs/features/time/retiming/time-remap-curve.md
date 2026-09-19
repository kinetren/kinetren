---
id: F-SPD-002
title: Ретайм-кривая — время как анимируемый параметр
status: кандидат
priority: B
cost: L
category: time/retiming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: частично
crates: [kt-model, kt-fx, kt-engine, kt-app, kt-otio, kt-cache]
invariants: [2, 4, 5, 7, 8, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-SPD-001, F-SPD-003, F-AUT-001, F-AUT-002]
checked: 2026-09-07
---

# F-SPD-002 — Ретайм-кривая — время как анимируемый параметр

## Суть

Не «скорость 50 %», а ГРАФИК: для каждого момента события известно, какой
кадр материала на нём показать. По такому графику один клип внутри ОДНОГО
события успевает замедлиться, разогнаться, замереть и пойти назад — без
резки на куски. Рисуют график двумя способами: по вертикали либо время
источника (кривая ПОЛОЖЕНИЯ), либо процент скорости (кривая СКОРОСТИ,
которую редактор интегрирует). Вещь одна, ощущаются они противоположно, и
что можно нарисовать в одной — в другой нельзя.

Постоянная скорость события и модель ripple разобраны в F-SPD-001,
достройка промежуточных кадров — в F-SPD-003; здесь только сама кривая.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026 (справка 23) | **Velocity envelope** — огибающая СОБЫТИЯ (не дорожки) | кривая СКОРОСТИ, диапазон **−100 % … 0 % … +300 %**: 0 = стоп-кадр, минус = ход назад, всё на той же линии, без отдельных команд. Длина события не меняется — материал кончается раньше, и остаток «заполнится либо фризом последнего кадра, либо дополнительным материалом из файла». Безье нет: 6 пресетов интерполяции на СЕГМЕНТ (Linear/Fast/Slow/Smooth/Sharp/Hold) плюс рисование от руки кистью. Аудио огибающей не подчиняется вовсе |
| Adobe Premiere | 26 | **Time Remapping → Speed** — фиксированный эффект клипа + «резинка» на событии | кривая СКОРОСТИ. Ключ **расщепляется**: «When the speed keyframe is dragged apart and spread out beyond one frame, the halves form a speed change transition»; между половинками серая зона и «a blue curve control» с безье-ручками. Ход назад — Ctrl/Cmd-drag ключа, официально **palindrome reverse**, в дорожке скорости появляются «left-pointing angle brackets». Стоп-кадр — Ctrl+Alt-drag, «hold keyframes» с вертикальными засечками. Вниз кривая доходит только до 1 %, не до 0 (по свидетельству пользователей, см. цитаты; в справке предела нет). Числа в Effect Controls — «for reference only. You cannot edit these values directly there». Длина клипа меняется; звук «remains at 100 % speed» и уезжает |
| DaVinci Resolve | 21 (сверено с 20) | **Retime Controls** (Cmd+R) · **Retime Curve**: кривые **Retime Frame** и **Retime Speed** · Change Clip Speed | ТРИ интерфейса над ОДНОЙ моделью: «the control points of each of the speed curves have a 1:1 correspondence to the speed points that are exposed in the Retime controls», правки зеркалятся вживую, обе кривые можно показать разом. Retime Frame — положение (вертикаль — кадр источника, горизонталь — кадр таймлайна); Retime Speed — проценты, и там тянут САМ СЕГМЕНТ, а не точки. Безье-изинг ретайм-ключей общий с прочими параметрами (21). Freeze — две speed point и красные полосы; Reverse Segment — пункт меню |
| Final Cut Pro | 12.3 | **Retime Editor** (Cmd+R) — цветная полоса над клипом | КРИВОЙ НЕТ ВООБЩЕ: только сегменты (оранжевый — медленнее, синий — быстрее, красный — hold), скорость тянется ручкой у правого края сегмента. Плавность — отдельный объект **Speed Transition**: «a shaded bar on either side of the border between speed segments», тримится за края, снимается галкой на конкретном стыке. Hold — Shift-H, 2 с по умолчанию. Назад — только целым клипом или пресетом Rewind 1×/2×/4×. Скорость не кейфреймится и в Video Animation её нет |
| After Effects | 26 | **Time Remap** (Layer → Time → Enable Time Remapping, Ctrl+Alt+T) | кривая ПОЛОЖЕНИЯ: значение свойства — момент источника, «an upward angle indicates forward playback, while a downward angle indicates reverse playback». Включение сразу ставит два ключа — в начале и в конце слоя, «with vertical time values equal to their horizontal position on the timeline». Freeze — просто плоский участок. Тот же Graph Editor показывает и Value Graph, и Speed Graph одного свойства; Info-панель пишет мгновенную скорость в сек/сек. Слой можно тянуть за пределы длины исходника. Звук ретаймится ВМЕСТЕ с видео, с плывущим питчем |

## Фишка фишки

**1. Кривая положения и кривая скорости не равны в правах: на кривой
скорости нельзя нарисовать ни ноль, ни минус.** Математически одна —
производная другой, но авторить в них можно РАЗНОЕ, и это видно по трём
независимым свидетельствам. Resolve, где обе кривые лежат над одним
набором точек («1:1 correspondence») и обе редактируемые, прямо
запрещает: «it's impossible to create reverse motion using the Retime
Speed curve; you need to use either the Retime controls or the Retime
Frame curve». Premiere, где кривая только скоростная, вниз доходит лишь до
1 % (свидетельство пользователя, см. цитаты) — стоп-кадр приходится делать
отдельным жестом (Ctrl+Alt-drag,
«hold keyframes»), а ход назад — третьим (Ctrl-drag, «palindrome
reverse»), и оба рисуют на резинке СВОИ значки, а не точки кривой. AE, где
кривая позиционная, не имеет ни одной специальной команды: freeze — это
«drag the current keyframe marker to a value equal to the previous
keyframe value so that the graph line is flat», реверс — просто
убывающий участок.

Причина геометрическая. Положение кадра — интеграл скорости; чтобы
получить остановку, скорость должна быть ровно нулём, а функция «сколько
материала съедено» — перестать строго возрастать, и обратная к ней
(«в какой момент играется вот этот кадр») исчезает. Все, кто хранит
скорость, упираются в это одинаково и решают одинаково — клампом чуть выше
нуля плюс отдельным объектом «hold». У нас тот же кламп и по той же
причине: `RATE_CLAMP = (0.25, 32.0)`, и в `seg_dt` записано «знаменатель ≥
r0 ≥ RATE_CLAMP.0, нулём не бывает». Премьеровский 1 % — эмпирическое
подтверждение, что это не наша частность.

Побочно: в мануале Resolve 20 этот самый запрет напечатан с ошибкой —
приписан «Retime **Position** curve» и отсылает к «Retime **Speed**
curve», то есть абзац про Speed-кривую ссылается сам на себя; в 21
исправлено. Из опечатки видно легаси-имя: нынешняя **Retime Frame** — это
прежняя **Retime Position**, поэтому «position curve» в старых тредах и
«Retime Frame» в новых — одно и то же.

**2. «Плавный разгон» — три разных объекта в трёх редакторах, и ни один не
является ключом.** В Premiere ключ скорости физически расщепляется:
«Speed keyframes can be split in half, acting as two keyframes for marking
the beginning and end of a speed-change transition», и дальше «A gray area
appears between the halves of the speed keyframe, indicating the length of
the speed transition… A blue curve control appears in the gray area» —
ручка принадлежит ЗОНЕ, а не точке, и половинки создаются с разницей в
один кадр. В Final Cut разгон вообще отдельная сущность: «A transition
appears as a shaded bar on either side of the border between speed
segments» — те же две половинки, но с собственной длительностью, которую
тримят за края. В Resolve 21 то же решено интерполяцией: «The Retime
parameter controls in the Keyframe Editor have been enhanced to use the
same bezier easing tools as the other parameters». У VEGAS безье нет
совсем — только выбор одного из шести пресетов интерполяции на сегмент.

Следствие, ради которого это стоит записать: у КРИВОЙ СКОРОСТИ плавности
нет бесплатно. Кусочно-линейная скорость даёт непрерывное, но негладкое
положение кадра, и излом виден как рывок — гладкость приходится добывать
второй сущностью (Premiere, FCP) либо безье поверх (Resolve). У кривой
ПОЛОЖЕНИЯ обычная ручка ключа и есть разгон, ничего добавлять не надо.

**3. Проценты у AE и Premiere означают противоположное, и это не описка
пользователя.** Один и тот же «50 %» в Time Stretch After Effects — это
вдвое БЫСТРЕЕ (слой ужимается до половины), а в Speed/Duration Premiere —
вдвое МЕДЛЕННЕЕ. Внутри одного пакета Adobe. Отсюда же вторая половина
путаницы: у AE ретайм называется Time Remapping и по вертикали у него
время, у Premiere тоже называется Time Remapping — но по вертикали
скорость, и на форуме предложений Adobe прямо просят переименовать
премьеровский в «Speed Curve or whatever».

**4. Звук: единственный, кто тянет его за кривой, — не NLE.** VEGAS пишет
в справке «the event's audio will not play in sync with the video»,
Premiere — «the audio remains linked to the video, but remains at 100 %
speed», Resolve носит в одном pdf три взаимоисключающих утверждения
(«audio … will be muted» / «can now play either pitch-corrected or
un-pitch-corrected variable speed audio» / «always pitch corrected»), FCP
тянет, но только постоянную скорость. Ретаймит звук по кривой ровно один —
After Effects, и делает это тем же способом, что мы: «You can remap audio
files to gradually decrease or increase the pitch, play audio backward, or
create a warbled or scratchy sound», причём подраздел так и называется
«Time-remap audio pitch» и советует держать скорость ниже 200 %, иначе
визг. То есть варп с плывущим питчем — это не наша самодеятельность, а
модель AE; вопрос сохранения питча отдельным DSP разобран в F-SPD-001.

## Плюсы

- Скорость меняется ВНУТРИ события, поэтому «замедлить середину» не даёт
  двух лишних склеек, которые потом надо двигать и синхронизировать вместе.
- Кривая положения адресует КАДР, поэтому попадание держится: удар на 12-м
  кадре остаётся на 12-м, что бы ни делали с кривой раньше по времени.
  У кривой скорости то же попадание — результат всего интеграла.
- На кривой положения freeze и реверс — обычные участки графика, а не
  отдельные команды с отдельными значками: словарь интерфейса не растёт.
- Кривая скорости зато показывает число, которое монтажёр называет вслух
  («поставь 300 %»), — на позиционной его надо вычислять из наклона.
- Одна модель под несколькими интерфейсами (Resolve) означает, что ручки
  на клипе и точки на кривой — одно и то же, без конвертации и без вопроса
  «а что главнее».
- У FCP плавность — объект с длительностью, значит её тримят и снимают
  поштучно, а не подбирают формой ключа.

## Минусы

- Кривая скорости: правка РАННЕГО ключа сдвигает всё последующее, потому
  что положение кадра — интеграл. Каждое «чуть быстрее в начале» отменяет
  расстановку попаданий дальше по клипу.
- И она же не дотягивается до нуля и не уходит в минус, поэтому стоп-кадр
  и ход назад приходится вводить отдельными жестами и отдельными видами
  ключей — у Premiere это три разных модификатора мыши на одной резинке.
- Кривая положения: скорость нигде не написана числом, «ровно 50 %»
  превращается в подбор наклона.
- Расщеплённый ключ Premiere: сущностей вдвое больше, чем видно точек,
  «удалить ключ» перестаёт быть однозначной операцией, а числа в Effect
  Controls «for reference only» — редактировать их нельзя.
- Модель длины делит редакторы на два лагеря (см. F-SPD-001), и у Premiere
  третий, свой: клип не риплит и не перекрывает соседа, а «pushes remaining
  frames into the tail of the lengthened clip» — кадры не пропадают, но и
  не видны, пока не сделаешь дырку и не растянешь край.
- Перенос между редакторами ретайм не переживает: OTIO как обменный формат
  знает только `LinearTimeWarp{time_scalar}` и `FreezeFrame`, кривой в
  схеме нет.

## Что говорят пользователи

Ровно про две модели одной вещи — заявка в трекер предложений Adobe,
название темы «Make Time Remapping less confusing»:

> In After Effects, Time Remapping is quite logical: set a keyframe in the clip, and tell what frame should show at that time - everything in between keyframes is interpolated accordingly. But, unfortunately, Premiere Pro messes with this principle: it kind of allows to set 'Speed' keyframes, which are not actually editable.
> — Jaydude-wb, Adobe Community, 06.08.2024, https://community.adobe.com/feature-requests-730/make-time-remapping-less-confusing-1327690

Про то, что различие моделей чувствуется именно длиной события:

> Unlike After Effects' Time Remapping, Premiere Pro's speed ramping constantly changes the clip length, making it frustrating to refine the timing of edits.
> — michak19395341, Adobe Community, 28.02.2025, https://community.adobe.com/feature-requests-730/improve-speed-ramping-to-behave-more-like-after-effects-time-remapping-1328339

Про то, что у скоростной резинки нет полноценного графика:

> Creating speed ramps requires manipulating small split keyframes and limited Bezier handles directly inside the timeline clip. There is no proper scalable speed or value graph comparable to After Effects.
> — Alex-Dawson, Adobe Community, 08.08.2026, https://community.adobe.com/feature-requests-730/improve-speed-ramping-to-behave-more-like-after-effects-time-remapping-1328339

И про кламп у нуля — человек ожидал, что стоп-кадр это просто 0 % на
кривой:

> I've never actually played with Premiere's time remapping via keyframes before. I assumed I'd be able to ramp down to 0% speed for a freeze, but it seems you can only go to 1%…
> — Gareth R, Creative COW, форум Adobe Premiere Pro, 01.03.2024, https://creativecow.net/forums/thread/time-remapping-keyframes-only-go-down-to-1-not-0-is-that-right/

«Хочу как в Resolve» от пользователя FCP, у которого кривой нет вовсе:

> I used davinci resolve the other day and absolutely love how the speed ramping works with retime curves. … I wish fcpx would have a similar implementation.
> — Darren Martin, Creative COW, форум Apple Final Cut Pro, 13.09.2018, https://creativecow.net/forums/thread/speed-ramping-question/

Обратная жалоба — из AE, где звук за кривой всё-таки идёт, и это мешает:

> The issue is now that whenever the video slows down or speeds up the audio also slows down and speeds up along with it. I want to know how to keep the audio speed default without the time remap affecting it.
> — Sachin Jadhav, Creative COW, форум Adobe After Effects, 06.04.2021, https://creativecow.net/forums/thread/audio-getting-affected-by-time-remapping/

И про фиксированное окно события в VEGAS (ответов в треде не было):

> I know using Envelope won't shorten or lengthen the duration of the selected chunk. What I don't understand at all why there's NO ANY hotkey (pressing down CTRL or something) to make this happen?
> — Tobin Bell, Creative COW, форум VEGAS Pro, 26.10.2014, https://creativecow.net/forums/thread/velocity-envelope-mess/

## Как ложится на Kinotrain

### Что уже есть

**Переменная скорость видео у нас РАБОТАЕТ с Р.37** (пп.246–251
`docs/porting-notes.md`). Устройство:

- Кривая скорости — обычная автоматизация параметра `rate` эффекта
  `SlowedReverb` (`src/libs/kt-model/src/project.rs`, спека
  `rate: f64, 0.25, 32.0, 1.0, 0.01, Ratio, Smooth`). Отдельной сущности
  «ретайм» в модели нет вовсе.
- Карта времени — интеграл этой кривой: `S(tau) = ∫rate` по ВЫХОДНОМУ
  прогрессу события, кусочно-квадратичная; обратная `T = S⁻¹` — корень
  квадратного уравнения в форме citardauq (`src/libs/kt-fx/src/lib.rs`:
  `source_progress`, `output_time_for_source`, `warp_material_ticks`).
- Точка правки на видео ровно одна — `clip_material_pos`
  (`src/libs/kt-engine/src/video.rs`), от неё сами следуют `layers_at`,
  `wanted_frames`, префетч плеера, прогрев превью и экспорт (t247d
  сравнивает множествами «что согрето» и «что прочитано»).
- Скорость СИНХРОННА у link-пары A/V: `sync_warp_to_link_group` в мутациях
  `SetFx`/`SetFxAuto`/`SetFxBypass` (`src/libs/kt-model/src/model.rs`).
  Правило живёт в модели, а не в окне FX, потому что `SetFx` прилетает ещё
  из вставки цепочки из буфера, из bake морфера, из ридера формата и из
  kt-cli. По этому пункту мы стоим на модели AE, а не NLE: звук идёт за
  кривой и питч плывёт.
- Модель длины — как у VEGAS и AE: событие держит окно на таймлайне,
  меняется только то, какой материал в него попал.
- Достройка кадров: `FrameMode::Nearest` и `Blend` — предмет F-SPD-003.
- Превью события считает ТУ ЖЕ карту: волноформа и филмстрип идут через
  `kt_engine::ClipWarp::material_off` (`src/libs/kt-engine/src/lib.rs`),
  карты кэшируются по `model_rev` в `warp_maps()`
  (`src/apps/kt-app/src/app.rs`).
- Домен ключа — тик материала, но нормированная ось ЛИНЕЙНА и не зависит от
  самой кривой (Р.37 С1, п.246): `(k.t − source_in) / duration`.
  Подробности домена — F-AUT-002.

### Чего не хватает

1. **Второго вида кривой нет.** У нас только скоростной, а «покажи этот
   кадр в этот момент» не выражается. Resolve даёт оба над одними точками,
   AE — позиционный, и именно на нём freeze и реверс бесплатны.
2. **Нет остановки и нет хода назад внутри события.** `RATE_CLAMP =
   (0.25, 32.0)`; стоп-кадра в проекте нет вообще (`grep -i freeze` по
   `src/` даёт один тест лупа), реверс — только на всё событие целиком
   (`ClipEvent.reversed`). У Premiere та же стена (1 %), но обойдена двумя
   отдельными жестами; у VEGAS стены нет, потому что за огибающей не идёт
   звук.
3. **Плавного разгона нет.** Кривая `rate` кусочно ЛИНЕЙНА по построению:
   `rate_at` принудительно зовёт `param_at(…, false)`, а
   `AutoProfiles::from_curves` гасит `ease` именно для `"rate"` — интеграл
   от smoothstep был бы четвёртой степени. Безье нет нигде (F-AUT-001),
   расщеплённого ключа-пары нет и подавно; из шести вегасовских пресетов
   сегмента у нас один — Linear.
4. **Ретайм не виден на клипе.** Кривая правится только в окне FX
   (`src/apps/kt-app/src/fx_editor.rs` + виджет
   `src/apps/kt-app/src/automation.rs`); на событии линией по верху
   рисуется лишь уровень (`opacity`/`gain_db`). У всех пяти ретайм живёт
   ПРЯМО НА КЛИПЕ, у троих это основной способ.
5. **Скорость показана отношением, а не процентом.** `ParamUnit::Ratio`
   печатается без суффикса (`src/apps/kt-app/src/widgets.rs`): видно
   «0.50» там, где во всех пяти написано «50 %». С учётом того, что даже
   Adobe развела проценты в две стороны, единица измерения тут — не мелочь.
6. **Чужая постоянная скорость из `.otio` не играется.** Импорт понимает
   `LinearTimeWarp` только со скалярами −1 (это наш `reversed`) и 1; любой
   другой уходит в passthrough с notice «time warp x{} is not rendered
   (kept untouched)» (`src/libs/kt-otio/src/convert.rs`).

### Что мешает

**Круг «параметр задаёт домен собственных ключей».** Ключ автоматизации у
нас адресован тиком МАТЕРИАЛА (`AutoKey.t`, F-AUT-002), а ретайм-кривая —
это и есть отображение «таймлайн → материал». Ключ такой кривой,
адресованный материалом, зависит от значения самой кривой: подвинул
значение — уехала точка. Круг уже срабатывал: до Р.37 ось `rate` считалась
через варп, и пользователь поймал ровно это (п.246).

**Кривая ПОЛОЖЕНИЯ от круга свободна по построению, и это, а не удобство,
объясняет, почему Resolve авторит реверс именно на ней.** Её домен —
время таймлайна, которое ни от какой кривой не зависит; материал стоит в
ЗНАЧЕНИИ ключа. Мы выбрали противоположный разрыв: хранить скорость, домен
ключа оставить материальным, а нормированную ось трактовать как выходную
долю события — `(k.t − source_in)/duration`, где знаменатель есть
номинальная длина, про варп не знающая. «Материальный» тик ключа у нас —
переодетое смещение по телу события; при линейной карте они совпадают, и
поэтому `AutoProfiles::from_curves` не поменялась ни на строку. Но у
такого разрыва есть цена, и она видна в трёх местах:

- **Есть ДВЕ «материальные позиции», и они расходятся.** Номинальная
  (`kt_model::clip_material_at`, `src/libs/kt-model/src/project.rs`) —
  домен автоматизации, ось окна FX, арифметика тримов; фактическая
  (`clip_material_pos` с варпом, `src/libs/kt-engine/src/video.rs`) —
  какой кадр декодировать. На варпнутом клипе `raw_trim_start`
  (`src/libs/kt-model/src/ops.rs`) двигает `source_in` на Δ, тогда как
  реально съедено `S(Δ)`, — трим берёт не тот кадр. Записано в Р.37 как
  известный дефект вне объёма («`Split`/`TrimStart`/`punch_out_region`
  считают `source_in` линейно и на варпнутом клипе врут»).
- **Модель уже ВЕТИРУЕТ резку из-за круга.** Предикат
  `FxInstance::warps_time` стоит ПЕРВЫМ в `windows_on`
  (`src/libs/kt-model/src/project.rs`): морфер-слой не режет клип с
  варпом, «линейный `source_in` куска указал бы не туда»
  (`src/libs/kt-model/CLAUDE.md`). Полноценная ретайм-кривая расширит эту
  зону запрета со слоя на все геометрические операции — либо заставит
  научить их карте.
- **Полоска прогретости на варпнутом клипе врёт.** `coverage_runs`
  (`src/apps/kt-app/src/app.rs`) спрашивает кэш про диапазон
  `source_in .. source_in + duration`, а run рисуется линейно:
  `t0 = c.start + run.start − c.source_in`
  (`src/apps/kt-app/src/timeline.rs`). На клипе 0.5× половина
  запрошенного материала клипом вообще не читается, а прогретый участок
  выводится в первую половину события вместо всего. Р.37 С4 перевёл на
  карту варпа волноформу и филмстрип, а coverage — нет. Инварианту №4 это
  не противоречит (правда о coverage по-прежнему одна, из
  `BurstCache::coverage()`), но «пиксель → индекс бёрста» считается второй,
  линейной формулой. **Найдено при написании этой заметки, в
  `docs/porting-notes.md` не записано.**

**Ноль и минус упираются не в предусловие спан-ридера, а в другое.** Это
важно уточнить, потому что вердикт F-SPD-001 называет причиной «ключи
строго возрастают». На деле это предусловие выполняется по построению:
`wanted_frames` (`src/libs/kt-engine/src/video.rs`) не решает интеграл в
обратную сторону, а перебирает ВЫХОДНЫЕ кадры по сетке, гонит каждый через
ту же карту и в конце делает `sort()` + `dedup()` — на выходе всегда
строго возрастающий список, что бы ни творила карта. Настоящих стен три:

1. **Обратимость.** `seg_dt` делит на `r0 + √(r0² + 2·b·ds)`, «знаменатель
   ≥ r0 ≥ RATE_CLAMP.0, нулём не бывает»; при `rate = 0` функция `S`
   перестаёт быть строго возрастающей и `T = S⁻¹` не определена, а на ней
   стоит расчёт окна источника для аудио (`chain_source_window_ext`).
2. **Последовательный курсор.** `ClipProcessor` потребляет источник СТРОГО
   ПОСЛЕДОВАТЕЛЬНО через внутренний курсор rubato — «ни пропусков, ни
   дублей» (`src/libs/kt-fx/CLAUDE.md`); ход назад означает курсор назад,
   чего у ресемплера нет, а инвариант №2 запрещает трогать FX-состояние
   откуда-либо, кроме аудио-render-потока.
3. **Зеркало — свойство ВСЕГО окна, а не куска.** Реверс события сделан как
   виртуальный форвард-поток: `fill_source`
   (`src/libs/kt-engine/src/audio.rs`) читает реальный источник зеркально
   внутри окна `[source_in, window_end)`, а цепочка FX видит обычный поток
   вперёд и о реверсе не знает; поэтому реверс и варп composeятся как
   «ВАРП, потом ЗЕРКАЛО» с якорем на потреблённом `used`. Кусочный реверс
   этим приёмом не выражается — звук пришлось бы резать на сегменты со
   своим окном у каждого, то есть заводить второй механизм рядом с
   `reversed`. Транспорт назад (`playback_rate < 0`, `reverse_frames`,
   тест 42) — тоже про весь таймлайн, а не про кусок клипа.

**Что действительно больно для префетча — не немонотонность, а разгон.**
На 32× один выходной кадр стоит 32 исходных, и декодер уходит в seek на
каждом кадре — тот класс, на котором мы уже обжигались (п.14 р.14: «при
fps проекта > fps источника он на каждом втором кадре уходил в seek +
пересоздание декодера»). При живом воспроизведении это идёт через ОДИН
MediaWorker на источник (инвариант №5) с дедлайном: не успел — тишина и
последний кадр. На замедлении, наоборот, `dedup` работает в плюс (t247d:
60 кадров вместо 120).

### Какие инварианты задевает

- **№7** (код композиции один): смешивание кадров сделано в общем коде
  плана, а не отдельно в CPU-ядре и в WGSL, и второй вид кривой обязан
  оставаться пересчётом того же плана, а не вторым путём.
- **№5** (один MediaWorker на источник) и **№4** (кэш — единственная правда
  о coverage): разгон бьёт по очереди с дедлайном, а полоска прогретости на
  варпе уже считается линейной формулой.
- **№2** (FX-состояние только в аудио-потоке) и **№11** (аудио
  байт-в-байт) — упор для нулевой и отрицательной скорости.
- **№8** (скролл/зум не порождает декода) — соблюдён: ретайм меняет только
  отображение «пиксель → ключ тумбы».
- **UI-правило 17** («парные способы одного действия ведут себя
  одинаково») — Resolve показывает цену трёх входов в одну модель: три
  разных ответа на «поедут ли соседи». Если у нас появится и полоса на
  клипе, и окно FX, это обязана быть одна кривая с одним поведением.
  **13** («не тратить место впустую») — полоса ретайма на событии
  конкурирует с линией уровня, которая там уже живёт. **20** («ничего не
  отрастает само») — про модель длины: событие не должно менять длину само.

## Оценка

| Что | Ответ |
|---|---|
| Объём | L — целый раунд; половина (кривая скорости с честным интегралом) уже сделана в Р.37 |
| Где основная работа | kt-fx (карта: кламп, hold, ветка назад), kt-model (геометрия: трим/split/резка по карте), kt-app (полоса на событии + второй вид кривой в общем виджете) |
| Формат проекта | расширение диапазона `rate` версии не требует; отдельная кривая ПОЛОЖЕНИЯ как второе хранилище — да, `KT_META_VERSION` 4 → 5 (`src/libs/kt-otio/src/meta.rs`). Обменного представления у кривой нет: OTIO знает только `LinearTimeWarp{time_scalar}` и `FreezeFrame` |
| Тесты, которые придётся написать | трим варпнутого клипа берёт тот кадр (kt-model); coverage-полоска по карте варпа (kt-app `layout_tests.rs`); второй вид кривой = производная первого, правка в любом виде даёт одни и те же ключи (kt-app); hold не ломает `T = S⁻¹` (kt-fx `integral_map.rs`); сегмент назад ↔ звук (kt-engine `reverse` + `fx`); префетч == прочитанное на рывковой кривой (расширение t247d) |
| Зависит от | F-SPD-001 (постоянная скорость и модель длины), F-SPD-003 (достройка кадров), F-AUT-001 (безье), F-AUT-002 (домен ключа) |
| Ломает совместимость | нет, пока не заводится вторая кривая как отдельное хранилище |

## Вердикт

Берём, но не целиком и не одним куском. Половина уже стоит с Р.37 — кривая
скорости с честным интегралом, общая у пары A/V; ближайшая работа не новая
фича, а ДОЛГИ этой половины: трим, split и резка слоем, считающие
`source_in` линейно, и coverage-полоска, рисующая прогретый участок не
туда.

Второй кривой отдельной сущностью не заводить: у нас один виджет
автоматизации и один домен ключа, а вторая кривая того же смысла — прямое
нарушение п.17 чек-листа. Правильная цель — путь Resolve без его беды:
ОДИН набор ключей и второй ВИД (кадр вместо процента) как пересчёт. Тогда
не возникает вопроса «пересчитываются ли ключи при переключении», а
асимметрия прав («ноль и минус рисуются только в виде положения»)
перестаёт быть архитектурой и становится следствием клампа, который можно
снимать сознательно и по частям.

Hold и участок назад — отдельный раунд с честной ценой: они ломают
обратимость карты, последовательный курсор ресемплера и пооконное зеркало
реверса, а не «требуют кнопки». Ссылка F-SPD-001 на предусловие
«ключи строго возрастают» как на причину отказа неверна: оно выполняется
по построению (`wanted_frames` сортирует и дедуплицирует), и решение
должно опираться на три стены выше.

**Раунд состоялся — Р.57 (пп.453-454), по прямой претензии владельца.**
Все три стены сняты одним решением: РАЗДЕЛИТЬ ДВЕ ОСИ — знаковую позицию
P(τ) = ∫rate (адрес чтения) и накопленное потребление A(τ) = ∫|rate|
(курсор ресемплера, монотонен по построению).

1. *Обратимость* — обращать надо было НЕ ТО. Инверсия нужна двум местам
   (тримы растянутого клипа), и там спрашивают «докуда дотянуть край», то
   есть A, а не P. `output_time_for_consumption` обратима всегда, кроме
   плато нулевой скорости, где правило названо: левый край плато.
2. *Последовательный курсор* — он и остался последовательным: рубато ест
   виртуальный поток со скоростью |rate|, а направление живёт в АДРЕСЕ
   (раны постоянного знака, внутри рана ±1 на кадр).
3. *Пооконное зеркало* — «Реверс» остался свойством всего события и
   применяется ПОВЕРХ карты, вокруг ПРОЛЁТА позиции (min P, max P).

Ноль стал стоп-кадром (видео держит кадр, звук молчит, реверб
дозванивает); диапазон −32…32. Байтовая неизменность прежних проектов
доказана замороженным оракулом в тесте, а не надеждой. Осталось за
рамками: второй ВИД кривой (кадр вместо процента) — он и был главной
мыслью этой заметки и от знака не зависит.

## Источники

- Adobe After Effects CS5 Help (архивный официальный PDF, разделы
  «Time-remapping», «Time-remap audio pitch», «In/Out points»):
  https://help.adobe.com/archive/en_US/aftereffects/cs5/after_effects_cs5_help.pdf
  — два начальных ключа «with vertical time values equal to their
  horizontal position on the timeline», «an upward angle indicates forward
  playback, while a downward angle indicates reverse playback», freeze
  плоским участком, ретайм звука с плывущим питчем (дата обращения
  2026-09-07)
- Adobe Premiere Pro CS5 Help (архивный официальный PDF, разделы про Time
  Remapping):
  https://help.adobe.com/archive/en_US/premierepro/cs5/premierepro_cs5_help.pdf
  — расщепление ключа скорости и «blue curve control» в серой зоне,
  «palindrome reverse» через Ctrl-drag, «hold keyframes» через
  Ctrl+Alt-drag, «for reference only. You cannot edit these values directly
  there», «pushes remaining frames into the tail of the lengthened clip»,
  «the audio … remains at 100 % speed» (дата обращения 2026-09-07)
- DaVinci Resolve 20 Reference Manual (гл. 51, стр. 1071–1082) и DaVinci
  Resolve 21 Reference Manual (гл. 58, стр. 1254–1264; гл. 60, стр. 1305):
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf
  и https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
  — «1:1 correspondence», Retime Frame как кривая положения, запрет реверса
  на Speed-кривой (в 20 напечатан с ошибкой, в 21 исправлен), безье-изинг
  ретайм-ключей (дата обращения 2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/edit — «graphical
  curve editor for both frame position and playback speed» (дата обращения
  2026-09-07)
- https://support.apple.com/guide/final-cut-pro/retime-clips-verf4d4b1b46/mac
  и https://support.apple.com/guide/final-cut-pro/add-speed-transitions-ver41a2cd3ae/mac
  — состав раздела ретайма FCP (ни одной статьи про кривую или кейфреймы
  скорости) и «a shaded bar on either side of the border between speed
  segments» (дата обращения 2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/event_envelopes.htm
  и https://help.magix-hub.com/video/vegas/23/en/content/topics/3-projects/hfr-clips.htm
  — velocity как огибающая СОБЫТИЯ, −100…+300 %, «duration that is not
  changed by velocity envelopes», «the event's audio will not play in sync
  with the video» (дата обращения 2026-09-07)
- Треды Adobe Community и Creative COW, перечисленные в разделе «Что
  говорят пользователи» (даты там же); про инвертированные проценты AE и
  Premiere — Navarro Parker, Creative COW, 25.10.2013,
  https://creativecow.net/forums/thread/converting-between-ae-premiere-speed-ramps/
- https://opentimelineio.readthedocs.io/en/latest/tutorials/otio-serialized-schema.html
  — `LinearTimeWarp.time_scalar`, `FreezeFrame`; кривой ретайма в схеме нет
  (дата обращения 2026-09-07)

**Оговорки.** `helpx.adobe.com` (текущая справка Premiere/AE) из этого
окружения не открывается вовсе, поэтому официальные формулировки Adobe
взяты из АРХИВНЫХ PDF редакции CS5/CS5.5 на `help.adobe.com`. Механика
(расщепление ключа, палиндромный реверс, два ключа Time Remap, value/speed
graph) подтверждается ветками сообщества 2024–2026 гг., процитированными
выше, но названия пунктов меню и модификаторы мыши в версии 26 могли
измениться — вход в Premiere 26 сегодня идёт через FX-бейдж клипа
[НЕ ПРОВЕРЕНО по первоисточнику v26]. Поведение соседних клипов в v26
тоже не подтверждено: справка CS5 говорит «обрезает хвост», а пользователь
в апреле 2026 пишет «pushes everything on your timeline around».
`forum.blackmagicdesign.com` закрыт WAF-челленджем целиком, поэтому по
Resolve живого фидбека нет — только первоисточник. Справка VEGAS взята
версии 23 (последняя онлайн; каталоги `/2026/` отдают 404), числовые
пределы для 2026 могли сдвинуться.
