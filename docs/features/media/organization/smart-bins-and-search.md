---
id: F-MED-001
title: Умные папки, метаданные и разметка материала
status: кандидат
priority: B
cost: L
category: media/organization
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: частично
crates: [kt-model, kt-otio, kt-media, kt-ffmpeg, kt-app]
invariants: [4, 5, 8]
ui_rules: [13, 14, 17, 20]
round: null
point: null
tests: []
depends_on: [F-AUT-002]
checked: 2026-09-07
---

# F-MED-001 — Умные папки, метаданные и разметка материала

## Суть

Отснято два часа — это 300–400 файлов с именами вида `C0012.MP4`, и монтаж
начинается задолго до первой склейки: надо за три секунды найти «тот дубль,
где он смеётся». Слоёв организации пять, и они кладутся друг на друга:
ручные папки → умные папки (сохранённый ЗАПРОС вместо списка) → метаданные
(из камеры и свои) → рейтинги, флаги и цветовые метки → и последний,
который довели до конца только у FCP, — разметка ЧАСТЕЙ клипа, когда
ключевое слово, «избранное» и «брак» вешаются не на файл, а на диапазон
внутри него.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| Final Cut Pro | 12.3 | Keywords / Keyword Collections / Smart Collections / Favorite–Reject | самая проработанная модель, и единственная, где разметка диапазонов доведена до конца. Справка Apple: «apply one or more keywords to a range within a clip, a whole clip, or a group of clips in an event»; размеченный кусок помечается прямо в браузере — «the clip or range is marked with a blue line in the browser», избранное зелёной линией, брак красной. Keyword Collection «contains pointers (aliases) to clips tagged with a specific keyword», и в ней виден ИМЕННО КУСОК. Smart Collection — сохранённое окно Filter: «Anytime a new item that matches the Smart Collection's search criteria is brought into the event or library, the new item is automatically added»; «Clips and projects that appear in Smart Collections are not duplicates». Критерии перечислены поимённо: «clip or project name, rating, media type, used media, excessive shake, keywords, the presence of people, format information, date, roles, and item type». Рейтинг бинарный (F — favorite, Delete — reject, U — снять), фильтр браузера — фиксированный набор All Clips / Hide Rejected / No Ratings or Keywords / Favorites / Rejected / Unused |
| DaVinci Resolve | 21 | Bins / Smart Bins / Power Bins / Metadata Editor | «Bins are folders that you create in the media pool to organize footage» — руками; «Smart Bins use metadata to automatically organize your clips and are always kept up to date as new footage is added to your project» — запрос, а не список; «Power Bins are visible across all projects on your system» — уровень ВЫШЕ проекта. Метаданные первого класса: «over 200 different metadata parameters for every clip», «both file and frame level metadata», и «Metadata tags can even be used to change the display names for clips using scene, shot and take information». Разметка диапазонов есть наполовину: маркер с длительностью принимает ключевые слова, но умный бин отдаёт весь клип — см. «Фишка фишки», п.2 |
| VEGAS Pro | 2026.0 | Project Media: bins, smart bins, media tags, Search Media Bins, колонка Use Count | бины есть и они виртуальные: «Bins are folders within projects that you can use to organize your media files», «They do not affect the way media is saved on your computer»; удаление из бина не трогает проект — «the file is removed from the bin, but remains part of your project». Часть бинов заводится сама: «Bins are automatically created for each media tag» и для каждого типа медиа. Есть диалог Search Media Bins («search the selected bin and all subbins»), а его результат сохраняется умным бином: «Smart bins are saved search results that are automatically updated when new media items match the search results». Рейтингов, флагов и цветовых меток на материале нет; разметки диапазонов внутри клипа нет; «Use Count» в режиме Details показывает, сколько раз материал взят на таймлайн, а в тумбнейл-режиме этого не видно |
| Adobe Premiere | 26.x | Bins / Search Bins / метаданные XMP в колонках | папки-бины плюс **Search Bin** — бин, содержимым которого является сохранённый запрос. Ограничения известны из форума Adobe и подтверждены сотрудником Adobe: бины не являются результатами поиска — «Premiere Pro's search bins are designed to filter and display media assets based on metadata and labels, but they don't inherently support the inclusion of bins as search results» (Kevin J. Monahan Jr., Adobe, 08.10.2024); условий максимум два — «Two total search filter terms connected by AND and OR»; пользовательские XMP-свойства не ищутся — «User-added custom XMP properties are nowhere to be had»; поле Description не ищется вовсе (заявка открыта с 2016 г., см. цитаты). Разметки диапазонов внутри мастер-клипа нет: её роль играют субклипы и маркеры с комментариями |
| After Effects | 26.x | панель Project: папки, строка поиска, цветовые метки | почти ничего: ручные папки, поиск по имени в шапке панели, цветовые метки у элементов. Умных папок нет, рейтингов нет, разметки диапазонов нет — материал он организует не для отбора, а для сборки композиции. [НЕ ПРОВЕРЕНО первоисточником: `helpx.adobe.com` из этого окружения не открывается; строка написана по вторичным упоминаниям и проверке живьём не проходила] |

