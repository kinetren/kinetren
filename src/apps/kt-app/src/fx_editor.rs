//! Окно FX-цепочки клипа (порт ClipFxChainWindow + FxChainEditorWidget):
//! полоса карточек «Media → fx → fx», форма параметров, полоса автоматизации
//! (ромбы: клик = выбор, драг = перенос t, double-click = удалить),
//! live-применение через apply_coalesced (вся сессия окна = 1 undo),
//! Отмена = откат к состоянию на момент открытия.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::*;

/// Снимок одной цепочки на момент открытия окна: сама цепочка, кривые,
/// выключатели слотов и мастер-выключатель.
pub type ChainSnapshot = (ClipId, Vec<FxInstance>, Vec<Vec<kt_model::ParamCurve>>, Vec<bool>, bool);

pub struct FxEditor {
    /// Якорь окна: событие, по которому его открыли (его id — id окна).
    pub clip: ClipId,
    /// Все цепочки, которые показывает окно: якорь и его link-партнёры
    /// (Р.33 п.2 — «для одного и того же события с примагниченными треками
    /// можно и сразу все chain в одном окне давать»).
    pub members: Vec<ClipId>,
    pub picker_query: String,
    /// Строка пикера МЕДИА в голове «Параметры композиции» (Р.63 п.12):
    /// у ручки типа «медиа» свой поиск, и мешать его с поиском эффектов
    /// нельзя — это разные списки.
    pub inst_query: String,
    /// Состояние КАЖДОЙ цепочки на момент открытия — «Отмена» откатывает
    /// всё окно, а не только якорь.
    pub backup: Vec<ChainSnapshot>,
    /// Значения генератора на момент открытия (Р.59, стадия 2): у
    /// события-генератора голова полосы — его ИСТОЧНИК, и «Отмена»
    /// обязана возвращать и его, иначе окно откатывало бы половину своей
    /// же сессии.
    pub gen_backup: Vec<(ClipId, Option<GeneratorKind>, Vec<kt_model::ParamCurve>)>,
    /// Ведущий узел «Геометрия» на момент открытия (Р.62): «Отмена»
    /// возвращает и его — он часть той же полосы.
    pub geo_backup: Vec<(ClipId, kt_model::EventGeometry, usize, Vec<kt_model::ParamCurve>)>,
    /// Переопределения экземпляра ленты на момент открытия (Р.63 п.12):
    /// «Параметры композиции» переехали ГОЛОВОЙ в это окно, значит и
    /// «Отмена» обязана возвращать их — иначе окно откатывает половину
    /// своей же сессии (тот же довод, что у генератора и геометрии).
    pub ov_backup: Vec<(ClipId, Vec<kt_model::Override>)>,
    pub gesture: u64,
    /// Что выбрано в каждой цепочке: слот эффекта или приколотая голова.
    pub selected: std::collections::HashMap<ClipId, crate::widgets::ChainSel>,
    /// Состояние виджета автоматизации на каждую (цепочку, слот):
    /// вид, выбранный ключ, незакоммиченный ввод времени.
    pub auto_views: std::collections::HashMap<(ClipId, usize), crate::automation::AutoView>,
    /// То же для кривых ГЕНЕРАТОРА: у головы своего индекса слота нет.
    pub gen_views: std::collections::HashMap<ClipId, crate::automation::AutoView>,
    /// Кэш предупреждений титра на кадр (п.413).
    pub gen_warn: std::collections::HashMap<ClipId, crate::gen_ui::GenWarn>,
    /// Момент, на кадре которого считаются миниатюры пикера (Р.54).
    /// Пока НЕ играем — следует за курсором (он неподвижен, картинка
    /// стабильна); при игре остаётся на последнем значении: пересчитывать
    /// десятки ячеек на каждый квант часов незачем, а мерцающая полоска
    /// хуже слегка устаревшей.
    pub preview_t: Option<kt_time::Tick>,
    /// Когда окно открыли — мисклик-гард ЗАКАЗОВ декода (п.430).
    pub opened_at: std::time::Instant,
    pub open: bool,
}

impl KtApp {
    /// Открыть окно FX. Окно, которое уже показывает эту цепочку (своим
    /// якорем или как партнёра), второй раз не открывается: у A/V-пары
    /// одно окно на обе дорожки.
    /// Открыть окно эффектов события НА ГОЛОВЕ — то есть на форме его
    /// источника (Р.59, стадия 2).
    ///
    /// Прежние окна генератора и титра исчезли: владелец просил видеть
    /// генератор крошкой ПЕРЕД эффектами, в том же окне. Все входы
    /// («Параметры генератора…» у события, у карточки бина, создание из
    /// меню) ведут сюда — второго тела у этого действия нет (правило 17).
    pub fn open_fx_editor_head(&mut self, clip: ClipId) {
        self.open_fx_editor(clip);
        if let Some(ed) = self.fx_editors.iter_mut().find(|e| e.members.contains(&clip)) {
            ed.open = true;
            ed.selected.insert(clip, crate::widgets::ChainSel::Head);
        }
    }

