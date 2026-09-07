---
id: F-MED-002
title: Прокси — монтаж на лёгких копиях, рендер с оригиналов
status: кандидат
priority: B
cost: L
category: media/proxies
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: нет
crates: [kt-model, kt-media, kt-ffmpeg, kt-cache, kt-engine, kt-otio, kt-app]
invariants: [4, 5, 9, 10, 11]
ui_rules: [12, 13, 16, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MED-002 — Прокси — монтаж на лёгких копиях, рендер с оригиналов

## Суть

Монтажёр бросает на таймлайн часовой материал с 4K-камеры, и превью
перестаёт поспевать: скраб дёргается, кадр отстаёт от звука, вентилятор
воет. Прокси — это когда программа один раз перекодирует исходники в
маленькие лёгкие копии, привязывает их к оригиналам и монтирует на них, а
на финальный рендер сама, без напоминания, возвращает оригиналы.
Переключатель у фичи один, и вся её ценность в том, что его можно забыть
выключить.

## У кого и как

| Редактор | Версия | Как называется | Как устроено |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Create Video Proxy (ПКМ в Project Media) | файл `.sfvp0` кладётся **рядом с исходником, с тем же именем**, и в Project Media НЕ показывается — его видно только в проводнике. Отдельного тумблера «прокси/оригинал» нет вовсе: прокси включает тот же регулятор, что качество превью — «When the preview quality is set to Draft or Preview, the proxy file is used. When the preview quality is set to Good or Best, the original file is used». Для 4K прокси строятся автоматически. Рендер прокси не видит в принципе |
| Adobe Premiere | 26.x | Proxies: Ingest → Create Proxies / Attach Proxies, кнопка Toggle Proxies в Program Monitor | прокси создаются через Media Encoder по пресету ingest (типовой «Low Resolution Proxy» — 1024×540), файлы пишутся в выбранную при импорте папку. Чужие прокси цепляются командой Attach Proxies, и Adobe требует имя `<оригинал>_Proxy.<расш>` в отдельной папке рядом с мастерами. В диалоге экспорта есть галочка **Use Proxies** (появилась в 14.3.2), и Adobe прямо предупреждает: «The "Use Proxies" checkbox is not affected by enabling or disabling proxies while editing» — состояние монтажа и решение рендера НЕ СВЯЗАНЫ. Галочка сохраняется в пресет экспорта; соседняя «Use Previews» имеет над ней приоритет |
| DaVinci Resolve | 21 (механика проверялась по 18.x–20.x) | Proxy Media, Optimized Media, Render Cache — ТРИ разные вещи | Optimized Media — внутренний, непортируемый и неэкспортируемый кэш перекодированного материала; Proxy Media — обычные файлы (ProRes Proxy на macOS, DNxHR/H.264 на Windows; четверть/восьмая разрешения), которые можно унести и раздать; папку задаёт Working Folders → Proxy generation location, а отдельное бесплатное приложение Blackmagic Proxy Generator строит их по watch-папке. В Render Settings на Deliver есть переключатель «Use optimized media» — по умолчанию рендер идёт с оригиналов |
| Final Cut Pro | 12.3 | Optimized media / Proxy media, меню View → Media | прокси — Apple ProRes 422 Proxy **или** H.264, «in a variety of frame sizes from 12.5 percent to 100 percent of the original», хранятся внутри библиотеки или во внешнем storage location. Режимов ТРИ: Optimized/Original, **Proxy Preferred** (нет прокси — молча берёт оригинал) и **Proxy Only**. Защита от рендера с прокси — фраза в справке: «make sure to switch back to optimized/original media before sharing your project» |
| After Effects | 26.x | Proxy (Set Proxy → File), Placeholder, Comp Proxy | прокси ставится на КОНКРЕТНЫЙ элемент проекта или на композицию, переключается квадратиком у имени элемента. Отдельная строка в Render Settings — **Proxy Use**: `Use No Proxies` (умолчание в шаблоне Best Settings) / `Use Comp Proxies Only` / `Use All Proxies` / `Current Settings`, то есть решение принимается на уровне задания рендера, а не проекта |

## Фишка фишки

**1. Защита от «отрендерил с прокси» — это не галочка, а РАЗНАЯ
КОНСТРУКЦИЯ, и разброс между редакторами максимальный.** VEGAS вообще не
даёт такой возможности: в справке написано «Video proxy files are not used
for rendering your project», а тумблера «прокси/оригинал» там нет — прокси
включается тем же регулятором, что и качество превью (Draft/Preview —
прокси, Good/Best — оригинал). То есть Vegas СКЛЕИЛ «долю разрешения» и
«прокси» в один орган управления и тем закрыл вопрос. Premiere пошёл
ровно в другую сторону и сделал это явно: галочка Use Proxies живёт в
диалоге экспорта, и Adobe в анонсе подчёркивает, что она «is not affected
by enabling or disabling proxies while editing» — то есть состояние
монтажа и решение рендера у Premiere ДВА РАЗНЫХ, не связанных между собой
переключателя, да ещё и сохраняемых в пресет экспорта. Отсюда и берутся
треды «Premiere is exporting my proxies». FCP держит середину — «Proxy
Preferred» сам подставляет оригинал, когда прокси нет, а «Proxy Only»
честно отдаст рендер с прокси, и единственная защита Apple — предложение
в справке. Тот же вопрос, три несовместимых ответа.

**2. Совпасть прокси обязан по ЧАСТОТЕ КАДРОВ, ИМЕНИ и ЗВУКУ — а
разрешение как раз может отличаться.** Adobe требует, чтобы чужой прокси
назывался `<оригинал>_Proxy.<расш>` и лежал в отдельной папке рядом с
мастерами; Larry Jordan добавляет, что частота кадров прокси обязана
совпадать с мастером. Самое неочевидное — про звук: если у мастера больше
двух аудиоканалов (хотя бы и пустых), прокси нельзя делать в H.264,
потому что Premiere пишет H.264 только в MP4, а MP4 несёт два канала —
остаются ProRes Proxy или DNxHR LB. То есть «сделаю прокси ffmpeg'ом в
h264» ломается не об видео, а об аудио, и ломается молча.

**3. Прокси другого разрешения меняет не скорость, а КАРТИНКУ.** Эффект,
параметр которого задан в пикселях, считается в пикселях ТЕКУЩЕГО кадра:
блюр радиусом 20 px на прокси 1/4 покрывает вчетверо большую долю
изображения, чем тот же блюр на оригинале. Ловушка не гипотетическая — на
неё наступают даже без прокси, просто работая на пониженном разрешении
превью. И к нам она относится напрямую: у нас доля разрешения превью
1/16…1/1 уже есть, а масштабируется ею РОВНО ОДИН эффект — трёхмерный
(`Cam3d.world_scale`); `Blur`, `Glow`, `Edges`, `Noise`, `Vhs` с единицей
`Px` считаются в пикселях кадра как есть.

## Плюсы

- Экономия достаётся ДЕКОДУ — единственной статье расхода, которую не
  уменьшают ни кэш результата, ни доля разрешения превью: те работают уже
  после того, как кадр раскодирован.
- Подмена источника не трогает монтаж. Клип адресует материал временем и
  идентификатором медиа, а не файлом, поэтому тумблер — одна операция над
  медиа, а не переписывание сорока событий.
- Прокси — файлы на диске: они переживают перезапуск программы, их можно
  сделать один раз на всю студию и раздать по сети, а прокси с площадки
  (камерные) подцепить как есть. Прогретый кэш умирает вместе с
  процессом, а доля разрешения вообще никуда не передаётся.
- Прокси ставятся пофайлово: тяжёлый материал проксируется, лёгкий
  остаётся как есть — «оптимизировать» проект целиком не требуется.
- У Resolve видно, чем платят за отказ от файлов: Optimized Media быстрее
  заводится, но «cannot be exported, and is not user accessible» — то
  есть с коллегой не поделишься и на другую машину не увезёшь.

## Минусы

- За одним медиа в каждый момент стоит ровно один открытый файл, поэтому
  переключение тумблера обнуляет всё прогретое по этому источнику:
  оригинал и прокси не могут лежать в кэше одновременно, не раздвоив
  правду о прогретости.
- Прокси — второй набор файлов: место на диске, необходимость
  пересоздавать их при правке оригинала, вопрос «где мои прокси» при
  переносе проекта и отдельная процедура переподключения на сотнях
  клипов.
- Эффект с параметром в пикселях даёт на прокси другую картинку. Это не
  дефект реализации, а следствие того, что эффект не знает, в каком
  масштабе живёт.
- Прокси кодируются с потерями и обычно в 8 бит 4:2:0, поэтому всё, что
  оценивается по тонким градациям — банды, шум, край ключа, результат
  грейда — на прокси оценивать нельзя. Прокси ускоряют монтаж, но не
  цветокоррекцию: там нужен полноценный интермедиат (Optimized Media в
  Resolve, ProRes 422 в FCP), а это уже не «лёгкая копия».
- Генерация — полный проход «декод + энкод» по каждому исходнику: на часе
  4K это десятки минут, которые нужно где-то показать, дать отменить и
  пережить закрытие диалога.
- Требования к совпадению (имя, частота кадров, число аудиоканалов) нигде
  не проверяются заранее: узнаёшь о них по молчаливому отказу привязки
  или по разъехавшейся синхронизации.

## Что говорят пользователи

Экспорт с прокси случается и с людьми, которые знают, что делают:

> HELP! Premiere is exporting my proxies instead of the source file
> — заголовок треда markalext, Adobe Community, https://community.adobe.com/questions-729/help-premiere-is-exporting-my-proxies-instead-of-the-source-file-1390164

> If You used the proxies in the timeline, that is not the way to go.
> — Ann Bens (Community Expert), Adobe Community, там же

Ответ эксперта в том же треде объясняет, почему обратной дороги нет: «you
will have to replace all proxy files on the timeline with the original to
get it exported with the original files» — прокси, попавший в монтаж как
обычный клип, перестаёт быть прокси и становится материалом.

Пониженное разрешение меняет картинку, и это выясняется в конце проекта:

> I typically preview at half resolution to use less CPU/RAM, and unfortunately, I just finished an entire project only to realize that when I render or preview at the full 1080p resolution, any text layers that have layer styles enabled are rendering completely wrong.
> — James Inniani, Creative COW, 15.10.2018, https://creativecow.net/forums/thread/ae-half-resolution-preview-not-accurate/

> Be careful working at a resolution other than 'full', as many things like plugins, not just styles, can render incorrectly or in unexpected ways.
> — Andrew Somers, тот же тред, 15.10.2018

Привязка чужих прокси — отдельная история; требования Adobe озвучил свой
же сотрудник, и они про ИМЕНА, а не про картинку:

> The Proxies must have names matching the Full Res/original with '_Proxy' appended to the name. e.g.: Full Res = scene01.mxf, Proxy = scene01_Proxy.mov
> — Peter Garaway (Adobe), Creative COW, 24.10.2016, https://creativecow.net/forums/thread/proxies-do-not-automatically-attach-to-original-fo/

В том же треде — цена ошибки на большом проекте: автор поста работает с
«over 500 clips», а другой участник (02.02.2021) пишет про RED-прокси
«These can only be attached one at a time – trying to do so as a batch
usually causes the programme to crash».

Где физически лежат прокси, пользователи выясняют друг у друга:

> If you look in the folder of your source media the proxy files will have the same name but the extension will be '*.sfvp0'.
> — George Dean, Creative COW, 07.02.2019, https://creativecow.net/forums/thread/problems-withquestions-about-video-proxies/

Разницу «прокси против оптимизированного материала» в Resolve на форуме
BMD объясняют цитатой из документации самого установщика:

> Optimized Media is managed internally by DaVinci Resolve, cannot be exported, and is not user accessible. In contrast, Proxy Media creates fully portable and independent media that can be easily managed by the user.
> — Singularity, forum.blackmagicdesign.com, 12.11.2020, https://forum.blackmagicdesign.com/viewtopic.php?p=686710

Чего найти НЕ удалось: живой цитаты про прокси, разъехавшийся с
оригиналом по длительности или таймкоду («proxies out of sync»). Искал по
`creativecow.net`, `community.adobe.com` и через поисковые запросы вида
«proxy different duration / out of sync / timecode mismatch» — выдача
уходит либо в reddit (недоступен из этого окружения), либо в
видео-туториалы, из которых цитировать нечего. Требование совпадения
частоты кадров подтверждено первоисточником (Larry Jordan), само
расхождение длительностей — пока только по документации.

## Как ложится на Kinotrain

### Что уже есть

**Три экономии, из которых у нас две.** Прокси подменяют ИСТОЧНИК, кэш
сохраняет РЕЗУЛЬТАТ, доля разрешения уменьшает РАБОТУ. Наши две:

- *Доля разрешения превью* 1/16 … 1/1 — `draw_preview_controls` в
  `src/apps/kt-app/src/app.rs` (массив `fracs`, `q_for`,
  `Player::set_preview_quality`). Доля живёт «поверх» разрешения проекта:
  при смене разрешения сохраняется доля, а не пиксели. Автоматического
  понижения под нагрузкой нет — выбор всегда за человеком.
- *Кэш* — `src/libs/kt-cache/src/lib.rs`: пре-FX
  `BurstKind::Video{quality}` и пост-FX мемоизация
  `PostFxVideo{quality, fx_hash}` (idx = таймлайн-тик). `quality` — часть
  ключа: записи разных качеств не взаимозаменяемы, и coverage считается
  отдельно на каждое качество (инвариант №4).

Заодно ответ на вопрос «прокси — это та же ось, что `quality`?»: **нет**.
`quality` — это запрос К ДЕКОДЕРУ на размер выдачи внутри ОДНОГО
источника. Прокси — другой файл за тем же `MediaId`. Это разные оси, и
смешивать их в ключе кэша нельзя (см. «Что мешает»).

**Экономия доли разрешения НЕ трогает декод.** `FfmpegSource`
декодирует кадр целиком через `avcodec`, а `Quality` применяется потом, в
`sws_scale` (`src/libs/kt-ffmpeg/src/source_impl.rs`). Значит 1/16
экономит swscale, цепочку FX, композит и байты кэша — но 4K H.265 всё
равно декодируется полностью, кадр за кадром. Именно эту статью расхода
закрывают ТОЛЬКО прокси, и это главный аргумент за них.

**Машинерия переключателя уже написана и покрыта тестами.**
`Player::replace_media` (`src/libs/kt-engine/src/player.rs`) делает ровно
то, что нужно кнопке «прокси/оригинал», и в правильном порядке: гасит
`MediaWorker` → `cache.invalidate_source` → поднимает новый воркер →
бампает `project_version` (иначе префетч в простое не заметит смены) →
будит цикл. Транспорт, позиция, FX-состояние и кэш остальных источников
не трогаются. Фоновая часть — `KtApp::reopen_rx` и поток `reopen-media` в
`src/apps/kt-app/src/app.rs`. Тесты t209 (заглушка), t210 (рендер с
заглушками), t211 (вернувшийся файл подменяет заглушку) — это тот же
сценарий «за `MediaId` встал другой файл», только повод другой. Это
сильный аргумент «дёшево»: самая нервная часть фичи — подмена источника
на ходу без потери позиции и FX-хвостов — уже отлажена.

**Sidecar-механика «файл рядом с проектом» отработана.**
`<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`,
`docs/media-index.md`) уже хранит probe, пакетный индекс, peak-пирамиду и
тумбы; идентичность файла проверяется `size + mtime + xxhash`, и
перестраивается ТОЛЬКО затронутое медиа. Таблица привязки «медиа →
прокси-файл» добавляется туда аддитивно, без бампа `SCHEMA_VERSION` — в
`index.rs` для этого есть готовый приём (`CREATE TABLE IF NOT EXISTS` на
каждом открытии, так заведена таблица `poster`).