## Фишка фишки

**1. Умная папка — это не папка, а СОХРАНЁННЫЙ ЗАПРОС, и вся её сущность
сидит в одном поле, которое пользователь не видит.** Плюс очевиден: новый
файл, подходящий под условие, попадает в неё сам, и «разойтись» с
материалом она не может. Минусы — три, и все три вылезают только на
практике. (а) В неё НЕЛЬЗЯ перетащить клип руками: перетаскивание не
меняет условие, а членство определяется только условием, — и один
клип-исключение («вот этот дубль всё-таки нужен») стоит переписывания
правила, которое заденет остальных. (б) Потеря условия = потеря папки: в
истории версий VEGAS стоит строка «Fixed a bug that prevented smart bins
from preserving their Condition setting» — бин переживал сохранение, а его
условие нет, и оставалась пустая коробка с именем. (в) Самое злое: условие
сравнивает СТРОКИ, а не значения, и заметно это становится на двузначных
номерах. В Resolve автоматические умные бины по сцене годами кладут клипы
сцены 16 одновременно в бин «16» и в бин «6» (см. цитату), потому что
«6» — подстрока «16»; на съёмке из девяти сцен всё работает, на пятнадцати
разъезжается, и монтажёр обнаруживает это не в момент настройки, а через
две недели. Ручной бин таким сломаться не умеет в принципе.

**2. Разметка диапазона стоит дёшево, а вот ПОКАЗ диапазона как отдельного
объекта — дорого, и на этом Resolve застрял.** У FCP отмеченный кусок
становится самостоятельным элементом браузера: «when we click **Yellow
star** in the Library List, only that *portion* of the NASA Solar Flare
clip that was marked when we created the Yellow star keyword is displayed»
(Larry Jordan). Разметил пятнадцать кусков внутри часового интервью —
получил пятнадцать элементов, которые ничего не копируют и не рвут связь с
исходником. Resolve прошёл ровно полпути: маркер с длительностью
ключевые слова принимает, но результат не тот — «The filtered clips in the
Smart Bin show the entire clip and not just what is between the IN and OUT
of the Marker Duration» (Scott Simmons, ProVideoCoalition, 07.12.2018), и
за восемь лет это не поменялось: в 2024 г. монтажёры пишут, что
автоматические keyword-бины «doesn't work with duration marker keywords».
Вывод для нас важнее самого факта: ценность даёт не хранение диапазона, а
то, что диапазон становится ПЕРВОКЛАССНЫМ элементом списка — со своим
именем, своим постером и своей длиной. Это решение уровня модели, а не
интерфейса.

**3. По имени файла не ищет никто, потому что имени файла нет.**
Практический ответ отрасли — не «искать лучше», а «сделать имя
осмысленным»: у Resolve «Metadata tags can even be used to change the
display names for clips using scene, shot and take information. That means
you'll never have to rename media files again!» — отображаемое имя
ГЕНЕРИРУЕТСЯ из метаданных камеры. Второй ход — искать по тому, чего
человек не вводил: у FCP среди критериев Filter стоят «used media»,
«excessive shake» и «the presence of people», то есть результаты анализа.
Третий, самый дешёвый, — колонка «Use Count» в VEGAS: вопрос «я это уже
брал?» решается колонкой, а не памятью. И зеркальное следствие: там, где
данных нет, ломается всё остальное. У Premiere Search Bin не ищет по полю
Description с 2016 года, не видит цветовых меток и не умеет
пользовательских XMP-свойств, а условий берёт максимум два — поэтому
«умный бин» там регулярно оказывается пустым, и люди решают, что фича не
работает вовсе (заголовок треда — «Search Bins do absolutely nothing»).

