//! Экранные контролы геометрии над кадром превью (Р.64).
//!
//! Владелец (Р.63 п.6): «fx геометрии по прежнему очень скуден, сплошные
//! скучные поля ввода… КАК МИНИМУМ у нас должно быть редактирование прям в
//! превью удобное». Уточнение того же раунда — контролы В ДОПОЛНЕНИЕ к
//! полям, а не вместо них.
//!
//! Три правила, на которых здесь всё держится:
//!
//! 1. **Арифметика размещения ОДНА** — `kt_fx::geom::geometry_placement` и
//!    её обратная пара. Рамка рисуется тем же прямоугольником, по которому
//!    композит кладёт слой; жест возвращает значения теми же функциями,
//!    которыми рендер их читает. Второй формулы здесь нет ни одной.
//! 2. **Жест идёт СУЩЕСТВУЮЩЕЙ командой** `SetGeometry` через
//!    `apply_coalesced` с токеном, выданным на нажатии: живая картинка и
//!    один шаг истории на жест получаются сами (тот же приём, что у драга
//!    титра в превью и у линии уровня на таймлайне).
//! 3. **Активен ровно ОДИН набор** (трансформация ИЛИ кроп) — правило
//!    Resolve: два набора над одним кадром дерутся за ту же мышь.

use crate::app::KtApp;
use eframe::egui;
use kt_fx::conform::LeadCtx;
use kt_fx::geom::{
    Handle, Placement, anchor_move, crop_for_handle, geometry_placement, pos_for_drag,
    rot_for_point, zoom_for_handle,
};
use kt_model::{ClipId, ClipKind, Command, EventGeometry, FxInstance, HasParams};

/// Хват-зона ручки в экранных px (правило 1 чек-листа: в подпись высотой в
/// строку целиться не должно быть нужно).
pub const GRAB: f32 = 14.0;
/// Рисуемый радиус ручки.
const DOT: f32 = 4.0;

/// Набор контролов. Активен ровно один.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeomMode {
    Off,
    Transform,
    Crop,
}

impl GeomMode {
    pub fn from_u32(v: u32) -> GeomMode {
        match v {
            1 => GeomMode::Transform,
            2 => GeomMode::Crop,
            _ => GeomMode::Off,
        }
    }
    pub fn as_u32(self) -> u32 {
        match self {
            GeomMode::Off => 0,
            GeomMode::Transform => 1,
            GeomMode::Crop => 2,
        }
    }
    /// Следующий набор по кругу — один переключатель на все состояния.
    pub fn next(self) -> GeomMode {
        match self {
            GeomMode::Off => GeomMode::Transform,
            GeomMode::Transform => GeomMode::Crop,
            GeomMode::Crop => GeomMode::Off,
        }
    }
    pub fn handles(self) -> &'static [Handle] {
        match self {
            GeomMode::Off => &[],
            GeomMode::Transform => &[
                Handle::TopLeft,
                Handle::Top,
                Handle::TopRight,
                Handle::Right,
                Handle::BottomRight,
                Handle::Bottom,
                Handle::BottomLeft,
                Handle::Left,
                Handle::Rot,
                Handle::Anchor,
            ],
            GeomMode::Crop => {
                &[Handle::CropLeft, Handle::CropTop, Handle::CropRight, Handle::CropBottom]
            }
        }
    }
}

/// Идущий жест: что тянут, откуда и каким токеном истории.
///
/// Жест АБСОЛЮТНЫЙ: считается от точки нажатия и геометрии НА МОМЕНТ
/// нажатия, а не от покадровой дельты. Инкрементальный вариант дважды
/// применял дельту того кадра, в котором драг начался (поймано t760), и
/// вдобавок накапливал бы ошибку на каждом кадре.
#[derive(Debug, Clone, Copy)]
pub struct GeomDrag {
    pub clip: ClipId,
    pub handle: Option<Handle>,
    pub gesture: u64,
    /// Геометрия на момент нажатия — от неё считается ВЕСЬ жест.
    pub start: EventGeometry,
    /// Точка нажатия в координатах ХОЛСТА.
    pub press: (f64, f64),
}

/// Почему рамки нет — причина называется словом (правило 16).
pub enum NoFrame {
    NothingSelected,
    ManySelected,
    NotVideo,
    NotUnderCursor,
}