**Кодировщик есть.** `kt-ffmpeg::Muxer`
(`src/libs/kt-ffmpeg/src/muxer_impl.rs`) принимает RGBA-кадры и f32-аудио
и пишет MP4 h264+AAC системным FFmpeg с автовыбором аппаратного энкодера
(инварианты №9, №10). Генерация прокси — это НЕ экспорт таймлайна: это
отдельный маленький цикл «читать `MediaSource` при уменьшенном `Quality`
→ писать в `Muxer`».

**В формате проекта место под прокси уже ЕСТЬ — и не наше.** OTIO держит
у клипа не одну ссылку на материал, а карту:
`Clip.media_references: IndexMap<String, Option<MediaRefNode>>` +
`active_media_reference_key` (`src/libs/kt-otio/src/schema.rs`),
`DEFAULT_MEDIA_KEY = "DEFAULT_MEDIA"`, плюс писаный апгрейд Clip.1 →
Clip.2. Мы читаем только активную ссылку, но ВСЮ карту возим passthrough
(`ClipExtra::media_refs` в `src/libs/kt-otio/src/convert.rs`) — то есть
проект, пришедший из чужого инструмента с ключом вида `proxy`, у нас уже
не теряется; мы просто не умеем на него переключиться.

### Чего не хватает

1. У медиа в НАШЕЙ модели один путь: `MediaRef { id, path }`
   (`src/libs/kt-model/src/project.rs`). Второго пути и признака «сейчас
   активен прокси» нет.