    /// Окно эффектов МЕДИА-генератора: его правит первое событие на этом
    /// медиа. Значения живут на СОБЫТИИ, поэтому «параметры медиа» без
    /// события больше не существуют — и молчать об этом нельзя (правило
    /// 16): вид меняется из карточки бина, а править нечего, пока
    /// генератор не поставлен на монтаж.
    pub fn open_generator_editor(&mut self, media: MediaId) {
        let Some(kt_model::Source::File(m)) = self.model.project().source(media) else {
            return;
        };
        let path = m.path.clone();
        // не генератор (или вид новее сборки — заглушка): МОЛЧАТЬ нельзя
        if kt_model::parse_gen_path(&path).is_none() {
            self.note(crate::tf!(k::genw::UNKNOWN_KIND, path));
            return;
        }
        // ищем в РАБОЧЕМ ВИДЕ: окно правит то, что человек видит на
        // таймлайне, а события чужой (не раскрытой) ленты ему отсюда
        // недоступны — к ним он идёт через саму ленту
        let first = self
            .model
            .project()
            .clips
            .iter()
            .find(|c| c.media == media && c.r#gen.is_some())
            .map(|c| c.id);
        match first {
            Some(id) => self.open_fx_editor_head(id),
            None => self.note(tr(k::genw::NO_EVENTS)),
        }
    }

    pub fn open_fx_editor(&mut self, clip: ClipId) {
        if self.fx_editors.iter().any(|e| e.members.contains(&clip)) {
            return;
        }
        let Some(c) = self.model.clip(clip).cloned() else { return };
        let members: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|m| m.id == clip || (c.link_group.is_some() && m.link_group == c.link_group))
            // якорь первым, дальше по виду и id — порядок стабилен
            .map(|m| (m.id != clip, m.kind == ClipKind::Audio, m.id.0, m.id))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|(_, _, _, id)| id)
            .collect();
        self.gesture_counter += 1;
        let backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.fx.clone(), m.fx_auto.clone(), m.fx_bypass.clone(), m.fx_off))
            .collect();
        let gen_backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.r#gen.clone(), m.gen_auto.clone()))
            .collect();
        let geo_backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.geometry, m.geometry_pos, m.geometry_auto.clone()))
            .collect();
        let ov_backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.overrides.clone()))
            .collect();
        // у события-генератора с пустой цепочкой выбирать нечего, кроме
        // головы: `Fx(0)` на пустой цепочке показал бы пустоту. У
        // ЭКЗЕМПЛЯРА ленты голова — «Параметры композиции» (Р.63 п.12), и
        // правило то же.
        let doc = self.model.document();
        let selected = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| {
                let has_head = m.r#gen.is_some() || doc.sequence(m.media).is_some();
                let sel = if m.fx.is_empty() && has_head {
                    crate::widgets::ChainSel::Head
                } else {
                    crate::widgets::ChainSel::Fx(m.fx.len().saturating_sub(1))
                };
                (m.id, sel)
            })
            .collect();
        self.fx_editors.push(FxEditor {
            clip,
            members,
            picker_query: String::new(),
            inst_query: String::new(),
            backup,
            gen_backup,
            geo_backup,
            ov_backup,
            gesture: self.gesture_counter,
            selected,
            auto_views: std::collections::HashMap::new(),
            gen_views: std::collections::HashMap::new(),
            gen_warn: std::collections::HashMap::new(),
            preview_t: None,
            opened_at: std::time::Instant::now(),
            open: true,
        });
    }

    pub(crate) fn set_fx_coalesced(&mut self, clip: ClipId, fx: Vec<FxInstance>, gesture: u64) {
        self.apply_coalesced(Command::SetFx { id: clip, fx }, gesture);
    }

    /// Кривые автоматизации: тот же gesture, что у SetFx — один undo-шаг
    /// на сессию окна.
    pub(crate) fn set_fx_auto_coalesced(
        &mut self,
        clip: ClipId,
        mut auto: Vec<Vec<kt_model::ParamCurve>>,
        gesture: u64,
    ) {
        while auto.last().is_some_and(|v| v.is_empty()) {
            auto.pop(); // хвост без кривых не сериализуется зря
        }
        self.apply_coalesced(Command::SetFxAuto { id: clip, auto }, gesture);
    }

    pub fn draw_fx_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.fx_editors);
        for ed in &mut editors {
            self.draw_one_fx_editor(ctx, ed);
        }
        // закрыли окно — считать его ячейки больше незачем (Р.54): всё,
        // что осталось в очереди миниатюр, выбрасывается сменой поколения
        if editors.iter().any(|e| !e.open) {
            self.fx_thumbs.bump_generation();
        }
        editors.retain(|e| e.open);
        self.fx_editors = editors;
    }

    #[allow(clippy::too_many_lines)]
    fn draw_one_fx_editor(&mut self, ctx: &egui::Context, ed: &mut FxEditor) {
        // подсказки из ui-замыканий (там `self` занят) — в накопитель
        // кадра, потом в hover_hint: попапов нет (п.14 р.8)
        let mut hover_out: Option<String> = None;
        // цель правого клика в полосе цепочки (п.363): полоса — виджет,
        // про KtApp она не знает, и цель отдаёт наружу
        let mut panel_pick: Option<crate::ctx_menu::PanelTarget> = None;
        // окно живёт, пока жива хоть одна его цепочка
        ed.members.retain(|id| self.model.clip(*id).is_some());
        // Публикация (Р.59, стадия 3): документ клонируется ОДИН раз на
        // отрисовку окна — `publish_ctx` зовётся на каждый носитель, и
        // клон проекта на строку был бы аллокацией в пути отрисовки.
        let pub_doc = self.model.document();
        // правки ручек композиции: применяются ПОСЛЕ отрисовки токеном
        // окна — внутри ui-замыкания `self` занят заимствованием
        let mut inst_ops: Vec<kt_model::Command> = Vec::new();
        // отказ формулы — ошибкой в строку состояния (Р.67, правило 16)
        let mut auto_error: Option<String> = None;
        let Some(anchor) = self.model.clip(ed.clip).cloned() else {
            ed.open = false;
            return;
        };
        // пресеты: показываем только те, что законны для ЭТОГО вида
        // события (совместимость считается по реестру эффектов)
        let is_audio_anchor = anchor.kind == ClipKind::Audio;
        let preset_rows: Vec<crate::user_store::PresetRow> = self
            .presets
            .rows()
            .iter()
            .filter(|r| if is_audio_anchor { r.audio_ok } else { r.video_ok })
            .cloned()
            .collect();
        // состояние библиотеки сообщается ОДИН раз за сессию окна: файл
        // битый, файл новее нас, часть записей не разобралась
        if !self.presets_reported {
            self.presets_reported = true;
            if self.presets.is_broken() {
                self.note(tr(k::msg::PRESETS_BROKEN));
            }
            if self.presets.is_readonly() {
                self.note(tr(k::msg::PRESETS_READONLY));
            }
            let skipped = self.presets.skipped();
            if skipped > 0 {
                self.note(format!("{} {skipped}", tr(k::msg::PRESETS_SKIPPED)));
            }
        }
        let mut preset_save: Option<kt_model::ops::FxChainBlock> = None;
        let mut preset_action: Option<crate::widgets::PresetAction> = None;
        // камеры для комбо Pan3d (Р.29): id + подпись
        let cameras: Vec<(u64, String)> = self
            .model
            .project()
            .cameras
            .iter()
            .map(|c| {
                (c.id.0, if c.name.is_empty() { format!("C{}", c.id.0) } else { c.name.clone() })
            })
            .collect();
        let title = if ed.members.len() > 1 {
            crate::tf!(k::fx::TITLE_GROUP, ed.clip.0)
        } else {
            crate::tf!(
                k::fx::TITLE,
                if anchor.kind == ClipKind::Audio {
                    tr(k::fx::KIND_AUDIO)
                } else {
                    tr(k::fx::KIND_VIDEO)
                },
                ed.clip.0
            )
        };
        let playhead = self.player.transport.playhead();
        // буферы живут в приложении; в окно отдаём их на время кадра,
        // чтобы не держать &mut self внутри ui-замыкания
        // нажатие «вставить огибающую»: сюда кладётся адрес цели
        let mut paste_req = false;
        let mut paste_target: Option<(ClipId, usize)> = None;
        let mut fx_clip = self.fx_clipboard.clone();
        // что окно наредактировало за кадр: применяем ПОСЛЕ отрисовки
        struct Edited {
            clip: ClipId,
            fx: Option<Vec<FxInstance>>,
            auto: Option<Vec<Vec<kt_model::ParamCurve>>>,
            bypass: Option<Vec<bool>>,
            off: Option<bool>,
            /// Значения генератора события (Р.59, стадия 2) — `SetClipGen`.
            r#gen: Option<GeneratorKind>,
            /// Его кривые — `SetClipGenAuto`.
            gen_auto: Option<Vec<kt_model::ParamCurve>>,
            /// Пресет титра ставит ВСЁ разом — и это СВОЙ шаг истории.
            own_step: bool,
            /// «Применить ко всем»: `ApplyGenToAll`, один шаг истории.
            apply_all: bool,
            /// Смена ВИДА генератора: свойство медиа, свой шаг истории.
            switch_kind: Option<GeneratorKind>,
            /// Ведущий узел «Геометрия», его позиция и кривые (Р.62) —
            /// `SetGeometry`, тот же шаг истории, что и цепочка.
            geometry: Option<(kt_model::EventGeometry, usize, Vec<kt_model::ParamCurve>)>,
        }
        let mut edits: Vec<Edited> = Vec::new();
        let mut revert = false;
        let mut lead_refused = false;
        // мини-таймлайн виджета автоматизации двигает ГЛАВНЫЙ курсор
        let mut seek: Option<kt_time::Tick> = None;
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        // окно главного таймлайна: график автоматизации показывает то же
        // самое, пока не сняли замок (Р.38 п.14)
        let tl_view = self.tl_view();
        // замок оси один на программу (Р.40 п.15)
        let axis_linked = self.auto_axis_linked;
        let dflt_shape = self.model.document().settings.default_shape;
        let mut axis_req = crate::automation::AxisReq::default();
        // «мест: N» у генератора и путь бегущей строки — считаются ДО
        // отрисовки: внутри ui-замыкания `&mut self` уже занят, а счётчик
        // мест линейно обходит дерево и на кадр его считать незачем
        let uses_of = self.media_uses();
        let frame_size = {
            let st = &self.model.project().settings;
            (st.width, st.height)
        };
        let mut open = ed.open;
        let members = ed.members.clone();
        // ---- контекст миниатюр пикера (Р.54, п.432) -------------------
        // WYSIWYG: ячейка показывает ЦЕПОЧКУ СОБЫТИЯ плюс кандидата в
        // хвосте, то есть ровно то, что получится после клика. Кадр —
        // ближайшая готовая тумба к позиции курсора: формула перевода
        // «тик события → кадр источника» ОДНА с филмстрипом
        // (`thumbs::material_key_t`), поэтому ключи общие и нового декода
        // полоска не порождает (инвариант №8).
        let pv_t = {
            let want = anchor
                .start
                .0
                .max(playhead.0 - playhead.0.rem_euclid(frame_step))
                .min((anchor.start.0 + anchor.duration.0 - 1).max(anchor.start.0));
            if self.player.transport.is_playing() {
                *ed.preview_t.get_or_insert(kt_time::Tick(want))
            } else {
                let t = kt_time::Tick(want);
                ed.preview_t = Some(t);
                t
            }
        };
        let pv_span = anchor.duration.0.max(1);
        let pv_u = (pv_t.0 - anchor.start.0) as f64 / pv_span as f64;
        // камера разрешается ПО ЯКОРЮ, но как если бы Pan3d в цепочке был:
        // вариант «Pan 3D» в полоске обязан показать перспективу и на
        // клипе, где 3D ещё не стоит. Правило разрешения — одно с движком
        // (kt_engine::resolve_cam3d, п.429).
        let pv_cam = kt_engine::resolve_cam3d(
            &[kt_model::FxInstance::Pan3d {
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                rot_x: 0.0,
                rot_y: 0.0,
                rot_z: 0.0,
                anchor_x: 0.5,
                anchor_y: 0.5,
                scale: 1.0,
                camera: None,
            }],
            &self.model.project().cameras,
            pv_t,
            crate::fx_thumbs::REF_W,
            self.model.project().settings.width,
        );
        // цепочка ЯКОРЯ с разрешёнными кривыми: значения на момент
        // курсора, домен — материал события (Р.37: скорость ось не жмёт)
        let pv_chain: Vec<FxInstance> = {
            let mat = kt_model::clip_material_at(
                anchor.source_in,
                anchor.start,
                anchor.duration,
                anchor.reversed,
                frame_step,
                pv_t,
            );
            anchor
                .fx
                .iter()
                .enumerate()
                .filter(|(i, f)| {
                    !anchor.fx_bypass.get(*i).copied().unwrap_or(false) && !f.is_audio()
                })
                .map(|(i, f)| {
                    let mut fx = f.clone();
                    if let Some(curves) = anchor.fx_auto.get(i) {
                        kt_model::apply_fx_auto(&mut fx, curves, mat);
                    }
                    fx
                })
                .collect()
        };
        // кадр-основа: точная тумба → постер медиа → встроенный образец.
        // Пустых полосок не бывает, а ЗАКАЗЫ декода ставятся только после
        // мисклик-гарда (п.430).
        let decode_ok = crate::fx_thumbs::decode_orders_allowed(ed.opened_at.elapsed());
        let is_nest = self.model.project().sequence(anchor.media).is_some();
        let pv_step = crate::thumbs::THUMB_STEP_LADDER[1];
        let (pv_base, pv_img) = if is_nest {
            // у ленты своего файла нет, и тумб у неё не бывает (п.395) —
            // показываем образец, а не пустоту
            (crate::fx_thumbs::FxThumbBase::Pattern, None)
        } else {
            let warps = self.warp_maps();
            // адрес ИСТОЧНИКА события, а не его медиа (Р.59, стадия 2):
            // у генератора значения живут на событии, и кадр у каждого
            // свой. Правило одно на программу — `clip_source_addr`.
            let pv_src = kt_engine::clip_source_addr(&anchor);
            let key = crate::thumbs::ThumbKey {
                media: pv_src,
                t: crate::thumbs::material_key_t(
                    anchor.source_in.0,
                    warps.get(&anchor.id),
                    anchor.duration,
                    anchor.reversed,
                    pv_t.0 - anchor.start.0,
                    pv_step,
                ),
            };
            match self.thumbs.get(&key) {
                Some(img) => (crate::fx_thumbs::FxThumbBase::Thumb(key), Some(img)),
                None => {
                    if decode_ok {
                        // ближайшая готовая тумба из sidecar-БД вместо
                        // декода (механизм скраба бина, п.14 р.7)
                        self.thumbs.request_near(key, pv_step * 2);
                    }
                    let poster =
                        if decode_ok { self.thumbs.poster_t_or_request(pv_src) } else { None };
                    let pk = poster
                        .map(|t| crate::thumbs::ThumbKey { media: pv_src, t })
                        .and_then(|pk| self.thumbs.get(&pk).map(|img| (pk, img)));
                    match pk {
                        Some((pk, img)) => (crate::fx_thumbs::FxThumbBase::Thumb(pk), Some(img)),
                        None => (crate::fx_thumbs::FxThumbBase::Pattern, None),
                    }
                }
            }
        };
        let mut pv = crate::fx_thumbs::PickerCtx::new(
            &self.fx_thumbs,
            pv_base,
            pv_img,
            pv_chain,
            pv_u,
            pv_t.0,
            pv_cam,
        );
        // Прокрутка одна — у ТЕЛА (`window_body`): внешний `vscroll` окна
        // прокручивал бы и кнопки действия, и «Готово» уходило за нижнюю
        // кромку (Р.67, нашлось живым сценарием `curves`). Высота окна
        // прежняя — тело само оставляет место кнопкам.
        let shown = egui::Window::new(title)
            .id(egui::Id::new(("fx", ed.clip)))
            .open(&mut open)
            .default_size([580.0, 520.0])
            .min_size([460.0, 320.0])
            .resizable(true)
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                for (n, id) in members.iter().enumerate() {
                    let Some(clip) = self.model.clip(*id).cloned() else { continue };
                    let mut fx = clip.fx.clone();
                    let mut bypass = clip.fx_bypass.clone();
                    bypass.resize(fx.len(), false);
                    let mut auto = clip.fx_auto.clone();
                    auto.resize(fx.len(), Vec::new());
                    // ВЕДУЩИЙ узел «Геометрия» (Р.62) — вклеивается в полосу
                    // на свою позицию, как Pan/Crop в Vegas: перетаскивается,
                    // не удаляется, такой же узел добавляется из каталога.
                    // В модели он — поле события, не слот: индексы слотов и
                    // адреса кривых от него не двигаются.
                    // Вклейка живёт ТОЛЬКО внутри вызова полосы: наружу
                    // индексы слотов остаются сырыми (по ним адресуются
                    // кривые, публикация и контекст-меню), а выбор узла —
                    // свой вариант `ChainSel::Lead`.
                    let lead_pos0 =
                        (clip.kind == ClipKind::Video).then(|| clip.geometry_pos.min(fx.len()));
                    let mut lead_pos = lead_pos0.unwrap_or(0);
                    let mut lead_node = clip.geometry.node();
                    let mut lead_auto = clip.geometry_auto.clone();
                    let mut lead_rule = clip.geometry.rule;
                    let mut chain_on = !clip.fx_off;
                    // ГОЛОВА полосы (Р.59, стадия 2): у события-генератора
                    // это его ИСТОЧНИК, первой крошкой перед эффектами —
                    // ровно как просил владелец. Значения правятся тут же,
                    // в этом же окне; своего окна у генератора больше нет.
                    let mut gen_kind = clip.r#gen.clone();
                    let mut gen_auto = clip.gen_auto.clone();
                    let mut gen_edit = crate::gen_ui::GenEdit::default();
                    let mut ch = crate::widgets::ChainChanged::default();
                    let mut params_changed = crate::widgets::ParamsChanged::default();
                    let head = crate::tf!(
                        k::fx::CHAIN_OF,
                        if clip.kind == ClipKind::Audio {
                            tr(k::fx::KIND_AUDIO)
                        } else {
                            tr(k::fx::KIND_VIDEO)
                        },
                        id.0
                    );
                    // свёртка на цепочку: у пары видно обе, лишнюю можно
                    // свернуть («можно дать возможность скрытия-переключения»)
                    egui::CollapsingHeader::new(head)
                        .id_salt(("fxgrp", id.0))
                        .default_open(n == 0 || members.len() <= 2)
                        .show(ui, |ui| {
                            // выбранный слот и строку поиска берём в
                            // локальные переменные: иначе два поля одного
                            // и того же `ed` одолжены одновременно
                            let mut sel_slot =
                                *ed.selected.get(id).unwrap_or(&crate::widgets::ChainSel::Fx(0));
                            // головы нет — выбирать её нечем: у события без
                            // генератора она и не рисуется
                            if gen_kind.is_none()
                                && !pub_doc.sequence(clip.media).is_some()
                                && sel_slot == crate::widgets::ChainSel::Head
                            {
                                sel_slot = crate::widgets::ChainSel::Fx(0);
                            }
                            if lead_pos0.is_none() && sel_slot == crate::widgets::ChainSel::Lead {
                                sel_slot = crate::widgets::ChainSel::Fx(0);
                            }
                            // ПАРАМЕТРЫ КОМПОЗИЦИИ — ГОЛОВА полосы (Р.63
                            // п.12), а не своя кнопка и не своё окно:
                            // «их можно поместить в тот же fx-chain для
                            // удобства» (владелец). Дверь одна, и она там
                            // же, где голова генератора.
                            let is_nest = pub_doc.sequence(clip.media).is_some();
                            let mut query = std::mem::take(&mut ed.picker_query);
                            let head_slot = match (gen_kind.as_ref(), is_nest) {
                                (Some(g), _) => Some(crate::widgets::HeadSlot {
                                    label: crate::tf!(
                                        k::genw::HEAD,
                                        crate::gen_ui::kind_name(g)
                                    ),
                                    // выключателя у источника нет: выключенный
                                    // он оставил бы событие без картинки вовсе
                                    on: None,
                                    hint: tr(k::genw::HEAD_HINT),
                                    color: crate::theme::ACCENT,
                                }),
                                // у ЭКЗЕМПЛЯРА ленты источника-генератора
                                // нет, зато есть объявленные лентой ручки —
                                // и живут они ровно тут (Р.63 п.12).
                                //
                                // Подпись — ИМЯ ЛЕНТЫ, а не «Параметры
                                // композиции»: длинная строка закрывала
                                // полосу, а акцентный цвет при выделении
                                // давал «длинный синий прямоугольник»
                                // (владелец, Р.67 п.8). Цвет — как у слота,
                                // полное имя — в подсказке строки.
                                (None, true) => Some(crate::widgets::HeadSlot {
                                    label: format!(
                                        "{} {}",
                                        crate::icons::PIN,
                                        pub_doc
                                            .sequence(clip.media)
                                            .map(|s| s.name.clone())
                                            .unwrap_or_else(|| tr(k::pubp::INST_TITLE).to_string())
                                    ),
                                    on: None,
                                    hint: tr(k::pubp::INST_OPEN),
                                    color: ui.visuals().text_color(),
                                }),
                                (None, false) => None,
                            };
                            // спликс ведущего узла на его позицию + перевод
                            // выбора в индексы полосы
                            let (mut fx_s, mut bypass_s, mut auto_s) =
                                (fx.clone(), bypass.clone(), auto.clone());
                            let mut sel_s = sel_slot;
                            if let Some(lp) = lead_pos0 {
                                let lp = lp.min(fx_s.len());
                                fx_s.insert(lp, lead_node.clone());
                                bypass_s.insert(lp, false);
                                auto_s.insert(lp, lead_auto.clone());
                                sel_s = match sel_slot {
                                    crate::widgets::ChainSel::Fx(i) => {
                                        crate::widgets::ChainSel::Fx(if i >= lp {
                                            i + 1
                                        } else {
                                            i
                                        })
                                    }
                                    crate::widgets::ChainSel::Lead => {
                                        crate::widgets::ChainSel::Fx(lp)
                                    }
                                    other => other,
                                };
                            }
                            ch = crate::widgets::chain_strip(
                                ui,
                                ("fxchain", id.0),
                                crate::widgets::ChainEdit {
                                    fx: &mut fx_s,
                                    bypass: &mut bypass_s,
                                    auto: &mut auto_s,
                                    selected: &mut sel_s,
                                    master_on: &mut chain_on,
                                    head: head_slot,
                                    presets: crate::widgets::ChainPresets {
                                        rows: &preset_rows,
                                        save: &mut preset_save,
                                        action: &mut preset_action,
                                    },
                                },
                                clip.kind == ClipKind::Audio,
                                &mut query,
                                &mut fx_clip,
                                clip.source_in,
                                &mut hover_out,
                                &mut crate::widgets::ChainCtx {
                                    chain: *id,
                                    picked: &mut panel_pick,
                                },
                                // у аудио-цепочки картинок нет: миниатюру
                                // звука не нарисовать (решение владельца)
                                if clip.kind == ClipKind::Audio { None } else { Some(&mut pv) },
                            );
                            ed.picker_query = query;
                            // ведущий узел ВЫКЛЕИВАЕТСЯ обратно: в модель
                            // уходят сырая цепочка и отдельно узел с позицией
                            // и кривыми. Крестик на нём — отказ (правило 16):
                            // узел в модели остаётся, человеку — ошибка.
                            if lead_pos0.is_some() {
                                match fx_s.iter().position(|f| f.is_lead_geometry()) {
                                    Some(li) => {
                                        lead_node = fx_s.remove(li);
                                        if li < bypass_s.len() {
                                            bypass_s.remove(li);
                                        }
                                        if li < auto_s.len() {
                                            lead_auto = auto_s.remove(li);
                                        }
                                        lead_pos = li;
                                        sel_s = match sel_s {
                                            crate::widgets::ChainSel::Fx(j) if j == li => {
                                                crate::widgets::ChainSel::Lead
                                            }
                                            crate::widgets::ChainSel::Fx(j) if j > li => {
                                                crate::widgets::ChainSel::Fx(j - 1)
                                            }
                                            other => other,
                                        };
                                    }
                                    None => lead_refused = true,
                                }
                            }
                            fx = fx_s;
                            bypass = bypass_s;
                            auto = auto_s;
                            sel_slot = sel_s;
                            ed.selected.insert(*id, sel_slot);
                            // ---- ГОЛОВА ЭКЗЕМПЛЯРА: параметры композиции
                            // (Р.63 п.12) — та же форма, что жила в своём
                            // окне, в том же окне FX
                            if sel_slot == crate::widgets::ChainSel::Head && is_nest {
                                let hc = crate::pubparam::InstHeadCtx {
                                    doc: &pub_doc,
                                    inst: &clip,
                                    seq: clip.media,
                                    playhead,
                                    tl_view,
                                    axis_linked,
                                };
                                // вид автоматизации у головы свой; у
                                // экземпляра ленты генератора не бывает,
                                // поэтому карта `gen_views` свободна
                                let view = ed.gen_views.entry(*id).or_default();
                                let out = crate::app::KtApp::draw_inst_head(
                                    &hc,
                                    ui,
                                    view,
                                    &mut ed.inst_query,
                                );
                                if let Some(t) = out.seek {
                                    seek = Some(t);
                                }
                                axis_req = axis_req.merge_req(&out.axis);
                                inst_ops.extend(out.ops);
                            }
                            // ---- ГОЛОВА: форма источника (Р.59) --------
                            if let (crate::widgets::ChainSel::Head, Some(g)) =
                                (sel_slot, gen_kind.as_mut())
                            {
                                let gen_pubc = self.publish_ctx(
                                    &pub_doc,
                                    *id,
                                    kt_model::ParamOwner::Generator,
                                );
                                let gen_link_doc = &pub_doc;
                                let gen_me = *id;
                                let gen_links =
                                    move || crate::automation::link_events(gen_link_doc, gen_me);
                                let gctx = crate::gen_ui::GenCtx {
                                    links: Some(&gen_links),
                                    default_shape: dflt_shape,
                                    clip: &clip,
                                    pubc: Some(&gen_pubc),
                                    uses: uses_of.get(&clip.media).copied().unwrap_or(1),
                                    frame: frame_size,
                                    frame_step,
                                    playhead,
                                    tl_view,
                                    axis_linked,
                                };
                                let view = ed.gen_views.entry(*id).or_default();
                                let warn = ed.gen_warn.entry(*id).or_default();
                                gen_edit = crate::gen_ui::draw_gen_section(
                                    ui,
                                    &gctx,
                                    g,
                                    &mut gen_auto,
                                    view,
                                    warn,
                                    &mut hover_out,
                                );
                                if let Some(t) = gen_edit.seek {
                                    seek = Some(t);
                                }
                                // просьбы про ОСЬ собираются с ВСЕХ графиков
                                // окна и применяются один раз (Р.40 п.15)
                                axis_req.set_linked =
                                    gen_edit.axis.set_linked.or(axis_req.set_linked);
                                axis_req.set_view = gen_edit.axis.set_view.or(axis_req.set_view);
                            }
                            // ВЕДУЩИЙ узел правится тем же кодом, что слот:
                            // на время секции он кладётся в хвост локальной
                            // цепочки и снимается обратно (адрес кривых у
                            // него свой — `usize::MAX`)
                            let lead_sel =
                                sel_slot == crate::widgets::ChainSel::Lead && lead_pos0.is_some();
                            if lead_sel {
                                fx.push(lead_node.clone());
                                auto.push(lead_auto.clone());
                            }
                            let sel = if lead_sel {
                                fx.len() - 1
                            } else {
                                sel_slot.fx().unwrap_or(usize::MAX)
                            };
                            let view_key = if lead_sel { usize::MAX } else { sel };
                            if let Some(f) = fx.get_mut(sel) {
                                // параметры — ОДНИМ кодом по спекам модели
                                // позиция курсора в домене кривой слота —
                                // материал события (Р.37: скорость ось не
                                // сжимает)
                                let at = kt_model::clip_material_at(
                                    clip.source_in,
                                    clip.start,
                                    clip.duration,
                                    clip.reversed,
                                    frame_step,
                                    playhead,
                                );
                                let view_sel =
                                    ed.auto_views.get(&(*id, view_key)).and_then(|v| v.sel);
                                let mut bind = auto.get_mut(sel).map(|curves| {
                                    crate::widgets::CurveBind { curves, sel: view_sel, at }
                                });
                                // Р.43 п.291: правка ключа ЧИСЛОМ уходит в
                                // `auto`, а не в `fx` — и коммититься обязана
                                // командой SetFxAuto. Пока флаг был один,
                                // локальный клон `auto` умирал вместе с
                                // кадром, и поле «скидывалось на предыдущее
                                // значение».
                                // ДВЕРЬ ПУБЛИКАЦИИ ЕСТЬ И У ВЕДУЩЕГО УЗЛА
                                // (Р.63 п.9): «по какой причине у первого
                                // geometry нет возможности опубликовать
                                // параметры а у второго есть?» — разница
                                // была только в том, что у узла не было
                                // адреса. Теперь он есть, и подписи полей
                                // ведут в ту же дверь тем же кодом.
                                let pubc = Some(self.publish_ctx(
                                    &pub_doc,
                                    *id,
                                    if lead_sel {
                                        kt_model::ParamOwner::Geometry
                                    } else {
                                        kt_model::ParamOwner::FxSlot { slot: sel }
                                    },
                                ));
                                // правило приведения — только у ВЕДУЩЕГО
                                // узла: «следовать проекту» либо своё
                                if f.is_lead_geometry() {
                                    let label = |c: kt_model::Conform| match c {
                                        kt_model::Conform::Project => tr(k::fx::CONFORM_PROJECT),
                                        kt_model::Conform::Fit => tr(k::fx::CONFORM_FIT),
                                        kt_model::Conform::Fill => tr(k::fx::CONFORM_FILL),
                                        kt_model::Conform::Stretch => tr(k::fx::CONFORM_STRETCH),
                                        kt_model::Conform::AsIs => tr(k::fx::CONFORM_AS_IS),
                                    };
                                    // ряд из пяти слов, а не выпадающий список:
                                    // один клик, всё видно (правило 9 — режим
                                    // виден, а не спрятан)
                                    let r = ui.horizontal(|ui| {
                                        ui.label(tr(k::fx::CONFORM));
                                        for c in [
                                            kt_model::Conform::Project,
                                            kt_model::Conform::Fit,
                                            kt_model::Conform::Fill,
                                            kt_model::Conform::Stretch,
                                            kt_model::Conform::AsIs,
                                        ] {
                                            params_changed.base |= ui
                                                .selectable_value(&mut lead_rule, c, label(c))
                                                .changed();
                                        }
                                    });
                                    crate::widgets::field_probe::note(
                                        "geo.conform",
                                        r.response.rect,
                                    );
                                    crate::app::hint_into(
                                        &mut hover_out,
                                        r.response,
                                        tr(k::fx::LEAD_HINT),
                                    );
                                }
                                // ГРУППА «Параметры» — образец «Трансформации»
                                // (Р.66); у ведущего узла своя форма с
                                // собственными группами, её не дублируем
                                if f.is_lead_geometry() {
                                    params_changed |= draw_video_fx_params(
                                        ui,
                                        f,
                                        bind.as_mut(),
                                        &cameras,
                                        clip.kind == ClipKind::Video,
                                        &mut hover_out,
                                        pubc.as_ref(),
                                    );
                                } else if let Some(ch) = crate::widgets::group(
                                    ui,
                                    "fx.params",
                                    tr(k::grp::PARAMS),
                                    |ui| {
                                        draw_video_fx_params(
                                            ui,
                                            f,
                                            bind.as_mut(),
                                            &cameras,
                                            clip.kind == ClipKind::Video,
                                            &mut hover_out,
                                            pubc.as_ref(),
                                        )
                                    },
                                ) {
                                    params_changed |= ch;
                                }
                                let mut auto_ch = false;
                                if let Some(curves) = auto.get_mut(sel) {
                                    let view = ed.auto_views.entry((*id, view_key)).or_default();
                                    // домен слота события — материал;
                                    // перевод учитывает РЕВЕРС и больше
                                    // ничего (Р.37: скорость ось не
                                    // сжимает)
                                    let to_tl = |t: kt_time::Tick| {
                                        kt_model::clip_timeline_at(
                                            clip.source_in,
                                            clip.start,
                                            clip.duration,
                                            clip.reversed,
                                            frame_step,
                                            t,
                                        )
                                    };
                                    let to_mat = |t: kt_time::Tick| {
                                        kt_model::clip_material_at(
                                            clip.source_in,
                                            clip.start,
                                            clip.duration,
                                            clip.reversed,
                                            frame_step,
                                            t,
                                        )
                                    };
                                    let link_doc = &pub_doc;
                                    let me = *id;
                                    let links = move || crate::automation::link_events(link_doc, me);
                                    let actx = crate::automation::clip_ctx(
                                        &clip,
                                        f,
                                        frame_step,
                                        &to_tl,
                                        &to_mat,
                                        tl_view,
                                        axis_linked,
                                        dflt_shape,
                                        Some(&links),
                                    );
                                    if paste_req && paste_target.is_none() && !lead_sel {
                                        paste_target = Some((*id, sel));
                                    }
                                    let r = draw_auto_for(
                                        ui,
                                        f,
                                        curves,
                                        view,
                                        actx,
                                        playhead,
                                        &mut paste_req,
                                        (id.0, view_key),
                                        &mut hover_out,
                                    );
                                    auto_ch = r.changed;
                                    if let Some(e) = &r.error {
                                        auto_error = Some(e.clone());
                                    }
                                    // правый клик по ключу — тоже цель (п.364);
                                    // у ведущего узла адреса слота нет
                                    if let (Some((ci, ki)), false) = (r.key_menu, lead_sel) {
                                        panel_pick = Some(crate::ctx_menu::PanelTarget::CurveKey {
                                            chain: *id,
                                            slot: sel,
                                            curve: ci,
                                            key: ki,
                                        });
                                    }
                                    // Поле параметра читает выбранный ключ
                                    // ДО отрисовки графика, поэтому смена
                                    // выбора видна ему только следующим
                                    // кадром. В простое следующего кадра
                                    // может не быть (repaint = движение,
                                    // п.38), и поле застревало на значении
                                    // прошлого ключа — просим кадр явно
                                    // (Р.43 п.291).
                                    if view.sel != view_sel {
                                        ui.ctx().request_repaint();
                                    }
                                    if let Some(t) = r.seek {
                                        seek = Some(t);
                                    }
                                    axis_req = axis_req.merge(&r);
                                }
                                ch.auto |= auto_ch;
                            }
                            if lead_sel {
                                lead_auto = auto.pop().unwrap_or_default();
                                lead_node = fx.pop().unwrap_or_else(FxInstance::geometry);
                            }
                            // про пустую цепочку говорит сама полоса —
                            // второй раз повторять не надо
                        });
                    // ведущий узел ВЫКЛЕИВАЕТСЯ обратно: в модель уходят
                    // сырая цепочка и отдельно узел с позицией и кривыми.
                    // Крестик на нём — отказ (правило 16): узел в модели
                    // остаётся, человеку — ошибка в строке состояния.
                    let geometry_edit = lead_pos0.map(|_| {
                        let g = kt_model::EventGeometry { rule: lead_rule, ..Default::default() }
                            .with_node(&lead_node);
                        (g, lead_pos.min(fx.len()), lead_auto.clone())
                    });
                    let geo_changed = geometry_edit.as_ref().is_some_and(|(g, p, a)| {
                        *g != clip.geometry || *p != clip.geometry_pos || *a != clip.geometry_auto
                    });
                    if ch.any()
                        || params_changed.any()
                        || gen_edit.changed.any()
                        || gen_edit.apply_all
                        || gen_edit.switch_kind.is_some()
                        || geo_changed
                    {
                        edits.push(Edited {
                            clip: *id,
                            fx: (ch.fx || params_changed.base).then_some(fx),
                            auto: (ch.auto || params_changed.curve).then_some(auto),
                            bypass: ch.bypass.then_some(bypass),
                            off: ch.master.then_some(!chain_on),
                            r#gen: gen_edit.changed.base.then_some(gen_kind).flatten(),
                            gen_auto: gen_edit.changed.curve.then_some(gen_auto),
                            own_step: gen_edit.preset,
                            apply_all: gen_edit.apply_all,
                            switch_kind: gen_edit.switch_kind,
                            geometry: geo_changed.then_some(geometry_edit).flatten(),
                        });
                    }
                }
                });
                crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    // прямоугольники кнопок — наружу живым сценариям (Р.27)
                    let r = ui.button(tr(k::fx::DONE));
                    crate::widgets::field_probe::note("fx.done", r.rect);
                    if r.clicked() {
                        ed.open = false;
                    }
                    let r = ui.button(tr(k::fx::CANCEL));
                    crate::widgets::field_probe::note("fx.cancel", r.rect);
                    if r.clicked() {
                        revert = true;
                        ed.open = false;
                    }
                });
                });
            });
        // рамка окна целиком — по ней режут скриншоты лендинга (site/screenshots.py)
        if let Some(r) = &shown {
            crate::widgets::field_probe::note("win.fx", r.response.rect);
        }

        // запросы полосы по пресетам (Р.48): полоса их только кладёт —
        // файл пишет окно, потому что диалог имени живёт у приложения
        if let Some(mut block) = preset_save.take() {
            // ведущий узел — свойство события, в пресет цепочки не идёт
            block.slots.retain(|s| !s.fx.is_lead_geometry());
            self.preset_save = Some(crate::app::PresetSaveDialog {
                block,
                name: String::new(),
                rename_of: None,
                focus: true,
            });
        }
        match preset_action.take() {
            Some(crate::widgets::PresetAction::Rename(old)) => {
                self.preset_save = Some(crate::app::PresetSaveDialog {
                    // переименование блок не использует
                    block: kt_model::ops::FxChainBlock {
                        slots: vec![],
                        src_in: kt_time::Tick(0),
                        fx_off: false,
                    },
                    name: old.clone(),
                    rename_of: Some(old),
                    focus: true,
                });
            }
            Some(crate::widgets::PresetAction::Delete(name)) => match self.presets.remove(&name) {
                Ok(()) => self.note(format!("{} {name}", tr(k::msg::PRESET_DELETED))),
                Err(e) => self.note(format!("{}: {e}", tr(k::msg::PRESET_SAVE_FAILED))),
            },
            None => {}
        }
        if let Some(t) = panel_pick {
            self.set_panel_target(t); // цели гасят друг друга (п.363)
        }
        for c in inst_ops {
            self.apply_coalesced(c, ed.gesture);
        }
        if let Some(h) = hover_out {
            self.hover_hint = Some(h); // подсказка едет в строку состояния
        }
        if let Some(e) = auto_error {
            self.refuse(e);
        }
        if lead_refused {
            self.refuse(tr(k::fx::LEAD_LOCKED));
        }
        // просьбы графика про ОСЬ применяет одно место (Р.40 п.15)
        self.apply_axis_req(axis_req);
        if let Some(t) = seek {
            // «курсор внутри мини-таймлайна двигает курсор в основном
            // таймлайне, чтобы сразу видеть к чему относится» (Р.33 п.5)
            self.player.seek(kt_time::Tick(t.0.max(0)));
        }
        if !open {
            ed.open = false;
        }
        self.fx_clipboard = fx_clip;
        // Заказ чтения буфера — ПОСЛЕ отрисовки и в ФОНЕ (Р.61): читать
        // синхронно нельзя, владелец буфера на X11 — наше же окно.
        if paste_req && self.env_read.is_none() {
            let t = paste_target.unwrap_or((ed.clip, 0));
            self.env_read = Some(crate::envclip::spawn_read(t));
        }
        if revert {
            // «Отмена» откатывает ВСЁ окно: у пары обе цепочки
            for (id, fx, auto, bypass, off) in ed.backup.clone() {
                self.set_fx_coalesced(id, fx, ed.gesture);
                self.set_fx_auto_coalesced(id, auto, ed.gesture);
                self.apply_coalesced(Command::SetFxBypass { id, bypass }, ed.gesture);
                self.apply_coalesced(Command::SetFxOff { id, off }, ed.gesture);
            }
            // ...и ЗНАЧЕНИЯ ИСТОЧНИКА тоже: голова полосы — часть той же
            // сессии окна, и откатывать половину её было бы ложью
            for (id, kind, auto) in ed.gen_backup.clone() {
                if self.model.clip(id).is_none_or(|c| c.r#gen == kind && c.gen_auto == auto) {
                    continue;
                }
                self.apply_coalesced(Command::SetClipGen { id, kind: kind.clone() }, ed.gesture);
                self.apply_coalesced(Command::SetClipGenAuto { id, auto }, ed.gesture);
            }
            // ...и ведущий узел «Геометрия» (Р.62) — та же полоса
            for (id, geometry, pos, auto) in ed.geo_backup.clone() {
                if self.model.clip(id).is_none_or(|c| {
                    c.geometry == geometry && c.geometry_pos == pos && c.geometry_auto == auto
                }) {
                    continue;
                }
                self.apply_coalesced(Command::SetGeometry { id, geometry, pos, auto }, ed.gesture);
            }
            // ...и РУЧКИ КОМПОЗИЦИИ из головы полосы (Р.63 п.12)
            for (id, was) in ed.ov_backup.clone() {
                let now = self.model.clip(id).map(|c| c.overrides.clone()).unwrap_or_default();
                if now == was {
                    continue;
                }
                for o in &now {
                    if !was.iter().any(|b| b.pub_id == o.pub_id) {
                        self.apply_coalesced(
                            Command::ClearOverride { id, pub_id: o.pub_id },
                            ed.gesture,
                        );
                    }
                }
                for b in was {
                    self.apply_coalesced(
                        Command::SetOverride { id, ov: Box::new(b) },
                        ed.gesture,
                    );
                }
            }
            return;
        }
        for e in edits {
            // Смена ВИДА — свойство МЕДИА и СВОЙ шаг истории (правило 20):
            // один Ctrl+Z обязан отменять «случайно ткнул другой вид», а
            // не всю работу в окне. Значения событий переодевает сама
            // модель (`fit_gen_to_media`), поэтому команда одна.
            if let Some(kind) = e.switch_kind {
                let Some(media) = self.model.clip(e.clip).map(|c| c.media) else { continue };
                self.gesture_counter += 1;
                let g = self.gesture_counter;
                self.apply_coalesced(
                    Command::SetMediaPath { id: media, path: kt_model::gen_kind_path(&kind) },
                    g,
                );
                continue;
            }
            // Пресет ставит ВСЁ (значения и кривые) — тоже свой шаг
            // истории: нажал, не понравилось, один Ctrl+Z вернул набор.
            let gesture = if e.own_step {
                self.gesture_counter += 1;
                self.gesture_counter
            } else {
                ed.gesture
            };
            if let Some(fx) = e.fx {
                self.set_fx_coalesced(e.clip, fx, gesture);
            }
            if let Some(mut auto) = e.auto {
                // ключи держим отсортированными (модель это валидирует)
                for curves in &mut auto {
                    for c in curves.iter_mut() {
                        c.keys.sort_by_key(|k| k.t.0);
                    }
                }
                self.set_fx_auto_coalesced(e.clip, auto, gesture);
            }
            if let Some(mut b) = e.bypass {
                while b.last() == Some(&false) {
                    b.pop(); // хвостовые false не сериализуются зря
                }
                self.apply_coalesced(Command::SetFxBypass { id: e.clip, bypass: b }, gesture);
            }
            if let Some(off) = e.off {
                self.apply_coalesced(Command::SetFxOff { id: e.clip, off }, gesture);
            }
            if let Some((geometry, pos, mut auto)) = e.geometry {
                for c in auto.iter_mut() {
                    c.keys.sort_by_key(|k| k.t.0);
                }
                self.apply_coalesced(
                    Command::SetGeometry { id: e.clip, geometry, pos, auto },
                    gesture,
                );
            }
            // Значения генератора: команда ТОЛЬКО при реально изменившемся
            // значении (п.200) — иначе ревизия росла бы каждый кадр жеста.
            if let Some(kind) = e.r#gen
                && self.model.clip(e.clip).is_some_and(|c| c.r#gen.as_ref() != Some(&kind))
            {
                self.apply_coalesced(Command::SetClipGen { id: e.clip, kind: Some(kind) }, gesture);
            }
            if let Some(mut auto) = e.gen_auto {
                for c in auto.iter_mut() {
                    c.keys.sort_by_key(|k| k.t.0);
                }
                if self.model.clip(e.clip).is_some_and(|c| c.gen_auto != auto) {
                    self.apply_coalesced(Command::SetClipGenAuto { id: e.clip, auto }, gesture);
                }
            }
            // «Применить ко всем» — ОТДЕЛЬНАЯ команда и отдельный шаг
            // истории: правка одного события соседей не трогает, а этот
            // жест трогает все и говорит об этом подписью.
            if e.apply_all
                && let Some(c) = self.model.clip(e.clip).cloned()
                && let Some(kind) = c.r#gen.clone()
            {
                self.gesture_counter += 1;
                let g = self.gesture_counter;
                self.apply_coalesced(
                    Command::ApplyGenToAll { media: c.media, kind, auto: c.gen_auto.clone() },
                    g,
                );
            }
        }
    }
}