impl NoFrame {
    pub fn key(&self) -> &'static str {
        use crate::i18n::{k, tr};
        match self {
            NoFrame::NothingSelected => tr(k::geomctl::NO_SELECTION),
            NoFrame::ManySelected => tr(k::geomctl::MANY_SELECTED),
            NoFrame::NotVideo => tr(k::geomctl::NOT_VIDEO),
            NoFrame::NotUnderCursor => tr(k::geomctl::NOT_UNDER_CURSOR),
        }
    }
}

impl KtApp {
    pub(crate) fn geom_mode(&self) -> GeomMode {
        self.geom_ctl
    }

    /// Переключить набор по кругу. Удобство, не содержимое: `dirty_ui`.
    pub(crate) fn cycle_geom_mode(&mut self) {
        self.geom_ctl = self.geom_ctl.next();
        self.dirty_ui = true;
        self.note(crate::tf!(crate::i18n::k::geomctl::TOGGLE, mode_name(self.geom_ctl)));
    }

    /// Событие, у которого рисуется рамка, либо причина, почему её нет.
    pub(crate) fn geom_target(&self) -> Result<ClipId, NoFrame> {
        let sel: Vec<ClipId> = self.selected.iter().copied().collect();
        if sel.is_empty() {
            return Err(NoFrame::NothingSelected);
        }
        if sel.len() > 1 {
            return Err(NoFrame::ManySelected);
        }
        let id = sel[0];
        let Some(c) = self.model.clip(id) else { return Err(NoFrame::NothingSelected) };
        if c.kind != ClipKind::Video {
            return Err(NoFrame::NotVideo);
        }
        let t = self.player.transport.playhead();
        if t < c.start || t >= c.start + c.duration {
            return Err(NoFrame::NotUnderCursor);
        }
        Ok(id)
    }

    /// Размещение ведущего узла события на холсте превью.
    pub(crate) fn geom_placement(&self, clip: ClipId, canvas: (f64, f64)) -> Option<Placement> {
        let g = self.geom_effective(clip)?;
        self.geom_placement_of(&g, clip, canvas)
    }

    /// Позиция курсора В ДОМЕНЕ КРИВЫХ события — тик МАТЕРИАЛА (Р.37: ось
    /// линейна, скорость её не сжимает). Тем же правилом живёт поле окна FX.
    pub(crate) fn geom_curve_at(&self, clip: ClipId) -> kt_time::Tick {
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        let playhead = self.player.transport.playhead();
        match self.model.clip(clip) {
            Some(c) => kt_model::clip_material_at(
                c.source_in,
                c.start,
                c.duration,
                c.reversed,
                frame_step,
                playhead,
            ),
            None => playhead,
        }
    }

    /// Геометрия, КОТОРУЮ ВИДНО: база, поверх которой лежат значения живых
    /// кривых под курсором. Рамка обязана показывать то же, что рендер, а
    /// рендер сэмплирует `geometry_auto` как автоматизацию обычного слота.
    pub(crate) fn geom_effective(&self, clip: ClipId) -> Option<EventGeometry> {
        let c = self.model.clip(clip)?;
        let mut g = c.geometry;
        if c.geometry_auto.is_empty() {
            return Some(g);
        }
        let at = self.geom_curve_at(clip);
        let mut curves = c.geometry_auto.clone();
        let bind = crate::widgets::CurveBind { curves: &mut curves, sel: None, at };
        let names: Vec<String> = g.param_specs().iter().map(|s| s.name.to_string()).collect();
        for n in names {
            let v = bind.shown(&g, &n);
            g.set_param(&n, v);
        }
        Some(g)
    }

    /// То же для ПРОИЗВОЛЬНОЙ геометрии события (жест считает от снимка
    /// на момент нажатия).
    pub(crate) fn geom_placement_of(
        &self,
        g_of: &EventGeometry,
        clip: ClipId,
        canvas: (f64, f64),
    ) -> Option<Placement> {
        let c = self.model.clip(clip)?;
        let rule = g_of.rule.resolve(self.model.project().settings.conform);
        let view = self.player.source_view(c.media);
        let lead = LeadCtx::new(view, rule);
        // вход узла — вид источника (ведущий узел стоит первым в цепочке);
        // неизвестен — холст, правило вырождается в «растянуть»
        let in_size = view.unwrap_or(canvas);
        geometry_placement(&g_of.node(), Some(lead), in_size, canvas)
    }

