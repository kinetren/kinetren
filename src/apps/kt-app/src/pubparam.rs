//! Публичные параметры композиции: авторская сторона и потребительская
//! (Р.59, стадия 3, пункт 5 плана).
//!
//! Претензия владельца дословно: «кто-то создал проект-шаблон для
//! новостного выпуска… очевидно не будут же люди каждый раз лезть в
//! композицию и менять там текст логотипы». Отсюда ровно две стороны и
//! два окна:
//!
//! * **АВТОР** объявляет ручки. Дверь одна — правый клик по ПОДПИСИ поля
//!   внутри ленты (`widgets::PublishCtx`), а окно «Публичные параметры»
//!   держит уже объявленное: имя, порядок, сужение пределов, снятие.
//! * **ПОТРЕБИТЕЛЬ** заполняет объявленное. Окно «Параметры композиции»
//!   у экземпляра — самое важное окно этого раунда: оно и есть ответ на
//!   «не лезть внутрь».
//!
//! Три решения, которые легко потерять:
//!
//! 1. **Имя ручки — ТЕКСТ АВТОРА** (решение владельца). Оно рисуется как
//!    есть, мимо `tr()`, как имя события или ленты: это содержимое
//!    документа, а не текст программы (t131/t684).
//! 2. **Значение шаблона не хранится** — окно читает его у цели живьём
//!    (`Project::published_value`) и показывает рядом со строкой. Строка
//!    без переопределения рисуется приглушённо: видно, что экземпляр от
//!    ленты пока ничем не отличается.
//! 3. **Окна правят ТЕКУЩУЮ ленту.** Команды публикации ленту не
//!    называют (как `AddClip` и остальные шестьдесят рук), поэтому
//!    авторское окно открывается ВНУТРИ ленты, и вход — часть открытия:
//!    то же тело, что у глагола «войти» (правило 17).

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{
    ClipId, Command, MediaId, Override, ParamAddr, ParamType, ParamValue, PubId, PubRange,
    PublishedParam,
};

/// Диалог имени публикуемой ручки. Форма — окна имени ленты (t345):
/// живой предпросмотр значения цели, Enter/Esc, фокус отдаётся при
/// закрытии (правило 14).
pub struct PublishDialog {
    pub seq: MediaId,
    pub addr: ParamAddr,
    pub ty: ParamType,
    /// Умолчание — ПОДПИСЬ поля на языке интерфейса; после подтверждения
    /// это уже текст автора и в каталог не возвращается никогда.
    pub name: String,
    pub focus: bool,
}

/// Окно «Публичные параметры» ленты (сторона АВТОРА).
pub struct PublishedWin {
    pub seq: MediaId,
    /// Вся сессия окна — ОДИН шаг истории (тот же приём, что у окна FX).
    pub gesture: u64,
    /// Набор на момент открытия: «Отмена» возвращает его тем же токеном.
    pub backup: Vec<PublishedParam>,
    /// Подтверждение снятия: ручка и сколько переопределений умрёт.
    pub confirm: Option<(PubId, usize)>,
    pub open: bool,
}

/// Окно «Параметры композиции» экземпляра (сторона ПОТРЕБИТЕЛЯ).
pub struct InstParamsWin {
    pub clip: ClipId,
    pub seq: MediaId,
    pub gesture: u64,
    pub backup: Vec<Override>,
    /// Строка запроса пикера медиа — своя на окно.
    pub query: String,
    /// Состояние виджета автоматизации: активная кривая, выбранный ключ,
    /// вид графика. Своё на окно — как у слота эффекта и у генератора
    /// (Р.60): до этого раунда кривые переопределений вообще нечем было
    /// показать, и окно молча писало ключи по курсору.
    pub view: crate::automation::AutoView,
    pub open: bool,
}

/// Имя кривой ручки В ПРЕДЕЛАХ ОКНА.
///
/// Виджет автоматизации рисует кривые ОДНОГО носителя и различает их по
/// `ParamCurve::param`. У переопределений имя цели (`radius`, `text`) не
/// годится: две ручки ленты вправе целиться в одноимённые параметры
/// разных событий, и в окне они слиплись бы в одну линию. Поэтому внутри
/// окна кривая зовётся по НОМЕРУ РУЧКИ, а при записи в модель имя
/// возвращается целевое — то самое, которое ждёт `apply_overrides`.
fn pub_key(id: PubId) -> String {
    format!("pub{}", id.0)
}

/// Человекочитаемое значение ручки — одной строкой рядом со строкой.
/// Своей формы у него быть не должно: это ТОТ ЖЕ текст, что в поле.
fn value_text(v: &ParamValue, media_name: &dyn Fn(MediaId) -> String) -> String {
    match v {
        ParamValue::Num(x) => {
            if (x - x.round()).abs() < 1e-9 {
                format!("{}", x.round() as i64)
            } else {
                format!("{x:.3}")
            }
        }
        ParamValue::Text(s) => s.clone(),
        ParamValue::Color([r, g, b, a]) => format!("{r:02x}{g:02x}{b:02x}{a:02x}"),
        ParamValue::Flag(b) => (if *b { "1" } else { "0" }).to_string(),
        ParamValue::Choice(s) => s.clone(),
        ParamValue::Media(m) => media_name(*m),
    }
}