/// Секция автоматизации слота: выбор кривых, копипаст огибающих, ключ по
/// курсору — и сам виджет (`automation::draw`: график ↔ таблица).
/// У слота без автоматизируемых параметров (Mirror) не рисуется вовсе.
///
/// Возвращает `AutoResult` целиком (Р.40 п.15): кроме «изменилось» и «куда
/// двинуть курсор» в нём едут просьбы про ось — переключить замок и
/// подвинуть окно ГЛАВНОГО таймлайна. Пара `(bool, Option<Tick>)` их
/// молча теряла бы, а замок теперь один на всю программу.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_auto_for(
    ui: &mut egui::Ui,
    f: &FxInstance,
    curves: &mut Vec<kt_model::ParamCurve>,
    view: &mut crate::automation::AutoView,
    ctx: crate::automation::AutoCtx<'_>,
    playhead: kt_time::Tick,
    // Нажата ли «вставить огибающую»: сам буфер читается ПОЗЖЕ и в ФОНЕ
    // — см. `crate::envclip` (Р.61).
    paste_req: &mut bool,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    hover_out: &mut Option<String>,
) -> crate::automation::AutoResult {
    let (src_in, mat_end) = (ctx.range.0, ctx.range.1.0);
    // только Smooth-параметры: структурные (длина линий реверба) кривой
    // не подлежат — их смена пересобирает DSP (Р.33)
    let specs: Vec<&kt_model::ParamSpec> = f.auto_params().collect();
    if specs.is_empty() {
        return crate::automation::AutoResult::default();
    }
    let mut changed = false;
    ui.separator();
    ui.horizontal(|ui| {
        // параметры без кривой — кандидаты на добавление
        let free: Vec<&kt_model::ParamSpec> =
            specs.iter().copied().filter(|s| !curves.iter().any(|c| c.param == s.name)).collect();
        if !free.is_empty() {
            // МЕНЮ, а не ComboBox: выбранный текст у него никогда не
            // менялся (это список действий, а не селектор), и заодно у
            // кнопки появляется имя для доступности — ComboBox отдавал
            // пустое, и до пункта нельзя было добраться иначе как мышью
            let add = ui.menu_button(tr(k::fx::AUTO_ADD), |ui| {
                {
                    for spec in &free {
                        let n: &str = &spec.name;
                        let item = ui.button(crate::widgets::param_label(n));
                        // щуп на каждый пункт: живому сценарию иначе не во
                        // что целиться (правило Р.27 — координаты от самой
                        // программы)
                        crate::widgets::field_probe::note(&format!("auto.add.{n}"), item.rect);
                        if item.clicked() {
                            // стартовая кривая — константа текущего значения
                            let v0 = f.get_param(n).unwrap_or(spec.default);
                            curves.push(kt_model::ParamCurve::constant(
                                n,
                                src_in,
                                kt_time::Tick(mat_end),
                                v0,
                                ctx.default_shape,
                            ));
                            // новый параметр становится АКТИВНЫМ (Р.38
                            // п.17: «добавил второй параметр — но
                            // редактируется только первый»): раньше клик
                            // по графику продолжал класть ключи в первую
                            // кривую, и вторая выглядела мёртвой
                            view.active = curves.len() - 1;
                            view.sel = None;
                            changed = true;
                        }
                    }
                }
            });
            crate::widgets::field_probe::note("auto.add", add.response.rect);
        }
        // выбор активной кривой, кнопки ключей и БЛОК КРИВОЙ (форма всех
        // отрезков, формула, связь, удалить) — В САМОМ ВИДЖЕТЕ (Р.67):
        // их получают все носители кривых разом
    });
    ui.horizontal(|ui| {
        // копипаст огибающих (модель — kt_model::ops, Р.33)
        let cp = ui.add_enabled(!curves.is_empty(), egui::Button::new(tr(k::fx::COPY_ENV)));
        crate::widgets::field_probe::note("env.copy", cp.rect);
        if cp.clicked() {
            let block = kt_model::ops::EnvelopeBlock {
                fx_tag: kt_model::ops::fx_tag(f),
                curves: curves.clone(),
                src_in,
            };
            if let Ok(json) = serde_json::to_string_pretty(&block) {
                ui.ctx().copy_text(json);
            }
        }
        // Кнопка НИКОГДА не гаснет: содержимое буфера читается ПО
        // НАЖАТИЮ, а не хранится зеркалом внутри программы (Р.61).
        // Чтение — конечной фоновой задачей: на X11 буфером владеет окно, и
        // синхронный запрос к САМИМ СЕБЕ повис бы в цикле событий, который
        // в этот момент рисует.
        let pb = ui.button(tr(k::fx::PASTE_ENV));
        crate::widgets::field_probe::note("env.paste", pb.rect);
        if crate::app::hint_into(hover_out, pb, tr(k::fx::PASTE_ENV_HINT)).clicked() {
            *paste_req = true;
        }
        // Кнопка «+ keyframe at playhead» отсюда УБРАНА (Р.40 п.18,
        // дословно: «а еще получается "+keyframe at playhead" кнопка
        // лишняя т.к. есть в таймлайне автоматизации "+"»). Её
        // единственное отличие от «+» — она писала во ВСЕ кривые слота, о
        // чём подпись не говорила ни слова. Это поведение переехало на
        // «+» модификатором Ctrl, и подсказка о нём говорит (правило 18).
    });
    // единый виджет: график ↔ таблица (Р.33 С3)
    let mut res =
        crate::automation::draw(ui, ("auto", id), &ctx, curves, view, playhead, hover_out);
    res.changed |= changed;
    res
}
pub(crate) fn fx_display_name(f: &FxInstance) -> &'static str {
    match f {
        FxInstance::Gain { .. } => "Gain",
        // сила слоя не пользовательский эффект: в пикере её нет, а в
        // полосе морфера она подписана как «Сила» (Р.38 п.12)
        FxInstance::LayerMix { .. } => "Layer mix",
        FxInstance::SlowedReverb { .. } => "Slowed+Reverb",
        // ведущий узел помечен точкой: он один, неудаляем и не из каталога
        FxInstance::Geometry { lead: true, .. } => "• Geometry",
        FxInstance::Geometry { .. } => "Geometry",
        FxInstance::CamMotion { .. } => "CamMotion",
        FxInstance::Mirror { .. } => "Mirror",
        FxInstance::Blur { .. } => "Blur",
        FxInstance::Greenscreen { .. } => "Greenscreen",
        FxInstance::Edges { .. } => "Edges",
        FxInstance::Noise { .. } => "Noise",
        FxInstance::Vhs { .. } => "VHS",
        FxInstance::Glow { .. } => "Glow",
        FxInstance::EventGrid { .. } => "EventGrid",
        FxInstance::Pan3d { .. } => "Pan 3D",
        FxInstance::Filter { .. } => "Filter",
        FxInstance::Eq { .. } => "EQ",
        FxInstance::Distortion { .. } => "Distortion",
        FxInstance::Bitcrusher { .. } => "Bitcrusher",
        FxInstance::StereoWidth { .. } => "Stereo Width",
        FxInstance::Tremolo { .. } => "Tremolo",
        FxInstance::AutoPan { .. } => "AutoPan",
        FxInstance::Vibrato { .. } => "Vibrato",
        FxInstance::Chorus { .. } => "Chorus",
        FxInstance::Flanger { .. } => "Flanger",
        FxInstance::Phaser { .. } => "Phaser",
        FxInstance::Delay { .. } => "Delay",
        FxInstance::Compressor { .. } => "Compressor",
        FxInstance::Gate { .. } => "Gate",
        FxInstance::Limiter { .. } => "Limiter",
        FxInstance::PitchShift { .. } => "Pitch Shift",
        FxInstance::ColorGrade { .. } => "Color Grade",
        FxInstance::Duotone { .. } => "Duotone",
        FxInstance::Posterize { .. } => "Posterize",
        FxInstance::Vignette { .. } => "Vignette",
        FxInstance::CornerPin { .. } => "Corner Pin",
        FxInstance::Sharpen { .. } => "Sharpen",
        FxInstance::Pixelate { .. } => "Pixelate",
        FxInstance::ChromaticAberration { .. } => "Chromatic Aberration",
        FxInstance::RadialBlur { .. } => "Radial Blur",
        FxInstance::DropShadow { .. } => "Drop Shadow",
        FxInstance::Halftone { .. } => "Halftone",
        FxInstance::LensDistort { .. } => "Lens Distort",
        FxInstance::Twirl { .. } => "Twirl",
        FxInstance::Ripple { .. } => "Ripple",
        FxInstance::Kaleidoscope { .. } => "Kaleidoscope",
        FxInstance::Cylinder3d { .. } => "Cylinder 3D",
    }
}