**4. Брак ПРЯЧУТ, а не удаляют, и это делает отбор обратимым.** «Rejecting
a clip does NOT delete it, it simply allows you not to display in the Event
Browser» (Larry Jordan), а справка Apple добавляет: «If you chose Hide
Rejected from the Filter pop-up menu at the top of the browser, clips
marked as rejected disappear from view». Отсюда способ работы, которого нет
в остальных четырёх редакторах: монтаж ВЫЧИТАНИЕМ — идёшь по материалу с
двумя клавишами (F и Delete), потом прячешь брак и смотришь, что осталось,
причём брак ставится на КУСОК, а не на файл. Ровно та же логика, что у
нашего ключа автоматизации за обрезкой (F-AUT-002): обрезка ключ прячет, а
не удаляет, — и по той же причине это безопасно.

## Плюсы

Следствия устройства, а не оценки.

- Умная папка не требует дисциплины: правило «всё, что 4K и длиннее
  минуты» действует и на файл, который импортируют завтра. Ручная папка
  требует, чтобы кто-то не забыл положить.
- Запрос компактнее списка: условие «rating = Favorite» в файле проекта —
  одна строка, а эквивалентная ручная папка — сотня ссылок, которые надо
  чинить при переименовании и переносе.
- Разметка диапазонами убирает целый класс объектов: субклип, который
  копирует ссылку и живёт своей жизнью, заменён меткой на исходнике.
  Переразметил — все коллекции перестроились, удалять и пересоздавать
  нечего. Монтажёры прямо формулируют это как цель: «tag and organize media
  without creating additional subclips which get messy with large
  projects».
- Отбор вычитанием обратим: браковка прячет, а не режет, поэтому «покажи
  всё» возвращает материал целиком, и решение недельной давности не
  стоит ничего отменить.
- Метаданные из камеры дают то, чего человек не введёт руками никогда:
  таймкод, объектив, ISO, сцену и дубль на 400 файлах. Именно их наличие
  решает, сработает ли автоматика: сгенерированное имя, умная папка по
  сцене, синхронизация по таймкоду — это одна и та же зависимость.
- Уровень выше проекта (Power Bins) снимает копирование логотипов и
  подложек между проектами: ассет один, ссылок много.
- Виртуальность бина снимает страх: у VEGAS «the file is removed from the
  bin, but remains part of your project» — раскладывать материал по папкам
  можно, ничего не потеряв.

## Минусы

Цена того же устройства.

- **Умную папку нельзя поправить рукой** — исключений в ней не бывает по
  построению. На практике рядом заводят ручную папку, и организация
  раздваивается.
- **Сущность в одном поле.** Потеря условия = потеря папки (реальный
  дефект VEGAS с `Condition setting`). У ручного бина такого единственного
  места отказа нет.
- **Условие сравнивает строки.** Подстрочное совпадение по номеру сцены в
  Resolve живёт с 2023 г. как минимум по 2024-й и ломается ровно тогда,
  когда проект вырос, — то есть в самый неудобный момент.
- **Автообновление противоречит предсказуемости.** Содержимое умной папки
  меняется без жеста пользователя — после импорта, после правки
  метаданных, после анализа. Это ровно то, что наш чек-лист UI запрещает
  правилом 20 («ничего не отрастает и не отрубается само»), и в чужих
  редакторах регулярно читается как «куда делся клип». Крайний случай — у
  Resolve автоматические keyword-бины нельзя удалить: «Seems that any
  automatically generated keyword-based Smart Bins can't be removed from
  the project», и пустая папка исчезает только после перезагрузки проекта.
- **Умная папка плохо стыкуется с остальным интерфейсом.** В Resolve
  команда «найти в медиапуле» прыгает в статический бин, из-за чего работа
  внутри умного бина на 350 клипах разваливается — жалоба 2019 года
  повторена слово в слово в 2024-м. Смарт-бины не видят Power Bins («Media
  is not "in" a project until you add to a regular bin»), а сами Power Bins
  не бэкапятся и у людей пропадают вместе с разложенным материалом.
- **Разметка диапазонами не переносится.** Keyword ranges — понятие
  библиотеки FCP; более того, она не переносится даже внутри FCP: «all of
  the information pertaining to the organizing and classifying of media is
  stored within the events and NOT the projects», поэтому логгер размечает,
  а монтажёр эту разметку не получает. Самый ценный слой организации —
  самый непортируемый.
- **Ключевых слов становится слишком много.** У FCP быстрых слотов ровно
  десять (⌃1…⌃9 плюс Favorite), дальше мышь, а поиск по списку из восьмидесяти
  слов превращается в визуальный перебор; совет самого Ларри Джордана —
  «Keep the total number of keywords small, so you don't overwhelm the
  search box» — это по сути признание предела системы.
