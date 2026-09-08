---
id: F-HLP-009
title: «Где это потом изменить обратно» — плашка, называющая место возврата
status: кандидат
priority: B
cost: S
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2, blender-5.2, photoshop-27, ableton-live-12]
we_have: нет
crates: [kt-app, kt-model]
invariants: []
ui_rules: [13, 17, 18, 19]
round: null
point: null
tests: []
depends_on: [F-HLP-010, F-HLP-033]
trackers: [openshot, kde-bugzilla, kde-invent, losslesscut]
checked: 2026-09-08
---

# F-HLP-009 — «Где это потом изменить обратно» — плашка, называющая место возврата

## Суть

Программа один раз спрашивает пользователя (или сама принимает за него
решение) — и тем же экраном говорит, в каком меню или настройке это
переключается назад. Не «как отменить последнее действие» (это undo), а «где
живёт тумблер, который я сейчас щёлкнул, не глядя». Сильная форма механизма —
не фраза с путём, а КНОПКА, открывающая нужное место; тогда указание не может
устареть от переименования пункта меню.

Владелец сформулировал требование капсом и в один приём с самой опцией
(Р.28 п.9, `docs/user-feedback.md:477-480`):

> «меня интересует опция проекта, которая позволит переслаивать несколько
> событий одной дорожки одновременно… Сделай в контекстном меню такую опцию и
> включи ее по умолчанию… ЕЩЕ ОБЯЗАТЕЛЬНО В ПЛАШКЕ ГОВОРИ ГДЕ ЭТО МОЖНО
> ВЕРНУТЬ ОБРАТНО. И агентам тоже обяжи чтобы в ui/ux ДУМАЛИ о пользователе».

