---
id: F-AUD-002
title: Динамическая обработка — компрессор, лимитер, гейт, экспандер
status: сделано
priority: B
cost: M
category: audio/processing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: да
crates: [kt-fx, kt-model, kt-engine, kt-app, kt-media]
invariants: [1, 2, 3, 5, 6, 11]
ui_rules: [13, 17, 20]
round: "Р.48"
point: 338
tests: [322, 326, 334, 335, 336, 341]
depends_on: [F-AUT-002, F-CLR-006]
trackers: [kde-bugzilla, kde-invent, olive, openshot, mlt, pitivi]
checked: 2026-09-14
---

# F-AUD-002 — Динамическая обработка — компрессор, лимитер, гейт, экспандер

## Суть

Один и тот же приём: громкость правит не рука, а сам сигнал. Детектор меряет
уровень, кривая «вход → выход» решает, на сколько дБ прижать, сглаживание
(attack/release) решает, как быстро. Компрессор прижимает то, что ГРОМЧЕ
порога; гейт и экспандер додавливают то, что ТИШЕ его; лимитер — тот же
компрессор, но с обещанием «выше потолка не выпущу ни одного сэмпла».
Монтажёру это видно как «реплики не прыгают от плана к плану», «шум в паузах
между фразами исчез» и «экспорт не клиппит».

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Track Compressor · Track Noise Gate · Graphic Dynamics · Wave Hammer · Multi-Band Dynamics | обычные плагины в цепочке, а цепочек несколько уровней: «Audio effect settings are displayed when you click the button on a track, bus, or assignable FX control» — плюс уровень события (Event FX, F-MPH-005). Дорожечный плагин действует на всё: «Track-level DirectX or VST plug-ins are applied to every event on the selected track». Параметры дорожечных/шинных/assignable-эффектов автоматизируются огибающими. [НЕ ПРОВЕРЕНО: уровни цепочек — по официальной справке, а ИМЕНА приборов собраны из форумных постов и вики; поимённого списка аудио-плагинов в справке Vegas 22/23 найти не удалось, состав в линейке 2026 не сверялся] |
| Adobe Premiere | 26.x | эффекты Dynamics / Hard Limiter / DeEsser + панель Essential Sound с блоком Ducking | эффекты — живые, в стеке клипа или дорожки; а ducking живой обработкой НЕ является: кнопка Generate Keyframes вешает на музыкальный клип эффект громкости и пишет в него ключи. Перегенерация затирает прежние: «Each time you click Generate Keyframes, existing keyframes are replaced with new keyframes based on the current ducking settings» [НЕ ПРОВЕРЕНО первоисточником: helpx.adobe.com из этого окружения не открывается, текст взят из поискового сниппета] |
| DaVinci Resolve | 21 | Dynamics в канальной полоске (Expander/Gate + Compressor + Limiter) + Ducker в Track FX | динамика — не плагин, а ЧАСТЬ канала: «All channel strips include a fader…, along with dynamics and a 6 band parametric equalizer on every channel», и «Double clicking the dynamics area will open the dynamics window featuring an expander/gate, compressor and limiter dynamics processing». Дакинг — живой сайдчейн: «The track FX ducker plug-in uses the signal from one or more dialogue tracks to automatically control volume reduction on a target track» |
| Final Cut Pro | 12.3 | набор Logic-эффектов в группе «Levels»: Compressor, Limiter, Adaptive Limiter, Expander, Noise Gate, Multipressor, DeEsser 2, Enveloper, Spectral Gate, Surround Compressor | эффекты на клипе/роли; шин и дорожек как понятия нет (F-MPH-005). У гейта есть и lookahead, и внешний ключевой вход: «When no external side chain is selected, the input signal is used as the side chain» |
| After Effects | 26.x | нет | в списке аудио-эффектов только Backwards, Bass & Treble, Delay, Flange & Chorus, High-Low Pass, Modulator, Parametric EQ, Reverb, Stereo Mixer, Tone — ни компрессора, ни лимитера, ни гейта [НЕ ПРОВЕРЕНО первоисточником: страница списка на helpx недоступна, состав собран из сниппета] |

## Фишка фишки