- **Метаданные теряются на первом же перекодировании** (транскод в
  «монтажный» кодек или прокси не самим редактором обычно не переносит теги
  камеры), и умные папки по сцене/дублю молча пустеют: автоматика,
  зависящая от данных, отказывает тихо. [НЕ ПРОВЕРЕНО: живых жалоб именно
  на потерю метаданных при транскоде найти не удалось]
- **Панель материала стареет хуже остального.** У VEGAS Project Media
  сообщество прямо называет «an old and fragile area of code», из-за чего
  простейший индикатор «уже использовано» не появляется годами; типовой
  обходной путь — раскладывать материал папками в проводнике ОС, а не в
  бинах редактора.

## Что говорят пользователи

Умный бин ломается о подстроку, и это годами:

> However, clips from scene 16 are both in the smart bin for scene 16 and
> scene 6. And clips from scene 14 are both in the smart bin for scene 14
> and scene 4, etc.
> — DaniKamani (Daniel Kamangar), форум Blackmagic Design, 07.07.2023,
> https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=184257

> The automatic smart bins are still not filtering correctly in v19.0.3 in
> Windows 11, apparently. I'm still getting scene 9 and scene 19 in the
> scene 9 bin.
> — Rakesh Malik, там же, 20.10.2024

Про Premiere и его Search Bins — восьмилетний открытый дефект «поиск не
смотрит в поле Description»:

> I have children who have learned to walk, speak, run, read, play soccer,
> and do math in less time than it has taken Adobe to fix this critical bug.
> — Andy Adkins, форум Adobe, 09.01.2024,
> https://community.adobe.com/t5/premiere-pro-discussions/premiere-project-and-search-bins-not-searching-description-data/td-p/8737178

> I created a Search Bin filtering for 'Label - Magenta'…nothing. I
> literally saw a clip labled 'Magenta' direclty above the bin. What's going
> on?
> — channonp4602134, форум Adobe, 20.09.2025, тред «Search Bins do
> absolutely nothing»,
> https://community.adobe.com/t5/premiere-pro-discussions/search-bins-do-absolutely-nothing/td-p/15513405

Почему поиск по имени файла бесполезен — прямыми словами пользователя
VEGAS (это и есть ситуация «`C0012.MP4`»):

> While what @Jetdv posted can work, that means I would have to know the
> "name" of everyone clip. And since I don't rename them (they are the file
> names from the camera), that method doesn't work for me.
> — arpierre, форум Boris FX (VEGAS Pro), 10.05.2026,
> https://forum.borisfx.com/t/media-in-use-indicator-only-in-explorer-window-not-in-project-media/23315

Там же — просьба о флагах и метках с прямой ссылкой на Resolve как
эталон, и объяснение, почему панель материала не развивают:

> I think Project Media can be more useful, besides of giving tag or
> keywords, for easy search during ongoing project editing, also, giving
> color tag or any other 'marker' or 'clue' (which Resolve call it as
> 'flag').
> — set_chung, там же, 11.05.2026

> My memory of comments made was that whilst it would seem easy to do, it
> actually isn't. That's because it involves an old and fragile area of
> code.
> — ericlnz, там же, 10.05.2026

Живой дефект поиска по бинам в VEGAS 2026 — ровно наше правило 14 («фича
обязана работать ПОВТОРНО, а не один раз»), нарушенное у соседа:

> Now however the search button is greyed out, so I can't run another
> search without closing and reopening VP.
> — wjauch, форум Boris FX, 20.04.2026,
> https://forum.borisfx.com/t/search-media-bins-only-allows-search-once/22660

Про отбор вычитанием и про то, что у сильной стороны FCP есть свой потолок:

> Rejecting a clip does NOT delete it, it simply allows you not to display
> in the Event Browser.
> — Larry Jordan, 27.01.2013,
> https://larryjordan.com/articles/fcp-x-selecting-the-best-clips/

> I just keyworded several hours of footage with about 80 keywords and now
> I'm left searching for them visually in an alphabetical list.
> — Brandon VanBeek, комментарий, 22.03.2019,
> https://larryjordan.com/articles/fcp-x-find-media-using-keywords/

## Как ложится на Kinotrain

### Что уже есть

- **Медиа-бин** — `src/apps/kt-app/src/bin_panel.rs`: карточки 128×72 с
  постером и отсмотром «возюканьем» (48 логических позиций скраба на файл),
  мини-волноформа вместо чёрного квадрата у аудио, два режима раскладки
  (сетка и свободные позиции, `bin_free`), автоскролл у кромок
  (`bin_edge_scroll`), перетаскивание группой, вынос на таймлайн с запасом
  `BIN_ESCAPE_SLACK`. Пункты 62, 99, 111, 118, 123 `docs/porting-notes.md`.