impl KtApp {
    /// Объявленные ручки ленты — из ДОКУМЕНТА, а не из рабочего вида:
    /// стоя внутри ленты, её запись в `project.sequences` держит
    /// содержимое корня.
    pub fn published_of(&self, seq: MediaId) -> Vec<PublishedParam> {
        self.model.document().sequence(seq).map(|s| s.published.clone()).unwrap_or_default()
    }

    /// Сколько переопределений этой ручки живёт ВО ВСЁМ документе.
    ///
    /// Считается по всему дереву, а не по видимому виду: снятие ручки
    /// убивает их везде одним шагом истории (`reconcile_published`), и
    /// обещать надо ровно это число.
    pub fn override_uses(&self, seq: MediaId, id: PubId) -> usize {
        let doc = self.model.document();
        let hit =
            |c: &kt_model::ClipEvent| c.media == seq && c.overrides.iter().any(|o| o.pub_id == id);
        doc.clips.iter().filter(|c| hit(c)).count()
            + doc.sequences.iter().flat_map(|s| s.clips.iter()).filter(|c| hit(c)).count()
    }

    /// Лента-ВЛАДЕЛЕЦ события в рабочем виде (Р.59, стадия 3).
    ///
    /// Публикация адресует цель по `ClipId`, а адрес обязан лежать в той
    /// ленте, ЧЬЁ это содержимое. Событие раскрытой полосы физически
    /// стоит в рабочем виде рядом с корневыми, но принадлежит ленте
    /// полосы: опубликуй мы его в текущей, `reconcile_published` снял бы
    /// ручку тем же батчем, которым её завели, — молча и без следа.
    /// `None` — корень: у него публикаций не бывает (он никому не
    /// экземпляр), и пункта меню там нет вовсе (правило 9).
    pub fn owner_reel_of(&self, doc: &kt_model::Project, id: ClipId) -> Option<MediaId> {
        for b in self.model.bands() {
            if doc
                .sequence(b.seq)
                .is_some_and(|s| s.clips.iter().any(|c| kt_model::same_clip(c.id, id)))
            {
                return Some(b.seq);
            }
        }
        self.model.editing()
    }

    /// Готовый контекст публикации для носителя свойств.
    ///
    /// Документ приходит СНАРУЖИ и клонируется один раз на отрисовку
    /// окна, а не на строку: `document()` — клон всего проекта, и звать
    /// его на каждый параметр значило бы аллоцировать в пути отрисовки
    /// (инвариант 12).
    pub fn publish_ctx(
        &self,
        doc: &kt_model::Project,
        clip: ClipId,
        owner: kt_model::ParamOwner,
    ) -> crate::widgets::PublishCtx {
        let seq = self.owner_reel_of(doc, clip);
        let published = seq
            .and_then(|s| doc.sequence(s))
            .map(|s| {
                s.published
                    .iter()
                    .filter(|p| {
                        p.target.owner == owner
                            && (matches!(
                                owner,
                                kt_model::ParamOwner::Transition { .. }
                                    | kt_model::ParamOwner::Camera { .. }
                            ) || kt_model::same_clip(p.target.clip, clip))
                    })
                    .map(|p| (p.target.param.clone(), p.id))
                    .collect()
            })
            .unwrap_or_default();
        crate::widgets::PublishCtx {
            seq,
            clip,
            owner,
            published,
            explain_outside: !doc.sequences.is_empty(),
        }
    }

    /// Свободный номер ручки в этой ленте. Номера живут внутри ленты
    /// (`Nested` адресует ручку СВОЕЙ ленты), поэтому и максимум берётся
    /// по ней.
    fn next_pub_id(&self, seq: MediaId) -> PubId {
        PubId(self.published_of(seq).iter().map(|p| p.id.0).max().unwrap_or(0) + 1)
    }

    /// Сколько ручек экземпляр переопределил и сколько их всего — текст
    /// подсказки под булавкой (правило 13: место уже занято подписью).
    ///
    /// Спрашивает РАБОЧИЙ ВИД, а не документ, и это не лень: зовётся из
    /// пути наведения, то есть на каждый кадр, а `document()` — клон
    /// всего проекта (инвариант 12, «ни аллокаций в draw-путях на
    /// кадр»). Ответ при этом верный: стоя внутри ленты X, её запись в
    /// `project.sequences` держит набор КОРНЯ — но экземпляра X внутри
    /// самой X не бывает по циклам, так что у настоящего экземпляра
    /// рабочий вид отвечает тем же, чем документ.
    pub fn instance_override_counts(&self, c: &kt_model::ClipEvent) -> (usize, usize) {
        let Some(all) = self.model.project().sequence(c.media).map(|s| &s.published) else {
            return (0, 0);
        };
        let n = c.overrides.iter().filter(|o| all.iter().any(|p| p.id == o.pub_id)).count();
        (n, all.len())
    }

    /// Забрать заявку подписи поля и исполнить её (конец кадра).
    pub(crate) fn drain_publish_req(&mut self) {
        let Some(req) = crate::widgets::publish_sink::take() else { return };
        match req {
            crate::widgets::PublishReq::Publish { seq, addr, ty, label } => {
                self.publish_dialog =
                    Some(PublishDialog { seq, addr, ty, name: label, focus: true });
            }
            crate::widgets::PublishReq::Unpublish { seq, id } => {
                // Снятие ИЗ ПОДПИСИ — там же, где публикация: путь один
                // и тот же (правило 17). Спрашивать подтверждение здесь
                // тоже обязательно — переопределения умрут во всех местах.
                let n = self.override_uses(seq, id);
                if n == 0 {
                    self.apply(Command::UnpublishParam { id });
                } else {
                    self.open_published_win(seq);
                    if let Some(w) = self.published_win.as_mut() {
                        w.confirm = Some((id, n));
                    }
                }
            }
        }
    }

