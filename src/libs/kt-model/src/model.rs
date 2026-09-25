//! Редактируемая модель: команды + undo.
//!
//! Undo — снапшоты всего `Project` (просто и надёжно; глубина ограничена).
//! Команда ВАЛИДИРУЕТСЯ ДО снапшота: невозможная команда возвращает Err и не
//! оставляет no-op записей в истории (дефект q старого TimelineEditor, где
//! push шёл до проверок).

use crate::genmedia::GeneratorKind;
use crate::project::*;
use crate::published::*;
use kt_time::{Tick, TickRange};

pub const MAX_UNDO: usize = 80;

/// Минимальная длительность клипа. 1 мс, как в старом редакторе.
/// Выровнять СКОРОСТЬ варпа у link-партнёров (Р.37 п.2, решение
/// пользователя: «эффект для аудио/видео можно объединять сразу»).
///
/// Правило живёт в МОДЕЛИ, а не в окне FX, потому что `SetFx` прилетает
/// ещё из вставки цепочки из буфера, из bake морфера, из ридера формата и
/// из kt-cli. «Пара A/V потребляет источник с одной скоростью» — это
/// инвариант монтажа: разъедутся скорости, разъедутся картинка и звук.
/// Прецедент — `reconcile_transitions` после каждой геометрической
/// мутации. Внутри `mutate_on` зеркалирование попадает в ТОТ ЖЕ undo-шаг
/// по построению, даже под `apply_coalesced`.
///
/// Синхронно: наличие слота, `rate` и его кривая. Своё у каждого:
/// параметры реверба, режим кадров, bypass слота (галка на видео-слоте —
/// это и есть «отключить замедление картинки»).
/// Значения генератора — общие у link-пары A/V НА ОДНОМ МЕДИА (Р.59,
/// стадия 2).
///
/// Прецедент и обоснование те же, что у `sync_warp_to_link_group`:
/// звучащий генератор (SMPTE с тоном, отсчёт с биипом) кладёт на
/// таймлайн ПАРУ событий на одно медиа, и у них один вид. Разойдись
/// значения — картинка показывала бы полосы одного уровня, а тон звучал
/// бы от другого, и объяснить это было бы нечем. Правило живёт В МОДЕЛИ,
/// потому что команды прилетают не только из окна: их шлют вставка
/// цепочки, «применить ко всем» и ридер формата.
///
/// Разные медиа не синхронизируются: у них разные виды и разные адреса
/// источника — это уже не пара, а два независимых генератора.
fn sync_gen_to_link_group(p: &mut Project, id: ClipId) {
    let Some(src) = p.clips.iter().find(|c| c.id == id) else { return };
    let Some(group) = src.link_group else { return };
    let (media, kind, auto) = (src.media, src.r#gen.clone(), src.gen_auto.clone());
    for c in &mut p.clips {
        if c.id != id && c.link_group == Some(group) && c.media == media {
            c.r#gen = kind.clone();
            c.gen_auto = auto.clone();
        }
    }
}

/// Привести значения генератора в согласие с МЕДИА события (Р.59).
///
/// Единственное место, где значения появляются или исчезают сами: клип
/// лёг на генератор — берём вид из пути медиа (там же и живут значения
/// старых файлов, так что миграция выходит бесплатной); клип уехал на
/// файл — значений у него быть не может.
fn fit_gen_to_media(p: &Project, c: &mut ClipEvent) {
    match p.default_gen_for(c.media) {
        Some(k) => {
            let same = c.r#gen.as_ref().is_some_and(|g| g.tag() == k.tag());
            if !same {
                c.r#gen = Some(k);
                c.gen_auto.clear();
            }
        }
        None => {
            c.r#gen = None;
            c.gen_auto.clear();
        }
    }
}

/// Проверка согласованности значений генератора для команды — ТЕМ ЖЕ
/// кодом, что и валидация файла.
fn kt_check(p: &Project, c: &ClipEvent) -> Result<(), CommandError> {
    check_clip_gen(c, p.gen_media_path(c.media)).map_err(CommandError::InvalidClip)
}

fn sync_warp_to_link_group(p: &mut Project, id: ClipId) {
    let Some(src) = p.clips.iter().find(|c| c.id == id) else { return };
    let Some(group) = src.link_group else { return };
    // слот ищется ПО ВИДУ, а не по индексу: у аудио может быть
    // [Gain, Slowed], у видео [Blur, Slowed, Mirror]
    let warp_slot = |c: &ClipEvent| c.fx.iter().position(|f| f.warps_time());
    let mine = warp_slot(src);
    let (tmpl, rate_curve) = match mine {
        Some(i) => (
            Some(src.fx[i].clone()),
            src.fx_auto.get(i).and_then(|cs| cs.iter().find(|c| c.param == "rate").cloned()),
        ),
        None => (None, None),
    };
    let partners: Vec<ClipId> = p
        .clips
        .iter()
        .filter(|c| c.id != id && c.link_group == Some(group))
        .map(|c| c.id)
        .collect();
    for pid in partners {
        let Some(c) = p.clips.iter_mut().find(|c| c.id == pid) else { continue };
        match (warp_slot(c), &tmpl) {
            // у обоих есть — переносим ТОЛЬКО скорость и её кривую
            (Some(j), Some(t)) => {
                if let (
                    FxInstance::SlowedReverb { rate, .. },
                    FxInstance::SlowedReverb { rate: r0, .. },
                ) = (&mut c.fx[j], t)
                {
                    *rate = *r0;
                }
                if c.fx_auto.len() <= j {
                    c.fx_auto.resize(j + 1, Vec::new());
                }
                c.fx_auto[j].retain(|cu| cu.param != "rate");
                if let Some(rc) = &rate_curve {
                    c.fx_auto[j].push(rc.clone());
                }
            }
            // появился у одного — заводим у партнёра (одна добавка кладёт
            // экземпляр на ОБА события пары)
            (None, Some(t)) => {
                c.fx.push(t.clone());
                let mut curves = Vec::new();
                if let Some(rc) = &rate_curve {
                    curves.push(rc.clone());
                }
                c.fx_auto.resize(c.fx.len(), Vec::new());
                // слот только что добавлен — last_mut есть по построению
                if let Some(slot) = c.fx_auto.last_mut() {
                    *slot = curves;
                }
            }
            // исчез у одного — убираем у партнёра вместе с его кривыми
            (Some(j), None) => {
                c.fx.remove(j);
                if j < c.fx_auto.len() {
                    c.fx_auto.remove(j);
                }
                if j < c.fx_bypass.len() {
                    c.fx_bypass.remove(j);
                }
            }
            (None, None) => {}
        }
    }
}

pub const MIN_CLIP_DURATION: Tick = Tick(90);

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddMedia {
        id: MediaId,
        path: String,
    },
    /// Смена пути медиа (Р.52): правка параметров генератора — это смена
    /// его `gen:`-пути, и все клипы этого медиа обновляются (семантика
    /// Vegas: два клипа на один генератор делят вид). Путь для модели
    /// непрозрачен, как в `AddMedia`, — команда годится и будущему
    /// relink. Пересоздание источника у плеера — забота call-site
    /// (`KtApp::sync_media_sources`), как и у `AddMedia`.
    SetMediaPath {
        id: MediaId,
        path: String,
    },
    AddClip(ClipEvent),
    RemoveClip {
        id: ClipId,
    },
    /// Двигает клип (и всю его link-группу — тем же смещением).
    MoveClip {
        id: ClipId,
        new_start: Tick,
        new_lane: u32,
    },
    /// Триммит начало: start/duration/source_in двигаются согласованно.
    TrimStart {
        id: ClipId,
        new_start: Tick,
    },
    TrimEnd {
        id: ClipId,
        new_end: Tick,
    },
    /// Заметка о РОДСТВЕ: `child` — событие, рождённое от `parent` в этом же
    /// пакете (Р.49, п.357). Состояние проекта не меняет вовсе.
    ///
    /// Зачем командой. Производные события — правая половина разреза, остатки
    /// кропа и punch-out, вставленная копия, куски bake — рождаются ВНУТРИ
    /// `ops::*`, и снаружи их родителя уже не вычислить: после перенумерации
    /// это была бы догадка по совпадению материала и окна. Тот, кто режет,
    /// родителя знает точно — пусть он его и назовёт. Носитель родства должен
    /// пережить undo вместе с самой правкой, а значит жить в пакете команд.
    NoteBirth {
        child: ClipId,
        parent: ClipId,
    },
    /// Разрез в точке `at` (абсолютный тик). Правая половина получает новый id
    /// и новую link-группу (правые половины A/V-пары остаются связаны).
    Split {
        id: ClipId,
        at: Tick,
        new_id: ClipId,
        new_group: Option<GroupId>,
    },
    /// Опция «трим растягивает анимацию вместе с событием» (Р.40).
    /// Выключена по умолчанию: ключ остаётся на своём кадре материала,
    /// куда бы ни ехал край события.
    SetTrimStretchesAuto {
        on: bool,
    },
    SetGain {
        id: ClipId,
        gain_db: f32,
    },
    /// Уровень (непрозрачность) видео-события 0..1 — Р.28, п.12.
    SetOpacity {
        id: ClipId,
        opacity: f32,
    },
    /// Ведущий узел «Геометрия» события и его позиция в цепочке (Р.62).
    SetGeometry {
        id: ClipId,
        geometry: crate::EventGeometry,
        pos: usize,
        auto: Vec<ParamCurve>,
    },
    /// Мастер-выключатель FX-цепочки события (Р.33). Состояние слотов не
    /// трогает: включил обратно — цепочка та же.
    SetFxOff {
        id: ClipId,
        off: bool,
    },
    /// Реверс события (п.16). Undo-able.
    SetReversed {
        id: ClipId,
        reversed: bool,
    },
    /// Bypass-флаги FX-слотов (п.2 v1.3). Undo-able.
    SetFxBypass {
        id: ClipId,
        bypass: Vec<bool>,
    },
    SetFx {
        id: ClipId,
        fx: Vec<FxInstance>,
    },
    /// Автоматизация параметров видео-FX (Р.30): полная замена кривых
    /// клипа (параллельно fx, как fx_bypass). UI шлёт coalesced-жестом.
    SetFxAuto {
        id: ClipId,
        auto: Vec<Vec<ParamCurve>>,
    },
    /// Значения генератора НА СОБЫТИИ (Р.59, стадия 2).
    ///
    /// Плана кадра не трогает (`touches_render_plan() == false`): адрес
    /// источника считается от события, а не от значений, и они доезжают
    /// до ЖИВОГО источника полосой `retune` — ровно то, ради чего эта
    /// полоса заводилась в Р.53. Иначе каждое нажатие клавиши в титре
    /// останавливало бы аудио-render-поток.
    SetClipGen {
        id: ClipId,
        kind: Option<GeneratorKind>,
    },
    /// Кривые параметров генератора (домен — тик материала, как fx_auto).
    SetClipGenAuto {
        id: ClipId,
        auto: Vec<ParamCurve>,
    },
    /// «Править во всех местах разом» — ОТДЕЛЬНАЯ команда (решение
    /// владельца Р.59): правка одного события соседей больше не трогает,
    /// а «мест: N» остаётся доступно и умещается в один шаг истории.
    ApplyGenToAll {
        media: MediaId,
        kind: GeneratorKind,
        auto: Vec<ParamCurve>,
    },
    LinkClips {
        ids: Vec<ClipId>,
        group: GroupId,
    },
    UnlinkClip {
        id: ClipId,
    },
    /// Переход на стыке двух соседних клипов одной дорожки (Р.24). Undo-able.
    AddTransition(TransitionEvent),
    RemoveTransition {
        id: TransitionId,
    },
    /// Полная замена перехода (драг краёв/тела — батчи и apply_coalesced).
    SetTransition(TransitionEvent),
    /// Метка; duration > 0 = диапазонная (Р.24, marked_range).
    AddMarker {
        at: Tick,
        duration: Tick,
    },
    RemoveMarker {
        at: Tick,
    },
    /// Задать/сменить имя метки (п.1 р.10). Отдельная команда: создание
    /// с именем = батч [AddMarker, SetMarkerName] (один undo-шаг), живой
    /// ввод — apply_coalesced по токену жеста.
    SetMarkerName {
        at: Tick,
        name: String,
    },
    MoveMarker {
        from: Tick,
        to: Tick,
    },
    /// Длительность метки (0 = схлопнуть в точечную). Края ленты — жестами.
    SetMarkerDuration {
        at: Tick,
        duration: Tick,
    },
    /// Вставить пустую дорожку вида `kind` с рангом `at` (Р.62: одна
    /// команда на все четыре вида); номера содержимого вида ≥ at сдвигаются
    /// на +1, строка встаёт прямо над бывшей `at` (в конец, если её нет).
    InsertTrack {
        kind: LaneKind,
        at: u32,
    },
    /// Удалить дорожку вместе с её содержимым; номера выше съезжают на −1.
    /// Последняя видео- или аудио-дорожка не удаляется (`LastTrack`).
    RemoveTrack {
        kind: LaneKind,
        at: u32,
    },
    /// Свойства дорожки одним куском: имя, высота, выключатели, уровень,
    /// режим наложения (Р.62). Цепочка — `SetTrackFx`.
    SetTrackProps {
        id: TrackId,
        props: TrackProps,
    },
    /// Цепочка FX дорожки: слоты и bypass. Кривых у дорожки нет — её
    /// ключ тик таймлайна, а это второй домен автоматизации (следующий
    /// change), и команда их отвергает, а не принимает молча.
    SetTrackFx {
        id: TrackId,
        fx: Vec<FxInstance>,
        bypass: Vec<bool>,
    },
    SetTrackFxOff {
        id: TrackId,
        off: bool,
    },
    RemoveMedia {
        id: MediaId,
    },
    SetSettings {
        settings: ProjectSettings,
    },
    /// Morpher (порт v1). В отличие от v1 ВСЕ операции undo-able (дефект D6).
    AddMorpher(MorpherEvent),
    RemoveMorpher {
        id: MorphId,
    },
    /// Полная замена события (окно редактора; в UI — coalesced-жест).
    SetMorpher(MorpherEvent),
    /// Bake: деструктивно вписать патч в целевые клипы и удалить событие.
    /// Результат рендера не меняется: effective_clips уже применял то же.
    BakeMorpher {
        id: MorphId,
    },
    /// Порядок строк дорожек сверху вниз (Р.35). С переездом морфера в
    /// слой он влияет на КАРТИНКУ, поэтому живёт в модели и ходит через
    /// undo: перетащил дорожку — изменил монтаж. Полная замена; вектор
    /// нормализуется мутацией, проверять его не нужно.
    SetLaneOrder {
        order: Vec<(LaneKind, u32)>,
    },
    /// Именованные области (Р.35 п.2). Адресация ПО ИМЕНИ: индексы
    /// разъезжаются под undo/redo, а имя — это адрес.
    AddArea(MarkedArea),
    RemoveArea {
        name: String,
    },
    /// Переименование правит и ССЫЛКИ морферов — одним шагом истории.
    RenameArea {
        from: String,
        to: String,
    },
    SetAreaRange {
        name: String,
        start: Tick,
        duration: Tick,
    },
    SetAreaLanes {
        name: String,
        lanes: Vec<(LaneKind, u32)>,
    },
    /// Камеры (Р.29, 3D-композитинг) — по образцу морферных команд.
    AddCamera(CameraEvent),
    /// Полная замена события (окно параметров; в UI — coalesced-жест).
    SetCamera(CameraEvent),
    RemoveCamera {
        id: CameraId,
    },
    // --- вложенные секвенции (Р.50) ---
    /// Завести пустую ленту. Содержимое приезжает отдельной командой:
    /// сворачивание — это пакет «завести, наполнить, убрать оригиналы,
    /// поставить экземпляр», и он обязан быть ОДНИМ шагом истории.
    AddSequence {
        id: MediaId,
        name: String,
    },
    /// Убрать ленту. Отказ, пока на неё смотрит хоть одно событие дерева:
    /// молча осиротить клипы дороже, чем сказать «занята».
    RemoveSequence {
        id: MediaId,
    },
    RenameSequence {
        id: MediaId,
        name: String,
    },
    /// Свои растр и fps ленты (Р.62); `None` — как у проекта.
    SetSequenceSettings {
        id: MediaId,
        raster: Option<(u32, u32)>,
        fps: Option<u32>,
    },
    /// Полная замена СОДЕРЖИМОГО ленты (Box — чтобы `Command` не разнесло
    /// по размеру ради одного варианта).
    SetSequenceContent {
        id: MediaId,
        content: Box<Sequence>,
    },
    /// Сменить источник события — файл или ленту. Несущая команда и
    /// сворачивания, и «сделать независимым».
    SetClipMedia {
        id: ClipId,
        media: MediaId,
    },
    // --- публичные параметры композиции (Р.59, стадия 3) ---
    /// Объявить свойство НАЗВАННОЙ ленты наружу.
    ///
    /// **Лента называется ЯВНО (Р.63 п.11), и это не украшение.** До
    /// этого раунда команда правила «ту ленту, в которой стоишь», и
    /// публикация из раскрытой полосы требовала СНАЧАЛА войти в ленту:
    /// «если опубликовать параметр не входя полностью в композицию — то
    /// зачем-то мы автоматически полностью входим в композицию»
    /// (владелец). Адрес в самой команде снимает и вход, и зависимость
    /// от контекста: ручка ложится в названную ленту, где бы человек ни
    /// стоял, а несуществующая лента — честный отказ.
    PublishParam {
        seq: MediaId,
        param: Box<PublishedParam>,
    },
    /// Снять ручку. Переопределения на неё исчезают тем же шагом истории
    /// (`reconcile_published`) — мёртвой ручки снаружи не остаётся.
    UnpublishParam {
        seq: MediaId,
        id: PubId,
    },
    /// Переименование и сужение пределов — полная замена объявления.
    SetPublished {
        seq: MediaId,
        param: Box<PublishedParam>,
    },
    /// Порядок ручек в окне потребителя — часть авторского интерфейса
    /// (Resolve 21 завёл его отдельно, разведка F-TL-012).
    ReorderPublished {
        seq: MediaId,
        order: Vec<PubId>,
    },
    /// Переопределение у ЭКЗЕМПЛЯРА. Полная замена одной ручки.
    SetOverride {
        id: ClipId,
        ov: Box<Override>,
    },
    ClearOverride {
        id: ClipId,
        pub_id: PubId,
    },
}

