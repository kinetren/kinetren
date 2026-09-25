//! KtApp: состояние приложения, панели, клавиши, дропы, lifecycle.

use crate::audio_out;
use crate::i18n::{k, tr};
use crate::timeline::{CtxTarget, Drag};
use crate::util::*;
use eframe::egui;
use kt_engine::Player;
use kt_model::*;
use kt_time::Tick;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Важность уведомления (п.18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
}

/// Вопрос, на который программа обязана получить ответ ДО действия (Р.49).
/// Отдельный тип, а не флаг на каждый случай: вопросов будет несколько
/// (занятый проект, несохранённая работа), и все они рисуются одним местом —
/// иначе у каждого заведётся своя кнопка «Отмена» со своим поведением.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// Проект занят другой копией программы — писать всё равно?
    SaveOverLock { pid: Option<u32> },
    /// В редакторе несохранённая работа, а пользователь уходит.
    Unsaved(Pending),
    /// На окно брошен ЧУЖОЙ ПРОЕКТ (Р.55, С6): открыть его или вставить
    /// композицией. Претензия владельца дословно: «я переношу файл проекта
    /// на таймлайн и открывается новый таймлайн, а я хочу чтобы оно дало
    /// мне выбор». До этого раунда выбора не было — и, что хуже, не было
    /// и вопроса о несохранённой работе: она пропадала молча.
    DropOtio,
}

/// Что программа собиралась сделать, когда спросила про несохранённое.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pending {
    New,
    Quit,
    Open,
    /// Открыть ПРОЕКТ, БРОШЕННЫЙ НА ОКНО (Р.55, С6). Путь лежит отдельным
    /// полем `KtApp.pending_drop`: `Pending` — `Copy`, и `PathBuf` в него
    /// не кладётся.
    DropOpen,
}

/// Ответ на вопрос. Три состояния, а не `bool`: «не сохранять» и «отмена» —
/// РАЗНЫЕ ответы (первый продолжает действие, второй отменяет его), и путать
/// их значит терять работу по нажатию кнопки «Отмена».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Yes,
    No,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct Notice {
    pub at: String,
    pub severity: Severity,
    pub text: String,
    /// Сколько раз подряд пришло это же сообщение (показывается «×N»).
    pub repeats: u32,
}

/// Результат фонового переоткрытия: медиа, имя для сообщения и источник
/// (None — файл снова не открылся, заглушка остаётся).
pub type ReopenedMedia = (MediaId, String, Option<Box<dyn kt_media::MediaSource>>);

/// Окно «как назвать вставленное» (Р.35 п.4).
/// Диалог автодакинга (Р.48, п.341): по какой дорожке считать паузы и
/// насколько приглушать. Снимок, а не живой сайдчейн — поэтому это
/// обычная правка модели с одним undo-шагом.
/// Диалог «сохранить цепочку пресетом» (Р.48, п.344). Пресет — файл
/// ПОЛЬЗОВАТЕЛЯ, а не модель: undo проекта его не откатывает.
pub struct PresetSaveDialog {
    /// Что сохраняем (цепочка на момент нажатия).
    pub block: kt_model::ops::FxChainBlock,
    pub name: String,
    /// Переименование существующего вместо создания нового.
    pub rename_of: Option<String>,
    pub focus: bool,
}

/// Уровень пути внутрь вложений (Р.50, В1).
///
/// Хранит ВИД СНАРУЖИ целиком: выйти обязано означать «вернулся туда же»,
/// а не «оказался в начале таймлайна с пустым выделением» (правило 14 —
/// фича работает повторно, и правило 5 — действие не гасит соседнее
/// состояние).
#[derive(Clone)]
pub struct NestStep {
    /// Где стояли ДО входа (`None` — корень).
    pub parent: Option<kt_model::MediaId>,
    /// Через какой экземпляр вошли: по нему переводится курсор обратно.
    pub inst: ClipId,
    /// Имя ленты — для крошек.
    pub name: String,
    pub view_start: f64,
    pub ticks_per_px: f64,
    pub tl_scroll: f32,
    pub cursor: Tick,
    pub region: Option<kt_model::Region>,
    pub selected: std::collections::BTreeSet<ClipId>,
    /// Выделение служебных строк. Хранится ровно потому же, почему и
    /// `selected`: `enter_nest` его чистит (внутри ленты оно бессмысленно),
    /// и без возврата выход переставал быть тождеством — правило «жест
    /// работает повторно» ломалось на морф-слое и камере.
    pub selected_morph: std::collections::BTreeSet<MorphId>,
    pub selected_camera: std::collections::BTreeSet<kt_model::CameraId>,
    /// Экземпляр, РАСКРЫТЫЙ в покидаемом контексте на момент входа (Р.58,
    /// п.461) — в том числе сам `inst`, если входили через его полосу
    /// вторым Enter. Решение владельца Р.58 (пересмотр Р.56 п.5):
    /// восстанавливать ВСЁ, включая входную полосу, — каскад Enter/Esc
    /// симметричен. Хранится id, а не `Band`: на возврате полоса
    /// ПЕРЕсчитывается `expand_band` (содержимое ленты могло измениться
    /// внутри — базы и длина другие); исчез экземпляр или перестал быть
    /// вложением — молча без полосы.
    pub band: Option<ClipId>,
}

/// Окно имени ленты (Р.50). Та же форма, что у окна имени пресета: живой
/// предпросмотр, Enter/Esc, фокус отдаётся при закрытии (правило 14).
pub struct SeqNameDialog {
    pub seq: kt_model::MediaId,
    pub name: String,
    pub focus: bool,
}

#[derive(Clone)]
pub struct DuckDialog {
    /// События, к которым применится кривая (выделение на момент вызова).
    pub targets: Vec<kt_model::ClipId>,
    /// Дорожка-источник (та, под которую приглушаем).
    pub src_lane: u32,
    pub params: kt_engine::ducking::DuckParams,
}

pub struct PasteNamesDialog {
    /// Позиция вставки, замороженная на момент Ctrl+V.
    pub at: kt_time::Tick,
    /// И СТРОКА вставки — по той же причине: под окном человек может
    /// увести и курсор, и мышь (Р.56, п.450).
    pub row: Option<(LaneKind, u32)>,
    /// Слово-приставка и сторона (Р.38 п.19: умолчание — СУФФИКС).
    pub affix: String,
    pub before: bool,
    /// Забрать фокус в поле на первом кадре — и ОТДАТЬ при закрытии
    /// (правило 14: фича обязана работать повторно).
    pub focus: bool,
}

/// Что делает Enter по выделенному вложению (Р.56, п.451).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterOnNest {
    /// Свёрнутое — раскрыть НА МЕСТЕ (режим по умолчанию).
    Expand,
    /// Уже раскрытое — ПОЛНЫЙ ВХОД: лента играет со своего нуля.
    Enter,
    /// Не лента — клавише тут делать нечего.
    NotANest,
}

/// Рамка выделения в домене модели: [начало, конец) и задетые дорожки.
pub type Band = (kt_time::Tick, kt_time::Tick, Vec<(LaneKind, u32)>);

pub struct KtApp {
    pub model: TimelineModel,
    pub project_path: Option<PathBuf>,
    /// Правки СОДЕРЖИМОГО (клипы, FX, morpher, метки, дорожки, медиа):
    /// только они дают звёздочку и запускают автосейв (п.5 р.11).
    pub dirty_content: bool,
    /// Правки УДОБСТВА (регион/loop, раскладка бина, имена дорожек,
    /// порядок строк): пользователь ничего не изменил — звёздочки нет,
    /// на диск они попадают попутно с сохранением/выходом.
    pub dirty_ui: bool,
    pub player: Player,
    /// Keep-alive хэндл cpal-стрима (Drop = остановка вывода).
    #[allow(dead_code)]
    pub audio: Option<audio_out::AudioOutput>,
    pub peaks: Arc<Mutex<HashMap<MediaId, Peaks>>>,
    /// Служба индекса: ОДИН поток на проект с очередью (Р.49, п.368).
    /// Раньше на её месте было три задачи, притворявшихся сервисом.
    pub index: crate::util::IndexService,
    /// Об ошибках индекса сказано (один раз за адрес).
    pub index_errs_said: std::collections::HashSet<String>,
    /// Громкость медиа: (LUFS, dBTP, LRA). Читается из индекс-БД по мере
    /// готовности (Р.49, п.373).
    pub loudness: Arc<Mutex<crate::util::LoudnessMap>>,
    /// Офлайн-анализ материала (п.372).
    pub analysis: crate::analysis::Analysis,
    /// Интервалы дакинга, посчитанные фоном, и число недоступных медиа.
    pub duck_spans: Arc<Mutex<(Vec<std::ops::Range<i64>>, usize)>>,
    /// Настройки дакинга, ждущие своих интервалов.
    pub duck_pending: Option<DuckDialog>,
    pub texture: Option<egui::TextureHandle>,
    pub view_start: f64, // тики
    pub ticks_per_px: f64,
    /// Выделенные события (Р.33 п.7): множество, а не одно — рамка,
    /// Ctrl+клик и двойной клик по метке-диапазону выделяют пачками.
    pub selected: std::collections::BTreeSet<ClipId>,
    /// Режим-инструмент выделения (кнопка тулбара, как в Vegas): обычный
    /// драг по пустому месту рисует рамку вместо региона. Сессионный —
    /// в проект не пишется.
    pub sel_tool: bool,
    /// Замок оси графиков автоматизации — ОДИН НА ВСЮ ПРОГРАММУ и
    /// помнится в проекте (Р.40 п.15, решение пользователя дословно:
    /// «Один на всю программу, помнится в проекте»). До этого он жил в
    /// `AutoView` каждого окна и умирал вместе с окном.
    pub auto_axis_linked: bool,
    /// Выделенные morph-блоки (H1; приоритет над клипом при Del).
    ///
    /// МНОЖЕСТВО с Р.40: было `Option`, и рамка выделения физически не
    /// могла поймать больше одного морфера — а ловила ноль, потому что
    /// сам её улов был `Vec<ClipId>`.
    pub selected_morph: BTreeSet<MorphId>,
    /// Выделенные камеры (Р.38 п.5, «не понимаю как работает камерный
    /// трек»): до того раунда камеру нельзя было ни выделить, ни удалить
    /// с клавиатуры — только через контекст-меню. Множество — с Р.40.
    pub selected_camera: BTreeSet<kt_model::CameraId>,
    /// Режим размещения morpher'а (порт pendingMorpherPlacement v1):
    /// замороженные цели + вид; Esc — отмена.
    pub drag: Option<Drag>,
    /// Основная (зафиксированная) скорость воспроизведения (пп.4-5 v1.4).
    pub base_rate: f32,
    /// Позиция shuttle-слайдера, пока его держат (None = не держат).
    pub shuttle_override: Option<f32>,
    /// Какая полоса спид-контрола схвачена драгом: Some(true)=верхняя
    /// (постоянная), Some(false)=нижняя (shuttle). Пп.18-19 р.7.
    pub speed_drag_upper: Option<bool>,
    /// Виртуальный курсор полосы скорости в пикселях от её левого края
    /// (Р.57): драг копит ПРИРАЩЕНИЯ, поэтому Shift замедляет ход, а не
    /// телепортирует значение (`drag_advance`).
    pub speed_drag_vx: f32,
    /// Открытое поле ввода скорости — НА МЕСТЕ полосы, не окном (Р.57).
    pub speed_entry: Option<SpeedEntry>,
    /// Прямоугольник контрола скорости последнего кадра (живой `rate.py`).
    pub speed_rect: egui::Rect,
    /// Отложенный одиночный клик по превью (п.11 р.7): ждём окно
    /// double-click, потом play/pause.
    pub preview_click_at: Option<std::time::Instant>,
    /// Позиция мыши во время DnD файла (X11-опрос; None на Wayland).
    pub dnd_hover: Option<egui::Pos2>,
    /// Кэш геометрии таймлайна последнего кадра (дроп, п.2 v1.4).
    pub tl_geom: Option<crate::timeline::TlGeom>,
    /// KT_GEOM_DUMP — путь для дампа геометрии живым сценариям (Р.27).
    pub(crate) geom_dump_path: Option<String>,
    pub(crate) geom_dumped_at: Option<std::time::Instant>,
    /// Display-порядок дорожек (п.6 v1.4); нормализуется каждый кадр,
    /// сохраняется в UiState проекта.
    /// Превью отделено в собственное окно (п.7 v1.4, второй монитор).
    pub preview_detached: bool,
    /// Экономный UI: 30 fps при воспроизведении (п.1 раунда 5, слабые GPU).
    pub ui_economy: bool,
    /// Выбранный язык интерфейса; `None` — «авто» (по локали ОС, п.3 р.19).
    /// Это ПОЛЬЗОВАТЕЛЬСКАЯ настройка: живёт в eframe Storage, а не в
    /// `.ktproj`, и звёздочку несохранённого не поднимает.
    pub lang_choice: Option<crate::i18n::Lang>,
    /// Момент последней паузы (двойной Space = стоп, п.15 раунда 6).
    pub paused_at: Option<std::time::Instant>,
    /// Пробел зажат с этого момента (п.3 р.16). None = не зажат.
    pub space_down_at: Option<std::time::Instant>,
    /// Текущий множитель разгона удержанием пробела (None = не разгоняем).
    pub space_boost: Option<f32>,
    /// Окно следует за курсором при игре; гаснет при ручном пане/зуме
    /// (п.14 раунда 6) — используется в L2, ставится в on_play.
    pub follow_playhead: bool,
    /// Момент последнего РУЧНОГО скролла/зума (п.30 р.7: grace для follow_rearm).
    pub last_user_scroll: Option<std::time::Instant>,
    /// Драг окна начат из развёрнутого состояния: сначала восстановить
    /// окно, StartDrag — СЛЕДУЮЩИМ кадром (п.10 р.8, X11).
    pub pending_unmax_drag: bool,
    /// Куда подмотать окно таймлайна на следующем кадре (п.11 р.9:
    /// прыжок к точке остановки должен показывать курсор).
    pub pending_reveal: Option<i64>,

    /// Индивидуальные высоты строк (Р.38 п.11) — зеркало

    /// Активный инлайн-ввод имени метки (п.1 р.10).
    pub marker_edit: Option<MarkerEdit>,
    /// Запросить фокус полю имени метки (один раз при открытии).
    pub marker_edit_focus: bool,
    /// Активный инлайн-ввод имени дорожки (п.2 р.10).
    pub lane_edit: Option<LaneEdit>,
    pub area_edit: Option<AreaEdit>,
    pub area_edit_focus: bool,
    /// Запросить фокус полю имени дорожки.
    pub lane_edit_focus: bool,
    /// Ввод тайм-кода в тулбаре (п.4 р.10): Some — поле открыто.
    pub tc_edit: Option<String>,
    pub tc_edit_focus: bool,
    /// Палитра поиска по проекту (п.3 р.10).
    pub search_open: bool,
    pub search_focus: bool,
    pub search_query: String,
    pub search_sel: usize,
    /// Вертикальный скролл зоны дорожек, px (п.12 р.6).
    pub tl_scroll: f32,
    /// Явная высота строки (Shift+Ctrl+колесо); None = авто-подгонка.
    pub tl_v_zoom: Option<f32>,
    /// Последний удачный бакет превью в бине (скраб без миганий, п.11 р.6).
    pub bin_last_good: HashMap<MediaId, crate::thumbs::ThumbKey>,
    /// Прошлый offline-снапшот бина: детект offline→online (п.1 р.7).
    pub bin_last_offline: std::collections::HashSet<kt_model::MediaId>,
    /// Медиа-бин: свободная раскладка (п.7 р.8) вместо сетки.
    pub bin_free: bool,
    /// Позиции карточек свободной раскладки (живут в проекте, UiState).
    pub bin_layout: std::collections::HashMap<kt_model::MediaId, egui::Pos2>,
    /// Какую карточку тащим внутри бина (свободный режим).
    pub bin_drag: Option<crate::bin_panel::BinDragState>,
    /// Группы карточек бина (п.13 р.11): живут здесь, в проект едут
    /// вместе с остальным UiState.
    pub bin_groups: Vec<kt_model::BinGroup>,
    /// Выделение карточек (рамкой или Ctrl+клик) — только в этой сессии.
    pub bin_sel: std::collections::BTreeSet<kt_model::MediaId>,
    /// Начало рамки выделения (координаты области бина).
    pub bin_rubber: Option<egui::Pos2>,
    /// Переименование группы: индекс + буфер.
    pub bin_group_edit: Option<(usize, String)>,
    /// Фокус в поле имени группы запрашивается ОДИН раз (как у меток):
    /// безусловный request_focus каждый кадр не давал полю «потерять
    /// фокус», и Enter не сохранял имя вовсе (жалоба р.13).
    pub bin_group_focus: bool,
    /// Зоны, где ресайз окна НЕ начинается (п.9 р.12): кнопки окна, меню,
    /// колокольчик. Зона ресайза читает сырой указатель в конце кадра и
    /// мимо хит-теста egui забирает нажатие себе, поэтому кнопки в 16 px
    /// от края были практически некликабельны. Буфер переиспользуется.
    pub no_resize_rects: Vec<egui::Rect>,
    /// Прямоугольники карточек этого кадра (свободный режим): нужны
    /// рамке выделения и рамкам групп, заодно дают harness-тесту куда
    /// целиться мышью. Буфер переиспользуется, не аллоцируется на кадр.
    pub bin_card_rects: Vec<(kt_model::MediaId, egui::Rect)>,
    /// Захват ручки фейдера: смещение курсора относительно ручки —
    /// тянем «относительно», без рывка к позиции (п.2 р.8).
    pub fader_grab: Option<f32>,
    /// Последний отправленный заголовок окна (шлём Title только при смене).
    pub(crate) last_title: String,
    /// Переиспользуемый буфер композиции превью (без аллокаций на кадр).
    pub(crate) preview_buf: Vec<u8>,
    /// Троттлинг glow-мипа: каждые N новых кадров (амбиент не мерцает).
    glow_skip: u8,
    /// Имя адаптера/бэкенда wgpu (диагностика перф-панели).
    pub gpu_info: Option<String>,
    /// Notice о потере GPU-рендера видео показан (один раз за сессию, Р.29).
    pub gpu_fx_lost_notified: bool,
    /// Кэш max_source_resolution по model_rev (Р.31 п.3: функция открывает
    /// ВСЕ медиафайлы — звать её на кадр из окна рендера нельзя).
    pub max_src_cache: Option<(u64, Option<(u32, u32)>)>,
    /// Фоновый автосейв (Р.31 п.2): (rev снапшота, результат). UI-поток
    /// больше не пишет файл сам — только сериализует снапшот и ждёт итог.
    pub autosave_rx: Option<(u64, std::sync::mpsc::Receiver<std::io::Result<SaveOutcome>>)>,
    /// Сторож записи проекта (Р.32): ревизия модели, ЗАПИСАННАЯ на диск
    /// последней. Фоновый автосейв обязан свериться с ним перед подменой
    /// файла — иначе он перетирает более свежий Ctrl+S своим устаревшим
    /// снапшотом (гонка, найденная живыми сценариями).
    /// Самый большой номер события, рождавшийся в ЭТОЙ сессии (п.356).
    pub clip_id_hw: std::cell::Cell<u64>,
    pub saved_rev: std::sync::Arc<std::sync::Mutex<u64>>,
    /// Сторож файла ВОССТАНОВЛЕНИЯ — отдельный от сторожа проекта (Р.49,
    /// п.350). Один мьютекс на две разные цели начинал отменять Ctrl+S
    /// из-за чужой ревизии: фоновая запись recovery поднимала планку, и
    /// следующая явная запись проекта оказывалась «устаревшей».
    pub recovery_rev: std::sync::Arc<std::sync::Mutex<u64>>,
    /// Замок на файл проекта (п.352). `None` — проект безымянный либо
    /// занят другой копией: тогда `lock_holder` несёт её номер процесса.
    pub project_lock: Option<crate::project_lock::ProjectLock>,
    /// Номер процесса-держателя, если проект занят не нами.
    pub lock_holder: Option<Option<u32>>,
    /// Вопрос «проект занят — всё равно сохранить?» задаётся ОДИН раз за
    /// сессию: дальше пользователь уже принял решение.
    pub lock_override: bool,
    /// Заданный и ещё не отвеченный вопрос.
    pub ask: Option<Ask>,
    /// Выход уже разрешён пользователем: следующий `close_requested` не
    /// перехватываем, иначе окно не закроется никогда.
    pub quit_confirmed: bool,
    /// Последний рубеж включён (п.354). В харнессе выключен: тысяча
    /// создаваемых и уничтожаемых приложений не должна сыпать файлами
    /// восстановления в общий каталог данных.
    pub recovery_on_drop: bool,
    /// Цель ПАНЕЛЬНОГО контекстного меню (Р.49, п.363). Взаимно гасится с
    /// `ctx_target`: меню на экране одно.
    pub ctx_panel: Option<crate::ctx_menu::PanelTarget>,
    /// Найденная несохранённая работа и выбранный кандидат (п.353).
    pub recovery_offer: Vec<RecoveryCandidate>,
    pub recovery_pick: usize,
    pub recovery_open: bool,
    /// Файл восстановления, написанный ЭТОЙ копией программы. Удаляем при
    /// сохранении только его: иначе Ctrl+S в одной копии убивал бы
    /// восстановление другой (Р.49, п.351).
    pub recovery_written: Option<PathBuf>,
    pub snap_line: Option<i64>,
    /// (поколение транспорта, число loop-перескоков) на прошлом кадре
    /// превью. Смена любого — законная причина уехать ключу показа назад:
    /// seek бампает поколение, а loop-wrap намеренно его НЕ бампает (звук
    /// и FX-хвосты продолжаются), поэтому у него свой счётчик — без него
    /// на втором круге loop'а картинка замерзала (п.14 р.14, р.15 п.2).
    pub preview_gen: (u64, u64),
    /// Магнит временно выключен (зажат Ctrl во время перетаскивания, п.2 р.11).
    /// Полезное поведение — умолчание, модификатор его СНИМАЕТ.
    pub snap_off: bool,
    /// Последнее направление воспроизведения (п.5 р.13): шлейф курсора
    /// светит позади него, а на паузе не должен прыгать со стороны на
    /// сторону — держим знак последней ненулевой скорости.
    pub play_dir_back: bool,
    /// Ширина панели метров (её тянет своя полоса, п.6 р.13). «Удобство»:
    /// едет в UiState, звёздочку не зажигает.
    pub meters_w: f32,
    /// Рабочее место пользователя (Р.49, п.360): ширина метров и вид по
    /// проектам живут ЗДЕСЬ, а не в чужом файле проекта.
    pub workspace: crate::user_store::Workspace,
    /// Интервал фоновой записи восстановления. Отдельным полем ради живых
    /// сценариев (`KT_AUTOSAVE_SECS`): ждать полминуты в каждом прогоне
    /// нечестно по отношению к проверке, а подделывать таймер изнутри
    /// значило бы проверять не ту программу, которую запускает пользователь.
    pub autosave_secs: u64,
    /// Куда писать рабочее место. `None` — общий файл пользователя; в
    /// харнессе и живых сценариях подменяется, иначе полторы сотни тестов
    /// писали бы в домашний каталог разработчика.
    pub workspace_file: Option<PathBuf>,
    /// Вид, ПРОЧИТАННЫЙ из файла проекта. Пишется обратно как есть:
    /// байт-стабильность чужого файла цела, а наши правки вида уезжают в
    /// рабочее место.
    pub ui_passthrough: Option<kt_model::UiState>,
    pub ctx_target: Option<CtxTarget>,
    /// Строка таймлайна под правым кликом (п.5 р.16): новая дорожка из
    /// контекстного меню встаёт СРАЗУ ПОД ней, а не в самый низ.
    pub ctx_row: Option<(LaneKind, u32)>,
    /// ВРЕМЯ правого клика (Р.56, п.450): «Вставить» из меню кладёт блок
    /// туда, куда человек ткнул, а не туда, где стоит курсор.
    pub ctx_at: Option<kt_time::Tick>,
    /// Live-громкость volume rail до отпускания (id, dB).
    pub live_gain: Option<(ClipId, f32)>,
    /// Живое превью уровня видео при драге линии (Р.28, п.12).
    pub live_opacity: Option<(ClipId, f32)>,
    pub status: Option<(String, std::time::Instant)>,
    /// Медиа, вместо которых сейчас стоит плейсхолдер (Р.33 п.10): файл не
    /// открылся при загрузке проекта. Пустеет, когда файл возвращается.
    pub placeholder_media: std::collections::HashSet<MediaId>,
    /// Переоткрытие вернувшегося файла идёт в фоне (открытие FFmpeg
    /// блокирующее — в UI-потоке это заметная пауза).
    pub reopen_rx: Option<std::sync::mpsc::Receiver<ReopenedMedia>>,
    /// Внутренний буфер обмена (блок клипов+переходов, ребейз к нулю).
    pub clipboard: kt_model::ops::ClipBlock,
    /// Passthrough-остатки загруженного .otio (чужие поля/схемы/metadata):
    /// пишутся обратно при каждом сохранении, вне undo (Р.24).
    pub retained: kt_otio::Retained,
    /// Заказанное ЧТЕНИЕ системного буфера огибающих (Р.61).
    ///
    /// Зеркала буфера внутри программы БОЛЬШЕ НЕТ: оно гасило кнопку на
    /// содержимом, скопированном в другом окне, и запрещало вставку в
    /// чужой вид эффекта. Здесь живёт не содержимое, а конечная задача
    /// «сходи прочитай» — на X11 синхронное чтение собственного буфера
    /// повисло бы в цикле событий, который в этот момент рисует.
    pub env_read: Option<crate::envclip::ClipJob>,
    /// Остаток сопоставления, который решает человек: кривые, чьих имён у
    /// цели нет, и свободные параметры цели.
    pub env_fit: Option<crate::envclip::FitDialog>,
    /// Буфер FX-цепочки/слота.
    pub fx_clipboard: Option<kt_model::ops::FxChainBlock>,
    /// Медиа, чьи файлы сейчас недоступны (watchdog, 900 мс).
    pub offline: Arc<Mutex<std::collections::HashSet<MediaId>>>,
    /// Drop останавливает watchdog-поток (переоткрытие без утечек).
    #[allow(dead_code)]
    pub watchdog: Option<crate::util::WatchdogHandle>,
    pub meter: crate::meters::MeterState,
    pub smooth_scaling: bool,
    pub fx_editors: Vec<crate::fx_editor::FxEditor>,
    /// Окна переходов (Р.33 С4) — по одному на переход.
    pub transition_editors: Vec<crate::transition_editor::TransitionEditor>,
    /// Пути медиа, под которые у плеера подняты источники, — снимок для
    /// `sync_media_sources` (Р.52): смена пути (окно генератора,
    /// undo/redo) видна диффом против него.
    pub(crate) media_paths: HashMap<MediaId, String>,
    /// Значения генераторов, под которые подняты ИСТОЧНИКИ СОБЫТИЙ
    /// (Р.59, стадия 2). С этого раунда параметры живут на событии, и
    /// адрес источника считается от него (`kt_engine::clip_source_addr`),
    /// поэтому снимок ключуется событием, а не медиа.
    pub(crate) gen_states: HashMap<ClipId, kt_model::GenState>,
    /// Первый вызов синка принимает состояние build_player как данность.
    pub(crate) media_synced: bool,
    /// Ревизия модели, на которой синк отработал (Р.53, п.413): карта
    /// путей не пересобирается, пока модель не менялась.
    pub(crate) media_synced_rev: u64,
    /// Сколько раз карта путей реально пересобиралась (сторож t503).
    pub(crate) media_sync_builds: u64,
    /// Семьи шрифтов, о подмене которых уже сказали (Р.53, п.413):
    /// иначе при отсутствующем шрифте каждая правка пути титра давала
    /// свой notice — на драге это шторм одинаковых сообщений.
    pub(crate) font_warned: std::collections::HashSet<String>,
    /// Окна камер (Р.34 К3).
    pub camera_editors: Vec<crate::camera_editor::CameraEditor>,
    pub morph_editors: Vec<crate::morpher_editor::MorphEditor>,
    pub gesture_counter: u64,
    pub render_job: Option<crate::render_dialog::RenderJob>,
    pub render_setup: crate::render_dialog::RenderSetup,
    pub last_autosave: std::time::Instant,
    /// Хинт наведённой кнопки тулбара — дублируется в статусбар (порт).
    pub hover_hint: Option<String>,
    /// Отказ действию вне таймлайна (Р.62): показывается ОШИБКОЙ в строке
    /// состояния, как упор жеста (`blocked`), с тем же TTL, что статус.
    pub error_status: Option<(String, std::time::Instant)>,
    /// Препятствие текущего жеста (п.5 Р.28): подсвечивается на таймлайне,
    /// подсказка идёт ОШИБОЧНЫМ стилем и говорит, куда смотреть.
    pub blocked: Option<crate::timeline::Blocked>,
    /// КОНЕЦ МАТЕРИАЛА, в который упёрся трим (Р.63 п.2): тик, где
    /// кончается файл. Препятствие тут не событие, а край источника —
    /// подсветить нечего, но СКАЗАТЬ обязаны (правило 16), иначе край
    /// просто перестаёт ехать и это выглядит как заедание мыши.
    pub blocked_material: Option<i64>,
    /// ЗАПЕРТАЯ дорожка, в которую упёрся жест (Р.62): подсвечивается
    /// красным ровно один кадр, как и любое другое препятствие.
    /// Запертая дорожка, в которую упёрся жест, и МОМЕНТ отказа. Время
    /// тут не для красоты: отказ приходит и от КЛАВИШИ (удаление,
    /// разрез), а `handle_keys` идёт раньше отрисовки таймлайна — гасни
    /// подсветка каждый кадр, человек увидел бы её ровно один кадр, то
    /// есть никогда. Живёт столько же, сколько текст отказа в строке
    /// состояния (`STATUS_TTL_MS`).
    pub blocked_track: Option<(kt_model::TrackId, std::time::Instant)>,
    /// Последняя рамка выделения в ДОМЕНЕ МОДЕЛИ (время × ключи дорожек)
    /// — из неё делается именованная область (Р.35 п.2). Сессионное:
    /// пережить перезапуск ему незачем.
    pub last_band: Option<Band>,
    /// Окно префикса вставки (Р.35 п.4). Неблокирующее, с ЗАМОРОЖЕННОЙ
    /// позицией вставки: курсор под окном её не уводит.
    pub paste_names: Option<PasteNamesDialog>,
    /// Строка таймлайна ПОД МЫШЬЮ (Р.56, п.450) — только пока указатель
    /// над таймлайном. Ctrl+V кладёт блок сюда: клавиатурная вставка
    /// обязана попадать туда же, куда попала бы вставка из меню.
    pub tl_hover_row: Option<(LaneKind, u32)>,
    pub preset_save: Option<PresetSaveDialog>,
    /// Окно «Настройки проекта» (Р.62).
    pub settings_win: Option<crate::settings_window::SettingsWindow>,
    /// Окно ДОРОЖКИ (Р.62): свойства и цепочка. Одно на программу — как
    /// окно настроек; второе окно на ту же дорожку не заводится.
    pub track_win: Option<crate::track_window::TrackWindow>,
    /// Окно переименования вложенной ленты (Р.50).
    pub seq_name: Option<SeqNameDialog>,
    /// Диалог имени публикуемой ручки (Р.59, стадия 3).
    pub publish_dialog: Option<crate::pubparam::PublishDialog>,
    /// Окно «Публичные параметры» ленты — сторона АВТОРА. Одно на
    /// программу: набор объявляется у ТОЙ ленты, в которой стоишь.
    pub published_win: Option<crate::pubparam::PublishedWin>,
    /// Окна «Параметры композиции» экземпляров — сторона ПОТРЕБИТЕЛЯ.
    /// Список, а не одно: сравнить два выпуска рядом — обычная работа.
    /// Путь внутрь вложений: по уровню на каждый вход (Р.50, В1).
    pub nest_path: Vec<NestStep>,
    /// Полоса, раскрытая ВНУТРИ ленты на момент выхода из неё (Р.58,
    /// п.461): вернулся в ту же ленту — раскрытие на месте. Пара к
    /// `NestStep::band` (тот помнит полосу ПОКИДАЕМОГО контекста при
    /// входе, этот — при выходе). Хранится id: полоса пересчитывается.
    pub reel_bands: std::collections::HashMap<kt_model::MediaId, ClipId>,
    /// Ленты, про которые уже сказано «стоит в N местах» (Р.55, С5).
    /// По ЛЕНТЕ, а не на сессию целиком: две разные общие ленты — два
    /// разных предупреждения, и это честно. В файл не пишется.
    pub shared_warned: std::collections::HashSet<kt_model::MediaId>,
    /// Брошенный на окно проект: путь и место дропа (Р.55, С6).
    pub pending_drop: Option<(PathBuf, Option<(Tick, (ClipKind, u32))>)>,
    /// Библиотека FX-пресетов (Р.48): файл пользователя, общий на все
    /// проекты. Загружается лениво при первом обращении.
    pub presets: crate::user_store::FxPresetStore,
    /// Состояние библиотеки пресетов уже показано (один раз за сессию).
    pub presets_reported: bool,
    /// О невозможности сбросить каталог сказано (один раз за сессию).
    pub dir_sync_warned: bool,
    /// Прямоугольник строки состояния (заполняется при её отрисовке).
    pub statusbar_rect: egui::Rect,
    /// Что реально показано в строке подсказки (для harness-теста).
    pub status_line_shown: (String, HintKind),
    /// Rect таймлайна прошлого кадра (для плавающего тулбара).
    pub timeline_rect: egui::Rect,
    /// Ретроспектива уведомлений (п.18): кольцо + счётчик непрочитанных,
    /// окно открывается колокольчиком в статусбаре.
    pub notices: std::collections::VecDeque<Notice>,
    pub notices_unread: usize,
    pub notices_open: bool,
    /// Софт-рендер (llvmpipe/Cpu) — детект один раз (пп. 17, 19).
    pub sw_render: Option<bool>,
    pub perf: PerfState,
    /// Ключ последнего ЗАГРУЖЕННОГО в текстуру кадра превью: (playhead-тик,
    /// качество, полный ли). Текстура обновляется только при смене —
    /// раньше кадр заливался в GPU 60 раз/с даже в паузе (п.1).
    pub preview_key: Option<(i64, (u32, u32), bool)>,
    /// Идущий жест экранных контролов геометрии (Р.64).
    pub geom_drag: Option<crate::geomctl::GeomDrag>,
    /// Активный набор экранных контролов (зеркало `UiState.geom_controls`).
    pub geom_ctl: crate::geomctl::GeomMode,
    /// Контролы дошли до разбора жеста в этом кадре (сцена не вырождена).
    pub geom_ctl_drawn: bool,
    /// Версия СОДЕРЖИМОГО источников на момент последнего композита
    /// (Р.53, п.412): ключ превью её обязан учитывать, иначе правка
    /// параметров при неподвижном курсоре не доезжает до экрана.
    pub preview_content: u64,
    /// Версия ПРОЕКТА в плеере на момент последнего композита (Р.62,
    /// t722b): правка модели на паузе — правило приведения, параметр FX,
    /// уровень — не двигает ни курсор, ни качество, ни содержимое
    /// источников, и без этого поля превью висело со старым кадром до
    /// первого движения playhead'а (нашёл живой сценарий `conform`).
    pub preview_project: u64,
    /// Когда превью впервые стало неполным (скрим «Загрузка» — после 180 мс,
    /// как в v1, чтобы не мигал на быстрых промахах; п.14).
    pub incomplete_since: Option<std::time::Instant>,
    /// Последняя попытка композита неполного кадра (троттлинг ретраев, п.1).
    pub last_compose_retry: std::time::Instant,
    /// Размытая мини-копия кадра для ambient glow (п.18): 32×18,
    /// обновляется вместе с кадром, рисуется LINEAR-растяжкой.
    pub glow_texture: Option<egui::TextureHandle>,
    /// Rect превью прошлого кадра (для оверлей-свечения поверх UI, п.8).
    pub preview_rect: egui::Rect,
    /// Прямоугольник САМОЙ КАРТИНКИ внутри превью (поля letterbox уже срезаны):
    /// масштаб «холст → экран» считается по нему, а не по панели.
    pub preview_img_rect: egui::Rect,
    /// Прямоугольник медиа-бина прошлого кадра (Р.49, п.365): по нему
    /// ловится правый клик по свободному фону.
    pub bin_rect: egui::Rect,
    /// Rect метров прошлого кадра + цвет доминирующей зоны.
    /// (панель метров, интенсивность 0..1, уровень dB) — оверлей красит
    /// ореол по ЗОНАМ высоты (п.8 v1.4), не одним цветом.
    pub meter_glow: Option<(egui::Rect, f32, f32)>,
    pub thumbs: crate::thumbs::ThumbService,
    /// Миниатюры эффектов для пикера (Р.54): свой сервис, потому что
    /// считает он не кадр источника, а цепочку НА кадре (п.430).
    pub fx_thumbs: crate::fx_thumbs::FxThumbService,
    /// Текстуры филмстрипа (LRU держит ThumbService, тут egui-хэндлы).
    pub thumb_textures: HashMap<crate::thumbs::ThumbKey, egui::TextureHandle>,
    /// Порядок вставки текстур — LRU-вытеснение вместо clear() (п.2).
    pub thumb_lru: std::collections::VecDeque<crate::thumbs::ThumbKey>,
    /// Версия модели (бамп в apply/undo/redo) — ключ кэшей отрисовки (п.1).
    pub model_rev: u64,
    /// Кэш coverage-опросов: (cache.epoch, model_rev, quality) → per-clip
    /// runs. Раньше draw_timeline брал лок кэша ДВАЖДЫ на клип на кадр.
    pub coverage_cache: CoverageCache,
    /// Кэш имён/подавления имён клипов по model_rev (были 2 HashMap +
    /// O(n²) на каждый кадр).
    pub names_cache: NamesCache,
    /// «Мест на монтаже» по медиа, по ревизии модели (Р.53, п.416).
    uses_cache: (Option<u64>, Arc<HashMap<MediaId, usize>>),
    /// Карты растяжения времени по клипам, по model_rev (Р.37 п.3).
    /// Превью обязано считать позицию ТОЙ ЖЕ картой, что рендер, но
    /// строить профиль на каждый пиксельный столбец — аллокация в
    /// draw-пути (инвариант №12), поэтому раз на ревизию.
    pub warp_cache: WarpCache,
    /// Превью вложенных лент: кадры и огибающая (Р.56, п.452).
    pub reel_previews: crate::reel_preview::ReelPreviews,
    /// Текстуры кадров полосы: (ключ ленты, индекс кадра).
    pub reel_textures: HashMap<(crate::reel_preview::ReelKey, usize), egui::TextureHandle>,
    /// Редакции ЛЕНТ на текущей модели (ключ превью). Считается по
    /// `model_rev`, как карты варпа: `content_hash` обходит содержимое
    /// ленты, и делать это на каждый UI-кадр незачем.
    pub reel_hash_cache: (Option<u64>, Arc<HashMap<ClipId, crate::reel_preview::ReelKey>>),
}

impl KtApp {
    fn empty_project() -> Project {
        Project::new(ProjectSettings {
            fps: 30,
            width: 1280,
            height: 720,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        })
    }

    /// Р.29: собрать GPU-движок видео-FX на живом устройстве eframe и
    /// инжектировать в плеер. Зовётся из первого кадра ui() — и потому
    /// повторяется для каждого нового KtApp (открытие проекта пересоздаёт
    /// и плеер, и настоящий детект). `KT_VIDEO_GPU=0` — принудительный
    /// CPU (отладка/сравнение); неудача создания — notice + CPU.
    fn init_video_gpu(&mut self, frame: &eframe::Frame) {
        if std::env::var("KT_VIDEO_GPU").as_deref() == Ok("0") {
            return;
        }
        let built = match frame.wgpu_render_state() {
            None => Err("no wgpu render state".to_string()),
            Some(rs) => kt_gpu::GpuFxEngine::new(kt_gpu::GpuContext::from_parts(
                std::sync::Arc::new(rs.device.clone()),
                std::sync::Arc::new(rs.queue.clone()),
                rs.adapter.get_info(),
            ))
            .map_err(|e| e.to_string()),
        };
        match built {
            Ok(engine) => self.player.set_video_gpu(Some(engine)),
            Err(e) => self.warn(crate::tf!(k::msg::VIDEO_FX_CPU, e)),
        }
    }

    /// Плеер + список НЕДОСТУПНЫХ медиа (id, имя, ошибка). Недоступный
    /// файл получает плейсхолдер и едет дальше: проект открывается, превью
    /// и рендер работают (Р.33 п.10). Раньше здесь терялись все ошибки,
    /// кроме последней, и клип оставался вообще без источника — превью
    /// показывало вечное «Loading».
    fn build_player(
        project: &Project,
        base: Option<&Path>,
        index_db: Option<&Path>,
    ) -> (
        Player,
        Option<audio_out::AudioOutput>,
        Vec<(MediaId, String, String)>,
        Vec<(MediaId, kt_time::Tick)>,
    ) {
        let mut sources = Vec::new();
        let mut missing = Vec::new();
        // ДЛИНЫ МАТЕРИАЛА (Р.63 п.2): их знает только тот, кто открыл
        // источник, а на СТАРТЕ это здесь. Без этого модель узнавала бы
        // их лишь при следующей смене пути — то есть у открытого проекта
        // никогда, и хвост за концом файла ничем бы не отличался.
        let mut lens: Vec<(MediaId, kt_time::Tick)> = Vec::new();
        for m in &project.media {
            let label = crate::util::media_label(&m.path);
            let (src, err) = crate::util::open_source_or_placeholder(
                &resolve_media_path(base, &m.path),
                index_db,
                &label,
            );
            if let Some(e) = err {
                missing.push((m.id, label, e));
            }
            lens.push((m.id, src.info().duration));
            sources.push((m.id, src));
        }
        let (player, ring) = Player::new(project.clone(), sources);
        // «чёрный ящик» — панического хука (п.355). Переприсваивается при
        // каждом пересоздании приложения: дамп должен быть от ЖИВОГО плеера.
        crate::journal::set_flight(player.flight.clone());
        let audio = match audio_out::start(ring, player.shared.clone(), player.transport.clone()) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("audio output unavailable: {e}");
                None
            }
        };
        (player, audio, missing, lens)
    }

    pub(crate) fn new_empty(error: Option<String>) -> KtApp {
        let project = Self::empty_project();
        let project2 = project.clone();
        let (player, audio, _, _) = Self::build_player(&project, None, None);
        let peaks: Arc<Mutex<HashMap<MediaId, Peaks>>> = Arc::new(Mutex::new(HashMap::new()));
        let loudness_map: Arc<Mutex<crate::util::LoudnessMap>> =
            Arc::new(Mutex::new(Default::default()));
        let index = crate::util::IndexService::start(peaks.clone(), loudness_map.clone());
        KtApp {
            model: TimelineModel::new(project),
            project_path: None,
            dirty_content: false,
            dirty_ui: false,
            player,
            audio,
            peaks,
            index,
            index_errs_said: Default::default(),
            loudness: loudness_map,
            analysis: Default::default(),
            duck_spans: Arc::new(Mutex::new((Vec::new(), 0))),
            duck_pending: None,
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: std::collections::BTreeSet::new(),
            sel_tool: false,
            auto_axis_linked: Self::default_axis_linked(),
            selected_morph: BTreeSet::new(),
            selected_camera: BTreeSet::new(),
            drag: None,
            base_rate: 1.0,
            shuttle_override: None,
            speed_drag_upper: None,
            speed_drag_vx: 0.0,
            speed_entry: None,
            speed_rect: egui::Rect::NOTHING,
            preview_click_at: None,
            dnd_hover: None,
            tl_geom: None,
            geom_dump_path: std::env::var("KT_GEOM_DUMP").ok(),
            geom_dumped_at: None,
            // KT_DETACH_PREVIEW=1 — старт с отделённым превью (отладка)
            preview_detached: std::env::var_os("KT_DETACH_PREVIEW").is_some(),
            ui_economy: false,
            lang_choice: None,
            paused_at: None,
            space_down_at: None,
            space_boost: None,
            follow_playhead: false,
            last_user_scroll: None,
            pending_unmax_drag: false,
            pending_reveal: None,
            marker_edit: None,
            marker_edit_focus: false,
            lane_edit: None,
            area_edit: None,
            area_edit_focus: false,
            lane_edit_focus: false,
            tc_edit: None,
            tc_edit_focus: false,
            search_open: false,
            search_focus: false,
            search_query: String::new(),
            search_sel: 0,
            tl_scroll: 0.0,
            tl_v_zoom: None,
            bin_last_good: HashMap::new(),
            bin_last_offline: std::collections::HashSet::new(),
            bin_free: true,
            bin_layout: std::collections::HashMap::new(),
            bin_drag: None,
            bin_groups: Vec::new(),
            bin_sel: Default::default(),
            bin_rubber: None,
            bin_group_edit: None,
            bin_group_focus: false,
            no_resize_rects: Vec::new(),
            bin_card_rects: Vec::new(),
            fader_grab: None,
            last_title: String::new(),
            preview_buf: Vec::new(),
            glow_skip: 0,
            gpu_info: None,
            gpu_fx_lost_notified: false,
            max_src_cache: None,
            autosave_rx: None,
            clip_id_hw: std::cell::Cell::new(0),
            saved_rev: std::sync::Arc::new(std::sync::Mutex::new(0)),
            recovery_rev: std::sync::Arc::new(std::sync::Mutex::new(0)),
            project_lock: None,
            lock_holder: None,
            lock_override: false,
            ask: None,
            quit_confirmed: false,
            recovery_on_drop: true,
            ctx_panel: None,
            recovery_offer: Vec::new(),
            recovery_pick: 0,
            recovery_open: false,
            recovery_written: None,
            snap_line: None,
            preview_gen: (0, 0),
            snap_off: false,
            play_dir_back: false,
            meters_w: 70.0,
            workspace: crate::user_store::Workspace::default(),
            workspace_file: None,
            autosave_secs: std::env::var("KT_AUTOSAVE_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v| *v > 0)
                .unwrap_or(AUTOSAVE_SECS),
            ui_passthrough: None,
            ctx_target: None,
            ctx_row: None,
            ctx_at: None,
            live_gain: None,
            live_opacity: None,
            status: error.map(|e| (e, std::time::Instant::now())),
            placeholder_media: Default::default(),
            reopen_rx: None,
            clipboard: kt_model::ops::ClipBlock::default(),
            retained: kt_otio::Retained::default(),
            env_read: None,
            env_fit: None,
            fx_clipboard: None,
            offline: Arc::new(Mutex::new(std::collections::HashSet::new())),
            watchdog: None,
            meter: Default::default(),
            smooth_scaling: true,
            glow_texture: None,
            preview_rect: egui::Rect::NOTHING,
            preview_img_rect: egui::Rect::NOTHING,
            bin_rect: egui::Rect::NOTHING,
            meter_glow: None,
            thumbs: crate::thumbs::ThumbService::start(&project2, None),
            fx_thumbs: crate::fx_thumbs::FxThumbService::start(),
            fx_editors: Vec::new(),
            transition_editors: Vec::new(),
            media_paths: HashMap::new(),
            gen_states: HashMap::new(),
            media_synced: false,
            media_synced_rev: 0,
            media_sync_builds: 0,
            font_warned: std::collections::HashSet::new(),
            camera_editors: Vec::new(),
            morph_editors: Vec::new(),
            gesture_counter: 0,
            render_job: None,
            render_setup: Default::default(),
            last_autosave: std::time::Instant::now(),
            hover_hint: None,
            error_status: None,
            blocked: None,
            blocked_material: None,
            blocked_track: None,
            last_band: None,
            paste_names: None,
            tl_hover_row: None,
            preset_save: None,
            settings_win: None,
            track_win: None,
            seq_name: None,
            publish_dialog: None,
            published_win: None,
            nest_path: Vec::new(),
            reel_bands: std::collections::HashMap::new(),
            shared_warned: Default::default(),
            pending_drop: None,
            presets: Default::default(),
            presets_reported: false,
            dir_sync_warned: false,
            statusbar_rect: egui::Rect::NOTHING,
            status_line_shown: (String::new(), HintKind::Idle),
            timeline_rect: egui::Rect::NOTHING,
            notices: std::collections::VecDeque::new(),
            notices_unread: 0,
            notices_open: false,
            sw_render: None,
            perf: PerfState::new(),
            preview_key: None,
            geom_drag: None,
            geom_ctl: crate::geomctl::GeomMode::Off,
            geom_ctl_drawn: false,
            preview_content: 0,
            preview_project: 0,
            incomplete_since: None,
            last_compose_retry: std::time::Instant::now(),
            thumb_textures: HashMap::new(),
            thumb_lru: std::collections::VecDeque::new(),
            model_rev: 0,
            coverage_cache: Default::default(),
            names_cache: Default::default(),
            uses_cache: Default::default(),
            warp_cache: Default::default(),
            reel_previews: Default::default(),
            reel_textures: HashMap::new(),
            reel_hash_cache: (None, Arc::new(HashMap::new())),
        }
    }

    pub(crate) fn open(path: &Path) -> Result<KtApp, String> {
        let base = path.parent();
        let (project, retained, notices) = load_project_file(path)?;
        let (player, audio, err, media_lens) =
            Self::build_player(&project, base, Some(&path.with_extension("ktindex.db")));
        let (offline, watchdog) = spawn_media_watchdog(&project, base);
        let thumbs_svc = crate::thumbs::ThumbService::start(&project, Some(path.to_path_buf()));
        let peaks = Arc::new(Mutex::new(HashMap::new()));
        let media: Vec<_> =
            project.media.iter().map(|m| (m.id, resolve_media_path(base, &m.path))).collect();
        let loudness_map: Arc<Mutex<crate::util::LoudnessMap>> =
            Arc::new(Mutex::new(Default::default()));
        let index = crate::util::IndexService::start(peaks.clone(), loudness_map.clone());
        index.enqueue(Some(path.with_extension("ktindex.db")), media);
        let ui_state = project.ui.clone();
        let mut app = KtApp {
            model: TimelineModel::new(project),
            project_path: Some(path.to_path_buf()),
            dirty_content: false,
            dirty_ui: false,
            player,
            audio,
            peaks,
            index,
            index_errs_said: Default::default(),
            loudness: loudness_map,
            analysis: Default::default(),
            duck_spans: Arc::new(Mutex::new((Vec::new(), 0))),
            duck_pending: None,
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: std::collections::BTreeSet::new(),
            sel_tool: false,
            auto_axis_linked: Self::default_axis_linked(),
            selected_morph: BTreeSet::new(),
            selected_camera: BTreeSet::new(),
            drag: None,
            base_rate: 1.0,
            shuttle_override: None,
            speed_drag_upper: None,
            speed_drag_vx: 0.0,
            speed_entry: None,
            speed_rect: egui::Rect::NOTHING,
            preview_click_at: None,
            dnd_hover: None,
            tl_geom: None,
            geom_dump_path: std::env::var("KT_GEOM_DUMP").ok(),
            geom_dumped_at: None,
            // KT_DETACH_PREVIEW=1 — старт с отделённым превью (отладка)
            preview_detached: std::env::var_os("KT_DETACH_PREVIEW").is_some(),
            ui_economy: false,
            lang_choice: None,
            paused_at: None,
            space_down_at: None,
            space_boost: None,
            follow_playhead: false,
            last_user_scroll: None,
            pending_unmax_drag: false,
            pending_reveal: None,
            marker_edit: None,
            marker_edit_focus: false,
            lane_edit: None,
            area_edit: None,
            area_edit_focus: false,
            lane_edit_focus: false,
            tc_edit: None,
            tc_edit_focus: false,
            search_open: false,
            search_focus: false,
            search_query: String::new(),
            search_sel: 0,
            tl_scroll: 0.0,
            tl_v_zoom: None,
            bin_last_good: HashMap::new(),
            bin_last_offline: std::collections::HashSet::new(),
            bin_free: true,
            bin_layout: std::collections::HashMap::new(),
            bin_drag: None,
            bin_groups: Vec::new(),
            bin_sel: Default::default(),
            bin_rubber: None,
            bin_group_edit: None,
            bin_group_focus: false,
            no_resize_rects: Vec::new(),
            bin_card_rects: Vec::new(),
            fader_grab: None,
            last_title: String::new(),
            preview_buf: Vec::new(),
            glow_skip: 0,
            gpu_info: None,
            gpu_fx_lost_notified: false,
            max_src_cache: None,
            autosave_rx: None,
            clip_id_hw: std::cell::Cell::new(0),
            saved_rev: std::sync::Arc::new(std::sync::Mutex::new(0)),
            recovery_rev: std::sync::Arc::new(std::sync::Mutex::new(0)),
            project_lock: None,
            lock_holder: None,
            lock_override: false,
            ask: None,
            quit_confirmed: false,
            recovery_on_drop: true,
            ctx_panel: None,
            recovery_offer: Vec::new(),
            recovery_pick: 0,
            recovery_open: false,
            recovery_written: None,
            snap_line: None,
            preview_gen: (0, 0),
            snap_off: false,
            play_dir_back: false,
            meters_w: 70.0,
            workspace: crate::user_store::Workspace::default(),
            workspace_file: None,
            autosave_secs: std::env::var("KT_AUTOSAVE_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v| *v > 0)
                .unwrap_or(AUTOSAVE_SECS),
            ui_passthrough: None,
            ctx_target: None,
            ctx_row: None,
            ctx_at: None,
            live_gain: None,
            live_opacity: None,
            status: None,
            placeholder_media: err.iter().map(|(id, _, _)| *id).collect(),
            reopen_rx: None,
            clipboard: kt_model::ops::ClipBlock::default(),
            retained,
            env_read: None,
            env_fit: None,
            fx_clipboard: None,
            offline,
            watchdog: Some(watchdog),
            meter: Default::default(),
            smooth_scaling: true,
            glow_texture: None,
            preview_rect: egui::Rect::NOTHING,
            preview_img_rect: egui::Rect::NOTHING,
            bin_rect: egui::Rect::NOTHING,
            meter_glow: None,
            thumbs: thumbs_svc,
            fx_thumbs: crate::fx_thumbs::FxThumbService::start(),
            fx_editors: Vec::new(),
            transition_editors: Vec::new(),
            media_paths: HashMap::new(),
            gen_states: HashMap::new(),
            media_synced: false,
            media_synced_rev: 0,
            media_sync_builds: 0,
            font_warned: std::collections::HashSet::new(),
            camera_editors: Vec::new(),
            morph_editors: Vec::new(),
            gesture_counter: 0,
            render_job: None,
            render_setup: Default::default(),
            last_autosave: std::time::Instant::now(),
            hover_hint: None,
            error_status: None,
            blocked: None,
            blocked_material: None,
            blocked_track: None,
            last_band: None,
            paste_names: None,
            tl_hover_row: None,
            preset_save: None,
            settings_win: None,
            track_win: None,
            seq_name: None,
            publish_dialog: None,
            published_win: None,
            nest_path: Vec::new(),
            reel_bands: std::collections::HashMap::new(),
            shared_warned: Default::default(),
            pending_drop: None,
            presets: Default::default(),
            presets_reported: false,
            dir_sync_warned: false,
            statusbar_rect: egui::Rect::NOTHING,
            status_line_shown: (String::new(), HintKind::Idle),
            timeline_rect: egui::Rect::NOTHING,
            notices: std::collections::VecDeque::new(),
            notices_unread: 0,
            notices_open: false,
            sw_render: None,
            perf: PerfState::new(),
            preview_key: None,
            geom_drag: None,
            geom_ctl: crate::geomctl::GeomMode::Off,
            geom_ctl_drawn: false,
            preview_content: 0,
            preview_project: 0,
            incomplete_since: None,
            last_compose_retry: std::time::Instant::now(),
            thumb_textures: HashMap::new(),
            thumb_lru: std::collections::VecDeque::new(),
            model_rev: 0,
            coverage_cache: Default::default(),
            names_cache: Default::default(),
            uses_cache: Default::default(),
            warp_cache: Default::default(),
            reel_previews: Default::default(),
            reel_textures: HashMap::new(),
            reel_hash_cache: (None, Arc::new(HashMap::new())),
        };
        // loop-регион загруженного проекта — в плеер сразу (раньше
        // синхронизация была только в on_region_changed, т.е. только при
        // ПРАВКЕ региона — загруженный loop не зацикливался вовсе)
        {
            let p = app.model.project();
            let region = p
                .region
                .filter(|_| p.loop_enabled)
                .map(|r| kt_time::TickRange::new(r.start, r.end));
            app.player.set_loop_region(region);
        }
        // замечания импорта/конвертации — пользователю (колокольчик)
        for n in notices {
            app.note(n);
        }
        // недоступные исходники: КАЖДЫЙ отдельной строкой в колокольчике и
        // счётчик в статусе. Проект при этом открыт, монтаж цел, вместо
        // кадров стоят плашки с именами файлов (Р.33 п.10)
        for (_, label, e) in &err {
            app.warn(crate::tf!(k::msg::MEDIA_MISSING, label, e));
        }
        if !err.is_empty() {
            app.note(crate::tf!(k::msg::MEDIA_MISSING_COUNT, err.len()));
        }
        // KT_AUTOPLAY=1 — сразу играть (отладка перф без ввода)
        if std::env::var_os("KT_AUTOPLAY").is_some() {
            app.follow_playhead = true;
            app.player.play();
        }
        // восстановление вьюпорта/курсора (п.13, v1: fromJson)
        app.ui_passthrough = ui_state.clone();
        if let Some(ui) = ui_state {
            // сперва tpp (единые пределы п.17), потом view_start — раньше
            // view_start клампился по ещё-дефолтному tpp
            app.ticks_per_px =
                ui.ticks_per_px.clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
            app.view_start = ui.view_start.max(-40.0 * app.ticks_per_px);
            app.player.seek(Tick(ui.cursor.max(0)));
            // порядок строк с Р.35 приезжает в модели; ui.lane_order —
            // легаси-вход, его переносит ридер формата
            app.bin_free = ui.bin_free; // п.7 р.8
            app.bin_layout =
                ui.bin_layout.iter().map(|(id, x, y)| (*id, egui::pos2(*x, *y))).collect();
            app.bin_groups = ui.bin_groups;
            // замок оси графиков (Р.40 п.15): в файле лежит ОБРАТНОЕ, чтобы
            // serde-умолчание `false` означало включённый замок
            app.auto_axis_linked = !ui.auto_axis_unlinked;
            app.geom_ctl = crate::geomctl::GeomMode::from_u32(ui.geom_controls);
            if ui.meters_w > 0.0 {
                app.meters_w = ui.meters_w.clamp(METERS_W_MIN, METERS_W_MAX);
            }
        }
        // Р.49 п.360: рабочее место пользователя сильнее того, что лежит в
        // чужом файле. Пустое хранилище (первый запуск после переезда)
        // оставляет значения из файла — миграция без единой строки кода.
        // измеренные длины материала — В МОДЕЛЬ (Р.63 п.2): от них зависят
        // упор трима и штриховка хвоста. Вне undo и без звёздочки: это
        // факт о файле, а не правка.
        for (id, d) in media_lens {
            app.model.note_media_length(id, d);
        }
        app.workspace = crate::user_store::load_workspace();
        if app.workspace.meters_w > 0.0 {
            app.meters_w = app.workspace.meters_w.clamp(METERS_W_MIN, METERS_W_MAX);
        }
        if let Some(v) = app.workspace.view_of(Some(path)) {
            app.ticks_per_px =
                v.ticks_per_px.clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
            app.view_start = v.view_start.max(-40.0 * app.ticks_per_px);
            app.player.seek(Tick(v.cursor.max(0)));
        }
        Ok(app)
    }

    /// Запомнить рабочее место. Зовётся там же, где программа и так пишет:
    /// при сохранении проекта и на выходе — своего расписания ему не надо.
    pub(crate) fn save_workspace(&mut self) {
        // файл новее нашей сборки — мы его не трогаем, но и молчать нельзя:
        // иначе ширины панелей «не запоминаются» без единого объяснения
        if self.workspace.is_readonly() {
            let p = self
                .workspace_file
                .clone()
                .unwrap_or_else(|| crate::user_store::store_dir().join("workspace.json"));
            self.warn(crate::tf!(k::msg::WORKSPACE_READONLY, p.display()));
            return;
        }
        self.workspace.meters_w = self.meters_w;
        self.workspace.set_view(
            self.project_path.as_deref(),
            crate::user_store::ProjectView {
                view_start: self.view_start,
                ticks_per_px: self.ticks_per_px,
                cursor: self.player.transport.playhead().0,
            },
        );
        match &self.workspace_file {
            Some(p) => crate::user_store::save_workspace_at(p, &self.workspace),
            None => crate::user_store::save_workspace(&self.workspace),
        }
    }

    #[cfg(test)]
    pub(crate) fn ui_state_for_test(&self) -> kt_model::UiState {
        self.ui_state()
    }

    /// Индивидуальная высота строки (Р.38 п.11). `None` — сбросить к
    /// расчётной. С Р.62 высота живёт НА ДОРОЖКЕ и ходит через undo
    /// вместе с именем: трёх параллельных векторов свойств больше нет.
    pub fn set_row_height(&mut self, key: (kt_model::LaneKind, u32), h: Option<f32>) {
        let Some(t) = self.model.project().track_at(key.0, key.1) else { return };
        let (id, mut props) = (t.id, t.props());
        props.height = h.unwrap_or(0.0);
        if props != t.props() {
            self.apply(Command::SetTrackProps { id, props });
        }
    }

    /// Высоты строк для геометрии кадра: `(вид, номер, px)`.
    pub fn row_heights(&self) -> Vec<(kt_model::LaneKind, u32, f32)> {
        let p = self.model.project();
        p.rows()
            .into_iter()
            .filter_map(|(k, l)| {
                p.track_at(k, l).filter(|t| t.height > 0.0).map(|t| (k, l, t.height))
            })
            .collect()
    }

    /// ОТКАЗ КОМАНДЫ человеку (правило 16): невозможность — это ошибка с
    /// причиной и адресом, а не блёклая подсказка.
    ///
    /// Замок дорожки называет ДОРОЖКУ и место, где его снять: до Р.62
    /// такой отказ приходил бы английским текстом ошибки модели в
    /// дежурной строке — «нажал и ничего не произошло».
    pub(crate) fn refuse_command(&mut self, e: &kt_model::CommandError) {
        if let kt_model::CommandError::TrackLocked(id) = e {
            let name = self
                .model
                .project()
                .rows()
                .into_iter()
                .find(|(k, l)| self.model.project().track_at(*k, *l).map(|t| t.id) == Some(*id))
                .and_then(|(k, l)| {
                    self.model.project().track_at(k, l).map(|t| t.label(l))
                })
                .unwrap_or_default();
            self.blocked_track = Some((*id, std::time::Instant::now()));
            self.refuse(crate::tf!(k::track::LOCKED, name));
            return;
        }
        self.note(e.to_string());
    }

    /// Переключить бейдж дорожки (Р.62) — ОДНО тело на бейдж, пункт меню
    /// и горячую клавишу (правило 17). FX открывает редактор цепочки:
    /// «включить цепочку» без цепочки бессмысленно.
    pub fn toggle_track_badge(&mut self, id: kt_model::TrackId, badge: crate::timeline::TrackBadge) {
        use crate::timeline::TrackBadge as B;
        let Some(t) = self.model.project().track(id) else { return };
        if badge == B::Fx {
            self.open_track_fx(id);
            return;
        }
        let mut props = t.props();
        match badge {
            B::Mute => props.mute = !props.mute,
            B::Solo => props.solo = !props.solo,
            B::Lock => props.lock = !props.lock,
            B::Fx => unreachable!("FX opens the editor above"),
        }
        self.apply(Command::SetTrackProps { id, props });
    }

    /// Дорожка, на которую смотрит клавиша (Р.62). «Выделенной
    /// дорожки» у нас нет — выделяются СОБЫТИЯ, поэтому целью служит
    /// дорожка выделенного события, а без выделения — строка под
    /// указателем (та же, что подсвечена бейджами). Тело одно с меню и
    /// бейджем: `toggle_track_badge` (правило UI 17).
    pub fn key_target_track(&self) -> Option<kt_model::TrackId> {
        let (kind, lane) = self
            .selected
            .iter()
            .copied()
            .find_map(|id| self.model.clip(id).map(kt_model::lane_key))
            .or(self.tl_hover_row)?;
        self.model.project().track_at(kind, lane).map(|t| t.id)
    }

    /// Подпись строки: имя дорожки либо формула V1/A1/M1/C1.
    pub fn row_label(&self, kind: kt_model::LaneKind, lane: u32) -> String {
        self.model.project().track_label(kind, lane)
    }

    /// Видимое окно главного таймлайна в тиках (Р.38 п.14): по нему
    /// график автоматизации строит ось, пока не сняли замок. Одна
    /// функция на все окна с кривыми — иначе у слота, камеры и перехода
    /// зум разъедется, как когда-то разъехались их редакторы ключей.
    pub fn tl_view(&self) -> (kt_time::Tick, kt_time::Tick) {
        let w =
            self.tl_geom.as_ref().map(|g| g.rect.width() as f64 * g.tpp).unwrap_or(10.0 * 90_000.0);
        let s = self.view_start;
        (kt_time::Tick(s.max(0.0) as i64), kt_time::Tick((s + w) as i64))
    }

    /// Снимок вьюпорта для записи в проект (п.13).
    fn ui_state(&self) -> kt_model::UiState {
        // Поля РАБОЧЕГО МЕСТА (вид и ширина метров) возвращаются в файл
        // такими, какими пришли (Р.49, п.360): свои живут в user_store.
        let keep = self.ui_passthrough.clone().unwrap_or_default();
        kt_model::UiState {
            view_start: keep.view_start,
            ticks_per_px: keep.ticks_per_px,
            cursor: keep.cursor,
            lane_order: Vec::new(), // порядок строк живёт в модели (Р.35)
            bin_free: self.bin_free,
            bin_layout: self.bin_layout.iter().map(|(id, p)| (*id, p.x, p.y)).collect(),
            bin_groups: self.bin_groups.clone(),
            meters_w: keep.meters_w,
            // имена и высоты дорожек с Р.62 живут на дорожках-объектах
            // (модель, undo); в `ui` остались только легаси-входы чтения
            lane_names: Vec::new(),
            lane_heights: Vec::new(),
            auto_axis_unlinked: !self.auto_axis_linked,
            geom_controls: self.geom_ctl.as_u32(),
        }
    }

    /// Полная отрисовка кадра. Вынесена из `eframe::App::ui`, чтобы её
    /// мог вызывать harness-тест вёрстки (правило п.9 р.8: вёрстка и
    /// интеракции проверяются egui_kittest, не только скриншотом).
    pub(crate) fn draw_all(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // Потеря GPU-исполнителя видео посреди сессии (DeviceLost) —
        // превью уже на CPU-фолбэке, notice один раз (п.2 Р.29, тест 185)
        if !self.gpu_fx_lost_notified
            && let kt_engine::VideoBackendStatus::Poisoned { reason, .. } =
                self.player.video_backend()
        {
            self.gpu_fx_lost_notified = true;
            self.warn(crate::tf!(k::msg::VIDEO_FX_GPU_LOST, reason));
        }
        // Корневой Ui, который отдаёт eframe, обрамлён дефолтными 8 px —
        // из-за них ВЕСЬ интерфейс не доходил до краёв окна (жалоба п.3
        // р.9 про «отступ метров справа» — на самом деле отступала каждая
        // панель). Работаем на полном content_rect.
        let mut root =
            ui.new_child(egui::UiBuilder::new().max_rect(ctx.content_rect()).layout(*ui.layout()));
        let ui = &mut root;
        // имя проекта — в заголовок окна (п.19 р.6), не каждый кадр
        // имя БЕЗ расширения (п.5 р.12: «.ktproj» и так понятно) и версия
        // сборки: в debug полная vYY.MM.DD-hash, в релизе короткая ktYY.MM
        let title = format!(
            "{}{} — {}",
            self.project_path
                .as_ref()
                .and_then(|p| p.file_stem().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| tr(k::win::UNTITLED).to_string()),
            if self.dirty_content { " *" } else { "" },
            kt_version::display()
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        // КЛИК МИМО ПОЛЯ ОТДАЁТ ФОКУС — одно правило на всю программу
        // (Р.63 п.4), и стоит оно ДО чтения клавиш.
        //
        // egui отдаёт фокус `TextEdit` только по Esc, Tab, другому
        // фокусируемому виджету или его исчезновению. Таймлайн, превью и
        // тулбар фокусируемыми не являются (`Sense::click_and_drag`),
        // поэтому клик по ним фокус НЕ снимал, а `handle_keys` целиком
        // возвращается на `egui_wants_keyboard_input()`. Итог словами
        // владельца: «как бы я не тыкал по остальной части интерфейса —
        // нажатие пробела не приводит к паузе… пока я не закрою окно
        // эффекта». Правило одно на программу намеренно: в каждом окне
        // порознь следующий агент его забудет (правило 17).
        self.surrender_focus_on_outside_click(&ctx);
        self.handle_keys(&ctx);
        // разгон удержанием пробела — РОВНО раз в кадр (handle_keys при
        // отделённом превью зовётся дважды, п.3 р.16)
        self.tick_space_boost(&ctx);
        self.handle_drops(&ctx);
        self.hover_hint = None;
        self.no_resize_rects.clear();
        let rate = self.player.playback_rate();
        if rate.abs() > 0.01 {
            self.play_dir_back = rate < 0.0;
        }

        // п.10 р.8: окно восстановлено из maximized прошлым кадром —
        // теперь можно начинать перетаскивание
        if self.pending_unmax_drag {
            self.pending_unmax_drag = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            Self::release_pointer_after_wm_grab(&ctx);
        }
        // статусбар (низ)
        egui::Panel::bottom("status")
            .resizable(false)
            .frame(egui::Frame::new().fill(crate::theme::PANEL).inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: 1,
            }))
            .exact_size(20.0)
            .show(ui, |ui| {
                self.draw_statusbar(ui);
            });
        // таймлайн (низ, на всю ширину) + плавающий тулбар поверх
        // п.10 р.9: таймлайну нужен ПОТОЛОК, иначе при малой высоте он
        // забирает всё и вытесняет шапку с лого/меню и кнопки окна (они
        // живут внутри боковых колонок). Умное сжатие панелей egui при
        // этом не ломается — добавлен только верхний предел.
        let tl_max = (ui.available_height() - 140.0).max(80.0);
        egui::Panel::bottom("timeline")
            .frame(egui::Frame::new().fill(crate::theme::PANEL))
            .resizable(true)
            .default_size(300.0)
            .max_size(tl_max)
            .show(ui, |ui| {
                let full = ui.available_rect_before_wrap();
                self.timeline_rect = full;
                let strip_h = 34.0;
                let tl_rect = egui::Rect::from_min_max(
                    full.min,
                    egui::pos2(full.right(), full.bottom() - strip_h),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(tl_rect), |ui| {
                    self.draw_timeline(ui);
                });
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                        egui::pos2(full.left(), full.bottom() - strip_h),
                        full.max,
                    )),
                    |ui| {
                        self.draw_toolbar_strip(ui);
                    },
                );
            });
        // полоса времени над таймлайном (порт Timeline time strip)
        // левая колонка: медиа-бин + перфоманс-бокс (порт widget_3)
        // ПРЕДЕЛ ПОСТАВЛЕН ПО ЖАЛОБЕ п.4 р.13 — не ужимать без новой просьбы
        // пользователя: 420 px были страховкой р.7.1, в р.9 подняты до
        // половины окна по его прямой просьбе, а в р.11 я срезал их
        // обратно «по аналогии» с метрами и сломал работу в третий раз.
        egui::Panel::left("left_col")
            .frame(crate::theme::panel_frame())
            .resizable(true)
            .default_size(232.0)
            .min_size(72.0)
            .max_size((ui.max_rect().width() * 0.7).max(420.0))
            .show(ui, |ui| {
                // шапка: лого + меню + имя проекта (пп.15,6,7 р.8)
                self.draw_column_header(ui);
                ui.add_space(2.0);
                // п.27 р.7: контент в ScrollArea — фрейм панели не перерастает
                // высоту, и сепаратор egui больше не залезает на таймлайн
                // (panel.rs берёт ФАКТИЧЕСКИЙ rect фрейма); заодно колонка
                // скроллится при малой высоте окна
                // auto_shrink по ШИРИНЕ обязателен: [false] заставлял контент
                // требовать всю доступную ширину, панель раздувалась по
                // контенту и съедала превью (регрессия п.3 р.7.1). По высоте
                // false — ради п.27 (фрейм не перерастает панель).
                egui::ScrollArea::vertical().auto_shrink([true, false]).show(ui, |ui| {
                    egui::Frame::group(ui.style())
                        .fill(crate::theme::WINDOW)
                        .corner_radius(1.0)
                        .show(ui, |ui| {
                            self.draw_bin(ui);
                        });
                    ui.add_space(8.0);
                    egui::Frame::group(ui.style())
                        .fill(crate::theme::WINDOW)
                        .corner_radius(1.0)
                        .show(ui, |ui| {
                            self.draw_perf_box(ui);
                        });
                });
            });
        // центр: превью + метры справа, под ними контролы.
        // Фрейм без воздуха: дефолтные 8 px по периметру съедали место и
        // отодвигали кнопки окна от угла (п.15 р.8).
        egui::CentralPanel::default().frame(egui::Frame::new().fill(crate::theme::PANEL)).show(
            ui,
            |ui| {
                let p = self.model.project();
                if let Some(r) = p.region
                    && !p.loop_enabled
                    && self.player.transport.is_playing()
                    && self.player.transport.playhead() >= r.end
                    && self.player.transport.playhead() < r.end + Tick(9000)
                {
                    self.player.pause();
                    // как ручная пауза: двойной Space после авто-паузы = стоп
                    // в начало региона (п.29 р.7)
                    self.paused_at = Some(std::time::Instant::now());
                }
                egui::Panel::bottom("prev_controls").show(ui, |ui| {
                    ui.add_space(3.0);
                    self.draw_preview_controls(ui);
                    ui.add_space(3.0);
                });
                // Ширину держим САМИ, штатный resizable выключен (п.6 р.13):
                // хват egui — ±8 px по ВСЕЙ высоте панели и регистрируется
                // ПОСЛЕ её контента (так задумано в egui), поэтому он выигрывал
                // у кнопок окна и «свернуть» нажималась наполовину. Своя полоса
                // лежит ЛЕВЕЕ панели и внутрь неё не заходит.
                let mw = self.meters_w.clamp(METERS_W_MIN, METERS_W_MAX);
                egui::Panel::right("meters_col")
                    .resizable(false)
                    .exact_size(mw)
                    .frame(egui::Frame::new().fill(crate::theme::PANEL))
                    .show(ui, |ui| {
                        // кнопки окна — здесь, в вёрстке (п.15 р.8)
                        self.draw_window_buttons(ui);
                        self.draw_meters(ui);
                    });
                // полоса перетаскивания — уже ВНЕ панели: место под неё забрала
                // сама панель, поэтому правый край оставшегося места и есть её
                // левая кромка. Ниже полосы драга окна, чтобы не спорить с ней.
                {
                    // кромка панели считается от окна (ui.max_rect() панель не
                    // ужимает), полоса лежит СЛЕВА от неё и в панель не заходит
                    let win = ui.ctx().content_rect();
                    let edge = win.right() - mw;
                    let sep = egui::Rect::from_min_max(
                        egui::pos2(edge - METERS_SEP_W, win.top() + TOP_DRAG_H),
                        egui::pos2(edge, win.bottom()),
                    );
                    let r = ui.interact(sep, egui::Id::new("meters-sep"), egui::Sense::drag());
                    if r.hovered() || r.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if r.dragged()
                        && let Some(pos) = r.interact_pointer_pos()
                    {
                        self.meters_w = (win.right() - pos.x).clamp(METERS_W_MIN, METERS_W_MAX);
                        self.dirty_ui = true;
                    }
                }
                egui::CentralPanel::default().frame(egui::Frame::new()).show(ui, |ui| {
                    // п.2 р.9: НЕПРЕРЫВНАЯ зона драга окна по всей ширине.
                    // Полоса невидима и лежит между кромкой ресайза (WIN_EDGE)
                    // и низом шапок колонок, поэтому места не занимает и
                    // стыкуется с шапкой слева и кнопками окна справа.
                    if std::env::var_os("KT_DECORATIONS").is_none() {
                        let full = ui.available_rect_before_wrap();
                        // В развёрнутом окне ресайза НЕТ (блок ниже выключен
                        // гардом maximized), поэтому верхние WIN_EDGE пикселей
                        // оставались МЁРТВЫМИ: тянешь у самой кромки — ничего,
                        // чуть ниже — работает (п.11 р.13). Отдаём их драгу.
                        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                        let top = if maximized { full.top() } else { full.top() + WIN_EDGE };
                        let strip = egui::Rect::from_min_max(
                            egui::pos2(full.left() - 8.0, top),
                            egui::pos2(full.right() + 8.0, full.top() + TOP_DRAG_H),
                        );
                        let bg = ui.interact(
                            strip,
                            ui.id().with("top-drag"),
                            egui::Sense::click_and_drag(),
                        );
                        self.window_drag_from(&bg, ui.ctx());
                        // МЕСТА полоса не занимает (п.1 р.11): раньше здесь
                        // стоял advance_cursor_after_rect, и превью теряло
                        // 20 px по высоте ни за что. Клики полоса не крадёт —
                        // egui ищет цель клика и цель драга независимо, а
                        // превью само не принимает клики выше TOP_DRAG_H.
                    }
                    if self.preview_detached {
                        ui.centered_and_justified(|ui| {
                            ui.weak(tr(k::hint::PREVIEW_DETACHED));
                        });
                    } else {
                        self.draw_preview(ui);
                    }
                });
            },
        );

        // ---- отделённое окно превью (п.7 v1.4): второй монитор ------------
        // show_viewport_immediate: один egui::Context на все вьюпорты →
        // текстура превью работает в чужом окне без переезда; deferred не
        // годится (требует Send+Sync при &mut self).
        if self.preview_detached {
            let vp = egui::ViewportId::from_hash_of("kt-preview-window");
            ctx.show_viewport_immediate(
                vp,
                egui::ViewportBuilder::default()
                    .with_title(tr(k::win::PREVIEW_TITLE))
                    .with_inner_size([960.0, 560.0]),
                |ui, _class| {
                    // input этого вьюпорта: пробел/шорткаты работают из
                    // окна; Esc здесь ТОЛЬКО возвращает превью (не трогает
                    // глобальный каскад — регион/окна главного не страдают)
                    let child_ctx = ui.ctx().clone();
                    let esc = child_ctx.input(|i| i.key_pressed(egui::Key::Escape));
                    if !esc {
                        self.handle_keys(&child_ctx);
                    }
                    // без отступов, чёрный фон — только видео (п.7 раунда 5)
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(8, 8, 10)))
                        .show(ui, |ui| {
                            self.draw_preview(ui);
                        });
                    // Esc в окне превью возвращает его в главное
                    if esc || child_ctx.input(|i| i.viewport().close_requested()) {
                        self.preview_detached =
                            preview_detached_next(self.preview_detached, false, true);
                    }
                },
            );
        }

        self.sample_perf();
        self.draw_search(&ctx);
        self.draw_fx_editors(&ctx);
        self.draw_transition_editors(&ctx);
        self.draw_camera_editors(&ctx);
        self.draw_morph_editors(&ctx);
        self.draw_paste_names(&ctx);
        self.draw_preset_save(&ctx);
        self.draw_settings_window(&ctx);
        self.draw_track_window(&ctx);
        self.draw_seq_name(&ctx);
        // Публичные параметры композиции (Р.59, стадия 3): диалог имени,
        // окно автора и окна потребителей.
        self.draw_publish_dialog(&ctx);
        self.draw_published_win(&ctx);
        self.draw_env_fit(&ctx);
        // заявка подписи поля забирается ПОСЛЕ отрисовки окон — внутри
        // `Window::show` до модели не дотянуться (тот же механизм, что у
        // `hint_sink`)
        self.drain_publish_req();
        self.draw_render_dialog(&ctx);
        self.draw_diagnostics(&ctx);

        // автосейв (порт autosaveTimer): безымянный проект — в
        // recovery-файл ~/.local/share/kinotrain/ (п.20, v1).
        // п.4 р.11: было каждые 4 с и по ЛЮБОЙ правке, включая движение
        // региона и зум — то есть постоянная запись всего проекта в
        // UI-потоке. Теперь: раз в 30 с, только от правок СОДЕРЖИМОГО и
        // не во время жеста/игры (страховка — если тихо больше двух
        // интервалов, пишем всё равно: терять правки хуже микрофриза).
        // Р.31 п.2: сериализация и запись — В ФОНЕ (чек-лист п.12
        // «фоновая работа не мешает»): на большом проекте/медленном диске
        // автосейв в UI-потоке ронял FPS в ноль каждые 30 с.
        // dirty_ui/сдвиг вьюпорта тоже сохраняются попутно (п.5 р.11) —
        // раньше это делал синхронный сейв из eframe::save каждые 30 с
        // Р.49 п.351: цель фоновой записи — ФАЙЛ ВОССТАНОВЛЕНИЯ, а не проект
        // пользователя (прямое решение владельца). Проект меняет только явное
        // сохранение; поэтому и звёздочка тут больше не гаснет.
        // Дрейф вьюпорта (зум/скролл) больше НЕ повод для записи: раньше он
        // раз в 30 секунд переписывал весь проект без единой правки.
        let due = self.autosave_rx.is_none() && self.autosave_due();
        if (self.dirty_content || self.dirty_ui) && due {
            self.last_autosave = std::time::Instant::now();
            let ui = Some(self.ui_state());
            self.model.set_ui_state(ui);
            let target = self.recovery_target();
            let project = self.model.document();
            let retained = self.retained.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            let snap_rev = self.model_rev;
            self.autosave_rx = Some((snap_rev, rx));
            let guard = self.recovery_rev.clone();
            self.recovery_written = Some(target.clone());
            std::thread::Builder::new()
                .name("autosave".into())
                .spawn(move || {
                    let out = kt_otio::save_project(&project, &retained, target.parent());
                    // подмена файла — под сторожем: если пока мы
                    // сериализовали, на диск лёг БОЛЕЕ СВЕЖИЙ снимок
                    // (Ctrl+S), свой устаревший не пишем вовсе
                    let _ = tx.send(write_project_guarded(&guard, &target, snap_rev, &out));
                })
                .ok();
        }
        self.poll_returned_sources();
        self.poll_env_clipboard();
        // Офлайн-анализ: забрать готовое, запустить следующее (п.372).
        // `animating` держится на `is_running()`, а не на «окно открыто» —
        // иначе перерисовка не остановится после финиша (повтор t38).
        if let Some((kind, rev, res)) = self.analysis.poll(crate::analysis::spawn_default) {
            self.on_analysis_done(kind, rev, res);
        }
        if self.analysis.is_running() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        // Превью лент (Р.56, п.452). Пауза — не вежливость: офлайн-рендер
        // ленты декодирует файлы, и во время игры он отнимал бы процессор
        // у реального времени (инвариант 6).
        self.reel_previews
            .set_paused(self.player.transport.is_playing() || self.render_job.is_some());
        if self.reel_previews.poll() {
            self.dirty_ui = true;
            // текстуры живут ровно столько, сколько их полоса: лента,
            // вытесненная из LRU или устаревшая после правки, не имеет
            // права держать десятки мегабайт в видеопамяти
            let live = &self.reel_previews;
            self.reel_textures.retain(|(k, _), _| live.get(k).is_some());
        }
        if self.reel_previews.is_running() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        // Ошибки индекса — ВСЛУХ (Р.49, п.368). Четыре `let _ =` молчали о
        // том, что индекс не построился, и человек видел плейсхолдеры
        // вместо волноформ без единого слова о причине. Один раз на адрес:
        // служба может повторить попытку, а сообщение повторять незачем.
        for e in self.index.take_errors() {
            if self.index_errs_said.insert(e.clone()) {
                self.warn(crate::tf!(k::msg::INDEX_FAILED, e));
            }
        }
        if let Some((snap_rev, rx)) = &self.autosave_rx
            && let Ok(res) = rx.try_recv()
        {
            let snap_rev = *snap_rev;
            self.autosave_rx = None;
            let _ = snap_rev;
            match res {
                // Звёздочка НЕ гаснет: работа пользователя легла в файл
                // восстановления, а его проект по-прежнему без этих правок.
                Ok(SaveOutcome::Wrote { dir_synced }) => self.warn_if_dir_unsynced(dir_synced),
                // штатно и молча: значит уже записан более свежий снимок
                Ok(SaveOutcome::SkippedNewerOnDisk) => {}
                Err(e) => {
                    self.recovery_written = None;
                    self.warn(crate::tf!(k::msg::AUTOSAVE_FAILED, e))
                }
            }
        }

        // предложение восстановить несохранённую работу (п.353)
        if self.recovery_open {
            let mut act: Option<u8> = None;
            egui::Modal::new(egui::Id::new("kt_recovery")).show(ui.ctx(), |ui| {
                ui.set_max_width(560.0);
                ui.heading(tr(k::ask::RECOVERY_TITLE));
                ui.add_space(6.0);
                ui.label(tr(k::ask::RECOVERY_INTRO));
                ui.add_space(6.0);
                for (i, c) in self.recovery_offer.iter().enumerate() {
                    let owner = match &c.for_project {
                        Some(p) => p.display().to_string(),
                        None => tr(k::ask::RECOVERY_UNNAMED).to_string(),
                    };
                    let label = format!("{}  —  {owner}", crate::journal::stamp_of(c.mtime));
                    if ui.radio(self.recovery_pick == i, label).clicked() {
                        self.recovery_pick = i;
                    }
                    ui.weak(c.path.display().to_string());
                }
                ui.add_space(8.0);
                // правило 19: диалог говорит, что решение ничего не ломает
                ui.weak(tr(k::ask::RECOVERY_NOTE));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    // координаты кнопок — наружу живым сценариям (Р.27):
                    // модальное окно egui в дереве доступности не ищется
                    let r = ui.button(tr(k::ask::RECOVERY_RESTORE));
                    crate::widgets::field_probe::note("ask.restore", r.rect);
                    if r.clicked() {
                        act = Some(0);
                    }
                    let r = ui.button(tr(k::ask::RECOVERY_FORGET));
                    crate::widgets::field_probe::note("ask.forget", r.rect);
                    if r.clicked() {
                        act = Some(1);
                    }
                    let r = ui.button(tr(k::ask::RECOVERY_LATER));
                    crate::widgets::field_probe::note("ask.later", r.rect);
                    if r.clicked() {
                        act = Some(2);
                    }
                });
            });
            match act {
                Some(0) => self.restore_recovery(self.recovery_pick),
                Some(1) => self.forget_recovery(self.recovery_pick),
                Some(2) => self.recovery_open = false,
                _ => {}
            }
        }

        // вопрос, на который программа ждёт ответа (п.352). Модальное окно:
        // пока на него не ответили, сохранение не продолжается.
        if let Some(ask) = self.ask.clone() {
            let mut answer: Option<Answer> = None;
            egui::Modal::new(egui::Id::new("kt_ask")).show(ui.ctx(), |ui| {
                ui.set_max_width(440.0);
                match &ask {
                    Ask::SaveOverLock { pid } => {
                        ui.heading(tr(k::ask::LOCKED_TITLE));
                        ui.add_space(6.0);
                        ui.label(match pid {
                            Some(n) => crate::tf!(k::msg::PROJECT_LOCKED_BY, n),
                            None => tr(k::msg::PROJECT_LOCKED).to_string(),
                        });
                        ui.add_space(4.0);
                        ui.weak(tr(k::ask::LOCKED_BODY));
                        // правило 19: где это переиграть
                        ui.weak(tr(k::ask::LOCKED_WHERE));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button(tr(k::ask::SAVE_ANYWAY)).clicked() {
                                answer = Some(Answer::Yes);
                            }
                            if ui.button(tr(k::ask::DONT_SAVE)).clicked() {
                                answer = Some(Answer::No);
                            }
                        });
                    }
                    Ask::Unsaved(p) => {
                        ui.heading(tr(k::ask::UNSAVED_TITLE));
                        ui.add_space(6.0);
                        ui.label(tr(match p {
                            Pending::New => k::ask::UNSAVED_NEW,
                            Pending::Open | Pending::DropOpen => k::ask::UNSAVED_OPEN,
                            Pending::Quit => k::ask::UNSAVED_QUIT,
                        }));
                        ui.add_space(4.0);
                        // правило 19: решение ни к чему не приговаривает
                        ui.weak(tr(k::ask::UNSAVED_WHERE));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            let r = ui.button(tr(k::ask::SAVE_AND_GO));
                            crate::widgets::field_probe::note("ask.save_and_go", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::Yes);
                            }
                            let r = ui.button(tr(k::ask::DISCARD));
                            crate::widgets::field_probe::note("ask.discard", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::No);
                            }
                            let r = ui.button(tr(k::ask::STAY));
                            crate::widgets::field_probe::note("ask.stay", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::Cancel);
                            }
                        });
                    }
                    Ask::DropOtio => {
                        ui.heading(tr(k::ask::DROP_OTIO_TITLE));
                        ui.add_space(6.0);
                        let what = self
                            .pending_drop
                            .as_ref()
                            .map(|(p, _)| p.display().to_string())
                            .unwrap_or_default();
                        ui.label(crate::tf!(k::ask::DROP_OTIO_BODY, what));
                        ui.add_space(4.0);
                        ui.weak(tr(k::ask::DROP_OTIO_WHERE));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            let r = ui.button(tr(k::ask::DROP_OPEN));
                            crate::widgets::field_probe::note("ask.drop_open", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::Yes);
                            }
                            let r = ui.button(tr(k::ask::DROP_INSERT));
                            crate::widgets::field_probe::note("ask.drop_insert", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::No);
                            }
                            let r = ui.button(tr(k::ask::STAY_HERE));
                            crate::widgets::field_probe::note("ask.drop_cancel", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::Cancel);
                            }
                        });
                    }
                }
            });
            if let Some(a) = answer {
                self.answer_ask(a);
            }
        }

        // окно прогресса анализа (п.372). ОДНО на все задачи: у каждой
        // своё окно означало бы частокол окон при очереди из десятка файлов.
        if self.analysis.open {
            let mut cancel = false;
            egui::Window::new(tr(k::win::ANALYSIS_TITLE))
                .id(egui::Id::new("kt_analysis"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
                .show(ui.ctx(), |ui| {
                    ui.add(egui::ProgressBar::new(self.analysis.progress()).desired_width(220.0));
                    if self.analysis.queued() > 0 {
                        ui.weak(crate::tf!(k::win::ANALYSIS_QUEUED, self.analysis.queued()));
                    }
                    if ui.button(tr(k::win::ANALYSIS_CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            if cancel {
                self.analysis.cancel_all();
            }
        }

        // окно уведомлений (п.18)
        if self.notices_open {
            let mut open = self.notices_open;
            egui::Window::new(tr(k::win::NOTICES_TITLE))
                .id(egui::Id::new("kt_notices"))
                .open(&mut open)
                .default_size([460.0, 340.0])
                .min_size([320.0, 200.0])
                .resizable(true)
                .vscroll(true)
                .show(ui.ctx(), |ui| {
                    if self.notices.is_empty() {
                        ui.weak(tr(k::win::NOTICES_EMPTY));
                    }
                    for n in self.notices.iter().rev() {
                        let col = match n.severity {
                            Severity::Warn => egui::Color32::from_rgb(255, 190, 120),
                            Severity::Info => egui::Color32::from_gray(200),
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.weak(&n.at);
                            ui.colored_label(col, &n.text);
                            if n.repeats > 1 {
                                ui.weak(format!("×{}", n.repeats));
                            }
                        });
                    }
                });
            self.notices_open = open;
        }

        // ---- ресайз borderless-окна за кромки и углы (пп.11-13 р.8) ----
        // ВАЖНО: не виджет и не в начале кадра. Виджет проигрывал hit-test
        // всем панелям (egui при равном расстоянии отдаёт клик виджету,
        // зарегистрированному ПОЗЖЕ), а `drag_started` у click_and_drag
        // наступал только после сдвига >6 px — курсор к тому моменту уже
        // покидал зону, и BeginResize не уходил НИКОГДА. Теперь читаем
        // указатель напрямую в конце кадра: курсор выставляется последним
        // (побеждает), команда шлётся на самом нажатии.
        if !ctx.input(|i| i.viewport().maximized.unwrap_or(false))
            && std::env::var_os("KT_DECORATIONS").is_none()
        {
            let win = ctx.content_rect();
            // кнопки у краёв (окно, меню, колокольчик) забирают нажатие
            // себе — кроме САМОЙ кромки в 2 px, где остаётся ресайз, в том
            // числе диагональный за угол (п.9 р.12)
            let guarded = |slf: &Self, pos: egui::Pos2| {
                win.shrink(WIN_BORDER).contains(pos)
                    && slf.no_resize_rects.iter().any(|r| r.contains(pos))
            };
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos())
                && !guarded(self, pos)
                && let Some(dir) = resize_zone(win, pos, WIN_EDGE, WIN_CORNER)
            {
                ctx.set_cursor_icon(resize_cursor(dir));
                if ctx.input(|i| i.pointer.primary_pressed()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                    Self::release_pointer_after_wm_grab(&ctx);
                }
            }
        }

        // подсказка — последней, поверх статусбара (п.1 р.9)
        self.draw_status_hint(&ctx);

        // Оверлей-свечение поверх интерфейса (п.8): превью и метры мягко
        // «светят» за границы своих панелей, не мешая чтению UI.
        if self.sw_render == Some(false) {
            let fg = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("ambient-overlay"),
            ));
            // превью: размытая текстура чуть шире панели, очень низкая альфа
            if let (Some(glow), true, false) =
                (&self.glow_texture, self.preview_rect.is_finite(), self.preview_detached)
            {
                let r = self.preview_rect.expand(28.0);
                fg.image(
                    glow.id(),
                    r,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14),
                );
            }
            // метры: ореол влево от панели, цвет по ЗОНАМ высоты (п.8);
            // правая кромка — ровно левый край панели (п.10: раньше +8 px
            // наезжали на первый бар у подписи dB)
            if let Some((r, _k, loud_db)) = self.meter_glow
                && loud_db > crate::meters::METER_MIN_DB + 0.5
            {
                {
                    let glow_rect = crate::meters::overlay_glow_rect(r);
                    // строки на границах зон: цвет зоны, альфа по k;
                    // выше уровня — затухание в прозрачность
                    let mut mesh = egui::Mesh::default();
                    let y_of = |db: f32| -> f32 {
                        let t = (db - crate::meters::METER_MIN_DB)
                            / (crate::meters::METER_MAX_DB - crate::meters::METER_MIN_DB);
                        glow_rect.bottom() - t.clamp(0.0, 1.0) * glow_rect.height()
                    };
                    let mut push_row = |y: f32, col: egui::Color32, alpha: u8| {
                        for (x, aa) in [(glow_rect.left(), 0u8), (glow_rect.right(), alpha)] {
                            mesh.vertices.push(egui::epaint::Vertex {
                                pos: egui::pos2(x, y),
                                uv: egui::epaint::WHITE_UV,
                                color: egui::Color32::from_rgba_unmultiplied(
                                    col.r(),
                                    col.g(),
                                    col.b(),
                                    aa,
                                ),
                            });
                        }
                    };
                    // альфа ряда — от ЕГО dB, а не от общей громкости
                    // (п.4 р.16): одна общая `k` заставляла мерцать весь
                    // ореол по высоте, включая всегда горящий низ
                    let stops = crate::meters::glow_stops(loud_db);
                    for &db in &stops {
                        push_row(
                            y_of(db),
                            crate::meters::zone_color(db),
                            crate::meters::glow_row_alpha(
                                db,
                                loud_db,
                                crate::meters::GLOW_OVERLAY_A,
                            ),
                        );
                    }
                    let top_col = crate::meters::zone_color(loud_db);
                    push_row((y_of(loud_db) - 30.0).max(glow_rect.top()), top_col, 0);
                    let rows = stops.len() + 1;
                    for row in 0..rows - 1 {
                        let b = (row * 2) as u32;
                        mesh.indices.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
                    }
                    fg.add(egui::Shape::mesh(mesh));
                }
            }
        }

        // Перерисовка ПО ПОТРЕБНОСТИ (п.1: безусловные 60 fps + заливка
        // текстуры каждый кадр грели систему даже с пустым проектом; egui
        // и так перерисовывает мгновенно на любой ввод).
        let animating = self.player.transport.is_playing()
            || self.space_down_at.is_some()
            || self.drag.is_some()
            || self.render_job.is_some()
            || self.meter.is_active();
        // «сбавить обороты, когда занято» (правило 12): миниатюры пикера
        // считаются в своём потоке, и при игре/экспорте он обязан уступать
        self.fx_thumbs.set_busy(self.player.transport.is_playing() || self.render_job.is_some());
        // ...а пока у него есть незакрытые заявки — просим кадры, иначе
        // готовая ячейка появится только на следующем вводе. Гейт висит на
        // «работа ЖИВА», а не на «пикер открыт»: открытое меню не имеет
        // права держать 60 fps в простое (дефект t38).
        if self.fx_thumbs.pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
        let delay = if animating {
            std::time::Duration::from_millis(if self.ui_economy { 33 } else { 16 })
        } else if self.incomplete_since.is_some() {
            std::time::Duration::from_millis(100) // ретраи неполного кадра
        } else {
            repaint_delay(false)
        };
        ctx.request_repaint_after(delay);
    }

    /// После системного драга/ресайза окна (X11 `_NET_WM_MOVERESIZE`) WM
    /// забирает указатель и ButtonReleased в приложение НЕ приходит — egui
    /// вечно считает кнопку зажатой, `dragged` держит виджет драга, и весь
    /// остальной UI лишается hovered (скролл таймлайна умирал до клика,
    /// п.28 р.7). Звать сразу после send_viewport_cmd(StartDrag/BeginResize).
    pub(crate) fn release_pointer_after_wm_grab(ctx: &egui::Context) {
        ctx.stop_dragging();
        ctx.input_mut(|i| i.pointer = Default::default());
    }

    /// Строка подсказки — рисуется ПОСЛЕДНЕЙ в кадре, поверх статусбара
    /// (п.1 р.9). Статусбар как панель рисуется вторым, задолго до кнопок,
    /// поэтому читать `hover_hint` внутри него бессмысленно — там всегда
    /// None. Слой Foreground + interactable(false): драг окна за статусбар
    /// не перехватывается.
    fn draw_status_hint(&mut self, ctx: &egui::Context) {
        let strip = self.statusbar_rect;
        if !strip.is_finite() || strip.width() < 1.0 {
            return;
        }
        // подсказки общих виджетов приходят из замыканий окон (сток
        // `widgets::hint_sink`, п.415): забираем ровно здесь, в конце
        // кадра, чтобы место показа осталось ОДНО (правило 10)
        if self.hover_hint.is_none() {
            self.hover_hint = crate::widgets::hint_sink::take();
        } else {
            let _ = crate::widgets::hint_sink::take();
        }
        let mode = if self.dnd_hover.is_some() { Some(tr(k::hint::DROP_HERE)) } else { None };
        let status =
            self.status.as_ref().map(|(m, at)| (m.as_str(), at.elapsed().as_millis() as u64));
        let blocked = self
            .blocked
            .map(|b| self.blocked_text(b))
            .or_else(|| {
                self.blocked_material
                    .map(|t| crate::tf!(k::tl::hint::TRIM_MATERIAL_END, format!("{:.1}", t as f64 / 90_000.0)))
            })
            .or_else(|| {
            self.error_status
                .as_ref()
                .filter(|(_, at)| (at.elapsed().as_millis() as u64) < STATUS_TTL_MS)
                .map(|(t, _)| t.clone())
        });
        let (text, kind) = status_line(
            blocked.as_deref(),
            self.hover_hint.as_deref(),
            mode,
            status,
            STATUS_TTL_MS,
            tr(k::hint::IDLE),
        );
        if kind != HintKind::Status
            && self
                .status
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed().as_millis() as u64 >= STATUS_TTL_MS)
        {
            self.status = None;
        }
        let col = match kind {
            HintKind::Error => HINT_ERROR,
            HintKind::Status => egui::Color32::from_rgb(255, 210, 120),
            HintKind::Mode => egui::Color32::from_rgb(150, 215, 175),
            HintKind::Hover => egui::Color32::from_gray(205),
            HintKind::Idle => egui::Color32::from_gray(120),
        };
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("status-hint"),
        ));
        let galley = painter.layout_no_wrap(text.clone(), egui::FontId::proportional(12.0), col);
        let x1 = strip.right() - 6.0;
        // длинная подсказка спокойно затирает метрики слева — она важнее
        let plate = egui::Rect::from_min_max(
            egui::pos2((x1 - galley.size().x - 8.0).max(strip.left()), strip.top()),
            egui::pos2(x1, strip.bottom()),
        );
        painter.rect_filled(plate, 0.0, crate::theme::PANEL);
        painter.galley(
            egui::pos2(plate.left() + 4.0, strip.center().y - galley.size().y * 0.5),
            galley,
            col,
        );
        // что РЕАЛЬНО отрисовано — для harness-теста (проверять внутреннее
        // поле hover_hint бесполезно: оно наполняется уже после статусбара)
        self.status_line_shown = (text, kind);
    }

    /// Палитра поиска по проекту (п.3 р.10): Ctrl+F, стрелки, Enter.
    /// Esc обрабатывается ЗДЕСЬ: пока фокус в поле, handle_keys не
    /// выполняется вообще и до общего Esc-каскада дело не доходит.
    fn draw_search(&mut self, ctx: &egui::Context) {
        if !self.search_open {
            return;
        }
        let hits = search_hits(self.model.project(), &self.bin_groups, &self.search_query);
        let mut open = true;
        let mut go: Option<Hit> = None;
        egui::Window::new(tr(k::win::SEARCH_TITLE))
            .id(egui::Id::new("kt_search"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -80.0])
            .default_width(420.0)
            .show(ctx, |ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text(tr(k::search::FIELD))
                        .desired_width(400.0),
                );
                if !r.has_focus() && self.search_focus {
                    r.request_focus();
                    self.search_focus = false;
                }
                let (up, down, enter, esc) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowUp),
                        i.key_pressed(egui::Key::ArrowDown),
                        i.key_pressed(egui::Key::Enter),
                        i.key_pressed(egui::Key::Escape),
                    )
                });
                if down {
                    self.search_sel = (self.search_sel + 1).min(hits.len().saturating_sub(1));
                }
                if up {
                    self.search_sel = self.search_sel.saturating_sub(1);
                }
                if esc {
                    self.search_open = false;
                }
                ui.separator();
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (i, h) in hits.iter().enumerate() {
                        let tag = match h.kind {
                            HitKind::Marker => tr(k::search::MARKER),
                            HitKind::Media => tr(k::search::MEDIA),
                            HitKind::Track => tr(k::search::TRACK),
                            HitKind::Clip => tr(k::search::CLIP),
                            HitKind::Group => tr(k::search::GROUP),
                        };
                        let sel = i == self.search_sel;
                        let text = format!("{tag}  ·  {}", h.label);
                        if ui.selectable_label(sel, text).clicked() {
                            go = Some(h.clone());
                        }
                    }
                    if hits.is_empty() && !self.search_query.trim().is_empty() {
                        ui.weak(tr(k::search::NOTHING));
                    }
                });
                if enter && let Some(h) = hits.get(self.search_sel) {
                    go = Some(h.clone());
                }
            });
        if !open {
            self.search_open = false;
        }
        if let Some(h) = go {
            self.search_open = false;
            match h.kind {
                HitKind::Media => {
                    if let Some(id) = h.media {
                        self.place_media_at_cursor(id);
                    }
                }
                // группа: показать её в бине — свободный режим и
                // выделение целиком (переход «в никуда» бесполезен)
                HitKind::Group => {
                    if let Some(first) = h.media
                        && let Some(gi) = self.bin_group_of(first)
                    {
                        self.bin_free = true;
                        self.bin_sel = self.bin_groups[gi].members.iter().copied().collect();
                    }
                }
                _ => {
                    self.player.seek(Tick(h.at));
                    self.pending_reveal = Some(h.at);
                    match h.clip {
                        Some(id) => self.select_only(id),
                        None => self.selected.clear(),
                    }
                }
            }
        }
    }

    /// Прыжок к предыдущей/следующей точке остановки (п.16 р.8).
    /// `far` (Shift) — сразу в начало/конец проекта.
    pub(crate) fn jump_stop(&mut self, forward: bool, far: bool) {
        let p = self.model.project();
        let pts = stop_points(p);
        // «конец» — не только конец материала: маркер или граница региона
        // могут лежать правее, и тогда прыжок вперёд утаскивал курсор
        // НАЗАД (п.7 р.10)
        let end = kt_engine::export::project_range(p).0.max(pts.last().copied().unwrap_or(0));
        let cursor = self.player.transport.playhead().0;
        let at = if far {
            if forward { end } else { 0 }
        } else if forward {
            next_stop(cursor, &pts).unwrap_or(end)
        } else {
            prev_stop(cursor, &pts).unwrap_or(0)
        };
        let at = Tick(at.max(0)).align_audio_up();
        self.player.seek(at);
        // окно таймлайна обязано показать курсор (п.11 р.9); применяем в
        // draw_timeline, где известны актуальные rect и зум
        self.pending_reveal = Some(self.player.transport.playhead().0);
    }

    /// Навигация «Назад»/«Вперёд» с Shift = ТЯНУТЬ регион (п.2 р.16).
    ///
    /// Shift — тот же модификатор «расширить выделение», что в Premiere,
    /// Resolve и DAW. Shift+A ставит ЛЕВЫЙ край региона в новую позицию,
    /// Shift+D — правый; если региона ещё нет, вторым краем становится
    /// позиция ДО прыжка, поэтому первый же Shift+D выделяет пройденный
    /// участок. Регион — «удобство», поэтому только `dirty_ui`.
    pub(crate) fn nav_jump(&mut self, forward: bool, far: bool, drag_region: bool) {
        let before = self.player.transport.playhead();
        self.jump_stop(forward, far);
        if !drag_region {
            return;
        }
        if self.model.project().region.is_none() {
            let at = self.player.transport.playhead();
            self.model.set_region(Region::normalized(before, at));
        }
        // край ставится в НОВУЮ позицию курсора; противоположный край
        // зажимается внутри set_region_edge_from_cursor
        self.set_region_edge_from_cursor(!forward);
    }

    /// Зум кнопками — якорь на PLAYHEAD (п.26 р.7: раньше якорился левый
    /// край окна). Курсор остаётся на той же экранной позиции; если он вне
    /// окна — центрируем.
    pub(crate) fn zoom_at_playhead(&mut self, factor: f64) {
        self.follow_playhead = false; // ручной зум (п.14 р.6)
        self.last_user_scroll = Some(std::time::Instant::now());
        let ph = self.player.transport.playhead().0 as f64;
        let old_tpp = self.ticks_per_px;
        let new_tpp = (old_tpp * factor).clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
        let w = (self.timeline_rect.width() as f64 - crate::timeline::GUTTER_W as f64).max(1.0);
        let px = (ph - self.view_start) / old_tpp; // экранная позиция курсора
        let px = if (0.0..=w).contains(&px) { px } else { w * 0.5 };
        self.view_start = ph - px * new_tpp;
        self.ticks_per_px = new_tpp;
    }

    /// Подсказка: попап + строка в правом низу статусбара (п.16 р.7 —
    /// hover_hint читает draw_statusbar). Все on_hover_text ходят сюда.
    pub(crate) fn hint(&mut self, r: egui::Response, text: &str) -> egui::Response {
        if r.hovered() {
            self.hover_hint = Some(text.to_string());
        }
        // п.14 р.8: НИКАКИХ всплывающих попапов — подсказка живёт ровно в
        // одном месте, в правом низу строки состояния
        r
    }

    /// Транспорт-машина Space/кнопки (п.15 раунда 6, тест 58; заменяет
    /// прежние правила «из паузы при регионе — всегда в начало», санкция
    /// пользователя):
    /// - играет → пауза, курсор на месте;
    /// - пауза + повторный Space в течение DOUBLE_PAUSE_MS → «стоп»:
    ///   курсор в начало региона (или 0);
    /// - пауза + Space позже → resume С ТОГО ЖЕ места; но при включённом
    ///   loop и курсоре ВНЕ региона — с начала региона (wrap транспорта
    ///   работает только изнутри региона).
    pub fn transport_toggle(
        playing: bool,
        pause_recent: bool,
        cursor: i64,
        region: Option<(i64, i64)>,
        loop_enabled: bool,
    ) -> TransportAction {
        if playing {
            return TransportAction::Pause;
        }
        let region = region.filter(|(s, e)| e > s);
        if pause_recent {
            return TransportAction::SeekStart(region.map(|(s, _)| s).unwrap_or(0));
        }
        match region {
            Some((s, e)) if loop_enabled && !(cursor >= s && cursor < e) => {
                TransportAction::Resume { seek: Some(s) }
            }
            // п.29 р.7: регион отыгран (авто-пауза оставила курсор на/за
            // концом) → Play играет регион заново; иначе resume попадал в
            // окно авто-паузы тем же кадром и «ничего не делал». Только
            // cursor >= e: игра ДО региона не трогается.
            Some((s, e)) if !loop_enabled && cursor >= e => {
                TransportAction::Resume { seek: Some(s) }
            }
            _ => TransportAction::Resume { seek: None },
        }
    }

    /// Разгон удержанием пробела (п.3 р.16, как на YouTube): за кадр dt
    /// множитель подъезжает к `SPACE_BOOST_MAX`, до потолка ~2 секунды.
    ///
    /// Схема АНАЛИТИЧЕСКАЯ (экспонента), а не явная: `stable_dt` после
    /// простоя приходит равным задержке idle-перерисовки (до 250 мс), и
    /// явное интегрирование на таких шагах перелетает цель (п.7 р.9 —
    /// ровно так фейдер улетал на 400 dB).
    pub fn space_boost_step(cur: f32, dt: f32) -> f32 {
        // за SPACE_BOOST_SECS проходим 95% пути до потолка
        let k = -(0.05f32.ln()) / SPACE_BOOST_SECS;
        let a = (-k * dt.clamp(0.0, 0.25)).exp();
        let next = SPACE_BOOST_MAX + (cur - SPACE_BOOST_MAX) * a;
        next.clamp(1.0, SPACE_BOOST_MAX)
    }

    /// Кадр разгона: пока пробел зажат дольше порога — скорость ползёт
    /// вверх. Вызывается РОВНО один раз за кадр (не из handle_keys: тот
    /// зовётся дважды при отделённом превью и разгонял бы вдвое быстрее).
    fn tick_space_boost(&mut self, ctx: &egui::Context) {
        let Some(t0) = self.space_down_at else { return };
        // шаттл — более явное намерение пользователя, ему не мешаем
        if self.shuttle_override.is_some() {
            return;
        }
        if t0.elapsed().as_millis() < SPACE_HOLD_MS {
            return;
        }
        if self.space_boost.is_none() {
            self.space_boost = Some(1.0);
            // стояли — удержание сразу запускает игру (иначе «ускорять»
            // нечего), но play/pause по отпусканию уже не переключаем
            if !self.player.transport.is_playing() {
                self.on_play();
            }
        }
        let dt = ctx.input(|i| i.stable_dt);
        let b = Self::space_boost_step(self.space_boost.unwrap_or(1.0), dt);
        self.space_boost = Some(b);
        // гистерезис: каждый set_playback_rate сбрасывает окно
        // экстраполяции часов показа (п.14 р.14) — дёргать его на каждую
        // сотую нельзя
        let want = self.base_rate * b;
        if boost_needs_update(want, self.player.playback_rate(), self.base_rate) {
            self.player.set_playback_rate(want);
        }
        ctx.request_repaint();
    }

    /// Что показывать на НИЖНЕЙ (временной) полосе контрола скорости:
    /// shuttle, а при разгоне удержанием пробела — фактическую скорость
    /// (п.3 р.16). Иначе разгон идёт, а виджет молча показывает 1×.
    pub fn speed_temp_slider(&self) -> Option<f32> {
        if self.shuttle_override.is_some() {
            return self.shuttle_override;
        }
        self.space_boost.map(|_| rate_to_slider(self.player.playback_rate()))
    }

    /// Пробел отпущен: короткое нажатие = play/pause, длинное = конец
    /// разгона (транспорт при этом НЕ переключается).
    fn on_space_released(&mut self) {
        let held = self.space_down_at.take().map(|t| t.elapsed().as_millis()).unwrap_or(0);
        if self.space_boost.take().is_some() {
            self.player.set_playback_rate(self.base_rate);
            return;
        }
        if held < SPACE_HOLD_MS {
            self.on_transport_toggle();
        }
    }

    /// Единая точка Space/кнопки Play-Pause (обе идут сюда).
    pub fn on_transport_toggle(&mut self) {
        let p = self.model.project();
        let region = p.region.map(|r| (r.start.0, r.end.0));
        let pause_recent =
            self.paused_at.map(|t| t.elapsed().as_millis() < DOUBLE_PAUSE_MS).unwrap_or(false);
        match Self::transport_toggle(
            self.player.transport.is_playing(),
            pause_recent,
            self.player.transport.playhead().0,
            region,
            p.loop_enabled,
        ) {
            TransportAction::Pause => {
                self.player.pause();
                self.paused_at = Some(std::time::Instant::now());
            }
            TransportAction::SeekStart(at) => {
                self.player.seek(Tick(at));
                self.paused_at = None;
            }
            TransportAction::Resume { seek } => {
                if let Some(at) = seek {
                    self.player.seek(Tick(at));
                }
                self.paused_at = None;
                self.on_play();
            }
        }
    }

    /// Запуск воспроизведения с текущей позиции (resume-семантика п.15).
    pub fn on_play(&mut self) {
        self.follow_playhead = true;
        self.player.play();
    }

    /// Регион отпущен мышью (конец драга). Во время воспроизведения курсор
    /// немедленно уводится в начало нового региона (правило «выделил во
    /// время плея — отпустил — играем регион», п.4/v1-clamp).
    pub fn on_region_released(&mut self) {
        if self.player.transport.is_playing()
            && let Some(r) = self.model.project().region
            && r.end > r.start
        {
            self.player.seek(r.start);
        }
    }

    /// Путь sidecar индекс-БД текущего проекта (None у безымянного).
    fn index_db_path(&self) -> Option<std::path::PathBuf> {
        self.project_path.as_ref().map(|p| p.with_extension("ktindex.db"))
    }

    /// Покомпонентный максимум разрешения по видео-источникам проекта
    /// (порт computeMaxSourceVideoResolution; короткоживущие пробы).
    /// Кэшированный вариант (Р.31 п.3): пересчёт только при смене модели.
    pub fn max_source_resolution_cached(&mut self) -> Option<(u32, u32)> {
        if let Some((rev, v)) = self.max_src_cache
            && rev == self.model_rev
        {
            return v;
        }
        let v = self.max_source_resolution();
        self.max_src_cache = Some((self.model_rev, v));
        v
    }

    pub fn max_source_resolution(&self) -> Option<(u32, u32)> {
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let db = self.index_db_path();
        let (mut mw, mut mh) = (0u32, 0u32);
        for m in &self.model.project().media {
            // псевдо-источники не участвуют: «натив» генератора — это
            // запас деталей для ступени декода (GEN_NATIVE, Р.52), а не
            // разрешение материала, и в «Max (по источникам)» ему не место
            if kt_model::is_pseudo_path(&m.path) {
                continue;
            }
            if let Ok(src) =
                open_source_indexed(&resolve_media_path(base.as_deref(), &m.path), db.as_deref())
            {
                let i = src.info();
                if i.has_video {
                    mw = mw.max(i.width);
                    mh = mh.max(i.height);
                }
            }
        }
        (mw >= 2 && mh >= 2).then_some((mw & !1, mh & !1))
    }

    /// Esc-каскад (п.11): окна → активный драг → регион+выделение.
    pub fn escape_action(&mut self) {
        // 0) ввод скорости числом (Р.57) — он живёт НА МЕСТЕ полосы, и
        // закрыть его надо раньше всего остального; фокус отдаёт
        // `draw_speed_control` следующим кадром
        if self.speed_entry.take().is_some() {
            return;
        }
        if self.search_open {
            self.search_open = false;
            return;
        }
        // 0) окно префикса вставки (Р.35 п.4)
        if self.preset_save.take().is_some() {
            return;
        }
        if self.paste_names.take().is_some() {
            return;
        }
        // 1) закрыть верхнее наше окно. Окна публичных параметров (Р.59,
        // стадия 3) идут ПЕРЕД окнами эффектов: их открывают поверх — и
        // человек ждёт, что Esc уберёт то, что он видит последним.
        // Диалог имени ручки в каскад не входит намеренно: он гасит Esc
        // сам, как окно имени ленты, — пока в нём фокус, сюда вообще не
        // доходит (`egui_wants_keyboard_input`).
        if let Some(w) = self.published_win.as_mut().filter(|w| w.open) {
            w.open = false;
            return;
        }
        if self.render_setup.open {
            self.render_setup.open = false;
            return;
        }
        // окно дорожки (Р.62) — такое же «наше окно», как окно эффектов
        if let Some(w) = self.track_win.as_mut().filter(|w| w.open) {
            w.open = false;
            return;
        }
        if let Some(ed) = self.fx_editors.iter_mut().rev().find(|e| e.open) {
            ed.open = false;
            return;
        }
        if let Some(ed) = self.morph_editors.iter_mut().rev().find(|e| e.open) {
            ed.open = false;
            return;
        }
        if self.notices_open {
            self.notices_open = false;
            return;
        }
        if self.perf.diag_open {
            self.perf.diag_open = false;
            return;
        }
        // 2) отменить активный драг
        if self.drag.is_some() {
            self.drag = None;
            self.snap_line = None;
            return;
        }
        // 3) снять регион и выделение (loop остаётся взведённым — п.3 р.11)
        let had = self.model.project().region.is_some()
            || !self.selected.is_empty()
            || !self.selected_morph.is_empty()
            || !self.selected_camera.is_empty();
        if self.model.project().region.is_some() {
            self.clear_region_keep_loop();
        }
        self.selected.clear();
        self.selected_morph.clear();
        self.selected_camera.clear();
        // 4) СВЕРНУТЬ РАСКРЫТИЕ — перед выходом из ленты (Р.55, С5):
        // свёртка полосы стоит человеку меньше внимания, чем выход наружу,
        // а вернуть её одним жестом дешевле.
        if !had && self.collapse_all_in_place() {
            return;
        }
        // 5) ВЫЙТИ ИЗ ЛЕНТЫ — ПОСЛЕДНИМ (Р.50, В3). Стой он раньше,
        // «снять выделение» выкидывало бы человека наружу: одна клавиша
        // делала бы два разных дела, и второе — необратимое по вниманию.
        if !had && !self.nest_path.is_empty() {
            self.leave_nest();
        }
    }

    /// Перестановка display-строк дорожек — ОДНА команда на любые виды
    /// (Р.56, п.449).
    ///
    /// До Р.56 их было две: между видео-строками перекладывалось
    /// СОДЕРЖИМОЕ (`reorder_lane`, батч `MoveClip`), а любой другой случай
    /// был «чисто визуальным сдвигом вектора». Из-за второй ветки
    /// видео-строка, перетащенная к морф- или камерной строке, меняла
    /// место на экране и НЕ меняла номер — то есть оставалась тем же
    /// слоем композита. Теперь номер дорожки = ранг строки, и
    /// перенумеровывает содержимое сама модель: перетащил дорожку —
    /// изменил монтаж (решение владельца Р.35), и пустая строка двигается
    /// наравне с полной.
    pub fn reorder_rows(&mut self, from: usize, to: usize) {
        let mut rows = self.model.project().rows();
        if from == to || from >= rows.len() || to >= rows.len() {
            return;
        }
        // Строки адресуются КЛЮЧОМ, а не индексом: свёртка полосы
        // (ниже) меняет их число, и индексы жеста после неё врали бы.
        let (a, b) = (rows[from], rows[to]);
        self.collapse_bands_for_rows();
        rows = self.model.project().rows();
        let (Some(from), Some(to)) =
            (rows.iter().position(|&k| k == a), rows.iter().position(|&k| k == b))
        else {
            // строка принадлежала свёрнутой полосе — переставлять уже
            // нечего, и это честнее, чем переставить не ту
            return;
        };
        let e = rows.remove(from);
        rows.insert(to, e);
        let perm = kt_model::LanePerm::from_rows(
            &rows,
            kt_model::LaneBases::of_root(self.model.project()),
        );
        if self.apply(Command::SetLaneOrder { order: rows }) {
            // имена и высоты живут ВНЕ undo и адресуются парой (вид,
            // номер) — значит едут тем же отображением, что содержимое
            // имя и высота живут НА ДОРОЖКЕ и едут вместе с объектом —
            // перекладывать их отдельно больше нечем (Р.62)
            let _ = perm;
        }
    }

    /// Свернуть раскрытие перед правкой СТРОК и сказать об этом
    /// (Р.56, п.449).
    ///
    /// Порядок и число строк при открытой полосе модель не принимает
    /// (`CommandError::BandsOpen`): полоса живёт в хвосте числового
    /// пространства, и перенумерация разложила бы её события не по тем
    /// лентам. Молча свернуть было бы тоже неверно — человек ищет своё
    /// раскрытие там, где оставил (правило 16).
    pub fn collapse_bands_for_rows(&mut self) {
        if self.collapse_all_in_place() {
            self.note(tr(k::msg::ROWS_COLLAPSED_BAND));
        }
    }

    /// Вставка дорожки на КОНКРЕТНОЕ место: команда модели, display-строка
    /// и ИМЕНА разом (п.5 р.16).
    ///
    /// Имена раньше не двигались вовсе: функции `lane_names_insert` и
    /// соседние существовали и были покрыты t90, но из приложения не
    /// вызывались — после вставки подпись оказывалась у чужой дорожки.
    pub fn insert_lane_at(&mut self, kind: LaneKind, at: u32) -> bool {
        let ok = self.apply(Command::InsertTrack { kind, at });
        ok
    }

    /// Добавить дорожку СРАЗУ ПОД кликнутой строкой (п.5 р.16: «чтобы
    /// добавление происходило не в самый низ, а после той дорожки по
    /// которой кликнули»).
    ///
    /// Номер новой дорожки — её РАНГ среди строк своего вида (Р.56,
    /// п.449). До Р.56 строка другого вида давала `at = tail`, то есть
    /// САМЫЙ БОЛЬШОЙ номер: дорожка вставала под потолок на экране и
    /// оказывалась НИЖНИМ слоем композита — «закинул туда видео, и его
    /// не видно» (претензия владельца п.2). Порядок и номер ставятся
    /// ОДНИМ батчем, то есть одним шагом undo: их было два.
    pub fn insert_lane_after_row(&mut self, kind: LaneKind, after: Option<(LaneKind, u32)>) {
        self.collapse_bands_for_rows();
        let rows = self.model.project().rows();
        // ранг = сколько строк этого вида стоит НЕ НИЖЕ кликнутой
        let at = match after.and_then(|row| rows.iter().position(|&x| x == row)) {
            Some(pos) => rows[..=pos].iter().filter(|e| e.0 == kind).count() as u32,
            None => rows.iter().filter(|e| e.0 == kind).count() as u32,
        };
        let insert = Command::InsertTrack { kind, at };
        // Порядок считаем САМИ и кладём в тот же батч: `lane_order_insert`
        // ставит строку перед следующей дорожкой СВОЕГО вида, а если такой
        // нет — в самый конец списка. Живая проверка: клик по V2 при
        // порядке V1,V2,A1,A2 давал V1,V2,A1,A2,V3 — то самое «в самый
        // низ», на которое жаловались.
        let mut order = rows;
        kt_model::lane_order_insert(&mut order, kind, at);
        if let Some(row) = after
            && let Some(f) = order.iter().position(|&e| e == (kind, at))
        {
            let e = order.remove(f);
            let to = order.iter().position(|&x| x == row).map(|i| i + 1).unwrap_or(order.len());
            order.insert(to, e);
        }
        self.apply_batch(vec![insert, Command::SetLaneOrder { order }]);
    }

    /// Удаление дорожки (п.8, решение пользователя: вместе с событиями,
    /// один undo-шаг). С Р.62 это ОДНА команда модели: она уносит события
    /// дорожки, съезжает номера выше и забирает с собой имя, высоту и
    /// свойства — трёх параллельных таблиц больше нет.
    pub fn delete_lane(&mut self, kind: ClipKind, lane: u32) {
        // правку СТРОК при открытой полосе модель не принимает (Р.56)
        self.collapse_bands_for_rows();
        let lk = kt_model::lane_key_of(kind);
        if self.model.project().lanes(lk) <= 1 {
            self.note(tr(k::msg::LAST_LANE));
            return;
        }
        self.apply(Command::RemoveTrack { kind: lk, at: lane });
    }

    /// Видимый диапазон таймлайна в тиках (для расчёта «куда смотреть»).
    pub(crate) fn view_range_ticks(&self) -> (i64, i64) {
        let w = self.tl_geom.as_ref().map(|g| g.rect.width() as f64).unwrap_or(0.0);
        (self.view_start as i64, (self.view_start + w * self.ticks_per_px) as i64)
    }

    /// Текст подсказки о препятствии: внутри вида — просто «мешает
    /// событие (подсвечено)», за пределами — со стороной и расстоянием,
    /// чтобы не пришлось «самому туда промотать» (п.5 Р.28).
    pub(crate) fn blocked_text(&self, b: crate::timeline::Blocked) -> String {
        use crate::timeline::{BlockView, block_view};
        let view = self.view_range_ticks();
        let secs = |ticks: i64| format!("{:.1}", ticks as f64 / 90_000.0);
        match block_view(b.span, view) {
            BlockView::Visible => tr(k::tl::hint::CLAMPED).to_string(),
            BlockView::Left(d) => crate::tf!(k::tl::hint::CLAMPED_FAR_LEFT, secs(d)),
            BlockView::Right(d) => crate::tf!(k::tl::hint::CLAMPED_FAR_RIGHT, secs(d)),
        }
    }

    /// Выложить «чёрный ящик» воспроизведения в файл рядом с проектом
    /// (безымянный проект — в рабочий каталог). Р.28, п.10.
    pub(crate) fn save_flight_log(&self) -> std::io::Result<String> {
        let path = match &self.project_path {
            Some(p) => p.with_extension("ktflight.log"),
            None => std::path::PathBuf::from("kt-flight.log"),
        };
        std::fs::write(&path, self.player.flight.dump())?;
        Ok(path.display().to_string())
    }

    pub fn note(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        self.push_notice(Severity::Info, text.clone());
        self.status = Some((text, std::time::Instant::now()));
    }

    /// Отказ: невозможность действия показывается как ОШИБКА, а не блёклой
    /// подсказкой (правило 16) — красным в строке состояния, в журнал и в
    /// ретроспективу.
    pub fn refuse(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        crate::journal::log("REFUSE", &text);
        self.push_notice(Severity::Warn, text.clone());
        self.error_status = Some((text, std::time::Instant::now()));
    }

    /// Warning-уведомление: в статусбар и в ретроспективу (п.18).
    pub fn warn(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        // в журнал — чтобы у жалобы «оно что-то сказало и пропало» был адрес
        crate::journal::log("WARN", &text);
        self.push_notice(Severity::Warn, text.clone());
        self.status = Some((text, std::time::Instant::now()));
    }

    /// Сказать ОДИН раз за сессию, что каталог этой файловой системы не
    /// сбрасывается: файл на месте, но при потере питания может потеряться
    /// само переименование (exFAT, сетевые шары, не-unix).
    /// Записать файл восстановления ПРЯМО СЕЙЧАС, синхронно. Зовётся там,
    /// где фонового потока уже не будет: `on_exit` и (п.354) `Drop`. Исход —
    /// в журнал: окна нет, показать некому.
    pub(crate) fn write_recovery_now(&mut self) {
        let target = self.recovery_target();
        let json = kt_otio::save_project(
            &self.model.document(),
            &self.retained,
            self.project_path.as_deref().and_then(|p| p.parent()),
        );
        match write_project_guarded(&self.recovery_rev, &target, self.model_rev, &json) {
            Ok(SaveOutcome::Wrote { dir_synced }) => {
                self.recovery_written = Some(target);
                if !dir_synced {
                    crate::journal::log("WARN", "recovery write: directory sync unsupported");
                }
            }
            Ok(SaveOutcome::SkippedNewerOnDisk) => {}
            Err(e) => crate::journal::log(
                "WARN",
                &format!("recovery write failed: {} ({e})", target.display()),
            ),
        }
    }

    /// Поискать несохранённую работу и предложить её. Зовётся на старте и
    /// пунктом меню: механизм обязан работать ПОВТОРНО (правило 14), а не
    /// один раз при запуске.
    pub fn offer_recovery(&mut self, quiet: bool) {
        let list = recovery_candidates(self.project_path.as_deref());
        self.offer_recovery_list(list, quiet);
    }

    pub(crate) fn offer_recovery_list(&mut self, list: Vec<RecoveryCandidate>, quiet: bool) {
        self.recovery_offer = list;
        self.recovery_pick = 0;
        self.recovery_open = !self.recovery_offer.is_empty();
        if self.recovery_offer.is_empty() && !quiet {
            self.note(tr(k::msg::RECOVERY_NONE));
        }
    }

    /// Забрать работу из файла восстановления В РЕДАКТОР. Файл-источник не
    /// трогается: восстановиться можно ещё раз (передумал, отменил, открыл
    /// не тот кандидат), а исходник несохранённой работы уничтожать вправе
    /// только пользователь.
    pub fn restore_recovery(&mut self, i: usize) {
        let Some(c) = self.recovery_offer.get(i).cloned() else { return };
        match load_project_file(&c.path) {
            Ok((project, retained, notices)) => {
                self.model = kt_model::TimelineModel::new(project);
                self.retained = retained;
                self.model_rev += 1;
                self.push_project_to_player();
                self.sync_media_sources();
                // работа В РЕДАКТОРЕ и нигде больше: звёздочка горит, пока
                // пользователь не нажмёт Ctrl+S
                self.dirty_content = true;
                self.recovery_open = false;
                for n in notices {
                    self.warn(n);
                }
                self.note(crate::tf!(k::msg::RECOVERY_RESTORED, c.path.display()));
            }
            Err(e) => self.warn(crate::tf!(k::msg::RECOVERY_FAILED, c.path.display(), e)),
        }
    }

    /// Выбросить файл восстановления: работа из него не нужна.
    pub fn forget_recovery(&mut self, i: usize) {
        if i >= self.recovery_offer.len() {
            return;
        }
        let c = self.recovery_offer.remove(i);
        let _ = std::fs::remove_file(&c.path);
        if self.recovery_written.as_deref() == Some(c.path.as_path()) {
            self.recovery_written = None;
        }
        self.recovery_pick = self.recovery_pick.min(self.recovery_offer.len().saturating_sub(1));
        self.recovery_open = !self.recovery_offer.is_empty();
        self.note(crate::tf!(k::msg::RECOVERY_FORGOTTEN, c.path.display()));
    }

    /// Синхронизировать два выделенных события по звуку (Р.49, п.375).
    ///
    /// ОДНО ТЕЛО на оба пути — пункт меню события и пункт главного меню
    /// (правило 17: парные способы ведут себя одинаково). Невозможность
    /// показывается ОШИБКОЙ, а не блёклой подсказкой (правило 16): «индекс
    /// ещё строится» и «записи не совпадают» — разные причины, и обе
    /// называются вслух.
    pub fn sync_selected_by_audio(&mut self) {
        let ids: Vec<kt_model::ClipId> = self.selected.iter().copied().collect();
        if ids.len() != 2 {
            self.warn(tr(k::msg::SYNC_NEED_TWO));
            return;
        }
        let (a, b) = (ids[0], ids[1]);
        let (Some(ca), Some(cb)) = (self.model.clip(a).cloned(), self.model.clip(b).cloned())
        else {
            return;
        };
        // Пирамиды обоих файлов обязаны быть готовы. «Ещё не готово» — это
        // честный отказ, а не «сдвиг ноль»: ноль неотличим от «они и так
        // совпадают», и монтажёр поверит.
        let peaks = self.peaks.lock().unwrap();
        let (Some(pa), Some(pb)) = (peaks.get(&ca.media), peaks.get(&cb.media)) else {
            drop(peaks);
            self.warn(tr(k::msg::SYNC_PENDING));
            return;
        };
        let la: Vec<Vec<(i16, i16)>> = pa.iter().map(|(l, _)| l.clone()).collect();
        let lb: Vec<Vec<(i16, i16)>> = pb.iter().map(|(l, _)| l.clone()).collect();
        drop(peaks);
        let Some(off) = kt_media::sync::offset_from_pyramids(&la, &lb, kt_media::PEAK_BLOCK_FRAMES)
        else {
            self.warn(tr(k::msg::SYNC_NO_MATCH));
            return;
        };
        let cmds =
            match kt_model::ops::sync_by_audio_commands(self.model.project(), a, b, off.ticks) {
                kt_model::ops::SyncPlan::Move(c) => c,
                kt_model::ops::SyncPlan::AlreadyAligned => {
                    self.note(crate::tf!(k::msg::SYNC_DONE, 0, (off.confidence * 100.0).round()));
                    return;
                }
                kt_model::ops::SyncPlan::OutOfRange => {
                    self.warn(tr(k::msg::SYNC_OUT_OF_RANGE));
                    return;
                }
            };
        if self.apply_batch(cmds) {
            self.note(crate::tf!(
                k::msg::SYNC_DONE,
                off.ticks.as_ms(),
                (off.confidence * 100.0).round()
            ));
        }
    }

    // ---- вложенные ленты (Р.50, п.381) ------------------------------------

    /// Свернуть выделение в ленту. ОДНО ТЕЛО на все пути вызова (правило 17):
    /// меню события, главное меню.
    pub fn collapse_selection(&mut self) {
        let ids = self.selected_with_groups();
        if ids.is_empty() {
            self.warn(tr(k::msg::NEST_NEED_SELECTION));
            return;
        }
        let mut n = kt_model::ops::NestCounters::of(self.model.project());
        let taken: Vec<String> =
            self.model.project().sequences.iter().map(|s| s.name.clone()).collect();
        let name = kt_model::unique_name(&taken, tr(k::msg::NEST_DEFAULT_NAME), "copy");
        let (cmds, rep) =
            kt_model::ops::collapse_into_sequence(self.model.project(), &ids, &name, n.ids());
        if cmds.is_empty() {
            self.warn(tr(k::msg::NEST_NEED_SELECTION));
            return;
        }
        // Материал НЕ режется: события переехали в ленту целиком, поэтому
        // возврата ручек (apply_batch_restoring) тут быть не должно — он
        // отдал бы материал клипам, которых в корне уже нет.
        if !self.apply_batch(cmds) {
            return;
        }
        if let Some(seq) = rep.seq
            && let Some(inst) = self.model.project().clips.iter().find(|c| c.media == seq)
        {
            let id = inst.id;
            self.select_only(id);
        }
        self.note(crate::tf!(k::msg::NEST_COLLAPSED, name.clone()));
        // Морфер-слой, накрывавший свёрнутое СНАРУЖИ, теперь бьёт по
        // собранной картинке, а не по каждому куску: картинка законно
        // изменилась, и человек обязан узнать об этом СРАЗУ, а не искать
        // причину потом.
        if rep.outer_layers > 0 {
            self.warn(crate::tf!(k::msg::NEST_OUTER_LAYERS, rep.outer_layers));
        }
        // Что УЕХАЛО внутрь — тоже обязано быть названо (правило 16):
        // человек ищет свой слой или камеру там, где они были, а их там
        // больше нет. До Р.55 слой уезжал и МОЛЧА умирал внутри (порядок
        // строк ленты был дефолтным, и под морф-строкой не оказывалось
        // ничего), а камера не уезжала вовсе.
        if rep.moved_layers > 0 || rep.moved_cameras > 0 {
            self.note(crate::tf!(k::msg::NEST_MOVED_EXTRAS, rep.moved_layers, rep.moved_cameras));
        }
        if rep.kept_ref_layers > 0 {
            self.warn(crate::tf!(k::msg::NEST_REF_LAYERS_STAY, rep.kept_ref_layers));
        }
    }

    /// Развернуть экземпляр обратно на дорожки.
    pub fn expand_nest(&mut self, id: ClipId) {
        let mut n = kt_model::ops::NestCounters::of(self.model.project());
        let (cmds, rep) = kt_model::ops::expand_instance(self.model.project(), id, n.ids());
        if cmds.is_empty() {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        }
        if !self.apply_batch(cmds) {
            return;
        }
        // правило 16: снятое обязано быть НАЗВАНО, а не обнаружено потом
        if rep.chain_dropped || rep.level_dropped {
            self.warn(tr(k::msg::NEST_EXPAND_DROPPED));
        } else {
            self.note(tr(k::msg::NEST_EXPANDED));
        }
    }

    /// Отвязать экземпляр от остальных мест: содержимое копируется в свою ленту.
    pub fn nest_make_independent(&mut self, id: ClipId) {
        let p = self.model.project();
        let Some(seq) = p.clips.iter().find(|c| c.id == id).and_then(|c| p.sequence(c.media))
        else {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        };
        let taken: Vec<String> = p.sequences.iter().map(|s| s.name.clone()).collect();
        let name = kt_model::unique_name(&taken, &seq.name, "copy");
        let mut n = kt_model::ops::NestCounters::of(p);
        let cmds = kt_model::ops::make_independent(self.model.project(), id, &name, n.ids());
        if self.apply_batch(cmds) {
            self.note(crate::tf!(k::msg::NEST_INDEPENDENT, name));
        }
    }

    /// Прыгнуть к СЛЕДУЮЩЕМУ месту, где стоит та же лента (Р.50, В3).
    ///
    /// Бейдж «×N» говорит, что мест несколько; без этого глагола он был бы
    /// сообщением без продолжения — «их три, а где они, ищи сам».
    pub fn goto_next_place(&mut self, id: ClipId) {
        let Some(c) = self.model.clip(id).cloned() else { return };
        let here = c.start;
        let mut places: Vec<kt_model::ClipEvent> =
            self.model.project().clips.iter().filter(|x| x.media == c.media).cloned().collect();
        if places.len() < 2 {
            self.warn(tr(k::msg::NEST_ONE_PLACE));
            return;
        }
        places.sort_by_key(|x| (x.start, x.id));
        // по кругу: после последнего — первое, иначе у глагола был бы
        // тупик ровно там, где он нужнее всего
        let next =
            places.iter().find(|x| x.start > here).or_else(|| places.first()).cloned().unwrap();
        self.select_only(next.id);
        self.player.seek(next.start);
        self.center_view_on(next.start);
    }

    /// Привести момент в вид (крошка общей арифметики: своей формулы
    /// прокрутки в местах вызова не заводим).
    fn center_view_on(&mut self, t: Tick) {
        let half = self.timeline_rect.width().max(1.0) as f64 * self.ticks_per_px / 2.0;
        self.view_start = (t.0 as f64 - half).max(self.view_floor(self.ticks_per_px));
    }

    /// НИЖНЯЯ ГРАНИЦА ВРЕМЕНИ ЖЕСТОВ над событиями (Р.58, п.462; решение
    /// владельца): в корне ноль — стена, внутри полного входа ось экрана =
    /// ось ленты, и её ноль стеной НЕ является — событие законно уезжает
    /// левее нуля (там его окно экземпляра всё равно не играет). Курсор и
    /// воспроизведение сюда не ходят: у них свой пол,
    /// `timeline::CURSOR_FLOOR`. Одна функция на все жесты; сторож по
    /// корпусу — t637c.
    pub fn time_floor(&self) -> i64 {
        if self.model.editing().is_some() { -(i64::MAX / 4) } else { 0 }
    }

    /// Нижняя граница ПРОКРУТКИ (`view_start`), в тиках: в корне прежние
    /// −40 px, внутри ленты — до самого раннего события содержимого (оно
    /// может стоять левее нуля), и те же −40 px запаса.
    pub fn view_floor(&self, tpp: f64) -> f64 {
        let earliest = if self.model.editing().is_some() {
            self.model.project().clips.iter().map(|c| c.start.0).min().unwrap_or(0).min(0)
        } else {
            0
        };
        earliest as f64 - 40.0 * tpp
    }

    /// Завести ПУСТУЮ ленту (Р.55, С6).
    ///
    /// Она сразу видна в бине, и с ней уже можно работать: поставить на
    /// таймлайн, раскрыть, переименовать, убрать. До этого раунда
    /// `AddSequence` конструировалась только внутри сворачивания и
    /// отвязывания — завести композицию с нуля было нечем.
    pub fn create_empty_reel(&mut self) {
        let taken: Vec<String> =
            self.model.document().sequences.iter().map(|s| s.name.clone()).collect();
        let name = kt_model::unique_name(&taken, tr(k::msg::NEST_DEFAULT_NAME), "copy");
        let id = kt_model::MediaId(kt_model::ops::NestCounters::of(&self.model.document()).media);
        if self.apply(Command::AddSequence { id, name: name.clone() }) {
            self.note(crate::tf!(k::msg::NEST_CREATED, name));
        }
    }

    /// Убрать ленты, на которые больше никто не смотрит.
    pub fn remove_unused_seqs(&mut self) {
        let cmds = kt_model::ops::remove_unused_sequences(self.model.project());
        let n = cmds.len();
        if n == 0 {
            self.note(tr(k::msg::NEST_NONE_UNUSED));
            return;
        }
        if self.apply_batch(cmds) {
            self.note(crate::tf!(k::msg::NEST_REMOVED_UNUSED, n));
        }
    }

    /// Раскрыть ленту события НА МЕСТЕ (Р.55, С5) — режим по умолчанию.
    ///
    /// Из корня мы при этом НЕ ВЫХОДИМ: играет вся программа, курсор в её
    /// оси, соседи и переходы на месте. Ровно это владелец и просил —
    /// «и по основному таймлайну даст проигрывание и по композиции
    /// одновременно». Плата — что на экране две системы координат, и
    /// строки полосы обязаны быть видно отличимы.
    ///
    /// Работает и ВНУТРИ полного входа (Р.58, п.460): полоса считается от
    /// канона, а каноном после `swap_reel` стоит текущая лента — базы у
    /// неё свои, и приложению здесь делать нечего. Прежний сторож
    /// «раскрыть можно только в корне» был ошибкой в голове, не в модели.
    pub fn expand_in_place(&mut self, id: ClipId) {
        if self.model.bands().iter().any(|b| b.inst == id) {
            self.collapse_in_place(id);
            return; // жест работает ПОВТОРНО (правило 14): второй раз сворачивает
        }
        if !self.is_nest(id) {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        }
        // ОДНА полоса за раз (Р.55, С5). Ограничение стоит константой, а не
        // рассыпано: на глубине четырёх раскрытий это четыре взаимозависимых
        // смещения на каждую лейн-команду и четыре уровня затемнения на два
        // десятка строк — нечитаемо. Снимается одной строкой плюс тестом.
        let had = !self.model.bands().is_empty();
        self.model.collapse_all_bands();
        if !self.model.expand_band(id) {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        }
        if had {
            self.note(tr(k::msg::NEST_ONE_BAND));
        }
        // экземпляр остаётся ВЫДЕЛЕННЫМ: следующий Enter обязан найти цель
        self.after_bands_changed(Some(id));
    }

    /// Свернуть раскрытие обратно. Ни шага истории, ни следа в файле:
    /// раскрытие — рабочее место, а не работа.
    pub fn collapse_in_place(&mut self, id: ClipId) {
        if self.model.collapse_band(id) {
            self.after_bands_changed(Some(id));
        }
    }

    pub fn collapse_all_in_place(&mut self) -> bool {
        if self.model.bands().is_empty() {
            return false;
        }
        self.model.collapse_all_bands();
        self.after_bands_changed(None);
        true
    }

    /// Общее на раскрытие и свёртку: проект НЕ изменился (в плеер и в файл
    /// уходит тот же документ), а вот вид, выделение и кэши строк — да.
    ///
    /// `keep` — экземпляр, выделение которого переживает смену строк. Он
    /// событие КОРНЯ, а правило «выделение живёт ровно в одной ленте»
    /// защищает от адресов ПОЛОСЫ; удерживать его законно и нужно: без
    /// этого второй Enter не находил цели и бил в пустоту (Р.56, п.451).
    fn after_bands_changed(&mut self, keep: Option<ClipId>) {
        // выделение живёт РОВНО В ОДНОЙ ленте: после смены набора строк
        // держать в нём чужие адреса значит обещать жест, которого не будет
        self.selected.retain(|id| Some(*id) == keep);
        self.selected_morph.clear();
        self.selected_camera.clear();
        self.model_rev += 1;
        self.dirty_ui = true;
    }

    /// Что делает Enter по выделенному вложению (Р.56, п.451).
    ///
    /// Решение владельца: быстрый жест даёт СЛЕДУЮЩИЙ режим, а не тумблер.
    /// Первый Enter раскрывает на месте (режим по умолчанию), второй по уже
    /// раскрытому — полный вход. Сворачивает по-прежнему повторный двойной
    /// клик и Esc: у клавиши одно направление, у мыши — другое, и оба
    /// названы в меню.
    pub fn enter_pressed_on_nest(&self, id: ClipId) -> EnterOnNest {
        if !self.is_nest(id) {
            return EnterOnNest::NotANest;
        }
        // решение ТОЛЬКО по полосе, контекст не при чём (Р.58, п.460):
        // внутри ленты каскад тот же — первый Enter раскрывает, второй входит
        if self.model.bands().iter().any(|b| b.inst == id) {
            EnterOnNest::Enter
        } else {
            EnterOnNest::Expand
        }
    }

    /// Войти в ленту события `id` (Р.50, В1).
    ///
    /// Курсор ПЕРЕВОДИТСЯ, а не сбрасывается: `t_вн = source_in + (t_нар −
    /// start)`. Человек вошёл, а не перепрыгнул — превью показывает тот же
    /// кадр, и это единственное, по чему видно, что он попал туда, куда
    /// целился.
    pub fn enter_nest(&mut self, id: ClipId) {
        let Some(c) = self.model.clip(id).cloned() else { return };
        let Some(seq) = self.model.project().sequence(c.media).map(|s| (s.id, s.name.clone()))
        else {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        };
        if self.nest_path.len() >= kt_model::MAX_NEST_DEPTH {
            self.warn(crate::tf!(k::msg::NEST_TOO_DEEP, kt_model::MAX_NEST_DEPTH));
            return;
        }
        let here = self.player.transport.playhead();
        // полоса покидаемого контекста запоминается ДО `set_editing`: тот
        // чистит полосы безусловно (инвариант модели), а вернуть её на
        // выходе — дело приложения (Р.58, п.461)
        let band = self.model.bands().first().map(|b| b.inst);
        self.nest_path.push(NestStep {
            parent: self.model.editing(),
            inst: id,
            name: seq.1,
            view_start: self.view_start,
            ticks_per_px: self.ticks_per_px,
            tl_scroll: self.tl_scroll,
            cursor: here,
            region: self.model.project().region,
            selected: std::mem::take(&mut self.selected),
            selected_morph: std::mem::take(&mut self.selected_morph),
            selected_camera: std::mem::take(&mut self.selected_camera),
            band,
        });
        self.model.set_editing(Some(seq.0));
        // вернулись в ленту, где раскрытие уже было, — оно на месте (Р.58)
        if let Some(inst) = self.reel_bands.get(&seq.0).copied() {
            self.model.expand_band(inst);
        }
        // регион живёт в ОСИ ВРЕМЕНИ ленты, а снаружи он был в оси корня:
        // унести его внутрь значило бы накрыть не то место
        self.model.set_region(None);
        // выделение снаружи внутри бессмысленно: там другие события
        // (сами множества уже забраны в снимок выше)
        self.selected.clear();
        // Курсор ПЕРЕВОДИТСЯ, а не сбрасывается — но только пока ему есть
        // куда переводиться. Стоял он ВНЕ тела экземпляра (человек вошёл
        // из меню, не подводя курсор) — переведённая позиция оказывается
        // за содержимым ленты, и «вошёл» означало бы «смотрю в пустоту».
        // Тогда курсор садится на ближайший край материала: это и есть
        // ближайший кадр, который лента показывает.
        // Конец содержимого берётся ПОСЛЕ обмена: `set_editing` уже
        // поменял корень и ленту местами, и `sequence(c.media)` теперь
        // отдаёт запись, которая держит содержимое КОРНЯ, а не ленты.
        let end = self
            .model
            .project()
            .clips
            .iter()
            .map(|x| x.start + x.duration)
            .max()
            .unwrap_or(c.duration);
        let inner = (c.source_in + (here - c.start)).clamp(Tick::ZERO, end.max(Tick::ZERO));
        self.on_context_changed(inner);
        // ВИД приводится к переведённому курсору. Без этого прокрутка
        // оставалась в оси КОРНЯ: войдя во вложение, стоящее на десятой
        // минуте, человек видел пустой экран, потому что содержимое ленты
        // лежит около её нуля. Выход вид не трогает — там он ВОЗВРАЩАЕТСЯ
        // из снимка целиком (t456).
        self.center_view_on(inner);
    }

    /// Выйти на уровень выше. `None` в пути — мы и так в корне.
    pub fn leave_nest(&mut self) {
        let Some(step) = self.nest_path.pop() else { return };
        let inner = self.player.transport.playhead();
        // раскрытие ЭТОЙ ленты запоминается до выхода: `set_editing` его
        // сотрёт, а вернуться сюда человек может (Р.58, п.461)
        if let Some(cur) = self.model.editing() {
            match self.model.bands().first().map(|b| b.inst) {
                Some(inst) => {
                    self.reel_bands.insert(cur, inst);
                }
                None => {
                    self.reel_bands.remove(&cur);
                }
            }
        }
        self.model.set_editing(step.parent);
        self.model.set_region(step.region);
        self.view_start = step.view_start;
        self.ticks_per_px = step.ticks_per_px;
        self.tl_scroll = step.tl_scroll;
        self.selected = step.selected;
        self.selected_morph = step.selected_morph;
        self.selected_camera = step.selected_camera;
        // курсор переводится обратно ТЕМ ЖЕ правилом; событие могло
        // уехать или исчезнуть под undo — тогда возвращаемся туда, где
        // стояли до входа
        let back = match self.model.clip(step.inst).cloned() {
            Some(c) => (c.start + (inner - c.source_in)).max(Tick::ZERO),
            None => step.cursor,
        };
        self.on_context_changed(back);
        // ПОЛОСА ВОЗВРАЩАЕТСЯ (Р.58, п.461; решение владельца — пересмотр
        // Р.56 п.5): раскрытие — рабочее место, и выход из ленты его не
        // отнимает, включая полосу, через которую входили вторым Enter.
        // Пересчитывается заново: содержимое могло измениться внутри.
        // Исчез экземпляр или перестал быть вложением — молча без полосы.
        // Выделение уже вернулось из снимка — `after_bands_changed`
        // удерживает ровно его, если оно одно (экземпляр).
        if let Some(inst) = step.band
            && self.model.expand_band(inst)
        {
            let keep = (self.selected.len() == 1).then(|| *self.selected.iter().next().unwrap());
            self.after_bands_changed(keep);
        }
    }

    /// Общее на вход и выход: кэши кадра пересобираются, курсор встаёт на
    /// переведённое место, а плеер получает то, что обязан играть ЗДЕСЬ
    /// (`player_project`: в корне — документ, внутри ленты — сама лента).
    fn on_context_changed(&mut self, cursor: Tick) {
        self.player.seek(cursor);
        self.blocked = None;
        // ревизия — то, на чём висят кэши кадра (coverage, имена, пути) и
        // сборка рендерера: смена контекста меняет ВСЁ, что они держат
        self.model_rev += 1;
        self.push_project_to_player();
        self.sync_media_sources();
    }

    /// Крошки пути: корень и каждая лента по дороге внутрь.
    pub fn nest_crumbs(&self) -> Vec<String> {
        self.nest_path.iter().map(|s| s.name.clone()).collect()
    }

    /// Окно имени ленты (эталон — окно имени пресета, t345).
    fn draw_seq_name(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.seq_name.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = true;
        let taken: Vec<String> = self
            .model
            .project()
            .sequences
            .iter()
            .filter(|s| s.id != d.seq)
            .map(|s| s.name.clone())
            .collect();
        egui::Window::new(tr(k::msg::NEST_RENAME_TITLE))
            .id(egui::Id::new("seq_name"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::msg::NEST_NAME_LABEL));
                    let resp = ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(200.0));
                    if d.focus {
                        resp.request_focus();
                        d.focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                });
                // сколько мест изменится вместе с этим — наш единственный
                // ответ на общую беду Premiere/FCP/VEGAS, и он бесплатен
                ui.label(
                    egui::RichText::new(crate::tf!(k::msg::NEST_USES, self.nest_uses(d.seq)))
                        .weak()
                        .small(),
                );
                ui.horizontal(|ui| {
                    if ui.button(tr(k::msg::NEST_RENAME_DO)).clicked() {
                        go = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if go && !d.name.trim().is_empty() {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("seq_name")));
            let name = kt_model::unique_name(&taken, d.name.trim(), "copy");
            self.apply(Command::RenameSequence { id: d.seq, name });
            return;
        }
        if cancel || !open {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("seq_name")));
            return;
        }
        self.seq_name = Some(d);
    }

    /// Событие-ВЛОЖЕНИЕ? (Р.50)
    /// Прямоугольник карточки медиа в бине (кадр назад) — там, где
    /// действует человек. Список этого кадра короткий (карточки бина),
    /// линейный поиск дешевле карты. Зовут его harness-тесты и живые
    /// сценарии: в продуктовом пути координата карточки не нужна.
    #[allow(dead_code)]
    pub fn bin_card_rect(&self, id: MediaId) -> Option<egui::Rect> {
        self.bin_card_rects.iter().find(|(m, _)| *m == id).map(|(_, r)| *r)
    }

    pub fn is_nest(&self, id: ClipId) -> bool {
        self.model.clip(id).is_some_and(|c| self.model.project().sequence(c.media).is_some())
    }

    /// Сколько мест дерева смотрит на эту ленту.
    pub fn nest_uses(&self, seq: kt_model::MediaId) -> usize {
        self.model.project().sequence_uses(seq)
    }

    /// Ответить на заданный вопрос. Вынесено из отрисовки, чтобы тест
    /// отвечал ровно тем же кодом, что и кнопка (правило 17).
    pub fn answer_ask(&mut self, ans: Answer) {
        let Some(a) = self.ask.take() else { return };
        match (a, ans) {
            (Ask::SaveOverLock { .. }, Answer::Yes) => {
                self.lock_override = true;
                self.save();
            }
            (Ask::SaveOverLock { .. }, _) => {}
            (Ask::Unsaved(_), Answer::Cancel) => {}
            (Ask::Unsaved(p), Answer::Yes) => {
                self.save();
                // сохранение могло не состояться (отменён Save-As, отказ
                // записи, вопрос о занятом проекте) — тогда уходить нельзя:
                // ровно этим «сохранить и выйти» и отличается от «выйти»
                if !self.dirty_content && !self.dirty_ui {
                    self.proceed(p);
                }
            }
            // БРОШЕННЫЙ ПРОЕКТ: три ответа, и все три РАЗНЫЕ по смыслу —
            // ровно то, ради чего у `Answer` их три, а не `bool`
            (Ask::DropOtio, Answer::Yes) => {
                // «Открыть как проект» — через ту же дверь, что пункт меню:
                // вопрос о несохранённом и замок на файле (Р.49).
                if self.guard_unsaved(Pending::DropOpen) {
                    self.open_dropped_now();
                }
            }
            (Ask::DropOtio, Answer::No) => self.insert_dropped_as_sequence(),
            (Ask::DropOtio, Answer::Cancel) => {
                self.pending_drop = None;
            }
            (Ask::Unsaved(p), Answer::No) => {
                // работа объявлена ненужной: гасим флаги ДО перехода, иначе
                // `Drop` воскресит её файлом восстановления
                self.dirty_content = false;
                self.dirty_ui = false;
                self.drop_own_recovery();
                self.proceed(p);
            }
        }
    }

    /// Сделать то, ради чего спрашивали.
    fn proceed(&mut self, p: Pending) {
        match p {
            Pending::New => *self = Self::new_empty(None),
            Pending::Open => self.open_project_dialog_now(),
            Pending::Quit => self.quit_confirmed = true,
            Pending::DropOpen => self.open_dropped_now(),
        }
    }

    /// Открыть брошенный проект — ТЕМ ЖЕ путём, что пункт меню «Открыть»
    /// (Р.55, С6).
    ///
    /// До этого раунда дроп делал `*self = app` в обход и вопроса о
    /// несохранённом, и замка на файле: работа исчезала без единого
    /// вопроса, а вторая копия программы на том же файле не обнаруживалась.
    fn open_dropped_now(&mut self) {
        let Some((path, _)) = self.pending_drop.take() else { return };
        match KtApp::open(&path) {
            Ok(mut app) => {
                app.claim_project(&path);
                *self = app;
            }
            Err(e) => self.warn(format!("{}: {e}", path.display())),
        }
    }

    /// Вставить брошенный проект КОМПОЗИЦИЕЙ в текущий монтаж (Р.55, С6).
    fn insert_dropped_as_sequence(&mut self) {
        let Some((path, target)) = self.pending_drop.take() else { return };
        let src = match load_project_file(&path) {
            Ok((p, _, notices)) => {
                for n in notices {
                    self.warn(n);
                }
                p
            }
            Err(e) => {
                self.warn(format!("{}: {e}", path.display()));
                return;
            }
        };
        let name = kt_model::unique_name(
            &self.model.project().sequences.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default().trim(),
            "copy",
        );
        let (at, lane) = match target {
            Some((at, (_, lane))) => (at, lane),
            None => (self.player.transport.playhead(), 0),
        };
        let doc = self.model.document();
        let mut n = kt_model::ops::NestCounters::of(&doc);
        let (cmds, rep) = kt_model::ops::import_as_sequences(
            self.model.project(),
            &src,
            &name,
            at,
            lane,
            n.ids(),
        );
        if cmds.is_empty() {
            self.warn(tr(k::msg::IMPORT_EMPTY));
            return;
        }
        if !self.apply_batch(cmds) {
            return;
        }
        self.note(crate::tf!(k::msg::IMPORT_DONE, name.clone(), rep.media_added));
        // Что НЕ приехало — обязано быть НАЗВАНО (правило 16): области,
        // метки и настройки чужого проекта принадлежат ДОКУМЕНТУ, а не
        // монтажу, и подменять ими свои нельзя.
        if rep.dropped_extras > 0 {
            self.warn(crate::tf!(k::msg::IMPORT_DROPPED, rep.dropped_extras));
        }
    }

    /// Спросить, если есть что терять. `true` — можно действовать сразу.
    ///
    /// До Р.49 программа не спрашивала НИЧЕГО, и это был законный выбор
    /// (вердикт F-RES-005): фоновый автосейв писал прямо в файл пользователя,
    /// то есть терять было нечего. Владелец это убрал (п.351) — вместе с
    /// правом не спрашивать.
    pub fn guard_unsaved(&mut self, p: Pending) -> bool {
        if !self.dirty_content && !self.dirty_ui {
            return true;
        }
        self.ask = Some(Ask::Unsaved(p));
        false
    }

    /// Занять проект (п.352). Вторая копия не запрещается — она честно
    /// деградирует: предупреждение с номером чужого процесса, своё
    /// восстановление, свой временный файл и вопрос при первом сохранении.
    pub(crate) fn claim_project(&mut self, path: &Path) {
        // старый замок отпускаем ДО взятия нового: иначе «Сохранить как…»
        // в тот же файл отказал бы сам себе
        self.project_lock = None;
        self.lock_holder = None;
        self.lock_override = false;
        match crate::project_lock::acquire(path) {
            crate::project_lock::LockState::Owned(l) => self.project_lock = Some(l),
            crate::project_lock::LockState::Busy { pid } => {
                self.lock_holder = Some(pid);
                match pid {
                    Some(n) => self.warn(crate::tf!(k::msg::PROJECT_LOCKED_BY, n)),
                    None => self.warn(tr(k::msg::PROJECT_LOCKED)),
                }
            }
            // замок взять не удалось совсем — молча ведём себя как владелец:
            // отказывать в работе из-за отсутствия каталога данных нельзя
            crate::project_lock::LockState::Unavailable => {}
        }
    }

    /// Адрес своего файла восстановления. У второй копии он с номером
    /// процесса: иначе две копии одного проекта писали бы в один файл и
    /// затирали несохранённую работу друг друга — ровно то, от чего вся
    /// эта волна.
    pub(crate) fn recovery_target(&self) -> PathBuf {
        let base = recovery_path_for(self.project_path.as_deref());
        match self.lock_holder {
            None => base,
            Some(_) => {
                let name =
                    base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                base.with_file_name(format!("{name}.{}.otio", std::process::id()))
            }
        }
    }

    /// Убрать файл восстановления, написанный ЭТОЙ копией.
    pub(crate) fn drop_own_recovery(&mut self) {
        if let Some(p) = self.recovery_written.take() {
            let _ = std::fs::remove_file(p);
        }
    }

    fn warn_if_dir_unsynced(&mut self, dir_synced: bool) {
        if !dir_synced && !self.dir_sync_warned {
            self.dir_sync_warned = true;
            self.warn(tr(k::msg::DIR_SYNC_UNSUPPORTED));
        }
    }

    fn push_notice(&mut self, severity: Severity, text: String) {
        let at = chrono_like_now();
        // Дубликаты подряд схлопываются в «текст ×N», но счётчик
        // непрочитанных растёт ВСЕГДА (Р.49, п.350): раньше ранний
        // возврат гасил и его, и вторая подряд «save failed» не красила
        // колокольчик — единственный канал сообщения об ошибке записи
        // умирал ровно тогда, когда ошибка повторялась.
        if let Some(last) = self.notices.back_mut()
            && last.text == text
        {
            last.repeats += 1;
            last.at = at;
            last.severity = severity;
            self.notices_unread += 1;
            return;
        }
        self.notices.push_back(Notice { at, severity, text, repeats: 1 });
        if self.notices.len() > 200 {
            self.notices.pop_front();
        }
        self.notices_unread += 1;
    }

    /// Проект, который ИГРАЕТ плеер — ЕДИНСТВЕННОЕ место, где решается,
    /// что именно слышно и видно (Р.55, С1).
    ///
    /// Стоя в корне, играет документ. Стоя ВНУТРИ ленты, играет САМА ЛЕНТА
    /// со своего нуля: до Р.55 сюда всегда уходил `document()`, и человек,
    /// войдя в композицию, видел на таймлайне её события, а слышал и видел
    /// КОРЕНЬ, взятый в момент, численно равный локальному времени ленты.
    /// Совпадало это ровно тогда, когда экземпляр стоит в нуле — поэтому
    /// ни один тест и ни один живой сценарий дефекта не видел.
    ///
    /// В ФАЙЛ и в ЭКСПОРТ по-прежнему уходит `document()`: там расхождение
    /// означало бы записать на диск не то, что человек монтировал (t456b).
    pub(crate) fn player_project(&self) -> kt_model::Project {
        match self.model.editing() {
            Some(_) => self.model.reel_document(),
            None => self.model.document(),
        }
    }

    /// Отдать плееру то, что он обязан играть. Своих `set_project` в
    /// приложении быть не должно: их было восемь, и первая же правка
    /// внутри ленты возвращала в плеер корень (сторож — t456c).
    pub(crate) fn push_project_to_player(&mut self) {
        let p = self.player_project();
        self.player.set_project(p);
    }

    pub fn apply(&mut self, cmd: Command) -> bool {
        self.model_rev += 1;
        // через `apply_batch`, а не `apply`: родство рождённых событий
        // приходит ОДНИМ путём (п.357), и одиночный разрез не исключение
        match self.model.apply_batch(vec![cmd]) {
            Ok(births) => {
                self.push_project_to_player();
                self.sync_media_sources();
                self.dirty_content = true;
                self.bump_clip_hw();
                self.inherit_extras(&births);
                true
            }
            Err(e) => {
                self.refuse_command(&e);
                false
            }
        }
    }

    // ---- выделение событий (Р.33 п.7) ---------------------------------------

    /// Чья это лента: `None` — корень, `Some` — раскрытая полоса (Р.55, С5).
    pub fn reel_of_clip(&self, id: ClipId) -> Option<kt_model::MediaId> {
        let c = self.model.clip(id)?;
        self.model.band_of_row(kt_model::lane_key(c).0, c.lane).map(|b| b.seq)
    }

    /// Выделить ровно это событие (обычный клик).
    pub fn select_only(&mut self, id: ClipId) {
        self.selected.clear();
        self.selected.insert(id);
    }

    /// Ctrl+клик: добавить/убрать из выделения.
    ///
    /// **Выделение живёт РОВНО В ОДНОЙ ленте** (Р.55, С5). Смешанное
    /// выделение выглядело бы обычным, а жест по нему поехал бы только
    /// наполовину: `selected_with_groups` собирает группы по одному
    /// пространству, а команды на события разных лент один батч не
    /// переживут. Поэтому клик в другой полосе ЗАМЕНЯЕТ выделение и
    /// говорит об этом — молчаливое выбрасывание половины было бы хуже.
    pub fn toggle_selected(&mut self, id: ClipId) {
        if self.selected.remove(&id) {
            return;
        }
        let here = self.reel_of_clip(id);
        if self.selected.iter().any(|o| self.reel_of_clip(*o) != here) {
            self.selected.clear();
            self.note(tr(k::msg::NEST_SELECTION_MOVED));
        }
        self.selected.insert(id);
    }

    pub fn is_selected(&self, id: ClipId) -> bool {
        self.selected.contains(&id)
    }

    /// Единственное выделенное событие — для операций, у которых на
    /// множестве нет определённого смысла (магнит A/V, разрыв связи,
    /// перенос на соседнюю дорожку, трим к курсору).
    pub fn selected_one(&self) -> Option<ClipId> {
        (self.selected.len() == 1).then(|| self.selected.iter().copied().next()).flatten()
    }

    /// То же, но молчание заменяется внятным сообщением: «выбери событие»
    /// (п.16 чек-листа — невозможность действия видна, а не проглатывается).
    fn require_one_selected(&mut self) -> Option<ClipId> {
        match self.selected_one() {
            Some(id) => Some(id),
            None => {
                self.note(tr(k::msg::PICK_CLIP));
                None
            }
        }
    }

    /// Выделенные события вместе с их link-группами (перенос, удаление,
    /// копирование, цели морфера — всё работает по этому набору).
    pub fn selected_with_groups(&self) -> Vec<ClipId> {
        let p = self.model.project();
        let groups: Vec<GroupId> = p
            .clips
            .iter()
            .filter(|c| self.selected.contains(&c.id))
            .filter_map(|c| c.link_group)
            .collect();
        let mut ids: Vec<ClipId> = p
            .clips
            .iter()
            .filter(|c| {
                self.selected.contains(&c.id) || c.link_group.is_some_and(|g| groups.contains(&g))
            })
            .map(|c| c.id)
            .collect();
        ids.sort_by_key(|i| i.0);
        ids.dedup();
        ids
    }

    /// Живая правка ВНУТРИ жеста: команды с одним `gesture` схлопываются в
    /// один undo-шаг (драг края перехода, сессия окна FX/морфера).
    ///
    /// Все такие правки обязаны идти через эту обёртку. Пока каждая звала
    /// `model.apply_coalesced` сама, `model_rev` не рос — и фоновый автосейв,
    /// гейт которого «`model_rev` == снятая ревизия», гасил звёздочку, хотя
    /// правки сделаны ПОСЛЕ снимка: изменения выглядели сохранёнными и
    /// терялись при выходе (п.200).
    ///
    /// Ошибку не показываем: путь живой, команда шлётся каждый кадр драга, и
    /// плашка на каждый кадр была бы шумом — жест просто не двигается дальше.
    pub fn apply_coalesced(&mut self, cmd: Command, gesture: u64) -> bool {
        // Закон П (Р.53, п.412): команда, не трогающая план, не имеет
        // права клонировать проект и пересобирать графы. Правка
        // параметров генератора — это `SetMediaPath`, её адресат воркер
        // этого медиа, а не рендереры (иначе каждое нажатие клавиши в
        // поле титра останавливало бы аудио-render-поток).
        let touches_plan = cmd.touches_render_plan();
        if self.model.apply_coalesced(cmd, gesture).is_err() {
            return false;
        }
        self.model_rev += 1;
        if touches_plan {
            self.push_project_to_player();
        }
        self.sync_media_sources();
        self.dirty_content = true;
        true
    }

    /// Пакет команд + возврат «ручек» кроссфейдов, которые этот жест убивает
    /// (Р.26): растащил клипы или удалил соседа — срезанный при наезде
    /// материал возвращается тем же undo-шагом. Все новые пути удаления и
    /// перемещения клипов обязаны ходить через эту обёртку, а не через
    /// apply_batch, иначе материал снова начнёт молча теряться.
    pub fn apply_batch_restoring(&mut self, mut cmds: Vec<Command>) -> bool {
        if let Ok(after) = self.model.preview(&cmds) {
            cmds.extend(kt_model::ops::restore_handles(self.model.project(), &after));
        }
        self.apply_batch(cmds)
    }

    /// Лента стоит в НЕСКОЛЬКИХ местах, а человек правит её в раскрытии —
    /// сказать об этом один раз (Р.55, С5, решение владельца).
    ///
    /// Правка правит ВСЕ места: это ссылка, ради этого её и заводят. Но
    /// выглядит она как обычный драг на таймлайне, и промолчать значит
    /// дать человеку изменить три места, думая, что он изменил одно
    /// (правило 16). Дальше молчим: предупреждение на каждую правку
    /// превратилось бы в шум, который перестают читать.
    fn warn_shared_band(&mut self, cmds: &[Command]) {
        let touched: Vec<kt_model::MediaId> = cmds
            .iter()
            .filter_map(|c| c.target_clip())
            .filter_map(|id| self.reel_of_clip(id))
            .collect();
        for seq in touched {
            if self.shared_warned.contains(&seq) {
                continue;
            }
            let uses = self.model.document().sequence_uses(seq);
            if uses > 1 {
                self.shared_warned.insert(seq);
                self.warn(crate::tf!(k::msg::NEST_SHARED_EDIT, uses));
            }
        }
    }

    /// Пакет команд = один undo-снапшот на жест (атомарно).
    pub fn apply_batch(&mut self, cmds: Vec<Command>) -> bool {
        self.model_rev += 1;
        self.warn_shared_band(&cmds);
        match self.model.apply_batch(cmds) {
            Ok(births) => {
                self.push_project_to_player();
                self.sync_media_sources();
                self.dirty_content = true;
                self.bump_clip_hw();
                self.inherit_extras(&births);
                true
            }
            Err(e) => {
                self.refuse_command(&e);
                false
            }
        }
    }

    // ---- операции паритета (регион/маркеры/клипборд/дорожки) ----------------

    pub fn marker_toggle_at_cursor(&mut self) {
        let at = self.player.transport.playhead();
        self.marker_here(at);
    }

    /// Регион по span link-группы события — ОДНО тело для Shift+двойного
    /// клика и пункта меню (п.17 чек-листа: парные способы ведут себя
    /// одинаково).
    pub fn region_from_event(&mut self, id: ClipId) {
        let span = {
            let p = self.model.project();
            let group = p.clips.iter().find(|c| c.id == id).and_then(|c| c.link_group);
            let members: Vec<&ClipEvent> = p
                .clips
                .iter()
                .filter(|c| c.id == id || (group.is_some() && c.link_group == group))
                .collect();
            members
                .iter()
                .map(|c| c.start)
                .min()
                .zip(members.iter().map(|c| c.start + c.duration).max())
        };
        if let Some((start, end)) = span {
            self.model.set_region(Region::normalized(start, end));
            self.on_region_changed();
        }
    }

    /// Вернувшийся на диск исходник заменяет свою заглушку (Р.33 п.10).
    /// Watchdog уже следит за существованием файлов, поэтому здесь только
    /// реакция: файл больше не offline, а на его месте стоит плейсхолдер —
    /// открываем ЗАНОВО в фоне и подменяем источник у плеера. Позиция,
    /// транспорт и кэш прочих источников не трогаются.
    /// Источники плеера догоняют модель (Р.52): дифф путей
    /// `document().media` против снимка последнего синка. Смена пути
    /// (окно генератора, undo/redo) = пересоздание источника фабрикой
    /// (`replace_media` гасит воркер и чистит его кэш); исчезновение
    /// медиа = `remove_media` (заодно закрыт старый долг: «Убрать из
    /// проекта» оставлял воркер жить); появление БЕЗ ручной регистрации
    /// (redo добавления) = поднятие источника. Зовётся из всех обёрток
    /// правок и из undo/redo — иначе undo смены цвета не доехал бы до
    /// картинки: источник живёт вне project→player синка.
    /// Подменённый шрифт титра — ВИДИМОЕ уведомление (Р.52 С4).
    ///
    /// Р.59, стадия 2: семья живёт на СОБЫТИИ, а не в пути медиа, —
    /// поэтому спрашивают отсюда с готовым видом, а не с путём.
    fn note_missing_title_font(&mut self, kind: &kt_model::GeneratorKind) {
        let kt_model::GeneratorKind::Text { family, .. } = kind else { return };
        // о каждой семье говорим ОДИН раз за сессию (п.413): титр
        // меняется на каждый символ, и без дедупа это шторм
        if self.font_warned.contains(family) {
            return;
        }
        if let Some(warn) = crate::gen_ui::font_warning(kind) {
            self.font_warned.insert(family.clone());
            self.note(warn);
        }
    }

    /// ЗНАЧЕНИЯ генераторов доезжают до живых источников (Р.59, стадия 2).
    ///
    /// Структурные правки (событие родилось, умерло, сменило вид) едут
    /// сами: их команды трогают план, значит `set_project` уже позвал
    /// `Player::sync_generators`. Здесь остаётся ровно то, что план НЕ
    /// трогает (закон П) — значения и кривые: их адресат воркер этого
    /// события, а не рендереры. Иначе каждое нажатие клавиши в титре
    /// останавливало бы аудио-render-поток, и это ровно та претензия
    /// владельца, ради которой в Р.53 заводилась полоса `retune`.
    fn sync_gen_sources(&mut self) {
        let mut cur: HashMap<ClipId, kt_model::GenState> = HashMap::new();
        {
            let doc = self.model.document();
            for c in doc.clips.iter().chain(doc.sequences.iter().flat_map(|s| s.clips.iter())) {
                if let Some(k) = &c.r#gen {
                    cur.insert(
                        c.id,
                        kt_model::GenState { kind: k.clone(), auto: c.gen_auto.clone() },
                    );
                }
            }
        }
        if cur == self.gen_states {
            return;
        }
        // паспорт источника мог смениться (SMPTE со включённым тоном —
        // это уже ЗВУЧАЩЕЕ медиа, другой набор клипов): тогда
        // перенастройка честно отказывается, и источник пересоздаётся
        let mut respawn = false;
        let mut fonts: Vec<kt_model::GeneratorKind> = Vec::new();
        for (id, st) in &cur {
            let old = self.gen_states.get(id);
            if old == Some(st) {
                continue;
            }
            fonts.push(st.kind.clone());
            let addr = kt_engine::gen_source_addr(*id);
            // Тумбы события: у сервиса тумб СВОЙ источник, и ему нужен
            // кадр, а не адрес, — поэтому путь полный, со значениями.
            let path = kt_model::format_gen_path(&st.kind);
            match old {
                Some(prev) => {
                    if !self.player.update_gen(*id, st.kind.clone(), st.auto.clone()) {
                        respawn = true;
                    }
                    let prev_path = kt_model::format_gen_path(&prev.kind);
                    if kt_media::factory::can_retune(&prev_path, &path) {
                        self.thumbs.retune_media(addr, path);
                    } else {
                        self.thumbs.add_media(addr, path);
                        self.thumbs.invalidate_media(addr);
                    }
                }
                None => self.thumbs.add_media(addr, path),
            }
            // ячейки пикера помнят ТОТ ЖЕ ThumbKey, а он от значений
            // генератора не зависит (дефект-близнец п.412)
            self.fx_thumbs.invalidate_media(addr);
        }
        if respawn {
            let p = self.player_project();
            self.player.sync_generators(&p);
        }
        for k in fonts {
            self.note_missing_title_font(&k);
        }
        self.gen_states = cur;
    }

    /// Драг титра ПРЯМО В ПРЕВЬЮ (Р.52 С6): рамка блока текста из той же
    /// раскладки, что рисует кадр; перетаскивание пишет pos_x/pos_y ТОЙ ЖЕ
    /// командой и тем же gesture, что окно, — оба способа ведут себя
    /// одинаково по построению (правило 17). Хват-зона шире рамки
    /// (правило 1); клик-play вне рамки жив.
    fn title_drag_in_preview(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        img: egui::Rect,
    ) {
        let Some((clip, gesture)) = self.edited_title() else { return };
        let Some(mut kind) = self.model.clip(clip).and_then(|c| c.r#gen.clone()) else { return };
        let before = kind.clone();
        let (iw, ih) = (img.width().max(1.0) as u32, img.height().max(1.0) as u32);
        let Some(info) = kt_media::generator::title_layout(&kind, iw, ih) else { return };
        let b = info.block;
        let frame = egui::Rect::from_min_size(
            img.min + egui::vec2(b.x as f32, b.y as f32),
            egui::vec2(b.w.max(8.0) as f32, b.h.max(8.0) as f32),
        )
        .intersect(img);
        painter.rect(
            frame,
            0.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(120, 190, 255, 200)),
            egui::StrokeKind::Outside,
        );
        let resp =
            ui.interact(frame.expand(8.0), ui.id().with(("title-drag", clip)), egui::Sense::drag());
        if resp.dragged() {
            let d = resp.drag_delta();
            if d != egui::Vec2::ZERO
                && let kt_model::GeneratorKind::Text { pos_x, pos_y, .. } = &mut kind
            {
                *pos_x = (*pos_x + d.x as f64 / img.width().max(1.0) as f64).clamp(0.0, 1.0);
                *pos_y = (*pos_y + d.y as f64 / img.height().max(1.0) as f64).clamp(0.0, 1.0);
                // ТА ЖЕ команда и тот же gesture, что у формы в окне
                // (правило 17 по построению); значения живут на СОБЫТИИ
                if kind != before {
                    self.apply_coalesced(
                        kt_model::Command::SetClipGen { id: clip, kind: Some(kind) },
                        gesture,
                    );
                }
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    /// Событие, чей ТИТР сейчас правится (Р.59, стадия 2): открытое окно
    /// эффектов, в котором выбрана ГОЛОВА полосы, а источник события —
    /// текст. Своего окна у титра больше нет, поэтому и рамка драга, и
    /// safe-зоны спрашивают одно и то же место.
    pub(crate) fn edited_title(&self) -> Option<(ClipId, u64)> {
        for ed in self.fx_editors.iter().rev().filter(|e| e.open) {
            for id in &ed.members {
                if ed.selected.get(id) != Some(&crate::widgets::ChainSel::Head) {
                    continue;
                }
                let is_text = self
                    .model
                    .clip(*id)
                    .and_then(|c| c.r#gen.as_ref())
                    .is_some_and(|g| matches!(g, kt_model::GeneratorKind::Text { .. }));
                if is_text {
                    return Some((*id, ed.gesture));
                }
            }
        }
        None
    }

    /// Safe-зоны рисуются, пока правится титр (Р.52 С3).
    pub(crate) fn show_safe_zones(&self) -> bool {
        self.edited_title().is_some()
    }

    pub(crate) fn sync_media_sources(&mut self) {
        // Дешёвая сверка ДО сборки карты (Р.53, п.413): синк зовётся из
        // каждой обёртки правок, то есть на КАЖДОМ кадре любого
        // coalesced-жеста (драг края перехода, уровень, пипетка). Пока
        // ревизия модели не сдвинулась, состав медиа не мог измениться —
        // а собирать HashMap с клоном каждого пути на кадр запрещено
        // инвариантом 12 (никаких аллокаций в путях кадра).
        if self.media_synced && self.media_synced_rev == self.model_rev {
            return;
        }
        self.media_synced_rev = self.model_rev;
        self.media_sync_builds += 1;
        let cur: HashMap<MediaId, String> =
            self.model.document().media.iter().map(|m| (m.id, m.path.clone())).collect();
        if !self.media_synced {
            // первый вызов: источники уже поднял build_player
            self.media_paths = cur;
            self.media_synced = true;
            self.sync_gen_sources();
            return;
        }
        if self.media_paths == cur {
            // состав медиа тот же, а ЗНАЧЕНИЯ генераторов могли
            // измениться: они живут на событиях (Р.59, стадия 2)
            self.sync_gen_sources();
            return;
        }
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let db = self.index_db_path();
        for (id, path) in &cur {
            let known = self.media_paths.get(id).cloned();
            if known.as_deref() == Some(path.as_str()) {
                continue;
            }
            let resolved = resolve_media_path(base.as_deref(), path);
            // ПЕРЕНАСТРОЙКА НА МЕСТЕ (Р.53, п.412): у генератора смена
            // параметров — не подмена файла. Поток декодера остаётся
            // жить, рендереры целы; кадры старых параметров выбрасывает
            // сам воркер. Паспорт источника при этом меняться не может —
            // за этим следит предикат фабрики (t499).
            if let Some(old) = known.as_deref()
                && kt_media::factory::can_retune(old, path)
                && self.player.retune_media(*id, path)
            {
                self.player.set_media_path(*id, path);
                self.thumbs.retune_media(*id, resolved);
                // ячейки пикера помнят ТОТ ЖЕ ThumbKey, а он от параметров
                // генератора не зависит: без этой строки полоска показывала
                // бы эффект на кадре СТАРЫХ параметров — дефект-близнец
                // п.412, только в пикере
                self.fx_thumbs.invalidate_media(*id);
                continue;
            }
            let label = crate::util::media_label(path);
            let (src, err) =
                crate::util::open_source_or_placeholder(&resolved, db.as_deref(), &label);
            let opened = err.is_none();
            if let Some(e) = err {
                self.note(e);
            }
            // ДЛИНА МАТЕРИАЛА — от того, кто источник открыл (Р.63 п.2).
            // Модель её не добывает (закон П), а без неё у правого трима
            // нет упора: ровно так событие уезжает за конец файла и
            // «звучит тишиной». Заглушка своей длины не знает — её
            // паспорт пуст, и предела не появится.
            if opened {
                let d = src.info().duration;
                if d.0 > 0 {
                    self.model.note_media_length(*id, d);
                }
            }
            if known.is_some() {
                self.player.replace_media(*id, src);
            } else {
                self.player.add_media(*id, src);
            }
            self.thumbs.add_media(*id, resolved);
            self.thumbs.invalidate_media(*id);
        }
        let gone: Vec<MediaId> =
            self.media_paths.keys().filter(|id| !cur.contains_key(id)).copied().collect();
        for id in gone {
            self.player.remove_media(id);
        }
        self.media_paths = cur;
        self.sync_gen_sources();
    }

    /// «Создать генератор» из меню бина (Р.52): медиа + источник +
    /// карточка + сразу окно параметров. Клип НЕ ставится — карточку
    /// тянут на таймлайн или жмут «Поставить на курсор», как у файла.
    /// Новый генератор: медиа И КЛИП под курсором — одним батчем, одним
    /// шагом истории (Р.53, п.416).
    ///
    /// До этого пункта пункт меню заводил только медиа в бине: окно
    /// параметров открывалось, а в превью ничего не было — набирать титр
    /// приходилось вслепую, потом идти в бин и ставить карточку руками.
    /// Ctrl+Z возвращает всё разом, потому что батч один.
    pub fn create_generator(&mut self, kind: kt_model::GeneratorKind) {
        // Р.59, стадия 2: в пути медиа — ВИД, значения едут на СОБЫТИИ.
        let path = kt_model::gen_kind_path(&kind);
        let id = MediaId(self.next_media_id());
        // источник поднимаем сами (снимок синка — ДО apply); он же отдаёт
        // паспорт, по которому считается набор клипов
        let src = match crate::util::open_source(&path) {
            Ok(s) => s,
            Err(e) => {
                self.note(e);
                return;
            }
        };
        let info = src.info().clone();
        self.media_paths.insert(id, path.clone());
        let at = self.player.transport.playhead();
        let mut cmds = vec![Command::AddMedia { id, path: path.clone() }];
        cmds.extend(self.clip_cmds_for(id, &info, at, None));
        // выбранные в меню значения — на события этого же батча: одним
        // шагом истории, как и всё остальное создание (Р.53, п.416)
        for c in cmds.iter_mut() {
            if let Command::AddClip(ev) = c {
                ev.r#gen = Some(kind.clone());
            }
        }
        if !self.apply_batch(cmds) {
            self.media_paths.remove(&id);
            return;
        }
        self.player.add_media(id, src);
        self.thumbs.add_media(id, path);
        self.open_generator_editor(id);
    }

    pub(crate) fn poll_returned_sources(&mut self) {
        if let Some(rx) = self.reopen_rx.take() {
            let mut ready = Vec::new();
            while let Ok(msg) = rx.try_recv() {
                ready.push(msg);
            }
            if ready.is_empty() {
                self.reopen_rx = Some(rx); // ещё открывается — ждём
            }
            for (id, label, src) in ready {
                // не открылся снова — заглушка остаётся, шуметь не о чем
                if let Some(s) = src {
                    self.player.replace_media(id, s);
                    self.placeholder_media.remove(&id);
                    self.note(crate::tf!(k::msg::MEDIA_ONLINE, label));
                }
            }
        }
        if self.placeholder_media.is_empty() || self.reopen_rx.is_some() {
            return;
        }
        let offline = self.offline.lock().unwrap().clone();
        let base = self.project_path.as_deref().and_then(|p| p.parent());
        let db = self.project_path.as_deref().map(|p| p.with_extension("ktindex.db"));
        let back: Vec<(MediaId, String, String)> = self
            .model
            .project()
            .media
            .iter()
            .filter(|m| self.placeholder_media.contains(&m.id) && !offline.contains(&m.id))
            .map(|m| (m.id, crate::util::media_label(&m.path), resolve_media_path(base, &m.path)))
            .collect();
        if back.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.reopen_rx = Some(rx);
        std::thread::Builder::new()
            .name("reopen-media".into())
            .spawn(move || {
                for (id, label, path) in back {
                    let src = open_source_indexed(&path, db.as_deref()).ok();
                    let _ = tx.send((id, label, src));
                }
            })
            .ok();
    }

    /// Регион по метке-диапазону + выделение её событий (Р.33 п.8): ОДНО
    /// тело для двойного клика по ленте и любого будущего пункта меню.
    pub fn region_from_marked_range(&mut self, marker_start: i64) {
        let Some(m) =
            self.model.project().markers.iter().find(|m| m.t.0 == marker_start && m.is_range())
        else {
            return;
        };
        let Some(r) = Region::normalized(m.t, m.end()) else { return };
        let ids: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| r.intersects_clip(c))
            .map(|c| c.id)
            .collect();
        self.model.set_region(Some(r));
        self.on_region_changed();
        self.selected.clear();
        self.selected.extend(ids);
        if !self.selected.is_empty() {
            self.selected_morph.clear();
            self.selected_camera.clear();
        }
    }

    /// Метка в точке `at` — ОДНО тело для жеста M и пункта меню «Marker
    /// here» (п.11 пользователя: «marker here при выделенном регионе должен
    /// порождать region marker… очевидно что есть парные способы что-то
    /// сделать»). Парные способы обязаны учитывать контекст одинаково —
    /// п.17 чек-листа apps/kt-app/CLAUDE.md.
    pub fn marker_here(&mut self, at: Tick) {
        // Р.24 (жест пользователя, дословно): «при выделенном регионе и
        // курсоре в нем - m дает marked_range, при курсоре вне региона -
        // одинарная метка по курсору». Повторное M на том же регионе — toggle.
        // Грани региона считаются «внутри»: правый клик по кромке — это
        // клик ПО региону, а не мимо него.
        if let Some(r) = self.model.project().region
            && (r.contains(at) || at == r.start || at == r.end)
        {
            let dur = r.end - r.start;
            let at_start = self.model.project().markers.iter().find(|m| m.t == r.start).cloned();
            match at_start {
                Some(m) if m.duration == dur => {
                    self.apply(Command::RemoveMarker { at: r.start });
                }
                Some(_) => {
                    // точечная на старте региона — растягиваем в диапазон
                    self.apply(Command::SetMarkerDuration { at: r.start, duration: dur });
                }
                None => {
                    if self.apply(Command::AddMarker { at: r.start, duration: dur }) {
                        self.marker_edit =
                            Some(MarkerEdit { at: r.start.0, buf: String::new(), is_new: true });
                        self.marker_edit_focus = true;
                    }
                }
            }
            return;
        }
        // магнит как в старом: удалить ближайший в пределах порога, иначе добавить
        let threshold = ((crate::timeline::SNAP_PX * self.ticks_per_px) as i64).max(900);
        let near = self
            .model
            .project()
            .markers
            .iter()
            .map(|m| m.t)
            .min_by_key(|t| (t.0 - at.0).abs())
            .filter(|t| (t.0 - at.0).abs() <= threshold);
        match near {
            Some(t) => {
                self.apply(Command::RemoveMarker { at: t });
            }
            None => {
                // новая метка: сразу открываем поле имени (п.1 р.10);
                // клик мимо/Esc оставит метку без имени
                if self.apply(Command::AddMarker { at, duration: Tick::ZERO }) {
                    self.marker_edit =
                        Some(MarkerEdit { at: at.0, buf: String::new(), is_new: true });
                    self.marker_edit_focus = true;
                }
            }
        };
    }

    /// Регион in/out от курсора; без региона — затравка [cursor, cursor+1с].
    pub fn set_region_edge_from_cursor(&mut self, set_start: bool) {
        let at = self.player.transport.playhead();
        let cur =
            self.model.project().region.unwrap_or(Region { start: at, end: Tick(at.0 + 90_000) });
        let r = if set_start {
            Region::normalized(at, cur.end.max(Tick(at.0 + 90)))
        } else {
            Region::normalized(cur.start.min(at), at)
        };
        self.model.set_region(r);
        self.on_region_changed();
    }

    /// Пора ли автосохраняться: раз в 30 с, не во время жеста и не при
    /// игре — но если тихого момента нет дольше двух интервалов, пишем
    /// всё равно (потерять правки хуже, чем разово моргнуть, п.4 р.11).
    pub fn autosave_due(&self) -> bool {
        let idle = self.last_autosave.elapsed().as_secs();
        if idle < self.autosave_secs {
            return false;
        }
        let busy =
            self.drag.is_some() || self.bin_drag.is_some() || self.player.transport.is_playing();
        !busy || idle >= self.autosave_secs * 2
    }

    pub fn clear_region(&mut self) {
        self.model.set_region(None);
        // п.16 раунда 6: без региона включённый loop — «лампочка горит,
        // не зацикливает» (и переживал рестарт) — гасим вместе
        self.model.set_loop_enabled(false);
        self.on_region_changed();
    }

    /// Esc: снять регион, НЕ трогая loop (п.3 р.11). Loop — отдельное
    /// намерение пользователя; выключала его только явная команда «Снять
    /// регион», а Esc заодно гасил и его.
    pub fn clear_region_keep_loop(&mut self) {
        self.model.set_region(None);
        self.on_region_changed();
    }

    pub fn toggle_loop(&mut self) {
        let on = !self.model.project().loop_enabled;
        self.model.set_loop_enabled(on);
        self.on_region_changed();
    }

    /// Синхронизация региона/loop с плеером.
    ///
    /// `set_project` здесь НЕ зовём (Р.43, п.296): регион и `loop_enabled`
    /// живут в «удобстве», движок их из проекта не читает вовсе — луп
    /// доезжает отдельным `set_loop_region`. А `set_project` — это клон
    /// всего проекта плюс пересборка обоих рендереров: на монтаже
    /// пользователя (1961 событие) каждый сдвиг границы региона и каждое
    /// нажатие Loop останавливали аудио-render-поток, кольцо пустело, и
    /// начинались callback-underrun'ы — «регион какой-то поломанный при
    /// воспроизведении». Правки СОДЕРЖИМОГО ходят через `apply`/
    /// `apply_batch`, и проект плееру доносят они.
    pub fn on_region_changed(&mut self) {
        let p = self.model.project();
        let region =
            p.region.filter(|_| p.loop_enabled).map(|r| kt_time::TickRange::new(r.start, r.end));
        self.player.set_loop_region(region);
        self.dirty_ui = true;
    }

    pub fn copy_selection(&mut self) {
        let mut block = match self.model.project().region {
            Some(r) => kt_model::ops::copy_region_block(self.model.project(), r),
            // без региона — всё выделенное (каждое со своей link-группой)
            None => kt_model::ops::copy_clips(self.model.project(), &self.selected_with_groups()),
        };
        // КЛИПБОРД ЖИВЁТ В КООРДИНАТАХ ЛЕНТЫ-ВЛАДЕЛЬЦА (Р.56, п.450).
        // Копируем мы из РАБОЧЕГО вида, а в нём полоса раскрытия лежит со
        // смещением `base[вид]`, и смещения у видео, аудио и морф-строк
        // РАЗНЫЕ. Повтори вставка эти номера как есть — видео встало бы у
        // потолка, аудио внизу, а морфер в самом низу, где под него
        // завелась бы новая строка. Ровно на это и жаловался владелец.
        // Лента блока решается по ПЕРВОМУ событию, а не «нашлась хоть
        // одна полоса»: регион, задевший и корень, и полосу, иначе
        // локализовался бы в ленту и выбросил бы корневое большинство.
        let band = block
            .clips
            .first()
            .and_then(|c| self.model.band_of_row(kt_model::lane_key(c).0, c.lane))
            .map(|b| (b.base, b.len));
        let root = kt_model::LaneBases::default();
        let (base, len) = band.unwrap_or((
            root,
            // корень: «своё» — всё, что НИЖЕ начала любой полосы
            kt_model::LaneBases::of_root(self.model.project()),
        ));
        let dropped = block.localize(base, len);
        if dropped > 0 {
            self.note(crate::tf!(k::msg::COPY_ONE_REEL, dropped));
        }
        if block.clips.is_empty() {
            self.note(tr(k::msg::NOTHING_TO_COPY));
        } else {
            self.note(crate::tf!(k::msg::CLIPS_COPIED, block.clips.len()));
            self.clipboard = block;
        }
    }

    /// Окно префикса вставки (Р.35 п.4). Неблокирующее: позиция вставки
    /// заморожена, Enter вставляет, Esc отменяет (он же в Esc-каскаде),
    /// при закрытии фокус ОТДАЁТСЯ — иначе горячие клавиши умрут до
    /// клика мимо (правило 14, дефект п.8 р.12).
    fn draw_paste_names(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.paste_names.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new(tr(k::msg::PASTE_PREFIX_TITLE))
            .id(egui::Id::new("paste_names"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::msg::PASTE_PREFIX_LABEL));
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut d.affix)
                            .desired_width(180.0)
                            .hint_text(tr(k::msg::PASTE_AFFIX_DEFAULT)),
                    );
                    if d.focus {
                        resp.request_focus();
                        d.focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                    // сторона — переключателем рядом с полем, а не вторым
                    // окном: это тот же выбор, а не отдельная настройка
                    if ui.selectable_label(d.before, tr(k::msg::PASTE_AFFIX_BEFORE)).clicked() {
                        d.before = !d.before;
                    }
                });
                // живой предпросмотр: пользователь видит имя ДО вставки
                let sample = self
                    .clipboard
                    .areas
                    .first()
                    .map(|a| a.name.clone())
                    .or_else(|| {
                        self.clipboard
                            .markers
                            .iter()
                            .find(|m| !m.name.is_empty())
                            .map(|m| m.name.clone())
                    })
                    .unwrap_or_default();
                if !sample.is_empty() {
                    let a = d.affix.trim();
                    let shown = if a.is_empty() {
                        sample.clone()
                    } else if d.before {
                        format!("{a} {sample}")
                    } else {
                        format!("{sample} {a}")
                    };
                    ui.label(
                        egui::RichText::new(kt_model::unique_name(
                            &self.model.project().taken_names(),
                            &shown,
                            "copy",
                        ))
                        .strong(),
                    );
                }
                ui.label(egui::RichText::new(tr(k::msg::PASTE_PREFIX_NOTE)).weak().small());
                ui.horizontal(|ui| {
                    if ui.button(tr(k::msg::PASTE_DO)).clicked() {
                        go = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if go {
            let (at, row, affix, before) = (d.at, d.row, d.affix.trim().to_string(), d.before);
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("paste_names")));
            self.paste_here(at, row, affix, before);
            return;
        }
        if cancel || !open {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("paste_names")));
            return;
        }
        self.paste_names = Some(d);
    }

    /// Окно имени пресета (эталон — окно префикса вставки, t250):
    /// живой предпросмотр итогового имени, Enter/Esc, фокус отдаётся при
    /// закрытии. Модель НЕ трогается — это запись в файл пользователя.
    fn draw_preset_save(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.preset_save.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = true;
        let taken = self.presets.taken_names();
        egui::Window::new(tr(k::msg::PRESET_SAVE_TITLE))
            .id(egui::Id::new("preset_save"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::msg::PRESET_NAME_LABEL));
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut d.name)
                            .desired_width(200.0)
                            .hint_text(tr(k::msg::PRESET_NAME_HINT)),
                    );
                    if d.focus {
                        resp.request_focus();
                        d.focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                });
                // живой предпросмотр: пользователь видит ИТОГОВОЕ имя до
                // сохранения (занятое получит суффикс)
                let wanted = d.name.trim();
                if !wanted.is_empty() {
                    let taken_now: Vec<String> = taken
                        .iter()
                        .filter(|n| Some(n.as_str()) != d.rename_of.as_deref())
                        .cloned()
                        .collect();
                    let shown = kt_model::unique_name(&taken_now, wanted, "copy");
                    ui.label(egui::RichText::new(shown).strong());
                }
                ui.label(egui::RichText::new(tr(k::msg::PRESET_SAVE_NOTE)).weak().small());
                ui.horizontal(|ui| {
                    if ui.button(tr(k::msg::PRESET_SAVE_DO)).clicked() {
                        go = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if go && !d.name.trim().is_empty() {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("preset_save")));
            let taken_now: Vec<String> = taken
                .iter()
                .filter(|n| Some(n.as_str()) != d.rename_of.as_deref())
                .cloned()
                .collect();
            let name = kt_model::unique_name(&taken_now, d.name.trim(), "copy");
            let res = match &d.rename_of {
                Some(old) => self.presets.rename(old, &name),
                None => self.presets.add(&name, &d.block),
            };
            match res {
                Ok(()) => self.note(format!("{} {name}", tr(k::msg::PRESET_SAVED))),
                Err(e) => self.note(format!("{}: {e}", tr(k::msg::PRESET_SAVE_FAILED))),
            }
            return;
        }
        if cancel || !open {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("preset_save")));
            return;
        }
        self.preset_save = Some(d);
    }

    /// Окно автодакинга. По кнопке считает огибающую дорожки-источника
    /// (честным рендером — гейны, фейды и FX там уже учтены) и кладёт
    /// кривую громкости на выделенные события ОДНИМ шагом истории.
    /// Посчитать интервалы и применить кривую.
    ///
    /// Анализ читает источники НАПРЯМУЮ, своим фетчером: декодерами
    /// владеет MediaWorker плеера (инвариант №5), и лезть к ним из
    /// UI-потока нельзя, а кэш прогрет только вокруг курсора — для
    /// огибающей целой дорожки этого мало.
    /// Автодакинг — В ФОНЕ (Р.49, п.376; долг Р.48).
    ///
    /// Раньше считался СИНХРОННО в UI-потоке: открыть все источники, прочесть
    /// их целиком, посчитать огибающую. На длинном монтаже окно замирало на
    /// десятки секунд без единого признака жизни — ровно то, чего быть не
    /// должно (чек-лист «фоновая работа не мешает»). Теперь это третий
    /// потребитель общего каркаса задач, и им же доказывается, что каркас
    /// общий, а не выкроен под рендер.
    pub(crate) fn start_ducking(&mut self, d: &DuckDialog) {
        // дакинг считает по времени ТОГО, ЧТО ПОД РУКАМИ (Р.50, В1)
        let project = self.model.reel_document();
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let index = self.index_db_path();
        let params = d.params;
        let src_lane = d.src_lane;
        let spans_out = self.duck_spans.clone();
        self.duck_pending = Some(d.clone());
        let rev = self.model_rev;
        self.analysis.request_with(
            crate::analysis::AnalysisKind::Duck,
            rev,
            true,
            move |ctl: crate::bg::BgCtl, send: crate::bg::BgSend<f32>| {
                let mut f = kt_engine::DirectFetcher::new(None);
                let mut missing = 0usize;
                let used: Vec<_> = project
                    .media
                    .iter()
                    .filter(|m| {
                        project
                            .clips
                            .iter()
                            .any(|c| c.media == m.id && c.kind == kt_model::ClipKind::Audio)
                    })
                    .cloned()
                    .collect();
                for (i, m) in used.iter().enumerate() {
                    if ctl.should_stop() {
                        return Err("cancelled".to_string());
                    }
                    send.send(i as f32 / used.len().max(1) as f32 * 0.5);
                    let resolved = resolve_media_path(base.as_deref(), &m.path);
                    match open_source_indexed(&resolved, index.as_deref()) {
                        Ok(src) => f.add_source(m.id, src),
                        Err(_) => missing += 1,
                    }
                }
                send.send(0.5);
                let spans = kt_engine::ducking::duck_spans(&project, &mut f, src_lane, params);
                let n = spans.len();
                *spans_out.lock().unwrap() = (spans, missing);
                send.send(1.0);
                Ok(crate::analysis::AnalysisOut::Duck { spans: n })
            },
        );
    }

    /// Померить громкость медиа по прямой просьбе (Р.49, п.373).
    ///
    /// Именно ПО ПРОСЬБЕ: сам показ ничего не запускает (инвариант 8), а
    /// служба индекса меряет только то, что и так индексирует.
    pub fn measure_loudness(&mut self, id: MediaId) {
        let Some(m) = self.model.project().media.iter().find(|m| m.id == id).cloned() else {
            return;
        };
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let index = self.index_db_path();
        let path = resolve_media_path(base.as_deref(), &m.path);
        let rev = self.model_rev;
        let kind = crate::analysis::AnalysisKind::Loudness(id);
        // «попробуй ещё раз» — это и есть прямая просьба: негативный кэш
        // снимается только здесь
        let retry = self.analysis.failure(kind).is_some();
        self.analysis.request_with(
            kind,
            rev,
            retry,
            move |ctl: crate::bg::BgCtl, send: crate::bg::BgSend<f32>| {
                let mut src = open_source_indexed(&path, index.as_deref())?;
                let total =
                    src.info().duration.to_audio_frames() / kt_time::AUDIO_BURST_FRAMES as i64;
                let mut meter = kt_fx::audio::loudness::LoudnessMeter::new();
                for b in 0..total.max(1) {
                    if ctl.should_stop() {
                        return Err("cancelled".into());
                    }
                    let Ok(burst) = src.read_audio_burst(b) else { break };
                    meter.feed(burst.data.as_slice());
                    if b % 64 == 0 {
                        send.send(b as f32 / total.max(1) as f32);
                    }
                }
                let l = meter.finish();
                Ok(crate::analysis::AnalysisOut::Loudness {
                    media: id,
                    lufs: l.lufs,
                    dbtp: l.dbtp,
                    lra: l.lra,
                })
            },
        );
    }

    /// Результат задачи анализа доехал.
    ///
    /// Стейл-защита по ревизии: пока задача считала, пользователь мог
    /// подменить исходник или перестроить монтаж — применять к другой модели
    /// число, посчитанное по старой, нельзя.
    fn on_analysis_done(
        &mut self,
        kind: crate::analysis::AnalysisKind,
        rev: u64,
        res: Result<crate::analysis::AnalysisOut, String>,
    ) {
        use crate::analysis::{AnalysisKind as K, AnalysisOut as O};
        let out = match res {
            Ok(o) => o,
            Err(e) => {
                if e != "cancelled" {
                    self.warn(crate::tf!(k::msg::ANALYSIS_FAILED, e));
                }
                self.duck_pending = None;
                return;
            }
        };
        // стейл-защита: ревизия И присутствие медиа. Подменили исходник —
        // число, посчитанное по прежнему файлу, к нему не относится.
        let alive = |m: MediaId| self.model.project().media.iter().any(|x| x.id == m);
        let stale = self.model_rev != rev || !kind.media().into_iter().all(alive);
        match (kind, out) {
            (K::Duck, O::Duck { .. }) => {
                if stale {
                    // монтаж успел измениться — кривые от старой модели легли
                    // бы не на те события
                    self.duck_pending = None;
                    self.note(tr(k::msg::ANALYSIS_STALE));
                } else {
                    self.apply_ducking();
                }
            }
            (_, O::Loudness { media, lufs, dbtp, lra }) => {
                // число по исчезнувшему или подменённому медиа относится не
                // к этому проекту: применять его — врать о материале
                if stale {
                    return;
                }
                if let Some(l) = lufs {
                    self.loudness.lock().unwrap().insert(media, (l, dbtp, lra));
                }
            }
            (_, O::Duck { .. }) => {}
        }
    }

    /// Тот же приём результата, что в кадре, — для теста стейл-защиты.
    #[cfg(test)]
    pub(crate) fn on_analysis_done_for_test(
        &mut self,
        kind: crate::analysis::AnalysisKind,
        rev: u64,
        res: Result<crate::analysis::AnalysisOut, String>,
    ) {
        self.on_analysis_done(kind, rev, res);
    }

    /// Применить посчитанные в фоне интервалы дакинга.
    fn apply_ducking(&mut self) {
        let Some(d) = self.duck_pending.take() else { return };
        let (spans, missing) = std::mem::take(&mut *self.duck_spans.lock().unwrap());
        if missing > 0 {
            self.note(format!("{} {missing}", tr(k::msg::DUCK_MISSING)));
        }
        let cmds = kt_model::ops::duck_curve_commands(
            self.model.project(),
            &d.targets,
            &spans,
            d.params.depth_db,
            d.params.attack_ms,
            d.params.release_ms,
        );
        let keys = spans.len();
        if cmds.is_empty() {
            self.note(tr(k::msg::DUCK_NOTHING));
            return;
        }
        self.apply_batch(cmds);
        self.note(format!("{} {keys}", tr(k::msg::DUCK_DONE)));
    }

    /// Есть ли в буфере ИМЕНОВАННОЕ — только тогда спрашиваем префикс
    /// (иначе вставка идёт молча, как раньше).
    pub(crate) fn clipboard_has_names(&self) -> bool {
        !self.clipboard.areas.is_empty()
            || self.clipboard.markers.iter().any(|m| !m.name.is_empty())
    }

    /// Ctrl+V: время — КУРСОР, строка — под мышью, если мышь над
    /// таймлайном (Р.56, п.450).
    pub fn paste_at_cursor(&mut self) {
        let (at, row) = (self.player.transport.playhead(), self.tl_hover_row);
        self.paste_at(at, row)
    }

    /// Пункт «Вставить» правого меню: ВРЕМЯ И СТРОКА КЛИКА (Р.56, п.450,
    /// решение владельца). Кликнув в конкретное место, человек называет
    /// место — искать курсор ему не нужно.
    pub fn paste_at_click(&mut self) {
        let at = self.ctx_at.unwrap_or_else(|| self.player.transport.playhead());
        let row = self.ctx_row;
        self.paste_at(at, row)
    }

    /// Вставка в НАЗВАННОЕ место: время и строка клика (пункт «Вставить»
    /// правого меню) или курсор и строка под мышью (Ctrl+V).
    pub fn paste_at(&mut self, at: Tick, row: Option<(LaneKind, u32)>) {
        if self.clipboard.clips.is_empty() {
            self.note(tr(k::msg::CLIPBOARD_EMPTY));
            return;
        }
        // именованное в буфере — спрашиваем префикс окном (Р.35 п.4);
        // МЕСТО вставки ЗАМОРАЖИВАЕТСЯ, чтобы курсор и мышь под окном его
        // не увели
        if self.clipboard_has_names() && self.paste_names.is_none() {
            self.paste_names = Some(crate::app::PasteNamesDialog {
                at,
                row,
                affix: tr(k::msg::PASTE_AFFIX_DEFAULT).to_string(),
                before: false,
                focus: true,
            });
            return;
        }
        let (at, row) = match self.paste_names.take() {
            Some(d) => (d.at, d.row),
            None => (at, row),
        };
        self.paste_here(at, row, String::new(), false)
    }

    /// Собственно вставка: ОДИН `paste_block` → ОДИН `apply_batch`.
    ///
    /// `row` — строка РАБОЧЕГО ВИДА, по которой кликнули. Блок хранится в
    /// координатах своей ленты (см. `copy_selection`), поэтому здесь он
    /// сначала приводится к строке, а потом, если целевая строка
    /// принадлежит раскрытой полосе, переводится в её координаты.
    pub(crate) fn paste_here(
        &mut self,
        at: Tick,
        row: Option<(LaneKind, u32)>,
        affix: String,
        before: bool,
    ) {
        let target = row.and_then(|(k, l)| self.model.band_of_row(k, l)).map(|b| (b.base, b.len));
        let local_row = row.map(|(k, l)| (k, l - target.map_or(0, |(b, _)| b.get(k))));
        let (block, clamped) = self.clipboard.clone().shifted_to_row(local_row);
        if !clamped.is_empty() {
            self.note(tr(k::msg::PASTE_LANES_CLAMPED));
        }
        let mut block = block;
        // ВСТАВКА В КОРЕНЬ НЕ ЛЕЗЕТ В РАСКРЫТУЮ ПОЛОСУ (Р.62). До change B
        // полоса лежала в ХВОСТЕ числового пространства, и низкие номера
        // всегда принадлежали корню; теперь строки полосы стоят ПОД своим
        // экземпляром, то есть в середине, и половина пары (аудио к
        // кликнутому видео) могла молча уехать внутрь ленты. Строку
        // кликают у ОДНОГО вида — остальные виды перешагивают полосу.
        if target.is_none() && !self.model.bands().is_empty() {
            let mut push = kt_model::LaneBases::default();
            for k in [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera] {
                let Some((first, _)) = block.lane_span(k) else { continue };
                if let Some(b) = self.model.band_of_row(k, first) {
                    *push.get_mut(k) = b.base.get(k) + b.len.get(k) - first;
                }
            }
            block.offset_lanes(push);
        }
        // Дорожки под блок заводятся ЯВНО. Молчаливый рост в `AddClip`
        // при раскрытой полосе положил бы событие не в ту ленту: номер
        // `base` принадлежит полосе, а `base + len` — уже никому.
        let need = block.lanes_needed();
        let mut grow = Vec::new();
        let have = match target {
            Some((_, len)) => len,
            None => kt_model::LaneBases::of_root(self.model.project()),
        };
        for k in [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera] {
            let tail = match target {
                Some((base, len)) => base.get(k) + len.get(k),
                None => have.get(k),
            };
            for i in have.get(k)..need.get(k) {
                let at = tail + (i - have.get(k));
                grow.push(Command::InsertTrack { kind: k, at });
            }
        }
        if let Some((base, _)) = target {
            block.offset_lanes(base);
        }
        let mut next_clip = self.next_clip_id();
        let mut next_group = self.next_group_id();
        let mut next_transition = self.next_transition_id();
        let mut next_morph = self.next_morph_id();
        let mut next_camera = self.next_camera_id();
        let names =
            kt_model::ops::PasteNames { affix, before, taken: self.model.project().taken_names() };
        let (cmds, end, skipped) = kt_model::ops::paste_block(
            &block,
            at,
            kt_model::ops::PasteIds {
                clip: &mut next_clip,
                group: &mut next_group,
                transition: &mut next_transition,
                morph: &mut next_morph,
                camera: &mut next_camera,
            },
            &names,
        );
        if skipped.morphers > 0 {
            self.note(crate::tf!(k::msg::PASTE_SKIPPED_MORPHERS, skipped.morphers));
        }
        if skipped.markers > 0 {
            self.note(crate::tf!(k::msg::PASTE_SKIPPED_MARKERS, skipped.markers));
        }
        let first = cmds.iter().find_map(|c| match c {
            Command::AddClip(ev) => Some(ev.id),
            _ => None,
        });
        // дорожки заводятся ПЕРЕД содержимым и в том же батче — один шаг
        // истории на всю вставку
        let cmds = grow.into_iter().chain(cmds).collect::<Vec<_>>();
        if self.apply_batch(cmds) {
            match first {
                Some(id) => self.select_only(id),
                None => self.selected.clear(),
            }
            // курсор в конец блока — chain-paste (семантика старого)
            self.player.seek(end);
        }
    }

    /// S: ВСЕГДА разрез по курсору (п.5): выделенное+группа, без
    /// выделения — все события под курсором. Вырезка региона — только из
    /// контекст-меню (punch_out_action).
    pub fn cut_action(&mut self) {
        if !self.selected.is_empty() {
            self.split_selected();
        } else {
            self.split_all_at_cursor();
        }
    }

    /// Разрез ВСЕХ клипов, пересекающих курсор (S без выделения).
    pub fn split_all_at_cursor(&mut self) {
        let at = self.player.transport.playhead();
        let targets: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| {
                c.start + MIN_CLIP_DURATION <= at && at + MIN_CLIP_DURATION <= c.start + c.duration
            })
            .map(|c| c.id)
            .collect();
        if targets.is_empty() {
            self.note(tr(k::msg::NO_EVENTS_HERE));
            return;
        }
        let mut next_id = self.next_clip_id();
        let mut next_group = self.next_group_id();
        // одна новая группа на каждую старую (правые половины пар остаются парами)
        let mut group_map: std::collections::HashMap<GroupId, GroupId> =
            std::collections::HashMap::new();
        let mut cmds = Vec::new();
        #[allow(clippy::explicit_counter_loop)]
        for id in targets {
            let old_group = self.model.clip(id).and_then(|c| c.link_group);
            let new_group = old_group.map(|g| {
                *group_map.entry(g).or_insert_with(|| {
                    let ng = GroupId(next_group);
                    next_group += 1;
                    ng
                })
            });
            cmds.push(Command::Split { id, at, new_id: ClipId(next_id), new_group });
            next_id += 1;
        }
        self.apply_batch(cmds);
    }

    /// Вырезка интерьера региона (контекст-меню).
    pub fn punch_out_action(&mut self) {
        if let Some(r) = self.model.project().region {
            let mut next = self.next_clip_id();
            let cmds = kt_model::ops::punch_out_region(self.model.project(), r, &mut next);
            if cmds.is_empty() {
                self.note(tr(k::msg::REGION_MISSES));
            } else {
                self.apply_batch_restoring(cmds);
                self.selected.clear();
            }
        }
    }

    /// Del: с регионом = удалить пересекающиеся (+партнёры), без — выделение.
    pub fn delete_action(&mut self) {
        // выделенный morph-блок — приоритет (фикс D6 v1: при нарисованном
        // регионе morpher было не удалить с клавиатуры)
        if !self.selected_morph.is_empty() || !self.selected_camera.is_empty() {
            // пачкой: рамка теперь ловит служебные блоки, и удаление
            // обязано снимать РОВНО то, что подсвечено (Р.40)
            let cmds: Vec<Command> = self
                .selected_morph
                .iter()
                .map(|id| Command::RemoveMorpher { id: *id })
                .chain(self.selected_camera.iter().map(|id| Command::RemoveCamera { id: *id }))
                .collect();
            if self.apply_batch(cmds) {
                self.selected_morph.clear();
                self.selected_camera.clear();
            }
            return;
        }
        if let Some(r) = self.model.project().region {
            let cmds = kt_model::ops::delete_region(self.model.project(), r);
            if cmds.is_empty() {
                self.note(tr(k::msg::REGION_MISSES));
            } else {
                self.apply_batch_restoring(cmds);
                self.selected.clear();
            }
        } else {
            self.delete_selected();
        }
    }

    pub fn trim_selected_start_to_cursor(&mut self) {
        let Some(sel) = self.require_one_selected() else { return };
        let at = self.player.transport.playhead();
        let cmds = kt_model::ops::trim_group_start_to(self.model.project(), sel, at);
        if cmds.is_empty() {
            self.note(tr(k::msg::CURSOR_OUTSIDE));
        } else {
            self.apply_batch(cmds);
        }
    }

    pub fn trim_selected_end_to_cursor(&mut self) {
        let Some(sel) = self.require_one_selected() else { return };
        let at = self.player.transport.playhead();
        let cmds = kt_model::ops::trim_group_end_to(self.model.project(), sel, at);
        if cmds.is_empty() {
            self.note(tr(k::msg::CURSOR_OUTSIDE));
        } else {
            self.apply_batch(cmds);
        }
    }

    /// Перенос выделенного на соседнюю дорожку своего типа (партнёр на месте).
    pub fn move_selected_lane(&mut self, delta: i32) {
        let Some(sel) = self.require_one_selected() else { return };
        let Some(c) = self.model.clip(sel).cloned() else { return };
        let lanes = match c.kind {
            ClipKind::Video => self.model.project().lanes(kt_model::LaneKind::Video),
            ClipKind::Audio => self.model.project().lanes(kt_model::LaneKind::Audio),
        };
        let new_lane = (c.lane as i64 + delta as i64).clamp(0, lanes.saturating_sub(1) as i64);
        if new_lane as u32 != c.lane {
            // MoveClip двигает всю группу по времени; здесь время не меняется,
            // а lane меняется только у выбранного — семантика старого
            self.apply(Command::MoveClip {
                id: sel,
                new_start: c.start,
                new_lane: new_lane as u32,
            });
        }
    }

    /// «Добавить дорожку» из меню: в конец своего вида. Идёт через ту же
    /// точку, что и вставка из контекстного меню, — иначе имена и
    /// display-порядок расходятся (п.5 р.16).
    pub fn add_track(&mut self, kind: ClipKind) {
        let lk = match kind {
            ClipKind::Video => LaneKind::Video,
            ClipKind::Audio => LaneKind::Audio,
        };
        self.insert_lane_after_row(lk, None);
    }

    /// Магнит A/V: ближайший клип противоположного типа того же медиа
    /// в пределах 2×порога → общий старт/длительность/линк (старый magnetize).
    pub fn magnetize_selected(&mut self) {
        let Some(sel) = self.require_one_selected() else { return };
        let mut next_group = self.next_group_id();
        match kt_model::ops::magnetize_commands(
            self.model.project(),
            sel,
            Tick(90_000 * 5), // партнёр в пределах 5 c
            &mut next_group,
        ) {
            None => self.note(tr(k::msg::NO_OTHER_KIND)),
            Some(cmds) if cmds.is_empty() => self.note(tr(k::msg::ALREADY_LINKED)),
            Some(cmds) => {
                if self.apply_batch(cmds) {
                    self.note(tr(k::msg::AV_LINKED));
                }
            }
        }
    }

    pub fn unlink_selected(&mut self) {
        let Some(sel) = self.require_one_selected() else { return };
        let group = self.model.clip(sel).and_then(|c| c.link_group);
        let ids: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| c.id == sel || (group.is_some() && c.link_group == group))
            .map(|c| c.id)
            .collect();
        self.apply_batch(ids.into_iter().map(|id| Command::UnlinkClip { id }).collect());
    }

    pub fn undo(&mut self) {
        self.model_rev += 1;
        if self.model.undo().is_ok() {
            self.push_project_to_player();
            self.sync_media_sources();
            self.dirty_content = true;
            self.bump_clip_hw();
        }
    }

    pub fn redo(&mut self) {
        self.model_rev += 1;
        if self.model.redo().is_ok() {
            self.push_project_to_player();
            self.sync_media_sources();
            self.dirty_content = true;
            self.bump_clip_hw();
        }
    }

    /// Меню Файл → Новый (п.3 р.7): свежий безымянный проект на месте.
    /// Старое состояние (плеер/потоки/watchdog) корректно дропается ПОСЛЕ
    /// постройки нового.
    pub fn new_project(&mut self) {
        if self.guard_unsaved(Pending::New) {
            *self = Self::new_empty(None);
        }
    }

    /// Меню Файл → Открыть…: диалог + пересоздание приложения на месте.
    pub fn open_project_dialog(&mut self) {
        if self.guard_unsaved(Pending::Open) {
            self.open_project_dialog_now();
        }
    }

    fn open_project_dialog_now(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .set_title(tr(k::dialog::OPEN_PROJECT))
            .add_filter("OpenTimelineIO", &["otio"])
            .pick_file()
        else {
            return;
        };
        match Self::open(&p) {
            Ok(mut app) => {
                app.claim_project(&p);
                *self = app;
            }
            Err(e) => self.note(crate::tf!(k::msg::OPEN_FAILED, e)),
        }
    }

    /// Меню Файл → Сохранить как…: диалог принудительно (save() без
    /// project_path сам его открывает); отмена возвращает старый путь.
    pub fn save_as(&mut self) {
        let old = self.project_path.take();
        self.save();
        if self.project_path.is_none() {
            self.project_path = old; // пользователь передумал
        }
    }

    pub fn save(&mut self) {
        let path = match self.project_path.clone() {
            Some(p) => p,
            // безымянный проект: Save-As диалог (п.12 — отказ здесь был
            // «это не должно быть проблемой»)
            None => {
                let Some(p) = rfd::FileDialog::new()
                    .set_title(tr(k::dialog::SAVE_PROJECT))
                    .add_filter("OpenTimelineIO", &["otio"])
                    .set_file_name("untitled.otio")
                    .save_file()
                else {
                    return; // пользователь передумал
                };
                let p = if p.extension().is_none() { p.with_extension("otio") } else { p };
                self.project_path = Some(p.clone());
                self.claim_project(&p);
                // индекс-БД и тумбы теперь могут жить рядом с проектом
                self.index.enqueue(Some(p.with_extension("ktindex.db")), {
                    let base = p.parent();
                    self.model
                        .project()
                        .media
                        .iter()
                        .map(|m| (m.id, resolve_media_path(base, &m.path)))
                        .collect()
                });
                p
            }
        };
        // Проект занят другой копией — спрашиваем ОДИН раз за сессию (п.352).
        // Не запрещаем: вторая копия законна, а её работа лежит в своём файле
        // восстановления. Но молча затирать чужой Ctrl+S программа не вправе.
        if let Some(pid) = self.lock_holder
            && !self.lock_override
        {
            self.ask = Some(Ask::SaveOverLock { pid });
            return;
        }
        let ui = Some(self.ui_state());
        self.model.set_ui_state(ui);
        let out = kt_otio::save_project(&self.model.document(), &self.retained, path.parent());
        // под сторожем: фоновый автосейв не должен перетереть нашу
        // запись своим более старым снапшотом (Р.32)
        match write_project_guarded(&self.saved_rev, &path, self.model_rev, &out) {
            Ok(SaveOutcome::Wrote { dir_synced }) => {
                self.dirty_content = false;
                self.dirty_ui = false;
                self.note(crate::tf!(k::msg::SAVED, path.display()));
                self.warn_if_dir_unsynced(dir_synced);
                self.save_workspace();
                // работа легла в проект — СВОЁ восстановление больше не
                // нужно. Убираем именно своё (Р.49, п.351): чужое рядом —
                // это несохранённая работа другой копии программы.
                self.drop_own_recovery();
            }
            // НЕ писали: звёздочка остаётся, файл восстановления цел —
            // иначе пользователь считал бы работу сохранённой (п.350)
            Ok(SaveOutcome::SkippedNewerOnDisk) => {
                self.note(tr(k::msg::SAVE_SKIPPED_NEWER));
            }
            Err(e) => self.warn(crate::tf!(k::msg::SAVE_FAILED, e)),
        }
    }

    pub fn next_morph_id(&self) -> u64 {
        self.model.project().max_morph_id() + 1
    }

    pub fn next_camera_id(&self) -> u64 {
        self.model.project().max_camera_id() + 1
    }

    /// Камера под курсором воспроизведения (Р.29): дефолтная «50 мм»,
    /// 5 секунд. Параметры и кривые — в окне камеры (двойной клик по
    /// блоку; до Р.34 этот комментарий врал — окна не было вовсе).
    pub fn add_camera_at_playhead(&mut self, lane: u32) {
        let id = kt_model::CameraId(
            self.model.project().cameras.iter().map(|c| c.id.0).max().unwrap_or(0) + 1,
        );
        let start = self.player.transport.playhead();
        let cam = kt_model::CameraEvent {
            id,
            lane,
            start,
            duration: Tick(5 * 90_000),
            name: String::new(),
            cam: kt_model::CameraParams::default(),
            keyframes: vec![],
            auto: vec![],
            ease: true,
        };
        if self.apply(Command::AddCamera(cam)) {
            self.dirty_content = true;
        }
    }

    /// Первая СВОБОДНАЯ на промежутке служебная строка вида `kind`
    /// (Р.40 п.4). Порядок — display, сверху вниз: слой действует «на всё,
    /// что ниже», и верхняя свободная строка накрывает больше всего.
    /// `None` — свободной нет, зовущий заводит новую.
    ///
    /// Одна функция на морфер и камеру: «положить поверх уже лежащего» —
    /// один дефект, а не два (правило «замечание = КЛАСС проблем»).
    pub fn free_service_lane(&self, kind: LaneKind, start: Tick, duration: Tick) -> Option<u32> {
        let (s0, e0) = (start.0, (start + duration).0);
        let p = self.model.project();
        p.rows().into_iter().filter(|(k, _)| *k == kind).map(|(_, l)| l).find(|lane| match kind {
            LaneKind::Morph => !p
                .morphers
                .iter()
                .any(|m| m.lane == *lane && m.start.0 < e0 && (m.start + m.duration).0 > s0),
            LaneKind::Camera => !p
                .cameras
                .iter()
                .any(|c| c.lane == *lane && c.start.0 < e0 && (c.start + c.duration).0 > s0),
            _ => false,
        })
    }

    /// «+C»: камера в проект (Р.40 п.5, дословно: «я не понимаю как
    /// что-то добавить на камерный трек — для чего он?????»).
    ///
    /// До этого раунда единственным входом был ПКМ по ЗАГОЛОВКУ камерной
    /// строки — полосе шириной 34 px, — а самой строки могло не быть
    /// вовсе: завести её было нечем, кроме того же меню другой строки.
    /// Симметрично «+M» (правило 17: парные способы одного действия).
    pub fn add_camera_layer(&mut self) {
        let start = Tick(self.player.transport.playhead().0.max(0));
        let duration = Tick(5 * 90_000);
        if let Some(lane) = self.free_service_lane(LaneKind::Camera, start, duration) {
            self.add_camera_at_playhead(lane);
            return;
        }
        // свободной строки нет — заводим новую. Место в display-порядке
        // выбирает сама `InsertCameraLane` (`lane_order_insert`): рядом с
        // прочими камерными строками. Своей раскладки тут не считаем —
        // разошлась бы с меню строки, которое зовёт ту же команду.
        let lane = self.model.project().lanes(kt_model::LaneKind::Camera);
        if self.apply(Command::InsertTrack { kind: LaneKind::Camera, at: lane }) {
            self.add_camera_at_playhead(lane);
        }
    }

    /// Выделить РОВНО этот морфер (и снять всё прочее). Отдельная
    /// функция, потому что «выделить» — это три поля, и забытое четвёртое
    /// место уже стоило раунда.
    pub fn select_morph_only(&mut self, id: MorphId) {
        self.selected.clear();
        self.selected_camera.clear();
        self.selected_morph.clear();
        self.selected_morph.insert(id);
    }

    /// То же для камеры.
    pub fn select_camera_only(&mut self, id: kt_model::CameraId) {
        self.selected.clear();
        self.selected_morph.clear();
        self.selected_camera.clear();
        self.selected_camera.insert(id);
    }

    /// Умолчание замка оси: ВКЛЮЧЁН (правило 4 чек-листа — полезный режим
    /// по умолчанию, настраивать программу под нормальное поведение
    /// пользователь не должен). Одно место на продукт и тест.
    pub fn default_axis_linked() -> bool {
        true
    }

    /// Применить просьбы графика автоматизации про ОСЬ (Р.40 п.15).
    ///
    /// Замок один на всю программу, а окон с графиками несколько — значит
    /// применять его обязано ОДНО место, а не каждое окно по-своему.
    /// `set_view` приезжает при НАДЕТОМ замке: колесо над графиком зумит
    /// главный таймлайн, и оси остаются синхронными.
    pub fn apply_axis_req(&mut self, req: crate::automation::AxisReq) {
        if let Some(v) = req.set_linked {
            self.auto_axis_linked = v;
            self.dirty_ui = true;
        }
        if let Some((a, b)) = req.set_view {
            let w = (b.0 - a.0).max(1) as f64;
            // ширина полосы дорожек — из геометрии кадра, своей не заводим
            let px = self.tl_geom.as_ref().map(|g| g.rect.width() as f64).unwrap_or(800.0).max(1.0);
            self.ticks_per_px = (w / px).clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
            self.view_start = (a.0 as f64).max(self.view_floor(self.ticks_per_px));
            self.dirty_ui = true;
        }
    }

    /// Сбор целей morpher'а (порт collectMorpherTargetClipIds v1): регион
    /// приоритетнее выделения (клипы всех дорожек, пересекающие регион),
    /// иначе выделенный клип; в обоих случаях цели расширяются link-группами.
    pub fn morph_target_ids(&self) -> Vec<ClipId> {
        let p = self.model.project();
        let mut ids: Vec<ClipId> = match p.region {
            Some(r) => p.clips.iter().filter(|c| r.intersects_clip(c)).map(|c| c.id).collect(),
            None => self.selected.iter().copied().collect(),
        };
        let groups: Vec<GroupId> =
            p.clips.iter().filter(|c| ids.contains(&c.id)).filter_map(|c| c.link_group).collect();
        for c in &p.clips {
            if let Some(g) = c.link_group
                && groups.contains(&g)
                && !ids.contains(&c.id)
            {
                ids.push(c.id);
            }
        }
        ids.sort_by_key(|i| i.0);
        ids.dedup();
        ids
    }

    /// Добавить морфер-СЛОЙ (Р.35 п.3 и п.5). Четыре пункта меню («Add
    /// morpher», «Add FX batch», те же по региону) схлопнулись в один, и
    /// он не спрашивает ни вид (видов больше нет), ни цели: слой
    /// действует на всё, что НИЖЕ.
    ///
    /// Две вещи делаются здесь ОДНИМ шагом истории, потому что порознь
    /// они бессмысленны: заводится морф-дорожка И поднимается строкой 0.
    /// Умолчание `normalize_lane_order` кладёт морф-строки ПОД видео и
    /// аудио, то есть «всё, что ниже» было бы пусто, — а пользователь
    /// просил ровно обратного: «треки морфера по умолчанию создаём
    /// сверху над всеми».
    /// Где встанет новый морф-блок: ЕСТЬ РЕГИОН — по региону, нет — от
    /// курсора на 1.5 с (решение п.3 Р.38, дословно «есть регион — по
    /// региону; нет — от курсора»). Правило одно на все способы создания,
    /// иначе меню и кнопка разойдутся, как жест M с пунктом «Marker here»
    /// (правило 17 чек-листа).
    pub fn morph_span(&self) -> (Tick, Tick) {
        match self.model.project().region {
            Some(r) if (r.end - r.start).0 >= kt_model::MIN_CLIP_DURATION.0 => {
                (r.start, r.end - r.start)
            }
            _ => (Tick(self.player.transport.playhead().0.max(0)), Tick(90 * 1500)),
        }
    }

    /// Морфер на УЖЕ СУЩЕСТВУЮЩУЮ морф-дорожку — симметрично
    /// `add_camera_at_playhead` (претензия п.2 Р.38: «я не могу никак
    /// добавить морфер-событие на морфер трек»).
    pub fn add_morpher_at(&mut self, lane: u32) {
        let id = MorphId(self.next_morph_id());
        let (start, duration) = self.morph_span();
        let ev = MorpherEvent {
            id,
            lane,
            start,
            duration,
            targets: kt_model::MorphTargets::Below,
            patch: kt_model::MorphPatch::default(),
            timed: true,
            strength: 1.0,
            fade_in: Tick::ZERO,
            fade_out: Tick::ZERO,
            enabled: true,
        };
        if self.apply(Command::AddMorpher(ev)) {
            self.select_morph_only(id);
            self.selected.clear();
            self.open_morph_editor(id);
        }
    }

    /// «+M»: морф-слой в проект.
    ///
    /// Дорожка ПЕРЕИСПОЛЬЗУЕТСЯ (Р.38 п.4, дословно: «если я жму тот же
    /// add morpher layer в другой области — он мне создает еще один
    /// трек»), но только СВОБОДНАЯ НА ЭТОМ ПРОМЕЖУТКЕ (Р.40, дословно:
    /// «можно сто раз добавить морфер слой, и он будет каждый раз его
    /// создавать на одном и том же треке — в итоге там может быть куча
    /// морферов скрыта за одним и тем же местом»). Р.38 переиспользовал
    /// ПЕРВУЮ строку всегда — и клал блок поверх уже лежащего, где его
    /// нечем было схватить.
    ///
    /// Порядок поиска — сверху вниз по display-строкам: слой действует
    /// «на всё, что ниже», и верхняя свободная строка накрывает больше
    /// всего. Свободной нет — заводится новая, наверх.
    pub fn add_morph_layer(&mut self) {
        let (start, duration) = self.morph_span();
        let reuse = self.free_service_lane(LaneKind::Morph, start, duration);
        if let Some(lane) = reuse {
            self.add_morpher_at(lane);
            return;
        }
        let id = MorphId(self.next_morph_id());
        let mut cmds = vec![Command::InsertTrack { kind: LaneKind::Morph, at: 0 }];
        // порядок строк: новая морф-строка наверх, остальные как были
        let mut order: Vec<(LaneKind, u32)> = self
            .model
            .project()
            .rows()
            .into_iter()
            .map(|(k, l)| if k == LaneKind::Morph { (k, l + 1) } else { (k, l) })
            .collect();
        order.insert(0, (LaneKind::Morph, 0));
        cmds.push(Command::SetLaneOrder { order });
        cmds.push(Command::AddMorpher(MorpherEvent {
            id,
            lane: 0,
            start,
            duration,
            targets: kt_model::MorphTargets::Below,
            patch: kt_model::MorphPatch::default(),
            timed: true,
            strength: 1.0,
            fade_in: Tick::ZERO,
            fade_out: Tick::ZERO,
            enabled: true,
        }));
        if self.apply_batch(cmds) {
            self.select_morph_only(id);
            self.selected.clear();
            // v1: окно редактора открывается сразу после размещения
            self.open_morph_editor(id);
        }
    }

    /// Сохранить область (Р.35 п.2). ОДНА функция на все три способа —
    /// отпускание рамки, пункт меню, кнопка тулбара (правило 17): иначе
    /// они разойдутся в том, что именно попадает в область.
    ///
    /// Геометрия берётся из последней рамки выделения, а если её не было
    /// — из региона на всех дорожках. Имя генерится уникальным ТУТ ЖЕ,
    /// чтобы пользователь увидел готовое имя, а не получил отказ.
    pub fn save_marked_area(&mut self) {
        let p = self.model.project();
        let (start, end, lanes) = match self.last_band.clone() {
            Some((a, b, lanes)) if b.0 > a.0 => (a, b, lanes),
            _ => match p.region {
                Some(r) => (r.start, r.end, Vec::new()),
                None => {
                    self.note(tr(k::msg::AREA_NEEDS_BAND));
                    return;
                }
            },
        };
        if (end - start).0 < kt_model::MIN_CLIP_DURATION.0 {
            self.note(tr(k::msg::AREA_NEEDS_BAND));
            return;
        }
        let name = kt_model::unique_name(&p.taken_names(), "", tr(k::msg::AREA_STEM));
        let area = kt_model::MarkedArea {
            name: name.clone(),
            start,
            duration: end - start,
            lanes,
            color: None,
        };
        if self.apply(Command::AddArea(area)) {
            self.note(crate::tf!(k::msg::AREA_SAVED, name));
        }
    }

    /// Свободный номер события. Номера НЕ ПЕРЕИСПОЛЬЗУЮТСЯ в пределах
    /// сессии (Р.49, п.356): прежняя формула «максимум по текущим клипам + 1»
    /// после удаления последнего события выдавала его же номер заново, и
    /// новорождённый клип наследовал ЧУЖОЙ конверт из `Retained.clips` —
    /// имя, маркеры и ссылки на материал удалённого. По тому же номеру
    /// ходят link-группы и ссылки морферов.
    ///
    /// Формат это НЕ ТРОГАЕТ: класс живёт внутри сессии — при переоткрытии
    /// файла конверты пересобираются из него же. Вариант «чистить `Retained`
    /// при удалении» отвергнут: удаление откатывается через undo, а
    /// вычищенный конверт — нет.
    pub fn next_clip_id(&self) -> u64 {
        let live = self.model.project().max_clip_id();
        let hw = self.clip_id_hw.get().max(live);
        self.clip_id_hw.set(hw);
        hw + 1
    }

    /// Производное событие наследует конверт родителя (Р.49, п.357).
    ///
    /// Половина разреза, остаток кропа, вставленная копия, кусок bake — всё
    /// это «то же самое событие, только короче». До Р.49 они теряли ИМЯ,
    /// чужие эффекты (включая непонятые тайм-варпы), маркеры и чужие
    /// metadata: в файл уезжал безымянный клип без всего, что чужой редактор
    /// туда положил.
    ///
    /// Маркеры копируются ЦЕЛИКОМ, а раскладывает их по половинам ПИСАТЕЛЬ,
    /// по пересечению с окном материала. Разложить здесь значило бы потерять
    /// их при undo: отменённый разрез вернул бы родителя без маркеров,
    /// уехавших к детям.
    fn inherit_extras(&mut self, births: &kt_model::Births) {
        for (child, parent) in &births.pairs {
            if self.retained.clips.contains_key(child) {
                continue; // у ребёнка уже свой конверт — не трогаем
            }
            if let Some(ex) = self.retained.clips.get(parent).cloned() {
                self.retained.clips.insert(*child, ex);
            }
        }
    }

    /// Запомнить номера, которые уже рождались. Зовётся ПОСЛЕ каждой правки
    /// модели: только там новые клипы уже в проекте, а удалить их ещё не
    /// успели.
    fn bump_clip_hw(&mut self) {
        let live = self.model.project().clips.iter().map(|c| c.id.0).max().unwrap_or(0);
        self.clip_id_hw.set(self.clip_id_hw.get().max(live));
    }

    /// Зафиксировать жест ручки фейда (Р.28, п.12). Длина меньше минимума
    /// окна — фейда нет: уголок задвинут обратно, переход снимается. Иначе
    /// создаём/правим фейд, целиком лежащий ВНУТРИ события (внешний офсет
    /// нулевой) — ровно как в Vegas, без второго события рядом.
    pub(crate) fn commit_fade_handle(&mut self, id: ClipId, left: bool, len: i64) {
        let existing = self
            .model
            .project()
            .transitions
            .iter()
            .find(|t| {
                t.is_fade()
                    && match (left, t.a, t.b) {
                        (true, None, Some(b)) => b == id,
                        (false, Some(a), None) => a == id,
                        _ => false,
                    }
            })
            .cloned();
        if len < kt_model::MIN_TRANSITION_DURATION.0 {
            if let Some(t) = existing {
                self.apply(Command::RemoveTransition { id: t.id });
            }
            return;
        }
        let (a, b) = if left { (None, Some(id)) } else { (Some(id), None) };
        let (in_offset, out_offset) =
            if left { (Tick::ZERO, Tick(len)) } else { (Tick(len), Tick::ZERO) };
        let had = existing.is_some();
        let ev = match existing {
            // повторный драг уголка правит ТОЛЬКО длину окна: выбранный
            // вид, кривые и кривая аудио не отрубаются сами (правило 20,
            // п.397 — раньше тут собирался свежий Crossfade и молча
            // стирал настройки фейда)
            Some(mut t) => {
                t.in_offset = in_offset;
                t.out_offset = out_offset;
                t
            }
            None => TransitionEvent {
                id: TransitionId(self.next_transition_id()),
                a,
                b,
                in_offset,
                out_offset,
                kind: TransitionKind::Crossfade,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            },
        };
        self.apply(if had { Command::SetTransition(ev) } else { Command::AddTransition(ev) });
    }

    pub fn next_transition_id(&self) -> u64 {
        self.model.project().max_transition_id() + 1
    }

    pub fn next_group_id(&self) -> u64 {
        self.model.project().max_group_id() + 1
    }

    /// Свежий адрес медиа — по ВСЕМУ пространству (Р.50): ленты живут в
    /// одном пространстве адресов с файлами.
    pub fn next_media_id(&self) -> u64 {
        let p = self.model.project();
        p.media.iter().map(|m| m.id.0).chain(p.sequences.iter().map(|s| s.id.0)).max().unwrap_or(0)
            + 1
    }

    pub fn split_selected(&mut self) {
        if self.selected.is_empty() {
            self.note(tr(k::msg::NOTHING_TO_CUT));
            return;
        }
        let at = self.player.transport.playhead();
        // режутся все выделенные (и их link-партнёры), которых курсор реально
        // пересекает; тем, кого не пересекает, разрез не навязываем
        let targets: Vec<ClipId> = {
            let ids = self.selected_with_groups();
            self.model
                .project()
                .clips
                .iter()
                .filter(|c| ids.contains(&c.id) && c.start < at && at < c.start + c.duration)
                .map(|c| c.id)
                .collect()
        };
        if targets.is_empty() {
            self.note(tr(k::msg::CURSOR_OUTSIDE));
            return;
        }
        let mut next_id = self.next_clip_id();
        let mut next_group = self.next_group_id();
        // одна новая группа на каждую старую — правые половины пар остаются
        // парами (та же схема, что у разреза без выделения)
        let mut group_map: std::collections::HashMap<GroupId, GroupId> =
            std::collections::HashMap::new();
        let mut cmds = Vec::new();
        #[allow(clippy::explicit_counter_loop)]
        for id in targets {
            let new_group = self.model.clip(id).and_then(|c| c.link_group).map(|g| {
                *group_map.entry(g).or_insert_with(|| {
                    let ng = GroupId(next_group);
                    next_group += 1;
                    ng
                })
            });
            cmds.push(Command::Split { id, at, new_id: ClipId(next_id), new_group });
            next_id += 1;
        }
        self.apply_batch(cmds);
    }

    pub fn delete_selected(&mut self) {
        // удаляем выделенное вместе с link-группами (как Del по A/V-паре
        // в старом редакторе, только теперь пачкой)
        let ids = self.selected_with_groups();
        if ids.is_empty() {
            return;
        }
        self.apply_batch_restoring(ids.into_iter().map(|id| Command::RemoveClip { id }).collect());
        self.selected.clear();
    }

    pub fn add_media_clip(&mut self, path: &str) {
        let at = self.player.transport.playhead();
        self.add_media_clip_at(path, at, None);
    }

    /// Медиа + клип(ы) в конкретную позицию (дроп в позицию мыши, п.2 v1.4).
    pub fn add_media_clip_at(&mut self, path: &str, at: Tick, lane_hint: Option<(ClipKind, u32)>) {
        let media_id = MediaId(self.next_media_id());
        // снимок синка — ДО apply: источник поднимаем сами ниже (Р.52)
        self.media_paths.insert(media_id, path.to_string());
        if !self.apply(Command::AddMedia { id: media_id, path: path.to_string() }) {
            self.media_paths.remove(&media_id);
            return;
        }
        self.place_media_at(media_id, at, lane_hint);
        // Инкрементально: новый воркер на общий кэш. Пересоздание плеера
        // здесь сбрасывало ВЕСЬ прогретый кэш (п.27) и ломало дельты
        // perf-счётчиков (п.24: «10^19 evictions/с»).
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let resolved = resolve_media_path(base.as_deref(), path);
        match open_source_indexed(&resolved, self.index_db_path().as_deref()) {
            Ok(src) => self.player.add_media(media_id, src),
            Err(e) => self.note(format!("{path}: {e}")),
        }
        self.thumbs.add_media(media_id, resolved.clone());
        // в индекс-БД — резолвнутый путь (п.150: сырой относительный путь
        // давал файлу ВТОРОЙ media_id в кэше рядом с резолвнутым)
        self.index.enqueue(
            self.project_path.as_ref().map(|p| p.with_extension("ktindex.db")),
            vec![(media_id, resolved.clone())],
        );
        self.note(crate::tf!(k::msg::ADDED, path));
    }

    /// Клип(ы) существующего медиа на курсор (клик в бине).
    pub fn place_media_at_cursor(&mut self, media_id: MediaId) {
        let at = self.player.transport.playhead();
        self.place_media_at(media_id, at, None);
    }

    /// Экземпляр ЛЕНТЫ в позицию `at` (Р.55, С6).
    ///
    /// Пара видео+аудио в одной link-группе — как у сворачивания
    /// (`collapse_into_sequence`): тогда «поставил, потом замедли целиком»
    /// получается даром через `sync_warp_to_link_group`.
    fn place_reel_at(&mut self, seq: MediaId, at: Tick, lane_hint: Option<(ClipKind, u32)>) {
        let doc = self.model.document();
        let Some(s) = doc.sequence(seq) else { return };
        let len = s.end().max(kt_model::MIN_CLIP_DURATION);
        let kinds: Vec<ClipKind> = [ClipKind::Video, ClipKind::Audio]
            .into_iter()
            .filter(|k| s.clips.iter().any(|c| c.kind == *k))
            .collect();
        if kinds.is_empty() {
            self.warn(tr(k::msg::NEST_EMPTY_REEL));
            return;
        }
        let mut next_clip = self.next_clip_id();
        let group = (kinds.len() > 1).then(|| {
            let g = kt_model::GroupId(self.next_group_id());
            g
        });
        let mut cmds = Vec::new();
        let mut first = None;
        for k in kinds {
            let id = ClipId(next_clip);
            next_clip += 1;
            first.get_or_insert(id);
            cmds.push(Command::AddClip(kt_model::ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id,
                link_group: group,
                kind: k,
                lane: match lane_hint {
                    Some((hk, l)) if hk == k => l,
                    _ => 0,
                },
                start: at,
                duration: len,
                source_in: Tick::ZERO,
                media: seq,
                gain_db: 0.0,
                reversed: false,
                fx: Vec::new(),
                fx_bypass: Vec::new(),
                fx_auto: Vec::new(),
                opacity: 1.0,
                fx_off: false,
            }));
        }
        if self.apply_batch(cmds)
            && let Some(id) = first
        {
            self.select_only(id);
        }
    }

    /// Клип(ы) медиа в позицию `at`; `lane_hint` — дорожка под мышкой при
    /// дропе (п.2 v1.4): её kind получает эту lane, парная половина — lane 0.
    pub fn place_media_at(
        &mut self,
        media_id: MediaId,
        at: Tick,
        lane_hint: Option<(ClipKind, u32)>,
    ) {
        // ЛЕНТА ставится ЭКЗЕМПЛЯРОМ (Р.55, С6): у неё нет ни файла, ни
        // длительности из источника — длина берётся из её содержимого.
        // Раньше поставить ленту во второе место можно было ТОЛЬКО
        // копипастом уже стоящего экземпляра.
        if self.model.document().sequence(media_id).is_some() {
            self.place_reel_at(media_id, at, lane_hint);
            return;
        }
        let Some(path) =
            self.model.project().media.iter().find(|m| m.id == media_id).map(|m| m.path.clone())
        else {
            return;
        };
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let resolved = resolve_media_path(base.as_deref(), &path);
        let info = match open_source_indexed(&resolved, self.index_db_path().as_deref()) {
            Ok(s) => s.info().clone(),
            Err(e) => {
                self.note(format!("{path}: {e}"));
                return;
            }
        };
        let cmds = self.clip_cmds_for(media_id, &info, at, lane_hint);
        self.apply_batch(cmds);
    }

    /// Команды «положить клип(ы) этого медиа в точку». Вынесено из
    /// `place_media_at` (Р.53, п.416): создание генератора обязано класть
    /// клип ТЕМ ЖЕ батчем, что и медиа — иначе новый вид появляется
    /// только в бине, и пользователь набирает титр вслепую (правило 4:
    /// полезное умолчание).
    pub(crate) fn clip_cmds_for(
        &mut self,
        media_id: MediaId,
        info: &kt_media::MediaInfo,
        at: Tick,
        lane_hint: Option<(ClipKind, u32)>,
    ) -> Vec<Command> {
        let dur = if info.duration.0 > 0 { info.duration } else { Tick(90_000 * 5) };
        let group = (info.has_video && info.has_audio).then(|| GroupId(self.next_group_id()));
        let mut next_id = self.next_clip_id();
        let mut cmds = Vec::new();
        // Р.62: ПЕРВЫЙ клип в пустой проект с неявными настройками задаёт
        // растр и fps (как Premiere: «изменить настройки под клип»); дальше
        // несовпадение пропорций НЕ спрашивает (правило 4), а сообщает с
        // адресом «где изменить» (правила 16, 19). Процедурные источники
        // (fps = None) пропорций не имеют и проект не задают.
        if info.has_video && info.fps.is_some() {
            let settings = self.model.document().settings.clone();
            let (vw, vh) = info.view_size();
            let even = |v: f64| ((v.round() as u32).max(16)) & !1;
            let (w, h) = (even(vw), even(vh));
            let empty_root =
                self.model.editing().is_none() && self.model.project().clips.is_empty();
            if empty_root && !settings.settings_explicit {
                let fps = info
                    .fps
                    .filter(|(n, d)| *d > 0 && n % d == 0)
                    .map(|(n, d)| n / d)
                    .filter(|f| (1..=120).contains(f) && 90_000 % f == 0);
                let mut st = settings.clone();
                st.width = w;
                st.height = h;
                if let Some(f) = fps {
                    st.fps = f;
                }
                if st != settings {
                    // доля превью сохраняется (п.20 р.7)
                    let pq = self.player.preview_quality();
                    let frac = pq.width as f32 / settings.width.max(1) as f32;
                    self.player.set_preview_quality(preview_quality_for(w, h, frac));
                    cmds.push(Command::SetSettings { settings: st });
                }
                match fps {
                    Some(f) => self.note(crate::tf!(k::msg::FIRST_CLIP_SET_PROJECT, w, h, f)),
                    None => {
                        let (n, d) = info.fps.unwrap_or((0, 1));
                        let shown = format!("{:.3}", n as f64 / d.max(1) as f64);
                        self.note(crate::tf!(
                            k::msg::FIRST_CLIP_FPS_KEPT,
                            w,
                            h,
                            shown,
                            settings.fps
                        ));
                    }
                }
            } else {
                let pa = settings.width as f64 / settings.height.max(1) as f64;
                let va = vw / vh.max(1.0);
                if ((va - pa) / pa).abs() > 0.01 {
                    let name = self
                        .model
                        .project()
                        .media
                        .iter()
                        .find(|m| m.id == media_id)
                        .map(|m| {
                            std::path::Path::new(&m.path)
                                .file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_else(|| m.path.clone())
                        })
                        .unwrap_or_default();
                    let rule = crate::settings_window::rule_label(settings.conform);
                    self.note(crate::tf!(
                        k::msg::CONFORM_MISMATCH,
                        name,
                        w,
                        h,
                        settings.width,
                        settings.height,
                        rule
                    ));
                }
            }
        }
        for (kind, present) in
            [(ClipKind::Video, info.has_video), (ClipKind::Audio, info.has_audio)]
        {
            if !present {
                continue;
            }
            let lane = match lane_hint {
                Some((k, l)) if k == kind => l,
                _ => 0,
            };
            cmds.push(Command::AddClip(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(next_id),
                link_group: group,
                kind,
                lane,
                start: at,
                duration: dur,
                source_in: Tick(0),
                media: media_id,
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            }));
            next_id += 1;
        }
        cmds
    }

    // -- снап ---------------------------------------------------------------

    fn draw_preview(&mut self, ui: &mut egui::Ui) {
        let t = self.player.video_playhead();
        let q = self.player.preview_quality();
        // Ключ — тик, ВЫРОВНЕННЫЙ по сетке fps (тест 39): раньше ключом
        // был сырой тик — при игре он менялся каждый UI-кадр и текстура
        // перезаливалась в GPU 60 раз/с независимо от fps проекта
        // (до ~500 МБ/с на 1080p — главный источник «визуально <30 fps»).
        let grid = self.player.video_grid();
        let raw_key = preview_key_tick(grid, t);
        // Ключ не едет назад ВООБЩЕ, кроме seek/loop-wrap (п.14 р.14):
        // прежний допуск «откат до 2 ячеек» был слеп к скорости и при
        // 0.25× пропускал откаты картинки на пару кадров назад.
        let tgen = (self.player.transport.generation(), self.player.transport.wraps());
        let reanchor = !self.player.transport.is_playing() || tgen != self.preview_gen;
        let (key_tick, skipped, rolled) = kt_engine::player::preview_key_step(
            self.preview_key.map(|k| k.0),
            raw_key,
            grid.frame_step(),
            if self.play_dir_back { -1 } else { 1 },
            reanchor,
        );
        self.preview_gen = tgen;
        self.perf.video_cells_skipped += skipped;
        if rolled {
            self.perf.video_rollbacks += 1;
        }
        let key_now = (key_tick, (q.width, q.height));
        // СОДЕРЖИМОЕ источников — часть ключа превью (Р.53, п.412).
        //
        // Правка параметров генератора не двигает ни курсор, ни качество,
        // ни план: ключ остаётся прежним, композит считается «уже
        // посчитанным», и на экране висит СТАРЫЙ кадр — до первого
        // движения playhead'а. Харнесс этого не видел: t478 спрашивает
        // кэш плеера напрямую, мимо мемоизации превью, и был зелёным на
        // программе, где правка цвета до картинки не доезжала (живой
        // сценарий `generator` показал чёрное превью при оранжевом
        // значении в модели).
        let content = self.player.content_version();
        // …и ПРОЕКТ — часть ключа тоже (Р.62, t722b): смена правила
        // приведения в окне настроек на паузе не трогает ничего из
        // перечисленного выше, и композит считался «уже посчитанным»
        let project = self.player.project_version();
        let need = match self.preview_key {
            Some((pt, pq, was_complete)) => {
                pt != key_now.0
                    || pq != key_now.1
                    || !was_complete
                    || self.preview_content != content
                    || self.preview_project != project
            }
            None => true,
        };
        self.preview_content = content;
        self.preview_project = project;
        let new_frame = self.preview_key.map(|k| k.0) != Some(key_now.0);
        let mut complete = self.preview_key.map(|k| k.2).unwrap_or(false);
        if need {
            self.last_compose_retry = std::time::Instant::now();
            let mut rgba = std::mem::take(&mut self.preview_buf);
            // композиция по РАТЧЕТНОМУ ключу, а не по сырому t: иначе на
            // ретрае (кадр ещё догружается) картинка пересобиралась по
            // позиции, которая на 1-2 ячейки позади ключа, и на экран
            // уходил более ранний кадр (п.14 р.14)
            let (cq, c) = self.player.compose_preview_into(Tick(key_tick), &mut rgba);
            complete = c;
            // держим последний ПОЛНЫЙ кадр: неполный композит не затирает
            // картинку (мигание «чёрный кадр ↔ Загрузка», п.14/15)
            if c || self.texture.is_none() {
                // glow-мип: грубый box-даунскейл 32×18; при игре — каждый
                // 4-й новый кадр (амбиент-свечение мерцания не заметит)
                self.glow_skip = self.glow_skip.wrapping_add(1);
                if self.sw_render == Some(false) && self.glow_skip % 4 == 1 {
                    let (gw, gh) = (32usize, 18usize);
                    let mut small = vec![0u8; gw * gh * 4];
                    let (sw, sh) = (cq.width as usize, cq.height as usize);
                    for gy in 0..gh {
                        let sy = gy * sh / gh;
                        for gx in 0..gw {
                            let sx = gx * sw / gw;
                            let si = (sy * sw + sx) * 4;
                            let di = (gy * gw + gx) * 4;
                            small[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
                        }
                    }
                    let gimg = egui::ColorImage::from_rgba_unmultiplied([gw, gh], &small);
                    match &mut self.glow_texture {
                        Some(t) => t.set(gimg, egui::TextureOptions::LINEAR),
                        None => {
                            self.glow_texture = Some(ui.ctx().load_texture(
                                "preview-glow",
                                gimg,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                    }
                }
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [cq.width as usize, cq.height as usize],
                    &rgba,
                );
                let opts = if self.smooth_scaling {
                    egui::TextureOptions::LINEAR
                } else {
                    egui::TextureOptions::NEAREST
                };
                match &mut self.texture {
                    Some(tex) => tex.set(image, opts),
                    None => {
                        self.texture = Some(ui.ctx().load_texture("preview", image, opts));
                    }
                }
                // честная метрика V-fps: реальные показы НОВЫХ кадров
                if new_frame {
                    self.perf.video_frames_shown += 1;
                }
            }
            self.preview_key = Some((key_now.0, key_now.1, c));
            self.preview_buf = rgba; // буфер обратно (переиспользование)
        }
        if complete {
            self.incomplete_since = None;
        } else if self.incomplete_since.is_none() {
            self.incomplete_since = Some(std::time::Instant::now());
        }
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        self.preview_rect = rect;
        // картинка рисуется от самого верха окна, а клики принимаются
        // НИЖЕ полосы перетаскивания окна: иначе нажатие у кромки — это
        // одновременно ресайз, перенос окна и play/pause (п.1 р.11)
        let hit =
            egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + TOP_DRAG_H), rect.max);
        let resp = ui.interact(hit, ui.id().with("preview-click"), egui::Sense::click());
        let resp = self.hint(resp, tr(k::hint::PREVIEW_CLICK));
        // Правый клик по превью (Р.49, п.365). Двойной клик уже умеет
        // отделять и возвращать окно, но узнать об этом можно было только
        // из подсказки — на правую кнопку превью молчало. Пункт ровно один
        // и ровно тот, что делает жест: новых ключей ноль.
        {
            let detached = self.preview_detached;
            let mut toggle = false;
            let mut picked = false;
            let mut cycle = false;
            let mode_now = self.geom_ctl;
            resp.clone().context_menu(|ui| {
                picked = true;
                // КОНТРОЛЫ НАД КАДРОМ (Р.64) — первым пунктом: это то,
                // зачем к превью правой кнопкой и приходят чаще всего.
                // Один пункт на все три состояния: набор активен ровно
                // один (правило Resolve), и переключатель у него один.
                let r = ui.button(crate::tf!(
                    k::geomctl::TOGGLE,
                    crate::geomctl::mode_name(mode_now.next())
                ));
                crate::widgets::field_probe::note("geom.toggle", r.rect);
                if r.clicked() {
                    cycle = true;
                    ui.close();
                }
                ui.separator();
                let label = if detached {
                    tr(k::menu::view::ATTACH_PREVIEW)
                } else {
                    tr(k::menu::view::DETACH_PREVIEW)
                };
                if ui.button(label).clicked() {
                    toggle = true;
                    ui.close();
                }
            });
            if cycle {
                self.cycle_geom_mode();
            }
            if picked {
                self.set_panel_target(crate::ctx_menu::PanelTarget::Preview);
            }
            if toggle {
                self.preview_detached = !detached;
                self.dirty_ui = true;
            }
        }
        // пп.10-11 р.7: одиночный клик (отложенный на окно double-click) =
        // play/pause; двойной = detach/attach. Работает и в отдельном окне
        // превью (двойной клик там возвращает превью обратно).
        {
            let pending_ms = self.preview_click_at.map(|t| t.elapsed().as_millis());
            let (action, op) = preview_click_step(
                pending_ms,
                resp.clicked(),
                resp.double_clicked(),
                PREVIEW_CLICK_DELAY_MS,
            );
            match op {
                PendingOp::Arm => self.preview_click_at = Some(std::time::Instant::now()),
                PendingOp::Clear => self.preview_click_at = None,
                PendingOp::Keep => {}
            }
            match action {
                PreviewClick::TogglePlay => self.on_transport_toggle(),
                PreviewClick::ToggleDetach => {
                    self.preview_detached =
                        preview_detached_next(self.preview_detached, true, false);
                }
                PreviewClick::None => {}
            }
            if let Some(t0) = self.preview_click_at {
                // дождаться истечения окна без ввода
                let left = PREVIEW_CLICK_DELAY_MS.saturating_sub(t0.elapsed().as_millis());
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(left as u64 + 5));
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(10, 10, 12));
        if let Some(tex) = &self.texture {
            let scale =
                (rect.width() / q.width as f32).min(rect.height() / q.height as f32).max(0.05);
            let size = egui::vec2(q.width as f32 * scale, q.height as f32 * scale);
            let img_rect = egui::Rect::from_center_size(rect.center(), size);
            // ambient glow как на youtube (п.18): РАЗМЫТАЯ мини-копия
            // (32×18, LINEAR-растяжка = сильный блюр) + виньетка к фону
            if self.sw_render == Some(false)
                && let Some(glow) = &self.glow_texture
            {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                for (expand, alpha) in [(0.35f32, 40u8), (0.18, 60)] {
                    let glow_rect = egui::Rect::from_center_size(
                        img_rect.center(),
                        img_rect.size() * (1.0 + expand),
                    )
                    .intersect(rect);
                    painter.image(
                        glow.id(),
                        glow_rect,
                        uv,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                    );
                }
                // виньетка: края области затухают в цвет фона
                let bg = egui::Color32::from_rgb(10, 10, 12);
                let fade = 36.0f32;
                let mut vin = egui::Mesh::default();
                let mut quad = |a: egui::Pos2,
                                b: egui::Pos2,
                                ca: egui::Color32,
                                cb: egui::Color32,
                                horiz: bool| {
                    let i = vin.vertices.len() as u32;
                    let pts = if horiz {
                        [
                            (egui::pos2(a.x, a.y), ca),
                            (egui::pos2(b.x, a.y), cb),
                            (egui::pos2(b.x, b.y), cb),
                            (egui::pos2(a.x, b.y), ca),
                        ]
                    } else {
                        [
                            (egui::pos2(a.x, a.y), ca),
                            (egui::pos2(b.x, a.y), ca),
                            (egui::pos2(b.x, b.y), cb),
                            (egui::pos2(a.x, b.y), cb),
                        ]
                    };
                    for (p, c) in pts {
                        vin.vertices.push(egui::epaint::Vertex {
                            pos: p,
                            uv: egui::epaint::WHITE_UV,
                            color: c,
                        });
                    }
                    vin.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                };
                let t = egui::Color32::from_rgba_unmultiplied(10, 10, 12, 0);
                quad(rect.left_top(), egui::pos2(rect.left() + fade, rect.bottom()), bg, t, true);
                quad(egui::pos2(rect.right() - fade, rect.top()), rect.right_bottom(), t, bg, true);
                quad(rect.left_top(), egui::pos2(rect.right(), rect.top() + fade), bg, t, false);
                quad(
                    egui::pos2(rect.left(), rect.bottom() - fade),
                    rect.right_bottom(),
                    t,
                    bg,
                    false,
                );
                painter.add(egui::Shape::mesh(vin));
            }
            painter.image(
                tex.id(),
                img_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            // тонкая рамка кадра
            painter.rect(
                img_rect,
                2.0,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 24)),
                egui::StrokeKind::Outside,
            );
            // safe-зоны (Р.52 С3, F-TXT-001): пока открыто окно титра —
            // action 5% и title 10% (наследие кинескопов, живо до сих пор)
            if self.show_safe_zones() {
                for r in safe_zone_rects(img_rect) {
                    painter.rect(
                        r,
                        0.0,
                        egui::Color32::TRANSPARENT,
                        egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 70),
                        ),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            // КОНТРОЛЫ ГЕОМЕТРИИ — до драга титра: у более мелкой и
            // более специфичной цели (блок титра) приоритет, egui отдаёт
            // указатель последнему зарегистрированному интерактору
            self.preview_img_rect = img_rect;
            self.geometry_controls(ui, &painter, img_rect, (q.width, q.height));
            self.title_drag_in_preview(ui, &painter, img_rect);
            // скрим — только если неполнота держится >180 мс (v1)
            let show_overlay = self
                .incomplete_since
                .map(|t0| t0.elapsed() > std::time::Duration::from_millis(180))
                .unwrap_or(false);
            if show_overlay {
                painter.rect_filled(img_rect, 2.0, egui::Color32::from_black_alpha(140));
                painter.text(
                    img_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    tr(k::perf::LOADING),
                    egui::FontId::proportional(16.0),
                    egui::Color32::from_gray(230),
                );
            }
        }
    }
    /// Компактный контрол скорости (пп.4-5 раунда 5): точное поле основной
    /// скорости + ОДНА полоса — drag по ней = shuttle (упругий, отпустил —
    /// вернулся к основной, Vegas), double-click = 1×; маркер основной
    /// скорости виден на полосе. Маппинг полосы — slider_to_rate (тест 46).
    /// Контрол скорости (пп.18-19 р.7, решение пользователя): ДВЕ
    /// параллельные полосы в прежнем футпринте 170×20 — верхняя двигает
    /// ПОСТОЯННУЮ скорость (остаётся), нижняя — shuttle с автовозвратом.
    /// Значение при драге — оверлеем у курсора; поле ввода и правая
    /// надпись «+1.00×» убраны (очевидные надписи не делаем).
    pub(crate) fn draw_speed_control(&mut self, ui: &mut egui::Ui) {
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(170.0, 20.0), egui::Sense::click_and_drag());
        self.speed_rect = rect;
        crate::widgets::field_probe::note("speed", rect);
        // Ввод числом живёт В ТОМ ЖЕ прямоугольнике (Р.57): ни окна, ни
        // лишней полосы — правила 3, 8 и 13.
        if self.speed_entry.is_some() {
            self.draw_speed_entry(ui, rect, resp);
            return;
        }
        // поле закрыли СНАРУЖИ (Esc-каскад) — фокус всё равно обязан
        // вернуться, иначе горячие клавиши мертвы до клика мимо (правило 14)
        if resp.has_focus() {
            ui.memory_mut(|m| m.surrender_focus(resp.id));
        }
        let p = ui.painter_at(rect);
        // временная скорость на нижней полосе — это shuttle ИЛИ разгон
        // удержанием пробела: без этого при разгоне виджет врал «1×»
        // (п.3 р.16, найдено живой проверкой)
        let temp_t = self.speed_temp_slider();
        let overriding = temp_t.is_some();
        let mid = rect.center().y;
        let upper = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), mid));
        let lower = egui::Rect::from_min_max(egui::pos2(rect.left(), mid), rect.max);
        p.rect_filled(rect, 2.0, egui::Color32::from_gray(24));
        p.line_segment(
            [egui::pos2(rect.left(), mid), egui::pos2(rect.right(), mid)],
            egui::Stroke::new(1.0, crate::theme::SEAM),
        );
        let to_x = |t: f32| rect.left() + (t + 1.0) * 0.5 * rect.width();
        // засечки: центр (0/стоп) и ±1×, ±2× — на всю высоту
        for (t, strong) in [
            (0.0f32, true),
            (rate_to_slider(1.0), true),
            (rate_to_slider(-1.0), true),
            (1.0, false),
            (-1.0, false),
        ] {
            let x = to_x(t);
            p.line_segment(
                [
                    egui::pos2(x, rect.top() + if strong { 2.0 } else { 5.0 }),
                    egui::pos2(x, rect.bottom() - if strong { 2.0 } else { 5.0 }),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_gray(if strong { 70 } else { 48 })),
            );
        }
        // полоса выбирается в кадре нажатия и держится до конца драга
        if resp.drag_started()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            self.speed_drag_upper = Some(pos.y < mid);
            self.speed_drag_vx = pos.x - rect.left();
        } else if resp.dragged() {
            // Shift — ТОЧНАЯ подстройка (Р.57): копим ПРИРАЩЕНИЯ в свой
            // виртуальный курсор. Делить абсолютную позицию указателя
            // нельзя — Shift, нажатый посреди жеста, утащил бы значение
            // к центру скачком.
            let fine = ui.input(|i| i.modifiers.shift);
            self.speed_drag_vx = drag_advance(self.speed_drag_vx, resp.drag_delta().x, fine);
        }
        if resp.dragged() || resp.drag_started() {
            let raw = self.speed_drag_vx / rect.width() * 2.0 - 1.0;
            let t = raw.clamp(-1.0, 1.0);
            // ушли за пределы виджета — «второе дыхание» (п.4 р.8)
            let over_px = ((raw.abs() - 1.0) * rect.width() * 0.5).max(0.0);
            let rate_of =
                |t: f32| overshoot_rate(slider_to_rate(t), over_px, OVERSHOOT_PX_PER_DOUBLE);
            if self.speed_drag_upper == Some(true) {
                // верхняя полоса: постоянная скорость (остаётся)
                self.base_rate = rate_of(t);
                if self.shuttle_override.is_none() {
                    self.player.set_playback_rate(self.base_rate);
                }
            } else {
                // нижняя: временный shuttle с автовозвратом
                self.shuttle_override = Some(t);
                self.player.set_playback_rate(rate_of(t));
            }
        }
        if resp.drag_stopped() {
            self.speed_drag_upper = None;
            if self.shuttle_override.take().is_some() {
                self.player.set_playback_rate(self.base_rate);
            }
        }
        if resp.double_clicked() {
            self.base_rate = 1.0;
            self.shuttle_override = None;
            self.player.set_playback_rate(1.0);
        }
        let base_t = rate_to_slider(self.base_rate);
        let cur_t = temp_t.unwrap_or(base_t);
        // верхняя полоса: заливка от центра до base + маркер
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(to_x(0.0).min(to_x(base_t)), upper.top() + 3.0),
                egui::pos2(to_x(0.0).max(to_x(base_t)), upper.bottom() - 1.0),
            ),
            1.0,
            egui::Color32::from_rgba_unmultiplied(120, 160, 255, 70),
        );
        p.line_segment(
            [egui::pos2(to_x(base_t), upper.top() + 1.0), egui::pos2(to_x(base_t), upper.bottom())],
            egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(150, 180, 255, 220)),
        );
        // нижняя полоса: shuttle-заливка + кружок текущей скорости
        let (x0, x1) = (to_x(0.0).min(to_x(cur_t)), to_x(0.0).max(to_x(cur_t)));
        if overriding {
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x0, lower.top() + 1.0),
                    egui::pos2(x1, lower.bottom() - 3.0),
                ),
                1.0,
                egui::Color32::from_rgba_unmultiplied(255, 190, 90, 90),
            );
        }
        p.circle_filled(
            egui::pos2(to_x(cur_t), lower.center().y),
            4.5,
            if overriding {
                egui::Color32::from_rgb(255, 200, 100)
            } else {
                egui::Color32::from_gray(210)
            },
        );
        // значение — оверлеем у курсора ТОЛЬКО при драге (п.19 р.7)
        if self.speed_drag_upper.is_some()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let cur = self.player.playback_rate();
            let shown = if self.speed_drag_upper == Some(true) { self.base_rate } else { cur };
            egui::Area::new(egui::Id::new("speed-drag-overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos + egui::vec2(14.0, -26.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.monospace(format_rate(shown));
                    });
                });
        }
        // ВВОД ЧИСЛОМ (Р.57, просьба владельца «должен иметь возможность
        // выставить 0,05»): правый клик или Ctrl+клик по полосе. Оба входа
        // ведут в одно тело (правило 17).
        if resp.secondary_clicked() || (resp.clicked() && ui.input(|i| i.modifiers.command)) {
            self.speed_entry = Some(SpeedEntry { text: String::new(), focus: true, bad: false });
        }
        self.hint(resp, tr(k::hint::SPEED));
    }

    /// Поле ввода скорости НА МЕСТЕ полосы (Р.57). Форма — та же, что у
    /// маски таймкода в тулбаре (`toolbar.rs`): тот же прямоугольник, тот
    /// же моноширинный шрифт, подчёркивание акцентом; события съедаются,
    /// чтобы цифры и Enter не утекли в хоткеи. «Тупо воткнутый
    /// `TextEdit`» тут запрещён правилом 8.
    fn draw_speed_entry(&mut self, ui: &mut egui::Ui, rect: egui::Rect, resp: egui::Response) {
        let Some(mut st) = self.speed_entry.clone() else { return };
        if st.focus {
            resp.request_focus();
            st.focus = false;
        }
        // стрелки/Tab не должны уводить фокус из поля
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                resp.id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: false,
                },
            )
        });
        let mut apply = false;
        let mut cancel = false;
        ui.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Text(t)
                    if t.chars().all(|c| {
                        c.is_ascii_digit()
                            || matches!(c, '.' | ',' | '-' | '+' | '\u{2212}' | 'x' | '\u{00d7}')
                    }) =>
                {
                    st.text.push_str(t);
                    st.bad = false;
                    false
                }
                egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                    st.text.pop();
                    st.bad = false;
                    false
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                    apply = true;
                    false
                }
                egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                    cancel = true;
                    false
                }
                _ => true,
            })
        });
        let font = egui::FontId::monospace(13.0);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(22, 24, 28));
        painter.line_segment(
            [
                egui::pos2(rect.left() + 2.0, rect.bottom() - 1.0),
                egui::pos2(rect.right() - 2.0, rect.bottom() - 1.0),
            ],
            egui::Stroke::new(
                1.0,
                // правило 16: нечитаемое значение — ОШИБКА, а не блёклая
                // подсказка; поле при этом остаётся открытым
                if st.bad { HINT_ERROR } else { crate::theme::ACCENT },
            ),
        );
        // пусто — показываем текущую скорость приглушённо: первая цифра
        // заменяет её целиком (то же, что «открылось с выделенным всем»)
        let (txt, col) = if st.text.is_empty() {
            (format_rate(self.base_rate), egui::Color32::from_gray(120))
        } else {
            (st.text.clone(), egui::Color32::from_rgb(235, 235, 240))
        };
        painter.text(
            egui::pos2(rect.left() + 5.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            txt,
            font,
            col,
        );
        if apply {
            match parse_rate_text(&st.text).map(normalize_rate) {
                Some(r) => {
                    self.base_rate = r;
                    if self.shuttle_override.is_none() {
                        self.player.set_playback_rate(r);
                    }
                    self.speed_entry = None;
                }
                None => {
                    st.bad = true;
                    self.speed_entry = Some(st);
                }
            }
        } else if cancel || (resp.lost_focus() && !resp.has_focus()) {
            self.speed_entry = None;
        } else {
            self.speed_entry = Some(st);
        }
        // ФОКУС ОБЯЗАН ВЕРНУТЬСЯ (правило 14). Гард стоит на КАЖДОМ пути
        // закрытия сразу — включая внешний (Esc-каскад гасит `speed_entry`
        // без нашего ведома): виджет живёт каждый кадр с тем же id, и
        // «просто закрыть» мало — egui считал бы его сфокусированным, а
        // `handle_keys` гейтится `egui_wants_keyboard_input()`.
        if self.speed_entry.is_none() {
            ui.memory_mut(|m| m.surrender_focus(resp.id));
        }
        self.hint(resp, tr(k::hint::SPEED_ENTRY));
    }

    /// Контролы превью (п.17 р.7: без очевидных надписей — значения
    /// самодостаточны; п.20: Разрешение ПЕРЕД Качеством — качество есть
    /// доля разрешения проекта, движок превью видит только произведение).
    fn draw_preview_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let proj = self.model.project().settings.clone();
            let cur_q = self.player.preview_quality();
            let fracs = [1.0f32 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 2.0, 1.0];
            let mut frac_idx = {
                let ratio = cur_q.width as f32 / proj.width.max(1) as f32;
                fracs
                    .iter()
                    .enumerate()
                    .min_by(|a, b| (a.1 - ratio).abs().partial_cmp(&(b.1 - ratio).abs()).unwrap())
                    .map(|(i, _)| i)
                    .unwrap_or(2)
            };
            // размер доли — ПОДОБНЫЙ кадру проекта (Р.64)
            let q_for = preview_quality_for;
            let mut fps = proj.fps;
            let before_fps = fps;
            let r = egui::ComboBox::from_id_salt("fps")
                .selected_text(fps.to_string())
                .show_ui(ui, |ui| {
                    for f in [24u32, 25, 30, 60] {
                        ui.selectable_value(&mut fps, f, f.to_string());
                    }
                })
                .response;
            self.hint(r, tr(k::hint::FPS));
            if fps != before_fps {
                let mut st = proj.clone();
                st.fps = fps;
                self.apply(Command::SetSettings { settings: st });
            }
            ui.separator();
            let mut res = (proj.width, proj.height);
            let before_res = res;
            let res_resp = egui::ComboBox::from_id_salt("res")
                .selected_text(format!("{}×{}", res.0, res.1))
                .show_ui(ui, |ui| {
                    for r in
                        [(640u32, 360u32), (1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)]
                    {
                        ui.selectable_value(&mut res, r, format!("{}×{}", r.0, r.1));
                    }
                    // «Max (W×H)» — покомпонентный максимум по источникам (v1)
                    if let Some((mw, mh)) = self.max_source_resolution_cached() {
                        ui.selectable_value(&mut res, (mw, mh), format!("Max ({mw}×{mh})"));
                    }
                })
                .response;
            self.hint(res_resp, tr(k::hint::RESOLUTION));
            if res != before_res {
                let mut st = proj.clone();
                st.width = res.0;
                st.height = res.1;
                self.apply(Command::SetSettings { settings: st });
                // качество — доля разрешения: сохранить долю, а не пиксели
                // (раньше показанная доля молча съезжала, п.20 р.7)
                self.player.set_preview_quality(q_for(res.0, res.1, fracs[frac_idx]));
            }
            ui.separator();
            // движок питча — настройка ПРОЕКТА (Р.48): выбор один на весь
            // монтаж, поэтому он стоит рядом с fps и разрешением, а не в
            // параметрах эффекта
            let mut engine = proj.pitch_engine;
            let before_engine = engine;
            let eng_resp = egui::ComboBox::from_id_salt("pitch_engine")
                .selected_text(match engine {
                    kt_model::PitchEngine::Wsola => tr(k::fx::PITCH_WSOLA),
                    kt_model::PitchEngine::Vocoder => tr(k::fx::PITCH_VOCODER),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut engine,
                        kt_model::PitchEngine::Wsola,
                        tr(k::fx::PITCH_WSOLA),
                    );
                    ui.selectable_value(
                        &mut engine,
                        kt_model::PitchEngine::Vocoder,
                        tr(k::fx::PITCH_VOCODER),
                    );
                })
                .response;
            self.hint(eng_resp, tr(k::hint::PITCH_ENGINE));
            if engine != before_engine {
                let mut st = proj.clone();
                st.pitch_engine = engine;
                self.apply(Command::SetSettings { settings: st });
            }
            ui.separator();
            let labels = ["1/16", "1/8", "1/4", "1/2", "1/1"];
            let before = frac_idx;
            let q_resp = egui::ComboBox::from_id_salt("quality")
                .selected_text(labels[frac_idx])
                .show_ui(ui, |ui| {
                    for (i, l) in labels.iter().enumerate() {
                        ui.selectable_value(&mut frac_idx, i, *l);
                    }
                })
                .response;
            self.hint(q_resp, tr(k::hint::PREVIEW_QUALITY));
            if frac_idx != before {
                self.player.set_preview_quality(q_for(proj.width, proj.height, fracs[frac_idx]));
            }
            ui.separator();
            let sm = ui.checkbox(&mut self.smooth_scaling, tr(k::perf::SMOOTHING));
            self.hint(sm, tr(k::hint::SMOOTHING));
        });
    }

    fn draw_perf_box(&mut self, ui: &mut egui::Ui) {
        // длинные строки не должны задавать минимум ширины панели (п.18 р.6)
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        ui.label(egui::RichText::new(tr(k::perf::TITLE)).strong());
        ui.add_space(2.0);
        let ord = std::sync::atomic::Ordering::Relaxed;
        let q = self.player.preview_quality();
        // самая крупная ступень декода на монтаже (Р.46, п.322)
        let max_stage = self
            .player
            .video_decode_qualities()
            .into_iter()
            .map(|(_, dq)| (dq.width / q.width.max(1)).max(1))
            .max()
            .unwrap_or(1);
        // п.15 р.7: короткие метрики — двумя колонками (экономия высоты)
        let s = self.model.project().settings.clone();
        let rows: [(String, String); 4] = [
            (
                crate::tf!(
                    k::perf::CACHE_MB,
                    format!("{:.0}", self.player.cache.audio_bytes() as f64 / 1e6),
                    format!("{:.0}", self.player.cache.video_bytes() as f64 / 1e6)
                ),
                // ЧТО ЭТО ЗА ДВА ЧИСЛА (Р.43, п.289): левое — промахи
                // РЕНДЕРА (кэш не отдал бёрст), правое — голодание
                // КОЛБЭКА (кольцо пустое). Пользователь читал их как
                // «левый/правый канал» — и был вправе: подписи не было.
                crate::tf!(
                    k::perf::UNDERRUNS,
                    self.player.underruns.load(ord),
                    self.player.shared.callback_underruns.load(ord)
                ),
            ),
            (
                crate::tf!(k::perf::PREVIEW_RES, q.width, q.height),
                // ступень декода (Р.46): показывается, только когда она
                // есть — на обычном монтаже строка места не занимает
                if max_stage > 1 {
                    crate::tf!(k::perf::DECODE_SCALE, max_stage)
                } else {
                    format!("{} fps, {}×{}", s.fps, s.width, s.height)
                },
            ),
            (
                crate::tf!(k::perf::CLIPS_N, self.model.project().clips.len()),
                crate::tf!(k::perf::MEDIA_N, self.model.project().media.len()),
            ),
            (
                crate::tf!(k::perf::V_SHOWN, self.perf.video_frames_shown),
                // короткая подпись: в узкой колонке длинная строка
                // обрезалась многоточием и счётчик откатов было не видно
                crate::tf!(
                    k::perf::V_SKIPPED,
                    self.perf.video_cells_skipped,
                    self.perf.video_rollbacks
                ),
            ),
        ];
        // ui.columns делит ДОСТУПНУЮ ширину пополам и не требует минимума —
        // Grid с Extend раздувал панель по контенту и съедал превью
        // (регрессия п.3 р.7.1; тот же дефект, что чинил п.18 р.6)
        ui.columns(2, |cols| {
            for c in cols.iter_mut() {
                c.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                c.spacing_mut().item_spacing.y = 1.0;
            }
            for (a, b) in rows {
                cols[0].weak(a);
                cols[1].weak(b);
            }
        });
        if let Some(g) = &self.gpu_info {
            ui.weak(format!("GPU: {g}"));
        }
        // бэкенд видео-FX (Р.29): GPU по умолчанию, CPU — фолбэк с notice
        match self.player.video_backend() {
            kt_engine::VideoBackendStatus::Gpu(info) => {
                ui.weak(crate::tf!(k::perf::VIDEO_FX_GPU, info.name));
            }
            _ => {
                ui.weak(tr(k::perf::VIDEO_FX_CPU));
            }
        }
        let r = ui.checkbox(&mut self.ui_economy, tr(k::perf::ECONOMY));
        self.hint(r, tr(k::hint::ECONOMY));
        // «Чёрный ящик» воспроизведения (Р.28, п.10): кольцо событий
        // транспорта и счётчиков живёт всегда, кнопка выкладывает его в
        // файл — по нему ловят плавающие баги, не заставляя пользователя
        // ловить момент. Залипание видно прямо здесь.
        {
            let recs = self.player.flight.records();
            let stalls = recs.iter().filter(|r| r.kind == kt_engine::flight::Ev::Stall).count();
            let btn = ui.button(tr(k::perf::FLIGHT_SAVE));
            let btn = self.hint(btn, tr(k::hint::FLIGHT));
            if btn.clicked() {
                match self.save_flight_log() {
                    Ok(path) => self.note(crate::tf!(k::msg::FLIGHT_SAVED, path)),
                    Err(e) => self.warn(e.to_string()),
                }
            }
            if stalls > 0 {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 105, 97),
                    crate::tf!(k::perf::FLIGHT_STALLS, stalls),
                );
            }
        }
        // кумулятивы FFmpeg-слоя (порт строки avcpp open/read/vdec/adec v1)
        let (opens, reads) = kt_media::ffmpeg::io_stats();
        ui.weak(format!(
            "FFmpeg: open {opens} | read {reads} | vdec {} | adec {}",
            self.player.decode_stats.video_decodes.load(ord),
            self.player.decode_stats.audio_decodes.load(ord)
        ));
        ui.add_space(4.0);
        self.perf.quality.draw(ui, 96.0);
        ui.add_space(4.0);
        self.perf.decode.draw(ui, 96.0);
    }

    fn draw_statusbar(&mut self, ui: &mut egui::Ui) {
        // п.5 р.7: за статусбар окно ТАСКАЕТСЯ (виджеты поверх перехватят
        // свои клики; пустые зоны — драг)
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 22.0),
        );
        // прямоугольник строки состояния нужен в КОНЦЕ кадра: подсказка
        // рисуется там (п.1 р.9 — иначе статусбар видит hover_hint прошлого
        // кадра, точнее вообще ничего: он рисуется РАНЬШЕ всех кнопок)
        self.statusbar_rect = strip;
        let bg = ui.interact(strip, ui.id().with("status-drag"), egui::Sense::click_and_drag());
        if bg.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            Self::release_pointer_after_wm_grab(ui.ctx());
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let ord = std::sync::atomic::Ordering::Relaxed;
            ui.add_space(6.0); // п.6 р.7: колокольчик не липнет к краю
            // колокольчик уведомлений (п.18): бейдж = непрочитанные
            let bell = if self.notices_unread > 0 {
                format!("{} {}", crate::icons::BELL, self.notices_unread)
            } else {
                crate::icons::BELL.to_string()
            };
            let has_warn = self
                .notices
                .iter()
                .rev()
                .take(self.notices_unread)
                .any(|n| n.severity == Severity::Warn);
            let bell_btn = egui::Button::new(egui::RichText::new(bell).color(if has_warn {
                egui::Color32::from_rgb(255, 170, 80)
            } else if self.notices_unread > 0 {
                egui::Color32::from_rgb(160, 200, 255)
            } else {
                egui::Color32::from_gray(140)
            }))
            .frame(false);
            let bell_resp = ui.add(bell_btn);
            self.no_resize_rects.push(bell_resp.rect.expand(2.0));
            if self.hint(bell_resp, tr(k::win::NOTICES_TITLE)).clicked() {
                self.notices_open = !self.notices_open;
                if self.notices_open {
                    self.notices_unread = 0;
                }
            }
            ui.separator();
            // пп.21,24,25 р.7: «зум:» стёрт (значение с единицей само себя
            // объясняет), «слои» удалены, «(loop)» не пишем — горит кнопка
            ui.weak(format!("{:.0} px/s", 90_000.0 / self.ticks_per_px));
            if let Some(r) = self.model.project().region {
                ui.separator();
                ui.weak(crate::tf!(
                    k::perf::REGION_S,
                    format!("{:.2}", r.start.as_secs_f64()),
                    format!("{:.2}", r.end.as_secs_f64()),
                ));
            }
            // п.22 р.7: кэш/underruns после региона
            ui.separator();
            ui.weak(crate::tf!(
                k::perf::CACHE_SHORT,
                format!("{:.0}", self.player.cache.audio_bytes() as f64 / 1e6),
                format!("{:.0}", self.player.cache.video_bytes() as f64 / 1e6),
            ));
            ui.separator();
            // голодание колбэка — это КАДРЫ, которых не было на выходе;
            // в миллисекундах видно, сколько тишины набежало (Р.43 п.290)
            let cu = self.player.shared.callback_underruns.load(ord);
            ui.weak(crate::tf!(
                k::perf::UNDERRUNS_SHORT,
                self.player.underruns.load(ord),
                cu,
                cu / (kt_time::AUDIO_SAMPLE_RATE as u64 / 1000).max(1)
            ));
            ui.separator();
            // п.1 р.7.1: V-fps (показанные НОВЫЕ кадры) — здесь, не в тулбаре
            ui.weak(format!("{:.0} vfps", self.perf.vfps));
        });
    }

    /// Снять фокус с поля ввода, если нажатие пришлось ВНЕ его
    /// прямоугольника (Р.63 п.4). Прямоугольник берётся у самого egui
    /// (`read_response`) — своего реестра полей мы не заводим: он разошёлся
    /// бы с настоящей вёрсткой на первом же новом окне.
    fn surrender_focus_on_outside_click(&mut self, ctx: &egui::Context) {
        let Some(id) = ctx.memory(|m| m.focused()) else { return };
        let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) else { return };
        if !ctx.input(|i| i.pointer.primary_pressed() || i.pointer.secondary_pressed()) {
            return;
        }
        // нет прямоугольника (виджет исчез) — фокусу тем более не за что
        // держаться
        let inside = ctx.read_response(id).is_some_and(|r| r.rect.contains(pos));
        if !inside {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        // ГЕЙТ — ТЕКСТОВЫЙ ВВОД, а не «что-то в фокусе» (Р.63 п.4).
        //
        // `egui_wants_keyboard_input()` истинно, когда фокус есть у ЛЮБОГО
        // виджета, а в egui фокусируемы и кнопки, и галки, и ползунки.
        // Один клик по кнопке в окне эффектов — и все горячие клавиши
        // мертвы до закрытия окна: «как бы я не тыкал по остальной части
        // интерфейса, пробел не приводит к паузе… наверное по всем другим
        // полям и кнопкам такая же ситуация» (владелец, Р.63 п.4).
        // Клавиатуру у нас забирает только НАБОР ТЕКСТА.
        // Свои маски (таймкод в тулбаре, скорость на полосе) рисуются НЕ
        // `TextEdit`-ом, а полем на месте значения (правило 8), и egui о
        // них ничего не знает — называем их поимённо. Новое текстовое
        // поле обязано появиться в этом списке; сторожа — t98 и t627.
        if ctx.text_edit_focused() || self.tc_edit.is_some() || self.speed_entry.is_some() {
            return;
        }
        struct Keys {
            cut: bool,
            del: bool,
            undo: bool,
            redo: bool,
            save: bool,
            marker: bool,
            track_mute: bool,
            track_solo: bool,
            region_in: bool,
            region_out: bool,
            loop_toggle: bool,
            esc: bool,
            enter_nest: bool,
            copy: bool,
            paste: bool,
            nav_prev: bool,
            nav_next: bool,
            nav_far: bool,
            nav_region: bool,
            goto_tc: bool,
            find: bool,
        }
        // Пробел читаем СОБЫТИЯМИ, а не key_pressed: тот включает
        // автоповтор клавиатуры, и зажатый пробел слал серию play/pause
        // (п.3 р.16). Нажатие только запоминаем — решение принимается на
        // ОТПУСКАНИИ: короткое нажатие = play/pause, длинное = разгон.
        let (space_down, space_up) = ctx.input(|i| {
            let mut down = false;
            let mut up = false;
            for e in &i.events {
                if let egui::Event::Key { key: egui::Key::Space, pressed, repeat, .. } = e {
                    if *pressed {
                        down |= !*repeat;
                    } else {
                        up = true;
                    }
                }
            }
            (down, up)
        });
        if space_down && self.space_down_at.is_none() {
            self.space_down_at = Some(std::time::Instant::now());
        }
        if space_up {
            self.on_space_released();
        }
        let k = ctx.input(|i| Keys {
            cut: i.key_pressed(egui::Key::S) && !i.modifiers.ctrl,
            del: i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            undo: i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
            redo: i.modifiers.ctrl
                && (i.modifiers.shift && i.key_pressed(egui::Key::Z)
                    || i.key_pressed(egui::Key::Y)),
            save: i.modifiers.ctrl && i.key_pressed(egui::Key::S),
            marker: i.key_pressed(egui::Key::M) && !i.modifiers.ctrl && !i.modifiers.alt,
            // ДОРОЖКА (Р.62): голые M и S уже заняты меткой и разрезом —
            // оба поставлены прямой просьбой владельца и не двигаются.
            // Глушение и соло берут ALT, а действуют на дорожку
            // ВЫДЕЛЕННОГО события (иначе «выделенной дорожки» у нас нет).
            track_mute: i.modifiers.alt && i.key_pressed(egui::Key::M),
            track_solo: i.modifiers.alt && i.key_pressed(egui::Key::S),
            region_in: i.key_pressed(egui::Key::I) && !i.modifiers.ctrl && !i.modifiers.alt,
            region_out: i.key_pressed(egui::Key::O) && !i.modifiers.ctrl && !i.modifiers.alt,
            loop_toggle: i.key_pressed(egui::Key::L) && !i.modifiers.ctrl && !i.modifiers.alt,
            // Ctrl+C/Ctrl+V НЕ приходят как key_pressed: egui-winit
            // перехватывает их и шлёт Event::Copy / Event::Paste(..)
            // (регрессия п.21 — «Ctrl+V не работает»)
            esc: i.key_pressed(egui::Key::Escape),
            enter_nest: i.key_pressed(egui::Key::Enter) && !i.modifiers.any(),
            copy: i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::C)),
            paste: i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::V)),
            // п.16 р.8: Home/End — по точкам остановки, с Shift — в
            // самое начало/конец проекта
            // п.8 р.10: A/D — те же прыжки, что кнопки навигации.
            // «В самый конец» остаётся только за Shift+Home/End: Shift+D
            // не должен внезапно улетать в конец проекта.
            nav_prev: i.key_pressed(egui::Key::Home)
                || (i.key_pressed(egui::Key::A) && !i.modifiers.ctrl && !i.modifiers.alt),
            nav_next: i.key_pressed(egui::Key::End)
                || (i.key_pressed(egui::Key::D) && !i.modifiers.ctrl && !i.modifiers.alt),
            goto_tc: i.modifiers.ctrl && i.key_pressed(egui::Key::L),
            find: i.modifiers.ctrl && i.key_pressed(egui::Key::F),
            nav_far: i.modifiers.shift
                && (i.key_pressed(egui::Key::Home) || i.key_pressed(egui::Key::End)),
            // п.2 р.16: Shift+A/D — прыжок ТЯНЕТ край региона (Shift как
            // «расширить выделение» в проф. программах). У Home/End Shift
            // занят под «в самое начало/конец», поэтому смотрим A/D.
            nav_region: i.modifiers.shift
                && !i.modifiers.ctrl
                && !i.modifiers.alt
                && (i.key_pressed(egui::Key::A) || i.key_pressed(egui::Key::D)),
        });
        // п.9 р.11: работает и во время воспроизведения (seek на ходу
        // плеер умеет; гейт «только на паузе» был перестраховкой)
        if k.goto_tc {
            self.tc_edit = Some(String::new());
            self.tc_edit_focus = true;
        }
        if k.find {
            self.search_open = true;
            self.search_focus = true;
        }
        if k.nav_prev {
            self.nav_jump(false, k.nav_far, k.nav_region);
        }
        if k.nav_next {
            self.nav_jump(true, k.nav_far, k.nav_region);
        }
        if k.cut {
            self.cut_action();
        }
        if k.del {
            self.delete_action();
        }
        if k.undo {
            self.undo();
        }
        if k.redo {
            self.redo();
        }
        if k.save {
            self.save();
        }
        if k.marker {
            self.marker_toggle_at_cursor();
        }
        if k.track_mute
            && let Some(id) = self.key_target_track()
        {
            self.toggle_track_badge(id, crate::timeline::TrackBadge::Mute);
        }
        if k.track_solo
            && let Some(id) = self.key_target_track()
        {
            self.toggle_track_badge(id, crate::timeline::TrackBadge::Solo);
        }
        if k.region_in {
            self.set_region_edge_from_cursor(true);
        }
        if k.region_out {
            self.set_region_edge_from_cursor(false);
        }
        if k.loop_toggle {
            self.toggle_loop();
        }
        if k.esc {
            self.escape_action();
        }
        // Enter — КАСКАД по выделенному вложению (Р.56, п.451): первый
        // раскрывает на месте (режим по умолчанию, клавиатурный близнец
        // двойного клика), второй по уже раскрытому — ПОЛНЫЙ ВХОД.
        // Быстрый жест ведёт ВГЛУБЬ, а не переключает: назад — Esc,
        // который и так идёт каскадом в обратную сторону.
        if k.enter_nest
            && let Some(id) = self.selected.iter().copied().find(|id| self.is_nest(*id))
        {
            match self.enter_pressed_on_nest(id) {
                EnterOnNest::Expand => self.expand_in_place(id),
                EnterOnNest::Enter => self.enter_nest(id),
                EnterOnNest::NotANest => {}
            }
        }
        if k.copy {
            self.copy_selection();
        }
        if k.paste {
            self.paste_at_cursor();
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        // во время DnD-hover winit/X11 не шлёт движение мыши (координаты
        // XdndPosition выброшены в winit) — читаем указатель напрямую;
        // на Wayland вернётся None → честный fallback на playhead
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if hovering && let Some(p) = dnd_pointer_pos(ctx) {
            self.dnd_hover = Some(p);
        }
        if hovering {
            ctx.request_repaint(); // движение мыши при DnD не шлётся — опрос
        }
        // Позиция живёт, ПОКА идёт hover, и забирается ТОЛЬКО при дропе
        // (dnd_keep_hover, тест 47): прежний take() на каждом кадре стирал
        // позицию в том же кадре, где она ставилась, — к дропу всегда None
        // (дроп падал на playhead) и силуэт не рисовался вовсе.
        if !dnd_keep_hover(hovering, !dropped.is_empty()) {
            self.dnd_hover = None;
        }
        // позиция дропа нужна и полю источника морфера, и таймлайну:
        // берём её ДО того, как её заберёт расчёт цели на дорожке
        let drop_pos = if dropped.is_empty() { None } else { self.dnd_hover };
        let target = if dropped.is_empty() {
            None
        } else {
            self.dnd_hover.take().and_then(|pos| {
                let g = self.tl_geom.clone()?;
                if !g.in_lanes(pos) {
                    return None;
                }
                let lane = g.lane_at_y(pos.y)?;
                let at = self.snap(g.x_to_tick(pos.x).max(0), None);
                Some((Tick(at), lane))
            })
        };
        for path in dropped {
            if path.extension().map(|e| e == "ktproj").unwrap_or(false) {
                // Р.24 п.12: .ktproj как проект не открывается
                self.note(tr(k::msg::KTPROJ_IMPORT_ONLY));
            } else if path.extension().map(|e| e == "otio").unwrap_or(false) {
                // БРОШЕН ПРОЕКТ — СПРАШИВАЕМ (Р.55, С6). До этого раунда
                // здесь стояло `*self = app`: ни выбора «открыть или
                // вставить», ни вопроса о несохранённой работе, ни замка
                // на файле. Работа исчезала молча — в обход всего, что
                // построено в Р.49.
                self.pending_drop = Some((path.clone(), target));
                self.ask = Some(Ask::DropOtio);
            } else if let Some(id) = self.morph_editors.iter().find_map(|e| {
                let r = e.src_rect?;
                let pos = drop_pos?;
                (e.open && r.contains(pos)).then_some(e.id)
            }) {
                // файл брошен В ПОЛЕ ИСТОЧНИКА открытого морфера (Р.33
                // п.2): он и становится новым источником, а не новым
                // клипом на таймлайне
                self.set_morph_source_file(id, &path.to_string_lossy());
            } else if let Some((at, lane)) = target {
                self.add_media_clip_at(&path.to_string_lossy(), at, Some(lane));
            } else {
                self.add_media_clip(&path.to_string_lossy());
            }
        }
    }
}
/// Действие транспорт-машины Space (п.15 раунда 6, тест 58).
#[derive(Debug, PartialEq, Eq)]
pub enum TransportAction {
    Pause,
    Resume { seek: Option<i64> },
    SeekStart(i64),
}

/// Окно «двойного Space» = стоп (мс).
/// Интервал автосейва (п.4 р.11): было 4 с — постоянная запись всего
/// проекта в UI-потоке при любом чихе вроде движения региона.
pub const AUTOSAVE_SECS: u64 = 30;
pub const STATUS_TTL_MS: u64 = 6000;
pub const DOUBLE_PAUSE_MS: u128 = 400;
/// Сколько держать пробел, прежде чем это станет «разгоном», а не
/// play/pause (п.3 р.16). Заметно меньше окна двойного нажатия, поэтому
/// «стоп в начало региона» двумя быстрыми нажатиями не страдает.
pub const SPACE_HOLD_MS: u128 = 220;
/// Потолок разгона удержанием (ответ пользователя р.16: «до 4×»).
pub const SPACE_BOOST_MAX: f32 = 4.0;
/// За сколько секунд удержания разгон доезжает до потолка. 5 с по
/// прямой просьбе п.3 р.17 («надо не за 2с а за 5, а то очень быстро») —
/// НЕ ускорять без новой просьбы.
pub const SPACE_BOOST_SECS: f32 = 5.0;

/// Состояние перф-панели: rolling-графики, дельты счётчиков, журнал проблем.
/// Кэш coverage-опросов таймлайна (п.1).
#[derive(Default)]
pub struct CoverageCache {
    stamp: Option<(u64, u64, (u32, u32))>,
    /// Arc: снапшот на кадр — клон указателя, не карты (п.1 v1.4).
    pub per_clip: Arc<HashMap<ClipId, Vec<std::ops::Range<i64>>>>,
    /// Троттлинг при игре (п.1 раунда 5): epoch кэша меняется КАЖДОЙ
    /// вставкой → пересчёт шёл 60 раз/с с мьютексом кэша (контеншен с
    /// декодерами и аудио-рендером). Полоски освежаются не чаще 4 Гц.
    last_refresh: Option<std::time::Instant>,
}

/// Кэш имён клипов (п.1: HashMap'ы и O(n²) suppress пересобирались кадр).
/// Растяжения времени клипов, посчитанные раз на версию модели.
#[derive(Default)]
pub struct WarpCache {
    /// Ключ — (версия модели, число клипов): модель можно подменить
    /// ЦЕЛИКОМ, не бампнув `model_rev` (открытие проекта, харнесс-тесты),
    /// и тогда спасает второе число — та же страховка, что у `names_cache`.
    rev: Option<(u64, usize)>,
    /// Только клипы С варпом: у остальных отображение линейно, и хранить
    /// про них нечего.
    maps: Arc<HashMap<ClipId, kt_engine::ClipWarp>>,
}

/// Подпись медиа для пользователя: у файла — имя файла, у генератора —
/// ИМЯ ВИДА (Р.53, п.416). До этого пункта клип-генератор подписывался
/// сырым путём `gen:solid:c=ff8800`, который к тому же менялся на каждое
/// нажатие в окне параметров: подпись дёргалась под руками. Место одно —
/// его же зовёт карточка бина.
pub fn media_display_name(path: &str) -> String {
    if let Some(kd) = kt_model::parse_gen_path(path) {
        return crate::gen_ui::kind_name(&kd).to_string();
    }
    std::path::Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

#[derive(Default)]
pub struct NamesCache {
    rev: Option<u64>,
    /// Язык, на котором посчитаны имена (Р.53, п.416). Подпись
    /// клипа-генератора — ИМЯ ВИДА, то есть строка каталога; смена языка
    /// модель не трогает, и без этого штампа таймлайн до первой правки
    /// монтажа оставался бы на старом языке.
    lang: Option<crate::i18n::Lang>,
    /// Порядок отрисовки клипов (индексы в `project().clips`) — тот же
    /// back-to-front, что у композиции движка: задняя дорожка первой, а
    /// внутри дорожки раньше начавшийся раньше, чтобы поздний лёг СВЕРХУ
    /// (правило Vegas, п.177). Считается раз на версию модели: на кадр
    /// сортировать тысячи клипов нельзя (п.1 v1.4).
    pub draw_order: Arc<Vec<usize>>,
    /// Arc: снапшоты на кадр — клон указателя, не структур (п.1 v1.4).
    pub names: Arc<HashMap<ClipId, String>>,
    pub suppress: Arc<std::collections::HashSet<ClipId>>,
    pub media_paths: Arc<HashMap<MediaId, String>>,
    /// Зоны ГОЛЫХ наложений (Р.51, п.399): перекрытия пар одной
    /// (kind, lane), у которых нет перехода. Полоса метит их штриховкой —
    /// цель ПКМ-жеста «Переход с…» обязана быть видима (правило 16;
    /// минус «наложил и не заметил» из F-TL-010).
    pub overlaps: Arc<Vec<(ClipKind, u32, i64, i64)>>,
}

impl KtApp {
    /// Карты растяжения времени клипов (Р.37 п.3): превью считает позицию
    /// материала ТОЙ ЖЕ картой, что рендер. Пересчёт — раз на версию
    /// модели; на кадр отдаётся клон указателя.
    pub fn warp_maps(&mut self) -> Arc<HashMap<ClipId, kt_engine::ClipWarp>> {
        let stamp = (self.model_rev, self.model.project().clips.len());
        if self.warp_cache.rev != Some(stamp) {
            self.warp_cache.rev = Some(stamp);
            let maps: HashMap<ClipId, kt_engine::ClipWarp> = self
                .model
                .project()
                .clips
                .iter()
                .filter_map(|c| kt_engine::clip_warp_map(c).map(|w| (c.id, w)))
                .collect();
            self.warp_cache.maps = Arc::new(maps);
        }
        self.warp_cache.maps.clone()
    }

    /// Ключи превью по ЭКЗЕМПЛЯРАМ: `ClipId` → (лента, редакция)
    /// (Р.56 п.452, переписано в Р.59 стадии 3 — t680).
    ///
    /// Редакция ленты стала функцией ПАРЫ (лента, набор переопределений
    /// экземпляра), поэтому и ключ превью — по экземпляру: два выпуска с
    /// разными заголовками обязаны показывать на таймлайне СВОИ кадры, а
    /// не кадры шаблона. У экземпляра без переопределений
    /// `instance_content` РАВЕН `content_hash`, так что прежние ключи,
    /// дисковый ярус и все проекты без публикаций не двигаются ни на бит.
    ///
    /// Считается по `model_rev`, как карты варпа: хэш обходит всё
    /// содержимое ленты, и делать это на каждый UI-кадр незачем.
    pub fn reel_keys(&mut self) -> Arc<HashMap<ClipId, crate::reel_preview::ReelKey>> {
        if self.reel_hash_cache.0 != Some(self.model_rev) {
            let doc = self.model.document();
            let mut map: HashMap<ClipId, crate::reel_preview::ReelKey> = HashMap::new();
            // У ОДНОЙ ленты теперь НЕСКОЛЬКО живых редакций сразу, и
            // инвалидация обязана держать их все: выброси она «чужую»,
            // два экземпляра пересчитывали бы друг друга по кругу.
            let mut live: HashMap<MediaId, std::collections::HashSet<u64>> = HashMap::new();
            for s in &doc.sequences {
                live.entry(s.id).or_default();
            }
            for c in &self.model.project().clips {
                if !live.contains_key(&c.media) {
                    continue;
                }
                let hash = kt_engine::instance_content(&doc, c.media, &c.overrides);
                map.insert(c.id, crate::reel_preview::ReelKey { seq: c.media, hash });
                live.entry(c.media).or_default().insert(hash);
            }
            for (id, hs) in &live {
                self.reel_previews.invalidate_seq(*id, hs);
            }
            self.reel_hash_cache = (Some(self.model_rev), Arc::new(map));
        }
        self.reel_hash_cache.1.clone()
    }

    /// Заявка на превью ленты — ЕДИНСТВЕННЫЙ вход (зовётся из отрисовки).
    ///
    /// Ключ не зависит ни от зума, ни от прокрутки, поэтому скролл и зум
    /// заказов не порождают (инвариант 8): повторная заявка по тому же
    /// ключу отбрасывается дедупом.
    pub fn request_reel_preview(
        &mut self,
        key: crate::reel_preview::ReelKey,
        overrides: &[kt_model::Override],
    ) {
        if self.reel_previews.get(&key).is_some() {
            return;
        }
        let Some(mut p) = self.model.document().reel_as_root(key.seq) else { return };
        // Переопределения ВПЕКАЮТСЯ в документ, который считает превью:
        // ключ у нас уже по экземпляру, и посчитай мы шаблон — полоса
        // показала бы чужой заголовок под своим ключом (t680).
        // Правило одно на программу: набор ложится ДО морфер-слоёв, тем
        // же `apply_overrides`, что зовёт `resolve_reel`.
        if !overrides.is_empty() {
            // `swap_reel` уже положил набор ленты в `published` корня
            let published = p.published.clone();
            let frame_step = 90_000 / p.settings.fps.max(1) as i64;
            let mut clips = std::mem::take(&mut p.clips);
            let mut trans = std::mem::take(&mut p.transitions);
            let mut cams = std::mem::take(&mut p.cameras);
            kt_model::apply_overrides(
                &published,
                overrides,
                &mut clips,
                &mut trans,
                &mut cams,
                frame_step,
                &|m| p.default_gen_for(m),
            );
            p.clips = clips;
            p.transitions = trans;
            p.cameras = cams;
        }
        let base = self.project_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf());
        let db = self.index_db_path();
        self.reel_previews.request(key, p, db, base);
    }

    /// Runs покрытия per-clip; пересчёт только при смене epoch кэша,
    /// версии модели или качества превью.
    pub fn coverage_runs(&mut self) -> Arc<HashMap<ClipId, Vec<std::ops::Range<i64>>>> {
        use kt_time::AUDIO_BURST_FRAMES;
        let q = self.player.preview_quality();
        let stamp = (self.player.cache.epoch(), self.model_rev, (q.width, q.height));
        let throttled = self.player.transport.is_playing()
            && self
                .coverage_cache
                .last_refresh
                .map(|t| t.elapsed().as_millis() < 250)
                .unwrap_or(false);
        if self.coverage_cache.stamp != Some(stamp) && !throttled {
            self.coverage_cache.stamp = Some(stamp);
            self.coverage_cache.last_refresh = Some(std::time::Instant::now());
            // Качество ДЕКОДА клипа (Р.46, п.322): клип с зумящей цепочкой
            // греется крупнее Quality, и coverage по Quality показал бы
            // прогретый клип холодным. Морфер-слой мог нарезать клип на
            // куски с разными цепочками — берём объединение их качеств.
            let decode_q = self.player.video_decode_qualities();
            // Вложение: у ленты источника нет, и полоска спрашивает её
            // СОДЕРЖИМОЕ — пересечение прогретости детей (Р.50, Б6).
            let nest_cov: HashMap<ClipId, Vec<std::ops::Range<i64>>> =
                self.player.nest_coverage().into_iter().collect();
            let mut per_clip = HashMap::new();
            for c in &self.model.project().clips {
                // адрес ИСТОЧНИКА события (Р.59, стадия 2): у генератора
                // значения живут на событии, и греется его собственный
                // источник. Правило одно на программу — заказ, чтение и
                // полоска обязаны спрашивать `clip_source_addr` (инв. 4).
                let src = kt_engine::clip_source_addr(c);
                let runs = match c.kind {
                    ClipKind::Audio => {
                        let f0 = c.source_in.to_audio_frames();
                        let f1 = (c.source_in + c.duration).to_audio_frames();
                        self.player.cache.coverage(
                            kt_engine::source_id(src),
                            kt_cache::BurstKind::Audio,
                            f0.div_euclid(AUDIO_BURST_FRAMES as i64)
                                ..((f1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1),
                        )
                    }
                    ClipKind::Video if nest_cov.contains_key(&c.id) => {
                        nest_cov.get(&c.id).cloned().unwrap_or_default()
                    }
                    ClipKind::Video => {
                        let (b0, b1) = (c.source_in.0, c.source_in.0 + c.duration.0);
                        let mut qs: Vec<kt_media::Quality> = decode_q
                            .iter()
                            .filter(|(id, _)| kt_model::same_clip(*id, c.id))
                            .map(|(_, dq)| *dq)
                            .collect();
                        qs.sort();
                        qs.dedup();
                        if qs.is_empty() {
                            qs.push(q);
                        }
                        let mut runs: Vec<std::ops::Range<i64>> = Vec::new();
                        for dq in qs {
                            runs.extend(self.player.cache.coverage(
                                kt_engine::source_id(src),
                                kt_cache::BurstKind::video(Some(dq)),
                                b0..b1.max(b0 + 1),
                            ));
                        }
                        merge_runs(runs)
                    }
                };
                per_clip.insert(c.id, runs);
            }
            self.coverage_cache.per_clip = Arc::new(per_clip);
        }
        self.coverage_cache.per_clip.clone()
    }

    /// Имена клипов и множество «подавить имя» (партнёр пары) по model_rev.
    #[allow(clippy::type_complexity)]
    pub fn names(
        &mut self,
    ) -> (
        Arc<HashMap<ClipId, String>>,
        Arc<std::collections::HashSet<ClipId>>,
        Arc<HashMap<MediaId, String>>,
        Arc<Vec<usize>>,
    ) {
        // Сверка длины — страховка: порядок отрисовки теперь ВЛИЯЕТ НА
        // ХИТ-ТЕСТ (п.177), и устаревший кэш означал бы «клипов нет».
        // Модель можно подменить целиком, не бампнув model_rev (открытие
        // проекта, харнесс-тесты) — тогда спасает эта проверка.
        if self.names_cache.rev != Some(self.model_rev)
            || self.names_cache.lang != Some(crate::i18n::current())
            || self.names_cache.draw_order.len() != self.model.project().clips.len()
        {
            self.names_cache.rev = Some(self.model_rev);
            self.names_cache.lang = Some(crate::i18n::current());
            let mut names = HashMap::new();
            let mut suppress = std::collections::HashSet::new();
            let p = self.model.project();
            let media_names: HashMap<MediaId, String> =
                p.media.iter().map(|m| (m.id, media_display_name(&m.path))).collect();
            for c in &p.clips {
                if let Some(n) = media_names.get(&c.media) {
                    names.insert(c.id, n.clone());
                }
                // подавить имя у АУДИО-половины A/V-пары (рисуется на видео)
                if c.kind == ClipKind::Audio
                    && let Some(g) = c.link_group
                    && p.clips.iter().any(|o| {
                        o.kind == ClipKind::Video && o.link_group == Some(g) && o.media == c.media
                    })
                {
                    suppress.insert(c.id);
                }
            }
            self.names_cache.names = Arc::new(names);
            self.names_cache.suppress = Arc::new(suppress);
            self.names_cache.media_paths =
                Arc::new(p.media.iter().map(|m| (m.id, m.path.clone())).collect());
            let mut order: Vec<usize> = (0..p.clips.len()).collect();
            order.sort_by_key(|&i| {
                let c = &p.clips[i];
                (std::cmp::Reverse(c.lane), c.start.0)
            });
            self.names_cache.draw_order = Arc::new(order);
            // голые наложения: сортировка по дорожке + скан соседей,
            // O(n log n + пересечения) — раз на версию модели
            let mut sorted: Vec<&ClipEvent> = p.clips.iter().collect();
            sorted.sort_by_key(|c| (c.kind as u8, c.lane, c.start.0));
            let mut ov: Vec<(ClipKind, u32, i64, i64)> = Vec::new();
            for (i, a) in sorted.iter().enumerate() {
                let a_end = a.start.0 + a.duration.0;
                for b in sorted[i + 1..]
                    .iter()
                    .take_while(|b| b.kind == a.kind && b.lane == a.lane && b.start.0 < a_end)
                {
                    let covered = p.transitions.iter().any(|t| {
                        (t.a, t.b) == (Some(a.id), Some(b.id))
                            || (t.a, t.b) == (Some(b.id), Some(a.id))
                    });
                    if !covered {
                        ov.push((a.kind, a.lane, b.start.0, a_end.min(b.start.0 + b.duration.0)));
                    }
                }
            }
            self.names_cache.overlaps = Arc::new(ov);
        }
        (
            self.names_cache.names.clone(),
            self.names_cache.suppress.clone(),
            self.names_cache.media_paths.clone(),
            self.names_cache.draw_order.clone(),
        )
    }

    /// Сколько РАЗ каждое медиа стоит на монтаже (Р.53, п.416). Считается
    /// раз на ревизию модели: у бина карточек десятки, у монтажа событий
    /// тысячи, и опрос `Project::sequence_uses` на карточку в кадр — это
    /// линейный обход всего документа умноженный на число карточек
    /// (инвариант 12).
    pub fn media_uses(&mut self) -> Arc<HashMap<MediaId, usize>> {
        if self.uses_cache.0 != Some(self.model_rev) {
            let p = self.model.project();
            let mut m: HashMap<MediaId, usize> = HashMap::new();
            for c in p.clips.iter().chain(p.sequences.iter().flat_map(|s| s.clips.iter())) {
                *m.entry(c.media).or_default() += 1;
            }
            self.uses_cache = (Some(self.model_rev), Arc::new(m));
        }
        self.uses_cache.1.clone()
    }

    /// Зоны голых наложений (Р.51). Свежесть обеспечивает `names()` —
    /// звать после него в том же кадре.
    pub fn overlap_zones(&self) -> Arc<Vec<(ClipKind, u32, i64, i64)>> {
        self.names_cache.overlaps.clone()
    }
}

/// Задержка следующей перерисовки: 60 fps только когда есть что
/// анимировать, в простое — редкий тик (статусбар/watchdog).
/// Чистая функция — тест 20-repaint (п.1).
pub fn repaint_delay(animating: bool) -> std::time::Duration {
    if animating {
        std::time::Duration::from_millis(16)
    } else {
        std::time::Duration::from_millis(250)
    }
}

/// Позиция мыши в координатах окна (points) во время DnD файла.
/// winit 0.30/X11 выбрасывает координаты XdndPosition и не шлёт движение
/// мыши при перетаскивании файла (п.2 v1.4) — опрашиваем X-сервер напрямую
/// (x11rb). На Wayland/без X11 → None (fallback: дроп на playhead).
#[cfg(target_os = "linux")]
fn dnd_pointer_pos(ctx: &egui::Context) -> Option<egui::Pos2> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;
    static X11: std::sync::OnceLock<Option<(x11rb::rust_connection::RustConnection, u32)>> =
        std::sync::OnceLock::new();
    let conn = X11
        .get_or_init(|| {
            x11rb::connect(None).ok().map(|(c, screen)| {
                let root = c.setup().roots[screen].root;
                (c, root)
            })
        })
        .as_ref()?;
    let reply = conn.0.query_pointer(conn.1).ok()?.reply().ok()?;
    let inner = ctx.input(|i| i.viewport().inner_rect)?;
    let ppp = ctx.pixels_per_point();
    let p = egui::pos2(
        reply.root_x as f32 / ppp - inner.min.x,
        reply.root_y as f32 / ppp - inner.min.y,
    );
    (p.x >= 0.0 && p.y >= 0.0).then_some(p)
}

#[cfg(not(target_os = "linux"))]
fn dnd_pointer_pos(_ctx: &egui::Context) -> Option<egui::Pos2> {
    None
}

/// Жизнь dnd-позиции (п.2 v1.4, тест 47): держится при hover'е И на кадре
/// дропа (hovered_files к этому кадру уже пуст); гаснет только когда нет
/// ни того, ни другого.
pub fn dnd_keep_hover(hovering: bool, dropping: bool) -> bool {
    hovering || dropping
}

/// Зона ресайза borderless-окна по позиции у края (п.22 р.6, тест 64):
/// углы приоритетнее рёбер, центр → None.
/// Зона ресайза окна: ДВА радиуса (пп.11-13 р.8) — тонкая кромка и
/// заметно более крупный угол, иначе в углы невозможно попасть мышью.
pub fn resize_zone(
    win: egui::Rect,
    pos: egui::Pos2,
    edge: f32,
    corner: f32,
) -> Option<egui::ResizeDirection> {
    use egui::ResizeDirection::*;
    if !win.contains(pos) {
        return None;
    }
    let (dl, dr) = (pos.x - win.left(), win.right() - pos.x);
    let (dt, db) = (pos.y - win.top(), win.bottom() - pos.y);
    // углы приоритетнее рёбер и «ловятся» на большем расстоянии
    let (cl, cr, ct, cb) = (dl <= corner, dr <= corner, dt <= corner, db <= corner);
    if cl && ct {
        return Some(NorthWest);
    }
    if cr && ct {
        return Some(NorthEast);
    }
    if cl && cb {
        return Some(SouthWest);
    }
    if cr && cb {
        return Some(SouthEast);
    }
    Some(match (dl <= edge, dr <= edge, dt <= edge, db <= edge) {
        (true, ..) => West,
        (_, true, ..) => East,
        (_, _, true, _) => North,
        (_, _, _, true) => South,
        _ => return None,
    })
}

/// Курсор для направления ресайза.
pub fn resize_cursor(dir: egui::ResizeDirection) -> egui::CursorIcon {
    use egui::ResizeDirection::*;
    match dir {
        North | South => egui::CursorIcon::ResizeVertical,
        East | West => egui::CursorIcon::ResizeHorizontal,
        NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
        NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
    }
}

/// Ширина «хватаемой» кромки окна и радиус угла (п.12 р.8: 4 px требовали
/// пиксельного прицеливания).
/// Высота невидимой полосы перетаскивания окна вдоль верхней кромки
/// (п.2 р.9). Места не занимает — просто первые пиксели центральной зоны.
pub const TOP_DRAG_H: f32 = 20.0;
/// Самая кромка окна: тут ресайз выигрывает даже у кнопок, чтобы угол
/// окна оставался «тянущимся» (п.9 р.12).
/// Панель метров: пределы ширины и своя полоса перетаскивания СЛЕВА от
/// панели (п.6 р.13). Максимум поставлен по просьбе п.2 р.12 — не менять
/// без новой просьбы пользователя.
pub const METERS_W_MIN: f32 = 64.0;
pub const METERS_W_MAX: f32 = 90.0;
pub const METERS_SEP_W: f32 = 10.0;
pub const WIN_BORDER: f32 = 4.0;
pub const WIN_EDGE: f32 = 8.0;
pub const WIN_CORNER: f32 = 16.0;

/// State-машина деатача превью (п.7 v1.4, тест 51): закрытие окна всегда
/// возвращает превью; иначе кнопка переключает.
/// Что показывать в правом низу строки состояния (п.14 р.8). Приоритет:
/// наведение → активный режим → временное сообщение → дежурная строка.
/// Всё в одном месте: ни попапов, ни оверлеев поверх таймлайна.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintKind {
    /// Действие невозможно: красный, перебивает всё (п.5 Р.28).
    Error,
    Hover,
    Mode,
    Status,
    Idle,
}

/// Записать подсказку в накопитель кадра там, где `&mut self` недоступен
/// (внутри ui-замыканий панелей). Попапа НЕ создаёт (п.14 р.8).
pub fn hint_into(out: &mut Option<String>, r: egui::Response, text: &str) -> egui::Response {
    if r.hovered() {
        *out = Some(text.to_string());
    }
    r
}

pub fn status_line(
    error: Option<&str>,
    hover: Option<&str>,
    mode: Option<&str>,
    status: Option<(&str, u64)>, // (текст, возраст в мс)
    status_ttl_ms: u64,
    idle: &str,
) -> (String, HintKind) {
    // «везде в подсказках такие ситуации делаются как ошибочные» (п.5):
    // невозможность действия важнее любого hover'а и любого статуса
    if let Some(e) = error {
        return (e.to_string(), HintKind::Error);
    }
    if let Some(h) = hover {
        return (h.to_string(), HintKind::Hover);
    }
    if let Some(m) = mode {
        return (m.to_string(), HintKind::Mode);
    }
    if let Some((s, age)) = status
        && age < status_ttl_ms
    {
        return (s.to_string(), HintKind::Status);
    }
    (idle.to_string(), HintKind::Idle)
}

/// Отложенный одиночный клик по превью (пп.10-11 р.7): одиночный =
/// play/pause (ждёт окно double-click), двойной = detach/attach.
/// Чистая машина — тест 74.
#[derive(Debug, PartialEq)]
pub enum PreviewClick {
    None,
    TogglePlay,
    ToggleDetach,
}

#[derive(Debug, PartialEq)]
pub enum PendingOp {
    Keep,
    Arm,
    Clear,
}

pub const PREVIEW_CLICK_DELAY_MS: u128 = 300;

pub fn preview_click_step(
    pending_ms: Option<u128>,
    clicked: bool,
    double_clicked: bool,
    delay_ms: u128,
) -> (PreviewClick, PendingOp) {
    if double_clicked {
        // двойной отменяет отложенный одиночный (play не дёргается)
        return (PreviewClick::ToggleDetach, PendingOp::Clear);
    }
    if clicked {
        return (PreviewClick::None, PendingOp::Arm);
    }
    match pending_ms {
        Some(ms) if ms >= delay_ms => (PreviewClick::TogglePlay, PendingOp::Clear),
        _ => (PreviewClick::None, PendingOp::Keep),
    }
}

pub fn preview_detached_next(detached: bool, toggle: bool, close_requested: bool) -> bool {
    if close_requested {
        false
    } else if toggle {
        !detached
    } else {
        detached
    }
}

// ---- шкала скорости транспорта (Р.57, п.455) ----------------------------
// ТРИ разных «0.25» жили в программе и совпадение чисел приняли за правило:
// край мёртвой зоны слайдера, `RATE_MIN` тут же и `RATE_CLAMP` карты варпа.
// Решения владельца за пределом 0.25 нет — есть только предел ДВИЖКА
// (|rate| >= 0.001 в callback_step). Поэтому шкала идёт до 0.01, а ровно
// нулём отдаёт лишь узкая зона в 1.5 px вокруг центра.
/// Доля хода слайдера вокруг центра, отдающая РОВНО 0 (стоп-кадр).
/// 0.018 · 85 px ≈ 1.5 px — «узкая зона» из решения владельца.
pub const RATE_STOP: f32 = 0.018;
/// Наименьшая представимая скорость: ниже неё звук уже не тянется
/// (`kt_model::warp::RATE_AUDIO_FLOOR`), и шкале незачем идти дальше.
pub const RATE_FLOOR: f32 = 0.01;
/// Колено: до него ход ЛИНЕЕН по скорости (иначе 0.01…0.25 сжимается в
/// пиксель), после — логарифмичен по модулю.
pub const RATE_KNEE: f32 = 0.25;
/// Доля хода до колена.
pub const RATE_KNEE_T: f32 = 0.30;
/// |rate| на краю слайдера (дальше — только «второе дыхание» overshoot'а).
pub const RATE_MAX: f32 = 2.0;
/// Во сколько раз Shift замедляет ход при драге полосы.
pub const SPEED_FINE_DIV: f32 = 8.0;

/// Цвет «сделать нельзя» (правило 16): строка состояния и подчёркивание
/// поля ввода с нечитаемым значением обязаны говорить об ошибке ОДИНАКОВО.
pub const HINT_ERROR: egui::Color32 = egui::Color32::from_rgb(255, 105, 97);

/// Ввод скорости числом (Р.57). Живёт В ТОМ ЖЕ прямоугольнике 170×20, что
/// и полоса, — высота виджета поставлена просьбой владельца (п.18 р.7) и не
/// меняется; своего окна у ввода нет (правило 3 «ничего не скачет»).
#[derive(Debug, Clone, Default)]
pub struct SpeedEntry {
    /// Напечатанное. Пусто — показываем текущую скорость приглушённо:
    /// первая же цифра заменяет её целиком (то же «выделено всё»).
    pub text: String,
    /// Взять фокус на ближайшем кадре.
    pub focus: bool,
    /// Напечатанное не читается — подчёркивание в ошибочном стиле
    /// (правило 16: невозможность показывается ОШИБКОЙ), поле открыто.
    pub bad: bool,
}

/// Иконка окна (п.6 р.9): без неё таскбар показывал заглушку. Растр —
/// тот же силуэт, что рисуется в шапке и лежит в packaging/.
fn app_icon() -> egui::IconData {
    let img = image::load_from_memory_with_format(
        include_bytes!("../assets/kinotrain-256.png"),
        image::ImageFormat::Png,
    )
    .expect("application icon")
    .to_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData { rgba: img.into_raw(), width, height }
}

/// Что нашли поиском по проекту (п.3 р.10).
#[derive(Debug, Clone, PartialEq)]
pub enum HitKind {
    Marker,
    Media,
    Track,
    Clip,
    /// Группа карточек бина (п.13 р.11).
    Group,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub kind: HitKind,
    pub label: String,
    /// Куда перейти (тики); для медиа — 0.
    pub at: i64,
    pub media: Option<kt_model::MediaId>,
    pub clip: Option<kt_model::ClipId>,
}

/// Поиск по меткам, медиа, дорожкам и клипам. Чистая функция — ровно как
/// фильтр FX-реестра: тестируется без UI. Пустой запрос — пусто.
pub fn search_hits(
    p: &kt_model::Project,
    // группы живут в приложении и попадают в проект только при
    // сохранении — поиск обязан видеть ЖИВЫЕ (п.13 р.11)
    groups: &[kt_model::BinGroup],
    query: &str,
) -> Vec<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(u8, Hit)> = Vec::new();
    // ранг 0 — точное совпадение, 1 — начинается с запроса, 2 — подстрока
    let rank = |s: &str| -> Option<u8> {
        let l = s.to_lowercase();
        if l == q {
            Some(0)
        } else if l.starts_with(&q) {
            Some(1)
        } else if l.contains(&q) {
            Some(2)
        } else {
            None
        }
    };
    for m in &p.markers {
        let label = if m.name.is_empty() {
            crate::tf!(k::search::MARKER_AT, format!("{:.2}", m.t.as_secs_f64()))
        } else {
            m.name.clone()
        };
        if let Some(r) = rank(&label) {
            out.push((r, Hit { kind: HitKind::Marker, label, at: m.t.0, media: None, clip: None }));
        }
    }
    for g in groups {
        if g.name.is_empty() {
            continue;
        }
        if let Some(r) = rank(&g.name) {
            out.push((
                r,
                Hit {
                    kind: HitKind::Group,
                    label: format!("{} ({})", g.name, g.members.len()),
                    at: 0,
                    media: g.members.first().copied(),
                    clip: None,
                },
            ));
        }
    }
    for m in &p.media {
        let name = std::path::Path::new(&m.path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| m.path.clone());
        let r = rank(&name).or_else(|| rank(&m.path));
        if let Some(r) = r {
            out.push((
                r,
                Hit { kind: HitKind::Media, label: name, at: 0, media: Some(m.id), clip: None },
            ));
        }
    }
    let (v, a) =
        (p.lanes(kt_model::LaneKind::Video).max(1), p.lanes(kt_model::LaneKind::Audio).max(1));
    for (kind, count) in [
        (kt_model::LaneKind::Video, v),
        (kt_model::LaneKind::Audio, a),
        (kt_model::LaneKind::Morph, p.lanes(kt_model::LaneKind::Morph)),
    ] {
        for lane in 0..count {
            let label = p.track_label(kind, lane);
            if let Some(r) = rank(&label) {
                let at =
                    p.clips.iter().filter(|c| c.lane == lane).map(|c| c.start.0).min().unwrap_or(0);
                out.push((r, Hit { kind: HitKind::Track, label, at, media: None, clip: None }));
            }
        }
    }
    for c in &p.clips {
        let name = p
            .media
            .iter()
            .find(|m| m.id == c.media)
            .map(|m| {
                std::path::Path::new(&m.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| m.path.clone())
            })
            .unwrap_or_default();
        if let Some(r) = rank(&name) {
            out.push((
                r + 1, // клипы ниже медиа с тем же именем
                Hit {
                    kind: HitKind::Clip,
                    label: crate::tf!(
                        k::search::CLIP_AT,
                        name,
                        format!("{:.2}", c.start.as_secs_f64())
                    ),
                    at: c.start.0,
                    media: Some(c.media),
                    clip: Some(c.id),
                },
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.at.cmp(&b.1.at)));
    out.into_iter().map(|(_, h)| h).collect()
}

/// Инлайн-ввод имени метки (п.1 р.10).
#[derive(Debug, Clone)]
pub struct MarkerEdit {
    pub at: i64,
    pub buf: String,
    /// Метка только что создана: закрытие без Enter оставляет её без имени.
    pub is_new: bool,
}

/// Инлайн-ввод имени дорожки (п.2 р.10).
#[derive(Debug, Clone)]
pub struct LaneEdit {
    pub kind: kt_model::LaneKind,
    pub lane: u32,
    pub buf: String,
}

/// Переименование именованной области (Р.38 п.1). Отдельно от `LaneEdit`:
/// у области ключ — ИМЯ, и старое имя надо помнить, чтобы команда знала,
/// что именно переименовывают.
#[derive(Clone)]
pub struct AreaEdit {
    pub name: String,
    pub buf: String,
}

/// Точки остановки для навигации (п.16 р.8): ноль, границы региона, все
/// маркеры, границы ОБЪЕДИНЁННЫХ клипов (там, где пустота сменяется
/// материалом) и конец проекта. Отсортированы, без дублей. Строится
/// только по нажатию, не каждый кадр.
pub fn stop_points(p: &kt_model::Project) -> Vec<i64> {
    let mut v: Vec<i64> = vec![0];
    if let Some(r) = p.region {
        v.push(r.start.0);
        v.push(r.end.0);
    }
    v.extend(p.markers.iter().map(|m| m.t.0));
    // конец диапазонной метки — тоже точка остановки (Р.24)
    v.extend(p.markers.iter().filter(|m| m.is_range()).map(|m| m.end().0));
    // объединяем перекрывающиеся клипы: границей считается смена
    // «пусто ↔ есть материал», а не каждый стык внутри плотного монтажа
    let mut spans: Vec<(i64, i64)> =
        p.clips.iter().map(|c| (c.start.0, (c.start + c.duration).0)).collect();
    spans.sort_unstable();
    let mut merged: Vec<(i64, i64)> = Vec::with_capacity(spans.len());
    for (s, e) in spans {
        match merged.last_mut() {
            Some((_, pe)) if s <= *pe => *pe = (*pe).max(e),
            _ => merged.push((s, e)),
        }
    }
    for (s, e) in merged {
        v.push(s);
        v.push(e);
    }
    // Точки выравниваются к аудио-сетке (п.7 р.10): playhead хранится в
    // аудиокадрах, и round-trip «тик → кадр → тик» усекает вниз. Без
    // выравнивания курсор после прыжка вставал на тик ЛЕВЕЕ цели, и
    // «вперёд» бесконечно находил ту же самую точку.
    let mut v: Vec<i64> = v.into_iter().map(|t| Tick(t).align_audio_up().0).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Ближайшая точка остановки СТРОГО левее курсора. Строгое неравенство
/// само даёт «повторное нажатие — следующая точка», без хранения
/// направления и без залипания на текущей позиции.
pub fn prev_stop(cursor: i64, points: &[i64]) -> Option<i64> {
    points.iter().copied().filter(|&p| p < cursor).max()
}

/// Ближайшая точка остановки строго правее курсора.
pub fn next_stop(cursor: i64, points: &[i64]) -> Option<i64> {
    points.iter().copied().filter(|&p| p > cursor).min()
}

/// «Второе дыхание» шаттла (п.4 р.8): если во время драга мышь ушла за
/// пределы виджета, скорость растёт дальше края шкалы — до ±8×. Виджет
/// визуально не меняется. `over_px` — на сколько курсор вышел за край
/// (>=0), `k` — сколько пикселей даёт удвоение.
pub fn overshoot_rate(base: f32, over_px: f32, k: f32) -> f32 {
    if over_px <= 0.0 || base == 0.0 {
        return base;
    }
    let mult = 2f32.powf(over_px / k.max(1.0));
    let out = base.abs() * mult;
    out.min(ENGINE_RATE_MAX) * base.signum()
}

/// Потолок скорости движка (кольцо и упреждение рассчитаны под него).
pub const ENGINE_RATE_MAX: f32 = kt_engine::player::ENGINE_RATE_MAX;
/// Пикселей ухода за виджет на удвоение скорости.
pub const OVERSHOOT_PX_PER_DOUBLE: f32 = 60.0;

/// Слайдер [−1..1] → скорость [−2..2] (Р.57, тест 46).
///
/// Линейно `RATE_FLOOR → RATE_KNEE` на первых `RATE_KNEE_T` хода, дальше
/// логарифм `RATE_KNEE → RATE_MAX`. Ровно 0 отдаёт только `|t| < RATE_STOP`.
/// До Р.57 край мёртвой зоны в 7 % хода сразу равнялся 0.25×, и интервал
/// (0, 0.25) был непредставим ПО ПОСТРОЕНИЮ — жалоба владельца «должен
/// иметь возможность выставить 0,05». Симметрично для реверса.
pub fn slider_to_rate(t: f32) -> f32 {
    let a = t.abs();
    if a < RATE_STOP {
        return 0.0;
    }
    let r = if a < RATE_KNEE_T {
        RATE_FLOOR + (RATE_KNEE - RATE_FLOOR) * (a - RATE_STOP) / (RATE_KNEE_T - RATE_STOP)
    } else {
        let x = ((a - RATE_KNEE_T) / (1.0 - RATE_KNEE_T)).clamp(0.0, 1.0);
        RATE_KNEE * (RATE_MAX / RATE_KNEE).powf(x)
    };
    r.copysign(t)
}

/// Обратное отображение (тест 46): rate → позиция слайдера. |rate| > 2
/// садится на край шкалы — метка остаётся видимой (как и до Р.57).
pub fn rate_to_slider(rate: f32) -> f32 {
    let a = rate.abs();
    if a < RATE_FLOOR * 0.5 {
        return 0.0;
    }
    let t = if a < RATE_KNEE {
        RATE_STOP
            + (a.max(RATE_FLOOR) - RATE_FLOOR) / (RATE_KNEE - RATE_FLOOR)
                * (RATE_KNEE_T - RATE_STOP)
    } else {
        let x = (a.min(RATE_MAX) / RATE_KNEE).ln() / (RATE_MAX / RATE_KNEE).ln();
        RATE_KNEE_T + x * (1.0 - RATE_KNEE_T)
    };
    t.copysign(rate)
}

/// Напечатанное (или пришедшее из кода) значение скорости → то, что имеет
/// право уехать в плеер: почти-ноль становится СТОП-кадром, остальное
/// клампится полом шкалы и потолком движка со знаком.
pub fn normalize_rate(v: f32) -> f32 {
    if !v.is_finite() || v.abs() < 0.005 {
        return 0.0;
    }
    v.abs().clamp(RATE_FLOOR, ENGINE_RATE_MAX).copysign(v)
}

/// Разбор напечатанной скорости. Терпит то, что человек реально печатает:
/// минус типографский («−»), запятую вместо точки, хвост «×»/«x»,
/// ведущий «+». Ничего не клампит — это делает `normalize_rate`.
pub fn parse_rate_text(s: &str) -> Option<f32> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '\u{2212}' | '\u{2013}' | '\u{2014}' => '-',
            ',' => '.',
            c => c,
        })
        .collect();
    let body = cleaned.trim_end_matches(['\u{00d7}', 'x', 'X']).trim_start_matches('+');
    if body.is_empty() {
        return None;
    }
    body.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// Ход виртуального курсора полосы скорости за кадр драга.
///
/// Копится именно ПРИРАЩЕНИЕ: делить абсолютную позиции указателя на
/// `SPEED_FINE_DIV` нельзя — Shift, нажатый посреди драга, мгновенно
/// утащил бы значение к центру.
pub fn drag_advance(vx: f32, dx: f32, fine: bool) -> f32 {
    vx + if fine { dx / SPEED_FINE_DIV } else { dx }
}

/// Пора ли донести разогнанную пробелом скорость до плеера. Гистерезис
/// ОТНОСИТЕЛЬНЫЙ: прежние абсолютные 0.02 при базе 0.05 — это 40 % базы,
/// то есть разгон не доезжал до движка вовсе. Каждый `set_playback_rate`
/// сбрасывает окно экстраполяции часов показа (п.14 р.14), поэтому порог
/// всё же нужен; нижний пол держит его от нуля на базе 0.01.
pub fn boost_needs_update(want: f32, cur: f32, base: f32) -> bool {
    (want - cur).abs() > (base.abs() * 0.02).max(0.0005)
}

pub fn format_rate(r: f32) -> String {
    if r == 0.0 { tr(k::perf::RATE_STOP).to_string() } else { format!("{r:+.2}×") }
}

/// Ключ кадра превью: тик, выровненный по сетке fps проекта (тест 39).
/// Размер превью по доле — ПОДОБНЫЙ кадру проекта (Р.64).
///
/// Независимое чётное округление осей врало: 1920×1080 при 1/8 давало
/// 240×134 (1.791), при 1/16 — 120×66 (1.818) вместо 1.778. «Вписать»
/// считает рамку ОТ ХОЛСТА, поэтому превью и экспорт получали разные
/// рамки, а экранный контрол показывал бы не то, что уедет в файл.
///
/// Чётность остаётся требованием декодера (4:2:0), поэтому точного подобия
/// на малых долях не существует — берём ближайшую ЧЁТНУЮ ПАРУ из
/// окрестности ±2 по каждой оси, минимизирующую ошибку отношения; при
/// равенстве — большую площадь.
pub fn preview_quality_for(w: u32, h: u32, f: f32) -> kt_media::Quality {
    let even = |v: f32| ((v.max(2.0) as u32).max(2)) & !1;
    let (w0, h0) = (even(w as f32 * f), even(h as f32 * f));
    let want = w.max(1) as f64 / h.max(1) as f64;
    let mut best = (w0, h0);
    let mut best_err = f64::MAX;
    for dw in [-2i64, 0, 2] {
        for dh in [-2i64, 0, 2] {
            let cw = (w0 as i64 + dw).max(2) as u32 & !1;
            let ch = (h0 as i64 + dh).max(2) as u32 & !1;
            let err = (cw as f64 / ch as f64 - want).abs();
            let better = err < best_err - 1e-12
                || ((err - best_err).abs() <= 1e-12
                    && (cw as u64 * ch as u64) > (best.0 as u64 * best.1 as u64));
            if better {
                best = (cw, ch);
                best_err = err;
            }
        }
    }
    kt_media::Quality { width: best.0, height: best.1 }
}

pub fn preview_key_tick(grid: kt_time::FpsGrid, t: Tick) -> i64 {
    grid.align_floor(t).0
}

pub struct PerfState {
    pub quality: crate::graphs::RollingGraph,
    pub decode: crate::graphs::RollingGraph,
    pub churn: crate::graphs::RollingGraph,
    last_sample: std::time::Instant,
    // прошлые кумулятивные значения для дельт
    last_cache: kt_cache::CacheStats,
    last_decode: (u64, u64),
    last_io: (u64, u64),
    last_underruns: (u64, u64),
    frames: u32,
    /// Кумулятив реально показанных НОВЫХ видеокадров (выровненный ключ).
    pub video_frames_shown: u64,
    /// Сглаженный V-fps последнего окна замера (п.23 р.7: показывается в
    /// тайм-коде тулбара).
    pub vfps: f32,
    last_video_shown: u64,
    /// Пропущенные ячейки сетки кадров при игре (диагностика джиттера).
    pub video_cells_skipped: u64,
    /// Откаты ключа показа назад (п.14 р.14): счётчик односторонним быть
    /// не должен — при медленной игре пропусков вперёд не бывает вовсе,
    /// и панель молчала ровно в том случае, на который жаловались.
    pub video_rollbacks: u64,
    pub diag_open: bool,
    pub log: std::collections::VecDeque<String>,
    /// Дедуп подряд идущих одинаковых сообщений: (текст, повторов).
    last_msg: Option<(String, u32)>,
    last_offline: usize,
    last_hit_warn: Option<std::time::Instant>,
}

impl PerfState {
    fn new() -> PerfState {
        use crate::graphs::{Axis, RollingGraph};
        PerfState {
            quality: RollingGraph::new(
                k::perf::GRAPH_QUALITY,
                Axis::Percent,
                vec![
                    ("FPS", egui::Color32::from_rgb(120, 230, 90), 120.0),
                    // честный видео-fps: показанные НОВЫЕ кадры (п.1 v1.4;
                    // «FPS» — это ui-кадры и о видео ничего не говорит)
                    ("V-fps", egui::Color32::from_rgb(255, 145, 80), 120.0),
                    ("A-hit", egui::Color32::from_rgb(114, 214, 255), 100.0),
                    ("V-hit", egui::Color32::from_rgb(240, 190, 70), 100.0),
                ],
            ),
            // FFmpeg-активность: порт «avcpp Activity» v1 (те же 5 серий
            // и цвета), но по ЖИВЫМ счётчикам kt-ffmpeg::iostats
            decode: RollingGraph::new(
                k::perf::GRAPH_DECODE,
                Axis::LinearAuto,
                vec![
                    ("Total", egui::Color32::from_rgb(255, 145, 80), 10_000.0),
                    ("Open", egui::Color32::from_rgb(255, 205, 105), 500.0),
                    ("Read", egui::Color32::from_rgb(244, 99, 99), 10_000.0),
                    ("V-dec", egui::Color32::from_rgb(95, 209, 155), 1500.0),
                    ("A-dec", egui::Color32::from_rgb(130, 173, 255), 1500.0),
                ],
            ),
            churn: RollingGraph::new(
                k::perf::GRAPH_CHURN,
                Axis::LinearAuto,
                vec![
                    ("evict", egui::Color32::from_rgb(255, 160, 120), 2000.0),
                    ("miss", egui::Color32::from_rgb(200, 160, 255), 2000.0),
                    // самая ценная метрика v1: вытеснено ДО первого чтения
                    ("no-read", egui::Color32::from_rgb(255, 90, 90), 2000.0),
                ],
            ),
            last_sample: std::time::Instant::now(),
            last_cache: Default::default(),
            last_decode: (0, 0),
            last_io: (0, 0),
            last_underruns: (0, 0),
            frames: 0,
            video_frames_shown: 0,
            vfps: 0.0,
            last_video_shown: 0,
            video_cells_skipped: 0,
            video_rollbacks: 0,
            diag_open: false,
            log: std::collections::VecDeque::new(),
            last_msg: None,
            last_offline: 0,
            last_hit_warn: None,
        }
    }

    fn note(&mut self, msg: String) {
        // подряд идущие одинаковые сообщения схлопываются в «×N»
        // (в v1 всплеск забивал весь лог за секунды)
        if let Some((last, n)) = &mut self.last_msg
            && *last == msg
        {
            *n += 1;
            if let Some(line) = self.log.back_mut() {
                *line = format!("[{}] {} ×{}", chrono_like_now(), msg, n);
            }
            return;
        }
        self.last_msg = Some((msg.clone(), 1));
        let now = chrono_like_now();
        self.log.push_back(format!("[{now}] {msg}"));
        while self.log.len() > 800 {
            self.log.pop_front();
        }
    }
}

/// hh:mm:ss без зависимости chrono.
/// Файл автосохранения безымянного проекта (порт v1
/// defaultAutosaveProjectPath: ~/.local/share/kinotrain/).
/// Найденная несохранённая работа: файл, время и проект, к которому он
/// относится (`None` — работа, у которой имени проекта не было).
#[derive(Clone, Debug)]
pub struct RecoveryCandidate {
    pub path: PathBuf,
    pub mtime: std::time::SystemTime,
    pub for_project: Option<PathBuf>,
}

/// Собрать всё, что программа не успела отдать пользователю (п.353).
///
/// Отбор по ВРЕМЕНИ, а не по факту существования. Сценарий, который до Р.49
/// не покрывался ничем: открыл проект A → «Новый» → час работы → падение.
/// `last_project` указывал на A, программа послушно открывала A, а про час
/// не говорила ни строки — потому что смотрела только на `last_project` и
/// только в момент, когда его не было.
///
/// Восстановление проекта предлагается, только если оно СТРОГО новее самого
/// проекта: иначе после обычного Ctrl+S программа предлагала бы «восстановить»
/// то, что и так лежит в файле. Безымянная работа сравнивать не с чем —
/// она предлагается всегда, пока пользователь её не заберёт или не удалит.
pub fn recovery_candidates(project: Option<&Path>) -> Vec<RecoveryCandidate> {
    recovery_candidates_at(project, &autosave_path())
}

/// То же с явно названным файлом безымянной работы. Шов ради теста: каталог
/// данных один на процесс, а тесты идут параллельно — подменять переменную
/// окружения значило бы ронять соседей.
pub fn recovery_candidates_at(project: Option<&Path>, anon: &Path) -> Vec<RecoveryCandidate> {
    let mut out: Vec<RecoveryCandidate> = Vec::new();
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();

    if let Some(proj) = project {
        let base = recovery_path_for(Some(proj));
        let stem = base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let proj_time = mtime(proj);
        if let Some(dir) = base.parent()
            && let Ok(rd) = std::fs::read_dir(dir)
        {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                // и сам `<имя>.recovery.otio`, и файлы вторых копий
                // `<имя>.recovery.otio.<pid>.otio`
                if !name.starts_with(&stem) || !name.ends_with(".otio") {
                    continue;
                }
                let path = e.path();
                let Some(t) = mtime(&path) else { continue };
                if proj_time.is_some_and(|pt| t <= pt) {
                    continue; // проект новее — восстанавливать нечего
                }
                out.push(RecoveryCandidate { path, mtime: t, for_project: Some(proj.into()) });
            }
        }
    }

    // безымянная работа — в каталоге данных
    let stem = anon.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(dir) = anon.parent()
        && let Ok(rd) = std::fs::read_dir(dir)
    {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with(&stem) || !name.ends_with(".otio") {
                continue;
            }
            let path = e.path();
            let Some(t) = mtime(&path) else { continue };
            out.push(RecoveryCandidate { path, mtime: t, for_project: None });
        }
    }

    // свежее — сверху: первым предлагается то, над чем работали последним
    out.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path)));
    out
}

/// Файл восстановления для этого проекта: `<полное имя>.recovery.otio`
/// РЯДОМ с проектом. Суффикс дописывается к ПОЛНОМУ имени, а не подменяет
/// расширение (урок t150: `film.otio` и `film.mp4.otio` обязаны дать разные
/// имена, иначе один проект затирает восстановление другого).
/// Безымянный проект восстанавливается в каталоге данных: рядом с ним
/// каталога ещё нет.
pub fn recovery_path_for(project: Option<&Path>) -> PathBuf {
    match project {
        Some(p) => {
            let mut name = p.file_name().unwrap_or_default().to_os_string();
            name.push(".recovery.otio");
            p.with_file_name(name)
        }
        None => autosave_path(),
    }
}

pub fn autosave_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("kinotrain").join("autosave-project.otio")
}

/// Запись проекта: сначала во временный файл рядом, потом переименование.
/// Прямая перезапись живого файла при обрыве питания/ошибке на середине
/// оставляла пользователя с обрезанным проектом.
/// Запись проекта ПОД СТОРОЖЕМ (Р.32): `rev` — ревизия модели, которую
/// несёт `json`. Если на диск уже лёг снимок НОВЕЕ (`*guard > rev`) —
/// не пишем вовсе и возвращаем `Ok(false)`: иначе фоновый автосейв
/// перетирал более свежий Ctrl+S своим устаревшим снапшотом, и правки
/// пользователя терялись (гонку нашли живые сценарии).
/// Чем кончилась попытка записи (Р.49, п.350).
///
/// Раньше сюда возвращался `bool`, и все трое зовущих делали
/// `.map(|_| ())` — «на диске уже новее, я не писал» становилось
/// неотличимо от «записал». Следствие у Ctrl+S: гасла звёздочка,
/// печаталось «saved», и УДАЛЯЛСЯ файл восстановления. Тип снимает эту
/// ветку: у трёх зовущих три разных ответа, и забыть один нельзя.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    /// Записали. `dir_synced == false` — файл на месте, но каталог
    /// сбросить не удалось (exFAT, сетевая шара, не-unix).
    Wrote { dir_synced: bool },
    /// На диске уже лежит более свежий снимок — не писали вовсе.
    SkippedNewerOnDisk,
}

pub fn write_project_guarded(
    guard: &std::sync::Mutex<u64>,
    path: &std::path::Path,
    rev: u64,
    json: &str,
) -> std::io::Result<SaveOutcome> {
    // Отравленный мьютекс — не повод не сохранять: паниковавший под ним
    // поток оставил лишь число (последнюю записанную ревизию), и оно цело
    // (п.354 — в `Drop` любая паника означала бы abort).
    let mut last = guard.lock().unwrap_or_else(|e| e.into_inner());
    if *last > rev {
        return Ok(SaveOutcome::SkippedNewerOnDisk);
    }
    let w = kt_otio::write_atomic(path, json)?;
    *last = rev;
    Ok(SaveOutcome::Wrote { dir_synced: w.dir_synced })
}

/// Тонкая обёртка над единственным домом атомарной записи (Р.49, п.349):
/// сама запись, fsync файла и каталога живут в kt-otio, потому что туда
/// же ходит kt-cli — второго писателя файла проекта в дереве нет.
pub fn write_project_atomic(path: &std::path::Path, json: &str) -> std::io::Result<()> {
    kt_otio::write_atomic(path, json).map(|_| ())
}

/// Чтение .otio с диска для open() (Р.24 п.12: .ktproj — только импорт).
pub(crate) fn load_project_file(
    path: &Path,
) -> Result<(Project, kt_otio::Retained, Vec<String>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    // Пустой файл — не «битый JSON», а известный исход потери питания до
    // Р.49: переименование легло в журнал раньше данных. Сырая строка
    // serde («EOF while parsing a value at line 1 column 0») об этом не
    // говорит ничего, а пользователю нужно понять, что искать рядом файл
    // восстановления, а не чинить синтаксис.
    if text.trim().is_empty() {
        return Err(crate::tf!(k::msg::PROJECT_FILE_EMPTY, path.display().to_string()));
    }
    match kt_otio::load_project(&text, path.parent()) {
        Ok(b) => Ok((b.project, b.retained, b.notices)),
        Err(kt_otio::Error::NotOtio) => Err(tr(k::msg::KTPROJ_IMPORT_ONLY).to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn chrono_like_now() -> String {
    let secs =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let s = secs % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

impl KtApp {
    /// Сэмплер метрик раз в 500 мс (порт performanceTimer).
    fn sample_perf(&mut self) {
        self.perf.frames += 1;
        let dt = self.perf.last_sample.elapsed().as_secs_f32();
        if dt < 0.5 {
            return;
        }
        self.perf.last_sample = std::time::Instant::now();
        let ord = std::sync::atomic::Ordering::Relaxed;

        // saturating: пересоздание плеера обнуляет счётчики нового кэша,
        // а perf.last_* хранит кумулятивы старого — голое вычитание давало
        // u64-wrap и «10870945131100372992 evictions/с» (п.24)
        let cache = self.player.cache.stats();
        let lc = self.perf.last_cache;
        let d_ah = cache.audio_hits.saturating_sub(lc.audio_hits);
        let d_am = cache.audio_misses.saturating_sub(lc.audio_misses);
        let d_vh = cache.video_hits.saturating_sub(lc.video_hits);
        let d_vm = cache.video_misses.saturating_sub(lc.video_misses);
        let d_ev = cache.evictions.saturating_sub(lc.evictions);
        self.perf.last_cache = cache;

        let a_dec = self.player.decode_stats.audio_decodes.load(ord);
        let v_dec = self.player.decode_stats.video_decodes.load(ord);
        let (la, lv) = self.perf.last_decode;
        self.perf.last_decode = (a_dec, v_dec);
        // FFmpeg-активность: opens (demux+декодеры, вкл. reopen на seek) и
        // прочитанные пакеты (порт avcpp open/read v1)
        let (opens, reads) = kt_media::ffmpeg::io_stats();
        let (lo, lr) = self.perf.last_io;
        self.perf.last_io = (opens, reads);

        let fps = self.perf.frames as f32 / dt;
        self.perf.frames = 0;
        let a_hit =
            if d_ah + d_am > 0 { d_ah as f32 / (d_ah + d_am) as f32 * 100.0 } else { 100.0 };
        let v_hit =
            if d_vh + d_vm > 0 { d_vh as f32 / (d_vh + d_vm) as f32 * 100.0 } else { 100.0 };
        let v_shown = self.perf.video_frames_shown;
        let v_fps = v_shown.saturating_sub(self.perf.last_video_shown) as f32 / dt;
        self.perf.last_video_shown = v_shown;
        self.perf.vfps = v_fps; // п.23 р.7: тайм-код тулбара
        self.perf.quality.push(&[fps.min(120.0), v_fps.min(120.0), a_hit, v_hit]);
        let (r_open, r_read) =
            (opens.saturating_sub(lo) as f32 / dt, reads.saturating_sub(lr) as f32 / dt);
        let (r_vdec, r_adec) =
            (v_dec.saturating_sub(lv) as f32 / dt, a_dec.saturating_sub(la) as f32 / dt);
        self.perf.decode.push(&[r_open + r_read + r_vdec + r_adec, r_open, r_read, r_vdec, r_adec]);
        let d_unused = cache.unused_evictions.saturating_sub(lc.unused_evictions);
        self.perf.churn.push(&[d_ev as f32 / dt, (d_am + d_vm) as f32 / dt, d_unused as f32 / dt]);

        // журнал проблем
        let ur = self.player.underruns.load(ord);
        let uc = self.player.shared.callback_underruns.load(ord);
        let (lur, luc) = self.perf.last_underruns;
        if ur > lur {
            self.perf.note(crate::tf!(k::perf::note::RENDER_UNDERRUNS, ur - lur, ur));
        }
        if uc > luc {
            self.perf.note(crate::tf!(k::perf::note::CALLBACK_UNDERRUNS, uc - luc, uc));
        }
        self.perf.last_underruns = (ur, uc);
        if d_ev as f32 / dt > 500.0 {
            self.perf.note(crate::tf!(
                k::perf::note::EVICTION_SPIKE,
                format!("{:.0}", d_ev as f32 / dt)
            ));
        }
        // порт главного предупреждения v1: кадры выселены ДО первого чтения
        if d_unused > 0 {
            self.perf.note(crate::tf!(k::perf::note::PREFETCH_THRASH, d_unused));
        }
        // порог (в v1 порогов не было вовсе): низкий видео-hit при игре
        if v_hit < 80.0
            && self.player.transport.is_playing()
            && self.perf.last_hit_warn.map(|t| t.elapsed().as_secs() >= 10).unwrap_or(true)
        {
            self.perf.last_hit_warn = Some(std::time::Instant::now());
            self.perf.note(crate::tf!(k::perf::note::LOW_VIDEO_HIT, format!("{v_hit:.0}")));
        }
        let offline_now = self.offline.lock().unwrap().len();
        if offline_now != self.perf.last_offline {
            self.perf.note(crate::tf!(k::perf::note::OFFLINE_CHANGED, offline_now));
            self.perf.last_offline = offline_now;
        }
    }

    fn draw_diagnostics(&mut self, ctx: &egui::Context) {
        if !self.perf.diag_open {
            return;
        }
        let mut open = self.perf.diag_open;
        egui::Window::new(tr(k::win::DIAG_TITLE))
            .id(egui::Id::new("kt_diag"))
            .open(&mut open)
            .default_size([560.0, 460.0])
            .min_size([420.0, 300.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                // занятость видео-кэша от НАСТОЯЩЕГО лимита (в v1 константа
                // лимита была продублирована руками и врала в 384 раза)
                let used = self.player.cache.video_bytes() as f64;
                let limit = self.player.cache.max_video_bytes() as f64;
                let occ = if limit > 0.0 { (used / limit * 100.0).min(100.0) } else { 0.0 };
                ui.horizontal(|ui| {
                    ui.weak(crate::tf!(
                        k::perf::CACHE_OCCUPANCY,
                        format!("{occ:.0}"),
                        format!("{:.0}", used / 1e6),
                        format!("{:.0}", limit / 1e6)
                    ));
                    let cache = self.player.cache.stats();
                    ui.weak(crate::tf!(k::perf::NO_READ_DROPS, cache.unused_evictions));
                });
                let (bar, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 6.0),
                    egui::Sense::hover(),
                );
                let pnt = ui.painter_at(bar);
                pnt.rect_filled(bar, 3.0, egui::Color32::from_gray(30));
                let fill_w = bar.width() * (occ as f32 / 100.0);
                let col = if occ > 90.0 {
                    egui::Color32::from_rgb(230, 120, 90)
                } else {
                    egui::Color32::from_rgb(95, 209, 155)
                };
                pnt.rect_filled(
                    egui::Rect::from_min_size(bar.min, egui::vec2(fill_w, bar.height())),
                    3.0,
                    col,
                );
                ui.add_space(6.0);
                self.perf.churn.draw(ui, 110.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr(k::perf::LOG_TITLE)).strong());
                    if ui.button(tr(k::perf::LOG_CLEAR)).clicked() {
                        self.perf.log.clear();
                        self.perf.last_msg = None;
                    }
                    if ui.button(tr(k::perf::GRAPH_RESET)).clicked() {
                        self.perf.quality.clear();
                        self.perf.decode.clear();
                        self.perf.churn.clear();
                    }
                });
                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    for line in &self.perf.log {
                        ui.monospace(line);
                    }
                    if self.perf.log.is_empty() {
                        ui.weak(tr(k::perf::LOG_EMPTY));
                    }
                });
            });
        self.perf.diag_open = open;
    }
}

impl eframe::App for KtApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Перехват закрытия окна (п.354). `CancelClose` в eframe 0.36
        // работает и для КОРНЕВОГО окна — значит вопрос «сохранить перед
        // выходом?» делается штатно, без своей обработки системных событий.
        if ui.ctx().input(|i| i.viewport().close_requested())
            && !self.quit_confirmed
            && !self.guard_unsaved(Pending::Quit)
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if self.quit_confirmed {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // детект софт-рендера — один раз (пп. 17, 19)
        if self.sw_render.is_none() {
            let sw = _frame
                .wgpu_render_state()
                .map(|rs| {
                    let info = rs.adapter.get_info();
                    self.gpu_info = Some(format!("{} / {:?}", info.name, info.backend));
                    let name = info.name.to_lowercase();
                    info.device_type == eframe::wgpu::DeviceType::Cpu
                        || name.contains("llvmpipe")
                        || name.contains("softpipe")
                        || name.contains("swiftshader")
                })
                .unwrap_or(false);
            self.sw_render = Some(sw);
            if sw {
                self.warn(tr(k::msg::SOFTWARE_RENDER));
            }
            // Р.29: видео-FX и композит на GPU по умолчанию — движок на
            // устройстве eframe. Софт-рендер (llvmpipe) — тоже «GPU есть».
            self.init_video_gpu(_frame);
        }

        // Р.31 п.2: долгие кадры UI-потока — в перф-журнал. Меряется
        // ДЛИТЕЛЬНОСТЬ кадра (не промежуток между кадрами — idle-repaint
        // легально редкий): блокировку внутри кадра (запись на диск,
        // декод в UI-потоке) видно здесь с точной меткой времени.
        let t0 = std::time::Instant::now();
        self.draw_all(ui);
        let ms = t0.elapsed().as_millis();
        if ms >= 100 {
            self.perf.note(crate::tf!(k::perf::note::UI_STALL, ms));
        }
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Р.31 п.2: eframe зовёт save() КАЖДЫЕ 30 секунд (auto_save_interval)
        // — здесь остаётся только дешёвое (строки egui-storage). Прежний
        // синхронный сейв проекта отсюда ронял FPS в ноль каждые 30 с:
        // «moved» (курсор/вьюпорт сдвинулся) — почти всегда true, и весь
        // проект сериализовался и писался В UI-ПОТОКЕ даже без правок.
        // Периодику несёт наш ФОНОВЫЙ автосейв (draw_all), финальную
        // запись при выходе — on_exit ниже.
        storage.set_string("lang", crate::i18n::choice_code(self.lang_choice).to_string());
        if let Some(p) = &self.project_path {
            storage.set_string("last_project", p.to_string_lossy().to_string());
        }
    }

    fn on_exit(&mut self) {
        // финальный сейв при выходе — здесь можно синхронно (окна уже нет).
        // Сначала дожидаемся фоновый автосейв: его rename не должен
        // перезаписать наш более свежий снапшот старым.
        if let Some((_, rx)) = self.autosave_rx.take() {
            let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
        }
        // Р.49 п.351: выход — НЕ момент молча переписать проект пользователя.
        // Несохранённое уезжает в файл восстановления; сохранённое уносит свой
        // файл восстановления с собой, чтобы следующий старт не предлагал
        // восстановить то, что и так в проекте.
        self.save_workspace();
        if self.dirty_content || self.dirty_ui {
            self.model.set_ui_state(Some(self.ui_state()));
            self.write_recovery_now();
        } else {
            self.drop_own_recovery();
        }
    }
}

/// Последний рубеж (Р.49, п.354).
///
/// Срабатывает там, где `on_exit` не зовут вовсе: при `*self = app` (Файл →
/// Новый/Открыть пересоздают приложение НА МЕСТЕ) и при раскрутке стека от
/// паники — `panic = "unwind"` у нас сохранён намеренно.
///
/// В теле НЕТ НИ ОДНОГО `unwrap`. Паника внутри `Drop` во время раскрутки —
/// это немедленный `abort`, и он унёс бы ровно тот файл, ради которого всё
/// затевалось. По той же причине сторож берётся с терпимостью к отравленному
/// мьютексу: поток, паниковавший под замком, не должен лишать нас записи.
impl Drop for KtApp {
    fn drop(&mut self) {
        if !self.recovery_on_drop || !(self.dirty_content || self.dirty_ui) {
            return;
        }
        self.write_recovery_now();
    }
}

/// GPU-движок для экспорта (Р.29): свой headless-девайс на джоб — kt-cli
/// окна не имеет, а очередь UI не делит время с 4K-экспортом. None → CPU
/// (для 2D-проекта это полноценный путь; `KT_VIDEO_GPU=0` — принудительно).
pub(crate) fn export_video_gpu() -> Option<std::sync::Arc<dyn kt_engine::VideoGpuExec>> {
    if std::env::var("KT_VIDEO_GPU").as_deref() == Ok("0") {
        return None;
    }
    match kt_gpu::GpuContext::new_headless().and_then(kt_gpu::GpuFxEngine::new) {
        Ok(e) => Some(e),
        Err(err) => {
            eprintln!("video export on CPU (no GPU): {err}");
            None
        }
    }
}

pub fn run() -> eframe::Result {
    let arg = std::env::args().nth(1);
    // GPU: дефолт egui-wgpu — HighPerformance, т.е. дискретная карта; на
    // гибридной графике это PRIME-копирование каждого кадра и тормоза ВСЕЙ
    // системы (п.1 v1.4). UI-нагрузка скромная — берём энергоэффективный
    // адаптер; WGPU_POWER_PREF=high_performance возвращает дискретный.
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.power_preference = eframe::wgpu::PowerPreference::from_env()
            .unwrap_or(eframe::wgpu::PowerPreference::LowPower);
    }
    // borderless (п.22 р.6): свой титлбар в тулбар-строке; KT_DECORATIONS=1
    // возвращает системную рамку (страховка от капризов WM)
    let decorations = std::env::var_os("KT_DECORATIONS").is_some();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 860.0])
            .with_min_inner_size([640.0, 640.0])
            .with_icon(app_icon())
            // app_id для Wayland и связи с .desktop (на X11 WM_CLASS
            // всё равно = имя бинаря, см. StartupWMClass в .desktop)
            .with_app_id("kinotrain")
            .with_decorations(decorations)
            .with_title("Kinotrain"),
        wgpu_options,
        ..Default::default()
    };
    eframe::run_native(
        "kinotrain",
        options,
        Box::new(move |cc| {
            // Язык решается ДО построения KtApp: строки ниже (восстановленный
            // автосейв, приглашение открыть проект) уже переводятся.
            // KT_LANG — отладочный оверрайд в ряду KT_AUTOPLAY/KT_DECORATIONS.
            let lang_choice = std::env::var("KT_LANG")
                .ok()
                .or_else(|| cc.storage.and_then(|s| s.get_string("lang")))
                .and_then(|v| crate::i18n::choice_from_code(&v));
            crate::theme::apply(&cc.egui_ctx);
            crate::theme::apply_debug_flags(&cc.egui_ctx); // п.23 р.6
            crate::i18n::set_lang_ctx(&cc.egui_ctx, crate::i18n::resolve(lang_choice));
            crate::icons::install_fonts(&cc.egui_ctx);
            crate::journal::install_panic_hook();
            let restored = arg
                .clone()
                .or_else(|| cc.storage.and_then(|s| s.get_string("last_project")))
                .map(std::path::PathBuf::from)
                .filter(|p| p.exists());
            let mut app = match restored {
                Some(path) => {
                    let mut a = KtApp::open(&path).unwrap_or_else(|e| KtApp::new_empty(Some(e)));
                    a.claim_project(&path);
                    a
                }
                None => KtApp::new_empty(Some(tr(k::msg::OPEN_A_PROJECT).to_string())),
            };
            app.lang_choice = lang_choice;
            // Р.49 п.353: несохранённая работа больше не «подменяет проект,
            // если last_project не нашёлся», а ПРЕДЛАГАЕТСЯ — с путём и
            // временем каждого кандидата. Прежняя ветка не покрывала главный
            // сценарий: открыл проект A, нажал «Новый», час работы, падение —
            // last_project указывал на A, и про час не говорилось ни строки.
            // прошлая сессия упала — отчёт лежит и ждёт (п.355)
            if let Some(p) = crate::journal::take_crash_report() {
                app.warn(crate::tf!(k::msg::PREV_SESSION_CRASHED, p.display()));
            }
            app.offer_recovery(true);
            Ok(Box::new(app))
        }),
    )
}

impl KtApp {
    /// Файл → Импортировать .ktproj… (Р.24 п.12: как проект старый формат
    /// больше не открывается — конвертация в .otio и открытие результата;
    /// исходный .ktproj не трогается никогда).
    pub fn import_ktproj_dialog(&mut self) {
        let Some(src) = rfd::FileDialog::new()
            .set_title(tr(k::dialog::IMPORT_KTPROJ))
            .add_filter("Kinotrain legacy project", &["ktproj"])
            .pick_file()
        else {
            return;
        };
        let text = match std::fs::read_to_string(&src) {
            Ok(t) => t,
            Err(e) => {
                self.note(crate::tf!(k::msg::OPEN_FAILED, e));
                return;
            }
        };
        let (project, notices) = match kt_otio::import_legacy(&text) {
            Ok(v) => v,
            Err(e) => {
                self.note(crate::tf!(k::msg::OPEN_FAILED, e));
                return;
            }
        };
        let default_name = src
            .file_stem()
            .map(|s| format!("{}.otio", s.to_string_lossy()))
            .unwrap_or_else(|| "imported.otio".into());
        let Some(dst) = rfd::FileDialog::new()
            .set_title(tr(k::dialog::SAVE_PROJECT))
            .add_filter("OpenTimelineIO", &["otio"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        let dst = if dst.extension().is_none() { dst.with_extension("otio") } else { dst };
        let out = kt_otio::save_project(&project, &kt_otio::Retained::default(), dst.parent());
        if let Err(e) = write_project_atomic(&dst, &out) {
            self.note(crate::tf!(k::msg::SAVE_FAILED, e));
            return;
        }
        match Self::open(&dst) {
            Ok(app) => {
                *self = app;
                for n in notices {
                    self.note(n);
                }
            }
            Err(e) => self.note(crate::tf!(k::msg::OPEN_FAILED, e)),
        }
    }
}

/// Слить пересекающиеся/смежные диапазоны прогретости в один список
/// (Р.46, п.322): куски одного клипа могут греться в разных качествах
/// декода, но полоска под клипом — одна.
pub fn merge_runs(mut runs: Vec<std::ops::Range<i64>>) -> Vec<std::ops::Range<i64>> {
    if runs.len() < 2 {
        return runs;
    }
    runs.sort_by_key(|r| r.start);
    let mut out: Vec<std::ops::Range<i64>> = Vec::with_capacity(runs.len());
    for r in runs {
        match out.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => out.push(r),
        }
    }
    out
}

/// Прямоугольники safe-зон кадра: action 5 % и title 10 % с каждой
/// стороны (проценты — F-TXT-001, наследие кинескопов).
pub(crate) fn safe_zone_rects(img: egui::Rect) -> [egui::Rect; 2] {
    let inset = |f: f32| {
        egui::Rect::from_min_max(
            img.min + egui::vec2(img.width() * f, img.height() * f),
            img.max - egui::vec2(img.width() * f, img.height() * f),
        )
    };
    [inset(0.05), inset(0.10)]
}

#[cfg(test)]
mod repaint_tests {
    use super::repaint_delay;

    /// Тест 58 (п.15 раунда 6; заменяет прежние правила «из паузы при
    /// регионе — всегда в начало» с санкции пользователя): транспорт-машина
    /// Space — пауза на месте; resume с места; двойной Space (в окне
    /// DOUBLE_PAUSE_MS) = стоп в начало региона/0; resume при loop и
    /// курсоре вне региона — с начала региона (wrap работает изнутри).
    #[test]
    fn t58_transport_toggle_rules() {
        use super::{KtApp, TransportAction as A};
        let toggle = KtApp::transport_toggle;
        // играет → пауза (курсор не трогаем)
        assert_eq!(toggle(true, false, 500, Some((100, 200)), true), A::Pause);
        // пауза → resume с места (регион есть, но курсор внутри/loop любой)
        assert_eq!(toggle(false, false, 150, Some((100, 200)), true), A::Resume { seek: None });
        assert_eq!(toggle(false, false, 150, Some((100, 200)), false), A::Resume { seek: None });
        assert_eq!(toggle(false, false, 5, None, false), A::Resume { seek: None });
        // loop включён, курсор ВНЕ региона → resume с начала региона
        assert_eq!(
            toggle(false, false, 500, Some((100, 200)), true),
            A::Resume { seek: Some(100) }
        );
        // t73 (п.29 р.7, санкционированная правка t58): loop выключен,
        // курсор НА/ЗА концом региона (после авто-паузы) → Play играет
        // регион заново с начала; раньше Resume{None} тем же кадром
        // попадал обратно в окно авто-паузы — «play ничего не делает»
        assert_eq!(
            toggle(false, false, 500, Some((100, 200)), false),
            A::Resume { seek: Some(100) }
        );
        assert_eq!(
            toggle(false, false, 200, Some((100, 200)), false),
            A::Resume { seek: Some(100) }
        );
        // ...но игра ДО региона не трогается (cursor < start)
        assert_eq!(toggle(false, false, 50, Some((100, 200)), false), A::Resume { seek: None });
        // двойной Space в паузе → стоп в начало региона (или 0)
        assert_eq!(toggle(false, true, 150, Some((100, 200)), false), A::SeekStart(100));
        assert_eq!(toggle(false, true, 150, None, false), A::SeekStart(0));
        // пустой регион игнорируется
        assert_eq!(toggle(false, true, 150, Some((100, 100)), true), A::SeekStart(0));
        assert_eq!(toggle(false, false, 500, Some((100, 100)), true), A::Resume { seek: None });
    }

    /// Тест 765 (Р.64): доля превью ПОДОБНА кадру проекта.
    ///
    /// Независимое чётное округление осей давало 1920×1080 → 240×134 при
    /// 1/8 и 120×66 при 1/16 (1.791 и 1.818 против 1.778): «вписать»
    /// считало на превью другую рамку, чем в экспорте, и экранный контрол
    /// показывал бы не то, что уедет в файл.
    #[test]
    fn t765_preview_fraction_keeps_the_project_aspect() {
        use super::preview_quality_for;
        for (pw, ph) in [(1920u32, 1080u32), (1080, 1920), (1280, 720), (1998, 1080)] {
            let want = pw as f64 / ph as f64;
            for f in [1.0f32, 0.5, 0.25, 0.125, 0.0625] {
                let q = preview_quality_for(pw, ph, f);
                assert!(q.width >= 2 && q.height >= 2, "{pw}x{ph} @{f}: вырожденный размер {q:?}");
                assert!(
                    q.width.is_multiple_of(2) && q.height.is_multiple_of(2),
                    "{pw}x{ph} @{f}: нечётный размер {q:?} — декодер 4:2:0 его не отдаст"
                );
                // отклонение отношения — не больше половины пикселя по
                // КОРОТКОЙ стороне (точнее чётность не позволяет)
                let got = q.width as f64 / q.height as f64;
                let short = q.width.min(q.height) as f64;
                let tol = want * (0.5 / short) * 2.0;
                assert!(
                    (got - want).abs() <= tol,
                    "{pw}x{ph} @{f}: превью {q:?} не подобно кадру — {got:.4} против {want:.4} \
                     (допуск {tol:.4})"
                );
            }
        }
        // РАМКА «вписать» одна и та же в ДОЛЯХ холста на всех долях —
        // то, ради чего подобие и нужно: превью и экспорт обязаны считать
        // одну рамку. (Проверка стоит здесь, а не в kt-fx: там нет доли
        // превью, а в kt-app есть обе половины.)
        for view in [(1080.0f64, 1920.0f64), (3840.0, 2160.0), (720.0, 720.0)] {
            let mut ref_frac: Option<(f64, f64)> = None;
            for f in [1.0f32, 0.5, 0.25, 0.125, 0.0625] {
                let q = preview_quality_for(1920, 1080, f);
                let canvas = (q.width as f64, q.height as f64);
                let d = kt_fx::conform::conform_dest(
                    Some(view),
                    canvas,
                    kt_fx::conform::ConformRule::Fit,
                );
                let frac = (d.0 / canvas.0, d.1 / canvas.1);
                match ref_frac {
                    None => ref_frac = Some(frac),
                    Some(r) => {
                        let tol = 1.0 / q.width.min(q.height) as f64;
                        assert!(
                            (frac.0 - r.0).abs() <= tol && (frac.1 - r.1).abs() <= tol,
                            "вид {view:?} @{f}: рамка «вписать» в долях холста разошлась — \
                             {frac:?} против {r:?} (допуск {tol:.4})"
                        );
                    }
                }
            }
        }

        // …и это НЕ «любой размер подходит»: прежнее независимое
        // округление на этих же долях не проходит собственный допуск
        for (f, w, h) in [(0.125f32, 240u32, 134u32), (0.0625, 120, 66)] {
            let want = 1920.0 / 1080.0;
            let got = w as f64 / h as f64;
            let tol = want * (0.5 / h.min(w) as f64) * 2.0;
            assert!(
                (got - want).abs() > tol,
                "сцена вырождена: прежнее округление @{f} ({w}x{h}) уложилось в допуск"
            );
            let now = preview_quality_for(1920, 1080, f);
            assert_ne!(
                (now.width, now.height),
                (w, h),
                "@{f} доля осталась прежней — функция ничего не изменила"
            );
        }
    }

    /// Тест 39 (п.1 v1.4): ключ превью выровнен по сетке fps — внутри
    /// одной ячейки кадра ключ стабилен (текстура не перезаливается),
    /// на границе — новый. Счётчик V-fps растёт только на новом ключе.
    #[test]
    fn t39_preview_key_is_grid_aligned() {
        use super::preview_key_tick;
        let grid = kt_time::FpsGrid::new(30); // шаг 3000 тиков
        assert_eq!(preview_key_tick(grid, kt_time::Tick(0)), 0);
        assert_eq!(preview_key_tick(grid, kt_time::Tick(1)), 0);
        assert_eq!(preview_key_tick(grid, kt_time::Tick(2999)), 0, "внутри ячейки ключ тот же");
        assert_eq!(preview_key_tick(grid, kt_time::Tick(3000)), 3000, "граница — новый ключ");
        // симуляция UI 60 fps (шаг 1500 тиков) на проекте 30 fps:
        // из 60 ui-кадров ровно 30 дают новый ключ
        let mut shown = 0u64;
        let mut prev: Option<i64> = None;
        for ui_frame in 0..60i64 {
            let key = preview_key_tick(grid, kt_time::Tick(ui_frame * 1500));
            if prev != Some(key) {
                shown += 1;
            }
            prev = Some(key);
        }
        assert_eq!(shown, 30, "V-fps считает кадры проекта, а не ui-кадры");
    }

    /// Тест 46 (пп.4-5 v1.4, ПЕРЕПИСАН в Р.57 п.455): маппинг полосы
    /// скорости.
    ///
    /// Претензия владельца: «диапазон −0.25…0…0.25 превращается в 0, а я
    /// должен иметь возможность выставить 0,05 или −0,03». Причина была
    /// ровно тут: мёртвая зона в 7 % хода отдавала 0, а её КРАЙ уже равнялся
    /// 0.25× — интервал (0, 0.25) был непредставим по построению. Поэтому
    /// тест теперь утверждает не «мёртвая зона широкая», а обратное:
    /// нулём отдаёт только узкая зона, у пола нет обрыва, и 0.05 с 0.03
    /// разнесены так, что в них можно попасть мышью.
    #[test]
    fn t46_rate_slider_mapping() {
        use super::{RATE_FLOOR, RATE_MAX, RATE_STOP, rate_to_slider, slider_to_rate};
        // --- стоп: ТОЛЬКО |t| < RATE_STOP
        for t in [0.0f32, 0.01, -0.01, 0.0179, -0.0179] {
            assert_eq!(slider_to_rate(t), 0.0, "t={t} обязан быть стопом");
        }
        for t in [RATE_STOP, -RATE_STOP, 0.02, -0.02, 0.05] {
            assert_ne!(slider_to_rate(t), 0.0, "t={t} за зоной стопа, а отдаёт 0");
        }
        // --- непрерывность у пола: обрыв на краю зоны — не больше RATE_FLOOR
        // (до Р.57 он был 0.25× — это и есть дефект)
        let edge = slider_to_rate(RATE_STOP);
        assert!(
            (edge - RATE_FLOOR).abs() < 1e-6,
            "край зоны стопа обязан давать пол {RATE_FLOOR}, а даёт {edge}"
        );
        // --- нечётность
        for t in [0.02f32, 0.1, 0.3, 0.7, 1.0] {
            assert_eq!(slider_to_rate(t), -slider_to_rate(-t), "нечётность нарушена в t={t}");
        }
        // --- строгая монотонность шагом 0.001 (нет полок и провалов)
        let mut prev = 0.0f32;
        let mut t = RATE_STOP;
        while t <= 1.0 {
            let r = slider_to_rate(t);
            assert!(r > prev, "не растёт: t={t} даёт {r} после {prev}");
            prev = r;
            t += 0.001;
        }
        // --- границы
        assert!((slider_to_rate(1.0) - RATE_MAX).abs() < 1e-5);
        assert!((slider_to_rate(-1.0) + RATE_MAX).abs() < 1e-5);
        // --- round-trip, включая ТО, ЧТО ПРОСИЛ ВЛАДЕЛЕЦ
        for r in [0.01f32, 0.03, 0.05, 0.1, 0.25, 0.5, 1.0, 1.5, 2.0, -0.03, -0.25, -1.0, -2.0] {
            let back = slider_to_rate(rate_to_slider(r));
            assert!((back - r).abs() < 1e-4, "round-trip {r} → {back}");
        }
        assert_eq!(rate_to_slider(0.0), 0.0, "стоп — центр полосы");
        // |r| > 2 садится на край: метка остаётся видимой
        assert_eq!(rate_to_slider(8.0), 1.0);
        assert_eq!(rate_to_slider(-8.0), -1.0);
        // --- 1× стоит в [0.65, 0.80] хода: справа обязано остаться место
        // под ускорение, слева — весь медленный участок
        let one = rate_to_slider(1.0);
        assert!((0.65..=0.80).contains(&one), "1× уехал на {one} хода");
        // --- 0.05 и 0.03 разнесены на ~2 px (половина полосы 85 px):
        // «попал мышью с первого раза» — правило 1. До Р.57 обе величины
        // лежали в мёртвой зоне и были неотличимы от нуля ВООБЩЕ.
        let px = |r: f32| rate_to_slider(r) * 85.0;
        assert!(
            px(0.05) - px(0.03) >= 1.9,
            "0.03 и 0.05 слиплись: {} px против {} px",
            px(0.03),
            px(0.05)
        );
    }

    /// Тест 626 (Р.57): арифметика ввода и точной подстройки.
    #[test]
    fn t626_rate_entry_arithmetic() {
        use super::{
            ENGINE_RATE_MAX, RATE_FLOOR, SPEED_FINE_DIV, boost_needs_update, drag_advance,
            normalize_rate, parse_rate_text,
        };
        // --- normalize_rate: почти-ноль = стоп, иначе пол и потолок со знаком
        assert_eq!(normalize_rate(0.0), 0.0);
        assert_eq!(normalize_rate(0.004), 0.0);
        assert_eq!(normalize_rate(-0.004), 0.0);
        assert_eq!(normalize_rate(0.006), RATE_FLOOR, "выше порога — не ноль, а пол");
        assert_eq!(normalize_rate(0.05), 0.05);
        assert_eq!(normalize_rate(-0.03), -0.03);
        assert_eq!(normalize_rate(99.0), ENGINE_RATE_MAX);
        assert_eq!(normalize_rate(-99.0), -ENGINE_RATE_MAX);
        assert_eq!(normalize_rate(f32::NAN), 0.0);
        // --- parse_rate_text: то, что человек реально печатает
        for (s, want) in [
            ("0.05", 0.05f32),
            ("0,05", 0.05),
            ("-0.03", -0.03),
            ("\u{2212}0,03", -0.03), // типографский минус
            ("+1.5", 1.5),
            ("2×", 2.0),
            ("2x", 2.0),
            (" 0.5 ", 0.5),
        ] {
            let got = parse_rate_text(s).unwrap_or_else(|| panic!("«{s}» не разобрано"));
            assert!((got - want).abs() < 1e-6, "«{s}» → {got}, ждали {want}");
        }
        for s in ["", "×", "--1", "быстро", "1.2.3"] {
            assert!(parse_rate_text(s).is_none(), "«{s}» разобралось, а не должно");
        }
        // --- drag_advance: Shift делит ПРИРАЩЕНИЕ, а не позицию
        assert_eq!(drag_advance(85.0, 6.0, false), 91.0);
        assert_eq!(drag_advance(85.0, 6.0, true), 85.0 + 6.0 / SPEED_FINE_DIV);
        // цепочка с Shift посреди жеста не телепортирует значение: она
        // продолжается с того же места, где была
        let mut vx = 85.0f32;
        for _ in 0..4 {
            vx = drag_advance(vx, 8.0, false);
        }
        let plain = vx;
        vx = drag_advance(vx, 8.0, true);
        assert!(
            vx > plain && vx - plain < 8.0,
            "Shift обязан замедлить ход, а не прыгнуть: {plain} → {vx}"
        );
        // --- гистерезис разгона ОТНОСИТЕЛЬНЫЙ
        assert!(boost_needs_update(1.05, 1.0, 1.0), "на базе 1× 5 % обязаны доехать");
        assert!(!boost_needs_update(1.005, 1.0, 1.0), "на базе 1× 0.5 % — шум");
        // прежний абсолютный порог 0.02 на базе 0.05 — это 40 % базы:
        // разгон физически не доезжал до движка
        assert!(
            boost_needs_update(0.06, 0.05, 0.05),
            "на базе 0.05 разгон вдвое обязан доехать до плеера"
        );
    }

    /// Тест 47 (дополнение, регресс п.2 раунда 5): dnd-позиция живёт при
    /// hover'е и на кадре дропа — прежний безусловный take() стирал её
    /// каждый кадр, дроп всегда падал на playhead, силуэт не рисовался.
    #[test]
    fn t47_dnd_hover_survives_until_drop() {
        use super::dnd_keep_hover;
        assert!(dnd_keep_hover(true, false), "во время hover — живёт");
        assert!(dnd_keep_hover(false, true), "на кадре дропа (hover уже пуст) — живёт");
        assert!(dnd_keep_hover(true, true));
        assert!(!dnd_keep_hover(false, false), "без hover и без дропа — гаснет");
    }

    /// Тест 74 (пп.10-11 р.7): машина отложенного клика по превью —
    /// одиночный ждёт окно double-click и даёт play/pause; двойной
    /// отменяет одиночный и переключает detach.
    #[test]
    fn t74_preview_click_machine() {
        use super::{PendingOp as O, PreviewClick as C, preview_click_step};
        // клик → взводим, действия нет
        assert_eq!(preview_click_step(None, true, false, 300), (C::None, O::Arm));
        // окно ещё не истекло → ждём
        assert_eq!(preview_click_step(Some(100), false, false, 300), (C::None, O::Keep));
        // истекло → play/pause
        assert_eq!(preview_click_step(Some(300), false, false, 300), (C::TogglePlay, O::Clear));
        assert_eq!(preview_click_step(Some(999), false, false, 300), (C::TogglePlay, O::Clear));
        // двойной клик отменяет отложенный одиночный и детачит
        assert_eq!(preview_click_step(Some(120), false, true, 300), (C::ToggleDetach, O::Clear));
        // двойной без pending — тоже детач
        assert_eq!(preview_click_step(None, false, true, 300), (C::ToggleDetach, O::Clear));
        // без клика и без pending — ничего
        assert_eq!(preview_click_step(None, false, false, 300), (C::None, O::Keep));
    }

    /// Тест 92 (п.3 р.10): поиск по проекту — метки, медиа, дорожки;
    /// регистронезависимо, точное совпадение выше подстроки.
    #[test]
    fn t92_search_hits() {
        use super::{HitKind, search_hits};
        use kt_model::*;
        use kt_time::Tick;
        let mut p = Project::new(ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        });
        p.media.push(MediaRef::new(MediaId(1), "/tmp/Интервью.mp4"));
        p.markers.push(Marker::named(Tick(5000), "Интро"));
        p.markers.push(Marker::named(Tick(9000), "интервью — начало"));
        // имя дорожки с Р.62 живёт на дорожке-объекте
        p.track_at_mut(LaneKind::Audio, 0).unwrap().name = "Голос".into();
        let groups = vec![BinGroup {
            name: "Планы города".into(),
            members: vec![MediaId(1)],
            locked: false,
        }];

        // пустой запрос — пусто
        assert!(search_hits(&p, &groups, "   ").is_empty());

        // регистр не важен, точное совпадение первым
        let hits = search_hits(&p, &groups, "интро");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, HitKind::Marker);
        assert_eq!(hits[0].at, 5000);

        // одна подстрока находит и метку, и медиа
        let hits = search_hits(&p, &groups, "интервью");
        assert!(hits.iter().any(|h| h.kind == HitKind::Marker));
        assert!(hits.iter().any(|h| h.kind == HitKind::Media));

        // дорожка по пользовательскому имени и по формуле
        assert!(search_hits(&p, &groups, "голос").iter().any(|h| h.kind == HitKind::Track));
        assert!(search_hits(&p, &groups, "v1").iter().any(|h| h.kind == HitKind::Track));

        // мимо
        assert!(search_hits(&p, &groups, "щщщ").is_empty());
        // группы бина тоже ищутся по имени (п.13 р.11)
        let hits = search_hits(&p, &groups, "планы");
        assert!(hits.iter().any(|h| h.kind == HitKind::Group), "группа не найдена: {hits:?}");
        // безымянная группа в выдачу не лезет
        let anon = vec![BinGroup { name: String::new(), members: vec![MediaId(1)], locked: true }];
        assert!(search_hits(&p, &anon, "").is_empty());
    }

    /// Тест 82 (п.16 р.8): умная навигация — точки остановки и прыжки.
    /// Курсор РОВНО на точке уходит дальше (не залипает), цепочка нажатий
    /// монотонна, перекрытые клипы дают одну границу.
    #[test]
    fn t82_stop_points_and_jumps() {
        use super::{next_stop, prev_stop, stop_points};
        use kt_model::*;
        use kt_time::Tick;
        let mut p = Project::new(ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        });
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        let clip = |id: u64, start: i64, dur: i64| ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(start),
            duration: Tick(dur),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        };
        // два ПЕРЕКРЫВАЮЩИХСЯ клипа: 1000..3000 и 2500..4000 → одна
        // область 1000..4000, стык внутри не считается границей
        p.clips.push(clip(1, 1000, 2000));
        p.clips.push(clip(2, 2500, 1500));
        p.markers.push(Marker::at(Tick(5000)));
        p.region = Some(Region { start: Tick(7000), end: Tick(9000) });
        let pts = stop_points(&p);
        // точки выровнены ВВЕРХ к аудио-сетке (кратно 15 тикам, п.7 р.10):
        // сдвиг ≤14 тиков = 0.15 мс, зато прыжок вперёд не застревает
        assert_eq!(pts, vec![0, 1005, 4005, 5010, 7005, 9000], "точки: {pts:?}");
        assert!(pts.iter().all(|t| t % 15 == 0), "не выровнено: {pts:?}");

        // курсор ровно на точке — уходим дальше, не залипаем
        assert_eq!(prev_stop(5010, &pts), Some(4005));
        assert_eq!(next_stop(5010, &pts), Some(7005));
        // цепочка нажатий влево монотонна и без повторов
        let mut cur = 9000;
        let mut seen = vec![];
        while let Some(p) = prev_stop(cur, &pts) {
            assert!(p < cur);
            seen.push(p);
            cur = p;
        }
        assert_eq!(seen, vec![7005, 5010, 4005, 1005, 0]);
        // края
        assert_eq!(prev_stop(0, &pts), None);
        assert_eq!(next_stop(9000, &pts), None);
        // пустой проект — только ноль
        let empty = Project::new(ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        });
        assert_eq!(stop_points(&empty), vec![0]);
    }

    /// Тест 81 (п.4 р.8): «второе дыхание» шаттла — за пределами виджета
    /// скорость растёт до ±8×, симметрично по знаку и монотонно.
    #[test]
    fn t81_overshoot_rate() {
        use super::{ENGINE_RATE_MAX, OVERSHOOT_PX_PER_DOUBLE as K, overshoot_rate};
        // внутри виджета множителя нет
        assert_eq!(overshoot_rate(2.0, 0.0, K), 2.0);
        assert_eq!(overshoot_rate(-2.0, -5.0, K), -2.0);
        // ровно одно удвоение
        assert!((overshoot_rate(2.0, K, K) - 4.0).abs() < 1e-4);
        assert!((overshoot_rate(-2.0, K, K) + 4.0).abs() < 1e-4, "симметрия по знаку");
        // потолок движка
        assert_eq!(overshoot_rate(2.0, K * 10.0, K), ENGINE_RATE_MAX);
        assert_eq!(overshoot_rate(-2.0, K * 10.0, K), -ENGINE_RATE_MAX);
        // ноль (стоп) остаётся нулём
        assert_eq!(overshoot_rate(0.0, K * 3.0, K), 0.0);
        // монотонность
        let mut prev = 0.0;
        for i in 0..20 {
            let r = overshoot_rate(1.0, i as f32 * 10.0, K);
            assert!(r >= prev, "не монотонно на {i}");
            prev = r;
        }
    }

    /// Тест 78 (п.14 р.8): единственная строка подсказок — приоритеты
    /// наведение → режим → временное сообщение → дежурная строка.
    #[test]
    fn t78_status_line_priority() {
        use super::{HintKind as K, STATUS_TTL_MS, status_line};
        let idle = "дежурная";
        // невозможность действия перебивает даже наведение (п.5 Р.28)
        let (t, k) = status_line(
            Some("нельзя"),
            Some("hover"),
            Some("mode"),
            Some(("st", 0)),
            STATUS_TTL_MS,
            idle,
        );
        assert_eq!((t.as_str(), k), ("нельзя", K::Error));
        // наведение перебивает остальное
        let (t, k) =
            status_line(None, Some("hover"), Some("mode"), Some(("st", 0)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), ("hover", K::Hover));
        // режим перебивает временное сообщение
        let (t, k) = status_line(None, None, Some("mode"), Some(("st", 0)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), ("mode", K::Mode));
        // временное сообщение живёт до TTL
        let (t, k) =
            status_line(None, None, None, Some(("st", STATUS_TTL_MS - 1)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), ("st", K::Status));
        // ...и уступает дежурной строке после
        let (t, k) =
            status_line(None, None, None, Some(("st", STATUS_TTL_MS)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), (idle, K::Idle));
        // пусто → дежурная
        let (t, k) = status_line(None, None, None, None, STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), (idle, K::Idle));
    }

    /// Тест 77 (пп.11-13 р.8, расширяет t64): зоны ресайза окна с ДВУМЯ
    /// радиусами — углы «ловятся» на большем расстоянии и приоритетнее
    /// рёбер (раньше угол был 4×4 px и в него невозможно было попасть).
    #[test]
    fn t77_resize_zones_edge_and_corner() {
        use super::{WIN_CORNER, WIN_EDGE, resize_zone};
        use eframe::egui::{Rect, ResizeDirection::*, pos2};
        let w = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 300.0));
        let z = |x: f32, y: f32| resize_zone(w, pos2(x, y), WIN_EDGE, WIN_CORNER);
        // углы — по радиусу WIN_CORNER, даже вдали от самой кромки
        assert_eq!(z(12.0, 12.0), Some(NorthWest));
        assert_eq!(z(389.0, 14.0), Some(NorthEast));
        assert_eq!(z(10.0, 290.0), Some(SouthWest));
        assert_eq!(z(392.0, 288.0), Some(SouthEast));
        // рёбра — по радиусу WIN_EDGE
        assert_eq!(z(3.0, 150.0), Some(West));
        assert_eq!(z(397.0, 150.0), Some(East));
        assert_eq!(z(200.0, 3.0), Some(North));
        assert_eq!(z(200.0, 297.0), Some(South));
        // за кромкой ребра, но ещё не в углу — ничего
        assert_eq!(z(200.0, 150.0), None, "центр");
        assert_eq!(z(30.0, 150.0), None, "в стороне от кромки");
        // точные границы
        assert_eq!(z(WIN_EDGE, 150.0), Some(West));
        assert_eq!(z(WIN_EDGE + 0.1, 150.0), None);
        assert_eq!(z(WIN_CORNER, WIN_CORNER), Some(NorthWest));
        assert_eq!(z(WIN_CORNER + 0.1, WIN_CORNER + 0.1), None);
        // вне окна
        assert_eq!(z(-1.0, 150.0), None);
    }

    /// Тест 51 (п.7 v1.4): state-машина деатача превью — закрытие окна
    /// всегда возвращает превью в главное, кнопка переключает.
    #[test]
    fn t51_preview_detach_state() {
        use super::preview_detached_next as next;
        assert!(next(false, true, false), "кнопка отделяет");
        assert!(!next(true, true, false), "кнопка возвращает");
        assert!(!next(true, false, true), "закрытие окна возвращает");
        assert!(!next(true, true, true), "закрытие сильнее кнопки");
        assert!(!next(false, false, false), "покой");
        assert!(next(true, false, false), "остаётся отделённым");
    }

    /// Тест 20-repaint (п.1): в простое НЕ 60 fps.
    #[test]
    fn t20_idle_repaint_is_slow_playing_is_60fps() {
        assert_eq!(repaint_delay(true).as_millis(), 16);
        assert!(repaint_delay(false).as_millis() >= 100, "простой = редкий тик");
    }
}