    /// Контролы над кадром: рисунок и жесты. Зовётся из `draw_preview`
    /// ДО драга титра — у более мелкой и более специфичной цели приоритет.
    pub(crate) fn geometry_controls(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        img: egui::Rect,
        canvas: (u32, u32),
    ) {
        let mode = self.geom_mode();
        if mode == GeomMode::Off {
            return;
        }
        // при ИГРЕ контролов нет: они закрывают то, ради чего человек
        // смотрит в кадр (правило отрасли — FCP гасит всегда)
        if self.player.transport.is_playing() {
            return;
        }
        let canvas_f = (canvas.0.max(2) as f64, canvas.1.max(2) as f64);
        let clip = match self.geom_target() {
            Ok(id) => id,
            Err(why) => {
                // причина — только когда человек смотрит на кадр
                if ui.rect_contains_pointer(img) {
                    crate::widgets::hint_sink::put(why.key());
                }
                return;
            }
        };
        let Some(pl) = self.geom_placement(clip, canvas_f) else { return };
        let to_screen = |p: (f64, f64)| -> egui::Pos2 {
            egui::pos2(
                img.left() + (p.0 / canvas_f.0) as f32 * img.width(),
                img.top() + (p.1 / canvas_f.1) as f32 * img.height(),
            )
        };
        let to_canvas = |p: egui::Pos2| -> (f64, f64) {
            (
                ((p.x - img.left()) / img.width().max(1.0)) as f64 * canvas_f.0,
                ((p.y - img.top()) / img.height().max(1.0)) as f64 * canvas_f.1,
            )
        };

        // ---- РИСУНОК ---------------------------------------------------
        let accent = crate::theme::ACCENT;
        let cs = pl.corners().map(to_screen);
        painter.add(egui::Shape::closed_line(
            cs.to_vec(),
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(120, 190, 255, 200)),
        ));
        if mode == GeomMode::Transform {
            // плечо ручки поворота — линия от верхней стороны
            let top = to_screen(Handle::Top.point(&pl));
            let rot = to_screen(Handle::Rot.point(&pl));
            painter.line_segment(
                [top, rot],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(120, 190, 255, 140)),
            );
        }
        for h in mode.handles() {
            let p = to_screen(h.point(&pl));
            match h {
                Handle::Anchor => {
                    painter.circle_stroke(p, DOT + 2.0, egui::Stroke::new(1.5, accent));
                    painter.line_segment(
                        [p - egui::vec2(DOT + 4.0, 0.0), p + egui::vec2(DOT + 4.0, 0.0)],
                        egui::Stroke::new(1.0, accent),
                    );
                    painter.line_segment(
                        [p - egui::vec2(0.0, DOT + 4.0), p + egui::vec2(0.0, DOT + 4.0)],
                        egui::Stroke::new(1.0, accent),
                    );
                }
                Handle::Rot => {
                    painter.circle_stroke(p, DOT, egui::Stroke::new(1.5, accent));
                }
                _ => {
                    painter.rect_filled(
                        egui::Rect::from_center_size(p, egui::vec2(DOT * 2.0, DOT * 2.0)),
                        1.0,
                        accent,
                    );
                }
            }
            // прямоугольник ручки — живым сценариям (Р.27)
            if crate::widgets::field_probe::on() {
                crate::widgets::field_probe::note(
                    &format!("geom.{}", handle_tag(*h)),
                    egui::Rect::from_center_size(p, egui::vec2(GRAB, GRAB)),
                );
            }
        }
        if crate::widgets::field_probe::on() {
            crate::widgets::field_probe::note("geom.frame", egui::Rect::from_points(&cs));
        }

        // ---- ЖЕСТ ------------------------------------------------------
        // ДИАГНОСТИКА сцены живым сценариям и тестам: контрол дошёл до
        // разбора жеста (иначе «рамки нет» и «жест не поймали» выглядят
        // одинаково)
        self.geom_ctl_drawn = true;
        let id = ui.id().with(("geomctl", clip.0));
        // прямоугольник жеста накрывает и РУЧКИ: ручка поворота висит
        // НАД верхней стороной, и рамка из одних углов её не включает —
        // нажатие уходило бы в play/pause превью (поймано t760b)
        let mut pts: Vec<egui::Pos2> = cs.to_vec();
        for h in mode.handles() {
            pts.push(to_screen(h.point(&pl)));
        }
        let body = egui::Rect::from_points(&pts);
        let resp = ui.interact(body.expand(GRAB), id, egui::Sense::click_and_drag());
        let pointer = resp.interact_pointer_pos().or_else(|| ui.ctx().pointer_latest_pos());
        let alt = ui.input(|i| i.modifiers.alt);

        // ЖЕСТ КЛЮЧУЕТСЯ ТОЧКОЙ НАЖАТИЯ (`press_origin`), а не фактом
        // «драг идёт». Две причины, обе пойманы тестами:
        //
        // 1. РУЧКУ выбирать можно только по точке НАЖАТИЯ: к первому
        //    кадру, который до нас доходит, указатель уже ушёл за хват-зону,
        //    и жест ручки превращался в перенос тела (t760b);
        // 2. снимок геометрии берётся ОДИН раз на нажатие: egui сообщает
        //    stop/start посреди движения, и пересъёмка при живом
        //    `press_origin` засчитывала пройденное дважды (t760).
        let origin = ui.ctx().input(|i| i.pointer.press_origin()).or(pointer);
        if resp.dragged()
            && let Some(o) = origin
        {
            let oc = to_canvas(o);
            let same = self.geom_drag.is_some_and(|d| {
                d.clip == clip
                    && (d.press.0 - oc.0).abs() < 0.01
                    && (d.press.1 - oc.1).abs() < 0.01
            });
            if !same {
                self.gesture_counter += 1;
                let handle = nearest_handle(&pl, mode, o, &to_screen, alt);
                let start = self.geom_effective(clip).unwrap_or_default();
                self.geom_drag = Some(GeomDrag {
                    clip,
                    handle,
                    gesture: self.gesture_counter,
                    start,
                    press: oc,
                });
            }
        }
        if resp.hovered() {
            let over = pointer.and_then(|p| nearest_handle(&pl, mode, p, &to_screen, alt));
            ui.ctx().set_cursor_icon(match over {
                Some(Handle::Rot) => egui::CursorIcon::Alias,
                Some(Handle::Anchor) => egui::CursorIcon::Crosshair,
                Some(_) => egui::CursorIcon::ResizeNwSe,
                None => egui::CursorIcon::Grab,
            });
            crate::widgets::hint_sink::put(hint_for(over));
        }
        if resp.drag_stopped() {
            self.geom_drag = None;
        }
        if !resp.dragged() {
            return;
        }
        let Some(drag) = self.geom_drag.filter(|d| d.clip == clip) else { return };
        let Some(ptr) = pointer else { return };
        let Some(g0) = self.geom_effective(clip) else { return };
        // ВСЁ считается от НАЧАЛА жеста: размещение на момент нажатия и
        // полное смещение указателя. Покадровая дельта дважды применяла
        // движение того кадра, в котором драг начался (t760).
        let start = drag.start;
        let Some(pl0) = self.geom_placement_of(&start, clip, canvas_f) else { return };
        let mut g = start;
        let p = to_canvas(ptr);
        let dc = (p.0 - drag.press.0, p.1 - drag.press.1);
        let shift = ui.input(|i| i.modifiers.shift);
        match drag.handle {
            None => {
                let pos = pos_for_drag((start.pos_x, start.pos_y), canvas_f, dc);
                g.pos_x = pos.0;
                g.pos_y = pos.1;
            }
            Some(Handle::Rot) => g.rot = rot_for_point(&pl0, p, shift.then_some(15.0)),
            Some(Handle::Anchor) => {
                let (a, pos) = anchor_move(&pl0, p, (start.pos_x, start.pos_y), canvas_f);
                g.anchor_x = a.0;
                g.anchor_y = a.1;
                g.pos_x = pos.0;
                g.pos_y = pos.1;
            }
            Some(h) if matches!(h, Handle::CropLeft | Handle::CropTop | Handle::CropRight | Handle::CropBottom) => {
                if let Some(cr) = crop_for_handle(
                    &pl0,
                    h,
                    p,
                    [start.crop_l, start.crop_t, start.crop_r, start.crop_b],
                )
                {
                    g.crop_l = cr[0];
                    g.crop_t = cr[1];
                    g.crop_r = cr[2];
                    g.crop_b = cr[3];
                }
            }
            Some(h) => {
                if let Some((z, pos)) =
                    zoom_for_handle(&pl0, h, p, start.zoom, (start.pos_x, start.pos_y), canvas_f)
                {
                    g.zoom = z;
                    g.pos_x = pos.0;
                    g.pos_y = pos.1;
                }
            }
        }
        if g != g0 {
            self.set_geometry_coalesced(clip, g, drag.gesture);
        }
    }

    /// Записать геометрию события тем же путём, что окно FX: команда одна,
    /// токен жеста один — значит и шаг истории один (правило 17).
    pub(crate) fn set_geometry_coalesced(&mut self, clip: ClipId, g: EventGeometry, gesture: u64) {
        let Some(c) = self.model.clip(clip) else { return };
        let pos = c.geometry_pos;
        let mut base = c.geometry;
        let mut auto = c.geometry_auto.clone();
        let at = self.geom_curve_at(clip);
        // Параметр с ЖИВОЙ КРИВОЙ правится КЛЮЧОМ по курсору — тем же
        // правилом (`CurveBind`), что поле окна FX. Второго правила «куда
        // писать значение» в программе быть не должно (правило 17): иначе
        // жест стирал бы анимацию, показывая при этом её же значение.
        {
            let mut bind = crate::widgets::CurveBind { curves: &mut auto, sel: None, at };
            let names: Vec<String> = base.param_specs().iter().map(|s| s.name.to_string()).collect();
            for n in names {
                let Some(v) = g.get_param(&n) else { continue };
                if bind.shown(&base, &n) == v {
                    continue; // ключей «на всякий случай» жест не сеет
                }
                if bind.has_curve(&base, &n) {
                    bind.apply(&mut base, &n, v);
                } else {
                    base.set_param(&n, v);
                }
            }
        }
        base.rule = g.rule; // правило приведения — не число, кривой у него нет
        for c in auto.iter_mut() {
            c.keys.sort_by_key(|k| k.t.0); // модель требует порядок ключей
        }
        self.apply_coalesced(Command::SetGeometry { id: clip, geometry: base, pos, auto }, gesture);
    }
}