2. Нет транскодера «файл → файл». `export_mp4_with_backends`
   (`src/libs/kt-engine/src/export.rs`) рендерит КОМПОЗИЦИЮ и получает
   источники снаружи; для прокси нужен другой, куда более простой цикл.
3. Нет очереди фоновых заданий с прогрессом. Прокси на час материала —
   десятки минут работы, которые обязаны идти в фоне и переживать
   закрытие окна рендера.
4. Нет ни одного места в UI, где у ОТДЕЛЬНОГО медиа есть свойства и
   действия: у бина есть карточки и контекстное меню ГРУПП
   (`src/apps/kt-app/src/bin_panel.rs`), у медиа — нет.
5. Нет «attach proxies»: подцепить чужой (камерный) прокси негде и нечем
   проверить, что он подходит.

### Что мешает

**Точка привязки — `SourceId`, и она равна `MediaId` один в один**
(`SourceId(clip.media.0)` в `src/libs/kt-engine/src/video.rs`). Это
хорошо: прокси естественно встаёт «за тем же `MediaId`», и весь движок —
план кадра, префетч, полоски прогретости, ключи пост-FX — ничего не
замечает; `kt-ffmpeg` и так заперт за трейтом `MediaSource`
(`docs/architecture.md`). Но платой идёт то, что оригинал и прокси НЕ
МОГУТ лежать в кэше одновременно: `invalidate_source` выносит все записи
источника, и каждое переключение тумблера обнуляет прогретость.
Заводить прокси третьей осью ключа кэша (рядом с `quality`) нельзя: это
удвоило бы coverage и сломало формулировку инварианта №4 «кэш —
единственная правда о прогретости» — правды стало бы две, и полоскам
пришлось бы выбирать, какую показывать.