**1. Лимитер обязан видеть будущее, поэтому он не эффект, а ЗАДЕРЖКА, и
производитель пишет это прямым текстом.** Компрессор реагирует ПОСЛЕ того, как
пик уже прошёл через него: пока детектор поднял управляющий сигнал, первые
миллисекунды пика вышли наружу нетронутыми. Настоящий потолок так не держится,
поэтому у лимитера появляется lookahead — «Adjust how far ahead (in
milliseconds) Limiter analyzes the audio signal. This enables it to react
earlier to peak volumes by adjusting the amount of reduction» (Apple, справка
Limiter). Плата названа там же: «Lookahead causes latency, but this has no
perceptible effect when you use Limiter as a mastering effect on prerecorded
material», а у Adaptive Limiter Apple доходит до инструкции по эксплуатации:
«Using Adaptive Limiter adds latency when the Lookahead parameter is active…
Bypass Adaptive Limiter while recording». Ключевая тонкость, которую этот
абзац прячет: **задержка — это НЕ хвост.** Хвост (реверб) продлевает КОНЕЦ
события и ничего не двигает; задержка сдвигает ВЕСЬ выход эффекта, то есть
рассинхронизирует эту дорожку и с видео, и с необработанными соседями. В DAW
это лечит plugin delay compensation; в NLE PDC либо нет, либо он частичный и
завязан на режим шинной маршрутизации — по обсуждениям Fairlight компенсация
трековых инстансов работает в Flexi Bus и не работает в Fixed Bussing [НЕ
ПРОВЕРЕНО: forum.blackmagicdesign.com в этой сессии отдаёт 403, факт взят из
поискового сниппета треда 145413].

**2. У гейта не один порог, а два, и это единственное, что отличает гейт от
«тишины по условию».** Hysteresis: «Set the difference (in decibels) between
the threshold values that open and close the gate» (Apple, справка Noise
Gate). С одним порогом гейт на шумовой полке дребезжит — открывается и
закрывается десятки раз в секунду на одном и том же шуме; вторым порогом
(ниже первого) решение делается устойчивым. Там же живёт Hold — «Set the time
the gate remains open after the signal falls below the threshold»: гейт нельзя
закрывать сразу, иначе он режет хвосты слов. Ни гистерезиса, ни hold нет ни в
одном маркетинговом списке, а без них гейт непригоден к речи.

**3. Сайдчейн и «автоматический дакинг» — это ДВА РАЗНЫХ решения одной задачи,
и они расходятся не качеством, а архитектурой.** У гейта FCP детекторный вход
— отдельный сигнал, который просто по умолчанию замкнут на основной: «When no
external side chain is selected, the input signal is used as the side chain».
Ducker в Resolve — тот же принцип на уровне дорожек: «uses the signal from one
or more dialogue tracks to automatically control volume reduction on a target
track». Premiere пошёл другой дорогой: анализ + СГЕНЕРИРОВАННАЯ ОГИБАЮЩАЯ.
Механику видно из чужого бага изнутри — «an amplify audio effect is added to
the music clip, and no keyframes are generated» (то есть в норме фича именно
вешает эффект громкости и пишет в него ключи). Разница решающая: огибающая
правится рукой, ничего не задерживает, не требует у эффекта второго входа и
считается один раз — но она СНИМОК, и после переноса музыки или перемонтажа
реплик она остаётся старой. Живой сайдчейн всегда актуален, но требует, чтобы
у эффекта был второй вход, а у проекта — шины и посылы. Это ровно тот же упор,
об который встала F-CLR-006 («вход ключа — полноценный сигнал, а не параметр
внутри эффекта»), только пришли к нему с аудио-стороны.

## Плюсы

- Детектор и гейн-каскад разделены, поэтому компрессор, экспандер и гейт — это
  ОДИН DSP с кривой, загнутой в разные стороны от порога: одно состояние, один
  набор ручек, три эффекта в каталоге. Лимитер — тот же объект с ratio → ∞ и
  включённым lookahead.
- Lookahead не «улучшает звук», а меняет причинность: снижение усиления
  начинается ДО пика, поэтому потолок держится без искажения фронта. Именно
  поэтому лимитер без lookahead и лимитер с ним — разные приборы, а не
  настройка.
- Динамика в канальной полоске (Resolve) стоит на фиксированном месте цепочки:
  её нельзя случайно затащить после плагина, и «почему тихо» ищется в одном и
  том же месте на любом канале.
- Дакинг огибающей даёт монтажёру объект, который он умеет читать и править:
  это обычные ключи громкости, а не непрозрачная реакция прибора. Он же не
  вносит задержки и не требует пересчёта при экспорте.