/// Ближайшая к указателю ручка активного набора — или `None` (тело).
///
/// Приоритет ближайшей, а не «первой попавшейся»: у мелкой рамки хват-зоны
/// перекрываются, и правило «кто ближе» — единственное однозначное.
pub fn nearest_handle(
    pl: &Placement,
    mode: GeomMode,
    p: egui::Pos2,
    to_screen: &dyn Fn((f64, f64)) -> egui::Pos2,
    alt: bool,
) -> Option<Handle> {
    let mut best: Option<(Handle, f32)> = None;
    for h in mode.handles() {
        // ЯКОРЬ — только с Alt: по умолчанию он стоит в ЦЕНТРЕ рамки и
        // иначе забирал бы самое естественное место захвата картинки.
        // У After Effects для него ровно поэтому отдельный инструмент
        // (Pan Behind, `Y`); у нас инструментов нет — есть модификатор,
        // и о нём говорит подсказка строки состояния (правило 10).
        if *h == Handle::Anchor && !alt {
            continue;
        }
        let d = to_screen(h.point(pl)).distance(p);
        if d <= GRAB && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((*h, d));
        }
    }
    best.map(|(h, _)| h)
}

fn handle_tag(h: Handle) -> &'static str {
    match h {
        Handle::TopLeft => "tl",
        Handle::Top => "t",
        Handle::TopRight => "tr",
        Handle::Right => "r",
        Handle::BottomRight => "br",
        Handle::Bottom => "b",
        Handle::BottomLeft => "bl",
        Handle::Left => "l",
        Handle::Rot => "rot",
        Handle::Anchor => "anchor",
        Handle::CropLeft => "crop_l",
        Handle::CropTop => "crop_t",
        Handle::CropRight => "crop_r",
        Handle::CropBottom => "crop_b",
    }
}