**Прокси обязан совпасть с оригиналом по ДЛИТЕЛЬНОСТИ, и у нас на это уже
стоит грабля.** Ключ кадра в кэше — время, а не номер кадра:
`video_cache_idx(source_pts) = source_pts.0`
(`src/libs/kt-engine/src/video.rs`). Поэтому прокси с другим fps нам не
страшен — мы адресуем материал тиками, а не кадрами. Страшен прокси
другой ДЛИТЕЛЬНОСТИ, а наш собственный муксер делает ровно его:
`MuxConfig.fps` — целое число, `time_base = 1/fps`
(`src/libs/kt-ffmpeg/src/muxer_impl.rs`), а дробные частоты мы снапаем на
целую сетку (`grid_fps(23.976…) = 24`, t178 в
`src/libs/kt-otio/tests/convert.rs`). Прокси часового материала 23.976
fps, снятый нашим муксером на 24 fps, окажется короче оригинала почти на
четыре секунды — и хвост клипа поедет. Первая же реализация обязана либо
научить муксер дробным частотам, либо гарантировать длительность иначе.
Побочно: MP4 у нас несёт стерео (инвариант №1), поэтому ограничение
Premiere «h264-прокси не годится для многоканального звука» у нас
структурно то же самое — просто многоканального звука мы и так не знаем.