- Гистерезис и hold делают решение гейта устойчивым к шуму того же уровня, что
  и порог, — без них гейт не применим к речи вообще.

## Минусы

- Lookahead платится задержкой, и производитель советует ОТКЛЮЧАТЬ прибор при
  записи. Хост, у которого нет компенсации, тихо разъезжает обработанную
  дорожку с остальными; хост, у которого она есть, обязан отложить весь микс на
  максимум по цепочкам — то есть платить задержкой мониторинга за один прибор
  на одном клипе.
- Peak против RMS — источник вечного «компрессор не работает, я же вижу пик».
  RMS-детектор законно не реагирует на короткий выброс: он меряет энергию за
  окно интеграции. Ручки при этом одинаковые, и по ним разницу не видно.
- «Музыкальность» компрессора — это форма колена (knee) и время интеграции
  детектора, а не набор параметров. Два прибора с одинаковыми threshold/ratio/
  attack/release звучат по-разному, и сравнить их по списку ручек нельзя.
- Программно-зависимый release у «аналоговых» эмуляций делает результат
  функцией материала: тот же клип, обрезанный на полсекунды раньше, даёт другую
  огибающую. Для NLE с побайтовым эталоном рендера это прямой конфликт.
- Сгенерированная огибающая устаревает молча. Перегенерация затирает ручные
  правки целиком, а не сливается с ними.
- Автоматика ломается на состояниях, о которых не думали: у Premiere ducking
  не генерирует ключи, когда целевой клип соло (подтверждено Adobe как баг,
  см. цитаты), а у части пользователей роняет приложение.
- Динамика на уровне дорожки/шины невидима на клипе: смотришь на событие и не
  понимаешь, почему оно тише, чем в бине.