impl Command {
    /// Меняет ли команда то, что СЧИТАЮТ рендереры (Р.53, п.412).
    ///
    /// Закон П: путь медиа — свойство ИСТОЧНИКА, а не плана. Рендереры
    /// знают только `MediaId`, поэтому `SetMediaPath` не имеет права
    /// клонировать проект и пересобирать графы: его адресат — воркер
    /// этого медиа (`Player::retune_media`). До этого пункта каждое
    /// нажатие клавиши в поле титра останавливало аудио-render-поток.
    ///
    /// Match ИСЧЕРПЫВАЮЩИЙ и без `_ =>`: новая команда не соберётся,
    /// пока не ответили, трогает ли она план. Прецедент того же рода —
    /// `on_region_changed` без `set_project` (Р.43 п.296, сторож t286).
    /// Событие, которое команда правит, — если оно у неё одно и ЕСТЬ.
    ///
    /// Заведено ради одного вопроса: «в какой ЛЕНТЕ эта правка» (Р.55, С5).
    /// Перебирать варианты в вызывающем нельзя — это второе место, где
    /// знают устройство команд, и оно разошлось бы с первым.
    pub fn target_clip(&self) -> Option<ClipId> {
        match self {
            Command::AddClip(c) => Some(c.id),
            Command::RemoveClip { id }
            | Command::MoveClip { id, .. }
            | Command::TrimStart { id, .. }
            | Command::TrimEnd { id, .. }
            | Command::Split { id, .. }
            | Command::SetGain { id, .. }
            | Command::SetOpacity { id, .. }
            | Command::SetGeometry { id, .. }
            | Command::SetFxOff { id, .. }
            | Command::SetReversed { id, .. }
            | Command::SetFxBypass { id, .. }
            | Command::SetFx { id, .. }
            | Command::SetFxAuto { id, .. }
            | Command::SetClipGen { id, .. }
            | Command::SetClipGenAuto { id, .. }
            | Command::UnlinkClip { id }
            | Command::SetOverride { id, .. }
            | Command::ClearOverride { id, .. }
            | Command::SetClipMedia { id, .. } => Some(*id),
            _ => None,
        }
    }

    /// СТРОКИ, содержимое которых команда правит (Р.62, change B) —
    /// ЕДИНСТВЕННАЯ функция, по которой проверяется замок дорожки.
    ///
    /// Match ИСЧЕРПЫВАЮЩИЙ и без `_ =>`: новая команда не соберётся, пока
    /// её не отнесли к одной из двух половин — «трогает события» (тогда
    /// назови строки) или «не трогает» (пустой список). Это тот же приём,
    /// что у `Band::after_batch`, и по той же причине: пропущенная строка
    /// здесь означает жест, прошедший сквозь замок молча.
    ///
    /// Свойства САМОЙ дорожки (`SetTrackProps`) под замок не попадают —
    /// иначе запертую дорожку нельзя было бы отпереть.
    pub fn target_rows(&self, p: &Project) -> Vec<(LaneKind, u32)> {
        let of_clip =
            |id: ClipId| p.clips.iter().find(|c| c.id == id).map(|c| (lane_key_of(c.kind), c.lane));
        let mut out: Vec<(LaneKind, u32)> = Vec::new();
        let mut add = |row: Option<(LaneKind, u32)>| {
            if let Some(r) = row
                && !out.contains(&r)
            {
                out.push(r);
            }
        };
        match self {
            // события клипов: и откуда берут, и куда кладут
            Command::AddClip(c) => add(Some((lane_key_of(c.kind), c.lane))),
            Command::MoveClip { id, new_lane, .. } => {
                add(of_clip(*id));
                add(of_clip(*id).map(|(k, _)| (k, *new_lane)));
            }
            Command::RemoveClip { id }
            | Command::TrimStart { id, .. }
            | Command::TrimEnd { id, .. }
            | Command::Split { id, .. }
            | Command::SetGain { id, .. }
            | Command::SetOpacity { id, .. }
            | Command::SetGeometry { id, .. }
            | Command::SetFxOff { id, .. }
            | Command::SetReversed { id, .. }
            | Command::SetFxBypass { id, .. }
            | Command::SetFx { id, .. }
            | Command::SetFxAuto { id, .. }
            | Command::SetClipGen { id, .. }
            | Command::SetClipGenAuto { id, .. }
            | Command::UnlinkClip { id }
            | Command::SetOverride { id, .. }
            | Command::ClearOverride { id, .. }
            | Command::SetClipMedia { id, .. } => add(of_clip(*id)),
            Command::LinkClips { ids, .. } => {
                for id in ids {
                    add(of_clip(*id));
                }
            }
            // «во всех местах разом»: строки ВСЕХ событий этого медиа
            Command::ApplyGenToAll { media, .. } => {
                for c in p.clips.iter().filter(|c| c.media == *media) {
                    add(Some((lane_key_of(c.kind), c.lane)));
                }
            }
            // переходы живут НА СТЫКЕ двух событий — запирается любая из строк
            Command::AddTransition(t) | Command::SetTransition(t) => {
                add(t.a.and_then(of_clip));
                add(t.b.and_then(of_clip));
            }
            Command::RemoveTransition { id } => {
                if let Some(t) = p.transitions.iter().find(|t| t.id == *id) {
                    add(t.a.and_then(of_clip));
                    add(t.b.and_then(of_clip));
                }
            }
            // морферы и камеры — события своих служебных строк
            Command::AddMorpher(m) | Command::SetMorpher(m) => add(Some((LaneKind::Morph, m.lane))),
            Command::RemoveMorpher { id } | Command::BakeMorpher { id } => {
                add(p.morphers.iter().find(|m| m.id == *id).map(|m| (LaneKind::Morph, m.lane)));
            }
            Command::AddCamera(c) | Command::SetCamera(c) => add(Some((LaneKind::Camera, c.lane))),
            Command::RemoveCamera { id } => {
                add(p.cameras.iter().find(|c| c.id == *id).map(|c| (LaneKind::Camera, c.lane)));
            }
            // удаление дорожки уносит её события — это тоже правка содержимого
            Command::RemoveTrack { kind, at } => add(Some((*kind, *at))),
            // СОДЕРЖИМОГО СТРОК НЕ ТРОГАЮТ — перечислено поимённо
            Command::NoteBirth { .. }
            | Command::AddMedia { .. }
            | Command::SetMediaPath { .. }
            | Command::RemoveMedia { .. }
            | Command::SetSettings { .. }
            | Command::SetTrimStretchesAuto { .. }
            | Command::AddMarker { .. }
            | Command::RemoveMarker { .. }
            | Command::SetMarkerName { .. }
            | Command::MoveMarker { .. }
            | Command::SetMarkerDuration { .. }
            | Command::InsertTrack { .. }
            | Command::SetTrackProps { .. }
            | Command::SetTrackFx { .. }
            | Command::SetTrackFxOff { .. }
            | Command::SetLaneOrder { .. }
            | Command::AddArea(_)
            | Command::RemoveArea { .. }
            | Command::RenameArea { .. }
            | Command::SetAreaRange { .. }
            | Command::SetAreaLanes { .. }
            | Command::AddSequence { .. }
            | Command::RemoveSequence { .. }
            | Command::RenameSequence { .. }
            | Command::SetSequenceSettings { .. }
            | Command::SetSequenceContent { .. }
            | Command::PublishParam { .. }
            | Command::UnpublishParam { .. }
            | Command::SetPublished { .. }
            | Command::ReorderPublished { .. } => {}
        }
        out
    }