- **Ручные группы** — `BinGroup { name, members, locked }` в
  `src/libs/kt-model/src/project.rs`: рамка вокруг карточек, имя в разрыве
  верхней линии, замок. Живут в `UiState`, то есть **вне undo**, и едут в
  файл через `KtTimelineMeta.bin_groups` (`src/libs/kt-otio/src/meta.rs`).
  Это ровно «ручная папка» — и ничего, кроме неё.
- **Поиск по проекту (Ctrl+F)** — чистая `search_hits` в
  `src/apps/kt-app/src/app.rs`: ищет по меткам, ИМЕНАМ ГРУПП бина, медиа
  (имя файла и путь), дорожкам и клипам, ранжирование «точное совпадение →
  начало строки → подстрока»; живые группы передаются параметром, иначе
  поиск видел бы только сохранённые (пп.92, 99). Существенно для этой
  заметки: и `HitKind::Media`, и `HitKind::Clip` берут строку ровно из
  `Path::file_name(&m.path)` — **это и есть поиск по `C0012.MP4`**, тот
  самый, который в отрасли считается бесполезным. Ни фильтров по типу, ни
  сохранения запроса, ни поиска по чему-либо, кроме имён, нет.
- **Две разметки ВРЕМЕНИ ТАЙМЛАЙНА.** `Marker { t, duration, name, color }`
  — точечная и диапазонная метка; `MarkedArea { name, start, duration,
  lanes, color }` — именованная область «время × дорожки», причём `lanes`
  хранит СЕМАНТИЧЕСКИЕ ключи, а не номера строк (Р.35 п.2). Обе — в
  `src/libs/kt-model/src/project.rs`.
- **Sidecar-индекс** — `src/libs/kt-media/src/index.rs`: таблицы `media`
  (path, size, mtime, xxhash), `packet_index`, `audio_peaks`, `thumbnails`.
  Единственное место, где вообще живёт что-то «про файл», и оно объявлено
  КЭШЕМ: битая БД молча пересоздаётся.
- **Заглушки и возврат источника** —
  `src/libs/kt-media/src/placeholder.rs`, множество `offline` в бине
  (пп.206–209): отсутствующий исходник виден, но это про доступность, а не
  про организацию.

### Чего не хватает

- **Метаданных нет вообще, начиная с самого низа.** `MediaRef` — это
  `{ id, path }` и больше ничего (`src/libs/kt-model/src/project.rs`),
  команда `AddMedia { id, path }` в `src/libs/kt-model/src/model.rs` тоже.
  Выше `MediaInfo` (`src/libs/kt-media/src/lib.rs`) несёт
  `duration/has_video/has_audio/fps/width/height` — и всё; `kt_ffmpeg::Probe`
  (`src/libs/kt-ffmpeg/src/lib.rs`) — ровно те же шесть полей. **Функция
  `av_dict_get` в биндингах не объявлена ни разу** (`av_dict_set` есть — им
  задают опции энкодера и муксера): контейнерные и потоковые теги мы
  физически не читаем. Ни таймкода, ни `creation_time`, ни сцены и дубля,
  ни объектива. Показательная деталь: в схеме sidecar-БД колонка
  `probe_json TEXT NOT NULL DEFAULT ''` объявлена — и **не пишется и не
  читается ни одной строкой кода**. Место под метаданные разметили и не
  заняли.
- **Рейтингов, флагов и цветовых меток на материале нет.** Цвет есть у
  `Marker` и `MarkedArea` (для round-trip чужого OTIO), у медиа — нет.
- **Умных папок нет.** `BinGroup` — список `members`, места под условие в
  нём не предусмотрено.
- **Нет ответа на вопрос «я это уже брал?».** Аналога колонки Use Count у
  нас нет, хотя данные лежат под рукой: `Project.clips` знает `media` у
  каждого клипа. Это самая дешёвая часть всей заметки.
- **Разметки ВНУТРИ материала нет, и это другой домен, а не недоделка.**
  Наши метки и области адресуют ВРЕМЯ ТАЙМЛАЙНА: `Marker.t` и
  `MarkedArea.start` — тики монтажа, `MarkedArea.lanes` — дорожки. Keyword
  range у FCP адресует ТИК МАТЕРИАЛА: он переживает обрезку, разрез,
  перенос и реверс события, потому что про событие не знает вовсе, и один
  и тот же размеченный кусок виден из всех проектов, где есть этот файл.
  Это тот же выбор домена, что уже сделан и описан для автоматизации в
  F-AUT-002 (`AutoKey.t` = тик материала), — только там он сделан ВНУТРИ
  клипа, а здесь пришлось бы делать его для сущности, у которой клипа нет:
  разметка принадлежит ФАЙЛУ, а не событию. Наш `Marker` переехать в этот
  домен не может — он лежит в `Project.markers` и измеряется таймлайном.
  Практический критерий готовности берём у Resolve, который на этом
  застрял: мало хранить диапазон — он обязан стать отдельной КАРТОЧКОЙ в
  бине, со своим именем, постером и длиной.
