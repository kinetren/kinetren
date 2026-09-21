---
id: F-HLP-028
title: Туры, coach marks и интерактивные уроки
status: отвергнуто
priority: C
cost: L
category: help/onboarding
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2]
we_have: нет
crates: [kt-app]
invariants: [12]
ui_rules: [9, 10, 13, 14, 20]
round: null
point: null
tests: []
depends_on: [F-HLP-016, F-HLP-020, F-HLP-024, F-HLP-033, F-UI-002]
checked: 2026-09-07
---

# F-HLP-028 — Туры, coach marks и интерактивные уроки

## Суть

Программа сама берёт слово при первом запуске и ведёт человека по себе: либо
модальным туром («шаг 1 из 7» с кнопкой «Далее»), либо выносками поверх
интерфейса, которые тычут в конкретную кнопку, либо уроком, который заставляет
СДЕЛАТЬ действие и проверяет, что оно сделано. Общее у всех трёх — не текст, а
две вещи, которых больше нигде нет: заданный ПОРЯДОК шагов и ПАМЯТЬ о том, что
из них уже пройдено.

## У кого и как

Вопрос строки один: есть ли В САМОЙ ПРОГРАММЕ последовательность обучающих
шагов, которая ведёт по интерфейсу и помнит, где ты остановился.

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026.0 b66 | Welcome screen (вкладка «Getting Started»); раньше — Interactive Tutorials | Приветственный экран с вкладками «New Project / Open Project / Getting Started»; последняя — набор КНОПОК НАРУЖУ: «Use the buttons across the bottom to access tutorials, online help, various VEGAS communities». Выключается чекбоксом «Show Welcome Screen on Startup», возвращается в `Options → Preferences → General` («Always show Welcome Screen when program starts») — правило 19 у вендора выполнено. Настоящий тур («Interactive Tutorials» в меню Help и кнопкой на главной панели, «start an interactive guide that will show you each part of the VEGAS Pro interface and teach you how to create projects») задокументирован в руководствах Vegas Pro 13 и 17; в документе новых возможностей 2026 его нет, наличие в 2026 **[НЕ ПРОВЕРЕНО]**. Официальное обучение 2026 — видеосерия из 20+ уроков с бесплатными проектными файлами, «so you can follow each lesson step-by-step» |
| Adobe Premiere | 26 | Learn-панель (историческая); сегодня — `adobe.com/learn/premiere-pro` | Публичная точка обучения — веб-каталог «Tutorial article · Beginner · 3 min» с отдельным блоком «Try these tutorials with Premiere», то есть статья открывается в браузере, а программа рядом. Состояние встроенной Learn-панели в 26 проверить не удалось: `helpx.adobe.com` из этого окружения не открывается ни прямо, ни через текст-прокси **[НЕ ПРОВЕРЕНО]** |
| DaVinci Resolve | 21 | нет | Онбординг вынесен из программы в УСТАНОВЩИК: «the most important of these settings is selected via the installer's onboarding questions». Дальше руководство советует новичку при первом запуске просто проверить настройки; ни тура, ни выносок, ни чек-листа в интерфейсе нет |
| Final Cut Pro | 12.3 | нет тура; демо-проект | Раздел «Get started» руководства — это ГЛАВЫ («What is Final Cut Pro?», «Intro to the Magnetic Timeline», «Final Cut Pro workflow», «Final Cut Pro interface»), а единственный интерактивный ход рядом с ними — «Download a demo project» (через Apple Creator Studio). Учат на настоящем проекте, а не поверх интерфейса |
| After Effects | 26 | **[НЕ ПРОВЕРЕНО]** | Историческая Learn-панель с пошаговыми уроками существовала; состав в 26 проверить не удалось по той же причине, что у Premiere. Строка оставлена «нет данных» сознательно: догадка тут стоила бы дороже пробела |

**Вне ядра.** Пять NLE показывают только исход спора, но не механизм: чек-лист,
который отмечается САМ по факту выполненного действия, встроен в VS Code, а
единственная coach-mark-реализация, чей исходник можно прочитать и оценить по
стоимости, — у JetBrains. Без этих двух строк разговор о туре остаётся спором
о вкусах.