    /// Встать ВНУТРЬ ленты ради авторской работы. Тело входа — то же, что
    /// у глагола «войти» (правило 17): второго способа оказаться внутри
    /// заводить нельзя. `false` — войти не через что, и об этом сказано.
    fn enter_reel_for_authoring(&mut self, seq: MediaId) -> bool {
        match self.model.project().clips.iter().find(|c| c.media == seq).map(|c| c.id) {
            Some(inst) => self.enter_nest(inst),
            // ленты нет на монтаже: войти не через что, и молчать об
            // этом нельзя (правило 16)
            None => {
                self.warn(tr(k::msg::NEST_NOT_PLACED));
                return false;
            }
        }
        self.model.editing() == Some(seq)
    }

    /// Открыть окно «Публичные параметры» ленты.
    ///
    /// Вход внутрь — ЧАСТЬ открытия: команды публикации правят ту ленту,
    /// в которой стоишь, и окно, правящее не ту, было бы ловушкой хуже
    /// отсутствующего окна. Тело входа — то же, что у глагола «войти»
    /// (правило 17).
    pub fn open_published_win(&mut self, seq: MediaId) {
        if self.model.editing() != Some(seq) && !self.enter_reel_for_authoring(seq) {
            return;
        }
        if self.published_win.as_ref().is_some_and(|w| w.seq == seq) {
            return;
        }
        self.gesture_counter += 1;
        self.published_win = Some(PublishedWin {
            seq,
            gesture: self.gesture_counter,
            backup: self.published_of(seq),
            confirm: None,
            open: true,
        });
    }

    /// Открыть окно «Параметры композиции» экземпляра.
    ///
    /// Лента без объявленных ручек окна не открывает, и это НЕ молчание:
    /// строка состояния говорит, что публиковать надо внутри (правило 16).
    pub fn open_inst_params(&mut self, clip: ClipId) {
        let Some(c) = self.model.clip(clip).cloned() else { return };
        if self.model.document().sequence(c.media).is_none() {
            return;
        }
        if self.published_of(c.media).is_empty() {
            self.warn(tr(k::pubp::INST_NONE));
            return;
        }
        if self.inst_params.iter().any(|w| w.clip == clip) {
            return;
        }
        self.gesture_counter += 1;
        self.inst_params.push(InstParamsWin {
            clip,
            seq: c.media,
            gesture: self.gesture_counter,
            backup: c.overrides.clone(),
            query: String::new(),
            view: crate::automation::AutoView::default(),
            open: true,
        });
    }