**Экспорт с прокси у нас предотвращается структурно, а не галочкой.**
Источники для рендера открывает НЕ движок: `render_dialog::open_all`
(`src/apps/kt-app/src/render_dialog.rs`) и одноимённая функция в
`src/apps/kt-cli/src/main.rs` идут по `MediaRef::path` из модели и
передают готовый список в `export_mp4_with_backends`. Пока подмена
прокси живёт ТОЛЬКО в плеерном пути (`Player::replace_media`), экспорт
физически не видит прокси-файла — это конструкция VEGAS, а не Premiere,
и она правильная. Но её надо не «не забыть», а закрепить: тест, который
включает прокси, рендерит и сравнивает с эталоном на оригиналах, дёшев и
прямо защищает инвариант №11.

**Пиксельные параметры FX уже врут на доле разрешения, и прокси это
усугубит.** В `params_for!` (`src/libs/kt-model/src/project.rs`) единицу
`Px` носят `Blur.radius/radius_y`, `Edges.thickness`, `Noise.size`,
`Vhs.rgb_shift/scan_step`, `Glow.radius`. Масштабируется долей разрешения
РОВНО ОДНО: 3D — `Cam3d.world_scale = quality.width / project.width`
(`src/libs/kt-engine/src/video.rs`, `src/libs/kt-engine/src/video_plan.rs`).
Блюр же реализован даунскейлом с шагом `(radius/3).max(1)+1` в пикселях
ТЕКУЩЕГО кадра (`src/libs/kt-fx/src/video.rs`), поэтому на превью 1/4
радиус 8 даёт вчетверо более грубое размытие относительно картинки, чем в
экспорте. Инвариант №7 формально цел (превью == экспорт байт-в-байт
проверяется на ОДНОМ качестве), но глазами монтажёра это ровно та беда, на
которую жалуются пользователи чужих прокси. Правило крейта kt-fx
(«умолчание нового параметра = прежнее поведение байт-в-байт») запрещает
починить это молча: масштабирование `Px`-параметров сдвинет golden'ы, и
решение об этом — отдельное, а не попутное.