| Программа | Версия | Как называется | Как устроено |
|---|---|---|---|
| Visual Studio Code | 1.136 | Walkthroughs (страница «Get Started») | Точка вклада `contributes.walkthroughs` в манифесте расширения: `id`, `title`, `description`, `when` и массив `steps`; у шага — `title`, markdown-описание, `media` (SVG или markdown-файл) и `completionEvents`. Галочка ставится по СОБЫТИЮ: «Check off step when a command has been run» (`onCommand:`), `onSettingChanged:`, `onContext:`, `onView:`, `extensionInstalled:`, `onLink:`. Открывается сам при установке расширения; сброс прогресса — командой `workbench.action.resetGettingStartedProgress` |
| IntelliJ IDEA | 2026.2.2 | Learn tool window (IDE Features Trainer) + тур по новому UI | Уроки идут в НАСТОЯЩЕМ редакторе: «Run IntelliJ IDEA with this plugin, and then click the *Learn* tool window button». В том же плагине `intellij-community` лежат рядом три механизма — `src/` (уроки), `onboarding/` (тур), `tips/` (совет дня). Шаг тура (`LearnToolWindowStep.kt`) ищет кнопку в живом дереве компонентов по локализованному тексту действия, подсвечивает рамкой (в опциях есть `usePulsation`), ждёт появления окна (`yield()`, комментарий «wait for the tool window to appear») и ставит балун по вручную посчитанной точке; не нашёл виджет — молча рисует шаг без подсветки. Сам онбординг новых пользователей закрыт флагом эксперимента (`NewUsersOnboardingExperimentAccessorImpl`) |

## Фишка фишки

**Груз тура — не текст, а ПОРЯДОК и ПАМЯТЬ О СДЕЛАННОМ.** Одна строка
статусбара не вмещает ни того, ни другого по построению: она не знает, что было
до неё, и не помнит, что ты уже умеешь. Ни контекстное меню, ни шпаргалка, ни
описание эффекта этот груз тоже не несут — они отвечают на вопрос «что тут
можно», а не «с чего начать и что дальше». У нас порядок уходит в главы
руководства и в демо-проект, а память о сделанном не уходит никуда: мы её не
несём вообще и сознательно (см. «Вердикт»).

Три детали, которых нет ни в одном списке возможностей.

**1. У эталонной реализации «интерактивность» выключена по умолчанию.**
Документация VS Code: «By default, if no `completionEvents` events are provided,
the step will be checked off when any of it's buttons are clicked, or if the
step has no buttons, when it is opened». То есть ПРОЧИТАЛ = СДЕЛАНО, пока автор
урока не перечислил руками конкретные команды и настройки. Разница между «уроком»
и «слайдшоу с галочками» — это одно необязательное поле манифеста, и по умолчанию
получается второе. Любой разговор «интерактивный урок лучше тура» надо начинать
с вопроса, включено ли это поле.

**2. Walkthrough целится в КОМАНДЫ, а не в пиксели, — и поэтому переживает
перекомпоновку.** Шаг не знает, где кнопка: он знает `onCommand:commandId`,
`onSettingChanged:settingId`, `onContext:` и `onView:viewId`. Урок остаётся
верным после любой перестановки панелей, потому что описан на языке действий, а
не расположения. Обратная сторона ровно та же: без реестра команд как объектов
такой урок невыразим в принципе — некому сказать «шаг закрыт, команда выполнена».
Это делает VS-Code-форму зависимой не от рисования, а от F-UI-002.

**3. Вторая половина цены видна в исходнике JetBrains, и она про вёрстку.**
Один шаг тура — это поиск виджета в живом дереве компонентов по ЛОКАЛИЗОВАННОМУ
тексту действия, подсистема подсветки (`LearningUiHighlightingManager`) с
отдельной опцией `usePulsation`, ожидание появления окна корутиной, балун по
посчитанной точке (`component.width + 3`, `min(150, height/2)`) и регистрация
уборки в `Disposer`. Не нашёл виджет — шаг не падает и не сообщает, а тихо идёт
дальше без подсветки. Это цена ОДНОГО шага у вендора, который держит IDE
на четырёх платформах, и это же объясняет, почему их онбординг стоит за флагом
эксперимента, а не просто «включён».

## Плюсы

- Единственный канал, способный сказать «сначала X, потом Y»: у меню,
  подсказки, шпаргалки и справочника нет порядка — они списки, а не маршрут.