    pub fn touches_render_plan(&self) -> bool {
        match self {
            // Значения генератора — свойство ИСТОЧНИКА, а не плана (тот
            // же закон П, что у пути медиа). Их адресат — воркер этого
            // события, `Player::update_gen`.
            Command::SetMediaPath { .. }
            | Command::SetClipGen { .. }
            | Command::SetClipGenAuto { .. } => false,
            Command::NoteBirth { .. }
            | Command::AddMedia { .. }
            | Command::AddClip(_)
            | Command::RemoveClip { .. }
            | Command::MoveClip { .. }
            | Command::TrimStart { .. }
            | Command::TrimEnd { .. }
            | Command::Split { .. }
            | Command::SetTrimStretchesAuto { .. }
            | Command::SetGain { .. }
            | Command::SetOpacity { .. }
            | Command::SetGeometry { .. }
            | Command::SetFxOff { .. }
            | Command::SetReversed { .. }
            | Command::SetFxBypass { .. }
            | Command::SetFx { .. }
            | Command::SetFxAuto { .. }
            | Command::ApplyGenToAll { .. }
            | Command::LinkClips { .. }
            | Command::UnlinkClip { .. }
            | Command::AddTransition(_)
            | Command::RemoveTransition { .. }
            | Command::SetTransition(_)
            | Command::AddMarker { .. }
            | Command::RemoveMarker { .. }
            | Command::SetMarkerName { .. }
            | Command::MoveMarker { .. }
            | Command::SetMarkerDuration { .. }
            | Command::InsertTrack { .. }
            | Command::RemoveTrack { .. }
            | Command::SetTrackProps { .. }
            | Command::SetTrackFx { .. }
            | Command::SetTrackFxOff { .. }
            | Command::RemoveMedia { .. }
            | Command::SetSettings { .. }
            | Command::AddMorpher(_)
            | Command::RemoveMorpher { .. }
            | Command::SetMorpher(_)
            | Command::BakeMorpher { .. }
            | Command::SetLaneOrder { .. }
            | Command::AddArea(_)
            | Command::RemoveArea { .. }
            | Command::RenameArea { .. }
            | Command::SetAreaRange { .. }
            | Command::SetAreaLanes { .. }
            | Command::AddCamera(_)
            | Command::SetCamera(_)
            | Command::RemoveCamera { .. }
            | Command::AddSequence { .. }
            | Command::RemoveSequence { .. }
            | Command::RenameSequence { .. }
            | Command::SetSequenceSettings { .. }
            | Command::SetSequenceContent { .. }
            // Переопределение и публикация меняют СОДЕРЖИМОЕ ленты,
            // какой её видит движок: редакция становится другой, план
            // обязан пересобраться. Закон П сюда не распространяется —
            // полосой `retune` такое не довезёшь.
            | Command::PublishParam { .. }
            | Command::UnpublishParam { .. }
            | Command::SetPublished { .. }
            | Command::ReorderPublished { .. }
            | Command::SetOverride { .. }
            | Command::ClearOverride { .. }
            | Command::SetClipMedia { .. } => true,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CommandError {
    #[error("clip {0:?} not found")]
    ClipNotFound(ClipId),
    #[error("media {0:?} not found")]
    MediaNotFound(MediaId),
    #[error("id {0:?} already in use")]
    IdInUse(u64),
    #[error("result would violate minimal duration")]
    TooShort,
    #[error("marked area {0:?} not found")]
    AreaNotFound(String),
    #[error("marked area: name must not be empty")]
    AreaUnnamed,
    #[error("name {0:?} is already taken (areas and named ranges share one namespace)")]
    NameTaken(String),
    #[error("split point outside clip")]
    SplitOutside,
    #[error("published parameter {0:?} not found")]
    PublishedNotFound(PubId),
    #[error("the new order is not a permutation of the published set")]
    PublishedOrderBroken,
    #[error("invalid clip: {0}")]
    InvalidClip(#[from] ProjectError),
    #[error("transition {0:?} not found")]
    TransitionNotFound(TransitionId),
    #[error("layout invariant violated: {0}")]
    InvalidLayout(ProjectError),
    #[error("invalid transition: {0}")]
    InvalidTransition(ProjectError),
    #[error("marker at {0:?} not found")]
    MarkerNotFound(Tick),
    #[error("marker at {0:?} already exists")]
    MarkerExists(Tick),
    #[error("media {0:?} is used by clips")]
    MediaInUse(MediaId),
    #[error("sidechain key {0:?} is not an audio track of this reel (or is the track itself)")]
    SidechainKeyInvalid(TrackId),
    #[error("sidechain key would make a cycle through track {0:?}")]
    SidechainCycle(TrackId),
    #[error("sidechain key belongs on a track chain, not on a clip")]
    SidechainOnClip,
    #[error("lane count would orphan clips")]
    LanesTooFew,
    #[error("{0:?} track {1} does not exist")]
    TrackMissing(LaneKind, u32),
    #[error("the last {0:?} track cannot be removed")]
    LastTrack(LaneKind),
    #[error("track {0:?} not found")]
    TrackNotFound(TrackId),
    #[error("track {0:?}: property out of range")]
    TrackPropOutOfRange(TrackId),
    #[error("track {0:?}: curves on a track are not supported yet")]
    TrackCurvesUnsupported(TrackId),
    #[error("track {0:?} is locked")]
    TrackLocked(TrackId),
    #[error("morpher {0:?} not found")]
    MorpherNotFound(MorphId),
    #[error("morpher needs at least one target clip")]
    MorpherNoTargets,
    #[error("morph lane {0} not found")]
    MorphLaneNotFound(u32),
    #[error("camera {0:?} not found")]
    CameraNotFound(CameraId),
    #[error("invalid camera: {0}")]
    InvalidCamera(ProjectError),
    #[error("camera lane {0} not found")]
    CameraLaneNotFound(u32),
    #[error("sequence {0:?} not found")]
    SequenceNotFound(MediaId),
    #[error("sequence {0:?} is still used by {1} clip(s)")]
    SequenceInUse(MediaId, usize),
    #[error("row order and lane count cannot change while a reel is expanded in place")]
    BandsOpen,
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
}

/// Модель с историей. Движок читает `project()` (снапшот-клон при rebuild).
#[derive(Debug, Clone)]
pub struct TimelineModel {
    /// КАНОН: настоящая раскладка без раскрытых полос. Он уходит в файл,
    /// в плеер и в экспорт, и только он существует по-настоящему.
    project: Project,
    /// ПРОИЗВОДНОЕ: канон плюс раскрытые на месте ленты (Р.55, С5).
    ///
    /// Его отдаёт `project()`, то есть с ним работает весь интерфейс —
    /// отрисовка, хит-тест, снап, выделение, окна. Ни одна из этих рук не
    /// знает, что часть событий на экране живёт в другой ленте: они
    /// физически лежат в корневых векторах, только со смещением.
    /// При пустом `bands` это побайтовая копия канона, то есть прежнее
    /// поведение без единой лишней ветки.
    bench: Project,
    /// Раскрытые полосы. Рабочее место, а не работа: в файл не пишется
    /// (правило t390-t391), но КАЖДЫЙ undo-снимок помнит свои — иначе
    /// Ctrl+Z внутри раскрытия выглядел бы как «нажал, и всё пропало».
    bands: Vec<Band>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Токен последнего coalesced-жеста (см. apply_coalesced).
    last_gesture: Option<u64>,
    /// Лента, которую сейчас редактируют (Р.50, В1). `None` — корень.
    ///
    /// Живёт В МОДЕЛИ, а не в проекте: это не монтаж, а место, где стоит
    /// человек. В файл не пишется (вид внутри ленты — рабочее место, а не
    /// работа) и в undo-снимок не входит — но КАЖДЫЙ снимок помнит, в
    /// каком контексте правка делалась.
    editing: Option<MediaId>,
}

/// Снимок истории: проект и контекст, в котором правка делалась (Р.50, В1).
///
/// Без контекста Ctrl+Z внутри ленты выглядел бы как «нажал — ничего не
/// произошло»: отменилась бы правка в другом месте, которого не видно.
#[derive(Debug, Clone)]
struct Snapshot {
    project: Project,
    scope: Option<MediaId>,
    /// Какие полосы были раскрыты в момент правки (Р.55, С5).
    bands: Vec<Band>,
}

impl Band {
    /// Пересчитать полосу после пакета команд (Р.55, С5).
    ///
    /// **Единственное место конструкции, где правильность держится на
    /// человеке, а не на построении.** Поэтому `match` ИСЧЕРПЫВАЮЩИЙ, без
    /// `_ =>`: новая команда не соберётся, пока строку не заполнили — тот
    /// же приём, что у `Command::touches_render_plain` и таблицы t253.
    ///
    /// Нумерацию дорожек меняют ровно шесть команд. Полоса лежит в ХВОСТЕ
    /// числового пространства, поэтому правило простое: дорожка вставлена
    /// или убрана НИЖЕ начала полосы — едет вся полоса (`base`); внутри
    /// полосы — меняется её размер (`len`).
    ///
    /// Ошибка здесь не падает ни одним ассертом сама: полоса «съезжает», и
    /// `unmerge_reel` начинает отвергать батчи — либо, при симметричной
    /// ошибке, молча кладёт событие не в ту ленту. Сторож — t568.
    pub fn after_batch(&mut self, cmds: &[Command]) {
        for cmd in cmds {
            match cmd {
                Command::InsertTrack { kind, at } => self.lane_inserted(*kind, *at),
                // `SetLanes` ПОЛОСУ НЕ ДВИГАЕТ, и это решение, а не
                // недосмотр. Команда задаёт число дорожек целиком, а клипы
                // при этом не перенумеровываются — значит граница полосы
                // (`base`) обязана остаться там, где события и лежат.
                //
                // Единственный, кто её шлёт, — удаление дорожки, и оно
                // сопровождается `MoveClip` для всех, кто выше. Поэтому
                // удалять дорожку при раскрытой полосе приложение не даёт:
                // сначала сворачивает раскрытие и говорит об этом.
                //
                // `SetLaneOrder` с Р.56 перенумеровывает содержимое (номер
                // дорожки = ранг строки, п.449) — на бенче это переставило
                // бы события полосы из ХВОСТА числового пространства, и
                // `unmerge_reel` разобрал бы их не по тем лентам. Обе
                // команды отвергает `simulate` при открытой полосе
                // (`CommandError::BandsOpen`), сюда они не доходят.
                Command::RemoveTrack { .. } | Command::SetLaneOrder { .. } => {
                    debug_assert!(
                        false,
                        "RemoveTrack/SetLaneOrder are refused while a band is open"
                    )
                }
                // Всё остальное номеров дорожек не двигает. Перечислено
                // поимённо НАРОЧНО: `_ =>` спрятал бы следующую команду,
                // которая начнёт их двигать.
                Command::NoteBirth { .. }
                | Command::AddMedia { .. }
                | Command::SetMediaPath { .. }
                | Command::AddClip(_)
                | Command::RemoveClip { .. }
                | Command::MoveClip { .. }
                | Command::TrimStart { .. }
                | Command::TrimEnd { .. }
                | Command::Split { .. }
                | Command::SetTrimStretchesAuto { .. }
                | Command::SetGain { .. }
                | Command::SetOpacity { .. }
                | Command::SetGeometry { .. }
                | Command::SetFxOff { .. }
                | Command::SetReversed { .. }
                | Command::SetFxBypass { .. }
                | Command::SetFx { .. }
                | Command::SetFxAuto { .. }
                | Command::SetClipGen { .. }
                | Command::SetClipGenAuto { .. }
                | Command::ApplyGenToAll { .. }
                | Command::LinkClips { .. }
                | Command::UnlinkClip { .. }
                | Command::AddTransition(_)
                | Command::RemoveTransition { .. }
                | Command::SetTransition(_)
                | Command::AddMarker { .. }
                | Command::RemoveMarker { .. }
                | Command::SetMarkerName { .. }
                | Command::MoveMarker { .. }
                | Command::SetMarkerDuration { .. }
                | Command::RemoveMedia { .. }
                | Command::SetSettings { .. }
                | Command::AddMorpher(_)
                | Command::RemoveMorpher { .. }
                | Command::SetMorpher(_)
                | Command::BakeMorpher { .. }
                | Command::AddArea(_)
                | Command::RemoveArea { .. }
                | Command::RenameArea { .. }
                | Command::SetAreaRange { .. }
                | Command::SetAreaLanes { .. }
                | Command::SetTrackProps { .. }
                | Command::SetTrackFx { .. }
                | Command::SetTrackFxOff { .. }
                | Command::AddCamera(_)
                | Command::SetCamera(_)
                | Command::RemoveCamera { .. }
                | Command::AddSequence { .. }
                | Command::RemoveSequence { .. }
                | Command::RenameSequence { .. }
                | Command::SetSequenceSettings { .. }
                | Command::SetSequenceContent { .. }
                | Command::PublishParam { .. }
                | Command::UnpublishParam { .. }
                | Command::SetPublished { .. }
                | Command::ReorderPublished { .. }
                | Command::SetOverride { .. }
                | Command::ClearOverride { .. }
                | Command::SetClipMedia { .. } => {}
            }
        }
    }

    fn lane_inserted(&mut self, kind: LaneKind, at: u32) {
        let base = self.base.get(kind);
        if at <= base {
            *self.base.get_mut(kind) += 1;
        } else if at <= base + self.len.get(kind) {
            // ХВОСТ полосы принадлежит ПОЛОСЕ (Р.56): за ней ничего не
            // лежит по построению, а вставка ровно в `base + len` —
            // это «ленте нужна ещё одна дорожка» (вставка в раскрытии).
            // До Р.56 такая дорожка не доставалась никому, и событие на
            // ней молча уезжало в корень.
            *self.len.get_mut(kind) += 1;
        }
    }
}

/// Кто от кого родился в пакете команд (Р.49, п.357). Родство сообщает
/// МОДЕЛЬ, а не догадывается вызывающий: у `Split` оно прямо в аргументах, у
/// остальных производных — в заметке `NoteBirth`, которую оставляет тот, кто
/// резал. Догадка по совпадению материала и окна была бы именно догадкой:
/// после перенумерации отличить «половину разреза» от «соседнего события того
/// же файла» нечем.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Births {
    /// Пары «новое событие ← родитель», в порядке рождения.
    pub pairs: Vec<(ClipId, ClipId)>,
}

impl Births {
    fn of(cmds: &[Command]) -> Births {
        let mut pairs = Vec::new();
        for c in cmds {
            match c {
                Command::Split { id, new_id, .. } => pairs.push((*new_id, *id)),
                Command::NoteBirth { child, parent } => pairs.push((*child, *parent)),
                _ => {}
            }
        }
        Births { pairs }
    }

    pub fn parent_of(&self, child: ClipId) -> Option<ClipId> {
        self.pairs.iter().find(|(c, _)| *c == child).map(|(_, p)| *p)
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

impl TimelineModel {
    pub fn new(project: Project) -> TimelineModel {
        TimelineModel {
            bench: project.clone(),
            project,
            bands: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            last_gesture: None,
            editing: None,
        }
    }

    /// Пересобрать рабочий вид из канона. Зовётся ПОСЛЕ каждой правки
    /// канона — включая правки полей вне истории (регион, loop, ui).
    fn rebuild_bench(&mut self) {
        self.bench = self.project.clone();
        for b in &self.bands {
            self.bench.merge_reel(b);
        }
    }

    /// Раскрытые полосы.
    pub fn bands(&self) -> &[Band] {
        &self.bands
    }

    /// Раскрыть ленту экземпляра НА МЕСТЕ (Р.55, С5). `false` — это не
    /// вложение, либо оно уже раскрыто.
    pub fn expand_band(&mut self, inst: ClipId) -> bool {
        if self.bands.iter().any(|b| b.inst == inst) {
            return false;
        }
        // полоса считается от КАНОНА: `base` обязан быть числом корневых
        // дорожек, а не «сколько их сейчас на экране вместе с чужой полосой».
        // ВНУТРИ полного входа (Р.58, п.460) канон — это текущая лента
        // корнем (`swap_reel`), и `Band::of` считает базы от неё же; ни
        // одной особой ветки не нужно. Раскрыть саму ленту, в которой
        // стоим, невозможно по циклам (`validate`), сторож — на случай
        // ручной сборки проекта в обход валидации.
        let Some(b) = Band::of(&self.project, inst) else { return false };
        debug_assert!(
            Some(b.seq) != self.editing,
            "expanding the reel being edited inside itself: a cycle escaped validation"
        );
        self.bands.push(b);
        self.rebuild_bench();
        true
    }

    /// Свернуть раскрытие обратно.
    pub fn collapse_band(&mut self, inst: ClipId) -> bool {
        let Some(i) = self.bands.iter().position(|b| b.inst == inst) else { return false };
        self.bands.remove(i);
        self.rebuild_bench();
        true
    }

    pub fn collapse_all_bands(&mut self) {
        if self.bands.is_empty() {
            return;
        }
        self.bands.clear();
        self.rebuild_bench();
    }

    /// Чья это строка: корневая или раскрытой полосы.
    pub fn band_of_row(&self, kind: LaneKind, lane: u32) -> Option<&Band> {
        self.bands.iter().find(|b| b.owns(kind, lane))
    }

    /// Лента, которую сейчас редактируют. `None` — корень.
    pub fn editing(&self) -> Option<MediaId> {
        self.editing
    }

    /// Войти в ленту (или выйти в корень). Несуществующий адрес —
    /// молчаливый выход в корень: лента могла исчезнуть под undo.
    pub fn set_editing(&mut self, seq: Option<MediaId>) {
        let want = seq.filter(|id| {
            // адрес ищем в ДОКУМЕНТЕ: стоя внутри X, сам X в
            // `project.sequences` держит содержимое корня, но id у него
            // прежний — этого и достаточно
            self.project.sequence(*id).is_some()
        });
        if want == self.editing {
            return;
        }
        // Полный вход и раскрытие на месте — ДВА способа смотреть внутрь,
        // и одновременно они бессмысленны: полоса считалась от корня, а
        // корнем станет другая лента.
        self.bands.clear();
        if let Some(cur) = self.editing {
            self.project.swap_reel(cur);
        }
        if let Some(next) = want {
            self.project.swap_reel(next);
        }
        self.editing = want;
        self.last_gesture = None;
        self.rebuild_bench();
    }

    /// Документ, в котором ТЕКУЩАЯ лента стоит КОРНЕМ (Р.50, В1).
    ///
    /// Нужен офлайн-путям, считающим по времени того, что под руками:
    /// громкость, дакинг, превью материала. Рабочий вид им отдавать
    /// нельзя — в нём лента, внутри которой стоим, держит содержимое
    /// корня и выглядит вложенной в саму себя.
    pub fn reel_document(&self) -> Project {
        let mut p = self.project.clone();
        if let Some(cur) = self.editing {
            p.sequences.retain(|s| s.id != cur);
        }
        p
    }

    /// Проект КАК ЕГО ВИДИТ ЧЕЛОВЕК: содержимое ленты, в которой он сейчас
    /// стоит, лежит на месте корня (Р.50, В1), а раскрытые на месте ленты
    /// разложены на дорожки со смещением (Р.55, С5).
    ///
    /// Весь интерфейс — отрисовка, хит-тест, планирование жестов, снап,
    /// выделение, окна FX и морфера — работает с этим видом и о вложениях
    /// не знает вовсе. Тому, кому нужен ДОКУМЕНТ (движок, сохранение,
    /// экспорт), нужен `document()`; сторож — сканирующий тест t456b.
    ///
    /// Имя НЕ переименовывается в `bench` намеренно: `project()` зовут
    /// 182 раза, и все эти вызовы правильны — им нужен именно рабочий вид.
    /// Переименование ничего не прояснило бы, а различие «канон против
    /// вида» живёт в доке полей и в t456b.
    #[allow(clippy::misnamed_getters)]
    pub fn project(&self) -> &Project {
        &self.bench
    }

    /// ДОКУМЕНТ в настоящей раскладке: корень корнем, ленты лентами.
    ///
    /// Именно он уходит в плеер, в файл и в экспорт. Отдать туда рабочий
    /// вид значило бы записать на диск проект, у которого содержимое
    /// корня и ленты поменялись местами — потеря работы, которую человек
    /// заметил бы только при следующем открытии.
    pub fn document(&self) -> Project {
        let mut p = self.project.clone();
        if let Some(seq) = self.editing {
            p.swap_reel(seq);
        }
        p
    }

    /// Событие ПО ТОМУ, ЧТО ВИДИТ ЧЕЛОВЕК (Р.55, С5).
    ///
    /// Ищет в рабочем виде, а не в каноне: интерфейс спрашивает про то, по
    /// чему щёлкнул, а событие раскрытой полосы в каноне лежит внутри
    /// ленты. Искал бы здесь канон — каждый жест по полосе отвечал бы
    /// «такого события нет», причём молча.
    pub fn clip(&self, id: ClipId) -> Option<&ClipEvent> {
        self.bench.clips.iter().find(|c| c.id == id)
    }

    /// Полуоткрытый интервал клипа на таймлайне.
    pub fn clip_range(c: &ClipEvent) -> TickRange {
        TickRange::new(c.start, c.start + c.duration)
    }

    pub fn apply(&mut self, cmd: Command) -> Result<(), CommandError> {
        self.apply_batch(vec![cmd]).map(|_| ())
    }

    /// Пакет команд = ОДИН undo-снапшот на жест (сплит A/V-пары, удаление
    /// группы, paste, punch-out). Атомарно: валидация всей серии на клоне,
    /// провал любой команды = ноль изменений и ноль записей в истории.
    pub fn apply_batch(&mut self, cmds: Vec<Command>) -> Result<Births, CommandError> {
        if cmds.is_empty() {
            return Ok(Births::default());
        }
        let scratch = self.simulate(&cmds)?;
        let births = Births::of(&cmds);
        self.push_undo();
        self.project = scratch;
        // ПОЛОСЫ могли пережить правку с другими номерами дорожек
        // (`InsertLane` в корне двигает всю полосу вниз): пересчёт — там
        // же, где применяется результат, и только один раз.
        for b in &mut self.bands {
            b.after_batch(&cmds);
        }
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.last_gesture = None;
        Ok(births)
    }

    /// Проект, каким он СТАНЕТ после батча — без записи в undo (Р.26).
    /// Нужен call-site'ам, которым надо посчитать что-то по будущему
    /// состоянию: возврат «ручек» кроссфейда считает свободное место уже
    /// после того, как клип уехал (ops::restore_handles).
    pub fn preview(&self, cmds: &[Command]) -> Result<Project, CommandError> {
        // РАБОЧИЙ ВИД будущего, а не канон: вызывающий сравнивает его с
        // `project()` (возврат «ручек» кроссфейда, Р.26). Сравни он канон
        // с бенчем — и материал начал бы срезаться у чужих событий тихо и
        // с undo.
        let canon = self.simulate(cmds)?;
        let mut bench = canon;
        for b in &self.bands {
            bench.merge_reel(b);
        }
        Ok(bench)
    }

    fn simulate(&self, cmds: &[Command]) -> Result<Project, CommandError> {
        // ПОРЯДОК И ЧИСЛО СТРОК при открытой полосе не меняются (Р.56,
        // п.449). С Р.56 `SetLaneOrder` перенумеровывает содержимое, а
        // полоса раскрытия намеренно живёт в ХВОСТЕ числового
        // пространства: перенумеруй мы бенч — `unmerge_reel` разложил бы
        // события не по тем лентам, причём МОЛЧА. Приложение сворачивает
        // раскрытие само и говорит об этом.
        if !self.bands.is_empty()
            && cmds
                .iter()
                .any(|c| matches!(c, Command::SetLaneOrder { .. } | Command::RemoveTrack { .. }))
        {
            return Err(CommandError::BandsOpen);
        }
        // Скретч уже в РАБОЧЕМ виде: содержимое текущей ленты стоит на
        // месте корня (Р.50, В1), и команда правит его, не зная о
        // вложениях вовсе.
        let mut scratch = self.bench.clone();
        // ГДЕ СТОИМ: нужно только адресным командам публикации — набор
        // ленты, в которой стоишь, лежит в корне рабочего вида, а
        // остальных — в `sequences` (Р.63 п.11)
        let scope = self.editing;
        let run = |scratch: &mut Project| -> Result<(), CommandError> {
            for cmd in cmds {
                Self::check_on(scratch, scope, cmd)?;
                Self::mutate_on(scratch, scope, cmd.clone());
            }
            Ok(())
        };
        let res = run(&mut scratch);
        // reconcile — ОДИН раз на весь батч: жест атомарен, и переход не
        // должен умирать в переходном состоянии между командами (roll стыка
        // = TrimEnd+TrimStart: между ними стык формально разорван)
        Self::reconcile_transitions(&mut scratch);
        res?;
        // РАСКРЫТЫЕ ПОЛОСЫ собираются обратно в свои ленты (Р.55, С5) —
        // в обратном порядке, потому что каждая следующая раскрывалась
        // поверх уже раскрытых. Ошибка здесь значит, что жест вынес
        // событие через границу лент: батч отвергается ЦЕЛИКОМ, а не
        // «доправляется».
        let mut bands = self.bands.clone();
        for b in &mut bands {
            b.after_batch(cmds);
        }
        for b in bands.iter().rev() {
            scratch.unmerge_reel(b).map_err(CommandError::InvalidLayout)?;
        }
        // Судьба публикаций — ТЕМ ЖЕ шагом истории и по тому же образцу,
        // что у переходов: цель исчезла — исчезает и ручка, и все её
        // переопределения во всех экземплярах дерева. Иначе снаружи
        // остаётся ручка, которая ни на что не действует, и узнаёт об
        // этом потребитель (минус разведки F-TL-012).
        //
        // ПОСЛЕ сборки полос, и это не порядок ради порядка: пока полоса
        // раскрыта, содержимое ленты физически лежит в корне, а её
        // публикации остаются в `sequences` — каждая цель выглядела бы
        // пропавшей, и раскрытие тихо сносило бы весь публичный набор.
        Self::reconcile_published(&mut scratch, self.editing);
        // Валидация — ПО ДОКУМЕНТУ: в рабочем виде экземпляр ленты, в
        // которой стоим, лежит внутри неё самой и выглядит циклом.
        if let Some(seq) = self.editing {
            scratch.swap_reel(seq);
        }
        // Жёсткая валидация результата (Р.24, этап 6): перекрытия после
        // групповых мувов/тримов/вставок дешевле отловить одной проверкой,
        // чем дублировать их логику в check_on. Провал = ноль изменений.
        let ok = scratch.validate().map_err(CommandError::InvalidLayout);
        if let Some(seq) = self.editing {
            scratch.swap_reel(seq);
        }
        ok?;
        Ok(scratch)
    }

    /// Как apply, но серия вызовов с одним `gesture`-токеном сливается в
    /// одну undo-запись (live-редактирование параметров FX из окна).
    pub fn apply_coalesced(&mut self, cmd: Command, gesture: u64) -> Result<(), CommandError> {
        let scratch = self.simulate(std::slice::from_ref(&cmd))?;
        if self.last_gesture != Some(gesture) {
            self.push_undo();
        }
        self.project = scratch;
        for b in &mut self.bands {
            b.after_batch(std::slice::from_ref(&cmd));
        }
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.last_gesture = Some(gesture);
        Ok(())
    }

    fn push_undo(&mut self) {
        self.undo.push(Snapshot {
            project: self.project.clone(),
            scope: self.editing,
            bands: self.bands.clone(),
        });
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Регион ВНЕ undo-истории (семантика старого редактора).
    pub fn set_region(&mut self, r: Option<Region>) {
        self.project.region = r;
        self.bench.region = r;
    }

    pub fn set_loop_enabled(&mut self, on: bool) {
        self.project.loop_enabled = on;
        self.bench.loop_enabled = on;
    }

    /// ИЗМЕРЕННАЯ длина материала — вне undo (Р.63 п.2).
    ///
    /// Это не правка, а ФАКТ о файле: его приносит тот, кто источник
    /// открыл, и приносит асинхронно (фоновый синк медиа, возврат
    /// пропавшего файла). Командой это быть не может по двум причинам:
    /// откатывать «мы узнали, что файл длится 226 с» нечего, а звёздочка
    /// «проект изменён» от открытия источника — прямое нарушение правила
    /// «правки удобства не делают вид, что пользователь что-то изменил»
    /// (п.5 р.11). Соседи по этой полке — регион, loop и вьюпорт.
    ///
    /// Возвращает `true`, если значение ДЕЙСТВИТЕЛЬНО изменилось: синк
    /// зовут на каждом кадре, и молчаливое «то же самое» не должно
    /// двигать ни ревизию, ни автосейв.
    pub fn note_media_length(&mut self, id: crate::MediaId, duration: kt_time::Tick) -> bool {
        let set = |p: &mut crate::Project| -> bool {
            match p.media.iter_mut().find(|m| m.id == id) {
                Some(m) if m.duration != Some(duration) && duration.0 > 0 => {
                    m.duration = Some(duration);
                    true
                }
                _ => false,
            }
        };
        // длина живёт у медиа ПРОЕКТА (список медиа один на документ), а
        // рабочий вид и канон — разные проекты: правим оба, как регион
        let a = set(&mut self.project);
        let b = set(&mut self.bench);
        a || b
    }

    /// Вьюпорт (вне undo, как регион) — сохраняется в файл проекта.
    pub fn set_ui_state(&mut self, ui: Option<crate::UiState>) {
        self.project.ui = ui.clone();
        self.bench.ui = ui;
    }

    pub fn undo(&mut self) -> Result<(), CommandError> {
        let mut prev = self.undo.pop().ok_or(CommandError::NothingToUndo)?;
        // регион/loop не откатываются — переносим текущие значения
        prev.project.region = self.project.region;
        prev.project.loop_enabled = self.project.loop_enabled;
        // КОНТЕКСТ едет вместе с ПРАВКОЙ, а не с тем, где человек стоял в
        // момент отмены (Р.50, В1): отменяется то, что он сделал ТАМ —
        // значит и показать надо то место. И redo-запись помнит ТОТ ЖЕ
        // контекст, иначе обратный ход уводил бы не туда.
        let here = Snapshot {
            project: self.project.clone(),
            scope: prev.scope,
            bands: self.bands.clone(),
        };
        self.project = prev.project;
        // Снимок уже несёт раскладку СВОЕГО контекста — обмен тут был бы
        // вторым, и содержимое уехало бы не туда.
        self.editing = prev.scope.filter(|id| self.project.sequence(*id).is_some());
        // ПОЛОСЫ едут вместе с контекстом по той же причине, что и он:
        // отменяется правка, сделанная В РАСКРЫТИИ, — значит и показать
        // надо раскрытие, иначе Ctrl+Z выглядит как «нажал, и всё пропало».
        self.bands = prev.bands;
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.redo.push(here);
        self.last_gesture = None;
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), CommandError> {
        let mut next = self.redo.pop().ok_or(CommandError::NothingToRedo)?;
        next.project.region = self.project.region;
        next.project.loop_enabled = self.project.loop_enabled;
        let here = Snapshot {
            project: self.project.clone(),
            scope: next.scope,
            bands: self.bands.clone(),
        };
        self.project = next.project;
        self.editing = next.scope.filter(|id| self.project.sequence(*id).is_some());
        self.bands = next.bands;
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.undo.push(here);
        self.last_gesture = None;
        Ok(())
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Набор публикаций НАЗВАННОЙ ленты в РАБОЧЕМ виде (Р.63 п.11).
    ///
    /// Вход внутрь ленты сделан ОБМЕНОМ содержимого, поэтому набор той
    /// ленты, в которой стоишь, лежит в корне, а всех прочих — в
    /// `sequences`. Раскрытая полоса содержимое в корень переносит, а
    /// набор ленты оставляет на месте (t672) — и это ровно тот случай,
    /// ради которого адрес и заведён.
    fn published_slot(
        p: &Project,
        scope: Option<MediaId>,
        seq: MediaId,
    ) -> Option<&Vec<PublishedParam>> {
        if scope == Some(seq) {
            Some(&p.published)
        } else {
            p.sequence(seq).map(|s| &s.published)
        }
    }

    fn published_slot_mut(
        p: &mut Project,
        scope: Option<MediaId>,
        seq: MediaId,
    ) -> Option<&mut Vec<PublishedParam>> {
        if scope == Some(seq) {
            Some(&mut p.published)
        } else {
            p.sequence_mut(seq).map(|s| &mut s.published)
        }
    }

    fn check_on(p: &Project, scope: Option<MediaId>, cmd: &Command) -> Result<(), CommandError> {
        // ЗАМОК ДОРОЖКИ (Р.62): одна проверка на весь реестр команд.
        // Свойства самой дорожки сюда не попадают — запертую дорожку
        // надо чем-то отпирать.
        for (k, l) in cmd.target_rows(p) {
            if p.track_at(k, l).is_some_and(|t| t.lock) {
                return Err(CommandError::TrackLocked(
                    p.track_at(k, l).map(|t| t.id).unwrap_or_default(),
                ));
            }
        }
        let find =
            |id: ClipId| p.clips.iter().find(|c| c.id == id).ok_or(CommandError::ClipNotFound(id));
        match cmd {
            // заметка о родстве ничего не требует от проекта: она про пакет
            Command::NoteBirth { .. } => {}
            Command::AddMedia { id, .. } => {
                if p.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::IdInUse(id.0));
                }
            }
            Command::SetMediaPath { id, .. } => {
                if !p.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::MediaNotFound(*id));
                }
            }
            Command::AddClip(c) => {
                if p.clips.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::IdInUse(c.id.0));
                }
                // от КЛИПА к источнику — только через `Project::source()`:
                // он смотрит и в файлы, и во вложенные ленты (Р.50)
                if p.source(c.media).is_none() {
                    return Err(CommandError::MediaNotFound(c.media));
                }
                if c.duration < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                if c.source_in.0 < 0 {
                    return Err(CommandError::InvalidClip(ProjectError::NegativeSourceIn(c.id)));
                }
            }
            Command::RemoveClip { id }
            | Command::SetGain { id, .. }
            | Command::SetOpacity { id, .. }
            | Command::SetFxOff { id, .. }
            | Command::SetReversed { id, .. }
            | Command::SetFxBypass { id, .. }
            | Command::UnlinkClip { id }
            | Command::MoveClip { id, .. } => {
                find(*id)?;
            }
            Command::SetFx { id, fx } => {
                find(*id)?;
                // сайдчейн живёт ТОЛЬКО в цепочке дорожки (решение
                // владельца Р.65): у события нет ни своего окна ключа, ни
                // места в порядке стадий
                if crate::sidechain_keys(fx).next().is_some() {
                    return Err(CommandError::SidechainOnClip);
                }
            }
            Command::SetGeometry { id, pos, .. } => {
                let c = find(*id)?;
                if *pos > c.fx.len() {
                    return Err(CommandError::InvalidClip(ProjectError::BadGeometryPos(*id)));
                }
            }
            Command::SetFxAuto { id, auto } => {
                find(*id)?;
                check_fx_auto(*id, auto).map_err(CommandError::InvalidClip)?;
            }
            Command::TrimStart { id, new_start } => {
                let c = find(*id)?;
                let end = c.start + c.duration;
                if end - *new_start < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                // source_in не может уйти в минус (у reversed левый трим
                // source_in не трогает — п.16). Считает ТА ЖЕ функция, что
                // и мутация: иначе валидация и правка разойдутся на
                // растянутом клипе (Р.39).
                if !slice_fits_source(c, TickRange::new(*new_start, end)) {
                    return Err(CommandError::TooShort);
                }
            }
            Command::TrimEnd { id, new_end } => {
                let c = find(*id)?;
                if *new_end - c.start < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                // у reversed расширение вправо тянет source_in вниз (п.16)
                if c.reversed && (c.source_in + (c.start + c.duration - *new_end)).0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::Split { id, at, new_id, .. } => {
                let c = find(*id)?;
                if p.clips.iter().any(|x| x.id == *new_id) {
                    return Err(CommandError::IdInUse(new_id.0));
                }
                let r = Self::clip_range(c);
                if !(r.contains(*at)
                    && (*at - r.start) >= MIN_CLIP_DURATION
                    && (r.end - *at) >= MIN_CLIP_DURATION)
                {
                    return Err(CommandError::SplitOutside);
                }
            }
            Command::LinkClips { ids, .. } => {
                for id in ids {
                    find(*id)?;
                }
            }
            Command::AddTransition(t) => {
                if p.transitions.iter().any(|x| x.id == t.id) {
                    return Err(CommandError::IdInUse(t.id.0));
                }
                check_transition_full(p, t, None).map_err(CommandError::InvalidTransition)?;
            }
            Command::RemoveTransition { id } => {
                if !p.transitions.iter().any(|t| t.id == *id) {
                    return Err(CommandError::TransitionNotFound(*id));
                }
            }
            Command::SetTransition(t) => {
                if !p.transitions.iter().any(|x| x.id == t.id) {
                    return Err(CommandError::TransitionNotFound(t.id));
                }
                check_transition_full(p, t, Some(t.id)).map_err(CommandError::InvalidTransition)?;
            }
            Command::AddMarker { at, duration } => {
                if p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerExists(*at));
                }
                if duration.0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::RemoveMarker { at } => {
                if !p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
            }
            Command::SetMarkerDuration { at, duration } => {
                if !p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
                if duration.0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::SetMarkerName { at, name } => {
                // одно пространство имён с областями (Р.35 п.2)
                if !name.is_empty()
                    && p.markers.iter().any(|m| m.t == *at && m.duration.0 > 0)
                    && p.taken_names().iter().any(|t| t == name)
                    && !p.markers.iter().any(|m| m.t == *at && m.name == *name)
                {
                    return Err(CommandError::NameTaken(name.clone()));
                }
                if !p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
            }
            Command::MoveMarker { from, to } => {
                if !p.markers.iter().any(|m| m.t == *from) {
                    return Err(CommandError::MarkerNotFound(*from));
                }
                if p.markers.iter().any(|m| m.t == *to) && from != to {
                    return Err(CommandError::MarkerExists(*to));
                }
            }
            Command::InsertTrack { .. } => {}
            Command::RemoveTrack { kind, at } => {
                let n = p.lanes(*kind);
                if *at >= n {
                    return Err(CommandError::TrackMissing(*kind, *at));
                }
                if matches!(kind, LaneKind::Video | LaneKind::Audio) && n <= 1 {
                    return Err(CommandError::LastTrack(*kind));
                }
            }
            Command::SetTrackFx { id, fx, bypass } => {
                if p.track(*id).is_none() {
                    return Err(CommandError::TrackNotFound(*id));
                }
                if bypass.len() > fx.len() {
                    return Err(CommandError::TrackPropOutOfRange(*id));
                }
                // цепочка дорожки — ТОЛЬКО аудио или только видео, по её
                // виду: видео-эффект на аудиодорожке был бы мёртвым слотом
                let kind = p.track(*id).map(|t| t.kind);
                let wrong = fx.iter().any(|f| match kind {
                    Some(LaneKind::Audio) => f.class() != FxClass::Audio,
                    Some(LaneKind::Video) => f.class() == FxClass::Audio,
                    _ => true,
                });
                if wrong {
                    return Err(CommandError::TrackPropOutOfRange(*id));
                }
                // КЛЮЧ САЙДЧЕЙНА (Р.65): только на СВОЮ ленту, только
                // аудиодорожка, не сама себя и без цикла. Отказ назван —
                // молчаливое «ключ не сработал» было бы хуже ошибки.
                for k in crate::sidechain_keys(fx) {
                    let ok = k != *id
                        && p.track(k).is_some_and(|t| t.kind == LaneKind::Audio)
                        && p.tracks.iter().any(|t| t.id == k);
                    if !ok {
                        return Err(CommandError::SidechainKeyInvalid(k));
                    }
                }
                if crate::sidechain_cycle(&p.tracks, *id, fx) {
                    return Err(CommandError::SidechainCycle(*id));
                }
            }
            Command::SetTrackFxOff { id, .. } => {
                if p.track(*id).is_none() {
                    return Err(CommandError::TrackNotFound(*id));
                }
            }
            Command::SetTrackProps { id, props } => {
                if p.track(*id).is_none() {
                    return Err(CommandError::TrackNotFound(*id));
                }
                if !props.opacity.is_finite()
                    || !(0.0..=1.0).contains(&props.opacity)
                    || !props.gain_db.is_finite()
                    || !(TRACK_GAIN_MIN_DB..=TRACK_GAIN_MAX_DB).contains(&props.gain_db)
                    || !props.height.is_finite()
                    || props.height < 0.0
                {
                    return Err(CommandError::TrackPropOutOfRange(*id));
                }
            }

            Command::RemoveMedia { id } => {
                if !p.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::MediaNotFound(*id));
                }
                // ПО ВСЕМУ ДЕРЕВУ, а не только по корню (Р.50): файл, стоящий
                // лишь внутри ленты, удалился бы молча, и события повисли бы
                // на несуществующем адресе — всплыло бы это через неделю
                if p.sequence_uses(*id) > 0 {
                    return Err(CommandError::MediaInUse(*id));
                }
                // morpher с подменой на это медиа тоже держит его
                if p.morphers
                    .iter()
                    .chain(p.sequences.iter().flat_map(|s| s.morphers.iter()))
                    .any(|m| m.media_refs().any(|x| x == *id))
                {
                    return Err(CommandError::MediaInUse(*id));
                }
            }
            Command::AddSequence { id, name } => {
                if p.source(*id).is_some() {
                    return Err(CommandError::IdInUse(id.0));
                }
                if name.trim().is_empty() {
                    return Err(CommandError::InvalidClip(ProjectError::SequenceUnnamed(*id)));
                }
            }
            Command::RemoveSequence { id } => {
                if p.sequence(*id).is_none() {
                    return Err(CommandError::SequenceNotFound(*id));
                }
                let used = p.sequence_uses(*id);
                if used > 0 {
                    return Err(CommandError::SequenceInUse(*id, used));
                }
            }
            Command::RenameSequence { id, name } => {
                if p.sequence(*id).is_none() {
                    return Err(CommandError::SequenceNotFound(*id));
                }
                if name.trim().is_empty() {
                    return Err(CommandError::InvalidClip(ProjectError::SequenceUnnamed(*id)));
                }
            }
            Command::SetSequenceSettings { id, raster, fps } => {
                let Some(s) = p.sequence(*id) else {
                    return Err(CommandError::SequenceNotFound(*id));
                };
                let mut probe = s.clone();
                probe.raster = *raster;
                probe.fps = *fps;
                if !probe.settings_ok() {
                    return Err(CommandError::InvalidClip(ProjectError::BadSequenceSettings(*id)));
                }
            }
            Command::SetSequenceContent { id, content } => {
                if p.sequence(*id).is_none() {
                    return Err(CommandError::SequenceNotFound(*id));
                }
                if content.id != *id {
                    return Err(CommandError::SequenceNotFound(content.id));
                }
                // дорожек в ленте не меньше, чем занято её событиями:
                // «SetLanes не осиротит клипы» обязано выполняться и здесь
                let need = |k: ClipKind| {
                    content.clips.iter().filter(|c| c.kind == k).map(|c| c.lane + 1).max()
                };
                if content.lanes(LaneKind::Video) < need(ClipKind::Video).unwrap_or(1).max(1)
                    || content.lanes(LaneKind::Audio) < need(ClipKind::Audio).unwrap_or(1).max(1)
                {
                    return Err(CommandError::LanesTooFew);
                }
            }
            Command::SetClipMedia { id, media } => {
                if !p.clips.iter().any(|c| c.id == *id) {
                    return Err(CommandError::ClipNotFound(*id));
                }
                if p.source(*media).is_none() {
                    return Err(CommandError::MediaNotFound(*media));
                }
            }
            // Публикация проверяется ТЕМ ЖЕ кодом, что и валидация файла:
            // второго правила «что считать живой целью» быть не может.
            Command::PublishParam { seq, param } => {
                let set = Self::published_slot(p, scope, *seq)
                    .ok_or(CommandError::SequenceNotFound(*seq))?;
                if set.iter().any(|x| x.id == param.id) {
                    return Err(CommandError::IdInUse(param.id.0));
                }
                Self::check_published_on(p, scope, *seq, param)?;
            }
            Command::SetPublished { seq, param } => {
                let set = Self::published_slot(p, scope, *seq)
                    .ok_or(CommandError::SequenceNotFound(*seq))?;
                if !set.iter().any(|x| x.id == param.id) {
                    return Err(CommandError::PublishedNotFound(param.id));
                }
                Self::check_published_on(p, scope, *seq, param)?;
            }
            Command::UnpublishParam { seq, id } => {
                let set = Self::published_slot(p, scope, *seq)
                    .ok_or(CommandError::SequenceNotFound(*seq))?;
                if !set.iter().any(|x| x.id == *id) {
                    return Err(CommandError::PublishedNotFound(*id));
                }
            }
            Command::ReorderPublished { seq, order } => {
                // ПЕРЕСТАНОВКА, а не правка набора: порядок обязан быть
                // перестановкой ровно того, что объявлено, иначе команда
                // молча теряла бы или дублировала ручки.
                let set = Self::published_slot(p, scope, *seq)
                    .ok_or(CommandError::SequenceNotFound(*seq))?;
                let mut a: Vec<u64> = order.iter().map(|x| x.0).collect();
                let mut b: Vec<u64> = set.iter().map(|x| x.id.0).collect();
                a.sort_unstable();
                b.sort_unstable();
                if a != b {
                    return Err(CommandError::PublishedOrderBroken);
                }
            }
            Command::SetOverride { id, ov } => {
                let c = find(*id)?;
                let mut probe = c.clone();
                probe.overrides.retain(|x| x.pub_id != ov.pub_id);
                probe.overrides.push((**ov).clone());
                p.validate_overrides_for(&probe).map_err(CommandError::InvalidClip)?;
            }
            Command::ClearOverride { id, .. } => {
                find(*id)?;
            }
            // Значения генератора проверяются ТЕМ ЖЕ кодом, что и
            // валидация файла (`check_clip_gen`): второго правила «что
            // считать согласованным» быть не может, разойдись они — окно
            // ставило бы состояние, которое проект потом отказался бы
            // сохранить.
            Command::SetClipGen { id, kind } => {
                let c = find(*id)?;
                let mut probe = c.clone();
                probe.r#gen = kind.clone();
                if kind.is_none() {
                    probe.gen_auto.clear();
                }
                kt_check(p, &probe)?;
            }
            Command::SetClipGenAuto { id, auto } => {
                let c = find(*id)?;
                let mut probe = c.clone();
                probe.gen_auto = auto.clone();
                kt_check(p, &probe)?;
            }
            Command::ApplyGenToAll { media, kind, auto } => {
                if p.gen_media_path(*media).is_none() {
                    return Err(CommandError::MediaNotFound(*media));
                }
                let mut probe = ClipEvent {
                    r#gen: Some(kind.clone()),
                    gen_auto: auto.clone(),
                    media: *media,
                    ..p.clips
                        .iter()
                        .find(|c| c.media == *media)
                        .cloned()
                        .ok_or(CommandError::MediaNotFound(*media))?
                };
                probe.media = *media;
                kt_check(p, &probe)?;
            }
            Command::SetSettings { settings } => {
                if settings.fps == 0 || kt_time::TICK_HZ % settings.fps as i64 != 0 {
                    return Err(CommandError::InvalidClip(ProjectError::BadFps));
                }
            }
            Command::AddMorpher(m) => {
                if p.morphers.iter().any(|x| x.id == m.id) {
                    return Err(CommandError::IdInUse(m.id.0));
                }
                Self::check_morpher_on(p, m)?;
            }
            Command::SetMorpher(m) => {
                if !p.morphers.iter().any(|x| x.id == m.id) {
                    return Err(CommandError::MorpherNotFound(m.id));
                }
                Self::check_morpher_on(p, m)?;
            }
            Command::RemoveMorpher { id } | Command::BakeMorpher { id } => {
                if !p.morphers.iter().any(|x| x.id == *id) {
                    return Err(CommandError::MorpherNotFound(*id));
                }
            }
            Command::SetLaneOrder { .. } | Command::SetTrimStretchesAuto { .. } => {}
            Command::AddArea(a) => {
                if a.name.trim().is_empty() {
                    return Err(CommandError::AreaUnnamed);
                }
                if p.taken_names().contains(&a.name) {
                    return Err(CommandError::NameTaken(a.name.clone()));
                }
                if a.duration.0 <= 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::RemoveArea { name } | Command::SetAreaLanes { name, .. } => {
                if p.area(name).is_none() {
                    return Err(CommandError::AreaNotFound(name.clone()));
                }
            }
            Command::SetAreaRange { name, duration, .. } => {
                if p.area(name).is_none() {
                    return Err(CommandError::AreaNotFound(name.clone()));
                }
                if duration.0 <= 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::RenameArea { from, to } => {
                if p.area(from).is_none() {
                    return Err(CommandError::AreaNotFound(from.clone()));
                }
                if to.trim().is_empty() {
                    return Err(CommandError::AreaUnnamed);
                }
                if to != from && p.taken_names().contains(to) {
                    return Err(CommandError::NameTaken(to.clone()));
                }
            }
            Command::AddCamera(c) => {
                if p.cameras.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::IdInUse(c.id.0));
                }
                Self::check_camera_on(c)?;
            }
            Command::SetCamera(c) => {
                if !p.cameras.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::CameraNotFound(c.id));
                }
                Self::check_camera_on(c)?;
            }
            Command::RemoveCamera { id } => {
                if !p.cameras.iter().any(|x| x.id == *id) {
                    return Err(CommandError::CameraNotFound(*id));
                }
            }
        }
        Ok(())
    }

    fn check_morpher_on(p: &Project, m: &MorpherEvent) -> Result<(), CommandError> {
        // пустой список запрещён ТОЛЬКО в режиме замороженных целей: у
        // слоя («всё, что ниже») и у ссылок пустое множество законно —
        // просто сейчас под морфером ничего нет
        if let MorphTargets::Clips { ids } = &m.targets {
            if ids.is_empty() {
                return Err(CommandError::MorpherNoTargets);
            }
            for t in ids {
                if !p.clips.iter().any(|c| c.id == *t) {
                    return Err(CommandError::ClipNotFound(*t));
                }
            }
        }
        for mid in m.media_refs() {
            if !p.media.iter().any(|x| x.id == mid) {
                return Err(CommandError::MediaNotFound(mid));
            }
        }
        Ok(())
    }

    /// Проверка ОБЪЯВЛЕНИЯ публичного параметра тем же кодом, что и
    /// валидация файла (Р.59, стадия 3).
    ///
    /// Лента НАЗЫВАЕТСЯ (Р.63 п.11), и цель ищется в ней. Содержимое
    /// той ленты, в которой стоишь, лежит в корне рабочего вида — как и
    /// содержимое ленты, РАСКРЫТОЙ ПОЛОСОЙ, хотя набор её публикаций
    /// остался в `sequences`. Поэтому поиск цели падает обратно в корень:
    /// ошибиться этим нельзя — итоговый документ всё равно проходит
    /// `validate`, и чужая цель там отвергается.
    fn check_published_on(
        p: &Project,
        scope: Option<MediaId>,
        seq: MediaId,
        param: &PublishedParam,
    ) -> Result<(), CommandError> {
        if param.name.trim().is_empty() {
            return Err(CommandError::InvalidClip(ProjectError::PublishedUnnamed(None, param.id)));
        }
        let in_reel = (scope != Some(seq)).then_some(seq);
        let found = p.addr_type(in_reel, &param.target).or_else(|| p.addr_type(None, &param.target));
        let Some(ty) = found else {
            return Err(CommandError::InvalidClip(ProjectError::PublishedTargetMissing(
                None, param.id,
            )));
        };
        if !ty.same_family(&param.ty) {
            return Err(CommandError::InvalidClip(ProjectError::PublishedTypeMismatch(
                None, param.id,
            )));
        }
        Ok(())
    }

    /// Проверка камеры-события теми же правилами, что validate() (Р.29):
    /// команда не имеет права провести в проект то, что он потом отвергнет.
    fn check_camera_on(c: &CameraEvent) -> Result<(), CommandError> {
        if c.duration.0 <= 0 {
            return Err(CommandError::InvalidCamera(ProjectError::CameraBadDuration(c.id)));
        }
        let fovs = std::iter::once(c.cam.fov_deg).chain(c.keyframes.iter().map(|k| k.cam.fov_deg));
        for fov in fovs {
            if !(fov > 0.0 && fov < 180.0) {
                return Err(CommandError::InvalidCamera(ProjectError::CameraBadFov(c.id)));
            }
        }
        let sorted = c.keyframes.windows(2).all(|w| w[0].t <= w[1].t);
        if !sorted || c.keyframes.iter().any(|k| !(0.0..=1.0).contains(&k.t)) {
            return Err(CommandError::InvalidCamera(ProjectError::CameraBadKeyframes(c.id)));
        }
        Ok(())
    }

    fn mutate_on(p: &mut Project, scope: Option<MediaId>, cmd: Command) {
        match cmd {
            Command::NoteBirth { .. } => {}
            Command::AddMedia { id, path } => {
                // Путь генератора несёт ТОЛЬКО ВИД (Р.59, стадия 2) — и
                // приводится к канону в тех же двух местах, что и у
                // `SetMediaPath`. Значения приезжают отдельно, на СОБЫТИИ:
                // иначе первая же проверка споткнулась бы о «вид значений
                // не тот, что в пути».
                let path = match crate::genmedia::parse_gen_path(&path) {
                    Some(k) => crate::genmedia::gen_kind_path(&k),
                    None => path,
                };
                p.media.push(MediaRef::new(id, path));
            }
            Command::SetMediaPath { id, path } => {
                // Путь генератора несёт ТОЛЬКО ВИД (Р.59, стадия 2), и
                // приводит его к канону это место: команда осталась
                // дверью relink'а, а путь для неё по-прежнему непрозрачен,
                // поэтому окно может прислать и полный старый путь.
                // События на этом медиа обязаны остаться согласованными —
                // иначе первая же правка упёрлась бы в `GenKindMismatch`.
                let canon = match crate::genmedia::parse_gen_path(&path) {
                    Some(k) => crate::genmedia::gen_kind_path(&k),
                    None => path,
                };
                if let Some(m) = p.media.iter_mut().find(|m| m.id == id) {
                    m.path = canon;
                }
                let mut touched: Vec<ClipEvent> = std::mem::take(&mut p.clips);
                for c in touched.iter_mut().filter(|c| c.media == id) {
                    fit_gen_to_media(p, c);
                }
                p.clips = touched;
                for i in 0..p.sequences.len() {
                    let mut cs = std::mem::take(&mut p.sequences[i].clips);
                    for c in cs.iter_mut().filter(|c| c.media == id) {
                        fit_gen_to_media(p, c);
                    }
                    p.sequences[i].clips = cs;
                }
            }
            Command::AddClip(mut c) => {
                p.ensure_tracks(lane_key_of(c.kind), c.lane + 1);
                fit_gen_to_media(p, &mut c);
                p.clips.push(c)
            }
            Command::RemoveClip { id } => {
                p.clips.retain(|c| c.id != id);
                // D5 (v1): вычищаем id из замороженных целей morpher'ов
                for m in &mut p.morphers {
                    if let Some(ids) = m.targets.clip_ids_mut() {
                        ids.retain(|t| *t != id);
                    }
                }
            }
            Command::MoveClip { id, new_start, new_lane } => {
                let (group, delta, kind) = {
                    let c = p.clips.iter().find(|c| c.id == id).unwrap();
                    (c.link_group, new_start - c.start, c.kind)
                };
                p.ensure_tracks(lane_key_of(kind), new_lane + 1);
                for c in &mut p.clips {
                    if c.id == id {
                        c.start = new_start;
                        c.lane = new_lane;
                    } else if group.is_some() && c.link_group == group {
                        // партнёры по A/V-линку двигаются тем же смещением
                        c.start = c.start + delta;
                    }
                }
            }
            // Тримы применяются ко ВСЕЙ link-группе (v1; п.6 «магнит
            // распадается»: трим края видео оставлял аудио-партнёра прежней
            // длины). Партнёры клампятся к своим ограничениям.
            // Реверс (п.16): у reversed клипа source_in живёт «с другого
            // конца» — TrimStart его НЕ трогает, TrimEnd сдвигает.
            Command::TrimStart { id, new_start } => {
                // опция «растягивать анимацию вместе с событием» (Р.40):
                // ВЫКЛЮЧЕНА по умолчанию — ключ остаётся на своём кадре
                let stretch = p.trim_stretches_auto;
                let group = p.clips.iter().find(|c| c.id == id).and_then(|c| c.link_group);
                for c in &mut p.clips {
                    if c.id != id && (group.is_none() || c.link_group != group) {
                        continue;
                    }
                    let max_start = c.start + c.duration - MIN_CLIP_DURATION;
                    let ns = Tick(new_start.0.clamp(trim_min_start(c).0, max_start.0));
                    // геометрия и материал — ОДНИМ правилом (Р.39):
                    // у растянутого клипа `source_in` едет по интегралу
                    // скорости, а не на длину куска
                    let (span_old, anchor_old) = (crate::clip_material_span(c), c.source_in);
                    *c = clip_slice(c, TickRange::new(ns, c.start + c.duration), c.id);
                    if stretch {
                        let span_new = crate::clip_material_span(c);
                        crate::stretch_auto_span(c, anchor_old, span_old, span_new);
                    }
                }
            }
            Command::TrimEnd { id, new_end } => {
                let stretch = p.trim_stretches_auto;
                let group = p.clips.iter().find(|c| c.id == id).and_then(|c| c.link_group);
                // длины медиа берём ДО цикла: внутри `p.clips` занят
                // мутабельно, а `media_duration` смотрит в тот же проект
                let lens: Vec<(ClipId, Option<Tick>)> = p
                    .clips
                    .iter()
                    .filter(|c| c.id == id || (group.is_some() && c.link_group == group))
                    .map(|c| (c.id, p.media_duration(c.media)))
                    .collect();
                for c in &mut p.clips {
                    if c.id != id && (group.is_none() || c.link_group != group) {
                        continue;
                    }
                    let media_len =
                        lens.iter().find(|(cid, _)| *cid == c.id).and_then(|(_, l)| *l);
                    let min_end = c.start + MIN_CLIP_DURATION;
                    // Край материала — такой же упор, как его начало (Р.63).
                    // Предел НЕ МЕНЬШЕ нынешнего конца: событие, которое
                    // уже длиннее материала (файлы до Р.63), двигается в
                    // пределах сделанного и подрезается свободно — само
                    // оно не укорачивается (правило 20), только расти
                    // дальше ему больше не дают.
                    let was_end = (c.start + c.duration).0;
                    let limit = trim_max_end_in(c, media_len).0.max(was_end);
                    let ne = Tick(new_end.0.max(min_end.0).min(limit));
                    let (span_old, anchor_old) = (crate::clip_material_span(c), c.source_in);
                    *c = clip_slice(c, TickRange::new(c.start, ne), c.id);
                    if stretch {
                        let span_new = crate::clip_material_span(c);
                        crate::stretch_auto_span(c, anchor_old, span_old, span_new);
                    }
                }
            }
            Command::Split { id, at, new_id, new_group } => {
                // правило кусков ОДНО на весь проект — `clip_slice`
                // (реверс, п.16: левая половина играет ХВОСТ источника)
                let c = Self::clip_mut_on(p, id).clone();
                let body = TickRange::new(c.start, c.start + c.duration);
                *Self::clip_mut_on(p, id) = clip_slice(&c, TickRange::new(body.start, at), id);
                let mut right = clip_slice(&c, TickRange::new(at, body.end), new_id);
                right.link_group = new_group;
                p.clips.push(right);
                // D5 (v1): правая половина наследует таргетинг morpher'ов
                for m in &mut p.morphers {
                    if let Some(ids) = m.targets.clip_ids_mut()
                        && ids.contains(&id)
                    {
                        ids.push(new_id);
                    }
                }
                // переход, где клип был OUTGOING (стык на прежнем КОНЦЕ),
                // уезжает к правой половине; incoming-стык (начало) остаётся
                // за левой — она сохранила id
                for t in &mut p.transitions {
                    if t.a == Some(id) {
                        t.a = Some(new_id);
                    }
                }
            }
            Command::SetTrimStretchesAuto { on } => p.trim_stretches_auto = on,
            Command::SetGain { id, gain_db } => Self::clip_mut_on(p, id).gain_db = gain_db,
            Command::SetOpacity { id, opacity } => {
                Self::clip_mut_on(p, id).opacity = opacity.clamp(0.0, 1.0)
            }
            Command::SetGeometry { id, geometry, pos, auto } => {
                let c = Self::clip_mut_on(p, id);
                c.geometry = geometry;
                c.geometry_pos = pos.min(c.fx.len());
                c.geometry_auto = auto;
            }
            Command::SetFxOff { id, off } => Self::clip_mut_on(p, id).fx_off = off,
            Command::SetReversed { id, reversed } => Self::clip_mut_on(p, id).reversed = reversed,
            Command::SetFxBypass { id, bypass } => {
                Self::clip_mut_on(p, id).fx_bypass = bypass;
                sync_warp_to_link_group(p, id);
            }
            Command::SetFx { id, fx } => {
                let c = Self::clip_mut_on(p, id);
                // цепочка сменилась — кривые лишних слотов отпадают (Р.30);
                // перестановку слотов редактор ведёт сам и шлёт SetFxAuto
                // тем же жестом
                c.fx_auto.truncate(fx.len());
                // ведущий узел «Геометрия» (Р.62) стоял за концом
                // укоротившейся цепочки — прижимается к её концу
                c.geometry_pos = c.geometry_pos.min(fx.len());
                c.fx = fx;
                sync_warp_to_link_group(p, id);
            }
            Command::SetFxAuto { id, auto } => {
                Self::clip_mut_on(p, id).fx_auto = auto;
                sync_warp_to_link_group(p, id);
            }
            Command::SetClipGen { id, kind } => {
                let c = Self::clip_mut_on(p, id);
                if kind.is_none() {
                    c.gen_auto.clear();
                }
                c.r#gen = kind;
                sync_gen_to_link_group(p, id);
            }
            Command::SetClipGenAuto { id, auto } => {
                Self::clip_mut_on(p, id).gen_auto = auto;
                sync_gen_to_link_group(p, id);
            }
            Command::ApplyGenToAll { media, kind, auto } => {
                let apply = |clips: &mut Vec<ClipEvent>| {
                    for c in clips {
                        if c.media == media {
                            c.r#gen = Some(kind.clone());
                            c.gen_auto = auto.clone();
                        }
                    }
                };
                apply(&mut p.clips);
                for s in &mut p.sequences {
                    apply(&mut s.clips);
                }
            }
            Command::LinkClips { ids, group } => {
                for c in &mut p.clips {
                    if ids.contains(&c.id) {
                        c.link_group = Some(group);
                    }
                }
            }
            Command::UnlinkClip { id } => Self::clip_mut_on(p, id).link_group = None,
            Command::AddTransition(t) => p.transitions.push(t),
            Command::RemoveTransition { id } => p.transitions.retain(|t| t.id != id),
            Command::SetTransition(t) => {
                if let Some(slot) = p.transitions.iter_mut().find(|x| x.id == t.id) {
                    *slot = t;
                }
            }
            Command::AddMarker { at, duration } => {
                let i = p.markers.partition_point(|m| m.t < at);
                p.markers.insert(i, Marker { duration, ..Marker::at(at) });
            }
            Command::RemoveMarker { at } => p.markers.retain(|m| m.t != at),
            Command::SetMarkerName { at, name } => {
                if let Some(m) = p.markers.iter_mut().find(|m| m.t == at) {
                    m.name = name;
                }
            }
            Command::MoveMarker { from, to } => {
                // имя, длительность и цвет переезжают ВМЕСТЕ с меткой
                let moved =
                    p.markers.iter().find(|m| m.t == from).cloned().unwrap_or(Marker::at(from));
                p.markers.retain(|m| m.t != from);
                let i = p.markers.partition_point(|m| m.t < to);
                p.markers.insert(i, Marker { t: to, ..moved });
            }
            Command::SetMarkerDuration { at, duration } => {
                if let Some(m) = p.markers.iter_mut().find(|m| m.t == at) {
                    m.duration = duration;
                }
            }
            Command::InsertTrack { kind, at } => {
                p.insert_track(kind, at);
            }
            Command::RemoveTrack { kind, at } => p.remove_track(kind, at),
            Command::SetTrackProps { id, props } => {
                if let Some(t) = p.track_mut(id) {
                    t.set_props(&props);
                }
            }
            Command::SetTrackFx { id, fx, bypass } => {
                if let Some(t) = p.track_mut(id) {
                    t.fx = fx;
                    t.fx_bypass = bypass;
                }
            }
            Command::SetTrackFxOff { id, off } => {
                if let Some(t) = p.track_mut(id) {
                    t.fx_off = off;
                }
            }
            Command::SetLaneOrder { order } => {
                p.set_lane_order(&order);
            }
            Command::RemoveMedia { id } => p.media.retain(|m| m.id != id),
            Command::AddSequence { id, name } => {
                p.sequences.push(Sequence::new(id, name));
                p.assign_track_ids();
            }
            Command::RemoveSequence { id } => p.sequences.retain(|s| s.id != id),
            Command::SetSequenceSettings { id, raster, fps } => {
                if let Some(s) = p.sequence_mut(id) {
                    s.raster = raster;
                    s.fps = fps;
                }
            }
            Command::RenameSequence { id, name } => {
                if let Some(s) = p.sequence_mut(id) {
                    s.name = name;
                }
            }
            Command::SetSequenceContent { id, content } => {
                if let Some(slot) = p.sequences.iter_mut().find(|s| s.id == id) {
                    *slot = *content;
                }
            }
            Command::SetClipMedia { id, media } => {
                let mut c = Self::clip_mut_on(p, id).clone();
                c.media = media;
                fit_gen_to_media(p, &mut c);
                // Экземпляр переехал на ДРУГУЮ ленту — чужие
                // переопределения на ней не значат ничего.
                c.overrides.clear();
                *Self::clip_mut_on(p, id) = c;
            }
            Command::PublishParam { seq, param } => {
                if let Some(set) = Self::published_slot_mut(p, scope, seq) {
                    set.push(*param);
                }
            }
            Command::SetPublished { seq, param } => {
                if let Some(slot) = Self::published_slot_mut(p, scope, seq)
                    .and_then(|set| set.iter_mut().find(|x| x.id == param.id))
                {
                    *slot = *param;
                }
            }
            Command::UnpublishParam { seq, id } => {
                // Переопределения снятой ручки убирает `reconcile_published`
                // ТЕМ ЖЕ шагом истории — по всему документу сразу, а не
                // только там, куда дотянулась эта команда.
                if let Some(set) = Self::published_slot_mut(p, scope, seq) {
                    set.retain(|x| x.id != id);
                }
            }
            Command::ReorderPublished { seq, order } => {
                if let Some(set) = Self::published_slot_mut(p, scope, seq) {
                    let mut out = Vec::with_capacity(set.len());
                    for id in &order {
                        if let Some(x) = set.iter().find(|x| x.id == *id) {
                            out.push(x.clone());
                        }
                    }
                    *set = out;
                }
            }
            Command::SetOverride { id, ov } => {
                let c = Self::clip_mut_on(p, id);
                c.overrides.retain(|x| x.pub_id != ov.pub_id);
                c.overrides.push(*ov);
            }
            Command::ClearOverride { id, pub_id } => {
                Self::clip_mut_on(p, id).overrides.retain(|x| x.pub_id != pub_id);
            }
            Command::SetSettings { settings } => p.settings = settings,
            Command::AddMorpher(m) => {
                p.ensure_tracks(LaneKind::Morph, m.lane + 1);
                p.morphers.push(m);
            }
            Command::SetMorpher(m) => {
                p.ensure_tracks(LaneKind::Morph, m.lane + 1);
                if let Some(slot) = p.morphers.iter_mut().find(|x| x.id == m.id) {
                    *slot = m;
                }
            }
            Command::RemoveMorpher { id } => p.morphers.retain(|m| m.id != id),
            Command::BakeMorpher { id } => {
                let Some(pos) = p.morphers.iter().position(|m| m.id == id) else { return };
                // «bake == то, что рисовалось» — значит РЕЗКА тоже
                // материализуется. Считаем ровно этот морфер: остальные
                // остаются жить и режут по-прежнему.
                let baked = {
                    let mut q = p.clone();
                    // Вложения при bake НЕ раскрываются (Р.50): морфер
                    // материализует свою резку по клипам ЭТОЙ ленты, а
                    // экземпляр вложения — такой же клип. Раскрытие
                    // подменило бы его содержимым и потеряло бы само
                    // событие.
                    q.sequences.clear();
                    q.morphers.retain(|m| m.id == id);
                    q.effective_clips()
                };
                p.morphers.remove(pos);
                // синтетические id кусков → настоящие; кусок №0 держит id
                // оригинала (правило `Split`), остальные получают max+1…
                let mut next = p.clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 1;
                // (оригинал, новый id, конец куска) — для перевешивания
                let mut born: Vec<(ClipId, ClipId, Tick)> = Vec::new();
                let mut clips = Vec::with_capacity(baked.len());
                for mut c in baked {
                    if let Some(origin) = slice_origin(c.id) {
                        c.id = ClipId(next);
                        next += 1;
                        born.push((origin, c.id, c.start + c.duration));
                    }
                    clips.push(c);
                }
                let ends: Vec<(ClipId, Tick)> =
                    p.clips.iter().map(|c| (c.id, c.start + c.duration)).collect();
                p.clips = clips;
                if born.is_empty() {
                    return;
                }
                // переход, где разрезанный клип был OUTGOING, уезжает к
                // куску, которому достался стык (прежний КОНЕЦ тела)
                for t in &mut p.transitions {
                    let Some(a) = t.a else { continue };
                    let Some(&(_, end)) = ends.iter().find(|(i, _)| *i == a) else { continue };
                    if let Some(&(_, new, _)) = born.iter().find(|(o, _, e)| *o == a && *e == end) {
                        t.a = Some(new);
                    }
                }
                // куски наследуют таргетинг ОСТАЛЬНЫХ морферов (D5 v1)
                for m in &mut p.morphers {
                    if let Some(ids) = m.targets.clip_ids_mut() {
                        for (origin, new, _) in &born {
                            if ids.contains(origin) {
                                ids.push(*new);
                            }
                        }
                    }
                }
            }
            Command::AddArea(a) => p.areas.push(a),
            Command::RemoveArea { name } => {
                p.areas.retain(|a| a.name != name);
                // ссылка на удалённую область остаётся в файле и
                // резолвится в ничто: молча выкидывать её нельзя —
                // пользователь вернёт область тем же именем (правило 20)
            }
            Command::RenameArea { from, to } => {
                if let Some(a) = p.areas.iter_mut().find(|a| a.name == from) {
                    a.name = to.clone();
                }
                for m in &mut p.morphers {
                    if let MorphTargets::Areas { names } = &mut m.targets {
                        for n in names.iter_mut().filter(|n| **n == from) {
                            *n = to.clone();
                        }
                    }
                }
            }
            Command::SetAreaRange { name, start, duration } => {
                if let Some(a) = p.areas.iter_mut().find(|a| a.name == name) {
                    a.start = Tick(start.0.max(0));
                    a.duration = duration;
                }
            }
            Command::SetAreaLanes { name, lanes } => {
                if let Some(a) = p.areas.iter_mut().find(|a| a.name == name) {
                    a.lanes = lanes;
                }
            }
            Command::AddCamera(c) => {
                // строка обязана СУЩЕСТВОВАТЬ, иначе камера легла бы на
                // невидимую дорожку: создать можно, управлять нельзя.
                // То же правило, что у `MoveClip` для видео и аудио.
                p.ensure_tracks(LaneKind::Camera, c.lane + 1);
                p.cameras.push(c);
            }
            Command::SetCamera(c) => {
                if let Some(x) = p.cameras.iter_mut().find(|x| x.id == c.id) {
                    *x = c;
                }
            }
            Command::RemoveCamera { id } => p.cameras.retain(|c| c.id != id),
        }
    }

    /// Судьба переходов после правок геометрии клипов (Р.24, п.135):
    /// клип пропал или стык разорван (конец `a` != начало `b`, дорожка/вид
    /// разошлись) — переход умирает; трим укоротил соседа — офсеты клампятся
    /// телами, схлопнувшееся ниже минимума окно умирает. Групповой перенос
    /// обоих соседей стык сохраняет — переход живёт (идентичность по ClipId,
    /// позиция не хранится).
    /// Переходы сверяются с содержимым КАЖДОЙ ленты (Р.50): переход живёт
    /// на той ленте, где лежат его концы, и сверять его с чужими клипами —
    /// значит убить его на первой же команде.
    fn reconcile_transitions(p: &mut Project) {
        let Project { clips, transitions, sequences, .. } = p;
        reconcile_reel(clips, transitions);
        for s in sequences {
            reconcile_reel(&s.clips, &mut s.transitions);
        }
    }

    /// Судьба публикаций и переопределений после пакета команд (Р.59,
    /// стадия 3) — образец `reconcile_transitions`.
    ///
    /// `root_is` — чья лента стоит СЕЙЧАС на месте корня (вход внутрь
    /// сделан обменом содержимого). Без этого адреса нельзя понять, чьи
    /// переопределения осиротели, когда ручку сняли изнутри ленты.
    ///
    /// Цель, сменившая ТИП, считается исчезнувшей: слот, которому
    /// подменили вид эффекта, — это другой слот, и ручка на нём уже ни на
    /// что не действует.
    fn reconcile_published(p: &mut Project, root_is: Option<MediaId>) {
        // Быстрый путь первой строкой: проект без публикаций и без
        // переопределений — подавляющее большинство — платит один обход
        // и ни одной аллокации.
        let none = p.published.is_empty()
            && p.sequences.iter().all(|s| s.published.is_empty())
            && p.clips
                .iter()
                .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
                .all(|c| c.overrides.is_empty());
        if none {
            return;
        }
        // ДО НЕПОДВИЖНОЙ ТОЧКИ. Снятие ручки внутренней ленты убивает
        // ручку внешней, которая на неё ссылалась (`Nested`), — а та
        // могла быть целью ещё одной этажом выше. Один проход оставил бы
        // мёртвую ссылку до конца батча, и `validate` отверг бы ВЕСЬ
        // жест: правка была бы законной, а не проходила бы.
        for _ in 0..=max_publish_cascade() {
            if !Self::reconcile_published_once(p, root_is) {
                break;
            }
        }
    }

    /// Один проход согласования. Возвращает `true`, если что-то сняли, —
    /// значит надо пройти ещё раз.
    fn reconcile_published_once(p: &mut Project, root_is: Option<MediaId>) -> bool {
        let root_keep: Vec<bool> = {
            let alive = |seq: Option<MediaId>, prm: &PublishedParam| {
                p.addr_type(seq, &prm.target).is_some_and(|t| t.same_family(&prm.ty))
            };
            p.published.iter().map(|x| alive(None, x)).collect()
        };
        let seq_keep: Vec<Vec<bool>> = p
            .sequences
            .iter()
            .map(|s| {
                s.published
                    .iter()
                    .map(|x| {
                        p.addr_type(Some(s.id), &x.target).is_some_and(|t| t.same_family(&x.ty))
                    })
                    .collect()
            })
            .collect();
        let removed = root_keep.iter().any(|k| !k) || seq_keep.iter().flatten().any(|k| !k);
        let mut it = root_keep.into_iter();
        p.published.retain(|_| it.next().unwrap_or(true));
        for (s, keeps) in p.sequences.iter_mut().zip(seq_keep) {
            let mut it = keeps.into_iter();
            s.published.retain(|_| it.next().unwrap_or(true));
        }
        // Живые ручки по адресу ленты. Порядок важен: лента, стоящая на
        // месте корня, держит свой набор в `p.published`, а её запись в
        // `sequences` — чужое содержимое.
        let mut sets: std::collections::HashMap<MediaId, std::collections::HashSet<PubId>> =
            std::collections::HashMap::new();
        if let Some(rid) = root_is {
            sets.insert(rid, p.published.iter().map(|x| x.id).collect());
        }
        for s in &p.sequences {
            sets.entry(s.id).or_insert_with(|| s.published.iter().map(|x| x.id).collect());
        }
        let prune = |c: &mut ClipEvent| {
            if c.overrides.is_empty() {
                return;
            }
            match sets.get(&c.media) {
                Some(ids) => c.overrides.retain(|o| ids.contains(&o.pub_id)),
                // событие уехало на файл — переопределять больше нечего
                None => c.overrides.clear(),
            }
        };
        for c in &mut p.clips {
            prune(c);
        }
        for s in &mut p.sequences {
            for c in &mut s.clips {
                prune(c);
            }
        }
        removed
    }

    fn clip_mut_on(p: &mut Project, id: ClipId) -> &mut ClipEvent {
        p.clips.iter_mut().find(|c| c.id == id).unwrap()
    }
}