- Сайдчейн стоит не в приборе, а в ХОСТЕ: Olive умеет загрузить чужой
  sidechain-компрессор, и он всё равно бесполезен, потому что направить в него
  сигнал другой дорожки нечем (Olive #1753). Прибор без адресации второго входа
  — это не половина фичи, а ноль.
- Приборы динамики ломаются не алгоритмом, а стыком с трактом. У Kdenlive из
  четырёх задач про существующую динамику две подряд — «Cannot set sink channel
  counts» у Compressor/Expander (#489276, #490884): компрессор обязан
  договориться о числе каналов, и это отказывает чаще, чем сам DSP.
- Цена динамики взимается разрядностью цепочки, а не эффектом: у MLT звук
  внутри цепочки молча сводился в 16 бит, и та же самая компрессия звучала
  «грязно» в Kdenlive и чисто в Audacity (#1284). Прибор нельзя оценивать в
  отрыве от точки суммирования.
- Лимитер просят не как эффект на клипе, а как свойство ЭКСПОРТА («чтобы
  экспорт не клиппил») — то есть просьба приходит на уровень, которого у
  редактора может не быть вовсе.

## Что говорят пользователи

> the 'generate keyframes' button very briefly turns gray, an amplify audio
> effect is added to the music clip, and no keyframes are generated.
> — Logan_Barney5958, Adobe Support Community, 02.07.2025, https://community.adobe.com/t5/premiere-pro-bugs/essential-sound-ducking-won-t-generate-keyframes-when-target-clip-is-soloed/idc-p/15405003

> When I press 'Generate Keyframes', PPro crashes.
> — Stanley Arthur, там же, 03.07.2025, https://community.adobe.com/t5/premiere-pro-bugs/essential-sound-ducking-won-t-generate-keyframes-when-target-clip-is-soloed/idc-p/15405003

> I've checked with the team, and you are correct; it does appear to be a bug.
> I've written it up for our engineers.
> — Rach McIntire, Adobe Community Manager, там же, 07.07.2025, https://community.adobe.com/t5/premiere-pro-bugs/essential-sound-ducking-won-t-generate-keyframes-when-target-clip-is-soloed/idc-p/15405003

> I'd start with audio compression. I tend to use the Track Compressor plug-in,
> but you can also do compression via the Graphic Dynamics plug-in.
> — Brett Underberg-davis, Creative COW, 13.06.2009, https://creativecow.net/forums/thread/audio-difficulties/

> You can use the Track EQ to create a "notch" on the music track to keep the
> music from stomping on the VO.
> — Bob Peterson, Creative COW, 05.08.2010, https://creativecow.net/forums/thread/audio-compression-howae/

Читать это надо так. Первые три цитаты — не про «плохой алгоритм», а про то,
что автоматический дакинг у Premiere есть ОТДЕЛЬНАЯ СТАДИЯ с собственным
состоянием: она умеет не сработать, затереть чужие ключи и уронить хост, при
том что живой сайдчейн в этих местах ломаться нечему. Последняя цитата ценна
обратным: в треде «как приглушить музыку под голосом» опытный человек 2010 года
советует вырезать эквалайзером НОТЧ вместо приседания — потому что механизма
приседания у Vegas нет до сих пор, и обходят его вручную.

Живого отрицательного фидбека по Resolve Ducker и по задержке лимитера собрать
не удалось (форум BMD в этой сессии отдавал 403), зато трекеры опенсорсных
редакторов отвечают на тот же вопрос счётом — и отвечают резче, потому что там
видно не мнение, а состояние задачи.

Мастер-лимитер просят не как эффект, а как свойство ЭКСПОРТА:

> I really miss a master limiter effect that'd allow to export videos without
> clipping in case something gets too loud.
> — Olive #2064, 14.10.2022, закрыта,
> https://github.com/olive-editor/olive/issues/2064

Живой сайдчейн, у которого нет адресации второго входа, бесполезен вместе со
всем плагином — и это ровно тот упор, о который встала F-CLR-006:

> However, there is no way to direct the sidechain from one of the other audio
> tracks into the plugin, thus rendering it useless.
> — Olive #1753, 29.09.2021, закрыта,
> https://github.com/olive-editor/olive/issues/1753

Приборы динамики просят там, где их нет, ровно теми же словами, что и в
форумных тредах 2010 года, — только с готовым обходом через чужую программу:

> I miss a great feature in all NLE about audio, is the audio "autoduck"
> functionality. […] Is available in Audacity but I need to do the process OUT
> of the video editor and incorporate the result to the project.
> — OpenShot #2813, 29.05.2019, закрыта,
> https://github.com/OpenShot/openshot-qt/issues/2813

А там, где дакинг делают руками, он делается ИМЕННО огибающей — то есть тем
путём, который у нас дешевле всего:

> This way the super frequent performed manual audio ducking with keyframes, to
> lower e.g. music when a speaker is speaking […] would benefit from a massive
> workflow speed boost […]
> — Kdenlive (kde-invent) #1958, 27.01.2025, открыта,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1958

Уровень применения — отдельная просьба, и звучит она про динамику дословно:

> It would be very useful to be able to add effects on full layers for instance
> using one track for chroma-keying or compressor /limiter on audio track etc
> and not worry about adding effect to all clips in same track.
> — Pitivi #2472, 19.05.2020, открыта,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2472

И цена, которую платит тракт, а не алгоритм (инвариант №1 нашего движка — про
это буквально):

> I'd noticed that Kdenlive was producing quite rough audio output when applying
> effects (e.g. fade out / compression). The same compression algorithm was
> sounding clean on Audacity.
> — MLT #1284, 29.08.2026, закрыта,
> https://github.com/mltframework/mlt/issues/1284

СИГНАЛ: приборы динамики — 6 задач в kde-bugzilla, `auto.?duck|\bducking\b|\blimiter\b|\bcompressor\b|noise gate|side.?chain`; доля 0.1% против 0.2% в olive

Число мелкое, и мелкое оно честно: тема «звук» — 2522 задачи на одиннадцать
трекеров рейтинга, динамика внутри неё это 12 задач, то есть 0.5 % темы. Находка не в
величине, а в РАЗНИЦЕ СОСТАВА при вдвое большей доле там, где прибора нет. У
Olive (динамики нет вовсе) все три задачи — «дайте»: лимитер, автодакер,
сайдчейн. У Kdenlive (динамика есть, через MLT/avfilter) две задачи — просьбы
про автодакинг (одна из них, #392623, висит CONFIRMED с 2018 года), а
остальные четыре — поломки уже существующего: компрессор «explodes when
attributes changed» (#403650), дважды подряд «Cannot set sink channel counts»
у Compressor/Expander (#489276 и #490884) и искажение на целом наборе приборов,
включая лимитер (#513447). То есть класс ведёт себя не как «редкая экзотика», а
как «пока прибора нет — просят, как только появился — ломается на стыке с
трактом, а не в самом DSP». Именно поэтому единственная жалоба про звучание
(MLT #1284) оказалась не про алгоритм, а про разрядность цепочки.

## Как ложится на Kinotrain
CAPABILITY: openspec/specs/audio-fx/ — контракт в `spec.md`, «почему» и пути в коде в `design.md`.
Компрессор и лимитер — узлы аудио-цепочки с упреждением (PDC: вход обгоняет выход), без мастер-лимитера проекта.
АДРЕС: `src/libs/kt-engine/src/player.rs:375` — класс MLT #1284 («цепочка молча свелась в 16 бит, и компрессия звучит грязно при чистом алгоритме») у нас невозможен, потому что инвариант №1 держит f32/48k/2ch от декода до выхода, и любое отступление ловится побайтовым golden_export. А вот класс Olive #2064 («экспорт клиппит, и положить лимитер некуда») возможен, и его не ловит ничто: единственный объект с именем «мастер» — это МОНИТОРНЫЙ фейдер превью, `Player::set_master_gain` с комментарием «Мастер-громкость превью» (`player.rs:375`), живущий в атомике для cpal-колбэка; экспорт собирает тот же `AudioRenderer` вообще без него (`src/libs/kt-engine/src/export.rs:81`, `:103`). То есть в модели нет НИ ОДНОГО уровня, общего для превью и экспорта, куда лимитер защиты от клиппинга можно было бы приложить, — а golden_export стережёт байты того, что мы посчитали, и про существование потолка не знает. Именно поэтому мастер-лимитер в «Оценке» стоит L, а не M: цена не в DSP.

## Оценка

| Что | Ответ |
|---|---|
| Объём | M — 2–3 стадии: (1) дакинг огибающей, (2) компрессор+лимитер+гейт одним семейством, (3) разведение пре-ролла и хвоста. Мастер-лимитер поднимает до L (нужен уровень микса), живой сайдчейн — до XL (шины + формат) |
| Где основная работа | kt-fx (DSP + пре-ролл), kt-model (вариант `FxInstance` и строка в `params_for!`), kt-engine (`chain_tail` → два числа, окно префетча), kt-app (`fx_registry.rs`, i18n ×5, сабсеты шрифтов) |
| Формат проекта | версия не двигается: новый вариант `FxInstance` — аддитивная правка того же класса, что `Gain.pan` (Р.33). Чужой/незнакомый `kt:`-эффект уже не роняет загрузку, а сохраняется как foreign с notice (`src/libs/kt-otio/src/convert.rs:1214`) |
| Тесты, которые придётся написать | байт-детерминизм компрессора через seek/пре-ролл (инвариант №11); компрессор НЕ продлевает `ext_end` (клип не звучит после конца); потолок лимитера держится на синтетическом пике; lookahead не сдвигает выход (сэмпл-точное совпадение с gain-эталоном); окно префетча покрывает lookahead — ноль underrun'ов (инвариант №5); дакинг: кривая детерминирована и строится из пик-пирамиды без декода (инвариант №8); плюс t48 (реестр), t100/t125 (глифы и каталоги), layout-тест на строку эффекта |
| Зависит от | F-AUT-002 (домен ключа — для огибающей дакинга), F-CLR-006 (второй вход: тот же архитектурный упор), F-AUT-003 (если дакинг захочется на дорожку, а не на клип) |
| Ломает совместимость | нет |

## Вердикт

Берём, но двумя РАЗНЫМИ решениями, и не в одну стадию.

Первым — автоматический дакинг генерацией `ParamCurve` на `Gain.db`: он
закрывает самую частую живую претензию («музыка забивает голос»), не трогает
ни одного инварианта, не вносит задержки и почти целиком собирается из уже
готовых кусков (кривые, анализ по пик-пирамиде, виджет автоматизации).

Вторым — компрессор/лимитер/гейт одним семейством на уровне клипа, с
lookahead, реализованным подглядыванием в ИСТОЧНИК (пулл вперёд), а не
задержкой выхода; попутно `chain_tail` разводится на «сколько греть» и
«сколько звучать» — без этого компрессор либо теряет детерминизм, либо звучит
после конца клипа.

~~Живой сайдчейн — НЕ сейчас: ему нужны второй вход у эффекта, шины и правка
формата, то есть та же XL-стена, что у F-CLR-006, а практическая польза
покрывается огибающей.~~ **ОТМЕНЕНО владельцем (Р.63, подтверждено Р.65):**
«Дакинг → ЖИВОЙ САЙДЧЕЙН-КОМПРЕССОР… снимок убрать совсем». Оценка XL не
подтвердилась: шины не понадобились (ключ — ДОРОЖКА, а стадии дорожек
появились в Р.62), второй вход оказался аргументом обработки, а не новым
графом, правка формата — одно поле по содержимому (v12). Сделано change'ем
`sidechain-ducking-compressor`; снимок огибающей удалён целиком, тесты
t767–t778, живой сценарий `src/scripts/live/sidechain.py`. Мастер-лимитер — отдельный вопрос, он упирается не в
DSP, а в то, что мастер-фейдер сегодня существует только в превью
(`src/libs/kt-engine/src/player.rs:375`) и в экспорт не попадает.

## Источники

- Трекеры (дампы от 08.09.2026 в `~/.cache/kinotrain/trackers/`, в репозиторий
  не попадают — лицензии нет, см. `_trackers.md`): `kde-bugzilla` 6346 задач,
  `kde-invent` 2217, `olive` 1853, `openshot` 4516 (выгрузка ещё шла, в реестре
  4907), `mlt` 601, `pitivi` 2672. Число и доли строки `СИГНАЛ:`
  воспроизводятся `python3 src/scripts/tracker-rank.py --query
  'auto.?duck|\bducking\b|\blimiter\b|\bcompressor\b|noise gate|side.?chain'` на
  этом составе; `match()` смотрит заголовок, метки и компонент, поэтому
  совпадения только в теле задачи в счёт не идут (цитаты из тел приведены
  поимённо со ссылками).
- https://www.blackmagicdesign.com/products/davinciresolve/fairlight — Fairlight:
  динамика в канальной полоске, Ducker как Track FX (дата обращения 2026-09-07)
- https://support.apple.com/en-lamr/guide/final-cut-pro-logic-effects/lgex931a007e/mac —
  Limiter: отличие от компрессора, Lookahead, латентность (2026-09-07)
- https://support.apple.com/en-au/guide/final-cut-pro-logic-effects/lgex931a149c/mac —
  Adaptive Limiter: «adds latency when the Lookahead parameter is active»,
  Out Ceiling, True Peak (2026-09-07)
- https://support.apple.com/guide/final-cut-pro-logic-effects/noise-gate-controls-lgex931a1816/mac —
  Noise Gate: Hysteresis, Hold, Lookahead, Side Chain; там же список группы
  «Levels» целиком (2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/9-audio/audio_plug-ins.htm —
  уровни цепочек Vegas: track / bus / assignable FX (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/9-audio/adding_audioeffects.htm —
  дорожечный плагин действует на все события дорожки; огибающие параметров
  (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-bugs/essential-sound-ducking-won-t-generate-keyframes-when-target-clip-is-soloed/idc-p/15405003 —
  баг ducking'а, механика «эффект громкости + ключи» изнутри, подтверждение
  Adobe (2026-09-07)
- https://creativecow.net/forums/thread/audio-difficulties/ — набор плагинов
  динамики Vegas глазами практика (2026-09-07)
- https://creativecow.net/forums/thread/audio-compression-howae/ — пресеты
  Track Compressor / Wave Hammer, обход дакинга через notch (2026-09-07)

Помечено `[НЕ ПРОВЕРЕНО]` и почему: (1) формулировка Adobe про Generate
Keyframes и (2) состав аудио-эффектов After Effects взяты из поисковых
сниппетов — `helpx.adobe.com` из этого окружения не открывается («proxy refused
the connection»); (3) поведение PDC у Fairlight в Flexi Bus против Fixed
Bussing — из сниппета треда forum.blackmagicdesign.com t=145413, сам форум
отдавал 403 на всех проверенных URL. Строки Premiere и After Effects в таблице
«У кого и как» опираются на официальное сообщество Adobe и сниппеты
официальной справки, живьём не проверялись.

**Реализовано в Р.47-Р.48** — компрессор и гейт в Р.47 (п.329),
look-ahead лимитер и автодакинг в Р.48 (пп.337-338, 341). Живой
сайдчейн и мастер-лимитер остаются вне сделанного — причины в разделе
«Что мешает» выше; дальнейшая жизнь фичи — docs/porting-notes.md.