- Чек-лист по событиям (VS Code) знает, СДЕЛАЛ ли ты, а не только прочитал.
  Именно этим урок отличается от тура, и это отличие механическое, а не
  риторическое: `onCommand` либо сработал, либо нет.
- Push-канал: доносит до того, кто НЕ искал. Наша строка подсказки — чистый
  pull, и на предмет, о существовании которого не знаешь, мышь не наведёшь
  (тезис Р.40).
- Первый запуск — единственный момент, когда у человека нет своего проекта, и
  занять экран не жалко. Это то же основание, на котором стоят учащие пустые
  состояния (F-HLP-020), только на всё окно сразу.
- Привязка к командам и настройкам, а не к координатам, делает урок дешёвым в
  сопровождении — но только у того, у кого команды существуют как объекты.

## Минусы

- **Показывает один раз и в начале, а вопросы приходят посреди работы.** Это
  главный минус формы, и у нас он подтверждён собственной историей (см. «Как
  ложится», таблица по семи претензиям Р.38).
- **Coach mark по определению перекрывает то, на что указывает**: выноска стоит
  над интерфейсом и закрывает соседей цели. Перекрытие — решённый вопрос
  (п.14 р.8), и форма упирается в него не краем, а сутью.
- **Память человека короче тура.** NN/g: «our short-term memory cannot retain
  very much information, and that information fades in about 20 seconds» — семь
  шагов подряд не переживают дороги от тура до первого настоящего действия.
- **Автооткрытие не масштабируется**: как только источников шагов больше одного,
  получается «storm of walkthroughs» (цитата ниже).
- **Пропустил — надо уметь вернуть.** VS Code держит для этого отдельную команду
  `workbench.action.resetGettingStartedProgress`, VEGAS — чекбокс в
  `Preferences → General`. То есть механизм тянет за собой ещё и собственную
  строку «где вернуть решение» (правило 19, F-HLP-009), иначе он одноразовый.
- **Прицел ломается молча.** У неконтрактной реализации (JetBrains) ненайденный
  виджет — не ошибка, а пропуск подсветки: урок продолжает говорить «нажмите
  вот здесь», не показывая где.
- **Перевод шага — это абзац, а не строка.** Наш предел 160 символов на значение
  каталога и вся цепочка t100/t125/t128 считаны в F-HLP-033; шаги тура — самый
  дорогой вид текста из всех, что рассматривала волна.
- **Отрасль идёт в другую сторону.** Вендор, у которого тур был (VEGAS 13/17),
  сегодня учит видеосерией с проектными файлами; Apple — загружаемым демо-проектом;
  Blackmagic вынесла онбординг в вопросы установщика. Ни один из пяти NLE ядра
  сегодня не подтверждён как носитель живого тура по интерфейсу.

## Что говорят пользователи

> Oh, I never use the Welcome screen.
> — arpierre, цитируется в треде «Video Scopes Settings?», forum.borisfx.com,
> 13.08.2026, https://forum.borisfx.com/t/video-scopes-settings/25018/29

> Honestly, a beginner without any learning simply starts VEGAS is
> catastrophically. And the welcome guidance won’t help much.
> — YelandKeil, там же, 13.08.2026, https://forum.borisfx.com/t/video-scopes-settings/25018/29

> Top extension packages right now trigger a storm of walkthroughs opening and
> fighting for attention, which doesn't help do user onboard but causes confusion.
> — digitarald, microsoft/vscode#174304, 14.02.2023,
> https://github.com/microsoft/vscode/issues/174304