/// Сколько раз максимум может каскадировать снятие публикации: цепочка
/// ссылок `Nested` не длиннее дерева лент.
fn max_publish_cascade() -> usize {
    crate::MAX_NEST_DEPTH
}

fn reconcile_reel(clips: &[ClipEvent], transitions: &mut Vec<TransitionEvent>) {
    transitions.retain_mut(|t| {
        let side = |id: Option<ClipId>| match id {
            // сторона-пустота у фейда (Р.25) — законна и остаётся пустой
            None => Some(None),
            // клип стороны пропал: переход умирает целиком (фейды
            // заводятся явно, а не остатками от удаления)
            Some(id) => clips.iter().find(|c| c.id == id).map(Some),
        };
        let (Some(mut a), Some(mut b)) = (side(t.a), side(t.b)) else { return false };
        if let (Some(ca), Some(cb)) = (a, b) {
            // канонизация ролей (Р.51, п.399): Split и мувы могли поменять
            // порядок пары — «a» всегда ранний. Направленный вид при этом
            // зеркалится вместе с ролями; смерть перехода была бы хуже
            // (правило 20)
            if cb.start < ca.start {
                std::mem::swap(&mut t.a, &mut t.b);
                std::mem::swap(&mut a, &mut b);
            }
            let (ca, cb) = (a.unwrap(), b.unwrap());
            let (a_end, b_end) = (ca.start + ca.duration, cb.start + cb.duration);
            if ca.kind != cb.kind || ca.lane != cb.lane {
                return false;
            }
            if a_end != cb.start {
                // не встык: жив только как НАЛОЖЕННЫЙ переход (Р.51) —
                // чистая «лесенка» с окном не короче минимума и без
                // третьего клипа в нём (третий вполз мувом → переход
                // умирает, батч не валится — прецедент разорванного стыка)
                let stag = ca.start < cb.start && cb.start < a_end && a_end < b_end;
                if !stag || a_end - cb.start < MIN_TRANSITION_DURATION {
                    return false;
                }
                let busy = clips.iter().any(|c| {
                    c.id != ca.id
                        && c.id != cb.id
                        && c.kind == ca.kind
                        && c.lane == ca.lane
                        && c.start < a_end
                        && cb.start < c.start + c.duration
                });
                // офсеты у наложенного не клампятся — они мёртвый груз и
                // оживают при возврате пары встык
                return !busy;
            }
        }
        if let Some(a) = a {
            t.in_offset = t.in_offset.min(a.duration);
        }
        if let Some(b) = b {
            t.out_offset = t.out_offset.min(b.duration);
        }
        t.in_offset + t.out_offset >= MIN_TRANSITION_DURATION
    });
}