- **Экспорт метаданных наружу тоже пуст.** При записи `.otio` наш
  собственный материал получает `ExternalReference { available_range: None,
  metadata: {} }` (`src/libs/kt-otio/src/convert.rs`) — чужой редактор
  видит только путь. Чужие метаданные мы при этом НЕ теряем: `meta_foreign`
  и `media_refs` возятся passthrough целиком, а стартовый таймкод материала
  вычитается на чтении и возвращается на записи (t180,
  `src/libs/kt-otio/tests/convert.rs`). То есть таймкод источника проходит
  сквозь нас, но в модель не попадает и пользователю не виден — готовые
  данные пролетают мимо.

### Что мешает

- **Правило 13 «НЕ ТРАТИТЬ МЕСТО ВПУСТУЮ» и трижды повторённая жалоба на
  ширину бина.** Это прямое ограничение на любые UI-фантазии здесь.
  Дословно из `docs/user-feedback.md`: «виджет медиа **В ТРЕТИЙ РАЗ УЖЕ
  ОПЯТЬ ОГРАНИЧИВАЕТСЯ ПО ШИРИНЕ**» — реплика стоит пунктом 4 в разделе
  Р.12, а сводные таблицы того же файла и комментарий в коде приписывают её
  п.4 р.13; сама тема помечена как повторённая ТРИЖДЫ (р.9, р.12, р.13).
  Там же по итогам р.13 про карточки: «карточки стали слишком большими». Оба
  решения стоят в коде с комментариями: в `src/apps/kt-app/src/app.rs` —
  «ПРЕДЕЛ ПОСТАВЛЕН ПО ЖАЛОБЕ п.4 р.13 — не ужимать без новой просьбы
  пользователя» у `max_size((ui.max_rect().width() * 0.7).max(420.0))`, и в
  `src/apps/kt-app/src/bin_panel.rs` — «размер карточки НЕ раздуваем
  (жалоба р.13): широкая колонка даёт больше карточек в ряд, а не
  гигантские превью» у `card_w = ui.available_width().clamp(56.0,
  PREVIEW_W)`. Практический вывод: дерево умных папок, колонки метаданных и
  панель фильтров, каждая со своей полосой, в эту колонку НЕ ставятся.
  Рейтинг и флаг обязаны сесть на уже занятое место — уголком карточки,
  цветом рамки, линией по верху превью (как раз то, чем FCP помечает
  размеченные куски), — по образцу «имя группы в разрыве верхней линии
  рамки», который сам чек-лист приводит эталоном.
- **Правило 20 «ничего не отрастает и не отрубается само» противоречит
  самой идее умной папки.** Формально противоречия нет — правило про
  РЕДАКТИРОВАНИЕ, а умная папка ничего не редактирует, — но сказать это
  надо вслух и показать в интерфейсе: она обязана выглядеть иначе, чем
  ручная группа, иначе первая же жалоба будет «карточка сама уехала». У
  соседей это ровно так и звучит.
- **Правило 17 «парные способы одного действия ведут себя одинаково».**
  Ручная группа и умная папка внешне — одно и то же («рамка вокруг
  карточек»), а поведение при перетаскивании противоположное: в одну
  карточка кладётся, в другую нет. Либо у них разный вид, либо это дефект
  по чек-листу.
- **Правило 14 «фича обязана работать повторно».** Три из пяти чужих
  реализаций спотыкаются именно на этом (поиск VEGAS сереет после первого
  запроса; «найти в медиапуле» Resolve выкидывает из умного бина;
  автоматические бины Resolve не удаляются). Тест на повторный запрос
  обязателен с первого дня.
- **Собирать метаданные некому и негде.** Файлы читает отдельный
  thumb/probe-воркер (`src/apps/kt-app/src/thumbs.rs`; `duration_or_probe`
  ставит заявку и будит воркер), он же владеет источниками и пишет в
  sidecar-БД. Массовое чтение тегов на 400 файлах обязано идти этим же
  путём и в фоне, иначе ломается инвариант №8 (скролл/зум не порождает
  декод-запросов) и правило 12 чек-листа (фоновая работа не мешает).