Обратная сторона — восторга живой цитатой найти не удалось, и это надо сказать
прямо: искал в трекере VS Code (запросы `walkthrough in:title`, `walkthrough
disable OR annoying OR reopen`, сортировка по реакциям — верх выдачи занят
багами и планами итераций, не отзывами) и на форуме Boris FX (`interactive
tutorials`, `welcome screen startup`, `how to learn vegas beginner`). Самый
популярный живой запрос про walkthrough — «Provide a command to reset the
completed steps of a walkthrough» (#212819, 25 реакций, открыт), то есть люди
просят не показать тур, а СБРОСИТЬ его.

Отдельно, не отзыв, а рекомендация вендора одного из пяти редакторов ядра —
Apple HIG, раздел «Onboarding»: «People tend to grasp and retain information
better when they can actually perform the task they’re learning about instead of
just viewing instructional material», и рядом — «Consider providing a collection
of context-specific tips instead of a single onboarding flow». Первая фраза —
единственный сюжет ЗА в этой заметке, вторая — ровно то, что мы выбрали вместо.

## Как ложится на Kinotrain
CAPABILITY: openspec/specs/ui-reach/ — контракт в `spec.md`, «почему» и пути в коде в `design.md`.
Отвергнуто: обучающего слоя поверх UI нет — правило t83 (никаких всплывающих подсказок) и «правый клик — это и есть справка»; коучмарки противоречат обоим.

## Оценка

Цена указана для гипотетической реализации: чтобы отказ был сравним с
альтернативами, а не голословен.

| Что | Ответ |
|---|---|
| Объём | L — целый раунд: слой, прицел, хранилище прогресса, пункт меню, пять каталогов |
| Где основная работа | kt-app (слой поверх, поиск целей, хранилище) + `apps/kt-app/i18n/*.toml` |
| Формат проекта | не трогает |
| Тесты, которые придётся написать | при отказе — один страж `reach_tests::no_teaching_layer_over_the_ui` (закрытый список слоёв выше `Order::Middle`); при реализации — плюс тест на живучесть прицела при перекомпоновке и t128-подобный бюджет для абзацев шагов |
| Зависит от | F-UI-002 (реестр команд — без него урок невыразим), F-HLP-033 (цена текста), F-HLP-009 (строка «где вернуть») |
| Ломает совместимость | нет |

## Вердикт

Не берём. Разведка не нашла ни одного подтверждённого живого тура по интерфейсу
среди пяти редакторов ядра: у Resolve онбординг вынесен в установщик, у Apple —
в загружаемый демо-проект, у VEGAS тур задокументирован в руководствах 13 и 17,
а обучение 2026 продаётся видеосерией с проектными файлами. Мы отказываемся не
от отраслевого стандарта, а от формы, из которой отрасль уходит.

Главное же — адресат. Семь претензий «я не понимаю как» из Р.38 тур не снимает
ни одной наверняка: их написал владелец программы посреди работы, а тур говорит
с новичком при первом запуске. Ту же задачу дешевле и без перекрытия закрывают
четыре уже описанные поверхности — F-HLP-016, F-HLP-020, F-HLP-024 и
демо-проект (`docs/features/help/onboarding/sample-project-and-media.md`), —
каждая из которых отвечает в момент вопроса, а не за месяцы до него.

ОТВЕРГНУТО: инвариант 12 и правило 10 — обучающего слоя поверх интерфейса у нас не будет, и запрет надо не ослаблять, а расширять, поэтому отказ адресуется тесту `reach_tests::no_teaching_layer_over_the_ui`: сегодня выше `egui::Order::Middle` живут ровно восемь именованных слоёв (`status-hint`, `project-title`, `ambient-overlay`, `speed-drag-overlay`, `bin-group-name`, `marker-name-edit`, `lane-name-edit`, `area-name-edit`), из них четыре — ввод на месте и четыре — показ состояния, и ни один не объясняет интерфейс; t83 ловит только `egui::Order::Tooltip`, то есть выноска на `Order::Foreground` вернула бы перекрытие ПРИ ЗЕЛЁНОМ страже, а пульсация вдобавок стала бы шестым членом закрытого списка `animating` в `src/apps/kt-app/src/app.rs`, где в покое стоит `repaint_delay(false)` ≥ 100 мс (t20).

ОТЛОЖЕНО: до F-UI-002 — единственная форма, которую разведка не забраковала (урок, который ПРОВЕРЯЕТ выполненное действие, а не показывает картинку), выражается только через реестр команд: сказать «шаг закрыт, потому что команда выполнена» некому, пока у нас нет объекта «команда»; вернуться к этому разговору осмысленно после F-UI-002 и только в форме, которая целится в команды, а не в координаты.

**Дополнение разведки (позднее возвращение агента).** Отложенная форма — не
гипотеза: она отгружена. У **Ableton Live 12.4.5** есть **Learn View**, дословно
из мануала — «structured learning modules that combine video content with written
explanations and guide you through Live's interface and key features step by
step»; уроки фильтруются по темам и **отмечаются как пройденные** с отслеживанием
прогресса, вид открыт по умолчанию и закрывается через меню Help. Важно, КЕМ это
сделано: Ableton — единственный в выборке, кто выбрал ту же архитектуру подсказки,
что и мы (постоянное место вместо попапа, F-HLP-002). Столкнувшись с тем же
пределом, он завёл не выноски поверх интерфейса, а ОТДЕЛЬНЫЙ ВИД — то есть ровно
вторую поверхность, которую ищет вся волна: не перекрывает, не задерживает,
открывается намеренно. Это укрепляет обе строки вердикта разом: слой поверх
интерфейса отвергнут правильно, а отложенная форма имеет живой промышленный
образец. Отсрочка при этом остаётся: «шаг закрыт, потому что команда выполнена»
сказать по-прежнему некому, пока нет объекта «команда» (F-UI-002).

ОДНА СТРОКА: перекрытие — не трогает (и предлагает укрепить запрет тестом, потому что t83 ловит только `Order::Tooltip`) | задержка — не трогает | ёмкость — в строке статусбара не меняется ни на символ, и НИКАКОЙ новой поверхности эта заметка не заводит: весь груз расписан по уже принятым — контекстное меню (F-HLP-016), тела пустых контейнеров (F-HLP-020), демо-проект, шпаргалка (F-HLP-024), главы руководства (F-HLP-023).

## Источники

- VS Code, Contribution Points → `contributes.walkthroughs` (поля шага,
  `completionEvents`, поведение по умолчанию, автооткрытие при установке) —
  https://code.visualstudio.com/api/references/contribution-points
  (обращение 07.09.2026)
- microsoft/vscode#174304 «Only auto-open walkthroughs for extensions a user
  choose to install» — https://github.com/microsoft/vscode/issues/174304
  (07.09.2026)
- microsoft/vscode#212819 «Provide a command to reset the completed steps of a
  walkthrough» — https://github.com/microsoft/vscode/issues/212819 (07.09.2026)
- JetBrains, `plugins/ide-features-trainer/README.md` и
  `onboarding/src/.../LearnToolWindowStep.kt`, `NewUsersOnboardingExperimentAccessorImpl.kt` —
  https://github.com/JetBrains/intellij-community/tree/master/plugins/ide-features-trainer
  (07.09.2026)
- Apple, Human Interface Guidelines → Onboarding —
  https://developer.apple.com/design/human-interface-guidelines/onboarding
  (07.09.2026)
- Apple, Final Cut Pro User Guide for Mac, раздел «Get started» и «Download a
  demo project» — https://support.apple.com/guide/final-cut-pro/welcome/mac
  (07.09.2026)
- Nielsen Norman Group, Aurora Harley, «Instructional Overlays and Coach Marks for
  Mobile Apps», 16.02.2014 (резюме статьи: «users tend to dismiss them quickly and
  do not read thoroughly») —
  https://www.nngroup.com/articles/mobile-instructional-overlay/ (07.09.2026)
- VEGAS Pro 22 «Introduction to the new features» (приветственный экран, вкладка
  «Getting Started», возврат через `Options → Preferences → General`) и
  руководства Vegas Pro 13 / VEGAS Pro 17 (раздел «Interactive Tutorials»,
  меню Help и кнопка главной панели) — PDF вендора, разобраны `pdftotext -layout`
  (07.09.2026)
- Учебная страница VEGAS Pro 2026 (видеосерия из 20+ уроков, «Download Projects»)
  — https://www.vegascreativesoftware.com/vegas-pro/learn/ (07.09.2026)
- forum.borisfx.com, тред «Video Scopes Settings?», пост 29 —
  https://forum.borisfx.com/t/video-scopes-settings/25018/29 (07.09.2026)
- DaVinci Resolve 21 Reference Manual, глава 1 «Introduction to DaVinci Resolve»
  («installer's onboarding questions») — PDF Blackmagic Design (07.09.2026)
- Adobe Learn → Premiere (каталог веб-уроков, блок «Try these tutorials with
  Premiere») — https://www.adobe.com/learn/premiere-pro (07.09.2026)
- **Проверки живьём не было ни для одной программы**: строки Premiere и After
  Effects опираются на публичный каталог Adobe, потому что `helpx.adobe.com` из
  этого окружения не открывается ни прямо, ни через текст-прокси; строки VS Code
  и JetBrains — по документации и открытому исходнику; для софта вне ядра
  проверка живьём не предполагается (`docs/features/_editors.md`).