/// Шаблоны видео-FX с дефолтами v1 (кнопка «+ видео-FX…»).
pub(crate) fn video_fx_templates() -> Vec<(&'static str, FxInstance)> {
    vec![
        // CamMotion из списка убран (Р.34 К2): его кейфреймы — это Pan
        // с кривыми, и старые проекты конвертируются при чтении.
        // Р.62: узел «Геометрия» из каталога — дополнительный (ведущий у
        // события есть всегда и в каталоге не живёт); прежний Pan упразднён
        ("Geometry", FxInstance::geometry_node()),
        ("Mirror", FxInstance::Mirror { horizontal: true, vertical: false }),
        ("Blur", FxInstance::blur(8)),
        ("Greenscreen", FxInstance::greenscreen([0, 255, 0], 70, 25)),
        ("Edges", FxInstance::edges(48)),
        ("Noise", FxInstance::noise(0.08)),
        ("VHS", FxInstance::vhs(4, 0.4)),
        ("Glow", FxInstance::glow(0.65)),
        (
            // Р.29: первый 3D-эффект; дефолты — тождество (слой 1:1)
            "Pan 3D",
            FxInstance::Pan3d {
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                rot_x: 0.0,
                rot_y: 0.0,
                rot_z: 0.0,
                anchor_x: 0.5,
                anchor_y: 0.5,
                scale: 1.0,
                camera: None,
            },
        ),
        ("EventGrid", FxInstance::EventGrid { cols: 2, rows: 2, center: true }),
        // Р.47: поточечные цвет/стиль + первая честная перспектива
        ("Color Grade", FxInstance::color_grade()),
        ("Duotone", FxInstance::duotone()),
        ("Posterize", FxInstance::posterize()),
        ("Vignette", FxInstance::vignette()),
        ("Corner Pin", FxInstance::corner_pin()),
        // Р.47 Кв3: барьеры
        ("Sharpen", FxInstance::sharpen()),
        ("Pixelate", FxInstance::pixelate()),
        ("Chromatic Aberration", FxInstance::chromatic_aberration()),
        ("Radial Blur", FxInstance::radial_blur()),
        ("Drop Shadow", FxInstance::drop_shadow()),
        ("Halftone", FxInstance::halftone()),
        // Р.47 Кв4: нелинейные гео (Warp-прим) + цилиндр
        ("Lens Distort", FxInstance::lens_distort()),
        ("Twirl", FxInstance::twirl()),
        ("Ripple", FxInstance::ripple()),
        ("Kaleidoscope", FxInstance::kaleidoscope()),
        ("Cylinder 3D", FxInstance::cylinder3d()),
    ]
}