    /// Диалог имени публикуемой ручки.
    pub(crate) fn draw_publish_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.publish_dialog.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = true;
        // ЖИВОЙ предпросмотр: что именно уедет наружу под этим именем
        let doc = self.model.document();
        let media: Vec<(MediaId, String)> =
            doc.media.iter().map(|m| (m.id, m.path.clone())).collect();
        let shown = doc
            .addr_value(Some(d.seq), &d.addr)
            .map(|v| value_text(&v, &|m| media_name(&media, m)))
            .unwrap_or_else(|| tr(k::pubp::INST_TARGET_GONE).to_string());
        egui::Window::new(tr(k::pubp::DIALOG_TITLE))
            .id(egui::Id::new("pub_name"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::pubp::DIALOG_NAME));
                    let resp = ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(220.0));
                    crate::widgets::field_probe::note("pub.dialog.name", resp.rect);
                    if d.focus {
                        resp.request_focus();
                        d.focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                });
                ui.label(egui::RichText::new(crate::tf!(k::pubp::TEMPLATE, &shown)).weak().small());
                ui.horizontal(|ui| {
                    let r = ui.button(tr(k::pubp::DIALOG_DO));
                    crate::widgets::field_probe::note("pub.dialog.ok", r.rect);
                    if r.clicked() {
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
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("pub_name")));
            // Команда правит ТУ ленту, в которой стоишь. Публикация из
            // раскрытой полосы адресует содержимое ЕЁ ленты, поэтому
            // подтверждение заодно и входит туда: иначе ручка легла бы в
            // соседнюю ленту и умерла бы тем же батчем.
            if self.model.editing() != Some(d.seq) && !self.enter_reel_for_authoring(d.seq) {
                return;
            }
            let param = PublishedParam {
                id: self.next_pub_id(d.seq),
                name: d.name.trim().to_string(),
                target: d.addr.clone(),
                ty: d.ty.clone(),
                range: PubRange::default(),
            };
            self.apply(Command::PublishParam { param: Box::new(param) });
            return;
        }
        if cancel || !open {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("pub_name")));
            return;
        }
        self.publish_dialog = Some(d);
    }

    /// Окно «Публичные параметры» ленты.
    pub(crate) fn draw_published_win(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.published_win.take() else { return };
        // лента исчезла под undo — окну не о чем говорить
        if self.model.document().sequence(w.seq).is_none() {
            return;
        }
        let mut list = self.published_of(w.seq);
        let mut cmds: Vec<Command> = Vec::new();
        let mut cancel = false;
        let mut confirm_now: Option<(PubId, usize)> = None;
        let mut do_unpublish: Option<PubId> = None;
        let mut order: Option<Vec<PubId>> = None;
        let mut done = false;
        let title = format!(
            "{} — {}",
            tr(k::pubp::LIST_TITLE),
            self.model.document().sequence(w.seq).map(|s| s.name.clone()).unwrap_or_default()
        );
        // Паспорт цели — единственный источник пределов (Р.60). Снимаем
        // его ДО окна: `self` внутри замыкания окна уже занят. Ряд идёт
        // ПАРАЛЛЕЛЬНО `list` — порядок строк меняется командой, то есть
        // со следующего кадра, и разъехаться они внутри кадра не могут.
        let specs: Vec<Option<kt_model::ParamSpec>> = {
            let doc = self.model.document();
            list.iter().map(|p| doc.addr_spec(Some(w.seq), &p.target)).collect()
        };
        let mut open = w.open;
        egui::Window::new(title)
            .id(egui::Id::new(("published_win", w.seq.0)))
            .open(&mut open)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                if list.is_empty() {
                    ui.label(tr(k::pubp::LIST_EMPTY));
                    return;
                }
                let n = list.len();
                for i in 0..n {
                    let id = list[i].id;
                    ui.horizontal(|ui| {
                        // порядок строк — часть авторского интерфейса
                        // (Resolve 21 завёл его отдельно, F-TL-012)
                        let up =
                            ui.add_enabled(i > 0, egui::Button::new(crate::icons::LANE_UP).small());
                        crate::widgets::hint(&up, tr(k::pubp::LIST_HINT_ORDER));
                        let down = ui.add_enabled(
                            i + 1 < n,
                            egui::Button::new(crate::icons::LANE_DOWN).small(),
                        );
                        if up.clicked() || down.clicked() {
                            let mut o: Vec<PubId> = list.iter().map(|p| p.id).collect();
                            let j = if up.clicked() { i - 1 } else { i + 1 };
                            o.swap(i, j);
                            order = Some(o);
                        }
                        // ИМЯ — текст автора: рисуется как есть, мимо
                        // каталога (t684)
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut list[i].name).desired_width(180.0),
                        );
                        crate::widgets::field_probe::note(
                            &format!("pub.row.name.{}", id.0),
                            r.rect,
                        );
                        if r.changed() {
                            cmds.push(Command::SetPublished { param: Box::new(list[i].clone()) });
                        }
                        // сужение пределов — только у числа: у остальных
                        // типов «между вариантами» ничего нет, и мёртвое
                        // поле было бы нарушением правила 9.
                        //
                        // Умолчание — ПАСПОРТ ЦЕЛИ (Р.60): пустое сужение
                        // рисовалось нулём («значения min max всегда в 0
                        // проставляются по умолчанию»), хотя у цели свои
                        // пределы есть. Копии паспорта в публикации нет —
                        // она разошлась бы с целью при первой правке
                        // внутри ленты, как разошлась бы копия значения
                        // шаблона.
                        if list[i].ty == ParamType::Num
                            && let Some(spec) = specs.get(i).cloned().flatten()
                        {
                            let mut lo = list[i].range.min.unwrap_or(spec.min);
                            let mut hi = list[i].range.max.unwrap_or(spec.max);
                            let mut edited = false;
                            let step = if spec.step > 0.0 { spec.step } else { 0.05 };
                            for (label, v, is_min) in [
                                (tr(k::pubp::RANGE_MIN), &mut lo, true),
                                (tr(k::pubp::RANGE_MAX), &mut hi, false),
                            ] {
                                ui.label(label);
                                let rr = ui.add(
                                    egui::DragValue::new(v)
                                        .range(spec.min..=spec.max)
                                        .speed(step),
                                );
                                crate::widgets::hint(&rr, tr(k::pubp::RANGE_HINT));
                                crate::widgets::field_probe::note(
                                    &format!(
                                        "pub.row.{}.{}",
                                        if is_min { "min" } else { "max" },
                                        id.0
                                    ),
                                    rr.rect,
                                );
                                if rr.changed() {
                                    edited = true;
                                }
                            }
                            // «к пределам цели» — снять сужение целиком.
                            // Без этой кнопки суженную ручку нельзя было
                            // бы вернуть в исходное состояние иначе как
                            // сняв и объявив заново (правило 9).
                            let narrowed = !list[i].range.is_empty();
                            let rst = ui
                                .add_enabled(narrowed, egui::Button::new(tr(k::pubp::RANGE_RESET)));
                            crate::widgets::hint(&rst, tr(k::pubp::RANGE_RESET_HINT));
                            crate::widgets::field_probe::note(
                                &format!("pub.row.range_reset.{}", id.0),
                                rst.rect,
                            );
                            if rst.clicked() {
                                list[i].range = PubRange::default();
                                edited = true;
                            } else if edited {
                                list[i].range = PubRange::narrowed(&spec, lo, hi);
                            }
                            if edited {
                                cmds.push(Command::SetPublished {
                                    param: Box::new(list[i].clone()),
                                });
                            }
                        }
                        let rm = ui.button(crate::icons::CLEAR);
                        crate::widgets::field_probe::note(
                            &format!("pub.row.remove.{}", id.0),
                            rm.rect,
                        );
                        crate::widgets::hint(&rm, tr(k::pubp::UNPUBLISH));
                        if rm.clicked() {
                            confirm_now = Some((id, 0));
                        }
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr(k::pubp::DONE)).clicked() {
                        done = true;
                    }
                    // «Отмена» возвращает набор ТЕМ ЖЕ токеном жеста —
                    // вся сессия окна остаётся одним шагом истории
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });

        // --- подтверждение снятия -----------------------------------------
        if let Some((id, _)) = confirm_now {
            w.confirm = Some((id, self.override_uses(w.seq, id)));
        }
        if let Some((id, n)) = w.confirm {
            let name = list.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
            let mut close = false;
            egui::Window::new(tr(k::pubp::UNPUBLISH_TITLE))
                .id(egui::Id::new("pub_unpublish"))
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    // ЧИСЛО НАЗЫВАЕТСЯ ВСЛУХ: снятие убивает переопределения
                    // во всех местах дерева, и узнать об этом постфактум
                    // было бы потерей работы
                    ui.label(crate::tf!(k::pubp::UNPUBLISH_ASK, &name, n));
                    ui.horizontal(|ui| {
                        let r = ui.button(tr(k::pubp::UNPUBLISH));
                        crate::widgets::field_probe::note("pub.unpublish.ok", r.rect);
                        if r.clicked() {
                            do_unpublish = Some(id);
                            close = true;
                        }
                        if ui.button(tr(k::fx::CANCEL)).clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                w.confirm = None;
            }
        }

        for c in cmds {
            self.apply_coalesced(c, w.gesture);
        }
        if let Some(o) = order {
            self.apply_coalesced(Command::ReorderPublished { order: o }, w.gesture);
        }
        if let Some(id) = do_unpublish {
            self.apply_coalesced(Command::UnpublishParam { id }, w.gesture);
        }
        if cancel {
            // тем же токеном: возврат не заводит второго шага истории.
            // Переопределения снятых ручек этим не воскресают — о них
            // подтверждение сказало прямо, а полный возврат — Ctrl+Z.
            let now = self.published_of(w.seq);
            for p in &w.backup {
                if now.iter().any(|x| x.id == p.id) {
                    self.apply_coalesced(
                        Command::SetPublished { param: Box::new(p.clone()) },
                        w.gesture,
                    );
                } else {
                    self.apply_coalesced(
                        Command::PublishParam { param: Box::new(p.clone()) },
                        w.gesture,
                    );
                }
            }
            let o: Vec<PubId> = self
                .published_of(w.seq)
                .iter()
                .map(|p| p.id)
                .filter(|id| w.backup.iter().any(|p| p.id == *id))
                .collect();
            if o.len() == self.published_of(w.seq).len() {
                self.apply_coalesced(Command::ReorderPublished { order: o }, w.gesture);
            }
            return;
        }
        if !open || done {
            return;
        }
        w.open = open;
        self.published_win = Some(w);
    }

    /// Окно «Параметры композиции» экземпляра — сторона ПОТРЕБИТЕЛЯ.
    pub(crate) fn draw_inst_params(&mut self, ctx: &egui::Context) {
        let mut wins = std::mem::take(&mut self.inst_params);
        for w in &mut wins {
            self.draw_one_inst_params(ctx, w);
        }
        wins.retain(|w| w.open);
        // окна, открытые за время отрисовки, не теряются
        self.inst_params.extend(wins);
    }

    fn draw_one_inst_params(&mut self, ctx: &egui::Context, w: &mut InstParamsWin) {
        let doc = self.model.document();
        let Some(inst) = self.model.clip(w.clip).cloned() else {
            w.open = false;
            return;
        };
        let list = self.published_of(w.seq);
        if list.is_empty() {
            w.open = false;
            return;
        }
        // Домен кривой переопределения — МАТЕРИАЛ ЭКЗЕМПЛЯРА (оно же
        // время ленты): ровно то, что модель потом переводит в материал
        // цели тем же `clip_material_at` (§469).
        let frame_step = 90_000 / doc.settings.fps.max(1) as i64;
        let at = kt_model::clip_material_at(
            inst.source_in,
            inst.start,
            inst.duration,
            inst.reversed,
            frame_step,
            self.player.transport.playhead(),
        );
        // Ось графика — МАТЕРИАЛ ЭКЗЕМПЛЯРА, тот же домен, в котором
        // садится ключ; у зеркального экземпляра концы меняются местами,
        // поэтому min/max, а не «первый-последний».
        let mat_range = {
            let a = kt_model::clip_material_at(
                inst.source_in,
                inst.start,
                inst.duration,
                inst.reversed,
                frame_step,
                inst.start,
            );
            let b = kt_model::clip_material_at(
                inst.source_in,
                inst.start,
                inst.duration,
                inst.reversed,
                frame_step,
                inst.start + inst.duration,
            );
            (kt_time::Tick(a.0.min(b.0)), kt_time::Tick(a.0.max(b.0).max(a.0.min(b.0) + 1)))
        };
        let tl_view = self.tl_view();
        let axis_linked = self.auto_axis_linked;
        let playhead = self.player.transport.playhead();
        let media: Vec<(MediaId, String)> = doc
            .media
            .iter()
            // ЛЕНТА значением не предлагается: у секвенции нет `MediaRef`,
            // и подмена источника лентой — это другой механизм (Р.50)
            .filter(|m| doc.sequence(m.id).is_none())
            .map(|m| (m.id, m.path.clone()))
            .collect();
        // Паспорта ручек: пределы ЦЕЛИ, суженные автором, и ГОТОВАЯ
        // подпись — имя автора (Р.59: непустой `label` у динамического
        // владельца рисуется дословно, мимо каталога, t684).
        let specs: Vec<kt_model::ParamSpec> = list
            .iter()
            .map(|p| {
                let base = doc.addr_spec(Some(w.seq), &p.target);
                let mut sp = base.unwrap_or(kt_model::ParamSpec {
                    name: Default::default(),
                    ty: p.ty.clone(),
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    step: 0.01,
                    unit: kt_model::ParamUnit::None,
                    kind: kt_model::ParamKind::Smooth,
                    label: Default::default(),
                });
                sp.min = p.range.min.unwrap_or(sp.min).max(sp.min);
                sp.max = p.range.max.unwrap_or(sp.max).min(sp.max);
                sp.name = std::borrow::Cow::Owned(pub_key(p.id));
                sp.label = std::borrow::Cow::Owned(p.name.clone());
                sp
            })
            .collect();
        // Кривые ВСЕХ ручек — одним рядом: виджет рисует кривые одного
        // носителя, а носитель здесь — экземпляр. В модели они лежат по
        // переопределениям, поэтому имя на время окна подменяется на
        // номер ручки и возвращается при записи.
        let mut auto: Vec<kt_model::ParamCurve> = Vec::new();
        for p in &list {
            if let Some(o) = inst.overrides.iter().find(|o| o.pub_id == p.id)
                && let Some(c) = o.curves.iter().find(|c| c.param == p.target.param)
            {
                let mut c = c.clone();
                c.param = pub_key(p.id);
                auto.push(c);
            }
        }
        let auto_before = auto.clone();
        let (over_n, total) = self.instance_override_counts(&inst);
        let title = format!(
            "{} — {}",
            tr(k::pubp::INST_TITLE),
            doc.sequence(w.seq).map(|s| s.name.clone()).unwrap_or_default()
        );
        let mut ops: Vec<Command> = Vec::new();
        // что править в модели по итогам кадра: ручки с изменённым
        // значением, ручки со сброшенным переопределением и действующие
        // значения всех строк (кривой нужна база, если её заводят)
        let mut dirty: Vec<PubId> = Vec::new();
        let mut drop_curve: Vec<PubId> = Vec::new();
        let mut values: Vec<ParamValue> = Vec::new();
        let mut axis_req = crate::automation::AxisReq::default();
        let mut seek: Option<kt_time::Tick> = None;
        let mut hover_out: Option<String> = None;
        let mut cancel = false;
        let mut done = false;
        let mut open = w.open;
        let mut query = std::mem::take(&mut w.query);
        let view = &mut w.view;
        egui::Window::new(title)
            .id(egui::Id::new(("inst_params", w.clip.0)))
            .open(&mut open)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(crate::tf!(k::pubp::OVERRIDDEN, over_n, total))
                        .weak()
                        .small(),
                );
                egui::Grid::new(("inst_params_grid", w.clip.0))
                    .num_columns(4)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        for (i, p) in list.iter().enumerate() {
                            let ov = inst.overrides.iter().find(|o| o.pub_id == p.id).cloned();
                            let tmpl = doc.published_value(w.seq, p.id);
                            let cur = ov
                                .as_ref()
                                .map(|o| o.value.clone())
                                .or_else(|| tmpl.clone())
                                .unwrap_or(ParamValue::Num(0.0));
                            // СТРОКА БЕЗ ПЕРЕОПРЕДЕЛЕНИЯ — приглушённая:
                            // видно, что экземпляр от ленты пока ничем не
                            // отличается (правило 13: цвет вместо колонки)
                            let dim = ov.is_none();
                            // ИМЯ АВТОРА — как есть, мимо каталога (t684)
                            let name = egui::RichText::new(&p.name);
                            let name = if dim { name.weak() } else { name };
                            ui.label(name);
                            let mut next = cur.clone();
                            let mut bind = crate::widgets::CurveBind {
                                curves: &mut auto,
                                sel: view.sel,
                                at,
                            };
                            let changed = draw_value_row(
                                ui,
                                p,
                                &specs[i],
                                &mut next,
                                &mut bind,
                                &media,
                                &mut query,
                                doc.addr_type(Some(w.seq), &p.target),
                            );
                            // значение ШАБЛОНА рядом — читается живьём,
                            // копии в файле нет (решение владельца)
                            let t = tmpl
                                .as_ref()
                                .map(|v| value_text(v, &|m| media_name(&media, m)))
                                .unwrap_or_else(|| tr(k::pubp::INST_TARGET_GONE).to_string());
                            ui.label(
                                egui::RichText::new(crate::tf!(k::pubp::TEMPLATE, &t))
                                    .weak()
                                    .small(),
                            );
                            let reset =
                                ui.add_enabled(ov.is_some(), egui::Button::new(tr(k::pubp::RESET)));
                            // ДОСТУПНОЕ ИМЯ СОСТАВНОЕ, видимая подпись
                            // короткая (тот же приём, что у ячеек пикера
                            // Р.54): кнопок сброса в окне столько же,
                            // сколько ручек, и «Сбросить» их не различает
                            // ни для скринридера, ни для харнесса.
                            {
                                let name = format!("{} {}", tr(k::pubp::RESET), p.name);
                                let on = ov.is_some();
                                reset.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        on,
                                        name.clone(),
                                    )
                                });
                            }
                            crate::widgets::field_probe::note(
                                &format!("inst.reset.{}", p.id.0),
                                reset.rect,
                            );
                            crate::widgets::hint(&reset, tr(k::pubp::RESET_HINT));
                            if reset.clicked() {
                                ops.push(Command::ClearOverride { id: w.clip, pub_id: p.id });
                                drop_curve.push(p.id);
                            } else if changed {
                                dirty.push(p.id);
                            }
                            values.push(next);
                            ui.end_row();
                        }
                    });

                // ---- АВТОМАТИЗАЦИЯ ------------------------------------
                //
                // Тот же виджет, что у слота эффекта и у генератора
                // (решение Р.34 «один механизм анимации на всё»). До
                // Р.60 окно правило кривые ВСЛЕПУЮ: безымянный глиф
                // заводил кривую, и каждая правка поля клала ключ по
                // курсору, а посмотреть на кривую было негде.
                let curvable: Vec<usize> =
                    (0..list.len()).filter(|&i| list[i].ty.curvable()).collect();
                if !curvable.is_empty() {
                    ui.separator();
                    ui.horizontal(|ui| {
                        // заголовок секции объясняет, ЧЬИ это кривые:
                        // ось у них — время экземпляра, а не ленты
                        let ttl = ui.label(tr(k::pubp::AUTO_TITLE));
                        crate::widgets::hint(&ttl, tr(k::pubp::AUTO_HINT));
                        let free: Vec<usize> = curvable
                            .iter()
                            .copied()
                            .filter(|&i| !auto.iter().any(|c| c.param == pub_key(list[i].id)))
                            .collect();
                        if !free.is_empty() {
                            // МЕНЮ, а не глиф: у действия обязано быть
                            // имя — и в интерфейсе, и для харнесса
                            let mb = ui.menu_button(tr(k::fx::AUTO_ADD), |ui| {
                                for &i in &free {
                                    // пункт — ИМЯ АВТОРА дословно (t684)
                                    let item = ui.button(&list[i].name);
                                    crate::widgets::field_probe::note(
                                        &format!("inst.auto_add.{}", list[i].id.0),
                                        item.rect,
                                    );
                                    if item.clicked() {
                                        // стартовая кривая — константа
                                        // действующего значения на весь
                                        // материал экземпляра, как у слота
                                        let v0 = values
                                            .get(i)
                                            .and_then(|v| v.as_num())
                                            .unwrap_or(specs[i].default);
                                        auto.push(kt_model::ParamCurve {
                                            param: pub_key(list[i].id),
                                            keys: vec![
                                                kt_model::AutoKey::at(mat_range.0, v0),
                                                kt_model::AutoKey::at(mat_range.1, v0),
                                            ],
                                            source: Default::default(),
                                        });
                                        view.active = auto.len() - 1;
                                        view.sel = None;
                                        dirty.push(list[i].id);
                                        ui.close();
                                    }
                                }
                            });
                            crate::widgets::hint(&mb.response, tr(k::pubp::AUTO_ADD_HINT));
                            crate::widgets::field_probe::note("inst.auto_add", mb.response.rect);
                        }
                    });
                    if !auto.is_empty() {
                        let to_tl = |t: kt_time::Tick| {
                            kt_model::clip_timeline_at(
                                inst.source_in,
                                inst.start,
                                inst.duration,
                                inst.reversed,
                                frame_step,
                                t,
                            )
                        };
                        let to_mat = |t: kt_time::Tick| {
                            kt_model::clip_material_at(
                                inst.source_in,
                                inst.start,
                                inst.duration,
                                inst.reversed,
                                frame_step,
                                t,
                            )
                        };
                        let actx = crate::automation::AutoCtx {
                            specs: &specs,
                            range: mat_range,
                            view: crate::automation::view_in_curve(&to_mat, tl_view),
                            to_timeline: &to_tl,
                            to_curve: &to_mat,
                            frame_step,
                            linked: axis_linked,
                        };
                        let top = ui.cursor().min;
                        let res = crate::automation::draw(
                            ui,
                            ("instauto", w.clip.0),
                            &actx,
                            &mut auto,
                            view,
                            playhead,
                            &mut hover_out,
                        );
                        crate::widgets::field_probe::note(
                            "inst.auto",
                            egui::Rect::from_min_max(top, ui.cursor().min),
                        );
                        if res.changed {
                            // какая ручка поехала, виджет не сообщает —
                            // сверяем со снимком начала кадра
                            for p in &list {
                                let k = pub_key(p.id);
                                let now = auto.iter().find(|c| c.param == k);
                                let was = auto_before.iter().find(|c| c.param == k);
                                if now != was {
                                    dirty.push(p.id);
                                }
                            }
                        }
                        seek = res.seek;
                        axis_req = axis_req.merge(&res);
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr(k::pubp::DONE)).clicked() {
                        done = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        w.query = query;
        // Одна команда на ручку: значение строки плюс её кривая, имя
        // которой возвращается ЦЕЛЕВЫМ — таким его ждёт `apply_overrides`.
        dirty.retain(|id| !drop_curve.contains(id));
        dirty.dedup();
        for (i, p) in list.iter().enumerate() {
            if !dirty.contains(&p.id) {
                continue;
            }
            let key = pub_key(p.id);
            let curves: Vec<kt_model::ParamCurve> = auto
                .iter()
                .filter(|c| c.param == key)
                .map(|c| {
                    let mut c = c.clone();
                    c.param = p.target.param.clone();
                    c
                })
                .collect();
            let value = values.get(i).cloned().unwrap_or(ParamValue::Num(0.0));
            ops.push(Command::SetOverride {
                id: w.clip,
                ov: Box::new(Override { pub_id: p.id, value, curves }),
            });
        }
        if let Some(t) = seek {
            self.player.seek(t);
        }
        self.apply_axis_req(axis_req);
        if let Some(h) = hover_out {
            crate::widgets::hint_sink::put(&h);
        }
        // ВСЯ сессия окна — один шаг истории: токен жеста общий на все
        // строки и на откат
        for c in ops {
            self.apply_coalesced(c, w.gesture);
        }
        if cancel {
            let now = self.model.clip(w.clip).map(|c| c.overrides.clone()).unwrap_or_default();
            for o in &now {
                if !w.backup.iter().any(|b| b.pub_id == o.pub_id) {
                    self.apply_coalesced(
                        Command::ClearOverride { id: w.clip, pub_id: o.pub_id },
                        w.gesture,
                    );
                }
            }
            for b in &w.backup.clone() {
                self.apply_coalesced(
                    Command::SetOverride { id: w.clip, ov: Box::new(b.clone()) },
                    w.gesture,
                );
            }
            w.open = false;
            return;
        }
        w.open = open && !done;
    }
}