/// Подсказка — в строку состояния (правило 10: без всплывающих).
fn hint_for(h: Option<Handle>) -> &'static str {
    use crate::i18n::{k, tr};
    match h {
        None => tr(k::geomctl::HINT_BODY),
        Some(Handle::Rot) => tr(k::geomctl::HINT_ROT),
        Some(Handle::Anchor) => tr(k::geomctl::HINT_ANCHOR),
        Some(
            Handle::CropLeft | Handle::CropTop | Handle::CropRight | Handle::CropBottom,
        ) => tr(k::geomctl::HINT_CROP),
        Some(_) => tr(k::geomctl::HINT_SCALE),
    }
}

/// Человекочитаемое имя набора — для строки состояния и меню.
pub fn mode_name(m: GeomMode) -> &'static str {
    use crate::i18n::{k, tr};
    match m {
        GeomMode::Off => tr(k::geomctl::MODE_OFF),
        GeomMode::Transform => tr(k::geomctl::MODE_TRANSFORM),
        GeomMode::Crop => tr(k::geomctl::MODE_CROP),
    }
}

// ---- ФОРМА ПОЛЕЙ «Трансформация» (Р.64, раздел 5) -----------------------
//
// Претензия владельца дословно (Р.63 п.6): «fx геометрии по прежнему очень
// скуден, сплошные скучные поля ввода, даже в Vegas и то лучше сделано».
// Форма собрана из СУЩЕСТВУЮЩИХ кирпичей (`param_row_bound_ex`, дверь
// публикации у подписи, `CurveBind`): новых правил «куда писать значение»
// не заводится, добавляются группировка, единицы, ползунки и схема.