Требование записано правилом 19 чек-листа (`src/apps/kt-app/CLAUDE.md:274`)
и не выполнено ни разу — ни в той опции, ради которой писалось, ни где-либо
ещё.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026.0 | нет; подавление живёт в Preferences ▸ General | Чекбоксов «больше не показывать» в диалогах официально нет вовсе: вопросы включаются и выключаются заранее («Prompt to adjust project settings to match first media added to timeline», «Provide a warning if a track being deleted holds at least one event»). Сброс — постраничный «Default All». Диалог о Preferences не упоминает |
| Adobe Premiere | 26.x | нет; по чекбоксу на диалог, на РАЗНЫХ вкладках | Кнопки «Reset All Warning Dialogs» в Premiere НЕТ (она в Photoshop/InDesign/Lightroom, в видеоприложения не переехала). Возврат — отдельным чекбоксом: «Show Clip Mismatch Warning dialog» и кнопка «Reset Fit Clip dialog» лежат в Timeline, «Show Project Load Error dialog» — в General. Сам диалог («This clip does not match the sequence's settings…» + «Always ask») места не называет |
| DaVinci Resolve | 21 | нет как класса | В Reference Manual (июль 2026, 4443 стр.) **ноль** вхождений «Do not show» / «Don't show» / «Never ask»: BMD паттерн не документирует вообще. Есть только «Reset System Preferences» / «Reset User Preferences» из Option-меню. Одноразовый «Change Project Frame Rate?» вообще необратим: «Once clips have been imported into the Media Pool, the frame rate cannot be changed again, so choose carefully» — и об этом говорит руководство, а не диалог |
| Final Cut Pro | 12.3 | **Reset All** (Settings ▸ General ▸ Dialog warnings) | Единственный из пяти с настоящим общим сбросом: «Click this button to reset all warning dialogs that you dismissed by selecting the "Do not remind me again" checkbox». Но обратной ссылки нет: в полном User Guide 12.3 фраза «Do not remind me again» встречается РОВНО ОДИН РАЗ — в описании самой кнопки; какие диалоги её несут, Apple не перечисляет нигде |
| After Effects | 26.x | нет; ближайшее — Reset Coach Marks | В AE 26 панель General вообще упразднена (Preferences переорганизованы в 16 панелей), «Reset All Warning Dialogs» нет. Подавления живут в Preferences по одному («Do not warn me about missing fonts when opening projects»). Единственный документированный возврат уже показанного: «Reset Coach Marks: Resets the list of coach marks that have been seen, showing them again» |

**Итог по ядру: ноль из пяти.** Ни один из пяти редакторов не имеет ни одного
диалога, который сам называет место своей отмены, и ни одной кнопки-перехода
из диалога в настройки. Формулировки «You can change this behavior in
Preferences ▸ …» у Resolve и FCP встречаются десятками — но только в
РУКОВОДСТВЕ, никогда в интерфейсе. Это меняет статус требования владельца: оно
не догоняющее.

Механизм в этом домене живёт вне видеомонтажа, и по правилу `_editors.md`
(«„фишка фишки“ взята из этой программы») вторая таблица здесь обязательна.

**Вне ядра.** Без этих пяти программ фичу описать нельзя: только у них
механизм существует не как фраза в диалоге, а как адресуемый объект — команда,
URI, строка сгенерированной таблицы, — и только там видно, чем за него платят.

| Программа | Версия | Как называется | Как устроено |
|---|---|---|---|
| Visual Studio Code | 1.136 | Command links in notifications; `@id:` / `@modified`; Configure Trusted Domains | Уведомление несёт markdown-ссылку `[текст](command:<id>)` — с 1.34; аргументы едут URI-кодированным JSON. Настройка адресуема: `vscode://settings/<settingName>`, шестерёнка у настройки умеет «copy the settings URL». Фильтр `@modified` — отдельная поверхность «что я вообще менял» |
| IntelliJ IDEA | 2026.2 | Notification actions; Don't show again + `Appearance \| Behavior \| Notifications` | Баллон несёт действия (`NotificationAction.createSimple`), открытие настроек — `ShowSettingsUtil.showSettingsDialog(project, Configurable)`. Заглушённое возвращается таблицей «Don't ask again notifications» с кнопкой Remove. Настройки ищутся из Search Everywhere (`Shift Shift`, затем `/`) |
| Blender | 5.2 LTS | ПКМ ▸ Online Manual (F1); Allow Online Access | ПКМ по ЛЮБОМУ свойству даёт точную ссылку в мануал: RNA-путь резолвится по сгенерированной таблице `scripts/modules/_rna_manual_reference.py`. Диалог расширений говорит «You can adjust this later from "System" preferences» И даёт кнопку, зовущую `screen.userpref_show(section='SYSTEM')` |
| Adobe Photoshop | 27.x | Reset All Warning Dialogs | Кнопка в Preferences ▸ General возвращает ВСЕ подавленные предупреждения разом. Ни один диалог Photoshop при этом сам не называет своё место в настройках — пользователи узнают путь на форуме |
| Ableton Live | 12.4.5 | нет | Info View описывает элемент ПОД КУРСОРОМ, а не место настройки; диалогов, называющих, где отменить решение, не найдено. Preferences переименованы в Settings, поля поиска мануал не описывает |

Отдельно — платформенные предписания, потому что они противоречат друг другу
и это меняет постановку:

- **Microsoft (Win32 UX Guide, Dialog Boxes)** — единственная найденная
  формулировка правила КАК ПРАВИЛА: «If users may need to restore these dialog
  boxes, provide a **Restore messages** command in the program's Options
  dialog box.»
- **Apple HIG, Settings** — «consider providing a button that opens it
  directly from your interface»; `openSettingsURLString` документирован как
  «deep linking to settings». Схема `x-apple.systempreferences:` официально НЕ
  поддерживается (ответ Apple DTS на форуме разработчиков).
- **Material 3** — против: «Providing a third action, such as Learn more, is
  not recommended as it navigates the user away from the dialog.»
- **Android** — прямой запрет для одного частного случая: «Don't link to
  system settings in an effort to convince the user to change their decision.»

Устоявшегося ИМЕНИ у паттерна нет ни у ui-patterns.com, ни у NN/g, ни в
Material/Carbon/Polaris. Ближайшее — «deep linking to settings», и это термин
API, а не UX.

## Фишка фишки

**Что механизм несёт, чего одна строка статусбара вместить не может:** ЧУЖОЙ
АДРЕС. Не текст, а ссылку на другой элемент интерфейса — которая обязана
остаться верной, когда тот элемент переименуют, переставят или переведут. В
160 символов путь буквами влезает (у нас — 36–54 символа); туда не влезает его
ПРАВДИВОСТЬ. У нас груз денется в кнопку: адрес хранится типом, подпись
собирается из тех же ключей каталога, что рисуют само меню, и печатной копии
пути в каталоге не заводится вовсе.

Три детали, которых нет ни в одном списке возможностей.

1. **VS Code сделал адрес настройки первоклассным объектом — и дал его
   копировать.** Шестерёнка у любой настройки предлагает «copy the setting ID,
   copy a JSON name-value pair, or **copy the settings URL**»; URL имеет вид
   `vscode://settings/workbench.colorTheme`, а из уведомления та же точка
   открывается ссылкой `[текст](command:workbench.action.openSettings?["@id:…"])`.
   Смысл не в удобстве: пока адрес — строка в прозе, его нельзя ни
   скопировать, ни проверить; как только он стал URI, диалог может ДЕРЖАТЬ
   адрес, не произнося его. Ровно поэтому у VS Code есть и обратная
   поверхность `@modified` — «what did I change», с прямым обоснованием в
   доках: «if you have forgotten whether you configured a setting, or if the
   editor is not behaving as you expect because you accidentally configured a
   setting».

2. **Blender проверяет связность текста и места МАШИНОЙ, а не переводчиком.**
   ПКМ по любому свойству ▸ Online Manual (F1) ведёт не в поиск, а в точный
   якорь: RNA-путь кнопки резолвится по сгенерированной таблице
   `scripts/modules/_rna_manual_reference.py`
   (`("bpy.types.preferencesview.show_tooltips_python*",
   "editors/preferences/interface.html#…")`). То есть у каждой настройки есть
   машинно-адресуемый адрес документации, и разъезд «переименовали опцию —
   ссылка соврала» ловится сборкой таблицы, а не читателем. При этом сам
   Blender диалогов почти не имеет по идеологии («Blender doesn't pop up
   requesters that require the user to fill in data before things execute»,
   «Modal dialogs are not great design and should be avoided») — и паттерн
   появляется ровно там, где решение нельзя откатить через undo: диалог
   расширений говорит «You can adjust this later from "System" preferences»
   И даёт кнопку `screen.userpref_show(section='SYSTEM')`.

3. **JetBrains хранит адрес страницы возврата — и не требует кнопки туда;
   обе версии лежат в одном файле ресурсов.** Javadoc
   `Notification.configureDoNotAskOption(id, displayName)` описывает второй
   параметр как «tile for UI in *Preferences | Appearance & Behavior |
   Notifications*»: платформа ЗНАЕТ, куда пользователь придёт возвращать, и
   хранит для этого человеческое имя. Но контракт кнопки не требует, и в
   `ApplicationBundle.properties` соседствуют паттерн и антипаттерн: детектор
   отступов даёт баллон с действиями `Undo` и **`Code Style Settings`**
   (ссылка на точную страницу), а рядом —
   `notification.content.available=<b>{0} Language is available</b><br>It can
   be changed later in Settings`, то есть «где-то в Settings», без страницы и
   без кнопки. Один продукт, один файл, две дисциплины.

И ещё одно, что видно только когда поищешь фидбек: **хвалебных цитат про
диплинк не существует.** Люди пишут в сеть, когда что-то ПОТЕРЯЛИ, а не когда
кнопка сработала. Значит, окупаемость этого механизма нельзя измерить
восторгами — только отсутствием тредов «как вернуть».

## Плюсы

- Решение перестаёт быть ловушкой: «ответил один раз и застрял» — это
  формулировка самого правила 19, и она описывает механику, а не настроение.
  Диалог, назвавший место, превращает необратимый на вид выбор в обратимый.
- Кнопка вместо пути снимает связность текста с расположением: переименовали
  пункт — подпись поехала вместе с ним, потому что она и есть подпись пункта.
  Прозаический путь пришлось бы править в пяти файлах и в пяти языках.
- Механизм даёт побочную поверхность «что я вообще менял» (`@modified` у VS
  Code, таблица «Don't ask again notifications» у JetBrains). Она отвечает на
  вопрос, который пользователь задаёт чаще, чем помнит конкретную настройку:
  «программа ведёт себя не так, и я не знаю, что нажал».
- Требование машинно-проверяемо, если адрес — тип, а не строка. Это ровно то,
  чем Р.38 закрыл провал чек-листа: правило, которое можно уронить тестом,
  перестаёт быть прозой.
- Дёшево там, где место уже есть: у Blender диалог расширений зовёт
  `screen.userpref_show(section=…)` — одну существующую операцию.

## Минусы

- **Указание живёт дольше, чем место, на которое указывает.** Как только путь
  напечатан прозой, любая перестановка меню делает текст ложью — и ложью
  молчаливой: ни один тест не отличит верный путь от устаревшего.
- **Цена размножается на языки.** Путь сам переводится: `View → Economy UI` /
  `Вид → Экономный UI` / `视图 → 省电界面` / `عرض → واجهة موفرة`. Одно решение —
  пять текстов, каждый из которых надо править синхронно с меню.
- **Кнопка уводит из диалога.** Возражение Material 3 («navigates the user
  away from the dialog») настоящее: третье действие в модальном окне ломает
  выбор, ради которого окно открыто. Значит, кнопка законна не в любом месте,
  а в том, откуда уходить не жалко.
- **Диплинк соблазняет уговаривать.** Запрет Android («don't link to system
  settings in an effort to convince the user to change their decision») —
  про то, что механизм «где вернуть» легко вырождается в «передумай».
- **Дисциплина не держится сама.** Лучший пример — внутри Microsoft: Win32-гайд
  требует «Restore messages», а гайд VS Code для авторов расширений в разделе
  «✔️ Do» предписывает «Add a **Do not show again** option for every
  notification» и о дороге обратно не говорит ни слова.
- **Средство возврата само теряется.** Кнопка Photoshop «Reset All Warning
  Dialogs» существует, но актуальная страница Adobe о Preferences (июль 2026)
  её вообще не упоминает — пользователи находят её на форумах. У FCP то же
  ребром: общий «Reset All» есть, а фраза «Do not remind me again» во всём
  руководстве встречается ровно один раз — в описании самой кнопки. Место
  возврата без обратной ссылки — это половина механизма, и работает она
  только для тех, кто уже знает, что ищет.

  **Датировка делает этот довод решающим.** Раздел «Resetting all warning
  dialogs» стоит уже в руководстве Adobe Photoshop 6.0 (2000, стр. 97) — то
  есть кнопка возврата существует у Adobe четверть века, и всё это время ни
  один диалог Photoshop о ней не сообщает. Значит проблема не в отсутствии
  механизма возврата: механизм бывает и живёт десятилетиями. Проблема ровно в
  отсутствии **указателя на него в точке принятия решения**, а это и есть
  формулировка правила 19. Двадцать пять лет — достаточный срок, чтобы считать
  это не недоработкой конкретного вендора, а устойчивым провалом жанра.
  (Точная формулировка Adobe остаётся неподтверждённой: `helpx.adobe.com`
  отдаёт 403/таймаут, `web.archive.org` заблокирован, ManualsLib под CAPTCHA —
  лейбл и путь подтверждены тремя вторичными источниками, дословной цитаты
  первоисточника нет. `[НЕ ПРОВЕРЕНО дословно]`)
- **Указание в документации не заменяет указания в интерфейсе.** Resolve и
  FCP называют адрес настройки в руководстве десятками раз («You can change
  this behavior in Preferences ▸ User ▸ UI Settings…») — и ноль раз в самих
  диалогах. Пользователь читает диалог, а не главу про него.
- **Второе обещание в окне — вторая точка молчаливого отказа.** Трекер даёт
  случай, которого в этой заметке не было: у Kdenlive сломался сам чекбокс
  подавления (kde-invent #36, «"Do not ask again" doesn't work»), и заметил
  это пользователь, а не сборка. Диалог, который сегодня обещает одно
  («больше не спрошу»), после правила 19 обещает два, и второе — чужой
  адрес. Кнопка, ведущая не туда, не роняет ни сборку, ни существующий тест;
  роняет она доверие ко всему окну, а вернуть его дороже, чем один раз найти
  настройку.
- **Выключатель и дорога назад нужны разным людям, и первый мешает второму.**
  Счёт из «Что говорят пользователи» разводит их прямо: у losslesscut просьб
  «уберите вопрос совсем» втрое больше, чем «как вернуть», у openshot — ровно
  наоборот. Указатель места возврата обслуживает новичка и оплачивается
  строкой в окне того, кто просил противоположного — чтобы этого окна не было
  вовсе. Правило 19 добавляет текста ровно туда, где вторая половина
  аудитории уже требует тишины.

## Что говорят пользователи

СИГНАЛ: «выключил сам — и спрашиваю чужих, где вернуть» — 5 задач в openshot, `(can.?t|cannot|unable|do ?n.?t know how|how (do i|to)|no way)[^.]{0,35}(get|bring|restore|re.?enable|turn|switch)[^.]{0,22}(it|them|this|that|the \w+)? ?back`; доля 0.1% против 0.0% (ни одной из 6346) в kde-bugzilla

СИГНАЛ: «уберите этот вопрос навсегда» — 3 задачи в losslesscut, `(disable|remove|get rid of|hide|suppress|stop|turn off|avoid|skip|do ?n.?t (show|ask))[^.]{0,30}(warning|confirmation|dialog|prompt|popup|pop.?up|message ?box|notification|nag)`; доля 0.2% против 0.0% (3 из 6346) в kde-bugzilla

Два счёта смотрят на одну механику с разных концов, и внутри своих трекеров
они меняются местами. У losslesscut просьб «уберите вопрос» три, а жалоб
«верните, что я убрал» — одна; у openshot ровно наоборот: одна просьба убрать
против пяти «как вернуть». Это не разница продуктов, а разница пришедших:
кто уже знает программу, требует выключатель, а кто её осваивает — ищет дорогу
назад. Механизм правила 19 обслуживает второго за счёт первого, и обе стороны
в счёте видны.

Третий счёт объясняет, почему первые два малы. Фильтр
`(do ?n.?t|never|not) (show|ask|display|remind)[^.]{0,12}again` даёт **три
задачи на 25 442 записи одиннадцати трекеров корпуса `nle`**, и из этих трёх одна — просьба
чекбокс ЗАВЕСТИ (openshot #5489, 03.04.2024), вторая — жалоба, что он не
работает (kde-invent #36). То есть у опенсорсных монтажок подавляемого диалога
почти нет как класса, и это подтверждает вывод раздела «У кого и как» с другой
стороны: не «механизм есть, и о нём молчат», а «механизма нет вовсе».

Все найденные живые цитаты — об одном и том же: человек нажал «больше не
показывать» и потерял поведение, а путь возврата узнал у постороннего. Из
видеомонтажа — три, и первая описывает механику точнее любого гайдлайна.

> «After realizing that the sequences were not matching the clips anymore I
> wanted to bring back the feature of the "Clip Mismatch Warning" dialog box
> but after unchecking "Always ask" I was unable to find a way to bring back
> ONLY that feature.»
> — Ant Gonzalez, CreativeCOW (Premiere), 20.08.2014,
> https://creativecow.net/forums/thread/how-do-you-reset-the-always-ask-checkbox-in-premie/
> Он же годом позже в том же треде: «The only solution I found was to trash my
> preferences file and restart.» Правильный ответ («In PPro Preferences ->
> General, there is a check box…») пришёл от другого пользователя через
> девять месяцев — и с тех пор настройка переехала на вкладку Timeline, то
> есть ответ из треда УЖЕ устарел.

> «I accidentaly click on "always you this choice" when this "fit clip", "the
> source is longer than the destination" window appeared»
> — John28850210ubid, Adobe Community (Premiere), 22.03.2023,
> https://community.adobe.com/t5/premiere-pro-discussions/how-to-reset-the-warning-dialogs-to-their-default-settings/td-p/13670817

> «the window pops up that say: Change Project Frame Rate? and i want the
> project to match the video I am bring in what do I click? Don't Change or
> Change? I just want a simple answer, please.»
> — martinbrossman, Blackmagic Forum (Resolve), 15.01.2024,
> https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=195099

Тот же сюжет вне монтажа, где он документирован лучше всего:

> «I accidentally hit the 'Do Not Show Again' dialog box when deleting a layer
> mask. Now whenever I try to apply a layer mask, instead of asking if I want
> to delete or apply, it just automatically deletes it.»
> — Linda1234567, Adobe Community (Photoshop), 05.01.2018,
> https://community.adobe.com/t5/photoshop-ecosystem-discussions/un-checking-the-quot-do-not-show-again-quot-dialog-box/m-p/9613714
> (ответ в том же треде: «Go to Photoshop>Preferences. In the general tab,
> click 'reset all warning dialogs'.»)

> «I accidentally selected do not show this message again under Data Merge.
> Now when there is an issue with Data Merge I can't tell what it is because
> that message no longer appears.»
> — Martina_Doodles1, Adobe Community (InDesign), 16.09.2019,
> https://community.adobe.com/t5/indesign-discussions/how-do-i-undo-quot-do-not-show-this-message-again-quot/td-p/10610170

> «It use to pop up and I mistakenly clicked the "Don't show again" button. I
> want to turn it back on because I've realised how easy it is to make a
> mistake.»
> — Microsoft Q&A, 18.02.2016,
> https://learn.microsoft.com/en-us/answers/questions/2711827/how-to-turn-back-on-the-confirmation-dialogue-box

Голос со стороны проектировщика, формулирующий это как дефект, а не как
неудобство:

> «If a user selects this option the element is removed forever so the user has
> essentially deleted a portion of the UI that they can never get back with
> just one click.»
> — GitLab UX issue #213344,
> https://gitlab.com/gitlab-org/gitlab/-/issues/213344

Трекеры, недоступные при первом написании заметки, дали тот же сюжет уже не в
проприетарном софте, а в открытом, и в самой чистой форме: одно нажатие убрало
часть интерфейса, места возврата никто не назвал, и человек идёт спрашивать
посторонних.

> «So.. I pressed on one of those toolbar buttons where you see an icon of
> fullscreen and delete and i accidentaly pressed delete. now i can't get it
> back.»
> — issue #615, OpenShot, 02.05.2017,
> https://github.com/OpenShot/openshot-qt/issues/615

> «So i was on open shot and i pressed the x on the window of filters/effects.
> Anyone know how to get it back?»
> — issue #1825, OpenShot, 10.07.2018,
> https://github.com/OpenShot/openshot-qt/issues/1825

Обе задачи закрыты состоянием `completed` — то есть и панель, и окно вернуть
было можно. Значит место возврата в программе существовало, и цена его
безымянности измерена точно: заведённая задача в чужом багтрекере вместо одной
строки в интерфейсе. Тот же сюжет, что у Photoshop с его двадцатипятилетней
кнопкой «Reset All Warning Dialogs», но записанный номером, датой и
состоянием, а не воспоминанием.

И оборотная сторона: механизм подавления, о котором в интерфейсе не сказано
ничего, ломается молча — первым это замечает не тест, а пользователь.

> «If you press "do not ask again" for a warning message it doesn't work.»
> — issue #36, Kdenlive (invent.kde.org), 12.02.2019,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/36

Цитаты, ХВАЛЯЩЕЙ диплинк-кнопку, не нашлось и в трекерах — как не нашлось её
раньше в поиске («took me straight to the setting», «nice touch» рядом с VS
Code notification, «instead of hunting through menus» + permission settings).
Причина, похоже, содержательная и уже названа в «Фишке фишки»: про сработавшую
кнопку не пишут.

## Как ложится на Kinotrain

АДРЕС: src/libs/kt-model/src/project.rs:3306 — класс «нажал один раз и не
нашёл, где вернуть» у нас ВОЗМОЖЕН, и в форме хуже чужой: `overlap_layers:
true` — единственное однократное решение проекта, и переключить его нечем
вовсе. Присваивание вне тестов ровно одно — чтение из файла,
`src/libs/kt-otio/src/convert.rs:1026`; `Command` для него в
`src/libs/kt-model/src/model.rs` нет, пункта меню нет, плашки нет, а руками
флаг ставится только в тестах пяти файлов. У OpenShot из issue #615 панель
вернуть было можно, и вопрос закрылся одним ответом; у нас возвращать нечем и
негде, и ни один тест этого не видит.

### Что уже есть

- **Правило записано и не исполнено.** `src/apps/kt-app/CLAUDE.md:274-276`:
  «Диалог обязан сказать, где потом изменить решение. Любая плашка с выбором
  заканчивается фразой, в каком меню/настройке это переключается — иначе
  пользователь один раз ответил и застрял.»
- **Доказательство невыполнения — грепом, не на слово.** Корпус:
  `src/apps/kt-app/i18n/{en,es,ru,zh-Hans,ar}.toml`, по 522 значения в
  каждом, **2610 строк** (счёт по `^([A-Za-z_0-9]+)\s*=\s*"(.*)"$`). Три
  вопроса к корпусу:
  1. *Называет ли значение путь в меню?* Regex по именам меню всех пяти
     языков (`File|Edit|View|Файл|Правка|Вид|文件|编辑|视图|Archivo|Editar|Ver|
     ملف|تحرير|عرض`) с обязательным разделителем `/ → > »`. Результат — **один
     ключ**, `msg.ktproj_import_only` («Старый .ktproj открывается только
     через Файл / Импортировать .ktproj…»), и он про правило 18 (где НАЙТИ
     действие), а не про 19 (где ВЕРНУТЬ решение). Разделитель в regex
     обязателен: без него арабские `عرض` («показ») и `ملف` («файл») дают три
     ложных срабатывания в `cam.axis_fit_hint`, `hint.idle`,
     `msg.media_missing`.
  2. *Умеет ли каталог вообще говорить, как вернуть?* Умеет: **30 значений**
     несут акселератор в скобках (`play_pause`, `loop`, `undo`, `split`, …) —
     секция `[tb]` даже объявляет это правилом в комментарии. Каталог говорит
     КАК; он ни разу не говорит ГДЕ.
  3. *Есть ли слова про возврат?* Четыре ключа:
     `hint.preview_detached` («закрой его, чтобы вернуть»),
     `tl.lane.row_height` («ПКМ — сбросить»), `mrph.revert_hint`,
     `fx.cancel` («Отмена (вернуть как было)»). Первые два называют ЖЕСТ,
     вторые два — откат внутри окна. Места не называет ни один.

  Итог: правило 19 исполнено **0 раз из 2610**.
- **Мы один раз уже решали ровно эту задачу — и решили её именем, а не
  путём.** Подменю языка (`src/apps/kt-app/src/toolbar.rs:322-338`) рисует
  эндонимы, и комментарий в коде объясняет зачем: «выбравшему китайский нечего
  искать в русском списке». t125 держит самоназвания одинаковыми во всех пяти
  каталогах. Это наш собственный прецедент против печатного пути.
- **Готовая вторая поверхность.** Окно уведомлений
  (`src/apps/kt-app/src/app.rs:1103-1127`) — место, куда пользователь
  приходит намеренно; оно не перекрывает и не задерживает. `struct Notice {
  at, severity, text }` (`app.rs:22`) — чистый текст, поля под адрес нет.
- **Механика подстановки готова и покрыта**: `tf!`/`interp`
  (`src/apps/kt-app/src/i18n/mod.rs:148`) с позиционными `{0}`, t125 сверяет
  наборы плейсхолдеров с английским.
- **Прецедент машинной проверки правила чек-листа по каталогу**: t171
  (`src/apps/kt-app/src/i18n_tests.rs:278`) превратил правило 18 в тест по
  всем пяти языкам. Харнесс умеет ходить по меню:
  `h.get_by_label(tr(key)).click()` (`layout_tests.rs:4688`) и `right_click`
  (`reach_tests.rs:395`).

### Чего не хватает

Список решений, которые пользователь принимает (или получает) один раз, с их
настоящим местом переключения — готовый материал для будущего текста:

| Решение | Где принимается | Где переключается обратно | Живёт |
|---|---|---|---|
| Переслаивание клипов (`Project.overlap_layers`) | **нигде** | **нигде** | в файле |
| Язык | `toolbar.rs:322` Вид ▸ Язык | там же — но меню уже на выбранном языке | Storage `lang` |
| Эконом-режим | `toolbar.rs:318` Вид ▸ Эконом **и** `app.rs:3763` перф-панель | два места, ни одно не знает о другом | сессия |
| Доля разрешения превью | `app.rs:3676`, комбо «1/4» под превью | там же | сессия |
| Деатач превью | двойной клик по превью (`app.rs:3355`) **и** Вид ▸ Отделить | `hint.preview_detached` называет ЖЕСТ, не место | сессия |
| Магнит | Ctrl во время драга (`timeline.rs:1996`) | постоянной настройки нет вообще | один кадр |
| Loop | кнопка тулбара / `L`; гасится САМ в `clear_region` (`app.rs:2305`) | кнопка; про автогашение не сказано нигде | проект |
| Бэкенд видео-FX | программой: `Poisoned` → CPU (`app.rs:772`, `msg.video_fx_gpu_lost`) | ничем, кроме перезапуска; `KT_VIDEO_GPU=0` только ВЫКЛЮЧАЕТ GPU | сессия |
| Автосейв | не спрашивают (`msg.autosave_restored`) | настройки нет | — |
| `KT_METER_DEMO` | переменная окружения (`meters.rs:352`) | она же | процесс |

**Гвоздь — `overlap_layers`.** Единственное присваивание вне тестов —
`src/libs/kt-otio/src/convert.rs:1026` (`p.overlap_layers = !l.off;`), то
есть ЧТЕНИЕ из файла. Команды нет (`grep -rn "OverlapLayers\|SetOverlap"
src/libs/kt-model/src/` — пусто), пункта контекстного меню нет, плашки нет,
умолчание `true` (`src/libs/kt-model/src/project.rs:3156`). Следовательно
режим «без переслаивания» — целая ветка поведения (`ops.rs:705` демоция при
импорте, `plan_move`/`restore_handles`, кроссфейд из наезда,
`src/libs/kt-engine/src/audio.rs:283` вес перекрытия) — **недостижим из
программы вообще**; девять тестов в четырёх файлах ставят флаг руками, то есть
проверяют код, до которого пользователь не может дотянуться.

Из решения Р.28 п.1 («опция проекта, переключается из контекстного меню; при
первом переслоении плашка объясняет последствия и говорит, где вернуть
обратно») сделаны модель, формат и микс — и не сделаны ни меню, ни плашка, ни
указание места. Требование, породившее правило 19, не выполнено в собственном
раунде.

Отдельно: настроек как МЕСТА у нас нет. В eframe Storage пишутся ровно два
ключа — `lang` и `last_project` (`src/apps/kt-app/src/app.rs:5182-5185`);
всё остальное из таблицы живёт сессию. Плашек с выбором тоже нет: десять окон
(`morpher_editor.rs:273`, `fx_editor.rs:164`, `render_dialog.rs:275` и `:439`,
`camera_editor.rs:75`, `transition_editor.rs:141`, `app.rs:1105`, `:1347`,
`:2356`, `:5069`) — это редакторы, списки и один диалог имени; подтверждений
«да/нет» в программе нет ни одного. Поиск Ctrl+F (`app.rs:4543`) ищет
метки/медиа/дорожки/клипы/группы бина — команд и настроек не видит (поиском по
СОДЕРЖИМОМУ владеет F-MED-001).

### Что мешает

**Печатный путь нам дороже, чем кажется, и в двух местах прямо не работает.**

*Цена в символах.* Медиана значения каталога — 14–17 символов (zh — 6),
предел `MAX_VALUE_CHARS = 160` (`src/apps/kt-app/build.rs:42`), максимум
сегодня 145. Готовый путь «Вид → Эконом-режим» стоит en 40 · es 47 · ru 36 ·
zh 21 · **ar 54** символа — треть бюджета до того, как началась сама фраза.

*Цена в связности.* Путь состоит из двух значений, которые в каталоге УЖЕ
есть (`menu.view.title`, `menu.view.economy`). Написав его прозой, мы заводим
их третью и четвёртую копии в пяти файлах — и t125 этого не заметит: ключ на
месте, длина в пределах, набор плейсхолдеров совпадает, а указание врёт. Это
буквально минус «указание живёт дольше места» из раздела выше, только
подтверждённый нашим собственным тестом: t125 проверяет полноту, использование
и плейсхолдеры, но не ПРАВДИВОСТЬ адреса.

*Техническая невозможность для арабского.* Таблица `ar` переупорядочивается на
СБОРКЕ (`src/apps/kt-app/build.rs:135`, `bidi::visual`), поэтому `{0}` стоит
в ВИЗУАЛЬНОЙ позиции. Латинское имя файла туда подставляется нормально
(`msg.media_missing`) — это LTR-атом; собранный в рантайме арабский путь
приедет в логическом порядке внутрь визуальной строки, то есть задом наперёд.
Значит «подставим путь аргументом» — не обходной путь, а дефект.

*Дублирование мест.* Эконом-режим переключается из двух точек с двумя
разными ключами и ОДИНАКОВЫМ текстом (`menu.view.economy` и `perf.economy`).
Прозаический путь обязан выбрать одну из них и тем соврать про вторую;
кнопка — просто ведёт туда, где пользователь сейчас.

*Чего нельзя предлагать.* Плашка у нас — не всплывающий слой: правило 10
чек-листа и решение п.14 р.8 стоят, и заметка их не трогает. Адрес возврата
предлагается на поверхностях, которые уже есть: пункт меню, которым решение и
принимается, и окно уведомлений, куда приходят намеренно.

### Какие инварианты задевает

- **Правило 19** — предмет заметки.
- **Правило 18** («пункт меню говорит, ЧТО и ГДЕ») — соседнее и уже
  проверяемое машиной (t171); правило 19 — то же самое, только про обратную
  дорогу.
- **Правило 17** («парные способы одинаковы») — эконом-режим и деатач превью
  переключаются двумя способами каждый; указание места обязано быть общим,
  иначе пары разойдутся текстом при одинаковом поведении.
- **Правило 13** («не тратить место впустую») — прямой аргумент против
  печатного пути: 40–54 символа из 160 на то, что кнопка выражает подписью
  существующего пункта.
- Инварианты движка (1–12 корневого `CLAUDE.md`) не задеваются: ни нового
  слоя, ни перерисовки на кадр, ни работы в аудио-пути. Инвариант 12
  (repaint только при движении) не нарушается — кнопка рисуется в уже
  открытом окне.

## Оценка

| Что | Ответ |
|---|---|
| Объём | S — одна стадия раунда |
| Где основная работа | kt-app: тип адреса возврата + поле у `Notice` + кнопка в окне уведомлений; kt-model — `Command` для `overlap_layers`, которого нет |
| Формат проекта | не трогает (`overlap_layers` уже в конверте `metadata.kt`, `convert.rs:515`) |
| Тесты, которые придётся написать | `reach_tests::every_choice_names_its_way_back` — исчерпывающий `match` «решение → где переключается» с пустым allow-list по идиоме `NOT_FROM_UI` (`reach_tests.rs:209`), плюс харнесс-проверка, что названный адрес реально кликается; расширение t171 на пункты меню, участвующие в адресах |
| Зависит от | F-HLP-010 (канал ошибок и `Severity`), F-HLP-033 (цена локализации — не пересчитывать здесь) |
| Ломает совместимость | нет |

Рамка, а не ТЗ. Главное проектное решение, которое раунд обязан принять
сознательно: адрес возврата — ТИП (перечисление мест), а не строка каталога;
подпись собирается из ключей самого меню. Как только адрес станет строкой,
тест снова превратится в прозу.

## Вердикт

Правило 19 не «почти выполнено» и не «выполнено частично» — оно выполнено
ноль раз из 2610 строк каталога, и не выполнено в том самом раунде, который
его породил: опция переслаивания живёт в модели, в формате и в миксе, а
переключить её нельзя ничем, кроме правки .otio руками. Причина ровно та же,
которой закрылся Р.38: проза ничего не роняет. t252 этого не поймал не по
недосмотру, а по устройству — он перечисляет варианты `enum Command`, а
осиротела не команда, а РЕШЕНИЕ, живущее полем проекта.

Разведка при этом изменила и сам предмет требования. Владелец просил фразу
(«В ПЛАШКЕ ГОВОРИ ГДЕ»), а у всех, у кого механизм работает, это не фраза, а
адресуемый объект: URI настройки у VS Code, сгенерированная таблица якорей у
Blender, `Configurable` у JetBrains. Фраза дешева в написании и дорога в
поддержке — она жёстко связывает текст с расположением пункта меню, а у нас
это ещё и пять языков, третья копия уже существующих значений мимо t125 и
арабский, где подстановка пути физически неверна. Поэтому берём требование, но
не его форму: место называет КНОПКА, ведущая туда, а печатного пути в каталоге
не заводится вовсе.

Третье, что стоит сказать прямо: здесь мы не догоняем. В пяти редакторах ядра
диалогов, называющих место своей отмены, ноль, кнопок-переходов в настройки —
ноль, а Resolve не документирует паттерн вообще. Требование владельца ставит
нас впереди рынка, а не вровень с ним, и это редкий случай, когда «сделать как
у чужих» — заведомо хуже, чем сделать по его просьбе.

ТЕСТ: reach_tests::every_choice_names_its_way_back — сегодня падает на первой
же строке таблицы: `Project.overlap_layers` не имеет ни `Command`, ни пункта
меню, ни плашки (единственное присваивание вне тестов —
`src/libs/kt-otio/src/convert.rs:1026`, чтение из файла), поэтому
исчерпывающий `match` «решение → где переключается обратно» нечем заполнить, а
allow-list по идиоме `NOT_FROM_UI` обязан остаться пустым.

ОДНА СТРОКА: перекрытие — не трогает | задержка — не трогает | ёмкость —
строка статусбара не растёт ни на символ и печатного пути не получает вовсе;
адрес возврата едет ТИПОМ на уже существующих поверхностях — на пункте меню,
которым решение принимается, и кнопкой в окне уведомлений
(`src/apps/kt-app/src/app.rs:1103`), куда пользователь приходит намеренно, а
подпись кнопки берётся из тех же ключей `menu.*`, что рисуют само меню.

## Источники

Проверка живьём для программ вне ядра не предполагается (правило
`_editors.md`); всё ниже — документация, исходники и форумы.

- Microsoft, Win32 UX Guide, Dialog Boxes («Restore messages») —
  https://learn.microsoft.com/en-us/windows/win32/uxguide/win-dialog-box
  (обращение 2026-09-07)
- Microsoft, Launch the Windows Settings app (`ms-settings:` как диплинк) —
  https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings-app
- Apple HIG, Settings («a button that opens it directly») —
  https://developer.apple.com/design/human-interface-guidelines/settings
- Apple, `openSettingsURLString` —
  https://developer.apple.com/documentation/uikit/uiapplication/opensettingsurlstring ;
  неофициальность `x-apple.systempreferences:` — ответ Apple DTS,
  https://developer.apple.com/forums/thread/761314
- Material 3, Dialogs (против третьего действия) —
  https://m3.material.io/components/dialogs/guidelines
- Android, Requesting permissions («don't link to system settings…») —
  https://developer.android.com/training/permissions/requesting
- NN/g, Confirmation Dialogs Can Prevent User Errors —
  https://www.nngroup.com/articles/confirmation-dialog/
- VS Code 1.34, Command links in notifications —
  https://code.visualstudio.com/updates/v1_34#_command-links-in-notifications
- VS Code, Settings (`@id:`, `@modified`, «copy the settings URL»,
  `vscode://settings/<settingName>`) —
  https://code.visualstudio.com/docs/configure/settings (проверено 2026-09-07)
- VS Code, Command URIs (`command:` в markdown, `isTrusted`) —
  https://code.visualstudio.com/api/extension-guides/command (проверено
  2026-09-07)
- VS Code, UX Guidelines ▸ Notifications («Add a Do not show again option for
  every notification») —
  https://code.visualstudio.com/api/ux-guidelines/notifications
- JetBrains, Balloon notifications («configure settings», «Don't show again»)
  — https://plugins.jetbrains.com/docs/intellij/balloon.html
- JetBrains, Notifications ▸ Restore hidden suggestions —
  https://www.jetbrains.com/help/idea/notifications.html
- JetBrains, Settings dialog (поле поиска) —
  https://www.jetbrains.com/help/idea/settings-preferences-dialog.html
  (проверено 2026-09-07)
- JetBrains, Search Everywhere (поиск по настройкам) —
  https://www.jetbrains.com/help/idea/searching-everywhere.html
- Blender HIG (Non Blocking, Operate → Settings, Modal Dialogs) —
  https://developer.blender.org/docs/features/interface/human_interface_guidelines/
- Blender Manual, Context Menu (Online Manual, Copy Full Data Path) —
  https://docs.blender.org/manual/en/latest/interface/controls/buttons/menus.html#context-menu
- Blender Manual, Searching Preferences —
  https://docs.blender.org/manual/en/latest/editors/preferences/introduction.html#searching-preferences
- Photoshop, Adjust preferences —
  https://helpx.adobe.com/photoshop/desktop/get-started/settings-and-preferences/adjust-preferences.html
  (Last updated Jul 21, 2026). Дословная формулировка «Reset All Warning
  Dialogs» **[НЕ ПРОВЕРЕНО по живой странице Adobe]** — страница отдаёт
  403/таймаут; лейбл и путь подтверждены вторичным источником: HowToGeek,
  28.02.2021, https://www.howtogeek.com/712432/how-to-bring-back-photoshops-warning-dialogs/
- Ableton, Live 12 Manual, Live's Settings и Info View —
  https://www.ableton.com/en/live-manual/12/first-steps/
- Цитаты пользователей — ссылки в разделе «Что говорят пользователи».

Ядро (всё — официальная документация вендоров, обращение 2026-09-07):

- Premiere 26, General preferences и Timeline preferences (обновлены
  07.01.2026) —
  https://helpx.adobe.com/premiere/desktop/get-started/preferences-and-settings/general-preferences.html
  и
  https://helpx.adobe.com/premiere/desktop/get-started/preferences-and-settings/timeline-preferences.html
- After Effects 26, Preferences (обновлена 27.03.2026) —
  https://helpx.adobe.com/after-effects/using/preferences.html
- DaVinci Resolve 21 Reference Manual, июль 2026, 4443 стр. —
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
  (утверждение «ноль вхождений „Do not show“» получено grep'ом по всему PDF)
- Final Cut Pro, General settings ▸ Dialog warnings ▸ Reset All —
  https://support.apple.com/guide/final-cut-pro/general-settings-vera36110dbb/mac ;
  подсчёт «„Do not remind me again“ ровно один раз» — grep по полному
  User Guide 12.3, https://help.apple.com/pdf/final-cut-pro/en_US/final-cut-pro-user-guide.pdf
- VEGAS Pro 2026, Preferences ▸ General —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/12-preferences/prefgeneraltab.htm

Трекеры (Р.41; дампы живут в `~/.cache/kinotrain/trackers/` и в репозиторий не
попадают — `_trackers.md`, раздел «Лицензии»). Именно они закрыли пробел,
из-за которого в этой заметке раньше стояла честная пометка о ненайденной
цитате:

- https://github.com/OpenShot/openshot-qt/issues/615 — «I removed some
  toolbars and can't get them back», 02.05.2017, closed/completed
  (обращение 2026-09-08)
- https://github.com/OpenShot/openshot-qt/issues/1825 — «i "x"'d out a window
  and i dont know how to get it back», 10.07.2018, closed/completed
  (обращение 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/36 — «"Do not ask
  again" doesn't work.», 12.02.2019 (обращение 2026-09-08)
- https://github.com/OpenShot/openshot-qt/issues/5489 — просьба ЗАВЕСТИ
  чекбокс «Don't show this again», 03.04.2024 (обращение 2026-09-08)
- https://github.com/mifi/lossless-cut/issues/847 — «I always want to "merge"
  how do I disable pointless dialog?», 01.09.2021 — сторона «уберите вопрос»
  (обращение 2026-09-08)

Обе строки `СИГНАЛ:` воспроизводятся командой `python3
src/scripts/tracker-rank.py --query '<фильтр из обратных кавычек>'` на
выгрузке от 08.09.2026: 12 дампов, 28 537 записей (корпус `nle` — 25 442; ffmpeg-trac в своём корпусе). `blender-vse` и
`ffmpeg-trac` на момент работы ещё качались (файлы пустые) и в процентных
сравнениях не участвуют.

**Что осталось непроверенным** (чтобы следующая волна не искала заново):
дословная формулировка диалога VEGAS «match project settings to media» и
чекбокс окна VEGAS News — только вторичные источники: старый форум
`vegascreativesoftware.info` после покупки Boris FX редиректит на Discourse, а
старые треды не заархивированы; диалог Adobe System Compatibility Report —
страницы справки отдают 404; дословная строка Adobe «Reset All Warning
Dialogs» для Photoshop — живая страница отдаёт 403/таймаут, лейбл подтверждён
вторичным источником (HowToGeek). Кроме того, бюджет `WebSearch` сессии был
исчерпан (200 из 200) до начала работы над заметкой, `helpx.adobe.com` при
прямом обращении из основной сессии дал таймаут 60 с, а текст-прокси
`r.jina.ai` — HTTP 422; данные по Adobe собраны обходными путями и помечены
выше там, где остались неподтверждёнными.