fn media_name(media: &[(MediaId, String)], id: MediaId) -> String {
    media
        .iter()
        .find(|(m, _)| *m == id)
        .map(|(_, p)| {
            std::path::Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.clone())
        })
        .unwrap_or_else(|| format!("#{}", id.0))
}

/// Одна строка окна потребителя — виджет по ТИПУ ручки.
///
/// Поле правит ровно то, что показывает (то же правило, что у
/// `param_row_bound`): есть кривая — правится её ключ в домене
/// экземпляра, нет — само значение.
#[allow(clippy::too_many_arguments)]
fn draw_value_row(
    ui: &mut egui::Ui,
    p: &PublishedParam,
    spec: &kt_model::ParamSpec,
    v: &mut ParamValue,
    bind: &mut crate::widgets::CurveBind<'_>,
    media: &[(MediaId, String)],
    query: &mut String,
    live_ty: Option<ParamType>,
) -> bool {
    let probe = format!("inst.value.{}", p.id.0);
    let key = pub_key(p.id);
    let mut changed = false;
    match v {
        ParamValue::Num(x) => {
            // ЕДИНСТВЕННОЕ место, где решается «ключ или константа»:
            // ключ по курсору пишется, только когда кривая ЗАВЕДЕНА и
            // НАРИСОВАНА (правило Р.60). Раньше кривую заводил безымянный
            // глиф, и окно превращалось в скрытый режим записи.
            let ci = bind.curves.iter().position(|c| c.param == key);
            let mut shown = match ci {
                Some(i) => {
                    let sel = match bind.sel {
                        Some((c, k)) if c == i && k < bind.curves[i].keys.len() => Some(k),
                        _ => None,
                    };
                    crate::widgets::bound_value(&bind.curves[i], sel, bind.at)
                }
                None => *x,
            };
            // пределы ЦЕЛИ, суженные автором, — оба уже в паспорте
            let step = if spec.step > 0.0 { spec.step } else { 0.05 };
            let r = ui.add(
                egui::DragValue::new(&mut shown).range(spec.min..=spec.max).speed(step),
            );
            crate::widgets::field_probe::note(&probe, r.rect);
            if r.changed() {
                match ci {
                    // КЛЮЧ садится в домене ЭКЗЕМПЛЯРА — это и есть время
                    // ленты, модель переведёт его в материал цели сама
                    Some(i) => {
                        let sel = match bind.sel {
                            Some((c, k)) if c == i && k < bind.curves[i].keys.len() => Some(k),
                            _ => None,
                        };
                        let at = bind.at;
                        crate::widgets::bound_apply(&mut bind.curves[i], sel, at, shown);
                    }
                    None => *x = shown,
                }
                changed = true;
            }
        }
        ParamValue::Text(s) => {
            let r = ui.add(egui::TextEdit::singleline(s).desired_width(200.0));
            crate::widgets::field_probe::note(&probe, r.rect);
            changed = r.changed();
        }
        ParamValue::Color(rgba) => {
            let mut col = egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
            let r = ui.color_edit_button_srgba(&mut col);
            crate::widgets::field_probe::note(&probe, r.rect);
            if r.changed() {
                *rgba = col.to_srgba_unmultiplied();
                changed = true;
            }
        }
        ParamValue::Flag(b) => {
            let r = ui.checkbox(b, "");
            crate::widgets::field_probe::note(&probe, r.rect);
            changed = r.changed();
        }
        ParamValue::Choice(sel) => {
            // Варианты принадлежат ЦЕЛИ и читаются у неё живьём: копия в
            // файле молча разошлась бы с новой сборкой (§469).
            let variants: Vec<String> = match live_ty {
                Some(ParamType::Choice { variants }) => {
                    variants.iter().map(|s| s.to_string()).collect()
                }
                _ => vec![sel.clone()],
            };
            let r = egui::ComboBox::from_id_salt(("inst-choice", p.id.0))
                .selected_text(sel.clone())
                .show_ui(ui, |ui| {
                    for var in &variants {
                        if ui.selectable_label(var == sel, var).clicked() {
                            *sel = var.clone();
                            changed = true;
                        }
                    }
                });
            crate::widgets::field_probe::note(&probe, r.response.rect);
        }
        ParamValue::Media(m) => {
            ui.label(media_name(media, *m));
            if let Some(Some(pick)) =
                crate::widgets::media_picker(ui, ("inst-media", p.id.0), media, query)
            {
                *m = pick;
                changed = true;
            }
        }
    }
    changed
}