/// Доля показывается ПРОЦЕНТОМ (п.415): пересчёт живёт в `param_value`, в
/// модели и в файле значение остаётся долей.
fn shown_spec(s: &kt_model::ParamSpec) -> kt_model::ParamSpec {
    let mut s = s.clone();
    if matches!(
        &*s.name,
        "crop_l" | "crop_t" | "crop_r" | "crop_b" | "pos_x" | "pos_y" | "anchor_x" | "anchor_y"
    ) {
        s.unit = kt_model::ParamUnit::Percent;
    }
    s
}

/// Единица, которой параметр ПОКАЗЫВАЕТСЯ в форме (для теста t759: единица
/// живёт в паспорте, а не в тексте подписи).
#[allow(dead_code)] // спрашивает тест t759: единица живёт в паспорте
pub(crate) fn shown_spec_unit(name: &str) -> Option<kt_model::ParamUnit> {
    let g = EventGeometry::default().node();
    let s = g.param_specs().iter().find(|s| &*s.name == name)?.clone();
    Some(shown_spec(&s).unit)
}

/// Строка формы: пара полей на одну строку (X и Y рядом, как в Pr/AE).
fn geom_row(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    specs: &[kt_model::ParamSpec],
    names: &[&str],
    slider: Option<f32>,
    bind: &mut Option<&mut crate::widgets::CurveBind<'_>>,
    pubc: Option<&crate::widgets::PublishCtx>,
) -> crate::widgets::ParamsChanged {
    let mut ch = crate::widgets::ParamsChanged::default();
    ui.horizontal(|ui| {
        for n in names {
            let Some(s) = specs.iter().find(|s| &*s.name == *n) else { continue };
            let s = shown_spec(s);
            ch |= crate::widgets::param_row_bound_ex(
                ui,
                f,
                &s,
                bind.as_deref_mut(),
                pubc,
                slider,
            );
        }
    });
    ch
}

/// Прямоугольник схемы и точка якоря — ТОЙ ЖЕ функцией размещения, что у
/// рендера и у экранных контролов, посчитанной на холсте размером со схему.
/// Поэтому кроп, зум, поворот и якорь на ней — не рисунок «по мотивам», а те
/// самые числа. Отдельно от рисования — чтобы это утверждал тест, а не глаз.
pub(crate) type SchemaShape = ([(f64, f64); 4], (f64, f64));

pub(crate) fn schema_quad(f: &FxInstance, size: egui::Vec2) -> Option<SchemaShape> {
    let canvas = (size.x.max(4.0) as f64, size.y.max(4.0) as f64);
    let g = EventGeometry::default().with_node(f);
    // вид источника здесь неизвестен, поэтому вход равен холсту: схема
    // показывает рамку ОТНОСИТЕЛЬНО кадра, а правило приведения на таком
    // входе вырождается в тождество для любого своего значения
    let lead = LeadCtx::new(None, kt_fx::conform::ConformRule::Fit);
    let pl = geometry_placement(&g.node(), Some(lead), canvas, canvas)?;
    Some((pl.corners(), pl.anchor))
}