### Какие инварианты задевает

- **№4** (кэш — единственная правда о coverage): переключение прокси
  идёт только через `invalidate_source`; отдельной оси в ключе не
  заводить.
- **№5** (декодером владеет ровно один MediaWorker): подмена делается
  заменой воркера, как в `replace_media`, а не вторым воркером на тот же
  `MediaId`.
- **№9 / №10** (экспорт внутрипроцессный, бинарь не линкуется с libav*):
  генератор прокси обязан идти через `kt-ffmpeg::Muxer`, а не звать
  внешний `ffmpeg`, хотя соблазн велик — это самая простая часть работы,
  которую проще всего сделать неправильно.
- **№11** (детерминизм): экспорт обязан идти с оригиналов. Единственный
  пункт, ради которого фича требует ТЕСТА, а не дисциплины.
- **№7** не нарушается: прокси меняет байты ИСТОЧНИКА, а не путь
  композиции.
- Правила UI `src/apps/kt-app/CLAUDE.md`: **20** («ничего не отрастает и
  не отрубается само» — прокси не включаются сами и не выключаются сами
  молча), **13** (тумблер обязан сесть на уже занятое место — например, в
  карточку бина, а не в новую полосу), **16** (нет прокси у этого медиа —
  это ОШИБКА жеста, а не блёклая подсказка), **12** (генерация — фоновая
  работа: не во время жеста и не во время воспроизведения).