/// Параметры эффекта: числовые — ЕДИНОЙ сеткой по спекам модели
/// (`widgets::params_grid`), а рядом — то, что числом не выражается:
/// режим кроп-рамки, флаги зеркала, пипетка ключевого цвета, выбор
/// камеры, кейфреймы CamMotion. `cameras` — (id, подпись) для комбо Pan3d.
///
/// До Р.33 у каждого варианта была своя ручная форма: одни и те же
/// величины рисовались слайдером в одном окне и drag-полем в другом, а
/// диапазоны расходились между окнами и спекой. Теперь диапазон, шаг и
/// единица приходят из одного места.
/// Ряды ключа Pan-камеры (Р.66). Пределы и единицы объявлены ЗДЕСЬ, рядом
/// друг с другом: у ключа нет `ParamSpec` (это точка траектории, а не
/// параметр эффекта), но и вписывать числа в код отрисовки нельзя — иначе
/// у одного и того же ключа в двух местах окажутся разные границы.
fn pan_key_rows() -> (
    crate::widgets::NumRow<'static>,
    crate::widgets::NumRow<'static>,
    crate::widgets::NumRow<'static>,
    crate::widgets::NumRow<'static>,
) {
    use kt_model::ParamUnit;
    (
        crate::widgets::NumRow {
            label: "",
            probe: "pan.key.t",
            unit: ParamUnit::Norm,
            min: 0.0,
            max: 1.0,
            step: 0.005,
            slider: None,
        },
        crate::widgets::NumRow {
            label: tr(k::fxp::PAN_X),
            probe: "pan.key.x",
            unit: ParamUnit::Norm,
            min: -4.0,
            max: 4.0,
            step: 0.01,
            slider: None,
        },
        crate::widgets::NumRow {
            label: tr(k::fxp::PAN_Y),
            probe: "pan.key.y",
            unit: ParamUnit::Norm,
            min: -4.0,
            max: 4.0,
            step: 0.01,
            slider: None,
        },
        crate::widgets::NumRow {
            label: tr(k::fxp::ZOOM),
            probe: "pan.key.zoom",
            unit: ParamUnit::Ratio,
            min: 0.05,
            max: 32.0,
            step: 0.01,
            slider: None,
        },
    )
}