fn frame_schema(ui: &mut egui::Ui, f: &FxInstance, size: egui::Vec2) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    let dim = egui::Color32::from_rgb(90, 92, 100);
    p.rect_filled(rect, 2.0, egui::Color32::from_rgb(18, 18, 22));
    p.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, dim), egui::StrokeKind::Inside);
    let Some((quad, anchor)) = schema_quad(f, rect.size()) else { return resp };
    let to = |q: (f64, f64)| {
        egui::pos2(rect.left() + q.0 as f32, rect.top() + q.1 as f32)
    };
    let cs: Vec<egui::Pos2> = quad.iter().map(|c| to(*c)).collect();
    p.add(egui::Shape::closed_line(cs, egui::Stroke::new(1.5, crate::theme::ACCENT)));
    let a = to(anchor);
    p.circle_stroke(a, 3.0, egui::Stroke::new(1.5, crate::theme::ACCENT));
    p.line_segment([a - egui::vec2(5.0, 0.0), a + egui::vec2(5.0, 0.0)], (1.0, dim));
    p.line_segment([a - egui::vec2(0.0, 5.0), a + egui::vec2(0.0, 5.0)], (1.0, dim));
    resp
}

/// Раздел «Трансформация» у ведущего узла: группы, пары X/Y одной строкой,
/// ползунки у зума и поворота, схема рамки и сброс одним шагом истории.
pub(crate) fn transform_form(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    mut bind: Option<&mut crate::widgets::CurveBind<'_>>,
    hover: &mut Option<String>,
    pubc: Option<&crate::widgets::PublishCtx>,
) -> crate::widgets::ParamsChanged {
    use crate::i18n::{k, tr};
    let mut ch = crate::widgets::ParamsChanged::default();
    let specs: Vec<kt_model::ParamSpec> = f.param_specs().to_vec();
    // свёртка групп — состояние ОКНА (память egui), не проекта
    let group = |ui: &mut egui::Ui, id: &str, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
        egui::CollapsingHeader::new(title)
            .id_salt(("geoform", id))
            .default_open(true)
            .show(ui, |ui| body(ui));
    };

    group(ui, "frame", tr(k::geomctl::GROUP_FRAME), &mut |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ch |= geom_row(ui, f, &specs, &["zoom"], Some(90.0), &mut bind, pubc);
                ch |= geom_row(ui, f, &specs, &["rot"], Some(90.0), &mut bind, pubc);
            });
            let r = frame_schema(ui, f, egui::vec2(116.0, 66.0));
            crate::widgets::field_probe::note("geo.schema", r.rect);
            crate::app::hint_into(hover, r, tr(k::geomctl::SCHEMA_HINT));
        });
    });
    group(ui, "pos", tr(k::geomctl::GROUP_POS), &mut |ui| {
        ch |= geom_row(ui, f, &specs, &["pos_x", "pos_y"], None, &mut bind, pubc);
        ch |= geom_row(ui, f, &specs, &["anchor_x", "anchor_y"], None, &mut bind, pubc);
    });
    group(ui, "crop", tr(k::geomctl::GROUP_CROP), &mut |ui| {
        ch |= geom_row(ui, f, &specs, &["crop_l", "crop_r"], None, &mut bind, pubc);
        ch |= geom_row(ui, f, &specs, &["crop_t", "crop_b"], None, &mut bind, pubc);
    });

    let r = ui.button(tr(k::geomctl::RESET));
    crate::widgets::field_probe::note("geo.reset", r.rect);
    let reset = r.clicked();
    crate::app::hint_into(hover, r, tr(k::geomctl::RESET_HINT));
    if reset {
        // сброс идёт ТЕМ ЖЕ правилом, что поле: у анимированного параметра
        // это ключ по курсору, а не стёртая кривая (правило 20 — ничего не
        // отрубается само)
        for s in &specs {
            if let Some(b) = bind.as_deref_mut()
                && b.has_curve(f, &s.name)
            {
                b.apply(f, &s.name, s.default);
                ch.curve = true;
                continue;
            }
            f.set_param(&s.name, s.default);
            ch.base = true;
        }
    }
    ch
}
