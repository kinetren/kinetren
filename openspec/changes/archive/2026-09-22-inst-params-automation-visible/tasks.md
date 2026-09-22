# Tasks

Раунд Р.60. Каждый раздел — коммит после зелёной проверки (правило автокоммита).
`openspec` — через Node 24: `export PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"`.

## 1. Летопись и модель

- [x] 1.1 `docs/user-feedback.md`: раздел «Раунд Р.60 (2026-09-22)» — претензия владельца дословно (кривые без кнопки, min/max в 0), разведка («+»-глиф, `bound_apply` по курсору, `NaN → 0`), принятые решения D1–D4; проверка — раздел на месте, таблица «повторял» не тронута
- [x] 1.2 kt-model: `Project::addr_spec(seq, &ParamAddr) -> Option<ParamSpec>` рядом с `addr_type` (`src/libs/kt-model/src/project.rs`), `clip_attr_spec` рядом с `clip_attr_type` (`published.rs`), ветка `Nested` — рекурсивно с наложением сужения; проверка — t694 (`src/libs/kt-model/tests/pubparam.rs`): для каждого `ParamOwner` паспорт есть, тип равен `addr_type`, у исчезнувшей цели `None`
- [x] 1.3 kt-model: нормализация сужения — `PubRange::narrowed(spec, lo, hi)` хранит `None` там, где значение равно пределу цели (допуск — шаг паспорта); проверка — t692 (`src/libs/kt-model/tests/pubparam.rs`): публикация без сужения → `is_empty()`, сужение `max` → только `max`, возврат → `None`; файл без сужений байт-в-байт прежний (сверка с t681)

## 2. Окно автора: пределы от паспорта цели

- [x] 2.1 `pubparam.rs`, `draw_published_win`: поля `min`/`max` показывают `range.min.unwrap_or(spec.min)`/`spec.max` через `addr_spec`; правка пишет через `PubRange::narrowed`; кнопка «к пределам цели» (`icons::CLEAR`-стиль, подпись в i18n) → `PubRange::default()`; подсказка `widgets::hint`; проверка — t692b (`src/apps/kt-app/src/layout_tests.rs`): после публикации проба `pub.row.min.<id>` показывает `spec.min`, не 0; сужение и сброс доезжают до модели
- [x] 2.2 i18n ×5 каталогов (`pubp.range_reset`, `pubp.range_hint`, `pubp.auto_add_hint`, `pubp.auto_hint`), при новых глифах — `python3 scripts/subset-fonts.py`; проверка — t100, t125, t128, t171 зелёные

## 3. Окно потребителя: виджет автоматизации вместо глифа

- [x] 3.1 `InstParamsWin` получает `view: AutoView`; `draw_value_row` принимает `Option<&mut CurveBind>` и `spec` (пределы цели + сужение) вместо прямого `bound_apply`; глиф `+` и проба `inst.curve.<id>` удаляются; проверка — компиляция, t683 переписан (ниже)
- [x] 3.2 Секция «Автоматизация» под сеткой: меню `fx.auto_add` по ручкам числа/цвета без кривой (пункт — имя автора дословно), стартовая кривая — константа текущего значения на весь материал экземпляра (как у слота), затем `automation::draw` с `AutoCtx` из D1 (`range` = материал экземпляра, `to_curve`/`to_timeline` через `clip_material_at` с реверсом, `linked` из `UiState`); `AutoResult.seek/set_linked/set_view` доводятся до плеера и главного таймлайна, как в `gen_ui.rs`; результат окна по-прежнему одним `SetOverride` с гестурой; проверка — t691 (`src/apps/kt-app/src/layout_tests.rs`): без кривой две правки при разных курсорах → одно значение, ноль ключей; после меню — проба виджета найдена, две правки → два ключа в материале экземпляра, таблица их показывает; `set_view` при замке двигает главный таймлайн; проба `inst.curve.*` не находится
- [x] 3.3 Переписать t683: часть «число с кривой садит ключ в материале экземпляра» идёт через меню и `CurveBind`, остальные утверждения (каждый тип своим виджетом, `ClearOverride`) без изменений; проверка — t683, t684, t685 зелёные
- [x] 3.4 Подсказки строки состояния у меню кривой и кнопок виджета в окне экземпляра (`widgets::hint`); проверка — t693 (`reach_tests.rs`): наведение на действие даёт текст в строке состояния (t392/t393 — про полосу уровня и акселераторы, к подсказкам виджетов отношения не имеют)
- [x] 3.5 Сторож «один виджет на все носители»: t693 (`src/apps/kt-app/src/reach_tests.rs`) — скан исходников kt-app: вызовы `bound_apply`/`bound_value` только из `widgets.rs` (`CurveBind`) и `automation.rs`; список окон с правкой по времени (слот, генератор, экземпляр) — у каждого проба `automation::draw`; проверка — t693 зелёный и красный на пробе с прямым `bound_apply` (проба удаляется)

## 4. Живой сценарий и финал

- [x] 4.1 `src/scripts/live/nestparam.py`: после переопределения ширины — открыть меню кривой, завести кривую, сдвинуть курсор, изменить значение, проверить в `.otio` два ключа у `overrides[].curves` в материале экземпляра и скриншот с графиком; проверка — `./src/scripts/live-check.sh nestparam` зелёный, скриншоты в `target/live-out`
- [x] 4.2 Обязательные тесты capability: `cargo test -p kt-model --test pubparam`, `cargo test -p kt-app` (reach t252–t255, layout t680–t685, t691–t693, font/i18n t100/t125/t128), `cargo test -p kt-engine --test nest_param` (t674–t688 — модель кривой не тронута), `cargo clippy --all-targets`; проверка — всё зелёное
- [x] 4.3 `openspec/specs/compositions/design.md` и `automation/design.md`: раздел «Неочевидные детали» — почему у окна экземпляра нет своего редактора ключей и почему пределы не копируются (D1/D3); `docs/features/timeline/structure/composition-public-parameters.md` — статус `сделано` (заморозить, `CAPABILITY:` уже есть); проверка — `openspec validate --all --strict`, `features-index.py --check`
- [x] 4.4 `src/scripts/finish.sh` — пересборка kt-app/kt-cli, версия `vYY.MM.DD-hash` в отчёте; проверка — скрипт зелёный, версия названа