- **Формату проекта нужен новый уровень.** Ручная группа поместилась в
  `KtTimelineMeta.bin_groups`; для умной папки надо хранить ЗАПРОС, а для
  разметки материала — сущность, привязанную к файлу, а не к таймлайну.
  Место у обеих одно — `metadata.kt` (`src/libs/kt-otio/src/meta.rs`), но
  запрос, который старая сборка прочтёт как пустую группу, — ровно тот
  случай, ради которого заведён бамп `KT_META_VERSION` (сегодня 4): молча
  потерять условие хуже, чем отказать (правило р.24 «наше чтобы никто не
  ломал»).
- **Уровень выше проекта нам ставить некуда.** Аналог Power Bins требует
  хранилища вне `.otio`; sidecar-БД на эту роль не годится по своему же
  правилу — она объявлена кэшем и молча пересоздаётся при повреждении
  (`src/libs/kt-media/src/index.rs`). Организация, которую можно потерять
  без предупреждения, — не организация; у Resolve это ровно та жалоба, что
  Power Bins пропадают и не бэкапятся.
- **Условие придётся типизировать сразу.** Дефект Resolve со сценой 16 в
  бине сцены 6 — не мелочь реализации, а решение о том, сравнивает ли
  запрос строки или значения. Спецификация условий должна быть такой же
  декларацией, как `params_for!` у эффектов, а не набором строковых
  сравнений в UI.

### Какие инварианты задевает

- **№8** (скролл/зум таймлайна не порождает декод-запросов) — прямо:
  фильтрация и показ метаданных в бине не должны открывать файлы; всё
  читается один раз при импорте и живёт в индексе.
- **№5** (декодером владеет ровно один поток) — косвенно: сбор тегов идёт
  через тот же воркер и ту же очередь приоритетов, а не новым потоком.
- **№4** (кэш — единственная правда о coverage) — по аналогии: у
  метаданных обязано быть ОДНО место правды; отдельная бухгалтерия «что мы
  про файл знаем» рядом с sidecar-БД — тот же класс ошибки. Тонкость: БД
  объявлена КЭШЕМ, а введённые пользователем теги и рейтинги — не кэш, и
  жить в ней не могут.
- Правила UI: **13** (место), **14** (повторно), **17** (парные способы),
  **20** (ничего не отрастает само), плюс **12** (фоновая работа не мешает).
- Инварианты движка 1–3, 6, 7, 9–11 не задеты вовсе: ни рендера, ни звука,
  ни экспорта эта фича не касается. Сильный аргумент «дёшево по риску» при
  дорогом объёме.

## Оценка

| Что | Ответ |
|---|---|
| Объём | L — целый раунд, если брать все пять слоёв. Режется на три независимые части: (а) метаданные + рейтинг/флаг на карточке + «уже использовано» — S–M; (б) умные папки — M; (в) разметка внутри материала — отдельная L, это новый домен и новый вид карточки |
| Где основная работа | `kt-ffmpeg` (связать `av_dict_get`, расширить `Probe`), `kt-media` (заполнить `probe_json`, который уже объявлен), `kt-model` (свойства у `MediaRef`, типизированное условие запроса), `kt-otio` (`metadata.kt` + бамп версии), `kt-app` (показ на карточке БЕЗ новых полос — правило 13) |
| Формат проекта | нужна `KT_META_V5`: умная папка, прочитанная старой сборкой как пустая группа, — тихая потеря, а по правилу р.24 это штатный `KtMetaTooNew`, а не молчание. Метаданные и рейтинги сами по себе версию не бампают, если едут `serde(default)` |
| Тесты, которые придётся написать | round-trip `metadata.kt` с умной папкой и рейтингами; отказ старой сборки на v5; вычисление запроса — ЧИСТАЯ функция без UI (по образцу `search_hits` и `filter_entries`) с обязательным случаем «сцена 16 не попадает в бин сцены 6»; повторный запуск запроса (правило 14); чтение тегов на фикстурах `src/fixtures/` в `kt-media/tests/ffmpeg_conformance.rs`; сохранность `probe_json` при `Reused`/`Rebuilt` (`index_db`); harness-тест бина в `layout_tests.rs` — новых полос не появилось, ширина колонки прежняя |
| Зависит от | F-AUT-002 — оттуда берётся уже принятое различение доменов «материал против таймлайна»; разметка внутри клипа обязана лечь в тот же домен материала |
| Ломает совместимость | да, на слое умных папок (v5); слои метаданных, рейтингов и «уже использовано» — нет |

## Вердикт