## Оценка

| Что | Ответ |
|---|---|
| Объём | L — целый раунд: транскодер, фоновая очередь с прогрессом, второй путь у медиа, тумблер, защита экспорта |
| Где основная работа | `kt-media`/`kt-ffmpeg` (цикл «файл → файл» поверх `Muxer`), `kt-app` (очередь, прогресс, тумблер и признак в бине), `kt-model` (второй путь и активный выбор у медиа), `kt-otio` (маппинг в штатные `media_references`) |
| Формат проекта | своей версии НЕ требует: место штатное — `media_references` + `active_media_reference_key` OTIO, которые мы уже разбираем и возим passthrough. Тонкость: у OTIO карта ссылок висит на КЛИПЕ, а у нас медиа живёт в проекте один раз — при записи ссылка размножится по всем клипам медиа, и обратное чтение обязано это схлопывать |
| Тесты, которые придётся написать | экспорт при включённых прокси совпадает с эталоном на оригиналах (защита №11); длительность прокси равна длительности оригинала с точностью до кадра; переключение туда-обратно не двигает playhead и не рушит FX-состояние (расширение t211); round-trip `media_references` с непустым не-дефолтным ключом; аддитивная таблица привязки в `ktindex.db` не бампает `SCHEMA_VERSION` |
| Зависит от | ничего не блокирует; смежные — F-SPD-004 (таймкод и синхронизация), F-CLR-005 (приборы: их принципиально нельзя смотреть на прокси) |
| Ломает совместимость | нет |

## Вердикт

Берём, но не раньше, чем появится материал, на котором это видно: доля
разрешения превью закрывает всё, кроме декода, и пока проекты не упираются
в декод 4K/H.265, прокси экономят чужую боль, а не нашу. Аргумент «за» —
не производительность как таковая, а то, что половина фичи уже написана и
оттестирована (`Player::replace_media`, sidecar-БД, `Muxer`,
`media_references` в OTIO), и что защита от рендера с прокси у нас
получается по конструкции VEGAS — экспорт открывает свои источники по
`MediaRef::path` и прокси-файла не видит вовсе. Делать надо в этом
порядке: сначала тест «рендер идёт с оригиналов», потом всё остальное.

## Источники

- VEGAS Pro 23 Help, «Creating proxy files for high-definition editing»:
  https://help.magix-hub.com/video/vegas/23/en/content/topics/5-preview/creating_intermediate_files.htm
  (дата обращения 07.09.2026) — `.sfvp0` рядом с исходником, привязка к
  Preview Quality, «Video proxy files are not used for rendering your
  project». Страница относится к линейке 23; в 2026.0 механика не
  проверялась живьём — **[НЕ ПРОВЕРЕНО]**, что она не менялась.
- Final Cut Pro User Guide, «Create optimized and proxy files»:
  https://support.apple.com/guide/final-cut-pro/create-optimized-and-proxy-files-verb8e5f6fd/mac
  (07.09.2026) — ProRes 422 Proxy или H.264, «frame sizes from 12.5
  percent to 100 percent of the original», хранение в библиотеке или во
  внешнем storage location.
- Final Cut Pro User Guide, «Control playback quality»:
  https://support.apple.com/guide/final-cut-pro/ver2fd7a8b94/mac
  (07.09.2026) — определения Optimized/Original, Proxy Preferred, Proxy
  Only и предупреждение про переключение перед Share.
- Final Cut Pro User Guide, «Create a proxy-only project»:
  https://support.apple.com/guide/final-cut-pro/create-a-proxy-only-project-vere88b1ee9f/mac
  (07.09.2026).
