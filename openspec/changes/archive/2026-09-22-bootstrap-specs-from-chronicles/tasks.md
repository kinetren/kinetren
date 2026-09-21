# Tasks

Все команды `openspec …` — через Node 24: `export PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"`.
Порядок — по design.md D4: сначала все 17 capability, потом усечение источников,
потом конфигурация и заморозка. До раздела 4 старые летописи остаются целыми.

## 1. Подготовка и инвентарь

- [x] 1.1 Снять базовую линию: `openspec validate --all --strict` и `python3 src/scripts/features-index.py --check` зелёные, вывод сохранён в scratchpad; проверка — оба кода возврата 0
- [x] 1.2 Построить карту «tNNN → capability» из таблицы «Карта крейт → обязательные тесты» корневого `CLAUDE.md` по таблице D2 design.md (скрипт в scratchpad: извлечь все `t\d{2,3}[a-g]?` по строкам таблицы, приписать capability); проверка — каждый tNNN таблицы приписан ровно одной capability, список неприписанных пуст или явно разложен вручную
- [x] 1.3 Построить карту «источник → capability» для трёх летописей: главы `docs/porting-notes.md` (по `## `/`### `), разделы раундов `docs/user-feedback.md` (`## Раунд Р.N`), заметки `docs/features/*` со статусом `сделано`/`принято`/`уже-есть`/`отвергнуто` — по таблице D2; проверка — карта сохранена в scratchpad, у каждой из 17 capability есть хотя бы один источник, у каждой «нашей» заметки есть capability
- [x] 1.4 Заготовить формат файлов: `spec.md` = `# <Имя>` / `## Purpose` / `## Requirements` с `### Requirement:` и `#### Scenario:` (WHEN/THEN + последняя строка `- **TEST** tNNN (`путь/tests/файл.rs`)`); `design.md` = разделы «Что было и у кого болело», «Решения и почему», «Отвергнуто и почему», «Неочевидные детали (пути src/…)», «Отложено» — проверка: пробная capability из 2.1 проходит `openspec validate --specs --strict`

## 2. Capability: spec.md + design.md (по одной, каждая проверяется отдельно)

Общая проверка каждой задачи раздела: `openspec validate --specs --strict` зелёный;
`openspec show <cap> --type spec --json` возвращает все требования; каждый сценарий
несёт строку `TEST`; все tNNN, приписанные capability в 1.2, встречаются в её `spec.md`
(скрипт-сверка из scratchpad); в `design.md` нет хронологии («в Р.N сначала…») и
сравнений с C++ без правила, к которому они ведут.