Берём, но не целиком и не сразу. Первым — дешёвый слой: чтение тегов
(`av_dict_get` + честный `probe_json`, под который место уже размечено),
рейтинг/флаг на карточке и индикатор «уже использовано». Он ничего в
движке не задевает и снимает главную боль: `C0012.MP4` перестаёт быть
единственным, что мы знаем о файле. Вторым — умные папки, обязательно с
ДРУГИМ ВИДОМ, чем ручные группы (иначе прямое нарушение правила 17), и с
типизированным условием с самого начала: строковое сравнение — это
готовый дефект «сцена 16 в бине сцены 6», прожитый Resolve на глазах у
всех. Разметку внутри материала (keyword ranges) выносим в отдельную
будущую заметку: это не «ещё одна метка», а новая сущность в домене
материала и новый вид карточки в бине, и заводить её надо после того, как
у медиа вообще появятся свойства. Отдельно зафиксировано ограничение: всё
это обязано влезть в существующую ширину бина — тема, повторённая
пользователем трижды, ревизии не подлежит.

## Источники

- Final Cut Pro User Guide, «Add keywords»:
  https://support.apple.com/guide/final-cut-pro/add-keywords-ver68416335/mac
  (дата обращения 2026-09-07)
- Final Cut Pro User Guide, «Create Smart Collections»:
  https://support.apple.com/guide/final-cut-pro/create-smart-collections-ver2833eb5b/mac
  (2026-09-07)
- Final Cut Pro User Guide, «Rate clips»:
  https://support.apple.com/guide/final-cut-pro/rate-clips-ver30ccd91f/mac
  (2026-09-07)
- Final Cut Pro User Guide, «Find clips and projects»:
  https://support.apple.com/guide/final-cut-pro/find-clips-and-projects-ver65764b45/mac
  (2026-09-07)
- Blackmagic Design, DaVinci Resolve — Media page:
  https://www.blackmagicdesign.com/products/davinciresolve/media (2026-09-07)
- VEGAS Pro Help, «Using media bins» (линейка 22; в 2026 раздел тот же):
  https://help.magix-hub.com/video/vegas/22/en/content/topics/4-media/media_bins.htm
  (2026-09-07)
- Scott Simmons, «DaVinci Resolve is a few steps away from range based
  keywording», ProVideoCoalition, 07.12.2018:
  https://www.provideocoalition.com/davinci-resolve-is-a-few-steps-away-from-range-based-keywording/
- Larry Jordan: «Use Keywords to Find the Clips You Need in FCP» (09.02.2020),
  «FCP X: Create Subclips Using Keywords» (18.01.2015), «FCP X: Selecting
  the Best Clips» (27.01.2013), «FCP X: Find Media Using Keywords» —
  https://larryjordan.com/
- Форум Blackmagic Design: темы 184257 (подстрочный дефект умных бинов),
  134254 (неудаляемые автоматические бины), 217689 и p=1177703 (Power
  Bins), p=1026091 (keyword-бины и маркеры-диапазоны), 99513 («найти в
  медиапуле» выкидывает из умного бина) — ссылки в разделе цитат.
- Форум Adobe (`community.adobe.com`): треды td-p/8737178 (Description не
  ищется, с 2016 г.), td-p/15513405 («Search Bins do absolutely nothing»),
  td-p/9229566 (пользовательские XMP и предел «два условия»), m-p/14903370
  (ответ сотрудника Adobe про бины как результаты поиска).
- **Форум VEGAS переехал:** `vegascreativesoftware.info` и
  `vegascreativesoftware.com/us/forum` отдают 301 на
  `https://forum.borisfx.com/` (Discourse, категория Vegas Pro); архив
  старых тредов недоступен, зато у нового форума работает JSON-поиск
  `/search.json?q=…`. Использованные темы: 22660 (поиск по бинам работает
  один раз), 23315 (индикатор «уже использовано», флаги, имена файлов от
  камеры), 24284 («Vegas Pro Release History» — фикс условия умных бинов).
- **Не проверено первоисточником:** строка After Effects и часть строки
  Premiere. `helpx.adobe.com` из этого окружения не открывается (см.
  `docs/features/_editors.md`, раздел «Что было недоступно»); страница
  поиска `community.adobe.com` отдаёт 403, хотя прямые URL тредов
  открываются; поисковые движки (DuckDuckGo, Bing, Brave, Ecosia, Mojeek,
  Yahoo, searx) отвечали CAPTCHA, 403, 429 или мусором. Формулировки
  Premiere взяты из форума Adobe, включая ответ сотрудника Adobe, — это
  форум, а не справка, и помечено как таковое.