- forum.blackmagicdesign.com, тред «Proxy media vs. Optimized Media»,
  12.11.2020: https://forum.blackmagicdesign.com/viewtopic.php?p=686710
  (07.09.2026; страница открывается только через читалку `r.jina.ai`) —
  цитата из документации установщика Resolve.
- Adobe Community, «HELP! Premiere is exporting my proxies instead of the
  source file»:
  https://community.adobe.com/questions-729/help-premiere-is-exporting-my-proxies-instead-of-the-source-file-1390164
  (07.09.2026) — заголовок и ответы Ann Bens. Даты постов на странице
  расходятся с упомянутой в них версией 23.3.1, поэтому дата поста здесь
  не цитируется — **[НЕ ПРОВЕРЕНО]**.
- Creative COW, «Proxies do not automatically attach to original
  footage», 23.10.2016 – 02.02.2021:
  https://creativecow.net/forums/thread/proxies-do-not-automatically-attach-to-original-fo/
  (07.09.2026) — требования Adobe к именованию (`_Proxy`), проблемы
  пакетной привязки RED-прокси.
- Creative COW, «Problems With/Questions About Video Proxies», 07.02.2019:
  https://creativecow.net/forums/thread/problems-withquestions-about-video-proxies/
  (07.09.2026) — `.sfvp0` в папке исходника.
- Creative COW, «AE half resolution preview not accurate», 15.10.2018:
  https://creativecow.net/forums/thread/ae-half-resolution-preview-not-accurate/
  (07.09.2026) — эффекты на пониженном разрешении.
- Larry Jordan, «Linking media to proxies created outside Premiere Pro»,
  24.07.2021:
  https://larryjordan.com/articles/linking-media-to-proxies-created-outside-premiere-pro/
  (07.09.2026) — совпадение частоты кадров, ограничение MP4 на два
  аудиоканала, выбор ProRes Proxy / DNxHR LB.
- OpenTimelineIO Python API, `opentimelineio.schema.Clip`:
  https://opentimelineio.readthedocs.io/en/latest/api/python/opentimelineio.schema.html
  (07.09.2026) — `media_references`, `active_media_reference_key`,
  `DEFAULT_MEDIA_KEY = 'DEFAULT_MEDIA'`. Назначение множественных ссылок
  спецификация НЕ поясняет: трактовка «это место под прокси» —
  **[НЕ ПРОВЕРЕНО]**, ни в `otio-file-format-specification`, ни в
  `versioning-schemas` про них ничего нет.
- Adobe Community (анонс), «New option to use proxy media when
  exporting»:
  https://community.adobe.com/announcements-732/discuss-new-option-to-use-proxy-media-when-exporting-313379
  (07.09.2026) — появление галочки в 14.3.2, её независимость от
  монтажного тумблера, приоритет «Use Previews», сохранение в пресет.
- Строки Premiere (пресет 1024×540, папка прокси) и Resolve 21 (кодеки
  и место генерации прокси, чекбокс
  «Use optimized media» в Render Settings) собраны из вторичных
  источников и сниппетов поиска: `helpx.adobe.com` из этого окружения не
  открывается, зеркало Resolve-мануала на `steakunderwater.com` отдало
  404 на нужные страницы. Обе строки помечены как проверенные по
  документации лишь частично — **[НЕ ПРОВЕРЕНО]** в части точных
  формулировок UI.
- Код Kinotrain: `src/libs/kt-cache/src/lib.rs`,
  `src/libs/kt-media/src/{lib.rs,index.rs}`,
  `src/libs/kt-ffmpeg/src/{lib.rs,muxer_impl.rs,source_impl.rs}`,
  `src/libs/kt-engine/src/{player.rs,video.rs,video_plan.rs,export.rs}`,
  `src/libs/kt-model/src/project.rs`,
  `src/libs/kt-otio/src/{schema.rs,convert.rs,lib.rs}`,
  `src/apps/kt-app/src/{app.rs,render_dialog.rs,util.rs,bin_panel.rs}`,
  `src/apps/kt-cli/src/main.rs`, `docs/architecture.md`,
  `docs/media-index.md` (прочитано 07.09.2026).
