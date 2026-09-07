---
id: F-MSK-004
title: Хромакей: показ матte, despill и обработка края
status: кандидат
priority: B
cost: M
category: masks/keying
editors: [after-effects-26, premiere-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: частично
crates: [kt-fx, kt-gpu, kt-model, kt-app]
invariants: [7, 11]
ui_rules: [4, 9, 13, 20]
round: null
point: null
tests: []
depends_on: [F-MSK-001, F-MSK-003]
checked: 2026-09-07
---

# F-MSK-004 — Хромакей: показ матte, despill и обработка края

## Суть

Актёр снят на зелёном фоне; надо, чтобы фон исчез, а актёр остался. Выбор
цвета пипеткой — это первые пять секунд работы. Дальше начинается сама
работа: волосы полупрозрачные, на тёмной куртке зелёная кайма, в дыму
дырки, у левого края в кадр лезет стойка от лампы, а после кея всё
это надо посмотреть НЕ как картинку, а как матte — чёрно-белую карту
прозрачности, потому что серую грязь на чёрном фоне глаз не видит, а
композиция её покажет. Профессиональный кей — это не «убрать цвет», а
цикл: посмотрел матte → добил чёрное в чёрное и белое в белое → поджал
или отпустил край → убрал засветку с кожи и волос → отрезал маской то,
что кею не по зубам.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| After Effects | 26.x | Keylight (The Foundry) + Advanced Spill Suppressor + Key Cleaner | эталон отрасли, лицензированный у чужой компании. Меню **View**, в котором кроме Screen Matte и Final Result есть Inside Mask, Outside Mask, Combined Matte, Intermediate Result и **Status** (полный перечень пунктов именно в версии для AE — **[НЕ ПРОВЕРЕНО]**, документация Foundry описывает узел Nuke). Отдельная группа Screen Matte: Clip Black / Clip White / Clip Rollback / Screen Dilate (с субпиксельной точностью) / Screen Softness / Screen Despot Black и White. Despill управляется **Screen Balance** и Despill Bias, а не одним ползунком. Два входа масок: **Inside Mask** (holdout — «firm up the foreground matte to stop print through») и **Outside Mask** (garbage — «a garbage mask to reveal the background»). Replace Method решает, ЧЕМ заполнить место, откуда вычли зелёный |
| Adobe Premiere | 26.x | Ultra Key | наследник отдельного продукта Adobe Ultra (Serious Magic): по википедии, «a plugin labeled "Ultra" is now present in Adobe Premiere Pro. It is a powerful but simplified keying solution». Меню **Output**: Composite / Alpha Channel (в терминах Adobe матte и есть «альфа-канал»). Три группы ручек: Matte Generation (Transparency и др.), Matte Cleanup (Choke, Soften), Spill Suppression (Desaturate). Garbage-маска отдельным входом НЕ приходит — её роль играет маска самого эффекта (общий для всех эффектов Premiere механизм: эллипс/многоугольник/кисть с трекингом) |
| DaVinci Resolve | 21 | **два разных кея**: Qualifier → 3D Keyer (страница Color) и Delta Keyer (страница Fusion) | на Color кей — часть механизма ВЫДЕЛЕНИЯ для коррекции: мазки по кадру, доводка в Matte Finesse, просмотр выделения через Highlight (**[НЕ ПРОВЕРЕНО]** по официальному руководству; официальная страница BMD про Color говорит лишь, что «the qualifier palette lets you select and adjust part of an image based on hue, saturation or luminance»). На Fusion — настоящий цветоразностный кей: «The pre-matte, matte, fringe and tuning tabs give you a complete set of controls to refine the key until it's perfect!», плюс узел **Clean Plate** («especially useful for clips with reflections, hair and smoke») и отдельные входы Garbage Matte и Solid Matte прямо на узле. Матte смотрится переключателем View Mode / каналов вьюера |
| Final Cut Pro | 12.3 | Keyer (в справке — Green Screen Keyer) | единственный из пяти, кто ставит ключ САМ при добавлении эффекта (автосэмпл); ручка ровно одна — Strength («Adjusts the tolerance (core transparency) of the automatic sampling»), остальное прячется в Refine Key. Три кнопки вида: Composite / **Matte** («displays the matte created by the chroma keyer, making it easier to identify areas not keying well») / Original. Уточнение — не числами, а ЖЕСТАМИ по картинке: Sample Color рисует прямоугольники, Edges — линию поперёк проблемного края. Despill включён по умолчанию: «By default, the Green Screen Keyer effect applies spill removal, which neutralizes any leftover tinting». Есть Light Wrap |
| VEGAS Pro | 2026.0 | Chroma Keyer (штатный OFX) + Chroma Blur; BCC Chroma Key Studio / Primatte Studio в старших редакциях | самый простой из пяти: цвет + Low threshold + High threshold + Blur amount — ровно наша схема «порог и мягкость» (набор параметров подтверждается описаниями настроек у пользователей). Тяжёлые случаи в Vegas не чинят штатным кеем, а меняют инструмент: либо BCC/Primatte, либо вообще не кей — Secondary Color Corrector и Channel Blend. Наличие в штатном Chroma Keyer чекбокса показа матte по первоисточнику подтвердить не удалось — **[НЕ ПРОВЕРЕНО]** |

## Фишка фишки

**1. У Keylight есть режим показа, который СОЗНАТЕЛЬНО ВРЁТ, и именно он
делает кей настраиваемым.** Все пятеро умеют показать матte как есть
(Premiere — Output → Alpha Channel, FCP — кнопка Matte, Fusion — View
Mode, аппаратный Ultimatte — «combined matte view, internal matte view»),
но честный матte бесполезен ровно там, где кей и разваливается: разница
между альфой 0 и альфой 6 на чёрном фоне не видна человеку, а на
композиции это «грязный фон», который проступит на светлом заднике.
Поэтому у Keylight отдельно от Screen Matte есть **Status**: «view an
exaggerated view of the key so that minor problems are shown clearly» —
почти-чёрное и почти-белое подкрашиваются так, чтобы глаз ЗАМЕТИЛ, то
есть на экран выводится не то, что будет отрендерено. Это переворачивает
роль вьюера: он перестаёт быть окном в результат и становится
измерительным прибором, у которого своя шкала. Отсюда же следует, что
режим показа обязан быть у САМОГО КЕЯ, а не у окна превью: «Screen Matte»
— это матte именно этого узла до подмешивания масок, а «Combined Matte» —
после; окно превью такой разницы знать не может.

**2. «Толерантность» — не число, а УРАВНЕНИЕ, и у Keylight его коэффициент
вынесен наружу отдельной ручкой Screen Balance: «blue screens tend to
work best with a balance of around 0.95 and green screens with a balance
of around 0.5».** То есть кей меряет не расстояние до цвета, а
цветоразностную величину вида «ключевой канал минус смесь двух других», и
пропорция этой смеси зависит от того, синий экран или зелёный. Практическое
следствие, которое видно только когда поработал: одна и та же цифра
допуска на синем и на зелёном экране означает разный физический смысл, и
кей, у которого баланса нет, на синем экране всегда будет хуже, чем на
зелёном, — не из-за настроек оператора, а по устройству. У Keylight
дополнительно разведены Screen Balance (влияет на МАТТE) и Despill Bias
(влияет на ЦВЕТ, и подбирается, по документации, по волосам и тону кожи):
одна и та же зелень в матte и в despill трактуется по-разному, и это
намеренно.

**3. Despill — это не вычитание, а ЗАМЕНА, и Keylight заставляет выбрать,
чем заменить.** Screen Replace / Inside Replace предлагают None, Source,
Hard Colour и Soft Colour, причём Soft Colour описан как попытка
«modulate the luminance of the resulting pixel so that it matches the
original pixel». Смысл в том, что если у пикселя просто отнять зелёный,
он не станет нейтральным — он станет ТЁМНЫМ, потому что зелёный канал
несёт большую часть яркости; светлые волосы после наивного despill
превращаются в серую паклю. Поэтому взрослый кей после вычитания
возвращает пикселю яркость — цветом фона, исходным цветом или заданным
цветом. Второй край той же мысли — что лежит под альфой: у Keylight
Final Result отдаёт ПРЕМУЛЬТИПЛИЦИРОВАННУЮ RGBA, то есть кей сам решает,
что цвет за пределами матte больше не существует, и никакая последующая
фильтрация не вытащит зелёный обратно в край.

## Плюсы

- Режим показа матte живёт у эффекта, поэтому в многопроходном кее
  (Keylight: Intermediate Result) видно матte КАЖДОГО прохода отдельно;
  окно превью показало бы только сумму и не сказало бы, какой из проходов
  наврал.
- Отдельные Clip Black и Clip White — это две независимые ручки на два
  независимых дефекта (грязь в фоне и дыры в объекте), и настраиваются
  они по разным местам кадра; один ползунок «контраст матte» связал бы их
  и заставил чинить одно, ломая другое.
- Clip Rollback существует именно потому, что клиппинг НЕОБРАТИМ по
  информации: у Keylight отдельная ручка «restored after clipping the
  Screen Matte too aggressively», то есть инструмент признаёт, что
  пользователь сначала переборщит, и даёт дорогу назад, не заставляя
  крутить обратно два других ползунка.
- Inside/Outside маски — ВХОДЫ узла, а не отдельные слои поверх: маска
  участвует в том же расчёте, что и кей, поэтому Combined Matte можно
  посмотреть до композита и увидеть, что маска режет как раз то, что
  надо.
- Clean Plate (Resolve) снимает главную причину неровного матte —
  неравномерно освещённый экран — ДО кея, а не подгонкой порогов после:
  один узел вместо десятка правок допуска по областям кадра.
- Автоматика FCP (кей ставится сам, одна ручка Strength) закрывает
  типовой случай в один клик, а уточнение делается ЖЕСТАМИ по картинке
  (прямоугольник, линия поперёк волос) — числовые ползунки не нужно даже
  открывать.

## Минусы

- Восемь режимов показа у Keylight — это восемь способов забыть
  переключиться обратно; у Ultra Key то же самое опаснее, потому что
  Output — параметр ЭФФЕКТА, а не окна, значит забытый Alpha Channel
  уезжает в рендер (прямым источником не подтверждено —
  **[НЕ ПРОВЕРЕНО]**, но это следует из места, где ручка живёт).
- Screen Balance «правильное» значение имеет разное для синего и зелёного
  — значит, предустановки кея нельзя переносить между экранами, и
  пресет, снятый на зелёном, на синем даёт заведомо худший результат.
- Порядок операций над матte жёсткий (clip → rollback → dilate → softness
  → despot у Keylight; pre-matte → matte → fringe → tuning у Delta Keyer)
  и в интерфейсе он выражен только ПОРЯДКОМ ГРУПП сверху вниз: перепутав
  его, пользователь получает другой результат при тех же числах, и
  документация вынуждена отдельно предупреждать («use it only after Key,
  Pre Matte/Clean Plate, Matte and Fringe are behaving»).
- Despill, который «включён по умолчанию» (FCP), незаметно меняет цвет
  объекта ещё до того, как пользователь что-то настроил: попытка потом
  подобрать цветокоррекцию под задник борется с уже применённой
  десатурацией.
- Resolve платит за два кея двумя разными словарями: у Color-страницы
  Matte Finesse, у Delta Keyer — Matte и Fringe, и это НЕ синонимы; пользователь,
  выучивший один, на другой странице начинает заново.
- Ultra Key «simplified» не по недосмотру, а по происхождению: у него нет
  ни входов масок, ни clean plate, ни multipass — сложный кадр вынуждает
  уходить в After Effects, то есть менять программу.

## Что говорят пользователи

> Change the Output menu from Composite (the final result) to Alpha
> Channel. This displays the key in shades of gray.
> — Larry Jordan, «Premiere Pro CC: Create a Green-Screen Key», 26.02.2016,
> https://larryjordan.com/articles/premiere-pro-cc-create-a-green-screen-key/

Первый же шаг настройки кея в учебнике — переключить вывод на матte;
не «если не получилось», а до всякой правки ползунков.

> I am not new to using Mocha but I am running into an issue with a scene
> where the actors hands move beyond the edge of the green screen.
> — kihlbahkt_e4833, форум Boris FX, 09.05.2016,
> https://forum.borisfx.com/t/2169

Ровно тот случай, ради которого существуют garbage/holdout-маски: кей
исправен, но экран кончился — и дальше речь уже про форму, а не про цвет.

> Well that's some pretty nasty footage - poor subject lighting, heavy
> compression and poor green screen setup.
> — Peter McAuley (Boris FX), 12.05.2020,
> https://forum.borisfx.com/t/8058

Диагноз специалиста производителя кея на вопрос «как починить края и
ореол»: три причины из трёх — на съёмочной площадке и в кодеке, ни одной
в алгоритме.

> I find that works more reliably than the chroma key Fx.
> — RogerS, форум VEGAS Pro (Boris FX), 27.04.2026,
> https://forum.borisfx.com/t/22928/3

Реплика о штатном хромакее Vegas в треде, где предлагают КЕИТЬ БЕЗ КЕЯ —
через Channel Blend и Secondary Color Corrector. Показательно, чем именно
плох простой кей «цвет + два порога»: не тем, что он не работает, а тем,
что ему не доверяют на реальном материале.

Чего найти НЕ УДАЛОСЬ и почему: reddit (r/editors, r/premiere,
r/VideoEditing) и video.stackexchange.com из этого окружения не
открываются, бюджет `WebSearch` в сессии исчерпан, а поисковые движки,
через которые ходили прошлые агенты (DuckDuckGo, Brave, Mojeek, Ecosia,
searx-зеркала), отвечали капчей, 403 или 429; Bing отдавал результаты
чужого рынка. Поэтому целевые формулировки заказчика — «Ultra Key vs
Keylight», «green screen edges look terrible», «why does my key look
better in Resolve» — прямыми цитатами не подтверждены: приведённые выше
четыре цитаты взяты с площадок, которые открылись (larryjordan.com и
форум Boris FX, где живёт и сообщество VEGAS Pro).

## Как ложится на Kinotrain

### Что уже есть

- `fn greenscreen` — `src/libs/kt-fx/src/video.rs:348`. Считает ЕВКЛИДОВО
  расстояние `d` от пикселя до ключевого цвета в u8-RGB (в f64),
  `hard = tolerance − softness`; при `d ≤ hard` альфа обнуляется, при
  `hard < d < tolerance` прежняя альфа УМНОЖАЕТСЯ на `(d−hard)/(tol−hard)`,
  дальше не трогается вовсе. `has_alpha = true` на выходе.
- Подавление засветки (Р.33 С5, п.228 `docs/porting-notes.md`): параметр
  `spill` тянет ДОМИНИРУЮЩИЙ КАНАЛ КЛЮЧА к большему из двух соседних, и
  только там, где `alpha > 0`. `spill = 0` — прежний путь байт-в-байт.
- Спека параметров — `params_for!` в
  `src/libs/kt-model/src/project.rs:2340`: `key_r/key_g/key_b`
  (0…255, Rgb8), `tolerance` (1…200, умолчание 70), `softness` (0…100,
  умолчание 25), `spill` (0…1, умолчание 0). Все шесть — `Smooth`, то есть
  автоматизируются кривыми наравне с прочими FX (`src/libs/kt-model/src/project.rs`,
  t190/t191).
- Ключевой цвет в UI берётся пипеткой, а не тремя числами:
  `src/apps/kt-app/src/fx_editor.rs:567` исключает `key_r/g/b` из
  `params_grid`. Категория эффекта — `cat::KEY`
  (`src/apps/kt-app/src/fx_registry.rs:36`).
- GPU-зеркало — `src/libs/kt-gpu/src/shaders/greenscreen.wgsl` (в f32),
  конформанс t181 гоняется с допуском `FrameTol::THRESHOLD`
  (`src/libs/kt-gpu/src/lib.rs:35`): до 0.5 % пикселей могут разойтись
  НА ЛЮБУЮ величину — это законные выбросы у жёсткой границы.
- AVX2-ядра у кея НЕТ и это решение, а не пробел: замер дал ×1.29 при
  пороге полезности 1.3 (п.198 `docs/porting-notes.md`), LLVM векторизует
  цикл сам. Значит правка формулы — это скаляр + WGSL, без третьей копии.
- Эталон — `src/libs/kt-fx/tests/video_fx.rs`: `greenscreen` стоит в
  таблице `GOLDEN` поимённо (`0x18dd7db23eb92552`) плюс свойство
  `t26_greenscreen_keys_out_green_corner`; ещё один экземпляр — в цепочках
  `simd_parity`/`golden_export`.
- Композит: FX считаются ПЕР-СЛОЙ, затем `alpha_over` со СТРАИТ-альфой
  поверх чёрного (`src/libs/kt-engine/src/video.rs:908`, план —
  `src/libs/kt-engine/src/video_plan.rs`).
- Полезное свойство, которое стоит знать: два `Greenscreen` в одной
  цепочке дают ПЕРЕСЕЧЕНИЕ непрозрачных областей (второй умножает альфу
  первого) — то есть двухцветный кей (два экрана в кадре) уже работает,
  как в приёме «Two Color Chroma-Key» у Premiere. Объединения и вычитания
  матte при этом нет никакого.

### Чего не хватает

1. **Показа матte — вообще никакого.** `Greenscreen` сразу пишет в альфу,
   а превью показывает КОМПОЗИТ: `draw_preview`
   (`src/apps/kt-app/src/app.rs:3095`) заливает результат в текстуру
   через `ColorImage::from_rgba_unmultiplied`, поэтому прозрачное место —
   это просто фон панели egui. Настроить `tolerance`/`softness` по такой
   картинке нельзя: серая грязь в фоне и полупрозрачность на плечах
   выглядят одинаково — «ничего не видно».
2. Операций над матte: clip black / clip white, shrink-grow (choke),
   размытия ТОЛЬКО альфы, удаления точечного мусора. Наш `Blur`
   (`src/libs/kt-fx/src/video.rs:332`) размывает все четыре канала
   сразу — как «блюр матte» он годится только ценой размытия картинки.
3. Despill бедный: канальный зажим без баланса (аналога Screen Balance
   нет — расстояние евклидово и симметрично по каналам), без bias по
   коже/волосам и без «чем заменить» (Replace Method).
4. Garbage/holdout-масок нет, потому что **пространственных масок нет в
   продукте вообще**: `Area` в `src/libs/kt-model/src/project.rs` — это
   ВРЕМЯ × ДОРОЖКИ, а не форма в кадре. Это территория F-MSK-001, и без
   неё половина реальных кадров (стойка лампы, край экрана, вышедшая за
   зелёное рука) не кеится в принципе. Второй возможный источник такой
   маски — альфа ЧУЖОГО клипа (F-MSK-003): у Keylight Inside/Outside —
   именно ВХОДЫ, а не собственные фигуры узла.
5. Нет clean plate (компенсации неровно освещённого экрана), light wrap и
   многопроходного кея с промежуточным результатом.

### Что мешает

- **Режим показа матte — это вывод рендера, а не состояние окна.**
  Инвариант №7: код композиции ОДИН для превью и экспорта
  (`VideoRenderer::frame_plan`), бэкенд — параметр исполнения, не второй
  путь. Значит есть ровно три честных варианта, и все три с ценой:
  (а) поле эффекта — тогда матte уедет в экспорт, как уезжает Output =
  Alpha Channel у Ultra Key (footgun, зато один код);
  (б) параметр ПЛАНА кадра, который выставляет только превью-вызов —
  ближе всего к нашей архитектуре, но это первый в проекте случай, когда
  превью и экспорт просят у одного кода разные картинки, и правило UI 20
  («ничего не отрастает и не отрубается само») требует, чтобы режим
  гасился явно и был виден;
  (в) отдельный видео-FX «показать альфу» в конце цепочки — дёшево,
  честно к №7, но не отличает матte ЭТОГО кея от суммарной альфы слоя.
- **Прозрачные пиксели у нас сохраняют исходный зелёный RGB.** Альфа
  зануляется, цвет — нет, а `spill` намеренно не трогает пиксели с
  `alpha == 0`. Всё, что после кея интерполирует кадр, работает со
  СТРАИТ-альфой поканально: `bilinear_into_scalar`
  (`src/libs/kt-fx/src/video.rs:251`, а значит `Blur`, `Pan`, `Glow`,
  `EventGrid`), `dissolve_into` и `transitions::mix_into`
  (`src/libs/kt-engine/src/transitions.rs:174`). Итог: любой блюр,
  зум или переход после хромакея затягивает зелёный обратно в край. Вот
  почему у Keylight есть Replace Method и премультиплицированный Final
  Result — это не украшение, а защита от ровно этого.
- **Кей считается на кадре ПРЕВЬЮ-КАЧЕСТВА.** Декодер отдаёт уже
  масштабированный кадр: `to_rgba(api, frame, width, height)` со
  `SWS_FAST_BILINEAR` (`src/libs/kt-ffmpeg/src/source_impl.rs:928`), а
  `Quality` — часть запроса воркера (`src/libs/kt-engine/src/worker.rs:47`)
  и часть ключа кэша (инвариант №4). Порог, подобранный на драфте, — не
  тот же порог на экспорте. У всех пятерых кей настраивают на полном
  разрешении.
- **Цветовое пространство.** `sws_getContext` зовётся без
  `sws_setColorspaceDetails`, то есть YUV→RGBA идёт коэффициентами по
  умолчанию, а кей меряет евклид в НЕЛИНЕЙНОМ u8-RGB. Профессиональные
  кеи работают по цветовым разностям, и Screen Balance — прямое следствие
  этого. Практическая цена нашей схемы: на синем экране и на тёмных
  насыщенных тонах порог ведёт себя хуже, чем на зелёном, а «сфера»
  вокруг ключа одинаково широка по всем трём осям, тогда как реальный
  разброс экрана — эллипс, вытянутый по яркости.
- **Правило крейта kt-fx**: умолчание нового параметра = прежнее
  поведение БАЙТ-В-БАЙТ, golden'ы (t26, GOLDEN_VIDEO) не
  перегенерируются; правка формулы — скаляр и WGSL одним коммитом. Для
  кея это выполнимо (все новые ручки нейтральны в нуле), но означает
  явную ветку «prev» в ядре, как сделано для `spill`.
- **UI**: числовой параметр рисуется ТОЛЬКО `widgets::param_row` по
  `ParamSpec` — а «режим показа» не число, ему нужен свой контрол рядом
  (прецедент — комбо `PanMode` в `src/apps/kt-app/src/fx_editor.rs:580`)
  плюс строки в пяти каталогах `src/apps/kt-app/i18n/*.toml`, сабсеты
  шрифтов (`scripts/subset-fonts.py`) и тесты t100/t125.

### Какие инварианты задевает

- №7 (код композиции один) — прямо и в самом больном месте: режим показа
  матte по определению делает превью НЕ равным экспорту.
- №11 (детерминизм) — новые пороги над матte расширяют зону, где CPU и
  GPU расходятся; допуск у кея уже самый широкий из всех
  (`FrameTol::THRESHOLD`), и каждый новый жёсткий порог добавляет к нему
  пиксели. Точное сравнение — только CPU против закоммиченного эталона.
- №4 (кэш — единственная правда о coverage) — косвенно: `Quality` входит
  в ключ кэша, поэтому «настроил на драфте — получил другое на экспорте»
  чинится не кеем, а решением про качество превью.
- Правила UI: 4 (умолчание полезное — показ матte выключен), 9 (ничего
  лишнего: восемь режимов, как у Keylight, нам не нужны), 13 (место под
  переключатель искать в уже занятой строке слота FX), 20 (режим показа
  не должен «остаться включённым сам»).

## Оценка

| Что | Ответ |
|---|---|
| Объём | M — 2 стадии: (1) показ матte, (2) операции над матte и despill; garbage/holdout в цену НЕ входят |
| Где основная работа | kt-fx (ядро: показ, clip black/white, choke, размытие альфы), kt-gpu (WGSL-зеркало), kt-app (контрол показа + i18n), kt-model (спека `params_for!`) |
| Формат проекта | не трогает: новые поля с serde-умолчаниями и `skip_serializing_if`, как у `spill`; версия схемы не растёт |
| Тесты, которые придётся написать | t26 обязан остаться на прежних хэшах при умолчаниях; новые кейсы в kt-gpu t181 (допуск THRESHOLD) и в `simd_parity`; свойства матte (монотонность clip black/white, choke не выходит за кадр); тест «включённый показ матte не попадает в экспорт» — если выберем вариант (б); layout-тест на контрол в FX-редакторе |
| Зависит от | F-MSK-001 (маски-формы) — без неё нет garbage/holdout, то есть закрывается только часть задачи |
| Ломает совместимость | нет |

## Вердикт

Берём, но по частям и в правильном порядке. **Первым делом — показ
матte**, отдельной маленькой стадией: сейчас у нас есть шесть ручек, ни
одну из которых не по чему настраивать, и это не «недостаток кея», а
причина, по которой кей у нас декоративный. Вторым — операции над матte
(clip black/white, choke, размытие альфы) и осмысленный despill с
возвратом яркости, потому что без них показ матte только покажет
проблему, но не даст её починить. Garbage/holdout-маски — не сюда: они
целиком зависят от F-MSK-001, и до неё честно писать «частично».
Переход на цветоразностную формулу (аналог Screen Balance) —
кандидат следующей волны: он даёт качество, но требует второй ветки в
ядре ради байт-в-байт совместимости старых проектов, и без показа матte
его улучшение всё равно никто не увидит.

## Источники

- Keylight (The Foundry), справочник узла — режимы View, Screen Matte,
  Screen Balance, Despill Bias, Inside/Outside Mask, Replace Method:
  https://learn.foundry.com/nuke/content/reference_guide/keyer_nodes/keylight.html
  (дата обращения 2026-09-07). Это документация Foundry на тот же
  плагин, что поставляется с After Effects; справка Adobe
  (`helpx.adobe.com`) из этого окружения не открывается, поэтому строка
  Premiere/AE опирается на неё, на larryjordan.com и на википедию.
- Final Cut Pro User Guide, «Use the Green Screen Keyer effect»:
  https://support.apple.com/guide/final-cut-pro/use-the-green-screen-keyer-effect-ver40b003bc/mac
  (2026-09-07).
- Blackmagic Design, DaVinci Resolve — Fusion (Delta Keyer, clean plate):
  https://www.blackmagicdesign.com/products/davinciresolve/fusion
  (2026-09-07). Разбор вкладок Delta Keyer и его входов Garbage/Solid
  Matte, а также предупреждение о несовпадении словарей с Color-страницей
  — по вторичному источнику davinciresolveclub.com
  (https://davinciresolveclub.com/davinci-resolve-delta-keyer/, 2026-09-07),
  **[НЕ ПРОВЕРЕНО]** по официальному руководству: PDF Reference Manual
  из окружения не забирался, форум blackmagicdesign.com отдавал 403.
- Blackmagic Design, Ultimatte 12 — перечень режимов монитора («combined
  matte view, internal matte view»):
  https://www.blackmagicdesign.com/products/ultimatte (2026-09-07).
- Larry Jordan, «Premiere Pro CC: Create a Green-Screen Key», 26.02.2016:
  https://larryjordan.com/articles/premiere-pro-cc-create-a-green-screen-key/
  (2026-09-07).
- Wikipedia, «Adobe Ultra» (происхождение Ultra Key и формулировка
  «powerful but simplified keying solution»):
  https://en.wikipedia.org/wiki/Adobe_Ultra (2026-09-07); «Chroma key»
  (определение spill) https://en.wikipedia.org/wiki/Chroma_key.
- Форум Boris FX (там же живёт сообщество VEGAS Pro), темы 2169, 8058,
  22928 — цитаты в разделе «Что говорят пользователи» (2026-09-07).
- Про 4:2:0 и кей: RED, «Chroma Subsampling»
  (https://www.reddigitalcinema.com/red-101/video-chroma-subsampling) —
  сам факт, что цветность прореживается вдвое и более; связь с качеством
  кея прямым текстом сформулирована только во вторичном источнике
  (https://crispvideo.app/learn/what-is-chroma-subsampling/,
  «pulling a clean key … is much harder at 4:2:0»), **[НЕ ПРОВЕРЕНО]** по
  вендорской документации. Для нас это про ИСТОЧНИК, а не про алгоритм:
  на 4:2:0 край субъекта размазан по цветности вдвое грубее яркости, и
  никакой despill этого не вернёт; приём «размыть цветность до кея» у нас
  сегодня неприменим — `sws_scale` отдаёт уже готовый RGBA
  (`src/libs/kt-ffmpeg/src/source_impl.rs:928`), доступа к плоскостям
  YUV у движка нет.
- Параметры штатного Chroma Keyer в VEGAS (Color, Low threshold, High
  threshold, Blur amount) — по описаниям настроек у пользователей;
  официальная справка (help.magix-hub.com,
  help.vegascreativesoftware.com) из окружения не открылась,
  **[НЕ ПРОВЕРЕНО]**.