pub(crate) fn draw_video_fx_params(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    mut bind: Option<&mut crate::widgets::CurveBind<'_>>,
    cameras: &[(u64, String)],
    on_video: bool,
    // накопитель подсказок кадра (Р.48: у графика АЧХ своя подсказка)
    hover: &mut Option<String>,
    // контекст ПУБЛИКАЦИИ подписи (Р.59, стадия 3). `None` у носителя,
    // адреса которому не существует, — морфер-патча.
    pubc: Option<&crate::widgets::PublishCtx>,
) -> crate::widgets::ParamsChanged {
    use kt_model::PanMode;
    // Р.43 п.291: правка БАЗЫ и правка КРИВОЙ — разные команды. Всё
    // нечисловое (галки, режимы, пипетки, выбор камеры) — всегда база.
    let mut ch = crate::widgets::ParamsChanged::default();
    // ключевой цвет удобнее пипеткой, чем тремя числами
    let skip: &[&str] = match f {
        FxInstance::Greenscreen { .. } => &["key_r", "key_g", "key_b"],
        // цвет линий контура и оттенок свечения — тоже пипеткой (Р.33 С5)
        FxInstance::Edges { .. } => &["color_r", "color_g", "color_b"],
        FxInstance::Glow { .. } => &["tint_r", "tint_g", "tint_b"],
        FxInstance::DropShadow { .. } => &["tint_r", "tint_g", "tint_b"],
        // обе краски дуотона — пипетками (Р.47 Кв2)
        FxInstance::Duotone { .. } => {
            &["shadow_r", "shadow_g", "shadow_b", "high_r", "high_g", "high_b"]
        }
        // на ВИДЕО-экземпляре у варпа значит только скорость: реверб там
        // не звучит, и показывать его ручки — врать пользователю (Р.37)
        FxInstance::SlowedReverb { .. } if on_video => {
            &["mix", "reverb_delay_ms", "damp", "decay", "wet_gain", "predelay_ms", "width"]
        }
        _ => &[],
    };

    // --- нечисловые контролы варианта ------------------------------------
    let mode_row =
        |ui: &mut egui::Ui, mode: &mut PanMode, ch: &mut crate::widgets::ParamsChanged| {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::MODE));
                egui::ComboBox::from_id_salt("panmode").selected_text(format!("{mode:?}")).show_ui(
                    ui,
                    |ui| {
                        for m in [PanMode::Fit, PanMode::Fill, PanMode::Stretch] {
                            ch.base |= ui.selectable_value(mode, m, format!("{m:?}")).changed();
                        }
                    },
                );
            });
        };
    match f {
        FxInstance::Delay { pingpong, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(pingpong, tr(k::fx::PINGPONG)).changed();
            });
        }
        // вид среза (Р.47): подписи из каталога, значения — serde-виды
        FxInstance::Filter { mode, .. } => {
            let label = |m: kt_model::FilterMode| match m {
                kt_model::FilterMode::Lowpass => tr(k::fx::FILTER_LP),
                kt_model::FilterMode::Highpass => tr(k::fx::FILTER_HP),
                kt_model::FilterMode::Bandpass => tr(k::fx::FILTER_BP),
            };
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::MODE));
                egui::ComboBox::from_id_salt("filtermode").selected_text(label(*mode)).show_ui(
                    ui,
                    |ui| {
                        for m in [
                            kt_model::FilterMode::Lowpass,
                            kt_model::FilterMode::Highpass,
                            kt_model::FilterMode::Bandpass,
                        ] {
                            ch.base |= ui.selectable_value(mode, m, label(m)).changed();
                        }
                    },
                );
            });
        }
        FxInstance::CamMotion { mode, ease, .. } => {
            let mut e = *ease;
            mode_row(ui, mode, &mut ch);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut e, tr(k::fx::EASE)).changed() {
                    ch.base = true;
                }
            });
            if let FxInstance::CamMotion { ease, .. } = f {
                *ease = e;
            }
        }
        FxInstance::SlowedReverb { frames, .. } if on_video => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::FRAMES));
                egui::ComboBox::from_id_salt("slowedframes")
                    .selected_text(tr(match frames {
                        kt_model::FrameMode::Nearest => k::fx::FRAMES_NEAREST,
                        kt_model::FrameMode::Blend => k::fx::FRAMES_BLEND,
                    }))
                    .show_ui(ui, |ui| {
                        // Р.56 (п.448): «смешивание» законно и на ВЛОЖЕНИИ —
                        // режим наследуется содержимому, и пару смешивают
                        // его листья. Гасить тут больше нечего.
                        for (m, key) in [
                            (kt_model::FrameMode::Nearest, k::fx::FRAMES_NEAREST),
                            (kt_model::FrameMode::Blend, k::fx::FRAMES_BLEND),
                        ] {
                            ch.base |= ui.selectable_value(frames, m, tr(key)).changed();
                        }
                    });
            });
            ui.label(egui::RichText::new(tr(k::fx::RATE_LINKED)).weak().small());
        }
        FxInstance::Mirror { horizontal, vertical } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(horizontal, tr(k::fx::HORIZONTAL)).changed();
                ch.base |= ui.checkbox(vertical, tr(k::fx::VERTICAL)).changed();
            });
        }
        FxInstance::Greenscreen { key_r, key_g, key_b, .. } => {
            ui.horizontal(|ui| {
                let mut col = [*key_r as f32 / 255.0, *key_g as f32 / 255.0, *key_b as f32 / 255.0];
                ui.label(tr(k::fx::KEY_COLOR));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *key_r = (col[0] * 255.0) as u8;
                    *key_g = (col[1] * 255.0) as u8;
                    *key_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::EventGrid { center, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(center, tr(k::fx::CENTER)).changed();
            });
        }
        FxInstance::Edges { color_r, color_g, color_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*color_r as f32 / 255.0, *color_g as f32 / 255.0, *color_b as f32 / 255.0];
                ui.label(tr(k::fx::LINE_COLOR));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *color_r = (col[0] * 255.0) as u8;
                    *color_g = (col[1] * 255.0) as u8;
                    *color_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::Glow { tint_r, tint_g, tint_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*tint_r as f32 / 255.0, *tint_g as f32 / 255.0, *tint_b as f32 / 255.0];
                ui.label(tr(k::fx::TINT));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *tint_r = (col[0] * 255.0) as u8;
                    *tint_g = (col[1] * 255.0) as u8;
                    *tint_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::Duotone { shadow_r, shadow_g, shadow_b, high_r, high_g, high_b, .. } => {
            ui.horizontal(|ui| {
                let mut sh =
                    [*shadow_r as f32 / 255.0, *shadow_g as f32 / 255.0, *shadow_b as f32 / 255.0];
                ui.label(tr(k::fx::SHADOW_COLOR));
                if ui.color_edit_button_rgb(&mut sh).changed() {
                    *shadow_r = (sh[0] * 255.0) as u8;
                    *shadow_g = (sh[1] * 255.0) as u8;
                    *shadow_b = (sh[2] * 255.0) as u8;
                    ch.base = true;
                }
                let mut hi =
                    [*high_r as f32 / 255.0, *high_g as f32 / 255.0, *high_b as f32 / 255.0];
                ui.label(tr(k::fx::HIGH_COLOR));
                if ui.color_edit_button_rgb(&mut hi).changed() {
                    *high_r = (hi[0] * 255.0) as u8;
                    *high_g = (hi[1] * 255.0) as u8;
                    *high_b = (hi[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::DropShadow { tint_r, tint_g, tint_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*tint_r as f32 / 255.0, *tint_g as f32 / 255.0, *tint_b as f32 / 255.0];
                ui.label(tr(k::fx::TINT));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *tint_r = (col[0] * 255.0) as u8;
                    *tint_g = (col[1] * 255.0) as u8;
                    *tint_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::RadialBlur { spin, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(spin, tr(k::fx::SPIN)).changed();
            });
        }
        FxInstance::PitchShift { .. } => {
            // правило 19: диалог обязан сказать, ГДЕ менять то, чего в нём
            // нет — движок питча живёт в настройках проекта
            ui.label(tr(k::fx::PITCH_ENGINE_NOTE));
        }
        FxInstance::Noise { color, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(color, tr(k::fx::COLOR_NOISE)).changed();
            });
        }
        _ => {}
    }

    // --- график АЧХ эквалайзера (Р.48, п.347) ------------------------------
    // Рисуется ПЕРЕД сеткой: драг правит значения до того, как поля их
    // покажут, — и в одном кадре они уже синхронны. Поля при этом
    // остаются: график их дополняет, а не заменяет.
    if matches!(f, FxInstance::Eq { .. }) {
        ch |= crate::eq_graph::draw(ui, f, bind.as_deref_mut(), hover);
    }

    // --- числовые параметры: одна сетка на все варианты -------------------
    // связанная с кривыми (Р.38 пп.15-16): у параметра с живой кривой поле
    // правит ВЫБРАННЫЙ КЛЮЧ, а не невидимую базу
    // ВЕДУЩИЙ УЗЕЛ — своя форма (Р.64, раздел 5): группы, пары X/Y одной
    // строкой, единицы, ползунки и схема рамки. Кирпичи те же, правило
    // записи значения то же — меняется только раскладка.
    if f.is_lead_geometry() {
        ch |= crate::geomctl::transform_form(ui, f, bind, hover, pubc);
        return ch;
    }
    ch |= crate::widgets::params_grid_bound(ui, ("fxp", fx_display_name(f)), f, skip, bind, pubc);

    // --- хвост: камера Pan3d и кейфреймы CamMotion ------------------------
    match f {
        FxInstance::Pan3d { camera, .. } => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::CAMERA));
                let sel = match camera {
                    None => tr(k::fx::CAMERA_ACTIVE).to_string(),
                    Some(id) => cameras
                        .iter()
                        .find(|(cid, _)| *cid == id.0)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| format!("C{}", id.0)),
                };
                egui::ComboBox::from_id_salt("pan3dcam").selected_text(sel).show_ui(ui, |ui| {
                    ch.base |=
                        ui.selectable_value(camera, None, tr(k::fx::CAMERA_ACTIVE)).changed();
                    for (cid, name) in cameras {
                        ch.base |= ui
                            .selectable_value(camera, Some(kt_model::CameraId(*cid)), name.clone())
                            .changed();
                    }
                });
            });
        }
        FxInstance::CamMotion { ease: true, keyframes, .. } => {
            ui.weak(crate::tf!(k::fx::KEYFRAMES_N, keyframes.len()));
            ui.horizontal(|ui| {
                if ui.button(tr(k::fx::ADD_KF)).clicked() {
                    let t = keyframes.last().map(|k| (k.t + 0.25).min(1.0)).unwrap_or(0.0);
                    keyframes.push(kt_model::CamKeyframe { t, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 });
                    ch.base = true;
                }
                if !keyframes.is_empty() && ui.button(tr(k::fx::DROP_LAST)).clicked() {
                    keyframes.pop();
                    ch.base = true;
                }
            });
            // ключи Pan-камеры: пределы ОБЪЯВЛЕНЫ ОДНИМ местом рядом друг с
            // другом, а рисует их общий ряд (Р.66) — своих `DragValue` в
            // окнах не бывает
            let (t_row, x_row, y_row, z_row) = pan_key_rows();
            for (i, kf) in keyframes.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("#{i}"));
                    ch.base |= crate::widgets::plain_row(ui, &t_row, &mut kf.t).changed();
                    ch.base |= crate::widgets::pair_row(
                        ui,
                        "",
                        (&x_row, &mut kf.pan_x),
                        (&y_row, &mut kf.pan_y),
                    );
                    ch.base |= crate::widgets::plain_row(ui, &z_row, &mut kf.zoom).changed();
                });
            }
        }
        _ => {}
    }
    ch
}