- [x] 2.1 `time-model` — из «Модель времени» CLAUDE.md, kt-time; проверка общая + решение по открытому вопросу design.md: если требований < 3 — записать в design.md capability пометку «слить в transport в 2.2» и выполнить слияние там
- [x] 2.2 `transport` — инв. 6/12, Р.43 (часы под голоданием, t284), Р.57 (шкала со знаком, t603/t604/t616–t619, t626–t629), kt-engine/CLAUDE.md, user-feedback Р.43/Р.57; проверка общая
- [x] 2.3 `audio-render` — инв. 1–3, каталог дефектов slowed&reverb (a–d → правила без C++-путей), cost.rs t292–t295, Р.48 (PDC t334–t341), loudness t403–t407, sync t413–t419, features audio/processing (8 «сделано») и loudness/audio-waveform-sync; проверка общая
- [x] 2.4 `video-plan` — инв. 4/7/11, Р.44–46 (ступень декода t310–t317, слияние гео t296–t309, память t66), kt-gpu/CLAUDE.md, features transform-concatenation; проверка общая
- [x] 2.5 `video-fx` — Р.47 реестр (t322/t328/t329/t330–t333), Р.54 миниатюры (t543–t551), features fx/* (4 «сделано»), color/basic-color-fx, preset-gallery; проверка общая
- [x] 2.6 `audio-fx` — реестр 16 узлов (t319–t327), features audio/processing по цепочке узлов; проверка общая
- [x] 2.7 `transitions` — Р.33 С4, Р.51 (t223–t226, t232, t459–t468), пара rs+wgsl, features transition-vocabulary, same-track-transitions; проверка общая
- [x] 2.8 `automation` — Р.33–34, §466–467, props t641–t649, fx_auto t190/t212–t215/t248, features keyframe-domain, presets-carrying-automation, expressions-and-drivers (принято — только интерфейс), plugin-abi-and-sandbox (принято — только интерфейс); проверка общая
- [x] 2.9 `morpher` — H1 (t35–t36), morph_slice (t233–t249, t555–t556), п.300 (подмена источника везёт кривые, t291), user-feedback про Adjustment Events; проверка общая
- [x] 2.10 `compositions` — Р.50/Р.55/Р.56/Р.58 (t423–t458, t552–t602, t631–t640), §459–464 (в т.ч. отложенное разрастание .otio → раздел «Отложено»), §469–470 и nest_param t671–t690, features nested-sequences, inline-nest-expansion, composition-public-parameters; проверка общая
- [x] 2.11 `generators` — Р.52–53, §468 (t469–t495, t656–t670), kt-text/CLAUDE.md (шрифт/пины/FONT-MANIFEST), features titles/* (5 «сделано»), solid-gradient-shapes; проверка общая
- [x] 2.12 `media-index` — инв. 5/8/10, docs/media-index.md, kt-media/kt-ffmpeg (t10/t21/t34/t37/t70/t208), Р.49 волна Б (t398–t412, t420–t421), kt-cache (t8/t14/t27/t28/t37/t65); проверка общая
- [x] 2.13 `export` — инв. 9, golden_export (11, 25, 33, 67, 479), export_loudness t406–t407, features hardware-encoder-selection, preview-vs-export-mismatch; проверка общая
- [x] 2.14 `project-format` — docs/file-format.md, kt-otio/CLAUDE.md (t144–t154, t164–t165, t178–t180, t189, t192, t212, t222, t240–t251, t431–t437, t465, t473–t474, t494, t593, t606, t638, t664–t665, t681), §464; проверка общая
- [x] 2.15 `durability` — Р.49 (t351–t391, journal t354, lock t362), features resilience/* (6 «сделано»), user-feedback «ты не проверял» (правило finish.sh + живые сценарии); проверка общая
- [x] 2.16 `ui-timeline` — kt-app/CLAUDE.md (repaint/часы показа/чек-лист удобства/локализация/шрифты), layout_tests и font/i18n тесты из таблицы CLAUDE.md, таблица «повторял больше одного раза» user-feedback (пределы «поставлено по жалобе»); проверка общая
- [x] 2.17 `ui-reach` — Р.38/Р.40 (t252–t255, t273), волна В Р.49 (t392–t396), features help/* (4 «уже-есть», accelerator-visibility «сделано»); проверка общая
- [x] 2.18 Сводная сверка раздела: скрипт из 1.2 подтверждает, что КАЖДЫЙ tNNN таблицы CLAUDE.md встречается ровно в одной `openspec/specs/*/spec.md`; список 17 (или 16 после 2.1) каталогов совпадает с D2; `openspec list --specs --json` показывает их все

## 3. Усечение источников (только после 2.18)

Предпосылка (записанное допущение): усекаются только заметки со статусом
`сделано`/`принято`/`уже-есть`/`отвергнуто` — их «Как ложится» есть решение или причина
отказа. У заметок `кандидат` раздел остаётся: это ещё не решение; их усечёт тот раунд,
который их возьмёт, по новому правилу README.

- [x] 3.1 Перед усечением каждой заметки сверить её «Как ложится» с `spec.md`/`design.md` capability: все пути `src/…` и tNNN из раздела либо есть в capability, либо переносятся туда (риск 1 design.md); проверка — скрипт в scratchpad печатает по заметке список непокрытых путей/тестов, список пуст
- [x] 3.2 Усечь «Как ложится на Kinotrain» в заметках `сделано` (30 шт.) до ≤ 5 строк: одна строка `CAPABILITY: openspec/specs/<cap>/` + суть решения; «Вердикт» не трогать (правило 7 README — заметка заморожена, но вырезка «нашего» разрешена этим change явно); проверка — `wc -l` раздела ≤ 5 у каждой, строка `CAPABILITY:` есть
- [x] 3.3 То же для заметок `принято` (3), `уже-есть` (7) и `отвергнуто` (6; у отвергнутых причина остаётся в «Вердикте» и дублируется в `design.md` capability, раздел «Отвергнуто и почему»); проверка — как в 3.2
- [x] 3.4 `docs/features/README.md`: правило 5 переписать — «Как ложится» обязана нести строку `CAPABILITY: openspec/specs/<cap>/` (существующий каталог) и у статусов `сделано`/`принято`/`уже-есть`/`отвергнуто` быть ≤ 5 строк; у `кандидат`/`исследовано` пути `src/…` по-прежнему обязательны; правило 8 дополнить: «проектирование живёт в change OpenSpec»; проверка — текст правил обновлён, таблица покрытия README не сломана
- [x] 3.5 `src/scripts/features-index.py --check`: реализовать правило 3.4 (проверка существования каталога capability; проверка «src/ обязателен» только для незамороженных статусов; лимит строк для замороженных); проверка — `--check` зелёный на дереве после 3.2–3.3 и красный на заметке-пробе без `CAPABILITY:` (проба удаляется)

## 4. Конфигурация OpenSpec и заморозка летописей

- [x] 4.1 `openspec/config.yaml`: `context` — модель времени и 12 инвариантов по строке каждый со ссылкой на capability, Node-24 замечание; `rules.proposal` — «перед Capabilities прочитать `openspec/specs/<cap>/design.md` каждой затрагиваемой capability»; `rules.design` — «решение, противоречащее `design.md` capability, оформляется пунктом `Пересмотр: <решение> — <причина>`»; `rules.tasks` — «работа заканчивается `src/scripts/finish.sh` и версией в отчёте»; проверка — `openspec instructions proposal --change <любой> --json` возвращает `context` и `rules` без предупреждений о неизвестных артефактах
- [x] 4.2 Корневой `CLAUDE.md`: таблицу «Карта крейт → обязательные тесты» заменить абзацем «контракты и обязательные тесты — `openspec/specs/<cap>/spec.md`, почему — `design.md` рядом; правишь крейт → `openspec show <cap> --type spec`»; инварианты 1–12 оставить по одной-две строки каждый с указанием capability, историю пунктов (п.NNN) из них убрать; проверка — в файле нет `t\d{3}` вне указателя, каждый инвариант ссылается на существующий каталог capability
- [x] 4.3 `docs/porting-notes.md`: в шапку добавить блок «ЗАМОРОЖЕНО на §470 (2026-09-21). Актуальные правила и тесты — `openspec/specs/`; новые дефекты — в change OpenSpec»; тело не менять; проверка — `git diff --stat` файла показывает только шапку
- [x] 4.4 `docs/architecture.md`: решить открытый вопрос design.md — если после переноса в нём остаётся только то, что есть в `design.md` capability, заменить файл оглавлением «capability → spec/design» (по строке на capability); иначе оставить с указателем; проверка — файл ссылается на все каталоги `openspec/specs/*`
- [x] 4.5 `CLAUDE.md` крейтов не трогать (в объёме нет), но в каждый добавить одну строку-указатель «контракты крейта — `openspec/specs/<cap1>,<cap2>`»; проверка — у каждого из 10 крейтовых CLAUDE.md есть строка, каталоги существуют

## 5. Проверка целиком

- [x] 5.1 `openspec validate --all --strict` зелёный; `python3 src/scripts/features-index.py --check` зелёный; `git diff --stat` не содержит файлов под `src/` кроме `src/scripts/features-index.py` и `src/**/CLAUDE.md`; `cd src && cargo test -p kt-time` (дешёвый дым, что дерево цело)
- [x] 5.2 Живая проверка правила D1: завести временный change `probe-rules`, выполнить `openspec instructions proposal --change probe-rules --json` и `… design …`, убедиться, что `rules` содержат обязанность читать `design.md` capability; временный change удалить; проверка — `openspec list --json` не показывает `probe-rules`
- [x] 5.3 Сводка в отчёт: число требований и сценариев по capability, число усечённых заметок, строк удалено из «Как ложится», список tNNN без capability (должен быть пуст)
